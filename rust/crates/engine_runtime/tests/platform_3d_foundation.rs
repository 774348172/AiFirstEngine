use engine_runtime::{
    character_animation, physics_3d, render_contract, runtime_asset_loader, runtime_package,
    spatial_contract,
};
use engine_runtime::{
    project_runtime_session::ProjectRuntimeMutationBuffer,
    scene_loader::load_scene_into_world,
    spatial_contract::{decode_runtime_scene, RuntimeSceneDocument},
    world::World,
};
use project_runtime_sdk::{ProjectRuntimeSpatialIntent as Intent, ProjectRuntimeTransform3D};
#[path = "support/asset_memory.rs"]
mod asset_memory;
#[path = "support/foundation_faults.rs"]
mod foundation_faults;
#[cfg(feature = "real-wgpu")]
#[path = "support/foundation_hud.rs"]
mod foundation_hud;
#[cfg(all(windows, feature = "real-wgpu"))]
#[path = "support/foundation_lifecycle.rs"]
mod foundation_lifecycle;
#[cfg(feature = "real-wgpu")]
#[path = "support/foundation_projection.rs"]
mod foundation_projection;
#[path = "support/streamed_cells.rs"]
mod streamed_cells;
#[path = "support/streaming_geometry_tests.rs"]
mod streaming_geometry_tests;

#[test]
#[ignore = "explicit published BM08 package; first real cell preparation diagnosis"]
fn bm08_published_first_cell_prepares_with_original_streaming_budget() {
    use engine_runtime::{
        render_resource::RenderResourceManager, runtime_asset_loader::StreamingBudget,
        runtime_instance_loader::*, runtime_package::*,
    };
    let root = std::path::PathBuf::from(std::env::var_os("AIFE_BM08_LOAD_PACKAGE").unwrap());
    let package = load_runtime_package(root).value.unwrap();
    let mut world = World::new();
    let mut loader = RuntimeInstanceLoader::from_package(&package);
    loader
        .asset_loader_mut()
        .set_streaming_budget(StreamingBudget {
            max_handles: 65_536,
            max_operations: 256,
            max_cpu_resident_bytes: 2 * 1024 * 1024 * 1024,
            ..Default::default()
        })
        .unwrap();
    assert!(loader.asset_loader_mut().mount_bundle("startup"));
    let (instance, report) = loader.load_active_scene_instance(&package, &mut world);
    assert!(instance.is_some(), "{report:?}");
    let ticket = loader
        .request_cell(
            CellActivationRequest {
                schema_version: "cell-activation.v1".into(),
                placement_id: "cell-000".into(),
                prefab: RuntimeAssetRef {
                    id: "cell-000".into(),
                    asset_type: "prefab".into(),
                    guid: Some("cell-000".into()),
                    sub_asset: None,
                },
                asset_revision_epoch: loader.asset_loader().asset_revision_epoch(),
                required_domains: std::collections::BTreeSet::from([
                    CellDomain::World,
                    CellDomain::Asset,
                    CellDomain::Render,
                ]),
                parent: None,
            },
            &world,
        )
        .unwrap();
    let mut manager = RenderResourceManager::new();
    let start = std::time::Instant::now();
    loop {
        match loader.prepare_cell(ticket, &world, 1, &mut manager) {
            Ok(_) => break,
            Err(CellError::NotReady) => assert!(
                start.elapsed().as_secs() < 30,
                "{:?}",
                loader.cell_error(ticket)
            ),
            Err(e) => panic!(
                "BM08 first actual cell: {e:?}; loader={:?}",
                loader.asset_loader().streaming_snapshot()
            ),
        }
        std::thread::yield_now();
    }
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires a real GPU"]
fn gpu_prepared_texture_receipt_rejects_replaced_pixels_under_the_same_handle() {
    use engine_runtime::{
        render_asset_production::RuntimeRenderAssetProducer, render_contract::RenderBindingKind,
        render_resource::RenderResourceManager, runtime_texture::RuntimeTexturePayload,
        wgpu_backend::real::RealWgpuBackend,
    };
    let mut manager = RenderResourceManager::new();
    let prepared = RuntimeRenderAssetProducer
        .prepare_texture_payload(
            0,
            RuntimeTexturePayload {
                asset_id: "texture".into(),
                cooked_asset_id: "cooked".into(),
                width: 1,
                height: 1,
                format: "rgba8UnormSrgb".into(),
                color_space: "srgb".into(),
                mip_count: 1,
                rgba8: vec![1, 2, 3, 255].into(),
                sampler: "linearClamp".into(),
                source_hash: "same-source".into(),
            },
            RenderBindingKind::Texture,
            &mut manager,
        )
        .unwrap();
    let mut backend = RealWgpuBackend::new_offscreen(8, 8).unwrap();
    let first = backend
        .begin_geometry_upload(&[], std::slice::from_ref(&prepared))
        .unwrap();
    let mut replaced = prepared.clone();
    std::sync::Arc::make_mut(&mut std::sync::Arc::make_mut(&mut replaced.payload).rgba8)[0] ^= 1;
    assert_eq!(
        backend.begin_geometry_upload(&[], &[replaced]).unwrap_err(),
        "geometry.texture_identity_conflict"
    );
    let mut equal = prepared.clone();
    equal.payload = std::sync::Arc::new(prepared.payload.as_ref().clone());
    let second = backend.begin_geometry_upload(&[], &[equal]).unwrap();
    backend.release_geometry_upload(first).unwrap();
    backend.release_geometry_upload(second).unwrap();
}

#[test]
fn worker_ready_material_preparations_share_immutable_pixels_after_package_removal() {
    use engine_runtime::{
        render_asset_production::RuntimeRenderAssetProducer, render_resource::RenderResourceManager,
    };
    let fixture = streamed_cells::build(true);
    let mut loader = engine_runtime::runtime_asset_loader::RuntimeAssetLoader::new(
        &fixture.package.package_dir,
        fixture.package.runtime_asset_index.clone(),
        fixture.package.runtime_asset_mount_table.clone(),
    );
    loader.mount_bundle("startup");
    let root = loader
        .request_async(&streamed_cells::reference("cell-a", "prefab"))
        .unwrap();
    let wait = std::time::Instant::now();
    while loader.get_handle_state(&root)
        == Some(engine_runtime::runtime_asset::RuntimeAssetLoadState::Loading)
    {
        loader.poll_async();
        assert!(wait.elapsed() < std::time::Duration::from_secs(5));
        std::thread::yield_now();
    }
    assert_eq!(
        loader.get_handle_state(&root),
        Some(engine_runtime::runtime_asset::RuntimeAssetLoadState::Ready)
    );
    std::fs::remove_dir_all(&fixture.package.package_dir).unwrap();
    let reference = engine_runtime::render_contract::RenderAssetReference {
        expected_kind: engine_runtime::render_contract::RenderAssetKind::Material,
        reference: streamed_cells::reference("material-a", "material"),
    };
    let mut manager = RenderResourceManager::new();
    let first = RuntimeRenderAssetProducer
        .prepare_cooked_from_package(1, &reference, &mut loader, &mut manager)
        .unwrap();
    let second = RuntimeRenderAssetProducer
        .prepare_cooked_from_package(2, &reference, &mut loader, &mut manager)
        .unwrap();
    assert!(std::sync::Arc::ptr_eq(
        &first.textures[0].payload,
        &second.textures[0].payload
    ));
    assert_eq!(first.textures[0].handle, second.textures[0].handle);
    let copied = first.textures[0].clone();
    assert!(std::sync::Arc::ptr_eq(
        &copied.payload,
        &first.textures[0].payload
    ));
    loader.release(&root).unwrap();
    assert!(loader.shutdown_streaming(std::time::Duration::from_secs(2)));
}

#[test]
fn synchronous_ready_material_preparations_share_decoded_pixels() {
    use engine_runtime::{
        render_asset_production::RuntimeRenderAssetProducer,
        render_resource::RenderResourceManager, runtime_asset_loader::RuntimeAssetLoader,
    };
    let fixture = streamed_cells::build(true);
    let mut loader = RuntimeAssetLoader::new(
        &fixture.package.package_dir,
        fixture.package.runtime_asset_index.clone(),
        fixture.package.runtime_asset_mount_table.clone(),
    );
    loader.mount_bundle("startup");
    let root = loader
        .load(&streamed_cells::reference("cell-a", "prefab"))
        .unwrap();
    let reference = engine_runtime::render_contract::RenderAssetReference {
        expected_kind: engine_runtime::render_contract::RenderAssetKind::Material,
        reference: streamed_cells::reference("material-a", "material"),
    };
    let mut manager = RenderResourceManager::new();
    let first = RuntimeRenderAssetProducer
        .prepare_cooked_from_package(1, &reference, &mut loader, &mut manager)
        .unwrap();
    let second = RuntimeRenderAssetProducer
        .prepare_cooked_from_package(2, &reference, &mut loader, &mut manager)
        .unwrap();
    assert_eq!(
        first.textures[0].payload.rgba8.as_ptr(),
        second.textures[0].payload.rgba8.as_ptr(),
        "synchronous Ready metadata caused repeated uncached pixel decode"
    );
    std::fs::remove_dir_all(&fixture.package.package_dir).unwrap();
    let third = RuntimeRenderAssetProducer
        .prepare_cooked_from_package(3, &reference, &mut loader, &mut manager)
        .unwrap();
    assert_eq!(
        first.textures[0].payload.rgba8.as_ptr(),
        third.textures[0].payload.rgba8.as_ptr()
    );
    loader.release(&root).unwrap();
}

#[test]
#[ignore = "local development cost diagnostic; wall-clock threshold is not a release qualification"]
fn worker_ready_large_texture_preparation_cost() {
    use engine_runtime::{
        render_asset_production::RuntimeRenderAssetProducer, render_resource::RenderResourceManager,
    };
    let fixture = streamed_cells::build_with_texture_extent(true, true, 2048);
    let mut loader = engine_runtime::runtime_asset_loader::RuntimeAssetLoader::new(
        &fixture.package.package_dir,
        fixture.package.runtime_asset_index.clone(),
        fixture.package.runtime_asset_mount_table.clone(),
    );
    loader.mount_bundle("startup");
    let root = loader
        .request_async(&streamed_cells::reference("cell-a", "prefab"))
        .unwrap();
    let wait = std::time::Instant::now();
    while loader.get_handle_state(&root)
        == Some(engine_runtime::runtime_asset::RuntimeAssetLoadState::Loading)
    {
        loader.poll_async();
        assert!(wait.elapsed() < std::time::Duration::from_secs(10));
        std::thread::yield_now();
    }
    assert_eq!(
        loader.get_handle_state(&root),
        Some(engine_runtime::runtime_asset::RuntimeAssetLoadState::Ready)
    );
    // Eliminate synchronous fallback as a possible explanation for this cost.
    std::fs::remove_dir_all(&fixture.package.package_dir).unwrap();
    let reference = engine_runtime::render_contract::RenderAssetReference {
        expected_kind: engine_runtime::render_contract::RenderAssetKind::Material,
        reference: streamed_cells::reference("material-a", "material"),
    };
    let mut manager = RenderResourceManager::new();
    let producer = RuntimeRenderAssetProducer::default();
    let started = std::time::Instant::now();
    for _ in 0..4 {
        let prepared = producer
            .prepare_cooked_from_package(1, &reference, &mut loader, &mut manager)
            .unwrap();
        assert_eq!(prepared.textures[0].payload.rgba8.len(), 16 * 1024 * 1024);
    }
    let elapsed = started.elapsed();
    eprintln!(
        "worker-ready-texture-cost: four_preparations_ms={:.3}",
        elapsed.as_secs_f64() * 1000.
    );
    loader.release(&root).unwrap();
    assert!(loader.shutdown_streaming(std::time::Duration::from_secs(2)));
    assert!(elapsed < std::time::Duration::from_millis(150), "already verified immutable pixels must not be rehashed on the caller on every binding preparation: {elapsed:?}");
}

#[test]
fn small_cooked_dependency_closure_reserves_its_decode_envelopes() {
    use engine_runtime::runtime_asset_loader::{RuntimeAssetLoader, StreamingBudget};
    use engine_runtime::runtime_package::RuntimeAssetRef;
    let fixture = streamed_cells::build(true);
    let mut loader = RuntimeAssetLoader::new(
        &fixture.package.package_dir,
        fixture.package.runtime_asset_index.clone(),
        fixture.package.runtime_asset_mount_table.clone(),
    );
    assert!(loader.mount_bundle("startup"));
    loader
        .set_streaming_budget(StreamingBudget {
            max_in_flight_bytes: 64 * 1024 * 1024,
            ..Default::default()
        })
        .unwrap();
    let handle = loader
        .request_async(&RuntimeAssetRef {
            id: "cell-a".into(),
            asset_type: "prefab".into(),
            guid: Some("cell-a".into()),
            sub_asset: None,
        })
        .expect("small nontexture dependencies must not each consume the full 32 MiB ceiling");
    assert!(loader.streaming_snapshot().reserved_bytes <= 64 * 1024 * 1024);
    let started = std::time::Instant::now();
    while loader.get_handle_state(&handle)
        == Some(engine_runtime::runtime_asset::RuntimeAssetLoadState::Loading)
    {
        loader.poll_async();
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
        std::thread::yield_now();
    }
    assert_eq!(
        loader.get_handle_state(&handle),
        Some(engine_runtime::runtime_asset::RuntimeAssetLoadState::Ready)
    );
    loader.release(&handle).unwrap();
    assert!(loader.shutdown_streaming(std::time::Duration::from_secs(2)));
}

#[test]
fn loading_original_builder_scene_is_accepted_by_the_same_background_loader() {
    let fixture = streamed_cells::build(false);
    let mut loader = engine_runtime::runtime_instance_loader::RuntimeInstanceLoader::from_package(
        &fixture.package,
    );
    let handle = loader
        .asset_loader_mut()
        .request_async(&engine_runtime::runtime_package::RuntimeAssetRef {
            id: fixture.package.active_scene.id.clone(),
            asset_type: "scene".into(),
            guid: None,
            sub_asset: None,
        })
        .expect("actual builder scene bytes must have a complete cooked identity");
    let started = std::time::Instant::now();
    loop {
        loader.asset_loader_mut().poll_async();
        if loader.asset_loader().get_handle_state(&handle)
            != Some(engine_runtime::runtime_asset::RuntimeAssetLoadState::Loading)
        {
            break;
        }
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
        std::thread::yield_now();
    }
    assert!(
        matches!(
            loader.asset_loader().decoded_payload(&handle).as_deref(),
            Some(engine_runtime::runtime_asset::RuntimeDecodedPayload::Scene(
                _
            ))
        ),
        "scene worker failure: {:?}",
        loader.asset_loader().streaming_error(&handle)
    );
    loader.asset_loader_mut().release(&handle).unwrap();
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(std::time::Duration::from_secs(2)));
}

