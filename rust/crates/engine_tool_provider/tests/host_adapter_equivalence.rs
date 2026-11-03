use engine_tool_provider::mcp_stdio::McpHostAdapter;
use engine_tool_provider::{HostSessionContext, HostToolCall, NativeHostAdapter, ToolSideEffect};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn native_provider_conformance_uses_concrete_definitions_and_canonical_results() {
    let fixture = Fixture::new("native");
    let mut adapter = NativeHostAdapter::attach(fixture.host()).unwrap();
    let definitions = adapter.tool_definitions();
    assert_eq!(
        definitions
            .iter()
            .map(|definition| definition.name.as_str())
            .collect::<Vec<_>>(),
        [
            "engine_project_inspect",
            "engine_project_check",
            "engine_project_mutate",
            "engine_project_rollback",
            "engine_runtime_run",
            "engine_project_build",
            "engine_runtime_playtest",
            "engine_runtime_observe",
            "engine_delivery_verify",
        ]
    );
    assert!(definitions
        .iter()
        .all(|definition| !definition.supports_cancellation));
    let result = adapter.invoke(HostToolCall {
        call_id: "native-inspect".to_string(),
        tool_name: "engine_project_inspect".to_string(),
        arguments: json!({}),
        approved: false,
    });
    assert_eq!(result.tool_name, "engine_project_inspect");
    assert!(result.project_revision.is_some());
    assert!(adapter.observe(&result.operation_id).unwrap().terminal);
}

#[test]
fn native_and_mcp_adapters_reject_the_same_hidden_tool_call() {
    let fixture = Fixture::new("hidden-equivalence");
    let mut native = NativeHostAdapter::attach(fixture.host()).unwrap();
    let mut mcp = McpHostAdapter::attach(fixture.host()).unwrap();

    let native_result = native.invoke(HostToolCall {
        call_id: "hidden-equivalent-call".to_string(),
        tool_name: "engine_project_diagnostics".to_string(),
        arguments: json!({}),
        approved: false,
    });
    let mcp_result = mcp.invoke_canonical(
        "hidden-equivalent-call",
        "engine_project_diagnostics",
        json!({}),
    );

    assert_eq!(native_result, mcp_result);
    assert_eq!(
        native_result.status,
        engine_tool_provider::CanonicalToolStatus::RejectedByEngine
    );
    assert_eq!(
        native_result.diagnostics[0].code,
        "engine_provider.tool_not_exposed"
    );
}

#[test]
fn native_and_mcp_adapters_return_the_same_canonical_engine_outcome() {
    let fixture = Fixture::new("equivalence");
    let mut native = NativeHostAdapter::attach(fixture.host()).unwrap();
    let mut mcp = McpHostAdapter::attach(fixture.host()).unwrap();
    assert_eq!(native.tool_definitions(), mcp.tool_definitions());
    let definition = native
        .tool_definitions()
        .into_iter()
        .find(|definition| definition.name == "engine_project_inspect")
        .unwrap();
    assert_eq!(definition.side_effect, ToolSideEffect::Read);

    let native_result = native.invoke(HostToolCall {
        call_id: "equivalent-call".to_string(),
        tool_name: "engine_project_inspect".to_string(),
        arguments: json!({}),
        approved: false,
    });
    let mcp_result = mcp.invoke_canonical("equivalent-call", "engine_project_inspect", json!({}));

    assert_eq!(native_result, mcp_result);
}

