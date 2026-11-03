use super::*;

fn project() -> Fixture {
    let fixture = Fixture::new();
    fixture.write("Assets/import.json", &strict_json(IMPORT).unwrap());
    fixture.write("Models/scene.gltf", &json!({
        "asset":{"version":"2.0"}, "scenes":[{"nodes":[0]}], "nodes":[{"mesh":0}],
        "meshes":[{"primitives":[{"attributes":{"POSITION":0},"material":0}]}],
        "materials":[{"extensions":{"KHR_materials_unlit":{}}}],
        "buffers":[{"uri":"triangle.bin","byteLength":36},{"uri":"unused.bin","byteLength":2}],
        "bufferViews":[{"buffer":0,"byteLength":36}],
        "accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[-1,0,0],"max":[1,1,0]}]
    }));
    let bytes: Vec<u8> = [-1.0f32, 0., 0., 1., 0., 0., 0., 1., 0.]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    fs::write(fixture.root.join("Models/triangle.bin"), bytes).unwrap();
    fs::write(fixture.root.join("Models/unused.bin"), [1, 2]).unwrap();
    let model: Value =
        serde_json::from_slice(&fs::read(fixture.root.join("Models/scene.gltf")).unwrap()).unwrap();
    fixture.write(
        "Assets/import.json",
        &super::import_scene::descriptor_for(&model, strict_json(IMPORT).unwrap()),
    );
    fixture
}

#[test]
fn import_decode_check_uses_immutable_snapshot_and_reports_dependency_identities() {
    let f = project();
    let (compiler, lease) = f.compiler();
    let profile = || CheckProfile::new(TargetProfile::WindowsDev);
    let report = compiler.check(&lease, profile()).unwrap();
    let imported = &report.model_imports[0];
    assert_eq!(imported.stage, "assembly");
    assert_eq!(imported.status, "accepted");
    assert!(imported.assembly_available);
    assert_eq!((imported.vertices, imported.indices), (3, 3));
    assert_eq!(imported.geometry_digests.len(), 1);
    assert_eq!(imported.dependencies.len(), 2);
    assert!(imported
        .dependencies
        .iter()
        .any(|d| d.source_path == "Models/triangle.bin"));
    assert!(imported.unconsumed.contains("/buffers/1"));
    let mut bytes = fs::read(f.root.join("Models/triangle.bin")).unwrap();
    bytes[..4].copy_from_slice(&(-0.5f32).to_le_bytes());
    fs::write(f.root.join("Models/triangle.bin"), bytes).unwrap();
    let captured = compiler.check(&lease, profile()).unwrap();
    assert_eq!(captured.model_imports, report.model_imports);
    let (fresh, new_lease) = f.compiler();
    let changed = fresh.check(&new_lease, profile()).unwrap();
    assert_ne!(
        changed.model_imports[0].source_digest,
        imported.source_digest
    );
    assert_ne!(
        changed.model_imports[0].geometry_digests,
        imported.geometry_digests
    );
    let before = fs::read(f.root.join("Assets/import.json")).unwrap();
    fresh
        .prepare(&new_lease, TargetProfile::WindowsDev)
        .unwrap();
    assert_eq!(before, fs::read(f.root.join("Assets/import.json")).unwrap());
    if let Some(root) = std::env::var_os("AIFE_A21_EVIDENCE_ROOT") {
        let root = std::path::PathBuf::from(root);
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("compiler-import-check.json"),
            serde_json::to_vec_pretty(
                &json!({"original":report,"retainedSnapshot":captured,"changedDependency":changed}),
            )
            .unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn import_decode_unused_binary_does_not_enter_consumed_dependency_digest() {
    let f = project();
    let (c, l) = f.compiler();
    let before = c
        .check(&l, CheckProfile::new(TargetProfile::WindowsDev))
        .unwrap();
    fs::write(f.root.join("Models/unused.bin"), [3, 4]).unwrap();
    let (c, l) = f.compiler();
    let after = c
        .check(&l, CheckProfile::new(TargetProfile::WindowsDev))
        .unwrap();
    assert_ne!(before.revision_id(), after.revision_id());
    assert_eq!(before.model_imports, after.model_imports);
}

#[test]
fn import_decode_check_reports_source_uri_location_without_live_fallback() {
    let f = project();
    let mut model: Value =
        serde_json::from_slice(&fs::read(f.root.join("Models/scene.gltf")).unwrap()).unwrap();
    model["buffers"][0]["uri"] = json!("https://invalid.example/triangle.bin");
    f.write("Models/scene.gltf", &model);
    let error = f.check_error();
    assert!(
        error.code().ends_with("three_d.import_uri_unsafe"),
        "{error:?}"
    );
    let d = &error.diagnostics()[0];
    assert_eq!(
        d.location.as_ref().unwrap().source_path,
        "Models/scene.gltf"
    );
    assert_eq!(
        d.location.as_ref().unwrap().field_path.as_deref(),
        Some("/buffers/0/uri")
    );
}

#[test]
fn budget_check_rejects_external_dependency_total_with_existing_error_contract() {
    let f = project();
    let mut descriptor = strict_json(IMPORT).unwrap();
    let primary = fs::metadata(f.root.join("Models/scene.gltf"))
        .unwrap()
        .len();
    let mut budgets = ImportBudgets::default();
    budgets.source_bytes = primary + 35;
    descriptor["budgets"] = serde_json::to_value(budgets).unwrap();
    f.write("Assets/import.json", &descriptor);
    let error = f.check_error();
    assert!(
        error.code().ends_with("three_d.source_budget_exceeded"),
        "{error:?}"
    );
    assert_eq!(
        error.diagnostics()[0]
            .location
            .as_ref()
            .unwrap()
            .source_path,
        "Models/scene.gltf"
    );
}

#[test]
fn import_decode_check_rejects_reparse_source_before_parser() {
    #[cfg(windows)]
    {
        // Existing inventory owner already rejects junction/reparse entries. A
        // directory junction needs no Developer Mode, unlike file symlinks.
        let f = project();
        let outside = Fixture::empty("aife-project.v3");
        let target = f.root.join("Models/escape");
        let output=std::process::Command::new("powershell").args(["-NoProfile","-Command","New-Item -ItemType Junction -Path $env:AIFE_IMPORT_LINK -Target $env:AIFE_IMPORT_LINK_TARGET | Out-Null"]).env("AIFE_IMPORT_LINK",&target).env("AIFE_IMPORT_LINK_TARGET",&outside.root).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let rejected = ProjectAuthoringSession::open(&f.root).is_err();
        fs::remove_dir(&target).unwrap();
        assert!(rejected, "reparse must fail during inventory capture");
    }
}
