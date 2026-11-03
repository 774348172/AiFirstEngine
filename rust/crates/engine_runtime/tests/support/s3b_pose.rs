use engine_runtime::spatial_contract::{decode_runtime_scene, RuntimeSceneDocument};
use engine_runtime::{
    character_animation::session::CharacterModule, character_animation::*, ids::EntityId,
    runtime_instance_loader::RuntimeInstanceLoader, runtime_package::*, runtime_package_builder::*,
    scene_loader::load_scene_into_world,
};
use serde_json::json;
use std::sync::Arc;

#[test]
#[ignore = "explicit original BM05 published package; actual 1x/1.5x/2x character owner capacity"]
fn s5d_original_bm05_character_capacity_keeps_full_rigs_and_presentation_read_only() {
    use std::collections::BTreeMap;
    fn remap(value: &mut serde_json::Value, ids: &BTreeMap<String, String>) {
        match value {
            serde_json::Value::String(id) => {
                if let Some(new) = ids.get(id) {
                    *id = new.clone();
                }
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    remap(value, ids);
                }
            }
            serde_json::Value::Object(values) => {
                for value in values.values_mut() {
                    remap(value, ids);
                }
            }
            _ => {}
        }
    }
    let root = std::path::PathBuf::from(std::env::var_os("AIFE_BM05_LOAD_PACKAGE").unwrap());
    let package = load_runtime_package(root).value.unwrap();
    let original = &package.active_scene;
    let mut groups = BTreeMap::<String, Vec<_>>::new();
    for entity in &original.entities {
        if let Some(binding) = entity
            .components
            .iter()
            .find(|c| c.component_type == "engine.character3d")
        {
            groups
                .entry(binding.data["character"]["id"].as_str().unwrap().to_owned())
                .or_default()
                .push(entity);
        }
    }
    assert_eq!(groups.len(), 10);
    assert!(groups.values().all(|group| group.len() == 10));
    let mut reference = BTreeMap::new();
    for count in [100usize, 150, 200] {
        let mut scene = original.clone();
        for (rig, roots) in &groups {
            for template in roots.iter().take((count - 100) / 10) {
                let family: Vec<_> = original
                    .entities
                    .iter()
                    .filter(|entity| {
                        entity.id == template.id
                            || entity.id.starts_with(&format!("{}-joint-", template.id))
                    })
                    .collect();
                assert_eq!(
                    family.len(),
                    65,
                    "{rig} must retain root and all64 authored joint entities"
                );
                let ids: BTreeMap<_, _> = family
                    .iter()
                    .map(|entity| (entity.id.clone(), format!("pressure-{}", entity.id)))
                    .collect();
                for entity in family {
                    let mut value = serde_json::to_value(entity).unwrap();
                    remap(&mut value, &ids);
                    scene.entities.push(serde_json::from_value(value).unwrap());
                }
            }
        }
        let mut world = load_scene_into_world(&scene).value.unwrap();
        let mut loader = RuntimeInstanceLoader::from_package(&package);
        assert!(loader.asset_loader_mut().mount_bundle("startup"));
        let mut module = CharacterModule::default();
        let ids: Vec<_> = scene
            .entities
            .iter()
            .filter(|entity| {
                entity
                    .components
                    .iter()
                    .any(|c| c.component_type == "engine.character3d")
            })
            .map(|e| EntityId::from(e.id.clone()))
            .collect();
        assert_eq!(ids.len(), count);
        assert_eq!(
            scene
                .entities
                .iter()
                .filter(|e| e.id.starts_with("static-"))
                .count(),
            1000
        );
        let start = std::time::Instant::now();
        for tick in 1..=2 {
            world.begin_spatial_tick(tick);
            module
                .advance(&mut world, loader.asset_loader_mut(), tick, 1. / 60.)
                .unwrap();
            world.finish_spatial_tick();
            for id in &ids {
                let pose = world.character_pose(id).unwrap();
                assert_eq!(pose.tick, tick);
                assert_eq!(pose.current.joints.len(), 64);
                assert_eq!(pose.joints.len(), 64);
                assert_eq!(pose.skin_sources.len(), 1);
                assert_eq!(pose.skin_sources[0].vertex_count, 15000);
                assert_eq!(pose.asset.skins[0].influences.len(), 15000);
                assert!(pose
                    .current
                    .joints
                    .iter()
                    .all(|joint| joint.validate().is_ok()));
                assert!(pose
                    .joints
                    .iter()
                    .all(|joint| world.transform3d(joint).is_some()));
                if !id.as_str().starts_with("pressure-") {
                    let key = (tick, id.clone());
                    if count == 100 {
                        reference.insert(key, pose.current.clone());
                    } else {
                        assert_eq!(
                            reference.get(&key),
                            Some(&pose.current),
                            "pressure changed an original character's actual sampled pose"
                        );
                    }
                }
            }
        }
        let committed: Vec<_> = ids
            .iter()
            .map(|id| world.character_pose(id).unwrap())
            .collect();
        for rate in [30u64, 60, 120] {
            for sample in [0, rate / 2, rate - 1] {
                world
                    .prepare_spatial_presentation(rate * 10 + sample, sample as f32 / rate as f32)
                    .unwrap();
                for (id, old) in ids.iter().zip(&committed) {
                    assert!(
                        Arc::ptr_eq(old, &world.character_pose(id).unwrap()),
                        "presentation advanced the private animation clock"
                    );
                }
            }
        }
        eprintln!("BM05 original package capacity: characters={count}, rigs=10, joints={}, actual15Kvertex skin per actor, two real owner ticks and read-only30/60/120Hz probes in{:.3}s; no fullK3/GPU/physics/performance claim",count*64,start.elapsed().as_secs_f64());
    }
}

