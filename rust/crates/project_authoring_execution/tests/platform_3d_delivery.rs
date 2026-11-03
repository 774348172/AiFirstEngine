use project_authoring_execution::{
    CheckProfile, GameProjectCompiler, ProjectAuthoringSession, TargetProfile,
};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
struct Fixture(PathBuf);

mod s5b {
    use super::*;
    use serde_json::json;

    #[test]
    #[ignore = "explicit unchanged historical RuntimePackage and fresh evidence directory"]
    fn current_reader_loads_actual_legacy_package() {
        let package = PathBuf::from(std::env::var_os("AIFE_S5_LEGACY_PACKAGE").expect("legacy package"));
        let evidence = PathBuf::from(std::env::var_os("AIFE_S5_READER_EVIDENCE").expect("fresh evidence"));
        fs::create_dir(&evidence).unwrap();
        let loaded = engine_runtime::runtime_package::load_runtime_package(&package);
        let issues: Vec<_> = loaded.diagnostics.issues.iter().map(|issue| json!({
            "code":issue.code,"path":issue.path,"stage":issue.stage,
            "message":issue.message,"nextAction":issue.next_action,
        })).collect();
        fs::write(evidence.join("reader.json"), serde_json::to_vec_pretty(&json!({
            "package":package,"loaded":loaded.value.is_some(),"issues":issues,
            "manifest":loaded.value.as_ref().map(|package| &package.manifest),
        })).unwrap()).unwrap();
        assert!(loaded.value.is_some(), "legacy data reader: {issues:#?}");
    }
}

mod s5a {
    use super::*;
    use serde_json::{json, Value};

    #[cfg(windows)]
    #[test]
    #[ignore = "explicit existing S5 delivery; inspect the actual DLL UI producer without rebuilding Player"]
    fn retained_delivery_ui_producer_and_document_present() {
        use engine_runtime::aui::{ProjectUiStateReportMode, ProjectUiStateSnapshotCache};
        use engine_runtime::project_runtime_module::ProjectRuntimeBootstrap;
        use engine_runtime::project_runtime_native_adapter::linked_project_runtime_set_from_dll;
        let root = PathBuf::from(std::env::var_os("AIFE_S5_UI_DELIVERY").expect("retained delivery"));
        let package = engine_runtime::runtime_package::load_runtime_package(root.join("data/runtime_package")).value.unwrap();
        let dll = fs::read_dir(root.join("data/bin")).unwrap().map(|e|e.unwrap().path()).find(|p|p.extension().is_some_and(|e|e=="dll")).unwrap();
        let linked = linked_project_runtime_set_from_dll(&dll).unwrap();
        let mut parts = ProjectRuntimeBootstrap::bind(&package, &linked).unwrap().into_parts();
        let document_id = &package.aui_manifest.documents[0].document_id;
        let document = package.aui_documents.get(document_id).expect("loaded document identity");
        println!("document {document_id}, {} nodes", document.nodes.len());
        let mut paths: Vec<String> = document.nodes.iter().flat_map(|node| node.binding_refs.iter().map(|binding| binding.path.clone())).collect();
        paths.sort(); paths.dedup();
        let mut cache = ProjectUiStateSnapshotCache::new(paths);
        let resolved = cache.resolve(parts.ui_state_producer.as_mut(), 1, &package, &engine_runtime::world::World::new(), ProjectUiStateReportMode::Summary);
        println!("UI producer: {resolved:#?}");
        assert!(resolved.is_ok(), "{resolved:#?}");
        let mut present = engine_runtime::aui_present_cache::AuiPresentCache::for_package(&package);
        let mut interaction = engine_runtime::aui::AuiInteractionState::default();
        let output = present.take_or_rebuild(&package, &engine_runtime::world::World::new(), 2, parts.ui_state_producer.as_mut(), engine_runtime::aui_present_cache::AuiPresentContext::new(None, None, &mut interaction));
        println!("UI present: {:#?}", output.as_ref().map(|v| &v.report));
        assert!(output.is_some(), "first UI frame missing");
    }

