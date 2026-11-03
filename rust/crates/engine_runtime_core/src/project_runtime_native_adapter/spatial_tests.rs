use super::*;
use crate::components::Hierarchy;
use crate::spatial_contract::{Camera3D, MeshRenderer3D, SpatialAssetRef, Transform3D};
use crate::world::World;
use project_runtime_sdk::{ProjectRuntimeSpatialIntent as Intent, ProjectRuntimeTransform3D};

#[test]
fn physics_batch_query_requires_the_actual_frame_loop_owner() {
    let world = world();
    let mut host = ReadOnlyWorldHost {
        world: WorldReadApi::new(&world),
        spawn_receipts: None,
        animator_snapshots: None,
        ui: None,
    };
    let request = serde_json::json!({
        "physics": {"queries": [{"kind":"raycast", "origin":[0.,4.,0.], "displacement":[0.,-8.,0.]}]},
        "all":[], "none":[]
    });
    let error = with_host_context(&mut host, |token| {
        project_runtime_sdk::call_host_json::<_, ProjectRuntimeWorldQueryResponse>(
            host_world_query,
            token,
            &request,
        )
    })
    .unwrap_err();
    assert_eq!(error.status, ProjectRuntimeAbiStatus::UNSUPPORTED,
        "a valid physics query must reach the host service boundary, never an empty fake world result");
}

#[test]
fn physics_batch_query_crosses_the_real_json_host_boundary_and_jolt_owner() {
    use project_runtime_sdk::physics_query3d::*;
    let mut world = world();
    world
        .try_insert_dynamic_component(
            "cube".into(),
            ComponentTypeId::new("engine.body3d"),
            crate::character_animation::runtime_json(
                serde_json::json!({"schemaVersion":"body3d.v1",
            "motion":"static","shape":{"kind":"box","halfExtents":[1,0.5,1]}}),
            ),
        )
        .unwrap();
    let mut physics = crate::physics_3d::PhysicsModule::default();
    world.begin_spatial_tick(9);
    physics.advance(&mut world, 9, 1. / 60.).unwrap();
    world.finish_spatial_tick();
    let mut queries = vec![
        PhysicsQuery3D::Raycast {
            origin: [0., 4., 0.],
            displacement: [0., -8., 0.]
        };
        200
    ];
    queries.extend(vec![
        PhysicsQuery3D::Sweep {
            origin: [0., 4., 0.],
            displacement: [0., -8., 0.],
            shape: PhysicsQueryShape3D::Sphere { radius: 0.25 },
            rotation_xyzw: [0., 0., 0., 1.]
        };
        50
    ]);
    let request: ProjectRuntimeWorldQueryRequest = serde_json::from_value(
        serde_json::json!({"physics":{"queries":queries},"all":[],"none":[]}),
    )
    .unwrap();
    let mut host = ReadOnlyWorldHost {
        world: WorldReadApi::new(&world).with_physics(Some(&physics)),
        spawn_receipts: None,
        animator_snapshots: None,
        ui: None,
    };
    let response = with_host_context(&mut host, |token| {
        project_runtime_sdk::call_host_json::<_, ProjectRuntimeWorldQueryResponse>(
            host_world_query,
            token,
            &request,
        )
    })
    .unwrap();
    assert!(
        response.entity_ids.is_empty()
            && response.entity_generations.is_empty()
            && response.cell.is_none()
    );
    let result = response.physics.unwrap();
    assert_eq!(result.tick, Some(9));
    assert_eq!(result.hits.len(), 250);
    for hit in result.hits {
        let hit = hit.unwrap();
        assert_eq!(hit.entity, "cube");
        assert_eq!(
            Some(hit.generation),
            world.entity_generation(&"cube".into())
        );
    }
    assert_eq!(world.physics3d_frame().tick, 9);
    assert_eq!(physics.solver_creation_count(), 1);
}

