//! Typed Cook-payload fixture -> real package -> World extraction -> production.
//! No backend/GPU residency claim: this exercises the existing native Prepare
//! contract and lifetime manager, with the sample's actual vertex/source bytes.
use super::*;
use crate::{
    archetype::ComponentValue,
    components::ComponentTypeId,
    ids::EntityId,
    render_command::apply_batch,
    render_contract::*,
    render_extract::RenderExtractContext,
    runtime_package::*,
    runtime_package_builder::*,
    world::{DirtyType, World},
};
use serde_json::json;

#[test]
#[ignore = "explicit full published BM13 package; real projection/Prepare diagnosis"]
fn s5d_bm13_published_projection_admits_all_forest_parts() {
    let root = std::path::PathBuf::from(std::env::var_os("AIFE_BM13_LOAD_PACKAGE").unwrap());
    let package = load_runtime_package(&root).value.unwrap();
    let mut world = World::new();
    let mut instances =
        crate::runtime_instance_loader::RuntimeInstanceLoader::from_package(&package);
    instances
        .asset_loader_mut()
        .set_streaming_budget(crate::runtime_asset_loader::StreamingBudget {
            max_handles: 65_536,
            max_operations: 256,
            max_cpu_resident_bytes: 2 * 1024 * 1024 * 1024,
            ..Default::default()
        })
        .unwrap();
    assert!(instances.asset_loader_mut().mount_bundle("startup"));
    let (loaded, report) = instances.load_active_scene_instance(&package, &mut world);
    assert!(loaded.is_some(), "{report:?}");
    let loader = instances.asset_loader_mut();
    world.finish_spatial_tick();
    let mut extract = RenderExtractContext::new();
    let mut scene = RenderSceneState::new();
    let queue = extract
        .extract_world_dirty(1, &mut world, &scene)
        .normalize_merge(&scene);
    assert!(apply_batch(&mut scene, &queue).is_empty());
    extract
        .project_main_camera(&world, &mut scene, [1765, 993])
        .unwrap()
        .expect("published scene has its real main camera");
    let mut cache = ProductionGeometryCache::default();
    let mut manager = RenderResourceManager::new();
    let start = std::time::Instant::now();
    let mut frame = 0;
    loop {
        frame += 1;
        loader.poll_async();
        let snapshot = cache.snapshot(
            frame,
            &scene,
            "bm13-published",
            Some(loader),
            &mut manager,
            None,
        );
        let requests = cache
            .vegetation
            .requests
            .iter()
            .map(|(key, r)| {
                (
                    key,
                    loader.get_handle_state(&r.handle),
                    loader.streaming_error(&r.handle),
                )
            })
            .collect::<Vec<_>>();
        if let Some(error) = snapshot.preparation_error() {
            assert_eq!(
                error.code,
                ContractErrorCode::ResourceNotReady,
                "{error:?}; requests={requests:?}; budget={:?}",
                loader.streaming_snapshot()
            );
            assert!(
                start.elapsed().as_secs() < 45,
                "{error:?}; requests={requests:?}"
            );
        } else {
            assert_eq!(
                cache.vegetation.instances.len(),
                100_000,
                "all 50K sources retain both LOD candidates"
            );
            let vegetation_ids: std::collections::HashSet<_> = cache
                .vegetation
                .instances
                .iter()
                .map(|instance| instance.instance_id)
                .collect();
            let selected = snapshot
                .build(
                    &crate::runtime_renderer::RenderTarget::surface("surface", 1765, 993),
                    &manager,
                )
                .unwrap();
            let views: Vec<_> = selected
                .views
                .iter()
                .filter(|view| !view.view.is_shadow_map())
                .collect();
            assert_eq!(views.len(), 1, "published main camera must be unambiguous");
            let mut visible_vegetation = std::collections::HashSet::new();
            for batch in views[0].batches.as_ref().unwrap() {
                for member in batch.visible.iter() {
                    let id = batch.instances[member.instance_index as usize].instance_id;
                    if vegetation_ids.contains(&id) {
                        assert!(
                            visible_vegetation.insert(id),
                            "duplicate selected source {id}"
                        );
                    }
                }
            }
            eprintln!(
                "BM13 published main-view selected vegetation={}, total sources=50000",
                visible_vegetation.len()
            );
            assert!(
                (10_000..=50_000).contains(&visible_vegetation.len()),
                "main view must select >=10K and at most one LOD per source"
            );
            break;
        }
        std::thread::yield_now();
    }
}

