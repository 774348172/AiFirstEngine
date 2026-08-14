use editor_core::{
    AiGoalBinding, AiRiskEnvelope, AiToolAccepted, AiToolAvailability, AiToolCancellationReceipt,
    AiToolCatalog, AiToolCatalogRequest, AiToolInspectRequest, AiToolInspectResult,
    AiToolInvocation, AiToolOperationSnapshot, AiToolResult,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};

pub const GATEWAY_PROTOCOL_VERSION: &str = "ai-tool-gateway.v2";
pub const GATEWAY_CLIENT_HELLO_SCHEMA_VERSION: &str = "ai-tool-gateway-client-hello.v2";
pub const GATEWAY_SESSION_BINDING_SCHEMA_VERSION: &str = "ai-tool-gateway-session-binding.v2";
pub const GATEWAY_REQUEST_SCHEMA_VERSION: &str = "ai-tool-gateway-request.v2";
pub const GATEWAY_REPLY_SCHEMA_VERSION: &str = "ai-tool-gateway-reply.v2";
pub const GATEWAY_EVENT_SCHEMA_VERSION: &str = "ai-tool-gateway-event.v1";
pub const GATEWAY_CLOSE_RECEIPT_SCHEMA_VERSION: &str = "ai-tool-gateway-close-receipt.v1";
pub const GATEWAY_SESSION_STATUS_SCHEMA_VERSION: &str = "ai-tool-gateway-session-status.v2";
pub const GATEWAY_ACCESS_REQUEST_SCHEMA_VERSION: &str = "ai-tool-gateway-access-request.v1";
pub const GATEWAY_ACCESS_DECISION_RECEIPT_SCHEMA_VERSION: &str =
    "ai-tool-gateway-access-decision-receipt.v1";
