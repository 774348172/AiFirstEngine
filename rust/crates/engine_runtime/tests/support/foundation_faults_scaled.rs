//! Original F03 failures beside a genuinely active 32 MiB cell.
//! This is a foundation consumer, not a static BM03 or performance qualification.
use super::super::{project_cell_session as session, streamed_cells};
use super::{Lifecycle, MIB};
use engine_runtime::{
    canonical_digest::sha256_prefixed,
    render_resource::RenderResourceManager,
    runtime_asset::RuntimeAssetIndex,
    runtime_asset_loader::StreamingError,
    runtime_instance_loader::{CellError, CellStatus, CellTicket, RuntimeInstanceLoader},
    runtime_package::CookedTextureAsset,
    wgpu_backend::real::{GeometryUploadBudget, RealWgpuBackend},
    world::World,
};
use project_runtime_sdk::{ProjectRuntimeCellIntent as Intent, ProjectRuntimeCellStatus as Status};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

fn inject(test: &mut Lifecycle, case: &str) {
    let asset = if case.starts_with("bc3") {
        "texture-a-0"
    } else {
        "cell-a"
    };
    let mut record = test
        .fixture
        .package
        .runtime_asset_index
        .record_by_guid(asset)
        .unwrap()
        .clone();
    let mut cooked = test
        .fixture
        .package
        .runtime_asset_index
        .cooked_asset(&record.cooked_asset_id)
        .unwrap()
        .clone();
    let path = test
        .fixture
        .package
        .package_dir
        .join(cooked.path.as_ref().unwrap());
    match case {
        "missing" => std::fs::remove_file(&path).unwrap(),
        "hash" => {
            let mut bytes = std::fs::read(&path).unwrap();
            bytes[0] ^= 1;
            std::fs::write(&path, bytes).unwrap();
        }
        "decode" => {
            let bytes = b"{";
            std::fs::write(&path, bytes).unwrap();
            record.hash = Some(sha256_prefixed(bytes));
            record.size = Some(bytes.len() as u64);
            cooked.hash = record.hash.clone();
            cooked.size = record.size;
        }
        "dependency" => record
            .dependencies
            .push("absent-required-dependency".into()),
        "layout" => cooked.compression = Some("unsupported-test-codec".into()),
        "bc3-valid" | "bc3-truncated" => {
            let mut metadata: CookedTextureAsset =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            // A complete 13-mip 1592x7900 BC3 chain is exactly 16 MiB.
            // The other 2048² RGBA texture preserves the 32 MiB cell.
            let red_block = [255, 255, 0, 0, 0, 0, 0, 0, 0, 248, 0, 248, 0, 0, 0, 0];
            let blocks = red_block.repeat(MIB as usize);
            assert_eq!(blocks.len(), 16 * MIB as usize);
            metadata.width = 1592;
            metadata.height = 7900;
            metadata.format = "bc3RgbaUnormSrgb".into();
            metadata.mip_count = 13;
            assert_eq!(
                engine_runtime::runtime_texture::texture_mip_byte_len(
                    &metadata.format,
                    metadata.width,
                    metadata.height,
                    metadata.mip_count,
                ),
                Some(blocks.len())
            );
            metadata.byte_length = blocks.len();
            metadata.source_hash = sha256_prefixed(&blocks);
            let bytes = serde_json::to_vec(&metadata).unwrap();
            let mut complete = bytes.clone();
            complete.extend_from_slice(&blocks);
            record.hash = Some(sha256_prefixed(&complete));
            record.size = Some(bytes.len() as u64);
            cooked.hash = record.hash.clone();
            cooked.size = record.size;
            std::fs::write(&path, bytes).unwrap();
            let length = if case == "bc3-truncated" {
                blocks.len() / 2
            } else {
                blocks.len()
            };
            std::fs::write(
                test.fixture
                    .package
                    .package_dir
                    .join(&metadata.pixel_data_path),
                &blocks[..length],
            )
            .unwrap();
        }
        _ => unreachable!(),
    }
    let patch = RuntimeAssetIndex::new(vec![record], vec![cooked]);
    test.fixture
        .package
        .runtime_asset_index
        .merge_patch(patch.clone());
    test.loader.asset_loader_mut().mount_patch_index(patch);
}

