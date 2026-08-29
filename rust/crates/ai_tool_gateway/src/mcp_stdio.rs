use crate::{ClientKind, GatewayRemoteAdapter, GatewayReplyPayload, GatewayRequestPayload};
use editor_core::{
    AiToolCatalog, AiToolCatalogRequest, AiToolContractRegistry, AiToolDescriptor,
    AiToolInvocation, AiToolInvocationPayload, AiToolSideEffect, AI_TOOL_INVOCATION_SCHEMA_VERSION,
    TOOL_ID_PROJECT_CREATE, TOOL_ID_PROJECT_INSPECT,
};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

pub const MCP_PROTOCOL_VERSION: &str = "2025-06-18";

pub fn run_mcp_stdio(
    discovery_path: &Path,
    input: impl BufRead,
    mut output: impl Write,
) -> Result<(), String> {
    let mut adapter = GatewayRemoteAdapter::connect_from_discovery(
        discovery_path,
        ClientKind::Mcp,
        "ai-first-game-engine-mcp.v1",
    )
    .map_err(|error| format!("{}: {}", error.code, error.message))?;
    for line in input.lines() {
        let line = line.map_err(|error| error.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = serde_json::from_str(&line).map_err(|error| error.to_string())?;
        let shutdown = request.get("method").and_then(Value::as_str) == Some("shutdown");
        if let Some(response) = handle_mcp_request(&mut adapter, request) {
            serde_json::to_writer(&mut output, &response).map_err(|error| error.to_string())?;
            output.write_all(b"\n").map_err(|error| error.to_string())?;
            output.flush().map_err(|error| error.to_string())?;
        }
        if shutdown || adapter.restart_required() {
            break;
        }
    }
    let _ = adapter.close();
    Ok(())
}

pub fn handle_mcp_request(adapter: &mut GatewayRemoteAdapter, request: Value) -> Option<Value> {
    let id = request.get("id").cloned();
    let method = request.get("method").and_then(Value::as_str)?;
    if id.is_none() {
        return None;
    }
    let id = id.unwrap();
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {"tools": {"listChanged": false}},
            "serverInfo": {"name": "ai-first-game-engine", "version": env!("CARGO_PKG_VERSION")}
        })),
        "ping" => Ok(json!({})),
        "shutdown" => Ok(json!({})),
        "tools/list" => list_tools(adapter),
        "tools/call" => call_tool(adapter, request.get("params").cloned().unwrap_or_default()),
        _ => Err((-32601, format!("Unsupported MCP method '{method}'."))),
    };
    Some(match result {
        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
        Err((code, message)) => {
            json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
        }
    })
}

fn list_tools(adapter: &mut GatewayRemoteAdapter) -> Result<Value, (i64, String)> {
    let reply = adapter
        .dispatch(GatewayRequestPayload::Catalog(
            AiToolCatalogRequest::default(),
        ))
        .map_err(|error| (-32000, format!("{}: {}", error.code, error.message)))?;
    match reply.payload {
        GatewayReplyPayload::Catalog(catalog) => Ok(json!({"tools": mcp_tools(&catalog)})),
        GatewayReplyPayload::Rejected(diagnostic) => Err((
            -32000,
            format!("{}: {}", diagnostic.code, diagnostic.message),
        )),
        _ => Err((
            -32603,
            "Gateway returned a non-Catalog reply for tools/list.".to_string(),
        )),
    }
}

fn call_tool(adapter: &mut GatewayRemoteAdapter, params: Value) -> Result<Value, (i64, String)> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| (-32602, "tools/call requires a tool name.".to_string()))?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let payload = match name {
        "aife_status" => {
            let _: EmptyArguments = decode_arguments(arguments)?;
            GatewayRequestPayload::SessionStatus
        }
        "aife_catalog" => {
            let _: EmptyArguments = decode_arguments(arguments)?;
            GatewayRequestPayload::Catalog(AiToolCatalogRequest::default())
        }
        "aife_observe" => {
            let arguments = decode_operation_arguments(arguments)?;
            GatewayRequestPayload::Observe {
                operation_id: arguments.operation_id,
            }
        }
        "aife_cancel" => {
            let arguments = decode_operation_arguments(arguments)?;
            GatewayRequestPayload::CancelSessionBound {
                operation_id: arguments.operation_id,
            }
        }
        _ => projected_tool_request(adapter, name, arguments)?,
    };
    let reply = adapter
        .dispatch(payload)
        .map_err(|error| (-32000, format!("{}: {}", error.code, error.message)))?;
    let is_error = matches!(reply.payload, GatewayReplyPayload::Rejected(_));
    let mut structured =
        serde_json::to_value(reply).map_err(|error| (-32603, error.to_string()))?;
    let session_binding =
        serde_json::to_value(adapter.binding()).map_err(|error| (-32603, error.to_string()))?;
    structured
        .as_object_mut()
        .ok_or_else(|| {
            (
                -32603,
                "Gateway reply must serialize as an object.".to_string(),
            )
        })?
        .insert("sessionBinding".to_string(), session_binding);
    Ok(json!({
        "content": [{"type": "text", "text": serde_json::to_string(&structured).unwrap_or_default()}],
        "structuredContent": structured,
        "isError": is_error
    }))
}

