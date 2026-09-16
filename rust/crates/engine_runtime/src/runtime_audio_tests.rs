use super::*;
use crate::archetype::ComponentValue;
use crate::components::{Hierarchy, Transform};
use crate::runtime_asset::{
    BundleRecord, CookedAssetRecord, RuntimeAssetIndex, RuntimeAssetRecord,
    RuntimePackageMountTable,
};
use crate::runtime_package::RuntimeAssetRef;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq)]
enum OutputCall {
    Prepare,
    Play(RuntimeEntityId, bool, f32),
    Stop(RuntimeEntityId),
    Pause(RuntimeEntityId, bool),
}

#[derive(Default)]
struct OutputState {
    calls: Vec<OutputCall>,
    voices: BTreeMap<RuntimeEntityId, (Arc<DecodedAudioClip>, bool)>,
    finished: BTreeSet<RuntimeEntityId>,
    prepare_error: Option<String>,
    device_error: Option<String>,
}

struct RecordingOutput(Rc<RefCell<OutputState>>);
impl AudioOutput for RecordingOutput {
    fn kind(&self) -> &'static str {
        "test-control-output"
    }
    fn prepare(&mut self) -> Result<(), String> {
        let mut state = self.0.borrow_mut();
        state.calls.push(OutputCall::Prepare);
        state.prepare_error.clone().map_or(Ok(()), Err)
    }
    fn play(
        &mut self,
        id: RuntimeEntityId,
        clip: Arc<DecodedAudioClip>,
        volume: f32,
        paused: bool,
    ) -> Result<(), String> {
        let mut state = self.0.borrow_mut();
        assert!(
            !state.voices.contains_key(&id),
            "the owner must retire the previous voice before replacement"
        );
        state.calls.push(OutputCall::Play(id, paused, volume));
        state.finished.remove(&id);
        state.voices.insert(id, (clip, paused));
        Ok(())
    }
    fn stop(&mut self, id: RuntimeEntityId) {
        let mut state = self.0.borrow_mut();
        state.calls.push(OutputCall::Stop(id));
        state.voices.remove(&id);
        state.finished.remove(&id);
    }
    fn set_paused(&mut self, id: RuntimeEntityId, paused: bool) {
        let mut state = self.0.borrow_mut();
        state.calls.push(OutputCall::Pause(id, paused));
        if let Some(voice) = state.voices.get_mut(&id) {
            voice.1 = paused;
        }
    }
    fn finished(&self, id: RuntimeEntityId) -> bool {
        self.0.borrow().finished.contains(&id)
    }
    fn take_error(&mut self) -> Option<String> {
        self.0.borrow_mut().device_error.take()
    }
}

fn recording_audio() -> (RuntimeAudio, Rc<RefCell<OutputState>>) {
    let state = Rc::new(RefCell::new(OutputState::default()));
    let audio = RuntimeAudio::new(Box::new(RecordingOutput(state.clone())));
    (audio, state)
}

struct AudioFixture(PathBuf);
impl AudioFixture {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "runtime-audio-owner-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(
            root.join("clip.wav"),
            crate::audio::tests::audio_wav_fixture(1, 44_100),
        )
        .unwrap();
        Self(root)
    }
    fn loader(&self) -> RuntimeAssetLoader {
        RuntimeAssetLoader::new(&self.0, self.index(), self.mount_table())
    }
    fn index(&self) -> RuntimeAssetIndex {
        RuntimeAssetIndex::new(
            vec![RuntimeAssetRecord {
                asset_guid: "guid-audio".into(),
                asset_id: "audio-clip".into(),
                asset_type: "audio".into(),
                sub_asset_id: None,
                version: "1".into(),
                cooked_asset_id: "cooked-audio".into(),
                bundle_id: "startup".into(),
                loader_kind: "audio".into(),
                dependencies: Vec::new(),
                hash: None,
                size: None,
                flags: Vec::new(),
                source_map_debug: None,
            }],
            vec![CookedAssetRecord {
                cooked_asset_id: "cooked-audio".into(),
                bundle_id: "startup".into(),
                path: Some("clip.wav".into()),
                offset: None,
                size: None,
                compression: None,
                hash: None,
            }],
        )
    }
    fn mount_table(&self) -> RuntimePackageMountTable {
        RuntimePackageMountTable::new(vec![BundleRecord {
            bundle_id: "startup".into(),
            mount_id: None,
            uri: "startup".into(),
            hash: None,
            version: None,
            mounted: true,
        }])
    }
}
impl Drop for AudioFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.0.join("clip.wav"));
        let _ = std::fs::remove_dir(&self.0);
    }
}

