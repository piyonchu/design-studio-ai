//! Audio generation behind the same provider boundary as images, so the
//! transport can be swapped without touching routes. Mirrors `images.rs`.
//!
//! Modes:
//!   - `AUDIO_MOCK=true` (default) → a deterministic synthesized WAV (a short
//!     chime whose pitch derives from the prompt). No network, no cost — this
//!     is what dev + CI run on.
//!   - otherwise → 503: no hosted audio provider is wired yet. The boundary is
//!     here so adding one (ElevenLabs / Stability Audio / etc.) is a localized
//!     change, exactly like the image client.

use std::f32::consts::PI;

use crate::error::AppError;

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
    Err(AppError::ServiceUnavailable(
        "audio generation is not configured (set AUDIO_MOCK=true for placeholders)".into(),
    ))
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
    use super::{generate_audio, wav_container};

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
