use crate::{
    ClientHello, ClientKind, ClientSessionBinding, CloseReceipt, GatewayAccessDecision,
    GatewayAccessDecisionReceipt, GatewayAccessRequest, GatewayDiagnostic,
    GatewayMutationAccessState, GatewayMutationAccessStatus, GatewayProjectContext,
    GatewayReadAccessState, GatewayReadAccessStatus, GatewayReply, GatewayReplyPayload,
    GatewayRequest, GatewayRequestPayload, GatewaySessionAccessStatus, GatewaySessionCleanupReport,
    GatewaySessionIdentityStatus, GatewaySessionProjectStatus, GatewaySessionState,
    GatewaySessionStatus, GATEWAY_ACCESS_DECISION_RECEIPT_SCHEMA_VERSION,
    GATEWAY_ACCESS_REQUEST_SCHEMA_VERSION, GATEWAY_CLOSE_RECEIPT_SCHEMA_VERSION,
    GATEWAY_PROTOCOL_VERSION, GATEWAY_REPLY_SCHEMA_VERSION, GATEWAY_SESSION_BINDING_SCHEMA_VERSION,
    GATEWAY_SESSION_CLEANUP_REPORT_SCHEMA_VERSION, GATEWAY_SESSION_STATUS_SCHEMA_VERSION,
};
use editor_core::{
    AiCapabilityGrant, AiCapabilityGrantKind, AiCapabilityScopeMode, AiCapabilityToolKernel,
    AiGoalBinding, AiGoalCompletionPolicy, AiGoalGrantSpec, AiGoalRiskClass, AiRiskEnvelope,
    AiRiskEnvelopeSpec, AiToolAvailability, AiToolAvailabilityBasis, AiToolAvailabilityOwner,
    AiToolAvailabilityReason, AiToolAvailabilityReasonCategory, AiToolAvailabilityResolutionKind,
    AiToolAvailabilityState, AiToolCatalog, AiToolCatalogRequest, AiToolInspectPayload,
    AiToolInspectRequest, AiToolInvocation, AiToolInvocationPayload, AiToolKernelError,
    AiToolMutationAvailabilityState, AiToolOperationSnapshot, AiToolOutput,
    AiToolReadAvailabilityState, EditorSession, GoalMutationModule, GoalMutationOwnerFacts,
    ProjectCandidateEntry, ProjectCandidatePayload, ProjectPatchLlmContextSnapshot,
    AI_TOOL_CATALOG_SCHEMA_VERSION, AI_TOOL_CATALOG_V1_SCHEMA_VERSION, TOOL_ID_PROJECT_CREATE,
    TOOL_ID_PROJECT_MUTATE, TOOL_ID_PROJECT_ROLLBACK,
};
use engine_runtime::canonical_digest::{canonical_json_bytes, sha256_prefixed};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const SESSION_TTL_MS: u64 = 30 * 60 * 1000;
pub const GATEWAY_GRANT_REF_RECEIPT_SCHEMA_VERSION: &str = "ai-tool-gateway-grant-ref-receipt.v1";
pub const GATEWAY_SESSION_READ_GRANT_REF: &str = "$session_read";
pub const GATEWAY_ACTIVE_MUTATION_GRANT_REF: &str = "$active_mutation";
const DEFAULT_MAX_PENDING_REQUESTS: usize = 256;
static SESSION_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GatewayControlError {
    pub code: String,
    pub message: String,
    pub next_action: String,
}

impl std::fmt::Display for GatewayControlError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for GatewayControlError {}

#[derive(Debug, Clone)]
struct ActiveClientSession {
    binding: ClientSessionBinding,
    client_kind: ClientKind,
    client_version: String,
    connected_at_epoch_ms: u64,
    last_seen_at_epoch_ms: u64,
    observed_project_digest: Option<String>,
    read_generation: u64,
    read_grant: Option<AiCapabilityGrant>,
    read_stale_reason: Option<String>,
    active_mutation_grant_ref: Option<String>,
    mutation_state: GatewayMutationAccessState,
    access_generation: u64,
    operation_generation: u64,
}

#[derive(Debug, Clone)]
struct RegisteredGrant {
    client_session_id: String,
    grant: AiCapabilityGrant,
    revoked: bool,
}

#[derive(Debug, Clone)]
struct OperationGrantSnapshot {
    client_session_id: String,
    grant: AiCapabilityGrant,
    context_authority_active: bool,
    detached_terminal: Option<AiToolOperationSnapshot>,
    terminal_observed: bool,
}

#[derive(Debug, Clone)]
struct GoalMutationOperation {
    client_session_id: String,
    request_id: Option<String>,
    invocation: AiToolInvocation,
    awaiting_snapshot: AiToolOperationSnapshot,
    kernel_started: bool,
}

#[derive(Debug, Clone)]
struct GatewayRollbackReference {
    editor_instance_id: String,
    client_session_id: String,
    project_identity: String,
    expected_project_digest: String,
    expected_read_generation: u64,
    expires_at_epoch_ms: u64,
    mutation_receipt: editor_core::AiToolMutationReceipt,
    consumed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GatewayGrantRefReceipt {
    pub schema_version: String,
    pub grant_ref: String,
    pub client_session_id: String,
    pub project_identity: String,
    pub grant_kind: AiCapabilityGrantKind,
    pub scope_mode: AiCapabilityScopeMode,
    pub expires_at_epoch_ms: Option<u64>,
}

pub struct GatewayCore {
    editor_instance_id: String,
    kernel: AiCapabilityToolKernel,
    sessions: BTreeMap<String, ActiveClientSession>,
    grants: BTreeMap<String, RegisteredGrant>,
    access_requests: BTreeMap<String, GatewayAccessRequest>,
    operation_grants: BTreeMap<String, OperationGrantSnapshot>,
    goal_mutation_operations: BTreeMap<String, GoalMutationOperation>,
    rollback_references: BTreeMap<String, GatewayRollbackReference>,
    mutation_rollback_refs: BTreeMap<String, String>,
    pending_rollback_invocations: BTreeMap<(String, String), String>,
    rollback_operation_refs: BTreeMap<String, String>,
    allowed_read_scope: BTreeSet<String>,
}

impl Default for GatewayCore {
    fn default() -> Self {
        Self::new()
    }
}

impl GatewayCore {
    pub fn new() -> Self {
        Self::new_for_editor_instance(crate::default_editor_instance_id())
    }

    pub fn new_for_editor_instance(editor_instance_id: impl Into<String>) -> Self {
        Self {
            editor_instance_id: editor_instance_id.into(),
            kernel: AiCapabilityToolKernel::new(),
            sessions: BTreeMap::new(),
            grants: BTreeMap::new(),
            access_requests: BTreeMap::new(),
            operation_grants: BTreeMap::new(),
            goal_mutation_operations: BTreeMap::new(),
            rollback_references: BTreeMap::new(),
            mutation_rollback_refs: BTreeMap::new(),
            pending_rollback_invocations: BTreeMap::new(),
            rollback_operation_refs: BTreeMap::new(),
            allowed_read_scope: ["catalog", "project"]
                .into_iter()
                .map(str::to_string)
                .collect(),
        }
    }

    pub fn connect(
        &mut self,
        session: &mut EditorSession,
        hello: ClientHello,
    ) -> Result<ClientSessionBinding, GatewayControlError> {
        if hello.schema_version != crate::GATEWAY_CLIENT_HELLO_SCHEMA_VERSION
            || hello.gateway_protocol_version != GATEWAY_PROTOCOL_VERSION
        {
            return Err(control_error(
                "gateway.binding.protocol_unsupported",
                "ClientHello uses an unsupported Gateway protocol or schema version.",
                "Reconnect using the protocol versions advertised by discovery.",
            ));
        }
        if hello.expected_editor_instance_id != self.editor_instance_id {
            return Err(control_error(
                "gateway.binding.editor_instance_mismatch",
                "ClientHello editorInstanceId does not match this Editor Gateway.",
                "Rediscover the exact Editor instance and reconnect.",
            ));
        }
        let project = current_project_binding_optional(session)?;
        let catalog_schema_version = if hello
            .supported_schema_versions
            .iter()
            .any(|version| version == AI_TOOL_CATALOG_SCHEMA_VERSION)
        {
            AI_TOOL_CATALOG_SCHEMA_VERSION
        } else if hello
            .supported_schema_versions
            .iter()
            .any(|version| version == AI_TOOL_CATALOG_V1_SCHEMA_VERSION)
        {
            AI_TOOL_CATALOG_V1_SCHEMA_VERSION
        } else {
            return Err(control_error(
                "gateway.binding.schema_negotiation_failed",
                "Client does not advertise the Tool Catalog schema required by this Gateway.",
                "Advertise ai-tool-catalog.v2 or ai-tool-catalog.v1 and reconnect.",
            ));
        };

        let client_kind = hello.client_kind;
        let client_version = hello.client_version.clone();
        let mut effective_read_scope = hello
            .requested_read_scope
            .into_iter()
            .filter(|scope| self.allowed_read_scope.contains(scope))
            .collect::<Vec<_>>();
        effective_read_scope.sort();
        effective_read_scope.dedup();
        let catalog = self
            .kernel
            .catalog_for_session(
                session,
                AiToolCatalogRequest {
                    schema_version: catalog_schema_version.to_string(),
                },
            )
            .map_err(kernel_control_error)?;
        let catalog_digest = catalog.catalog_digest();
        let now = now_epoch_ms();
        let sequence = SESSION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let client_session_id = format!(
            "gateway-session-{}",
            sha256_prefixed(
                format!(
                    "{}|{}|{}|{}",
                    std::process::id(),
                    now,
                    sequence,
                    self.editor_instance_id
                )
                .as_bytes()
            )
            .trim_start_matches("sha256:")
            .chars()
            .take(32)
            .collect::<String>()
        );
        let project_context = project.as_ref().map(|project| GatewayProjectContext {
            project_identity: project.project_id.clone(),
            canonical_project_root_digest: canonical_root_digest(&project.project_root),
            project_digest: project.project_digest.clone(),
            read_generation: 1,
        });
        let read_grant = project_context
            .as_ref()
            .map(|context| {
                session_read_grant(
                    &client_session_id,
                    &context.project_identity,
                    &context.project_digest,
                    context.read_generation,
                )
            })
            .transpose()?;
        let binding = ClientSessionBinding {
            schema_version: GATEWAY_SESSION_BINDING_SCHEMA_VERSION.to_string(),
            client_session_id: client_session_id.clone(),
            editor_process_identity: format!("editor-pid-{}", std::process::id()),
            editor_instance_id: self.editor_instance_id.clone(),
            project_context: project_context.clone(),
            gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
            effective_read_scope,
            catalog_schema_version: catalog_schema_version.to_string(),
            catalog_digest,
            expires_at_epoch_ms: now.saturating_add(SESSION_TTL_MS),
        };
        self.sessions.insert(
            client_session_id,
            ActiveClientSession {
                binding: binding.clone(),
                client_kind,
                client_version,
                connected_at_epoch_ms: now,
                last_seen_at_epoch_ms: now,
                observed_project_digest: project_context
                    .as_ref()
                    .map(|context| context.project_digest.clone()),
                read_generation: 1,
                read_grant,
                read_stale_reason: None,
                active_mutation_grant_ref: None,
                mutation_state: GatewayMutationAccessState::NotRequested,
                access_generation: 1,
                operation_generation: 0,
            },
        );
        Ok(binding)
    }

    pub fn request_goal_mutation_access(
        &mut self,
        session: &EditorSession,
        client_session_id: &str,
        goal_binding: AiGoalBinding,
        risk_envelope: AiRiskEnvelope,
    ) -> Result<GatewayAccessRequest, GatewayControlError> {
        self.request_goal_mutation_access_internal(
            session,
            client_session_id,
            goal_binding,
            risk_envelope,
            None,
        )
    }

    fn request_goal_mutation_access_internal(
        &mut self,
        session: &EditorSession,
        client_session_id: &str,
        goal_binding: AiGoalBinding,
        risk_envelope: AiRiskEnvelope,
        operation_id: Option<String>,
    ) -> Result<GatewayAccessRequest, GatewayControlError> {
        self.reconcile_session_context(session, client_session_id)?;
        goal_binding
            .validate_integrity()
            .map_err(goal_grant_control_error)?;
        risk_envelope
            .validate_integrity()
            .map_err(goal_grant_control_error)?;
        if risk_envelope.risk_class == AiGoalRiskClass::ExactDomains {
            return Err(control_error(
                "gateway.access.risk_class_unsupported",
                "Goal mutation approval requires an engine-derived project-owned or elevated risk envelope.",
                "Submit project.mutate so the Gateway can derive the risk class from the actual change.",
            ));
        }
        let active = self
            .sessions
            .get(client_session_id)
            .cloned()
            .ok_or_else(|| {
                control_error(
                    "gateway.access.session_unknown",
                    "Goal mutation access requires an active Gateway client session.",
                    "Reconnect through the current discovery record.",
                )
            })?;
        let project = current_project_binding(session)?;
        let context = binding_project_context(&active.binding)?;
        if project.project_id != context.project_identity
            || active.observed_project_digest.as_deref() != Some(project.project_digest.as_str())
            || goal_binding.project_identity != context.project_identity
            || active.observed_project_digest.as_deref()
                != Some(goal_binding.initial_project_digest.as_str())
        {
            return Err(control_error(
                "gateway.access.goal_project_mismatch",
                "Goal approval does not bind the session's current project revision.",
                "Inspect the current project and create a goal binding from that revision.",
            ));
        }
        if active.active_mutation_grant_ref.is_some()
            || self
                .access_requests
                .values()
                .any(|request| request.client_session_id == client_session_id)
        {
            return Err(control_error(
                "gateway.access.goal_already_requested",
                "This Gateway session already has an active or pending mutation authority.",
                "Finish, reject, or revoke the current goal authority before requesting another.",
            ));
        }
        let now = now_epoch_ms();
        let request = mutation_access_request(
            &active.binding,
            active.client_kind,
            &active.client_version,
            now,
            goal_binding,
            risk_envelope,
            operation_id,
        )?;
        self.access_requests
            .insert(request.request_id.clone(), request.clone());
        if let Some(active) = self.sessions.get_mut(client_session_id) {
            active.mutation_state = GatewayMutationAccessState::AwaitingUser;
            active.access_generation = active.access_generation.saturating_add(1);
        }
        Ok(request)
    }

    pub fn dispatch(
        &mut self,
        session: &mut EditorSession,
        request: GatewayRequest,
    ) -> GatewayReply {
        let response_limit = request.response_limit_bytes as usize;
        let request_id = request.request_id.clone();
        let client_session_id = request.client_session_id.clone();
        let mut payload = match self.validate_request_binding(session, &request) {
            Ok(()) => self.dispatch_bound(session, request),
            Err(error) => GatewayReplyPayload::Rejected(diagnostic(error)),
        };
        self.attach_latest_availability_basis(session, &client_session_id, &mut payload);
        let mut reply = GatewayReply {
            schema_version: GATEWAY_REPLY_SCHEMA_VERSION.to_string(),
            gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
            request_id,
            client_session_id,
            payload,
        };
        if serde_json::to_vec(&reply)
            .map(|bytes| bytes.len() > response_limit)
            .unwrap_or(true)
        {
            reply.payload = GatewayReplyPayload::Rejected(GatewayDiagnostic {
                code: "gateway.response.limit_exceeded".to_string(),
                message: "Gateway response exceeds the requested bounded response size."
                    .to_string(),
                next_action: "Narrow or page the request and retry.".to_string(),
                availability: None,
            });
        }
        reply
    }

    pub fn close(&mut self, client_session_id: &str) -> CloseReceipt {
        let diagnostic_code = if self.remove_session(client_session_id).is_some() {
            "gateway.session.closed"
        } else {
            "gateway.session.already_absent"
        };
        let receipt = CloseReceipt {
            schema_version: GATEWAY_CLOSE_RECEIPT_SCHEMA_VERSION.to_string(),
            client_session_id: client_session_id.to_string(),
            closed_at_epoch_ms: now_epoch_ms(),
            diagnostic_code: diagnostic_code.to_string(),
        };
        receipt
    }

    pub fn issue_grant_ref(
        &mut self,
        session: &EditorSession,
        client_session_id: &str,
        grant: AiCapabilityGrant,
    ) -> Result<GatewayGrantRefReceipt, GatewayControlError> {
        self.reconcile_session_context(session, client_session_id)?;
        grant.validate_integrity().map_err(kernel_control_error)?;
        let active = self.sessions.get(client_session_id).ok_or_else(|| {
            control_error(
                "gateway.grant_ref.session_unknown",
                "Cannot issue a grant reference for an unknown client session.",
                "Connect through the active Editor before requesting approval.",
            )
        })?;
        if now_epoch_ms() >= active.binding.expires_at_epoch_ms {
            return Err(control_error(
                "gateway.grant_ref.session_expired",
                "Cannot issue a grant reference for a closed or expired client session.",
                "Reconnect and request approval against the current project.",
            ));
        }
        let project = current_project_binding(session)?;
        let context = binding_project_context(&active.binding)?;
        if project.project_id != context.project_identity
            || canonical_root_digest(&project.project_root) != context.canonical_project_root_digest
            || grant.project_identity != context.project_identity
        {
            return Err(control_error(
                "gateway.grant_ref.project_mismatch",
                "CapabilityGrant does not match the active bound Editor project.",
                "Inspect the active project and request a correctly scoped grant.",
            ));
        }
        if grant.kind != AiCapabilityGrantKind::Read
            && (active.read_stale_reason.is_some()
                || active.observed_project_digest.as_deref()
                    != Some(grant.initial_base_digest.as_str())
                || active.observed_project_digest.as_deref()
                    != Some(project.project_digest.as_str()))
        {
            return Err(control_error(
                "gateway.grant_ref.project_drifted",
                "Mutation approval no longer matches the session-observed project revision.",
                "Inspect the current project, then request a fresh mutation approval.",
            ));
        }
        let sequence = SESSION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let grant_ref = format!(
            "grant-ref-{}",
            sha256_prefixed(
                format!(
                    "{}|{}|{}|{}",
                    client_session_id,
                    grant.grant_digest,
                    now_epoch_ms(),
                    sequence
                )
                .as_bytes()
            )
            .trim_start_matches("sha256:")
            .chars()
            .take(32)
            .collect::<String>()
        );
        if grant.kind != AiCapabilityGrantKind::Read {
            if let Some(previous) = self
                .sessions
                .get(client_session_id)
                .and_then(|active| active.active_mutation_grant_ref.clone())
            {
                if let Some(registered) = self.grants.get_mut(&previous) {
                    registered.revoked = true;
                }
            }
        }
        self.grants.insert(
            grant_ref.clone(),
            RegisteredGrant {
                client_session_id: client_session_id.to_string(),
                grant: grant.clone(),
                revoked: false,
            },
        );
        if grant.kind != AiCapabilityGrantKind::Read {
            if let Some(active) = self.sessions.get_mut(client_session_id) {
                active.active_mutation_grant_ref = Some(grant_ref.clone());
                active.mutation_state = GatewayMutationAccessState::Active;
                active.access_generation = active.access_generation.saturating_add(1);
            }
            self.access_requests
                .retain(|_, request| request.client_session_id != client_session_id);
        }
        Ok(GatewayGrantRefReceipt {
            schema_version: GATEWAY_GRANT_REF_RECEIPT_SCHEMA_VERSION.to_string(),
            grant_ref,
            client_session_id: client_session_id.to_string(),
            project_identity: grant.project_identity,
            grant_kind: grant.kind,
            scope_mode: grant.scope_mode,
            expires_at_epoch_ms: grant.expires_at_epoch_ms,
        })
    }

    pub fn active_client_bindings(&self) -> Vec<ClientSessionBinding> {
        let now = now_epoch_ms();
        self.sessions
            .values()
            .filter(|session| session.binding.expires_at_epoch_ms > now)
            .map(|session| session.binding.clone())
            .collect()
    }

    pub fn client_has_active_grant(&self, client_session_id: &str) -> bool {
        self.sessions
            .get(client_session_id)
            .and_then(|session| session.active_mutation_grant_ref.as_ref())
            .and_then(|grant_ref| self.grants.get(grant_ref))
            .is_some_and(|registered| {
                !registered.revoked
                    && registered.grant.kind != AiCapabilityGrantKind::Read
                    && registered.grant.validate_integrity().is_ok()
            })
    }

    pub fn revoke_grant_ref(&mut self, grant_ref: &str) -> Result<(), GatewayControlError> {
        let registered = self.grants.get_mut(grant_ref).ok_or_else(|| {
            control_error(
                "gateway.grant_ref.unknown",
                "Opaque grant reference is unknown.",
                "Request a new Native Editor approval.",
            )
        })?;
        registered.revoked = true;
        let client_session_id = registered.client_session_id.clone();
        if self
            .sessions
            .get(&client_session_id)
            .and_then(|session| session.active_mutation_grant_ref.as_deref())
            == Some(grant_ref)
        {
            if let Some(session) = self.sessions.get_mut(&client_session_id) {
                session.active_mutation_grant_ref = None;
                session.mutation_state = GatewayMutationAccessState::Revoked;
                session.access_generation = session.access_generation.saturating_add(1);
            }
        }
        Ok(())
    }

    pub fn renew_grant_ref(
        &mut self,
        grant_ref: &str,
        expires_at_epoch_ms: u64,
    ) -> Result<GatewayGrantRefReceipt, GatewayControlError> {
        if expires_at_epoch_ms <= now_epoch_ms() {
            return Err(control_error(
                "gateway.grant_ref.renewal_expired",
                "Grant renewal expiry must be in the future.",
                "Choose a bounded future expiry after user confirmation.",
            ));
        }
        let registered = self.grants.get_mut(grant_ref).ok_or_else(|| {
            control_error(
                "gateway.grant_ref.unknown",
                "Opaque grant reference is unknown.",
                "Request a new Native Editor approval.",
            )
        })?;
        if registered.revoked {
            return Err(control_error(
                "gateway.grant_ref.revoked",
                "Revoked grant references cannot be renewed.",
                "Request a new Native Editor approval.",
            ));
        }
        let changed = registered.grant.expires_at_epoch_ms != Some(expires_at_epoch_ms);
        if changed {
            registered.grant.expires_at_epoch_ms = Some(expires_at_epoch_ms);
            registered.grant = registered
                .grant
                .clone()
                .seal()
                .map_err(kernel_control_error)?;
            if let Some(active) = self.sessions.get_mut(&registered.client_session_id) {
                active.access_generation = active.access_generation.saturating_add(1);
            }
        }
        Ok(GatewayGrantRefReceipt {
            schema_version: GATEWAY_GRANT_REF_RECEIPT_SCHEMA_VERSION.to_string(),
            grant_ref: grant_ref.to_string(),
            client_session_id: registered.client_session_id.clone(),
            project_identity: registered.grant.project_identity.clone(),
            grant_kind: registered.grant.kind,
            scope_mode: registered.grant.scope_mode,
            expires_at_epoch_ms: registered.grant.expires_at_epoch_ms,
        })
    }

    pub fn approval_inbox(&mut self, now_epoch_ms: u64) -> Vec<GatewayAccessRequest> {
        self.expire_goal_mutation_requests(now_epoch_ms);
        self.access_requests
            .values()
            .filter(|request| {
                request.expires_at_epoch_ms > now_epoch_ms
                    && self.sessions.contains_key(&request.client_session_id)
            })
            .cloned()
            .collect()
    }