#[test]
fn native_and_mcp_mutation_have_identical_receipt_evidence_and_side_effects() {
    let fixture = Fixture::new("mutation-equivalence");
    let host = fixture.host();
    let arguments = json!({
        "goal":"add equivalent config",
        "domain":"config",
        "changes":[{
            "operation":"create_or_replace",
            "path":"Data/equivalent.json",
            "content":"{\"enabled\":true}"
        }]
    });

    let mut native = NativeHostAdapter::attach(host.clone()).unwrap();
    let native_result = native.invoke(HostToolCall {
        call_id: "equivalent-mutation".to_string(),
        tool_name: "engine_project_mutate".to_string(),
        arguments: arguments.clone(),
        approved: true,
    });
    let expected_bytes = fs::read(fixture.root.join("Data/equivalent.json")).unwrap();
    let rollback_ref = native_result.receipt_ref.clone().unwrap();
    let rollback = native.invoke(HostToolCall {
        call_id: "equivalent-rollback".to_string(),
        tool_name: "engine_project_rollback".to_string(),
        arguments: json!({"rollbackRef":rollback_ref}),
        approved: true,
    });
    assert_eq!(
        rollback.status,
        engine_tool_provider::CanonicalToolStatus::Completed
    );
    assert!(!fixture.root.join("Data/equivalent.json").exists());
    fs::remove_dir_all(fixture.root.join(".aife")).unwrap();

    let mut mcp = McpHostAdapter::attach(host).unwrap();
    let mcp_result =
        mcp.invoke_canonical("equivalent-mutation", "engine_project_mutate", arguments);

    assert_eq!(native_result, mcp_result);
    assert_eq!(
        fs::read(fixture.root.join("Data/equivalent.json")).unwrap(),
        expected_bytes
    );
    let evidence = mcp_result.evidence_refs.first().unwrap();
    let receipt_path = evidence.strip_prefix("project-evidence:").unwrap();
    assert!(fixture.root.join(receipt_path).is_file());
}

