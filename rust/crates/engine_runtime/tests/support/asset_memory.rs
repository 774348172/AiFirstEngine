use engine_runtime::{
    resource_memory::{MemoryCoverage, ResourceMemorySample},
    runtime_asset::RuntimeAssetLoadState,
    runtime_asset_loader::RuntimeAssetLoader,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

fn capture(loader: &RuntimeAssetLoader) -> ResourceMemorySample {
    let mut sample = ResourceMemorySample::new(1, 1, 0);
    loader.observe_memory(&mut sample);
    sample
}

fn fixture() -> (super::streamed_cells::Fixture, RuntimeAssetLoader) {
    let fixture = super::streamed_cells::build(true);
    let mut loader = RuntimeAssetLoader::new(
        &fixture.package.package_dir,
        fixture.package.runtime_asset_index.clone(),
        fixture.package.runtime_asset_mount_table.clone(),
    );
    loader.mount_bundle("startup");
    (fixture, loader)
}

fn wait_completions(loader: &RuntimeAssetLoader) -> ResourceMemorySample {
    let operations = loader.streaming_snapshot().dispatched_operations;
    let started = Instant::now();
    loop {
        let sample = capture(loader);
        if sample.capacities["readyQueue"] == operations {
            return sample;
        }
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "worker did not complete: {sample:?}"
        );
        std::thread::yield_now();
    }
}

#[test]
fn mesh_raw_releases_after_preparation_with_live_asset_and_shared_readers() {
    use engine_runtime::{
        render_asset_production::RuntimeRenderAssetProducer,
        render_contract::{RenderAssetKind, RenderAssetReference},
        render_resource::RenderResourceManager,
    };
    for asynchronous in [false, true] {
        let (_fixture, mut loader) = fixture();
        let reference = super::streamed_cells::reference("cell-a", "prefab");
        let root = if asynchronous {
            let root = loader.request_async(&reference).unwrap();
            let started = Instant::now();
            while loader.get_handle_state(&root) == Some(RuntimeAssetLoadState::Loading) {
                loader.poll_async();
                assert!(started.elapsed() < Duration::from_secs(5));
                std::thread::yield_now();
            }
            root
        } else {
            loader.load(&reference).unwrap()
        };
        let mesh = loader
            .load(&super::streamed_cells::reference("shared-mesh", "mesh"))
            .unwrap();
        let raw = loader.get_asset_bytes(&mesh).unwrap();
        let reader = raw.clone();
        let weak = Arc::downgrade(&raw);
        let allocation_id = format!("cpu-storage:{:x}", raw.as_ptr() as usize);
        let before = loader.streaming_snapshot().cpu_resident_bytes;
        assert!(capture(&loader)
            .allocations
            .iter()
            .any(|a| a.allocation_id == allocation_id));
        let mut manager = RenderResourceManager::new();
        let reference = RenderAssetReference {
            expected_kind: RenderAssetKind::Mesh,
            reference: super::streamed_cells::reference("shared-mesh", "mesh"),
        };
        let prepared = RuntimeRenderAssetProducer
            .prepare_cooked_from_package(1, &reference, &mut loader, &mut manager)
            .unwrap();
        loader.release(&mesh).unwrap();
        assert_eq!(
            loader.get_handle_state(&root),
            Some(RuntimeAssetLoadState::Ready)
        );
        drop(raw);
        assert!(
            weak.upgrade().is_some(),
            "another CPU reader must keep its bytes"
        );
        let raw_len = reader.len();
        drop(reader);
        assert!(
            weak.upgrade().is_none(),
            "prepared mesh raw remained pinned by a live asset"
        );
        assert_eq!(
            loader.streaming_snapshot().cpu_resident_bytes,
            before - raw_len
        );
        assert!(!capture(&loader)
            .allocations
            .iter()
            .any(|a| a.allocation_id == allocation_id));
        let repeated = RuntimeRenderAssetProducer
            .prepare_cooked_from_package(2, &reference, &mut loader, &mut manager)
            .unwrap();
        assert_eq!(prepared.asset, repeated.asset);
        assert_eq!(
            loader.streaming_snapshot().cpu_resident_bytes,
            before - raw_len
        );
        loader.release(&root).unwrap();
        assert_eq!(loader.decoded_cache_len(), 0);
        assert!(loader.shutdown_streaming(Duration::from_secs(2)));
    }
}