fn spawn_source(world: &mut World, name: &str) -> RuntimeEntityId {
    let id = EntityId::from(name);
    let runtime_id = world
        .try_spawn_entity(
            id.clone(),
            name,
            "actor",
            true,
            Hierarchy {
                parent_id: None,
                sibling_order: 0,
            },
        )
        .unwrap();
    world
        .try_insert_transform(id.clone(), Transform::identity())
        .unwrap();
    world
        .try_insert_component_value(
            id,
            ComponentTypeId::audio_source(),
            ComponentValue::AudioSource(AudioSource {
                clip_ref: RuntimeAssetRef {
                    id: "audio-clip".into(),
                    asset_type: "audio".into(),
                    guid: Some("guid-audio".into()),
                    sub_asset: None,
                },
                volume: 0.4,
            }),
        )
        .unwrap();
    runtime_id
}

fn command(world: &World, entity: &str, action: AudioSourceAction) -> AudioSourceCommand {
    let entity_id = EntityId::from(entity);
    AudioSourceCommand {
        runtime_id: world.runtime_id_for_source(&entity_id).unwrap(),
        entity_id,
        action,
    }
}

#[test]
fn audio_source_replay_replaces_voice_but_other_sources_keep_independent_progress() {
    let fixture = AudioFixture::new();
    let mut loader = fixture.loader();
    let mut world = World::new();
    let first = spawn_source(&mut world, "first");
    let second = spawn_source(&mut world, "second");
    let (mut audio, output) = recording_audio();
    audio.update(
        &world,
        Some(&mut loader),
        vec![
            command(&world, "first", AudioSourceAction::Play),
            command(&world, "second", AudioSourceAction::Play),
        ],
        1,
    );
    assert_eq!(audio.report().playing_count, 2);
    assert_eq!(loader.decoded_cache_len(), 1);
    {
        let state = output.borrow();
        assert!(Arc::ptr_eq(
            &state.voices[&first].0,
            &state.voices[&second].0
        ));
        assert_eq!(&state.voices[&first].0.samples[..3], &[-1.0, 0.0, 0.5]);
    }
    audio.update(
        &world,
        Some(&mut loader),
        vec![command(&world, "first", AudioSourceAction::Play)],
        2,
    );
    assert_eq!(audio.report().play_count, 3);
    assert_eq!(audio.report().playing_count, 2);
    assert!(output.borrow().voices.contains_key(&second));
    // A presentation/control update without new commands must not restart either voice.
    audio.update(&world, Some(&mut loader), Vec::new(), 3);
    assert_eq!(audio.report().play_count, 3);
    assert_eq!(
        output
            .borrow()
            .calls
            .iter()
            .filter(|c| matches!(c, OutputCall::Play(..)))
            .count(),
        3
    );
}

#[test]
fn audio_paused_play_stop_and_resume_never_create_an_unrequested_voice() {
    let fixture = AudioFixture::new();
    let mut loader = fixture.loader();
    let mut world = World::new();
    let id = spawn_source(&mut world, "source");
    let (mut audio, output) = recording_audio();
    audio.update(
        &world,
        Some(&mut loader),
        vec![
            command(&world, "source", AudioSourceAction::SetPaused(true)),
            command(&world, "source", AudioSourceAction::Play),
        ],
        1,
    );
    assert_eq!(audio.report().playing_count, 0);
    assert_eq!(audio.report().paused_count, 1);
    assert!(output.borrow().voices[&id].1);
    audio.update(
        &world,
        Some(&mut loader),
        vec![command(&world, "source", AudioSourceAction::Stop)],
        2,
    );
    assert!(output.borrow().voices.is_empty());
    assert_eq!(
        audio.report().paused_count,
        1,
        "stop preserves the explicit pause flag"
    );
    audio.update(
        &world,
        Some(&mut loader),
        vec![
            command(&world, "source", AudioSourceAction::SetPaused(false)),
            command(&world, "source", AudioSourceAction::Stop),
        ],
        3,
    );
    assert!(output.borrow().voices.is_empty());
    assert_eq!(audio.report().play_count, 1);
    assert_eq!(audio.report().paused_count, 0);
}