#[test]
fn s2_work_granularity_exact_entity_query_returns_only_current_incarnation() {
    let mut world = world();
    for i in 0..128 {
        spawn(&mut world, &format!("other-{i}"), None, true);
    }
    let mut host = ReadOnlyWorldHost {
        ui: None,
        world: WorldReadApi::new(&world),
        spawn_receipts: None,
        animator_snapshots: None,
    };
    let request: ProjectRuntimeWorldQueryRequest = serde_json::from_value(serde_json::json!({
        "entityId":"camera", "all":[], "none":[]
    }))
    .expect("exact identity query must be supported without enumerating the world");
    let response = host.query(request);
    assert_eq!(response.entity_ids, ["camera"]);
    assert_eq!(response.entity_generations.len(), 1);
    assert_eq!(
        response.entity_generations["camera"],
        world.entity_generation(&"camera".into()).unwrap()
    );
    let request =
        serde_json::from_value(serde_json::json!({"entityId":"absent","all":[],"none":[]}))
            .unwrap();
    assert!(host.query(request).entity_ids.is_empty());
    let invalid: ProjectRuntimeWorldQueryRequest = serde_json::from_value(
        serde_json::json!({"entityId":"camera","all":["engine.transform3d"],"none":[]}),
    )
    .unwrap();
    let error = with_host_context(&mut host, |token| {
        project_runtime_sdk::call_host_json::<_, ProjectRuntimeWorldQueryResponse>(
            host_world_query,
            token,
            &invalid,
        )
    })
    .unwrap_err();
    assert_eq!(error.status, ProjectRuntimeAbiStatus::INVALID_ARGUMENT);
    let first_generation = world.entity_generation(&"camera".into()).unwrap();
    world.try_despawn_entity(&"camera".into()).unwrap();
    spawn(&mut world, "camera", None, true);
    let generation = world.entity_generation(&"camera".into()).unwrap();
    assert_ne!(first_generation, generation);
    let mut context = LogicContext::new(
        1,
        1. / 60.,
        crate::logic_executor::RulePhase::FixedUpdate,
        crate::world_api::WorldWriteApi::new(&mut world),
    );
    let mut host = LogicWorldHost {
        context: &mut context,
    };
    let request: ProjectRuntimeWorldQueryRequest =
        serde_json::from_value(serde_json::json!({"entityId":"camera","all":[],"none":[]}))
            .unwrap();
    let response = with_host_context(&mut host, |token| {
        project_runtime_sdk::call_host_json::<_, ProjectRuntimeWorldQueryResponse>(
            host_world_query,
            token,
            &request,
        )
    })
    .unwrap();
    assert_eq!(response.entity_ids, ["camera"]);
    assert_eq!(response.entity_generations["camera"], generation);
    assert!(serde_json::to_vec(&response).unwrap().len() < 256);
    assert!(
        context.take_queries().is_empty(),
        "exact identity lookup enumerated the world"
    );
}

