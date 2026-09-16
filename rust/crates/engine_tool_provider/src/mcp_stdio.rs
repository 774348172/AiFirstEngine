use crate::{
    CanonicalToolStatus, EngineToolProvider, HostSessionContext, HostToolCall,
    NativeEngineToolProvider, ToolAudience, ToolDefinition, ToolSideEffect,
};
use serde_json::{json, Value};
use std::io::{BufRead, Write};

pub const MCP_PROTOCOL_VERSION: &str = "2025-06-18";
pub const MCP_MAX_RESULT_BYTES: usize = 1024 * 1024;

#[derive(Debug)]
pub struct McpHostAdapter {
    provider: EngineToolProvider,
}

impl McpHostAdapter {
    pub fn attach(host: HostSessionContext) -> Result<Self, crate::ToolDiagnostic> {
        Ok(Self {
            provider: EngineToolProvider::attach(host)?,
        })
    }

    pub fn tool_definitions(&self) -> Vec<ToolDefinition> {
        crate::tool_definitions_for(ToolAudience::ModelDefault)
    }

    pub fn invoke_canonical(
        &mut self,
        call_id: impl Into<String>,
        tool_name: impl Into<String>,
        arguments: Value,
    ) -> crate::CanonicalToolResult {
        let tool_name = tool_name.into();
        let approved = self
            .tool_definitions()
            .into_iter()
            .find(|definition| definition.name == tool_name)
            .is_some_and(|definition| definition.side_effect != ToolSideEffect::Read);
        self.invoke_canonical_with_approval(call_id, tool_name, arguments, approved)
    }

    /// An embedding host can pass a denial; stdio tools/call is already forwarded by the approving MCP host.
    pub fn invoke_canonical_with_approval(
        &mut self,
        call_id: impl Into<String>,
        tool_name: impl Into<String>,
        arguments: Value,
        approved: bool,
    ) -> crate::CanonicalToolResult {
        self.provider.invoke_model(HostToolCall {
            call_id: call_id.into(),
            tool_name: tool_name.into(),
            arguments,
            approved,
        })
    }

    pub fn handle_request(&mut self, request: Value) -> Option<Value> {
        let id = request.get("id").cloned();
        let method = request.get("method").and_then(Value::as_str)?;
        let id = id?;
        let outcome = match method {
            "initialize" => Ok(json!({
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities":{"tools":{"listChanged":false}},
                "serverInfo":{"name":"ai-first-game-engine","version":"1.0.0"}
            })),
            "ping" | "shutdown" => Ok(json!({})),
            "tools/list" => Ok(json!({
                "tools": self.tool_definitions().iter().map(project_tool).collect::<Vec<_>>()
            })),
            "tools/call" => self.call_tool(&id, request.get("params").cloned().unwrap_or_default()),
            _ => Err((-32601, format!("Unsupported MCP method '{method}'."))),
        };
        Some(match outcome {
            Ok(result) => json!({"jsonrpc":"2.0","id":id,"result":result}),
            Err((code, message)) => {
                json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
            }
        })
    }

    fn call_tool(&mut self, request_id: &Value, params: Value) -> Result<Value, (i64, String)> {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| (-32602, "tools/call requires a tool name.".to_string()))?;
        if !self
            .provider
            .tool_definitions()
            .iter()
            .any(|definition| definition.name == name)
        {
            return Err((-32602, format!("Unknown MCP tool '{name}'.")));
        }
        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let call_id = format!(
            "mcp-call-{}",
            serde_json::to_string(request_id).unwrap_or_else(|_| "unknown".to_string())
        );
        let result = self.invoke_canonical(call_id, name, arguments);
        mcp_tool_result(&result)
    }
}

fn mcp_tool_result(result: &crate::CanonicalToolResult) -> Result<Value, (i64, String)> {
    let structured = serde_json::to_value(result).map_err(|error| (-32603, error.to_string()))?;
    let text = serde_json::to_string(&structured).map_err(|error| (-32603, error.to_string()))?;
    if text.len() > MCP_MAX_RESULT_BYTES {
        return Err((
                -32001,
                format!(
                    "Engine tool result exceeds the MCP output limit of {MCP_MAX_RESULT_BYTES} bytes; use a bounded query or evidence reference."
                ),
            ));
    }
    Ok(json!({
        "content":[{"type":"text","text":text}],
        "structuredContent":structured,
        "isError":result.status != CanonicalToolStatus::Completed
    }))
}

