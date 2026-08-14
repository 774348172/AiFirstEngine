use crate::{
    ClientHello, ClientKind, ClientSessionBinding, GatewayControlError, GatewayDiscoveryRecord,
    GatewayNamedPipeClient, GatewayPipeWireReply, GatewayPipeWireRequest, GatewayReply,
    GatewayRequest, GatewayRequestPayload, GATEWAY_CLIENT_HELLO_SCHEMA_VERSION,
    GATEWAY_NAMED_PIPE_WIRE_SCHEMA_VERSION, GATEWAY_PROTOCOL_VERSION,
    GATEWAY_REQUEST_SCHEMA_VERSION,
};
use editor_core::{AI_TOOL_CATALOG_SCHEMA_VERSION, AI_TOOL_CATALOG_V1_SCHEMA_VERSION};
use std::path::Path;

const DEFAULT_RESPONSE_LIMIT_BYTES: u64 = 1024 * 1024;

pub struct GatewayRemoteAdapter {
    client: GatewayNamedPipeClient,
    binding: ClientSessionBinding,
    request_sequence: u64,
    restart_required: bool,
}

impl GatewayRemoteAdapter {
    pub fn connect_from_discovery(
        path: &Path,
        client_kind: ClientKind,
        client_version: impl Into<String>,
    ) -> Result<Self, GatewayControlError> {
        let bytes = std::fs::read(path).map_err(|error| {
            adapter_error(
                "gateway.adapter.discovery_read_failed",
                format!("Failed to read Gateway discovery record: {error}"),
                "Open the target project in the Editor and rediscover its endpoint.",
            )
        })?;
        let discovery: GatewayDiscoveryRecord =
            serde_json::from_slice(&bytes).map_err(|error| {
                adapter_error(
                    "gateway.adapter.discovery_parse_failed",
                    format!("Gateway discovery record is invalid JSON: {error}"),
                    "Discard stale discovery data and let the Editor republish it.",
                )
            })?;
        discovery.validate()?;
        let mut client = GatewayNamedPipeClient::connect(&discovery.pipe_locator)?;
        let connected = client.exchange(&GatewayPipeWireRequest::Connect {
            schema_version: GATEWAY_NAMED_PIPE_WIRE_SCHEMA_VERSION.to_string(),
            hello: ClientHello {
                schema_version: GATEWAY_CLIENT_HELLO_SCHEMA_VERSION.to_string(),
                gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
                client_kind,
                client_version: client_version.into(),
                supported_schema_versions: vec![
                    AI_TOOL_CATALOG_SCHEMA_VERSION.to_string(),
                    AI_TOOL_CATALOG_V1_SCHEMA_VERSION.to_string(),
                ],
                expected_editor_instance_id: discovery.editor_instance_id,
                requested_read_scope: vec!["catalog".to_string(), "project".to_string()],
            },
        })?;
        let GatewayPipeWireReply::Connected(binding) = connected else {
            return Err(reply_error(connected));
        };
        Ok(Self {
            client,
            binding,
            request_sequence: 0,
            restart_required: false,
        })
    }

    pub fn binding(&self) -> &ClientSessionBinding {
        &self.binding
    }

    pub fn restart_required(&self) -> bool {
        self.restart_required
    }

    pub fn dispatch(
        &mut self,
        payload: GatewayRequestPayload,
    ) -> Result<GatewayReply, GatewayControlError> {
        self.request_sequence = self.request_sequence.saturating_add(1);
        let reply = match self.client.exchange(&GatewayPipeWireRequest::Dispatch {
            schema_version: GATEWAY_NAMED_PIPE_WIRE_SCHEMA_VERSION.to_string(),
            request: GatewayRequest {
                schema_version: GATEWAY_REQUEST_SCHEMA_VERSION.to_string(),
                gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
                request_id: format!("adapter-request-{}", self.request_sequence),
                client_session_id: self.binding.client_session_id.clone(),
                deadline_epoch_ms: None,
                response_limit_bytes: DEFAULT_RESPONSE_LIMIT_BYTES,
                payload,
            },
        }) {
            Ok(reply) => reply,
            Err(error) => {
                self.restart_required = true;
                return Err(fixed_endpoint_unavailable(error));
            }
        };
        let GatewayPipeWireReply::Dispatched(reply) = reply else {
            self.restart_required = true;
            return Err(reply_error(reply));
        };
        if matches!(
            &reply.payload,
            crate::GatewayReplyPayload::Rejected(diagnostic)
                if adapter_restart_diagnostic(&diagnostic.code)
        ) {
            self.restart_required = true;
        }
        Ok(reply)
    }

    pub fn close(mut self) -> Result<crate::CloseReceipt, GatewayControlError> {
        let reply = self.client.exchange(&GatewayPipeWireRequest::Close {
            schema_version: GATEWAY_NAMED_PIPE_WIRE_SCHEMA_VERSION.to_string(),
            client_session_id: self.binding.client_session_id,
        })?;
        let GatewayPipeWireReply::Closed(receipt) = reply else {
            return Err(reply_error(reply));
        };
        Ok(receipt)
    }
}

fn adapter_restart_diagnostic(code: &str) -> bool {
    matches!(
        code,
        "gateway.status.reconnect_required"
            | "gateway.binding.stale_after_project_change"
            | "gateway.status.session_expired"
            | "gateway.binding.session_expired"
            | "gateway.status.session_unknown"
            | "gateway.binding.session_unknown"
    )
}

fn fixed_endpoint_unavailable(error: GatewayControlError) -> GatewayControlError {
    if adapter_restart_diagnostic(&error.code) {
        return error;
    }
    adapter_error(
        "gateway.status.reconnect_required",
        format!(
            "The fixed Gateway endpoint is no longer available ({}: {}).",
            error.code, error.message
        ),
        "End this Adapter process, rediscover the active Editor project, and start a fresh MCP process.",
    )
}

fn reply_error(reply: GatewayPipeWireReply) -> GatewayControlError {
    match reply {
        GatewayPipeWireReply::Rejected(error) => error,
        _ => adapter_error(
            "gateway.adapter.reply_kind_mismatch",
            "Gateway Named Pipe returned an unexpected reply kind.",
            "Reconnect using a compatible Adapter and Gateway protocol version.",
        ),
    }
}

fn adapter_error(
    code: impl Into<String>,
    message: impl Into<String>,
    next_action: impl Into<String>,
) -> GatewayControlError {
    GatewayControlError {
        code: code.into(),
        message: message.into(),
        next_action: next_action.into(),
    }
}
