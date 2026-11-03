//! Explicit original-package device consumer. PCM is muted after decoding;
//! this proves native consumption and lifetime, not audible output quality.
use super::*;
use engine_runtime::{
    ids::EntityId,
    project_runtime_session::ProjectRuntimeMutationBuffer,
    runtime_audio::{AudioSourceAction, AudioSourceCommand, RuntimeAudio},
    runtime_instance_loader::RuntimeInstanceLoader,
    runtime_package::load_runtime_package,
    world::World,
};
use serde_json::json;
use std::{cell::RefCell, path::PathBuf, rc::Rc, time::Instant};

// Consume the original key data directly. The project route's own unit tests
// require its crate-root modules, so that whole module is not a valid import here.
#[path = "../../../../samples/platform_3d_bm13_k2_project/RuntimeModule/src/track_keys.rs"]
#[allow(clippy::excessive_precision)]
mod track_keys;
#[derive(Clone, Copy)]
struct TrackKey {
    tick: u64,
    move_start: u64,
    teleport: bool,
    position: [f32; 3],
    rotation: [f32; 4],
}
const ROUTE_END: u64 = 36000;
struct Pose {
    translation: [f32; 3],
    rotation_xyzw: [f32; 4],
}
fn sample_track(tick: u64) -> Pose {
    let last = track_keys::KEYS.last().unwrap();
    let (mut p, mut q) = (last.position, last.rotation);
    if let Some(pair) = track_keys::KEYS.windows(2).find(|pair| tick < pair[1].tick) {
        let (a, b) = (pair[0], pair[1]);
        (p, q) = (a.position, a.rotation);
        if !b.teleport {
            let t = if b.tick == b.move_start {
                1.
            } else {
                (tick.saturating_sub(b.move_start) as f64 / (b.tick - b.move_start) as f64)
                    .clamp(0., 1.)
            };
            p = std::array::from_fn(|i| {
                (f64::from(a.position[i])
                    + (f64::from(b.position[i]) - f64::from(a.position[i])) * t)
                    as f32
            });
            if b.move_start > a.tick {
                let t = ((tick - a.tick) as f32 / (b.move_start - a.tick) as f32).clamp(0., 1.);
                let mut rotation = b.rotation;
                let mut dot = a
                    .rotation
                    .iter()
                    .zip(rotation)
                    .map(|(a, b)| a * b)
                    .sum::<f32>();
                if dot < 0. {
                    rotation = rotation.map(|v| -v);
                    dot = -dot;
                }
                let (wa, wb) = if dot > 0.9995 {
                    (1. - t, t)
                } else {
                    let angle = dot.clamp(0., 1.).acos();
                    (
                        ((1. - t) * angle).sin() / angle.sin(),
                        (t * angle).sin() / angle.sin(),
                    )
                };
                q = std::array::from_fn(|i| wa * a.rotation[i] + wb * rotation[i]);
                let norm = q.iter().map(|v| v * v).sum::<f32>().sqrt();
                q = q.map(|v| v / norm);
            }
        }
    }
    p[1] += 12.;
    Pose {
        translation: p,
        rotation_xyzw: q,
    }
}

struct ProbeOutput {
    native: Rc<RefCell<DeviceAudioOutput>>,
    published_gains: Rc<RefCell<BTreeMap<RuntimeEntityId, Option<SpatialAudioGains3D>>>>,
}
impl AudioOutput for ProbeOutput {
    fn kind(&self) -> &'static str {
        "windows-device-muted-original-bm13"
    }
    fn prepare(&mut self) -> Result<(), String> {
        self.native.borrow_mut().prepare()
    }
    fn play(
        &mut self,
        id: RuntimeEntityId,
        clip: Arc<DecodedAudioClip>,
        _: f32,
        paused: bool,
    ) -> Result<(), String> {
        self.native.borrow_mut().play(id, clip, 0., paused)
    }
    fn play_spatial(
        &mut self,
        id: RuntimeEntityId,
        clip: Arc<DecodedAudioClip>,
        _: f32,
        paused: bool,
        gains: Option<SpatialAudioGains3D>,
    ) -> Result<(), String> {
        self.native
            .borrow_mut()
            .play_spatial(id, clip, 0., paused, gains)?;
        self.published_gains.borrow_mut().insert(id, gains);
        Ok(())
    }
    fn stop(&mut self, id: RuntimeEntityId) {
        self.native.borrow_mut().stop(id);
        self.published_gains.borrow_mut().remove(&id);
    }
    fn set_paused(&mut self, id: RuntimeEntityId, paused: bool) {
        self.native.borrow_mut().set_paused(id, paused);
    }
    fn finished(&self, id: RuntimeEntityId) -> bool {
        self.native.borrow().finished(id)
    }
    fn set_spatial(
        &mut self,
        id: RuntimeEntityId,
        gains: Option<SpatialAudioGains3D>,
    ) -> Result<(), String> {
        self.native.borrow_mut().set_spatial(id, gains)?;
        self.published_gains.borrow_mut().insert(id, gains);
        Ok(())
    }
    fn reopen(&mut self) -> Result<(), String> {
        self.native.borrow_mut().reopen()
    }
    fn take_error(&mut self) -> Option<String> {
        self.native.borrow_mut().take_error()
    }
}