struct Fixture {
    root: std::path::PathBuf,
    package: RuntimePackage,
    world: World,
    extract: RenderExtractContext,
    scene: RenderSceneState,
    loader: RuntimeAssetLoader,
    cache: ProductionGeometryCache,
    manager: RenderResourceManager,
    frame: u64,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn payload(
    input: &mut RuntimePackageBuildInput,
    id: &str,
    kind: &str,
    bytes: Vec<u8>,
    dependencies: Vec<String>,
) {
    let mut asset = RuntimePackageSourceAsset::new(
        id,
        id,
        kind,
        format!("Assets/S4Environment/{id}.asset"),
        format!("cooked-assets/{id}.json"),
    )
    .with_runtime_payload(bytes.clone());
    asset.asset_guid = Some(format!("guid-{id}"));
    asset.hash = Some(crate::canonical_digest::sha256_prefixed(&bytes));
    asset.dependencies = dependencies;
    input.assets.push(asset);
}
fn fixture() -> Fixture {
    fixture_with_lods(false)
}
fn fixture_with_lods(lods: bool) -> Fixture {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let root = std::env::temp_dir().join(format!(
        "s4c-production-vegetation-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let package_dir = root.join("package");
    std::fs::create_dir_all(&root).unwrap();
    let forest = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../samples/platform_3d_world_project/Assets/S4Environment/vegetation.asset"
    ));
    let mut forest: serde_json::Value = serde_json::from_slice(forest).unwrap();
    if lods {
        for cluster in forest["clusters"].as_array_mut().unwrap() {
            cluster["lods"] = json!([{"distance":20.0,"mesh":json!({"expectedKind":"mesh","reference":{"id":"s4-tree-coarse","guid":"guid-s4-tree-coarse","type":"mesh"}})}]);
        }
    }
    let mesh_source = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../samples/platform_3d_world_project/Assets/S4Environment/tree-mesh.asset"
    ));
    let material_source = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../samples/platform_3d_world_project/Assets/S4Environment/tree-material.asset"
    ));
    let mesh: serde_json::Value = serde_json::from_slice(mesh_source).unwrap();
    let mesh: RenderContract<RenderAssetData> =
        serde_json::from_value(mesh["renderData"].clone()).unwrap();
    mesh.data.validate().unwrap();
    let material: serde_json::Value = serde_json::from_slice(material_source).unwrap();
    let color: [f32; 4] = serde_json::from_value(material["baseColor"].clone()).unwrap();
    let material = CookedMaterial {
        decal_bias: None,
        base_color_texture_sampling: None,
        opaque_alpha: false,
        ignore_vertex_color: false,
        preset: MaterialPreset::Unlit,
        texture_sampling: Default::default(),
        parameters: [("baseColor".into(), MaterialParameter::Vector4(color))].into(),
        textures: Default::default(),
        blend: MaterialBlend::Opaque,
        cull: MaterialCull::Back,
        depth: MaterialDepth::TestAndWrite,
        shader_source: UNLIT_SHADER_SOURCE.into(),
    };
    material.validate().unwrap();
    let mut document = json!({"schemaVersion":"runtime-scene.v2","id":"main","name":"Forest","gravity":0,"background":"#000000","skyColor":"#000000","mainCamera":"camera","entities":[{
        "schemaVersion":"runtime-entity.v2","id":"forest","name":"Forest","enabled":true,"parentId":null,"siblingOrder":0,
        "spatial":{"domain":"world3d","transform":{"translation":[0,0,0],"rotationXyzw":[0,0,0,1],"scale":[1,1,1]},
        "meshRenderer":{"schemaVersion":"mesh-renderer3d.v1","mesh":{"id":"s4-tree-mesh","type":"mesh","guid":"guid-s4-tree-mesh"},"materials":{"foliage":{"id":"s4-tree-material","type":"material","guid":"guid-s4-tree-material"}},"visible":true,"layers":["world"]}},
        "components":[{"componentType":"engine.vegetation3d","data":{"clusterAsset":{"assetId":"s4-forest","assetGuid":"guid-s4-forest"},"promotedInstanceIds":[]}}]}]});
    let ordinary: serde_json::Value = serde_json::from_slice(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../engine_runtime/tests/fixtures/three_d_a0/runtime-scene-v2.json"
    )))
    .unwrap();
    document["entities"]
        .as_array_mut()
        .unwrap()
        .push(ordinary["entities"][1].clone());
    let crate::spatial_contract::RuntimeSceneDocument::Spatial(scene) =
        crate::spatial_contract::decode_runtime_scene(&serde_json::to_vec(&document).unwrap())
            .unwrap()
    else {
        panic!()
    };
    let scene: RuntimeScene = scene.into();
    let mut input = RuntimePackageBuildInput::new(RuntimeProjectInfo::explicit_empty(
        "vegetation",
        "Vegetation",
        "1",
    ));
    let mapping = crate::input_mapping::InputMappingAsset::explicit_empty("input.none");
    input.input_mappings.push(RuntimePackageSourceJson {
        id: mapping.asset_id.clone(),
        document: serde_json::to_value(mapping).unwrap(),
    });
    input.assets.push(RuntimePackageSourceAsset::new(
        "main",
        "Main",
        "scene",
        "Scenes/main.json",
        "scenes/main.json",
    ));
    input.scenes.push(scene.clone());
    let mut coarse = mesh.data.clone();
    if let RenderAssetData::Mesh(data) = &mut coarse {
        data.index_count = 6;
        data.index_bytes.truncate(6 * data.index_format.byte_len());
        data.submeshes[0].index_count = 6;
    }
    for (id, source, data) in [
        ("s4-tree-mesh", mesh_source.as_slice(), mesh.data),
        (
            "s4-tree-material",
            material_source.as_slice(),
            RenderAssetData::Material(material),
        ),
    ]
    .into_iter()
    .chain(lods.then_some(("s4-tree-coarse", mesh_source.as_slice(), coarse)))
    {
        let kind = data.asset_kind().as_str();
        let bytes = serde_json::to_vec(&RenderContract::new(CookedRenderAsset {
            source_digest: crate::canonical_digest::sha256_prefixed(source),
            recipe: NATIVE_RENDER_COOK_RECIPE.into(),
            asset: data,
        }))
        .unwrap();
        payload(&mut input, id, kind, bytes, vec![]);
    }
    payload(
        &mut input,
        "s4-forest",
        "vegetation3d",
        serde_json::to_vec(&forest).unwrap(),
        if lods {
            vec![
                "guid-s4-tree-mesh".into(),
                "guid-s4-tree-material".into(),
                "guid-s4-tree-coarse".into(),
            ]
        } else {
            vec!["guid-s4-tree-mesh".into(), "guid-s4-tree-material".into()]
        },
    );
    let report = RuntimePackageBuilder::build(
        &RuntimePackageBuildRequest::dev_desktop(&package_dir, "main"),
        &input,
    );
    assert_eq!(
        report.status,
        RuntimePackageBuildStatus::Success,
        "{:?}",
        report.diagnostics
    );
    let loaded = load_runtime_package(&package_dir);
    assert!(loaded.diagnostics.is_ok(), "{:?}", loaded.diagnostics);
    let package = loaded.value.unwrap();
    let mut loader = RuntimeAssetLoader::new(
        package.package_dir.clone(),
        package.runtime_asset_index.clone(),
        package.runtime_asset_mount_table.clone(),
    );
    assert!(loader.mount_bundle("startup"));
    let world = crate::scene_loader::load_scene_into_world(&scene)
        .value
        .unwrap();
    Fixture {
        root,
        package,
        world,
        extract: RenderExtractContext::new(),
        scene: RenderSceneState::new(),
        loader,
        cache: ProductionGeometryCache::default(),
        manager: RenderResourceManager::new(),
        frame: 0,
    }
}

