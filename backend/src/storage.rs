//! Object storage for generated assets (S3 / MinIO), with an inline fallback.
//!
//! Two modes, chosen at boot from the environment:
//!   - `S3_BUCKET` set → real object storage via `rust-s3`. `S3_ENDPOINT` +
//!     `S3_PATH_STYLE=true` point it at the bundled MinIO for local dev; unset
//!     uses real AWS S3. Credentials come from `AWS_ACCESS_KEY_ID` /
//!     `AWS_SECRET_ACCESS_KEY`. The bucket is created on first boot.
//!   - `S3_BUCKET` empty/unset → `Inline` fallback: callers persist the image
//!     as a `data:` URL in the DB, so the app runs with no object store.

use std::env;

use s3::creds::Credentials;
use s3::{Bucket, BucketConfiguration, Region};

use crate::error::AppError;

pub enum Storage {
    S3 { bucket: Box<Bucket> },
    /// No object store configured — callers store image bytes inline instead.
    Inline,
}

impl Storage {
    /// Build storage from the environment. Never fails on a missing bucket; it
    /// falls back to `Inline` so dev/smoke runs need no object store.
    pub async fn from_env() -> anyhow::Result<Self> {
        let bucket_name = match env::var("S3_BUCKET").ok().filter(|b| !b.trim().is_empty()) {
            Some(b) => b,
            None => {
                tracing::warn!(
                    "S3_BUCKET unset — assets stored inline (data URLs). Set S3_BUCKET (+ MinIO/S3) for object storage."
                );
                return Ok(Storage::Inline);
            }
        };

        let region_name = env::var("AWS_REGION")
            .ok()
            .filter(|r| !r.trim().is_empty())
            .unwrap_or_else(|| "us-east-1".into());
        let endpoint = env::var("S3_ENDPOINT").ok().filter(|e| !e.trim().is_empty());
        // MinIO (and any custom endpoint) needs path-style addressing.
        let path_style = endpoint.is_some()
            || env::var("S3_PATH_STYLE").map(|v| v == "true").unwrap_or(false);

        let region = match &endpoint {
            Some(ep) => Region::Custom {
                region: region_name.clone(),
                endpoint: ep.clone(),
            },
            None => region_name.parse().unwrap_or(Region::UsEast1),
        };

        let creds = Credentials::new(
            env::var("AWS_ACCESS_KEY_ID").ok().as_deref(),
            env::var("AWS_SECRET_ACCESS_KEY").ok().as_deref(),
            None,
            None,
            None,
        )?;

        let open = |name: &str| -> anyhow::Result<Box<Bucket>> {
            let b = Bucket::new(name, region.clone(), creds.clone())?;
            Ok(if path_style { b.with_path_style() } else { b })
        };
        let bucket = open(&bucket_name)?;

        // Create the bucket on first boot (MinIO starts empty). A 409 means it
        // already exists under another caller — treat that as success.
        if !bucket.exists().await.unwrap_or(false) {
            tracing::info!("creating object-storage bucket '{bucket_name}'");
            let resp = Bucket::create_with_path_style(
                &bucket_name,
                region.clone(),
                creds.clone(),
                BucketConfiguration::default(),
            )
            .await
            .map_err(|e| anyhow::anyhow!("bucket create failed: {e}"))?;
            let code = resp.response_code;
            if !(200..300).contains(&code) && code != 409 {
                anyhow::bail!("bucket create failed: HTTP {code} — {}", resp.response_text);
            }
        }

        tracing::info!(
            "object storage ready: bucket '{bucket_name}'{}",
            endpoint.map(|e| format!(" @ {e}")).unwrap_or_default()
        );
        Ok(Storage::S3 { bucket })
    }

    /// True when a real object store is wired up (vs. the inline fallback).
    pub fn configured(&self) -> bool {
        matches!(self, Storage::S3 { .. })
    }

    /// Upload bytes under `key`. Only valid for the S3 backend.
    pub async fn put(&self, key: &str, bytes: &[u8], content_type: &str) -> Result<(), AppError> {
        match self {
            Storage::S3 { bucket } => {
                let resp = bucket
                    .put_object_with_content_type(key, bytes, content_type)
                    .await
                    .map_err(|e| AppError::ServiceUnavailable(format!("storage upload failed: {e}")))?;
                let code = resp.status_code();
                if !(200..300).contains(&code) {
                    return Err(AppError::ServiceUnavailable(format!(
                        "storage upload failed: HTTP {code}"
                    )));
                }
                Ok(())
            }
            Storage::Inline => Err(AppError::Internal("put() on inline storage".into())),
        }
    }

    /// Fetch the bytes for `key`. Only valid for the S3 backend.
    pub async fn get(&self, key: &str) -> Result<Vec<u8>, AppError> {
        match self {
            Storage::S3 { bucket } => {
                let resp = bucket
                    .get_object(key)
                    .await
                    .map_err(|e| AppError::ServiceUnavailable(format!("storage fetch failed: {e}")))?;
                let code = resp.status_code();
                if code == 404 {
                    return Err(AppError::NotFound);
                }
                if !(200..300).contains(&code) {
                    return Err(AppError::ServiceUnavailable(format!(
                        "storage fetch failed: HTTP {code}"
                    )));
                }
                Ok(resp.bytes().to_vec())
            }
            Storage::Inline => Err(AppError::Internal("get() on inline storage".into())),
        }
    }
}