#[test]
fn cooked_retarget_visual_ik_lod_and_gameplay_attachment_use_distinct_committed_poses() {
    let source = super::s3a::asset();
    let mut target = source.clone();
    target.asset_id = "target".into();
    target.clips.clear();
    target.joints[1].bind.translation = [0., 1., 0.];
    target.joints.push(CharacterJoint {
        key: "end".into(),
        parent: Some(1),
        bind: JointPose {
            translation: [0., 1., 0.],
            ..Default::default()
        },
    });
    let mut input = RuntimePackageBuildInput::new(RuntimeProjectInfo::explicit_empty(
        "s3b-pose", "S3 pose", "1",
    ));
    let mapping = engine_runtime::input_mapping::InputMappingAsset::gameplay_default();
    input.input_mappings.push(RuntimePackageSourceJson {
        id: mapping.asset_id.clone(),
        document: serde_json::to_value(mapping).unwrap(),
    });
    for asset in [&source, &target] {
        input.assets.push(
            RuntimePackageSourceAsset::new(
                &asset.asset_id,
                &asset.asset_id,
                "character3d",
                format!("Assets/{}.asset", asset.asset_id),
                format!("cooked/{}.asset", asset.asset_id),
            )
            .with_runtime_payload(serde_json::to_vec(asset).unwrap()),
        );
    }
    let entity = |id: &str, parent: Option<&str>| json!({"schemaVersion":"runtime-entity.v2","id":id,"name":id,"enabled":true,"parentId":parent,"siblingOrder":0,"spatial":{"domain":"world3d","transform":{"translation":[0,0,0],"rotationXyzw":[0,0,0,1],"scale":[1,1,1]}},"components":[]});
    let mut character = entity("character", None);
    character["components"] = json!([{"componentType":"engine.character3d","data":{"schemaVersion":"character3d-binding.v1","character":{"id":"target","type":"character3d"},"joints":[{"entityRef":"root"},{"entityRef":"hand"},{"entityRef":"end"}],"playOnAwake":true,"visualInterval":4,"retarget":{"source":{"id":"rig","type":"character3d"},"mapping":{"sourceAsset":"rig","targetAsset":"target","joints":[{"source":"root","target":"root","translationScale":1}]}},"visualIk":[{"root":"root","middle":"hand","end":"end","target":[1,1,0],"pole":[0,0,1],"weight":1}]}}]);
    let scene = json!({"schemaVersion":"runtime-scene.v2","id":"main","name":"Main","gravity":0,"background":"#000000","skyColor":"#000000","mainCamera":null,"entities":[character,entity("root",Some("character")),entity("hand",Some("root")),entity("end",Some("hand"))]});
    input.scenes.push(
        match decode_runtime_scene(&serde_json::to_vec(&scene).unwrap()).unwrap() {
            RuntimeSceneDocument::Spatial(s) => s.into(),
            _ => unreachable!(),
        },
    );
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../.artifacts/371/s3-simulation-20261026-084317")
        .join(format!("s3b-pose-package-{}", std::process::id()));
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
    let id = EntityId::from("character");
    world.begin_spatial_tick(1);
    module
        .advance(&mut world, loader.asset_loader_mut(), 1, 0.25)
        .unwrap();
    world.finish_spatial_tick();
    let old = world.character_pose(&id).unwrap();
    let gameplay = world.character_joint_matrix(&id, "end").unwrap();
    let visual = world.character_visual_joint_matrix(&id, "end").unwrap();
    assert!((gameplay[12] - 1.).abs() < 1e-5 && (gameplay[13] - 2.).abs() < 1e-5);
    assert!((visual[12] - 1.).abs() < 1e-4 && (visual[13] - 1.).abs() < 1e-4);
    assert_eq!(old.events.len(), 3);
    world.begin_spatial_tick(2);
    module
        .advance(&mut world, loader.asset_loader_mut(), 2, 0.25)
        .unwrap();
    world.finish_spatial_tick();
    let next = world.character_pose(&id).unwrap();
    assert!(!next.visual_updated);
    assert_eq!(next.visual, old.visual);
    assert_ne!(next.current, old.current);
    // Presenting the same tick at multiple rates cannot advance the private animator.
    for rate in [30, 60, 120] {
        for sample in 0..rate {
            world
                .prepare_spatial_presentation(sample as u64, sample as f32 / rate as f32)
                .unwrap();
            assert!(Arc::ptr_eq(&world.character_pose(&id).unwrap(), &next));
        }
    }
    assert_eq!(old.current.joints[0].translation, [1., 0., 0.]);
}