#[test]
fn s5d_vegetation_lods_select_each_source_per_view_across_origins_and_release() {
    let mut f = fixture_with_lods(true);
    f.extract(0.25);
    let first = f.ready();
    assert_eq!(
        first.instances.len(),
        256,
        "two representations of 128 sources"
    );
    let (_, camera_fixture) =
        super::super::output_binding_tests::mesh_snapshot(MaterialBlend::Opaque);
    let mut camera = camera_fixture.views[0].camera.clone();
    let matrix = first.instances[0].projection.matrix();
    let origin = first.instances[0].projection.origin();
    camera.origin = origin;
    camera.pose.position = [matrix[12], matrix[13], matrix[14]];
    let selected = |camera: &crate::render_projection_adapter::CameraProjection| {
        first
            .instances
            .iter()
            .filter(|i| i.projection.visible(camera))
            .map(|i| {
                let InstanceProjection::Vegetation { source, .. } = &i.projection else {
                    panic!()
                };
                (source.to_string(), i.instance_id)
            })
            .collect::<BTreeMap<_, _>>()
    };
    let near = selected(&camera);
    assert_eq!(near.len(), 128);
    assert_eq!(
        first
            .instances
            .iter()
            .filter(|i| i.projection.visible(&camera))
            .count(),
        128
    );
    camera.pose.position[0] += 100.;
    let far = selected(&camera);
    assert_eq!(far.len(), 128);
    assert!(near.iter().any(|(source, id)| far[source] != *id));
    let far_work = first.prepared_workset(&camera, &f.manager).unwrap();
    assert_eq!(far_work.instances.len(), 128);
    assert!(far_work.instances.iter().all(|i| i
        .projection
        .presentation()
        .unwrap()
        .range
        .unwrap()
        .0
        == 20.));
    assert_eq!(
        f.cache.vegetation.prepared.len(),
        2,
        "distinct real fine/coarse meshes use the original Prepare owner"
    );
    assert!(!Arc::ptr_eq(
        &first.instances[0].prepared,
        &first.instances[1].prepared
    ));
    // Rebase the camera without moving its absolute position; selection stays identical.
    camera.origin[0] += 1024.;
    camera.pose.position[0] -= 1024.;
    assert_eq!(far, selected(&camera));
    // Each half-open boundary selects exactly one representation of a source.
    for distance in [19.999, 20., 20.001] {
        camera.origin = origin;
        camera.pose.position = [matrix[12], matrix[13], matrix[14] + distance];
        assert_eq!(first.instances.iter().filter(|i| matches!(&i.projection, InstanceProjection::Vegetation { source, .. } if source.as_ref()=="tree-west-000") && i.projection.visible(&camera)).count(), 1);
    }
    let ty = ComponentTypeId::new("engine.vegetation3d");
    f.world.try_insert_component_value(EntityId::from("forest"),ty.clone(),ComponentValue::Dynamic{component_type:ty,value:crate::character_animation::runtime_json(json!({"clusterAsset":{"assetId":"s4-forest","assetGuid":"guid-s4-forest"},"promotedInstanceIds":["tree-west-000"]}))}).unwrap();
    f.extract(0.25);
    let promoted = f.ready();
    assert_eq!(promoted.instances.len(), 254);
    assert!(promoted.instances.iter().all(|i| matches!(&i.projection, InstanceProjection::Vegetation {source,..} if source.as_ref()!="tree-west-000")));
    f.world
        .try_despawn_entity(&EntityId::from("forest"))
        .unwrap();
    f.extract(0.25);
    let empty = f.ready();
    assert!(empty.instances.is_empty());
    assert!(f.cache.vegetation.requests.is_empty() && f.cache.vegetation.prepared.is_empty());
    drop((first, promoted, empty, far_work));
    f.manager.end_frame(f.frame + 100);
    assert_eq!(f.manager.report(f.frame + 100).resident_bytes, 0);
}
impl Fixture {
    fn extract(&mut self, time: f64) {
        self.world.presentation_time_seconds = time;
        self.world
            .mark_dirty(EntityId::from("forest"), DirtyType::RenderState);
        self.world.finish_spatial_tick();
        self.frame += 1;
        let queue = self
            .extract
            .extract_world_dirty(self.frame, &mut self.world, &self.scene)
            .normalize_merge(&self.scene);
        assert!(apply_batch(&mut self.scene, &queue).is_empty());
    }
    fn snapshot(&mut self) -> Arc<ProductionGeometrySnapshot> {
        self.frame += 1;
        self.loader.poll_async();
        self.cache.snapshot(
            self.frame,
            &self.scene,
            "s4-production-package",
            Some(&mut self.loader),
            &mut self.manager,
            None,
        )
    }
    fn ready(&mut self) -> Arc<ProductionGeometrySnapshot> {
        let start = std::time::Instant::now();
        loop {
            let snapshot = self.snapshot();
            if snapshot.preparation_error().is_none() {
                return snapshot;
            }
            assert!(
                start.elapsed().as_secs() < 10,
                "{:?}",
                snapshot.preparation_error()
            );
            std::thread::yield_now();
        }
    }
}