pub const GATEWAY_SESSION_CLEANUP_REPORT_SCHEMA_VERSION: &str =
    "ai-tool-gateway-session-cleanup-report.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientKind {
    Mcp,
    Cli,
    NativeEditor,
    Test,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientHello {
    pub schema_version: String,
    pub gateway_protocol_version: String,
    pub client_kind: ClientKind,
    pub client_version: String,
    pub supported_schema_versions: Vec<String>,
    pub expected_editor_instance_id: String,
    pub requested_read_scope: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GatewayProjectContext {
    pub project_identity: String,
    pub canonical_project_root_digest: String,
    pub project_digest: String,
    pub read_generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientSessionBinding {
    pub schema_version: String,
    pub client_session_id: String,
    pub editor_process_identity: String,
    pub editor_instance_id: String,
    pub project_context: Option<GatewayProjectContext>,
    pub gateway_protocol_version: String,
    pub effective_read_scope: Vec<String>,
    pub catalog_schema_version: String,
    pub catalog_digest: String,
    pub expires_at_epoch_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewaySessionState {
    Active,
    ReconnectRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayReadAccessState {
    Unavailable,
    Active,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayMutationAccessState {
    NotRequested,
    AwaitingUser,
    Active,
    Revoked,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GatewaySessionIdentityStatus {
    pub id: String,
    pub editor_instance_id: String,
    pub client_kind: ClientKind,
    pub client_version: String,
    pub connected_at_epoch_ms: u64,
    pub last_seen_at_epoch_ms: u64,
    pub age_ms: u64,
    pub expires_at_epoch_ms: u64,
    pub state: GatewaySessionState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GatewaySessionProjectStatus {
    pub identity: String,
    pub current_digest: String,
    pub observed_digest: String,
    pub runtime_module: String,
    pub catalog_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GatewayReadAccessStatus {
    pub state: GatewayReadAccessState,
    pub effective_scopes: Vec<String>,
    pub generation: u64,
    pub grant_digest: String,
    pub stale_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GatewayMutationAccessStatus {
    pub state: GatewayMutationAccessState,
    pub requested_profile: String,
    pub capabilities: Vec<String>,
    pub blocked_capabilities: Vec<String>,
    pub grant_digest: Option<String>,
    pub expires_at_epoch_ms: Option<u64>,
    pub remaining_time_budget_ms: Option<u64>,
    pub remaining_mutation_count: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GatewaySessionAccessStatus {
    pub read: GatewayReadAccessStatus,
    pub mutation: GatewayMutationAccessStatus,
    pub access_generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GatewaySessionStatus {
    pub schema_version: String,
    pub session: GatewaySessionIdentityStatus,
    pub project: Option<GatewaySessionProjectStatus>,
    pub access: GatewaySessionAccessStatus,
    pub operation_generation: u64,
    pub reconnect_required: bool,
    pub next_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GatewayAccessRequest {
    pub schema_version: String,
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    pub client_session_id: String,
    pub client_kind: ClientKind,
    pub client_version: String,
    pub project_identity: String,
    pub observed_project_digest: String,
    pub connected_at_epoch_ms: u64,
    pub expires_at_epoch_ms: u64,
    pub requested_profile: String,
    pub capabilities: Vec<String>,
    pub blocked_capabilities: Vec<String>,
    pub goal_binding: AiGoalBinding,
    pub risk_envelope: AiRiskEnvelope,
    pub approval_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayAccessDecision {
    Approve,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GatewayAccessDecisionReceipt {
    pub schema_version: String,
    pub request_id: String,
    pub client_session_id: String,
    pub decision: GatewayAccessDecision,
    pub decided_by: String,
    pub decided_at_epoch_ms: u64,
    pub mutation_state: GatewayMutationAccessState,
    pub grant_ref: Option<String>,
    pub grant_digest: Option<String>,
    pub diagnostic_code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GatewaySessionCleanupReport {
    pub schema_version: String,
    pub pruned_at_epoch_ms: u64,
    pub closed_session_ids: Vec<String>,
    pub expired_session_ids: Vec<String>,
    pub reconnect_required_session_ids: Vec<String>,
    pub revoked_grant_count: usize,
    pub removed_access_request_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "requestKind",
    content = "request",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum GatewayRequestPayload {
    SessionStatus,
    Catalog(AiToolCatalogRequest),
    Inspect(AiToolInspectRequest),
    ExecuteSessionBound {
        invocation: AiToolInvocation,
    },
    Execute {
        invocation: AiToolInvocation,
        grant_ref: String,
    },
    Observe {
        operation_id: String,
    },
    Cancel {
        operation_id: String,
        grant_ref: String,
    },
    CancelSessionBound {
        operation_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GatewayRequest {
    pub schema_version: String,
    pub gateway_protocol_version: String,
    pub request_id: String,
    pub client_session_id: String,
    pub deadline_epoch_ms: Option<u64>,
    pub response_limit_bytes: u64,
    pub payload: GatewayRequestPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GatewayDiagnostic {
    pub code: String,
    pub message: String,
    pub next_action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availability: Option<AiToolAvailability>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "replyKind",
    content = "reply",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum GatewayReplyPayload {
    SessionStatus(GatewaySessionStatus),
    Catalog(AiToolCatalog),
    Inspection(AiToolInspectResult),
    Accepted(AiToolAccepted),
    ToolResult(AiToolResult),
    Operation(AiToolOperationSnapshot),
    Cancellation(AiToolCancellationReceipt),
    Rejected(GatewayDiagnostic),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GatewayReply {
    pub schema_version: String,
    pub gateway_protocol_version: String,
    pub request_id: String,
    pub client_session_id: String,
    pub payload: GatewayReplyPayload,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GatewayEvent {
    pub schema_version: String,
    pub gateway_protocol_version: String,
    pub client_session_id: String,
    pub project_identity: String,
    pub event_sequence: u64,
    pub operation: AiToolOperationSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CloseReceipt {
    pub schema_version: String,
    pub client_session_id: String,
    pub closed_at_epoch_ms: u64,
    pub diagnostic_code: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GatewayProtocolLimits {
    pub max_message_bytes: usize,
    pub max_depth: usize,
    pub max_string_bytes: usize,
    pub max_array_items: usize,
    pub max_object_entries: usize,
    pub max_response_limit_bytes: usize,
}

impl Default for GatewayProtocolLimits {
    fn default() -> Self {
        Self {
            max_message_bytes: 1024 * 1024,
            max_depth: 32,
            max_string_bytes: 256 * 1024,
            max_array_items: 4096,
            max_object_entries: 4096,
            max_response_limit_bytes: 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayProtocolError {
    pub code: String,
    pub message: String,
    pub next_action: String,
}

impl std::fmt::Display for GatewayProtocolError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for GatewayProtocolError {}

pub fn decode_client_hello(
    bytes: &[u8],
    limits: GatewayProtocolLimits,
) -> Result<ClientHello, GatewayProtocolError> {
    let hello: ClientHello = decode_bounded_json(bytes, limits)?;
    validate_client_hello(&hello)?;
    Ok(hello)
}

pub fn decode_gateway_request(
    bytes: &[u8],
    limits: GatewayProtocolLimits,
) -> Result<GatewayRequest, GatewayProtocolError> {
    let request: GatewayRequest = decode_bounded_json(bytes, limits)?;
    validate_gateway_request(&request, limits)?;
    Ok(request)
}

pub fn client_hello_json_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "AI Tool Gateway ClientHello v2",
        "type": "object",
        "additionalProperties": false,
        "required": [
            "schemaVersion", "gatewayProtocolVersion", "clientKind", "clientVersion",
            "supportedSchemaVersions", "expectedEditorInstanceId", "requestedReadScope"
        ],
        "properties": {
            "schemaVersion": {"const": GATEWAY_CLIENT_HELLO_SCHEMA_VERSION},
            "gatewayProtocolVersion": {"const": GATEWAY_PROTOCOL_VERSION},
            "clientKind": {"enum": ["mcp", "cli", "native_editor", "test"]},
            "clientVersion": {"type": "string", "minLength": 1, "maxLength": 128},
            "supportedSchemaVersions": {
                "type": "array", "minItems": 1, "maxItems": 128,
                "items": {"type": "string", "minLength": 1, "maxLength": 128}
            },
            "expectedEditorInstanceId": {"type": "string", "minLength": 1, "maxLength": 128},
            "requestedReadScope": {
                "type": "array", "maxItems": 128,
                "items": {"type": "string", "minLength": 1, "maxLength": 128}
            }
        }
    })
}

pub fn gateway_request_json_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "AI Tool Gateway Request v2",
        "type": "object",
        "additionalProperties": false,
        "required": [
            "schemaVersion", "gatewayProtocolVersion", "requestId", "clientSessionId",
            "deadlineEpochMs", "responseLimitBytes", "payload"
        ],
        "properties": {
            "schemaVersion": {"const": GATEWAY_REQUEST_SCHEMA_VERSION},
            "gatewayProtocolVersion": {"const": GATEWAY_PROTOCOL_VERSION},
            "requestId": {"type": "string", "minLength": 1, "maxLength": 128},
            "clientSessionId": {"type": "string", "minLength": 1, "maxLength": 128},
            "deadlineEpochMs": {"type": ["integer", "null"], "minimum": 0},
            "responseLimitBytes": {"type": "integer", "minimum": 1, "maximum": 1048576},
            "payload": {
                "oneOf": [
                    {"type": "object", "required": ["requestKind", "request"], "properties": {"requestKind": {"const": "catalog"}}},
                    {"type": "object", "required": ["requestKind", "request"], "properties": {"requestKind": {"const": "inspect"}}},
                    {"type": "object", "required": ["requestKind"], "properties": {"requestKind": {"const": "session_status"}}},
                    {"type": "object", "required": ["requestKind", "request"], "properties": {"requestKind": {"const": "execute_session_bound"}}},
                    {"type": "object", "required": ["requestKind", "request"], "properties": {"requestKind": {"const": "execute"}}},
                    {"type": "object", "required": ["requestKind", "request"], "properties": {"requestKind": {"const": "observe"}}},
                    {"type": "object", "required": ["requestKind", "request"], "properties": {"requestKind": {"const": "cancel"}}},
                    {"type": "object", "required": ["requestKind", "request"], "properties": {"requestKind": {"const": "cancel_session_bound"}}}
                ]
            }
        }
    })
}

fn decode_bounded_json<T: DeserializeOwned>(
    bytes: &[u8],
    limits: GatewayProtocolLimits,
) -> Result<T, GatewayProtocolError> {
    if bytes.len() > limits.max_message_bytes {
        return Err(protocol_error(
            "gateway.protocol.message_oversize",
            "Gateway message exceeds the configured byte limit.",
            "Send bounded structured input and use evidence references for large content.",
        ));
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        protocol_error(
            "gateway.protocol.json_invalid",
            format!("Gateway message is not valid JSON: {error}"),
            "Regenerate the request from the advertised schema.",
        )
    })?;
    validate_json_shape(&value, limits, 0)?;
    serde_json::from_value(value).map_err(|error| {
        protocol_error(
            "gateway.protocol.schema_invalid",
            format!("Gateway message does not match the required schema: {error}"),
            "Read the negotiated Gateway schema and remove unknown fields.",
        )
    })
}

fn validate_json_shape(
    value: &Value,
    limits: GatewayProtocolLimits,
    depth: usize,
) -> Result<(), GatewayProtocolError> {
    if depth > limits.max_depth {
        return Err(protocol_error(
            "gateway.protocol.depth_exceeded",
            "Gateway JSON nesting exceeds the configured depth limit.",
            "Flatten the request or move large evidence into a referenced artifact.",
        ));
    }
    match value {
        Value::String(text) if text.len() > limits.max_string_bytes => Err(protocol_error(
            "gateway.protocol.string_oversize",
            "Gateway JSON string exceeds the configured byte limit.",
            "Use a bounded value or an evidence reference.",
        )),
        Value::Array(values) if values.len() > limits.max_array_items => Err(protocol_error(
            "gateway.protocol.array_oversize",
            "Gateway JSON array exceeds the configured item limit.",
            "Page the request instead of sending an unbounded array.",
        )),
        Value::Object(values) if values.len() > limits.max_object_entries => Err(protocol_error(
            "gateway.protocol.object_oversize",
            "Gateway JSON object exceeds the configured entry limit.",
            "Use the advertised typed request instead of an unbounded object.",
        )),
        Value::Array(values) => values
            .iter()
            .try_for_each(|item| validate_json_shape(item, limits, depth + 1)),
        Value::Object(values) => values.iter().try_for_each(|(key, item)| {
            if key.len() > limits.max_string_bytes {
                return Err(protocol_error(
                    "gateway.protocol.key_oversize",
                    "Gateway JSON key exceeds the configured byte limit.",
                    "Use the exact field names from the negotiated schema.",
                ));
            }
            validate_json_shape(item, limits, depth + 1)
        }),
        _ => Ok(()),
    }
}

fn validate_client_hello(hello: &ClientHello) -> Result<(), GatewayProtocolError> {
    if hello.schema_version != GATEWAY_CLIENT_HELLO_SCHEMA_VERSION
        || hello.gateway_protocol_version != GATEWAY_PROTOCOL_VERSION
    {
        return Err(protocol_error(
            "gateway.protocol.version_unsupported",
            "ClientHello uses an unsupported Gateway protocol or schema version.",
            "Reconnect using ai-tool-gateway.v2 and client-hello.v2.",
        ));
    }
    if hello.client_version.trim().is_empty()
        || hello.supported_schema_versions.is_empty()
        || hello.expected_editor_instance_id.trim().is_empty()
    {
        return Err(protocol_error(
            "gateway.protocol.hello_incomplete",
            "ClientHello is missing a required identity or schema capability.",
            "Provide client version, supported schemas, and the exact editorInstanceId.",
        ));
    }
    Ok(())
}

fn validate_gateway_request(
    request: &GatewayRequest,
    limits: GatewayProtocolLimits,
) -> Result<(), GatewayProtocolError> {
    if request.schema_version != GATEWAY_REQUEST_SCHEMA_VERSION
        || request.gateway_protocol_version != GATEWAY_PROTOCOL_VERSION
    {
        return Err(protocol_error(
            "gateway.protocol.version_unsupported",
            "Gateway request uses an unsupported protocol or schema version.",
            "Use the versions returned by the active session binding.",
        ));
    }
    if request.request_id.trim().is_empty() || request.client_session_id.trim().is_empty() {
        return Err(protocol_error(
            "gateway.protocol.request_identity_invalid",
            "Gateway request identity fields cannot be empty.",
            "Use the active ClientSessionBinding and a unique request id.",
        ));
    }
    if request.response_limit_bytes == 0
        || request.response_limit_bytes > limits.max_response_limit_bytes as u64
    {
        return Err(protocol_error(
            "gateway.protocol.response_limit_invalid",
            "Gateway response limit is zero or exceeds the negotiated maximum.",
            "Choose a positive bounded response limit and page larger evidence.",
        ));
    }
    match &request.payload {
        GatewayRequestPayload::Execute { grant_ref, .. }
        | GatewayRequestPayload::Cancel { grant_ref, .. }
            if grant_ref.trim().is_empty() =>
        {
            Err(protocol_error(
                "gateway.protocol.grant_ref_required",
                "Mutation or cancellation request requires an opaque grant reference.",
                "Use a grant reference issued by the active Native Editor session.",
            ))
        }
        GatewayRequestPayload::Observe { operation_id }
        | GatewayRequestPayload::Cancel { operation_id, .. }
        | GatewayRequestPayload::CancelSessionBound { operation_id }
            if operation_id.trim().is_empty() =>
        {
            Err(protocol_error(
                "gateway.protocol.operation_id_required",
                "Observe or cancel request requires an operation id.",
                "Use the operation id returned by Tool Kernel execution.",
            ))
        }
        _ => Ok(()),
    }
}

fn protocol_error(
    code: impl Into<String>,
    message: impl Into<String>,
    next_action: impl Into<String>,
) -> GatewayProtocolError {
    GatewayProtocolError {
        code: code.into(),
        message: message.into(),
        next_action: next_action.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use editor_core::{
        AiToolCatalogRequest, AiToolOperationState, AI_TOOL_CATALOG_SCHEMA_VERSION,
        AI_TOOL_OPERATION_SCHEMA_VERSION,
    };

    fn hello() -> ClientHello {
        ClientHello {
            schema_version: GATEWAY_CLIENT_HELLO_SCHEMA_VERSION.to_string(),
            gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
            client_kind: ClientKind::Test,
            client_version: "test-adapter.v1".to_string(),
            supported_schema_versions: vec![AI_TOOL_CATALOG_SCHEMA_VERSION.to_string()],
            expected_editor_instance_id: crate::default_editor_instance_id(),
            requested_read_scope: vec!["project".to_string()],
        }
    }

    fn catalog_request() -> GatewayRequest {
        GatewayRequest {
            schema_version: GATEWAY_REQUEST_SCHEMA_VERSION.to_string(),
            gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
            request_id: "request-1".to_string(),
            client_session_id: "session-1".to_string(),
            deadline_epoch_ms: None,
            response_limit_bytes: 64 * 1024,
            payload: GatewayRequestPayload::Catalog(AiToolCatalogRequest::default()),
        }
    }

    #[test]
    fn gateway_protocol_round_trips_strict_client_and_request_contracts() {
        let hello_json = serde_json::to_vec(&hello()).unwrap();
        assert_eq!(
            decode_client_hello(&hello_json, GatewayProtocolLimits::default()).unwrap(),
            hello()
        );

        let request = catalog_request();
        let request_json = serde_json::to_vec(&request).unwrap();
        assert_eq!(
            decode_gateway_request(&request_json, GatewayProtocolLimits::default()).unwrap(),
            request
        );
        assert!(client_hello_json_schema()["additionalProperties"] == false);
        assert!(gateway_request_json_schema()["additionalProperties"] == false);

        let mut unknown: Value = serde_json::from_slice(&request_json).unwrap();
        unknown["unknownRequiredSemantic"] = json!(true);
        let error = decode_gateway_request(
            &serde_json::to_vec(&unknown).unwrap(),
            GatewayProtocolLimits::default(),
        )
        .unwrap_err();
        assert_eq!(error.code, "gateway.protocol.schema_invalid");

        let binding = ClientSessionBinding {
            schema_version: GATEWAY_SESSION_BINDING_SCHEMA_VERSION.to_string(),
            client_session_id: "session-1".to_string(),
            editor_process_identity: "editor-42".to_string(),
            editor_instance_id: "editor-instance-42".to_string(),
            project_context: Some(GatewayProjectContext {
                project_identity: "project-example".to_string(),
                canonical_project_root_digest: "sha256:root".to_string(),
                project_digest: "sha256:project".to_string(),
                read_generation: 1,
            }),
            gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
            effective_read_scope: vec!["project".to_string()],
            catalog_schema_version: AI_TOOL_CATALOG_SCHEMA_VERSION.to_string(),
            catalog_digest: "sha256:catalog".to_string(),
            expires_at_epoch_ms: 1000,
        };
        assert_eq!(
            serde_json::from_slice::<ClientSessionBinding>(&serde_json::to_vec(&binding).unwrap())
                .unwrap(),
            binding
        );

        let reply = GatewayReply {
            schema_version: GATEWAY_REPLY_SCHEMA_VERSION.to_string(),
            gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
            request_id: "request-1".to_string(),
            client_session_id: "session-1".to_string(),
            payload: GatewayReplyPayload::Rejected(GatewayDiagnostic {
                code: "gateway.test_rejection".to_string(),
                message: "Rejected for protocol round-trip testing.".to_string(),
                next_action: "Use a valid test request.".to_string(),
                availability: None,
            }),
        };
        assert_eq!(
            serde_json::from_slice::<GatewayReply>(&serde_json::to_vec(&reply).unwrap()).unwrap(),
            reply
        );

        let operation = AiToolOperationSnapshot {
            schema_version: AI_TOOL_OPERATION_SCHEMA_VERSION.to_string(),
            operation_id: "operation-1".to_string(),
            invocation_id: "invocation-1".to_string(),
            invocation_digest: "sha256:invocation".to_string(),
            tool_id: "project.preview".to_string(),
            grant_digest: "sha256:grant".to_string(),
            project_identity: "project-example".to_string(),
            state: AiToolOperationState::Running,
            stage: "preflight".to_string(),
            started_at_epoch_ms: 1,
            completed_at_epoch_ms: None,
            result: None,
            artifact_refs: Vec::new(),
            cancel_signal_sent: false,
            commit_started: false,
            transitions: Vec::new(),
        };
        let event = GatewayEvent {
            schema_version: GATEWAY_EVENT_SCHEMA_VERSION.to_string(),
            gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
            client_session_id: "session-1".to_string(),
            project_identity: "project-example".to_string(),
            event_sequence: 1,
            operation,
        };
        assert_eq!(
            serde_json::from_slice::<GatewayEvent>(&serde_json::to_vec(&event).unwrap()).unwrap(),
            event
        );

        let close = CloseReceipt {
            schema_version: GATEWAY_CLOSE_RECEIPT_SCHEMA_VERSION.to_string(),
            client_session_id: "session-1".to_string(),
            closed_at_epoch_ms: 2,
            diagnostic_code: "gateway.session_closed".to_string(),
        };
        assert_eq!(
            serde_json::from_slice::<CloseReceipt>(&serde_json::to_vec(&close).unwrap()).unwrap(),
            close
        );
    }

    #[test]
    fn gateway_session_status_protocol_round_trips_session_bound_requests() {
        for payload in [
            GatewayRequestPayload::SessionStatus,
            GatewayRequestPayload::ExecuteSessionBound {
                invocation: AiToolInvocation {
                    schema_version: editor_core::AI_TOOL_INVOCATION_SCHEMA_VERSION.to_string(),
                    invocation_id: "session-bound-search".to_string(),
                    tool_id: editor_core::TOOL_ID_PROJECT_SEARCH.to_string(),
                    expected_project_digest: "sha256:project".to_string(),
                    payload: editor_core::AiToolInvocationPayload::ProjectSearch(
                        editor_core::ProjectSearchInput {
                            schema_version: editor_core::PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION
                                .to_string(),
                            query: "project".to_string(),
                            kinds: Vec::new(),
                            continuation_token: None,
                            page_size: 10,
                        },
                    ),
                },
            },
            GatewayRequestPayload::CancelSessionBound {
                operation_id: "operation-1".to_string(),
            },
        ] {
            let mut request = catalog_request();
            request.payload = payload;
            let encoded = serde_json::to_vec(&request).unwrap();
            assert_eq!(
                decode_gateway_request(&encoded, GatewayProtocolLimits::default()).unwrap(),
                request
            );
        }
    }

    #[test]
    fn gateway_schema_limits_reject_oversize_depth_and_invalid_union() {
        let tiny_limits = GatewayProtocolLimits {
            max_message_bytes: 32,
            ..GatewayProtocolLimits::default()
        };
        let error = decode_gateway_request(
            &serde_json::to_vec(&catalog_request()).unwrap(),
            tiny_limits,
        )
        .unwrap_err();
        assert_eq!(error.code, "gateway.protocol.message_oversize");

        let mut nested = json!(null);
        for _ in 0..40 {
            nested = json!([nested]);
        }
        let error = decode_bounded_json::<Value>(
            &serde_json::to_vec(&nested).unwrap(),
            GatewayProtocolLimits::default(),
        )
        .unwrap_err();
        assert_eq!(error.code, "gateway.protocol.depth_exceeded");

        let mut invalid = serde_json::to_value(catalog_request()).unwrap();
        invalid["payload"]["requestKind"] = json!("future_required_semantic");
        let error = decode_gateway_request(
            &serde_json::to_vec(&invalid).unwrap(),
            GatewayProtocolLimits::default(),
        )
        .unwrap_err();
        assert_eq!(error.code, "gateway.protocol.schema_invalid");
    }
}