    pub fn decide_access(
        &mut self,
        session: &EditorSession,
        request_id: &str,
        decision: GatewayAccessDecision,
        actor: &str,
        decided_at_epoch_ms: u64,
    ) -> Result<GatewayAccessDecisionReceipt, GatewayControlError> {
        if actor.trim().is_empty() {
            return Err(control_error(
                "gateway.access.actor_required",
                "Access decisions require a non-empty Native Editor actor identity.",
                "Retry from the Native Editor approval command with the current user identity.",
            ));
        }
        let request = self
            .access_requests
            .get(request_id)
            .cloned()
            .ok_or_else(|| {
                control_error(
                    "gateway.access.request_stale",
                    "Access request is absent, already decided, or no longer active.",
                    "Refresh the Gateway access inbox before deciding.",
                )
            })?;
        self.reconcile_session_context(session, &request.client_session_id)?;
        if !self.access_requests.contains_key(request_id) {
            return Err(control_error(
                "gateway.access.request_stale",
                "Access request was invalidated by an Editor project context transition.",
                "Inspect the current context and request fresh mutation authority if needed.",
            ));
        }
        let active = self
            .sessions
            .get(&request.client_session_id)
            .cloned()
            .ok_or_else(|| {
                control_error(
                    "gateway.access.request_stale",
                    "Access request belongs to a session that is no longer active.",
                    "Refresh the Gateway access inbox before deciding.",
                )
            })?;
        if decided_at_epoch_ms >= active.binding.expires_at_epoch_ms {
            self.remove_session(&request.client_session_id);
            return Err(control_error(
                "gateway.access.request_expired",
                "Access request expired with its Gateway client session.",
                "Reconnect the client and decide the newly issued request.",
            ));
        }
        if decided_at_epoch_ms >= request.expires_at_epoch_ms {
            self.access_requests.remove(request_id);
            if let Some(operation_id) = request.operation_id.as_deref() {
                self.finish_awaiting_goal_mutation(
                    operation_id,
                    editor_core::AiToolOperationState::Failed,
                    "gateway.access.request_expired",
                    "Goal mutation approval expired before the Native Editor decision.",
                );
            }
            if let Some(active) = self.sessions.get_mut(&request.client_session_id) {
                active.mutation_state = GatewayMutationAccessState::Expired;
                active.access_generation = active.access_generation.saturating_add(1);
            }
            return Err(control_error(
                "gateway.access.request_expired",
                "Access request expired before the Native Editor decision.",
                "Inspect the terminal operation and submit a new mutation only if still needed.",
            ));
        }
        let project = current_project_binding(session)?;
        let context = binding_project_context(&active.binding)?;
        if project.project_id != context.project_identity
            || canonical_root_digest(&project.project_root) != context.canonical_project_root_digest
        {
            return Err(control_error(
                "gateway.access.request_stale_project",
                "Access request belongs to a project context that is no longer active.",
                "Inspect the active project and request fresh mutation authority.",
            ));
        }
        if project.project_digest != request.observed_project_digest
            || active.observed_project_digest.as_deref()
                != Some(request.observed_project_digest.as_str())
        {
            self.mark_session_read_stale(&request.client_session_id);
            return Err(control_error(
                "gateway.access.request_stale_project",
                "Access request was reviewed against an older project revision.",
                "Inspect the current project and decide a newly generated request.",
            ));
        }

        self.access_requests.remove(request_id);
        let operation_id = request.operation_id.clone();
        let (mutation_state, grant_ref, grant_digest, diagnostic_code) = match decision {
            GatewayAccessDecision::Reject => {
                if let Some(active) = self.sessions.get_mut(&request.client_session_id) {
                    active.mutation_state = GatewayMutationAccessState::Revoked;
                    active.active_mutation_grant_ref = None;
                    active.access_generation = active.access_generation.saturating_add(1);
                }
                if let Some(operation_id) = operation_id.as_deref() {
                    self.finish_awaiting_goal_mutation(
                        operation_id,
                        editor_core::AiToolOperationState::Failed,
                        "gateway.access.rejected",
                        "Native Editor rejected the goal mutation.",
                    );
                }
                (
                    GatewayMutationAccessState::Revoked,
                    None,
                    None,
                    "gateway.access.rejected",
                )
            }
            GatewayAccessDecision::Approve => {
                validate_access_request_digest(&request)?;
                let spec = AiGoalGrantSpec::new(
                    request.goal_binding.clone(),
                    request.risk_envelope.clone(),
                    request.client_session_id.clone(),
                    actor,
                    Some(request.expires_at_epoch_ms),
                )
                .map_err(goal_grant_control_error)?;
                let grant =
                    match request.risk_envelope.risk_class {
                        AiGoalRiskClass::ProjectOwnedLowRisk => {
                            AiCapabilityGrant::project_owned_low_risk_for_goal(spec)
                        }
                        AiGoalRiskClass::Elevated => AiCapabilityGrant::elevated_for_goal(spec),
                        AiGoalRiskClass::ExactDomains => return Err(control_error(
                            "gateway.access.risk_class_unsupported",
                            "ExactDomains is not valid for engine-derived goal mutation approval.",
                            "Submit project.mutate so the Gateway derives a supported risk class.",
                        )),
                    }
                    .map_err(kernel_control_error)?;
                let receipt =
                    self.issue_grant_ref(session, &request.client_session_id, grant.clone())?;
                if let Some(operation_id) = operation_id.as_deref() {
                    self.start_approved_goal_mutation(session, operation_id, grant.clone())?;
                }
                (
                    GatewayMutationAccessState::Active,
                    Some(receipt.grant_ref),
                    Some(grant.grant_digest),
                    "gateway.access.approved",
                )
            }
        };
        Ok(GatewayAccessDecisionReceipt {
            schema_version: GATEWAY_ACCESS_DECISION_RECEIPT_SCHEMA_VERSION.to_string(),
            request_id: request.request_id,
            client_session_id: request.client_session_id,
            decision,
            decided_by: actor.to_string(),
            decided_at_epoch_ms,
            mutation_state,
            grant_ref,
            grant_digest,
            diagnostic_code: diagnostic_code.to_string(),
        })
    }

    pub fn session_status(
        &mut self,
        session: &EditorSession,
        client_session_id: &str,
        status_at_epoch_ms: u64,
    ) -> Result<GatewaySessionStatus, GatewayControlError> {
        self.reconcile_session_context(session, client_session_id)?;
        let active = self
            .sessions
            .get(client_session_id)
            .cloned()
            .ok_or_else(|| {
                control_error(
                    "gateway.status.session_unknown",
                    "Gateway session status is unavailable because the session is not active.",
                    "Reconnect through the current discovery record.",
                )
            })?;
        if status_at_epoch_ms >= active.binding.expires_at_epoch_ms {
            self.remove_session(client_session_id);
            return Err(control_error(
                "gateway.status.session_expired",
                "Gateway session expired before status was requested.",
                "Reconnect through the current discovery record.",
            ));
        }
        if let Some(active) = self.sessions.get_mut(client_session_id) {
            active.last_seen_at_epoch_ms = status_at_epoch_ms;
        }
        self.expire_goal_mutation_requests(status_at_epoch_ms);
        self.expire_active_mutation(client_session_id, status_at_epoch_ms);

        let project = current_project_binding_optional(session)?;
        let runtime_module = if project.is_some() {
            let inspection = self
                .kernel
                .inspect(session, AiToolInspectRequest::project())
                .map_err(kernel_control_error)?;
            match inspection.payload {
                AiToolInspectPayload::Project(project) => project.runtime_module_id,
                AiToolInspectPayload::GrantLineage(_) => String::new(),
            }
        } else {
            String::new()
        };
        let active = self
            .sessions
            .get(client_session_id)
            .cloned()
            .expect("validated Gateway session");
        let mutation = self.mutation_access_status(&active);
        let read_state = if active.binding.project_context.is_none() {
            GatewayReadAccessState::Unavailable
        } else if active.read_stale_reason.is_some() {
            GatewayReadAccessState::Stale
        } else {
            GatewayReadAccessState::Active
        };
        let next_action = match (read_state, mutation.state) {
            (GatewayReadAccessState::Unavailable, _) => {
                "Open or create a project before using project-scoped tools."
            }
            (GatewayReadAccessState::Stale, _) => {
                "Inspect the current project before dispatching more project tools."
            }
            (_, GatewayMutationAccessState::AwaitingUser) => {
                "Read tools are ready; wait for one Native Editor mutation decision."
            }
            (_, GatewayMutationAccessState::NotRequested) => {
                "Read tools are ready; request goal-bound mutation authority only when needed."
            }
            (_, GatewayMutationAccessState::Active) => {
                "Read and approved mutation tools are ready for this session."
            }
            _ => "Read tools remain ready; request a fresh mutation approval if needed.",
        };
        Ok(GatewaySessionStatus {
            schema_version: GATEWAY_SESSION_STATUS_SCHEMA_VERSION.to_string(),
            session: GatewaySessionIdentityStatus {
                id: active.binding.client_session_id.clone(),
                editor_instance_id: active.binding.editor_instance_id.clone(),
                client_kind: active.client_kind,
                client_version: active.client_version.clone(),
                connected_at_epoch_ms: active.connected_at_epoch_ms,
                last_seen_at_epoch_ms: active.last_seen_at_epoch_ms,
                age_ms: status_at_epoch_ms.saturating_sub(active.connected_at_epoch_ms),
                expires_at_epoch_ms: active.binding.expires_at_epoch_ms,
                state: GatewaySessionState::Active,
            },
            project: project.map(|project| GatewaySessionProjectStatus {
                identity: project.project_id,
                current_digest: project.project_digest.clone(),
                observed_digest: active
                    .observed_project_digest
                    .clone()
                    .unwrap_or(project.project_digest),
                runtime_module,
                catalog_digest: active.binding.catalog_digest.clone(),
            }),
            access: GatewaySessionAccessStatus {
                read: GatewayReadAccessStatus {
                    state: read_state,
                    effective_scopes: active.binding.effective_read_scope.clone(),
                    generation: active.read_generation,
                    grant_digest: active
                        .read_grant
                        .as_ref()
                        .map(|grant| grant.grant_digest.clone())
                        .unwrap_or_default(),
                    stale_reason: active.read_stale_reason.clone(),
                },
                mutation,
                access_generation: active.access_generation,
            },
            operation_generation: active.operation_generation,
            reconnect_required: false,
            next_action: next_action.to_string(),
        })
    }

    pub fn prune(
        &mut self,
        session: &EditorSession,
        prune_at_epoch_ms: u64,
    ) -> GatewaySessionCleanupReport {
        let session_ids = self.sessions.keys().cloned().collect::<Vec<_>>();
        for client_session_id in &session_ids {
            let _ = self.reconcile_session_context(session, client_session_id);
        }
        let mut expired_session_ids = Vec::new();
        for (client_session_id, active) in &self.sessions {
            if prune_at_epoch_ms >= active.binding.expires_at_epoch_ms {
                expired_session_ids.push(client_session_id.clone());
            }
        }
        let mut revoked_grant_count = 0;
        let mut removed_access_request_count = 0;
        for client_session_id in &expired_session_ids {
            if let Some((grants, requests)) = self.remove_session(client_session_id) {
                revoked_grant_count += grants;
                removed_access_request_count += requests;
            }
        }
        GatewaySessionCleanupReport {
            schema_version: GATEWAY_SESSION_CLEANUP_REPORT_SCHEMA_VERSION.to_string(),
            pruned_at_epoch_ms: prune_at_epoch_ms,
            closed_session_ids: Vec::new(),
            expired_session_ids,
            reconnect_required_session_ids: Vec::new(),
            revoked_grant_count,
            removed_access_request_count,
        }
    }

    fn dispatch_bound(
        &mut self,
        session: &mut EditorSession,
        request: GatewayRequest,
    ) -> GatewayReplyPayload {
        let client_session_id = request.client_session_id.clone();
        match request.payload {
            GatewayRequestPayload::SessionStatus => self
                .session_status(session, &client_session_id, now_epoch_ms())
                .map(GatewayReplyPayload::SessionStatus)
                .unwrap_or_else(|error| GatewayReplyPayload::Rejected(diagnostic(error))),
            GatewayRequestPayload::Catalog(request) => {
                match self.require_read_scope(&client_session_id, "catalog") {
                    Ok(()) => self
                        .catalog_for_client(session, &client_session_id, request)
                        .map(GatewayReplyPayload::Catalog)
                        .unwrap_or_else(|error| {
                            GatewayReplyPayload::Rejected(diagnostic(kernel_control_error(error)))
                        }),
                    Err(error) => GatewayReplyPayload::Rejected(diagnostic(error)),
                }
            }
            GatewayRequestPayload::Inspect(request) => {
                if let Err(error) = self.require_read_scope(&client_session_id, "project") {
                    return GatewayReplyPayload::Rejected(diagnostic(error));
                }
                match self.kernel.inspect(session, request) {
                    Ok(result) => {
                        if let AiToolInspectPayload::Project(project) = &result.payload {
                            if let Err(error) = self.refresh_observed_project(
                                &client_session_id,
                                &project.project_digest,
                            ) {
                                return GatewayReplyPayload::Rejected(diagnostic(error));
                            }
                        }
                        GatewayReplyPayload::Inspection(result)
                    }
                    Err(error) => {
                        GatewayReplyPayload::Rejected(diagnostic(kernel_control_error(error)))
                    }
                }
            }
            GatewayRequestPayload::ExecuteSessionBound { mut invocation } => {
                if let Err(error) =
                    self.bind_goal_mutation_invocation(session, &client_session_id, &mut invocation)
                {
                    return GatewayReplyPayload::Rejected(diagnostic(error));
                }
                if let Err(error) =
                    self.bind_rollback_ref_invocation(session, &client_session_id, &mut invocation)
                {
                    return GatewayReplyPayload::Rejected(diagnostic(error));
                }
                bind_session_owned_invocation_facts(session, &mut invocation);
                if invocation.tool_id == TOOL_ID_PROJECT_MUTATE {
                    return self.dispatch_goal_mutation(session, &client_session_id, invocation);
                }
                let tool_id = invocation.tool_id.clone();
                if invocation.tool_id == TOOL_ID_PROJECT_CREATE
                    || matches!(
                        invocation.payload,
                        AiToolInvocationPayload::ProjectCreate(_)
                    )
                {
                    let next_read_generation = self
                        .sessions
                        .get(&client_session_id)
                        .map(|active| active.read_generation.saturating_add(1))
                        .unwrap_or(1);
                    let result = self.kernel.execute_launcher_project_create(
                        session,
                        invocation,
                        next_read_generation,
                    );
                    if result.status == editor_core::AiToolExecutionStatus::Completed {
                        if let Err(error) =
                            self.reconcile_session_context(session, &client_session_id)
                        {
                            return GatewayReplyPayload::Rejected(diagnostic(error));
                        }
                    }
                    return GatewayReplyPayload::ToolResult(result);
                }
                match self.resolve_session_bound_grant(&client_session_id, &invocation) {
                    Ok(grant) => {
                        self.start_operation(session, &client_session_id, invocation, grant)
                    }
                    Err(error) => GatewayReplyPayload::Rejected(self.diagnostic_for_invocation(
                        session,
                        &client_session_id,
                        &tool_id,
                        error,
                    )),
                }
            }
            GatewayRequestPayload::Execute {
                mut invocation,
                grant_ref,
            } => {
                if let Err(error) =
                    self.bind_goal_mutation_invocation(session, &client_session_id, &mut invocation)
                {
                    return GatewayReplyPayload::Rejected(diagnostic(error));
                }
                if let Err(error) =
                    self.bind_rollback_ref_invocation(session, &client_session_id, &mut invocation)
                {
                    return GatewayReplyPayload::Rejected(diagnostic(error));
                }
                bind_session_owned_invocation_facts(session, &mut invocation);
                if invocation.tool_id == TOOL_ID_PROJECT_MUTATE {
                    return self.dispatch_goal_mutation(session, &client_session_id, invocation);
                }
                let tool_id = invocation.tool_id.clone();
                let grant = match invocation_access_class(&invocation) {
                    SessionInvocationAccessClass::Read => self
                        .require_read_scope(&client_session_id, "project")
                        .and_then(|()| self.resolve_grant(&client_session_id, &grant_ref)),
                    SessionInvocationAccessClass::Mutation => {
                        self.resolve_grant(&client_session_id, &grant_ref)
                    }
                };
                match grant {
                    Ok(grant) => {
                        self.start_operation(session, &client_session_id, invocation, grant)
                    }
                    Err(error) => GatewayReplyPayload::Rejected(self.diagnostic_for_invocation(
                        session,
                        &client_session_id,
                        &tool_id,
                        error,
                    )),
                }
            }
            GatewayRequestPayload::Observe { operation_id } => {
                if let Some(snapshot) =
                    self.goal_mutation_operation_snapshot(&client_session_id, &operation_id)
                {
                    return snapshot
                        .map(GatewayReplyPayload::Operation)
                        .unwrap_or_else(|error| GatewayReplyPayload::Rejected(diagnostic(error)));
                }
                match self.resolve_operation_snapshot(&client_session_id, &operation_id) {
                    Ok(snapshot) => snapshot
                        .detached_terminal
                        .map(GatewayReplyPayload::Operation)
                        .unwrap_or_else(|| {
                            self.kernel
                                .observe(&operation_id)
                                .map(GatewayReplyPayload::Operation)
                                .unwrap_or_else(|error| {
                                    GatewayReplyPayload::Rejected(diagnostic(kernel_control_error(
                                        error,
                                    )))
                                })
                        }),
                    Err(error) => GatewayReplyPayload::Rejected(diagnostic(error)),
                }
            }
            GatewayRequestPayload::Cancel {
                operation_id,
                grant_ref,
            } => match self.resolve_grant(&client_session_id, &grant_ref) {
                Ok(grant) => self
                    .kernel
                    .cancel_durable(session, &operation_id, &grant)
                    .map(GatewayReplyPayload::Cancellation)
                    .unwrap_or_else(|error| {
                        GatewayReplyPayload::Rejected(diagnostic(kernel_control_error(error)))
                    }),
                Err(error) => GatewayReplyPayload::Rejected(diagnostic(error)),
            },
            GatewayRequestPayload::CancelSessionBound { operation_id } => {
                if self
                    .goal_mutation_operations
                    .get(&operation_id)
                    .is_some_and(|operation| !operation.kernel_started)
                {
                    return match self.cancel_awaiting_goal_mutation(
                        &client_session_id,
                        &operation_id,
                        "gateway.operation.cancelled_awaiting_user",
                    ) {
                        Ok(receipt) => GatewayReplyPayload::Cancellation(receipt),
                        Err(error) => GatewayReplyPayload::Rejected(diagnostic(error)),
                    };
                }
                match self.resolve_cancellable_operation_snapshot(&client_session_id, &operation_id)
                {
                    Ok(snapshot) => self
                        .kernel
                        .cancel_durable(session, &operation_id, &snapshot.grant)
                        .map(GatewayReplyPayload::Cancellation)
                        .unwrap_or_else(|error| {
                            GatewayReplyPayload::Rejected(diagnostic(kernel_control_error(error)))
                        }),
                    Err(error) => GatewayReplyPayload::Rejected(diagnostic(error)),
                }
            }
        }
    }

    fn catalog_for_client(
        &self,
        session: &EditorSession,
        client_session_id: &str,
        request: AiToolCatalogRequest,
    ) -> Result<AiToolCatalog, AiToolKernelError> {
        let active = self
            .sessions
            .get(client_session_id)
            .ok_or_else(|| AiToolKernelError {
                code: "ai_tool.catalog_session_unknown".to_string(),
                message: "Catalog requires an active Gateway session.".to_string(),
                next_action: "Reconnect through the current discovery record.".to_string(),
            })?;
        if request.schema_version != active.binding.catalog_schema_version {
            return Err(AiToolKernelError {
                code: "ai_tool.catalog_session_schema_mismatch".to_string(),
                message: "Catalog request does not match the schema negotiated for this session."
                    .to_string(),
                next_action: "Use the catalog schema recorded in the session binding.".to_string(),
            });
        }
        let context = self.availability_context_for_client(session, client_session_id)?;
        self.kernel
            .catalog_for_session_with_context(request, context)
    }

    fn bind_goal_mutation_invocation(
        &self,
        session: &EditorSession,
        client_session_id: &str,
        invocation: &mut AiToolInvocation,
    ) -> Result<(), GatewayControlError> {
        if invocation.tool_id != TOOL_ID_PROJECT_MUTATE {
            return Ok(());
        }
        let intent = match &invocation.payload {
            AiToolInvocationPayload::ProjectMutationIntent(intent) => intent.clone(),
            _ => {
                return Err(control_error(
                    "gateway.mutation.intent_required",
                    "project.mutate accepts only the caller-owned goal and change intent.",
                    "Refresh the Tool Catalog and remove Candidate, project, Grant, operation, and receipt fields.",
                ))
            }
        };
        let active = self.sessions.get(client_session_id).ok_or_else(|| {
            control_error(
                "gateway.access.session_unknown",
                "Project mutation binding requires an active Gateway client session.",
                "Reconnect through the current discovery record.",
            )
        })?;
        let bound = GoalMutationModule::bind(
            session,
            intent,
            GoalMutationOwnerFacts {
                client_session_id: client_session_id.to_string(),
                read_generation: active.read_generation,
            },
        )
        .map_err(|error| control_error(error.code, error.message, error.next_action))?;
        invocation.payload = AiToolInvocationPayload::BoundGoalMutation(bound);
        Ok(())
    }

    fn bind_rollback_ref_invocation(
        &mut self,
        session: &EditorSession,
        client_session_id: &str,
        invocation: &mut AiToolInvocation,
    ) -> Result<(), GatewayControlError> {
        if invocation.tool_id != TOOL_ID_PROJECT_ROLLBACK {
            return Ok(());
        }
        let rollback_ref = match &invocation.payload {
            AiToolInvocationPayload::ProjectRollbackRef(input) => input.rollback_ref.clone(),
            _ => {
                return Err(control_error(
                    "gateway.rollback_ref.input_required",
                    "project.rollback accepts only one opaque rollbackRef.",
                    "Refresh the Tool Catalog and remove Candidate receipt or Grant fields.",
                ))
            }
        };
        let record = self
            .rollback_references
            .get(&rollback_ref)
            .cloned()
            .ok_or_else(|| {
                control_error(
                    "gateway.rollback_ref.unknown",
                    "rollbackRef is unknown to this Editor Gateway.",
                    "Use the rollbackRef returned by a completed project.mutate operation.",
                )
            })?;
        if record.editor_instance_id != self.editor_instance_id
            || record.client_session_id != client_session_id
        {
            return Err(control_error(
                "gateway.rollback_ref.session_mismatch",
                "rollbackRef belongs to a different Editor instance or MCP client session.",
                "Rollback from the exact session that received the mutation result.",
            ));
        }
        if record.consumed {
            return Err(control_error(
                "gateway.rollback_ref.consumed",
                "rollbackRef was already consumed by a successful rollback.",
                "Inspect the restored project; do not replay this rollbackRef.",
            ));
        }
        if now_epoch_ms() >= record.expires_at_epoch_ms {
            return Err(control_error(
                "gateway.rollback_ref.expired",
                "rollbackRef expired with its owning Gateway session lifetime.",
                "Inspect the current project and create a fresh mutation only if still needed.",
            ));
        }
        let active = self.sessions.get(client_session_id).ok_or_else(|| {
            control_error(
                "gateway.rollback_ref.session_inactive",
                "rollbackRef belongs to a Gateway session that is no longer active.",
                "Do not transfer rollbackRef across reconnects.",
            )
        })?;
        let project = current_project_binding(session)?;
        if record.project_identity != project.project_id
            || active
                .binding
                .project_context
                .as_ref()
                .map(|context| context.project_identity.as_str())
                != Some(record.project_identity.as_str())
        {
            return Err(control_error(
                "gateway.rollback_ref.project_mismatch",
                "rollbackRef belongs to a different project.",
                "Return to the exact project and session that produced the mutation.",
            ));
        }
        if active.read_stale_reason.is_some()
            || active.read_generation != record.expected_read_generation
            || active.observed_project_digest.as_deref()
                != Some(record.expected_project_digest.as_str())
            || project.project_digest != record.expected_project_digest
        {
            return Err(control_error(
                "gateway.rollback_ref.project_drifted",
                "Project digest or Read generation changed after rollbackRef was issued.",
                "Inspect the current project; rollbackRef will not overwrite later changes.",
            ));
        }
        if mutation_receipt_digest(&record.mutation_receipt)
            != record.mutation_receipt.receipt_digest
        {
            return Err(control_error(
                "gateway.rollback_ref.receipt_mismatch",
                "The mutation receipt bound to rollbackRef failed integrity validation.",
                "Do not attempt rollback; preserve the project and inspect the operation evidence.",
            ));
        }

        invocation.expected_project_digest = record.expected_project_digest;
        invocation.payload = AiToolInvocationPayload::RollbackCandidate {
            receipt: record.mutation_receipt.candidate_receipt,
        };
        self.pending_rollback_invocations.insert(
            (
                client_session_id.to_string(),
                invocation.invocation_id.clone(),
            ),
            rollback_ref,
        );
        Ok(())
    }

