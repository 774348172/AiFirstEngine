//! Exact fault primitives through actual package/Host/async owners.
//! These do not grant the entire BM11 static/dual-32-MiB recipe qualification.
use engine_runtime::{
    engine_host_loop::*, frame_loop::RuntimeFrameContext, project_runtime_session::*,
    runtime_asset::RuntimeAssetLoadState, runtime_instance_loader::RuntimeInstanceLoader,
    scene_loader::load_scene_into_world,
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Default)]
struct State {
    next: Option<ProjectRuntimeMutationBuffer>,
    receipts: BTreeMap<u64, (engine_runtime::ids::EntityId, u64)>,
}
struct Session(Arc<Mutex<State>>);

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "real GPU required; exact dual-cell closure budget faults, not full BM11 qualification"]
fn foundation_f07_dual_cell_seventy_five_percent_queue_and_gpu_closure_refuse_atomically() {
    use engine_runtime::{
        render_resource::RenderResourceManager,
        runtime_asset_loader::{StreamingBudget, StreamingError},
        runtime_instance_loader::{CellError, CellStatus},
        wgpu_backend::real::{GeometryUploadBudget, RealWgpuBackend},
        world::World,
    };
    let fixture = super::streamed_cells::build_dual_32mib_cells();
    let mut pending = vec!["cell-a".to_owned()];
    let mut closure = std::collections::BTreeSet::new();
    while let Some(id) = pending.pop() {
        if closure.insert(id.clone()) {
            pending.extend(
                fixture
                    .package
                    .runtime_asset_index
                    .record_by_guid(&id)
                    .unwrap()
                    .dependencies
                    .iter()
                    .cloned(),
            );
        }
    }
    let queue_limit = closure.len() * 3 / 4;
    assert!(queue_limit > 0 && queue_limit < closure.len());
    let mut refused_loader = RuntimeInstanceLoader::from_package(&fixture.package);
    let mut refused_world = World::new();
    refused_loader
        .asset_loader_mut()
        .set_streaming_budget(StreamingBudget {
            max_operations: queue_limit,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        refused_loader.request_cell(super::streamed_cells::request("a", 1, true), &refused_world),
        Err(CellError::Asset(StreamingError::BudgetExceeded))
    );
    let snapshot = refused_loader.asset_loader().streaming_snapshot();
    assert_eq!(snapshot.dispatched_operations, 0);
    assert_eq!(snapshot.reserved_bytes, 0);
    assert_eq!(refused_loader.cell_lifetime_counts(), (0, 0));
    assert_eq!(refused_world.entity_count(), 0);

    // Observe the exact successful physical mesh + texture closure first. This
    // is actual backing, including page padding, not the source envelope or RSS.
    let mut loader = RuntimeInstanceLoader::from_package(&fixture.package);
    let mut world = World::new();
    let mut manager = RenderResourceManager::new();
    let mut backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
    let normal_budget = GeometryUploadBudget {
        resident_bytes: 2 * 1024 * 1024 * 1024,
        staging_bytes: 128 * 1024 * 1024,
        bytes_per_frame: 16 * 1024 * 1024,
        max_tickets: 128,
    };
    backend.set_geometry_upload_budget(normal_budget).unwrap();
    let a = loader
        .request_cell(super::streamed_cells::request("a", 1, true), &world)
        .unwrap();
    super::prepare_streamed_cell(&mut loader, a, &world, &mut manager);
    loader.begin_cell_upload(a, &world, &mut backend).unwrap();
    for frame in 1..=16 {
        let uploaded = backend.pump_geometry_uploads(frame).unwrap();
        assert!(uploaded <= normal_budget.bytes_per_frame);
        if uploaded > 0 {
            backend
                .wait_gpu_submission(
                    backend.last_gpu_submission().unwrap(),
                    Duration::from_secs(5),
                )
                .unwrap();
        }
        match loader.commit_cell_gpu(a, &mut world, &mut backend) {
            Ok(_) => break,
            Err(CellError::NotReady) if frame < 16 => (),
            other => panic!("normal closure did not activate: {other:?}"),
        }
    }
    assert_eq!(loader.cell_status(a).unwrap(), CellStatus::Active);
    assert_eq!(world.entity_count(), 2);
    let residency = backend.geometry_residency();
    assert_eq!(residency.texture_bytes, 32 * 1024 * 1024);
    let normal_closure = residency.physical_mesh_bytes + residency.texture_bytes;
    let restricted = normal_closure * 3 / 4;
    assert!(restricted < 32 * 1024 * 1024);

    refused_loader
        .asset_loader_mut()
        .set_streaming_budget(Default::default())
        .unwrap();
    let mut refused_manager = RenderResourceManager::new();
    let b = refused_loader
        .request_cell(super::streamed_cells::request("b", 1, true), &refused_world)
        .unwrap();
    super::prepare_streamed_cell(&mut refused_loader, b, &refused_world, &mut refused_manager);
    let mut restricted_backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
    restricted_backend
        .set_geometry_upload_budget(GeometryUploadBudget {
            resident_bytes: restricted,
            ..normal_budget
        })
        .unwrap();
    assert_eq!(
        refused_loader.begin_cell_upload(b, &refused_world, &mut restricted_backend),
        Err(CellError::Render("geometry.upload_resident_budget".into()))
    );
    assert_eq!(refused_loader.cell_status(b).unwrap(), CellStatus::Prepared);
    assert!(matches!(
        refused_loader.commit_cell_gpu(b, &mut refused_world, &mut restricted_backend),
        Err(CellError::NotReady)
    ));
    assert_eq!(refused_world.entity_count(), 0);
    assert_eq!(restricted_backend.gpu_lifecycle_stats().submitted, 0);
    assert_eq!(
        restricted_backend.geometry_residency().physical_mesh_bytes,
        0
    );
    assert_eq!(restricted_backend.geometry_residency().texture_bytes, 0);
    assert_eq!(restricted_backend.geometry_upload_snapshot().tickets, 0);
    assert_eq!(loader.cell_status(a).unwrap(), CellStatus::Active);
    assert_eq!(
        world.entity_count(),
        2,
        "refusal cannot unpublish the active cell"
    );
    eprintln!("F07 closure: {} operations -> {queue_limit}; {normal_closure} physical bytes -> {restricted}; no partial activation", closure.len());

    refused_loader
        .release_cell_gpu(
            b,
            &mut refused_world,
            &mut refused_manager,
            17,
            &mut restricted_backend,
        )
        .unwrap();
    loader
        .release_cell_gpu(a, &mut world, &mut manager, 17, &mut backend)
        .unwrap();
    backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
    restricted_backend
        .shutdown_gpu(Duration::from_secs(5))
        .unwrap();
    for loader in [&mut loader, &mut refused_loader] {
        assert!(loader
            .asset_loader_mut()
            .shutdown_streaming(Duration::from_secs(2)));
        let snapshot = loader.asset_loader().streaming_snapshot();
        assert_eq!(snapshot.reserved_bytes, 0);
        assert_eq!(snapshot.cpu_resident_bytes, 0);
        assert_eq!(loader.cell_lifetime_counts(), (0, 0));
    }
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires a real GPU; consumes two distinct 32 MiB cells"]
fn foundation_f02_dual_32mib_cells_cancel_ten_upload_queued_generations_then_draw_and_retire() {
    use engine_runtime::{
        render_asset_production::RuntimeRenderAssetProducer,
        render_contract::*,
        render_geometry::*,
        render_resource::RenderResourceManager,
        runtime_instance_loader::*,
        wgpu_backend::real::{GeometryUploadBudget, RealWgpuBackend},
        world::World,
    };
    fn activate(
        loader: &mut RuntimeInstanceLoader,
        ticket: CellTicket,
        world: &mut World,
        backend: &mut RealWgpuBackend,
        frame: &mut u64,
    ) {
        for _ in 0..32 {
            *frame += 1;
            let uploaded = backend.pump_geometry_uploads(*frame).unwrap();
            assert!(uploaded <= 16 * 1024 * 1024);
            assert!(backend.geometry_upload_snapshot().staging_bytes <= 128 * 1024 * 1024);
            assert!(backend.gpu_lifecycle_stats().retiring_bytes <= 256 * 1024 * 1024);
            if uploaded > 0 {
                backend
                    .wait_gpu_submission(
                        backend.last_gpu_submission().unwrap(),
                        Duration::from_secs(5),
                    )
                    .unwrap();
            }
            match loader.commit_cell_gpu(ticket, world, backend) {
                Ok(_) => return,
                Err(CellError::NotReady) => (),
                other => panic!("cell activation failed: {other:?}"),
            }
        }
        panic!("two real 16 MiB textures did not finish in bounded slices");
    }
    let fixture = super::streamed_cells::build_dual_32mib_cells();
    let mut loader = RuntimeInstanceLoader::from_package(&fixture.package);
    let mut world = World::new();
    let mut manager = RenderResourceManager::new();
    let mut backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
    // Frozen PC ceilings, no change to production defaults or promised budgets.
    backend
        .set_geometry_upload_budget(GeometryUploadBudget {
            resident_bytes: 2 * 1024 * 1024 * 1024,
            staging_bytes: 128 * 1024 * 1024,
            bytes_per_frame: 16 * 1024 * 1024,
            max_tickets: 128,
        })
        .unwrap();
    let mut frame = 0;
    let b = loader
        .request_cell(super::streamed_cells::request("b", 1, true), &world)
        .unwrap();
    super::prepare_streamed_cell(&mut loader, b, &world, &mut manager);
    loader.begin_cell_upload(b, &world, &mut backend).unwrap();
    activate(&mut loader, b, &mut world, &mut backend, &mut frame);
    assert_eq!(backend.geometry_residency().texture_bytes, 32 * 1024 * 1024);
    let mut final_a = None;
    let mut cancelled_generations = Vec::new();
    for iteration in 0..10 {
        let old = loader
            .request_cell(super::streamed_cells::request("a", 1, true), &world)
            .unwrap();
        super::prepare_streamed_cell(&mut loader, old, &world, &mut manager);
        loader.begin_cell_upload(old, &world, &mut backend).unwrap();
        assert_eq!(loader.cell_status(old).unwrap(), CellStatus::Uploading);
        assert_eq!(world.entity_count(), 2);
        loader
            .release_cell_gpu(old, &mut world, &mut manager, frame, &mut backend)
            .unwrap();
        let current = loader
            .request_cell(super::streamed_cells::request("a", 1, true), &world)
            .unwrap();
        assert_ne!(old, current);
        super::prepare_streamed_cell(&mut loader, current, &world, &mut manager);
        loader
            .begin_cell_upload(current, &world, &mut backend)
            .unwrap();
        assert!(matches!(
            loader.commit_cell_gpu(current, &mut world, &mut backend),
            Err(CellError::NotReady)
        ));
        activate(&mut loader, current, &mut world, &mut backend, &mut frame);
        assert_eq!(world.entity_count(), 4);
        assert!(matches!(
            loader.commit_cell_gpu(old, &mut world, &mut backend),
            Err(CellError::NotReady | CellError::StaleTicket)
        ));
        assert_eq!(backend.geometry_residency().texture_bytes, 64 * 1024 * 1024);
        cancelled_generations.push(old);
        if iteration == 9 {
            final_a = Some(current);
        } else {
            loader
                .release_cell_gpu(current, &mut world, &mut manager, frame, &mut backend)
                .unwrap();
            assert_eq!(world.entity_count(), 2);
        }
    }
    let ids = world.entity_ids();
    let prepared = ids
        .iter()
        .map(|id| {
            RuntimeRenderAssetProducer
                .prepare_mesh_renderer(
                    frame,
                    world.mesh_renderer3d(id).unwrap(),
                    loader.asset_loader_mut(),
                    &mut manager,
                )
                .unwrap()
        })
        .collect::<Vec<_>>();
    let instances = ids
        .iter()
        .zip(&prepared)
        .enumerate()
        .map(|(i, (id, prepared))| {
            let mut instance =
                super::geometry_fixture::instance(&prepared.geometry[0], i as u64 + 1, [0.; 3]);
            instance.world_transform = world.spatial_world(id).unwrap().world_matrix;
            instance
        })
        .collect::<Vec<_>>();
    let view = RenderView::perspective(frame + 1, 128, 128, 60., 0.1, 10.);
    let geometry = GeometryFrame::build(
        "main",
        [128, 128],
        &[GeometryViewInput {
            view: &view,
            pose: Default::default(),
        }],
        &instances,
        &manager,
        DiagnosticLevel::Summary,
    )
    .unwrap();
    let plan = engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(
        &super::geometry_fixture::graph(&geometry),
    );
    // The final cell slice used this logical frame's entire allowance. A
    // second consumer in that frame must reject, rather than exceed 16 MiB.
    assert_eq!(
        backend.geometry_upload_snapshot().frame_uploaded_bytes,
        16 * 1024 * 1024
    );
    // The actual Player consumer preflights the immutable frame. Capacity is
    // a wait there, not the fatal direct-draw error asserted below.
    assert!(!backend
        .prepare_surface_resources(&plan, [128, 128], None)
        .unwrap());
    assert_eq!(
        backend
            .render_plan_to_rgba_bytes(&plan, 128, 128)
            .unwrap_err(),
        "geometry.visual_upload_capacity"
    );
    frame += 1;
    assert_eq!(backend.pump_geometry_uploads(frame).unwrap(), 0);
    assert!(backend
        .prepare_surface_resources(&plan, [128, 128], None)
        .unwrap());
    let pixels = backend.render_plan_to_rgba_bytes(&plan, 128, 128).unwrap();
    assert!(backend.geometry_upload_snapshot().frame_uploaded_bytes <= 16 * 1024 * 1024);
    let red = pixels
        .chunks_exact(4)
        .filter(|p| p[0] > 120 && p[1] < 8 && p[2] < 8)
        .count();
    let blue = pixels
        .chunks_exact(4)
        .filter(|p| p[2] > 120 && p[1] < 8 && p[0] < 8)
        .count();
    assert!(
        red > 100 && blue > 100,
        "independent red/blue material reference failed: {red}/{blue}"
    );
    if let Some(dir) = std::env::var_os("S1_F02_EVIDENCE_DIR") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("dual-cell-128x128.rgba"), &pixels).unwrap();
        std::fs::write(dir.join("dual-cell-pixels.json"), serde_json::to_vec_pretty(&serde_json::json!({"width":128,"height":128,"redPixels":red,"bluePixels":blue,"cancelledUploadQueuedGenerations":cancelled_generations.len(),"actualTextureBytes":64*1024*1024,"formalQualified":false})).unwrap()).unwrap();
    }
    drop(plan);
    drop(geometry);
    drop(instances);
    drop(prepared);
    loader
        .release_cell_gpu(
            final_a.unwrap(),
            &mut world,
            &mut manager,
            frame + 1,
            &mut backend,
        )
        .unwrap();
    loader
        .release_cell_gpu(b, &mut world, &mut manager, frame + 1, &mut backend)
        .unwrap();
    backend
        .wait_gpu_submission(
            backend.last_gpu_submission().unwrap(),
            Duration::from_secs(5),
        )
        .unwrap();
    assert_eq!(backend.poll_gpu_completion().unwrap().retiring_bytes, 0);
    assert_eq!(backend.geometry_residency().texture_bytes, 0);
    assert_eq!(loader.cell_lifetime_counts(), (0, 0));
    assert_eq!(world.entity_count(), 0);
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(Duration::from_secs(2)));
    assert_eq!(
        loader
            .asset_loader()
            .streaming_snapshot()
            .cpu_resident_bytes,
        0
    );
    assert_eq!(loader.asset_loader().streaming_snapshot().reserved_bytes, 0);
}