fn projected_tool_request(
    adapter: &mut GatewayRemoteAdapter,
    name: &str,
    arguments: Value,
) -> Result<GatewayRequestPayload, (i64, String)> {
    let registry = AiToolContractRegistry::new();
    let descriptor = registry
        .descriptors()
        .iter()
        .find(|descriptor| mcp_name_for_tool_id(&descriptor.tool_id) == name)
        .ok_or_else(|| (-32602, format!("Unknown MCP tool '{name}'.")))?;
    if descriptor.tool_id == TOOL_ID_PROJECT_INSPECT {
        return registry
            .decode_inspect_request(arguments)
            .map(GatewayRequestPayload::Inspect)
            .map_err(invalid_direct_input);
    }

    let payload = registry
        .decode_invocation_payload(&descriptor.tool_id, arguments)
        .map_err(invalid_direct_input)?;
    if descriptor.tool_id == TOOL_ID_PROJECT_CREATE {
        return Ok(GatewayRequestPayload::ExecuteSessionBound {
            invocation: AiToolInvocation {
                schema_version: AI_TOOL_INVOCATION_SCHEMA_VERSION.to_string(),
                invocation_id: next_mcp_invocation_id(),
                tool_id: descriptor.tool_id.clone(),
                expected_project_digest: String::new(),
                payload,
            },
        });
    }
    let status = read_session_status(adapter)?;
    let project = status.project.as_ref().ok_or_else(|| {
        (
            -32002,
            "The connected Editor has no active project context for this tool.".to_string(),
        )
    })?;
    let expected_project_digest = invocation_expected_project_digest(&payload)
        .unwrap_or(&project.observed_digest)
        .to_string();
    Ok(GatewayRequestPayload::ExecuteSessionBound {
        invocation: AiToolInvocation {
            schema_version: AI_TOOL_INVOCATION_SCHEMA_VERSION.to_string(),
            invocation_id: next_mcp_invocation_id(),
            tool_id: descriptor.tool_id.clone(),
            expected_project_digest,
            payload,
        },
    })
}

fn read_session_status(
    adapter: &mut GatewayRemoteAdapter,
) -> Result<crate::GatewaySessionStatus, (i64, String)> {
    let reply = adapter
        .dispatch(GatewayRequestPayload::SessionStatus)
        .map_err(|error| (-32000, format!("{}: {}", error.code, error.message)))?;
    match reply.payload {
        GatewayReplyPayload::SessionStatus(status) => Ok(status),
        GatewayReplyPayload::Rejected(diagnostic) => Err((
            -32000,
            format!("{}: {}", diagnostic.code, diagnostic.message),
        )),
        _ => Err((
            -32603,
            "Gateway returned a non-Status reply while binding a typed tool invocation."
                .to_string(),
        )),
    }
}

fn invocation_expected_project_digest(payload: &AiToolInvocationPayload) -> Option<&str> {
    match payload {
        AiToolInvocationPayload::Candidate(input) => {
            Some(&input.envelope.expected_base_project_digest)
        }
        AiToolInvocationPayload::RollbackCandidate { receipt } => {
            Some(&receipt.applied_project_digest)
        }
        _ => None,
    }
}

fn invalid_direct_input(error: editor_core::AiToolKernelError) -> (i64, String) {
    (-32602, format!("{}: {}", error.code, error.message))
}

fn decode_arguments<T: DeserializeOwned>(arguments: Value) -> Result<T, (i64, String)> {
    serde_json::from_value(arguments).map_err(|error| (-32602, error.to_string()))
}