#[test]
fn cell_logical_resource_receipts_cannot_release_another_managers_resources() {
    use engine_runtime::runtime_instance_loader::*;
    let fixture = streamed_cells::build(true);
    let mut loader = RuntimeInstanceLoader::from_package(&fixture.package);
    let mut other = RuntimeInstanceLoader::from_package(&fixture.package);
    let mut world = World::new();
    let mut other_world = World::new();
    let mut manager = engine_runtime::render_resource::RenderResourceManager::new();
    let mut other_manager = engine_runtime::render_resource::RenderResourceManager::new();
    let a = loader
        .request_cell(streamed_cells::request("a", 1, true), &world)
        .unwrap();
    let b = other
        .request_cell(streamed_cells::request("a", 1, true), &other_world)
        .unwrap();
    prepare_streamed_cell(&mut loader, a, &world, &mut manager);
    prepare_streamed_cell(&mut other, b, &other_world, &mut other_manager);
    let original_resident = other_manager.report(0).resident_bytes;
    assert!(matches!(
        loader.release_cell(a, &mut world, &mut other_manager, 1),
        Err(CellError::Render(_))
    ));
    assert_eq!(loader.cell_status(a).unwrap(), CellStatus::Prepared);
    assert_eq!(other_manager.report(1).resident_bytes, original_resident);
    loader.release_cell(a, &mut world, &mut manager, 1).unwrap();
    other
        .release_cell(b, &mut other_world, &mut other_manager, 1)
        .unwrap();
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(std::time::Duration::from_secs(2)));
    assert!(other
        .asset_loader_mut()
        .shutdown_streaming(std::time::Duration::from_secs(2)));
}

#[test]
fn loading_builder_derives_static_render_dependencies_without_explicit_prefab_identity() {
    let fixture = streamed_cells::build_with_identity(true, false);
    let record = fixture
        .package
        .runtime_asset_index
        .record_by_guid("cell-a")
        .unwrap();
    assert_eq!(record.dependencies, ["material-a", "shared-mesh"]);
    let mut loader = engine_runtime::runtime_instance_loader::RuntimeInstanceLoader::from_package(
        &fixture.package,
    );
    let world = World::new();
    let mut manager = engine_runtime::render_resource::RenderResourceManager::new();
    let ticket = loader
        .request_cell(streamed_cells::request("a", 1, true), &world)
        .unwrap();
    prepare_streamed_cell(&mut loader, ticket, &world, &mut manager);
    assert_eq!(
        loader.cell_status(ticket).unwrap(),
        engine_runtime::runtime_instance_loader::CellStatus::Prepared
    );
    loader
        .release_cell(ticket, &mut World::new(), &mut manager, 1)
        .unwrap_err();
    let mut world = world;
    loader
        .release_cell(ticket, &mut world, &mut manager, 1)
        .unwrap();
    assert_eq!(
        loader
            .asset_loader()
            .streaming_snapshot()
            .cpu_resident_bytes,
        0
    );
}

#[test]
fn loading_original_package_builder_emits_streamable_prefab_identity() {
    let fixture = streamed_cells::build(false);
    let mut loader = engine_runtime::runtime_instance_loader::RuntimeInstanceLoader::from_package(
        &fixture.package,
    );
    let world = World::new();
    loader
        .request_cell(
            streamed_cells::request("a", loader.asset_loader().asset_revision_epoch(), false),
            &world,
        )
        .expect(
            "the original builder must emit byte size and digest for its actual cooked prefab file",
        );
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(std::time::Duration::from_secs(2)));
}

fn prepare_streamed_cell(
    loader: &mut engine_runtime::runtime_instance_loader::RuntimeInstanceLoader,
    ticket: engine_runtime::runtime_instance_loader::CellTicket,
    world: &World,
    manager: &mut engine_runtime::render_resource::RenderResourceManager,
) {
    use engine_runtime::runtime_instance_loader::CellError;
    let start = std::time::Instant::now();
    loop {
        match loader.prepare_cell(ticket, world, 0, manager) {
            Ok(_) => return,
            Err(CellError::NotReady) => {
                assert!(start.elapsed() < std::time::Duration::from_secs(5));
                std::thread::yield_now();
            }
            Err(e) => panic!("cell prepare: {e:?}"),
        }
    }
}
#[test]
fn cell_two_cooked_placements_are_invisible_until_commit_and_soft_refs_survive_reload() {
    use engine_runtime::runtime_instance_loader::*;
    let census = |loader: &RuntimeInstanceLoader, world: &World| {
        let mut sample = engine_runtime::resource_memory::ResourceMemorySample::new(1, 1, 0);
        loader.observe_memory(&mut sample);
        world.observe_memory(&mut sample);
        let before = engine_runtime::resource_memory::memory_totals(&sample.allocations).unwrap();
        loader.observe_memory(&mut sample);
        world.observe_memory(&mut sample);
        assert_eq!(
            engine_runtime::resource_memory::memory_totals(&sample.allocations).unwrap(),
            before
        );
    };
    let fixture = streamed_cells::build(false);
    let mut loader = RuntimeInstanceLoader::from_package(&fixture.package);
    let mut world = World::new();
    let mut manager = engine_runtime::render_resource::RenderResourceManager::new();
    let a = loader
        .request_cell(streamed_cells::request("a", 1, false), &world)
        .unwrap();
    let b = loader
        .request_cell(streamed_cells::request("b", 1, false), &world)
        .unwrap();
    let soft = SoftCellEntityRef {
        placement_id: "placement-a".into(),
        source_entity_id: "node".into(),
    };
    prepare_streamed_cell(&mut loader, a, &world, &mut manager);
    prepare_streamed_cell(&mut loader, b, &world, &mut manager);
    census(&loader, &world);
    assert!(world.entity_ids().is_empty());
    assert!(loader.resolve_soft_cell_entity(&soft, &world).is_none());
    assert_eq!(loader.cell_activation_epoch(a).unwrap(), None);
    let first = loader.commit_cell(a, &mut world).unwrap();
    let second = loader.commit_cell(b, &mut world).unwrap();
    census(&loader, &world);
    assert_ne!(first.root_entity, second.root_entity);
    assert_eq!(loader.cell_activation_epoch(a).unwrap(), Some(1));
    assert_eq!(loader.cell_activation_epoch(b).unwrap(), Some(2));
    let old = loader.resolve_soft_cell_entity(&soft, &world).unwrap();
    loader.release_cell(a, &mut world, &mut manager, 1).unwrap();
    assert_eq!(world.entity_ids().len(), 1);
    assert!(loader.resolve_soft_cell_entity(&soft, &world).is_none());
    let reloaded = loader
        .request_cell(streamed_cells::request("a", 1, false), &world)
        .unwrap();
    assert_ne!(a, reloaded);
    prepare_streamed_cell(&mut loader, reloaded, &world, &mut manager);
    loader.commit_cell(reloaded, &mut world).unwrap();
    census(&loader, &world);
    assert_ne!(loader.resolve_soft_cell_entity(&soft, &world).unwrap(), old);
    assert!(loader.commit_cell(a, &mut world).is_err());
    loader
        .release_cell(reloaded, &mut world, &mut manager, 2)
        .unwrap();
    loader.release_cell(b, &mut world, &mut manager, 2).unwrap();
    census(&loader, &world);
    assert!(world.entity_ids().is_empty());
    assert_eq!(
        loader
            .asset_loader()
            .streaming_snapshot()
            .cpu_resident_bytes,
        0
    );
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(std::time::Duration::from_secs(2)));
}
#[test]
fn cell_cancel_revision_and_new_world_never_publish_prepared_entities() {
    use engine_runtime::runtime_instance_loader::*;
    let fixture = streamed_cells::build(false);
    let mut loader = RuntimeInstanceLoader::from_package(&fixture.package);
    let mut world = World::new();
    let mut manager = engine_runtime::render_resource::RenderResourceManager::new();
    let a = loader
        .request_cell(streamed_cells::request("a", 1, false), &world)
        .unwrap();
    loader.release_cell(a, &mut world, &mut manager, 0).unwrap();
    assert!(world.entity_ids().is_empty());
    let b = loader
        .request_cell(streamed_cells::request("b", 1, false), &world)
        .unwrap();
    prepare_streamed_cell(&mut loader, b, &world, &mut manager);
    assert!(matches!(
        loader.commit_cell(b, &mut World::new()),
        Err(CellError::WrongWorld)
    ));
    loader.asset_loader_mut().mount_patch_index(
        engine_runtime::runtime_asset::RuntimeAssetIndex::new(vec![], vec![]),
    );
    assert!(matches!(
        loader.commit_cell(b, &mut world),
        Err(CellError::RevisionChanged)
    ));
    assert!(world.entity_ids().is_empty());
    loader.release_cell(b, &mut world, &mut manager, 1).unwrap();
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(std::time::Duration::from_secs(2)));
    assert_eq!(
        loader
            .asset_loader()
            .streaming_snapshot()
            .cpu_resident_bytes,
        0
    );
}

#[test]
fn cell_preparation_and_activation_cannot_bypass_cpu_or_tick_budgets() {
    use engine_runtime::runtime_instance_loader::*;
    let fixture = streamed_cells::build(false);
    let mut loader = RuntimeInstanceLoader::from_package(&fixture.package);
    let mut world = World::new();
    let mut manager = engine_runtime::render_resource::RenderResourceManager::new();
    let a = loader
        .request_cell(streamed_cells::request("a", 1, false), &world)
        .unwrap();
    let start = std::time::Instant::now();
    while loader.asset_loader().streaming_snapshot().pending_tickets != 0
        || loader
            .asset_loader()
            .streaming_snapshot()
            .pending_operations
            != 0
    {
        loader.asset_loader_mut().poll_async();
        assert!(start.elapsed() < std::time::Duration::from_secs(5));
        std::thread::yield_now();
    }
    let root = fixture
        .package
        .runtime_asset_index
        .record_by_guid("cell-a")
        .unwrap();
    let cooked = fixture
        .package
        .runtime_asset_index
        .cooked_asset(&root.cooked_asset_id)
        .unwrap();
    let prepared_charge = cooked.size.unwrap() as usize * 64;
    let resident = loader
        .asset_loader()
        .streaming_snapshot()
        .cpu_resident_bytes;
    let budget = engine_runtime::runtime_asset_loader::StreamingBudget {
        max_asset_bytes: 65536,
        max_decode_bytes: 131072,
        max_in_flight_bytes: 262144,
        max_cpu_resident_bytes: resident + prepared_charge - 1,
        ..Default::default()
    };
    loader
        .asset_loader_mut()
        .set_streaming_budget(budget)
        .unwrap();
    assert_eq!(
        loader.prepare_cell(a, &world, 0, &mut manager),
        Err(CellError::Asset(
            engine_runtime::runtime_asset_loader::StreamingError::BudgetExceeded
        ))
    );
    assert_eq!(loader.cell_status(a).unwrap(), CellStatus::Failed);
    assert!(world.entity_ids().is_empty());
    assert_eq!(
        loader
            .asset_loader()
            .streaming_snapshot()
            .cpu_resident_bytes,
        0
    );
    loader
        .asset_loader_mut()
        .set_streaming_budget(Default::default())
        .unwrap();
    world
        .set_spatial_profile(engine_runtime::world::SpatialProfile {
            max_value_intents: 8,
            max_structure_intents: 1,
        })
        .unwrap();
    world.begin_spatial_tick(1);
    let a = loader
        .request_cell(streamed_cells::request("a", 1, false), &world)
        .unwrap();
    let b = loader
        .request_cell(streamed_cells::request("b", 1, false), &world)
        .unwrap();
    prepare_streamed_cell(&mut loader, a, &world, &mut manager);
    prepare_streamed_cell(&mut loader, b, &world, &mut manager);
    loader.commit_cell(a, &mut world).unwrap();
    assert!(matches!(
        loader.commit_cell(b, &mut world),
        Err(CellError::BudgetExceeded)
    ));
    assert_eq!(world.entity_ids().len(), 1);
    world.begin_spatial_tick(2);
    loader.commit_cell(b, &mut world).unwrap();
    assert_eq!(
        loader.release_cell(a, &mut world, &mut manager, 0),
        Err(CellError::BudgetExceeded)
    );
    assert_eq!(world.entity_ids().len(), 2);
    world.begin_spatial_tick(3);
    loader.release_cell(a, &mut world, &mut manager, 0).unwrap();
    world.begin_spatial_tick(4);
    loader.release_cell(b, &mut world, &mut manager, 0).unwrap();
    assert_eq!(
        loader
            .asset_loader()
            .streaming_snapshot()
            .cpu_resident_bytes,
        0
    );
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(std::time::Duration::from_secs(2)));
}