fn reject_a(test: &mut Lifecycle, case: &str) -> Option<CellTicket> {
    let mut scratch = RenderResourceManager::new();
    // Mounting the fault patch advances the loader epoch. Match the ordinary
    // session's current-epoch request so this reaches the intended asset fault.
    let revision = test.loader.asset_loader().asset_revision_epoch();
    let requested = test
        .loader
        .request_cell(streamed_cells::request("a", revision, true), &test.world);
    let ticket = requested.as_ref().ok().copied();
    let error = match requested {
        Err(error) => error,
        Ok(ticket) => {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                match test
                    .loader
                    .prepare_cell(ticket, &test.world, test.frame, &mut scratch)
                {
                    Err(CellError::NotReady) => {
                        assert!(Instant::now() < deadline, "{case} did not terminate");
                        std::thread::yield_now();
                    }
                    Err(error) => break error,
                    Ok(status) => panic!("{case} unexpectedly prepared: {status:?}"),
                }
            }
        }
    };
    match (case, &error) {
        ("missing", CellError::Asset(StreamingError::ReadFailed(_)))
        | ("hash" | "bc3-truncated", CellError::Asset(StreamingError::IntegrityMismatch))
        | ("decode", CellError::Asset(StreamingError::DecodeFailed(_)))
        | ("dependency", CellError::Asset(StreamingError::MissingAsset))
        | ("layout", CellError::Asset(StreamingError::UnsupportedCookedLayout)) => (),
        _ => panic!("wrong F03 first boundary, {case}: {error:?}"),
    }
    eprintln!("F03 {case}: {error:?}; active B remains 32 MiB");
    if let Some(ticket) = ticket {
        test.loader
            .release_cell(ticket, &mut test.world, &mut scratch, test.frame)
            .unwrap();
    }
    let resources = scratch.end_frame(test.frame + 10);
    assert_eq!(resources.resident_bytes, 0);
    assert_eq!(resources.pending_release_count, 0);
    ticket
}

#[test]
#[ignore = "real GPU; original dual32MiB F03 failure composition"]
fn foundation_f03_dual_32mib_failures_preserve_active_cell_pixels_and_retire() {
    for case in [
        "bc3-valid",
        "missing",
        "hash",
        "decode",
        "dependency",
        "layout",
        "bc3-truncated",
    ] {
        let mut test = Lifecycle::new();
        let root = test.fixture.root.clone();
        if let Some(dir) = std::env::var_os("S1_F03_EVIDENCE_DIR") {
            let directory = PathBuf::from(dir).join(case);
            std::fs::create_dir_all(&directory).unwrap();
            test.evidence = Some(directory);
        }
        inject(&mut test, case);
        let before = test.demand(vec![session::load(1, "b", true)], 1, Status::Active, 1);
        test.pixels(&before, false, true, "b-before");
        let entities = test.loader.project_cell_snapshot(&test.world).entities;
        if case == "bc3-valid" {
            let both = test.demand(vec![session::load(2, "a", true)], 2, Status::Active, 2);
            test.pixels(&both, true, true, "both");
            assert_eq!(test.backend.geometry_residency().texture_bytes, 64 * MIB);
            let after = test.demand(
                vec![Intent::Release { request_id: 2 }],
                2,
                Status::Released,
                1,
            );
            test.pixels(&after, false, true, "b-after");
            assert_eq!(before, after);
        } else {
            let _ = reject_a(&mut test, case);
            assert_eq!(test.world.entity_count(), 3, "{case} partially published A");
            assert_eq!(test.loader.cell_lifetime_counts(), (1, 1));
            assert_eq!(
                test.loader.project_cell_snapshot(&test.world).entities,
                entities
            );
            let after = test.step().expect("B has no pending upload");
            test.pixels(&after, false, true, "b-after");
            assert_eq!(before, after, "{case} changed the active cell's pixels");
            assert_eq!(test.backend.geometry_residency().texture_bytes, 32 * MIB);
        }
        let empty = test.demand(
            vec![Intent::Release { request_id: 1 }],
            1,
            Status::Released,
            0,
        );
        test.pixels(&empty, false, false, "all-unloaded");
        assert!(test
            .loader
            .asset_loader_mut()
            .shutdown_streaming(Duration::from_secs(5)));
        test.backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
        let cpu = test.loader.asset_loader().streaming_snapshot();
        assert_eq!(cpu.pending_operations, 0);
        assert_eq!(cpu.pending_tickets, 0);
        assert_eq!(cpu.reserved_bytes, 0);
        assert_eq!(cpu.cpu_resident_bytes, 0);
        assert_eq!(test.backend.geometry_residency().meshes, 0);
        assert_eq!(test.backend.geometry_residency().textures, 0);
        assert_eq!(test.backend.geometry_upload_snapshot().tickets, 0);
        assert_eq!(test.backend.gpu_lifecycle_stats().in_flight, 0);
        assert_eq!(test.backend.gpu_lifecycle_stats().retiring_bytes, 0);
        drop(test);
        assert!(!root.exists(), "owned fixture was not cleaned: {root:?}");
    }
}

