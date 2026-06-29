//! Audio generation behind the same provider boundary as images, so the
//! transport can be swapped without touching routes. Mirrors `images.rs`.
//!
//! Modes:
//!   - `AUDIO_MOCK=true` (default) → a deterministic synthesized WAV (a short
//!     chime whose pitch derives from the prompt). No network, no cost — this
//!     is what dev + CI run on.
//!   - otherwise → **OpenRouter `google/lyria-3-clip-preview`** (music model):
//!     the prompt is framed toward a short game loop / SFX cue, the streamed
//!     base64 MP3 is decoded, then trimmed to `AUDIO_CLIP_SECS` (default 8s, in
//!     the 5–10s the board wants — Lyria's native clip is ~30s). $0.04/clip.

use std::f32::consts::PI;
use std::sync::OnceLock;
use std::time::Duration;

use base64::Engine;
use serde_json::{json, Value};

use crate::error::AppError;

const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const DEFAULT_AUDIO_MODEL: &str = "google/lyria-3-clip-preview";
/// Lyria streams a full ~30s clip; give the generation room before timing out.
const TIMEOUT_SECS: u64 = 120;
/// Default trim length (seconds) — the board wants short 5–10s loops/cues.
const DEFAULT_CLIP_SECS: f32 = 8.0;

/// A generated audio clip ready to persist (raw bytes + MIME).
pub struct GeneratedAudio {
    pub bytes: Vec<u8>,
    pub mime: String,
    /// Clip length in milliseconds — handy for the asset's metadata.
    pub duration_ms: u32,
}

fn audio_mock() -> bool {
    std::env::var("AUDIO_MOCK").map(|v| v.trim().eq_ignore_ascii_case("true")).unwrap_or(true)
}

fn api_key() -> Option<String> {
    std::env::var("OPENROUTER_API_KEY").ok().filter(|k| !k.trim().is_empty())
}

fn audio_model() -> String {
    std::env::var("OPENROUTER_AUDIO_MODEL")
        .ok()
        .filter(|m| !m.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_AUDIO_MODEL.to_string())
}

/// Target trim length in seconds, env-tunable, clamped to a sane 3–30s.
fn clip_secs() -> f32 {
    std::env::var("AUDIO_CLIP_SECS")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(DEFAULT_CLIP_SECS)
        .clamp(3.0, 30.0)
}

fn client() -> &'static reqwest::Client {
    static C: OnceLock<reqwest::Client> = OnceLock::new();
    C.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(TIMEOUT_SECS))
            .build()
            .expect("reqwest client builds")
    })
}

/// Frame the user's text for short, loopable game audio. Lyria is a music model,
/// so nudge toward a tight game-ready loop / SFX-like cue, not a full song.
fn compile_audio_prompt(prompt: &str) -> String {
    format!(
        "Short, seamless looping game audio cue: {}. Keep it tight and minimal — \
         suitable as a game sound effect or background loop, no vocals, consistent tone.",
        prompt.trim()
    )
}

