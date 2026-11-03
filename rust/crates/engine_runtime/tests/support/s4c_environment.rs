//! Existing environment consumers against actual Cell lifetime transactions.
//! PCM is consumed through the real spatial cursor; no sound-device claim.
use super::s4a::{runtime_json, Fixture};
use engine_runtime::{
    archetype::ComponentValue,
    audio::{AudioSource, DecodedAudioClip},
    components::ComponentTypeId,
    ids::{EntityId, RuntimeEntityId},
    particle_effect::{program, *},
    particle_render_contract::ParticleSourceFrame,
    project_runtime_session::ProjectRuntimeMutationBuffer,
    runtime_asset::{CookedAssetRecord, RuntimeAssetIndex, RuntimeAssetRecord},
    runtime_audio::*,
    runtime_package::RuntimeAssetRef,
    runtime_particles::{ParticleAction, ParticleCommand, ParticleEffect, RuntimeParticles},
};
use project_runtime_sdk::ProjectRuntimeSpatialIntent;
use serde_json::json;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc, sync::Arc};

struct Voice {
    cursor: SpatialPcmCursor3D,
    gains: Arc<SpatialGainControl>,
    paused: bool,
    volume: f32,
}
#[derive(Default)]
struct PcmState {
    voices: BTreeMap<RuntimeEntityId, Voice>,
    plays: usize,
}
impl PcmState {
    fn mix(&mut self, samples: usize) -> Vec<f32> {
        (0..samples)
            .map(|_| {
                self.voices
                    .values_mut()
                    .filter(|voice| !voice.paused)
                    .map(|voice| voice.cursor.next().unwrap_or(0.) * voice.volume)
                    .sum()
            })
            .collect()
    }
}
struct Output(Rc<RefCell<PcmState>>);
impl AudioOutput for Output {
    fn kind(&self) -> &'static str {
        "s4c-real-spatial-pcm-cursor"
    }
    fn prepare(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn play(
        &mut self,
        id: RuntimeEntityId,
        clip: Arc<DecodedAudioClip>,
        volume: f32,
        paused: bool,
    ) -> Result<(), String> {
        let gains = Arc::new(SpatialGainControl::default());
        self.0.borrow_mut().voices.insert(
            id,
            Voice {
                cursor: SpatialPcmCursor3D::new(clip, gains.clone()),
                gains,
                paused,
                volume,
            },
        );
        self.0.borrow_mut().plays += 1;
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
    fn finished(&self, id: RuntimeEntityId) -> bool {
        !self.0.borrow().voices.contains_key(&id)
    }
    fn set_spatial(
        &mut self,
        id: RuntimeEntityId,
        gains: Option<SpatialAudioGains3D>,
    ) -> Result<(), String> {
        self.0
            .borrow()
            .voices
            .get(&id)
            .ok_or("missing PCM voice")?
            .gains
            .set(gains)
    }
    fn reopen(&mut self) -> Result<(), String> {
        self.0.borrow_mut().voices.clear();
        Ok(())
    }
}
fn reference(id: &str, kind: &str) -> RuntimeAssetRef {
    RuntimeAssetRef {
        id: id.into(),
        asset_type: kind.into(),
        guid: Some(format!("guid-{id}")),
        sub_asset: None,
    }
}
fn cooked_effect() -> CookedParticleEffect {
    let mut description: ParticleEffectDescription = serde_json::from_slice(include_bytes!(
        "../../../engine_runtime_core/tests/fixtures/particle-effect.json"
    ))
    .unwrap();
    description.asset_id = "environment-dust".into();
    description.asset_guid = "guid-environment-dust".into();
    description.emitters[0].capacity = 4;
    description.emitters[0].bursts[0].count = 1;
    description.validate().unwrap();
    let programs = description
        .emitters
        .iter()
        .map(|e| CookedParticleProgram {
            emitter: e.name.clone(),
            wgsl: program::compute_source(
                &description,
                e,
                &(program::prefix(&description, e)
                    + program::DEFAULT_BEHAVIOR
                    + program::CONTRACT_CALLS),
            ),
            author_path: None,
            author_first_line: 0,
            author_line_count: 0,
            particle_stride: program::particle_layout(e).2,
        })
        .collect();
    CookedParticleEffect {
        program_contract: PARTICLE_PROGRAM_CONTRACT.into(),
        description,
        programs,
        source_digests: Default::default(),
    }
}
fn environment_assets(f: &mut Fixture) {
    let root = f._files.root.join("package");
    std::fs::write(
        root.join("environment-dust.json"),
        serde_json::to_vec(&cooked_effect()).unwrap(),
    )
    .unwrap();
    let mut wav = hound::WavWriter::create(
        root.join("environment-tone.wav"),
        hound::WavSpec {
            channels: 1,
            sample_rate: 48000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for _ in 0..16384 {
        wav.write_sample(16384i16).unwrap();
    }
    wav.finalize().unwrap();
    let mut records = Vec::new();
    let mut cooked = Vec::new();
    for (id, kind, path) in [
        (
            "environment-dust",
            "particle-effect",
            "environment-dust.json",
        ),
        ("environment-tone", "audio", "environment-tone.wav"),
    ] {
        let cooked_id = format!("cooked-{id}");
        records.push(RuntimeAssetRecord {
            asset_guid: format!("guid-{id}"),
            asset_id: id.into(),
            asset_type: kind.into(),
            sub_asset_id: None,
            version: "1".into(),
            cooked_asset_id: cooked_id.clone(),
            bundle_id: "startup".into(),
            loader_kind: kind.into(),
            dependencies: vec![],
            hash: None,
            size: None,
            flags: vec![],
            source_map_debug: None,
        });
        cooked.push(CookedAssetRecord {
            cooked_asset_id: cooked_id,
            bundle_id: "startup".into(),
            path: Some(path.into()),
            offset: None,
            size: None,
            compression: None,
            hash: None,
        });
    }
    f.loader
        .asset_loader_mut()
        .mount_patch_index(RuntimeAssetIndex::new(records, cooked));
    assert!(f.loader.asset_loader_mut().mount_bundle("startup"));
}
fn dynamic(f: &mut Fixture, id: &EntityId, component: &str, data: serde_json::Value) {
    let ty = ComponentTypeId::new(component);
    f.world
        .try_insert_component_value(
            id.clone(),
            ty.clone(),
            ComponentValue::Dynamic {
                component_type: ty,
                value: runtime_json(data),
            },
        )
        .unwrap();
}
fn attach(f: &mut Fixture, id: &EntityId, play_on_awake: bool) {
    f.world
        .try_insert_component_value(
            id.clone(),
            ComponentTypeId::particle_effect(),
            ComponentValue::ParticleEffect(ParticleEffect {
                effect_ref: reference("environment-dust", "particle-effect"),
                play_on_awake,
                paused: false,
                parameters: Default::default(),
            }),
        )
        .unwrap();
    f.world
        .try_insert_component_value(
            id.clone(),
            ComponentTypeId::audio_source(),
            ComponentValue::AudioSource(AudioSource {
                clip_ref: reference("environment-tone", "audio"),
                volume: 1.,
            }),
        )
        .unwrap();
    dynamic(
        f,
        id,
        "engine.spatial_audio3d",
        json!({"schemaVersion":"spatial-audio3d.v1","minDistance":1,"maxDistance":30}),
    );
}
fn anchor(f: &mut Fixture, id: &EntityId, x: f64, preserve: bool) {
    let mut commands = ProjectRuntimeMutationBuffer::new();
    commands.spatial_command(
        id.clone(),
        f.world.entity_generation(id).unwrap(),
        ProjectRuntimeSpatialIntent::SetWorldAnchor {
            anchor: [x, 0., 0.],
            preserve_world: preserve,
        },
    );
    commands
        .prepare(&f.world)
        .unwrap()
        .commit(&mut f.world)
        .unwrap();
}
fn spatial_tick(f: &mut Fixture, tick: u64) {
    f.world.begin_spatial_tick(tick);
    f.world.finish_spatial_tick();
}
fn particle_update(
    f: &mut Fixture,
    particles: &mut RuntimeParticles,
    commands: Vec<ParticleCommand>,
    frame: u64,
) -> Vec<ParticleSourceFrame> {
    let frames = particles.update(
        &f.world,
        Some(f.loader.asset_loader_mut()),
        commands,
        frame,
        1. / 60.,
    );
    frames
}
fn particle_command(f: &Fixture, id: &EntityId, action: ParticleAction) -> ParticleCommand {
    ParticleCommand {
        entity_id: id.clone(),
        runtime_id: f.world.runtime_id_for_source(id).unwrap(),
        action,
    }
}
fn audio_command(f: &Fixture, id: &EntityId, action: AudioSourceAction) -> AudioSourceCommand {
    AudioSourceCommand {
        entity_id: id.clone(),
        runtime_id: f.world.runtime_id_for_source(id).unwrap(),
        action,
    }
}

#[test]
fn s4c_environment_cell_pause_far_origin_release_and_reload_do_not_resurrect_old_sources() {
    let mut f = Fixture::new(None);
    environment_assets(&mut f);
    let baseline = f.loader.asset_loader().decoded_cache_len();
    let owner = f.owner.clone();
    dynamic(
        &mut f,
        &owner,
        "engine.audio_listener3d",
        json!({"schemaVersion":"audio-listener3d.v1","enabled":true}),
    );
    let ticket = f.load("environment");
    let id = f.node(ticket);
    attach(&mut f, &id, true);
    anchor(&mut f, &owner, 1e9, false);
    anchor(&mut f, &id, 1e9, false);
    spatial_tick(&mut f, 1);
    let old_runtime = f.world.runtime_id_for_source(&id).unwrap();
    let mut particles = RuntimeParticles::default();
    let first = particle_update(&mut f, &mut particles, vec![], 1);
    assert!(
        particles.diagnostics.is_empty(),
        "{:?}",
        particles.diagnostics
    );
    assert_eq!(first.len(), 1);
    assert!(first[0].step.emitting);
    assert!(!first[0].step.paused);
    let state = Rc::new(RefCell::new(PcmState::default()));
    let mut audio = RuntimeAudio::new(Box::new(Output(state.clone())));
    let play = audio_command(&f, &id, AudioSourceAction::Play);
    audio.update(&f.world, Some(f.loader.asset_loader_mut()), vec![play], 1);
    assert!(!audio.has_errors(), "{:?}", audio.report());
    assert!(
        state
            .borrow_mut()
            .mix(128)
            .iter()
            .any(|sample| sample.abs() > 0.01),
        "live cell produces actual nonzero PCM"
    );
    let pause = particle_command(&f, &id, ParticleAction::SetPaused(true));
    let paused = particle_update(&mut f, &mut particles, vec![pause], 2);
    assert!(paused[0].step.paused);
    audio.set_suspended(true);
    assert!(state
        .borrow_mut()
        .mix(128)
        .iter()
        .all(|sample| *sample == 0.));
    // Recenter the coordinate decomposition while preserving absolute positions.
    // Neither adapter may restart its source or change world-space relationships.
    anchor(&mut f, &owner, 1e9 + 1024., true);
    anchor(&mut f, &id, 1e9 + 1024., true);
    spatial_tick(&mut f, 2);
    let recentered = particle_update(&mut f, &mut particles, vec![], 3);
    assert_eq!(first[0].instance, recentered[0].instance);
    assert_eq!(
        first[0].simulation_origin_bits,
        recentered[0].simulation_origin_bits
    );
    assert!(recentered[0].step.paused);
    for axis in 0..3 {
        assert!(
            (first[0].step.origin[axis].to_f32() - recentered[0].step.origin[axis].to_f32()).abs()
                < 0.001
        );
    }
    audio.update(&f.world, Some(f.loader.asset_loader_mut()), vec![], 2);
    assert_eq!(state.borrow().plays, 1);
    let resume = particle_command(&f, &id, ParticleAction::SetPaused(false));
    assert!(
        !particle_update(&mut f, &mut particles, vec![resume], 4)[0]
            .step
            .paused
    );
    audio.set_suspended(false);
    assert!(state
        .borrow_mut()
        .mix(128)
        .iter()
        .any(|sample| sample.abs() > 0.01));
    let stale_particle = particle_command(&f, &id, ParticleAction::Restart);
    let stale_audio = audio_command(&f, &id, AudioSourceAction::Play);
    f.release(ticket);
    assert!(f.world.try_resolve_runtime_entity(old_runtime).is_err());
    assert!(particle_update(&mut f, &mut particles, vec![], 5).is_empty());
    assert_eq!(particles.source_count(), 0);
    audio.update(&f.world, Some(f.loader.asset_loader_mut()), vec![], 5);
    assert!(state.borrow().voices.is_empty());
    assert!(state.borrow_mut().mix(256).iter().all(|s| *s == 0.));
    audio.reopen_output(&f.world).unwrap();
    assert!(state.borrow().voices.is_empty());
    assert!(particle_update(&mut f, &mut particles, vec![stale_particle.clone()], 6).is_empty());
    audio.update(
        &f.world,
        Some(f.loader.asset_loader_mut()),
        vec![stale_audio.clone()],
        6,
    );
    assert_eq!(state.borrow().plays, 1);
    assert!(audio.report().rejected_command_count > 0);
    assert_eq!(
        f.loader.asset_loader().decoded_cache_len(),
        baseline,
        "released consumers return asset cache to measured baseline"
    );
    let replacement = f.load("environment");
    let new_id = f.node(replacement);
    attach(&mut f, &new_id, false);
    anchor(&mut f, &new_id, 1e9, false);
    spatial_tick(&mut f, 3);
    assert_ne!(old_runtime, f.world.runtime_id_for_source(&new_id).unwrap());
    let fresh = particle_update(&mut f, &mut particles, vec![stale_particle], 7);
    assert_eq!(fresh.len(), 1);
    assert_ne!(first[0].instance, fresh[0].instance);
    assert_eq!(fresh[0].epoch, 0);
    assert!(!fresh[0].step.emitting);
    assert!(particles
        .diagnostics
        .iter()
        .any(|message| message.contains("stale_target")));
    audio.update(
        &f.world,
        Some(f.loader.asset_loader_mut()),
        vec![stale_audio],
        7,
    );
    assert!(state.borrow().voices.is_empty());
    assert_eq!(state.borrow().plays, 1);
    f.release(replacement);
    assert!(particle_update(&mut f, &mut particles, vec![], 8).is_empty());
    audio.update(&f.world, Some(f.loader.asset_loader_mut()), vec![], 8);
    assert!(state.borrow().voices.is_empty());
    assert_eq!(f.loader.asset_loader().decoded_cache_len(), baseline);
}

#[test]
#[ignore = "explicit original BM13 package; actual64 PCM mix and original10K particle source/history across three anchors"]
fn s5d_bm13_original_64_pcm_and_particle_source_keep_spatial_history_across_rebases() {
    use engine_runtime::{
        runtime_instance_loader::RuntimeInstanceLoader, runtime_package::load_runtime_package,
        world::World,
    };
    use project_runtime_sdk::ProjectRuntimeTransform3D;
    let root = std::path::PathBuf::from(std::env::var_os("AIFE_BM13_LOAD_PACKAGE").unwrap());
    let package = load_runtime_package(&root).value.unwrap();
    let mut reference: Vec<Vec<f32>> = Vec::new();
    for base in [0., 10_000., 1_000_000.] {
        let mut world = World::new();
        let mut instances = RuntimeInstanceLoader::from_package(&package);
        instances
            .asset_loader_mut()
            .set_streaming_budget(engine_runtime::runtime_asset_loader::StreamingBudget {
                max_handles: 65_536,
                max_operations: 256,
                max_cpu_resident_bytes: 2 * 1024 * 1024 * 1024,
                ..Default::default()
            })
            .unwrap();
        assert!(instances.asset_loader_mut().mount_bundle("startup"));
        let (loaded, report) = instances.load_active_scene_instance(&package, &mut world);
        assert!(loaded.is_some(), "{report:?}");
        let roots: Vec<_> = world
            .entity_ids()
            .into_iter()
            .filter(|id| {
                world.transform3d(id).is_some() && world.hierarchy(id).unwrap().parent_id.is_none()
            })
            .cloned()
            .collect();
        let sources: Vec<_> = (0..64)
            .map(|i| EntityId::from(format!("speaker-{i:02}")))
            .collect();
        let camera = EntityId::from("camera");
        let initial = world.transform3d(&camera).unwrap().clone();
        let authored_positions: BTreeMap<_, _> = sources
            .iter()
            .chain(std::iter::once(&camera))
            .map(|id| (id.clone(), world.transform3d(id).unwrap().translation))
            .collect();
        let mut initialize = ProjectRuntimeMutationBuffer::new();
        for id in &roots {
            initialize.spatial_command(
                id.clone(),
                world.entity_generation(id).unwrap(),
                ProjectRuntimeSpatialIntent::SetWorldAnchor {
                    anchor: [base, 0., 0.],
                    preserve_world: false,
                },
            );
        }
        initialize
            .prepare(&world)
            .unwrap()
            .commit(&mut world)
            .unwrap();
        world.finish_spatial_tick();
        let state = Rc::new(RefCell::new(PcmState::default()));
        let mut audio = RuntimeAudio::new(Box::new(Output(state.clone())));
        let mut particles = RuntimeParticles::default();
        let mut particle_identity = None;
        let particle_entities: Vec<_> = world
            .entity_ids()
            .into_iter()
            .filter(|id| world.particle_effect(id).is_some())
            .cloned()
            .collect();
        assert_eq!(particle_entities.len(), 1);
        let particle_entity = &particle_entities[0];
        let authored_particle = world.transform3d(particle_entity).unwrap().translation;
        let play = sources
            .iter()
            .map(|id| AudioSourceCommand {
                entity_id: id.clone(),
                runtime_id: world.runtime_id_for_source(id).unwrap(),
                action: AudioSourceAction::Play,
            })
            .collect();
        audio.update(&world, Some(instances.asset_loader_mut()), play, 1);
        assert!(!audio.has_errors(), "{:?}", audio.report());
        assert_eq!(audio.report().playing_count, 64);
        assert_eq!(state.borrow().voices.len(), 64);
        let mut previous_nonzero = false;
        for step in 0..=20 {
            if step > 0 {
                world.begin_spatial_tick(step + 1);
                let mut commands = ProjectRuntimeMutationBuffer::new();
                for id in &roots {
                    commands.spatial_command(
                        id.clone(),
                        world.entity_generation(id).unwrap(),
                        ProjectRuntimeSpatialIntent::SetWorldAnchor {
                            anchor: [base + step as f64 * 16., 0., 0.],
                            preserve_world: true,
                        },
                    );
                }
                commands
                    .prepare(&world)
                    .unwrap()
                    .commit(&mut world)
                    .unwrap();
                // Same absolute listener motion at all three anchors; keep the
                // original orientation and scale instead of synthesizing another camera.
                let mut commands = ProjectRuntimeMutationBuffer::new();
                let mut translation = initial.translation;
                translation[0] += step as f32 * (0.5 - 16.);
                commands.spatial_command(
                    camera.clone(),
                    world.entity_generation(&camera).unwrap(),
                    ProjectRuntimeSpatialIntent::SetLocalTransform {
                        transform: ProjectRuntimeTransform3D {
                            translation,
                            rotation_xyzw: initial.rotation_xyzw,
                            scale: initial.scale,
                        },
                    },
                );
                commands
                    .prepare(&world)
                    .unwrap()
                    .commit(&mut world)
                    .unwrap();
                world.finish_spatial_tick();
                audio.update(&world, Some(instances.asset_loader_mut()), vec![], step + 1);
            }
            let particle_frames = particles.update(
                &world,
                Some(instances.asset_loader_mut()),
                vec![],
                step + 1,
                1. / 60.,
            );
            assert!(
                particles.diagnostics.is_empty(),
                "{:?}",
                particles.diagnostics
            );
            assert_eq!(particle_frames.len(), 1);
            let source = &particle_frames[0];
            let identity = (source.instance, source.epoch, source.simulation_origin_bits);
            if let Some(first) = particle_identity {
                assert_eq!(
                    first, identity,
                    "rebase must not restart original particle source"
                );
            } else {
                particle_identity = Some(identity);
                let assets: engine_runtime::runtime_particles::ParticlePreparedAssets =
                    serde_json::from_str(&source.assets).unwrap();
                assert_eq!(assets.effect.description.budget.max_particles, 10000);
                assert_eq!(
                    assets
                        .effect
                        .description
                        .emitters
                        .iter()
                        .map(|e| e.capacity)
                        .sum::<u32>(),
                    10000
                );
            }
            assert_eq!(
                source.simulation_origin_bits.map(f64::from_bits),
                [base, 0., 0.]
            );
            let pose = world.spatial_world(particle_entity).unwrap();
            for (axis, authored) in authored_particle.iter().enumerate() {
                assert!((source.step.origin[axis].to_f32() - authored).abs() <= 0.001);
                let expected = f64::from(*authored) + if axis == 0 { base } else { 0. };
                assert!(
                    (pose.anchor[axis] + f64::from(pose.world_matrix[12 + axis]) - expected).abs()
                        <= 0.001
                );
                if step > 0 {
                    assert_eq!(
                        pose.previous_world_matrix[12 + axis],
                        pose.world_matrix[12 + axis],
                        "stationary original emitter previous/current disagree after rebase"
                    );
                }
            }
            assert!(!audio.has_errors(), "{:?}", audio.report());
            assert_eq!(audio.report().playing_count, 64);
            assert_eq!(state.borrow().plays, 64, "rebase must not restart a voice");
            // An independent authored-position oracle catches the same incorrect
            // rebase at all anchors; cross-anchor PCM equality alone would miss it.
            for (id, authored) in &authored_positions {
                let pose = world.spatial_world(id).unwrap();
                for (axis, value) in authored.iter().enumerate() {
                    let expected = f64::from(*value)
                        + if axis == 0 { base } else { 0. }
                        + if id == &camera && axis == 0 {
                            step as f64 * 0.5
                        } else {
                            0.
                        };
                    let actual = pose.anchor[axis] + f64::from(pose.world_matrix[12 + axis]);
                    assert!(
                        (actual - expected).abs() < 1e-5,
                        "{id:?}, base={base}, step={step}, axis={axis}: {actual} != {expected}"
                    );
                }
            }
            let mixed = state.borrow_mut().mix(4096);
            assert!(mixed.iter().all(|s| s.is_finite()));
            assert!(
                mixed.iter().any(|s| s.abs() > 0.001),
                "64 original PCM sources must produce actual samples"
            );
            if base == 0. {
                if let Some(last) = reference.last() {
                    previous_nonzero |= last != &mixed;
                }
                reference.push(mixed);
            } else {
                let error = mixed
                    .iter()
                    .zip(&reference[step as usize])
                    .map(|(a, b)| (a - b).abs())
                    .fold(0f32, f32::max);
                assert!(
                    error < 1e-5,
                    "base={base}, step={step}, maximum PCM sample error={error}"
                );
            }
        }
        assert!(
            base != 0. || previous_nonzero,
            "the PCM cursors must actually advance"
        );
        audio.set_suspended(true);
        assert!(state.borrow_mut().mix(128).iter().all(|v| *v == 0.));
        audio.set_suspended(false);
        assert!(state.borrow_mut().mix(128).iter().any(|v| v.abs() > 0.001));
        for id in &sources {
            world.try_despawn_entity(id).unwrap();
        }
        audio.update(&world, Some(instances.asset_loader_mut()), vec![], 100);
        assert!(state.borrow().voices.is_empty());
        assert_eq!(audio.report().playing_count, 0);
        eprintln!("BM13 original64 PCM sources: base={base}, twenty16m rebases, moving listener,21x4096 samples, pause/resume and despawn; original10K particle source projection/origin/previous/current stable; no physical sound-device or actual10K GPU particle position qualification");
    }
}