fn spawn(world: &mut World, name: &str, parent: Option<&str>, camera: bool) {
    let mut components = vec![ComponentValue::Transform3D(
        Transform3D::identity().canonicalize().unwrap(),
    )];
    if camera {
        components.push(ComponentValue::Camera3D(Camera3D::perspective_template()));
    } else {
        let asset = |name: &str, kind: &str| {
            SpatialAssetRef(crate::runtime_package::RuntimeAssetRef {
                id: name.into(),
                asset_type: kind.into(),
                guid: Some(format!("guid-{name}")),
                sub_asset: None,
            })
        };
        components.push(ComponentValue::MeshRenderer3D(MeshRenderer3D {
            schema_version: "mesh-renderer3d.v1".into(),
            mesh: asset("cube", "mesh"),
            materials: BTreeMap::from([("surface".into(), asset("blue", "material"))]),
            visible: true,
            layers: vec!["world".into()],
        }));
    }
    world.commit_prepared_entity(
        name.into(),
        name.into(),
        "spatial".into(),
        true,
        Hierarchy {
            parent_id: parent.map(EntityId::from),
            sibling_order: 0,
        },
        components,
    );
}
fn world() -> World {
    let mut world = World::new();
    spawn(&mut world, "cube", None, false);
    spawn(&mut world, "camera", None, true);
    world
}
fn append(buffer: &mut ProjectRuntimeMutationBuffer, world: &World, name: &str, intent: Intent) {
    // Exercise the actual DLL JSON DTO and native adapter, not only the buffer helper.
    let mutation = ProjectRuntimeDeferredMutation::Spatial {
        entity_id: name.into(),
        generation: world.entity_generation(&name.into()).unwrap(),
        intent,
    };
    let bytes = serde_json::to_vec(&mutation).unwrap();
    append_deferred_mutation(buffer, serde_json::from_slice(&bytes).unwrap()).unwrap();
}
fn moved() -> Intent {
    Intent::SetLocalTransform {
        transform: ProjectRuntimeTransform3D {
            translation: [1., 2., 3.],
            ..ProjectRuntimeTransform3D::identity()
        },
    }
}
fn material(slot: &str) -> Intent {
    Intent::SetMaterial {
        slot: slot.into(),
        material: project_runtime_sdk::ProjectRuntimeSpatialAssetRef {
            id: "red".into(),
            asset_type: "material".into(),
            guid: Some("guid-red".into()),
            sub_asset: None,
        },
    }
}

#[test]
fn spatial_wire_commits_transform_visibility_material_and_explicit_camera_atomically() {
    let mut world = world();
    let mut buffer = ProjectRuntimeMutationBuffer::new();
    for intent in [
        moved(),
        Intent::SetMeshVisible { visible: false },
        material("surface"),
    ] {
        append(&mut buffer, &world, "cube", intent);
    }
    append(&mut buffer, &world, "camera", Intent::SelectMainCamera {});
    let dirty = world.dirty_records().len();
    let prepared = buffer.prepare(&world).unwrap();
    assert_eq!(world.dirty_records().len(), dirty);
    assert!(world.main_camera().is_none());
    let report = prepared.commit(&mut world).unwrap();
    assert_eq!(report.committed_count, 4);
    assert_eq!(
        world.transform3d(&"cube".into()).unwrap().translation,
        [1., 2., 3.]
    );
    let mesh = world.mesh_renderer3d(&"cube".into()).unwrap();
    assert!(!mesh.visible);
    assert_eq!(mesh.materials["surface"].0.id, "red");
    assert_eq!(world.main_camera(), Some(&"camera".into()));
    assert_eq!(report.records[2].before, report.records[1].after);
}

#[test]
fn spatial_invalid_tail_rejects_whole_batch_without_dirty_writes() {
    let world = world();
    let invalid = [
        Intent::SetLocalTransform {
            transform: ProjectRuntimeTransform3D {
                rotation_xyzw: [0.; 4],
                ..ProjectRuntimeTransform3D::identity()
            },
        },
        Intent::SetLocalTransform {
            transform: ProjectRuntimeTransform3D {
                scale: [0., 1., 1.],
                ..ProjectRuntimeTransform3D::identity()
            },
        },
        material("missing-slot"),
        Intent::SelectMainCamera {},
    ];
    for intent in invalid {
        let dirty = world.dirty_records().len();
        let mut buffer = ProjectRuntimeMutationBuffer::new();
        append(&mut buffer, &world, "cube", moved());
        append(&mut buffer, &world, "cube", intent);
        let error = buffer.prepare(&world).unwrap_err();
        assert_ne!(error.code, "three_d.stale_entity");
        assert_eq!(error.report.committed_count, 0);
        assert_eq!(world.dirty_records().len(), dirty);
        assert_eq!(
            world.transform3d(&"cube".into()).unwrap().translation,
            [0.; 3]
        );
    }
}