    #[test]
    fn inspect_exposes_effective_import_budget_and_supported_runtime_domains() {
        let fixture = Fixture::new();
        fs::write(
            fixture.0.join("project.aife.json"),
            br#"{"schemaVersion":"aife-project.v3","projectId":"371.s5.inspect"}"#,
        )
        .unwrap();
        fs::create_dir(fixture.0.join("Assets")).unwrap();
        fs::create_dir(fixture.0.join("Models")).unwrap();
        fs::write(
            fixture.0.join("Assets/character.import.json"),
            include_bytes!(
                "../../../../samples/platform_3d_world_project/Assets/character.import.json"
            ),
        )
        .unwrap();
        fs::write(
            fixture.0.join("Models/character.gltf"),
            include_bytes!("../../../../samples/platform_3d_world_project/Models/character.gltf"),
        )
        .unwrap();
        let lease = fixture.lease();
        let report =
            project_authoring_execution::three_d_contract::inspect_spatial_project_snapshot(
                lease.snapshot(),
            );
        let value = serde_json::to_value(report).unwrap();
        let schemas = value["supportedSchemas"].as_array().unwrap();
        for schema in [
            "character3d.v1",
            "navigation3d-asset.v1",
            "collision3d-asset.v1",
            "terrain3d-asset.v1",
            "vegetation3d-asset.v1",
        ] {
            assert!(
                schemas.contains(&json!(schema)),
                "missing implemented domain {schema}"
            );
        }
        let asset = &value["assets"][0];
        assert_eq!(asset["sourcePath"], "Assets/character.import.json");
        assert_eq!(asset["schemaVersion"], "model-import.v1");
        let defaults = serde_json::to_value(
            project_authoring_execution::three_d_contract::ImportBudgets::default(),
        )
        .unwrap();
        assert_eq!(asset["effectiveBudgets"], defaults);
        assert_eq!(
            asset["sourceBytes"],
            fs::metadata(fixture.0.join("Models/character.gltf"))
                .unwrap()
                .len()
        );
        assert_eq!(value["qualification"], "not_assessed_by_inspect");
        assert_eq!(value["truncated"], false);
    }