#[test]
fn cell_cross_parent_and_persistent_migration_keep_world_and_asset_lifetime() {
    use engine_runtime::runtime_instance_loader::*;
    let fixture = streamed_cells::build(false);
    let mut loader = RuntimeInstanceLoader::from_package(&fixture.package);
    let mut world = World::new();
    let mut manager = engine_runtime::render_resource::RenderResourceManager::new();
    let a = loader
        .request_cell(streamed_cells::request("a", 1, false), &world)
        .unwrap();
    let b = loader
        .request_cell(streamed_cells::request("b", 1, false), &world)
        .unwrap();
    prepare_streamed_cell(&mut loader, a, &world, &mut manager);
    prepare_streamed_cell(&mut loader, b, &world, &mut manager);
    let ai = loader.commit_cell(a, &mut world).unwrap();
    let bi = loader.commit_cell(b, &mut world).unwrap();
    let aid = ai.source_to_world_entity[&"node".into()].clone();
    let bid = bi.source_to_world_entity[&"node".into()].clone();
    let before = world.spatial_world(&aid).unwrap().world_matrix;
    let parent_generation = world.entity_generation(&bid).unwrap();
    commit(
        &mut world,
        vec![(
            aid.as_str(),
            Intent::Reparent {
                parent_entity_id: Some(bid.as_str().into()),
                parent_generation,
                preserve_world: true,
            },
        )],
    );
    assert_eq!(
        loader.release_cell(b, &mut world, &mut manager, 0),
        Err(CellError::ForeignChildren)
    );
    assert_eq!(world.entity_ids().len(), 2);
    let generation = world.entity_generation(&aid).unwrap();
    let persistent = loader
        .migrate_cell_subtree(
            a,
            CellMigrationRequest {
                source_entity_id: "node".into(),
                source_generation: generation,
                persistent_placement_id: "persistent".into(),
                parent: None,
                preserve_world: true,
            },
            &mut world,
            &mut manager,
        )
        .unwrap();
    let after = world.spatial_world(&aid).unwrap().world_matrix;
    for (a, b) in before.into_iter().zip(after) {
        assert!((a - b).abs() < 1e-5);
    }
    assert_eq!(world.entity_generation(&aid), Some(generation));
    assert!(loader
        .despawn_prefab_instance(persistent.instance_id(), &mut world)
        .diagnostics
        .iter()
        .any(|d| d.kind == "cell.release_owner_required"));
    loader.release_cell(a, &mut world, &mut manager, 1).unwrap();
    loader.release_cell(b, &mut world, &mut manager, 1).unwrap();
    assert_eq!(world.entity_ids().len(), 1);
    assert!(loader
        .resolve_soft_cell_entity(
            &SoftCellEntityRef {
                placement_id: "placement-a".into(),
                source_entity_id: "node".into()
            },
            &world
        )
        .is_none());
    assert_eq!(
        loader.resolve_soft_cell_entity(
            &SoftCellEntityRef {
                placement_id: "persistent".into(),
                source_entity_id: "node".into()
            },
            &world
        ),
        ai.root_entity
    );
    assert!(
        loader
            .asset_loader()
            .streaming_snapshot()
            .cpu_resident_bytes
            > 0
    );
    loader
        .release_cell(persistent, &mut world, &mut manager, 2)
        .unwrap();
    assert!(world.entity_ids().is_empty());
    assert_eq!(
        loader
            .asset_loader()
            .streaming_snapshot()
            .cpu_resident_bytes,
        0
    );
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(std::time::Duration::from_secs(2)));
}
#[test]
fn cell_missing_simulation_owners_and_incomplete_strong_closure_are_rejected() {
    use engine_runtime::runtime_instance_loader::*;
    let mut fixture = streamed_cells::build(true);
    let mut root = fixture
        .package
        .runtime_asset_index
        .record_by_guid("cell-a")
        .unwrap()
        .clone();
    root.dependencies.clear();
    fixture.package.runtime_asset_index.merge_patch(
        engine_runtime::runtime_asset::RuntimeAssetIndex::new(vec![root], vec![]),
    );
    let mut loader = RuntimeInstanceLoader::from_package(&fixture.package);
    let world = World::new();
    let mut manager = engine_runtime::render_resource::RenderResourceManager::new();
    // Simulation ACK needs the actual module owners; request itself is now supported.
    for domain in [CellDomain::Physics, CellDomain::Navigation] {
        let valid = streamed_cells::build(true);
        let mut isolated = RuntimeInstanceLoader::from_package(&valid.package);
        let mut request = streamed_cells::request("a", 1, true);
        request.required_domains.insert(domain);
        let ticket = isolated.request_cell(request, &world).unwrap();
        let start = std::time::Instant::now();
        loop {
            match isolated.prepare_cell(ticket, &world, 0, &mut manager) {
                Err(CellError::NotReady) => {
                    assert!(start.elapsed() < std::time::Duration::from_secs(5));
                    std::thread::yield_now();
                }
                Err(CellError::UnsupportedDomain(actual)) => {
                    assert_eq!(actual, domain);
                    break;
                }
                other => panic!("unexpected {other:?}"),
            }
        }
        assert!(isolated
            .asset_loader_mut()
            .shutdown_streaming(std::time::Duration::from_secs(2)));
    }
    let a = loader
        .request_cell(streamed_cells::request("a", 1, true), &world)
        .unwrap();
    let start = std::time::Instant::now();
    loop {
        match loader.prepare_cell(a, &world, 0, &mut manager) {
            Err(CellError::NotReady) => {
                assert!(start.elapsed() < std::time::Duration::from_secs(5));
                std::thread::yield_now();
            }
            Err(CellError::MissingDependency(_)) => break,
            other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!(loader.cell_status(a).unwrap(), CellStatus::Failed);
    assert!(world.entity_ids().is_empty());
    assert_eq!(manager.report(0).resident_bytes, 0);
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(std::time::Duration::from_secs(2)));
}

#[test]
fn foundation_f09_strong_cell_cycles_edges_and_handle_capacity_refuse_before_dispatch() {
    use engine_runtime::runtime_asset_loader::StreamingError;
    use engine_runtime::runtime_instance_loader::*;
    for fault in ["cycle", "edges", "handles"] {
        let mut fixture = streamed_cells::build(true);
        if fault == "cycle" {
            let mut mesh = fixture
                .package
                .runtime_asset_index
                .record_by_guid("shared-mesh")
                .unwrap()
                .clone();
            mesh.dependencies.push("cell-a".into());
            fixture.package.runtime_asset_index.merge_patch(
                engine_runtime::runtime_asset::RuntimeAssetIndex::new(vec![mesh], vec![]),
            );
        }
        let mut loader = RuntimeInstanceLoader::from_package(&fixture.package);
        let mut budget = loader.asset_loader().streaming_budget();
        if fault == "edges" {
            budget.max_dependency_edges = 1;
        }
        if fault == "handles" {
            budget.max_handles = 1;
        }
        loader
            .asset_loader_mut()
            .set_streaming_budget(budget)
            .unwrap();
        let world = World::new();
        assert_eq!(
            loader.request_cell(streamed_cells::request("a", 1, true), &world),
            Err(CellError::Asset(if fault == "cycle" {
                StreamingError::DependencyCycle
            } else {
                StreamingError::BudgetExceeded
            })),
            "{fault}"
        );
        let after = loader.asset_loader().streaming_snapshot();
        assert_eq!(
            (
                after.dispatched_operations,
                after.pending_operations,
                after.pending_tickets,
                after.reserved_bytes
            ),
            (0, 0, 0, 0),
            "whole closure refusal must not dispatch a prefix for {fault}"
        );
        assert!(world.entity_ids().is_empty());
        assert_eq!(loader.cell_lifetime_counts(), (0, 0));
        assert!(loader
            .asset_loader_mut()
            .shutdown_streaming(std::time::Duration::from_secs(2)));
    }
}
#[test]
fn loading_cooked_texture_pixel_corruption_preserves_failure_and_releases_closure() {
    use engine_runtime::runtime_instance_loader::*;
    let fixture = streamed_cells::build(true);
    let record = fixture
        .package
        .runtime_asset_index
        .record_by_guid("shared-texture")
        .unwrap();
    let cooked = fixture
        .package
        .runtime_asset_index
        .cooked_asset(&record.cooked_asset_id)
        .unwrap();
    let meta: engine_runtime::runtime_package::CookedTextureAsset = serde_json::from_slice(
        &std::fs::read(
            fixture
                .package
                .package_dir
                .join(cooked.path.as_ref().unwrap()),
        )
        .unwrap(),
    )
    .unwrap();
    let path = fixture.package.package_dir.join(meta.pixel_data_path);
    let mut pixels = std::fs::read(&path).unwrap();
    pixels[0] ^= 1;
    std::fs::write(path, pixels).unwrap();
    let mut loader = RuntimeInstanceLoader::from_package(&fixture.package);
    let world = World::new();
    let mut manager = engine_runtime::render_resource::RenderResourceManager::new();
    let a = loader
        .request_cell(streamed_cells::request("a", 1, true), &world)
        .unwrap();
    let start = std::time::Instant::now();
    loop {
        match loader.prepare_cell(a, &world, 0, &mut manager) {
            Err(CellError::NotReady) => {
                assert!(start.elapsed() < std::time::Duration::from_secs(5));
                std::thread::yield_now();
            }
            Err(CellError::Asset(
                engine_runtime::runtime_asset_loader::StreamingError::IntegrityMismatch,
            )) => break,
            other => panic!("unexpected {other:?}"),
        }
    }
    assert!(world.entity_ids().is_empty());
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(std::time::Duration::from_secs(2)));
    assert_eq!(
        loader
            .asset_loader()
            .streaming_snapshot()
            .cpu_resident_bytes,
        0
    );
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires a real GPU"]
fn cell_two_cooked_cells_use_real_segmented_gpu_ack_and_shared_retirement() {
    // Separate the deliberately sub-record transfer recipe from the actual
    // draw recipe. Both execute the same sharing/migration/retirement path;
    // the 8-byte case must still reject a real view/instance record.
    for bytes_per_frame in [8, 4096] {
        cell_two_cooked_cells_gpu_case(bytes_per_frame);
    }
}

#[cfg(feature = "real-wgpu")]
fn cell_two_cooked_cells_gpu_case(bytes_per_frame: usize) {
    use engine_runtime::runtime_instance_loader::*;
    use engine_runtime::wgpu_backend::real::{GeometryUploadBudget, RealWgpuBackend};
    let fixture = streamed_cells::build(true);
    let mut loader = RuntimeInstanceLoader::from_package(&fixture.package);
    let mut world = World::new();
    let mut manager = engine_runtime::render_resource::RenderResourceManager::new();
    let a = loader
        .request_cell(streamed_cells::request("a", 1, true), &world)
        .unwrap();
    let b = loader
        .request_cell(streamed_cells::request("b", 1, true), &world)
        .unwrap();
    prepare_streamed_cell(&mut loader, a, &world, &mut manager);
    prepare_streamed_cell(&mut loader, b, &world, &mut manager);
    assert!(world.entity_ids().is_empty());
    assert_eq!(
        loader
            .asset_loader()
            .streaming_snapshot()
            .dispatched_operations,
        6
    );
    // CPU preparation must have consumed cached worker output. Delete the package
    // payloads before the renderer's direct consumer below; any fallback I/O fails.
    let mut removed_payloads = Vec::new();
    for id in [
        "cell-a",
        "cell-b",
        "shared-mesh",
        "material-a",
        "material-b",
        "shared-texture",
    ] {
        let record = fixture
            .package
            .runtime_asset_index
            .record_by_guid(id)
            .unwrap();
        let cooked = fixture
            .package
            .runtime_asset_index
            .cooked_asset(&record.cooked_asset_id)
            .unwrap();
        let path = fixture
            .package
            .package_dir
            .join(cooked.path.as_ref().unwrap());
        removed_payloads.push((path.clone(), std::fs::read(&path).unwrap()));
        std::fs::remove_file(path).unwrap();
    }
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    backend
        .set_geometry_upload_budget(GeometryUploadBudget {
            resident_bytes: 1024 * 1024,
            staging_bytes: if bytes_per_frame == 8 { 32 } else { 4096 },
            bytes_per_frame,
            max_tickets: 4,
        })
        .unwrap();
    loader.begin_cell_upload(a, &world, &mut backend).unwrap();
    loader.begin_cell_upload(b, &world, &mut backend).unwrap();
    assert!(matches!(
        loader.commit_cell_gpu(a, &mut world, &mut backend),
        Err(CellError::NotReady)
    ));
    assert!(world.entity_ids().is_empty());
    for frame in 1..128 {
        let bytes = backend.pump_geometry_uploads(frame).unwrap();
        assert!(bytes <= bytes_per_frame);
        assert!(
            backend.geometry_upload_snapshot().staging_bytes
                <= if bytes_per_frame == 8 { 32 } else { 4096 }
        );
        if bytes != 0 {
            backend
                .wait_gpu_submission(
                    backend.last_gpu_submission().unwrap(),
                    std::time::Duration::from_secs(5),
                )
                .unwrap();
        }
        match loader.commit_cell_gpu(a, &mut world, &mut backend) {
            Ok(_) => break,
            Err(CellError::NotReady) => assert!(world.entity_ids().is_empty()),
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(loader.cell_status(a).unwrap(), CellStatus::Active);
    loader.commit_cell_gpu(b, &mut world, &mut backend).unwrap();
    assert_eq!(loader.cell_lifetime_counts(), (2, 2));
    assert_eq!(world.entity_ids().len(), 2);
    assert_eq!(backend.geometry_upload_count(), 1);
    assert_eq!(backend.geometry_residency().textures, 1);
    assert_eq!(backend.geometry_residency().texture_bytes, 64);
    let soft = SoftCellEntityRef {
        placement_id: "placement-a".into(),
        source_entity_id: "node".into(),
    };
    assert!(loader.resolve_soft_cell_entity(&soft, &world).is_some());
    let entity = loader
        .prefab_instance(a.instance_id())
        .unwrap()
        .source_to_world_entity[&"node".into()]
        .clone();
    let renderer = world.mesh_renderer3d(&entity).unwrap().clone();
    // A new preparation is a new integrity check, not a read of the old upload
    // lease. Stage46 deliberately rejects a cold texture getter with no receipt.
    assert!(
        engine_runtime::render_asset_production::RuntimeRenderAssetProducer
            .prepare_mesh_renderer(128, &renderer, loader.asset_loader_mut(), &mut manager)
            .is_err()
    );
    for (path, bytes) in removed_payloads {
        std::fs::write(path, bytes).unwrap();
    }
    let prepared = engine_runtime::render_asset_production::RuntimeRenderAssetProducer
        .prepare_mesh_renderer(128, &renderer, loader.asset_loader_mut(), &mut manager)
        .unwrap();
    let view = engine_runtime::render_contract::RenderView::perspective(1, 64, 64, 60., 0.1, 10.);
    let frame = engine_runtime::render_geometry::GeometryFrame::build(
        "main",
        [64, 64],
        &[engine_runtime::render_geometry::GeometryViewInput {
            view: &view,
            pose: Default::default(),
        }],
        &[geometry_fixture::instance(
            &prepared.geometry[0],
            1,
            [0., 0., -2.],
        )],
        &manager,
        engine_runtime::render_contract::DiagnosticLevel::Summary,
    )
    .unwrap();
    let plan = engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(
        &geometry_fixture::graph(&frame),
    );
    let pixels = backend.render_plan_to_rgba_bytes(&plan, 64, 64);
    if bytes_per_frame == 8 {
        assert_eq!(pixels.unwrap_err(), "geometry.visual_upload_capacity");
        assert!(backend.geometry_upload_snapshot().frame_uploaded_bytes <= 8);
    } else {
        assert!(
            pixels
                .unwrap()
                .chunks_exact(4)
                .any(|p| p[0] > 200 && p[1] < 20),
            "actual prepared texture/mesh/material must reach real pixels"
        );
    }
    let generation = world.entity_generation(&entity).unwrap();
    let persistent = loader
        .migrate_cell_subtree_gpu(
            a,
            CellMigrationRequest {
                source_entity_id: "node".into(),
                source_generation: generation,
                persistent_placement_id: "persistent-gpu".into(),
                parent: None,
                preserve_world: true,
            },
            &mut world,
            &mut manager,
            &mut backend,
        )
        .unwrap();
    assert_eq!(world.entity_generation(&entity), Some(generation));
    loader
        .release_cell_gpu(a, &mut world, &mut manager, 129, &mut backend)
        .unwrap();
    assert_eq!(world.entity_ids().len(), 2);
    assert_eq!(backend.geometry_residency().meshes, 1);
    assert_eq!(backend.geometry_residency().textures, 1);
    loader
        .release_cell_gpu(b, &mut world, &mut manager, 130, &mut backend)
        .unwrap();
    assert_eq!(world.entity_ids().len(), 1);
    assert_eq!(backend.geometry_residency().meshes, 1);
    assert_eq!(backend.geometry_residency().textures, 1);
    loader
        .release_cell_gpu(persistent, &mut world, &mut manager, 131, &mut backend)
        .unwrap();
    assert!(world.entity_ids().is_empty());
    assert_eq!(backend.geometry_residency().meshes, 0);
    assert_eq!(backend.geometry_residency().textures, 0);
    drop(frame);
    assert_eq!(backend.geometry_residency().texture_bytes, 0);
    assert_eq!(loader.cell_lifetime_counts(), (0, 0));
    drop(prepared);
    backend
        .wait_gpu_submission(
            backend.last_gpu_submission().unwrap(),
            std::time::Duration::from_secs(5),
        )
        .unwrap();
    assert_eq!(backend.poll_gpu_completion().unwrap().retiring_bytes, 0);
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(std::time::Duration::from_secs(2)));
    assert_eq!(
        loader
            .asset_loader()
            .streaming_snapshot()
            .cpu_resident_bytes,
        0
    );
}

fn world(count: usize) -> World {
    let RuntimeSceneDocument::Spatial(mut scene) =
        decode_runtime_scene(include_bytes!("fixtures/three_d_a0/runtime-scene-v2.json")).unwrap()
    else {
        panic!()
    };
    let template = scene.entities[0].clone();
    let camera = scene.entities[1].clone();
    scene.entities = (0..count)
        .map(|i| {
            let mut e = template.clone();
            if let engine_runtime::spatial_contract::RuntimeEntityDocument::Spatial(ref mut value) =
                e
            {
                value.id = format!("node-{i}");
                value.parent_id = None;
            }
            e
        })
        .collect();
    scene.entities.push(camera);
    let loaded = load_scene_into_world(&scene.into());
    assert!(loaded.diagnostics.is_ok(), "{:?}", loaded.diagnostics);
    let mut w = loaded.value.unwrap();
    w.take_dirty_records();
    w
}
fn write(buffer: &mut ProjectRuntimeMutationBuffer, world: &World, id: &str, x: f32) {
    buffer.spatial_command(
        id.into(),
        world.entity_generation(&id.into()).unwrap(),
        Intent::SetLocalTransform {
            transform: ProjectRuntimeTransform3D {
                translation: [x, 0., 0.],
                ..ProjectRuntimeTransform3D::identity()
            },
        },
    );
}
#[test]
fn spatial_noop_batch_produces_no_dirty_work() {
    let mut world = world(16);
    let mut batch = ProjectRuntimeMutationBuffer::new();
    for i in 0..16 {
        write(&mut batch, &world, &format!("node-{i}"), 0.);
    }
    batch.prepare(&world).unwrap().commit(&mut world).unwrap();
    assert!(
        world.take_dirty_records().is_empty(),
        "unchanged values must not invalidate downstream consumers"
    );
}

fn commit(world: &mut World, intents: Vec<(&str, Intent)>) {
    let mut batch = ProjectRuntimeMutationBuffer::new();
    for (id, intent) in intents {
        batch.spatial_command(
            id.into(),
            world.entity_generation(&id.into()).unwrap(),
            intent,
        );
    }
    batch.prepare(world).unwrap().commit(world).unwrap();
}
fn transform(x: f32) -> ProjectRuntimeTransform3D {
    ProjectRuntimeTransform3D {
        translation: [x, 0., 0.],
        ..ProjectRuntimeTransform3D::identity()
    }
}
fn move_to(x: f32) -> Intent {
    Intent::SetLocalTransform {
        transform: transform(x),
    }
}
fn near(a: f32, b: f32) {
    assert!((a - b).abs() < 1e-4, "{a} != {b}");
}
#[test]
fn spatial_sparse_full_and_final_revert_have_exact_field_masks() {
    let mut w = world(128);
    w.begin_spatial_tick(1);
    commit(
        &mut w,
        vec![("node-0", move_to(1.)), ("node-0", move_to(0.))],
    );
    assert!(w.take_dirty_records().is_empty());
    commit(&mut w, vec![("node-2", move_to(2.))]);
    let d = w.finish_spatial_tick();
    assert_eq!(d.changes.len(), 1);
    assert_eq!(
        d.changes[0].field_mask,
        engine_runtime::world::SpatialFieldMask::TRANSLATION
    );
    let another_consumer = w.spatial_delta();
    assert!(std::sync::Arc::ptr_eq(&d, &another_consumer));
    w.take_dirty_records();
    w.begin_spatial_tick(2);
    let mut b = ProjectRuntimeMutationBuffer::new();
    for i in 0..128 {
        write(&mut b, &w, &format!("node-{i}"), 5.);
    }
    b.prepare(&w).unwrap().commit(&mut w).unwrap();
    let d = w.finish_spatial_tick();
    assert_eq!(d.changes.len(), 128);
    assert_eq!(d.affected_count, 128);
}
#[test]
fn spatial_parent_child_dirty_roots_are_merged_and_snapshots_are_immutable() {
    let mut w = world(3);
    w.try_set_parent("node-1".into(), Some("node-0".into()))
        .unwrap();
    w.take_dirty_records();
    let old = w.committed_spatial_pose(&"node-1".into()).unwrap();
    w.begin_spatial_tick(1);
    commit(
        &mut w,
        vec![("node-0", move_to(2.)), ("node-1", move_to(3.))],
    );
    let d = w.finish_spatial_tick();
    assert_eq!(d.dirty_roots, 1);
    assert_eq!(d.affected_count, 2);
    assert_eq!(d.changes.len(), 2);
    near(old.state.world_matrix[12], 0.);
    near(
        w.spatial_world(&"node-1".into()).unwrap().world_matrix[12],
        5.,
    );
    assert!(!std::sync::Arc::ptr_eq(
        &old,
        &w.committed_spatial_pose(&"node-1".into()).unwrap()
    ));
}
#[test]
fn spatial_zero_new_tick_presentation_advances_without_simulation_writes() {
    let mut w = world(2);
    w.begin_spatial_tick(1);
    commit(&mut w, vec![("node-0", move_to(8.))]);
    w.finish_spatial_tick();
    let snapshot = w.committed_spatial_pose(&"node-0".into()).unwrap();
    let sim = w.transform3d(&"node-0".into()).unwrap().clone();
    let first = w.prepare_spatial_presentation(1, 0.25).unwrap();
    near(
        w.spatial_presentation(&"node-0".into())
            .unwrap()
            .world_matrix[12],
        2.,
    );
    let second = w.prepare_spatial_presentation(2, 0.75).unwrap();
    near(
        w.spatial_presentation(&"node-0".into())
            .unwrap()
            .world_matrix[12],
        6.,
    );
    assert_eq!(first.tick_pair, second.tick_pair);
    assert_eq!(second.changed.len(), 1);
    assert_eq!(w.transform3d(&"node-0".into()), Some(&sim));
    assert!(std::sync::Arc::ptr_eq(
        &snapshot,
        &w.committed_spatial_pose(&"node-0".into()).unwrap()
    ));
    assert!(w
        .prepare_spatial_presentation(3, 0.75)
        .unwrap()
        .changed
        .is_empty());
    w.begin_spatial_tick(2);
    w.finish_spatial_tick();
    w.prepare_spatial_presentation(4, 0.).unwrap();
    near(
        w.spatial_presentation(&"node-0".into())
            .unwrap()
            .world_matrix[12],
        8.,
    );
    assert!(w
        .prepare_spatial_presentation(5, 0.5)
        .unwrap()
        .changed
        .is_empty());
}
#[test]
fn spatial_projection_consumes_presentation_not_world_position() {
    use engine_runtime::{
        render_command::apply_batch,
        render_extract::RenderExtractContext,
        render_state::{RenderProxyPayload, RenderSceneState},
    };
    let mut w = world(1);
    w.begin_spatial_tick(1);
    commit(&mut w, vec![("node-0", move_to(4.))]);
    w.finish_spatial_tick();
    w.prepare_spatial_presentation(1, 0.5).unwrap();
    let mut extract = RenderExtractContext::new();
    let mut scene = RenderSceneState::new();
    let mut queue = extract.extract_world_dirty(1, &mut w, &scene);
    let merged = queue.normalize_merge(&scene);
    assert!(apply_batch(&mut scene, &merged).is_empty());
    let proxy = scene
        .proxy(scene.proxy_for_source(&"node-0".into()).unwrap())
        .unwrap();
    let RenderProxyPayload::Mesh3D(p) = &proxy.payload else {
        panic!()
    };
    near(p.world.world_matrix[12], 2.);
    near(
        w.spatial_world(&"node-0".into()).unwrap().world_matrix[12],
        4.,
    );
}
#[test]
fn spatial_rotation_uses_quaternion_history_and_hierarchy_composition() {
    let mut w = world(2);
    w.try_set_parent("node-1".into(), Some("node-0".into()))
        .unwrap();
    commit(&mut w, vec![("node-1", move_to(1.))]);
    w.take_dirty_records();
    w.begin_spatial_tick(1);
    let mut t = transform(0.);
    t.rotation_xyzw = [0., 0., 1., 0.];
    commit(
        &mut w,
        vec![("node-0", Intent::SetLocalTransform { transform: t })],
    );
    w.finish_spatial_tick();
    w.prepare_spatial_presentation(1, 0.5).unwrap();
    let child = w.spatial_presentation(&"node-1".into()).unwrap();
    near(child.world_matrix[12], 0.);
    near(child.world_matrix[13], 1.);
    near(
        w.spatial_world(&"node-1".into()).unwrap().world_matrix[12],
        -1.,
    );
}
#[test]
fn spatial_teleport_and_mode_switch_reset_history() {
    let mut w = world(1);
    w.begin_spatial_tick(1);
    commit(&mut w, vec![("node-0", move_to(2.))]);
    w.finish_spatial_tick();
    commit(
        &mut w,
        vec![(
            "node-0",
            Intent::Teleport {
                transform: transform(100.),
            },
        )],
    );
    w.prepare_spatial_presentation(1, 0.1).unwrap();
    near(
        w.spatial_presentation(&"node-0".into())
            .unwrap()
            .world_matrix[12],
        100.,
    );
    w.begin_spatial_tick(2);
    commit(
        &mut w,
        vec![
            ("node-0", Intent::SetInterpolation { enabled: false }),
            ("node-0", move_to(104.)),
        ],
    );
    w.finish_spatial_tick();
    w.prepare_spatial_presentation(2, 0.).unwrap();
    near(
        w.spatial_presentation(&"node-0".into())
            .unwrap()
            .world_matrix[12],
        104.,
    );
    commit(
        &mut w,
        vec![("node-0", Intent::SetInterpolation { enabled: true })],
    );
    w.begin_spatial_tick(3);
    commit(&mut w, vec![("node-0", move_to(108.))]);
    w.finish_spatial_tick();
    w.prepare_spatial_presentation(3, 0.5).unwrap();
    near(
        w.spatial_presentation(&"node-0".into())
            .unwrap()
            .world_matrix[12],
        106.,
    );
}
#[test]
fn spatial_batch_budget_cannot_be_bypassed_by_splitting_and_resets_per_tick() {
    use engine_runtime::world::SpatialProfile;
    let mut w = world(2);
    w.set_spatial_profile(SpatialProfile {
        max_value_intents: 2,
        max_structure_intents: 1,
    })
    .unwrap();
    w.begin_spatial_tick(1);
    commit(&mut w, vec![("node-0", move_to(1.))]);
    commit(&mut w, vec![("node-1", move_to(2.))]);
    let mut b = ProjectRuntimeMutationBuffer::new();
    write(&mut b, &w, "node-0", 8.);
    let failure = b.prepare(&w).unwrap_err();
    assert!(failure.message.contains("budget"));
    near(w.transform3d(&"node-0".into()).unwrap().translation[0], 1.);
    w.begin_spatial_tick(2);
    commit(&mut w, vec![("node-0", move_to(8.))]);
}
#[test]
fn spatial_invalid_tail_and_final_hierarchy_cycle_leave_world_unchanged() {
    let w = world(2);
    let mut b = ProjectRuntimeMutationBuffer::new();
    write(&mut b, &w, "node-0", 3.);
    b.spatial_command(
        "node-1".into(),
        w.entity_generation(&"node-1".into()).unwrap(),
        Intent::SetLocalTransform {
            transform: ProjectRuntimeTransform3D {
                scale: [0.; 3],
                ..transform(0.)
            },
        },
    );
    assert!(b.prepare(&w).is_err());
    near(w.transform3d(&"node-0".into()).unwrap().translation[0], 0.);
    let mut b = ProjectRuntimeMutationBuffer::new();
    for (id, parent) in [("node-0", "node-1"), ("node-1", "node-0")] {
        b.spatial_command(
            id.into(),
            w.entity_generation(&id.into()).unwrap(),
            Intent::Reparent {
                parent_entity_id: Some(parent.into()),
                parent_generation: w.entity_generation(&parent.into()).unwrap(),
                preserve_world: true,
            },
        );
    }
    assert!(b.prepare(&w).is_err());
    assert!(w.hierarchy(&"node-0".into()).unwrap().parent_id.is_none());
}
#[test]
fn spatial_anchor_preserve_world_keeps_root_linear_values_exact() {
    for base in [0., 10_000., 1_000_000.] {
        let mut w = world(1);
        let q = [0.217f32, -0.331, 0.129, 0.899];
        let norm = q.iter().map(|v| v * v).sum::<f32>().sqrt();
        let q = q.map(|v| v / norm);
        let mut intents = vec![];
        for (id, scale) in [("camera", [1.; 3]), ("node-0", [0.125, 2., 3.])] {
            intents.push((
                id,
                Intent::SetLocalTransform {
                    transform: ProjectRuntimeTransform3D {
                        translation: [5.25, 2., 3.],
                        rotation_xyzw: q,
                        scale,
                    },
                },
            ));
            intents.push((
                id,
                Intent::SetWorldAnchor {
                    anchor: [base, 0., 0.],
                    preserve_world: false,
                },
            ));
        }
        commit(&mut w, intents);
        let initial: Vec<_> = ["camera", "node-0"]
            .iter()
            .map(|id| w.transform3d(&(*id).into()).unwrap().clone())
            .collect();
        for step in 1..=20 {
            w.begin_spatial_tick(step);
            commit(
                &mut w,
                ["camera", "node-0"]
                    .into_iter()
                    .map(|id| {
                        (
                            id,
                            Intent::SetWorldAnchor {
                                anchor: [base + step as f64 * 16., 0., 0.],
                                preserve_world: true,
                            },
                        )
                    })
                    .collect(),
            );
            for (i, id) in ["camera", "node-0"].into_iter().enumerate() {
                let t = w.transform3d(&id.into()).unwrap();
                assert_eq!(t.rotation_xyzw, initial[i].rotation_xyzw);
                assert_eq!(t.scale, initial[i].scale);
                near(t.translation[0] + step as f32 * 16., 5.25);
            }
        }
        let mut bad = ProjectRuntimeMutationBuffer::new();
        bad.spatial_command(
            "camera".into(),
            w.entity_generation(&"camera".into()).unwrap(),
            Intent::SetLocalTransform {
                transform: ProjectRuntimeTransform3D {
                    scale: [1.000001, 1., 1.],
                    ..ProjectRuntimeTransform3D::identity()
                },
            },
        );
        assert!(bad
            .prepare(&w)
            .unwrap_err()
            .message
            .contains("camera_scale"));
    }
}

#[test]
fn spatial_cross_anchor_reparent_preserves_global_pose_and_identity() {
    let mut w = world(2);
    let generation = w.entity_generation(&"node-1".into());
    commit(
        &mut w,
        vec![
            (
                "node-0",
                Intent::SetWorldAnchor {
                    anchor: [1e12 - 16., 0., 0.],
                    preserve_world: false,
                },
            ),
            (
                "node-1",
                Intent::SetWorldAnchor {
                    anchor: [1e12, 0., 0.],
                    preserve_world: false,
                },
            ),
            ("node-1", move_to(0.125)),
        ],
    );
    w.take_dirty_records();
    let parent_generation = w.entity_generation(&"node-0".into()).unwrap();
    commit(
        &mut w,
        vec![(
            "node-1",
            Intent::Reparent {
                parent_entity_id: Some("node-0".into()),
                parent_generation,
                preserve_world: true,
            },
        )],
    );
    w.finish_spatial_tick();
    let state = w.spatial_world(&"node-1".into()).unwrap();
    assert_eq!(state.anchor, [1e12 - 16., 0., 0.]);
    near(state.world_matrix[12], 16.125);
    assert_eq!(w.entity_generation(&"node-1".into()), generation);
    commit(
        &mut w,
        vec![(
            "node-1",
            Intent::Reparent {
                parent_entity_id: None,
                parent_generation: 0,
                preserve_world: true,
            },
        )],
    );
    w.finish_spatial_tick();
    let state = w.spatial_world(&"node-1".into()).unwrap();
    assert_eq!(state.anchor, [1e12, 0., 0.]);
    near(state.world_matrix[12], 0.125);
}
#[test]
fn spatial_anchor_rebase_resets_history_and_preserves_small_displacement() {
    let mut w = world(1);
    commit(
        &mut w,
        vec![
            ("node-0", move_to(0.125)),
            (
                "node-0",
                Intent::SetWorldAnchor {
                    anchor: [1e12 - 16., 0., 0.],
                    preserve_world: false,
                },
            ),
            (
                "camera",
                Intent::SetWorldAnchor {
                    anchor: [1e12 - 16., 0., 0.],
                    preserve_world: false,
                },
            ),
        ],
    );
    w.take_dirty_records();
    w.begin_spatial_tick(1);
    commit(
        &mut w,
        vec![(
            "node-0",
            Intent::SetWorldAnchor {
                anchor: [1e12, 0., 0.],
                preserve_world: true,
            },
        )],
    );
    w.finish_spatial_tick();
    w.prepare_spatial_presentation(1, 0.1).unwrap();
    near(
        w.spatial_presentation(&"node-0".into())
            .unwrap()
            .world_matrix[12],
        0.125,
    );
}
#[test]
fn spatial_unrepresentable_shear_reparent_rejects_entire_batch() {
    let mut w = world(2);
    let mut t = transform(0.);
    t.scale = [2., 1., 1.];
    t.rotation_xyzw = [0., 0., 0.38268343, 0.9238795];
    commit(
        &mut w,
        vec![("node-0", Intent::SetLocalTransform { transform: t })],
    );
    w.take_dirty_records();
    let mut b = ProjectRuntimeMutationBuffer::new();
    write(&mut b, &w, "node-1", 4.);
    b.spatial_command(
        "node-1".into(),
        w.entity_generation(&"node-1".into()).unwrap(),
        Intent::Reparent {
            parent_entity_id: Some("node-0".into()),
            parent_generation: w.entity_generation(&"node-0".into()).unwrap(),
            preserve_world: true,
        },
    );
    let error = b.prepare(&w).unwrap_err();
    assert!(error.message.contains("shear"), "{error:?}");
    near(w.transform3d(&"node-1".into()).unwrap().translation[0], 0.);
    assert!(w.hierarchy(&"node-1".into()).unwrap().parent_id.is_none());
    assert!(w.dirty_records().is_empty());
}
#[test]
fn spatial_clock_30_60_120hz_pause_clamp_and_overload_preserve_debt() {
    use engine_runtime::runtime_time::SpatialFixedClock;
    for hz in [30, 60, 120] {
        let mut c = SpatialFixedClock::default();
        let mut total = 0;
        let mut zeros = 0;
        for _ in 0..hz {
            let p = c.plan(1. / hz as f32, 1., 0.33333334).unwrap();
            total += p.ticks;
            if p.ticks == 0 {
                zeros += 1;
            }
        }
        assert_eq!(total, 60);
        if hz == 120 {
            assert_eq!(zeros, 60);
        }
    }
    let mut c = SpatialFixedClock::default();
    let p = c.plan(1. / 120., 1., 1.).unwrap();
    let pause = c.plan(1., 0., 1.).unwrap();
    assert_eq!(pause.ticks, 0);
    near(p.alpha, pause.alpha);
    let p = c.plan(1., 1., 0.33333334).unwrap();
    assert_eq!(p.ticks, 4);
    assert!(p.debt_ticks > 15.);
    assert!(p.clamped_seconds > 0.6);
    let mut c = SpatialFixedClock::default();
    let mut before = loop {
        let before = c.clone();
        if c.plan(0.33333334, 1., 0.33333334).is_err() {
            break before;
        }
    };
    assert_eq!(
        c.plan(0., 1., 0.33333334).unwrap(),
        before.plan(0., 1., 0.33333334).unwrap()
    );
}

#[test]
fn spatial_tick_delta_keeps_separate_commits_and_removals_for_all_consumers() {
    let mut w = world(3);
    w.begin_spatial_tick(1);
    commit(&mut w, vec![("node-0", move_to(2.))]);
    let first = w.finish_spatial_tick();
    commit(&mut w, vec![("node-1", move_to(3.))]);
    w.finish_spatial_tick();
    w.try_despawn_entity(&"node-2".into()).unwrap();
    let final_delta = w.finish_spatial_tick();
    assert_eq!(final_delta.changes.len(), 3);
    assert!(final_delta
        .changes
        .iter()
        .any(|c| c.entity_id.as_str() == "node-2" && c.pose.is_none()));
    assert_eq!(first.changes.len(), 1);
    assert_eq!(first.changes[0].entity_id.as_str(), "node-0");
    assert!(std::sync::Arc::ptr_eq(&final_delta, &w.spatial_delta()));
}
#[test]
fn spatial_failed_presentation_preserves_previous_complete_cache() {
    let mut w = world(2);
    w.prepare_spatial_presentation(1, 1.).unwrap();
    let old = w.spatial_presentation(&"node-0".into()).unwrap().clone();
    w.begin_spatial_tick(1);
    commit(
        &mut w,
        vec![
            ("node-0", move_to(2.)),
            (
                "node-1",
                Intent::SetWorldAnchor {
                    anchor: [1e12, 0., 0.],
                    preserve_world: false,
                },
            ),
        ],
    );
    assert!(w.prepare_spatial_presentation(2, 1.).is_err());
    assert_eq!(w.spatial_presentation(&"node-0".into()), Some(&old));
    commit(
        &mut w,
        vec![(
            "node-1",
            Intent::SetWorldAnchor {
                anchor: [0.; 3],
                preserve_world: false,
            },
        )],
    );
    w.prepare_spatial_presentation(3, 1.).unwrap();
    near(
        w.spatial_presentation(&"node-0".into())
            .unwrap()
            .world_matrix[12],
        2.,
    );
}
#[test]
fn spatial_production_host_uses_zero_multiple_ticks_and_overload_terminal() {
    use engine_runtime::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
    let mut w = world(1);
    let mut host = EngineHostLoop::new("foundation");
    let input = |dt| {
        let mut input =
            EngineFrameInput::new(EngineHostMode::HeadlessServer).with_unscaled_delta_time(dt);
        input.build_render_submission = false;
        input
    };
    let first = host.tick(input(1. / 120.), &mut w);
    assert_eq!(first.time_trace_summary.unwrap().fixed_frame_count, 0);
    let second = host.tick(input(1. / 120.), &mut w);
    assert_eq!(second.time_trace_summary.unwrap().fixed_frame_count, 1);
    let third = host.tick(input(1. / 30.), &mut w);
    assert_eq!(third.time_trace_summary.unwrap().fixed_frame_count, 3);
    let mut step = EngineFrameInput::new(EngineHostMode::EditorStep).with_unscaled_delta_time(0.);
    step.build_render_submission = false;
    assert_eq!(
        host.tick(step, &mut w)
            .time_trace_summary
            .unwrap()
            .fixed_frame_count,
        4
    );
    let mut fault = None;
    for _ in 0..12 {
        let before = w.spatial_delta().tick;
        let out = host.tick(input(0.33333334), &mut w);
        if out
            .project_runtime_session_report
            .as_ref()
            .is_some_and(|r| r.terminal_fault)
        {
            assert_eq!(w.spatial_delta().tick, before);
            fault = out.project_runtime_session_report;
            break;
        }
    }
    assert!(fault.unwrap().status.contains("simulation_overloaded"));
}

#[test]
fn spatial_frame_loop_pause_scaled_ticks_and_explicit_limit_are_observable() {
    use engine_runtime::{
        frame_loop::{FrameLoop, ProjectRuntimeFrameSession},
        project_runtime_session::{EmptyProjectRuntimeSession, ProjectRuntimeSessionReportLevel},
        render_extract::RenderExtractContext,
        render_state::RenderSceneState,
    };
    let mut w = world(1);
    let mut frame = FrameLoop::new("foundation");
    let mut scene = RenderSceneState::new();
    let mut extract = RenderExtractContext::new();
    let mut session = EmptyProjectRuntimeSession;
    frame.set_spatial_auto_schedule(true);
    frame.runtime_time_mut().set_time_scale(0.5);
    fn binding(
        session: &mut dyn engine_runtime::project_runtime_session::ProjectRuntimeSession,
    ) -> ProjectRuntimeFrameSession<'_> {
        ProjectRuntimeFrameSession {
            session,
            actions: &[],
            report_level: ProjectRuntimeSessionReportLevel::Summary,
            observation_contract: None,
        }
    }
    frame
        .tick_runtime_frame_with_project_session_delta_and_fixed_steps(
            &mut w,
            &mut scene,
            &mut extract,
            None,
            None,
            1. / 30.,
            None,
            binding(&mut session),
            1,
        )
        .unwrap();
    assert_eq!(frame.runtime_time().fixed_frame_count, 1);
    near(frame.runtime_time().fixed_time, 1. / 60.);
    frame.runtime_time_mut().set_time_scale(0.);
    frame
        .tick_runtime_frame_with_project_session_delta_and_fixed_steps(
            &mut w,
            &mut scene,
            &mut extract,
            None,
            None,
            1.,
            None,
            binding(&mut session),
            1,
        )
        .unwrap();
    assert_eq!(frame.runtime_time().fixed_frame_count, 1);
    frame.set_spatial_auto_schedule(false);
    let old = w.spatial_delta().tick;
    let fault = frame
        .tick_runtime_frame_with_project_session_delta_and_fixed_steps(
            &mut w,
            &mut scene,
            &mut extract,
            None,
            None,
            0.,
            None,
            binding(&mut session),
            5,
        )
        .unwrap_err();
    assert!(fault.report.status.contains("explicit_tick_limit"));
    assert_eq!(w.spatial_delta().tick, old);
}

#[test]
fn spatial_structure_budget_and_invalid_profile_leave_entire_batch_unchanged() {
    use engine_runtime::world::SpatialProfile;
    let mut w = world(2);
    assert!(w
        .set_spatial_profile(SpatialProfile {
            max_value_intents: 0,
            max_structure_intents: 1
        })
        .is_err());
    w.set_spatial_profile(SpatialProfile {
        max_value_intents: 3,
        max_structure_intents: 1,
    })
    .unwrap();
    w.begin_spatial_tick(1);
    commit(
        &mut w,
        vec![(
            "node-0",
            Intent::SetWorldAnchor {
                anchor: [16., 0., 0.],
                preserve_world: false,
            },
        )],
    );
    let mut b = ProjectRuntimeMutationBuffer::new();
    write(&mut b, &w, "node-1", 5.);
    b.spatial_command(
        "node-1".into(),
        w.entity_generation(&"node-1".into()).unwrap(),
        Intent::SetWorldAnchor {
            anchor: [32., 0., 0.],
            preserve_world: false,
        },
    );
    assert!(b.prepare(&w).unwrap_err().message.contains("budget"));
    near(w.transform3d(&"node-1".into()).unwrap().translation[0], 0.);
    w.begin_spatial_tick(2);
    commit(
        &mut w,
        vec![(
            "node-1",
            Intent::SetWorldAnchor {
                anchor: [32., 0., 0.],
                preserve_world: false,
            },
        )],
    );
}

#[test]
fn spatial_aui_intents_join_first_tick_history_in_production_frame() {
    use engine_runtime::{
        aui::{AuiAction, AuiActionEvent},
        frame_loop::{FrameLoop, ProjectRuntimeFrameSession},
        project_runtime_session::{
            ProjectAuiActionBatch, ProjectRuntimeSession, ProjectRuntimeSessionContext,
            ProjectRuntimeSessionOutput, ProjectRuntimeSessionReportLevel,
        },
        render_extract::RenderExtractContext,
        render_state::RenderSceneState,
    };
    struct Move;
    impl ProjectRuntimeSession for Move {
        fn session_id(&self) -> &str {
            "foundation.move"
        }
        fn handle_aui_actions(
            &mut self,
            ctx: ProjectRuntimeSessionContext<'_>,
            _: ProjectAuiActionBatch<'_>,
        ) -> ProjectRuntimeSessionOutput {
            let mut batch = ProjectRuntimeMutationBuffer::new();
            batch.spatial_command(
                "node-0".into(),
                ctx.world.entity_generation(&"node-0".into()).unwrap(),
                move_to(8.),
            );
            ProjectRuntimeSessionOutput::applied(batch)
        }
        fn fixed_update(
            &mut self,
            _: ProjectRuntimeSessionContext<'_>,
        ) -> ProjectRuntimeSessionOutput {
            ProjectRuntimeSessionOutput::no_op()
        }
    }
    let mut w = world(1);
    let mut frame = FrameLoop::new("foundation");
    frame.set_spatial_auto_schedule(true);
    let mut scene = RenderSceneState::new();
    let mut extract = RenderExtractContext::new();
    let mut session = Move;
    let actions = [AuiAction {
        action_id: "move".into(),
        node_id: "move".into(),
        event: AuiActionEvent::Click,
        payload: None,
    }];
    frame
        .tick_runtime_frame_with_project_session_and_delta(
            &mut w,
            &mut scene,
            &mut extract,
            None,
            None,
            1. / 40.,
            None,
            ProjectRuntimeFrameSession {
                session: &mut session,
                actions: &actions,
                report_level: ProjectRuntimeSessionReportLevel::Summary,
                observation_contract: None,
            },
        )
        .unwrap();
    near(w.transform3d(&"node-0".into()).unwrap().translation[0], 8.);
    near(
        w.spatial_presentation(&"node-0".into())
            .unwrap()
            .world_matrix[12],
        4.,
    );
}

#[test]
fn spatial_binding_changes_do_not_dirty_descendants_and_visibility_has_its_own_mask() {
    use engine_runtime::world::SpatialFieldMask;
    let mut w = world(2);
    w.try_set_parent("node-1".into(), Some("node-0".into()))
        .unwrap();
    w.take_dirty_records();
    w.begin_spatial_tick(1);
    let slot = w
        .mesh_renderer3d(&"node-0".into())
        .unwrap()
        .materials
        .keys()
        .next()
        .unwrap()
        .clone();
    let mut material = w.mesh_renderer3d(&"node-0".into()).unwrap().materials[&slot]
        .0
        .clone();
    material.id = "new-material".into();
    material.guid = Some("guid-new-material".into());
    commit(
        &mut w,
        vec![(
            "node-0",
            Intent::SetMaterial {
                slot,
                material: project_runtime_sdk::ProjectRuntimeSpatialAssetRef {
                    id: material.id,
                    asset_type: material.asset_type,
                    guid: material.guid,
                    sub_asset: material.sub_asset,
                },
            },
        )],
    );
    let delta = w.finish_spatial_tick();
    assert_eq!(delta.changes.len(), 1);
    assert_eq!(delta.changes[0].field_mask, SpatialFieldMask::BINDING);
    assert!(!w
        .take_dirty_records()
        .iter()
        .any(|d| d.entity_id.as_str() == "node-1"));
    w.begin_spatial_tick(2);
    commit(
        &mut w,
        vec![("node-0", Intent::SetMeshVisible { visible: false })],
    );
    let delta = w.finish_spatial_tick();
    assert_eq!(delta.changes.len(), 2);
    assert!(delta
        .changes
        .iter()
        .all(|c| c.field_mask == SpatialFieldMask::VISIBILITY));
}

#[path = "support/geometry_fixture.rs"]
mod geometry_fixture;
#[test]
fn retained_shadow_depth_batches_ignore_opaque_color_binding_and_cross_mask_barriers() {
    use engine_runtime::render_contract::*;
    use engine_runtime::render_geometry::*;
    use engine_runtime::render_resource::RenderResourceManager;
    use geometry_fixture::*;
    let mut manager = RenderResourceManager::new();
    let mesh = prepare("shared", RenderAssetData::Mesh(triangle()), &mut manager);
    let red = prepare(
        "red",
        RenderAssetData::Material(unlit([1., 0., 0., 1.])),
        &mut manager,
    );
    let green = prepare(
        "green",
        RenderAssetData::Material(unlit([0., 1., 0., 1.])),
        &mut manager,
    );
    let mut mask = unlit([0., 0., 1., 0.5]);
    mask.blend = MaterialBlend::AlphaMask;
    mask.parameters
        .insert("alphaCutoff".into(), MaterialParameter::Float(0.5));
    let mask = prepare("mask", RenderAssetData::Material(mask), &mut manager);
    let red = PreparedGeometry::from_assets(&mesh, &red, 0, &manager).unwrap();
    let green = PreparedGeometry::from_assets(&mesh, &green, 0, &manager).unwrap();
    let mask = PreparedGeometry::from_assets(&mesh, &mask, 0, &manager).unwrap();
    let mut view = RenderView::perspective(1, 64, 64, 60., 0.1, 100.);
    view.view_kind = ViewKind::Shadow;
    view.target = TargetKind::ShadowMap;
    view.clear_policy = ClearPolicy::Depth { depth: 1. };
    view.depth_policy = DepthPolicy::TestAndWrite;
    let items = [
        instance(&red, 1, [0., 0., -2.]),
        instance(&mask, 2, [0., 0., -3.]),
        instance(&green, 3, [0., 0., -4.]),
        instance(&red, 4, [0., 0., -5.]),
    ];
    let frame = GeometryFrame::build(
        "atlas",
        [64, 64],
        &[GeometryViewInput {
            view: &view,
            pose: CameraPose::default(),
        }],
        &items,
        &manager,
        DiagnosticLevel::Off,
    )
    .unwrap();
    let draws = &frame.views[0].draws;
    let red = draws.iter().find(|d| d.instance_id == 1).unwrap();
    let green = draws.iter().find(|d| d.instance_id == 3).unwrap();
    assert_eq!(
        red.batch_identity(),
        green.batch_identity(),
        "opaque depth-only batches must not split on color-only material bindings"
    );
    let positions: Vec<_> = draws
        .iter()
        .enumerate()
        .filter(|(_, d)| d.blend == MaterialBlend::Opaque)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        positions.last().unwrap() - positions[0],
        2,
        "mask must not split the commutative opaque shadow depth batch"
    );
    let mask = draws.iter().find(|d| d.instance_id == 2).unwrap();
    assert_ne!(red.batch_identity(), mask.batch_identity());
}