    fn dispatch_goal_mutation(
        &mut self,
        session: &EditorSession,
        client_session_id: &str,
        invocation: AiToolInvocation,
    ) -> GatewayReplyPayload {
        let bound =
            match &invocation.payload {
                AiToolInvocationPayload::BoundGoalMutation(bound) => bound.clone(),
                _ => return GatewayReplyPayload::Rejected(diagnostic(control_error(
                    "gateway.mutation.binding_missing",
                    "project.mutate did not produce an engine-owned goal binding.",
                    "Discard this invocation and submit the declared project.mutate intent again.",
                ))),
            };
        let (goal_binding, risk_envelope) = match goal_mutation_authority(&bound) {
            Ok(authority) => authority,
            Err(error) => return GatewayReplyPayload::Rejected(diagnostic(error)),
        };
        if let Some(grant) = self.eligible_goal_mutation_grant(
            client_session_id,
            &bound,
            &goal_binding,
            &risk_envelope,
        ) {
            return self.start_operation(session, client_session_id, invocation, grant);
        }

        self.revoke_ineligible_active_mutation(client_session_id);
        let operation_id =
            AiCapabilityToolKernel::operation_id_for_invocation(&invocation, "awaiting-user");
        if let Some(existing) = self.goal_mutation_operations.get(&operation_id) {
            if existing.invocation == invocation {
                return GatewayReplyPayload::Accepted(accepted_from_gateway_snapshot(
                    &existing.awaiting_snapshot,
                ));
            }
            return GatewayReplyPayload::Rejected(diagnostic(control_error(
                "gateway.operation.identity_collision",
                "Goal mutation operation identity was reused with different content.",
                "Submit the changed mutation with a new invocation identity.",
            )));
        }
        let request = match self.request_goal_mutation_access_internal(
            session,
            client_session_id,
            goal_binding,
            risk_envelope,
            Some(operation_id.clone()),
        ) {
            Ok(request) => request,
            Err(error) => return GatewayReplyPayload::Rejected(diagnostic(error)),
        };
        let started_at_epoch_ms = now_epoch_ms();
        let snapshot = AiToolOperationSnapshot {
            schema_version: editor_core::AI_TOOL_OPERATION_SCHEMA_VERSION.to_string(),
            operation_id: operation_id.clone(),
            invocation_id: invocation.invocation_id.clone(),
            invocation_digest: gateway_invocation_digest(&invocation),
            tool_id: invocation.tool_id.clone(),
            grant_digest: format!("awaiting:{}", request.approval_digest),
            project_identity: bound.project_binding.project_id.clone(),
            state: editor_core::AiToolOperationState::AwaitingUser,
            stage: "awaiting_user".to_string(),
            started_at_epoch_ms,
            completed_at_epoch_ms: None,
            result: None,
            artifact_refs: Vec::new(),
            cancel_signal_sent: false,
            commit_started: false,
            transitions: vec![editor_core::AiToolOperationTransition {
                state: editor_core::AiToolOperationState::AwaitingUser,
                stage: "awaiting_user".to_string(),
                at_epoch_ms: started_at_epoch_ms,
            }],
        };
        self.goal_mutation_operations.insert(
            operation_id,
            GoalMutationOperation {
                client_session_id: client_session_id.to_string(),
                request_id: Some(request.request_id),
                invocation,
                awaiting_snapshot: snapshot.clone(),
                kernel_started: false,
            },
        );
        if let Some(active) = self.sessions.get_mut(client_session_id) {
            active.operation_generation = active.operation_generation.saturating_add(1);
        }
        GatewayReplyPayload::Accepted(accepted_from_gateway_snapshot(&snapshot))
    }

    fn eligible_goal_mutation_grant(
        &self,
        client_session_id: &str,
        bound: &editor_core::BoundGoalMutation,
        goal_binding: &AiGoalBinding,
        risk_envelope: &AiRiskEnvelope,
    ) -> Option<AiCapabilityGrant> {
        let active_ref = self
            .sessions
            .get(client_session_id)?
            .active_mutation_grant_ref
            .as_ref()?;
        let registered = self.grants.get(active_ref)?;
        if registered.client_session_id != client_session_id
            || registered.revoked
            || registered.grant.validate_integrity().is_err()
        {
            return None;
        }
        let grant = &registered.grant;
        let approved_goal = grant.goal_binding.as_ref()?;
        let approved_risk = grant.risk_envelope.as_ref()?;
        if approved_goal.user_visible_outcome != goal_binding.user_visible_outcome
            || approved_goal.project_identity != bound.project_binding.project_id
            || approved_risk != risk_envelope
        {
            return None;
        }
        let lineage = self.kernel.grant_lineage(&grant.grant_digest);
        let current_digest = lineage
            .as_ref()
            .map(|lineage| lineage.current_project_digest.as_str())
            .unwrap_or(grant.initial_base_digest.as_str());
        let mutation_count = lineage.as_ref().map_or(0, |lineage| lineage.mutation_count);
        let consumed_time_ms = lineage
            .as_ref()
            .map_or(0, |lineage| lineage.consumed_time_ms);
        let consumed_external_cost_microunits = lineage
            .as_ref()
            .map_or(0, |lineage| lineage.consumed_external_cost_microunits);
        if current_digest != bound.project_binding.project_digest
            || mutation_count >= grant.max_mutation_count
            || consumed_time_ms >= grant.time_budget_ms
            || (grant.external_cost_budget_microunits > 0
                && consumed_external_cost_microunits >= grant.external_cost_budget_microunits)
        {
            return None;
        }
        Some(grant.clone())
    }

    fn revoke_ineligible_active_mutation(&mut self, client_session_id: &str) {
        let active_ref = self
            .sessions
            .get(client_session_id)
            .and_then(|active| active.active_mutation_grant_ref.clone());
        if let Some(active_ref) = active_ref {
            if let Some(registered) = self.grants.get_mut(&active_ref) {
                registered.revoked = true;
            }
            if let Some(active) = self.sessions.get_mut(client_session_id) {
                active.active_mutation_grant_ref = None;
                active.mutation_state = GatewayMutationAccessState::NotRequested;
                active.access_generation = active.access_generation.saturating_add(1);
            }
        }
    }

    fn availability_context_for_client(
        &self,
        session: &EditorSession,
        client_session_id: &str,
    ) -> Result<editor_core::AiToolAvailabilityContext, AiToolKernelError> {
        let active = self
            .sessions
            .get(client_session_id)
            .ok_or_else(|| AiToolKernelError {
                code: "ai_tool.catalog_session_unknown".to_string(),
                message: "Catalog requires an active Gateway session.".to_string(),
                next_action: "Reconnect through the current discovery record.".to_string(),
            })?;
        let mut context = self.kernel.availability_context(session);
        if let Some(project) = &active.binding.project_context {
            context.basis.project_identity = Some(project.project_identity.clone());
            context.basis.project_digest = active.observed_project_digest.clone();
            context.basis.read_generation = Some(active.read_generation);
        }
        context.basis.access_generation = Some(active.access_generation);
        context.basis.operation_generation = Some(active.operation_generation);
        context.read_state = if active.read_stale_reason.is_some() {
            AiToolReadAvailabilityState::Stale
        } else {
            AiToolReadAvailabilityState::Active
        };
        context.mutation_state = match active.mutation_state {
            GatewayMutationAccessState::NotRequested => {
                AiToolMutationAvailabilityState::NotRequested
            }
            GatewayMutationAccessState::AwaitingUser => {
                AiToolMutationAvailabilityState::AwaitingUser
            }
            GatewayMutationAccessState::Active => AiToolMutationAvailabilityState::Active,
            GatewayMutationAccessState::Revoked => AiToolMutationAvailabilityState::Revoked,
            GatewayMutationAccessState::Expired => AiToolMutationAvailabilityState::Expired,
        };
        context.rollback_lineage_known = context.rollback_lineage_known
            || self.operation_grants.values().any(|snapshot| {
                snapshot.client_session_id == client_session_id && snapshot.context_authority_active
            });
        Ok(context)
    }

    fn attach_latest_availability_basis(
        &self,
        session: &EditorSession,
        client_session_id: &str,
        payload: &mut GatewayReplyPayload,
    ) {
        let GatewayReplyPayload::Rejected(diagnostic) = payload else {
            return;
        };
        let Some(availability) = diagnostic.availability.as_mut() else {
            return;
        };
        if let Ok(context) = self.availability_context_for_client(session, client_session_id) {
            availability.basis = context.basis;
        }
    }

    fn diagnostic_for_invocation(
        &self,
        session: &EditorSession,
        client_session_id: &str,
        tool_id: &str,
        error: GatewayControlError,
    ) -> GatewayDiagnostic {
        let mut diagnostic = diagnostic(error);
        let availability = self
            .availability_context_for_client(session, client_session_id)
            .and_then(|context| {
                self.kernel
                    .catalog_for_session_with_context(AiToolCatalogRequest::v2(), context)
            })
            .ok()
            .and_then(|catalog| catalog.availability(tool_id).cloned())
            .filter(|availability| availability.state != AiToolAvailabilityState::Ready);
        if availability.is_some() {
            diagnostic.availability = availability;
        }
        diagnostic
    }

    pub fn pump_operations(&mut self, session: &mut EditorSession, max_steps: usize) -> usize {
        self.expire_goal_mutation_requests(now_epoch_ms());
        let client_session_ids = self.sessions.keys().cloned().collect::<Vec<_>>();
        for client_session_id in client_session_ids {
            let _ = self.reconcile_session_context(session, &client_session_id);
        }
        let processed = self.kernel.pump_operations(session, max_steps);
        self.sync_completed_operation_receipts();
        processed
    }

    fn start_operation(
        &mut self,
        session: &EditorSession,
        client_session_id: &str,
        invocation: AiToolInvocation,
        grant: AiCapabilityGrant,
    ) -> GatewayReplyPayload {
        let rollback_ref = self.pending_rollback_invocations.remove(&(
            client_session_id.to_string(),
            invocation.invocation_id.clone(),
        ));
        let availability = self
            .availability_context_for_client(session, client_session_id)
            .and_then(|context| {
                self.kernel
                    .catalog_for_session_with_context(AiToolCatalogRequest::v2(), context)
            })
            .ok()
            .and_then(|catalog| catalog.availability(&invocation.tool_id).cloned());
        if let Some(availability) = availability {
            if availability.state != AiToolAvailabilityState::Ready {
                return GatewayReplyPayload::Rejected(GatewayDiagnostic {
                    code: "ai_tool.availability_blocked".to_string(),
                    message: "Tool is not available under the latest engine-owned facts."
                        .to_string(),
                    next_action:
                        "Resolve the structured availability reasons and retry with fresh facts."
                            .to_string(),
                    availability: Some(availability),
                });
            }
        }
        match self.kernel.start(session, invocation, &grant) {
            editor_core::AiToolStartOutcome::Accepted(accepted) => {
                if let Some(rollback_ref) = rollback_ref {
                    self.rollback_operation_refs
                        .insert(accepted.operation_id.clone(), rollback_ref);
                }
                let inserted = match self.operation_grants.entry(accepted.operation_id.clone()) {
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        entry.insert(OperationGrantSnapshot {
                            client_session_id: client_session_id.to_string(),
                            grant,
                            context_authority_active: true,
                            detached_terminal: None,
                            terminal_observed: false,
                        });
                        true
                    }
                    std::collections::btree_map::Entry::Occupied(_) => false,
                };
                if inserted {
                    if let Some(active) = self.sessions.get_mut(client_session_id) {
                        active.operation_generation = active.operation_generation.saturating_add(1);
                    }
                }
                GatewayReplyPayload::Accepted(accepted)
            }
            editor_core::AiToolStartOutcome::Terminal(result) => {
                GatewayReplyPayload::ToolResult(result)
            }
        }
    }

    fn start_approved_goal_mutation(
        &mut self,
        session: &EditorSession,
        operation_id: &str,
        grant: AiCapabilityGrant,
    ) -> Result<(), GatewayControlError> {
        let (client_session_id, invocation) = self
            .goal_mutation_operations
            .get(operation_id)
            .map(|operation| {
                (
                    operation.client_session_id.clone(),
                    operation.invocation.clone(),
                )
            })
            .ok_or_else(|| {
                control_error(
                    "gateway.operation.pending_missing",
                    "Approved mutation no longer has its original pending operation.",
                    "Do not create another mutation; inspect the operation and project state.",
                )
            })?;
        let outcome = self.start_operation(session, &client_session_id, invocation, grant);
        match outcome {
            GatewayReplyPayload::Accepted(accepted) => {
                if accepted.operation_id != operation_id {
                    self.finish_awaiting_goal_mutation(
                        operation_id,
                        editor_core::AiToolOperationState::Failed,
                        "gateway.operation.identity_changed",
                        "Approval continuation produced a different operation identity.",
                    );
                    return Ok(());
                }
                if let Some(operation) = self.goal_mutation_operations.get_mut(operation_id) {
                    operation.kernel_started = true;
                    operation.request_id = None;
                }
            }
            GatewayReplyPayload::ToolResult(result) => {
                self.finish_goal_mutation_with_result(operation_id, result);
            }
            GatewayReplyPayload::Rejected(diagnostic) => {
                self.finish_awaiting_goal_mutation(
                    operation_id,
                    editor_core::AiToolOperationState::Failed,
                    &diagnostic.code,
                    &diagnostic.message,
                );
            }
            _ => {
                self.finish_awaiting_goal_mutation(
                    operation_id,
                    editor_core::AiToolOperationState::Failed,
                    "gateway.operation.continuation_invalid",
                    "Approval continuation returned an invalid Gateway outcome.",
                );
            }
        }
        Ok(())
    }

    fn goal_mutation_operation_snapshot(
        &self,
        client_session_id: &str,
        operation_id: &str,
    ) -> Option<Result<AiToolOperationSnapshot, GatewayControlError>> {
        let operation = self.goal_mutation_operations.get(operation_id)?;
        if operation.client_session_id != client_session_id {
            return Some(Err(control_error(
                "gateway.operation.session_mismatch",
                "Operation belongs to a different Gateway client session.",
                "Observe or cancel only operations started by this session.",
            )));
        }
        if !operation.kernel_started {
            return Some(Ok(operation.awaiting_snapshot.clone()));
        }
        Some(
            self.kernel
                .observe(operation_id)
                .map(|mut snapshot| {
                    if snapshot.transitions.first().is_none_or(|transition| {
                        transition.state != editor_core::AiToolOperationState::AwaitingUser
                    }) {
                        snapshot
                            .transitions
                            .insert(0, operation.awaiting_snapshot.transitions[0].clone());
                    }
                    snapshot.started_at_epoch_ms = operation.awaiting_snapshot.started_at_epoch_ms;
                    if let Some(result) = snapshot.result.as_mut() {
                        result.rollback_ref =
                            self.mutation_rollback_refs.get(operation_id).cloned();
                    }
                    snapshot
                })
                .map_err(kernel_control_error),
        )
    }

    fn cancel_awaiting_goal_mutation(
        &mut self,
        client_session_id: &str,
        operation_id: &str,
        diagnostic_code: &str,
    ) -> Result<editor_core::AiToolCancellationReceipt, GatewayControlError> {
        let operation = self
            .goal_mutation_operations
            .get(operation_id)
            .ok_or_else(|| {
                control_error(
                    "gateway.operation.snapshot_missing",
                    "Gateway has no pending goal mutation with this operation identity.",
                    "Use an operation id returned by this active Gateway session.",
                )
            })?;
        if operation.client_session_id != client_session_id {
            return Err(control_error(
                "gateway.operation.session_mismatch",
                "Operation belongs to a different Gateway client session.",
                "Cancel only operations started by this session.",
            ));
        }
        if let Some(request_id) = operation.request_id.clone() {
            self.access_requests.remove(&request_id);
        }
        self.finish_awaiting_goal_mutation(
            operation_id,
            editor_core::AiToolOperationState::Cancelled,
            diagnostic_code,
            "Goal mutation was cancelled before Native Editor approval.",
        );
        if let Some(active) = self.sessions.get_mut(client_session_id) {
            active.mutation_state = GatewayMutationAccessState::Revoked;
            active.access_generation = active.access_generation.saturating_add(1);
        }
        Ok(editor_core::AiToolCancellationReceipt {
            schema_version: editor_core::AI_TOOL_CANCELLATION_RECEIPT_SCHEMA_VERSION.to_string(),
            operation_id: operation_id.to_string(),
            grant_digest: String::new(),
            status: editor_core::AiToolCancellationStatus::Cancelled,
            cancelled_at_epoch_ms: now_epoch_ms(),
            diagnostic_code: diagnostic_code.to_string(),
            signal_sent: false,
            child_termination_observed: true,
            commit_started: false,
            terminal: true,
        })
    }

    fn finish_awaiting_goal_mutation(
        &mut self,
        operation_id: &str,
        state: editor_core::AiToolOperationState,
        diagnostic_code: &str,
        message: &str,
    ) {
        let Some(operation) = self.goal_mutation_operations.get_mut(operation_id) else {
            return;
        };
        let completed_at = now_epoch_ms();
        operation.request_id = None;
        operation.awaiting_snapshot.state = state;
        operation.awaiting_snapshot.stage = "terminal".to_string();
        operation.awaiting_snapshot.completed_at_epoch_ms = Some(completed_at);
        operation
            .awaiting_snapshot
            .transitions
            .push(editor_core::AiToolOperationTransition {
                state,
                stage: "terminal".to_string(),
                at_epoch_ms: completed_at,
            });
        operation.awaiting_snapshot.result = Some(gateway_terminal_result(
            &operation.awaiting_snapshot,
            diagnostic_code,
            message,
        ));
    }

    fn finish_goal_mutation_with_result(
        &mut self,
        operation_id: &str,
        result: editor_core::AiToolResult,
    ) {
        let Some(operation) = self.goal_mutation_operations.get_mut(operation_id) else {
            return;
        };
        let state = match result.status {
            editor_core::AiToolExecutionStatus::Completed => {
                editor_core::AiToolOperationState::Completed
            }
            editor_core::AiToolExecutionStatus::Failed => editor_core::AiToolOperationState::Failed,
        };
        let completed_at = now_epoch_ms();
        operation.request_id = None;
        operation.awaiting_snapshot.state = state;
        operation.awaiting_snapshot.stage = "terminal".to_string();
        operation.awaiting_snapshot.completed_at_epoch_ms = Some(completed_at);
        operation.awaiting_snapshot.result = Some(result);
        operation
            .awaiting_snapshot
            .transitions
            .push(editor_core::AiToolOperationTransition {
                state,
                stage: "terminal".to_string(),
                at_epoch_ms: completed_at,
            });
    }

    fn resolve_operation_snapshot(
        &self,
        client_session_id: &str,
        operation_id: &str,
    ) -> Result<OperationGrantSnapshot, GatewayControlError> {
        let snapshot = self
            .operation_grants
            .get(operation_id)
            .cloned()
            .ok_or_else(|| {
                control_error(
                    "gateway.operation.snapshot_missing",
                    "Gateway has no exact owner and grant snapshot for this operation.",
                    "Use an operation id returned by this active Gateway session.",
                )
            })?;
        if snapshot.client_session_id != client_session_id {
            return Err(control_error(
                "gateway.operation.session_mismatch",
                "Operation belongs to a different Gateway client session.",
                "Observe or cancel only operations started by this session.",
            ));
        }
        Ok(snapshot)
    }

    fn resolve_cancellable_operation_snapshot(
        &self,
        client_session_id: &str,
        operation_id: &str,
    ) -> Result<OperationGrantSnapshot, GatewayControlError> {
        let snapshot = self.resolve_operation_snapshot(client_session_id, operation_id)?;
        if !snapshot.context_authority_active {
            return Err(control_error(
                "gateway.operation.context_authority_invalidated",
                "Operation authority was invalidated by an Editor project context transition.",
                "Observe the operation terminal outcome; do not cancel it in the replacement context.",
            ));
        }
        Ok(snapshot)
    }

    fn resolve_session_bound_grant(
        &mut self,
        client_session_id: &str,
        invocation: &AiToolInvocation,
    ) -> Result<AiCapabilityGrant, GatewayControlError> {
        self.expire_active_mutation(client_session_id, now_epoch_ms());
        let active = self.sessions.get(client_session_id).ok_or_else(|| {
            control_error(
                "gateway.access.session_unknown",
                "Session-bound access requires an active Gateway client session.",
                "Reconnect through the current discovery record.",
            )
        })?;
        if active.read_stale_reason.is_some() {
            return Err(control_error(
                "gateway.access.read_stale",
                "Session Read generation is stale after an unobserved project change.",
                "Inspect the current project before dispatching more project tools.",
            ));
        }
        if let AiToolInvocationPayload::RollbackCandidate { receipt } = &invocation.payload {
            return self.resolve_rollback_grant(client_session_id, receipt);
        }
        match invocation_access_class(invocation) {
            SessionInvocationAccessClass::Read => {
                ensure_read_scope(active, "project")?;
                active.read_grant.clone().ok_or_else(|| {
                    control_error(
                        "gateway.context.project_required",
                        "Project-scoped read access requires an active Editor project context.",
                        "Open or create a project and retry on the same Gateway session.",
                    )
                })
            }
            SessionInvocationAccessClass::Mutation => {
                let grant_ref = active.active_mutation_grant_ref.as_deref().ok_or_else(|| {
                    let (code, message) = match active.mutation_state {
                        GatewayMutationAccessState::AwaitingUser => (
                            "gateway.access.mutation_awaiting_user",
                            "Mutation access is awaiting one Native Editor decision.",
                        ),
                        GatewayMutationAccessState::Expired => (
                            "gateway.access.mutation_expired",
                            "The session mutation grant expired.",
                        ),
                        _ => (
                            "gateway.access.mutation_inactive",
                            "The session has no active mutation grant.",
                        ),
                    };
                    control_error(
                        code,
                        message,
                        "Check session status and request a fresh Native Editor approval if needed.",
                    )
                })?;
                self.resolve_grant(client_session_id, grant_ref)
            }
        }
    }

    fn resolve_rollback_grant(
        &self,
        client_session_id: &str,
        receipt: &editor_core::ProjectCandidateApplyReceipt,
    ) -> Result<AiCapabilityGrant, GatewayControlError> {
        for (operation_id, snapshot) in &self.operation_grants {
            if snapshot.client_session_id != client_session_id || !snapshot.context_authority_active
            {
                continue;
            }
            let operation = snapshot
                .detached_terminal
                .clone()
                .or_else(|| self.kernel.observe(operation_id).ok());
            let Some(operation) = operation else {
                continue;
            };
            let Some(AiToolOutput::CandidateApplied(tool_receipt)) =
                operation.result.and_then(|result| result.output)
            else {
                continue;
            };
            if tool_receipt.candidate_receipt.receipt_binding_digest
                == receipt.receipt_binding_digest
                && tool_receipt.grant_digest == snapshot.grant.grant_digest
            {
                return Ok(snapshot.grant.clone());
            }
        }
        Err(control_error(
            "gateway.access.rollback_authority_missing",
            "Rollback receipt has no owning mutation operation in this Gateway session.",
            "Use the exact mutation receipt in its owning session; do not supply grant fields.",
        ))
    }

    fn require_read_scope(
        &self,
        client_session_id: &str,
        required_scope: &str,
    ) -> Result<(), GatewayControlError> {
        let active = self.sessions.get(client_session_id).ok_or_else(|| {
            control_error(
                "gateway.access.session_unknown",
                "Read access requires an active Gateway client session.",
                "Reconnect through the current discovery record.",
            )
        })?;
        ensure_read_scope(active, required_scope)
    }

