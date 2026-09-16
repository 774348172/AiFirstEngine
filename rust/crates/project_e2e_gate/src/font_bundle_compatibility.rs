use editor_core::{
    ProjectAssemblyArtifactCacheStatus, ProjectRuntimePackageAssembler,
    ProjectRuntimePackageAssemblyRequest, ProjectRuntimePackageAssemblyStatus,
    ENGINE_BUILT_IN_FONT_PACK_ID,
};
use engine_runtime::aui::{
    AuiRuntimePresenter, AuiSnapshotSource, ProjectUiStateSnapshot, ProjectUiStateSnapshotOutput,
};
use engine_runtime::runtime_package::{load_runtime_package, RuntimePackage};
use engine_runtime::runtime_package_builder::{
    RuntimePackageBuildRequest, RuntimePackageBuildStatus, RuntimePackageBuilder,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn non_game_specific_second_project_uses_public_font_bundle_v2_seam() {
    let project_root = isolated_font_bundle_project("font-v2-second-project-source");
    let (package, package_root) = assemble_build_load(&project_root, "font-v2-second-project");
    let bundle = package
        .font_bundles
        .default_bundle()
        .expect("second project default FontBundle");
    assert!(!bundle.metadata.legacy_mode);
    assert!(!bundle.metadata.fallback_used);
    assert!(bundle.metadata.quality_gate_eligible);
    assert!(package
        .assets
        .assets
        .iter()
        .all(|asset| asset.asset_type != "fontSource"));
    assert!(!package_tree_contains_source_font(&package_root));

    let document = package
        .aui_documents
        .get("font-showcase-hud")
        .expect("showcase AUI document");
    let present = AuiRuntimePresenter::present_project_snapshot_with_fonts(
        document,
        ProjectUiStateSnapshotOutput::new(
            "font-v2-second-project-gate",
            AuiSnapshotSource::PackageSmokeSnapshot,
            ProjectUiStateSnapshot::package_smoke_snapshot(1),
        ),
        &package.font_atlases,
        &package.font_bundles,
    );
    assert_eq!(
        present.report.rendered_glyph_count,
        present.report.requested_glyph_count
    );
    assert_eq!(present.report.unsupported_glyph_count, 0);
    assert!(!present.report.font_fallback_used);
    assert_eq!(
        present.report.font_source_kind.as_deref(),
        Some("project_font_bundle_v2")
    );
    fs::remove_dir_all(package_root).expect("remove second-project font package");
    fs::remove_dir_all(project_root).expect("remove isolated second-project source");
}

#[test]
fn non_game_specific_second_project_reuses_sealed_font_producer_artifact() {
    let project_root = isolated_font_bundle_project("font-v2-producer-source");
    let cache_root = unique_temp_dir("font-v2-producer-cache");
    let first = ProjectRuntimePackageAssembler::assemble(
        ProjectRuntimePackageAssemblyRequest::new(&project_root)
            .with_artifact_cache_root(&cache_root),
    );
    assert_eq!(first.status, ProjectRuntimePackageAssemblyStatus::Success);
    let first_font = first
        .report
        .producer_reports
        .iter()
        .find(|report| report.producer_id == "font-cook")
        .expect("first FontCookProducer report");
    assert_eq!(
        first_font.cache_status,
        ProjectAssemblyArtifactCacheStatus::Produced
    );
    assert!(first_font.produce_duration_ms > 0);

    let second = ProjectRuntimePackageAssembler::assemble(
        ProjectRuntimePackageAssemblyRequest::new(&project_root)
            .with_artifact_cache_root(&cache_root),
    );
    assert_eq!(second.status, ProjectRuntimePackageAssemblyStatus::Success);
    let second_font = second
        .report
        .producer_reports
        .iter()
        .find(|report| report.producer_id == "font-cook")
        .expect("second FontCookProducer report");
    assert_eq!(
        second_font.cache_status,
        ProjectAssemblyArtifactCacheStatus::Hit
    );
    assert_eq!(second_font.produce_duration_ms, 0);
    assert!(second_font
        .substages
        .iter()
        .filter(|stage| stage.stage_id == "raster_bitmap" || stage.stage_id == "raster_msdf")
        .all(|stage| stage.skipped));
    assert_eq!(
        first_font.output_digest, second_font.output_digest,
        "cache hit must return the sealed typed artifact"
    );
    fs::remove_dir_all(cache_root).expect("remove second-project producer cache");
    fs::remove_dir_all(project_root).expect("remove isolated second-project source");
}

#[test]
fn sample_projects_use_the_engine_builtin_default_font_bundle() {
    for project_name in ["complex_shooter_project", "switch_puzzle_project"] {
        let project_root = workspace_root().join("samples").join(project_name);
        let (package, package_root) =
            assemble_build_load(&project_root, &format!("built-in-font-{project_name}"));
        let bundle = package
            .font_bundles
            .default_bundle()
            .unwrap_or_else(|| panic!("{project_name} default built-in FontBundle"));
        assert_eq!(bundle.metadata.font_bundle_id, ENGINE_BUILT_IN_FONT_PACK_ID);
        assert!(!bundle.metadata.legacy_mode);
        assert!(!bundle.metadata.fallback_used);
        assert!(bundle.metadata.quality_gate_eligible);
        assert!(package.font_atlases.default_atlas().is_none());
        assert!(!package_tree_contains_source_font(&package_root));
        fs::remove_dir_all(package_root).expect("remove built-in font package");
    }
}

fn assemble_build_load(project_root: &Path, label: &str) -> (RuntimePackage, PathBuf) {
    let assembly = ProjectRuntimePackageAssembler::assemble(
        ProjectRuntimePackageAssemblyRequest::new(project_root),
    );
    assert_eq!(
        assembly.status,
        ProjectRuntimePackageAssemblyStatus::Success,
        "{:?}",
        assembly.report.diagnostics
    );
    let input = assembly.build_input.expect("successful assembly input");
    let package_root = unique_temp_dir(label);
    let build = RuntimePackageBuilder::build(
        &RuntimePackageBuildRequest::dev_desktop(
            &package_root,
            assembly.active_scene_id.expect("active scene id"),
        ),
        &input,
    );
    assert_eq!(
        build.status,
        RuntimePackageBuildStatus::Success,
        "{:?}",
        build.diagnostics
    );
    let loaded = load_runtime_package(&package_root);
    let package = loaded.value.unwrap_or_else(|| {
        panic!(
            "runtime package load failed: {:?}",
            loaded.diagnostics.issues
        )
    });
    (package, package_root)
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("{label}-{}-{stamp}", std::process::id()))
}

fn isolated_font_bundle_project(label: &str) -> PathBuf {
    let source = workspace_root()
        .join("rust")
        .join("fixtures")
        .join("projects")
        .join("font_bundle_v2_second_project");
    let target = unique_temp_dir(label);
    copy_directory(&source, &target);
    target
}

fn copy_directory(source: &Path, target: &Path) {
    fs::create_dir_all(target).expect("create isolated font fixture directory");
    for entry in fs::read_dir(source).expect("read font fixture directory") {
        let entry = entry.expect("font fixture entry");
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_directory(&source_path, &target_path);
        } else {
            fs::copy(&source_path, &target_path).expect("copy font fixture file");
        }
    }
}

fn package_tree_contains_source_font(root: &Path) -> bool {
    let Ok(entries) = fs::read_dir(root) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let path = entry.path();
        if path.is_dir() {
            package_tree_contains_source_font(&path)
        } else {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    matches!(
                        extension.to_ascii_lowercase().as_str(),
                        "ttf" | "otf" | "ttc" | "otc"
                    )
                })
        }
    })
}