impl ProjectRuntimeSession for Session {
    fn session_id(&self) -> &str {
        "foundation-faults"
    }
    fn handle_aui_actions(
        &mut self,
        _: ProjectRuntimeSessionContext<'_>,
        _: ProjectAuiActionBatch<'_>,
    ) -> ProjectRuntimeSessionOutput {
        ProjectRuntimeSessionOutput::no_op()
    }
    fn fixed_update(&mut self, _: ProjectRuntimeSessionContext<'_>) -> ProjectRuntimeSessionOutput {
        self.0
            .lock()
            .unwrap()
            .next
            .take()
            .map(ProjectRuntimeSessionOutput::applied)
            .unwrap_or_else(ProjectRuntimeSessionOutput::no_op)
    }
    fn accept_spawn_receipts(
        &mut self,
        receipts: BTreeMap<u64, (engine_runtime::ids::EntityId, u64)>,
    ) {
        self.0.lock().unwrap().receipts = receipts;
    }
}
#[test]
fn foundation_f01_burst_creates_1000_roots_requests_100_assets_and_destroys_half_next_tick() {
    let fixture = super::streamed_cells::build_burst_scene(10000);
    let mut world = load_scene_into_world(&fixture.package.active_scene)
        .value
        .unwrap();
    let before = world.entity_count();
    assert_eq!(before, 10001);
    let mut loader = RuntimeInstanceLoader::from_package(&fixture.package);
    // The generic loader defaults to 64 operations. F-01 uses the frozen PC
    // profile (256 requests); this does not change production defaults.
    let mut budget = loader.asset_loader().streaming_budget();
    assert_eq!(budget.max_operations, 64);
    budget.max_operations = 256;
    loader
        .asset_loader_mut()
        .set_streaming_budget(budget)
        .unwrap();
    let root = loader
        .asset_loader_mut()
        .load(&super::streamed_cells::reference("cell-a", "prefab"))
        .unwrap();
    let state = Arc::new(Mutex::new(State::default()));
    let mut host = EngineHostLoop::with_project_runtime_session(
        "main",
        Default::default(),
        Box::new(Session(state.clone())),
    );
    host.set_project_runtime_session_report_level(ProjectRuntimeSessionReportLevel::Summary);
    let tick = |host: &mut EngineHostLoop,
                world: &mut engine_runtime::world::World,
                loader: &mut RuntimeInstanceLoader| {
        let output = host.tick_with_runtime_context(
            EngineFrameInput::new(EngineHostMode::ExportedGame).with_fixed_step_count(1),
            world,
            RuntimeFrameContext {
                package: &fixture.package,
                instance_loader: loader,
                cell_resources: None,
                #[cfg(feature = "real-wgpu")]
                cell_backend: None,
            },
        );
        assert!(
            !output
                .project_runtime_session_report
                .as_ref()
                .unwrap()
                .terminal_fault,
            "{output:?}"
        );
    };
    let mut burst = ProjectRuntimeMutationBuffer::new();
    for i in 1..=1000 {
        burst.instantiate_spatial_prefab(
            i,
            project_runtime_sdk::ProjectRuntimeSpatialAssetRef {
                id: "cell-a".into(),
                asset_type: "prefab".into(),
                guid: Some("cell-a".into()),
                sub_asset: Some("root".into()),
            },
        );
    }
    state.lock().unwrap().next = Some(burst);
    let handles = (0..100)
        .map(|i| {
            loader
                .asset_loader_mut()
                .request_async(&super::streamed_cells::reference(
                    &format!("cell-burst-{i}"),
                    "prefab",
                ))
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        loader.asset_loader().streaming_snapshot().pending_tickets,
        100
    );
    tick(&mut host, &mut world, &mut loader);
    assert_eq!(world.entity_count(), before + 1000);
    let receipts = state.lock().unwrap().receipts.clone();
    assert_eq!(receipts.len(), 1000);
    let mut remove = ProjectRuntimeMutationBuffer::new();
    for (id, generation) in receipts.values().take(500) {
        remove.despawn_spatial_prefab(id.clone(), *generation);
    }
    state.lock().unwrap().next = Some(remove);
    tick(&mut host, &mut world, &mut loader);
    assert_eq!(world.entity_count(), before + 500);
    for (id, generation) in receipts.values().take(500) {
        assert_eq!(world.entity_generation(id), None);
        assert_ne!(world.entity_generation(id), Some(*generation));
    }
    for (id, generation) in receipts.values().skip(500) {
        assert_eq!(world.entity_generation(id), Some(*generation));
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        loader.asset_loader_mut().poll_async();
        if handles.iter().all(|h| {
            loader.asset_loader().get_handle_state(h) != Some(RuntimeAssetLoadState::Loading)
        }) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "100 resource burst must terminate"
        );
        std::thread::yield_now();
    }
    for handle in &handles {
        assert_eq!(
            loader.asset_loader().get_handle_state(handle),
            Some(RuntimeAssetLoadState::Ready)
        );
        loader.asset_loader_mut().release(handle).unwrap();
    }
    let mut remove = ProjectRuntimeMutationBuffer::new();
    for (id, generation) in receipts.values().skip(500) {
        remove.despawn_spatial_prefab(id.clone(), *generation);
    }
    state.lock().unwrap().next = Some(remove);
    tick(&mut host, &mut world, &mut loader);
    assert_eq!(world.entity_count(), before);
    loader.asset_loader_mut().release(&root).unwrap();
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(Duration::from_secs(10)));
    assert_eq!(
        loader.asset_loader().streaming_snapshot().pending_tickets,
        0
    );
    assert_eq!(
        loader
            .asset_loader()
            .streaming_snapshot()
            .cpu_resident_bytes,
        0
    );
}