    fn sync_completed_operation_receipts(&mut self) {
        let mut terminal = Vec::new();
        for (operation_id, snapshot) in &self.operation_grants {
            if snapshot.terminal_observed {
                continue;
            }
            let operation = snapshot
                .detached_terminal
                .clone()
                .or_else(|| self.kernel.observe(operation_id).ok());
            let Some(operation) = operation else {
                continue;
            };
            let Some(result) = operation.result.as_ref() else {
                continue;
            };
            let (observed_digest, applied_receipt, rollback_completed) =
                match result.output.as_ref() {
                    Some(AiToolOutput::CandidateApplied(receipt)) => (
                        Some(receipt.after_project_digest.clone()),
                        Some(receipt.clone()),
                        false,
                    ),
                    Some(AiToolOutput::CandidateRolledBack(receipt)) => {
                        (Some(receipt.restored_project_digest.clone()), None, true)
                    }
                    _ => (None, None, false),
                };
            terminal.push((
                operation_id.clone(),
                snapshot.client_session_id.clone(),
                snapshot.context_authority_active,
                observed_digest,
                applied_receipt,
                rollback_completed,
            ));
        }
        for (
            operation_id,
            client_session_id,
            context_authority_active,
            observed_digest,
            applied_receipt,
            rollback_completed,
        ) in terminal
        {
            if let Some(snapshot) = self.operation_grants.get_mut(&operation_id) {
                snapshot.terminal_observed = true;
            }
            if let Some(active) = self.sessions.get_mut(&client_session_id) {
                active.operation_generation = active.operation_generation.saturating_add(1);
            }
            if context_authority_active {
                if let Some(observed_digest) = observed_digest {
                    let _ = self.refresh_observed_project(&client_session_id, &observed_digest);
                }
            }
            if let Some(receipt) = applied_receipt {
                if let Some(active) = self.sessions.get(&client_session_id) {
                    let rollback_ref = opaque_rollback_ref(
                        &self.editor_instance_id,
                        &client_session_id,
                        &operation_id,
                        &receipt.receipt_digest,
                    );
                    self.mutation_rollback_refs
                        .insert(operation_id.clone(), rollback_ref.clone());
                    self.rollback_references.insert(
                        rollback_ref,
                        GatewayRollbackReference {
                            editor_instance_id: self.editor_instance_id.clone(),
                            client_session_id: client_session_id.clone(),
                            project_identity: receipt.project_identity.clone(),
                            expected_project_digest: receipt.after_project_digest.clone(),
                            expected_read_generation: active.read_generation,
                            expires_at_epoch_ms: active.binding.expires_at_epoch_ms,
                            mutation_receipt: receipt,
                            consumed: false,
                        },
                    );
                }
            }
            if rollback_completed {
                if let Some(rollback_ref) = self.rollback_operation_refs.get(&operation_id) {
                    if let Some(record) = self.rollback_references.get_mut(rollback_ref) {
                        record.consumed = true;
                    }
                }
            };
        }
    }

    fn refresh_observed_project(
        &mut self,
        client_session_id: &str,
        observed_project_digest: &str,
    ) -> Result<(), GatewayControlError> {
        {
            let active = self.sessions.get_mut(client_session_id).ok_or_else(|| {
                control_error(
                    "gateway.status.session_unknown",
                    "Cannot refresh a Read generation for an inactive Gateway session.",
                    "Reconnect through the current discovery record.",
                )
            })?;
            if active.observed_project_digest.as_deref() == Some(observed_project_digest)
                && active.read_stale_reason.is_none()
            {
                return Ok(());
            }
            active.read_generation = active.read_generation.saturating_add(1);
            let project_identity = active
                .binding
                .project_context
                .as_ref()
                .ok_or_else(|| {
                    control_error(
                        "gateway.context.project_required",
                        "Cannot refresh project facts without an active project context.",
                        "Open or create a project and retry.",
                    )
                })?
                .project_identity
                .clone();
            active.observed_project_digest = Some(observed_project_digest.to_string());
            if let Some(context) = active.binding.project_context.as_mut() {
                context.project_digest = observed_project_digest.to_string();
                context.read_generation = active.read_generation;
            }
            active.read_grant = Some(session_read_grant(
                client_session_id,
                &project_identity,
                observed_project_digest,
                active.read_generation,
            )?);
            active.read_stale_reason = None;
        }
        Ok(())
    }

    fn mark_session_read_stale(&mut self, client_session_id: &str) {
        let pending_goal_operations = self
            .goal_mutation_operations
            .iter()
            .filter(|(_, operation)| {
                operation.client_session_id == client_session_id && !operation.kernel_started
            })
            .map(|(operation_id, _)| operation_id.clone())
            .collect::<Vec<_>>();
        for operation_id in pending_goal_operations {
            self.finish_awaiting_goal_mutation(
                &operation_id,
                editor_core::AiToolOperationState::Failed,
                "gateway.operation.project_drifted",
                "Project facts changed while the goal mutation awaited approval.",
            );
        }
        let had_pending_request = self
            .access_requests
            .values()
            .any(|request| request.client_session_id == client_session_id);
        self.access_requests
            .retain(|_, request| request.client_session_id != client_session_id);
        let active_grant_ref = self
            .sessions
            .get(client_session_id)
            .and_then(|active| active.active_mutation_grant_ref.clone());
        if let Some(grant_ref) = &active_grant_ref {
            if let Some(grant) = self.grants.get_mut(grant_ref) {
                grant.revoked = true;
            }
        }
        if let Some(active) = self.sessions.get_mut(client_session_id) {
            active.read_stale_reason = Some("external_project_drift".to_string());
            let authority_changed = had_pending_request || active_grant_ref.is_some();
            if authority_changed {
                active.active_mutation_grant_ref = None;
                active.mutation_state = GatewayMutationAccessState::Revoked;
                active.access_generation = active.access_generation.saturating_add(1);
            }
        }
    }

    fn expire_active_mutation(&mut self, client_session_id: &str, now_epoch_ms: u64) {
        let active_ref = self
            .sessions
            .get(client_session_id)
            .and_then(|active| active.active_mutation_grant_ref.clone());
        let Some(active_ref) = active_ref else {
            return;
        };
        let expired = self.grants.get(&active_ref).is_none_or(|registered| {
            registered.revoked
                || registered
                    .grant
                    .expires_at_epoch_ms
                    .is_some_and(|expires| now_epoch_ms >= expires)
        });
        if expired {
            if let Some(registered) = self.grants.get_mut(&active_ref) {
                registered.revoked = true;
            }
            if let Some(active) = self.sessions.get_mut(client_session_id) {
                active.active_mutation_grant_ref = None;
                active.mutation_state = GatewayMutationAccessState::Expired;
                active.access_generation = active.access_generation.saturating_add(1);
            }
        }
    }

    fn expire_goal_mutation_requests(&mut self, at_epoch_ms: u64) {
        let expired = self
            .access_requests
            .values()
            .filter(|request| {
                request.operation_id.is_some() && at_epoch_ms >= request.expires_at_epoch_ms
            })
            .map(|request| {
                (
                    request.request_id.clone(),
                    request.client_session_id.clone(),
                    request.operation_id.clone().expect("filtered operation id"),
                )
            })
            .collect::<Vec<_>>();
        for (request_id, client_session_id, operation_id) in expired {
            self.access_requests.remove(&request_id);
            self.finish_awaiting_goal_mutation(
                &operation_id,
                editor_core::AiToolOperationState::Failed,
                "gateway.access.request_expired",
                "Goal mutation approval expired before a Native Editor decision.",
            );
            if let Some(active) = self.sessions.get_mut(&client_session_id) {
                active.mutation_state = GatewayMutationAccessState::Expired;
                active.access_generation = active.access_generation.saturating_add(1);
            }
        }
    }

    fn mutation_access_status(&self, active: &ActiveClientSession) -> GatewayMutationAccessStatus {
        let grant = active
            .active_mutation_grant_ref
            .as_ref()
            .and_then(|grant_ref| self.grants.get(grant_ref))
            .filter(|registered| !registered.revoked)
            .map(|registered| &registered.grant);
        let lineage = grant.and_then(|grant| self.kernel.grant_lineage(&grant.grant_digest));
        let requested_risk = grant
            .and_then(|grant| grant.risk_envelope.as_ref())
            .or_else(|| {
                self.access_requests
                    .values()
                    .find(|request| request.client_session_id == active.binding.client_session_id)
                    .map(|request| &request.risk_envelope)
            });
        GatewayMutationAccessStatus {
            state: if grant.is_some() {
                GatewayMutationAccessState::Active
            } else {
                active.mutation_state
            },
            requested_profile: requested_risk
                .map(|risk| match risk.risk_class {
                    AiGoalRiskClass::ProjectOwnedLowRisk => "project_owned_low_risk",
                    AiGoalRiskClass::Elevated => "elevated",
                    AiGoalRiskClass::ExactDomains => "exact_domains",
                })
                .unwrap_or("project_owned_low_risk")
                .to_string(),
            capabilities: mutation_capabilities(),
            blocked_capabilities: requested_risk
                .map(blocked_mutation_capabilities_for)
                .unwrap_or_else(blocked_mutation_capabilities),
            grant_digest: grant.map(|grant| grant.grant_digest.clone()),
            expires_at_epoch_ms: grant.and_then(|grant| grant.expires_at_epoch_ms),
            remaining_time_budget_ms: grant.map(|grant| {
                grant.time_budget_ms.saturating_sub(
                    lineage
                        .as_ref()
                        .map_or(0, |lineage| lineage.consumed_time_ms),
                )
            }),
            remaining_mutation_count: grant.map(|grant| {
                grant
                    .max_mutation_count
                    .saturating_sub(lineage.as_ref().map_or(0, |lineage| lineage.mutation_count))
            }),
        }
    }

    fn remove_session(&mut self, client_session_id: &str) -> Option<(usize, usize)> {
        let goal_operation_ids = self
            .goal_mutation_operations
            .iter()
            .filter(|(_, operation)| operation.client_session_id == client_session_id)
            .map(|(operation_id, _)| operation_id.clone())
            .collect::<Vec<_>>();
        for operation_id in goal_operation_ids {
            let kernel_started = self
                .goal_mutation_operations
                .get(&operation_id)
                .is_some_and(|operation| operation.kernel_started);
            if kernel_started {
                if let Some(snapshot) = self.kernel.invalidate_operation_authority(&operation_id) {
                    self.finish_goal_mutation_with_result(
                        &operation_id,
                        snapshot.result.clone().unwrap_or_else(|| {
                            gateway_terminal_result(
                                &snapshot,
                                "gateway.operation.disconnected",
                                "Gateway session disconnected before the mutation completed.",
                            )
                        }),
                    );
                }
            } else {
                self.finish_awaiting_goal_mutation(
                    &operation_id,
                    editor_core::AiToolOperationState::Interrupted,
                    "gateway.operation.disconnected",
                    "Gateway session disconnected while the goal mutation awaited approval.",
                );
            }
        }
        let session_operation_ids = self
            .operation_grants
            .iter()
            .filter(|(_, snapshot)| snapshot.client_session_id == client_session_id)
            .map(|(operation_id, _)| operation_id.clone())
            .collect::<Vec<_>>();
        for operation_id in session_operation_ids {
            let _ = self.kernel.invalidate_operation_authority(&operation_id);
        }
        self.sessions.remove(client_session_id)?;
        let grant_refs = self
            .grants
            .iter()
            .filter(|(_, grant)| grant.client_session_id == client_session_id)
            .map(|(grant_ref, _)| grant_ref.clone())
            .collect::<Vec<_>>();
        let grant_count = grant_refs.len();
        for grant_ref in grant_refs {
            self.grants.remove(&grant_ref);
        }
        let before = self.access_requests.len();
        self.access_requests
            .retain(|_, request| request.client_session_id != client_session_id);
        self.operation_grants
            .retain(|_, snapshot| snapshot.client_session_id != client_session_id);
        Some((grant_count, before - self.access_requests.len()))
    }

    fn resolve_grant(
        &self,
        client_session_id: &str,
        grant_ref: &str,
    ) -> Result<AiCapabilityGrant, GatewayControlError> {
        if grant_ref == GATEWAY_SESSION_READ_GRANT_REF {
            let active = self.sessions.get(client_session_id).ok_or_else(|| {
                control_error(
                    "gateway.grant_ref.session_unknown",
                    "Session Read grant belongs to an inactive Gateway session.",
                    "Reconnect through the current discovery record.",
                )
            })?;
            if active.read_stale_reason.is_some() {
                return Err(control_error(
                    "gateway.access.read_stale",
                    "Session Read generation is stale after an unobserved project change.",
                    "Inspect the current project before dispatching more project tools.",
                ));
            }
            return active.read_grant.clone().ok_or_else(|| {
                control_error(
                    "gateway.context.project_required",
                    "Session Read grant is unavailable without an active project context.",
                    "Open or create a project and retry on the same Gateway session.",
                )
            });
        }
        let selected_grant_ref =
            if grant_ref == "$active" || grant_ref == GATEWAY_ACTIVE_MUTATION_GRANT_REF {
                self.sessions
                    .get(client_session_id)
                    .and_then(|active| active.active_mutation_grant_ref.as_deref())
                    .ok_or_else(|| {
                        control_error(
                    "gateway.grant_ref.active_missing",
                    "No Native Editor-approved mutation grant is active for this client session.",
                    "Approve the connected Codex session in the Native Editor AI Panel.",
                )
                    })?
            } else {
                grant_ref
            };
        let registered = self.grants.get(selected_grant_ref).ok_or_else(|| {
            control_error(
                "gateway.grant_ref.unknown",
                "Opaque grant reference is unknown.",
                "Request a fresh Native Editor approval.",
            )
        })?;
        if registered.client_session_id != client_session_id {
            return Err(control_error(
                "gateway.grant_ref.session_mismatch",
                "Opaque grant reference belongs to a different client session.",
                "Use only grant references issued to the active connection.",
            ));
        }
        if registered.revoked {
            return Err(control_error(
                "gateway.grant_ref.revoked",
                "Opaque grant reference has been revoked.",
                "Request a fresh Native Editor approval.",
            ));
        }
        registered
            .grant
            .validate_integrity()
            .map_err(kernel_control_error)?;
        Ok(registered.grant.clone())
    }

    fn validate_request_binding(
        &mut self,
        session: &EditorSession,
        request: &GatewayRequest,
    ) -> Result<(), GatewayControlError> {
        self.reconcile_session_context(session, &request.client_session_id)?;
        let active = self
            .sessions
            .get(&request.client_session_id)
            .cloned()
            .ok_or_else(|| {
                control_error(
                    "gateway.binding.session_unknown",
                    "Gateway request does not reference an active client session.",
                    "Reconnect through the current discovery record.",
                )
            })?;
        let now = now_epoch_ms();
        if now >= active.binding.expires_at_epoch_ms {
            self.remove_session(&request.client_session_id);
            return Err(control_error(
                "gateway.binding.session_expired",
                "Gateway client session is closed or expired.",
                "Reconnect and obtain a fresh project binding.",
            ));
        }
        if request
            .deadline_epoch_ms
            .is_some_and(|deadline| now_epoch_ms() > deadline)
        {
            return Err(control_error(
                "gateway.request.deadline_exceeded",
                "Gateway request deadline elapsed before owner-thread dispatch.",
                "Reinspect current state and retry with a new request id.",
            ));
        }
        if let Some(active) = self.sessions.get_mut(&request.client_session_id) {
            active.last_seen_at_epoch_ms = now;
        }
        Ok(())
    }