#[test]
fn mesh_raw_reread_preserves_integrity_errors_and_recovers_without_pinning() {
    use engine_runtime::{
        render_asset_production::RuntimeRenderAssetProducer,
        render_contract::{ContractErrorCode, RenderAssetKind, RenderAssetReference},
        render_resource::RenderResourceManager,
    };
    let (fixture, mut loader) = fixture();
    let reference = RenderAssetReference {
        expected_kind: RenderAssetKind::Mesh,
        reference: super::streamed_cells::reference("shared-mesh", "mesh"),
    };
    let handle = loader.load(&reference.reference).unwrap();
    let record = loader.asset_index().record_by_guid("shared-mesh").unwrap();
    let cooked = loader
        .asset_index()
        .cooked_asset(&record.cooked_asset_id)
        .unwrap();
    let path = fixture
        .package
        .package_dir
        .join(cooked.path.as_ref().unwrap());
    let original = std::fs::read(&path).unwrap();
    let mut manager = RenderResourceManager::new();
    let first = RuntimeRenderAssetProducer
        .prepare_cooked_from_package(1, &reference, &mut loader, &mut manager)
        .unwrap();
    let resident = loader.streaming_snapshot().cpu_resident_bytes;
    let reread = loader.get_asset_bytes(&handle).unwrap();
    assert_eq!(reread.as_ref(), original);
    let weak = Arc::downgrade(&reread);
    drop(reread);
    assert!(
        weak.upgrade().is_none(),
        "reread must not repopulate the raw cache"
    );
    let mut corrupted = original.clone();
    corrupted[0] ^= 1;
    for bytes in [&corrupted[..], &original[..original.len() - 1]] {
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(
            RuntimeRenderAssetProducer
                .prepare_cooked_from_package(2, &reference, &mut loader, &mut manager)
                .unwrap_err()
                .code,
            ContractErrorCode::AssetDigestMismatch
        );
        assert_eq!(loader.streaming_snapshot().cpu_resident_bytes, resident);
    }
    std::fs::remove_file(&path).unwrap();
    assert_eq!(
        RuntimeRenderAssetProducer
            .prepare_cooked_from_package(3, &reference, &mut loader, &mut manager)
            .unwrap_err()
            .code,
        ContractErrorCode::MissingCookedPayload
    );
    std::fs::write(&path, &original).unwrap();
    let recovered = RuntimeRenderAssetProducer
        .prepare_cooked_from_package(4, &reference, &mut loader, &mut manager)
        .unwrap();
    assert_eq!(first.asset, recovered.asset);
    assert_eq!(first.cooked_digest, recovered.cooked_digest);
    assert_eq!(first.binding, recovered.binding);
    assert_eq!(
        loader.get_handle_state(&handle),
        Some(RuntimeAssetLoadState::Ready)
    );
    loader.release(&handle).unwrap();
    assert_eq!(
        loader.decoded_cache_len(),
        0,
        "failed prepare leaked a loader reference"
    );
    assert_eq!(loader.streaming_snapshot().cpu_resident_bytes, 0);
    assert!(loader.get_asset_bytes(&handle).is_none());
}