pub fn run_mcp_stdio(
    host: HostSessionContext,
    input: impl BufRead,
    mut output: impl Write,
) -> Result<(), String> {
    let mut adapter = McpHostAdapter::attach(host)
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    for line in input.lines() {
        let line = line.map_err(|error| error.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let request = match serde_json::from_str::<Value>(&line) {
            Ok(request) => request,
            Err(error) => {
                write_response(
                    &mut output,
                    &json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":error.to_string()}}),
                )?;
                continue;
            }
        };
        let shutdown = request.get("method").and_then(Value::as_str) == Some("shutdown");
        if let Some(response) = adapter.handle_request(request) {
            write_response(&mut output, &response)?;
        }
        if shutdown {
            break;
        }
    }
    Ok(())
}

fn write_response(output: &mut impl Write, response: &Value) -> Result<(), String> {
    serde_json::to_writer(&mut *output, response).map_err(|error| error.to_string())?;
    output.write_all(b"\n").map_err(|error| error.to_string())?;
    output.flush().map_err(|error| error.to_string())
}

fn project_tool(definition: &ToolDefinition) -> Value {
    let read_only = definition.side_effect == ToolSideEffect::Read;
    json!({
        "name":definition.name,
        "description":definition.description,
        "inputSchema":definition.input_schema,
        "annotations":{
            "readOnlyHint":read_only,
            "destructiveHint":definition.side_effect == ToolSideEffect::Write,
            "idempotentHint":read_only,
            "openWorldHint":false
        },
        "_meta":{
            "engine/maturity":definition.maturity,
            "engine/risk":definition.risk,
            "engine/duration":definition.duration,
            "engine/supportsCancellation":definition.supports_cancellation,
            "engine/supportsRollback":definition.supports_rollback
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn mcp_model_default_projection_is_exact_and_stable() {
        let fixture = Fixture::new();
        let mut adapter = McpHostAdapter::attach(fixture.host()).unwrap();
        let response = adapter
            .handle_request(json!({"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}))
            .unwrap();
        let names = response["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
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
    }

    #[test]
    fn mcp_local_transition_projects_complete_v2_structured_content() {
        let fixture = Fixture::new();
        let mut adapter = McpHostAdapter::attach(fixture.host()).unwrap();
        let response = adapter
            .handle_request(json!({
                "jsonrpc":"2.0",
                "id":2,
                "method":"tools/call",
                "params":{
                    "name":"engine_project_inspect",
                    "arguments":{"unknown":true}
                }
            }))
            .unwrap();
        let structured = &response["result"]["structuredContent"];

        assert_eq!(structured["schemaVersion"], "engine-tool-result.v2");
        assert_eq!(structured["retryability"], "retry_after_correction");
        assert_eq!(
            structured["recommendedLocalTransitions"][0]["category"],
            "retry_with_correction"
        );
        assert_eq!(
            structured["recommendedLocalTransitions"][0]["toolName"],
            "engine_project_inspect"
        );
    }

    #[test]
    fn mcp_hidden_tool_is_distinct_from_unknown_protocol_tool() {
        let fixture = Fixture::new();
        let mut adapter = McpHostAdapter::attach(fixture.host()).unwrap();
        let protocol = adapter
            .handle_request(json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"missing","arguments":{}}}))
            .unwrap();
        let engine = adapter
            .handle_request(json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"engine_project_read_object","arguments":{"path":"missing.json"}}}))
            .unwrap();
        assert_eq!(protocol["error"]["code"], -32602);
        assert!(engine.get("error").is_none());
        assert_eq!(engine["result"]["isError"], true);
        assert_eq!(
            engine["result"]["structuredContent"]["status"],
            "rejected_by_engine"
        );
        assert_eq!(
            engine["result"]["structuredContent"]["diagnostics"][0]["code"],
            "engine_provider.tool_not_exposed"
        );
        assert_eq!(
            engine["result"]["structuredContent"]["schemaVersion"],
            "engine-tool-result.v2"
        );
        assert_eq!(
            engine["result"]["structuredContent"]["retryability"],
            "not_retryable"
        );
        assert!(
            engine["result"]["structuredContent"]["recommendedLocalTransitions"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn mcp_output_limit_rejects_oversized_inline_results() {
        let result = crate::CanonicalToolResult {
            schema_version: crate::ENGINE_TOOL_RESULT_SCHEMA_VERSION.to_string(),
            call_id: "large".to_string(),
            tool_name: "engine_evidence_read".to_string(),
            status: CanonicalToolStatus::Completed,
            operation_id: "large-operation".to_string(),
            project_revision: None,
            output: json!({"text":"x".repeat(MCP_MAX_RESULT_BYTES)}),
            diagnostics: Vec::new(),
            retryability: crate::ToolRetryability::NotNeeded,
            recommended_local_transitions: Vec::new(),
            receipt_ref: None,
            evidence_refs: vec!["project-evidence:Library/Reports/large.json".to_string()],
            replayed: false,
        };
        let error = mcp_tool_result(&result).unwrap_err();
        assert_eq!(error.0, -32001);
        assert!(error.1.contains("output limit"));
    }

    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "aife-engine-provider-mcp-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&root).unwrap();
            fs::write(
                root.join("project.aife.json"),
                br#"{"schemaVersion":"aife-project.v2","projectId":"provider.mcp"}"#,
            )
            .unwrap();
            Self { root }
        }

        fn host(&self) -> HostSessionContext {
            HostSessionContext {
                session_id: "mcp-test".to_string(),
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
}