#[test]
#[ignore = "real GPU; exact dual32MiB read/decode recovery consumer"]
fn foundation_dual_32mib_read_decode_failures_retry_after_input_recovery() {
    for case in ["missing", "decode"] {
        let mut test = Lifecycle::new();
        let root = test.fixture.root.clone();
        if let Some(dir) = std::env::var_os("S1_RETRY_EVIDENCE_DIR") {
            let directory = PathBuf::from(dir).join(case);
            std::fs::create_dir_all(&directory).unwrap();
            test.evidence = Some(directory);
        }
        let original_record = test
            .fixture
            .package
            .runtime_asset_index
            .record_by_guid("cell-a")
            .unwrap()
            .clone();
        let original_cooked = test
            .fixture
            .package
            .runtime_asset_index
            .cooked_asset(&original_record.cooked_asset_id)
            .unwrap()
            .clone();
        let path = test
            .fixture
            .package
            .package_dir
            .join(original_cooked.path.as_ref().unwrap());
        let original_bytes = std::fs::read(&path).unwrap();
        inject(&mut test, case);
        let before = test.demand(vec![session::load(1, "b", true)], 1, Status::Active, 1);
        test.pixels(&before, false, true, "b-before");
        let original_b = test.loader.project_cell_snapshot(&test.world).entities;
        let failed = reject_a(&mut test, case).expect("the read/decode request was allocated");
        assert_eq!(test.world.entity_count(), 3, "A must not partially publish");
        assert_eq!(test.loader.cell_status(failed), Ok(CellStatus::Released));
        let rejected = test.step().unwrap();
        test.pixels(&rejected, false, true, "b-after-failure");
        assert_eq!(before, rejected);
        assert_eq!(
            test.loader.project_cell_snapshot(&test.world).entities,
            original_b
        );

        // A changed cooked index invalidates all old-epoch requests. Retire B
        // explicitly before restoring that index; file-only repair needs no epoch.
        let b_key = if case == "decode" {
            let empty = test.demand(
                vec![Intent::Release { request_id: 1 }],
                1,
                Status::Released,
                0,
            );
            test.pixels(&empty, false, false, "before-index-recovery");
            3
        } else {
            1
        };
        std::fs::write(&path, original_bytes).unwrap();
        if case == "decode" {
            let patch = RuntimeAssetIndex::new(vec![original_record], vec![original_cooked]);
            test.fixture
                .package
                .runtime_asset_index
                .merge_patch(patch.clone());
            test.loader.asset_loader_mut().mount_patch_index(patch);
            let reloaded = test.demand(
                vec![session::load(b_key, "b", true)],
                b_key,
                Status::Active,
                1,
            );
            test.pixels(&reloaded, false, true, "b-after-index-recovery");
            assert_eq!(before, reloaded);
            for (id, generation) in original_b.values() {
                assert_ne!(test.world.entity_generation(id), Some(*generation));
            }
        }
        let mut scratch = RenderResourceManager::new();
        assert!(test
            .loader
            .prepare_cell(failed, &test.world, test.frame, &mut scratch)
            .is_err());
        assert!(test.loader.commit_cell(failed, &mut test.world).is_err());
        assert!(test.loader.prefab_instance(failed.instance_id()).is_none());
        let both = test.demand(vec![session::load(4, "a", true)], 4, Status::Active, 2);
        test.pixels(&both, true, true, "both-after-retry");
        let after = test.demand(
            vec![Intent::Release { request_id: 4 }],
            4,
            Status::Released,
            1,
        );
        test.pixels(&after, false, true, "b-after-retry");
        assert_eq!(before, after);
        if case == "missing" {
            assert_eq!(
                test.loader.project_cell_snapshot(&test.world).entities,
                original_b
            );
        }
        let empty = test.demand(
            vec![Intent::Release { request_id: b_key }],
            b_key,
            Status::Released,
            0,
        );
        test.pixels(&empty, false, false, "all-unloaded");
        assert!(test
            .loader
            .asset_loader_mut()
            .shutdown_streaming(Duration::from_secs(5)));
        test.backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
        let cpu = test.loader.asset_loader().streaming_snapshot();
        assert_eq!((cpu.pending_operations, cpu.pending_tickets), (0, 0));
        assert_eq!((cpu.reserved_bytes, cpu.cpu_resident_bytes), (0, 0));
        assert_eq!(test.loader.cell_lifetime_counts(), (0, 0));
        assert_eq!(test.world.entity_count(), 1);
        assert_eq!(test.backend.geometry_residency().texture_bytes, 0);
        assert_eq!(test.backend.geometry_residency().physical_mesh_bytes, 0);
        assert_eq!(test.backend.geometry_upload_snapshot().tickets, 0);
        assert_eq!(test.backend.geometry_upload_snapshot().staging_bytes, 0);
        assert_eq!(test.backend.gpu_lifecycle_stats().in_flight, 0);
        assert_eq!(test.backend.gpu_lifecycle_stats().retiring_bytes, 0);
        eprintln!("recovered {case}: red/blue pixels restored, old request rejected, all resources drained");
        drop(test);
        assert!(!root.exists(), "owned fixture was not cleaned: {root:?}");
    }
}