fn command(
    world: &World,
    id: &EntityId,
    intent: serde_json::Value,
    commands: &mut ProjectRuntimeMutationBuffer,
) {
    commands.spatial_command(
        id.clone(),
        world.entity_generation(id).unwrap(),
        serde_json::from_value(intent).unwrap(),
    );
}

#[test]
#[ignore = "explicit original BM13 K2 package; actual Windows audio device and600s wall"]
fn s5d_original_bm13_64_device_voices_follow_full_k2_and_retire() {
    let package_dir = PathBuf::from(
        std::env::var_os("AIFE_BM13_LOAD_PACKAGE").expect("original428 RuntimePackage"),
    );
    let evidence = PathBuf::from(
        std::env::var_os("AIFE_BM13_AUDIO_EVIDENCE").expect("fresh run-owned evidence"),
    );
    std::fs::create_dir(&evidence).unwrap();
    let package = load_runtime_package(&package_dir).value.unwrap();
    assert_eq!(package.manifest.project.project_id, "platform-3d-bm13-k2");
    let mut instances = RuntimeInstanceLoader::from_package(&package);
    assert!(instances.asset_loader_mut().mount_bundle("startup"));
    let mut world = World::new();
    let (loaded, report) = instances.load_active_scene_instance(&package, &mut world);
    assert!(loaded.is_some(), "{report:?}");
    let sources: Vec<_> = (0..64)
        .map(|i| EntityId::from(format!("speaker-{i:02}")))
        .collect();
    let camera = EntityId::from("camera");
    let ambience = EntityId::from("ambience-root");
    assert!(sources.iter().all(|id| world.audio_source(id).is_some()));
    let native = Rc::new(RefCell::new(DeviceAudioOutput::default()));
    let published_gains = Rc::new(RefCell::new(BTreeMap::new()));
    let mut audio = RuntimeAudio::new(Box::new(ProbeOutput {
        native: native.clone(),
        published_gains: published_gains.clone(),
    }));
    let plays = |world: &World| {
        sources
            .iter()
            .map(|id| AudioSourceCommand {
                entity_id: id.clone(),
                runtime_id: world.runtime_id_for_source(id).unwrap(),
                action: AudioSourceAction::Play,
            })
            .collect()
    };
    audio.update(&world, Some(instances.asset_loader_mut()), plays(&world), 1);
    assert!(!audio.has_errors(), "{:?}", audio.report());
    assert_eq!(audio.report().playing_count, 64);
    let start = Instant::now();
    let mut rebases = 0u64;
    let mut restart_slot = 0u64;
    let mut checks = 0u64;
    let mut progress_samples = 0u64;
    let mut per_voice_progress = BTreeMap::<RuntimeEntityId, u64>::new();
    let mut paused = false;
    let mut previous_positions = BTreeMap::<RuntimeEntityId, Duration>::new();
    let mut next_log = 30.;
    loop {
        let elapsed = start.elapsed().as_secs_f64();
        let track = ((elapsed * 60.) as u64).min(ROUTE_END);
        let frame = checks + 2;
        world.begin_spatial_tick(frame);
        let next_rebase = (track / 1800).min(20);
        if next_rebase != rebases {
            let voice_keys: Vec<_> = native
                .borrow()
                .voices
                .iter()
                .map(|(id, v)| (*id, Arc::as_ptr(&v.gains)))
                .collect();
            let before_gains = published_gains.borrow().clone();
            let before_plays = audio.report().play_count;
            let mut commands = ProjectRuntimeMutationBuffer::new();
            for id in [&camera, &ambience] {
                command(
                    &world,
                    id,
                    json!({"operation":"setWorldAnchor", "anchor":[next_rebase as f64 * 16., 0., 0.], "preserveWorld":true}),
                    &mut commands,
                );
            }
            commands
                .prepare(&world)
                .unwrap()
                .commit(&mut world)
                .unwrap();
            world.finish_spatial_tick();
            audio.update(&world, Some(instances.asset_loader_mut()), vec![], frame);
            assert_eq!(
                audio.report().play_count,
                before_plays,
                "origin change restarted a voice"
            );
            assert_eq!(
                native
                    .borrow()
                    .voices
                    .iter()
                    .map(|(id, v)| (*id, Arc::as_ptr(&v.gains)))
                    .collect::<Vec<_>>(),
                voice_keys
            );
            for (id, expected) in before_gains {
                let (a, b) = (published_gains.borrow()[&id].unwrap(), expected.unwrap());
                assert!(
                    (a.left - b.left).abs() < 1e-5 && (a.right - b.right).abs() < 1e-5,
                    "keep-world migration changed spatial gains"
                );
            }
            rebases = next_rebase;
        }
        let pose = sample_track(track);
        let mut commands = ProjectRuntimeMutationBuffer::new();
        for (id, height, rotation) in [
            (&camera, pose.translation[1], pose.rotation_xyzw),
            (&ambience, 0., [0., 0., 0., 1.]),
        ] {
            command(
                &world,
                id,
                json!({"operation":"setLocalTransform", "transform": {
                    "translation": [pose.translation[0] - rebases as f32 * 16., height, pose.translation[2]], "rotationXyzw":rotation, "scale":[1.,1.,1.],
                }}),
                &mut commands,
            );
        }
        commands
            .prepare(&world)
            .unwrap()
            .commit(&mut world)
            .unwrap();
        world.finish_spatial_tick();
        let next_restart = track / 300;
        let replay = if next_restart != restart_slot {
            restart_slot = next_restart;
            previous_positions.clear();
            plays(&world)
        } else {
            vec![]
        };
        audio.update(&world, Some(instances.asset_loader_mut()), replay, frame);
        assert!(!audio.has_errors(), "track={track} {:?}", audio.report());
        assert_eq!(audio.report().playing_count, 64);
        {
            let state = native.borrow();
            assert_eq!(state.voices.len(), 64);
            for (id, voice) in &state.voices {
                assert!(
                    !voice.sink.empty(),
                    "track={track} voice={id} PCM exhausted"
                );
                assert!(!voice.sink.is_paused());
                let pos = voice.sink.get_pos();
                if let Some(previous) = previous_positions.insert(*id, pos) {
                    assert!(
                        pos >= previous,
                        "voice playback cursor moved backwards without a project replay"
                    );
                    let advanced = u64::from(pos > previous);
                    progress_samples += advanced;
                    *per_voice_progress.entry(*id).or_default() += advanced;
                }
                assert!(published_gains.borrow()[id].is_some());
            }
        }
        if !paused && track >= 18000 {
            audio.set_suspended(true);
            assert!(native.borrow().voices.values().all(|v| v.sink.is_paused()));
            std::thread::sleep(Duration::from_millis(250));
            audio.set_suspended(false);
            assert!(native.borrow().voices.values().all(|v| !v.sink.is_paused()));
            paused = true;
        }
        checks += 1;
        if elapsed >= next_log {
            let progress = json!({"elapsedSeconds":elapsed,"track":track,"rebases":rebases,"liveNativeVoices":native.borrow().voices.len(),"playCount":audio.report().play_count,"advancingCursorSamples":progress_samples});
            std::fs::write(
                evidence.join("progress.json"),
                serde_json::to_vec_pretty(&progress).unwrap(),
            )
            .unwrap();
            eprintln!("BM13 actual audio: {progress}");
            next_log += 30.;
        }
        if track == ROUTE_END {
            break;
        }
        assert!(elapsed < 630., "bounded original600s audio consumer");
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(rebases, 20);
    assert!(paused && progress_samples > 6400);
    assert_eq!(per_voice_progress.len(), 64);
    assert!(
        per_voice_progress.values().all(|samples| *samples > 100),
        "every original native voice must repeatedly consume PCM: {per_voice_progress:?}"
    );
    for id in &sources {
        world.try_despawn_entity(id).unwrap();
    }
    audio.update(
        &world,
        Some(instances.asset_loader_mut()),
        vec![],
        checks + 3,
    );
    assert!(!audio.has_errors(), "{:?}", audio.report());
    assert_eq!(audio.report().playing_count, 0);
    assert!(native.borrow().voices.is_empty());
    std::fs::write(evidence.join("completed.json"), serde_json::to_vec_pretty(&json!({"elapsedSeconds":start.elapsed().as_secs_f64(),"checks":checks,"rebases":rebases,"track":ROUTE_END,"advancingCursorSamples":progress_samples,"minimumAdvancingCursorSamplesPerVoice":per_voice_progress.values().min(),"finalAudio":audio.report(),"scope":"Original428 PCM/source layout and RuntimeAudio to actual muted Windows DeviceAudioOutput; original600s K2 audio motion,20keep-world migrations,64native voices,original5s replay cadence,pause/resume and retirement. No audible,fullPlayer,GPU,OSdisplay or performance qualification."})).unwrap()).unwrap();
}
