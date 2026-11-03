use engine_runtime::archetype::ComponentValue;
use engine_runtime::components::{ComponentRegistry, ComponentTypeId, Hierarchy, Transform};
use engine_runtime::ids::EntityId;
use engine_runtime::render_asset_production::RuntimeRenderAssetProducer;
use engine_runtime::render_command::apply_batch;
use engine_runtime::render_contract::*;
use engine_runtime::render_extract::RenderExtractContext;
use engine_runtime::render_geometry::*;
use engine_runtime::render_projection_adapter::MeshProjection;
use engine_runtime::render_resource::RenderResourceManager;
use engine_runtime::render_state::{RenderProxyPayload, RenderSceneState};
use engine_runtime::runtime_instance_loader::RuntimeInstanceLoader;
use engine_runtime::runtime_package::*;
use engine_runtime::runtime_package_builder::*;
use engine_runtime::runtime_scene_hydration::RuntimeSceneHydrator;
use engine_runtime::spatial_contract::*;
use engine_runtime::world::World;
use serde_json::{json, Value};
use std::{fs, path::PathBuf};
#[cfg(feature = "real-wgpu")]
#[path = "three_d_runtime_a/generated_gpu.rs"]
mod generated_gpu;

fn production_draws(
    output: &engine_runtime::render_thread::RenderThreadFrameOutput,
) -> Vec<DrawGeometry> {
    output
        .renderer_output
        .rhi_command_plan
        .commands
        .iter()
        .flat_map(|command| command.geometry_instances())
        .map(|(head, instance, depth)| {
            // Test-only legacy inspection, including every selected batch row.
            // The actual GPU still consumes the original immutable batch plan.
            let mut draw = head.clone();
            *std::ops::DerefMut::deref_mut(&mut draw) = instance.as_ref().clone();
            draw.camera_depth = depth;
            draw
        })
        .collect()
}

#[test]
fn production_frame_identity_keeps_real_tick_pair_world_and_run_lineage() {
    use engine_runtime::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
    use engine_runtime::frame_loop::RuntimeFrameContext;
    use engine_runtime::render_thread::RenderThreadMode;
    for mode in [
        RenderThreadMode::InlineDeterministic,
        RenderThreadMode::DedicatedThread,
    ] {
        let fixture = Fixture::new(base(), None);
        let (mut world, mut hydrator) = fixture.hydrate();
        let mut host = EngineHostLoop::new_with_render_mode("main", mode);
        let mut retained = Vec::new();
        for ticks in [0, 3, 0, 2] {
            let output = host
                .tick_with_runtime_context(
                    EngineFrameInput::new(EngineHostMode::ExportedGame)
                        .with_fixed_step_count(ticks),
                    &mut world,
                    RuntimeFrameContext::new(&fixture.package, hydrator.instance_loader_mut()),
                )
                .render_thread_frame
                .unwrap();
            let plan = &output.renderer_output.rhi_command_plan;
            let id = plan.frame_identity.as_ref().expect("frozen source").clone();
            let delta = world.spatial_presentation_delta();
            assert_eq!(id.frame_index, plan.frame_index);
            assert_eq!(id.tick_pair, delta.tick_pair);
            assert_eq!(id.presentation_alpha_bits, delta.alpha.to_bits());
            assert_eq!(id.source.world_epoch, world.epoch());
            assert_eq!(
                id.source.package_content_hash,
                fixture.package.manifest.content_hash
            );
            assert!(!id.views.is_empty());
            for (identity, frozen) in id
                .views
                .iter()
                .zip(host.retained_geometry_snapshot().unwrap().views.iter())
            {
                assert_eq!(identity.view_id, frozen.camera.view.view_id);
                assert_eq!(identity.origin_bits, frozen.camera.origin.map(f64::to_bits));
                assert_eq!(identity.history_version, frozen.history_version);
            }
            retained.push(id);
        }
        assert_eq!(
            retained[1].tick_pair, retained[2].tick_pair,
            "zero new tick must retain the real pair"
        );
        assert_ne!(retained[0].tick_pair, retained[1].tick_pair);
        assert!(std::sync::Arc::ptr_eq(
            &retained[0].source,
            &retained[3].source
        ));
        let old = serde_json::to_value(&retained).unwrap();
        let old_snapshot = host.retained_geometry_snapshot().unwrap().clone();
        let (mut replacement, mut replacement_hydrator) = fixture.hydrate();
        assert_ne!(replacement.epoch(), world.epoch());
        assert_eq!(
            replacement.clone().epoch(),
            replacement.epoch(),
            "transactional clone retains lineage"
        );
        let changed = host
            .tick_with_runtime_context(
                EngineFrameInput::new(EngineHostMode::ExportedGame),
                &mut replacement,
                RuntimeFrameContext::new(
                    &fixture.package,
                    replacement_hydrator.instance_loader_mut(),
                ),
            )
            .render_thread_frame
            .unwrap();
        assert!(changed.report.resource_lifetime_report.events.iter().any(|e| e.event_type == engine_runtime::render_resource::RenderResourceEventType::PendingRelease), "World replacement must retire the previous cache while old snapshots retain their leases");
        assert!(
            changed
                .report
                .resource_lifetime_report
                .pending_release_count
                > 0
        );
        assert_eq!(
            host.retained_geometry_snapshot().unwrap().views[0].history_version,
            1,
            "replacement World starts fresh camera history"
        );
        assert!(old_snapshot.content_version > 0);
        let changed_id = changed
            .renderer_output
            .rhi_command_plan
            .frame_identity
            .unwrap();
        assert_ne!(
            changed_id.source.world_epoch,
            retained[0].source.world_epoch
        );
        assert_eq!(changed_id.source.run_epoch, retained[0].source.run_epoch);
        assert_eq!(
            serde_json::to_value(&retained).unwrap(),
            old,
            "old packets cannot acquire a new World identity"
        );
        let mut restarted = EngineHostLoop::new_with_render_mode("main", mode);
        let restarted_frame = restarted
            .tick_with_runtime_context(
                EngineFrameInput::new(EngineHostMode::ExportedGame),
                &mut replacement,
                RuntimeFrameContext::new(
                    &fixture.package,
                    replacement_hydrator.instance_loader_mut(),
                ),
            )
            .render_thread_frame
            .unwrap();
        assert_ne!(
            restarted_frame
                .renderer_output
                .rhi_command_plan
                .frame_identity
                .unwrap()
                .source
                .run_epoch,
            changed_id.source.run_epoch
        );
    }
}