#[test]
fn spatial_commit_rechecks_incarnation_after_prepare_and_rebirth_in_another_slot() {
    let mut world = world();
    let stale = world.entity_generation(&"cube".into()).unwrap();
    let mut buffer = ProjectRuntimeMutationBuffer::new();
    append(&mut buffer, &world, "camera", moved());
    append(&mut buffer, &world, "cube", moved());
    let prepared = buffer.prepare(&world).unwrap();
    world.try_despawn_entity(&"cube".into()).unwrap();
    spawn(&mut world, "occupy-old-slot", None, false);
    spawn(&mut world, "cube", None, false);
    assert_ne!(world.entity_generation(&"cube".into()), Some(stale));
    let dirty = world.dirty_records().len();
    assert_eq!(
        prepared.commit(&mut world).unwrap_err().code,
        "three_d.stale_entity"
    );
    assert_eq!(world.dirty_records().len(), dirty);
    assert_eq!(
        world.transform3d(&"camera".into()).unwrap().translation,
        [0.; 3]
    );
    for generation in [0, stale] {
        let mut buffer = ProjectRuntimeMutationBuffer::new();
        append_deferred_mutation(
            &mut buffer,
            ProjectRuntimeDeferredMutation::Spatial {
                entity_id: "cube".into(),
                generation,
                intent: moved(),
            },
        )
        .unwrap();
        assert_eq!(
            buffer.prepare(&world).unwrap_err().code,
            "three_d.stale_entity"
        );
    }
}

#[test]
fn spatial_generic_mutations_cannot_replace_reserved_components() {
    let world = world();
    for mutation in [
        ProjectRuntimeDeferredMutation::ReplaceDynamicComponent {
            entity_id: "cube".into(),
            component_type: "engine.transform3d".into(),
            fields: BTreeMap::new(),
        },
        ProjectRuntimeDeferredMutation::WriteComponentField {
            entity_id: "cube".into(),
            component_type: "engine.transform3d".into(),
            field_path: "translation".into(),
            value: ProjectRuntimeValue::Vec3([1.; 3]),
        },
    ] {
        let mut buffer = ProjectRuntimeMutationBuffer::new();
        append(&mut buffer, &world, "cube", moved());
        append_deferred_mutation(&mut buffer, mutation).unwrap();
        assert_eq!(
            buffer.prepare(&world).unwrap_err().report.committed_count,
            0
        );
    }
}

#[test]
fn spatial_real_host_query_and_read_require_current_nonzero_generation() {
    let world = world();
    let token = world.entity_generation(&"cube".into()).unwrap();
    let mut host = ReadOnlyWorldHost {
        ui: None,
        world: WorldReadApi::new(&world),
        spawn_receipts: None,
        animator_snapshots: None,
    };
    let response = host.query(ProjectRuntimeWorldQueryRequest {
        generated_content: None,
        project_storage: None,
        project_storage_take: None,
        physics: None,
        entity_id: None,
        cell: None,
        spawn_request_id: None,
        all: vec!["engine.transform3d".into()],
        none: vec![],
    });
    assert_eq!(response.entity_generations["cube"], token);
    for (generation, expected) in [(token, true), (0, false), (token + 1, false)] {
        let read = host.read(ProjectRuntimeWorldReadRequest {
            entity_id: "cube".into(),
            component_type: "engine.transform3d".into(),
            generation,
        });
        assert_eq!(read.is_some(), expected);
        if let Some(read) = read {
            let ProjectRuntimeValue::Object(fields) = read.value else {
                panic!("typed object")
            };
            assert_eq!(fields["translation"], ProjectRuntimeValue::Vec3([0.; 3]));
        }
    }
}

#[test]
fn spatial_combined_parent_child_overflow_is_rejected_before_any_write() {
    let mut world = world();
    spawn(&mut world, "child", Some("cube"), false);
    let mut buffer = ProjectRuntimeMutationBuffer::new();
    for name in ["cube", "child"] {
        append(
            &mut buffer,
            &world,
            name,
            Intent::SetLocalTransform {
                transform: ProjectRuntimeTransform3D {
                    scale: [1e30; 3],
                    ..ProjectRuntimeTransform3D::identity()
                },
            },
        );
    }
    let dirty = world.dirty_records().len();
    assert!(buffer.prepare(&world).is_err());
    assert_eq!(world.dirty_records().len(), dirty);
}

