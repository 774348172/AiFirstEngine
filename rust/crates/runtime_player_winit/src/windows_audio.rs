//! Windows device adapter for the Runtime AudioSource owner.
use engine_runtime::audio::DecodedAudioClip;
use engine_runtime::ids::RuntimeEntityId;
use engine_runtime::runtime_audio::AudioOutput;
use rodio::{OutputStream, OutputStreamBuilder, Sink, Source};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Default)]
pub(crate) struct WindowsAudioOutput {
    sinks: BTreeMap<RuntimeEntityId, Sink>,
    stream: Option<OutputStream>,
    error: Arc<Mutex<Option<String>>>,
}

impl AudioOutput for WindowsAudioOutput {
    fn kind(&self) -> &'static str {
        "windows-device"
    }

    fn prepare(&mut self) -> Result<(), String> {
        if self.stream.is_none() {
            let error = self.error.clone();
            let mut stream = OutputStreamBuilder::from_default_device()
                .map_err(|error| error.to_string())?
                .with_error_callback(move |failure| {
                    if let Ok(mut first) = error.lock() {
                        if first.is_none() {
                            *first = Some(failure.to_string());
                        }
                    }
                })
                .open_stream()
                .map_err(|error| error.to_string())?;
            stream.log_on_drop(false);
            self.stream = Some(stream);
        }
        Ok(())
    }

    fn play(
        &mut self,
        id: RuntimeEntityId,
        clip: Arc<DecodedAudioClip>,
        volume: f32,
        paused: bool,
    ) -> Result<(), String> {
        self.prepare()?;
        self.stop(id);
        let sink = Sink::connect_new(self.stream.as_ref().expect("prepared stream").mixer());
        sink.set_volume(volume);
        if paused {
            sink.pause();
        }
        sink.append(ClipSamples { clip, position: 0 });
        self.sinks.insert(id, sink);
        Ok(())
    }

    fn stop(&mut self, id: RuntimeEntityId) {
        if let Some(sink) = self.sinks.remove(&id) {
            sink.stop();
        }
    }

    fn set_paused(&mut self, id: RuntimeEntityId, paused: bool) {
        if let Some(sink) = self.sinks.get(&id) {
            if paused {
                sink.pause();
            } else {
                sink.play();
            }
        }
    }

    fn finished(&self, id: RuntimeEntityId) -> bool {
        self.sinks.get(&id).is_none_or(Sink::empty)
    }

    fn take_error(&mut self) -> Option<String> {
        self.error.lock().ok().and_then(|mut error| error.take())
    }
}

/// A playback cursor over shared decoded PCM; play never copies or decodes the clip again.
struct ClipSamples {
    clip: Arc<DecodedAudioClip>,
    position: usize,
}

impl Iterator for ClipSamples {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let sample = self.clip.samples.get(self.position).copied()?;
        self.position += 1;
        Some(sample)
    }
}

impl Source for ClipSamples {
    fn current_span_len(&self) -> Option<usize> {
        Some(self.clip.samples.len() - self.position)
    }
    fn channels(&self) -> u16 {
        self.clip.channels
    }
    fn sample_rate(&self) -> u32 {
        self.clip.sample_rate
    }
    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f64(
            self.clip.samples.len() as f64
                / f64::from(self.clip.channels)
                / f64::from(self.clip.sample_rate),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_playback_cursors_share_pcm_but_advance_independently() {
        let clip = Arc::new(DecodedAudioClip {
            channels: 1,
            sample_rate: 44_100,
            samples: Arc::from([0.1, 0.2, 0.3]),
        });
        let mut first = ClipSamples {
            clip: clip.clone(),
            position: 0,
        };
        let mut second = ClipSamples {
            clip: clip.clone(),
            position: 0,
        };
        assert_eq!(first.next(), Some(0.1));
        assert_eq!(first.next(), Some(0.2));
        assert_eq!(second.next(), Some(0.1));
        assert!(Arc::ptr_eq(&first.clip, &second.clip));
        assert_eq!(first.current_span_len(), Some(1));
    }
}