#[test]
fn audio_natural_completion_releases_voice_and_keeps_clip_available_for_next_play() {
    let fixture = AudioFixture::new();
    let mut loader = fixture.loader();
    let mut world = World::new();
    let id = spawn_source(&mut world, "source");
    let (mut audio, output) = recording_audio();
    audio.update(
        &world,
        Some(&mut loader),
        vec![command(&world, "source", AudioSourceAction::Play)],
        1,
    );
    output.borrow_mut().finished.insert(id);
    audio.update(&world, Some(&mut loader), Vec::new(), 2);
    assert!(output.borrow().voices.is_empty());
    assert_eq!(audio.report().playing_count, 0);
    assert_eq!(audio.report().source_count, 1);
    assert_eq!(loader.decoded_cache_len(), 1);
    audio.update(
        &world,
        Some(&mut loader),
        vec![command(&world, "source", AudioSourceAction::Play)],
        3,
    );
    assert_eq!(audio.report().play_count, 2);
    assert!(output.borrow().voices.contains_key(&id));
}

#[test]
fn audio_control_update_retires_disabled_removed_and_destroyed_sources_without_commands() {
    let fixture = AudioFixture::new();
    let mut loader = fixture.loader();
    let mut world = World::new();
    let disabled = spawn_source(&mut world, "disabled");
    spawn_source(&mut world, "removed");
    spawn_source(&mut world, "destroyed");
    let (mut audio, output) = recording_audio();
    audio.update(
        &world,
        Some(&mut loader),
        ["disabled", "removed", "destroyed"]
            .into_iter()
            .map(|id| command(&world, id, AudioSourceAction::Play))
            .collect(),
        1,
    );
    let mut meta = world.entity(&EntityId::from("disabled")).unwrap().clone();
    meta.enabled = false;
    world
        .try_insert_component_value(
            meta.id.clone(),
            ComponentTypeId::entity_meta(),
            ComponentValue::EntityMeta(meta),
        )
        .unwrap();
    world
        .try_remove_component_value(&EntityId::from("removed"), &ComponentTypeId::audio_source())
        .unwrap();
    world
        .try_despawn_entity(&EntityId::from("destroyed"))
        .unwrap();
    // No gameplay or FixedUpdate is involved in this control/lifecycle pass.
    audio.update(&world, Some(&mut loader), Vec::new(), 2);
    assert_eq!(audio.report().retired_count, 3);
    assert_eq!(audio.report().source_count, 0);
    assert!(output.borrow().voices.is_empty());
    assert_eq!(loader.decoded_cache_len(), 0);
    let mut meta = world.entity(&EntityId::from("disabled")).unwrap().clone();
    meta.enabled = true;
    world
        .try_insert_component_value(
            meta.id.clone(),
            ComponentTypeId::entity_meta(),
            ComponentValue::EntityMeta(meta),
        )
        .unwrap();
    audio.update(&world, Some(&mut loader), Vec::new(), 3);
    assert_eq!(audio.report().source_count, 1);
    assert!(!audio.sources[&disabled].playing);
    assert!(!audio.sources[&disabled].paused);
    assert!(
        output.borrow().voices.is_empty(),
        "reenabling is not autoplay"
    );
}

#[test]
fn audio_prepared_command_cannot_start_a_new_entity_with_reused_source_name() {
    let fixture = AudioFixture::new();
    let mut loader = fixture.loader();
    let mut world = World::new();
    let old_id = spawn_source(&mut world, "source");
    let stale = command(&world, "source", AudioSourceAction::Play);
    let (mut audio, output) = recording_audio();
    audio.update(&world, Some(&mut loader), vec![stale.clone()], 1);
    world.try_despawn_entity(&EntityId::from("source")).unwrap();
    let new_id = spawn_source(&mut world, "source");
    assert_ne!(old_id, new_id);
    audio.update(&world, Some(&mut loader), vec![stale], 2);
    assert!(!output.borrow().voices.contains_key(&old_id));
    assert!(!output.borrow().voices.contains_key(&new_id));
    assert_eq!(audio.report().rejected_command_count, 1);
    assert_eq!(audio.report().play_count, 1);
    assert!(audio
        .report()
        .diagnostics
        .iter()
        .any(|d| d.code == "audio.source_retired"));
}