#[test]
fn spatial_camera_validation_consumes_final_enabled_overlay() {
    let world = world();
    let mut buffer = ProjectRuntimeMutationBuffer::new();
    append(&mut buffer, &world, "camera", Intent::SelectMainCamera {});
    let mut meta = world.entity(&"camera".into()).unwrap().clone();
    meta.enabled = false;
    buffer.replace_component(
        "camera".into(),
        ComponentTypeId::entity_meta(),
        ComponentValue::EntityMeta(meta),
    );
    assert!(buffer.prepare(&world).is_err());
}

#[test]
fn spatial_spawn_receipt_resolves_only_its_committed_live_incarnation() {
    let mut world = world();
    let generation = world.entity_generation(&"camera".into()).unwrap();
    let receipts = BTreeMap::from([(7, (EntityId::from("camera"), generation))]);
    let query = |world: &World, key| {
        ReadOnlyWorldHost {
            ui: None,
            world: WorldReadApi::new(world),
            spawn_receipts: Some(&receipts),
            animator_snapshots: None,
        }
        .query(ProjectRuntimeWorldQueryRequest {
            generated_content: None,
            project_storage: None,
            project_storage_take: None,
            physics: None,
            entity_id: None,
            cell: None,
            spawn_request_id: Some(key),
            all: vec![],
            none: vec![],
        })
    };
    assert_eq!(query(&world, 7).entity_generations["camera"], generation);
    assert!(query(&world, 8).entity_ids.is_empty());
    world.try_despawn_entity(&"camera".into()).unwrap();
    spawn(&mut world, "camera", None, true);
    assert!(
        query(&world, 7).entity_ids.is_empty(),
        "receipt must not follow a reused source name"
    );
}

#[test]
fn spatial_foundation_wire_consumes_history_anchor_and_parent_intents() {
    let mut w = world();
    spawn(&mut w, "parent", None, false);
    w.take_dirty_records();
    w.begin_spatial_tick(1);
    let mut b = ProjectRuntimeMutationBuffer::new();
    append(
        &mut b,
        &w,
        "cube",
        Intent::Teleport {
            transform: ProjectRuntimeTransform3D {
                translation: [4., 0., 0.],
                ..ProjectRuntimeTransform3D::identity()
            },
        },
    );
    append(
        &mut b,
        &w,
        "cube",
        Intent::SetInterpolation { enabled: false },
    );
    append(
        &mut b,
        &w,
        "cube",
        Intent::SetWorldAnchor {
            anchor: [16., 0., 0.],
            preserve_world: true,
        },
    );
    b.prepare(&w).unwrap().commit(&mut w).unwrap();
    assert_eq!(
        w.transform3d(&"cube".into()).unwrap().translation,
        [-12., 0., 0.]
    );
    let parent_generation = w.entity_generation(&"parent".into()).unwrap();
    let mut b = ProjectRuntimeMutationBuffer::new();
    append(
        &mut b,
        &w,
        "cube",
        Intent::Reparent {
            parent_entity_id: Some("parent".into()),
            parent_generation,
            preserve_world: true,
        },
    );
    let prepared = b.prepare(&w).unwrap();
    prepared.commit(&mut w).unwrap();
    w.prepare_spatial_presentation(1, 0.).unwrap();
    assert_eq!(
        w.spatial_presentation(&"cube".into()).unwrap().world_matrix[12],
        4.
    );
    assert_eq!(
        w.hierarchy(&"cube".into()).unwrap().parent_id,
        Some("parent".into())
    );
}
#[test]
fn spatial_foundation_prepared_parent_reuse_rejects_without_child_mutation() {
    let mut w = world();
    spawn(&mut w, "parent", None, false);
    w.take_dirty_records();
    let mut b = ProjectRuntimeMutationBuffer::new();
    append(&mut b, &w, "cube", moved());
    let generation = w.entity_generation(&"parent".into()).unwrap();
    append(
        &mut b,
        &w,
        "cube",
        Intent::Reparent {
            parent_entity_id: Some("parent".into()),
            parent_generation: generation,
            preserve_world: true,
        },
    );
    let prepared = b.prepare(&w).unwrap();
    w.try_despawn_entity(&"parent".into()).unwrap();
    spawn(&mut w, "parent", None, false);
    assert_ne!(w.entity_generation(&"parent".into()), Some(generation));
    assert_eq!(
        prepared.commit(&mut w).unwrap_err().report.committed_count,
        0
    );
    assert_eq!(w.transform3d(&"cube".into()).unwrap().translation, [0.; 3]);
    assert!(w.hierarchy(&"cube".into()).unwrap().parent_id.is_none());
    let delta = w.finish_spatial_tick();
    let incarnations = delta
        .changes
        .iter()
        .filter(|c| c.entity_id.as_str() == "parent")
        .collect::<Vec<_>>();
    assert_eq!(incarnations.len(), 2);
    assert_ne!(incarnations[0].runtime_id, incarnations[1].runtime_id);
    assert!(incarnations.iter().any(|c| c.pose.is_none()));
    assert!(incarnations.iter().any(|c| c.pose.is_some()));
}

