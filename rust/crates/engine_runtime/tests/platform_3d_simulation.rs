//! S3 owner/consumer contracts; deterministic correctness, not scale qualification.
#[path = "support/bm10_navigation_origin.rs"]
mod bm10_navigation_origin;
#[cfg(windows)]
#[path = "support/s5d_long_replay.rs"]
mod s5d_long_replay;

#[test]
#[ignore = "explicit BM13 published assets; current authored probe profile to actual Detour"]
fn bm13_authored_probe_profile_finds_a_real_path_across_both_terrain_seams() {
    use engine_runtime::{
        ids::EntityId,
        navigation_3d::{NavigationModule, NavigationStatus3D},
        physics_3d::PhysicsModule,
        project_runtime_session::ProjectRuntimeMutationBuffer,
        runtime_instance_loader::RuntimeInstanceLoader,
        runtime_package::load_runtime_package,
        scene_loader::load_scene_into_world,
    };
    use project_runtime_sdk::ProjectRuntimeSpatialIntent as Intent;
    let root = std::path::PathBuf::from(std::env::var_os("AIFE_BM13_LOAD_PACKAGE").unwrap());
    let package = load_runtime_package(root).value.unwrap();
    let authored: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../samples/platform_3d_bm13_project/Scenes/main.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let probe = authored["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entity| entity["id"] == "nav-probe")
        .unwrap();
    let profile = probe["components"]
        .as_array()
        .unwrap()
        .iter()
        .find(|component| component["componentType"] == "engine.navigation_agent3d")
        .unwrap()["data"]["profile"]
        .clone();
    let mut scene = serde_json::to_value(&package.active_scene).unwrap();
    let probe = scene["entities"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|entity| entity["id"] == "nav-probe")
        .unwrap();
    let agent = probe["components"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|component| component["componentType"] == "engine.navigation_agent3d")
        .unwrap();
    agent["data"]["profile"] = profile.clone();
    let scene = serde_json::from_value(scene).unwrap();
    let mut world = load_scene_into_world(&scene).value.unwrap();
    let mut loader = RuntimeInstanceLoader::from_package(&package);
    assert!(loader.asset_loader_mut().mount_bundle("startup"));
    let mut navigation = NavigationModule::default();
    let mut physics = PhysicsModule::default();
    let id = EntityId::from("nav-probe");
    let started = std::time::Instant::now();
    for tick in 1..=600 {
        world.begin_spatial_tick(tick);
        if tick == 1 {
            let mut batch = ProjectRuntimeMutationBuffer::default();
            batch.spatial_command(
                id.clone(),
                world.entity_generation(&id).unwrap(),
                Intent::NavigationTarget {
                    target: [80., 0., 16.],
                    deadline_ticks: 600,
                },
            );
            batch.prepare(&world).unwrap().commit(&mut world).unwrap();
        }
        navigation
            .advance(&mut world, loader.asset_loader_mut(), tick, 1. / 60.)
            .unwrap();
        physics
            .advance_with_assets(&mut world, loader.asset_loader_mut(), tick, 1. / 60.)
            .unwrap();
        world.finish_spatial_tick();
        let state = world.navigation3d_state(&id).unwrap();
        if state.status != NavigationStatus3D::Pending {
            assert_eq!(navigation.resident_surface_count(), 9);
            eprintln!("BM13 authored profile={profile} tick={tick} actual Detour state={state:?}");
            assert_eq!(state.status, NavigationStatus3D::Following);
            assert!(state.points.iter().any(|point| point[0] < 32.));
            assert!(state.points.iter().any(|point| point[0] > 64.));
            return;
        }
        assert!(started.elapsed().as_secs() < 20);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    panic!("Actual Detour request did not terminate");
}

#[test]
#[ignore = "explicit original BM07 published package; 1x/1.5x/2x actual motor capacity refusal and recovery"]
fn s5d_bm07_original_motor_pressure_rejects_without_commit_and_recovers_original_load() {
    use engine_runtime::{
        ids::EntityId, navigation_3d::NavigationModule, physics_3d::PhysicsModule,
        runtime_instance_loader::RuntimeInstanceLoader, runtime_package::load_runtime_package,
        scene_loader::load_scene_into_world,
    };
    let root = std::path::PathBuf::from(std::env::var_os("AIFE_BM07_LOAD_PACKAGE").unwrap());
    let package = load_runtime_package(root).value.unwrap();
    let original: Vec<_> = package
        .active_scene
        .entities
        .iter()
        .filter(|entity| {
            entity
                .components
                .iter()
                .any(|c| c.component_type == "engine.motor3d")
        })
        .cloned()
        .collect();
    assert_eq!(original.len(), 1000);
    assert!(original.iter().all(|entity| entity
        .components
        .iter()
        .any(|c| c.component_type == "engine.navigation_agent3d")));
    for count in [1000usize, 1500, 2000] {
        let mut scene = package.active_scene.clone();
        let mut extra = Vec::new();
        for (i, template) in original.iter().enumerate().take(count - 1000) {
            let mut entity = template.clone();
            entity.id = format!("pressure-extra-{i:04}");
            entity.name = entity.id.clone();
            assert!(entity.parent_id.is_none());
            extra.push(EntityId::from(entity.id.clone()));
            scene.entities.push(entity);
        }
        let mut world = load_scene_into_world(&scene).value.unwrap();
        let mut loader = RuntimeInstanceLoader::from_package(&package);
        assert!(loader.asset_loader_mut().mount_bundle("startup"));
        let mut navigation = NavigationModule::default();
        navigation
            .advance(&mut world, loader.asset_loader_mut(), 1, 1. / 60.)
            .unwrap();
        let agents = scene
            .entities
            .iter()
            .filter(|e| {
                e.components
                    .iter()
                    .any(|c| c.component_type == "engine.navigation_agent3d")
            })
            .count();
        assert_eq!(agents, count);
        assert_eq!(
            scene
                .entities
                .iter()
                .filter(|e| {
                    e.components
                        .iter()
                        .any(|c| c.component_type == "engine.navigation_agent3d")
                        && world
                            .navigation3d_state(&EntityId::from(e.id.clone()))
                            .is_some()
                })
                .count(),
            count,
            "every original and pressure agent must acquire actual navigation owner state"
        );
        assert_eq!(
            scene
                .entities
                .iter()
                .filter(|e| e
                    .components
                    .iter()
                    .any(|c| c.component_type == "engine.body3d"))
                .count(),
            54
        );
        assert_eq!(
            scene
                .entities
                .iter()
                .filter(|e| e
                    .components
                    .iter()
                    .any(|c| c.component_type == "engine.navigation3d"))
                .count(),
            1
        );
        let before: Vec<_> = original
            .iter()
            .map(|e| {
                let id = EntityId::from(e.id.clone());
                (
                    id.clone(),
                    world.entity_generation(&id).unwrap(),
                    world.transform3d(&id).unwrap().clone(),
                )
            })
            .collect();
        let mut physics = PhysicsModule::default();
        world.begin_spatial_tick(1);
        let started = std::time::Instant::now();
        let result =
            physics.advance_with_assets(&mut world, loader.asset_loader_mut(), 1, 1. / 60.);
        if count == 1000 {
            result.unwrap();
            assert_eq!(world.physics3d_frame().motors.len(), 1000);
        } else {
            let error = result.unwrap_err();
            assert_eq!(error.field_path, "/capacity");
            assert!(error.message.contains("budget exceeded"), "{error:?}");
            assert!(
                started.elapsed().as_secs() < 10,
                "pressure refusal exceeded bounded response"
            );
            assert!(
                world.physics3d_frame().motors.is_empty(),
                "failed physics published a partial frame"
            );
            for (id, generation, transform) in &before {
                assert_eq!(world.entity_generation(id), Some(*generation));
                assert_eq!(world.transform3d(id), Some(transform));
            }
            for id in &extra {
                world.try_despawn_entity(id).unwrap();
            }
            world.finish_spatial_tick();
            world.begin_spatial_tick(2);
            physics
                .advance_with_assets(&mut world, loader.asset_loader_mut(), 2, 1. / 60.)
                .unwrap();
            assert_eq!(
                world.physics3d_frame().motors.len(),
                1000,
                "original load did not recover after controlled refusal"
            );
        }
        world.finish_spatial_tick();
        eprintln!("BM07 original package motor pressure: requested={count}, navigation admitted, physics={} in{:.3}s, original1000 real motors active; no full BM12/path/performance claim",if count==1000 {"accepted"} else {"controlled capacity refusal then recovered"},started.elapsed().as_secs_f64());
    }
}

#[test]
#[ignore = "explicit published BM09 package; spatial transaction owner diagnosis"]
fn bm09_original_relocation_transaction_preflights_with_actual_packaged_scene() {
    use engine_runtime::{
        ids::EntityId, physics_3d::PhysicsModule,
        project_runtime_session::ProjectRuntimeMutationBuffer,
        runtime_instance_loader::RuntimeInstanceLoader, runtime_package::load_runtime_package,
        world::World,
    };
    use project_runtime_sdk::ProjectRuntimeSpatialIntent as Intent;
    let root = std::path::PathBuf::from(std::env::var_os("AIFE_BM09_LOAD_PACKAGE").unwrap());
    let package = load_runtime_package(root).value.unwrap();
    let mut world = World::new();
    let mut loader = RuntimeInstanceLoader::from_package(&package);
    let (instance, report) = loader.load_active_scene_instance(&package, &mut world);
    assert!(instance.is_some(), "{report:?}");
    let roots: Vec<_> = (0..1000)
        .map(|i| EntityId::from(format!("node-{i:04}")))
        .chain([EntityId::from("domain-root"), EntityId::from("camera")])
        .collect();
    let mut physics = PhysicsModule::default();
    for tick in 1..=30 {
        world.begin_spatial_tick(tick);
        let mut batch = ProjectRuntimeMutationBuffer::default();
        if tick == 1 || tick == 30 {
            for id in &roots {
                batch.spatial_command(
                    id.clone(),
                    world.entity_generation(id).unwrap(),
                    Intent::SetWorldAnchor {
                        anchor: [if tick == 30 { 16. } else { 0. }, 0., 0.],
                        preserve_world: tick == 30,
                    },
                );
            }
        }
        for i in 0..10 {
            let id = EntityId::from(format!("actor-{i:02}"));
            batch.spatial_command(
                id.clone(),
                world.entity_generation(&id).unwrap(),
                Intent::MotorMove {
                    displacement: [0.001, 0., 0.],
                },
            );
        }
        let prepared = batch
            .prepare(&world)
            .unwrap_or_else(|e| panic!("tick {tick}: {e:?}"));
        prepared.commit(&mut world).unwrap();
        if tick == 30 {
            let id = EntityId::from("actor-00");
            let state = world.spatial_world(&id).unwrap();
            eprintln!(
                "BM09 before physics local={:?} anchor={:?} world={:?}",
                world.transform3d(&id),
                state.anchor,
                &state.world_matrix[12..15]
            );
            assert!(
                (state.anchor[0] + f64::from(state.world_matrix[12]) - 2.029).abs() < 0.002,
                "keep-world moved the motor before its solver tick"
            );
        }
        physics
            .advance_with_assets(&mut world, loader.asset_loader_mut(), tick, 1. / 60.)
            .unwrap();
        world.finish_spatial_tick();
        if tick == 30 {
            let id = EntityId::from("actor-00");
            let state = world.spatial_world(&id).unwrap();
            eprintln!(
                "BM09 after physics local={:?} anchor={:?} world={:?}",
                world.transform3d(&id),
                state.anchor,
                &state.world_matrix[12..15]
            );
            assert!(
                (state.anchor[0] + f64::from(state.world_matrix[12]) - 2.030).abs() < 0.002,
                "keep-world moved the motor during its solver tick"
            );
        }
    }
}

#[path = "support/s3b_pose.rs"]
mod s3b_pose;
#[cfg(not(target_arch = "wasm32"))]
#[path = "support/s3c_physics.rs"]
mod s3c;
#[cfg(not(target_arch = "wasm32"))]
#[path = "support/s3d_navigation.rs"]
mod s3d;
#[cfg(not(target_arch = "wasm32"))]
#[path = "support/s3d_audio.rs"]
mod s3d_audio;
#[cfg(not(target_arch = "wasm32"))]
#[path = "support/s3e_character.rs"]
mod s3e;
#[cfg(not(target_arch = "wasm32"))]
#[path = "support/s3e_cells.rs"]
mod s3e_cells;
mod s3a {
    use engine_runtime::character_animation::*;
    use std::{collections::BTreeMap, sync::Arc};
    pub fn asset() -> CharacterAsset {
        CharacterAsset {
            schema_version: "character3d.v1".into(),
            asset_id: "rig".into(),
            source_digest: format!("sha256:{}", "0".repeat(64)),
            recipe: CHARACTER_RECIPE.into(),
            joints: vec![
                CharacterJoint {
                    key: "root".into(),
                    parent: None,
                    bind: JointPose::default(),
                },
                CharacterJoint {
                    key: "hand".into(),
                    parent: Some(0),
                    bind: JointPose {
                        translation: [0., 1., 0.],
                        ..Default::default()
                    },
                },
            ],
            skins: vec![],
            clips: vec![AnimationClip3D {
                key: "walk".into(),
                duration: 2.,
                tracks: vec![AnimationTrack {
                    joint: 0,
                    property: TrackProperty::Translation,
                    interpolation: KeyInterpolation::Linear,
                    times: vec![0., 0.25, 2.],
                    width: 3,
                    values: vec![0., 0., 0., 1., 0., 0., 8., 0., 0.],
                }],
                events: vec![
                    AnimationEvent3D {
                        time: 0.,
                        name: "start".into(),
                    },
                    AnimationEvent3D {
                        time: 0.25,
                        name: "left".into(),
                    },
                    AnimationEvent3D {
                        time: 0.25,
                        name: "sound".into(),
                    },
                    AnimationEvent3D {
                        time: 1.25,
                        name: "right".into(),
                    },
                ],
            }],
        }
    }
    #[test]
    fn nonuniform_linear_step_cubic_and_shortest_rotation_match_scalar_reference() {
        let asset = asset();
        asset.validate().unwrap();
        assert_eq!(
            asset.sample(0, 1.125).unwrap().joints[0].translation,
            [4.5, 0., 0.]
        );
        let mut track = asset.clips[0].tracks[0].clone();
        track.interpolation = KeyInterpolation::Step;
        assert_eq!(track.sample(1.125).unwrap(), vec![1., 0., 0.]);
        let cubic = AnimationTrack {
            joint: 0,
            property: TrackProperty::Translation,
            interpolation: KeyInterpolation::CubicSpline,
            times: vec![0., 2.],
            width: 3,
            values: vec![
                0., 0., 0., 0., 0., 0., 1., 0., 0., 1., 0., 0., 2., 0., 0., 0., 0., 0.,
            ],
        };
        assert_eq!(cubic.sample(1.).unwrap(), vec![1., 0., 0.]);
        let rotation = AnimationTrack {
            joint: 0,
            property: TrackProperty::Rotation,
            interpolation: KeyInterpolation::Linear,
            times: vec![0., 2.],
            width: 4,
            values: vec![0., 0., 0., 1., 0., 0., 0., -1.],
        };
        assert!((rotation.sample(1.).unwrap()[3].abs() - 1.).abs() < 1e-6);
        let global = asset
            .global_matrices(&asset.sample(0, 1.125).unwrap())
            .unwrap();
        assert_eq!([global[1][12], global[1][13], global[1][14]], [4.5, 1., 0.]);
    }
    #[test]
    fn loops_pause_seek_and_equal_time_events_have_defined_order_and_no_replay() {
        let clip = &asset().clips[0];
        let mut clock = ClipClock::new(true);
        let got = clock
            .advance(clip, 4.25)
            .unwrap()
            .iter()
            .map(|e| (e.cycle, e.name.clone()))
            .collect::<Vec<_>>();
        assert_eq!(
            got,
            vec![
                (0, "start".into()),
                (0, "left".into()),
                (0, "sound".into()),
                (0, "right".into()),
                (1, "start".into()),
                (1, "left".into()),
                (1, "sound".into()),
                (1, "right".into()),
                (2, "start".into()),
                (2, "left".into()),
                (2, "sound".into())
            ]
        );
        clock.paused = true;
        assert!(clock.advance(clip, 1.).unwrap().is_empty());
        assert_eq!(clock.time, 4.25);
        clock.seek(1.25).unwrap();
        clock.paused = false;
        assert!(clock.advance(clip, 0.1).unwrap().is_empty());
        let before = clock.time;
        assert!(clock.advance(clip, 100000.).is_err());
        assert_eq!(clock.time, before);
        let mut finite = ClipClock::new(false);
        assert_eq!(finite.advance(clip, 10.).unwrap().len(), 4);
        assert!(finite.advance(clip, 10.).unwrap().is_empty());
    }
    #[test]
    fn malformed_skeleton_keys_times_rotation_and_weights_are_rejected() {
        let original = asset();
        let mut a = original.clone();
        a.joints[0].parent = Some(1);
        assert!(a.validate().is_err());
        let mut a = original.clone();
        a.joints[1].key = "root".into();
        assert!(a.validate().is_err());
        let mut a = original.clone();
        a.clips[0].tracks[0].times[1] = 0.;
        assert!(a.validate().is_err());
        let mut a = original.clone();
        a.joints[1].bind.rotation = [0.; 4];
        assert!(a.validate().is_err());
        let mut a = original;
        a.skins.push(CharacterSkin {
            mesh_joint: 0,
            mesh_asset_id: "mesh".into(),
            joints: vec![0],
            inverse_bind: vec![JointPose::default().matrix()],
            influences: vec![VertexInfluence {
                joints: [0; 4],
                weights: [0.1; 4],
            }],
            morph_targets: vec![],
            morph_weights: vec![],
        });
        assert!(a.validate().is_err());
    }
    #[test]
    fn graph_transition_mask_additive_and_offscreen_fixed_ticks() {
        let mut asset = asset();
        let mut idle = asset.clips[0].clone();
        idle.key = "idle".into();
        idle.tracks.clear();
        idle.events.clear();
        asset.clips.push(idle);
        let graph = AnimationGraph3D {
            initial: "idle".into(),
            parameters: BTreeMap::from([("speed".into(), 0.)]),
            states: vec![
                AnimationState3D {
                    key: "idle".into(),
                    clip: "idle".into(),
                    looping: true,
                },
                AnimationState3D {
                    key: "moving".into(),
                    clip: "walk".into(),
                    looping: true,
                },
            ],
            transitions: vec![AnimationTransition3D {
                from: "idle".into(),
                to: "moving".into(),
                parameter: "speed".into(),
                minimum: 0.5,
                blend_seconds: 0.5,
            }],
            layers: vec![],
        };
        let mut animator = Animator3D::new(Arc::new(asset.clone()), graph).unwrap();
        assert!(animator.set_parameter("typo", 1.).is_err());
        animator.set_parameter("speed", 1.).unwrap();
        animator.tick(1, 0.25).unwrap();
        assert_eq!(animator.state(), "moving");
        assert_eq!(animator.pose.joints[0].translation[0], 0.5);
        assert_eq!(animator.events.len(), 3);
        animator.tick(2, 0.25).unwrap();
        assert!((animator.pose.joints[0].translation[0] - 2.).abs() < 1e-6);
        assert!(animator.tick(2, 0.25).is_err());
        animator.set_paused(true);
        animator.tick(3, 0.25).unwrap();
        assert!(animator.events.is_empty());
        let mut base = asset.sample(1, 0.).unwrap();
        let layer = asset.sample(0, 0.25).unwrap();
        let reference = base.clone();
        blend_pose(&mut base, &layer, 0.5, &[1], Some(&reference)).unwrap();
        assert_eq!(base.joints[0].translation[0], 0.);
        blend_pose(&mut base, &layer, 0.5, &[0], Some(&reference)).unwrap();
        assert_eq!(base.joints[0].translation[0], 0.5);
    }
    #[test]
    fn cooked_package_generation_checked_sdk_intent_commits_real_joint_pose_and_events() {
        use engine_runtime::{
            character_animation::session::CharacterModule, ids::EntityId,
            project_runtime_session::ProjectRuntimeMutationBuffer,
            runtime_instance_loader::RuntimeInstanceLoader, runtime_package::*,
            runtime_package_builder::*, scene_loader::load_scene_into_world, spatial_contract::*,
        };
        use project_runtime_sdk::ProjectRuntimeSpatialIntent as I;
        use serde_json::json;
        let asset = asset();
        let bytes = serde_json::to_vec(&asset).unwrap();
        let mut input = RuntimePackageBuildInput::new(RuntimeProjectInfo::explicit_empty(
            "s3a",
            "S3 animation",
            "1",
        ));
        let mapping = engine_runtime::input_mapping::InputMappingAsset::gameplay_default();
        input.input_mappings.push(RuntimePackageSourceJson {
            id: mapping.asset_id.clone(),
            document: serde_json::to_value(mapping).unwrap(),
        });
        input.assets.push(
            RuntimePackageSourceAsset::new(
                "rig",
                "Rig",
                "character3d",
                "Assets/rig.asset",
                "cooked/rig.asset",
            )
            .with_runtime_payload(bytes),
        );
        let t = json!({"translation":[0,0,0],"rotationXyzw":[0,0,0,1],"scale":[1,1,1]});
        let entity = |id: &str, parent: Option<&str>| json!({"schemaVersion":"runtime-entity.v2","id":id,"name":id,"enabled":true,"parentId":parent,"siblingOrder":0,"spatial":{"domain":"world3d","transform":t},"components":[]});
        let mut root = entity("character", None);
        root["components"] = json!([{"componentType":"engine.character3d","data":{"schemaVersion":"character3d-binding.v1","character":{"id":"rig","type":"character3d"},"joints":[{"entityRef":"root"},{"entityRef":"hand"}],"graph":null,"playOnAwake":false}}]);
        let scene = json!({"schemaVersion":"runtime-scene.v2","id":"main","name":"Main","gravity":0,"background":"#000000","skyColor":"#000000","mainCamera":null,"entities":[root,entity("root",Some("character")),entity("hand",Some("root"))]});
        input.scenes.push(
            match decode_runtime_scene(&serde_json::to_vec(&scene).unwrap()).unwrap() {
                RuntimeSceneDocument::Spatial(s) => s.into(),
                _ => unreachable!(),
            },
        );
        let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../.artifacts/371/s3-simulation-20261026-084317")
            .join(format!("04-runtime-package-{}", std::process::id()));
        let report = RuntimePackageBuilder::build(
            &RuntimePackageBuildRequest::dev_desktop(&output, "main"),
            &input,
        );
        assert_eq!(
            report.status,
            RuntimePackageBuildStatus::Success,
            "{report:?}"
        );
        let package = load_runtime_package(&output).value.unwrap();
        let mut world = load_scene_into_world(&package.active_scene).value.unwrap();
        let mut loader = RuntimeInstanceLoader::from_package(&package);
        let mut module = CharacterModule::default();
        let character = EntityId::from("character");
        world.begin_spatial_tick(1);
        module
            .advance(&mut world, loader.asset_loader_mut(), 1, 0.25)
            .unwrap();
        world.finish_spatial_tick();
        assert_eq!(
            world
                .transform3d(&EntityId::from("root"))
                .unwrap()
                .translation,
            [0.; 3]
        );
        let generation = world.entity_generation(&character).unwrap();
        let mut buffer = ProjectRuntimeMutationBuffer::default();
        buffer.spatial_command(
            character.clone(),
            generation,
            I::CharacterPlay {
                state: "walk".into(),
                reset: true,
            },
        );
        buffer.prepare(&world).unwrap().commit(&mut world).unwrap();
        world.begin_spatial_tick(2);
        module
            .advance(&mut world, loader.asset_loader_mut(), 2, 0.25)
            .unwrap();
        world.finish_spatial_tick();
        assert_eq!(
            world
                .transform3d(&EntityId::from("root"))
                .unwrap()
                .translation,
            [1., 0., 0.]
        );
        let frame = world.character_pose(&character).unwrap();
        assert_eq!(frame.events.len(), 3);
        assert_eq!(
            world.character_joint_matrix(&character, "hand").unwrap()[12],
            1.
        );
        let mut stale = ProjectRuntimeMutationBuffer::default();
        stale.spatial_command(
            character.clone(),
            generation + 1,
            I::CharacterPaused { paused: true },
        );
        assert!(stale.prepare(&world).is_err());
        let mut pause = ProjectRuntimeMutationBuffer::default();
        pause.spatial_command(
            character.clone(),
            generation,
            I::CharacterPaused { paused: true },
        );
        pause.prepare(&world).unwrap().commit(&mut world).unwrap();
        world.begin_spatial_tick(3);
        module
            .advance(&mut world, loader.asset_loader_mut(), 3, 0.25)
            .unwrap();
        world.finish_spatial_tick();
        assert!(world.character_pose(&character).unwrap().events.is_empty());
        assert_eq!(
            frame.events.len(),
            3,
            "old committed snapshots remain immutable"
        );
    }
}
mod s3b {
    use engine_runtime::character_animation::*;
    use std::sync::Arc;
    pub(super) fn deformation() -> SkinDeformation3D {
        let mut moved = engine_runtime::render_contract::IDENTITY_MATRIX;
        moved[12] = 2.;
        let skin = CharacterSkin {
            mesh_joint: 0,
            mesh_asset_id: "mesh".into(),
            joints: vec![0, 1],
            inverse_bind: vec![engine_runtime::render_contract::IDENTITY_MATRIX; 2],
            influences: vec![VertexInfluence {
                joints: [0, 1, 0, 0],
                weights: [0.5, 0.5, 0., 0.],
            }],
            morph_targets: vec![MorphTarget3D {
                position_deltas: vec![[0., 1., 0.]],
                normal_deltas: vec![[0.; 3]],
            }],
            morph_weights: vec![0.5],
        };
        SkinDeformation3D::new(
            Arc::new(SkinVertexData3D::from_skin(&skin)),
            &[engine_runtime::render_contract::IDENTITY_MATRIX, moved],
            &[engine_runtime::render_contract::IDENTITY_MATRIX; 2],
            &[0.5],
            &[0.],
            false,
        )
        .unwrap()
    }
    #[test]
    fn morph_before_skin_previous_pose_bounds_and_packed_stream_rejections() {
        let skin = deformation();
        let (p, n) = skin
            .reference_vertex(0, [1., 0., 0.], [0., 0., 1.], false)
            .unwrap();
        assert_eq!(p, [2., 0.5, 0.]);
        assert_eq!(n, [0., 0., 1.]);
        assert_eq!(
            skin.reference_vertex(0, [1., 0., 0.], [0., 0., 1.], true)
                .unwrap()
                .0,
            [1., 0., 0.]
        );
        let bounds = engine_runtime::render_contract::GeometryBounds {
            min: [-1.; 3],
            max: [1.; 3],
            sphere_center: [0.; 3],
            sphere_radius: 3f32.sqrt(),
        };
        let dynamic = skin.conservative_bounds(&bounds);
        assert!(dynamic.max[0] >= 3. && dynamic.max[1] >= 1.5);
        let mut bad = skin.clone();
        Arc::make_mut(&mut bad.current_weights).copy_from_slice(&2f32.to_le_bytes());
        assert!(bad.validate().is_err());
        let mut bad = skin.clone();
        Arc::make_mut(&mut Arc::make_mut(&mut bad.source).influences)[..4]
            .copy_from_slice(&99u32.to_le_bytes());
        assert!(bad.validate().is_err());
        assert_eq!(
            skin.reference_vertex(0, [1., 0., 0.], [0., 0., 1.], false)
                .unwrap()
                .0,
            p,
            "detached malformed input cannot mutate old skin"
        );
    }
    #[test]
    fn two_bone_ik_preserves_lengths_rejects_degenerate_and_clamps_unreachable() {
        let (joint, end, reach) = two_bone_ik(
            [0.; 3],
            [1., 0., 0.],
            [2., 0., 0.],
            [1., 1., 0.],
            [0., 0., 1.],
        )
        .unwrap();
        assert_eq!(reach, IkReach::Reached);
        let length = |a: [f32; 3], b: [f32; 3]| {
            a.iter()
                .zip(b)
                .map(|(a, b)| (a - b).powi(2))
                .sum::<f32>()
                .sqrt()
        };
        assert!((length(joint, [0.; 3]) - 1.).abs() < 1e-5);
        assert!((length(joint, end) - 1.).abs() < 1e-5);
        assert_eq!(end, [1., 1., 0.]);
        assert_eq!(
            two_bone_ik(
                [0.; 3],
                [1., 0., 0.],
                [2., 0., 0.],
                [5., 0., 0.],
                [0., 1., 0.]
            )
            .unwrap()
            .2,
            IkReach::Clamped
        );
        assert!(two_bone_ik([0.; 3], [0.; 3], [1., 0., 0.], [2., 0., 0.], [0., 1., 0.]).is_err());
        assert!(two_bone_ik(
            [0.; 3],
            [1., 0., 0.],
            [2., 0., 0.],
            [1., 0., 0.],
            [2., 0., 0.]
        )
        .is_err());
    }
    #[test]
    fn retarget_requires_explicit_rig_mapping_and_visual_lod_never_skips_gameplay_clock() {
        let source = super::s3a::asset();
        let mut target = source.clone();
        target.asset_id = "target".into();
        target.joints[1].bind.translation = [0., 2., 0.];
        let map = RetargetMap3D {
            source_asset: "rig".into(),
            target_asset: "target".into(),
            joints: vec![RetargetJoint3D {
                source: "root".into(),
                target: "root".into(),
                translation_scale: 2.,
            }],
        };
        let pose = map
            .apply(&source, &target, &source.sample(0, 0.25).unwrap())
            .unwrap();
        assert_eq!(pose.joints[0].translation, [2., 0., 0.]);
        assert_eq!(pose.joints[1].translation, [0., 2., 0.]);
        target.asset_id = "wrong".into();
        assert!(map.apply(&source, &target, &pose).is_err());
        let lod = AnimationLod3D { visual_interval: 4 };
        let mut clock = ClipClock::new(true);
        let mut events = 0;
        for tick in 1..=8 {
            events += clock.advance(&source.clips[0], 0.25).unwrap().len();
            assert_eq!(lod.evaluate_visual(tick, true), tick % 4 == 0);
            assert!(!lod.evaluate_visual(tick, false));
        }
        assert_eq!(events, 5);
    }
}
