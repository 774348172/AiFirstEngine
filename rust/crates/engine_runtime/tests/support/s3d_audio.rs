use super::s3c::{command, entity, world};
use engine_runtime::{
    archetype::ComponentValue,
    audio::{AudioSource, DecodedAudioClip},
    components::ComponentTypeId,
    ids::{EntityId, RuntimeEntityId},
    runtime_asset::*,
    runtime_asset_loader::RuntimeAssetLoader,
    runtime_audio::*,
    runtime_package::RuntimeAssetRef,
};
use project_runtime_sdk::ProjectRuntimeSpatialIntent as I;
use serde_json::json;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc, sync::Arc};
struct Voice {
    cursor: SpatialPcmCursor3D,
    gains: Arc<SpatialGainControl>,
    paused: bool,
}
#[derive(Default)]
struct State {
    voices: BTreeMap<RuntimeEntityId, Voice>,
    plays: usize,
    reopens: usize,
}
struct Output(Rc<RefCell<State>>);
impl AudioOutput for Output {
    fn kind(&self) -> &'static str {
        "test-real-pcm-cursor"
    }
    fn prepare(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn play(
        &mut self,
        id: RuntimeEntityId,
        clip: Arc<DecodedAudioClip>,
        _: f32,
        paused: bool,
    ) -> Result<(), String> {
        let mut s = self.0.borrow_mut();
        let gains = Arc::new(SpatialGainControl::default());
        s.voices.insert(
            id,
            Voice {
                cursor: SpatialPcmCursor3D::new(clip, gains.clone()),
                gains,
                paused,
            },
        );
        s.plays += 1;
        Ok(())
    }
    fn stop(&mut self, id: RuntimeEntityId) {
        self.0.borrow_mut().voices.remove(&id);
    }
    fn set_paused(&mut self, id: RuntimeEntityId, paused: bool) {
        if let Some(v) = self.0.borrow_mut().voices.get_mut(&id) {
            v.paused = paused;
        }
    }
    fn set_spatial(
        &mut self,
        id: RuntimeEntityId,
        gains: Option<SpatialAudioGains3D>,
    ) -> Result<(), String> {
        self.0.borrow().voices.get(&id).unwrap().gains.set(gains)
    }
    fn finished(&self, id: RuntimeEntityId) -> bool {
        !self.0.borrow().voices.contains_key(&id)
    }
    fn reopen(&mut self) -> Result<(), String> {
        let mut s = self.0.borrow_mut();
        s.voices.clear();
        s.reopens += 1;
        Ok(())
    }
}
fn action(world: &engine_runtime::world::World, action: AudioSourceAction) -> AudioSourceCommand {
    let entity_id = EntityId::from("source");
    AudioSourceCommand {
        runtime_id: world.runtime_id_for_source(&entity_id).unwrap(),
        entity_id,
        action,
    }
}
#[test]
fn s3d_spatial_audio_world_pcm_movement_pause_reopen_and_generation_retirement() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../.artifacts/371/s3-simulation-20261026-084317")
        .join(format!("s3d-pcm-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut wav = hound::WavWriter::create(
        root.join("clip.wav"),
        hound::WavSpec {
            channels: 1,
            sample_rate: 48000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for _ in 0..4096 {
        wav.write_sample(16384i16).unwrap();
    }
    wav.finalize().unwrap();
    let mut loader = RuntimeAssetLoader::new(
        &root,
        RuntimeAssetIndex::new(
            vec![RuntimeAssetRecord {
                asset_guid: "guid-audio".into(),
                asset_id: "clip".into(),
                asset_type: "audio".into(),
                sub_asset_id: None,
                version: "1".into(),
                cooked_asset_id: "cooked".into(),
                bundle_id: "startup".into(),
                loader_kind: "audio".into(),
                dependencies: vec![],
                hash: None,
                size: None,
                flags: vec![],
                source_map_debug: None,
            }],
            vec![CookedAssetRecord {
                cooked_asset_id: "cooked".into(),
                bundle_id: "startup".into(),
                path: Some("clip.wav".into()),
                offset: None,
                size: None,
                compression: None,
                hash: None,
            }],
        ),
        RuntimePackageMountTable::new(vec![BundleRecord {
            bundle_id: "startup".into(),
            mount_id: None,
            uri: "startup".into(),
            hash: None,
            version: None,
            mounted: true,
        }]),
    );
    let mut world = world(vec![
        entity(
            "listener",
            [0.; 3],
            vec![
                json!({"componentType":"engine.audio_listener3d","data":{"schemaVersion":"audio-listener3d.v1","enabled":true}}),
            ],
        ),
        entity(
            "source",
            [1., 0., 0.],
            vec![
                json!({"componentType":"engine.spatial_audio3d","data":{"schemaVersion":"spatial-audio3d.v1","minDistance":1,"maxDistance":11}}),
            ],
        ),
    ]);
    let source = EntityId::from("source");
    world
        .try_insert_component_value(
            source.clone(),
            ComponentTypeId::audio_source(),
            ComponentValue::AudioSource(AudioSource {
                clip_ref: RuntimeAssetRef {
                    id: "clip".into(),
                    asset_type: "audio".into(),
                    guid: Some("guid-audio".into()),
                    sub_asset: None,
                },
                volume: 1.,
            }),
        )
        .unwrap();
    world.begin_spatial_tick(1);
    world.finish_spatial_tick();
    let id = world.runtime_id_for_source(&source).unwrap();
    let state = Rc::new(RefCell::new(State::default()));
    let mut audio = RuntimeAudio::new(Box::new(Output(state.clone())));
    audio.update(
        &world,
        Some(&mut loader),
        vec![action(&world, AudioSourceAction::Play)],
        1,
    );
    assert!(!audio.has_errors(), "{:?}", audio.report());
    let clip = state.borrow().voices[&id].cursor.clip.clone();
    {
        let mut s = state.borrow_mut();
        let cursor = &mut s.voices.get_mut(&id).unwrap().cursor;
        assert_eq!(cursor.next(), Some(0.));
        assert_eq!(cursor.next(), Some(0.5));
    }
    // Identical far anchors preserve the pair. Movement changes gain, not PCM owner/cursor.
    for name in ["listener", "source"] {
        command(
            &mut world,
            name,
            I::SetWorldAnchor {
                anchor: [1e9, 0., 0.],
                preserve_world: false,
            },
        );
    }
    world.begin_spatial_tick(2);
    world.finish_spatial_tick();
    audio.update(&world, Some(&mut loader), vec![], 2);
    assert_eq!(state.borrow().plays, 1);
    assert!(Arc::ptr_eq(&clip, &state.borrow().voices[&id].cursor.clip));
    command(
        &mut world,
        "source",
        I::SetLocalTransform {
            transform: project_runtime_sdk::ProjectRuntimeTransform3D {
                translation: [-6., 0., 0.],
                rotation_xyzw: [0., 0., 0., 1.],
                scale: [1.; 3],
            },
        },
    );
    world.begin_spatial_tick(3);
    world.finish_spatial_tick();
    audio.update(&world, Some(&mut loader), vec![], 3);
    assert_eq!(state.borrow().plays, 1);
    {
        let mut s = state.borrow_mut();
        let cursor = &mut s.voices.get_mut(&id).unwrap().cursor;
        let samples = cursor.by_ref().take(2400).collect::<Vec<_>>();
        assert!((samples[2398] - 0.25).abs() < 0.002);
        assert!(samples[2399].abs() < 0.004);
    }
    audio.set_suspended(true);
    assert!(state.borrow().voices[&id].paused);
    audio.update(
        &world,
        Some(&mut loader),
        vec![action(&world, AudioSourceAction::SetPaused(true))],
        4,
    );
    audio.set_suspended(false);
    assert!(state.borrow().voices[&id].paused);
    audio.update(
        &world,
        Some(&mut loader),
        vec![action(&world, AudioSourceAction::SetPaused(false))],
        5,
    );
    assert!(!state.borrow().voices[&id].paused);
    audio.reopen_output(&world).unwrap();
    assert_eq!(state.borrow().reopens, 1);
    assert_eq!(state.borrow().plays, 2);
    assert!(Arc::ptr_eq(&clip, &state.borrow().voices[&id].cursor.clip));
    assert_eq!(
        state
            .borrow_mut()
            .voices
            .get_mut(&id)
            .unwrap()
            .cursor
            .next(),
        Some(0.25)
    );
    let stale = action(&world, AudioSourceAction::Play);
    world.try_despawn_entity(&source).unwrap();
    audio.update(&world, Some(&mut loader), vec![], 6);
    assert!(state.borrow().voices.is_empty());
    audio.reopen_output(&world).unwrap();
    assert!(state.borrow().voices.is_empty());
    audio.update(&world, Some(&mut loader), vec![stale], 7);
    assert_eq!(audio.report().rejected_command_count, 1);
    assert_eq!(state.borrow().plays, 2);
    assert_eq!(loader.decoded_cache_len(), 0);
}
#[test]
fn s3d_distance_pan_reference_and_invalid_listener() {
    let c = SpatialAudioSource3D {
        schema_version: "spatial-audio3d.v1".into(),
        min_distance: 1.,
        max_distance: 11.,
    };
    for anchor in [0., 1e9] {
        let p = [anchor, 2., 3.];
        let g = spatial_audio_gains(p, [1., 0., 0.], [anchor + 6., 2., 3.], &c).unwrap();
        assert_eq!(
            g,
            SpatialAudioGains3D {
                left: 0.,
                right: 0.5
            }
        );
        let center = spatial_audio_gains(p, [1., 0., 0.], p, &c).unwrap();
        assert!((center.left - 0.5f32.sqrt()).abs() < 1e-6);
    }
    assert!(spatial_audio_gains([0.; 3], [0.; 3], [1.; 3], &c).is_err());
}