#[test]
fn cell_real_native_query_reads_original_owner_and_rejects_mixed_query_forms() {
    use crate::runtime_asset::{RuntimeAssetIndex, RuntimePackageMountTable};
    use crate::runtime_asset_loader::RuntimeAssetLoader;
    use crate::runtime_instance_loader::RuntimeInstanceLoader;
    use project_runtime_sdk::{ProjectRuntimeCellIntent as I, ProjectRuntimeCellQuery};
    let world = World::new();
    let mut loader = RuntimeInstanceLoader::new(RuntimeAssetLoader::new(
        std::env::temp_dir().join("not-read-cell-query"),
        RuntimeAssetIndex::new(vec![], vec![]),
        RuntimePackageMountTable::new(vec![]),
    ));
    let intent = I::Load {
        request_id: 11,
        placement_id: "west".into(),
        prefab: project_runtime_sdk::ProjectRuntimeSpatialAssetRef {
            id: "west-prefab".into(),
            asset_type: "prefab".into(),
            guid: Some("guid-west".into()),
            sub_asset: None,
        },
        parent: None,
        render_required: true,
    };
    let wire = serde_json::to_vec(&ProjectRuntimeDeferredMutation::Cell {
        intent: intent.clone(),
    })
    .unwrap();
    let mut buffer = ProjectRuntimeMutationBuffer::new();
    append_deferred_mutation(&mut buffer, serde_json::from_slice(&wire).unwrap()).unwrap();
    assert!(
        buffer.prepare(&world).is_err(),
        "cell wire must require the original runtime owner, not generic World mutation"
    );
    loader.accept_project_cells(vec![intent], &world).unwrap();
    let mut host = ReadOnlyWorldHost {
        world: WorldReadApi::with_runtime_cells(&world, Some(&loader)),
        spawn_receipts: None,
        animator_snapshots: None,
        ui: None,
    };
    let query = ProjectRuntimeCellQuery {
        request_id: Some(11),
        placement_id: None,
        source_entity_id: None,
    };
    let response = host.query(ProjectRuntimeWorldQueryRequest {
        generated_content: None,
        project_storage: None,
        project_storage_take: None,
        physics: None,
        entity_id: None,
        cell: Some(query.clone()),
        spawn_request_id: None,
        all: vec![],
        none: vec![],
    });
    assert_eq!(
        response.cell.unwrap().status,
        project_runtime_sdk::ProjectRuntimeCellStatus::Accepted
    );
    assert!(response.entity_ids.is_empty());
    let mixed = host.query(ProjectRuntimeWorldQueryRequest {
        generated_content: None,
        project_storage: None,
        project_storage_take: None,
        physics: None,
        entity_id: None,
        cell: Some(query),
        spawn_request_id: Some(11),
        all: vec![],
        none: vec![],
    });
    assert!(mixed.cell.is_none());
    assert!(mixed.entity_ids.is_empty());
    assert_eq!(
        loader
            .asset_loader()
            .streaming_snapshot()
            .dispatched_operations,
        0,
        "queries never initiate IO"
    );
    assert!(
        serde_json::from_value::<I>(
            serde_json::json!({"operation":"release","request_id":11,"gpu_ready":true})
        )
        .is_err(),
        "projects cannot provide a GPU acknowledgement"
    );
}