#[test]
fn retained_phase_buckets_opaque_but_preserves_transparent_order() {
    use engine_runtime::render_contract::*;
    use engine_runtime::render_geometry::*;
    use engine_runtime::render_resource::RenderResourceManager;
    use geometry_fixture::*;
    let mut manager = RenderResourceManager::new();
    let mesh = prepare("shared", RenderAssetData::Mesh(triangle()), &mut manager);
    let red = prepare(
        "red",
        RenderAssetData::Material(unlit([1., 0., 0., 1.])),
        &mut manager,
    );
    let green = prepare(
        "green",
        RenderAssetData::Material(unlit([0., 1., 0., 1.])),
        &mut manager,
    );
    let red = PreparedGeometry::from_assets(&mesh, &red, 0, &manager).unwrap();
    let green = PreparedGeometry::from_assets(&mesh, &green, 0, &manager).unwrap();
    let mut transparent = unlit([0., 0., 1., 0.5]);
    transparent.blend = MaterialBlend::AlphaBlend;
    transparent.depth = MaterialDepth::TestOnly;
    let transparent = prepare(
        "transparent",
        RenderAssetData::Material(transparent),
        &mut manager,
    );
    let transparent = PreparedGeometry::from_assets(&mesh, &transparent, 0, &manager).unwrap();
    let view = RenderView::perspective(1, 64, 64, 60., 0.1, 100.);
    let items = [
        instance(&red, 1, [0., 0., -2.]),
        instance(&green, 2, [0., 0., -3.]),
        instance(&red, 3, [0., 0., -4.]),
        instance(&transparent, 4, [0., 0., -5.]),
        instance(&transparent, 5, [0., 0., -6.]),
    ];
    let frame = GeometryFrame::build(
        "main",
        [64, 64],
        &[GeometryViewInput {
            view: &view,
            pose: CameraPose::default(),
        }],
        &items,
        &manager,
        DiagnosticLevel::Off,
    )
    .unwrap();
    let draws = &frame.views[0].draws;
    let first_red = draws.iter().position(|d| d.instance_id == 1).unwrap();
    let second_red = draws.iter().position(|d| d.instance_id == 3).unwrap();
    assert_eq!(
        second_red,
        first_red + 1,
        "same opaque batch must not remain split by depth interleaving"
    );
    assert_eq!(
        draws[3..].iter().map(|d| d.instance_id).collect::<Vec<_>>(),
        [5, 4]
    );
    assert_eq!(
        draws[first_red].batch_identity(),
        draws[second_red].batch_identity()
    );
}
#[test]
fn retained_native_flat_contract_roundtrip_and_pipeline_identity() {
    use engine_runtime::render_contract::*;
    use geometry_fixture::*;
    let original = unlit([1.; 4]);
    let old_bytes = serde_json::to_vec(&original).unwrap();
    assert!(!String::from_utf8(old_bytes.clone())
        .unwrap()
        .contains("ignoreVertexColor"));
    let decoded: CookedMaterial = serde_json::from_slice(&old_bytes).unwrap();
    assert!(!decoded.ignore_vertex_color);
    let mut flat = original.clone();
    flat.ignore_vertex_color = true;
    assert_ne!(
        original.pipeline_identity().unwrap(),
        flat.pipeline_identity().unwrap()
    );
    let roundtrip: CookedMaterial =
        serde_json::from_slice(&serde_json::to_vec(&flat).unwrap()).unwrap();
    assert!(roundtrip.ignore_vertex_color);
}

