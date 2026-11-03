//! Payload-size heap regressions at the package owner, without Compiler/Cargo/Player composition.
use engine_runtime::canonical_digest::sha256_prefixed;
use engine_runtime::runtime_package::{RuntimeProjectInfo, RuntimeScene};
use engine_runtime::runtime_package_builder::{
    RuntimePackageBuildInput, RuntimePackageBuildRequest, RuntimePackageBuildStatus,
    RuntimePackageBuilder, RuntimePackageSourceAsset, RuntimePackageSourceJson,
};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

struct HeapMeter;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static TEST_LOCK: Mutex<()> = Mutex::new(());

fn allocated(bytes: usize) {
    let live = LIVE.fetch_add(bytes, Ordering::Relaxed) + bytes;
    PEAK.fetch_max(live, Ordering::Relaxed);
}

// Delegate unchanged allocation layouts and pointers to System; counters never allocate.
unsafe impl GlobalAlloc for HeapMeter {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            allocated(layout.size());
        }
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            allocated(layout.size());
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let result = unsafe { System.realloc(pointer, layout, size) };
        if !result.is_null() {
            if size >= layout.size() {
                allocated(size - layout.size());
            } else {
                LIVE.fetch_sub(layout.size() - size, Ordering::Relaxed);
            }
        }
        result
    }
}

#[global_allocator]
static ALLOCATOR: HeapMeter = HeapMeter;

fn fixture() -> RuntimePackageBuildInput {
    let mut input = RuntimePackageBuildInput::new(RuntimeProjectInfo::explicit_empty(
        "memory-owner",
        "Memory owner",
        "0.1.0",
    ));
    input.scenes.push(RuntimeScene {
        main_camera: None,
        schema_version: "runtime-scene.v1".into(),
        id: "main".into(),
        name: "Main".into(),
        gravity: 0.0,
        background: "#000000".into(),
        sky_color: "#000000".into(),
        entities: Vec::new(),
    });
    let mapping = engine_input::InputMappingAsset::gameplay_default();
    input.input_mappings.push(RuntimePackageSourceJson {
        id: mapping.asset_id.clone(),
        document: serde_json::to_value(mapping).unwrap(),
    });
    input.assets.push(
        RuntimePackageSourceAsset::new(
            "blob",
            "Blob",
            "asset",
            "Assets/blob.bin",
            "cooked/blob.bin",
        )
        .with_runtime_payload(vec![73; 16 * 1024 * 1024]),
    );
    input
}

fn measure<T>(f: impl FnOnce() -> T) -> (T, usize) {
    let baseline = LIVE.load(Ordering::Relaxed);
    PEAK.store(baseline, Ordering::Relaxed);
    let result = f();
    (
        result,
        PEAK.load(Ordering::Relaxed).saturating_sub(baseline),
    )
}

#[test]
fn assembly_digest_does_not_copy_binary_payload() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let input = fixture();
    let (digest, peak) = measure(|| input.assembly_input_digest().unwrap());
    // Captured from the original owner before the borrowing repair.
    assert_eq!(
        digest.0.value,
        "b7efec681eb2208681ae5fd42afb303f9dd22c9ec35c2e791f6d295fdbdc0316"
    );
    eprintln!(
        "assembly_digest extra_peak_bytes={peak}, digest={}",
        digest.0.value
    );
    assert!(
        peak < 1024 * 1024,
        "digest duplicated binary payload: {peak} bytes"
    );
    let mut changed = input;
    changed.assets[0].runtime_payload.as_mut().unwrap()[0] ^= 1;
    assert_ne!(digest, changed.assembly_input_digest().unwrap());
}

#[test]
fn package_publication_does_not_retain_binary_payload_copies() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let input = fixture();
    let root = std::env::temp_dir().join(format!("aife-package-memory-{}", std::process::id()));
    assert!(
        !root.exists(),
        "fresh test root required: {}",
        root.display()
    );
    let request = RuntimePackageBuildRequest::dev_desktop(&root, "main");
    let (report, peak) = measure(|| RuntimePackageBuilder::build(&request, &input));
    eprintln!(
        "package_publication extra_peak_bytes={peak}, status={:?}",
        report.status
    );
    assert_eq!(
        report.status,
        RuntimePackageBuildStatus::Success,
        "{report:#?}"
    );
    let written = std::fs::read(root.join("cooked/blob.bin")).unwrap();
    assert_eq!(
        sha256_prefixed(&written),
        sha256_prefixed(input.assets[0].runtime_payload.as_ref().unwrap())
    );
    let entries = report
        .outputs
        .payload_inventory
        .iter()
        .map(|path| (path.clone(), std::fs::read(root.join(path)).unwrap()))
        .collect::<Vec<_>>();
    let tree = engine_runtime::canonical_digest::payload_tree_digest(
        entries
            .iter()
            .map(|(path, bytes)| (path.as_str(), bytes.as_slice())),
    )
    .unwrap();
    assert_eq!(
        report.outputs.payload_tree_digest,
        Some(tree.prefixed_value())
    );
    std::fs::remove_dir_all(&root).unwrap();
    // Metadata and loader bookkeeping may allocate; another whole 16 MiB payload may not.
    assert!(
        peak < 4 * 1024 * 1024,
        "publication duplicated binary payload: {peak} bytes"
    );
}