#[test]
fn foundation_f03_missing_chunk_and_corrupt_hash_are_terminal_without_world_publication() {
    use engine_runtime::{
        runtime_asset_loader::StreamingError, runtime_instance_loader::CellError,
    };
    for missing in [true, false] {
        let fixture = super::streamed_cells::build(false);
        let record = fixture
            .package
            .runtime_asset_index
            .record_by_guid("cell-a")
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
        if missing {
            std::fs::remove_file(path).unwrap();
        } else {
            let mut bytes = std::fs::read(&path).unwrap();
            bytes[0] ^= 1;
            std::fs::write(path, bytes).unwrap();
        }
        let mut loader = RuntimeInstanceLoader::from_package(&fixture.package);
        let world = engine_runtime::world::World::new();
        let mut resources = engine_runtime::render_resource::RenderResourceManager::new();
        let cell = loader
            .request_cell(super::streamed_cells::request("a", 1, false), &world)
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match loader.prepare_cell(cell, &world, 1, &mut resources) {
                Err(CellError::NotReady) => {
                    assert!(Instant::now() < deadline);
                    std::thread::yield_now();
                }
                Err(CellError::Asset(StreamingError::ReadFailed(_))) if missing => break,
                Err(CellError::Asset(StreamingError::IntegrityMismatch)) if !missing => break,
                other => panic!("wrong first error, missing={missing}: {other:?}"),
            }
        }
        assert_eq!(world.entity_count(), 0);
        assert!(loader
            .asset_loader_mut()
            .shutdown_streaming(Duration::from_secs(2)));
        assert_eq!(
            loader
                .asset_loader()
                .streaming_snapshot()
                .cpu_resident_bytes,
            0
        );
    }
}