#[test]
fn mesh_raw_reread_rejects_old_patch_identity_and_stale_handles() {
    use engine_runtime::{
        render_asset_production::RuntimeRenderAssetProducer,
        render_contract::{RenderAssetKind, RenderAssetReference},
        render_resource::RenderResourceManager,
        runtime_asset::RuntimeAssetIndex,
    };
    for change_cooked_path_only in [false, true] {
        let (fixture, mut loader) = fixture();
        let reference = RenderAssetReference {
            expected_kind: RenderAssetKind::Mesh,
            reference: super::streamed_cells::reference("shared-mesh", "mesh"),
        };
        let old = loader.load(&reference.reference).unwrap();
        let reader = loader.get_asset_bytes(&old).unwrap();
        let weak = Arc::downgrade(&reader);
        let mut manager = RenderResourceManager::new();
        RuntimeRenderAssetProducer
            .prepare_cooked_from_package(1, &reference, &mut loader, &mut manager)
            .unwrap();
        let mut record = loader
            .asset_index()
            .record_by_guid("shared-mesh")
            .unwrap()
            .clone();
        let mut cooked = loader
            .asset_index()
            .cooked_asset(&record.cooked_asset_id)
            .unwrap()
            .clone();
        if change_cooked_path_only {
            let replacement = "mesh-raw-patch.json";
            std::fs::write(
                fixture.package.package_dir.join(replacement),
                reader.as_ref(),
            )
            .unwrap();
            cooked.path = Some(replacement.into());
        } else {
            record.version = "mesh-raw-patch-2".into();
        }
        loader.mount_patch_index(RuntimeAssetIndex::new(vec![record], vec![cooked]));
        assert!(
            loader.get_asset_bytes(&old).is_none(),
            "old identity must not reread patched source"
        );
        assert!(
            weak.upgrade().is_some(),
            "patch must preserve borrowed CPU bytes"
        );
        let new = loader.load(&reference.reference).unwrap();
        let fresh = loader.get_asset_bytes(&new).unwrap();
        assert_eq!(reader.as_ref(), fresh.as_ref());
        assert!(!Arc::ptr_eq(&reader, &fresh));
        let mut stale = new.clone();
        stale.generation += 1;
        assert!(loader.get_asset_bytes(&stale).is_none());
        let mut forged = new.clone();
        forged.asset_guid = "other-mesh".into();
        assert!(loader.get_asset_bytes(&forged).is_none());
        loader.release(&old).unwrap();
        assert!(loader.get_asset_bytes(&old).is_none());
        assert!(Arc::ptr_eq(&fresh, &loader.get_asset_bytes(&new).unwrap()));
        RuntimeRenderAssetProducer
            .prepare_cooked_from_package(2, &reference, &mut loader, &mut manager)
            .unwrap();
        drop(reader);
        assert!(weak.upgrade().is_none());
        loader.release(&new).unwrap();
        assert_eq!(loader.decoded_cache_len(), 0);
        assert_eq!(loader.streaming_snapshot().cpu_resident_bytes, 0);
    }
}