#[test]
fn production_frame_inline_and_worker_share_prepared_buffers_and_retarget_views() {
    use engine_runtime::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
    use engine_runtime::frame_loop::RuntimeFrameContext;
    use engine_runtime::render_thread::RenderThreadMode;
    use engine_runtime::runtime_renderer::RenderTarget;
    for mode in [
        RenderThreadMode::InlineDeterministic,
        RenderThreadMode::DedicatedThread,
    ] {
        let mut source = base();
        source["entities"][1]["spatial"]["transform"]["translation"] = json!([0, 0, 5]);
        let fixture = Fixture::new(source, None);
        let (mut world, mut hydrator) = fixture.hydrate();
        let mut host = EngineHostLoop::new_with_render_mode("main", mode);
        let mut tick = |host: &mut EngineHostLoop| {
            host.tick_with_runtime_context(
                EngineFrameInput::new(EngineHostMode::ExportedGame),
                &mut world,
                RuntimeFrameContext::new(&fixture.package, hydrator.instance_loader_mut()),
            )
            .render_thread_frame
            .unwrap()
        };
        let first = tick(&mut host);
        assert_eq!(first.report.rhi_status, "ok", "{:?}", first.report);
        assert_eq!(production_draws(&first).len(), 1);
        assert!(first.report.resource_lifetime_report.created_count >= 2);
        let second = tick(&mut host);
        assert_eq!(second.report.resource_lifetime_report.created_count, 0);
        assert_eq!(second.report.resource_lifetime_report.uploaded_bytes, 0);
        assert!(std::sync::Arc::ptr_eq(
            &production_draws(&first)[0].geometry,
            &production_draws(&second)[0].geometry
        ));
        let resized =
            host.render_thread_for_target(RenderTarget::surface("actual-surface", 800, 400));
        let draw = &production_draws(&resized)[0];
        assert_eq!(draw.target.as_ref(), "actual-surface");
        assert_eq!(draw.view.viewport, [0, 0, 800, 400]);
        assert!(std::sync::Arc::ptr_eq(
            &draw.geometry,
            &production_draws(&first)[0].geometry
        ));
        assert_ne!(
            draw.view.projection_matrix,
            production_draws(&first)[0].view.projection_matrix
        );
        let suspended =
            host.render_thread_for_target(RenderTarget::surface("actual-surface", 0, 0));
        assert!(production_draws(&suspended).is_empty());
    }
}

#[test]
fn production_frame_missing_package_fails_closed_without_placeholder_draws() {
    use engine_runtime::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
    let fixture = Fixture::new(base(), None);
    let (mut world, _hydrator) = fixture.hydrate();
    let mut host = EngineHostLoop::new("main");
    let output = host
        .tick(
            EngineFrameInput::new(EngineHostMode::ExportedGame),
            &mut world,
        )
        .render_thread_frame
        .unwrap();
    assert_eq!(output.report.rhi_status, "error");
    assert_eq!(output.report.present_status, "not_submitted");
    assert!(output.renderer_output.rhi_command_plan.commands.is_empty());
    assert!(output.report.diagnostics.iter().any(
        |d| d.code == "three_d.production_frame_failed" && d.message.contains("runtimePackage")
    ));
}