fn decode_operation_arguments(arguments: Value) -> Result<OperationArguments, (i64, String)> {
    let arguments: OperationArguments = decode_arguments(arguments)?;
    let operation_id_length = arguments.operation_id.chars().count();
    if arguments.operation_id.trim().is_empty() || operation_id_length > 128 {
        return Err((
            -32602,
            "operationId must contain 1 to 128 characters and cannot be blank.".to_string(),
        ));
    }
    Ok(arguments)
}

fn next_mcp_invocation_id() -> String {
    static NEXT_INVOCATION_ID: AtomicU64 = AtomicU64::new(1);
    format!(
        "mcp-invocation-{}",
        NEXT_INVOCATION_ID.fetch_add(1, Ordering::Relaxed)
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyArguments {}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OperationArguments {
    operation_id: String,
}

fn mcp_tools(catalog: &AiToolCatalog) -> Vec<Value> {
    let mut tools = vec![
        tool(
            "aife_status",
            "Read this Gateway session, project binding, and access state.",
            json!({"type":"object","additionalProperties":false}),
            read_annotations(),
        ),
        tool(
            "aife_catalog",
            "Read the active project Tool Catalog.",
            json!({"type":"object","additionalProperties":false}),
            read_annotations(),
        ),
        tool(
            "aife_observe",
            "Observe one durable operation.",
            json!({"type":"object","required":["operationId"],"properties":{"operationId":{"type":"string","minLength":1,"maxLength":128}},"additionalProperties":false}),
            read_annotations(),
        ),
        tool(
            "aife_cancel",
            "Request cancellation of one durable operation.",
            json!({"type":"object","required":["operationId"],"properties":{"operationId":{"type":"string","minLength":1,"maxLength":128}},"additionalProperties":false}),
            json!({"readOnlyHint":false,"destructiveHint":true,"idempotentHint":true,"openWorldHint":false}),
        ),
    ];
    tools.extend(catalog.tools.iter().map(projected_tool));
    tools
}

fn projected_tool(descriptor: &AiToolDescriptor) -> Value {
    let read_only = descriptor.side_effects.iter().all(|effect| {
        matches!(
            effect,
            AiToolSideEffect::None | AiToolSideEffect::ProjectRead
        )
    });
    tool(
        &mcp_name_for_tool_id(&descriptor.tool_id),
        &descriptor.summary,
        descriptor.input_schema.clone(),
        json!({
            "readOnlyHint": read_only,
            "destructiveHint": false,
            "idempotentHint": read_only,
            "openWorldHint": false
        }),
    )
}

fn mcp_name_for_tool_id(tool_id: &str) -> String {
    format!("aife_{}", tool_id.replace('.', "_"))
}

fn read_annotations() -> Value {
    json!({"readOnlyHint":true,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false})
}

fn tool(name: &str, description: &str, input_schema: Value, annotations: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
        "annotations": annotations
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_catalog() -> AiToolCatalog {
        editor_core::AiCapabilityToolKernel::new()
            .catalog(AiToolCatalogRequest::default())
            .unwrap()
    }

    #[test]
    fn mcp_projection_tool_annotations_keep_reads_non_mutating_and_writes_explicit() {
        let tools = mcp_tools(&test_catalog());
        let annotations = |name: &str| {
            tools
                .iter()
                .find(|tool| tool["name"] == name)
                .expect("MCP tool")
                .get("annotations")
                .expect("MCP tool annotations")
        };

        for name in [
            "aife_status",
            "aife_catalog",
            "aife_project_inspect",
            "aife_project_search",
            "aife_observe",
        ] {
            assert_eq!(annotations(name)["readOnlyHint"], true);
            assert_eq!(annotations(name)["destructiveHint"], false);
        }
        assert_eq!(annotations("aife_project_mutate")["readOnlyHint"], false);
        assert_eq!(annotations("aife_project_create")["readOnlyHint"], false);
        assert_eq!(annotations("aife_cancel")["readOnlyHint"], false);
        assert_eq!(annotations("aife_cancel")["destructiveHint"], true);
    }

    #[test]
    fn mcp_projection_tool_list_advertises_typed_catalog_projection_not_raw_execute() {
        let tools = mcp_tools(&test_catalog());
        let names = tools
            .iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str))
            .collect::<std::collections::BTreeSet<_>>();

        assert!(
            !names.contains("aife_execute"),
            "raw aife_execute cannot give Codex a strict tool-specific input contract"
        );
        for expected in [
            "aife_status",
            "aife_project_create",
            "aife_project_inspect",
            "aife_project_search",
            "aife_project_mutate",
            "aife_project_preview",
            "aife_runtime_capture_issue",
            "aife_ui_locate",
            "aife_ui_explain_visibility",
            "aife_project_build_export",
            "aife_project_delivery_verify",
        ] {
            assert!(
                names.contains(expected),
                "missing typed MCP tool {expected}"
            );
        }
    }

    #[test]
    fn mcp_catalog_v2_keeps_typed_list_stable_and_returns_dynamic_availability() {
        let catalog = serde_json::to_value(test_catalog()).unwrap();
        let names = mcp_tools(&test_catalog())
            .into_iter()
            .filter_map(|tool| tool["name"].as_str().map(str::to_string))
            .collect::<std::collections::BTreeSet<_>>();

        assert!(catalog["availabilityDigest"].as_str().is_some());
        assert!(names.contains("aife_project_inspect"));
        assert!(names.contains("aife_project_mutate"));
    }

    #[test]
    fn mcp_typed_tool_projection_uses_registry_direct_input_schemas() {
        let registry = AiToolContractRegistry::new();
        let tools = mcp_tools(&test_catalog());
        for descriptor in registry.descriptors() {
            let name = mcp_name_for_tool_id(&descriptor.tool_id);
            let projected = tools
                .iter()
                .find(|tool| tool["name"] == name)
                .expect("projected MCP tool");
            assert_eq!(projected["inputSchema"], descriptor.input_schema);
            let properties = projected["inputSchema"]["properties"]
                .as_object()
                .expect("direct input properties");
            for forbidden in ["projectIdentity", "toolVersion", "payloadKind", "grantRef"] {
                assert!(
                    !properties.contains_key(forbidden),
                    "{name} exposes Adapter-owned field {forbidden}"
                );
            }
            if descriptor.tool_id == editor_core::TOOL_ID_PROJECT_ROLLBACK {
                assert_eq!(
                    properties["schemaVersion"]["const"],
                    editor_core::EXTERNAL_PROJECT_ROLLBACK_SCHEMA_VERSION
                );
            } else {
                assert!(
                    !properties.contains_key("schemaVersion"),
                    "{name} exposes Adapter-owned field schemaVersion"
                );
            }
        }
    }

    #[test]
    fn mcp_projection_evidence_read_exposes_scope_and_preview_consumer_contract() {
        let tools = mcp_tools(&test_catalog());
        let evidence_read = tools
            .iter()
            .find(|tool| tool["name"] == "aife_evidence_read")
            .expect("projected evidence.read tool");
        let description = evidence_read["description"]
            .as_str()
            .expect("evidence.read description");
        assert!(description.contains("project-evidence:"));
        assert!(description.contains("Library/Reports/"));
        assert!(description.contains("Library/AiToolKernel/"));
        assert!(description.contains("runtime.capture_issue"));

        let evidence_ref = &evidence_read["inputSchema"]["properties"]["evidenceRef"];
        assert_eq!(
            evidence_ref["oneOf"][0]["pattern"],
            "^project-evidence:Library/Reports/"
        );
        assert_eq!(
            evidence_ref["oneOf"][1]["pattern"],
            "^project-evidence:Library/AiToolKernel/"
        );
        assert!(evidence_ref["description"]
            .as_str()
            .expect("evidenceRef description")
            .contains("not a Preview frameEvidenceRef"));
    }

    #[test]
    fn mcp_typed_tool_operation_arguments_reject_blank_and_oversized_ids() {
        assert_eq!(
            decode_operation_arguments(json!({"operationId": "operation-1"}))
                .unwrap()
                .operation_id,
            "operation-1"
        );
        for invalid in [String::new(), "   ".to_string(), "x".repeat(129)] {
            let error = decode_operation_arguments(json!({"operationId": invalid})).unwrap_err();
            assert_eq!(error.0, -32602);
        }
        let error = decode_operation_arguments(
            json!({"operationId": "operation-1", "grantRef": "$active"}),
        )
        .unwrap_err();
        assert_eq!(error.0, -32602);
    }
}