#[test]
fn retained_cpu_packet_roundtrip_keeps_old_material_transform_and_alpha() {
    use engine_runtime::render_resource::RenderResourceManager;
    use engine_runtime::rhi_command_plan::{
        compile_render_graph_to_rhi_plan, RhiCommand, RhiDrawPayload,
    };
    use geometry_fixture::*;
    let mut manager = RenderResourceManager::new();
    let source = source(&mut manager);
    let old = frame(&source, &manager);
    let old_plan = compile_render_graph_to_rhi_plan(&graph(&old));
    assert!(!old_plan.has_errors());
    let bytes = serde_json::to_vec(&old_plan).unwrap();
    let mut new = old.clone();
    std::sync::Arc::make_mut(&mut new.views[0].draws[0]).world_transform[12] = 0.5.into();
    std::sync::Arc::make_mut(&mut new.views[0].draws[0]).base_color =
        [0., 1., 0., 0.4].map(Into::into);
    let new_plan = compile_render_graph_to_rhi_plan(&graph(&new));
    assert_ne!(serde_json::to_vec(&new_plan).unwrap(), bytes);
    assert_eq!(serde_json::to_vec(&old_plan).unwrap(), bytes);
    let roundtrip: engine_runtime::rhi_command_plan::RhiCommandPlan =
        serde_json::from_slice(&bytes).unwrap();
    let draw = roundtrip
        .commands
        .iter()
        .find_map(|c| match c {
            RhiCommand::Draw {
                payload: RhiDrawPayload::Geometry { draw },
                ..
            } => Some(draw),
            _ => None,
        })
        .unwrap();
    assert_eq!(draw, &old.views[0].draws[0]);
}