#[test]
#[ignore = "real GPU; exact dual32MiB resident-admission retry consumer"]
fn foundation_dual_32mib_gpu_admission_retries_the_same_prepared_ticket() {
    let fixture = streamed_cells::build_dual_32mib_cells();
    let root = fixture.root.clone();
    let mut loader = RuntimeInstanceLoader::from_package(&fixture.package);
    let mut world = World::new();
    let mut manager = RenderResourceManager::new();
    let mut backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
    let normal = GeometryUploadBudget {
        resident_bytes: 48 * MIB,
        staging_bytes: 128 * MIB as usize,
        bytes_per_frame: 16 * MIB as usize,
        max_tickets: 128,
    };
    backend.set_geometry_upload_budget(normal).unwrap();
    let mut frame = 0;
    let b = loader
        .request_cell(streamed_cells::request("b", 1, true), &world)
        .unwrap();
    super::super::prepare_streamed_cell(&mut loader, b, &world, &mut manager);
    loader.begin_cell_upload(b, &world, &mut backend).unwrap();
    finish_upload(&mut loader, b, &mut world, &mut backend, &mut frame);
    let b_entities: Vec<_> = world.entity_ids().into_iter().cloned().collect();
    assert_eq!(backend.geometry_residency().texture_bytes, 32 * MIB);
    let a = loader
        .request_cell(streamed_cells::request("a", 1, true), &world)
        .unwrap();
    super::super::prepare_streamed_cell(&mut loader, a, &world, &mut manager);
    let before = backend.geometry_residency();
    let submitted = backend.gpu_lifecycle_stats().submitted;
    assert_eq!(
        loader.begin_cell_upload(a, &world, &mut backend),
        Err(CellError::Render("geometry.upload_resident_budget".into()))
    );
    assert_eq!(loader.cell_status(a), Ok(CellStatus::Prepared));
    assert!(matches!(
        loader.commit_cell_gpu(a, &mut world, &mut backend),
        Err(CellError::NotReady)
    ));
    assert_eq!(world.entity_ids(), b_entities.iter().collect::<Vec<_>>());
    assert_eq!(loader.cell_status(b), Ok(CellStatus::Active));
    assert_eq!(
        backend.geometry_residency().texture_bytes,
        before.texture_bytes
    );
    assert_eq!(
        backend.geometry_residency().physical_mesh_bytes,
        before.physical_mesh_bytes
    );
    assert_eq!(backend.geometry_upload_snapshot().tickets, 1);
    assert_eq!(backend.geometry_upload_snapshot().staging_bytes, 0);
    assert_eq!(backend.gpu_lifecycle_stats().submitted, submitted);
    // The backend correctly forbids changing upload budgets with live tickets.
    // Recover capacity through the ordinary retirement path, keeping the same
    // 48 MiB limit and A's original Prepared ticket throughout the retry.
    loader
        .release_cell_gpu(b, &mut world, &mut manager, frame + 1, &mut backend)
        .unwrap();
    backend.poll_gpu_completion().unwrap();
    assert_eq!(backend.geometry_residency().texture_bytes, 0);
    assert_eq!(backend.geometry_upload_snapshot().tickets, 0);
    assert_eq!(loader.cell_status(a), Ok(CellStatus::Prepared));
    loader.begin_cell_upload(a, &world, &mut backend).unwrap();
    finish_upload(&mut loader, a, &mut world, &mut backend, &mut frame);
    assert_eq!(loader.cell_status(a), Ok(CellStatus::Active));
    assert_eq!(loader.cell_status(b), Ok(CellStatus::Released));
    assert_eq!(world.entity_count(), 2);
    assert_eq!(backend.geometry_residency().texture_bytes, 32 * MIB);
    assert_eq!(backend.geometry_upload_snapshot().tickets, 1);
    loader
        .release_cell_gpu(a, &mut world, &mut manager, frame + 1, &mut backend)
        .unwrap();
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(Duration::from_secs(5)));
    backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
    let cpu = loader.asset_loader().streaming_snapshot();
    assert_eq!((cpu.pending_operations, cpu.pending_tickets), (0, 0));
    assert_eq!((cpu.reserved_bytes, cpu.cpu_resident_bytes), (0, 0));
    assert_eq!(loader.cell_lifetime_counts(), (0, 0));
    assert_eq!(world.entity_count(), 0);
    let logical = manager.end_frame(frame + 10);
    assert_eq!(logical.resident_bytes, 0);
    assert_eq!(logical.pending_release_count, 0);
    assert_eq!(backend.geometry_residency().texture_bytes, 0);
    assert_eq!(backend.geometry_residency().physical_mesh_bytes, 0);
    assert_eq!(backend.geometry_upload_snapshot().tickets, 0);
    assert_eq!(backend.geometry_upload_snapshot().staging_bytes, 0);
    assert_eq!(backend.gpu_lifecycle_stats().in_flight, 0);
    assert_eq!(backend.gpu_lifecycle_stats().retiring_bytes, 0);
    eprintln!("GPU admission retry: same Prepared ticket activated after B retired at the unchanged48MiB limit; all resources drained");
    drop(fixture);
    assert!(!root.exists(), "owned fixture was not cleaned: {root:?}");
}

fn finish_upload(
    loader: &mut RuntimeInstanceLoader,
    ticket: CellTicket,
    world: &mut World,
    backend: &mut RealWgpuBackend,
    frame: &mut u64,
) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        assert!(Instant::now() < deadline, "retry upload did not complete");
        *frame += 1;
        let bytes = backend.pump_geometry_uploads(*frame).unwrap();
        assert!(bytes <= 16 * MIB as usize);
        if bytes != 0 {
            backend
                .wait_gpu_submission(
                    backend.last_gpu_submission().unwrap(),
                    Duration::from_secs(5),
                )
                .unwrap();
        }
        match loader.commit_cell_gpu(ticket, world, backend) {
            Ok(_) => return,
            Err(CellError::NotReady) => std::thread::yield_now(),
            other => panic!("retry upload failed: {other:?}"),
        }
    }
}