#[test]
fn repeated_package_cells_release_backings_only_after_the_last_consumer() {
    use engine_runtime::{
        render_asset_production::RuntimeRenderAssetProducer,
        render_contract::{RenderAssetKind, RenderAssetReference},
        render_resource::RenderResourceManager,
        runtime_asset_loader::StreamingBudget,
    };
    let (_fixture, mut loader) = fixture();
    loader
        .set_streaming_budget(StreamingBudget {
            max_handles: 32,
            ..loader.streaming_budget()
        })
        .unwrap();
    let reference = super::streamed_cells::reference("cell-a", "prefab");
    let material = RenderAssetReference {
        expected_kind: RenderAssetKind::Material,
        reference: super::streamed_cells::reference("material-a", "material"),
    };
    for cycle in 0..128 {
        let root = if cycle % 2 == 0 {
            loader.load(&reference).unwrap()
        } else {
            let h = loader.request_async(&reference).unwrap();
            let started = Instant::now();
            while loader.get_handle_state(&h) == Some(RuntimeAssetLoadState::Loading) {
                loader.poll_async();
                assert!(started.elapsed() < Duration::from_secs(5));
                std::thread::yield_now();
            }
            assert_eq!(
                loader.get_handle_state(&h),
                Some(RuntimeAssetLoadState::Ready)
            );
            h
        };
        let shared = loader.load(&reference).unwrap();
        let mesh = loader
            .load(&super::streamed_cells::reference("shared-mesh", "mesh"))
            .unwrap();
        let raw = loader.get_asset_bytes(&mesh).unwrap();
        let raw_weak = Arc::downgrade(&raw);
        loader.release(&mesh).unwrap();
        let mut manager = RenderResourceManager::new();
        let first = RuntimeRenderAssetProducer
            .prepare_cooked_from_package(cycle, &material, &mut loader, &mut manager)
            .unwrap();
        let second = RuntimeRenderAssetProducer
            .prepare_cooked_from_package(cycle, &material, &mut loader, &mut manager)
            .unwrap();
        let pixels = &first.textures[0].payload.rgba8;
        assert!(Arc::ptr_eq(pixels, &second.textures[0].payload.rgba8));
        let pixels_weak = Arc::downgrade(pixels);
        drop(raw);
        loader.release(&root).unwrap();
        assert!(raw_weak.upgrade().is_some());
        assert_eq!(
            loader.get_handle_state(&shared),
            Some(RuntimeAssetLoadState::Ready)
        );
        loader.release(&shared).unwrap();
        assert_eq!(loader.decoded_cache_len(), 0, "cycle {cycle}");
        assert_eq!(loader.streaming_snapshot().cpu_resident_bytes, 0);
        assert!(
            raw_weak.upgrade().is_none(),
            "released mesh backing survived cycle {cycle}"
        );
        assert!(
            pixels_weak.upgrade().is_some(),
            "prepared consumer lost its pixels"
        );
        drop(first);
        assert!(pixels_weak.upgrade().is_some());
        drop(second);
        assert!(
            pixels_weak.upgrade().is_none(),
            "pixel backing leaked after cycle {cycle}"
        );
    }
    assert!(loader.shutdown_streaming(Duration::from_secs(2)));
}

#[test]
fn deep_prefab_project_json_storage_is_observed_without_counting_raw_twice() {
    // A non-power-of-two string makes this a real decoded allocation probe,
    // distinguishable from source JSON, entities, metadata or a reservation.
    let marker = "x".repeat(65539);
    let fixture = super::streamed_cells::build_with_component(
        false,
        true,
        4,
        Some(serde_json::json!({"componentType":"project.memory", "data":{"probe":[marker]}})),
    );
    let mut loader = RuntimeAssetLoader::new(
        &fixture.package.package_dir,
        fixture.package.runtime_asset_index.clone(),
        fixture.package.runtime_asset_mount_table.clone(),
    );
    loader.mount_bundle("startup");
    capture(&loader);
    let root = loader
        .request_async(&super::streamed_cells::reference("cell-a", "prefab"))
        .unwrap();
    let ready = wait_completions(&loader);
    let before = ready
        .allocations
        .iter()
        .find(|a| a.owner == "assets" && a.bytes == 65539)
        .expect("decoded project JSON string backing is missing from the completed prefab");
    assert!(before.roles.contains("queuePayload"));
    let identity = before.allocation_id.clone();
    loader.poll_async();
    let cached = capture(&loader);
    let after = cached
        .allocations
        .iter()
        .find(|a| a.allocation_id == identity)
        .unwrap();
    assert!(after.roles.contains("decodedCache"));
    let raw = loader.get_asset_bytes(&root).unwrap();
    assert_ne!(identity, format!("cpu-storage:{:x}", raw.as_ptr() as usize));
    assert!(cached.allocations.iter().any(|a| a.owner == "assets"
        && a.bytes
            >= std::mem::size_of::<engine_runtime::runtime_package::RuntimeEntity>() as u64
        && a.allocation_id != format!("cpu-storage:{:x}", raw.as_ptr() as usize)));
    loader.release(&root).unwrap();
    assert!(loader.shutdown_streaming(Duration::from_secs(2)));
    assert!(!capture(&loader)
        .allocations
        .iter()
        .any(|a| a.allocation_id == identity));
}