#[test]
fn retained_projection_origin_tracks_anchored_fallback_and_presented_basis() {
    use engine_runtime::{
        render_command::apply_batch,
        render_extract::RenderExtractContext,
        render_state::{RenderProxyPayload, RenderSceneState},
    };
    let mut w = world(1);
    commit(
        &mut w,
        vec![
            (
                "node-0",
                Intent::SetWorldAnchor {
                    anchor: [100., 0., 0.],
                    preserve_world: false,
                },
            ),
            (
                "camera",
                Intent::SetWorldAnchor {
                    anchor: [200., 0., 0.],
                    preserve_world: false,
                },
            ),
        ],
    );
    let mut extract = RenderExtractContext::new();
    let mut scene = RenderSceneState::new();
    let queue = extract
        .extract_world_dirty(1, &mut w, &scene)
        .normalize_merge(&scene);
    assert!(apply_batch(&mut scene, &queue).is_empty());
    let proxy = scene
        .proxy(scene.proxy_for_source(&"node-0".into()).unwrap())
        .unwrap();
    let RenderProxyPayload::Mesh3D(p) = &proxy.payload else {
        panic!()
    };
    assert_eq!(p.coordinate_origin, [100., 0., 0.]);
    near(p.world.world_matrix[12], 0.);
    w.prepare_spatial_presentation(2, 1.).unwrap();
    let queue = extract
        .extract_world_dirty(2, &mut w, &scene)
        .normalize_merge(&scene);
    assert!(apply_batch(&mut scene, &queue).is_empty());
    let proxy = scene
        .proxy(scene.proxy_for_source(&"node-0".into()).unwrap())
        .unwrap();
    let RenderProxyPayload::Mesh3D(p) = &proxy.payload else {
        panic!()
    };
    assert_eq!(p.coordinate_origin, [200., 0., 0.]);
    near(p.world.world_matrix[12], -100.);
}