#[test]
fn s2b_light_commands_compose_overlay_atomically_and_recheck_generation() {
    use crate::spatial_contract::{Light3D, LightKind};
    for kind in [
        LightKind::Directional {
            illuminance_lux: 1.,
        },
        LightKind::Point {
            intensity_candela: 1.,
            range_m: 10.,
        },
        LightKind::Spot {
            intensity_candela: 1.,
            range_m: 10.,
            inner_half_angle_degrees: 15.,
            outer_half_angle_degrees: 30.,
        },
    ] {
        let mut world = world();
        world
            .try_insert_component_value(
                "cube".into(),
                ComponentTypeId::light3d(),
                ComponentValue::Light3D(Light3D {
                    cast_shadows: false,
                    schema_version: "light3d.v1".into(),
                    enabled: true,
                    color: [1.; 3],
                    kind,
                    layers: vec!["world".into()],
                }),
            )
            .unwrap();
        let mut buffer = ProjectRuntimeMutationBuffer::new();
        append(
            &mut buffer,
            &world,
            "cube",
            Intent::SetLightColor {
                color: [0.2, 0.3, 0.4],
            },
        );
        append(
            &mut buffer,
            &world,
            "cube",
            Intent::SetLightIntensity { intensity: 12. },
        );
        append(
            &mut buffer,
            &world,
            "cube",
            Intent::SetLightEnabled { enabled: false },
        );
        buffer.prepare(&world).unwrap().commit(&mut world).unwrap();
        let before = world.light3d(&"cube".into()).unwrap().clone();
        assert_eq!(before.color, [0.2, 0.3, 0.4]);
        assert!(!before.enabled);
        match &before.kind {
            LightKind::Directional { illuminance_lux } => assert_eq!(*illuminance_lux, 12.),
            LightKind::Point {
                intensity_candela,
                range_m,
            }
            | LightKind::Spot {
                intensity_candela,
                range_m,
                ..
            } => {
                assert_eq!(*intensity_candela, 12.);
                assert_eq!(*range_m, 10.);
            }
        }
        for bad in [
            Intent::SetLightColor {
                color: [-1., 0., 0.],
            },
            Intent::SetLightIntensity { intensity: -1. },
            Intent::SetLightIntensity {
                intensity: f32::MAX,
            },
        ] {
            let mut buffer = ProjectRuntimeMutationBuffer::new();
            append(
                &mut buffer,
                &world,
                "cube",
                Intent::SetLightColor { color: [2.; 3] },
            );
            append(&mut buffer, &world, "cube", bad);
            let dirty = world.dirty_records().len();
            assert!(buffer.prepare(&world).is_err());
            assert_eq!(world.light3d(&"cube".into()), Some(&before));
            assert_eq!(world.dirty_records().len(), dirty);
        }
        let mut buffer = ProjectRuntimeMutationBuffer::new();
        append(
            &mut buffer,
            &world,
            "camera",
            Intent::SetLightEnabled { enabled: false },
        );
        assert!(buffer.prepare(&world).is_err());
        let mut buffer = ProjectRuntimeMutationBuffer::new();
        append(
            &mut buffer,
            &world,
            "cube",
            Intent::SetLightEnabled { enabled: true },
        );
        let pending = buffer.prepare(&world).unwrap();
        world.try_despawn_entity(&"cube".into()).unwrap();
        spawn(&mut world, "cube", None, false);
        assert_eq!(
            pending.commit(&mut world).unwrap_err().code,
            "three_d.stale_entity"
        );
    }
}