    fn reconcile_session_context(
        &mut self,
        session: &EditorSession,
        client_session_id: &str,
    ) -> Result<(), GatewayControlError> {
        let active = self.sessions.get(client_session_id).ok_or_else(|| {
            control_error(
                "gateway.binding.session_unknown",
                "Gateway request does not reference an active client session.",
                "Reconnect through the current discovery record.",
            )
        })?;
        let current = current_project_binding_optional(session)?;
        let current_identity = current.as_ref().map(|project| {
            (
                project.project_id.as_str(),
                canonical_root_digest(&project.project_root),
                project.project_digest.as_str(),
            )
        });
        let unchanged = match (&active.binding.project_context, &current_identity) {
            (None, None) => true,
            (Some(previous), Some((project_identity, root_digest, project_digest))) => {
                previous.project_identity == *project_identity
                    && previous.canonical_project_root_digest == *root_digest
                    && previous.project_digest == *project_digest
            }
            _ => false,
        };
        if unchanged {
            return Ok(());
        }
        let same_project_newer_digest = match (&active.binding.project_context, &current_identity) {
            (Some(previous), Some((project_identity, root_digest, project_digest))) => {
                previous.project_identity == *project_identity
                    && previous.canonical_project_root_digest == *root_digest
                    && previous.project_digest != *project_digest
            }
            _ => false,
        };
        if same_project_newer_digest {
            self.mark_session_read_stale(client_session_id);
            if let (Some(active), Some(project)) =
                (self.sessions.get_mut(client_session_id), current.as_ref())
            {
                if let Some(context) = active.binding.project_context.as_mut() {
                    context.project_digest = project.project_digest.clone();
                }
            }
            return Ok(());
        }

        let next_generation = active.read_generation.saturating_add(1);
        let catalog_schema_version = active.binding.catalog_schema_version.clone();
        let previous_project_identity = active
            .binding
            .project_context
            .as_ref()
            .map(|context| context.project_identity.clone());
        let next_context = current.as_ref().map(|project| GatewayProjectContext {
            project_identity: project.project_id.clone(),
            canonical_project_root_digest: canonical_root_digest(&project.project_root),
            project_digest: project.project_digest.clone(),
            read_generation: next_generation,
        });
        let next_read_grant = next_context
            .as_ref()
            .map(|context| {
                session_read_grant(
                    client_session_id,
                    &context.project_identity,
                    &context.project_digest,
                    context.read_generation,
                )
            })
            .transpose()?;
        let catalog = self
            .kernel
            .catalog_for_session(
                session,
                AiToolCatalogRequest {
                    schema_version: catalog_schema_version,
                },
            )
            .map_err(kernel_control_error)?;
        let detached_operations = previous_project_identity
            .as_deref()
            .map(|project_identity| {
                self.kernel
                    .invalidate_project_context_operations(project_identity)
            })
            .unwrap_or_default();

        let grant_refs = self
            .grants
            .iter()
            .filter(|(_, grant)| grant.client_session_id == client_session_id)
            .map(|(grant_ref, _)| grant_ref.clone())
            .collect::<Vec<_>>();
        for grant_ref in grant_refs {
            self.grants.remove(&grant_ref);
        }
        let pending_goal_operation_ids = self
            .goal_mutation_operations
            .iter()
            .filter(|(_, operation)| {
                operation.client_session_id == client_session_id && !operation.kernel_started
            })
            .map(|(operation_id, _)| operation_id.clone())
            .collect::<Vec<_>>();
        for operation_id in pending_goal_operation_ids {
            self.finish_awaiting_goal_mutation(
                &operation_id,
                editor_core::AiToolOperationState::Failed,
                "gateway.operation.project_context_replaced",
                "Editor project context changed while the goal mutation awaited approval.",
            );
        }
        self.access_requests
            .retain(|_, request| request.client_session_id != client_session_id);
        for (operation_id, snapshot) in &mut self.operation_grants {
            if previous_project_identity
                .as_deref()
                .is_some_and(|project_identity| snapshot.grant.project_identity == project_identity)
            {
                snapshot.context_authority_active = false;
                if let Some(detached_terminal) = detached_operations.get(operation_id) {
                    snapshot.detached_terminal = Some(detached_terminal.clone());
                }
            }
        }

        let active = self
            .sessions
            .get_mut(client_session_id)
            .expect("Gateway session validated before context reconcile");
        active.binding.project_context = next_context.clone();
        active.binding.catalog_digest = catalog.catalog_digest();
        active.observed_project_digest = next_context
            .as_ref()
            .map(|context| context.project_digest.clone());
        active.read_generation = next_generation;
        active.read_grant = next_read_grant;
        active.read_stale_reason = None;
        active.active_mutation_grant_ref = None;
        active.mutation_state = GatewayMutationAccessState::NotRequested;
        active.access_generation = active.access_generation.saturating_add(1);
        active.operation_generation = active.operation_generation.saturating_add(1);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionInvocationAccessClass {
    Read,
    Mutation,
}

pub fn bind_session_owned_invocation_facts(
    session: &EditorSession,
    invocation: &mut AiToolInvocation,
) {
    let AiToolInvocationPayload::Candidate(input) = &mut invocation.payload else {
        return;
    };
    if matches!(
        input.envelope.payload,
        ProjectCandidatePayload::ProjectPatch(_)
    ) {
        input.envelope.project_patch_context_hash =
            Some(ProjectPatchLlmContextSnapshot::capture(session).context_hash);
    }
}

fn invocation_access_class(invocation: &AiToolInvocation) -> SessionInvocationAccessClass {
    match &invocation.payload {
        AiToolInvocationPayload::ProjectMutationIntent(_)
        | AiToolInvocationPayload::BoundGoalMutation(_)
        | AiToolInvocationPayload::Candidate(_)
        | AiToolInvocationPayload::RollbackCandidate { .. } => {
            SessionInvocationAccessClass::Mutation
        }
        _ => SessionInvocationAccessClass::Read,
    }
}

fn ensure_read_scope(
    active: &ActiveClientSession,
    required_scope: &str,
) -> Result<(), GatewayControlError> {
    if active
        .binding
        .effective_read_scope
        .iter()
        .any(|scope| scope == required_scope)
    {
        return Ok(());
    }
    Err(control_error(
        "gateway.access.read_scope_missing",
        format!("Gateway session does not include the required '{required_scope}' Read scope."),
        format!("Reconnect and request the '{required_scope}' Read scope."),
    ))
}

fn session_read_grant(
    client_session_id: &str,
    project_identity: &str,
    project_digest: &str,
    generation: u64,
) -> Result<AiCapabilityGrant, GatewayControlError> {
    AiCapabilityGrant::read(
        format!("gateway-session-read-{client_session_id}-{generation}"),
        project_identity,
        project_digest,
        format!("gateway-session:{client_session_id}"),
    )
    .map_err(kernel_control_error)
}

fn binding_project_context(
    binding: &ClientSessionBinding,
) -> Result<&GatewayProjectContext, GatewayControlError> {
    binding.project_context.as_ref().ok_or_else(|| {
        control_error(
            "gateway.context.project_required",
            "This operation requires an active Editor project context.",
            "Open or create a project and retry on the same Gateway session.",
        )
    })
}

fn mutation_access_request(
    binding: &ClientSessionBinding,
    client_kind: ClientKind,
    client_version: &str,
    requested_at_epoch_ms: u64,
    goal_binding: AiGoalBinding,
    risk_envelope: AiRiskEnvelope,
    operation_id: Option<String>,
) -> Result<GatewayAccessRequest, GatewayControlError> {
    let project = binding_project_context(binding)?;
    let request_id = format!(
        "gateway-access-{}",
        sha256_prefixed(
            format!(
                "{}|{}|{}|{}",
                binding.client_session_id,
                project.project_identity,
                project.project_digest,
                requested_at_epoch_ms
            )
            .as_bytes()
        )
        .trim_start_matches("sha256:")
        .chars()
        .take(32)
        .collect::<String>()
    );
    let mut request = GatewayAccessRequest {
        schema_version: GATEWAY_ACCESS_REQUEST_SCHEMA_VERSION.to_string(),
        request_id,
        operation_id,
        client_session_id: binding.client_session_id.clone(),
        client_kind,
        client_version: client_version.to_string(),
        project_identity: project.project_identity.clone(),
        observed_project_digest: project.project_digest.clone(),
        connected_at_epoch_ms: requested_at_epoch_ms,
        expires_at_epoch_ms: binding
            .expires_at_epoch_ms
            .min(requested_at_epoch_ms.saturating_add(risk_envelope.time_budget_ms)),
        requested_profile: match risk_envelope.risk_class {
            AiGoalRiskClass::ProjectOwnedLowRisk => "project_owned_low_risk",
            AiGoalRiskClass::Elevated => "elevated",
            AiGoalRiskClass::ExactDomains => "exact_domains",
        }
        .to_string(),
        capabilities: mutation_capabilities(),
        blocked_capabilities: blocked_mutation_capabilities_for(&risk_envelope),
        goal_binding,
        risk_envelope,
        approval_digest: String::new(),
    };
    request.approval_digest = access_request_digest(&request)?;
    Ok(request)
}

fn goal_mutation_authority(
    bound: &editor_core::BoundGoalMutation,
) -> Result<(AiGoalBinding, AiRiskEnvelope), GatewayControlError> {
    let goal_id = format!(
        "goal-{}",
        bound
            .goal_digest
            .trim_start_matches("sha256:")
            .chars()
            .take(32)
            .collect::<String>()
    );
    let goal = AiGoalBinding::new(
        goal_id,
        bound.normalized_goal_outcome.clone(),
        bound.project_binding.project_id.clone(),
        bound.project_binding.project_digest.clone(),
        AiGoalCompletionPolicy::CommitVerified,
    )
    .map_err(goal_grant_control_error)?;
    let elevated = bound.derived_risk_class == AiGoalRiskClass::Elevated;
    let risk = AiRiskEnvelope::new(AiRiskEnvelopeSpec {
        risk_class: bound.derived_risk_class,
        allowed_paths: Vec::new(),
        denied_paths: Vec::new(),
        allowed_objects: Vec::new(),
        max_mutation_count: 16,
        time_budget_ms: 900_000,
        external_cost_budget_microunits: 0,
        allow_delete: elevated,
        allow_dependency_change: false,
        allow_network: false,
    })
    .map_err(goal_grant_control_error)?;
    Ok((goal, risk))
}

fn gateway_invocation_digest(invocation: &AiToolInvocation) -> String {
    serde_json::to_value(invocation)
        .ok()
        .and_then(|value| canonical_json_bytes(&value).ok())
        .map(|bytes| sha256_prefixed(&bytes))
        .unwrap_or_else(|| sha256_prefixed(invocation.invocation_id.as_bytes()))
}

fn opaque_rollback_ref(
    editor_instance_id: &str,
    client_session_id: &str,
    operation_id: &str,
    receipt_digest: &str,
) -> String {
    let digest = sha256_prefixed(
        format!("{editor_instance_id}\n{client_session_id}\n{operation_id}\n{receipt_digest}")
            .as_bytes(),
    );
    format!("rbk_{}", digest.trim_start_matches("sha256:"))
}

fn mutation_receipt_digest(receipt: &editor_core::AiToolMutationReceipt) -> String {
    let mut canonical = receipt.clone();
    canonical.receipt_digest.clear();
    serde_json::to_value(canonical)
        .map_err(|error| error.to_string())
        .and_then(|value| canonical_json_bytes(&value).map_err(|error| error.to_string()))
        .map(|bytes| sha256_prefixed(&bytes))
        .unwrap_or_default()
}

fn accepted_from_gateway_snapshot(
    snapshot: &AiToolOperationSnapshot,
) -> editor_core::AiToolAccepted {
    editor_core::AiToolAccepted {
        schema_version: editor_core::AI_TOOL_ACCEPTED_SCHEMA_VERSION.to_string(),
        operation_id: snapshot.operation_id.clone(),
        invocation_id: snapshot.invocation_id.clone(),
        tool_id: snapshot.tool_id.clone(),
        project_identity: snapshot.project_identity.clone(),
        state: snapshot.state,
        accepted_at_epoch_ms: snapshot.started_at_epoch_ms,
    }
}

fn gateway_terminal_result(
    snapshot: &AiToolOperationSnapshot,
    diagnostic_code: &str,
    message: &str,
) -> editor_core::AiToolResult {
    editor_core::AiToolResult {
        schema_version: editor_core::AI_TOOL_RESULT_SCHEMA_VERSION.to_string(),
        status: editor_core::AiToolExecutionStatus::Failed,
        tool_id: snapshot.tool_id.clone(),
        tool_version: editor_core::AI_TOOL_IMPLEMENTATION_VERSION_V1.to_string(),
        operation_id: snapshot.operation_id.clone(),
        project_identity: Some(snapshot.project_identity.clone()),
        facts: BTreeMap::new(),
        diagnostics: vec![editor_core::AiToolDiagnostic {
            severity: editor_core::AiToolDiagnosticSeverity::Error,
            code: diagnostic_code.to_string(),
            message: message.to_string(),
            next_action: "Inspect the terminal operation and submit a new explicit mutation only if the user still wants it.".to_string(),
        }],
        suggested_next_actions: Vec::new(),
        changed_domains: Vec::new(),
        output: None,
        rollback_ref: None,
        evidence_refs: Vec::new(),
        duration_ms: now_epoch_ms().saturating_sub(snapshot.started_at_epoch_ms),
        external_cost_microunits: 0,
    }
}

fn access_request_digest(request: &GatewayAccessRequest) -> Result<String, GatewayControlError> {
    let mut value = serde_json::to_value(request).map_err(|source| {
        control_error(
            "gateway.access.request_digest_failed",
            format!("Failed to serialize goal approval request: {source}"),
            "Discard the request and create a fresh goal approval request.",
        )
    })?;
    value
        .as_object_mut()
        .expect("GatewayAccessRequest serializes as an object")
        .insert(
            "approvalDigest".to_string(),
            serde_json::Value::String(String::new()),
        );
    canonical_json_bytes(&value)
        .map(|bytes| sha256_prefixed(&bytes))
        .map_err(|source| {
            control_error(
                "gateway.access.request_digest_failed",
                source.to_string(),
                "Discard the request and create a fresh goal approval request.",
            )
        })
}

fn validate_access_request_digest(
    request: &GatewayAccessRequest,
) -> Result<(), GatewayControlError> {
    request
        .goal_binding
        .validate_integrity()
        .map_err(goal_grant_control_error)?;
    request
        .risk_envelope
        .validate_integrity()
        .map_err(goal_grant_control_error)?;
    if access_request_digest(request)? != request.approval_digest {
        return Err(control_error(
            "gateway.access.request_digest_mismatch",
            "Goal approval request content no longer matches its digest.",
            "Reject the changed request and create a fresh goal approval request.",
        ));
    }
    Ok(())
}

fn mutation_capabilities() -> Vec<String> {
    [
        "project_patch",
        "controlled_source_patch",
        "asset_import",
        "rollback",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn blocked_mutation_capabilities() -> Vec<String> {
    [
        "delete",
        "dependency_change",
        "network",
        "external_cost",
        "engine_core_write",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn blocked_mutation_capabilities_for(risk: &AiRiskEnvelope) -> Vec<String> {
    let mut blocked = vec!["engine_core_write".to_string()];
    if !risk.allow_delete {
        blocked.push("delete".to_string());
    }
    if !risk.allow_dependency_change {
        blocked.push("dependency_change".to_string());
    }
    if !risk.allow_network {
        blocked.push("network".to_string());
    }
    if risk.external_cost_budget_microunits == 0 {
        blocked.push("external_cost".to_string());
    }
    blocked.sort();
    blocked
}

pub(crate) enum GatewayOwnerThreadCommand {
    Connect {
        hello: ClientHello,
        reply: Sender<Result<ClientSessionBinding, GatewayControlError>>,
    },
    Dispatch {
        request: GatewayRequest,
        reply: Sender<GatewayReply>,
    },
    Close {
        client_session_id: String,
        reply: Sender<CloseReceipt>,
    },
}

#[derive(Clone)]
pub struct GatewayOwnerThreadClient {
    sender: Sender<GatewayOwnerThreadCommand>,
    wake: Option<GatewayOwnerThreadWake>,
}

pub type GatewayOwnerThreadWake = Arc<dyn Fn() + Send + Sync + 'static>;

pub struct GatewayOwnerThreadDispatcher {
    receiver: Receiver<GatewayOwnerThreadCommand>,
    max_pending_per_pump: usize,
}

pub fn gateway_owner_thread_channel() -> (GatewayOwnerThreadClient, GatewayOwnerThreadDispatcher) {
    gateway_owner_thread_channel_inner(None)
}

pub fn gateway_owner_thread_channel_with_wake(
    wake: GatewayOwnerThreadWake,
) -> (GatewayOwnerThreadClient, GatewayOwnerThreadDispatcher) {
    gateway_owner_thread_channel_inner(Some(wake))
}

fn gateway_owner_thread_channel_inner(
    wake: Option<GatewayOwnerThreadWake>,
) -> (GatewayOwnerThreadClient, GatewayOwnerThreadDispatcher) {
    let (sender, receiver) = mpsc::channel();
    (
        GatewayOwnerThreadClient { sender, wake },
        GatewayOwnerThreadDispatcher {
            receiver,
            max_pending_per_pump: DEFAULT_MAX_PENDING_REQUESTS,
        },
    )
}

impl GatewayOwnerThreadClient {
    pub fn submit_connect(
        &self,
        hello: ClientHello,
    ) -> Result<Receiver<Result<ClientSessionBinding, GatewayControlError>>, GatewayControlError>
    {
        let (reply, receiver) = mpsc::channel();
        self.sender
            .send(GatewayOwnerThreadCommand::Connect { hello, reply })
            .map_err(|_| dispatcher_closed())?;
        self.wake_owner_thread();
        Ok(receiver)
    }

    pub fn submit_dispatch(
        &self,
        request: GatewayRequest,
    ) -> Result<Receiver<GatewayReply>, GatewayControlError> {
        let (reply, receiver) = mpsc::channel();
        self.sender
            .send(GatewayOwnerThreadCommand::Dispatch { request, reply })
            .map_err(|_| dispatcher_closed())?;
        self.wake_owner_thread();
        Ok(receiver)
    }

    pub fn submit_close(
        &self,
        client_session_id: impl Into<String>,
    ) -> Result<Receiver<CloseReceipt>, GatewayControlError> {
        let (reply, receiver) = mpsc::channel();
        self.sender
            .send(GatewayOwnerThreadCommand::Close {
                client_session_id: client_session_id.into(),
                reply,
            })
            .map_err(|_| dispatcher_closed())?;
        self.wake_owner_thread();
        Ok(receiver)
    }

    fn wake_owner_thread(&self) {
        if let Some(wake) = &self.wake {
            wake();
        }
    }
}

impl GatewayOwnerThreadDispatcher {
    pub fn pump(&mut self, core: &mut GatewayCore, session: &mut EditorSession) -> usize {
        let mut processed = 0;
        while processed < self.max_pending_per_pump {
            let Ok(command) = self.receiver.try_recv() else {
                break;
            };
            match command {
                GatewayOwnerThreadCommand::Connect { hello, reply } => {
                    let _ = reply.send(core.connect(session, hello));
                }
                GatewayOwnerThreadCommand::Dispatch { request, reply } => {
                    let _ = reply.send(core.dispatch(session, request));
                }
                GatewayOwnerThreadCommand::Close {
                    client_session_id,
                    reply,
                } => {
                    let _ = reply.send(core.close(&client_session_id));
                }
            }
            processed += 1;
        }
        processed
    }
}

fn current_project_binding(
    session: &EditorSession,
) -> Result<editor_core::ProjectCandidateProjectBinding, GatewayControlError> {
    ProjectCandidateEntry::inspect_project_binding(session)
        .map_err(|error| control_error(error.code, error.message, error.next_action))
}

fn current_project_binding_optional(
    session: &EditorSession,
) -> Result<Option<editor_core::ProjectCandidateProjectBinding>, GatewayControlError> {
    match ProjectCandidateEntry::inspect_project_binding(session) {
        Ok(binding) => Ok(Some(binding)),
        Err(error) if error.code == "project_candidate_entry.no_active_project" => Ok(None),
        Err(error) => Err(control_error(error.code, error.message, error.next_action)),
    }
}

pub fn canonical_root_digest(canonical_project_root: &str) -> String {
    let normalized = canonical_project_root.replace('\\', "/").to_lowercase();
    let normalized = if let Some(unc_path) = normalized.strip_prefix("//?/unc/") {
        format!("//{unc_path}")
    } else {
        normalized
            .strip_prefix("//?/")
            .unwrap_or(&normalized)
            .to_string()
    };
    sha256_prefixed(normalized.as_bytes())
}

fn kernel_control_error(error: AiToolKernelError) -> GatewayControlError {
    control_error(error.code, error.message, error.next_action)
}

fn goal_grant_control_error(error: editor_core::AiGoalGrantError) -> GatewayControlError {
    control_error(
        error.code,
        error.message,
        "Inspect the current project and create a fresh goal-bound approval request.",
    )
}

fn diagnostic(error: GatewayControlError) -> GatewayDiagnostic {
    let availability = availability_for_gateway_error(&error);
    GatewayDiagnostic {
        code: error.code,
        message: error.message,
        next_action: error.next_action,
        availability,
    }
}

fn availability_for_gateway_error(error: &GatewayControlError) -> Option<AiToolAvailability> {
    let authorization = error.code.starts_with("gateway.access.mutation_")
        || error.code.starts_with("gateway.grant_ref.")
        || error.code == "gateway.access.rollback_authority_missing";
    let blocked = error.code == "gateway.access.read_stale"
        || error.code.starts_with("gateway.binding.")
        || error.code.starts_with("gateway.operation.")
        || error.code.starts_with("gateway.status.reconnect_")
        || error.code.starts_with("gateway.access.request_stale");
    if !authorization && !blocked {
        return None;
    }
    let (state, category, resolution_kind, owner) = if authorization {
        (
            AiToolAvailabilityState::AuthorizationRequired,
            AiToolAvailabilityReasonCategory::Authorization,
            AiToolAvailabilityResolutionKind::RequestAuthorization,
            AiToolAvailabilityOwner::GatewayAuthority,
        )
    } else {
        (
            AiToolAvailabilityState::Blocked,
            AiToolAvailabilityReasonCategory::SessionFreshness,
            AiToolAvailabilityResolutionKind::RefreshSessionFacts,
            AiToolAvailabilityOwner::GatewayAuthority,
        )
    };
    Some(AiToolAvailability {
        state,
        reasons: vec![AiToolAvailabilityReason {
            code: error.code.clone(),
            category,
            message: error.message.clone(),
            resolution_kind,
            owner,
        }],
        basis: AiToolAvailabilityBasis::default(),
        input_dependent_checks_remain: true,
    })
}

fn control_error(
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

fn dispatcher_closed() -> GatewayControlError {
    control_error(
        "gateway.dispatcher.closed",
        "Editor owner-thread dispatcher is no longer available.",
        "Reopen the Editor and rediscover its Gateway endpoint.",
    )
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ClientKind, GatewayCacheState, GatewayPerformanceContractReport,
        GatewayPerformanceStageSample, GatewayRequestPayload, GATEWAY_CLIENT_HELLO_SCHEMA_VERSION,
        GATEWAY_REQUEST_SCHEMA_VERSION,
    };
    use editor_core::{
        command_for_test, AiCapabilityGrant, AiToolCatalogRequest, AiToolExecutionStatus,
        AiToolInvocation, AiToolInvocationPayload, AiToolOutput, CommandStatus,
        ExternalProjectMutationChange, ExternalProjectMutationGoal, ExternalProjectMutationIntent,
        ExternalProjectRollbackInput, InputPatchOperation, PatchOperation, PatchSource,
        ProjectCreateDirectInput, ProjectPatchDocument, ProjectSearchInput,
        AI_TOOL_CATALOG_SCHEMA_VERSION, AI_TOOL_INVOCATION_SCHEMA_VERSION,
        EXTERNAL_PROJECT_MUTATION_INTENT_SCHEMA_VERSION, EXTERNAL_PROJECT_ROLLBACK_SCHEMA_VERSION,
        PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION, TOOL_ID_PROJECT_CREATE, TOOL_ID_PROJECT_MUTATE,
        TOOL_ID_PROJECT_ROLLBACK, TOOL_ID_PROJECT_SEARCH,
    };
    use editor_ui_model::{InputActionValueKind, UiCommandPayload};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn created_session(name: &str) -> (EditorSession, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "ai-tool-gateway-{name}-{}-{}",
            std::process::id(),
            SESSION_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut session = EditorSession::new();
        let result = session.execute_command(command_for_test(UiCommandPayload::CreateProject {
            path: root.display().to_string(),
            name: name.to_string(),
        }));
        assert_eq!(result.status, CommandStatus::Committed);
        (session, root)
    }

    fn project_create_invocation(
        invocation_id: &str,
        root: &std::path::Path,
        project_name: &str,
    ) -> AiToolInvocation {
        AiToolInvocation {
            schema_version: AI_TOOL_INVOCATION_SCHEMA_VERSION.to_string(),
            invocation_id: invocation_id.to_string(),
            tool_id: TOOL_ID_PROJECT_CREATE.to_string(),
            expected_project_digest: String::new(),
            payload: AiToolInvocationPayload::ProjectCreate(ProjectCreateDirectInput {
                requested_project_root: root.display().to_string(),
                project_name: project_name.to_string(),
            }),
        }
    }

    #[test]
    fn project_create_runs_from_launcher_without_project_grant_and_replays_exactly() {
        let mut session = EditorSession::new();
        let mut core = GatewayCore::new();
        let client_hello = hello(&session);
        let binding = core.connect(&mut session, client_hello).unwrap();
        let root = std::env::temp_dir().join(format!(
            "gateway-project-create-{}-{}",
            std::process::id(),
            SESSION_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let invocation = project_create_invocation("create-once", &root, "CreatedByTool");
        let request = session_bound_request(
            &binding,
            "create-request",
            GatewayRequestPayload::ExecuteSessionBound {
                invocation: invocation.clone(),
            },
        );

        let first = core.dispatch(&mut session, request);
        let GatewayReplyPayload::ToolResult(first_result) = first.payload else {
            panic!("project.create must return a typed result");
        };
        assert_eq!(first_result.status, AiToolExecutionStatus::Completed);
        let first_receipt_id = first_result.facts["receiptId"].clone();
        assert_eq!(first_result.facts["replayed"], "false");
        assert_eq!(first_result.facts["readGeneration"], "2");
        assert_eq!(first_result.facts["openedInEditor"], "true");
        assert!(session.active_project_session().is_some());

        let replay = core.dispatch(
            &mut session,
            session_bound_request(
                &binding,
                "create-replay",
                GatewayRequestPayload::ExecuteSessionBound { invocation },
            ),
        );
        let GatewayReplyPayload::ToolResult(replay_result) = replay.payload else {
            panic!("exact replay must return the typed result");
        };
        assert_eq!(replay_result.facts["receiptId"], first_receipt_id);
        assert_eq!(replay_result.facts["replayed"], "true");

        let changed_target = root.parent().unwrap().join(format!(
            "gateway-project-create-changed-{}",
            std::process::id()
        ));
        let changed_replay = core.dispatch(
            &mut session,
            session_bound_request(
                &binding,
                "create-changed-replay",
                GatewayRequestPayload::ExecuteSessionBound {
                    invocation: project_create_invocation(
                        "create-once",
                        &changed_target,
                        "ChangedInput",
                    ),
                },
            ),
        );
        let GatewayReplyPayload::ToolResult(changed_result) = changed_replay.payload else {
            panic!("changed replay must return typed failure");
        };
        assert_eq!(changed_result.status, AiToolExecutionStatus::Failed);
        assert!(changed_result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ai_tool.invocation_replay_mismatch"));
        assert!(!changed_target.exists());
    }

    #[test]
    fn project_create_existing_target_is_zero_write_typed_failure() {
        let mut session = EditorSession::new();
        let mut core = GatewayCore::new();
        let client_hello = hello(&session);
        let binding = core.connect(&mut session, client_hello).unwrap();
        let target = std::env::temp_dir().join(format!(
            "gateway-project-create-existing-{}-{}",
            std::process::id(),
            SESSION_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&target).unwrap();
        let sentinel = target.join("caller-owned.txt");
        std::fs::write(&sentinel, b"unchanged").unwrap();

        let reply = core.dispatch(
            &mut session,
            session_bound_request(
                &binding,
                "existing-target-create",
                GatewayRequestPayload::ExecuteSessionBound {
                    invocation: project_create_invocation(
                        "existing-target-create",
                        &target,
                        "ExistingTarget",
                    ),
                },
            ),
        );

        let GatewayReplyPayload::ToolResult(result) = reply.payload else {
            panic!("target_exists must return typed failure");
        };
        assert_eq!(result.status, AiToolExecutionStatus::Failed);
        assert!(result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "project_create.target_exists"));
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"unchanged");
        assert!(!target.join("project.aife.json").exists());
    }

    #[test]
    fn project_create_is_blocked_in_project_context_before_target_write() {
        let (mut session, existing_root) = created_session("create-blocked-context");
        let mut core = GatewayCore::new();
        let client_hello = hello(&session);
        let binding = core.connect(&mut session, client_hello).unwrap();
        let target = existing_root
            .parent()
            .unwrap()
            .join(format!("blocked-target-{}", std::process::id()));

        let reply = core.dispatch(
            &mut session,
            session_bound_request(
                &binding,
                "blocked-create",
                GatewayRequestPayload::ExecuteSessionBound {
                    invocation: project_create_invocation(
                        "blocked-create",
                        &target,
                        "MustNotCreate",
                    ),
                },
            ),
        );

        let GatewayReplyPayload::ToolResult(result) = reply.payload else {
            panic!("blocked project.create must return typed failure");
        };
        assert_eq!(result.status, AiToolExecutionStatus::Failed);
        assert!(result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "project_create.launcher_required"));
        assert!(!target.exists());
    }

    fn hello(_session: &EditorSession) -> ClientHello {
        ClientHello {
            schema_version: GATEWAY_CLIENT_HELLO_SCHEMA_VERSION.to_string(),
            gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
            client_kind: ClientKind::Test,
            client_version: "test-adapter.v1".to_string(),
            supported_schema_versions: vec![AI_TOOL_CATALOG_SCHEMA_VERSION.to_string()],
            expected_editor_instance_id: crate::default_editor_instance_id(),
            requested_read_scope: vec![
                "catalog".to_string(),
                "project".to_string(),
                "not-allowed".to_string(),
            ],
        }
    }

    fn candidate_invocation(
        session: &EditorSession,
        invocation_id: &str,
        action_id: &str,
    ) -> AiToolInvocation {
        let mapping_path = format!("Input/{invocation_id}.input.json");
        let create_operation_id = format!("create-{invocation_id}");
        let patch = ProjectPatchDocument::new(
            format!("patch-{invocation_id}"),
            format!("Add {action_id}"),
            PatchSource::Test,
            vec![
                PatchOperation::Input(InputPatchOperation::CreateDefaultInputMapping {
                    operation_id: create_operation_id.clone(),
                    depends_on: Vec::new(),
                    path: mapping_path.clone(),
                }),
                PatchOperation::Input(InputPatchOperation::AddInputAction {
                    operation_id: format!("operation-{invocation_id}"),
                    depends_on: vec![create_operation_id],
                    path: mapping_path,
                    action_id: action_id.to_string(),
                    value_type: InputActionValueKind::Button,
                }),
            ],
        );
        let binding = ProjectCandidateEntry::inspect_project_binding(session).unwrap();
        AiToolInvocation {
            schema_version: AI_TOOL_INVOCATION_SCHEMA_VERSION.to_string(),
            invocation_id: invocation_id.to_string(),
            tool_id: TOOL_ID_PROJECT_MUTATE.to_string(),
            expected_project_digest: binding.project_digest,
            payload: AiToolInvocationPayload::ProjectMutationIntent(
                ExternalProjectMutationIntent {
                    schema_version: EXTERNAL_PROJECT_MUTATION_INTENT_SCHEMA_VERSION.to_string(),
                    goal: ExternalProjectMutationGoal {
                        outcome: "Apply the test project change and leave the project valid."
                            .to_string(),
                    },
                    change: ExternalProjectMutationChange::ProjectPatch(patch),
                },
            ),
        }
    }

    fn goal_mutation_invocation(
        session: &EditorSession,
        invocation_id: &str,
        action_id: &str,
    ) -> AiToolInvocation {
        let mapping_path = format!("Input/{invocation_id}.input.json");
        let create_operation_id = format!("create-{invocation_id}");
        let patch = ProjectPatchDocument::new(
            format!("patch-{invocation_id}"),
            format!("Add {action_id}"),
            PatchSource::AiAssistant,
            vec![
                PatchOperation::Input(InputPatchOperation::CreateDefaultInputMapping {
                    operation_id: create_operation_id.clone(),
                    depends_on: Vec::new(),
                    path: mapping_path.clone(),
                }),
                PatchOperation::Input(InputPatchOperation::AddInputAction {
                    operation_id: format!("operation-{invocation_id}"),
                    depends_on: vec![create_operation_id],
                    path: mapping_path,
                    action_id: action_id.to_string(),
                    value_type: InputActionValueKind::Button,
                }),
            ],
        );
        AiToolInvocation {
            schema_version: AI_TOOL_INVOCATION_SCHEMA_VERSION.to_string(),
            invocation_id: invocation_id.to_string(),
            tool_id: TOOL_ID_PROJECT_MUTATE.to_string(),
            expected_project_digest: ProjectCandidateEntry::inspect_project_binding(session)
                .unwrap()
                .project_digest,
            payload: AiToolInvocationPayload::ProjectMutationIntent(
                ExternalProjectMutationIntent {
                    schema_version: EXTERNAL_PROJECT_MUTATION_INTENT_SCHEMA_VERSION.to_string(),
                    goal: ExternalProjectMutationGoal {
                        outcome: format!("Add input action {action_id}."),
                    },
                    change: ExternalProjectMutationChange::ProjectPatch(patch),
                },
            ),
        }
    }

    fn completed_goal_mutation(
        label: &str,
    ) -> (
        GatewayCore,
        EditorSession,
        ClientSessionBinding,
        PathBuf,
        String,
    ) {
        let (mut session, root) = created_session(label);
        let mut core = GatewayCore::new();
        let session_hello = hello(&session);
        let binding = core.connect(&mut session, session_hello).unwrap();
        let invocation = goal_mutation_invocation(
            &session,
            &format!("{label}-mutation"),
            &format!("action.{label}"),
        );
        let reply = core.dispatch(
            &mut session,
            session_bound_request(
                &binding,
                &format!("{label}-request"),
                GatewayRequestPayload::ExecuteSessionBound { invocation },
            ),
        );
        let GatewayReplyPayload::Accepted(accepted) = reply.payload else {
            panic!("project.mutate must await one Native Editor approval");
        };
        let approval = core
            .approval_inbox(now_epoch_ms())
            .into_iter()
            .find(|request| request.operation_id.as_deref() == Some(&accepted.operation_id))
            .expect("same-operation approval request");
        core.decide_access(
            &session,
            &approval.request_id,
            GatewayAccessDecision::Approve,
            "native-editor-user",
            now_epoch_ms(),
        )
        .unwrap();
        assert!(core.pump_operations(&mut session, 8) > 0);
        let rollback_ref = core
            .mutation_rollback_refs
            .get(&accepted.operation_id)
            .cloned()
            .expect("completed mutation rollbackRef");
        (core, session, binding, root, rollback_ref)
    }

    fn rollback_ref_request(
        session: &EditorSession,
        binding: &ClientSessionBinding,
        request_id: &str,
        rollback_ref: &str,
    ) -> GatewayRequest {
        session_bound_request(
            binding,
            request_id,
            GatewayRequestPayload::ExecuteSessionBound {
                invocation: AiToolInvocation {
                    schema_version: AI_TOOL_INVOCATION_SCHEMA_VERSION.to_string(),
                    invocation_id: request_id.to_string(),
                    tool_id: TOOL_ID_PROJECT_ROLLBACK.to_string(),
                    expected_project_digest: ProjectCandidateEntry::inspect_project_binding(
                        session,
                    )
                    .unwrap()
                    .project_digest,
                    payload: AiToolInvocationPayload::ProjectRollbackRef(
                        ExternalProjectRollbackInput {
                            schema_version: EXTERNAL_PROJECT_ROLLBACK_SCHEMA_VERSION.to_string(),
                            rollback_ref: rollback_ref.to_string(),
                        },
                    ),
                },
            },
        )
    }

    fn rejected_code(reply: GatewayReply) -> String {
        let GatewayReplyPayload::Rejected(diagnostic) = reply.payload else {
            panic!("rollbackRef owner check must reject before Kernel execution");
        };
        diagnostic.code
    }

    fn execute_request(
        binding: &ClientSessionBinding,
        request_id: &str,
        invocation: AiToolInvocation,
        grant_ref: &str,
    ) -> GatewayRequest {
        GatewayRequest {
            schema_version: GATEWAY_REQUEST_SCHEMA_VERSION.to_string(),
            gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
            request_id: request_id.to_string(),
            client_session_id: binding.client_session_id.clone(),
            deadline_epoch_ms: None,
            response_limit_bytes: 1024 * 1024,
            payload: GatewayRequestPayload::Execute {
                invocation,
                grant_ref: grant_ref.to_string(),
            },
        }
    }

    fn search_invocation(session: &EditorSession, invocation_id: &str) -> AiToolInvocation {
        AiToolInvocation {
            schema_version: AI_TOOL_INVOCATION_SCHEMA_VERSION.to_string(),
            invocation_id: invocation_id.to_string(),
            tool_id: TOOL_ID_PROJECT_SEARCH.to_string(),
            expected_project_digest: ProjectCandidateEntry::inspect_project_binding(session)
                .unwrap()
                .project_digest,
            payload: AiToolInvocationPayload::ProjectSearch(ProjectSearchInput {
                schema_version: PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION.to_string(),
                query: "project".to_string(),
                kinds: Vec::new(),
                continuation_token: None,
                page_size: 10,
            }),
        }
    }

    fn session_bound_request(
        binding: &ClientSessionBinding,
        request_id: &str,
        payload: GatewayRequestPayload,
    ) -> GatewayRequest {
        GatewayRequest {
            schema_version: GATEWAY_REQUEST_SCHEMA_VERSION.to_string(),
            gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
            request_id: request_id.to_string(),
            client_session_id: binding.client_session_id.clone(),
            deadline_epoch_ms: None,
            response_limit_bytes: 1024 * 1024,
            payload,
        }
    }

    fn approval_request_id(
        core: &mut GatewayCore,
        session: &EditorSession,
        client_session_id: &str,
    ) -> String {
        let project = ProjectCandidateEntry::inspect_project_binding(session).unwrap();
        let goal = AiGoalBinding::new(
            format!("test-goal-{client_session_id}"),
            "Apply the test project change and leave the project valid.",
            project.project_id,
            project.project_digest,
            editor_core::AiGoalCompletionPolicy::CommitVerified,
        )
        .unwrap();
        core.request_goal_mutation_access(
            session,
            client_session_id,
            goal,
            AiRiskEnvelope::default_project_owned_low_risk().unwrap(),
        )
        .unwrap();
        core.approval_inbox(now_epoch_ms())
            .into_iter()
            .find(|request| request.client_session_id == client_session_id)
            .expect("session approval request")
            .request_id
    }

    #[test]
    fn gateway_session_status_connects_with_read_active_and_mutation_not_requested() {
        let (mut session, root) = created_session("session-status");
        let mut core = GatewayCore::new();
        let session_hello = hello(&session);
        let binding = core.connect(&mut session, session_hello).unwrap();

        let reply = core.dispatch(
            &mut session,
            session_bound_request(
                &binding,
                "session-status",
                GatewayRequestPayload::SessionStatus,
            ),
        );
        let GatewayReplyPayload::SessionStatus(status) = reply.payload else {
            panic!("session status reply expected");
        };
        assert_eq!(status.session.id, binding.client_session_id);
        assert_eq!(status.session.state, GatewaySessionState::Active);
        assert_eq!(status.access.read.state, GatewayReadAccessState::Active);
        assert_eq!(status.access.read.generation, 1);
        assert_eq!(
            status.access.mutation.state,
            GatewayMutationAccessState::NotRequested
        );
        assert!(!status.reconnect_required);

        let inbox = core.approval_inbox(now_epoch_ms());
        assert!(inbox.is_empty());

        let request_id = approval_request_id(&mut core, &session, &binding.client_session_id);
        let request = core
            .approval_inbox(now_epoch_ms())
            .into_iter()
            .find(|request| request.request_id == request_id)
            .unwrap();
        assert_eq!(request.client_version, "test-adapter.v1");
        assert_eq!(
            request.goal_binding.project_identity,
            binding
                .project_context
                .as_ref()
                .expect("project context")
                .project_identity
        );
        assert_eq!(request.risk_envelope.max_mutation_count, 16);
        assert!(!request.approval_digest.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gateway_catalog_authority_states_and_generations_are_session_owned() {
        let (mut session, root) = created_session("catalog-authority-generations");
        let mut core = GatewayCore::new();
        let session_hello = hello(&session);
        let binding = core.connect(&mut session, session_hello).unwrap();
        let status = core
            .session_status(&session, &binding.client_session_id, now_epoch_ms())
            .unwrap();
        let value = serde_json::to_value(status).unwrap();

        assert_eq!(value["access"]["read"]["generation"], 1);
        assert_eq!(value["access"]["accessGeneration"], 1);
        assert_eq!(value["operationGeneration"], 0);

        let _request_id = approval_request_id(&mut core, &session, &binding.client_session_id);
        let awaiting = core.dispatch(
            &mut session,
            session_bound_request(
                &binding,
                "catalog-authority-awaiting",
                GatewayRequestPayload::Catalog(AiToolCatalogRequest::v2()),
            ),
        );
        let GatewayReplyPayload::Catalog(awaiting) = awaiting.payload else {
            panic!("awaiting Catalog expected");
        };
        let mutation = awaiting
            .availability(TOOL_ID_PROJECT_MUTATE)
            .expect("candidate availability");
        assert_eq!(
            mutation.state,
            AiToolAvailabilityState::AuthorizationRequired
        );
        assert!(mutation.reasons.iter().any(|reason| {
            reason.code == "ai_tool.availability.await_user_decision"
                && reason.resolution_kind == AiToolAvailabilityResolutionKind::AwaitUserDecision
        }));
        assert_eq!(mutation.basis.access_generation, Some(2));
        assert_eq!(core.approval_inbox(now_epoch_ms()).len(), 1);

        std::fs::write(root.join("catalog-authority-drift.txt"), "drift").unwrap();
        let stale = core
            .session_status(&session, &binding.client_session_id, now_epoch_ms())
            .unwrap();
        assert_eq!(stale.access.read.state, GatewayReadAccessState::Stale);
        assert_eq!(
            stale.access.mutation.state,
            GatewayMutationAccessState::Revoked
        );
        assert_eq!(stale.access.access_generation, 3);
        assert!(core.approval_inbox(now_epoch_ms()).is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gateway_catalog_schema_negotiation_selects_v2_and_preserves_v1_window() {
        let (mut session, root) = created_session("catalog-schema-negotiation");
        let mut core = GatewayCore::new();
        let mut session_hello = hello(&session);
        session_hello.supported_schema_versions = vec![
            "ai-tool-catalog.v1".to_string(),
            "ai-tool-catalog.v2".to_string(),
        ];
        let binding = core.connect(&mut session, session_hello).unwrap();
        let value = serde_json::to_value(binding).unwrap();

        assert_eq!(value["catalogSchemaVersion"], "ai-tool-catalog.v2");

        let mut v1_hello = hello(&session);
        v1_hello.supported_schema_versions = vec![AI_TOOL_CATALOG_V1_SCHEMA_VERSION.to_string()];
        let v1_binding = core.connect(&mut session, v1_hello).unwrap();
        assert_eq!(
            v1_binding.catalog_schema_version,
            AI_TOOL_CATALOG_V1_SCHEMA_VERSION
        );
        let v1_reply = core.dispatch(
            &mut session,
            session_bound_request(
                &v1_binding,
                "catalog-schema-v1",
                GatewayRequestPayload::Catalog(AiToolCatalogRequest::v1()),
            ),
        );
        let GatewayReplyPayload::Catalog(v1_catalog) = v1_reply.payload else {
            panic!("v1-only client must receive a Catalog");
        };
        let v1_value = serde_json::to_value(v1_catalog).unwrap();
        assert_eq!(v1_value.as_object().unwrap().len(), 2);
        assert!(v1_value.get("availabilityDigest").is_none());

        let mut unsupported = hello(&session);
        unsupported.supported_schema_versions = vec!["ai-tool-catalog.v0".to_string()];
        let error = core.connect(&mut session, unsupported).unwrap_err();
        assert_eq!(error.code, "gateway.binding.schema_negotiation_failed");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gateway_catalog_no_side_effects() {
        let (mut session, root) = created_session("catalog-no-side-effects");
        let mut core = GatewayCore::new();
        let session_hello = hello(&session);
        let binding = core.connect(&mut session, session_hello).unwrap();
        let request_id = approval_request_id(&mut core, &session, &binding.client_session_id);
        let project_before = current_project_binding(&session).unwrap();
        let counts_before = (
            core.access_requests.len(),
            core.grants.len(),
            core.operation_grants.len(),
        );
        let status_before = core
            .session_status(&session, &binding.client_session_id, now_epoch_ms())
            .unwrap();

        let first = core.dispatch(
            &mut session,
            session_bound_request(
                &binding,
                "catalog-no-side-effects-1",
                GatewayRequestPayload::Catalog(AiToolCatalogRequest::v2()),
            ),
        );
        let second = core.dispatch(
            &mut session,
            session_bound_request(
                &binding,
                "catalog-no-side-effects-2",
                GatewayRequestPayload::Catalog(AiToolCatalogRequest::v2()),
            ),
        );
        let project_after = current_project_binding(&session).unwrap();
        let status_after = core
            .session_status(&session, &binding.client_session_id, now_epoch_ms())
            .unwrap();

        let first_value = serde_json::to_value(first.payload).unwrap();
        let second_value = serde_json::to_value(second.payload).unwrap();
        assert_eq!(first_value, second_value);
        assert_eq!(project_before.project_digest, project_after.project_digest);
        assert_eq!(
            counts_before,
            (
                core.access_requests.len(),
                core.grants.len(),
                core.operation_grants.len(),
            )
        );
        assert_eq!(
            status_before.access.access_generation,
            status_after.access.access_generation
        );
        assert_eq!(
            status_before.operation_generation,
            status_after.operation_generation
        );
        assert_eq!(core.approval_inbox(now_epoch_ms()).len(), 1);
        assert_eq!(
            core.approval_inbox(now_epoch_ms())[0].request_id,
            request_id
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gateway_rollback_availability_does_not_require_active_mutation_grant() {
        let (mut session, root) = created_session("rollback-catalog-authority");
        let mut core = GatewayCore::new();
        let session_hello = hello(&session);
        let binding = core.connect(&mut session, session_hello).unwrap();
        let reply = core.dispatch(
            &mut session,
            session_bound_request(
                &binding,
                "rollback-catalog-authority",
                GatewayRequestPayload::Catalog(AiToolCatalogRequest {
                    schema_version: "ai-tool-catalog.v2".to_string(),
                }),
            ),
        );
        let GatewayReplyPayload::Catalog(catalog) = reply.payload else {
            panic!("v2 catalog expected");
        };
        let value = serde_json::to_value(catalog).unwrap();
        let rollback = value["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["descriptor"]["toolId"] == TOOL_ID_PROJECT_ROLLBACK)
            .expect("rollback entry");

        assert_ne!(rollback["availability"]["state"], "authorization_required");
        assert_eq!(rollback["availability"]["inputDependentChecksRemain"], true);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gateway_execute_rechecks_catalog_availability_before_side_effects() {
        let (mut session, root) = created_session("catalog-execute-recheck");
        let mut core = GatewayCore::new();
        let session_hello = hello(&session);
        let binding = core.connect(&mut session, session_hello).unwrap();
        std::fs::write(root.join("external-catalog-drift.txt"), "drift").unwrap();
        let invocation = search_invocation(&session, "catalog-execute-recheck");

        let reply = core.dispatch(
            &mut session,
            session_bound_request(
                &binding,
                "catalog-execute-recheck",
                GatewayRequestPayload::ExecuteSessionBound { invocation },
            ),
        );
        let value = serde_json::to_value(reply).unwrap();
        assert_eq!(
            value["payload"]["reply"]["availability"]["state"],
            "blocked"
        );
        assert_eq!(core.operation_grants.len(), 0);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gateway_session_bound_project_patch_binds_current_context_before_start() {
        let (mut session, root) = created_session("session-owned-project-patch-context");
        let mut core = GatewayCore::new();
        let session_hello = hello(&session);
        let binding = core.connect(&mut session, session_hello).unwrap();
        let request_id = approval_request_id(&mut core, &session, &binding.client_session_id);
        core.decide_access(
            &session,
            &request_id,
            GatewayAccessDecision::Approve,
            "native-editor-user",
            now_epoch_ms(),
        )
        .unwrap();

        let invocation = candidate_invocation(
            &session,
            "session-owned-project-patch-context",
            "action.session-owned-context",
        );
        let accepted = core.dispatch(
            &mut session,
            session_bound_request(
                &binding,
                "session-owned-project-patch-context",
                GatewayRequestPayload::ExecuteSessionBound { invocation },
            ),
        );
        let GatewayReplyPayload::Accepted(accepted) = accepted.payload else {
            panic!("Gateway must bind the current ProjectPatch context before kernel start");
        };
        core.pump_operations(&mut session, 6);
        let operation = core.kernel.observe(&accepted.operation_id).unwrap();
        assert_eq!(
            operation.state,
            editor_core::AiToolOperationState::Completed
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn goal_mutation_contract_executes_bound_intent_and_advances_session_facts() {
        let (mut session, root) = created_session("goal-mutation-contract-execute");
        let mut core = GatewayCore::new();
        let session_hello = hello(&session);
        let binding = core.connect(&mut session, session_hello).unwrap();
        let before = ProjectCandidateEntry::inspect_project_binding(&session)
            .unwrap()
            .project_digest;
        let invocation = goal_mutation_invocation(
            &session,
            "goal-mutation-contract-execute",
            "action.goal-mutation",
        );

        let reply = core.dispatch(
            &mut session,
            session_bound_request(
                &binding,
                "goal-mutation-contract-execute",
                GatewayRequestPayload::ExecuteSessionBound { invocation },
            ),
        );
        let GatewayReplyPayload::Accepted(accepted) = reply.payload else {
            panic!("project.mutate must start one bound operation");
        };
        assert_eq!(
            accepted.state,
            editor_core::AiToolOperationState::AwaitingUser
        );
        let access_request = core
            .approval_inbox(now_epoch_ms())
            .into_iter()
            .find(|request| request.operation_id.as_deref() == Some(&accepted.operation_id))
            .expect("same-operation Native Editor approval request");
        core.decide_access(
            &session,
            &access_request.request_id,
            GatewayAccessDecision::Approve,
            "native-editor-user",
            now_epoch_ms(),
        )
        .unwrap();
        assert!(core.pump_operations(&mut session, 6) > 0);
        let operation = core.kernel.observe(&accepted.operation_id).unwrap();
        assert_eq!(
            operation.state,
            editor_core::AiToolOperationState::Completed
        );
        let result = operation.result.expect("terminal result");
        assert_eq!(result.tool_id, TOOL_ID_PROJECT_MUTATE);
        let Some(AiToolOutput::CandidateApplied(receipt)) = result.output else {
            panic!("project.mutate must return the existing trusted Candidate receipt");
        };
        assert_eq!(receipt.tool_id, TOOL_ID_PROJECT_MUTATE);
        assert_eq!(receipt.before_project_digest, before);
        assert_ne!(receipt.after_project_digest, before);
        let active = core.sessions.get(&binding.client_session_id).unwrap();
        assert!(active.read_generation > 1);
        assert_eq!(
            active.observed_project_digest.as_deref(),
            Some(receipt.after_project_digest.as_str())
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rollback_ref_owner_rejects_expiry_receipt_tamper_and_project_mismatch() {
        for (label, expected_code, mutate) in [
            ("rollback-ref-expired", "gateway.rollback_ref.expired", 0_u8),
            (
                "rollback-ref-receipt-tampered",
                "gateway.rollback_ref.receipt_mismatch",
                1_u8,
            ),
            (
                "rollback-ref-project-mismatch",
                "gateway.rollback_ref.project_mismatch",
                2_u8,
            ),
        ] {
            let (mut core, mut session, binding, root, rollback_ref) =
                completed_goal_mutation(label);
            let record = core.rollback_references.get_mut(&rollback_ref).unwrap();
            match mutate {
                0 => record.expires_at_epoch_ms = now_epoch_ms(),
                1 => record.mutation_receipt.receipt_id.push_str("-tampered"),
                2 => record.project_identity.push_str("-other-project"),
                _ => unreachable!(),
            }
            let request = rollback_ref_request(
                &session,
                &binding,
                &format!("{label}-rollback"),
                &rollback_ref,
            );
            let reply = core.dispatch(&mut session, request);
            assert_eq!(rejected_code(reply), expected_code);
            let _ = std::fs::remove_dir_all(root);
        }
    }

    #[test]
    fn rollback_ref_kernel_rejects_tampered_internal_rollback_material() {
        let (mut core, mut session, binding, root, rollback_ref) =
            completed_goal_mutation("rollback-material-tampered");
        let record = core.rollback_references.get_mut(&rollback_ref).unwrap();
        record
            .mutation_receipt
            .candidate_receipt
            .candidate_id
            .push_str("-missing");
        record.mutation_receipt.receipt_digest = mutation_receipt_digest(&record.mutation_receipt);

        let request = rollback_ref_request(
            &session,
            &binding,
            "rollback-material-tampered-rollback",
            &rollback_ref,
        );
        let reply = core.dispatch(&mut session, request);
        let GatewayReplyPayload::Accepted(accepted) = reply.payload else {
            panic!("Gateway must pass the opaque ref to the Kernel owner");
        };
        assert!(core.pump_operations(&mut session, 8) > 0);
        let operation = core.kernel.observe(&accepted.operation_id).unwrap();
        let result = operation.result.expect("terminal rollback result");
        assert_eq!(result.status, AiToolExecutionStatus::Failed);
        assert!(result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "project_candidate_entry.receipt_tampered"));
        assert!(
            !core
                .rollback_references
                .get(&rollback_ref)
                .unwrap()
                .consumed
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gateway_expired_forward_grant_preserves_receipt_bound_rollback_authority() {
        let (mut session, root) = created_session("expired-grant-rollback");
        let initial_digest = ProjectCandidateEntry::inspect_project_binding(&session)
            .unwrap()
            .project_digest;
        let mut core = GatewayCore::new();
        let session_hello = hello(&session);
        let binding = core.connect(&mut session, session_hello).unwrap();
        let request_id = approval_request_id(&mut core, &session, &binding.client_session_id);
        let approval = core
            .decide_access(
                &session,
                &request_id,
                GatewayAccessDecision::Approve,
                "native-editor-user",
                now_epoch_ms(),
            )
            .unwrap();
        let grant_ref = approval.grant_ref.unwrap();

        let invocation = candidate_invocation(
            &session,
            "expired-grant-mutation",
            "action.expired-grant-rollback",
        );
        let accepted = core.dispatch(
            &mut session,
            session_bound_request(
                &binding,
                "expired-grant-mutation",
                GatewayRequestPayload::ExecuteSessionBound { invocation },
            ),
        );
        let GatewayReplyPayload::Accepted(accepted) = accepted.payload else {
            panic!("mutation must start before grant expiry");
        };
        core.pump_operations(&mut session, 6);
        let rollback_ref = core
            .mutation_rollback_refs
            .get(&accepted.operation_id)
            .cloned()
            .expect("completed mutation rollbackRef");

        core.renew_grant_ref(&grant_ref, now_epoch_ms() + 20)
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(30));
        let status = core
            .session_status(&session, &binding.client_session_id, now_epoch_ms())
            .unwrap();
        assert_eq!(
            status.access.mutation.state,
            GatewayMutationAccessState::Expired
        );

        let rollback_request =
            rollback_ref_request(&session, &binding, "expired-grant-rollback", &rollback_ref);
        let rollback = core.dispatch(&mut session, rollback_request);
        let GatewayReplyPayload::Accepted(rollback) = rollback.payload else {
            panic!("receipt-bound rollback authority must survive forward grant expiry");
        };
        core.pump_operations(&mut session, 6);
        assert_eq!(
            core.kernel.observe(&rollback.operation_id).unwrap().state,
            editor_core::AiToolOperationState::Completed
        );
        assert_eq!(
            ProjectCandidateEntry::inspect_project_binding(&session)
                .unwrap()
                .project_digest,
            initial_digest
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gateway_effective_read_scope_is_enforced_without_legacy_execute_bypass() {
        let (mut session, root) = created_session("read-scope-enforcement");
        let mut core = GatewayCore::new();

        let mut catalog_hello = hello(&session);
        catalog_hello.requested_read_scope = vec![
            "catalog".to_string(),
            "scene".to_string(),
            "unknown-scope".to_string(),
        ];
        let catalog_binding = core.connect(&mut session, catalog_hello).unwrap();
        assert_eq!(catalog_binding.effective_read_scope, vec!["catalog"]);

        let status = core.dispatch(
            &mut session,
            session_bound_request(
                &catalog_binding,
                "catalog-only-status",
                GatewayRequestPayload::SessionStatus,
            ),
        );
        assert!(matches!(
            status.payload,
            GatewayReplyPayload::SessionStatus(_)
        ));
        let catalog = core.dispatch(
            &mut session,
            session_bound_request(
                &catalog_binding,
                "catalog-only-catalog",
                GatewayRequestPayload::Catalog(AiToolCatalogRequest::default()),
            ),
        );
        assert!(matches!(catalog.payload, GatewayReplyPayload::Catalog(_)));

        let inspect = core.dispatch(
            &mut session,
            session_bound_request(
                &catalog_binding,
                "catalog-only-inspect",
                GatewayRequestPayload::Inspect(AiToolInspectRequest::project()),
            ),
        );
        assert!(matches!(
            inspect.payload,
            GatewayReplyPayload::Rejected(GatewayDiagnostic { ref code, .. })
                if code == "gateway.access.read_scope_missing"
        ));

        let catalog_session_bound_invocation =
            search_invocation(&session, "catalog-only-session-bound-read");
        let session_bound_read = core.dispatch(
            &mut session,
            session_bound_request(
                &catalog_binding,
                "catalog-only-session-bound-read",
                GatewayRequestPayload::ExecuteSessionBound {
                    invocation: catalog_session_bound_invocation,
                },
            ),
        );
        assert!(matches!(
            session_bound_read.payload,
            GatewayReplyPayload::Rejected(GatewayDiagnostic { ref code, .. })
                if code == "gateway.access.read_scope_missing"
        ));

        let catalog_legacy_invocation = search_invocation(&session, "catalog-only-legacy-read");
        let legacy_read = core.dispatch(
            &mut session,
            execute_request(
                &catalog_binding,
                "catalog-only-legacy-read",
                catalog_legacy_invocation,
                GATEWAY_SESSION_READ_GRANT_REF,
            ),
        );
        assert!(matches!(
            legacy_read.payload,
            GatewayReplyPayload::Rejected(GatewayDiagnostic { ref code, .. })
                if code == "gateway.access.read_scope_missing"
        ));

        let mut project_hello = hello(&session);
        project_hello.requested_read_scope = vec![
            "project".to_string(),
            "evidence".to_string(),
            "unknown-scope".to_string(),
        ];
        let project_binding = core.connect(&mut session, project_hello).unwrap();
        assert_eq!(project_binding.effective_read_scope, vec!["project"]);
        let project_catalog = core.dispatch(
            &mut session,
            session_bound_request(
                &project_binding,
                "project-only-catalog",
                GatewayRequestPayload::Catalog(AiToolCatalogRequest::default()),
            ),
        );
        assert!(matches!(
            project_catalog.payload,
            GatewayReplyPayload::Rejected(GatewayDiagnostic { ref code, .. })
                if code == "gateway.access.read_scope_missing"
        ));
        let project_inspect = core.dispatch(
            &mut session,
            session_bound_request(
                &project_binding,
                "project-only-inspect",
                GatewayRequestPayload::Inspect(AiToolInspectRequest::project()),
            ),
        );
        assert!(matches!(
            project_inspect.payload,
            GatewayReplyPayload::Inspection(_)
        ));
        let project_read_invocation = search_invocation(&session, "project-only-read");
        let project_read = core.dispatch(
            &mut session,
            session_bound_request(
                &project_binding,
                "project-only-read",
                GatewayRequestPayload::ExecuteSessionBound {
                    invocation: project_read_invocation,
                },
            ),
        );
        assert!(matches!(
            project_read.payload,
            GatewayReplyPayload::Accepted(_)
        ));

        let mut empty_hello = hello(&session);
        empty_hello.requested_read_scope = vec!["scene".to_string(), "unknown-scope".to_string()];
        let empty_binding = core.connect(&mut session, empty_hello).unwrap();
        assert!(empty_binding.effective_read_scope.is_empty());
        let empty_status = core.dispatch(
            &mut session,
            session_bound_request(
                &empty_binding,
                "empty-scope-status",
                GatewayRequestPayload::SessionStatus,
            ),
        );
        assert!(matches!(
            empty_status.payload,
            GatewayReplyPayload::SessionStatus(_)
        ));
        let empty_catalog = core.dispatch(
            &mut session,
            session_bound_request(
                &empty_binding,
                "empty-scope-catalog",
                GatewayRequestPayload::Catalog(AiToolCatalogRequest::default()),
            ),
        );
        assert!(matches!(
            empty_catalog.payload,
            GatewayReplyPayload::Rejected(GatewayDiagnostic { ref code, .. })
                if code == "gateway.access.read_scope_missing"
        ));

        let _ = std::fs::remove_dir_all(root);
    }
    #[test]
    fn gateway_launcher_session_reconciles_project_context_after_create() {
        let root = std::env::temp_dir().join(format!(
            "ai-tool-gateway-launcher-{}-{}",
            std::process::id(),
            SESSION_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut session = EditorSession::new();
        let mut core = GatewayCore::new();
        let session_hello = hello(&session);
        let binding = core.connect(&mut session, session_hello).unwrap();

        assert!(binding.project_context.is_none());
        let launcher_status = core
            .session_status(&session, &binding.client_session_id, now_epoch_ms())
            .unwrap();
        assert_eq!(launcher_status.session.id, binding.client_session_id);
        assert!(launcher_status.project.is_none());
        assert_eq!(
            launcher_status.access.read.state,
            GatewayReadAccessState::Unavailable
        );

        let create = session.execute_command(command_for_test(UiCommandPayload::CreateProject {
            path: root.display().to_string(),
            name: "launcher-context".to_string(),
        }));
        assert_eq!(create.status, CommandStatus::Committed);

        let project_status = core
            .session_status(&session, &binding.client_session_id, now_epoch_ms())
            .unwrap();
        assert_eq!(project_status.session.id, binding.client_session_id);
        assert!(project_status.project.is_some());
        assert_eq!(
            project_status.access.read.state,
            GatewayReadAccessState::Active
        );
        assert_eq!(project_status.access.read.generation, 2);
        assert_eq!(core.active_client_bindings().len(), 1);
        assert!(core.active_client_bindings()[0].project_context.is_some());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gateway_context_marks_newer_project_digest_stale_without_reconnect() {
        let (mut session, root) = created_session("session-drift");
        let mut core = GatewayCore::new();
        let session_hello = hello(&session);
        let binding = core.connect(&mut session, session_hello).unwrap();
        std::fs::write(root.join("external-drift.txt"), b"external change").unwrap();

        let stale = core
            .session_status(&session, &binding.client_session_id, now_epoch_ms())
            .unwrap();
        assert_eq!(stale.access.read.state, GatewayReadAccessState::Stale);
        assert_eq!(stale.access.read.generation, 1);
        assert_ne!(
            stale.project.as_ref().unwrap().current_digest,
            stale.project.as_ref().unwrap().observed_digest
        );
        assert_eq!(stale.session.id, binding.client_session_id);

        let inspection = core.dispatch(
            &mut session,
            session_bound_request(
                &binding,
                "refresh-current-project",
                GatewayRequestPayload::Inspect(AiToolInspectRequest::project()),
            ),
        );
        assert!(matches!(
            inspection.payload,
            GatewayReplyPayload::Inspection(_)
        ));
        let refreshed = core
            .session_status(&session, &binding.client_session_id, now_epoch_ms())
            .unwrap();
        assert_eq!(refreshed.access.read.state, GatewayReadAccessState::Active);
        assert_eq!(refreshed.access.read.generation, 2);
        assert_eq!(
            refreshed.project.as_ref().unwrap().current_digest,
            refreshed.project.as_ref().unwrap().observed_digest
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gateway_approval_lifecycle_rotates_mutation_and_cancel_uses_operation_snapshot() {
        let (mut session, root) = created_session("approval-lifecycle");
        let mut core = GatewayCore::new();
        let session_hello = hello(&session);
        let binding = core.connect(&mut session, session_hello).unwrap();
        let request_id = approval_request_id(&mut core, &session, &binding.client_session_id);
        let first_decision = core
            .decide_access(
                &session,
                &request_id,
                GatewayAccessDecision::Approve,
                "native-editor-user",
                now_epoch_ms(),
            )
            .unwrap();
        let first_grant_ref = first_decision
            .grant_ref
            .clone()
            .expect("mutation grant ref");

        let mutation_invocation = candidate_invocation(
            &session,
            "operation-owned-cancel",
            "action.operation_owned_cancel",
        );
        let execute = core.dispatch(
            &mut session,
            session_bound_request(
                &binding,
                "operation-owned-cancel",
                GatewayRequestPayload::ExecuteSessionBound {
                    invocation: mutation_invocation,
                },
            ),
        );
        let GatewayReplyPayload::Accepted(accepted) = execute.payload else {
            panic!("mutation operation should be accepted");
        };

        let project = ProjectCandidateEntry::inspect_project_binding(&session).unwrap();
        let replacement = AiCapabilityGrant::project_owned_low_risk(
            "replacement-mutation",
            project.project_id,
            sha256_prefixed(b"replacement-visible-outcome"),
            project.project_digest,
            "native-editor-user",
        )
        .unwrap();
        let replacement = core
            .issue_grant_ref(&session, &binding.client_session_id, replacement)
            .unwrap();
        assert_ne!(replacement.grant_ref, first_grant_ref);
        assert_eq!(
            core.resolve_grant(&binding.client_session_id, &first_grant_ref)
                .unwrap_err()
                .code,
            "gateway.grant_ref.revoked"
        );

        let cancel = core.dispatch(
            &mut session,
            session_bound_request(
                &binding,
                "cancel-old-operation",
                GatewayRequestPayload::CancelSessionBound {
                    operation_id: accepted.operation_id,
                },
            ),
        );
        assert!(matches!(
            cancel.payload,
            GatewayReplyPayload::Cancellation(_)
        ));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gateway_session_operation_ownership_isolates_observe_and_cancel() {
        let (mut session, root) = created_session("operation-session-ownership");
        let mut core = GatewayCore::new();
        let owner_hello = hello(&session);
        let owner = core.connect(&mut session, owner_hello).unwrap();
        let other_hello = hello(&session);
        let other = core.connect(&mut session, other_hello).unwrap();
        let request_id = approval_request_id(&mut core, &session, &owner.client_session_id);
        core.decide_access(
            &session,
            &request_id,
            GatewayAccessDecision::Approve,
            "native-editor-user",
            now_epoch_ms(),
        )
        .unwrap();

        let invocation =
            candidate_invocation(&session, "owned-operation", "action.owned_operation");
        let execute = core.dispatch(
            &mut session,
            session_bound_request(
                &owner,
                "start-owned-operation",
                GatewayRequestPayload::ExecuteSessionBound { invocation },
            ),
        );
        let GatewayReplyPayload::Accepted(accepted) = execute.payload else {
            panic!("owner operation should be accepted");
        };

        let owner_observe = core.dispatch(
            &mut session,
            session_bound_request(
                &owner,
                "owner-observe",
                GatewayRequestPayload::Observe {
                    operation_id: accepted.operation_id.clone(),
                },
            ),
        );
        assert!(matches!(
            owner_observe.payload,
            GatewayReplyPayload::Operation(ref operation)
                if operation.operation_id == accepted.operation_id
        ));

        for (request_id, payload) in [
            (
                "foreign-observe",
                GatewayRequestPayload::Observe {
                    operation_id: accepted.operation_id.clone(),
                },
            ),
            (
                "foreign-cancel",
                GatewayRequestPayload::CancelSessionBound {
                    operation_id: accepted.operation_id.clone(),
                },
            ),
        ] {
            let reply = core.dispatch(
                &mut session,
                session_bound_request(&other, request_id, payload),
            );
            assert!(matches!(
                reply.payload,
                GatewayReplyPayload::Rejected(GatewayDiagnostic { ref code, .. })
                    if code == "gateway.operation.session_mismatch"
            ));
        }

        let owner_cancel = core.dispatch(
            &mut session,
            session_bound_request(
                &owner,
                "owner-cancel",
                GatewayRequestPayload::CancelSessionBound {
                    operation_id: accepted.operation_id,
                },
            ),
        );
        assert!(matches!(
            owner_cancel.payload,
            GatewayReplyPayload::Cancellation(_)
        ));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gateway_approval_lifecycle_prunes_close_ttl_switch_and_reconciles_stale_decision() {
        let (mut first_session, first_root) = created_session("approval-cleanup-first");
        let (second_session, second_root) = created_session("approval-cleanup-second");
        let mut core = GatewayCore::new();
        let first_hello = hello(&first_session);
        let first = core.connect(&mut first_session, first_hello).unwrap();
        let stale_request =
            approval_request_id(&mut core, &first_session, &first.client_session_id);

        first_session = second_session;
        let stale = core
            .decide_access(
                &first_session,
                &stale_request,
                GatewayAccessDecision::Approve,
                "native-editor-user",
                now_epoch_ms(),
            )
            .unwrap_err();
        assert_eq!(stale.code, "gateway.access.request_stale");
        assert!(core.approval_inbox(now_epoch_ms()).is_empty());
        assert_eq!(core.active_client_bindings().len(), 1);
        assert_eq!(
            core.active_client_bindings()[0]
                .project_context
                .as_ref()
                .expect("reconciled second project context")
                .project_identity,
            current_project_binding(&first_session).unwrap().project_id
        );
        core.close(&first.client_session_id);

        let ttl_hello = hello(&first_session);
        let ttl_binding = core.connect(&mut first_session, ttl_hello).unwrap();
        let cleanup = core.prune(
            &first_session,
            ttl_binding.expires_at_epoch_ms.saturating_add(1),
        );
        assert_eq!(
            cleanup.expired_session_ids,
            vec![ttl_binding.client_session_id.clone()]
        );
        assert!(core.approval_inbox(u64::MAX).is_empty());

        let close_hello = hello(&first_session);
        let close_binding = core.connect(&mut first_session, close_hello).unwrap();
        core.close(&close_binding.client_session_id);
        assert!(core.approval_inbox(now_epoch_ms()).is_empty());
        assert!(core.active_client_bindings().is_empty());
        let _ = std::fs::remove_dir_all(first_root);
        let _ = std::fs::remove_dir_all(second_root);
    }

    #[test]
    fn gateway_context_switch_keeps_session_and_replaces_project_context() {
        let (mut first, first_root) = created_session("binding-first");
        let (second, second_root) = created_session("binding-second");
        let second_project = current_project_binding(&second).unwrap();
        let mut core = GatewayCore::new();
        let mut wrong = hello(&first);
        wrong.expected_editor_instance_id = "wrong-editor-instance".to_string();
        assert_eq!(
            core.connect(&mut first, wrong).unwrap_err().code,
            "gateway.binding.editor_instance_mismatch"
        );

        let first_hello = hello(&first);
        let binding = core.connect(&mut first, first_hello).unwrap();
        assert_eq!(binding.effective_read_scope, vec!["catalog", "project"]);
        let first_context = binding
            .project_context
            .as_ref()
            .expect("first project context");
        assert_ne!(first_context.project_identity, second_project.project_id);
        let client_session_id = binding.client_session_id.clone();
        let request = GatewayRequest {
            schema_version: GATEWAY_REQUEST_SCHEMA_VERSION.to_string(),
            gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
            request_id: "catalog-after-switch".to_string(),
            client_session_id: client_session_id.clone(),
            deadline_epoch_ms: None,
            response_limit_bytes: 1024 * 1024,
            payload: GatewayRequestPayload::SessionStatus,
        };
        first = second;
        let reply = core.dispatch(&mut first, request);
        let GatewayReplyPayload::SessionStatus(status) = reply.payload else {
            panic!("same Gateway session must survive project context replacement");
        };
        assert_eq!(status.session.id, client_session_id);
        assert_eq!(
            status
                .project
                .as_ref()
                .expect("second project context")
                .identity,
            second_project.project_id
        );
        assert_eq!(core.active_client_bindings().len(), 1);
        let _ = std::fs::remove_dir_all(first_root);
        let _ = std::fs::remove_dir_all(second_root);
    }

    #[test]
    fn gateway_catalog_context_transition_changes_only_dynamic_availability() {
        let root = std::env::temp_dir().join(format!(
            "ai-tool-gateway-catalog-context-{}-{}",
            std::process::id(),
            SESSION_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut session = EditorSession::new();
        let mut core = GatewayCore::new();
        let session_hello = hello(&session);
        let binding = core.connect(&mut session, session_hello).unwrap();
        let second_hello = hello(&session);
        let second_binding = core.connect(&mut session, second_hello).unwrap();

        let launcher_reply = core.dispatch(
            &mut session,
            session_bound_request(
                &binding,
                "catalog-launcher-context",
                GatewayRequestPayload::Catalog(AiToolCatalogRequest::v2()),
            ),
        );
        let GatewayReplyPayload::Catalog(launcher_catalog) = launcher_reply.payload else {
            panic!("launcher Catalog expected");
        };
        assert_eq!(
            launcher_catalog
                .availability(TOOL_ID_PROJECT_SEARCH)
                .expect("project search availability")
                .state,
            AiToolAvailabilityState::Blocked
        );
        let launcher_wire = serde_json::to_value(&launcher_catalog).unwrap();

        let create = session.execute_command(command_for_test(UiCommandPayload::CreateProject {
            path: root.display().to_string(),
            name: "catalog-context".to_string(),
        }));
        assert_eq!(create.status, CommandStatus::Committed);

        let project_reply = core.dispatch(
            &mut session,
            session_bound_request(
                &binding,
                "catalog-project-context",
                GatewayRequestPayload::Catalog(AiToolCatalogRequest::v2()),
            ),
        );
        let GatewayReplyPayload::Catalog(project_catalog) = project_reply.payload else {
            panic!("project Catalog expected");
        };
        assert_eq!(
            project_catalog
                .availability(TOOL_ID_PROJECT_SEARCH)
                .expect("project search availability")
                .state,
            AiToolAvailabilityState::Ready
        );
        let project_wire = serde_json::to_value(&project_catalog).unwrap();
        let second_project_reply = core.dispatch(
            &mut session,
            session_bound_request(
                &second_binding,
                "catalog-project-context-second-session",
                GatewayRequestPayload::Catalog(AiToolCatalogRequest::v2()),
            ),
        );
        let GatewayReplyPayload::Catalog(second_project_catalog) = second_project_reply.payload
        else {
            panic!("second Session project Catalog expected");
        };
        let second_project_wire = serde_json::to_value(second_project_catalog).unwrap();
        assert_eq!(
            launcher_wire["catalogDigest"], project_wire["catalogDigest"],
            "registered Tool contract remains static across context transition"
        );
        assert_ne!(
            launcher_wire["availabilityDigest"], project_wire["availabilityDigest"],
            "availability digest must track engine-owned context facts"
        );
        assert_eq!(
            project_wire["availabilityDigest"], second_project_wire["availabilityDigest"],
            "all Sessions must consume the same engine-owned context truth"
        );
        assert!(launcher_wire["basis"]["projectIdentity"].is_null());
        assert_eq!(
            project_wire["basis"]["projectIdentity"],
            core.active_client_bindings()[0]
                .project_context
                .as_ref()
                .expect("project context")
                .project_identity
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gateway_context_switch_invalidates_operation_authority_but_preserves_terminal_observation() {
        let (mut first, first_root) = created_session("operation-context-first");
        let (second, second_root) = created_session("operation-context-second");
        let mut core = GatewayCore::new();
        let session_hello = hello(&first);
        let binding = core.connect(&mut first, session_hello).unwrap();
        let second_session_hello = hello(&first);
        let second_binding = core.connect(&mut first, second_session_hello).unwrap();
        let request_id = approval_request_id(&mut core, &first, &binding.client_session_id);
        core.decide_access(
            &first,
            &request_id,
            GatewayAccessDecision::Approve,
            "native-editor-user",
            now_epoch_ms(),
        )
        .unwrap();
        let second_request_id =
            approval_request_id(&mut core, &first, &second_binding.client_session_id);
        core.decide_access(
            &first,
            &second_request_id,
            GatewayAccessDecision::Approve,
            "native-editor-user",
            now_epoch_ms(),
        )
        .unwrap();

        let invocation = candidate_invocation(
            &first,
            "operation-before-context-switch",
            "action.old_context",
        );
        let execute = core.dispatch(
            &mut first,
            session_bound_request(
                &binding,
                "start-operation-before-context-switch",
                GatewayRequestPayload::ExecuteSessionBound { invocation },
            ),
        );
        let GatewayReplyPayload::Accepted(accepted) = execute.payload else {
            panic!("queued operation expected");
        };
        let operation_id = accepted.operation_id;
        let second_invocation = candidate_invocation(
            &first,
            "second-operation-before-context-switch",
            "action.second_old_context",
        );
        let second_execute = core.dispatch(
            &mut first,
            session_bound_request(
                &second_binding,
                "start-second-operation-before-context-switch",
                GatewayRequestPayload::ExecuteSessionBound {
                    invocation: second_invocation,
                },
            ),
        );
        let GatewayReplyPayload::Accepted(second_accepted) = second_execute.payload else {
            panic!("second Session queued operation expected");
        };
        let second_operation_id = second_accepted.operation_id;

        first = second;
        let status = core
            .session_status(&first, &binding.client_session_id, now_epoch_ms())
            .unwrap();
        assert_eq!(
            status.project.expect("replacement project").identity,
            ProjectCandidateEntry::inspect_project_binding(&first)
                .unwrap()
                .project_id
        );

        let cancel = core.dispatch(
            &mut first,
            session_bound_request(
                &binding,
                "cancel-invalidated-operation",
                GatewayRequestPayload::CancelSessionBound {
                    operation_id: operation_id.clone(),
                },
            ),
        );
        let GatewayReplyPayload::Rejected(cancel_error) = cancel.payload else {
            panic!("context-invalidated operation must not be cancellable");
        };
        assert_eq!(
            cancel_error.code,
            "gateway.operation.context_authority_invalidated"
        );

        core.pump_operations(&mut first, 32);
        let observe = core.dispatch(
            &mut first,
            session_bound_request(
                &binding,
                "observe-invalidated-operation",
                GatewayRequestPayload::Observe {
                    operation_id: operation_id.clone(),
                },
            ),
        );
        let GatewayReplyPayload::Operation(operation) = observe.payload else {
            panic!(
                "invalidated operation must remain observable through its terminal outcome: {:?}",
                observe.payload
            );
        };
        let result = operation.result.expect("terminal operation result");
        assert_eq!(result.status, AiToolExecutionStatus::Failed);
        assert_eq!(result.operation_id, operation_id);
        let second_observe = core.dispatch(
            &mut first,
            session_bound_request(
                &second_binding,
                "observe-second-invalidated-operation",
                GatewayRequestPayload::Observe {
                    operation_id: second_operation_id.clone(),
                },
            ),
        );
        let GatewayReplyPayload::Operation(second_operation) = second_observe.payload else {
            panic!("all Sessions must retain their detached terminal operation");
        };
        let second_result = second_operation
            .result
            .expect("second Session terminal operation result");
        assert_eq!(second_result.status, AiToolExecutionStatus::Failed);
        assert_eq!(second_result.operation_id, second_operation_id);
        assert!(
            !second_root
                .join("Input/operation-before-context-switch.input.json")
                .exists(),
            "old queued operation must not write into the replacement project"
        );
        assert!(
            !second_root
                .join("Input/second-operation-before-context-switch.input.json")
                .exists(),
            "second Session old queued operation must not write into the replacement project"
        );
        let _ = std::fs::remove_dir_all(first_root);
        let _ = std::fs::remove_dir_all(second_root);
    }

    #[test]
    fn gateway_owner_thread_dispatch_processes_queued_catalog_request() {
        let (mut session, root) = created_session("owner-thread");
        let mut core = GatewayCore::new();
        let (client, mut dispatcher) = gateway_owner_thread_channel();
        let connect_reply = client.submit_connect(hello(&session)).unwrap();
        assert_eq!(dispatcher.pump(&mut core, &mut session), 1);
        let binding = connect_reply.recv().unwrap().unwrap();
        let request = GatewayRequest {
            schema_version: GATEWAY_REQUEST_SCHEMA_VERSION.to_string(),
            gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
            request_id: "catalog-1".to_string(),
            client_session_id: binding.client_session_id.clone(),
            deadline_epoch_ms: None,
            response_limit_bytes: 1024 * 1024,
            payload: GatewayRequestPayload::Catalog(AiToolCatalogRequest::default()),
        };
        let catalog_reply = client.submit_dispatch(request).unwrap();
        assert_eq!(dispatcher.pump(&mut core, &mut session), 1);
        assert!(matches!(
            catalog_reply.recv().unwrap().payload,
            GatewayReplyPayload::Catalog(_)
        ));
        let close_reply = client.submit_close(binding.client_session_id).unwrap();
        assert_eq!(dispatcher.pump(&mut core, &mut session), 1);
        assert_eq!(
            close_reply.recv().unwrap().diagnostic_code,
            "gateway.session.closed"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gateway_observation_limits_reject_small_response_budget_without_partial_json() {
        let (mut session, root) = created_session("observation-limits");
        let mut core = GatewayCore::new();
        let session_hello = hello(&session);
        let binding = core.connect(&mut session, session_hello).unwrap();
        let reply = core.dispatch(
            &mut session,
            GatewayRequest {
                schema_version: GATEWAY_REQUEST_SCHEMA_VERSION.to_string(),
                gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
                request_id: "small-response".to_string(),
                client_session_id: binding.client_session_id,
                deadline_epoch_ms: None,
                response_limit_bytes: 32,
                payload: GatewayRequestPayload::Catalog(AiToolCatalogRequest::default()),
            },
        );
        assert!(matches!(
            reply.payload,
            GatewayReplyPayload::Rejected(GatewayDiagnostic { ref code, .. })
                if code == "gateway.response.limit_exceeded"
        ));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gateway_grant_ref_is_opaque_bound_revocable_and_renewable() {
        let (mut session, root) = created_session("grant-ref");
        let mut core = GatewayCore::new();
        let session_hello = hello(&session);
        let binding = core.connect(&mut session, session_hello).unwrap();
        let project = ProjectCandidateEntry::inspect_project_binding(&session).unwrap();
        let grant = AiCapabilityGrant::read(
            "internal-grant-id",
            project.project_id,
            project.project_digest,
            "native-editor-user",
        )
        .unwrap();
        let grant_digest = grant.grant_digest.clone();
        let receipt = core
            .issue_grant_ref(&session, &binding.client_session_id, grant)
            .unwrap();
        assert!(!receipt.grant_ref.contains("internal-grant-id"));
        assert!(!receipt.grant_ref.contains(&grant_digest));
        let generation_before_renew = core.sessions[&binding.client_session_id].access_generation;
        let renewed_expiry = now_epoch_ms() + 60_000;
        let renewed = core
            .renew_grant_ref(&receipt.grant_ref, renewed_expiry)
            .unwrap();
        assert_eq!(renewed.grant_ref, receipt.grant_ref);
        assert_eq!(
            core.sessions[&binding.client_session_id].access_generation,
            generation_before_renew + 1
        );
        let renewed_grant = core
            .resolve_grant(&binding.client_session_id, &receipt.grant_ref)
            .unwrap();
        assert_ne!(renewed_grant.grant_digest, grant_digest);
        renewed_grant.validate_integrity().unwrap();
        let repeated_renew = core
            .renew_grant_ref(&receipt.grant_ref, renewed_expiry)
            .unwrap();
        assert_eq!(repeated_renew.expires_at_epoch_ms, Some(renewed_expiry));
        assert_eq!(
            core.sessions[&binding.client_session_id].access_generation,
            generation_before_renew + 1,
            "renewing to the existing expiry must not create a generation"
        );
        let session_read = core
            .resolve_grant(&binding.client_session_id, GATEWAY_SESSION_READ_GRANT_REF)
            .unwrap();
        assert_eq!(session_read.kind, AiCapabilityGrantKind::Read);
        core.revoke_grant_ref(&receipt.grant_ref).unwrap();
        assert_eq!(
            core.resolve_grant(&binding.client_session_id, "$active")
                .unwrap_err()
                .code,
            "gateway.grant_ref.active_missing"
        );

        let invocation = AiToolInvocation {
            schema_version: AI_TOOL_INVOCATION_SCHEMA_VERSION.to_string(),
            invocation_id: "revoked-search".to_string(),
            tool_id: TOOL_ID_PROJECT_SEARCH.to_string(),
            expected_project_digest: ProjectCandidateEntry::inspect_project_binding(&session)
                .unwrap()
                .project_digest,
            payload: AiToolInvocationPayload::ProjectSearch(ProjectSearchInput {
                schema_version: PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION.to_string(),
                query: "project".to_string(),
                kinds: Vec::new(),
                continuation_token: None,
                page_size: 10,
            }),
        };
        let reply = core.dispatch(
            &mut session,
            GatewayRequest {
                schema_version: GATEWAY_REQUEST_SCHEMA_VERSION.to_string(),
                gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
                request_id: "revoked-request".to_string(),
                client_session_id: binding.client_session_id,
                deadline_epoch_ms: None,
                response_limit_bytes: 1024 * 1024,
                payload: GatewayRequestPayload::Execute {
                    invocation,
                    grant_ref: receipt.grant_ref,
                },
            },
        );
        assert!(matches!(
            reply.payload,
            GatewayReplyPayload::Rejected(GatewayDiagnostic { ref code, .. })
                if code == "gateway.grant_ref.revoked"
        ));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gateway_invocation_replay_returns_same_result_and_rejects_changed_content() {
        let (mut session, root) = created_session("invocation-replay");
        let mut core = GatewayCore::new();
        let session_hello = hello(&session);
        let binding = core.connect(&mut session, session_hello).unwrap();
        let project = ProjectCandidateEntry::inspect_project_binding(&session).unwrap();
        let grant = AiCapabilityGrant::read(
            "replay-grant",
            project.project_id,
            project.project_digest.clone(),
            "native-editor-user",
        )
        .unwrap();
        let grant_ref = core
            .issue_grant_ref(&session, &binding.client_session_id, grant)
            .unwrap()
            .grant_ref;
        let invocation = AiToolInvocation {
            schema_version: AI_TOOL_INVOCATION_SCHEMA_VERSION.to_string(),
            invocation_id: "same-invocation".to_string(),
            tool_id: TOOL_ID_PROJECT_SEARCH.to_string(),
            expected_project_digest: project.project_digest,
            payload: AiToolInvocationPayload::ProjectSearch(ProjectSearchInput {
                schema_version: PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION.to_string(),
                query: "project".to_string(),
                kinds: Vec::new(),
                continuation_token: None,
                page_size: 10,
            }),
        };
        let execute = |request_id: &str, invocation: AiToolInvocation| GatewayRequest {
            schema_version: GATEWAY_REQUEST_SCHEMA_VERSION.to_string(),
            gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
            request_id: request_id.to_string(),
            client_session_id: binding.client_session_id.clone(),
            deadline_epoch_ms: None,
            response_limit_bytes: 1024 * 1024,
            payload: GatewayRequestPayload::Execute {
                invocation,
                grant_ref: grant_ref.clone(),
            },
        };
        let first = core.dispatch(&mut session, execute("replay-1", invocation.clone()));
        let second = core.dispatch(&mut session, execute("replay-2", invocation.clone()));
        let (
            GatewayReplyPayload::Accepted(first_accepted),
            GatewayReplyPayload::Accepted(second_accepted),
        ) = (first.payload, second.payload)
        else {
            panic!("expected accepted operations");
        };
        assert_eq!(first_accepted.operation_id, second_accepted.operation_id);
        assert_eq!(
            core.sessions[&binding.client_session_id].operation_generation, 1,
            "replaying an accepted operation must not create a generation"
        );
        core.pump_operations(&mut session, 3);
        assert_eq!(
            core.sessions[&binding.client_session_id].operation_generation, 2,
            "the first terminal transition creates one generation"
        );
        let terminal = core.dispatch(&mut session, execute("replay-terminal", invocation.clone()));
        let GatewayReplyPayload::ToolResult(terminal) = terminal.payload else {
            panic!("expected terminal replay result");
        };
        assert_eq!(terminal.status, AiToolExecutionStatus::Completed);
        assert_eq!(
            core.sessions[&binding.client_session_id].operation_generation, 2,
            "replaying a terminal result must not create a generation"
        );
        let mut changed = invocation;
        let AiToolInvocationPayload::ProjectSearch(input) = &mut changed.payload else {
            unreachable!();
        };
        input.query = "different".to_string();
        let changed = core.dispatch(&mut session, execute("replay-3", changed));
        let GatewayReplyPayload::ToolResult(changed_result) = changed.payload else {
            panic!("expected changed replay tool result");
        };
        assert_eq!(changed_result.status, AiToolExecutionStatus::Failed);
        assert_eq!(
            changed_result.diagnostics[0].code,
            "ai_tool.invocation_replay_mismatch"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gateway_bypass_drift_rejects_external_write_without_fake_receipt() {
        let (mut session, root) = created_session("bypass-drift");
        let mut core = GatewayCore::new();
        let session_hello = hello(&session);
        let binding = core.connect(&mut session, session_hello).unwrap();
        let invocation = candidate_invocation(&session, "bypass-drift", "action.bypass-drift");

        std::fs::write(root.join("external-maintainer-write.txt"), "drift").unwrap();
        let reply = core.dispatch(
            &mut session,
            session_bound_request(
                &binding,
                "bypass-drift",
                GatewayRequestPayload::ExecuteSessionBound { invocation },
            ),
        );
        let GatewayReplyPayload::Rejected(result) = reply.payload else {
            panic!("drift must fail before an operation is accepted")
        };
        assert_eq!(result.code, "gateway.access.goal_project_mismatch");
        assert!(core.operation_grants.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }
    #[test]
    fn gateway_performance_contract_measures_control_plane_without_build_side_effects() {
        let (mut session, root) = created_session("performance-contract");
        let startup_started = std::time::Instant::now();
        let mut core = GatewayCore::new();
        let startup_ms = startup_started.elapsed().as_millis() as u64;

        let handshake_started = std::time::Instant::now();
        let session_hello = hello(&session);
        let binding = core.connect(&mut session, session_hello).unwrap();
        let handshake_ms = handshake_started.elapsed().as_millis() as u64;

        let catalog_request = |request_id: &str| GatewayRequest {
            schema_version: GATEWAY_REQUEST_SCHEMA_VERSION.to_string(),
            gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
            request_id: request_id.to_string(),
            client_session_id: binding.client_session_id.clone(),
            deadline_epoch_ms: None,
            response_limit_bytes: 1024 * 1024,
            payload: GatewayRequestPayload::Catalog(AiToolCatalogRequest::default()),
        };
        let cold_catalog_started = std::time::Instant::now();
        let cold_catalog = core.dispatch(&mut session, catalog_request("catalog-cold"));
        let cold_catalog_ms = cold_catalog_started.elapsed().as_millis() as u64;
        assert!(matches!(
            cold_catalog.payload,
            GatewayReplyPayload::Catalog(_)
        ));
        let warm_catalog_started = std::time::Instant::now();
        let warm_catalog = core.dispatch(&mut session, catalog_request("catalog-warm"));
        let warm_catalog_ms = warm_catalog_started.elapsed().as_millis() as u64;
        assert!(matches!(
            warm_catalog.payload,
            GatewayReplyPayload::Catalog(_)
        ));

        let project = ProjectCandidateEntry::inspect_project_binding(&session).unwrap();
        let grant = AiCapabilityGrant::read(
            "performance-read-grant",
            project.project_id,
            project.project_digest.clone(),
            "performance-contract",
        )
        .unwrap();
        let grant_ref = core
            .issue_grant_ref(&session, &binding.client_session_id, grant)
            .unwrap()
            .grant_ref;
        let search = AiToolInvocation {
            schema_version: AI_TOOL_INVOCATION_SCHEMA_VERSION.to_string(),
            invocation_id: "performance-search".to_string(),
            tool_id: TOOL_ID_PROJECT_SEARCH.to_string(),
            expected_project_digest: project.project_digest,
            payload: AiToolInvocationPayload::ProjectSearch(ProjectSearchInput {
                schema_version: PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION.to_string(),
                query: "project".to_string(),
                kinds: Vec::new(),
                continuation_token: None,
                page_size: 10,
            }),
        };
        let search_started = std::time::Instant::now();
        let search = core.dispatch(
            &mut session,
            execute_request(&binding, "performance-search", search, &grant_ref),
        );
        let GatewayReplyPayload::Accepted(search) = search.payload else {
            panic!("search should be accepted")
        };
        core.pump_operations(&mut session, 3);
        let search_result = core.kernel.observe(&search.operation_id).unwrap();
        assert_eq!(
            search_result.result.unwrap().status,
            AiToolExecutionStatus::Completed
        );
        let search_ms = search_started.elapsed().as_millis() as u64;

        assert!(!root.join("Build").exists());
        assert!(!root.join("Library/Imported").exists());
        assert!(!root.join("target").exists());
        let report = GatewayPerformanceContractReport::new(
            vec![
                GatewayPerformanceStageSample::in_process(
                    "adapter_startup",
                    startup_ms,
                    GatewayCacheState::Cold,
                ),
                GatewayPerformanceStageSample::in_process(
                    "handshake",
                    handshake_ms,
                    GatewayCacheState::Cold,
                ),
                GatewayPerformanceStageSample::in_process(
                    "catalog",
                    cold_catalog_ms,
                    GatewayCacheState::Cold,
                ),
                GatewayPerformanceStageSample::in_process(
                    "catalog",
                    warm_catalog_ms,
                    GatewayCacheState::Warm,
                ),
                GatewayPerformanceStageSample::in_process(
                    "search",
                    search_ms,
                    GatewayCacheState::Warm,
                ),
            ],
            2 * 60 * 60 * 1000,
        );
        report.validate().unwrap();
        assert!(report.total_preflight_ms < 5_000, "report={report:#?}");
        assert!(report.remaining_budget_ms > 60 * 60 * 1000);
        serde_json::to_string(&report).unwrap();
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gateway_disconnect_reconnect_does_not_transfer_operation_ownership() {
        let (mut session, root) = created_session("disconnect-reconnect");
        let mut core = GatewayCore::new();
        let first_hello = hello(&session);
        let first_binding = core.connect(&mut session, first_hello).unwrap();
        let project = ProjectCandidateEntry::inspect_project_binding(&session).unwrap();
        let grant = AiCapabilityGrant::read(
            "disconnect-grant",
            project.project_id,
            project.project_digest.clone(),
            "native-editor-user",
        )
        .unwrap();
        let grant_ref = core
            .issue_grant_ref(&session, &first_binding.client_session_id, grant)
            .unwrap()
            .grant_ref;
        let accepted = core.dispatch(
            &mut session,
            GatewayRequest {
                schema_version: GATEWAY_REQUEST_SCHEMA_VERSION.to_string(),
                gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
                request_id: "disconnect-execute".to_string(),
                client_session_id: first_binding.client_session_id.clone(),
                deadline_epoch_ms: None,
                response_limit_bytes: 1024 * 1024,
                payload: GatewayRequestPayload::Execute {
                    invocation: AiToolInvocation {
                        schema_version: AI_TOOL_INVOCATION_SCHEMA_VERSION.to_string(),
                        invocation_id: "disconnect-operation".to_string(),
                        tool_id: TOOL_ID_PROJECT_SEARCH.to_string(),
                        expected_project_digest: project.project_digest,
                        payload: AiToolInvocationPayload::ProjectSearch(ProjectSearchInput {
                            schema_version: PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION.to_string(),
                            query: "project".to_string(),
                            kinds: Vec::new(),
                            continuation_token: None,
                            page_size: 10,
                        }),
                    },
                    grant_ref,
                },
            },
        );
        let GatewayReplyPayload::Accepted(accepted) = accepted.payload else {
            panic!("expected accepted operation");
        };
        let operation_id = accepted.operation_id;
        core.close(&first_binding.client_session_id);
        assert!(!core.operation_grants.contains_key(&operation_id));

        let second_hello = hello(&session);
        let second_binding = core.connect(&mut session, second_hello).unwrap();
        let observed = core.dispatch(
            &mut session,
            GatewayRequest {
                schema_version: GATEWAY_REQUEST_SCHEMA_VERSION.to_string(),
                gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
                request_id: "reconnected-observe".to_string(),
                client_session_id: second_binding.client_session_id,
                deadline_epoch_ms: None,
                response_limit_bytes: 1024 * 1024,
                payload: GatewayRequestPayload::Observe { operation_id },
            },
        );
        assert!(matches!(
            observed.payload,
            GatewayReplyPayload::Rejected(GatewayDiagnostic { ref code, .. })
                if code == "gateway.operation.snapshot_missing"
        ));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gateway_backpressure_rejects_work_beyond_bounded_operation_queue() {
        let (mut session, root) = created_session("backpressure");
        let mut core = GatewayCore::new();
        let session_hello = hello(&session);
        let binding = core.connect(&mut session, session_hello).unwrap();
        let project = ProjectCandidateEntry::inspect_project_binding(&session).unwrap();
        let grant = AiCapabilityGrant::read(
            "backpressure-grant",
            project.project_id,
            project.project_digest.clone(),
            "native-editor-user",
        )
        .unwrap();
        let grant_ref = core
            .issue_grant_ref(&session, &binding.client_session_id, grant)
            .unwrap()
            .grant_ref;

        for index in 0..64 {
            let reply = core.dispatch(
                &mut session,
                GatewayRequest {
                    schema_version: GATEWAY_REQUEST_SCHEMA_VERSION.to_string(),
                    gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
                    request_id: format!("backpressure-{index}"),
                    client_session_id: binding.client_session_id.clone(),
                    deadline_epoch_ms: None,
                    response_limit_bytes: 1024 * 1024,
                    payload: GatewayRequestPayload::Execute {
                        invocation: AiToolInvocation {
                            schema_version: AI_TOOL_INVOCATION_SCHEMA_VERSION.to_string(),
                            invocation_id: format!("backpressure-invocation-{index}"),
                            tool_id: TOOL_ID_PROJECT_SEARCH.to_string(),
                            expected_project_digest: project.project_digest.clone(),
                            payload: AiToolInvocationPayload::ProjectSearch(ProjectSearchInput {
                                schema_version: PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION
                                    .to_string(),
                                query: format!("query-{index}"),
                                kinds: Vec::new(),
                                continuation_token: None,
                                page_size: 1,
                            }),
                        },
                        grant_ref: grant_ref.clone(),
                    },
                },
            );
            assert!(matches!(reply.payload, GatewayReplyPayload::Accepted(_)));
        }

        let overflow = core.dispatch(
            &mut session,
            GatewayRequest {
                schema_version: GATEWAY_REQUEST_SCHEMA_VERSION.to_string(),
                gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
                request_id: "backpressure-overflow".to_string(),
                client_session_id: binding.client_session_id,
                deadline_epoch_ms: None,
                response_limit_bytes: 1024 * 1024,
                payload: GatewayRequestPayload::Execute {
                    invocation: AiToolInvocation {
                        schema_version: AI_TOOL_INVOCATION_SCHEMA_VERSION.to_string(),
                        invocation_id: "backpressure-overflow-invocation".to_string(),
                        tool_id: TOOL_ID_PROJECT_SEARCH.to_string(),
                        expected_project_digest: project.project_digest,
                        payload: AiToolInvocationPayload::ProjectSearch(ProjectSearchInput {
                            schema_version: PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION.to_string(),
                            query: "overflow".to_string(),
                            kinds: Vec::new(),
                            continuation_token: None,
                            page_size: 1,
                        }),
                    },
                    grant_ref,
                },
            },
        );
        let GatewayReplyPayload::Rejected(result) = overflow.payload else {
            panic!("overflow must be rejected before a Tool operation is accepted");
        };
        let availability = result.availability.expect("structured queue blocker");
        assert_eq!(availability.state, AiToolAvailabilityState::Blocked);
        assert!(availability
            .reasons
            .iter()
            .any(|reason| reason.code == "ai_tool.availability.operation_conflict"));
        assert_eq!(core.operation_grants.len(), 64);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn adapter_equivalence_preserves_catalog_and_frozen_invocation_identity() {
        let (mut session, root) = created_session("adapter-equivalence");
        let mut core = GatewayCore::new();
        let project = ProjectCandidateEntry::inspect_project_binding(&session).unwrap();
        let grant = AiCapabilityGrant::read(
            "adapter-equivalence-grant",
            project.project_id,
            project.project_digest.clone(),
            "native-editor-user",
        )
        .unwrap();
        let invocation = AiToolInvocation {
            schema_version: AI_TOOL_INVOCATION_SCHEMA_VERSION.to_string(),
            invocation_id: "adapter-equivalence-invocation".to_string(),
            tool_id: TOOL_ID_PROJECT_SEARCH.to_string(),
            expected_project_digest: project.project_digest,
            payload: AiToolInvocationPayload::ProjectSearch(ProjectSearchInput {
                schema_version: PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION.to_string(),
                query: "project".to_string(),
                kinds: Vec::new(),
                continuation_token: None,
                page_size: 10,
            }),
        };
        let mut catalog_json = None;
        let mut operation_id = None;
        for kind in [
            ClientKind::Mcp,
            ClientKind::Cli,
            ClientKind::NativeEditor,
            ClientKind::Test,
        ] {
            let mut adapter_hello = hello(&session);
            adapter_hello.client_kind = kind;
            let binding = core.connect(&mut session, adapter_hello).unwrap();
            let grant_ref = core
                .issue_grant_ref(&session, &binding.client_session_id, grant.clone())
                .unwrap()
                .grant_ref;
            let catalog = core.dispatch(
                &mut session,
                GatewayRequest {
                    schema_version: GATEWAY_REQUEST_SCHEMA_VERSION.to_string(),
                    gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
                    request_id: format!("catalog-{kind:?}"),
                    client_session_id: binding.client_session_id.clone(),
                    deadline_epoch_ms: None,
                    response_limit_bytes: 1024 * 1024,
                    payload: GatewayRequestPayload::Catalog(AiToolCatalogRequest::default()),
                },
            );
            let GatewayReplyPayload::Catalog(catalog) = catalog.payload else {
                panic!("adapter catalog expected");
            };
            let encoded = serde_json::to_value(catalog).unwrap();
            if let Some(expected) = &catalog_json {
                assert_eq!(&encoded, expected);
            } else {
                catalog_json = Some(encoded);
            }
            let execute = core.dispatch(
                &mut session,
                GatewayRequest {
                    schema_version: GATEWAY_REQUEST_SCHEMA_VERSION.to_string(),
                    gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
                    request_id: format!("execute-{kind:?}"),
                    client_session_id: binding.client_session_id,
                    deadline_epoch_ms: None,
                    response_limit_bytes: 1024 * 1024,
                    payload: GatewayRequestPayload::Execute {
                        invocation: invocation.clone(),
                        grant_ref,
                    },
                },
            );
            let GatewayReplyPayload::Accepted(accepted) = execute.payload else {
                panic!("adapter accepted result expected");
            };
            if let Some(expected) = &operation_id {
                assert_eq!(&accepted.operation_id, expected);
            } else {
                operation_id = Some(accepted.operation_id);
            }
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gateway_owner_thread_channel_wakes_host_for_each_queued_command() {
        let wake_count = Arc::new(AtomicUsize::new(0));
        let observed_wake_count = Arc::clone(&wake_count);
        let (client, _dispatcher) = gateway_owner_thread_channel_with_wake(Arc::new(move || {
            observed_wake_count.fetch_add(1, Ordering::SeqCst);
        }));
        let (session, root) = created_session("Gateway Wake");

        let _connect = client.submit_connect(hello(&session)).unwrap();
        let _dispatch = client
            .submit_dispatch(GatewayRequest {
                schema_version: GATEWAY_REQUEST_SCHEMA_VERSION.to_string(),
                gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
                request_id: "wake-dispatch".to_string(),
                client_session_id: "wake-client".to_string(),
                deadline_epoch_ms: None,
                response_limit_bytes: 1024,
                payload: GatewayRequestPayload::Catalog(AiToolCatalogRequest::default()),
            })
            .unwrap();
        let _close = client.submit_close("wake-client").unwrap();

        assert_eq!(wake_count.load(Ordering::SeqCst), 3);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn canonical_root_digest_ignores_windows_extended_path_prefix() {
        assert_eq!(
            canonical_root_digest(r"I:\EngineTest\AiFirstGame"),
            canonical_root_digest(r"\\?\I:\EngineTest\AiFirstGame")
        );
        assert_eq!(
            canonical_root_digest(r"\\server\share\AiFirstGame"),
            canonical_root_digest(r"\\?\UNC\server\share\AiFirstGame")
        );
    }
}