#[test]
fn late_cancel_and_shutdown_drain_diagnostic_leases_without_publishing_cache() {
    let (_fixture, mut loader) = fixture();
    capture(&loader);
    let root = loader
        .request_async(&super::streamed_cells::reference("cell-a", "prefab"))
        .unwrap();
    let ready = wait_completions(&loader);
    assert!(ready.capacities["loader.observedPayloads"] > 0);
    loader.release(&root).unwrap();
    loader.poll_async();
    assert_eq!(loader.decoded_cache_len(), 0);
    assert_eq!(capture(&loader).capacities["loader.observedPayloads"], 0);
    assert!(loader.shutdown_streaming(Duration::from_secs(2)));
    assert_eq!(capture(&loader).capacities["loader.observedPayloads"], 0);
}

#[test]
fn disabled_observation_does_not_retain_payloads_during_shutdown() {
    let (_fixture, mut loader) = fixture();
    let root = loader
        .request_async(&super::streamed_cells::reference("cell-a", "prefab"))
        .unwrap();
    // No observe_memory call while the worker is producing completions.
    let started = Instant::now();
    // Service completion counts are available only through the existing census;
    // waiting for the thread's bounded shutdown consumes all payloads instead.
    assert!(loader.shutdown_streaming(Duration::from_secs(2)));
    assert!(started.elapsed() < Duration::from_secs(5));
    let sample = capture(&loader);
    assert_eq!(sample.capacities["loader.observedPayloads"], 0);
    assert_eq!(
        loader.get_handle_state(&root),
        Some(RuntimeAssetLoadState::Failed)
    );
    assert_eq!(loader.decoded_cache_len(), 0);
}

#[test]
fn completed_payload_memory_moves_to_cache_and_disappears_after_release() {
    let (_fixture, mut loader) = fixture();
    // Enable the existing opt-in capture before dispatch. No worker polling is
    // permitted until we have captured the actual completed mailbox payloads.
    capture(&loader);
    let root = loader
        .request_async(&super::streamed_cells::reference("cell-a", "prefab"))
        .unwrap();
    let operations = loader.streaming_snapshot().dispatched_operations;
    let started = Instant::now();
    let ready = loop {
        let sample = capture(&loader);
        if sample.capacities["readyQueue"] == operations {
            break sample;
        }
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "worker did not complete: {sample:?}"
        );
        std::thread::yield_now();
    };
    assert_eq!(loader.decoded_cache_len(), 0);
    loader.poll_async();
    assert_eq!(
        loader.get_handle_state(&root),
        Some(RuntimeAssetLoadState::Ready)
    );
    let raw = loader.get_asset_bytes(&root).unwrap();
    let identity = format!("cpu-storage:{:x}", raw.as_ptr() as usize);
    let pending = ready
        .allocations
        .iter()
        .find(|a| a.allocation_id == identity)
        .expect("actual decoded source backing is missing while Completion owns it");
    assert_eq!(pending.bytes, raw.len() as u64);
    assert_eq!(pending.owner, "assets");
    assert!(pending.roles.contains("queuePayload"));
    let cached = capture(&loader);
    let resident = cached
        .allocations
        .iter()
        .find(|a| a.allocation_id == identity)
        .unwrap();
    assert!(resident.roles.contains("decodedCache"));
    assert!(!resident.roles.contains("queuePayload"));
    assert_eq!(cached.coverage["cpu.owner.assets"], MemoryCoverage::Partial);
    loader.release(&root).unwrap();
    assert!(loader.shutdown_streaming(Duration::from_secs(2)));
    let drained = capture(&loader);
    assert!(!drained
        .allocations
        .iter()
        .any(|a| a.allocation_id == identity));
    assert_eq!(drained.capacities["readyQueue"], 0);
}