// Actual first-boundary failures, not a claim for the complete static/dual-cell
// BM11 schedule. Rehashing malformed input distinguishes decoding from integrity.
#[test]
fn foundation_f03_truncated_decode_missing_dependency_and_layout_never_publish() {
    use engine_runtime::{
        canonical_digest::sha256_prefixed, runtime_asset::RuntimeAssetIndex,
        runtime_asset_loader::StreamingError, runtime_instance_loader::CellError, world::World,
    };
    for case in ["truncated", "decode", "dependency", "layout"] {
        let mut fixture = super::streamed_cells::build(false);
        let mut record = fixture
            .package
            .runtime_asset_index
            .record_by_guid("cell-a")
            .unwrap()
            .clone();
        let mut cooked = fixture
            .package
            .runtime_asset_index
            .cooked_asset(&record.cooked_asset_id)
            .unwrap()
            .clone();
        let path = fixture
            .package
            .package_dir
            .join(cooked.path.as_ref().unwrap());
        match case {
            "truncated" => {
                let mut bytes = std::fs::read(&path).unwrap();
                bytes.truncate(bytes.len() / 2);
                std::fs::write(path, bytes).unwrap();
            }
            "decode" => {
                let bytes = b"{";
                std::fs::write(path, bytes).unwrap();
                record.hash = Some(sha256_prefixed(bytes));
                cooked.hash = record.hash.clone();
                record.size = Some(bytes.len() as u64);
                cooked.size = record.size;
            }
            "dependency" => record
                .dependencies
                .push("absent-required-dependency".into()),
            "layout" => cooked.compression = Some("unsupported-test-codec".into()),
            _ => unreachable!(),
        }
        fixture
            .package
            .runtime_asset_index
            .merge_patch(RuntimeAssetIndex::new(vec![record], vec![cooked]));
        let mut loader = RuntimeInstanceLoader::from_package(&fixture.package);
        let world = World::new();
        let mut resources = engine_runtime::render_resource::RenderResourceManager::new();
        let requested = loader.request_cell(super::streamed_cells::request("a", 1, false), &world);
        let error = match requested {
            Err(error) => error,
            Ok(ticket) => {
                let deadline = Instant::now() + Duration::from_secs(5);
                loop {
                    match loader.prepare_cell(ticket, &world, 1, &mut resources) {
                        Err(CellError::NotReady) => {
                            assert!(Instant::now() < deadline, "{case} must terminate");
                            std::thread::yield_now();
                        }
                        Err(error) => break error,
                        Ok(_) => panic!("{case} must not prepare malformed content"),
                    }
                }
            }
        };
        match (case, &error) {
            ("truncated", CellError::Asset(StreamingError::IntegrityMismatch))
            | ("decode", CellError::Asset(StreamingError::DecodeFailed(_)))
            | ("dependency", CellError::Asset(StreamingError::MissingAsset))
            | ("layout", CellError::Asset(StreamingError::UnsupportedCookedLayout)) => (),
            _ => panic!("wrong first boundary for {case}: {error:?}"),
        }
        assert_eq!(world.entity_count(), 0, "{case}");
        assert_eq!(loader.cell_lifetime_counts().0, 0, "{case}");
        assert!(loader
            .asset_loader_mut()
            .shutdown_streaming(Duration::from_secs(2)));
        let snapshot = loader.asset_loader().streaming_snapshot();
        assert_eq!(snapshot.pending_operations, 0, "{case}");
        assert_eq!(snapshot.pending_tickets, 0, "{case}");
        assert_eq!(snapshot.reserved_bytes, 0, "{case}");
        assert_eq!(snapshot.cpu_resident_bytes, 0, "{case}");
    }
}

