//! AudioSource playback owner. World state is projected here; devices stay behind AudioOutput.
use crate::audio::{AudioSource, DecodedAudioClip};
use crate::components::ComponentTypeId;
use crate::ids::{EntityId, RuntimeEntityId};
use crate::query::QuerySpec;
use crate::runtime_asset::RuntimeAssetHandle;
use crate::runtime_asset_loader::RuntimeAssetLoader;
use crate::world::World;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioSourceAction {
    Play,
    Stop,
    SetPaused(bool),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioSourceCommand {
    pub entity_id: EntityId,
    pub runtime_id: RuntimeEntityId,
    pub action: AudioSourceAction,
}

/// Internal output seam: Windows uses a real device, headless tests explicitly do not.
pub trait AudioOutput {
    fn kind(&self) -> &'static str;
    fn prepare(&mut self) -> Result<(), String>;
    fn play(
        &mut self,
        id: RuntimeEntityId,
        clip: Arc<DecodedAudioClip>,
        volume: f32,
        paused: bool,
    ) -> Result<(), String>;
    fn stop(&mut self, id: RuntimeEntityId);
    fn set_paused(&mut self, id: RuntimeEntityId, paused: bool);
    fn finished(&self, id: RuntimeEntityId) -> bool;
    fn take_error(&mut self) -> Option<String> {
        None
    }
}

/// Control-only output; never qualifies a device or audible playback.
#[derive(Default)]
pub struct HeadlessAudioOutput;
impl AudioOutput for HeadlessAudioOutput {
    fn kind(&self) -> &'static str {
        "headless-control-only"
    }
    fn prepare(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn play(
        &mut self,
        _: RuntimeEntityId,
        _: Arc<DecodedAudioClip>,
        _: f32,
        _: bool,
    ) -> Result<(), String> {
        Ok(())
    }
    fn stop(&mut self, _: RuntimeEntityId) {}
    fn set_paused(&mut self, _: RuntimeEntityId, _: bool) {}
    fn finished(&self, _: RuntimeEntityId) -> bool {
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDiagnostic {
    pub code: String,
    pub entity_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTraceEvent {
    pub frame_index: u64,
    pub elapsed_ms: u64,
    pub entity_id: String,
    pub action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAudioReport {
    pub output_kind: String,
    pub source_count: usize,
    pub playing_count: usize,
    pub paused_count: usize,
    pub play_count: u64,
    pub stop_count: u64,
    pub retired_count: u64,
    pub rejected_command_count: u64,
    pub diagnostics: Vec<AudioDiagnostic>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trace: Vec<AudioTraceEvent>,
}

struct SourcePlayback {
    entity_id: EntityId,
    config: AudioSource,
    asset_handle: RuntimeAssetHandle,
    clip: Arc<DecodedAudioClip>,
    paused: bool,
    playing: bool,
}

pub struct RuntimeAudio {
    output: Box<dyn AudioOutput>,
    sources: BTreeMap<RuntimeEntityId, SourcePlayback>,
    diagnostics: Vec<AudioDiagnostic>,
    trace: Vec<AudioTraceEvent>,
    trace_enabled: bool,
    started: Instant,
    play_count: u64,
    stop_count: u64,
    retired_count: u64,
    rejected_command_count: u64,
}

impl Default for RuntimeAudio {
    fn default() -> Self {
        Self::new(Box::<HeadlessAudioOutput>::default())
    }
}

impl RuntimeAudio {
    pub fn new(output: Box<dyn AudioOutput>) -> Self {
        Self {
            output,
            sources: BTreeMap::new(),
            diagnostics: Vec::new(),
            trace: Vec::new(),
            trace_enabled: false,
            started: Instant::now(),
            play_count: 0,
            stop_count: 0,
            retired_count: 0,
            rejected_command_count: 0,
        }
    }

    pub fn set_trace_enabled(&mut self, enabled: bool) {
        self.trace_enabled = enabled;
    }

    pub fn report(&self) -> RuntimeAudioReport {
        RuntimeAudioReport {
            output_kind: self.output.kind().into(),
            source_count: self.sources.len(),
            playing_count: self
                .sources
                .values()
                .filter(|s| s.playing && !s.paused)
                .count(),
            paused_count: self.sources.values().filter(|s| s.paused).count(),
            play_count: self.play_count,
            stop_count: self.stop_count,
            retired_count: self.retired_count,
            rejected_command_count: self.rejected_command_count,
            diagnostics: self.diagnostics.clone(),
            trace: self.trace.clone(),
        }
    }

    pub fn has_errors(&self) -> bool {
        !self.diagnostics.is_empty()
    }

    fn error(&mut self, code: &str, entity: Option<&EntityId>, message: String) {
        if self.diagnostics.len() < 16 {
            self.diagnostics.push(AudioDiagnostic {
                code: code.into(),
                entity_id: entity.map(ToString::to_string),
                message,
            });
        }
    }

    fn event(&mut self, frame: u64, entity: &EntityId, action: &str) {
        if self.trace_enabled && self.trace.len() < 256 {
            self.trace.push(AudioTraceEvent {
                frame_index: frame,
                elapsed_ms: self.started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
                entity_id: entity.to_string(),
                action: action.into(),
            });
        }
    }

    /// Called once after each control update, including zero-fixed-tick frames.
    pub fn update(
        &mut self,
        world: &World,
        mut loader: Option<&mut RuntimeAssetLoader>,
        commands: Vec<AudioSourceCommand>,
        frame: u64,
    ) {
        if let Some(error) = self.output.take_error() {
            self.error("audio.output_failed", None, error);
        }
        let live = world
            .query_entities(&QuerySpec::all([ComponentTypeId::audio_source()]))
            .into_iter()
            .filter_map(|entity| {
                Some((
                    world.runtime_id_for_source(&entity)?,
                    (entity.clone(), world.audio_source(&entity)?.clone()),
                ))
            })
            .collect::<BTreeMap<_, _>>();

        let retired = self
            .sources
            .iter()
            .filter_map(|(id, source)| {
                (!live
                    .get(id)
                    .is_some_and(|(_, config)| config == &source.config)
                    || self.has_errors())
                .then_some(*id)
            })
            .collect::<Vec<_>>();
        for id in retired {
            if let Some(source) = self.sources.remove(&id) {
                self.output.stop(id);
                if let Some(loader) = loader.as_deref_mut() {
                    let _ = loader.release(&source.asset_handle);
                }
                self.retired_count += 1;
                self.event(frame, &source.entity_id, "retire");
            }
        }
        if self.has_errors() {
            return;
        }
        if !live.is_empty() {
            if let Err(error) = self.output.prepare() {
                self.error("audio.output_unavailable", None, error);
                return;
            }
        }
        for (id, (entity, config)) in live {
            if self.sources.contains_key(&id) {
                continue;
            }
            let Some(loader) = loader.as_deref_mut() else {
                self.error(
                    "audio.package_context_missing",
                    Some(&entity),
                    "AudioSource requires the RuntimePackage asset loader.".into(),
                );
                return;
            };
            let handle = match loader.load(&config.clip_ref) {
                Ok(handle) => handle,
                Err(()) => {
                    self.error(
                        "audio.clip_load_failed",
                        Some(&entity),
                        format!(
                            "Cannot load AudioSource clip {} from RuntimePackage.",
                            config.clip_ref.id
                        ),
                    );
                    return;
                }
            };
            let Some(clip) = loader.get_audio_clip(&handle) else {
                let _ = loader.release(&handle);
                self.error(
                    "audio.clip_decode_failed",
                    Some(&entity),
                    format!(
                        "AudioSource clip {} has no decoded PCM.",
                        config.clip_ref.id
                    ),
                );
                return;
            };
            self.sources.insert(
                id,
                SourcePlayback {
                    entity_id: entity,
                    config,
                    asset_handle: handle,
                    clip,
                    paused: false,
                    playing: false,
                },
            );
        }
        for (id, source) in &mut self.sources {
            if source.playing && self.output.finished(*id) {
                self.output.stop(*id);
                source.playing = false;
            }
        }
        self.consume(world, commands, frame);
    }

    fn consume(&mut self, world: &World, commands: Vec<AudioSourceCommand>, frame: u64) {
        for command in commands {
            if world.runtime_id_for_source(&command.entity_id) != Some(command.runtime_id)
                || !self.sources.contains_key(&command.runtime_id)
            {
                self.rejected_command_count += 1;
                self.error(
                    "audio.source_retired",
                    Some(&command.entity_id),
                    "Prepared audio command targets a retired or disabled Source.".into(),
                );
                continue;
            }
            let source = self
                .sources
                .get_mut(&command.runtime_id)
                .expect("source checked above");
            let action_name = match command.action {
                AudioSourceAction::Play => {
                    self.output.stop(command.runtime_id);
                    source.playing = false;
                    match self.output.play(
                        command.runtime_id,
                        source.clip.clone(),
                        source.config.volume,
                        source.paused,
                    ) {
                        Ok(()) => {
                            source.playing = true;
                            self.play_count += 1;
                        }
                        Err(error) => {
                            self.error("audio.output_failed", Some(&command.entity_id), error);
                        }
                    }
                    "play"
                }
                AudioSourceAction::Stop => {
                    self.output.stop(command.runtime_id);
                    source.playing = false;
                    self.stop_count += 1;
                    "stop"
                }
                AudioSourceAction::SetPaused(paused) => {
                    source.paused = paused;
                    self.output.set_paused(command.runtime_id, paused);
                    if paused {
                        "pause"
                    } else {
                        "resume"
                    }
                }
            };
            self.event(frame, &command.entity_id, action_name);
        }
    }
}

impl Drop for RuntimeAudio {
    fn drop(&mut self) {
        for id in self.sources.keys() {
            self.output.stop(*id);
        }
    }
}

#[cfg(test)]
#[path = "runtime_audio_tests.rs"]
mod tests;