#[path = "support/project_cell_session.rs"]
mod project_cell_session;

#[test]
fn cell_project_host_defers_load_and_unload_to_simulation_safe_points() {
    use project_cell_session::*;
    use project_runtime_sdk::{
        ProjectRuntimeCellIntent as CellIntent, ProjectRuntimeCellStatus as Status,
    };
    let fixture = streamed_cells::build(false);
    let mut loader = engine_runtime::runtime_instance_loader::RuntimeInstanceLoader::from_package(
        &fixture.package,
    );
    let mut world = World::new();
    let (mut host, control) = host();
    control.lock().unwrap().next = vec![load(1, "a", false), load(2, "b", false)];
    tick(
        &mut host,
        &mut world,
        &mut loader,
        &fixture.package,
        1,
        #[cfg(feature = "real-wgpu")]
        None,
    );
    assert_eq!(
        world.entity_count(),
        0,
        "accepting a demand must not publish entities"
    );
    assert_eq!(
        loader.project_cell_snapshot(&world).receipts[&1].status,
        Status::Accepted
    );
    for _ in 0..4 {
        tick(
            &mut host,
            &mut world,
            &mut loader,
            &fixture.package,
            0,
            #[cfg(feature = "real-wgpu")]
            None,
        );
    }
    assert_eq!(
        loader.project_cell_snapshot(&world).receipts[&1].status,
        Status::Accepted
    );
    wait_active(&mut host, &mut world, &mut loader, &fixture.package, 2);
    wait_active(&mut host, &mut world, &mut loader, &fixture.package, 1);
    assert_eq!(world.entity_count(), 2);
    let snapshot = loader.project_cell_snapshot(&world);
    assert_eq!(snapshot.entities.len(), 2);
    control.lock().unwrap().next = vec![CellIntent::Release { request_id: 1 }];
    tick(
        &mut host,
        &mut world,
        &mut loader,
        &fixture.package,
        1,
        #[cfg(feature = "real-wgpu")]
        None,
    );
    assert_eq!(world.entity_count(), 2, "release is a deferred demand");
    tick(
        &mut host,
        &mut world,
        &mut loader,
        &fixture.package,
        0,
        #[cfg(feature = "real-wgpu")]
        None,
    );
    assert_eq!(world.entity_count(), 2);
    tick(
        &mut host,
        &mut world,
        &mut loader,
        &fixture.package,
        1,
        #[cfg(feature = "real-wgpu")]
        None,
    );
    let after = loader.project_cell_snapshot(&world);
    assert_eq!(world.entity_count(), 1);
    assert_eq!(after.receipts[&1].status, Status::Released);
    assert!(!after
        .entities
        .contains_key(&("placement-a".into(), "node".into())));
    assert_eq!(after.receipts[&2].status, Status::Active);
}