/// FNV-1a over the prompt + index — stable pitch/character per (prompt, n).
fn seed(prompt: &str, n: usize) -> u64 {
    let mut h = 1469598103934665603u64;
    for b in prompt.bytes().chain([b'#', n as u8]) {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}

/// Generate one clip. `n` varies the result so a `count > 1` request yields a
/// distinct set rather than duplicates.
pub async fn generate_audio(prompt: &str, n: usize) -> Result<GeneratedAudio, AppError> {
    if audio_mock() {
        return Ok(mock_clip(prompt, n));
    }
    // Real path: Lyria has no per-call index, so a `count > 1` request just
    // generates several independent (naturally different) clips.
    generate_lyria(prompt).await
}

/// Generate one clip via OpenRouter `google/lyria-3-clip-preview`. Audio output
/// requires `stream: true`; the SSE body carries the MP3 as base64 in
/// `choices[].delta.audio.data`. We accumulate it, decode, and trim to
/// [`clip_secs`] for a short game loop.
async fn generate_lyria(prompt: &str) -> Result<GeneratedAudio, AppError> {
    let key = api_key().ok_or_else(|| {
        AppError::ServiceUnavailable(
            "audio generation not configured: set OPENROUTER_API_KEY, or AUDIO_MOCK=true".into(),
        )
    })?;
    let body = json!({
        "model": audio_model(),
        "modalities": ["audio", "text"],
        "stream": true,
        "messages": [{ "role": "user", "content": compile_audio_prompt(prompt) }],
    });

    let resp = client()
        .post(OPENROUTER_URL)
        .header("authorization", format!("Bearer {key}"))
        .header("content-type", "application/json")
        .header("x-title", "Design Studio AI")
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::ServiceUnavailable(format!("audio request failed: {e}")))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let detail = resp.text().await.unwrap_or_default();
        return Err(AppError::ServiceUnavailable(format!(
            "audio provider returned {status}: {}",
            detail.chars().take(200).collect::<String>()
        )));
    }

    // The stream is finite (clip then `[DONE]`) and ~1 MB, so read it whole and
    // parse the SSE `data:` lines rather than pulling in a streaming parser.
    let text = resp
        .text()
        .await
        .map_err(|e| AppError::ServiceUnavailable(format!("audio stream read failed: {e}")))?;
    let mut b64 = String::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("data:") else { continue };
        let rest = rest.trim();
        if rest.is_empty() || rest == "[DONE]" {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<Value>(rest) {
            if let Some(chunk) = v["choices"][0]["delta"]["audio"]["data"].as_str() {
                b64.push_str(chunk);
            }
        }
    }
    if b64.is_empty() {
        return Err(AppError::ServiceUnavailable("audio provider returned no audio".into()));
    }

    let mp3 = base64::engine::general_purpose::STANDARD
        .decode(b64.as_bytes())
        .map_err(|e| AppError::Internal(format!("audio decode failed: {e}")))?;
    let (clip, duration_ms) = trim_mp3(&mp3, clip_secs());
    Ok(GeneratedAudio { bytes: clip, mime: "audio/mpeg".to_string(), duration_ms })
}

/// Truncate an MP3 to at most `max_secs`, cutting on a frame boundary so the
/// result is a valid (shorter) MP3 — no decoder needed, since MPEG-1 Layer III
/// frames are self-contained. Reads each frame's own bitrate, so VBR is fine.
/// Returns the trimmed bytes + their actual duration in ms. Falls back to the
/// input unchanged if no frames parse.
fn trim_mp3(data: &[u8], max_secs: f32) -> (Vec<u8>, u32) {
    const BITRATES: [u32; 16] =
        [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0];
    const SRATES: [u32; 4] = [44100, 48000, 32000, 0];
    let n = data.len();

    // Skip an ID3v2 tag (syncsafe size) if present, and keep it in the output.
    let header_end = if n > 10 && &data[0..3] == b"ID3" {
        let sz = ((data[6] & 0x7f) as usize) << 21
            | ((data[7] & 0x7f) as usize) << 14
            | ((data[8] & 0x7f) as usize) << 7
            | (data[9] & 0x7f) as usize;
        (10 + sz).min(n)
    } else {
        0
    };

    let mut i = header_end;
    let mut dur = 0.0f32;
    let mut end = header_end;
    while i + 4 <= n {
        // Frame sync: 11 set bits (0xFF then top 3 bits of the next byte).
        if data[i] == 0xFF && (data[i + 1] & 0xE0) == 0xE0 {
            let br = BITRATES[((data[i + 2] >> 4) & 0xF) as usize] * 1000;
            let sr = SRATES[((data[i + 2] >> 2) & 0x3) as usize];
            if br == 0 || sr == 0 {
                i += 1;
                continue;
            }
            let pad = ((data[i + 2] >> 1) & 1) as u32;
            let flen = (144 * br / sr + pad) as usize;
            if flen == 0 {
                i += 1;
                continue;
            }
            dur += 1152.0 / sr as f32;
            i += flen;
            end = i.min(n);
            if dur >= max_secs {
                break;
            }
        } else {
            i += 1;
        }
    }

    if end <= header_end {
        // Nothing parsed — return the original so we never produce a broken clip.
        return (data.to_vec(), (dur * 1000.0) as u32);
    }
    (data[..end].to_vec(), (dur * 1000.0) as u32)
}