#[test]
fn authored_graph_layer_is_evaluated_and_pose_ik_rejects_invalid_chain() {
    let mut asset = super::s3a::asset();
    let mut layer = asset.clips[0].clone();
    layer.key = "upper".into();
    layer.tracks[0].joint = 1;
    asset.clips.push(layer);
    let graph = AnimationGraph3D {
        initial: "walk".into(),
        parameters: [("amount".into(), 0.5)].into(),
        states: vec![AnimationState3D {
            key: "walk".into(),
            clip: "walk".into(),
            looping: true,
        }],
        transitions: vec![],
        layers: vec![AnimationLayer3D {
            clip: "upper".into(),
            weight: 1.,
            weight_parameter: Some("amount".into()),
            mask: vec!["hand".into()],
            additive: true,
        }],
    };
    let mut animator = Animator3D::new(Arc::new(asset.clone()), graph).unwrap();
    animator.tick(1, 0.25).unwrap();
    assert_eq!(animator.pose.joints[0].translation, [1., 0., 0.]);
    assert_eq!(animator.pose.joints[1].translation, [0.5, 1., 0.]);
    assert!(PoseIk3D {
        root: "root".into(),
        middle: "hand".into(),
        end: "hand".into(),
        target: [1., 1., 0.],
        pole: [0., 0., 1.],
        weight: 1.
    }
    .apply(&asset, &mut animator.pose)
    .is_err());
}

