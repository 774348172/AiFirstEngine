//! Pure AudioSource data and WAV decoding; device playback lives in Runtime Audio.

use crate::runtime_package::RuntimeAssetRef;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AudioSource {
    pub clip_ref: RuntimeAssetRef,
    #[serde(default = "default_volume")]
    pub volume: f32,
}

fn default_volume() -> f32 {
    1.0
}

#[derive(Debug, Clone)]
pub struct DecodedAudioClip {
    pub channels: u16,
    pub sample_rate: u32,
    /// Interleaved linear PCM, normalized from signed 16-bit samples.
    pub samples: Arc<[f32]>,
}

pub fn decode_audio_source(value: &serde_json::Value) -> Result<AudioSource, String> {
    if let Some(reference) = value.get("clipRef").and_then(serde_json::Value::as_object) {
        if reference
            .keys()
            .any(|key| !matches!(key.as_str(), "id" | "type" | "guid" | "sub_asset"))
        {
            return Err("AudioSource clipRef contains an unknown AssetRef field.".into());
        }
    }
    let source: AudioSource = serde_json::from_value(value.clone())
        .map_err(|error| format!("AudioSource fields are invalid: {error}"))?;
    if source.clip_ref.asset_type != "audio"
        || (source.clip_ref.id.trim().is_empty()
            && source
                .clip_ref
                .guid
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty())
        || source
            .clip_ref
            .guid
            .as_ref()
            .is_some_and(|guid| guid.trim().is_empty())
        || source.clip_ref.sub_asset.is_some()
    {
        return Err("AudioSource clipRef must identify a whole audio asset by id or guid.".into());
    }
    if !source.volume.is_finite() || !(0.0..=1.0).contains(&source.volume) {
        return Err("AudioSource volume must be a finite linear gain in [0, 1].".into());
    }
    Ok(source)
}

pub fn decode_audio_wav(bytes: &[u8]) -> Result<DecodedAudioClip, String> {
    // Validate the container's declared extent before delegating format/sample decoding to hound.
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("Audio payload must be a RIFF/WAVE file.".into());
    }
    let riff_size = u32::from_le_bytes(bytes[4..8].try_into().expect("four RIFF size bytes"));
    if u64::from(riff_size) + 8 != bytes.len() as u64 {
        return Err("Audio RIFF size does not match the payload; the WAV may be truncated.".into());
    }
    let mut reader = hound::WavReader::new(Cursor::new(bytes))
        .map_err(|error| format!("Cannot decode audio WAV header: {error}"))?;
    let spec = reader.spec();
    if spec.sample_format != hound::SampleFormat::Int
        || spec.bits_per_sample != 16
        || !matches!(spec.channels, 1 | 2)
        || !matches!(spec.sample_rate, 44_100 | 48_000)
    {
        return Err("Audio WAV must be PCM16, mono/stereo, at 44100 or 48000 Hz.".into());
    }
    let samples = reader
        .samples::<i16>()
        .map(|sample| {
            sample
                .map(|sample| f32::from(sample) / 32768.0)
                .map_err(|error| format!("Cannot decode audio WAV samples: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if samples.is_empty() || samples.len() % usize::from(spec.channels) != 0 {
        return Err("Audio WAV must contain complete, non-empty PCM frames.".into());
    }
    Ok(DecodedAudioClip {
        channels: spec.channels,
        sample_rate: spec.sample_rate,
        samples: samples.into(),
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;

    pub(crate) fn audio_wav_fixture(channels: u16, sample_rate: u32) -> Vec<u8> {
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(
                &mut cursor,
                hound::WavSpec {
                    channels,
                    sample_rate,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
            )
            .unwrap();
            for sample in [-32768_i16, 0, 16384, 32767] {
                writer.write_sample(sample).unwrap();
            }
            writer.finalize().unwrap();
        }
        cursor.into_inner()
    }

    #[test]
    fn audio_wav_decodes_pcm16_and_rejects_wrong_format_or_truncation() {
        for (channels, rate) in [(1, 44_100), (2, 48_000)] {
            let bytes = audio_wav_fixture(channels, rate);
            let clip = decode_audio_wav(&bytes).unwrap();
            assert_eq!((clip.channels, clip.sample_rate), (channels, rate));
            assert_eq!(&clip.samples[..3], &[-1.0, 0.0, 0.5]);
            assert!(decode_audio_wav(&bytes[..bytes.len() - 1]).is_err());
            let mut truncated_data = bytes[..bytes.len() - 2].to_vec();
            let declared_size = (truncated_data.len() as u32 - 8).to_le_bytes();
            truncated_data[4..8].copy_from_slice(&declared_size);
            assert!(decode_audio_wav(&truncated_data).is_err());
        }
        assert!(decode_audio_wav(br#"{"schemaVersion":"audio-asset.v1"}"#).is_err());
        assert!(decode_audio_wav(&audio_wav_fixture(1, 22_050)).is_err());
    }

    #[test]
    fn audio_source_validates_reference_and_gain_without_runtime_state() {
        let valid = json!({"clipRef":{"id":"sound","guid":"guid-sound","type":"audio"}});
        let source = decode_audio_source(&valid).unwrap();
        assert_eq!(source.volume, 1.0);
        assert_eq!(source.clip_ref.guid.as_deref(), Some("guid-sound"));
        for patch in [
            json!({"clipRef":{"id":"sound","type":"texture"}}),
            json!({"clipRef":{"id":"","type":"audio"}}),
            json!({"clipRef":{"id":"sound","type":"audio","sub_asset":"part"}}),
            json!({"clipRef":{"id":"sound","type":"audio"},"volume":1.01}),
            json!({"clipRef":{"id":"sound","type":"audio"},"volume":-0.01}),
            json!({"clipRef":{"id":"sound","type":"audio"},"playing":true}),
        ] {
            assert!(decode_audio_source(&patch).is_err(), "{patch}");
        }
    }
}