    #[test]
    fn inspect_bounds_asset_rows_without_losing_total_or_relabeling_support() {
        let fixture = Fixture::new();
        fs::create_dir(fixture.0.join("Assets")).unwrap();
        for i in 0..70 {
            fs::write(
                fixture.0.join(format!("Assets/{i:03}.json")),
                serde_json::to_vec(
                    &json!({"schemaVersion":"project-data.v1", "assetId":format!("asset-{i}")}),
                )
                .unwrap(),
            )
            .unwrap();
        }
        let lease = fixture.lease();
        let report: Value = serde_json::to_value(
            project_authoring_execution::three_d_contract::inspect_spatial_project_snapshot(
                lease.snapshot(),
            ),
        )
        .unwrap();
        assert_eq!(report["assetCount"], 70);
        assert_eq!(report["assets"].as_array().unwrap().len(), 64);
        assert_eq!(report["truncated"], true);
        assert_eq!(report["assets"][0]["schemaVersion"], "project-data.v1");
        assert!(!report["supportedSchemas"]
            .as_array()
            .unwrap()
            .contains(&json!("project-data.v1")));
    }
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "aife-371-delivery-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(root.join("BuildProfiles")).unwrap();
        fs::write(root.join("BuildProfiles/windows.release.json"), include_bytes!("../../../../samples/platform_3d_benchmark_project/BuildProfiles/windows.release.json")).unwrap();
        fs::write(root.join("BuildProfiles/android.release.json"), include_bytes!("../../../../samples/platform_3d_benchmark_project/BuildProfiles/android.release.json")).unwrap();
        fs::write(
            root.join("project.aife.json"),
            br#"{"schemaVersion":"aife-project.v2","projectId":"371.delivery.fixture"}"#,
        )
        .unwrap();
        Self(root)
    }
    fn lease(&self) -> authoring_project_context::ProjectSnapshotLease {
        let mut s = ProjectAuthoringSession::open(&self.0).unwrap();
        let paths = s
            .source_inventory()
            .unwrap()
            .entries
            .into_iter()
            .map(|e| e.relative_path)
            .collect();
        s.acquire_snapshot_lease("371-delivery", paths).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn target_windows_release_check_is_not_dev_only() {
    let f = Fixture::new();
    let l = f.lease();
    let c = GameProjectCompiler::bind(&l).unwrap();
    let r = c
        .check(&l, CheckProfile::new(TargetProfile::WindowsRelease))
        .unwrap();
    assert_eq!(r.target_profile(), TargetProfile::WindowsRelease);
}

#[test]
fn target_release_requires_explicit_profile() {
    let f = Fixture::new();
    fs::remove_file(f.0.join("BuildProfiles/windows.release.json")).unwrap();
    let l = f.lease();
    let c = GameProjectCompiler::bind(&l).unwrap();
    assert_eq!(
        c.prepare(&l, TargetProfile::WindowsRelease)
            .unwrap_err()
            .code(),
        "game_project_compiler.build_profile_missing"
    );
}
#[test]
fn target_profile_mismatch_is_rejected_before_build() {
    let f = Fixture::new();
    let l = f.lease();
    let c = GameProjectCompiler::bind(&l).unwrap();
    let p = c.prepare(&l, TargetProfile::WindowsRelease).unwrap();
    let error = c
        .build(
            &p,
            project_authoring_execution::BuildRequest::new(TargetProfile::WindowsDev),
        )
        .unwrap_err();
    assert_eq!(
        error.code(),
        "game_project_compiler.target_profile_mismatch"
    );
}
#[test]
fn target_prepare_and_profile_consume_retained_source_after_live_change() {
    let f = Fixture::new();
    let l = f.lease();
    let c = GameProjectCompiler::bind(&l).unwrap();
    let before = c.prepare(&l, TargetProfile::WindowsRelease).unwrap();
    fs::write(
        f.0.join("BuildProfiles/windows.release.json"),
        b"invalid live profile",
    )
    .unwrap();
    let after = c.prepare(&l, TargetProfile::WindowsRelease).unwrap();
    assert_eq!(before.preparation_identity(), after.preparation_identity());
    let fresh = f.lease();
    let new = GameProjectCompiler::bind(&fresh).unwrap();
    assert!(new.prepare(&fresh, TargetProfile::WindowsRelease).is_err());
}
#[test]
fn target_identity_separates_windows_dev_release_and_android_release() {
    let f = Fixture::new();
    let l = f.lease();
    let c = GameProjectCompiler::bind(&l).unwrap();
    let dev = c.prepare(&l, TargetProfile::WindowsDev).unwrap();
    let pc = c.prepare(&l, TargetProfile::WindowsRelease).unwrap();
    let android = c.prepare(&l, TargetProfile::AndroidRelease).unwrap();
    assert_ne!(dev.preparation_identity(), pc.preparation_identity());
    assert_ne!(pc.preparation_identity(), android.preparation_identity());
    assert_eq!(
        serde_json::to_value(TargetProfile::AndroidRelease).unwrap(),
        "android-release"
    );
}
#[test]
fn target_android_release_rejects_dev_profile() {
    let f = Fixture::new();
    let path = f.0.join("BuildProfiles/android.release.json");
    let mut profile: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    profile["profile"] = "dev".into();
    fs::write(path, serde_json::to_vec(&profile).unwrap()).unwrap();
    let l = f.lease();
    let c = GameProjectCompiler::bind(&l).unwrap();
    assert!(c
        .prepare(&l, TargetProfile::AndroidRelease)
        .unwrap_err()
        .code()
        .contains("build_profile"));
}
#[test]
fn target_build_rejects_shipping_layout_instead_of_ignoring_metadata() {
    let f = Fixture::new();
    let path = f.0.join("BuildProfiles/windows.release.json");
    fs::write(
        &path,
        include_bytes!(
            "../../../../samples/switch_puzzle_project/BuildProfiles/windows.release.json"
        ),
    )
    .unwrap();
    let l = f.lease();
    let c = GameProjectCompiler::bind(&l).unwrap();
    let p = c.prepare(&l, TargetProfile::WindowsRelease).unwrap();
    assert_eq!(
        c.build(
            &p,
            project_authoring_execution::BuildRequest::new(TargetProfile::WindowsRelease)
        )
        .unwrap_err()
        .code(),
        "game_project_compiler.release_layout_pending"
    );
}
#[test]
fn target_prepare_remains_valid_when_live_project_source_is_unavailable() {
    let f = Fixture::new();
    let l = f.lease();
    let c = GameProjectCompiler::bind(&l).unwrap();
    let p = c.prepare(&l, TargetProfile::AndroidRelease).unwrap();
    fs::remove_file(f.0.join("project.aife.json")).unwrap();
    assert_eq!(
        p.preparation_identity(),
        c.prepare(&l, TargetProfile::AndroidRelease)
            .unwrap()
            .preparation_identity()
    );
}
#[test]
fn target_android_wrong_abi_is_rejected_even_without_toolchain() {
    use project_authoring_execution::android_export::*;
    let f = Fixture::new();
    let l = f.lease();
    let c = GameProjectCompiler::bind(&l).unwrap();
    let p = c.prepare(&l, TargetProfile::AndroidRelease).unwrap();
    let mut request = AndroidDevExportRequest::release(&f.0);
    request.abi = AndroidDevAbi::X86_64;
    let report = AndroidDevExportPipeline::export_prepared(request, &p);
    assert_eq!(report.status, AndroidDevExportStatus::Failed);
    assert_eq!(
        report.diagnostic.as_deref(),
        Some("android_export.target_profile_mismatch")
    );
    assert!(!f.0.join("Build").exists());
}

#[test]
fn target_build_rejects_live_drift_before_any_artifact_write() {
    let f = Fixture::new();
    let l = f.lease();
    let c = GameProjectCompiler::bind(&l).unwrap();
    let p = c.prepare(&l, TargetProfile::WindowsRelease).unwrap();
    fs::write(
        f.0.join("BuildProfiles/windows.release.json"),
        b"changed after prepare",
    )
    .unwrap();
    let error = c
        .build(
            &p,
            project_authoring_execution::BuildRequest::for_project(
                TargetProfile::WindowsRelease,
                &f.0,
                project_authoring_execution::ProjectRelativePath::parse("Build/Target").unwrap(),
                f.0.join("unused-player-cache"),
            ),
        )
        .unwrap_err();
    assert_eq!(
        error.code(),
        "game_project_compiler.execution_project_drifted"
    );
    assert!(!f.0.join("Build").exists());
    assert!(!f.0.join("unused-player-cache").exists());
}