#[test]
fn audio_output_prepare_and_asynchronous_errors_preserve_failure_summary() {
    let fixture = AudioFixture::new();
    let mut loader = fixture.loader();
    let mut world = World::new();
    let (mut audio, output) = recording_audio();
    output.borrow_mut().prepare_error = Some("test device unavailable".into());
    audio.update(&world, None, Vec::new(), 0);
    assert!(
        output.borrow().calls.is_empty(),
        "a silent project must not open a device"
    );
    spawn_source(&mut world, "source");
    audio.update(
        &world,
        Some(&mut loader),
        vec![command(&world, "source", AudioSourceAction::Play)],
        1,
    );
    assert!(audio.has_errors());
    assert_eq!(
        audio.report().diagnostics[0].code,
        "audio.output_unavailable"
    );
    assert!(audio.report().diagnostics[0]
        .message
        .contains("test device unavailable"));
    assert!(output.borrow().voices.is_empty());
    assert_eq!(loader.decoded_cache_len(), 0);

    let (mut audio, output) = recording_audio();
    audio.update(
        &world,
        Some(&mut loader),
        vec![command(&world, "source", AudioSourceAction::Play)],
        2,
    );
    output.borrow_mut().device_error = Some("test device disconnected".into());
    audio.update(&world, Some(&mut loader), Vec::new(), 3);
    assert!(audio.has_errors());
    assert!(output.borrow().voices.is_empty());
    assert_eq!(loader.decoded_cache_len(), 0);
    assert_eq!(audio.report().diagnostics[0].code, "audio.output_failed");
    audio.update(&world, Some(&mut loader), Vec::new(), 4);
    assert!(audio.report().diagnostics[0]
        .message
        .contains("test device disconnected"));
}

#[test]
fn audio_host_zero_fixed_tick_releases_retired_source_and_does_not_replay_commands() {
    use crate::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
    use crate::frame_loop::RuntimeFrameContext;
    use crate::project_runtime_session::{
        ProjectAuiActionBatch, ProjectRuntimeMutationBuffer, ProjectRuntimeSession,
        ProjectRuntimeSessionContext, ProjectRuntimeSessionOutput,
    };
    use crate::runtime_instance_loader::RuntimeInstanceLoader;

    struct PlayOnce(bool);
    impl ProjectRuntimeSession for PlayOnce {
        fn session_id(&self) -> &str {
            "audio-test-session"
        }
        fn handle_aui_actions(
            &mut self,
            _: ProjectRuntimeSessionContext<'_>,
            _: ProjectAuiActionBatch<'_>,
        ) -> ProjectRuntimeSessionOutput {
            ProjectRuntimeSessionOutput::no_op()
        }
        fn fixed_update(
            &mut self,
            _: ProjectRuntimeSessionContext<'_>,
        ) -> ProjectRuntimeSessionOutput {
            if self.0 {
                return ProjectRuntimeSessionOutput::no_op();
            }
            self.0 = true;
            let mut commands = ProjectRuntimeMutationBuffer::new();
            commands.audio_source_command(EntityId::from("source"), AudioSourceAction::Play);
            ProjectRuntimeSessionOutput::applied(commands)
        }
    }

    let fixture = AudioFixture::new();
    let package = audio_host_package(&fixture);
    let mut loader = RuntimeInstanceLoader::from_package(&package);
    let mut world = World::new();
    let id = spawn_source(&mut world, "source");
    let state = Rc::new(RefCell::new(OutputState::default()));
    let mut host = EngineHostLoop::with_project_runtime_session(
        "audio-test-scene",
        crate::project_logic::ProjectLogicRunner::empty(),
        Box::new(PlayOnce(false)),
    );
    host.set_audio_output(Box::new(RecordingOutput(state.clone())), true);
    let tick = |host: &mut EngineHostLoop,
                world: &mut World,
                loader: &mut RuntimeInstanceLoader,
                count| {
        host.tick_with_runtime_context(
            EngineFrameInput::new(EngineHostMode::HeadlessServer).with_fixed_step_count(count),
            world,
            RuntimeFrameContext {
                package: &package,
                instance_loader: loader,
            },
        )
    };
    tick(&mut host, &mut world, &mut loader, 1);
    assert_eq!(host.audio_report().play_count, 1);
    assert!(state.borrow().voices.contains_key(&id));
    tick(&mut host, &mut world, &mut loader, 0);
    assert_eq!(
        host.audio_report().play_count,
        1,
        "zero-tick presentation must not replay the preceding command"
    );
    world.try_despawn_entity(&EntityId::from("source")).unwrap();
    tick(&mut host, &mut world, &mut loader, 0);
    assert_eq!(host.audio_report().source_count, 0);
    assert_eq!(host.audio_report().retired_count, 1);
    assert!(state.borrow().voices.is_empty());
    assert_eq!(loader.asset_loader().decoded_cache_len(), 0);
    assert!(host.audio_report().diagnostics.is_empty());
}

