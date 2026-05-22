//! PCM/WAV helpers. Gemini TTS preview returns signed 16-bit LE PCM,
//! mono, at 24 kHz. We wrap it in a minimal WAV container so any decoder
//! (FFmpeg, audio players) can read it.

pub const GEMINI_TTS_SAMPLE_RATE: u32 = 24_000;
pub const GEMINI_TTS_NUM_CHANNELS: u16 = 1;
pub const GEMINI_TTS_BITS_PER_SAMPLE: u16 = 16;

/// Wraps raw PCM in a minimal RIFF/WAVE container.
pub fn pcm_to_wav(pcm: &[u8]) -> Vec<u8> {
    let sample_rate = GEMINI_TTS_SAMPLE_RATE;
    let num_channels = GEMINI_TTS_NUM_CHANNELS;
    let bits_per_sample = GEMINI_TTS_BITS_PER_SAMPLE;

    let byte_rate = sample_rate * u32::from(num_channels) * u32::from(bits_per_sample / 8);
    let block_align = num_channels * (bits_per_sample / 8);
    let data_size = u32::try_from(pcm.len()).unwrap_or(u32::MAX);
    let total_size = 36 + data_size;

    let mut out = Vec::with_capacity(44 + pcm.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&total_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // PCM chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM format = 1
    out.extend_from_slice(&num_channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&bits_per_sample.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_size.to_le_bytes());
    out.extend_from_slice(pcm);
    out
}

/// Approximate duration of a raw PCM buffer, in milliseconds.
pub fn pcm_duration_ms(pcm: &[u8]) -> u64 {
    let bytes_per_sample =
        usize::from(GEMINI_TTS_NUM_CHANNELS) * usize::from(GEMINI_TTS_BITS_PER_SAMPLE / 8);
    if bytes_per_sample == 0 {
        return 0;
    }
    let samples = pcm.len() / bytes_per_sample;
    (samples as u64 * 1000) / u64::from(GEMINI_TTS_SAMPLE_RATE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_is_44_bytes() {
        let wav = pcm_to_wav(&[0u8; 0]);
        assert_eq!(wav.len(), 44);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(&wav[36..40], b"data");
    }

    #[test]
    fn duration_matches_sample_count() {
        // 24_000 samples * 2 bytes/sample = 48_000 bytes = 1 second.
        let pcm = vec![0u8; 48_000];
        assert_eq!(pcm_duration_ms(&pcm), 1000);
    }
}