#[test]
fn zero_tick_skin_interpolates_and_keeps_last_presented_history_without_advancing_simulation() {
    use engine_runtime::{
        render_command::apply_batch,
        render_extract::RenderExtractContext,
        render_state::{RenderProxyPayload, RenderSceneState},
    };
    let mut asset = super::s3a::asset();
    asset.clips[0].tracks[0].joint = 1;
    asset.skins.push(CharacterSkin {
        mesh_joint: 0,
        mesh_asset_id: "mesh".into(),
        joints: vec![1],
        inverse_bind: vec![engine_runtime::render_contract::IDENTITY_MATRIX],
        influences: vec![VertexInfluence {
            joints: [0; 4],
            weights: [1., 0., 0., 0.],
        }],
        morph_targets: vec![],
        morph_weights: vec![],
    });
    let mut input = RuntimePackageBuildInput::new(RuntimeProjectInfo::explicit_empty(
        "s3e-interpolation",
        "S3 interpolation",
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
            "rig",
            "character3d",
            "Assets/rig.asset",
            "cooked/rig.asset",
        )
        .with_runtime_payload(serde_json::to_vec(&asset).unwrap()),
    );
    // This owner test consumes renderer identity only; GPU geometry is covered separately.
    for (id, kind) in [("mesh", "mesh"), ("mat", "material")] {
        input.assets.push(RuntimePackageSourceAsset::new(
            id,
            id,
            kind,
            format!("Assets/{id}.json"),
            format!("cooked/{id}.json"),
        ));
    }
    let entity = |id: &str, parent: Option<&str>| json!({"schemaVersion":"runtime-entity.v2","id":id,"name":id,"enabled":true,"parentId":parent,"siblingOrder":0,"spatial":{"domain":"world3d","transform":{"translation":[0,0,0],"rotationXyzw":[0,0,0,1],"scale":[1,1,1]}},"components":[]});
    let mut owner = entity("owner", None);
    owner["components"] = json!([{"componentType":"engine.character3d","data":{"schemaVersion":"character3d-binding.v1","character":{"id":"rig","type":"character3d"},"joints":[{"entityRef":"root"},{"entityRef":"hand"}],"playOnAwake":true}}]);
    let mut mesh_entity = entity("root", Some("owner"));
    mesh_entity["spatial"]["meshRenderer"] = json!({"schemaVersion":"mesh-renderer3d.v1","mesh":{"id":"mesh","type":"mesh"},"materials":{"surface":{"id":"mat","type":"material"}},"visible":true,"layers":["world"]});
    let camera: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/three_d_a0/runtime-scene-v2.json")).unwrap();
    let camera = camera["entities"][1].clone();
    let scene = json!({"schemaVersion":"runtime-scene.v2","id":"main","name":"Main","gravity":0,"background":"#000000","skyColor":"#000000","mainCamera":"camera","entities":[owner,mesh_entity,entity("hand",Some("root")),camera]});
    let RuntimeSceneDocument::Spatial(scene) =
        decode_runtime_scene(&serde_json::to_vec(&scene).unwrap()).unwrap()
    else {
        unreachable!()
    };
    input.scenes.push(scene.into());
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../.artifacts/371/s3-simulation-20261026-084317")
        .join(format!("s3e-interpolation-{}", std::process::id()));
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
    let owner = EntityId::from("owner");
    let mesh = EntityId::from("root");
    for tick in 1..=2 {
        world.begin_spatial_tick(tick);
        module
            .advance(&mut world, loader.asset_loader_mut(), tick, 0.25)
            .unwrap();
        let published_spatial_delta = world.spatial_delta();
        world.finish_spatial_tick();
        assert!(
            Arc::ptr_eq(&world.spatial_delta(), &published_spatial_delta),
            "Publishing a skin must not schedule another spatial commit"
        );
        if tick == 1 {
            world.prepare_spatial_presentation(1, 1.).unwrap();
        }
    }
    let committed = world.character_pose(&owner).unwrap();
    assert!(!committed.history_reset);
    assert_eq!(committed.current.joints[1].translation[0], 2.);
    let delta = world.prepare_spatial_presentation(2, 0.5).unwrap();
    assert!(
        delta.changed.contains(&mesh),
        "Skin-only updates must participate in PresentationDelta"
    );
    let first = world.character_skin(&mesh).unwrap();
    assert!(
        (first
            .reference_vertex(0, [0.; 3], [0., 0., 1.], false)
            .unwrap()
            .0[0]
            - 1.5)
            .abs()
            < 1e-6
    );
    assert!(
        (first
            .reference_vertex(0, [0.; 3], [0., 0., 1.], true)
            .unwrap()
            .0[0]
            - 1.)
            .abs()
            < 1e-6
    );
    let committed_spatial_delta = world.spatial_delta();
    let committed_mesh_pose = world.committed_spatial_pose(&mesh).unwrap();
    let hand = EntityId::from("hand");
    let committed_hand_pose = world.committed_spatial_pose(&hand).unwrap();
    let mut extract = RenderExtractContext::new();
    let mut scene = RenderSceneState::new();
    let mut queue = extract.extract_world_dirty(2, &mut world, &scene);
    let merged = queue.normalize_merge(&scene);
    assert!(apply_batch(&mut scene, &merged).is_empty());
    world.prepare_spatial_presentation(3, 0.75).unwrap();
    assert!(
        Arc::ptr_eq(&world.spatial_delta(), &committed_spatial_delta),
        "Skin interpolation must not publish another committed SpatialDelta"
    );
    assert!(Arc::ptr_eq(
        &world.committed_spatial_pose(&mesh).unwrap(),
        &committed_mesh_pose
    ));
    assert!(Arc::ptr_eq(
        &world.committed_spatial_pose(&hand).unwrap(),
        &committed_hand_pose
    ));
    let second = world.character_skin(&mesh).unwrap();
    assert!(
        (second
            .reference_vertex(0, [0.; 3], [0., 0., 1.], false)
            .unwrap()
            .0[0]
            - 1.75)
            .abs()
            < 1e-6
    );
    assert_eq!(second.previous_matrices, first.current_matrices);
    let mut queue = extract.extract_world_dirty(3, &mut world, &scene);
    let merged = queue.normalize_merge(&scene);
    assert!(apply_batch(&mut scene, &merged).is_empty());
    let proxy = scene.proxy(scene.proxy_for_source(&mesh).unwrap()).unwrap();
    let RenderProxyPayload::Mesh3D(projection) = &proxy.payload else {
        panic!("The skinned mesh must retain its actual render projection")
    };
    assert!(Arc::ptr_eq(
        projection.deformation.as_ref().unwrap(),
        &second
    ));
    assert!(Arc::ptr_eq(
        &world.character_pose(&owner).unwrap(),
        &committed
    ));
    assert_eq!(committed.tick, 2);
    let mut buffer =
        engine_runtime::project_runtime_session::ProjectRuntimeMutationBuffer::default();
    buffer.spatial_command(
        owner.clone(),
        world.entity_generation(&owner).unwrap(),
        project_runtime_sdk::ProjectRuntimeSpatialIntent::CharacterSeek { seconds: 0. },
    );
    buffer.prepare(&world).unwrap().commit(&mut world).unwrap();
    world.begin_spatial_tick(3);
    module
        .advance(&mut world, loader.asset_loader_mut(), 3, 0.25)
        .unwrap();
    world.finish_spatial_tick();
    world.prepare_spatial_presentation(4, 0.5).unwrap();
    let reset = world.character_skin(&mesh).unwrap();
    assert!(reset.history_reset);
    assert_eq!(reset.current_matrices, reset.previous_matrices);
    assert_ne!(first.current_matrices, reset.current_matrices);
    world.try_despawn_entity(&owner).unwrap();
    assert!(world.character_pose(&owner).is_none());
    assert!(world.character_skin(&mesh).is_none());
    assert!(world.character_joint_matrix(&owner, "root").is_err());
    // Retiring the World projection must not mutate an already published frame.
    assert_eq!(committed.tick, 2);
    assert_eq!(second.previous_matrices, first.current_matrices);
}