fn id(s: &str) -> EntityId {
    EntityId::from(s)
}
fn near(a: f32, b: f32) {
    assert!((a - b).abs() < 0.00001, "{a} != {b}");
}
fn near3(a: [f32; 3], b: [f32; 3]) {
    for i in 0..3 {
        near(a[i], b[i]);
    }
}
fn local(t: [f32; 3], r: [f32; 3], s: [f32; 3]) -> CanonicalTransform3D {
    Transform3D {
        schema_version: "transform3d.v1".into(),
        translation: t,
        rotation: Rotation3D::EulerDegrees { xyz: r },
        scale: s,
    }
    .canonicalize()
    .unwrap()
}
fn node(name: &str, parent: Option<&str>, t: CanonicalTransform3D) -> Value {
    json!({"schemaVersion":"runtime-entity.v2","id":name,"name":name,"parentId":parent,"siblingOrder":0,"enabled":true,"spatial":{"domain":"world3d","transform":t},"components":[]})
}
fn base() -> Value {
    serde_json::from_str(include_str!("fixtures/three_d_a0/runtime-scene-v2.json")).unwrap()
}
fn cooked_mesh() -> CookedGeometry {
    let RenderAssetData::Mesh(mesh) =
        RenderContract::<RenderAssetData>::decode(include_str!("fixtures/render_mesh_v1.json"))
            .unwrap()
            .data
    else {
        panic!()
    };
    mesh
}
struct Fixture {
    root: PathBuf,
    package: RuntimePackage,
}
impl Fixture {
    fn new(scene: Value, prefab: Option<Value>) -> Self {
        let RuntimeSceneDocument::Spatial(scene) =
            decode_runtime_scene(&serde_json::to_vec(&scene).unwrap()).unwrap()
        else {
            panic!()
        };
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "aife-359-a11-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let mut input = RuntimePackageBuildInput::new(RuntimeProjectInfo::explicit_empty(
            "spatial", "Spatial", "1",
        ));
        input.scenes.push(scene.into());
        input.assets.push(RuntimePackageSourceAsset::new(
            "main",
            "Main",
            "scene",
            "Scenes/main.json",
            "scenes/main.json",
        ));
        let mapping = engine_runtime::input_mapping::InputMappingAsset::gameplay_default();
        input.input_mappings.push(RuntimePackageSourceJson {
            id: mapping.asset_id.clone(),
            document: serde_json::to_value(mapping).unwrap(),
        });
        for (id, kind, text) in [
            ("cube", "mesh", include_str!("fixtures/render_mesh_v1.json")),
            (
                "blue",
                "material",
                include_str!("fixtures/render_material_v1.json"),
            ),
        ] {
            let asset = RenderContract::<RenderAssetData>::decode(text)
                .unwrap()
                .data;
            let payload = serde_json::to_vec(&RenderContract::new(CookedRenderAsset {
                source_digest: engine_runtime::canonical_digest::sha256_prefixed(text.as_bytes()),
                recipe: NATIVE_RENDER_COOK_RECIPE.into(),
                asset,
            }))
            .unwrap();
            let mut source = RuntimePackageSourceAsset::new(
                id,
                id,
                kind,
                format!("Assets/{id}.json"),
                format!("cooked/{id}.json"),
            );
            source.hash = Some(engine_runtime::canonical_digest::sha256_prefixed(&payload));
            input.assets.push(source.with_runtime_payload(payload));
        }
        if let Some(document) = prefab {
            input.prefabs.push(RuntimePackageSourcePrefab {
                identity: None,
                prefab_id: document["id"].as_str().unwrap().into(),
                document,
            });
        }
        let report = RuntimePackageBuilder::build(
            &RuntimePackageBuildRequest::dev_desktop(&root, "main"),
            &input,
        );
        assert_eq!(
            report.status,
            RuntimePackageBuildStatus::Success,
            "{report:?}"
        );
        let loaded = load_runtime_package(&root);
        assert!(loaded.diagnostics.is_ok(), "{:?}", loaded.diagnostics);
        Self {
            root,
            package: loaded.value.unwrap(),
        }
    }
    fn hydrate(&self) -> (World, RuntimeSceneHydrator) {
        let mut world = World::new();
        let mut hydrator = RuntimeSceneHydrator::from_package(&self.package);
        let report = hydrator.hydrate_active_scene(&self.package, &mut world);
        assert!(!report.has_errors(), "{report:?}");
        (world, hydrator)
    }
    fn prepared(
        &self,
        loader: &mut RuntimeInstanceLoader,
        manager: &mut RenderResourceManager,
    ) -> PreparedGeometry {
        let producer = RuntimeRenderAssetProducer::new();
        let mut prepare = |id, kind| {
            producer
                .prepare_cooked_from_package(
                    1,
                    &RenderAssetReference {
                        expected_kind: kind,
                        reference: RuntimeAssetRef {
                            id: String::from(id),
                            asset_type: match kind {
                                RenderAssetKind::Mesh => "mesh",
                                _ => "material",
                            }
                            .into(),
                            guid: None,
                            sub_asset: None,
                        },
                    },
                    loader.asset_loader_mut(),
                    manager,
                )
                .unwrap()
        };
        let mesh = prepare("cube", RenderAssetKind::Mesh);
        let material = prepare("blue", RenderAssetKind::Material);
        PreparedGeometry::from_assets(&mesh, &material, 0, manager).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
fn extract(
    world: &mut World,
    context: &mut RenderExtractContext,
    scene: &mut RenderSceneState,
    frame: u64,
) {
    let batch = context
        .extract_world_dirty(frame, world, scene)
        .normalize_merge(scene);
    assert!(apply_batch(scene, &batch).is_empty());
}
fn projection(scene: &RenderSceneState, name: &str) -> MeshProjection {
    let proxy = scene
        .proxy(scene.proxy_for_source(&id(name)).unwrap())
        .unwrap();
    let RenderProxyPayload::Mesh3D(p) = &proxy.payload else {
        panic!("not typed mesh")
    };
    (**p).clone()
}
fn set(world: &mut World, name: &str, value: ComponentValue) {
    world
        .try_insert_component_value(id(name), value.component_type(), value)
        .unwrap();
}

#[test]
fn hierarchy_real_package_typed_storage_rotation_order_mirror_and_previous() {
    let mut source = base();
    source["entities"][0]["parentId"] = json!("parent");
    source["entities"][0]["spatial"]["transform"] =
        json!(local([1., 0., 0.], [0., 90., 0.], [-1., 3., 1.]));
    source["entities"].as_array_mut().unwrap().push(node(
        "parent",
        None,
        local([10., 0., 0.], [0., 0., 90.], [2., 1., 1.]),
    ));
    let fixture = Fixture::new(source, None);
    let (mut world, _) = fixture.hydrate();
    for t in [
        ComponentTypeId::transform3d(),
        ComponentTypeId::mesh_renderer3d(),
        ComponentTypeId::visibility3d(),
        ComponentTypeId::camera3d(),
    ] {
        assert!(ComponentRegistry::new().contains(&t));
    }
    assert!(world.transform(&id("cube")).is_none());
    let mut context = RenderExtractContext::new();
    let mut scene = RenderSceneState::new();
    extract(&mut world, &mut context, &mut scene, 1);
    let first = world.spatial_world(&id("cube")).unwrap().clone();
    near3(first.pose.position, [10., 2., 0.]);
    near3(first.world_matrix[0..3].try_into().unwrap(), [0., 0., 1.]);
    near3(first.world_matrix[4..7].try_into().unwrap(), [-3., 0., 0.]);
    near3(first.world_matrix[8..11].try_into().unwrap(), [0., 2., 0.]);
    assert_eq!(first.world_matrix, first.previous_world_matrix);
    set(
        &mut world,
        "parent",
        ComponentValue::Transform3D(local([20., 0., 0.], [0., 0., 90.], [2., 1., 1.])),
    );
    extract(&mut world, &mut context, &mut scene, 2);
    let second = world.spatial_world(&id("cube")).unwrap();
    near3(second.pose.position, [20., 2., 0.]);
    assert_eq!(second.previous_world_matrix, first.world_matrix);
    assert_eq!(
        projection(&scene, "cube").world.world_matrix,
        second.world_matrix
    );
    let version = second.version;
    extract(&mut world, &mut context, &mut scene, 3);
    assert_eq!(
        world
            .spatial_world(&id("cube"))
            .unwrap()
            .previous_world_matrix,
        world.spatial_world(&id("cube")).unwrap().world_matrix
    );
    assert_eq!(world.spatial_world(&id("cube")).unwrap().version, version);
    assert_eq!(
        projection(&scene, "cube").world.previous_world_matrix,
        projection(&scene, "cube").world.world_matrix
    );
    assert!(world.take_dirty_records().is_empty());
}

#[test]
fn hierarchy_affine_shear_is_preserved_and_bounds_come_from_real_mesh() {
    let mut source = base();
    source["entities"][0]["parentId"] = json!("parent");
    source["entities"][0]["spatial"]["transform"] = json!(local([0.; 3], [0., 0., 45.], [1.; 3]));
    source["entities"].as_array_mut().unwrap().push(node(
        "parent",
        None,
        local([3., -2., -5.], [0.; 3], [2., 1., 1.]),
    ));
    let fixture = Fixture::new(source, None);
    let (mut world, _) = fixture.hydrate();
    let mut context = RenderExtractContext::new();
    let mut scene = RenderSceneState::new();
    extract(&mut world, &mut context, &mut scene, 1);
    let p = projection(&scene, "cube");
    let m = p.world.world_matrix;
    near(m[0] * m[4] + m[1] * m[5] + m[2] * m[6], -1.5);
    let (min, max) = p.world_bounds(&cooked_mesh()).unwrap();
    near3(min, [3. - 2f32.sqrt(), -2., -5.]);
    near3(max, [3. + 2f32.sqrt(), -2. + 2f32.sqrt(), -5.]);
    let mut bad = cooked_mesh();
    bad.bounds.max[0] = 0.5;
    assert!(p.world_bounds(&bad).is_err());
}

#[test]
fn hierarchy_reparent_preserves_local_and_rejects_cycles_orphans_cross_domain() {
    let mut source = base();
    source["entities"].as_array_mut().unwrap().push(node(
        "parent",
        None,
        local([2., 3., 4.], [0.; 3], [1.; 3]),
    ));
    let fixture = Fixture::new(source, None);
    let (mut world, _) = fixture.hydrate();
    let local = world.transform3d(&id("cube")).unwrap().clone();
    world
        .try_set_parent(id("cube"), Some(id("parent")))
        .unwrap();
    world.take_dirty_records();
    assert_eq!(*world.transform3d(&id("cube")).unwrap(), local);
    near3(
        world.spatial_world(&id("cube")).unwrap().pose.position,
        [2., 3., 4.],
    );
    for (child, parent) in [("parent", "cube"), ("cube", "cube"), ("cube", "missing")] {
        assert!(world.try_set_parent(id(child), Some(id(parent))).is_err());
    }
    world
        .try_spawn_with_components(
            id("legacy"),
            "Legacy",
            "entity",
            true,
            Hierarchy {
                parent_id: None,
                sibling_order: 0,
            },
            Some(Transform::identity()),
            None,
        )
        .unwrap();
    assert!(world
        .try_set_parent(id("cube"), Some(id("legacy")))
        .is_err());
    assert!(world
        .try_set_parent(id("legacy"), Some(id("cube")))
        .is_err());
    assert_eq!(
        world.hierarchy(&id("cube")).unwrap().parent_id,
        Some(id("parent"))
    );
    world.try_set_parent(id("cube"), None).unwrap();
    world.take_dirty_records();
    near3(
        world.spatial_world(&id("cube")).unwrap().pose.position,
        [0.; 3],
    );
}

#[test]
fn hierarchy_visibility_and_enabled_propagate_and_clean_frames_do_no_work() {
    let mut source = base();
    source["entities"][0]["parentId"] = json!("parent");
    source["entities"][0]["spatial"]["visibility"] =
        json!({"schemaVersion":"visibility3d.v1","visible":false});
    source["entities"].as_array_mut().unwrap().push(node(
        "parent",
        None,
        local([0.; 3], [0.; 3], [1.; 3]),
    ));
    let fixture = Fixture::new(source, None);
    let (mut world, _) = fixture.hydrate();
    let mut context = RenderExtractContext::new();
    let mut scene = RenderSceneState::new();
    extract(&mut world, &mut context, &mut scene, 1);
    assert!(!projection(&scene, "cube").visibility().authored);
    set(
        &mut world,
        "cube",
        ComponentValue::Visibility3D(Visibility3D {
            schema_version: "visibility3d.v1".into(),
            visible: true,
        }),
    );
    set(
        &mut world,
        "parent",
        ComponentValue::Visibility3D(Visibility3D {
            schema_version: "visibility3d.v1".into(),
            visible: false,
        }),
    );
    extract(&mut world, &mut context, &mut scene, 2);
    let p = projection(&scene, "cube");
    assert!(p.visibility().authored);
    assert_eq!(p.visibility().inherited, Evaluation::Known(false));
    set(
        &mut world,
        "parent",
        ComponentValue::Visibility3D(Visibility3D {
            schema_version: "visibility3d.v1".into(),
            visible: true,
        }),
    );
    let mut meta = world.entity(&id("parent")).unwrap().clone();
    meta.enabled = false;
    set(&mut world, "parent", ComponentValue::EntityMeta(meta));
    extract(&mut world, &mut context, &mut scene, 3);
    assert!(!projection(&scene, "cube").world.enabled);
    assert!(projection(&scene, "cube").world.inherited_visible);
    let mut mesh = world.mesh_renderer3d(&id("cube")).unwrap().clone();
    mesh.visible = false;
    set(&mut world, "cube", ComponentValue::MeshRenderer3D(mesh));
    extract(&mut world, &mut context, &mut scene, 4);
    assert!(!projection(&scene, "cube").world.authored_visible);
    assert!(context
        .extract_world_dirty(5, &mut world, &scene)
        .pending_commands
        .is_empty());
}

#[test]
fn projection_project_data_does_not_dirty_spatial_descendants() {
    let mut source = base();
    source["entities"][0]["parentId"] = json!("parent");
    source["entities"].as_array_mut().unwrap().push(node(
        "parent",
        None,
        local([0.; 3], [0.; 3], [1.; 3]),
    ));
    let fixture = Fixture::new(source, None);
    let (mut world, _) = fixture.hydrate();
    let mut context = RenderExtractContext::new();
    let mut scene = RenderSceneState::new();
    extract(&mut world, &mut context, &mut scene, 1);
    let before = world.spatial_world(&id("cube")).unwrap().clone();
    for name in ["parent", "cube"] {
        world
            .try_insert_dynamic_component(
                id(name),
                ComponentTypeId::new("project.counter"),
                engine_runtime::component_value::RuntimeValue::F64(1.),
            )
            .unwrap();
    }
    let dirty = world.take_dirty_records();
    assert_eq!(
        dirty.len(),
        2,
        "Project data must not dirty spatial descendants."
    );
    assert!(dirty
        .iter()
        .all(|r| r.dirty_type == engine_runtime::world::DirtyType::DynamicData));
    assert!(context
        .extract_dirty_records(2, &world, &scene, dirty)
        .pending_commands
        .is_empty());
    assert_eq!(world.spatial_world(&id("cube")), Some(&before));
}

#[test]
fn hierarchy_invalid_mutations_and_failed_hydration_are_atomic() {
    let fixture = Fixture::new(base(), None);
    let (mut world, mut hydrator) = fixture.hydrate();
    world.take_dirty_records();
    let original = world.transform3d(&id("cube")).unwrap().clone();
    let mut bad = original.clone();
    bad.scale[0] = 0.;
    assert!(world
        .try_insert_component_value(
            id("cube"),
            ComponentTypeId::transform3d(),
            ComponentValue::Transform3D(bad)
        )
        .is_err());
    assert!(world
        .try_insert_dynamic_component(
            id("cube"),
            ComponentTypeId::transform3d(),
            engine_runtime::component_value::RuntimeValue::Null
        )
        .is_err());
    assert!(world
        .try_insert_transform(id("cube"), Transform::identity())
        .is_err());
    assert!(world
        .try_remove_component_value(&id("cube"), &ComponentTypeId::transform3d())
        .is_err());
    assert_eq!(world.transform3d(&id("cube")), Some(&original));
    assert!(world.dirty_records().is_empty());
    let count = world.entity_count();
    let report = hydrator.hydrate_active_scene(&fixture.package, &mut world);
    assert!(report.has_errors());
    assert_eq!(world.entity_count(), count);
    assert!(world.dirty_records().is_empty());
    // Bypass file validation to exercise the actual hydration commit boundary.
    let mut broken = fixture.package.clone();
    broken.active_scene.entities[0].parent_id = Some("missing".into());
    let mut empty = World::new();
    let report = hydrator.hydrate_active_scene(&broken, &mut empty);
    assert!(report.has_errors());
    assert_eq!(empty.entity_count(), 0);
}

#[test]
fn hierarchy_world_overflow_and_camera_ancestor_scale_are_rejected_before_commit() {
    let mut source = base();
    source["entities"][1]["parentId"] = json!("rig");
    source["entities"].as_array_mut().unwrap().push(node(
        "rig",
        None,
        local([0.; 3], [0.; 3], [1.; 3]),
    ));
    let fixture = Fixture::new(source, None);
    let (mut world, _) = fixture.hydrate();
    world.take_dirty_records();
    let bad = local([0.; 3], [0.; 3], [2., 1., 1.]);
    assert!(world
        .try_insert_component_value(
            id("rig"),
            ComponentTypeId::transform3d(),
            ComponentValue::Transform3D(bad)
        )
        .unwrap_err()
        .code
        .contains("camera_scale"));
    set(
        &mut world,
        "cube",
        ComponentValue::Transform3D(local([0.; 3], [0.; 3], [2., 1., 1.])),
    );
    assert!(world.try_set_parent(id("rig"), Some(id("cube"))).is_err());
    let huge = local([f32::MAX, 0., 0.], [0.; 3], [1.; 3]);
    set(
        &mut world,
        "cube",
        ComponentValue::Transform3D(huge.clone()),
    );
    set(&mut world, "rig", ComponentValue::Transform3D(huge));
    assert!(world.try_set_parent(id("cube"), Some(id("rig"))).is_err());
    assert!(world.hierarchy(&id("cube")).unwrap().parent_id.is_none());
}

#[test]
fn hierarchy_prefab_instances_remap_and_destroy_without_stealing_main_camera() {
    let mut camera = base()["entities"][1].clone();
    camera["id"] = json!("prefabCamera");
    let mut mesh = base()["entities"][0].clone();
    mesh["parentId"] = json!("prefabCamera");
    let prefab = json!({"schemaVersion":"runtime-prefab.v2","id":"p","name":"P","rootEntityId":"prefabCamera","entities":[mesh,camera]});
    let fixture = Fixture::new(base(), Some(prefab));
    let (mut world, mut hydrator) = fixture.hydrate();
    let mut scene = RenderSceneState::new();
    let mut context = RenderExtractContext::new();
    extract(&mut world, &mut context, &mut scene, 1);
    let (instance, report) = hydrator
        .instance_loader_mut()
        .instantiate_prefab_from_package(
            &fixture.package,
            RuntimeAssetRef {
                id: "p".into(),
                asset_type: "prefab".into(),
                guid: None,
                sub_asset: None,
            },
            None,
            None,
            &mut world,
        );
    assert!(!report.has_errors(), "{report:?}");
    let instance = instance.unwrap();
    assert_eq!(world.main_camera(), Some(&id("camera")));
    assert_eq!(world.entity_count(), 4);
    extract(&mut world, &mut context, &mut scene, 2);
    assert_eq!(scene.proxies_len(), 2);
    let handle = *instance.source_to_runtime_entity.get(&id("cube")).unwrap();
    hydrator
        .instance_loader_mut()
        .despawn_prefab_instance(instance.instance_id, &mut world);
    extract(&mut world, &mut context, &mut scene, 3);
    assert_eq!(scene.proxies_len(), 1);
    assert_eq!(world.entity_count(), 2);
    assert!(world.try_resolve_runtime_entity(handle).is_err());
    assert_eq!(world.main_camera(), Some(&id("camera")));
}

#[test]
fn hierarchy_two_prefab_instances_have_distinct_spatial_history_and_live_identity() {
    let mut camera = base()["entities"][1].clone();
    camera["id"] = json!("prefabCamera");
    let mut mesh = base()["entities"][0].clone();
    mesh["parentId"] = json!("prefabCamera");
    let prefab = json!({"schemaVersion":"runtime-prefab.v2","id":"p","name":"P","rootEntityId":"prefabCamera","entities":[mesh,camera]});
    let fixture = Fixture::new(base(), Some(prefab));
    let (mut world, mut hydrator) = fixture.hydrate();
    let mut scene = RenderSceneState::new();
    let mut context = RenderExtractContext::new();
    extract(&mut world, &mut context, &mut scene, 1);
    let (instance, report) = hydrator
        .instance_loader_mut()
        .instantiate_prefab_from_package(
            &fixture.package,
            RuntimeAssetRef {
                id: "p".into(),
                asset_type: "prefab".into(),
                guid: None,
                sub_asset: None,
            },
            None,
            None,
            &mut world,
        );
    assert!(!report.has_errors(), "{report:?}");
    let instance = instance.unwrap();
    assert_eq!(world.main_camera(), Some(&id("camera")));
    assert_eq!(world.entity_count(), 4);
    extract(&mut world, &mut context, &mut scene, 2);
    assert_eq!(scene.proxies_len(), 2);
    let first = *instance.source_to_runtime_entity.get(&id("cube")).unwrap();
    let (second, report) = hydrator
        .instance_loader_mut()
        .instantiate_prefab_from_package(
            &fixture.package,
            RuntimeAssetRef {
                id: "p".into(),
                asset_type: "prefab".into(),
                guid: None,
                sub_asset: None,
            },
            None,
            None,
            &mut world,
        );
    assert!(!report.has_errors());
    let second = second.unwrap();
    let second_handle = *second.source_to_runtime_entity.get(&id("cube")).unwrap();
    assert_ne!(first, second_handle);
    let first_id = world
        .try_resolve_runtime_entity(first)
        .unwrap()
        .source_id
        .clone();
    let second_id = world
        .try_resolve_runtime_entity(second_handle)
        .unwrap()
        .source_id
        .clone();
    world.take_dirty_records();
    world.begin_spatial_tick(1);
    let old_second = world.committed_spatial_pose(&second_id).unwrap();
    let mut batch = engine_runtime::project_runtime_session::ProjectRuntimeMutationBuffer::new();
    batch.spatial_command(
        first_id.clone(),
        world.entity_generation(&first_id).unwrap(),
        project_runtime_sdk::ProjectRuntimeSpatialIntent::Teleport {
            transform: project_runtime_sdk::ProjectRuntimeTransform3D {
                translation: [3., 0., 0.],
                ..project_runtime_sdk::ProjectRuntimeTransform3D::identity()
            },
        },
    );
    batch.prepare(&world).unwrap().commit(&mut world).unwrap();
    world.finish_spatial_tick();
    assert_eq!(world.transform3d(&first_id).unwrap().translation[0], 3.);
    assert!(std::sync::Arc::ptr_eq(
        &old_second,
        &world.committed_spatial_pose(&second_id).unwrap()
    ));
    hydrator
        .instance_loader_mut()
        .despawn_prefab_instance(instance.instance_id, &mut world);
    assert!(world.try_resolve_runtime_entity(first).is_err());
    assert!(world.try_resolve_runtime_entity(second_handle).is_ok());
    assert_eq!(world.main_camera(), Some(&id("camera")));
}

fn camera(
    world: &mut World,
    context: &mut RenderExtractContext,
    scene: &mut RenderSceneState,
    extent: [u32; 2],
) -> engine_runtime::render_projection_adapter::CameraProjection {
    extract(world, context, scene, 1);
    let view = context
        .project_main_camera(world, scene, extent)
        .unwrap()
        .unwrap();
    scene.view(view).unwrap().spatial.clone().unwrap()
}
fn ndc(view: &ExecutableRenderView, p: [f32; 3]) -> [f32; 3] {
    let m = view.view_projection();
    let v: [f32; 4] =
        std::array::from_fn(|i| m[i] * p[0] + m[4 + i] * p[1] + m[8 + i] * p[2] + m[12 + i]);
    [v[0] / v[3], v[1] / v[3], v[2] / v[3]]
}

#[test]
fn camera_world_pose_movement_and_perspective_are_consumed() {
    let mut source = base();
    source["entities"][1]["spatial"]["camera"]["projection"] =
        json!({"kind":"perspective","fovYDegrees":90,"near":1,"far":11});
    let fixture = Fixture::new(source, None);
    let (mut world, _) = fixture.hydrate();
    let mut context = RenderExtractContext::new();
    let mut scene = RenderSceneState::new();
    let first = camera(&mut world, &mut context, &mut scene, [100, 100]);
    let view = first.executable().unwrap().unwrap();
    near3(ndc(&view, [1., 2., 3.]), [0.5, 0., 0.55]);
    set(
        &mut world,
        "camera",
        ComponentValue::Transform3D(local([1., 2., 5.], [0.; 3], [1.; 3])),
    );
    let moved = camera(&mut world, &mut context, &mut scene, [100, 100]);
    near3(
        ndc(&moved.executable().unwrap().unwrap(), [1., 2., 3.]),
        [0., 0., 0.55],
    );
    assert_eq!(first.view.view_id, moved.view.view_id);
    assert!(moved.view.version > first.view.version);
    assert_eq!(
        moved.view.provenance.source_component.as_deref(),
        Some("Camera3D")
    );
}

#[test]
fn camera_parent_rotation_orthographic_resize_and_suspend_restore() {
    let mut source = base();
    source["entities"][1]["parentId"] = json!("rig");
    source["entities"][1]["spatial"]["transform"] = json!(local([0., 0., 2.], [0.; 3], [1.; 3]));
    source["entities"][1]["spatial"]["camera"]["projection"] =
        json!({"kind":"orthographic","size":4,"near":1,"far":11});
    source["entities"].as_array_mut().unwrap().push(node(
        "rig",
        None,
        local([10., 0., 0.], [0., 90., 0.], [1.; 3]),
    ));
    let fixture = Fixture::new(source, None);
    let (mut world, _) = fixture.hydrate();
    let mut context = RenderExtractContext::new();
    let mut scene = RenderSceneState::new();
    let first = camera(&mut world, &mut context, &mut scene, [100, 100]);
    near3(first.pose.position, [12., 0., 0.]);
    near3(
        ndc(&first.executable().unwrap().unwrap(), [10., 1., -1.]),
        [0.5, 0.5, 0.1],
    );
    near3(
        ndc(&first.executable().unwrap().unwrap(), [8., 1., -1.]),
        [0.5, 0.5, 0.3],
    );
    let resized = camera(&mut world, &mut context, &mut scene, [200, 100]);
    near(
        ndc(&resized.executable().unwrap().unwrap(), [10., 1., -1.])[0],
        0.25,
    );
    assert_eq!(
        resized.executable().unwrap().unwrap().target_extent,
        [200, 100]
    );
    assert!(resized.view.version > first.view.version);
    let unchanged = camera(&mut world, &mut context, &mut scene, [200, 100]);
    assert_eq!(resized, unchanged);
    let suspended = camera(&mut world, &mut context, &mut scene, [0, 0]);
    assert!(suspended.suspended);
    assert!(suspended.executable().unwrap().is_none());
    assert!(suspended.view.version > resized.view.version);
    let restored = camera(&mut world, &mut context, &mut scene, [200, 100]);
    assert!(!restored.suspended);
    assert!(restored.view.version > suspended.view.version);
    assert_eq!(first.view.view_id, restored.view.view_id);
}

#[test]
fn camera_selection_and_destruction_clear_the_existing_view_registry() {
    let fixture = Fixture::new(base(), None);
    let (mut world, _) = fixture.hydrate();
    let mut context = RenderExtractContext::new();
    let mut scene = RenderSceneState::new();
    let current = camera(&mut world, &mut context, &mut scene, [100, 100]);
    assert!(world.select_main_camera(Some(id("cube"))).is_err());
    assert!(world.select_main_camera(Some(id("missing"))).is_err());
    world.try_despawn_entity(&id("camera")).unwrap();
    extract(&mut world, &mut context, &mut scene, 2);
    assert!(context
        .project_main_camera(&world, &mut scene, [100, 100])
        .is_err());
    assert!(scene
        .view(engine_runtime::render_state::RenderViewId(
            current.view.view_id
        ))
        .is_none());
}

#[test]
fn projection_existing_proxy_registry_identity_dirty_cleanup_and_no_legacy_draw() {
    let fixture = Fixture::new(base(), None);
    let (mut world, _) = fixture.hydrate();
    let mut context = RenderExtractContext::new();
    let mut scene = RenderSceneState::new();
    extract(&mut world, &mut context, &mut scene, 1);
    let proxy = scene.proxy_for_source(&id("cube")).unwrap();
    let first = projection(&scene, "cube");
    assert_eq!(first.provenance.source_entity_id.as_deref(), Some("cube"));
    assert_eq!(
        first.provenance.source_component.as_deref(),
        Some("MeshRenderer3D")
    );
    assert!(
        engine_runtime::renderer_feature_builder::RendererFeatureBuilder::new()
            .build(1, &scene)
            .draw_items
            .is_empty()
    );
    set(
        &mut world,
        "cube",
        ComponentValue::Transform3D(local([1., 0., -1.], [0.; 3], [1.; 3])),
    );
    extract(&mut world, &mut context, &mut scene, 2);
    assert_eq!(scene.proxy_for_source(&id("cube")), Some(proxy));
    assert_eq!(
        first.submesh_instance_id("surface"),
        projection(&scene, "cube").submesh_instance_id("surface")
    );
    assert_ne!(
        first.submesh_instance_id("surface"),
        first.submesh_instance_id("other")
    );
    world.try_despawn_entity(&id("cube")).unwrap();
    extract(&mut world, &mut context, &mut scene, 3);
    assert_eq!(scene.proxies_len(), 0);
    assert!(world.spatial_world(&id("cube")).is_none());
}

#[test]
fn projection_world_to_real_package_prepare_geometry_frame_layers_and_visibility() {
    let fixture = Fixture::new(base(), None);
    let (mut world, mut hydrator) = fixture.hydrate();
    let mut context = RenderExtractContext::new();
    let mut scene = RenderSceneState::new();
    let camera = camera(&mut world, &mut context, &mut scene, [100, 100]);
    let mut manager = RenderResourceManager::new();
    let prepared = fixture.prepared(hydrator.instance_loader_mut(), &mut manager);
    let build = |projection: &MeshProjection| {
        GeometryFrame::build(
            "main",
            [100, 100],
            &[GeometryViewInput {
                view: &camera.view,
                pose: camera.pose,
            }],
            &[projection.geometry_instance(&prepared).unwrap()],
            &manager,
            DiagnosticLevel::Summary,
        )
        .unwrap()
    };
    let first = build(&projection(&scene, "cube"));
    assert_eq!(first.views[0].draws.len(), 1);
    assert_eq!(
        first.views[0].view.source_camera_entity_id.as_deref(),
        Some("camera")
    );
    assert!(first.views[0].depth_target.is_some());
    let mut mesh = world.mesh_renderer3d(&id("cube")).unwrap().clone();
    mesh.layers = vec!["other".into()];
    set(
        &mut world,
        "cube",
        ComponentValue::MeshRenderer3D(mesh.clone()),
    );
    extract(&mut world, &mut context, &mut scene, 2);
    let excluded = build(&projection(&scene, "cube"));
    assert_eq!(
        excluded.views[0].summary.as_ref().unwrap().layer_excluded,
        1
    );
    assert!(excluded.views[0].draws.is_empty());
    mesh.layers = vec!["world".into()];
    mesh.visible = false;
    set(&mut world, "cube", ComponentValue::MeshRenderer3D(mesh));
    extract(&mut world, &mut context, &mut scene, 3);
    assert_eq!(
        build(&projection(&scene, "cube")).views[0]
            .summary
            .as_ref()
            .unwrap()
            .hidden,
        1
    );
}

#[test]
fn camera_invalid_selection_fails_without_stale_view_or_legacy_view_collision() {
    use engine_runtime::render_state::{
        RenderTargetKind, RenderViewId, RenderViewKind, RenderViewState,
    };
    let fixture = Fixture::new(base(), None);
    let (mut world, _) = fixture.hydrate();
    let mut context = RenderExtractContext::new();
    let mut scene = RenderSceneState::new();
    let legacy = RenderViewState::new(
        RenderViewId(1),
        RenderViewKind::Game,
        RenderTargetKind::Window,
    );
    scene.register_view(legacy.clone());
    let first = camera(&mut world, &mut context, &mut scene, [100, 100]);
    assert_ne!(first.view.view_id, 1);
    assert_eq!(scene.view(RenderViewId(1)), Some(&legacy));
    assert_eq!(
        RenderView::from_legacy(scene.view(RenderViewId(first.view.view_id)).unwrap()),
        first.view
    );
    let mut component = world.camera3d(&id("camera")).unwrap().clone();
    component.enabled = false;
    set(
        &mut world,
        "camera",
        ComponentValue::Camera3D(component.clone()),
    );
    extract(&mut world, &mut context, &mut scene, 2);
    assert!(context
        .project_main_camera(&world, &mut scene, [100, 100])
        .is_err());
    assert!(scene.view(RenderViewId(first.view.view_id)).is_none());
    component.enabled = true;
    set(&mut world, "camera", ComponentValue::Camera3D(component));
    let restored = camera(&mut world, &mut context, &mut scene, [100, 100]);
    assert!(restored.view.version > first.view.version);
    world.select_main_camera(None).unwrap();
    assert!(context
        .project_main_camera(&world, &mut scene, [100, 100])
        .is_err());
    assert!(scene.view(RenderViewId(restored.view.view_id)).is_none());
    assert_eq!(scene.view(RenderViewId(1)), Some(&legacy));
}

#[test]
fn hierarchy_reserved_deferred_and_legacy_spawn_cannot_leave_partial_spatial_entities() {
    use engine_runtime::component_value::RuntimeValue;
    use engine_runtime::gameplay_command::*;
    let mut camera = base()["entities"][1].clone();
    camera["id"] = json!("prefabCamera");
    let fixture = Fixture::new(
        base(),
        Some(
            json!({"schemaVersion":"runtime-prefab.v2","id":"p","name":"P","rootEntityId":"prefabCamera","entities":[camera]}),
        ),
    );
    let (mut world, mut hydrator) = fixture.hydrate();
    world.take_dirty_records();
    let count = world.entity_count();
    let results = apply_gameplay_commands(
        &mut world,
        vec![
            (
                GameplayCommandId(1),
                GameplayCommand::SpawnEntity {
                    entity_id: id("bad"),
                    name: "Bad".into(),
                    kind: "entity".into(),
                    enabled: true,
                    parent_id: None,
                    components: vec![ComponentValue::Transform3D(local(
                        [0.; 3], [0.; 3], [1.; 3],
                    ))],
                },
            ),
            (
                GameplayCommandId(2),
                GameplayCommand::AddComponent {
                    entity_id: id("cube"),
                    component_type: ComponentTypeId::transform3d(),
                    value: ComponentValue::Dynamic {
                        component_type: ComponentTypeId::transform3d(),
                        value: RuntimeValue::Null,
                    },
                },
            ),
            (
                GameplayCommandId(3),
                GameplayCommand::SetParent {
                    entity_id: id("cube"),
                    parent_id: Some(id("camera")),
                    keep_world_transform: true,
                },
            ),
        ],
    );
    assert!(results.iter().all(|r| r.result == "failed"));
    assert_eq!(world.entity_count(), count);
    assert!(world.dirty_records().is_empty());
    let records = apply_gameplay_commands_with_runtime(
        &mut world,
        vec![(
            GameplayCommandId(4),
            GameplayCommand::InstantiatePrefab {
                prefab_ref: RuntimeAssetRef {
                    id: "p".into(),
                    asset_type: "prefab".into(),
                    guid: None,
                    sub_asset: None,
                },
                parent_entity: None,
                target_scene_instance: None,
                position: Some(engine_runtime::math::Vec3::ZERO),
            },
        )],
        RuntimeCommandContext {
            package: &fixture.package,
            instance_loader: hydrator.instance_loader_mut(),
        },
    );
    assert_eq!(
        records[0].error_code,
        Some("three_d.legacy_position_unsupported")
    );
    assert_eq!(world.entity_count(), count);
    assert!(world.dirty_records().is_empty());
    let mut api = engine_runtime::world_api::WorldWriteApi::new(&mut world);
    assert!(api
        .write_component_field(
            id("cube"),
            ComponentTypeId::transform3d(),
            &engine_runtime::field_path::FieldPath::parse("scale.0").unwrap(),
            RuntimeValue::F64(0.)
        )
        .is_err());
}

#[test]
fn hierarchy_destroy_parent_and_unload_release_all_world_projection_state() {
    let mut source = base();
    source["entities"][0]["parentId"] = json!("root");
    source["entities"][1]["parentId"] = json!("root");
    source["entities"].as_array_mut().unwrap().push(node(
        "root",
        None,
        local([0.; 3], [0.; 3], [1.; 3]),
    ));
    let fixture = Fixture::new(source, None);
    let mut world = World::new();
    let mut hydrator = RuntimeSceneHydrator::from_package(&fixture.package);
    let report = hydrator.hydrate_active_scene(&fixture.package, &mut world);
    let instance = report.instance.unwrap();
    let mut context = RenderExtractContext::new();
    let mut scene = RenderSceneState::new();
    camera(&mut world, &mut context, &mut scene, [100, 100]);
    world.try_despawn_entity(&id("root")).unwrap();
    extract(&mut world, &mut context, &mut scene, 2);
    assert_eq!(world.entity_count(), 0);
    assert_eq!(scene.proxies_len(), 0);
    assert!(world.spatial_world(&id("camera")).is_none());
    assert!(context
        .project_main_camera(&world, &mut scene, [100, 100])
        .is_err());
    hydrator
        .instance_loader_mut()
        .unload_scene_instance(instance.instance_id, &mut world);
    assert!(context
        .project_main_camera(&world, &mut scene, [100, 100])
        .unwrap()
        .is_none());
    assert_eq!(scene.views().count(), 0);
    assert!(world.take_dirty_records().is_empty());
}

#[path = "three_d_runtime_a/session.rs"]
mod session;

#[test]
fn spatial_observation_selected_world_prepare_view_and_limits() {
    use engine_runtime::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
    use engine_runtime::frame_loop::RuntimeFrameContext;
    use engine_runtime::project_observation::ProjectObservationValue as V;
    let mut source = base();
    source["entities"][1]["spatial"]["transform"]["translation"] = json!([0, 0, 5]);
    let fixture = Fixture::new(source, None);
    let (mut world, mut hydrator) = fixture.hydrate();
    let mut host = EngineHostLoop::new("main");
    let output = host.tick_with_runtime_context(
        EngineFrameInput::new(EngineHostMode::ExportedGame),
        &mut world,
        RuntimeFrameContext::new(&fixture.package, hydrator.instance_loader_mut()),
    );
    let ids = vec!["cube".into(), "camera".into()];
    assert!(host
        .observe_spatial(&world, &ids, DiagnosticLevel::Off)
        .is_none());
    let report = host
        .observe_spatial(&world, &ids, DiagnosticLevel::Summary)
        .unwrap();
    assert_eq!(report.committed_frame, output.frame_index);
    assert_eq!(report.entities.len(), 2);
    assert_eq!(
        report.actual_value("engine.spatial/cube/inView"),
        Some(V::Bool(true))
    );
    assert_eq!(
        report.actual_value("engine.spatial/cube/material/surface"),
        Some(V::String("blue".into()))
    );
    assert_eq!(
        report.actual_value("engine.spatial/camera/z"),
        Some(V::Number(5.0))
    );
    assert_eq!(
        report.actual_value("engine.spatial/camera/mainCamera"),
        Some(V::Bool(true))
    );
    let draws = production_draws(output.render_thread_frame.as_ref().unwrap());
    assert_eq!(report.view.as_ref().unwrap().version, draws[0].view.version);
    assert_eq!(
        report.view.as_ref().unwrap().projection_matrix,
        draws[0].view.projection_matrix
    );
    let target = engine_runtime::runtime_renderer::RenderTarget::surface("resized", 1765, 993);
    assert!(host
        .observe_spatial_for_target(&world, &ids, DiagnosticLevel::Off, &target)
        .is_none());
    let resized = host
        .observe_spatial_for_target(&world, &ids, DiagnosticLevel::Summary, &target)
        .unwrap();
    assert!(resized.matches_projection);
    assert_eq!(resized.view.as_ref().unwrap().viewport, [0, 0, 1765, 993]);
    assert_eq!(resized.view.as_ref().unwrap().target_extent, [1765, 993]);
    let rendered = host.render_thread_for_target(target.clone());
    let resized_draws = production_draws(&rendered);
    assert_eq!(resized.view.as_ref(), Some(resized_draws[0].view.as_ref()));
    assert_eq!(resized.entities[0].in_view, report.entities[0].in_view);
    assert!(report.entities[0].bounds.is_some());
    let missing = host
        .observe_spatial(
            &world,
            &(0..100).map(|i| format!("missing-{i}")).collect::<Vec<_>>(),
            DiagnosticLevel::Summary,
        )
        .unwrap();
    assert!(missing.truncated);
    assert_eq!(missing.diagnostics.len(), 16);
    assert!(missing.entities.is_empty());
    let bounded = host
        .observe_spatial(&world, &vec!["camera".into(); 65], DiagnosticLevel::Summary)
        .unwrap();
    assert!(bounded.truncated);
    assert_eq!(bounded.entities.len(), 64);
    let trace = host
        .observe_spatial(&world, &vec!["camera".into(); 4097], DiagnosticLevel::Trace)
        .unwrap();
    assert!(trace.truncated);
    assert_eq!(trace.entities.len(), 4096);
    assert!(report.matches_projection);
    assert!(report.entities[0].culling.is_empty());
    assert_eq!(report.entities[0].culling_summary[0].included, 1);
    assert!(report.entities[0].resources[0]
        .geometry
        .content_digest
        .is_some());
    let detailed = host
        .observe_spatial(&world, &ids, DiagnosticLevel::Trace)
        .unwrap();
    assert_eq!(detailed.entities[0].culling.len(), 1);
    set(
        &mut world,
        "cube",
        ComponentValue::Transform3D(local([2., 0., 0.], [0.; 3], [1.; 3])),
    );
    let stale = host
        .observe_spatial(&world, &ids, DiagnosticLevel::Summary)
        .unwrap();
    assert!(!stale.matches_projection);
    assert!(stale.actual_value("engine.spatial/cube/x").is_none());
    assert!(
        !host
            .observe_spatial_for_target(&world, &ids, DiagnosticLevel::Summary, &target)
            .unwrap()
            .matches_projection
    );
}

#[cfg(feature = "real-wgpu")]
#[path = "three_d_runtime_a/lifecycle_gpu.rs"]
mod lifecycle_gpu;

#[path = "three_d_runtime_a/retained.rs"]
mod retained;

/// Public loader seam: completion occupancy is actual work, not admission capacity.
#[test]
fn resource_memory_loader_counts_real_completed_failed_cancelled_work_and_drain() {
    use engine_runtime::resource_memory::{MemoryCoverage, ResourceMemorySample};
    use engine_runtime::runtime_asset::{
        BundleRecord, CookedAssetRecord, RuntimeAssetIndex, RuntimeAssetRecord,
        RuntimePackageMountTable,
    };
    use engine_runtime::runtime_asset_loader::RuntimeAssetLoader;
    let root = std::env::temp_dir().join(format!("aife-memory-loader-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let reference = RuntimeAssetRef {
        id: "audio".into(),
        asset_type: "audio".into(),
        guid: Some("audio-guid".into()),
        sub_asset: None,
    };
    for outcome in ["success", "failure", "cancel"] {
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut w = hound::WavWriter::new(
                &mut cursor,
                hound::WavSpec {
                    channels: 1,
                    sample_rate: 44100,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
            )
            .unwrap();
            for sample in [-32768_i16, 0, 16384, 32767] {
                w.write_sample(sample).unwrap();
            }
            w.finalize().unwrap();
        }
        let bytes = cursor.into_inner();
        let hash = engine_runtime::canonical_digest::sha256_prefixed(&bytes);
        fs::write(
            root.join("sound.wav"),
            if outcome == "failure" {
                b"corrupt".as_slice()
            } else {
                &bytes
            },
        )
        .unwrap();
        let record = RuntimeAssetRecord {
            asset_guid: "audio-guid".into(),
            asset_id: "audio".into(),
            asset_type: "audio".into(),
            sub_asset_id: None,
            version: "1".into(),
            cooked_asset_id: "cooked-audio".into(),
            bundle_id: "startup".into(),
            loader_kind: "audio".into(),
            dependencies: vec![],
            hash: Some(hash.clone()),
            size: Some(bytes.len() as u64),
            flags: vec![],
            source_map_debug: None,
        };
        let cooked = CookedAssetRecord {
            cooked_asset_id: "cooked-audio".into(),
            bundle_id: "startup".into(),
            path: Some("sound.wav".into()),
            offset: None,
            size: Some(bytes.len() as u64),
            compression: None,
            hash: Some(hash),
        };
        let mut loader = RuntimeAssetLoader::new(
            &root,
            RuntimeAssetIndex::new(vec![record], vec![cooked]),
            RuntimePackageMountTable::new(vec![BundleRecord {
                bundle_id: "startup".into(),
                mount_id: None,
                uri: "startup".into(),
                hash: None,
                version: None,
                mounted: true,
            }]),
        );
        let census = |loader: &RuntimeAssetLoader| {
            let mut s = ResourceMemorySample::new(1, 1, 0);
            loader.observe_memory(&mut s);
            s
        };
        assert_eq!(census(&loader).capacities["activeRequests"], 0);
        let ticket = loader.request_async(&reference).unwrap();
        let shared_ticket = loader.request_async(&reference).unwrap();
        assert_eq!(loader.streaming_snapshot().dispatched_operations, 1);
        let start = std::time::Instant::now();
        // No poll: completed payload stays owned by the mailbox until consumed.
        loop {
            let s = census(&loader);
            if s.capacities["readyQueue"] == 1 {
                break;
            }
            assert!(start.elapsed() < std::time::Duration::from_secs(3));
            std::thread::yield_now();
        }
        let s = census(&loader);
        assert_eq!(s.capacities["activeRequests"], 2);
        assert_eq!(s.capacities["concurrentDecodes"], 0);
        assert_eq!(s.capacities["failureCache"], 0);
        if outcome == "cancel" {
            loader.release(&ticket).unwrap();
        }
        // The ready count includes a producer holding a completion just before send.
        // Drain the actual operation instead of assuming that interval is mailbox-ready.
        while loader.streaming_snapshot().pending_operations > 0 {
            loader.poll_async();
            assert!(start.elapsed() < std::time::Duration::from_secs(3));
            std::thread::yield_now();
        }
        let s = census(&loader);
        assert_eq!(s.capacities["activeRequests"], 0);
        assert_eq!(s.capacities["readyQueue"], 0);
        assert_eq!(s.capacities["concurrentDecodes"], 0);
        assert_eq!(
            s.capacities["failureCache"],
            2 * u64::from(outcome == "failure")
        );
        assert_eq!(s.coverage["cpu.owner.queues"], MemoryCoverage::Partial);
        if outcome == "success" {
            assert!(
                s.allocations
                    .iter()
                    .any(|a| a.bytes == 4 * std::mem::size_of::<f32>() as u64),
                "decoded PCM backing must be counted separately from WAV bytes"
            );
        }
        if outcome != "cancel" {
            loader.release(&ticket).unwrap();
        }
        loader.release(&shared_ticket).unwrap();
        assert!(loader.shutdown_streaming(std::time::Duration::from_secs(2)));
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn resource_memory_world_counts_archetype_and_nested_dynamic_capacity() {
    use engine_runtime::{
        component_value::RuntimeValue,
        components::{ComponentTypeId, Hierarchy},
        resource_memory::*,
        world::World,
    };
    let mut world = World::new();
    world
        .try_spawn_entity(
            "memory-node".into(),
            "Node",
            "actor",
            true,
            Hierarchy {
                parent_id: None,
                sibling_order: 0,
            },
        )
        .unwrap();
    let mut text = String::with_capacity(8192);
    text.push_str("actual dynamic payload");
    world
        .try_insert_dynamic_component(
            "memory-node".into(),
            ComponentTypeId::new("project.memory"),
            RuntimeValue::Array(vec![RuntimeValue::String(text)]),
        )
        .unwrap();
    let mut sample = ResourceMemorySample::new(1, 1, 0);
    world.observe_memory(&mut sample);
    assert!(sample.allocations.iter().any(|a| a.bytes == 8192));
    assert!(memory_totals(&sample.allocations).unwrap().cpu_allocated > 8192);
    assert_eq!(sample.coverage["cpu.owner.world"], MemoryCoverage::Partial);
    let before = memory_totals(&sample.allocations).unwrap().cpu_allocated;
    world.try_despawn_entity(&"memory-node".into()).unwrap();
    let mut after = ResourceMemorySample::new(2, 1, 1);
    world.observe_memory(&mut after);
    assert!(memory_totals(&after.allocations).unwrap().cpu_allocated < before);
    assert!(after.allocations.iter().all(|a| a.bytes != 8192));
}