#[test]
fn cell_project_cancel_before_prepare_and_reload_never_resurrects_old_request() {
    use project_cell_session::*;
    use project_runtime_sdk::{ProjectRuntimeCellIntent as I, ProjectRuntimeCellStatus as Status};
    let fixture = streamed_cells::build(false);
    let mut loader = engine_runtime::runtime_instance_loader::RuntimeInstanceLoader::from_package(
        &fixture.package,
    );
    let mut world = World::new();
    let (mut host, control) = host();
    control.lock().unwrap().next = vec![load(1, "a", false), I::Release { request_id: 1 }];
    tick(
        &mut host,
        &mut world,
        &mut loader,
        &fixture.package,
        1,
        #[cfg(feature = "real-wgpu")]
        None,
    );
    tick(
        &mut host,
        &mut world,
        &mut loader,
        &fixture.package,
        1,
        #[cfg(feature = "real-wgpu")]
        None,
    );
    assert_eq!(
        loader.project_cell_snapshot(&world).receipts[&1].status,
        Status::Released
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while loader
        .asset_loader()
        .streaming_snapshot()
        .pending_operations
        != 0
    {
        assert!(std::time::Instant::now() < deadline);
        loader.asset_loader_mut().poll_async();
        std::thread::yield_now();
    }
    assert_eq!(world.entity_count(), 0);
    control.lock().unwrap().next = vec![load(2, "a", false)];
    tick(
        &mut host,
        &mut world,
        &mut loader,
        &fixture.package,
        1,
        #[cfg(feature = "real-wgpu")]
        None,
    );
    wait_active(&mut host, &mut world, &mut loader, &fixture.package, 2);
    assert_eq!(
        loader.project_cell_snapshot(&world).receipts[&1].status,
        Status::Released
    );
    assert_eq!(world.entity_count(), 1);
}

#[test]
fn cell_project_rejected_callback_and_invalid_batch_do_not_accept_partial_demands() {
    use project_cell_session::*;
    let fixture = streamed_cells::build(false);
    let mut loader = engine_runtime::runtime_instance_loader::RuntimeInstanceLoader::from_package(
        &fixture.package,
    );
    let mut world = World::new();
    let (mut host, control) = host();
    {
        let mut c = control.lock().unwrap();
        c.next = vec![load(1, "a", false)];
        c.rejected = true;
    }
    tick(
        &mut host,
        &mut world,
        &mut loader,
        &fixture.package,
        1,
        #[cfg(feature = "real-wgpu")]
        None,
    );
    assert!(loader.project_cell_snapshot(&world).receipts.is_empty());
    {
        let mut c = control.lock().unwrap();
        c.next = vec![load(1, "a", false), load(1, "b", false)];
        c.rejected = false;
    }
    let output = tick(
        &mut host,
        &mut world,
        &mut loader,
        &fixture.package,
        1,
        #[cfg(feature = "real-wgpu")]
        None,
    );
    assert!(
        output
            .project_runtime_session_report
            .unwrap()
            .terminal_fault
    );
    assert!(loader.project_cell_snapshot(&world).receipts.is_empty());
    assert_eq!(
        loader
            .asset_loader()
            .streaming_snapshot()
            .dispatched_operations,
        0
    );
    assert_eq!(world.entity_count(), 0);
}

#[test]
fn cell_project_world_replacement_discards_prepared_resources_without_touching_new_entities() {
    use project_cell_session::*;
    use project_runtime_sdk::ProjectRuntimeCellStatus as Status;
    let fixture = streamed_cells::build(false);
    let mut loader = engine_runtime::runtime_instance_loader::RuntimeInstanceLoader::from_package(
        &fixture.package,
    );
    let mut old = World::new();
    let (mut host, control) = host();
    control.lock().unwrap().next = vec![load(1, "a", false)];
    tick(
        &mut host,
        &mut old,
        &mut loader,
        &fixture.package,
        1,
        #[cfg(feature = "real-wgpu")]
        None,
    );
    wait_active(&mut host, &mut old, &mut loader, &fixture.package, 1);
    let mut replacement = World::new();
    tick(
        &mut host,
        &mut replacement,
        &mut loader,
        &fixture.package,
        1,
        #[cfg(feature = "real-wgpu")]
        None,
    );
    let snapshot = loader.project_cell_snapshot(&replacement);
    assert_eq!(snapshot.receipts[&1].status, Status::Failed);
    assert_eq!(
        snapshot.receipts[&1].error_code.as_deref(),
        Some("cell.world_changed")
    );
    assert_eq!(replacement.entity_count(), 0);
    assert_eq!(
        loader
            .asset_loader()
            .streaming_snapshot()
            .cpu_resident_bytes,
        0,
        "old world cells must release assets"
    );
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires a real GPU"]
fn cell_project_host_real_gpu_upload_receipts_migration_and_retirement_are_consumed() {
    for bytes_per_frame in [8, 4096] {
        cell_project_host_gpu_case(bytes_per_frame);
    }
}

#[cfg(feature = "real-wgpu")]
fn cell_project_host_gpu_case(bytes_per_frame: usize) {
    use engine_runtime::game_view_presentation::*;
    use engine_runtime::wgpu_backend::real::{GeometryUploadBudget, RealWgpuBackend};
    use project_cell_session::*;
    use project_runtime_sdk::{
        ProjectRuntimeCellEntity, ProjectRuntimeCellIntent as I, ProjectRuntimeCellStatus as Status,
    };
    let fixture = streamed_cells::build(true);
    let mut loader = engine_runtime::runtime_instance_loader::RuntimeInstanceLoader::from_package(
        &fixture.package,
    );
    let mut world = world(0);
    commit(
        &mut world,
        vec![(
            "camera",
            Intent::SetLocalTransform {
                transform: ProjectRuntimeTransform3D {
                    translation: [0., 0., 2.],
                    ..ProjectRuntimeTransform3D::identity()
                },
            },
        )],
    );
    let (mut host, control) = host();
    host.set_game_view_target(GameViewTargetSpec {
        extent: GameViewExtent::new(64, 64),
        scale_policy: GameViewScalePolicy::Contain,
    });
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    backend
        .set_geometry_upload_budget(GeometryUploadBudget {
            resident_bytes: 1024 * 1024,
            staging_bytes: if bytes_per_frame == 8 { 32 } else { 4096 },
            bytes_per_frame,
            max_tickets: 4,
        })
        .unwrap();
    control.lock().unwrap().next = vec![load(1, "a", true), load(2, "b", true)];
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    let mut frame = 0;
    loop {
        frame += 1;
        assert!(
            std::time::Instant::now() < deadline,
            "{:?}",
            loader.project_cell_snapshot(&world)
        );
        let uploaded = backend.pump_geometry_uploads(frame).unwrap();
        assert!(uploaded <= bytes_per_frame);
        assert!(
            backend.geometry_upload_snapshot().staging_bytes
                <= if bytes_per_frame == 8 { 32 } else { 4096 }
        );
        let output = tick(
            &mut host,
            &mut world,
            &mut loader,
            &fixture.package,
            1,
            Some(&mut backend),
        );
        assert!(
            !output
                .project_runtime_session_report
                .as_ref()
                .is_some_and(|r| r.terminal_fault),
            "{output:?}"
        );
        let plan = output.render_thread_frame.unwrap();
        let snapshot = loader.project_cell_snapshot(&world);
        let pixels =
            backend.render_plan_to_rgba_bytes(&plan.renderer_output.rhi_command_plan, 64, 64);
        if let Err(error) = &pixels {
            assert_eq!(bytes_per_frame, 8, "drawable recipe failed: {error}");
            assert_eq!(error, "geometry.visual_upload_capacity");
            // A view clear itself has a 144-byte record, even before a cell
            // activates. The sub-record recipe must reject it as well.
            assert!(backend.geometry_upload_snapshot().frame_uploaded_bytes <= 8);
        }
        if snapshot.receipts[&1].status == Status::Active
            && snapshot.receipts[&2].status == Status::Active
        {
            if bytes_per_frame == 8 {
                assert_eq!(pixels.unwrap_err(), "geometry.visual_upload_capacity");
            } else {
                let pixels = pixels.unwrap();
                assert!(pixels.chunks_exact(4).any(|p| p[0] > 200 || p[2] > 200));
                save_cell_consumer_pixels("loaded", &pixels);
            }
            break;
        }
        for r in snapshot.receipts.values() {
            assert_ne!(r.status, Status::Failed, "{r:?}");
        }
        let ready = snapshot
            .receipts
            .values()
            .filter(|r| r.status == Status::Active)
            .count();
        assert_eq!(
            world.entity_count(),
            1 + ready,
            "only complete cells may appear in gameplay"
        );
    }
    assert_eq!(
        backend.geometry_upload_count(),
        1,
        "two cells share one physical mesh upload"
    );
    assert_eq!(backend.geometry_residency().textures, 1);
    let node = loader.project_cell_snapshot(&world).entities
        [&("placement-a".into(), "node".into())]
        .clone();
    control.lock().unwrap().next = vec![I::Migrate {
        request_id: 1,
        persistent_request_id: 3,
        source: ProjectRuntimeCellEntity {
            entity_id: node.0.to_string(),
            generation: node.1,
        },
        persistent_placement_id: "persistent".into(),
        parent: None,
    }];
    tick(
        &mut host,
        &mut world,
        &mut loader,
        &fixture.package,
        1,
        Some(&mut backend),
    );
    tick(
        &mut host,
        &mut world,
        &mut loader,
        &fixture.package,
        1,
        Some(&mut backend),
    );
    let migrated = loader.project_cell_snapshot(&world);
    assert_eq!(
        migrated.receipts[&3].status,
        Status::Active,
        "{:?}",
        migrated.receipts[&3]
    );
    assert_eq!(
        migrated.entities[&("persistent".into(), "node".into())],
        node
    );
    control.lock().unwrap().next = vec![I::Release { request_id: 1 }, I::Release { request_id: 2 }];
    tick(
        &mut host,
        &mut world,
        &mut loader,
        &fixture.package,
        1,
        Some(&mut backend),
    );
    let output = tick(
        &mut host,
        &mut world,
        &mut loader,
        &fixture.package,
        1,
        Some(&mut backend),
    );
    let plan = output.render_thread_frame.unwrap();
    let pixels = backend.render_plan_to_rgba_bytes(&plan.renderer_output.rhi_command_plan, 64, 64);
    if bytes_per_frame == 8 {
        assert_eq!(pixels.unwrap_err(), "geometry.visual_upload_capacity");
        assert!(backend.geometry_upload_snapshot().frame_uploaded_bytes <= 8);
    } else {
        let pixels = pixels.unwrap();
        assert!(
            pixels.chunks_exact(4).any(|p| p[0] > 200 && p[2] < 40),
            "persistent red cell must still render after both source cells unload"
        );
        save_cell_consumer_pixels("persistent-after-source-unload", &pixels);
    }
    assert_eq!(world.entity_generation(&node.0), Some(node.1));
    control.lock().unwrap().next = vec![I::Release { request_id: 3 }];
    tick(
        &mut host,
        &mut world,
        &mut loader,
        &fixture.package,
        1,
        Some(&mut backend),
    );
    let output = tick(
        &mut host,
        &mut world,
        &mut loader,
        &fixture.package,
        1,
        Some(&mut backend),
    );
    let empty_pixels = backend.render_plan_to_rgba_bytes(
        &output
            .render_thread_frame
            .unwrap()
            .renderer_output
            .rhi_command_plan,
        64,
        64,
    );
    if bytes_per_frame == 8 {
        assert_eq!(empty_pixels.unwrap_err(), "geometry.visual_upload_capacity");
    } else {
        save_cell_consumer_pixels("all-unloaded", &empty_pixels.unwrap());
    }
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(world.entity_count(), 1);
    assert_eq!(backend.geometry_upload_snapshot().tickets, 0);
    assert_eq!(backend.geometry_residency().meshes, 0);
    assert_eq!(backend.geometry_residency().textures, 0);
    assert_eq!(
        loader
            .asset_loader()
            .streaming_snapshot()
            .cpu_resident_bytes,
        0
    );
}

#[cfg(feature = "real-wgpu")]
fn save_cell_consumer_pixels(name: &str, pixels: &[u8]) {
    let Some(root) = std::env::var_os("AIFE_371_CELL_EVIDENCE") else {
        return;
    };
    let root = std::path::PathBuf::from(root);
    std::fs::create_dir_all(&root).unwrap();
    let file = std::fs::File::create_new(root.join(format!("{name}.png"))).unwrap();
    let mut encoder = png::Encoder::new(file, 64, 64);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(pixels)
        .unwrap();
}

#[test]
fn cell_project_revision_change_and_decode_budget_are_terminal_queryable_errors() {
    use project_cell_session::*;
    use project_runtime_sdk::ProjectRuntimeCellStatus as Status;
    let fixture = streamed_cells::build(false);
    let mut loader = engine_runtime::runtime_instance_loader::RuntimeInstanceLoader::from_package(
        &fixture.package,
    );
    let mut world = World::new();
    let (mut host, control) = project_cell_session::host();
    control.lock().unwrap().next = vec![load(1, "a", false)];
    tick(
        &mut host,
        &mut world,
        &mut loader,
        &fixture.package,
        1,
        #[cfg(feature = "real-wgpu")]
        None,
    );
    loader.asset_loader_mut().mount_patch_index(
        engine_runtime::runtime_asset::RuntimeAssetIndex::new(vec![], vec![]),
    );
    tick(
        &mut host,
        &mut world,
        &mut loader,
        &fixture.package,
        1,
        #[cfg(feature = "real-wgpu")]
        None,
    );
    let receipt = loader.project_cell_snapshot(&world).receipts[&1].clone();
    assert_eq!(receipt.status, Status::Failed);
    assert_eq!(receipt.error_code.as_deref(), Some("cell.revision_changed"));
    assert_eq!(world.entity_count(), 0);
    let mut loader = engine_runtime::runtime_instance_loader::RuntimeInstanceLoader::from_package(
        &fixture.package,
    );
    let mut budget = loader.asset_loader().streaming_budget();
    budget.max_asset_bytes = 8192;
    budget.max_cpu_resident_bytes = 8192;
    loader
        .asset_loader_mut()
        .set_streaming_budget(budget)
        .unwrap();
    let (mut host, control) = project_cell_session::host();
    control.lock().unwrap().next = vec![load(1, "a", false)];
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        assert!(
            std::time::Instant::now() < deadline,
            "a hard budget failure must not remain Loading forever"
        );
        tick(
            &mut host,
            &mut world,
            &mut loader,
            &fixture.package,
            1,
            #[cfg(feature = "real-wgpu")]
            None,
        );
        let snapshot = loader.project_cell_snapshot(&world);
        if snapshot.receipts[&1].status == Status::Failed {
            assert!(snapshot.receipts[&1].error_code.is_some());
            break;
        }
        std::thread::yield_now();
    }
    assert_eq!(world.entity_count(), 0);
    assert_eq!(
        loader
            .asset_loader()
            .streaming_snapshot()
            .cpu_resident_bytes,
        0
    );
}

// These exact-scale owner cases prove only spatial correctness/boundedness.
// They are not BM01 AUI/Release timing, BM09 pixel error or BM12 GPU qualification.
fn foundation_pure_spatial_world(count: usize) -> World {
    let mut scene: serde_json::Value =
        serde_json::from_slice(include_bytes!("fixtures/three_d_a0/runtime-scene-v2.json"))
            .unwrap();
    let mut template = scene["entities"][0].clone();
    template["spatial"]
        .as_object_mut()
        .unwrap()
        .remove("meshRenderer");
    let camera = scene["entities"][1].clone();
    let mut entities = Vec::with_capacity(count + 1);
    for i in 0..count {
        let mut v = template.clone();
        v["id"] = serde_json::json!(format!("node-{i}"));
        entities.push(v);
    }
    entities.push(camera);
    scene["entities"] = serde_json::json!(entities);
    let RuntimeSceneDocument::Spatial(scene) =
        decode_runtime_scene(&serde_json::to_vec(&scene).unwrap()).unwrap()
    else {
        panic!("spatial scene required")
    };
    let loaded = load_scene_into_world(&scene.into());
    assert!(loaded.diagnostics.is_ok(), "{:?}", loaded.diagnostics);
    let mut w = loaded.value.unwrap();
    w.take_dirty_records();
    w
}
#[test]
fn foundation_bm01_ten_thousand_actual_dirty_masks_are_exact() {
    let mut w = foundation_pure_spatial_world(10000);
    for (tick, count) in [(1, 0), (2, 100), (3, 1000), (4, 10000)] {
        w.begin_spatial_tick(tick);
        let mut batch = ProjectRuntimeMutationBuffer::new();
        for i in 0..count {
            write(&mut batch, &w, &format!("node-{i}"), tick as f32);
        }
        batch.prepare(&w).unwrap().commit(&mut w).unwrap();
        let delta = w.finish_spatial_tick();
        assert_eq!(delta.changes.len(), count);
        assert_eq!(delta.affected_count, count);
        assert!(delta
            .changes
            .iter()
            .all(|c| c.field_mask == engine_runtime::world::SpatialFieldMask::TRANSLATION));
        w.take_dirty_records();
    }
}
#[test]
fn foundation_bm09_thousand_objects_three_anchors_twenty_relocations_preserve_local_precision() {
    let mut w = foundation_pure_spatial_world(1000);
    let mut tick = 0;
    for anchor in [0., 10000., 1000000.] {
        for round in 0..20 {
            tick += 1;
            w.begin_spatial_tick(tick);
            let mut batch = ProjectRuntimeMutationBuffer::new();
            for i in 0..1000 {
                let id = format!("node-{i}");
                write(
                    &mut batch,
                    &w,
                    &id,
                    0.125 + i as f32 * 0.5 + if i < 10 { round as f32 * 0.001 } else { 0. },
                );
                batch.spatial_command(
                    id.clone().into(),
                    w.entity_generation(&id.into()).unwrap(),
                    Intent::SetWorldAnchor {
                        anchor: [anchor + round as f64 * 16., 0., 0.],
                        preserve_world: false,
                    },
                );
            }
            batch.spatial_command(
                "camera".into(),
                w.entity_generation(&"camera".into()).unwrap(),
                Intent::SetWorldAnchor {
                    anchor: [anchor + round as f64 * 16., 0., 0.],
                    preserve_world: false,
                },
            );
            batch.prepare(&w).unwrap().commit(&mut w).unwrap();
            w.finish_spatial_tick();
            w.prepare_spatial_presentation(tick, 1.0).unwrap();
            for i in 0..1000 {
                let expected =
                    0.125 + i as f32 * 0.5 + if i < 10 { round as f32 * 0.001 } else { 0. };
                let observed = w
                    .spatial_presentation(&format!("node-{i}").into())
                    .unwrap()
                    .world_matrix[12];
                assert!(
                    (observed - expected).abs() <= 0.001,
                    "anchor={anchor} round={round} i={i}: {observed} != {expected}"
                );
            }
            w.take_dirty_records();
        }
    }
}
#[test]
fn foundation_bm12_one_one_point_five_two_scale_preserve_atomic_budget_rejection() {
    for count in [10000, 15000, 20000] {
        let mut w = foundation_pure_spatial_world(count);
        w.begin_spatial_tick(1);
        let mut batch = ProjectRuntimeMutationBuffer::new();
        for i in 0..count {
            write(&mut batch, &w, &format!("node-{i}"), 1.);
        }
        if count <= 16384 {
            batch.prepare(&w).unwrap().commit(&mut w).unwrap();
            assert_eq!(w.finish_spatial_tick().changes.len(), count);
        } else {
            let error = batch.prepare(&w).unwrap_err();
            assert!(error.message.contains("budget"), "{error:?}");
            assert!(w.take_dirty_records().is_empty());
            for i in [0, count / 2, count - 1] {
                near(
                    w.transform3d(&format!("node-{i}").into())
                        .unwrap()
                        .translation[0],
                    0.,
                );
            }
            w.begin_spatial_tick(2);
            commit(&mut w, vec![("node-0", move_to(2.))]);
            assert_eq!(w.finish_spatial_tick().changes.len(), 1);
        }
    }
}