#[test]
fn s4c_real_sample_package_extract_prepare_shares_128_wind_instances_and_releases_original_owners()
{
    let mut f = fixture();
    assert!(f
        .package
        .runtime_asset_index
        .record_by_guid("guid-s4-forest")
        .is_some());
    f.extract(0.);
    let first = f.ready();
    assert_eq!(first.instances.len(), 128);
    assert_eq!(f.cache.vegetation.requests.len(), 1);
    assert_eq!(f.cache.vegetation.prepared.len(), 1);
    assert!(first
        .instances
        .iter()
        .all(|i| i.prepared.geometry[i.submesh].material_slot() == "foliage"));
    assert!(first
        .instances
        .iter()
        .all(|i| Arc::ptr_eq(&i.prepared, &first.instances[0].prepared)));
    let InstanceProjection::Vegetation { owner, .. } = &first.instances[0].projection else {
        panic!()
    };
    assert!(first.instances.iter().all(|i|matches!(&i.projection,InstanceProjection::Vegetation{owner:other,..} if Arc::ptr_eq(owner,other))));
    let initial = first
        .instances
        .iter()
        .map(|i| (i.instance_id, i.projection.matrix()))
        .collect::<BTreeMap<_, _>>();
    f.extract(0.25);
    let wind = f.ready();
    assert!(
        wind.instances
            .iter()
            .any(|i| initial[&i.instance_id] != i.projection.matrix()),
        "wind must change actual consumed matrix"
    );
    assert_eq!(
        first
            .instances
            .iter()
            .map(|i| (i.instance_id, i.projection.matrix()))
            .collect::<BTreeMap<_, _>>(),
        initial,
        "older packet must stay immutable"
    );
    f.extract(0.25);
    let paused = f.ready();
    assert!(
        wind.instances
            .iter()
            .zip(paused.instances.iter())
            .all(|(a, b)| a.instance_id == b.instance_id
                && a.projection.matrix() == b.projection.matrix()),
        "paused fixed time must keep matrices stable"
    );
    let ty = ComponentTypeId::new("engine.vegetation3d");
    f.world.try_insert_component_value(EntityId::from("forest"),ty.clone(),ComponentValue::Dynamic{component_type:ty,value:crate::character_animation::runtime_json(json!({"clusterAsset":{"assetId":"s4-forest","assetGuid":"guid-s4-forest"},"promotedInstanceIds":["tree-west-000"]}))}).unwrap();
    f.extract(0.25);
    let promoted = f.ready();
    assert_eq!(promoted.instances.len(), 127);
    assert!(promoted.instances.iter().all(|i|matches!(&i.projection,InstanceProjection::Vegetation{source,..} if source.as_ref()!="tree-west-000")));
    assert!(
        promoted
            .instances
            .iter()
            .all(|i| initial.contains_key(&i.instance_id)),
        "remaining source identities must survive promotion"
    );
    f.world
        .try_despawn_entity(&EntityId::from("forest"))
        .unwrap();
    f.extract(0.25);
    let empty = f.ready();
    assert!(empty.instances.is_empty());
    assert!(
        f.cache.vegetation.requests.is_empty()
            && f.cache.vegetation.prepared.is_empty()
            && f.cache.vegetation.handles.is_empty()
    );
    assert_eq!(
        f.loader.decoded_cache_len(),
        0,
        "removed owner must release original loader requests"
    );
    drop((first, wind, paused, promoted, empty));
    f.manager.end_frame(f.frame + 100);
    assert_eq!(
        f.manager.report(f.frame + 100).resident_bytes,
        0,
        "native manager must return to empty baseline after packet leases release"
    );
}