#[test]
fn foundation_f03_supported_bc3_stream_truncation_fails_before_cell_publication() {
    use engine_runtime::{
        canonical_digest::sha256_prefixed,
        runtime_asset::RuntimeAssetIndex,
        runtime_asset_loader::StreamingError,
        runtime_instance_loader::{CellError, CellStatus},
        runtime_package::CookedTextureAsset,
        world::World,
    };
    for truncated in [false, true] {
        let mut fixture = super::streamed_cells::build(true);
        let mut record = fixture
            .package
            .runtime_asset_index
            .record_by_guid("shared-texture")
            .unwrap()
            .clone();
        let mut cooked = fixture
            .package
            .runtime_asset_index
            .cooked_asset(&record.cooked_asset_id)
            .unwrap()
            .clone();
        let metadata_path = fixture
            .package
            .package_dir
            .join(cooked.path.as_ref().unwrap());
        let mut metadata: CookedTextureAsset =
            serde_json::from_slice(&std::fs::read(&metadata_path).unwrap()).unwrap();
        metadata.format = "bc3RgbaUnormSrgb".into();
        metadata.mip_count = 3; // 4x4, 2x2, 1x1: three actual 16-byte BC3 blocks.
        let blocks = vec![0xab; 48];
        metadata.byte_length = blocks.len();
        metadata.source_hash = sha256_prefixed(&blocks);
        let metadata_bytes = serde_json::to_vec(&metadata).unwrap();
        let mut complete = metadata_bytes.clone();
        complete.extend_from_slice(&blocks);
        record.hash = Some(sha256_prefixed(&complete));
        cooked.hash = record.hash.clone();
        record.size = Some(metadata_bytes.len() as u64);
        cooked.size = record.size;
        std::fs::write(&metadata_path, metadata_bytes).unwrap();
        std::fs::write(
            fixture.package.package_dir.join(&metadata.pixel_data_path),
            &blocks[..if truncated { 24 } else { blocks.len() }],
        )
        .unwrap();
        fixture
            .package
            .runtime_asset_index
            .merge_patch(RuntimeAssetIndex::new(vec![record], vec![cooked]));
        let mut loader = RuntimeInstanceLoader::from_package(&fixture.package);
        let mut world = World::new();
        let mut resources = engine_runtime::render_resource::RenderResourceManager::new();
        let ticket = loader
            .request_cell(super::streamed_cells::request("a", 1, true), &world)
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match loader.prepare_cell(ticket, &world, 1, &mut resources) {
                Err(CellError::NotReady) => {
                    assert!(Instant::now() < deadline, "BC3 truncation must terminate");
                    std::thread::yield_now();
                }
                Ok(CellStatus::Prepared) if !truncated => break,
                Err(CellError::Asset(StreamingError::IntegrityMismatch)) if truncated => break,
                other => panic!("wrong BC3 first boundary, truncated={truncated}: {other:?}"),
            }
        }
        assert_eq!(
            world.entity_count(),
            0,
            "Prepare cannot publish a partial cell"
        );
        if !truncated {
            assert_eq!(loader.cell_status(ticket).unwrap(), CellStatus::Prepared);
            loader
                .release_cell(ticket, &mut world, &mut resources, 2)
                .unwrap();
        } else {
            assert_eq!(loader.cell_lifetime_counts().0, 0);
        }
        assert!(loader
            .asset_loader_mut()
            .shutdown_streaming(Duration::from_secs(2)));
        let snapshot = loader.asset_loader().streaming_snapshot();
        assert_eq!(snapshot.pending_operations, 0);
        assert_eq!(snapshot.pending_tickets, 0);
        assert_eq!(snapshot.reserved_bytes, 0);
        assert_eq!(snapshot.cpu_resident_bytes, 0);
    }
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "real GPU; F11 foundation ACK waits and rejection, not device recovery qualification"]
fn foundation_f11_pending_and_invalid_required_acks_never_publish_partial_cells() {
    use engine_runtime::{
        components::Hierarchy,
        render_resource::RenderResourceManager,
        runtime_instance_loader::{CellDomain, CellError, CellStatus},
        wgpu_backend::real::{GeometryUploadBudget, RealWgpuBackend},
        world::World,
    };
    // Each case uses the actual package worker and a 32 MiB Render closure.
    // The small slice limit deliberately leaves a real upload incomplete.
    for fault in [
        "foreign-renderer",
        "wrong-world",
        "world-id-collision",
        "destroyed-device",
    ] {
        let fixture = super::streamed_cells::build_dual_32mib_cells();
        let root = fixture.root.clone();
        let mut loader = RuntimeInstanceLoader::from_package(&fixture.package);
        let mut world = World::new();
        let mut manager = RenderResourceManager::with_safe_frame_delay(0);
        for domain in [CellDomain::World, CellDomain::Asset] {
            let mut request = super::streamed_cells::request("a", 1, true);
            request.required_domains.remove(&domain);
            assert_eq!(
                loader.request_cell(request, &world),
                Err(CellError::InvalidRequest)
            );
        }
        assert_eq!(
            loader
                .asset_loader()
                .streaming_snapshot()
                .dispatched_operations,
            0
        );
        let ticket = loader
            .request_cell(super::streamed_cells::request("a", 1, true), &world)
            .unwrap();
        assert_eq!(loader.cell_status(ticket).unwrap(), CellStatus::Loading);
        assert!(matches!(
            loader.commit_cell(ticket, &mut world),
            Err(CellError::NotReady)
        ));
        assert_eq!(world.entity_count(), 0);
        super::prepare_streamed_cell(&mut loader, ticket, &world, &mut manager);
        assert_eq!(loader.cell_status(ticket).unwrap(), CellStatus::Prepared);
        assert!(matches!(
            loader.commit_cell(ticket, &mut world),
            Err(CellError::NotReady)
        ));
        assert_eq!(loader.cell_activation_epoch(ticket).unwrap(), None);
        let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        let budget = GeometryUploadBudget {
            resident_bytes: 128 * 1024 * 1024,
            staging_bytes: 4 * 1024 * 1024,
            bytes_per_frame: 1024 * 1024,
            max_tickets: 4,
        };
        backend.set_geometry_upload_budget(budget).unwrap();
        loader
            .begin_cell_upload(ticket, &world, &mut backend)
            .unwrap();
        assert!(matches!(
            loader.commit_cell_gpu(ticket, &mut world, &mut backend),
            Err(CellError::NotReady)
        ));
        let first = backend.pump_geometry_uploads(1).unwrap();
        assert!(first > 0 && first <= budget.bytes_per_frame);
        // Even if this first slice finishes immediately, the whole 32 MiB
        // closure is still incomplete. A CPU frame/fence is never its ACK.
        assert!(matches!(
            loader.commit_cell_gpu(ticket, &mut world, &mut backend),
            Err(CellError::NotReady)
        ));
        assert_eq!(loader.cell_status(ticket).unwrap(), CellStatus::Uploading);
        assert_eq!(world.entity_count(), 0);
        assert_eq!(loader.cell_activation_epoch(ticket).unwrap(), None);

        if fault == "foreign-renderer" {
            let mut foreign = RealWgpuBackend::new_offscreen(64, 64).unwrap();
            assert_ne!(
                backend.gpu_lifecycle_stats().device_epoch,
                foreign.gpu_lifecycle_stats().device_epoch
            );
            assert_eq!(
                loader
                    .commit_cell_gpu(ticket, &mut world, &mut foreign)
                    .unwrap_err(),
                CellError::Render("geometry.stale_upload_ticket".into())
            );
            assert_eq!(foreign.gpu_lifecycle_stats().submitted, 0);
            foreign.shutdown_gpu(Duration::from_secs(5)).unwrap();
        } else if fault == "wrong-world" {
            let mut foreign = World::new();
            assert_eq!(
                loader
                    .commit_cell_gpu(ticket, &mut foreign, &mut backend)
                    .unwrap_err(),
                CellError::WrongWorld
            );
            assert_eq!(foreign.entity_count(), 0);
        } else if fault == "destroyed-device" {
            // Explicit destruction is a real epoch teardown, not an injected
            // success callback or a claim of spontaneous loss/recovery.
            backend.abandon_device_epoch();
            assert_eq!(
                loader
                    .commit_cell_gpu(ticket, &mut world, &mut backend)
                    .unwrap_err(),
                CellError::Render("device abandoned".into())
            );
            assert!(backend.gpu_lifecycle_stats().abandoned);
            assert_eq!(backend.geometry_upload_snapshot().tickets, 0);
        }
        assert_eq!(world.entity_count(), 0, "{fault} published a partial cell");
        assert_eq!(loader.cell_activation_epoch(ticket).unwrap(), None);
        assert_eq!(loader.cell_status(ticket).unwrap(), CellStatus::Uploading);

        let mut sentinel = None;
        if fault != "destroyed-device" {
            // Consume all actual submissions, retaining the original request
            // after the wrong owner/World tried to ACK it.
            let mut ready = false;
            for frame in 2..=80 {
                let uploaded = backend.pump_geometry_uploads(frame).unwrap();
                assert!(uploaded <= budget.bytes_per_frame);
                if let Some(token) = backend.last_gpu_submission() {
                    backend
                        .wait_gpu_submission(token, Duration::from_secs(5))
                        .unwrap();
                }
                if backend.geometry_upload_snapshot().pending_resources == 0 {
                    ready = true;
                    break;
                }
                assert_eq!(world.entity_count(), 0);
            }
            assert!(ready, "{fault}: bounded real upload did not complete");
            if fault == "world-id-collision" {
                let id = engine_runtime::ids::SourceEntityId::from(format!(
                    "instance:{}:node",
                    ticket.instance_id()
                ));
                let runtime = world
                    .try_spawn_entity(
                        id.clone(),
                        "sentinel",
                        "actor",
                        true,
                        Hierarchy {
                            parent_id: None,
                            sibling_order: 0,
                        },
                    )
                    .unwrap();
                assert!(matches!(
                    loader.commit_cell_gpu(ticket, &mut world, &mut backend),
                    Err(CellError::Hydration(_))
                ));
                assert_eq!(world.entity_count(), 1);
                assert_eq!(world.runtime_id_for_source(&id), Some(runtime));
                assert_eq!(loader.cell_activation_epoch(ticket).unwrap(), None);
                sentinel = Some((id, runtime));
            } else {
                loader
                    .commit_cell_gpu(ticket, &mut world, &mut backend)
                    .unwrap();
                assert_eq!(loader.cell_status(ticket).unwrap(), CellStatus::Active);
                assert_eq!(world.entity_count(), 2);
                assert_eq!(loader.cell_activation_epoch(ticket).unwrap(), Some(1));
            }
        }
        loader
            .release_cell_gpu(ticket, &mut world, &mut manager, 81, &mut backend)
            .unwrap();
        backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
        assert_eq!(backend.geometry_upload_snapshot().tickets, 0);
        assert_eq!(backend.geometry_residency().texture_bytes, 0);
        assert_eq!(backend.gpu_lifecycle_stats().retiring_bytes, 0);
        assert_eq!(manager.end_frame(1000).resident_bytes, 0);
        if let Some((id, runtime)) = sentinel {
            assert_eq!(
                world.entity_count(),
                1,
                "release deleted a foreign World entity"
            );
            assert_eq!(world.runtime_id_for_source(&id), Some(runtime));
            world.try_despawn_entity(&id).unwrap();
        }
        assert_eq!(world.entity_count(), 0);
        assert!(loader
            .asset_loader_mut()
            .shutdown_streaming(Duration::from_secs(2)));
        assert_eq!(loader.asset_loader().streaming_snapshot().reserved_bytes, 0);
        assert_eq!(
            loader
                .asset_loader()
                .streaming_snapshot()
                .cpu_resident_bytes,
            0
        );
        assert_eq!(loader.cell_lifetime_counts(), (0, 0));
        drop(loader);
        drop(fixture);
        assert!(
            !root.exists(),
            "owned F11 fixture retained: {}",
            root.display()
        );
        eprintln!("F11 {fault}: real pending ACK refused, no partial publication, original owners released");
    }
}