#[test]
fn check_diagnostics_preserve_source_location_across_adapters() {
    let fixture = Fixture::new("check-location");
    fs::write(fixture.root.join("project.aife.json"),
        br#"{"schemaVersion":"aife-project.v2","projectId":"provider.adapter","defaultScene":"Scenes/Missing.scene.json"}"#).unwrap();
    let mut native = NativeHostAdapter::attach(fixture.host()).unwrap();
    let mut mcp = McpHostAdapter::attach(fixture.host()).unwrap();
    let result = native.invoke(HostToolCall {
        call_id: "check-location".into(),
        tool_name: "engine_project_check".into(),
        arguments: json!({}),
        approved: true,
    });
    assert_eq!(
        result,
        mcp.invoke_canonical("check-location", "engine_project_check", json!({}))
    );
    let value = serde_json::to_value(&result).unwrap();
    assert!(
        value["diagnostics"][0]["sourceLocation"]["sourcePath"].is_string(),
        "{value:#}"
    );
    let denied = native.invoke(HostToolCall {
        call_id: "check-denied".into(),
        tool_name: "engine_project_check".into(),
        arguments: json!({}),
        approved: false,
    });
    assert_ne!(
        denied.status,
        engine_tool_provider::CanonicalToolStatus::Completed
    );
    assert_eq!(
        denied.diagnostics[0].code,
        "engine_provider.host_approval_required"
    );
}

struct Fixture {
    root: PathBuf,
}

#[test]
fn import_decode_check_evidence_is_preserved_across_native_and_mcp() {
    let fixture = Fixture::new("import-decode");
    let sources = [
        (
            "Scenes/main.json",
            include_bytes!(
                "../../project_authoring_execution/tests/fixtures/three_d_a0/scene.json"
            )
            .as_slice(),
        ),
        (
            "Assets/cube.json",
            include_bytes!("../../project_authoring_execution/tests/fixtures/three_d_a0/mesh.json")
                .as_slice(),
        ),
        (
            "Assets/blue.json",
            include_bytes!(
                "../../project_authoring_execution/tests/fixtures/three_d_a0/material.json"
            )
            .as_slice(),
        ),
        (
            "Assets/import.json",
            include_bytes!(
                "../../project_authoring_execution/tests/fixtures/three_d_a0/import.json"
            )
            .as_slice(),
        ),
    ];
    for (path, bytes) in sources {
        let target = fixture.root.join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, bytes).unwrap();
    }
    fs::write(fixture.root.join("project.aife.json"), br#"{"schemaVersion":"aife-project.v3","projectId":"provider.adapter","defaultScene":"Scenes/main.json"}"#).unwrap();
    fs::create_dir_all(fixture.root.join("Models")).unwrap();
    fs::write(
        fixture.root.join("Models/scene.gltf"),
        br#"{"asset":{"version":"2.0"},"scenes":[{"nodes":[]}]}"#,
    )
    .unwrap();
    let import_path = fixture.root.join("Assets/import.json");
    let mut descriptor: serde_json::Value =
        serde_json::from_slice(&fs::read(&import_path).unwrap()).unwrap();
    let mut output = descriptor["outputs"][1].clone();
    use sha2::{Digest, Sha256};
    output["sourceFingerprint"] = json!(format!("sha256:{:x}", Sha256::digest(br#"{"nodes":[]}"#)));
    descriptor["outputs"] = json!([output]);
    descriptor["nodes"] = json!([]);
    fs::write(&import_path, serde_json::to_vec(&descriptor).unwrap()).unwrap();
    let mut native = NativeHostAdapter::attach(fixture.host()).unwrap();
    let mut mcp = McpHostAdapter::attach(fixture.host()).unwrap();
    let result = native.invoke(HostToolCall {
        call_id: "import-decode".into(),
        tool_name: "engine_project_check".into(),
        arguments: json!({}),
        approved: true,
    });
    assert_eq!(
        result,
        mcp.invoke_canonical("import-decode", "engine_project_check", json!({}))
    );
    assert_eq!(
        result.status,
        engine_tool_provider::CanonicalToolStatus::Completed,
        "{result:?}"
    );
    let report = &result.output["checkReport"]["modelImports"][0];
    assert_eq!(report["stage"], "assembly");
    assert_eq!(report["assemblyAvailable"], true);
    assert_eq!(report["dependencies"][0]["sourcePath"], "Models/scene.gltf");
    assert!(report["sourceDigest"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    fs::write(
        fixture.root.join("Models/scene.gltf"),
        br#"{"asset":{"version":"1.0"}}"#,
    )
    .unwrap();
    let error = native.invoke(HostToolCall {
        call_id: "import-invalid".into(),
        tool_name: "engine_project_check".into(),
        arguments: json!({}),
        approved: true,
    });
    assert_eq!(
        error,
        mcp.invoke_canonical("import-invalid", "engine_project_check", json!({}))
    );
    let error = serde_json::to_value(error).unwrap();
    assert!(error["diagnostics"][0]["code"]
        .as_str()
        .unwrap()
        .ends_with("import_version_unsupported"));
    assert_eq!(
        error["diagnostics"][0]["sourceLocation"]["sourcePath"],
        "Models/scene.gltf"
    );
    assert_eq!(
        error["diagnostics"][0]["sourceLocation"]["fieldPath"],
        "/asset"
    );
}

#[test]
fn semantic_adapters_share_approval_reference_and_schema_rejections() {
    let fixture = Fixture::new("semantic-equivalence");
    let mut native = NativeHostAdapter::attach(fixture.host()).unwrap();
    let mut mcp = McpHostAdapter::attach(fixture.host()).unwrap();
    assert_eq!(native.tool_definitions(), mcp.tool_definitions());
    for (index, (tool, arguments, approved, code)) in [
        (
            "engine_runtime_playtest",
            json!({}),
            false,
            "engine_provider.host_approval_required",
        ),
        (
            "engine_runtime_playtest",
            json!({"deliveryRef":"unknown"}),
            true,
            "engine_provider.delivery_ref_unknown",
        ),
        (
            "engine_runtime_observe",
            json!({"runRef":"unknown"}),
            false,
            "engine_provider.run_ref_unknown",
        ),
        (
            "engine_runtime_observe",
            json!({"runRef":"x", "path":"C:/secret"}),
            false,
            "engine_provider.input_schema_invalid",
        ),
        (
            "engine_runtime_playtest",
            json!({"deliveryRef":null}),
            true,
            "engine_provider.input_constraint_failed",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let id = format!("semantic-reject-{index}");
        let result = native.invoke(HostToolCall {
            call_id: id.clone(),
            tool_name: tool.into(),
            arguments: arguments.clone(),
            approved,
        });
        assert_eq!(
            result,
            mcp.invoke_canonical_with_approval(id, tool, arguments, approved)
        );
        assert_eq!(result.diagnostics[0].code, code);
        assert!(result.evidence_refs.is_empty());
    }
    assert!(
        !fixture.root.join("Library").exists(),
        "rejections must not reach generated output or Player"
    );
}

impl Fixture {
    fn new(label: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "aife-host-adapter-{label}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("project.aife.json"),
            br#"{"schemaVersion":"aife-project.v2","projectId":"provider.adapter"}"#,
        )
        .unwrap();
        fs::write(root.join("game.rs"), b"fn game() {}").unwrap();
        Self { root }
    }

    fn host(&self) -> HostSessionContext {
        HostSessionContext {
            session_id: "adapter-equivalence".to_string(),
            workspace_root: self.root.clone(),
            project_root: Some(self.root.clone()),
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn reimport_inspect_revision_bound_mutation_and_rollback_use_context_owner() {
    use engine_tool_provider::CanonicalToolStatus;
    use sha2::{Digest, Sha256};
    let fixture = Fixture::new("import-cas");
    for (path, bytes) in [
        (
            "Scenes/main.json",
            include_bytes!(
                "../../project_authoring_execution/tests/fixtures/three_d_a0/scene.json"
            )
            .as_slice(),
        ),
        (
            "Assets/cube.json",
            include_bytes!("../../project_authoring_execution/tests/fixtures/three_d_a0/mesh.json")
                .as_slice(),
        ),
        (
            "Assets/blue.json",
            include_bytes!(
                "../../project_authoring_execution/tests/fixtures/three_d_a0/material.json"
            )
            .as_slice(),
        ),
    ] {
        let target = fixture.root.join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, bytes).unwrap();
    }
    fs::write(fixture.root.join("project.aife.json"),br#"{"schemaVersion":"aife-project.v3","projectId":"provider.adapter","defaultScene":"Scenes/main.json"}"#).unwrap();
    fs::create_dir_all(fixture.root.join("Models")).unwrap();
    fs::create_dir_all(fixture.root.join("Assets")).unwrap();
    fs::write(
        fixture.root.join("Models/scene.gltf"),
        br#"{"asset":{"version":"2.0"},"scenes":[{"nodes":[]}]}"#,
    )
    .unwrap();
    let mut descriptor: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../project_authoring_execution/tests/fixtures/three_d_a0/import.json"
    ))
    .unwrap();
    descriptor["outputs"] = json!([]);
    descriptor["nodes"] = json!([]);
    let original = serde_json::to_vec(&descriptor).unwrap();
    fs::write(fixture.root.join("Assets/import.json"), &original).unwrap();
    let mut native = NativeHostAdapter::attach(fixture.host()).unwrap();
    let mut mcp = McpHostAdapter::attach(fixture.host()).unwrap();
    let request = json!({"modelImport":{"descriptorPath":"Assets/import.json"}});
    let inspect = native.invoke(HostToolCall {
        call_id: "import-plan".into(),
        tool_name: "engine_project_inspect".into(),
        arguments: request.clone(),
        approved: false,
    });
    assert_eq!(
        inspect,
        mcp.invoke_canonical("import-plan", "engine_project_inspect", request.clone())
    );
    assert_eq!(
        inspect.status,
        CanonicalToolStatus::Completed,
        "{inspect:?}"
    );
    let plan = &inspect.output["modelImportInspection"];
    assert_eq!(plan["buildable"], false);
    assert!(plan["requiredObjects"]
        .as_array()
        .unwrap()
        .iter()
        .any(|o| o["sourceLocator"] == "/scenes/0"));
    assert_eq!(
        fs::read(fixture.root.join("Assets/import.json")).unwrap(),
        original
    );
    descriptor["outputs"] = json!([{"key":"persistent-prefab","assetId":"import.prefab","assetGuid":"guid-import-prefab","kind":"prefab","sourceLocator":"/scenes/0","sourceFingerprint":format!("sha256:{:x}",Sha256::digest(br#"{"nodes":[]}"#)),"materialSlots":[]}]);
    let mut mutation = json!({"goal":"explicit first import mapping","domain":"assets","expectedRevisionId":plan["expectedRevisionId"],"changes":[{"operation":"create_or_replace","path":"Assets/import.json","content":serde_json::to_string(&descriptor).unwrap()}]});
    fs::write(
        fixture.root.join("game.rs"),
        b"fn game() { /* changed after preview */ }",
    )
    .unwrap();
    let stale = native.invoke(HostToolCall {
        call_id: "import-stale".into(),
        tool_name: "engine_project_mutate".into(),
        arguments: mutation.clone(),
        approved: true,
    });
    assert_ne!(stale.status, CanonicalToolStatus::Completed);
    assert!(stale.receipt_ref.is_none());
    assert!(stale.diagnostics[0].code.contains("revision"), "{stale:?}");
    assert_eq!(
        fs::read(fixture.root.join("Assets/import.json")).unwrap(),
        original
    );
    let fresh = native.invoke(HostToolCall {
        call_id: "import-fresh".into(),
        tool_name: "engine_project_inspect".into(),
        arguments: request.clone(),
        approved: false,
    });
    mutation["expectedRevisionId"] =
        fresh.output["modelImportInspection"]["expectedRevisionId"].clone();
    let applied = native.invoke(HostToolCall {
        call_id: "import-apply".into(),
        tool_name: "engine_project_mutate".into(),
        arguments: mutation,
        approved: true,
    });
    assert_eq!(
        applied.status,
        CanonicalToolStatus::Completed,
        "{applied:?}"
    );
    assert_ne!(applied.project_revision, fresh.project_revision);
    let after = native.invoke(HostToolCall {
        call_id: "import-after".into(),
        tool_name: "engine_project_inspect".into(),
        arguments: request,
        approved: false,
    });
    assert_eq!(
        after.output["modelImportInspection"]["buildable"], true,
        "{after:?}"
    );
    let checked = native.invoke(HostToolCall {
        call_id: "import-checked".into(),
        tool_name: "engine_project_check".into(),
        arguments: json!({}),
        approved: true,
    });
    assert_eq!(
        checked.status,
        CanonicalToolStatus::Completed,
        "{checked:?}"
    );
    let old_model = fs::read(fixture.root.join("Models/scene.gltf")).unwrap();
    let old_mapping = fs::read(fixture.root.join("Assets/import.json")).unwrap();
    let renamed_scene = json!({"nodes":[],"name":"Renamed"});
    let renamed_model = json!({"asset":{"version":"2.0"},"scenes":[renamed_scene]});
    descriptor["outputs"][0]["sourceFingerprint"] = json!(format!(
        "sha256:{:x}",
        Sha256::digest(serde_json::to_vec(&renamed_scene).unwrap())
    ));
    let reimport=native.invoke(HostToolCall {call_id:"import-source-and-mapping".into(),tool_name:"engine_project_mutate".into(),arguments:json!({"goal":"explicit reimport keeps persistent prefab identity","expectedRevisionId":checked.project_revision,"changes":[
        {"operation":"create_or_replace","path":"Models/scene.gltf","content":serde_json::to_string(&renamed_model).unwrap()},
        {"operation":"create_or_replace","path":"Assets/import.json","content":serde_json::to_string(&descriptor).unwrap()}
    ]}),approved:true});
    assert_eq!(
        reimport.status,
        CanonicalToolStatus::Completed,
        "{reimport:?}"
    );
    let checked_again = native.invoke(HostToolCall {
        call_id: "reimport-checked".into(),
        tool_name: "engine_project_check".into(),
        arguments: json!({}),
        approved: true,
    });
    assert_eq!(
        checked_again.status,
        CanonicalToolStatus::Completed,
        "{checked_again:?}"
    );
    assert_ne!(checked_again.project_revision, checked.project_revision);
    let rollback_reimport = native.invoke(HostToolCall {
        call_id: "reimport-rollback".into(),
        tool_name: "engine_project_rollback".into(),
        arguments: json!({"rollbackRef":reimport.receipt_ref.unwrap()}),
        approved: true,
    });
    assert_eq!(
        rollback_reimport.status,
        CanonicalToolStatus::Completed,
        "{rollback_reimport:?}"
    );
    assert_eq!(
        fs::read(fixture.root.join("Models/scene.gltf")).unwrap(),
        old_model
    );
    assert_eq!(
        fs::read(fixture.root.join("Assets/import.json")).unwrap(),
        old_mapping
    );
    let rollback = native.invoke(HostToolCall {
        call_id: "import-rollback".into(),
        tool_name: "engine_project_rollback".into(),
        arguments: json!({"rollbackRef":applied.receipt_ref.unwrap()}),
        approved: true,
    });
    assert_eq!(
        rollback.status,
        CanonicalToolStatus::Completed,
        "{rollback:?}"
    );
    assert_eq!(
        fs::read(fixture.root.join("Assets/import.json")).unwrap(),
        original
    );
    assert_eq!(
        fs::read(fixture.root.join("game.rs")).unwrap(),
        b"fn game() { /* changed after preview */ }"
    );
}