#[test]
fn cache_memory_includes_shared_texture_descriptor_and_pixel_vector_object() {
    use engine_runtime::{
        render_asset_production::RuntimeRenderAssetProducer,
        render_contract::{RenderAssetKind, RenderAssetReference},
        render_resource::RenderResourceManager,
    };
    let (_fixture, mut loader) = fixture();
    let root = loader
        .request_async(&super::streamed_cells::reference("cell-a", "prefab"))
        .unwrap();
    let started = Instant::now();
    while loader.get_handle_state(&root) == Some(RuntimeAssetLoadState::Loading) {
        loader.poll_async();
        assert!(started.elapsed() < Duration::from_secs(5));
        std::thread::yield_now();
    }
    let mut manager = RenderResourceManager::new();
    let prepared = RuntimeRenderAssetProducer
        .prepare_cooked_from_package(
            1,
            &RenderAssetReference {
                expected_kind: RenderAssetKind::Material,
                reference: super::streamed_cells::reference("material-a", "material"),
            },
            &mut loader,
            &mut manager,
        )
        .unwrap();
    let texture = &prepared.textures[0].payload;
    let sample = capture(&loader);
    for (address, bytes) in [
        (
            Arc::as_ptr(texture) as usize,
            std::mem::size_of_val(texture.as_ref()),
        ),
        (
            Arc::as_ptr(&texture.rgba8) as usize,
            std::mem::size_of::<Vec<u8>>(),
        ),
        (
            texture.asset_id.as_ptr() as usize,
            texture.asset_id.capacity(),
        ),
    ] {
        let identity = format!("cpu-storage:{address:x}");
        let allocation = sample
            .allocations
            .iter()
            .find(|a| a.allocation_id == identity)
            .unwrap_or_else(|| panic!("shared texture backing {identity} is missing"));
        assert_eq!(allocation.bytes, bytes as u64);
        assert_eq!(allocation.owner, "assets");
    }
    loader.release(&root).unwrap();
    assert!(loader.shutdown_streaming(Duration::from_secs(2)));
}

#[test]
fn snapshot_memory_observes_shared_descriptor_and_camera_strings_without_loader_census() {
    use engine_runtime::{
        engine_host_loop::*, frame_loop::RuntimeFrameContext,
        runtime_instance_loader::RuntimeInstanceLoader, scene_loader::load_scene_into_world,
    };
    let fixture = super::streamed_cells::build_spatial_scene(1);
    let mut world = load_scene_into_world(&fixture.package.active_scene)
        .value
        .unwrap();
    let mut loader = RuntimeInstanceLoader::from_package(&fixture.package);
    let mut host = EngineHostLoop::new("main");
    let output = host.tick_with_runtime_context(
        EngineFrameInput::new(EngineHostMode::ExportedGame),
        &mut world,
        RuntimeFrameContext::new(&fixture.package, &mut loader),
    );
    let rendered = output.render_thread_frame.unwrap();
    let texture = &rendered.geometry_textures[0].payload;
    let snapshot = host.retained_geometry_snapshot().unwrap();
    let mut sample = ResourceMemorySample::new(1, 1, 0);
    snapshot.observe_memory(&mut sample);
    let camera_id = snapshot.views[0]
        .camera
        .view
        .source_camera_entity_id
        .as_ref()
        .unwrap();
    for (address, bytes, owner) in [
        (
            Arc::as_ptr(texture) as usize,
            std::mem::size_of_val(texture.as_ref()),
            "assets",
        ),
        (
            Arc::as_ptr(&texture.rgba8) as usize,
            std::mem::size_of::<Vec<u8>>(),
            "assets",
        ),
        (
            camera_id.as_ptr() as usize,
            camera_id.capacity(),
            "snapshots",
        ),
    ] {
        let identity = format!("cpu-storage:{address:x}");
        let actual = sample
            .allocations
            .iter()
            .find(|a| a.allocation_id == identity)
            .unwrap_or_else(|| panic!("snapshot backing {identity} is missing"));
        assert_eq!(actual.bytes, bytes as u64);
        assert_eq!(actual.owner, owner);
    }
    // The same descriptor must reconcile with the loader, not become a second
    // allocation or a conflicting owner when the production census combines both.
    loader.asset_loader_mut().observe_memory(&mut sample);
    let mut unique = std::collections::BTreeMap::new();
    for a in &sample.allocations {
        if let Some(old) = unique.insert(&a.allocation_id, (&a.owner, a.bytes)) {
            assert_eq!(
                old,
                (&a.owner, a.bytes),
                "aliased allocation conflicts: {a:?}"
            );
        }
    }
}