/// Minimal already-loaded package context for the Host control-update test.
/// PCM comes from the real package-relative WAV and RuntimeAssetLoader;
/// this fixture does not qualify Compiler output or package-file loading.
fn audio_host_package(fixture: &AudioFixture) -> crate::runtime_package::RuntimePackage {
    use crate::runtime_package::*;
    let empty_input = engine_input::InputMappingAsset::explicit_empty("input.none");
    RuntimePackage {
        package_dir: fixture.0.clone(),
        manifest: RuntimePackageManifest {
            schema_version: RUNTIME_PACKAGE_SCHEMA_VERSION.into(),
            package_mode: RUNTIME_PACKAGE_MODE.into(),
            project: RuntimeProjectInfo::explicit_empty(
                "audio-host-test",
                "Audio Host Test",
                "0.1.0",
            ),
            active_scene_id: "audio-test-scene".into(),
            scenes: Vec::new(),
            assets: RuntimeManifestAssetIndex {
                path: "assets/manifest.json".into(),
                asset_count: 1,
            },
            rules: RuntimeManifestRuleIndex {
                path: "rules/manifest.json".into(),
                mode: "none".into(),
            },
            input: RuntimeManifestInputIndex {
                path: "input/manifest.json".into(),
                default_mapping_id: "input.none".into(),
                mapping_count: 1,
            },
            aui: None,
            font_atlases: None,
            font_bundles: None,
            animator2d: None,
            observation_contract: None,
            content_hash: None,
        },
        active_scene: RuntimeScene {
            schema_version: RUNTIME_SCENE_SCHEMA_VERSION.into(),
            id: "audio-test-scene".into(),
            name: "Audio Test".into(),
            gravity: 0.0,
            background: "#000000".into(),
            sky_color: "#000000".into(),
            entities: Vec::new(),
        },
        assets: RuntimeAssetManifest {
            schema_version: RUNTIME_ASSET_MANIFEST_SCHEMA_VERSION.into(),
            assets: Vec::new(),
            runtime_asset_index: Vec::new(),
            bundle_table: Vec::new(),
            cooked_asset_table: Vec::new(),
            dependency_table: Vec::new(),
        },
        runtime_asset_index: fixture.index(),
        runtime_asset_mount_table: fixture.mount_table(),
        rules: RuntimeRuleManifest {
            schema_version: RUNTIME_RULE_MANIFEST_SCHEMA_VERSION.into(),
            mode: "none".into(),
            rules: Vec::new(),
            modules: Vec::new(),
        },
        aui_manifest: RuntimeAuiManifest::empty(),
        aui_documents: RuntimeAuiDocumentRegistry::empty("audio-test"),
        font_atlas_manifest: RuntimeFontAtlasManifest::empty(),
        font_atlases: RuntimeAuiFontAtlasRegistry::empty("audio-test"),
        font_bundle_manifest: crate::font_bundle::RuntimeFontBundleManifest::empty(),
        font_bundles: crate::font_bundle::RuntimeFontBundleRegistry::default(),
        animator2d_registry: crate::animator2d::CookedAnimator2DRegistry::empty(),
        input_manifest: RuntimeInputManifest {
            schema_version: RUNTIME_INPUT_MANIFEST_SCHEMA_VERSION.into(),
            default_mapping_id: "input.none".into(),
            mappings: Vec::new(),
        },
        input_mappings: vec![empty_input.clone()],
        default_input_mapping: Some(empty_input),
    }
}
