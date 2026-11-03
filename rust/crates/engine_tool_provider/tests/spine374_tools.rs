use engine_tool_provider::{
    CanonicalToolStatus, HostSessionContext, HostToolCall, NativeHostAdapter,
};
use serde_json::{json, Value};
use std::{fs, path::PathBuf};

fn write(root: &std::path::Path, path: &str, value: Value) {
    let p = root.join(path);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, serde_json::to_vec(&value).unwrap()).unwrap();
}
#[test]
fn spine374_d_existing_inspect_and_check_expose_domain_and_correctable_errors() {
    let parent = std::env::var_os("SPINE374_EVIDENCE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../.artifacts/374/tools-d")
        });
    let root = parent.join(format!("provider-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    write(
        &root,
        "project.aife.json",
        json!({"schemaVersion":"aife-project.v2","projectId":"spine374.tools"}),
    );
    write(
        &root,
        "Scenes/main.scene.json",
        json!({"schemaVersion":"scene.v1","id":"main","entities":[]}),
    );
    write(
        &root,
        "Assets/hero.spine.json",
        json!({"schemaVersion":"spine-skeleton-asset.v1","assetId":"hero","skeletonSource":"Assets/hero.skeleton.json","atlasSource":"Assets/hero.atlas","pages":{"probe.png":{"assetId":"texture-probe"}},"format":"json","exportVersion":"4.2.00","pixelsPerUnit":100}),
    );
    write(
        &root,
        "Assets/page.asset",
        json!({"schemaVersion":"texture-asset.v1","assetId":"texture-probe","assetGuid":"texture-probe","sourceImage":"Assets/page.png"}),
    );
    fs::write(
        root.join("Assets/hero.skeleton.json"),
        include_str!("../../engine_spine_runtime/tests/fixtures/probe.json"),
    )
    .unwrap();
    fs::write(
        root.join("Assets/hero.atlas"),
        include_str!("../../engine_spine_runtime/tests/fixtures/probe.atlas"),
    )
    .unwrap();
    // Check validates the source reference; pixel decoding belongs to prepare/Cook.
    fs::write(root.join("Assets/page.png"), b"check-only-source-reference").unwrap();
    let mut d = json!({"schema":"animator-description-2d.v2","assetId":"hero-controller","backend":"spine","skeleton":{"assetId":"hero"},"default":"idle","layers":[{"name":"base","blend":"replace"}],"animations":{"idle":{"clip":"motion","layer":"base","playback":"loop","speedPermille":1000,"mixInSeconds":0.1}}});
    write(
        &root,
        "Animations/hero.animator-description-2d.json",
        d.clone(),
    );
    let mut provider = NativeHostAdapter::attach(HostSessionContext {
        session_id: "spine374-tools".into(),
        workspace_root: root.clone(),
        project_root: Some(root.clone()),
    })
    .unwrap();
    let inspect = provider.invoke(HostToolCall {
        call_id: "inspect".into(),
        tool_name: "engine_project_inspect".into(),
        arguments: json!({}),
        approved: false,
    });
    assert_eq!(
        inspect.status,
        CanonicalToolStatus::Completed,
        "{inspect:#?}"
    );
    assert_eq!(
        inspect.output["spineInspection"]["capability"],
        "animator.spine.4.2.v1"
    );
    assert_eq!(
        inspect.output["spineInspection"]["capabilityStatus"],
        if cfg!(feature = "spine") {
            "planned"
        } else {
            "unsupported"
        }
    );
    assert_eq!(
        inspect.output["spineInspection"]["controllers"][0]["actions"][0]["mixInTicks"],
        6
    );
    let check = provider.invoke(HostToolCall {
        call_id: "check".into(),
        tool_name: "engine_project_check".into(),
        arguments: json!({}),
        approved: true,
    });
    if cfg!(feature = "spine") {
        assert_eq!(check.status, CanonicalToolStatus::Completed, "{check:#?}");
        assert_eq!(
            check.output["checkReport"]["spineInspection"]["controllers"][0]["defaultAction"],
            "idle"
        );
        d["animations"]["idle"]["clip"] = json!("absent");
        write(&root, "Animations/hero.animator-description-2d.json", d);
        let bad = provider.invoke(HostToolCall {
            call_id: "bad-check".into(),
            tool_name: "engine_project_check".into(),
            arguments: json!({}),
            approved: true,
        });
        assert_eq!(
            bad.status,
            CanonicalToolStatus::RejectedByEngine,
            "{bad:#?}"
        );
        assert!(
            bad.diagnostics
                .iter()
                .any(|d| d.code.starts_with("game_project_compiler.animator.")),
            "{bad:#?}"
        );
    } else {
        assert_eq!(
            check.status,
            CanonicalToolStatus::RejectedByEngine,
            "{check:#?}"
        );
        assert!(
            check
                .diagnostics
                .iter()
                .any(|d| d.code.contains("backend_not_compiled")),
            "{check:#?}"
        );
    }
}