/// A short two-tone chime as 16-bit mono PCM WAV. Deterministic from the prompt:
/// the base pitch and interval shift with the seed so different prompts sound
/// different, while the same prompt is reproducible.
fn mock_clip(prompt: &str, n: usize) -> GeneratedAudio {
    let s = seed(prompt, n);
    let sample_rate = 22_050u32;
    let secs = 1.2f32;
    let total = (sample_rate as f32 * secs) as usize;

    let base = 220.0 + (s % 440) as f32; // 220–660 Hz
    let interval = 1.5 + ((s >> 16) % 3) as f32 * 0.25; // a fifth-ish, varied

    let mut pcm: Vec<u8> = Vec::with_capacity(total * 2);
    for i in 0..total {
        let t = i as f32 / sample_rate as f32;
        let env = (1.0 - t / secs).clamp(0.0, 1.0).powf(1.5); // gentle decay
        let wave = (2.0 * PI * base * t).sin() * 0.55
            + (2.0 * PI * base * interval * t).sin() * 0.30;
        let v = (wave * env * i16::MAX as f32 * 0.7) as i16;
        pcm.extend_from_slice(&v.to_le_bytes());
    }

    GeneratedAudio {
        bytes: wav_container(sample_rate, 1, &pcm),
        mime: "audio/wav".to_string(),
        duration_ms: (secs * 1000.0) as u32,
    }
}

/// Wrap 16-bit PCM in a canonical 44-byte WAV header.
fn wav_container(sample_rate: u32, channels: u16, pcm: &[u8]) -> Vec<u8> {
    let bits = 16u16;
    let block_align = channels * bits / 8;
    let byte_rate = sample_rate * block_align as u32;
    let data_len = pcm.len() as u32;

    let mut out = Vec::with_capacity(44 + pcm.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // PCM fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // audio format = PCM
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&bits.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.extend_from_slice(pcm);
    out
}

#[cfg(test)]
mod tests {
    use super::{generate_audio, trim_mp3, wav_container};

    /// One 128 kbps / 44.1 kHz MPEG-1 Layer III frame: header 0xFF 0xFB 0x90 0x00
    /// (bitrate idx 9 = 128k, srate idx 0 = 44100, no pad) → 417 bytes.
    fn mp3_frame() -> Vec<u8> {
        let mut f = vec![0xFF, 0xFB, 0x90, 0x00];
        f.resize(417, 0);
        f
    }

    #[test]
    fn trim_mp3_cuts_on_a_frame_boundary_by_duration() {
        // 40 frames ≈ 1.04s of audio; trim to 0.5s.
        let mut data = Vec::new();
        for _ in 0..40 {
            data.extend(mp3_frame());
        }
        let (clip, ms) = trim_mp3(&data, 0.5);
        // Each frame is 1152/44100 ≈ 26.12 ms → 20 frames cross 500 ms.
        assert_eq!(clip.len(), 20 * 417, "kept whole frames up to the target");
        assert!((480..=560).contains(&ms), "duration ~0.5s, got {ms}ms");
        // Output is a valid prefix (starts on a frame sync).
        assert_eq!(&clip[0..2], &[0xFF, 0xFB]);
    }

    #[test]
    fn trim_mp3_skips_an_id3_tag_and_keeps_it() {
        let mut data = vec![b'I', b'D', b'3', 3, 0, 0, 0, 0, 0, 5]; // syncsafe size = 5
        data.extend([0u8; 5]); // tag body
        let tag_len = data.len();
        for _ in 0..10 {
            data.extend(mp3_frame());
        }
        let (clip, _) = trim_mp3(&data, 0.1); // ~4 frames
        assert_eq!(&clip[0..3], b"ID3", "ID3 header preserved");
        assert!(clip.len() > tag_len && clip.len() < data.len(), "trimmed past the tag");
    }

    #[test]
    fn wav_container_has_riff_wave_header_and_length() {
        let pcm = vec![0u8; 100];
        let w = wav_container(22_050, 1, &pcm);
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(&w[8..12], b"WAVE");
        assert_eq!(w.len(), 44 + pcm.len()); // 44-byte header + payload
    }

    #[tokio::test]
    async fn mock_clip_is_a_valid_nonempty_wav() {
        // AUDIO_MOCK defaults true, so this synthesizes locally (no network).
        let clip = generate_audio("sword clang", 0).await.unwrap();
        assert_eq!(clip.mime, "audio/wav");
        assert_eq!(&clip.bytes[0..4], b"RIFF");
        assert!(clip.bytes.len() > 44 && clip.duration_ms > 0);
    }
}
