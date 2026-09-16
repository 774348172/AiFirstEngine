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
