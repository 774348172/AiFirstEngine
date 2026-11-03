//! Actual device adapter shared by Windows (WASAPI) and Android (AAudio via CPAL).
use engine_runtime::{
    audio::DecodedAudioClip,
    ids::RuntimeEntityId,
    runtime_audio::{AudioOutput, SpatialAudioGains3D, SpatialGainControl, SpatialPcmCursor3D},
};
use rodio::{OutputStream, OutputStreamBuilder, Sink, Source};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};
struct Voice {
    sink: Sink,
    gains: Arc<SpatialGainControl>,
}
#[derive(Default)]
pub(crate) struct DeviceAudioOutput {
    voices: BTreeMap<RuntimeEntityId, Voice>,
    stream: Option<OutputStream>,
    error: Arc<Mutex<Option<String>>>,
}
impl AudioOutput for DeviceAudioOutput {
    fn kind(&self) -> &'static str {
        if cfg!(target_os = "android") {
            "android-aaudio-device"
        } else {
            "windows-device"
        }
    }
    fn prepare(&mut self) -> Result<(), String> {
        if self.stream.is_none() {
            let error = self.error.clone();
            let mut stream = OutputStreamBuilder::from_default_device()
                .map_err(|e| e.to_string())?
                .with_error_callback(move |failure| {
                    if let Ok(mut first) = error.lock() {
                        if first.is_none() {
                            *first = Some(failure.to_string());
                        }
                    }
                })
                .open_stream()
                .map_err(|e| e.to_string())?;
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
        let sink = Sink::connect_new(self.stream.as_ref().unwrap().mixer());
        sink.set_volume(volume);
        if paused {
            sink.pause();
        }
        let gains = Arc::new(SpatialGainControl::default());
        sink.append(ClipSamples(SpatialPcmCursor3D::new(clip, gains.clone())));
        self.voices.insert(id, Voice { sink, gains });
        Ok(())
    }
    fn stop(&mut self, id: RuntimeEntityId) {
        if let Some(voice) = self.voices.remove(&id) {
            voice.sink.stop();
        }
    }
    fn play_spatial(
        &mut self,
        id: RuntimeEntityId,
        clip: Arc<DecodedAudioClip>,
        volume: f32,
        paused: bool,
        initial: Option<SpatialAudioGains3D>,
    ) -> Result<(), String> {
        self.prepare()?;
        self.stop(id);
        let gains = Arc::new(SpatialGainControl::default());
        gains.set(initial)?;
        let sink = Sink::connect_new(self.stream.as_ref().unwrap().mixer());
        sink.set_volume(volume);
        if paused {
            sink.pause();
        }
        sink.append(ClipSamples(SpatialPcmCursor3D::new(clip, gains.clone())));
        self.voices.insert(id, Voice { sink, gains });
        Ok(())
    }
    fn set_paused(&mut self, id: RuntimeEntityId, paused: bool) {
        if let Some(voice) = self.voices.get(&id) {
            if paused {
                voice.sink.pause();
            } else {
                voice.sink.play();
            }
        }
    }
    fn set_spatial(
        &mut self,
        id: RuntimeEntityId,
        gains: Option<SpatialAudioGains3D>,
    ) -> Result<(), String> {
        if let Some(voice) = self.voices.get(&id) {
            voice.gains.set(gains)?;
        }
        Ok(())
    }
    fn reopen(&mut self) -> Result<(), String> {
        for (_, voice) in std::mem::take(&mut self.voices) {
            voice.sink.stop();
        }
        self.stream = None;
        if let Ok(mut error) = self.error.lock() {
            *error = None;
        }
        self.prepare()
    }
    fn finished(&self, id: RuntimeEntityId) -> bool {
        self.voices.get(&id).is_none_or(|voice| voice.sink.empty())
    }
    fn take_error(&mut self) -> Option<String> {
        self.error.lock().ok().and_then(|mut e| e.take())
    }
}
struct ClipSamples(SpatialPcmCursor3D);
impl Iterator for ClipSamples {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        self.0.next()
    }
}
impl Source for ClipSamples {
    fn current_span_len(&self) -> Option<usize> {
        Some(self.0.remaining_samples())
    }
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        self.0.clip.sample_rate
    }
    fn total_duration(&self) -> Option<Duration> {
        Some(self.0.duration())
    }
}
#[cfg(all(test, target_os = "windows"))]
#[path = "device_audio_s5d.rs"]
mod s5d_bm13_device;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn s3d_device_cursor_consumes_atomic_spatial_gains_over_shared_pcm() {
        let clip = Arc::new(DecodedAudioClip {
            channels: 1,
            sample_rate: 48000,
            samples: Arc::from([1., 0.5]),
        });
        let gain = Arc::new(SpatialGainControl::default());
        gain.set(Some(SpatialAudioGains3D {
            left: 0.,
            right: 1.,
        }))
        .unwrap();
        let mut samples = ClipSamples(SpatialPcmCursor3D::new(clip.clone(), gain));
        assert_eq!(samples.channels(), 2);
        assert_eq!(samples.next(), Some(0.));
        assert_eq!(samples.next(), Some(1.));
        assert!(Arc::ptr_eq(&samples.0.clip, &clip));
    }
    #[test]
    #[ignore = "Requires an actual OS audio device; silent PCM only, no audible qualification"]
    fn s3d_real_device_play_pause_reopen_and_retire() {
        let mut output = DeviceAudioOutput::default();
        output.prepare().unwrap();
        let id = RuntimeEntityId {
            index: 1,
            generation: 1,
        };
        let clip = Arc::new(DecodedAudioClip {
            channels: 1,
            sample_rate: 48000,
            samples: vec![0.; 960].into(),
        });
        output.play(id, clip.clone(), 1., true).unwrap();
        output
            .set_spatial(
                id,
                Some(SpatialAudioGains3D {
                    left: 0.2,
                    right: 0.8,
                }),
            )
            .unwrap();
        output.set_paused(id, false);
        output.set_paused(id, true);
        output.stop(id);
        assert!(output.finished(id));
        output.reopen().unwrap();
        output.play(id, clip, 1., false).unwrap();
        output.stop(id);
        assert!(output.voices.is_empty());
    }
}
