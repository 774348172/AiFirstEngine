use authoring_project_context::{
    ProjectMutation, ProjectMutationOperation, ProjectMutationReceipt, RefreshReport,
    PROJECT_MUTATION_SCHEMA_VERSION,
};
use project_authoring_execution::{
    default_project_runtime_player_build_root, BuildRequest, DeliveryRef, ExecutionMode,
    GameProjectCompiler, ProjectAuthoringSession, ProjectRelativePath, ProjectSourceInventory,
    RunOptions, TargetProfile, VerifyRequest,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub mod codex_config;
pub mod mcp_stdio;
mod semantic;
use semantic::{RetainedDelivery, RetainedPlaytest};

pub const ENGINE_TOOL_RESULT_SCHEMA_VERSION: &str = "engine-tool-result.v2";
pub const ENGINE_TOOL_DEFINITION_SCHEMA_VERSION: &str = "engine-tool-definition.v1";
pub const ENGINE_CAPABILITY_GRANT_SCHEMA_VERSION: &str = "engine-capability-grant.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolSideEffect {
    Read,
    Write,
    ProcessSpawn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCapability {
    ReadProject,
    MutateProject,
    DeleteProjectContent,
    GenerateFiles,
    SpawnProcess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolRisk {
    Low,
    Elevated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolDuration {
    Instant,
    Short,
    Long,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolMaturity {
    Ready,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolAudience {
    ModelDefault,
    InternalDiagnostics,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolDefinition {
    pub schema_version: String,
    pub name: String,
    pub description: String,
    pub side_effect: ToolSideEffect,
    pub maturity: ToolMaturity,
    pub input_schema: Value,
    pub output_schema: Value,
    pub required_capabilities: Vec<ToolCapability>,
    pub risk: ToolRisk,
    pub duration: ToolDuration,
    pub supports_cancellation: bool,
    pub supports_rollback: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostSessionContext {
    pub session_id: String,
    pub workspace_root: PathBuf,
    pub project_root: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HostToolCall {
    pub call_id: String,
    pub tool_name: String,
    pub arguments: Value,
    pub approved: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanonicalToolStatus {
    Completed,
    RejectedByHost,
    RejectedByEngine,
    Failed,
    Cancelled,
    UnsupportedOnAdapter,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolDiagnostic {
    pub code: String,
    pub message: String,
    pub next_action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_location: Option<project_authoring_execution::SourceLocation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compiler_diagnostics: Vec<project_authoring_execution::CheckDiagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolRetryability {
    NotNeeded,
    RetryAfterInspect,
    RetryAfterCorrection,
    NotRetryable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalTransitionCategory {
    InspectLatestRevision,
    RetryWithCorrection,
    VerifyDelivery,
    RollbackAvailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecommendedLocalTransition {
    pub category: LocalTransitionCategory,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanonicalToolResult {
    pub schema_version: String,
    pub call_id: String,
    pub tool_name: String,
    pub status: CanonicalToolStatus,
    pub operation_id: String,
    pub project_revision: Option<String>,
    pub output: Value,
    pub diagnostics: Vec<ToolDiagnostic>,
    pub retryability: ToolRetryability,
    pub recommended_local_transitions: Vec<RecommendedLocalTransition>,
    pub receipt_ref: Option<String>,
    pub evidence_refs: Vec<String>,
    pub replayed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationState {
    Running,
    Completed,
    Rejected,
    Failed,
    Cancelled,
}

impl OperationState {
    fn is_terminal(self) -> bool {
        self != Self::Running
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationSnapshot {
    pub operation_id: String,
    pub state: OperationState,
    pub terminal: bool,
    pub result: Option<CanonicalToolResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancellationStatus {
    Cancelled,
    AlreadyTerminal,
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CancellationReceipt {
    pub operation_id: String,
    pub status: CancellationStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DetachReceipt {
    pub session_id: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EngineCapabilityGrant {
    pub schema_version: String,
    pub grant_digest: String,
    pub session_id: String,
    pub call_id: String,
    pub tool_name: String,
    pub project_identity: Option<String>,
    pub project_revision: Option<String>,
    pub capabilities: Vec<ToolCapability>,
    pub risk: ToolRisk,
    pub max_mutation_count: u32,
    pub allow_delete: bool,
}

#[derive(Debug, Clone)]
struct OperationRecord {
    call_digest: String,
    grant: EngineCapabilityGrant,
    state: OperationState,
    result: Option<CanonicalToolResult>,
}

pub trait NativeEngineToolProvider {
    fn tool_definitions(&self) -> Vec<ToolDefinition>;
    fn invoke(&mut self, call: HostToolCall) -> CanonicalToolResult;
    fn observe(&self, operation_id: &str) -> Option<OperationSnapshot>;
    fn cancel(&mut self, operation_id: &str) -> CancellationReceipt;
}

#[derive(Debug)]
pub struct NativeHostAdapter {
    provider: EngineToolProvider,
}

impl NativeHostAdapter {
    pub fn attach(host: HostSessionContext) -> Result<Self, ToolDiagnostic> {
        Ok(Self {
            provider: EngineToolProvider::attach(host)?,
        })
    }

    pub fn tool_definitions(&self) -> Vec<ToolDefinition> {
        tool_definitions_for(ToolAudience::ModelDefault)
    }

    pub fn invoke(&mut self, call: HostToolCall) -> CanonicalToolResult {
        self.provider.invoke_model(call)
    }

    pub fn observe(&self, operation_id: &str) -> Option<OperationSnapshot> {
        self.provider.observe(operation_id)
    }

    pub fn cancel(&mut self, operation_id: &str) -> CancellationReceipt {
        self.provider.cancel(operation_id)
    }
}

#[derive(Debug)]
pub struct EngineToolProvider {
    host: HostSessionContext,
    project: Option<ProjectAuthoringSession>,
    operations: HashMap<String, OperationRecord>,
    replay_operations: HashMap<String, String>,
    rollback_receipts: HashMap<String, ProjectMutationReceipt>,
    deliveries: HashMap<String, RetainedDelivery>,
    playtests: HashMap<String, RetainedPlaytest>,
    next_operation: u64,
}

impl EngineToolProvider {
    pub fn attach(host: HostSessionContext) -> Result<Self, ToolDiagnostic> {
        let workspace_root = host.workspace_root.canonicalize().map_err(|error| {
            diagnostic(
                "engine_provider.workspace_unavailable",
                format!("Workspace root cannot be resolved: {error}"),
                "Provide an existing workspace root.",
            )
        })?;
        let mut provider = Self {
            host: HostSessionContext {
                workspace_root,
                ..host
            },
            project: None,
            operations: HashMap::new(),
            replay_operations: HashMap::new(),
            rollback_receipts: HashMap::new(),
            deliveries: HashMap::new(),
            playtests: HashMap::new(),
            next_operation: 0,
        };
        if let Some(project_root) = provider.host.project_root.clone() {
            provider.bind_project(&project_root, None)?;
        } else if provider
            .host
            .workspace_root
            .join("project.aife.json")
            .is_file()
        {
            let root = provider.host.workspace_root.clone();
            provider.bind_project(&root, None)?;
        }
        Ok(provider)
    }

    pub(crate) fn invoke_model(&mut self, call: HostToolCall) -> CanonicalToolResult {
        let definition = tool_definitions()
            .into_iter()
            .find(|definition| definition.name == call.tool_name);
        match definition {
            None => self.invoke(call),
            Some(definition) if tool_is_exposed(&definition, ToolAudience::ModelDefault) => {
                self.invoke(call)
            }
            Some(_) => self.reject_not_exposed(call),
        }
    }

    fn reject_not_exposed(&mut self, call: HostToolCall) -> CanonicalToolResult {
        let call_digest = call_digest(&call);
        if let Some(existing_operation_id) = self.replay_operations.get(&call.call_id) {
            let existing = self
                .operations
                .get(existing_operation_id)
                .expect("replay index must reference an operation");
            if existing.call_digest == call_digest {
                if let Some(mut result) = existing.result.clone() {
                    result.replayed = true;
                    return result;
                }
            } else {
                return self.invoke(call);
            }
        }
        let operation_id = self.next_operation_id();
        let grant = self.initial_grant(&call);
        self.start_operation(&call, &operation_id, call_digest, grant, true);
        self.finish(
            &call,
            operation_id,
            CanonicalToolStatus::RejectedByEngine,
            None,
            Value::Null,
            vec![diagnostic(
                "engine_provider.tool_not_exposed",
                "Tool is not exposed to the model-default Engine tool audience.",
                "Choose a concrete tool returned by tools/list.",
            )],
            None,
            Vec::new(),
        )
    }

    pub fn detach(self) -> DetachReceipt {
        DetachReceipt {
            session_id: self.host.session_id,
            status: "detached".to_string(),
        }
    }

    fn bind_project(
        &mut self,
        requested: &Path,
        current_operation_id: Option<&str>,
    ) -> Result<(), ToolDiagnostic> {
        if self.operations.iter().any(|(operation_id, operation)| {
            Some(operation_id.as_str()) != current_operation_id && !operation.state.is_terminal()
        }) {
            return Err(diagnostic(
                "engine_provider.project_rebind_blocked",
                "Project binding cannot change while another operation is non-terminal.",
                "Wait for or cancel the active operation, then retry engine_project_open.",
            ));
        }
        let root = if requested.is_absolute() {
            requested.to_path_buf()
        } else {
            self.host.workspace_root.join(requested)
        };
        let root = root.canonicalize().map_err(|error| {
            diagnostic(
                "engine_provider.project_root_unavailable",
                format!("Project root cannot be resolved: {error}"),
                "Choose an existing project root inside the workspace.",
            )
        })?;
        if !root.starts_with(&self.host.workspace_root) {
            return Err(diagnostic(
                "engine_provider.project_outside_workspace",
                "Project root is outside the host workspace.",
                "Choose a project inside the bound workspace.",
            ));
        }
        if self
            .project
            .as_ref()
            .is_some_and(|project| project.project_root() == root)
        {
            return Ok(());
        }
        self.project = Some(ProjectAuthoringSession::open(&root).map_err(context_diagnostic)?);
        self.rollback_receipts.clear();
        self.deliveries.clear();
        self.playtests.clear();
        Ok(())
    }

    fn next_operation_id(&mut self) -> String {
        self.next_operation = self.next_operation.saturating_add(1);
        format!("engine-op-{}-{}", self.host.session_id, self.next_operation)
    }

    #[allow(clippy::too_many_arguments)]
    fn finish(
        &mut self,
        call: &HostToolCall,
        operation_id: String,
        status: CanonicalToolStatus,
        revision: Option<String>,
        output: Value,
        diagnostics: Vec<ToolDiagnostic>,
        receipt_ref: Option<String>,
        evidence_refs: Vec<String>,
    ) -> CanonicalToolResult {
        let local_flow = derive_local_flow(
            &call.tool_name,
            status,
            &output,
            &diagnostics,
            receipt_ref.as_deref(),
        );
        let result = CanonicalToolResult {
            schema_version: ENGINE_TOOL_RESULT_SCHEMA_VERSION.to_string(),
            call_id: call.call_id.clone(),
            tool_name: call.tool_name.clone(),
            status,
            operation_id: operation_id.clone(),
            project_revision: revision,
            output,
            diagnostics,
            retryability: local_flow.retryability,
            recommended_local_transitions: local_flow.transitions,
            receipt_ref,
            evidence_refs,
            replayed: false,
        };
        let state = operation_state_for_status(status);
        if let Some(operation) = self.operations.get_mut(&operation_id) {
            operation.state = state;
            operation.result = Some(result.clone());
        }
        result
    }

    fn start_operation(
        &mut self,
        call: &HostToolCall,
        operation_id: &str,
        call_digest: String,
        grant: EngineCapabilityGrant,
        record_replay: bool,
    ) {
        self.operations.insert(
            operation_id.to_string(),
            OperationRecord {
                call_digest,
                grant,
                state: OperationState::Running,
                result: None,
            },
        );
        if record_replay {
            self.replay_operations
                .insert(call.call_id.clone(), operation_id.to_string());
        }
    }

    fn initial_grant(&self, call: &HostToolCall) -> EngineCapabilityGrant {
        let mut grant = EngineCapabilityGrant {
            schema_version: ENGINE_CAPABILITY_GRANT_SCHEMA_VERSION.to_string(),
            grant_digest: String::new(),
            session_id: self.host.session_id.clone(),
            call_id: call.call_id.clone(),
            tool_name: call.tool_name.clone(),
            project_identity: None,
            project_revision: None,
            capabilities: Vec::new(),
            risk: ToolRisk::Low,
            max_mutation_count: 0,
            allow_delete: false,
        };
        grant.grant_digest = grant_digest(&grant).unwrap_or_default();
        grant
    }

    fn prepare_project_refresh(
        &mut self,
        call: &HostToolCall,
    ) -> Result<Option<RefreshReport>, ToolDiagnostic> {
        if self.retained_playtest_lineage(call)?.is_some() {
            return Ok(None);
        }
        if matches!(
            call.tool_name.as_str(),
            "engine_project_open" | "engine_operation_observe" | "engine_operation_cancel"
        ) {
            return Ok(None);
        }
        self.project_mut()?
            .refresh()
            .map(Some)
            .map_err(context_diagnostic)
    }

    fn issue_grant(
        &self,
        call: &HostToolCall,
        definition: &ToolDefinition,
        refresh: Option<&RefreshReport>,
    ) -> Result<EngineCapabilityGrant, ToolDiagnostic> {
        let mut capabilities = definition.required_capabilities.clone();
        let mutation_count = mutation_change_count(&call.tool_name, &call.arguments)?;
        let allow_delete = mutation_allows_delete(&call.tool_name, &call.arguments)?;
        if allow_delete {
            capabilities.push(ToolCapability::DeleteProjectContent);
        }
        capabilities.sort();
        capabilities.dedup();
        let retained = self.retained_playtest_lineage(call)?;
        let mut grant = EngineCapabilityGrant {
            schema_version: ENGINE_CAPABILITY_GRANT_SCHEMA_VERSION.to_string(),
            grant_digest: String::new(),
            session_id: self.host.session_id.clone(),
            call_id: call.call_id.clone(),
            tool_name: call.tool_name.clone(),
            project_identity: refresh
                .map(|report| report.revision.portable_project_identity.clone())
                .or_else(|| retained.map(|lineage| lineage.project_identity().to_string())),
            project_revision: refresh
                .map(|report| report.revision.revision_id.clone())
                .or_else(|| retained.map(|lineage| lineage.revision_id().to_string())),
            capabilities,
            risk: if allow_delete {
                ToolRisk::Elevated
            } else {
                definition.risk
            },
            max_mutation_count: mutation_count,
            allow_delete,
        };
        grant.grant_digest = grant_digest(&grant)?;
        validate_grant(&grant, call, definition)?;
        Ok(grant)
    }

    fn project_mut(&mut self) -> Result<&mut ProjectAuthoringSession, ToolDiagnostic> {
        self.project.as_mut().ok_or_else(|| {
            diagnostic(
                "engine_provider.project_binding_required",
                "No project is bound to this provider session.",
                "Call engine_project_open with a workspace-contained project root.",
            )
        })
    }

    fn invoke_ready(
        &mut self,
        call: &HostToolCall,
        operation_id: &str,
        refresh: Option<&RefreshReport>,
        grant: &EngineCapabilityGrant,
    ) -> Result<InvocationSuccess, ToolDiagnostic> {
        match call.tool_name.as_str() {
            "engine_project_open" => {
                let project_root = required_string(&call.arguments, "projectRoot")?;
                self.bind_project(Path::new(project_root), Some(operation_id))?;
                let session = self.project_mut()?;
                let revision = session.revision().map_err(context_diagnostic)?;
                Ok(InvocationSuccess::new(
                    revision.revision_id.clone(),
                    json!({
                        "projectRoot": session.project_root(),
                        "projectIdentity": revision.portable_project_identity,
                        "qualification": revision.qualification,
                        "recovery": session.recovery_report(),
                    }),
                ))
            }
            "engine_project_inspect" => {
                let session = self.project_mut()?;
                let inventory = session.source_inventory().map_err(context_diagnostic)?;
                let refresh = required_refresh(refresh)?;
                Ok(InvocationSuccess::new(
                    refresh.revision.revision_id.clone(),
                    inspection_output(&refresh, &inventory),
                ))
            }
            "engine_project_search"
            | "engine_project_references"
            | "engine_project_source_symbols"
            | "engine_ui_locate"
            | "engine_ui_explain_visibility"
            | "engine_project_trace_ui_owner" => {
                let query = required_string(&call.arguments, "query")?.to_string();
                let session = self.project_mut()?;
                let inventory = session.source_inventory().map_err(context_diagnostic)?;
                let refresh = required_refresh(refresh)?;
                let output = search_project(
                    session,
                    &inventory,
                    &query,
                    &call.tool_name,
                    operation_id,
                    &refresh.revision.revision_id,
                )?;
                Ok(InvocationSuccess::new(
                    refresh.revision.revision_id.clone(),
                    output,
                ))
            }
            "engine_project_read_object" => {
                let path = required_string(&call.arguments, "path")?.to_string();
                let session = self.project_mut()?;
                let refresh = required_refresh(refresh)?;
                let lease = session
                    .acquire_snapshot_lease(operation_id, vec![path.clone()])
                    .map_err(context_diagnostic)?;
                if lease.snapshot().revision.revision_id != refresh.revision.revision_id {
                    return Err(revision_changed_diagnostic());
                }
                let file = lease.snapshot().files.first().cloned().ok_or_else(|| {
                    diagnostic(
                        "engine_provider.object_missing",
                        "Requested object was not present in the Context snapshot.",
                        "Inspect the project and choose an existing canonical path.",
                    )
                })?;
                let _ = lease.release();
                Ok(InvocationSuccess::new(
                    refresh.revision.revision_id.clone(),
                    json!({
                        "path": path,
                        "contentDigest": file.content_digest,
                        "length": file.length,
                        "text": String::from_utf8_lossy(&file.bytes),
                    }),
                ))
            }
            "engine_project_diagnostics" => {
                let refresh = required_refresh(refresh)?;
                Ok(InvocationSuccess::new(
                    refresh.revision.revision_id.clone(),
                    json!({
                        "qualification": refresh.revision.qualification,
                        "diagnostics": refresh.diagnostics,
                    }),
                ))
            }
            "engine_project_check" => self.invoke_project_check(call, operation_id, refresh),
            "engine_runtime_run" => self.invoke_project_run(call, operation_id, refresh),
            "engine_runtime_playtest" => self.invoke_playtest(call, operation_id, refresh),
            "engine_runtime_observe" => self.invoke_playtest_observe(call),
            "engine_project_build" => self.invoke_project_build(call, operation_id, refresh),
            "engine_delivery_verify" => self.invoke_delivery_verify(call, operation_id, refresh),
            "engine_project_mutate" => self.invoke_mutation(call, operation_id, refresh, grant),
            "engine_project_rollback" => self.invoke_rollback(call, refresh),
            "engine_evidence_read" => self.invoke_evidence_read(call, refresh),
            "engine_operation_observe" => self.invoke_operation_observe(call),
            "engine_operation_cancel" => self.invoke_operation_cancel(call),
            _ => Err(diagnostic(
                "engine_provider.tool_not_ready",
                "This concrete Engine tool is registered but not ready on the Headless provider.",
                "Use a ready tool or complete the required Headless execution adapter.",
            )),
        }
    }

    fn invoke_project_run(
        &mut self,
        call: &HostToolCall,
        operation_id: &str,
        refresh: Option<&RefreshReport>,
    ) -> Result<InvocationSuccess, ToolDiagnostic> {
        let input: ProjectRunInput = serde_json::from_value(call.arguments.clone())
            .map_err(|error| execution_input_diagnostic("run", error))?;
        if input.mode.as_deref().unwrap_or("headless") != "headless" {
            return Err(diagnostic(
                "engine_provider.run_mode_unsupported",
                "The qualified No-Editor run tool currently supports only headless mode.",
                "Use mode=headless.",
            ));
        }
        let refresh = required_refresh(refresh)?;
        let frame_limit = input.frame_limit.unwrap_or(3).max(1);
        let timeout_ms = input.timeout_ms.unwrap_or(30_000).max(1);
        let (project_root, report, preparation_summary, compiler, scenario) = {
            let session = self.project_mut()?;
            let project_root = session.project_root().to_path_buf();
            let (lease, compiler) = compiler_for_operation(session, operation_id, refresh)?;
            let prepared = compiler
                .prepare_with_artifact_cache(
                    &lease,
                    TargetProfile::WindowsDev,
                    &project_root.join("Library/CompilerCache"),
                )
                .map_err(compiler_diagnostic)?;
            let output = engine_tool_output(operation_id, "Run")?;
            let player_build_root = default_project_runtime_player_build_root();
            let report = compiler
                .run(
                    &prepared,
                    RunOptions::new(ExecutionMode::Headless, frame_limit, timeout_ms)
                        .with_project_delivery(&project_root, output, player_build_root),
                )
                .map_err(compiler_diagnostic)?;
            let scenario = prepared.load_playtest_scenario().ok();
            (
                project_root,
                report,
                prepared.prepare_summary(),
                compiler,
                scenario,
            )
        };
        let delivery_ref = opaque_delivery_ref(report.delivery());
        self.deliveries.insert(
            delivery_ref.clone(),
            RetainedDelivery {
                delivery: report.delivery().clone(),
                compiler,
                scenario,
            },
        );
        let desktop = report.desktop_export();
        Ok(InvocationSuccess {
            revision: Some(report.lineage().revision_id().to_string()),
            output: json!({
                "mode": "headless",
                "preparationIdentity": report.preparation_identity(),
                "prepareSummary": preparation_summary,
                "deliveryRef": delivery_ref,
                "deliveryIdentity": report.delivery().delivery_identity(),
                "artifactIdentity": report.delivery().artifact_identity(),
                "projectRoot": project_root,
                "packageDir": report.delivery().package_dir(),
                "playerExitCode": desktop.player_exit_code,
                "playerExitReason": desktop.player_exit_reason,
            }),
            receipt_ref: None,
            evidence_refs: report.delivery().evidence_refs().to_vec(),
        })
    }

    fn invoke_project_check(
        &mut self,
        call: &HostToolCall,
        operation_id: &str,
        refresh: Option<&RefreshReport>,
    ) -> Result<InvocationSuccess, ToolDiagnostic> {
        let refresh = required_refresh(refresh)?;
        let session = self.project_mut()?;
        let (lease, compiler) = compiler_for_operation(session, operation_id, refresh)?;
        let report = compiler
            .check(
                &lease,
                project_authoring_execution::CheckProfile::new(
                    project_authoring_execution::TargetProfile::WindowsDev,
                )
                .with_process_approval(call.approved),
            )
            .map_err(compiler_diagnostic)?;
        Ok(InvocationSuccess::new(
            report.revision_id().to_string(),
            json!({
                "projectIdentity": report.project_identity(),
                "targetProfile": report.target_profile().as_str(),
                "sourceFileCount": report.source_file_count(),
                "qualification": "ready",
                "checkReport": report
            }),
        ))
    }

    fn invoke_project_build(
        &mut self,
        call: &HostToolCall,
        operation_id: &str,
        refresh: Option<&RefreshReport>,
    ) -> Result<InvocationSuccess, ToolDiagnostic> {
        let input: ProjectBuildInput = serde_json::from_value(call.arguments.clone())
            .map_err(|error| execution_input_diagnostic("build", error))?;
        if input.target_profile.as_deref().unwrap_or("windows-dev") != "windows-dev" {
            return Err(diagnostic(
                "engine_provider.build_target_not_ready",
                "Only the existing Windows Dev delivery target is ready on this Provider.",
                "Use targetProfile=windows-dev.",
            ));
        }
        let refresh = required_refresh(refresh)?;
        let frame_limit = input.frame_limit.unwrap_or(3).max(1);
        let (report, preparation_summary, compiler, scenario) = {
            let session = self.project_mut()?;
            let project_root = session.project_root().to_path_buf();
            let (lease, compiler) = compiler_for_operation(session, operation_id, refresh)?;
            let prepare_started = std::time::Instant::now();
            let prepared = compiler
                .prepare_with_artifact_cache(
                    &lease,
                    TargetProfile::WindowsDev,
                    &project_root.join("Library/CompilerCache"),
                )
                .map_err(compiler_diagnostic)?;
            let prepare_duration_ms = prepare_started.elapsed().as_secs_f64() * 1000.0;
            let mut preparation_summary = prepared.prepare_summary();
            // Only Compiler preparation; excludes refresh, Player build and export.
            preparation_summary["durationMs"] = json!(prepare_duration_ms);
            let output = engine_tool_output(operation_id, "Build")?;
            let player_build_root = default_project_runtime_player_build_root();
            let report = compiler
                .build(
                    &prepared,
                    BuildRequest::for_project(
                        TargetProfile::WindowsDev,
                        &project_root,
                        output,
                        player_build_root,
                    )
                    .with_frame_limit(frame_limit)
                    .with_player_verification(false),
                )
                .map_err(compiler_diagnostic)?;
            let scenario = prepared.load_playtest_scenario().ok();
            (report, preparation_summary, compiler, scenario)
        };
        let delivery_ref = opaque_delivery_ref(report.delivery());
        self.deliveries.insert(
            delivery_ref.clone(),
            RetainedDelivery {
                delivery: report.delivery().clone(),
                compiler,
                scenario,
            },
        );
        let desktop = report.desktop_export();
        Ok(InvocationSuccess {
            revision: Some(report.delivery().lineage().revision_id().to_string()),
            output: json!({
                "targetProfile": "windows-dev",
                "prepareSummary": preparation_summary,
                "deliveryRef": delivery_ref,
                "deliveryIdentity": report.delivery().delivery_identity(),
                "artifactIdentity": report.delivery().artifact_identity(),
                "packageDir": report.delivery().package_dir(),
                "runtimePackageDir": desktop.runtime_package_dir,
                "playerExecutable": desktop.player_executable,
                "status": desktop.status,
            }),
            receipt_ref: None,
            evidence_refs: report.delivery().evidence_refs().to_vec(),
        })
    }

    fn invoke_delivery_verify(
        &mut self,
        call: &HostToolCall,
        operation_id: &str,
        refresh: Option<&RefreshReport>,
    ) -> Result<InvocationSuccess, ToolDiagnostic> {
        let input: DeliveryVerifyInput = serde_json::from_value(call.arguments.clone())
            .map_err(|error| execution_input_diagnostic("delivery verification", error))?;
        let mode = match input.mode.as_deref().unwrap_or("headless") {
            "headless" => ExecutionMode::Headless,
            "windowed" => ExecutionMode::Windowed,
            _ => {
                return Err(diagnostic(
                    "engine_provider.delivery_verify_mode_unsupported",
                    "Delivery verification supports headless or windowed mode.",
                    "Use windowed for GPU presentation; headless remains the default.",
                ))
            }
        };
        let delivery = self
            .deliveries
            .get(&input.delivery_ref)
            .map(|retained| retained.delivery.clone())
            .ok_or_else(|| {
                diagnostic(
                    "engine_provider.delivery_ref_unknown",
                    "Delivery reference is unknown in this Provider session.",
                    "Use the opaque deliveryRef returned by engine_runtime_run or engine_project_build.",
                )
            })?;
        let refresh = required_refresh(refresh)?;
        let report = {
            let session = self.project_mut()?;
            let (_lease, compiler) = compiler_for_operation(session, operation_id, refresh)?;
            compiler
                .verify(
                    &delivery,
                    VerifyRequest::new(
                        mode,
                        input.frame_limit.unwrap_or(3).max(1),
                        input.timeout_ms.unwrap_or(30_000).max(1),
                        input.screenshot.unwrap_or(false),
                    ),
                )
                .map_err(compiler_diagnostic)?
        };
        let process = report.process_verification();
        let mut evidence_refs = report.delivery().evidence_refs().to_vec();
        if !evidence_refs.contains(&process.child_report_path) {
            evidence_refs.push(process.child_report_path.clone());
        }
        evidence_refs.push(
            report
                .delivery()
                .package_dir()
                .join("reports/exported-player-process-verification-report.json")
                .display()
                .to_string(),
        );
        Ok(InvocationSuccess {
            revision: Some(report.delivery().lineage().revision_id().to_string()),
            output: json!({
                "deliveryRef": input.delivery_ref,
                "deliveryIdentity": report.delivery().delivery_identity(),
                "artifactIdentity": report.delivery().artifact_identity(),
                "status": process.status,
                "processExitCode": process.process_exit_code,
                "childPlayerExitCode": process.child_player_exit_code,
                "childFramesCompleted": process.child_frames_completed,
                "processExitReason": process.process_exit_reason,
            }),
            receipt_ref: None,
            evidence_refs,
        })
    }

    fn invoke_mutation(
        &mut self,
        call: &HostToolCall,
        operation_id: &str,
        refresh: Option<&RefreshReport>,
        grant: &EngineCapabilityGrant,
    ) -> Result<InvocationSuccess, ToolDiagnostic> {
        let input: MutationInput =
            serde_json::from_value(call.arguments.clone()).map_err(|error| {
                diagnostic(
                    "engine_provider.mutation_input_invalid",
                    format!("Mutation input is invalid: {error}"),
                    "Provide goal and a bounded changes array.",
                )
            })?;
        if input.changes.is_empty() || input.changes.len() > 32 {
            return Err(diagnostic(
                "engine_provider.mutation_change_count_invalid",
                "Mutation must contain 1-32 changes.",
                "Split or regenerate the mutation.",
            ));
        }
        let refresh = required_refresh(refresh)?;
        if grant.project_revision.as_deref() != Some(refresh.revision.revision_id.as_str()) {
            return Err(revision_changed_diagnostic());
        }
        if input.changes.len() > grant.max_mutation_count as usize {
            return Err(diagnostic(
                "engine_provider.grant_mutation_budget_exceeded",
                "Mutation exceeds its operation-bound Engine Grant.",
                "Regenerate a bounded mutation call.",
            ));
        }
        let session = self.project_mut()?;
        let mut write_set = Vec::new();
        let mut operations = Vec::new();
        for change in input.changes {
            match change {
                MutationChange::CreateOrReplace { path, content } => {
                    validate_relative_path(&path)?;
                    write_set.push(path.clone());
                    operations.push(ProjectMutationOperation::CreateOrReplace {
                        path,
                        bytes: content.into_bytes(),
                    });
                }
                MutationChange::Delete { path } => {
                    validate_relative_path(&path)?;
                    write_set.push(path.clone());
                    operations.push(ProjectMutationOperation::Delete { path });
                }
                MutationChange::Move { from, to } => {
                    validate_relative_path(&from)?;
                    validate_relative_path(&to)?;
                    write_set.push(from.clone());
                    write_set.push(to.clone());
                    operations.push(ProjectMutationOperation::Move { from, to });
                }
            }
        }
        write_set.sort();
        write_set.dedup();
        let before = session
            .capture_mutation_before(&write_set)
            .map_err(context_diagnostic)?;
        let validation_digest =
            sha256_prefixed(&serde_json::to_vec(&call.arguments).map_err(|error| {
                diagnostic(
                    "engine_provider.mutation_input_invalid",
                    error.to_string(),
                    "Regenerate the mutation input.",
                )
            })?);
        let receipt = session
            .commit_mutation(ProjectMutation {
                schema_version: PROJECT_MUTATION_SCHEMA_VERSION.to_string(),
                mutation_id: operation_id.to_string(),
                domain: input.domain.unwrap_or_else(|| "project".to_string()),
                expected_revision_id: refresh.revision.revision_id.clone(),
                validation_digest,
                declared_read_set: Vec::new(),
                declared_write_set: write_set,
                expected_before: before,
                operations,
            })
            .map_err(context_diagnostic)?;
        let rollback_ref = format!("engine-rollback:{}", receipt.receipt_id);
        self.rollback_receipts
            .insert(rollback_ref.clone(), receipt.clone());
        Ok(InvocationSuccess {
            revision: Some(receipt.after_revision.revision_id.clone()),
            output: json!({
                "goal": input.goal,
                "beforeRevision": receipt.before_revision.revision_id,
                "afterRevision": receipt.after_revision.revision_id,
                "changedPaths": receipt.changed_paths,
                "receiptId": receipt.receipt_id,
            }),
            receipt_ref: Some(rollback_ref),
            evidence_refs: vec![format!("project-evidence:{}", receipt.receipt_path)],
        })
    }

    fn invoke_rollback(
        &mut self,
        call: &HostToolCall,
        refresh: Option<&RefreshReport>,
    ) -> Result<InvocationSuccess, ToolDiagnostic> {
        let rollback_ref = required_string(&call.arguments, "rollbackRef")?.to_string();
        let receipt = self
            .rollback_receipts
            .remove(&rollback_ref)
            .ok_or_else(|| {
                diagnostic(
                    "engine_provider.rollback_ref_unknown",
                    "Rollback reference is unknown or already consumed in this session.",
                    "Use the opaque rollbackRef returned by engine_project_mutate.",
                )
            })?;
        let session = self.project_mut()?;
        let refresh = required_refresh(refresh)?;
        if session.revision().map_err(context_diagnostic)?.revision_id
            != refresh.revision.revision_id
        {
            return Err(revision_changed_diagnostic());
        }
        let rollback = session
            .rollback_mutation(&receipt)
            .map_err(context_diagnostic)?;
        Ok(InvocationSuccess::new(
            rollback.restored_revision.revision_id.clone(),
            json!({
                "sourceReceiptId": rollback.source_receipt_id,
                "restoredRevision": rollback.restored_revision.revision_id,
                "replacedRevision": rollback.replaced_revision.revision_id,
                "changedPaths": rollback.changed_paths,
            }),
        ))
    }

    fn invoke_evidence_read(
        &mut self,
        call: &HostToolCall,
        refresh: Option<&RefreshReport>,
    ) -> Result<InvocationSuccess, ToolDiagnostic> {
        let path = required_string(&call.arguments, "path")?;
        let relative = validate_relative_path(path)?;
        let allowed = relative.starts_with("Library/Reports/")
            || relative.starts_with("Library/EngineTools/");
        if !allowed {
            return Err(diagnostic(
                "engine_provider.evidence_scope_rejected",
                "Evidence reads are limited to Library/Reports and Library/EngineTools.",
                "Pass a project-contained Engine evidence path.",
            ));
        }
        let session = self.project_mut()?;
        let refresh = required_refresh(refresh)?;
        let absolute = contained_join(session.project_root(), &relative)?;
        let bytes = fs::read(&absolute).map_err(|error| {
            diagnostic(
                "engine_provider.evidence_read_failed",
                format!("Evidence could not be read: {error}"),
                "Use an existing bounded evidence path.",
            )
        })?;
        if bytes.len() > 1024 * 1024 {
            return Err(diagnostic(
                "engine_provider.evidence_too_large",
                "Evidence exceeds the 1 MiB tool result limit.",
                "Read a bounded summary evidence artifact.",
            ));
        }
        Ok(InvocationSuccess {
            revision: Some(refresh.revision.revision_id.clone()),
            output: json!({
                "path": relative,
                "length": bytes.len(),
                "contentDigest": sha256_prefixed(&bytes),
                "text": String::from_utf8_lossy(&bytes),
            }),
            receipt_ref: None,
            evidence_refs: vec![format!("project-evidence:{relative}")],
        })
    }

    fn invoke_operation_observe(
        &self,
        call: &HostToolCall,
    ) -> Result<InvocationSuccess, ToolDiagnostic> {
        let operation_id = required_string(&call.arguments, "operationId")?;
        let snapshot = self.observe(operation_id).ok_or_else(|| {
            diagnostic(
                "engine_provider.operation_unknown",
                "Operation id is not present in this provider session.",
                "Use an operation id returned by this session.",
            )
        })?;
        Ok(InvocationSuccess::without_revision(
            json!({"operation": snapshot}),
        ))
    }

    fn invoke_operation_cancel(
        &mut self,
        call: &HostToolCall,
    ) -> Result<InvocationSuccess, ToolDiagnostic> {
        let operation_id = required_string(&call.arguments, "operationId")?.to_string();
        let receipt = self.cancel(&operation_id);
        if receipt.status == CancellationStatus::NotFound {
            return Err(diagnostic(
                "engine_provider.operation_unknown",
                "Operation id is not present in this provider session.",
                "Use an operation id returned by this session.",
            ));
        }
        Ok(InvocationSuccess::without_revision(
            json!({"cancellation": receipt}),
        ))
    }
}

impl NativeEngineToolProvider for EngineToolProvider {
    fn tool_definitions(&self) -> Vec<ToolDefinition> {
        tool_definitions_for(ToolAudience::InternalDiagnostics)
    }

    fn invoke(&mut self, call: HostToolCall) -> CanonicalToolResult {
        let call_digest = call_digest(&call);
        if let Some(existing_operation_id) = self.replay_operations.get(&call.call_id) {
            let existing = self
                .operations
                .get(existing_operation_id)
                .expect("replay index must reference an operation");
            if existing.call_digest == call_digest {
                if let Some(mut result) = existing.result.clone() {
                    result.replayed = true;
                    return result;
                }
            } else {
                let operation_id = self.next_operation_id();
                let grant = self.initial_grant(&call);
                self.start_operation(&call, &operation_id, call_digest, grant, false);
                return self.finish(
                    &call,
                    operation_id,
                    CanonicalToolStatus::RejectedByEngine,
                    None,
                    Value::Null,
                    vec![diagnostic(
                        "engine_provider.call_replay_mismatch",
                        "Call id was reused with different tool input or approval facts.",
                        "Use a new call id for the changed invocation.",
                    )],
                    None,
                    Vec::new(),
                );
            }
        }
        let operation_id = self.next_operation_id();
        let grant = self.initial_grant(&call);
        self.start_operation(&call, &operation_id, call_digest, grant, true);
        let definition = tool_definitions()
            .into_iter()
            .find(|definition| definition.name == call.tool_name);
        let Some(definition) = definition else {
            return self.finish(
                &call,
                operation_id,
                CanonicalToolStatus::RejectedByEngine,
                None,
                Value::Null,
                vec![diagnostic(
                    "engine_provider.tool_unknown",
                    "Tool is not registered by this provider.",
                    "Choose a concrete tool returned by tools/list.",
                )],
                None,
                Vec::new(),
            );
        };
        if definition.maturity != ToolMaturity::Ready {
            return self.finish(
                &call,
                operation_id,
                CanonicalToolStatus::UnsupportedOnAdapter,
                None,
                Value::Null,
                vec![diagnostic(
                    "engine_provider.tool_not_ready",
                    "Tool has no qualified Headless execution adapter.",
                    "Use a ready tool or complete the declared Engine capability.",
                )],
                None,
                Vec::new(),
            );
        }
        if definition.side_effect != ToolSideEffect::Read && !call.approved {
            return self.finish(
                &call,
                operation_id,
                CanonicalToolStatus::RejectedByHost,
                None,
                Value::Null,
                vec![diagnostic(
                    "engine_provider.host_approval_required",
                    "Host approval is required for this side effect.",
                    "Approve the concrete tool call in the host.",
                )],
                None,
                Vec::new(),
            );
        }
        if let Err(error) = validate_tool_input(&call.tool_name, &call.arguments) {
            return self.finish(
                &call,
                operation_id,
                CanonicalToolStatus::RejectedByEngine,
                None,
                Value::Null,
                vec![error],
                None,
                Vec::new(),
            );
        }
        let refresh = match self.prepare_project_refresh(&call) {
            Ok(refresh) => refresh,
            Err(error) => {
                return self.finish(
                    &call,
                    operation_id,
                    CanonicalToolStatus::RejectedByEngine,
                    None,
                    Value::Null,
                    vec![error],
                    None,
                    Vec::new(),
                );
            }
        };
        let grant = match self.issue_grant(&call, &definition, refresh.as_ref()) {
            Ok(grant) => grant,
            Err(error) => {
                return self.finish(
                    &call,
                    operation_id,
                    CanonicalToolStatus::RejectedByEngine,
                    refresh
                        .as_ref()
                        .map(|report| report.revision.revision_id.clone()),
                    Value::Null,
                    vec![error],
                    None,
                    Vec::new(),
                );
            }
        };
        self.operations
            .get_mut(&operation_id)
            .expect("started operation")
            .grant = grant.clone();
        match self.invoke_ready(&call, &operation_id, refresh.as_ref(), &grant) {
            Ok(success) => {
                let outcome_failed = call.tool_name == "engine_runtime_playtest"
                    && success.output["overall"] != "passed";
                let diagnostics = if outcome_failed {
                    vec![diagnostic("engine_provider.playtest_outcome_not_passed",
                        "One or more required semantic outcomes did not pass; retained evidence describes the result.",
                        "Inspect outcome and assertions; correct the project or scenario before explicitly retrying.")]
                } else {
                    Vec::new()
                };
                self.finish(
                    &call,
                    operation_id,
                    if outcome_failed {
                        CanonicalToolStatus::Failed
                    } else {
                        CanonicalToolStatus::Completed
                    },
                    success.revision,
                    success.output,
                    diagnostics,
                    success.receipt_ref,
                    success.evidence_refs,
                )
            }
            Err(error) => self.finish(
                &call,
                operation_id,
                CanonicalToolStatus::RejectedByEngine,
                self.project
                    .as_ref()
                    .and_then(|session| session.revision().ok())
                    .map(|revision| revision.revision_id),
                Value::Null,
                vec![error],
                None,
                Vec::new(),
            ),
        }
    }

    fn observe(&self, operation_id: &str) -> Option<OperationSnapshot> {
        self.operations
            .get(operation_id)
            .map(|operation| OperationSnapshot {
                operation_id: operation_id.to_string(),
                state: operation.state,
                terminal: operation.state.is_terminal(),
                result: operation.result.clone(),
            })
    }

    fn cancel(&mut self, operation_id: &str) -> CancellationReceipt {
        let status = match self.operations.get_mut(operation_id) {
            None => CancellationStatus::NotFound,
            Some(operation) if operation.state.is_terminal() => CancellationStatus::AlreadyTerminal,
            Some(operation) => {
                operation.state = OperationState::Cancelled;
                operation.result = Some(CanonicalToolResult {
                    schema_version: ENGINE_TOOL_RESULT_SCHEMA_VERSION.to_string(),
                    call_id: operation.grant.call_id.clone(),
                    tool_name: operation.grant.tool_name.clone(),
                    status: CanonicalToolStatus::Cancelled,
                    operation_id: operation_id.to_string(),
                    project_revision: operation.grant.project_revision.clone(),
                    output: Value::Null,
                    diagnostics: vec![diagnostic(
                        "engine_provider.operation_cancelled",
                        "Operation was cancelled before reaching a terminal result.",
                        "Start a new tool call if the operation is still required.",
                    )],
                    retryability: ToolRetryability::NotRetryable,
                    recommended_local_transitions: Vec::new(),
                    receipt_ref: None,
                    evidence_refs: Vec::new(),
                    replayed: false,
                });
                CancellationStatus::Cancelled
            }
        };
        CancellationReceipt {
            operation_id: operation_id.to_string(),
            status,
        }
    }
}

#[derive(Debug)]
struct InvocationSuccess {
    revision: Option<String>,
    output: Value,
    receipt_ref: Option<String>,
    evidence_refs: Vec<String>,
}

impl InvocationSuccess {
    fn new(revision: String, output: Value) -> Self {
        Self {
            revision: Some(revision),
            output,
            receipt_ref: None,
            evidence_refs: Vec::new(),
        }
    }

    fn without_revision(output: Value) -> Self {
        Self {
            revision: None,
            output,
            receipt_ref: None,
            evidence_refs: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MutationInput {
    goal: String,
    #[serde(default)]
    domain: Option<String>,
    changes: Vec<MutationChange>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectRunInput {
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    frame_limit: Option<u64>,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectBuildInput {
    #[serde(default)]
    target_profile: Option<String>,
    #[serde(default)]
    frame_limit: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeliveryVerifyInput {
    delivery_ref: String,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    frame_limit: Option<u64>,
    #[serde(default)]
    timeout_ms: Option<u64>,
    #[serde(default)]
    screenshot: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum MutationChange {
    CreateOrReplace { path: String, content: String },
    Delete { path: String },
    Move { from: String, to: String },
}

pub fn tool_definitions() -> Vec<ToolDefinition> {
    let object = |properties: Value, required: &[&str]| {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": properties,
            "required": required,
        })
    };
    let mut definitions = vec![
        definition(
            "engine_project_open",
            "Bind one workspace-contained AI First Engine project.",
            ToolSideEffect::Read,
            ToolMaturity::Ready,
            object(json!({"projectRoot":{"type":"string"}}), &["projectRoot"]),
        ),
        definition(
            "engine_project_inspect",
            "Inspect current canonical project facts and Context revision.",
            ToolSideEffect::Read,
            ToolMaturity::Ready,
            object(json!({}), &[]),
        ),
        definition(
            "engine_project_search",
            "Search bounded canonical source text.",
            ToolSideEffect::Read,
            ToolMaturity::Ready,
            query_schema(),
        ),
        definition(
            "engine_project_read_object",
            "Read one canonical source object through a Context snapshot.",
            ToolSideEffect::Read,
            ToolMaturity::Ready,
            object(json!({"path":{"type":"string"}}), &["path"]),
        ),
        definition(
            "engine_project_references",
            "Find bounded textual references in canonical source.",
            ToolSideEffect::Read,
            ToolMaturity::Ready,
            query_schema(),
        ),
        definition(
            "engine_project_source_symbols",
            "Find source declarations matching a query.",
            ToolSideEffect::Read,
            ToolMaturity::Ready,
            query_schema(),
        ),
        definition(
            "engine_project_diagnostics",
            "Refresh and return canonical project qualification diagnostics.",
            ToolSideEffect::Read,
            ToolMaturity::Ready,
            object(json!({}), &[]),
        ),
        definition(
            "engine_project_check",
            "Check the canonical project snapshot for the Windows Dev target.",
            ToolSideEffect::ProcessSpawn,
            ToolMaturity::Ready,
            object(json!({}), &[]),
        ),
        definition(
            "engine_project_mutate",
            "Apply a bounded structured project mutation with Context CAS.",
            ToolSideEffect::Write,
            ToolMaturity::Ready,
            mutation_schema(),
        ),
        definition(
            "engine_project_rollback",
            "Rollback an exact mutation receipt using its opaque rollback reference.",
            ToolSideEffect::Write,
            ToolMaturity::Ready,
            object(json!({"rollbackRef":{"type":"string"}}), &["rollbackRef"]),
        ),
        definition(
            "engine_evidence_read",
            "Read bounded Engine evidence from a project-contained evidence root.",
            ToolSideEffect::Read,
            ToolMaturity::Ready,
            object(json!({"path":{"type":"string"}}), &["path"]),
        ),
        definition(
            "engine_ui_locate",
            "Locate an AUI or Scene UI object by semantic query.",
            ToolSideEffect::Read,
            ToolMaturity::Ready,
            query_schema(),
        ),
        definition(
            "engine_ui_explain_visibility",
            "Find UI visibility facts matching a semantic query.",
            ToolSideEffect::Read,
            ToolMaturity::Ready,
            query_schema(),
        ),
        definition(
            "engine_project_trace_ui_owner",
            "Trace project source that owns a matching UI semantic.",
            ToolSideEffect::Read,
            ToolMaturity::Ready,
            query_schema(),
        ),
        definition(
            "engine_operation_observe",
            "Observe one operation owned by this provider session.",
            ToolSideEffect::Read,
            ToolMaturity::Ready,
            operation_schema(),
        ),
        definition(
            "engine_operation_cancel",
            "Cancel one non-terminal operation owned by this provider session.",
            ToolSideEffect::Write,
            ToolMaturity::Ready,
            operation_schema(),
        ),
        definition(
            "engine_runtime_run",
            "Prepare and run the current project through the qualified Headless Runtime path.",
            ToolSideEffect::ProcessSpawn,
            ToolMaturity::Ready,
            run_schema(),
        ),
        definition(
            "engine_project_build",
            "Build a Windows Dev delivery from the current canonical project revision.",
            ToolSideEffect::ProcessSpawn,
            ToolMaturity::Ready,
            build_schema(),
        ),
        definition(
            "engine_runtime_playtest",
            "Run the project's bounded semantic scenario, or retest a retained delivery without rebuilding live source.",
            ToolSideEffect::ProcessSpawn,
            ToolMaturity::Ready,
            semantic::playtest_schema(),
        ),
        definition(
            "engine_runtime_observe",
            "Read a retained run's semantic and capture evidence without executing or selecting a latest run.",
            ToolSideEffect::Read,
            ToolMaturity::Ready,
            semantic::observe_schema(),
        ),
        definition(
            "engine_delivery_verify",
            "Verify an opaque delivery produced by this Provider session. Defaults to headless; use mode=windowed for GPU presentation and screenshots.",
            ToolSideEffect::ProcessSpawn,
            ToolMaturity::Ready,
            delivery_verify_schema(),
        ),
    ];
    for (name, description, side_effect) in [
        (
            "engine_project_create",
            "Create a minimal engine project.",
            ToolSideEffect::Write,
        ),
        (
            "engine_runtime_capture_issue",
            "Capture a bounded runtime issue evidence bundle.",
            ToolSideEffect::ProcessSpawn,
        ),
    ] {
        definitions.push(definition(
            name,
            description,
            side_effect,
            ToolMaturity::Unavailable,
            object(json!({}), &[]),
        ));
    }
    definitions
}

fn definition(
    name: &str,
    description: &str,
    side_effect: ToolSideEffect,
    maturity: ToolMaturity,
    input_schema: Value,
) -> ToolDefinition {
    let required_capabilities = match side_effect {
        ToolSideEffect::Read => {
            if name.starts_with("engine_operation_") || name == "engine_project_open" {
                Vec::new()
            } else {
                vec![ToolCapability::ReadProject]
            }
        }
        ToolSideEffect::Write => vec![ToolCapability::MutateProject],
        ToolSideEffect::ProcessSpawn => vec![
            ToolCapability::ReadProject,
            ToolCapability::GenerateFiles,
            ToolCapability::SpawnProcess,
        ],
    };
    let duration = match side_effect {
        ToolSideEffect::Read => ToolDuration::Instant,
        ToolSideEffect::Write => ToolDuration::Short,
        ToolSideEffect::ProcessSpawn => ToolDuration::Long,
    };
    ToolDefinition {
        schema_version: ENGINE_TOOL_DEFINITION_SCHEMA_VERSION.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        side_effect,
        maturity,
        input_schema,
        output_schema: canonical_result_schema(),
        required_capabilities,
        risk: if side_effect == ToolSideEffect::Read {
            ToolRisk::Low
        } else {
            ToolRisk::Elevated
        },
        duration,
        supports_cancellation: false,
        supports_rollback: name == "engine_project_mutate",
    }
}

pub(crate) fn tool_definitions_for(audience: ToolAudience) -> Vec<ToolDefinition> {
    tool_definitions()
        .into_iter()
        .filter(|definition| tool_is_exposed(definition, audience))
        .collect()
}

fn tool_is_exposed(definition: &ToolDefinition, audience: ToolAudience) -> bool {
    if audience == ToolAudience::ModelDefault && definition.maturity != ToolMaturity::Ready {
        return false;
    }
    match audience {
        ToolAudience::InternalDiagnostics => true,
        ToolAudience::ModelDefault => matches!(
            definition.name.as_str(),
            "engine_project_inspect"
                | "engine_project_check"
                | "engine_project_mutate"
                | "engine_project_rollback"
                | "engine_runtime_run"
                | "engine_runtime_playtest"
                | "engine_runtime_observe"
                | "engine_project_build"
                | "engine_delivery_verify"
        ),
    }
}

fn query_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"properties":{"query":{"type":"string","minLength":1,"maxLength":256}},"required":["query"]})
}

fn operation_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"properties":{"operationId":{"type":"string","minLength":1,"maxLength":256}},"required":["operationId"]})
}

fn run_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "properties":{
            "mode":{"type":"string","enum":["headless"],"default":"headless"},
            "frameLimit":{"type":"integer","minimum":1,"maximum":10000,"default":3},
            "timeoutMs":{"type":"integer","minimum":1,"maximum":300000,"default":30000}
        },
        "required":[]
    })
}

fn build_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "properties":{
            "targetProfile":{"type":"string","enum":["windows-dev"],"default":"windows-dev"},
            "frameLimit":{"type":"integer","minimum":1,"maximum":10000,"default":3}
        },
        "required":[]
    })
}

fn delivery_verify_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "properties":{
            "deliveryRef":{"type":"string","minLength":1,"maxLength":256},
            "mode":{"type":"string","enum":["headless","windowed"],"default":"headless"},
            "frameLimit":{"type":"integer","minimum":1,"maximum":10000,"default":3},
            "timeoutMs":{"type":"integer","minimum":1,"maximum":300000,"default":30000},
            "screenshot":{"type":"boolean","default":false}
        },
        "required":["deliveryRef"]
    })
}

fn canonical_result_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["schemaVersion","callId","toolName","status","operationId","output","diagnostics","retryability","recommendedLocalTransitions","evidenceRefs","replayed"]
    })
}

fn mutation_schema() -> Value {
    json!({
        "type":"object","additionalProperties":false,
        "properties":{
            "goal":{"type":"string","minLength":1,"maxLength":512},
            "domain":{"type":"string","minLength":1,"maxLength":64},
            "changes":{"type":"array","minItems":1,"maxItems":32,"items":{"oneOf":[
                {"type":"object","additionalProperties":false,"properties":{"operation":{"const":"create_or_replace"},"path":{"type":"string"},"content":{"type":"string"}},"required":["operation","path","content"]},
                {"type":"object","additionalProperties":false,"properties":{"operation":{"const":"delete"},"path":{"type":"string"}},"required":["operation","path"]},
                {"type":"object","additionalProperties":false,"properties":{"operation":{"const":"move"},"from":{"type":"string"},"to":{"type":"string"}},"required":["operation","from","to"]}
            ]}}
        },
        "required":["goal","changes"]
    })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyInput {}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectOpenInput {
    project_root: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct QueryInput {
    query: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PathInput {
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RollbackInput {
    rollback_ref: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OperationInput {
    operation_id: String,
}

fn validate_tool_input(tool_name: &str, arguments: &Value) -> Result<(), ToolDiagnostic> {
    let valid = match tool_name {
        "engine_runtime_playtest" => semantic::validate_playtest_input(arguments),
        "engine_runtime_observe" => semantic::validate_observe_input(arguments),
        "engine_project_open" => serde_json::from_value::<ProjectOpenInput>(arguments.clone())
            .map(|input| !input.project_root.trim().is_empty()),
        "engine_project_inspect" => {
            serde_json::from_value::<EmptyInput>(arguments.clone()).map(|_| true)
        }
        "engine_project_search"
        | "engine_project_references"
        | "engine_project_source_symbols"
        | "engine_ui_locate"
        | "engine_ui_explain_visibility"
        | "engine_project_trace_ui_owner" => {
            serde_json::from_value::<QueryInput>(arguments.clone())
                .map(|input| !input.query.trim().is_empty() && input.query.len() <= 256)
        }
        "engine_project_read_object" | "engine_evidence_read" => {
            serde_json::from_value::<PathInput>(arguments.clone())
                .map(|input| !input.path.trim().is_empty())
        }
        "engine_project_diagnostics" | "engine_project_check" => {
            serde_json::from_value::<EmptyInput>(arguments.clone()).map(|_| true)
        }
        "engine_project_mutate" => {
            serde_json::from_value::<MutationInput>(arguments.clone()).map(|input| {
                !input.goal.trim().is_empty()
                    && input.goal.len() <= 512
                    && !input.changes.is_empty()
                    && input.changes.len() <= 32
            })
        }
        "engine_project_rollback" => serde_json::from_value::<RollbackInput>(arguments.clone())
            .map(|input| !input.rollback_ref.trim().is_empty()),
        "engine_operation_observe" | "engine_operation_cancel" => {
            serde_json::from_value::<OperationInput>(arguments.clone())
                .map(|input| !input.operation_id.trim().is_empty())
        }
        _ => return Ok(()),
    };
    match valid {
        Ok(true) => Ok(()),
        Ok(false) => Err(diagnostic(
            "engine_provider.input_constraint_failed",
            "Tool input does not satisfy its bounded contract.",
            "Regenerate the call from the concrete tool schema.",
        )),
        Err(error) => Err(diagnostic(
            "engine_provider.input_schema_invalid",
            format!("Tool input does not match its schema: {error}"),
            "Regenerate the call from the concrete tool schema.",
        )),
    }
}

fn mutation_change_count(tool_name: &str, arguments: &Value) -> Result<u32, ToolDiagnostic> {
    if tool_name != "engine_project_mutate" {
        return Ok(0);
    }
    let input: MutationInput = serde_json::from_value(arguments.clone()).map_err(|error| {
        diagnostic(
            "engine_provider.mutation_input_invalid",
            error.to_string(),
            "Regenerate the mutation input.",
        )
    })?;
    u32::try_from(input.changes.len()).map_err(|_| {
        diagnostic(
            "engine_provider.mutation_change_count_invalid",
            "Mutation change count exceeds the Engine Grant representation.",
            "Split the mutation.",
        )
    })
}

fn mutation_allows_delete(tool_name: &str, arguments: &Value) -> Result<bool, ToolDiagnostic> {
    if tool_name != "engine_project_mutate" {
        return Ok(false);
    }
    let input: MutationInput = serde_json::from_value(arguments.clone()).map_err(|error| {
        diagnostic(
            "engine_provider.mutation_input_invalid",
            error.to_string(),
            "Regenerate the mutation input.",
        )
    })?;
    Ok(input
        .changes
        .iter()
        .any(|change| matches!(change, MutationChange::Delete { .. })))
}

fn validate_grant(
    grant: &EngineCapabilityGrant,
    call: &HostToolCall,
    definition: &ToolDefinition,
) -> Result<(), ToolDiagnostic> {
    if grant.schema_version != ENGINE_CAPABILITY_GRANT_SCHEMA_VERSION
        || grant.session_id.trim().is_empty()
        || grant.call_id != call.call_id
        || grant.tool_name != call.tool_name
        || grant.grant_digest != grant_digest(grant)?
    {
        return Err(diagnostic(
            "engine_provider.grant_integrity_invalid",
            "Engine Grant identity or digest is invalid.",
            "Discard the call and issue a fresh operation-bound grant.",
        ));
    }
    if definition.side_effect != ToolSideEffect::Read && !call.approved {
        return Err(diagnostic(
            "engine_provider.host_approval_required",
            "Host approval is required before an Engine Grant can authorize this side effect.",
            "Approve the concrete tool call in the host.",
        ));
    }
    if definition.required_capabilities.iter().any(|capability| {
        matches!(
            capability,
            ToolCapability::ReadProject | ToolCapability::MutateProject
        )
    }) && (grant.project_identity.is_none() || grant.project_revision.is_none())
    {
        return Err(diagnostic(
            "engine_provider.grant_project_binding_missing",
            "Engine Grant requires a refreshed canonical project binding.",
            "Bind a project and retry the tool call.",
        ));
    }
    if grant.allow_delete
        && !grant
            .capabilities
            .contains(&ToolCapability::DeleteProjectContent)
    {
        return Err(diagnostic(
            "engine_provider.grant_delete_scope_missing",
            "Delete mutation is outside the Engine Grant scope.",
            "Request an explicitly approved delete operation.",
        ));
    }
    Ok(())
}

fn grant_digest(grant: &EngineCapabilityGrant) -> Result<String, ToolDiagnostic> {
    let mut unsigned = grant.clone();
    unsigned.grant_digest.clear();
    let value = serde_json::to_value(unsigned).map_err(|error| {
        diagnostic(
            "engine_provider.grant_serialization_failed",
            error.to_string(),
            "Issue a fresh Engine Grant.",
        )
    })?;
    Ok(sha256_prefixed(&canonical_json_bytes(&value)))
}

fn call_digest(call: &HostToolCall) -> String {
    sha256_prefixed(&canonical_json_bytes(&json!({
        "callId": call.call_id,
        "toolName": call.tool_name,
        "arguments": call.arguments,
        "approved": call.approved,
    })))
}

fn canonical_json_bytes(value: &Value) -> Vec<u8> {
    fn canonicalize(value: &Value) -> Value {
        match value {
            Value::Object(object) => {
                let mut keys = object.keys().collect::<Vec<_>>();
                keys.sort();
                let mut canonical = serde_json::Map::new();
                for key in keys {
                    canonical.insert(key.clone(), canonicalize(&object[key]));
                }
                Value::Object(canonical)
            }
            Value::Array(values) => Value::Array(values.iter().map(canonicalize).collect()),
            _ => value.clone(),
        }
    }
    serde_json::to_vec(&canonicalize(value)).expect("serde_json::Value must serialize")
}

fn required_refresh(refresh: Option<&RefreshReport>) -> Result<&RefreshReport, ToolDiagnostic> {
    refresh.ok_or_else(|| {
        diagnostic(
            "engine_provider.project_refresh_missing",
            "Project tool did not receive operation-bound refresh facts.",
            "Bind the project and retry the tool call.",
        )
    })
}

fn revision_changed_diagnostic() -> ToolDiagnostic {
    diagnostic(
        "engine_provider.operation_revision_changed",
        "Canonical project revision changed while preparing the operation snapshot.",
        "Retry the tool call against the latest project facts.",
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LocalFlow {
    retryability: ToolRetryability,
    transitions: Vec<RecommendedLocalTransition>,
}

fn derive_local_flow(
    tool_name: &str,
    status: CanonicalToolStatus,
    output: &Value,
    diagnostics: &[ToolDiagnostic],
    receipt_ref: Option<&str>,
) -> LocalFlow {
    let revision_drifted = diagnostics
        .iter()
        .any(|diagnostic| is_revision_drift_diagnostic(&diagnostic.code));
    let correctable = !revision_drifted
        && diagnostics.iter().any(|diagnostic| {
            is_correctable_diagnostic(&diagnostic.code)
                || (tool_name == "engine_runtime_playtest"
                    && diagnostic
                        .code
                        .starts_with("game_project_compiler.playtest")
                    && diagnostic
                        .source_location
                        .as_ref()
                        .is_some_and(|location| !location.generated))
                || (diagnostic.code == "game_project_compiler.check_cargo_failed"
                    && diagnostic.compiler_diagnostics.iter().any(|detail| {
                        detail.severity == "error"
                            && detail
                                .location
                                .as_ref()
                                .is_some_and(|location| !location.generated)
                    }))
        });
    let retryability = if status == CanonicalToolStatus::Completed {
        ToolRetryability::NotNeeded
    } else if revision_drifted {
        ToolRetryability::RetryAfterInspect
    } else if correctable {
        ToolRetryability::RetryAfterCorrection
    } else {
        ToolRetryability::NotRetryable
    };
    let mut transitions = Vec::new();
    if revision_drifted {
        transitions.push(RecommendedLocalTransition {
            category: LocalTransitionCategory::InspectLatestRevision,
            tool_name: Some("engine_project_inspect".to_string()),
            reason: "The project revision changed after the operation input was prepared."
                .to_string(),
        });
    } else if correctable {
        transitions.push(RecommendedLocalTransition {
            category: LocalTransitionCategory::RetryWithCorrection,
            tool_name: Some(tool_name.to_string()),
            reason: "The tool input or canonical project source can be corrected before retrying."
                .to_string(),
        });
    }
    if status == CanonicalToolStatus::Completed
        && tool_name == "engine_project_build"
        && output
            .get("deliveryRef")
            .and_then(Value::as_str)
            .is_some_and(|delivery_ref| !delivery_ref.is_empty())
    {
        transitions.push(RecommendedLocalTransition {
            category: LocalTransitionCategory::VerifyDelivery,
            tool_name: Some("engine_delivery_verify".to_string()),
            reason: "The completed build returned a delivery reference that can be verified."
                .to_string(),
        });
    }
    if status == CanonicalToolStatus::Completed
        && receipt_ref.is_some_and(|receipt_ref| !receipt_ref.is_empty())
    {
        transitions.push(RecommendedLocalTransition {
            category: LocalTransitionCategory::RollbackAvailable,
            tool_name: Some("engine_project_rollback".to_string()),
            reason: "The completed mutation returned a rollback receipt that can be applied conditionally."
                .to_string(),
        });
    }
    LocalFlow {
        retryability,
        transitions,
    }
}

fn is_revision_drift_diagnostic(code: &str) -> bool {
    code == "engine_provider.operation_revision_changed"
        || code == "game_project_compiler.operation_binding_mismatch"
        || code.ends_with("revision_drifted")
        || code.ends_with("revision_mismatch")
        || code.ends_with("project_drifted")
}

fn is_correctable_diagnostic(code: &str) -> bool {
    matches!(
        code,
        "engine_provider.input_constraint_failed"
            | "engine_provider.playtest_outcome_not_passed"
            | "engine_provider.input_schema_invalid"
            | "engine_provider.execution_input_invalid"
            | "engine_provider.mutation_input_invalid"
            | "engine_provider.mutation_change_count_invalid"
            | "engine_provider.query_invalid"
            | "engine_provider.input_required"
            | "engine_provider.path_invalid"
            | "game_project_compiler.manifest_missing_from_snapshot"
            | "game_project_compiler.manifest_invalid"
            | "game_project_compiler.build_profile_invalid"
            | "game_project_compiler.project_invalid"
            | "game_project_compiler.manifest_project_identity_mismatch"
            | "game_project_compiler.build_profile_target_invalid"
            | "game_project_compiler.rule_manifest_invalid"
            | "game_project_compiler.observation_contract_invalid"
            | "game_project_compiler.scene_entities_missing"
            | "game_project_compiler.source_json_invalid"
            | "game_project_compiler.source_reference_missing"
    ) || code.starts_with("game_project_compiler.animator2d.")
        || code.starts_with("game_project_compiler.prefab_")
        || code.starts_with("game_project_compiler.aui_")
}

fn operation_state_for_status(status: CanonicalToolStatus) -> OperationState {
    match status {
        CanonicalToolStatus::Completed => OperationState::Completed,
        CanonicalToolStatus::RejectedByHost
        | CanonicalToolStatus::RejectedByEngine
        | CanonicalToolStatus::UnsupportedOnAdapter => OperationState::Rejected,
        CanonicalToolStatus::Failed => OperationState::Failed,
        CanonicalToolStatus::Cancelled => OperationState::Cancelled,
    }
}

fn inspection_output(
    refresh: &authoring_project_context::RefreshReport,
    inventory: &ProjectSourceInventory,
) -> Value {
    json!({
        "projectIdentity": refresh.revision.portable_project_identity,
        "revision": refresh.revision.revision_id,
        "qualification": refresh.revision.qualification,
        "sourceDigest": inventory.source_digest,
        "files": inventory.entries,
        "diagnostics": refresh.diagnostics,
    })
}

fn search_project(
    session: &mut ProjectAuthoringSession,
    inventory: &ProjectSourceInventory,
    query: &str,
    tool_name: &str,
    operation_id: &str,
    expected_revision: &str,
) -> Result<Value, ToolDiagnostic> {
    let query_lower = query.to_ascii_lowercase();
    if query.trim().is_empty() || query.len() > 256 {
        return Err(diagnostic(
            "engine_provider.query_invalid",
            "Search query must contain 1-256 characters.",
            "Provide a bounded semantic or source query.",
        ));
    }
    let candidates = inventory
        .entries
        .iter()
        .filter(|entry| entry.length <= 1024 * 1024)
        .filter(|entry| is_text_source(&entry.relative_path))
        .map(|entry| entry.relative_path.clone())
        .take(128)
        .collect::<Vec<_>>();
    let lease = session
        .acquire_snapshot_lease(operation_id, candidates)
        .map_err(context_diagnostic)?;
    if lease.snapshot().revision.revision_id != expected_revision {
        return Err(revision_changed_diagnostic());
    }
    let mut matches = Vec::new();
    for file in &lease.snapshot().files {
        let text = String::from_utf8_lossy(&file.bytes);
        for (line_index, line) in text.lines().enumerate() {
            let declaration_only = tool_name == "engine_project_source_symbols";
            let trimmed = line.trim_start();
            let declaration = trimmed.starts_with("fn ")
                || trimmed.starts_with("pub fn ")
                || trimmed.starts_with("struct ")
                || trimmed.starts_with("pub struct ")
                || trimmed.starts_with("enum ")
                || trimmed.starts_with("pub enum ");
            if line.to_ascii_lowercase().contains(&query_lower)
                && (!declaration_only || declaration)
            {
                matches.push(json!({"path":file.relative_path,"line":line_index + 1,"preview":line.chars().take(240).collect::<String>()}));
                if matches.len() >= 100 {
                    break;
                }
            }
        }
        if matches.len() >= 100 {
            break;
        }
    }
    let truncated = matches.len() >= 100;
    let _ = lease.release();
    Ok(json!({"query":query,"matches":matches,"truncated":truncated}))
}

fn is_text_source(path: &str) -> bool {
    [
        ".rs", ".json", ".toml", ".md", ".txt", ".yaml", ".yml", ".ron",
    ]
    .iter()
    .any(|suffix| path.ends_with(suffix))
}

fn required_string<'a>(arguments: &'a Value, key: &str) -> Result<&'a str, ToolDiagnostic> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            diagnostic(
                "engine_provider.input_required",
                format!("Required string field is missing: {key}"),
                "Call the tool with its declared typed schema.",
            )
        })
}

fn validate_relative_path(path: &str) -> Result<String, ToolDiagnostic> {
    if path.trim().is_empty() {
        return Err(path_diagnostic(path));
    }
    let candidate = Path::new(path);
    if candidate.is_absolute()
        || candidate.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(path_diagnostic(path));
    }
    let normalized = candidate.to_string_lossy().replace('\\', "/");
    if normalized.starts_with("Library/AuthoringProjectContext/") {
        return Err(path_diagnostic(path));
    }
    Ok(normalized)
}

fn contained_join(root: &Path, relative: &str) -> Result<PathBuf, ToolDiagnostic> {
    let relative = validate_relative_path(relative)?;
    let path = root.join(relative);
    let parent = path.parent().unwrap_or(root);
    let canonical_parent = parent.canonicalize().map_err(|error| {
        diagnostic(
            "engine_provider.path_parent_unavailable",
            format!("Path parent cannot be resolved: {error}"),
            "Choose an existing project-contained path.",
        )
    })?;
    if !canonical_parent.starts_with(root) {
        return Err(path_diagnostic(path.to_string_lossy().as_ref()));
    }
    Ok(path)
}

fn path_diagnostic(path: &str) -> ToolDiagnostic {
    diagnostic(
        "engine_provider.path_invalid",
        format!("Path is not a safe project-relative path: {path}"),
        "Use a normalized project-relative path without parent traversal.",
    )
}

fn context_diagnostic(error: authoring_project_context::ContextError) -> ToolDiagnostic {
    diagnostic(
        error.diagnostic.code,
        error.diagnostic.message,
        error.diagnostic.next_action,
    )
}

fn diagnostic(
    code: impl Into<String>,
    message: impl Into<String>,
    next_action: impl Into<String>,
) -> ToolDiagnostic {
    ToolDiagnostic {
        code: code.into(),
        message: message.into(),
        next_action: next_action.into(),
        source_location: None,
        compiler_diagnostics: Vec::new(),
    }
}

fn compiler_for_operation(
    session: &mut ProjectAuthoringSession,
    operation_id: &str,
    refresh: &RefreshReport,
) -> Result<
    (
        authoring_project_context::ProjectSnapshotLease,
        GameProjectCompiler,
    ),
    ToolDiagnostic,
> {
    let paths = session
        .source_inventory()
        .map_err(context_diagnostic)?
        .entries
        .into_iter()
        .map(|entry| entry.relative_path)
        .collect();
    let lease = session
        .acquire_snapshot_lease(operation_id, paths)
        .map_err(context_diagnostic)?;
    if lease.snapshot().revision.revision_id != refresh.revision.revision_id {
        return Err(revision_changed_diagnostic());
    }
    let compiler = GameProjectCompiler::bind(&lease).map_err(compiler_diagnostic)?;
    Ok((lease, compiler))
}

fn engine_tool_output(
    operation_id: &str,
    purpose: &str,
) -> Result<ProjectRelativePath, ToolDiagnostic> {
    ProjectRelativePath::parse(format!(
        "Library/EngineTools/Deliveries/{operation_id}/{purpose}/Windows"
    ))
    .map_err(|error| {
        diagnostic(
            error.code,
            error.to_string(),
            "Start a fresh Provider operation with a valid operation identity.",
        )
    })
}

fn opaque_delivery_ref(delivery: &DeliveryRef) -> String {
    format!("engine-delivery:{}", delivery.delivery_identity())
}

fn compiler_diagnostic(
    error: project_authoring_execution::GameProjectCompilerError,
) -> ToolDiagnostic {
    let mut result = diagnostic(error.code(), error.message(), error.next_action());
    result.source_location = error.source_location().cloned();
    result.compiler_diagnostics = error.diagnostics().to_vec();
    result
}

fn execution_input_diagnostic(kind: &str, error: serde_json::Error) -> ToolDiagnostic {
    diagnostic(
        "engine_provider.execution_input_invalid",
        format!("Engine project {kind} input is invalid: {error}"),
        "Regenerate the call from the concrete tool schema.",
    )
}

fn sha256_prefixed(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("sha256:{digest:x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn delivery_verify_windowed_mode_reaches_same_delivery_identity_guard() {
        let fixture = Fixture::new("windowed-verify-mode");
        let mut provider = fixture.provider();
        let call = write_call(
            "verify-windowed",
            "engine_delivery_verify",
            json!({"deliveryRef":"unknown", "mode":"windowed"}),
        );
        let error = provider
            .invoke_delivery_verify(&call, "verify-windowed", None)
            .err()
            .unwrap();
        assert_eq!(error.code, "engine_provider.delivery_ref_unknown");
        assert_eq!(
            delivery_verify_schema()["properties"]["mode"]["enum"],
            json!(["headless", "windowed"])
        );
        for mode in [None, Some("headless")] {
            let mut arguments = json!({"deliveryRef":"unknown"});
            if let Some(mode) = mode {
                arguments["mode"] = mode.into();
            }
            let call = write_call("verify-default", "engine_delivery_verify", arguments);
            assert_eq!(
                provider
                    .invoke_delivery_verify(&call, "verify-default", None)
                    .err()
                    .unwrap()
                    .code,
                "engine_provider.delivery_ref_unknown"
            );
        }
        let call = write_call(
            "verify-invalid",
            "engine_delivery_verify",
            json!({"deliveryRef":"unknown", "mode":"arbitrary"}),
        );
        assert_eq!(
            provider
                .invoke_delivery_verify(&call, "verify-invalid", None)
                .err()
                .unwrap()
                .code,
            "engine_provider.delivery_verify_mode_unsupported"
        );
    }

    #[test]
    fn compiler_diagnostic_fields_are_optional_and_retry_requires_project_error() {
        let mut failure = diagnostic(
            "game_project_compiler.check_cargo_failed",
            "Cargo failed",
            "Inspect diagnostics",
        );
        let old_wire = serde_json::to_value(&failure).unwrap();
        assert!(old_wire.get("sourceLocation").is_none());
        assert!(old_wire.get("compilerDiagnostics").is_none());
        assert_eq!(
            serde_json::from_value::<ToolDiagnostic>(old_wire).unwrap(),
            failure
        );
        for (generated, expected) in [
            (true, ToolRetryability::NotRetryable),
            (false, ToolRetryability::RetryAfterCorrection),
        ] {
            failure.compiler_diagnostics = vec![project_authoring_execution::CheckDiagnostic {
                code: "E0308".into(),
                severity: "error".into(),
                message: "mismatched types".into(),
                location: Some(project_authoring_execution::SourceLocation {
                    source_path: "RuntimeModule/src/lib.rs".into(),
                    field_path: None,
                    line: Some(1),
                    column: Some(1),
                    generated,
                }),
                next_action: "Repair source".into(),
            }];
            let flow = derive_local_flow(
                "engine_project_check",
                CanonicalToolStatus::RejectedByEngine,
                &Value::Null,
                &[failure.clone()],
                None,
            );
            assert_eq!(flow.retryability, expected);
            assert_eq!(
                serde_json::from_value::<ToolDiagnostic>(serde_json::to_value(&failure).unwrap())
                    .unwrap(),
                failure
            );
        }
        failure.compiler_diagnostics.clear();
        assert_eq!(
            derive_local_flow(
                "engine_project_check",
                CanonicalToolStatus::RejectedByEngine,
                &Value::Null,
                &[failure],
                None
            )
            .retryability,
            ToolRetryability::NotRetryable
        );
    }

    #[test]
    fn local_transition_is_bounded_and_reconstructed_from_result_facts() {
        let revision_drift = derive_local_flow(
            "engine_project_mutate",
            CanonicalToolStatus::RejectedByEngine,
            &Value::Null,
            &[diagnostic(
                "authoring_context.mutation_revision_drifted",
                "revision drifted",
                "refresh",
            )],
            None,
        );
        assert_eq!(
            revision_drift.retryability,
            ToolRetryability::RetryAfterInspect
        );
        assert_eq!(
            revision_drift.transitions,
            [RecommendedLocalTransition {
                category: LocalTransitionCategory::InspectLatestRevision,
                tool_name: Some("engine_project_inspect".to_string()),
                reason: "The project revision changed after the operation input was prepared."
                    .to_string(),
            }]
        );

        let correctable_input = derive_local_flow(
            "engine_project_inspect",
            CanonicalToolStatus::RejectedByEngine,
            &Value::Null,
            &[diagnostic(
                "engine_provider.input_schema_invalid",
                "invalid input",
                "correct input",
            )],
            None,
        );
        assert_eq!(
            correctable_input.retryability,
            ToolRetryability::RetryAfterCorrection
        );
        assert_eq!(
            correctable_input.transitions[0].category,
            LocalTransitionCategory::RetryWithCorrection
        );
        assert_eq!(
            correctable_input.transitions[0].tool_name.as_deref(),
            Some("engine_project_inspect")
        );

        let build = derive_local_flow(
            "engine_project_build",
            CanonicalToolStatus::Completed,
            &json!({"deliveryRef":"engine-delivery:sha256:one"}),
            &[],
            None,
        );
        assert_eq!(build.retryability, ToolRetryability::NotNeeded);
        assert_eq!(
            build.transitions[0].category,
            LocalTransitionCategory::VerifyDelivery
        );
        assert_eq!(
            build.transitions[0].tool_name.as_deref(),
            Some("engine_delivery_verify")
        );

        let mutation = derive_local_flow(
            "engine_project_mutate",
            CanonicalToolStatus::Completed,
            &json!({}),
            &[],
            Some("engine-rollback:receipt-one"),
        );
        assert_eq!(
            mutation.transitions[0].category,
            LocalTransitionCategory::RollbackAvailable
        );
        assert_eq!(
            mutation.transitions[0].tool_name.as_deref(),
            Some("engine_project_rollback")
        );

        let inspect = derive_local_flow(
            "engine_project_inspect",
            CanonicalToolStatus::Completed,
            &json!({}),
            &[],
            None,
        );
        assert_eq!(inspect.retryability, ToolRetryability::NotNeeded);
        assert!(inspect.transitions.is_empty());

        let fixture = Fixture::new("local-transition-result");
        let mut provider = fixture.provider();
        let result = provider.invoke(read_call(
            "correctable-result",
            "engine_project_inspect",
            json!({"unknown":true}),
        ));
        assert_eq!(result.schema_version, "engine-tool-result.v2");
        assert_eq!(result.retryability, ToolRetryability::RetryAfterCorrection);
        assert_eq!(
            result.recommended_local_transitions,
            correctable_input.transitions
        );
        let serialized = serde_json::to_value(result).unwrap();
        assert!(serialized.get("recommendedLocalTransitions").is_some());
    }

    #[test]
    fn model_default_projection_is_exact_and_stable() {
        let fixture = Fixture::new("model-default-projection");
        let adapter = NativeHostAdapter::attach(fixture.host()).unwrap();
        let internal = tool_definitions();
        let names = adapter
            .tool_definitions()
            .into_iter()
            .map(|definition| definition.name)
            .collect::<Vec<_>>();

        assert_eq!(internal.len(), 23);
        assert!(internal
            .iter()
            .any(|definition| definition.name == "engine_runtime_run"));
        assert!(!internal
            .iter()
            .any(|definition| definition.name == "engine_project_run"));
        assert!(internal
            .iter()
            .all(|definition| !definition.supports_cancellation));
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
    fn hidden_tool_is_rejected_by_model_adapter() {
        let fixture = Fixture::new("hidden-model-tool");
        let mut adapter = NativeHostAdapter::attach(fixture.host()).unwrap();
        let result = adapter.invoke(read_call(
            "hidden-model-call",
            "engine_project_diagnostics",
            json!({}),
        ));

        assert_eq!(result.status, CanonicalToolStatus::RejectedByEngine);
        assert_eq!(
            result.diagnostics[0].code,
            "engine_provider.tool_not_exposed"
        );
    }

    #[test]
    fn qualified_check_requires_process_capabilities() {
        let internal = tool_definitions_for(ToolAudience::InternalDiagnostics);
        let check = internal
            .iter()
            .find(|definition| definition.name == "engine_project_check")
            .unwrap();
        assert_eq!(check.maturity, ToolMaturity::Ready);
        assert_eq!(check.side_effect, ToolSideEffect::ProcessSpawn);
        assert!(check
            .required_capabilities
            .contains(&ToolCapability::SpawnProcess));
        assert!(check
            .required_capabilities
            .contains(&ToolCapability::GenerateFiles));
        assert!(tool_definitions_for(ToolAudience::ModelDefault)
            .iter()
            .any(|definition| definition.name == "engine_project_check"));
    }

    #[test]
    fn concrete_tool_surface_has_no_catalog_execute_or_context_tools() {
        let definitions = tool_definitions();
        assert!(definitions.iter().all(|definition| {
            definition.name.starts_with("engine_")
                && !definition.name.contains("catalog")
                && !definition.name.contains("execute")
                && !definition.name.contains("context")
                && !definition.name.contains("editor")
                && !definition
                    .input_schema
                    .to_string()
                    .contains("expectedProjectDigest")
        }));
        assert!(definitions
            .iter()
            .any(|definition| definition.name == "engine_project_inspect"));
        assert!(definitions
            .iter()
            .any(|definition| definition.name == "engine_operation_observe"));
        assert!(definitions.iter().all(|definition| {
            !definition.output_schema.is_null()
                && !definition.schema_version.is_empty()
                && !definition.supports_cancellation
        }));
    }

    #[test]
    fn direct_file_refresh_is_observed_by_next_engine_tool() {
        let fixture = Fixture::new("refresh");
        let mut provider = fixture.provider();
        let first = provider.invoke(read_call("one", "engine_project_inspect", json!({})));
        fs::write(fixture.root.join("game.rs"), b"fn after() {}").unwrap();
        let second = provider.invoke(read_call("two", "engine_project_inspect", json!({})));
        assert_eq!(first.status, CanonicalToolStatus::Completed);
        assert_eq!(second.status, CanonicalToolStatus::Completed);
        assert_ne!(first.project_revision, second.project_revision);
        assert!(second.output["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["relativePath"] == "game.rs"));
    }

    #[test]
    fn direct_file_refresh_invalid_manifest_replaces_ready_qualification() {
        let fixture = Fixture::new("refresh-invalid");
        let mut provider = fixture.provider();
        let first = provider.invoke(read_call("one", "engine_project_inspect", json!({})));
        fs::write(fixture.root.join("project.aife.json"), b"{invalid").unwrap();

        let second = provider.invoke(read_call("two", "engine_project_inspect", json!({})));

        assert_eq!(first.status, CanonicalToolStatus::Completed);
        assert_eq!(second.status, CanonicalToolStatus::Completed);
        assert_ne!(first.project_revision, second.project_revision);
        assert_eq!(second.output["qualification"], "invalid");
    }

    #[test]
    fn warm_cache_is_not_authority_for_object_reads() {
        let fixture = Fixture::new("warm-cache");
        fs::write(fixture.root.join("game.rs"), b"fn before() {}").unwrap();
        let mut provider = fixture.provider();
        let first = provider.invoke(read_call(
            "before",
            "engine_project_read_object",
            json!({"path":"game.rs"}),
        ));
        fs::write(fixture.root.join("game.rs"), b"fn after() {}").unwrap();

        let second = provider.invoke(read_call(
            "after",
            "engine_project_read_object",
            json!({"path":"game.rs"}),
        ));

        assert_eq!(first.output["text"], "fn before() {}");
        assert_eq!(second.output["text"], "fn after() {}");
        assert_ne!(first.project_revision, second.project_revision);
    }

    #[test]
    fn clean_repeated_inspect_does_not_touch_canonical_bytes_or_mtime() {
        let fixture = Fixture::new("clean-inspect");
        let source = fixture.root.join("game.rs");
        fs::write(&source, b"fn stable() {}").unwrap();
        let mut provider = fixture.provider();
        let manifest = fixture.root.join("project.aife.json");
        let before = [file_state(&manifest), file_state(&source)];

        let first = provider.invoke(read_call("clean-1", "engine_project_inspect", json!({})));
        let second = provider.invoke(read_call("clean-2", "engine_project_inspect", json!({})));

        assert_eq!(first.status, CanonicalToolStatus::Completed);
        assert_eq!(second.status, CanonicalToolStatus::Completed);
        assert_eq!(first.project_revision, second.project_revision);
        assert_eq!(before, [file_state(&manifest), file_state(&source)]);
    }

    #[test]
    fn mutation_injects_revision_and_exact_rollback_consumes_opaque_ref() {
        let fixture = Fixture::new("mutation");
        let mut provider = fixture.provider();
        let mutation = provider.invoke(write_call("mutate", "engine_project_mutate", json!({
            "goal":"add config",
            "changes":[{"operation":"create_or_replace","path":"Data/config.json","content":"{\"enabled\":true}"}]
        })));
        assert_eq!(mutation.status, CanonicalToolStatus::Completed);
        assert!(fixture.root.join("Data/config.json").is_file());
        assert_eq!(
            mutation.recommended_local_transitions[0].category,
            LocalTransitionCategory::RollbackAvailable
        );
        let rollback = provider.invoke(write_call(
            "rollback",
            "engine_project_rollback",
            json!({"rollbackRef":mutation.receipt_ref.unwrap()}),
        ));
        assert_eq!(rollback.status, CanonicalToolStatus::Completed);
        assert!(!fixture.root.join("Data/config.json").exists());
    }

    #[test]
    fn grant_is_operation_bound_and_scoped_to_the_refreshed_revision() {
        let fixture = Fixture::new("grant");
        let mut provider = fixture.provider();
        let mutation = provider.invoke(write_call("grant-call", "engine_project_mutate", json!({
            "goal":"replace config",
            "domain":"config",
            "changes":[{"operation":"create_or_replace","path":"Data/config.json","content":"{}"}]
        })));
        let operation = provider.operations.get(&mutation.operation_id).unwrap();

        assert_eq!(operation.grant.session_id, "test-session");
        assert_eq!(operation.grant.call_id, "grant-call");
        assert_eq!(operation.grant.max_mutation_count, 1);
        assert!(operation
            .grant
            .capabilities
            .contains(&ToolCapability::MutateProject));
        assert!(operation.grant.project_identity.is_some());
        assert!(operation.grant.project_revision.is_some());
        assert_eq!(
            operation.grant.grant_digest,
            grant_digest(&operation.grant).unwrap()
        );
    }

    #[test]
    fn operation_exact_replay_is_idempotent_and_mismatch_is_rejected() {
        let fixture = Fixture::new("replay");
        let mut provider = fixture.provider();
        let call = read_call(
            "same-call",
            "engine_project_search",
            json!({"query":"player"}),
        );
        let first = provider.invoke(call.clone());
        let replay = provider.invoke(call);
        let mismatch = provider.invoke(read_call(
            "same-call",
            "engine_project_search",
            json!({"query":"enemy"}),
        ));

        assert_eq!(first.operation_id, replay.operation_id);
        assert!(!first.replayed);
        assert!(replay.replayed);
        assert_eq!(mismatch.status, CanonicalToolStatus::RejectedByEngine);
        assert_eq!(
            mismatch.diagnostics[0].code,
            "engine_provider.call_replay_mismatch"
        );
    }

    #[test]
    fn operation_cancel_distinguishes_cancelled_terminal_and_unknown() {
        let fixture = Fixture::new("cancel");
        let mut provider = fixture.provider();
        let call = read_call("held-call", "engine_project_inspect", json!({}));
        let grant = provider.initial_grant(&call);
        provider.start_operation(&call, "held-operation", call_digest(&call), grant, true);

        assert_eq!(
            provider.cancel("held-operation").status,
            CancellationStatus::Cancelled
        );
        assert_eq!(
            provider.cancel("held-operation").status,
            CancellationStatus::AlreadyTerminal
        );
        assert_eq!(
            provider.cancel("missing-operation").status,
            CancellationStatus::NotFound
        );
        assert_eq!(
            provider.observe("held-operation").unwrap().state,
            OperationState::Cancelled
        );
    }

    #[test]
    fn provider_session_rebind_rejects_another_non_terminal_operation() {
        let fixture = Fixture::new("rebind");
        let second_project = fixture.create_project("second");
        let mut provider = fixture.provider();
        let held = read_call("held", "engine_project_inspect", json!({}));
        let grant = provider.initial_grant(&held);
        provider.start_operation(&held, "held-op", call_digest(&held), grant, true);

        let result = provider.invoke(read_call(
            "open-second",
            "engine_project_open",
            json!({"projectRoot":second_project}),
        ));

        assert_eq!(result.status, CanonicalToolStatus::RejectedByEngine);
        assert_eq!(
            result.diagnostics[0].code,
            "engine_provider.project_rebind_blocked"
        );
    }

    #[test]
    fn project_locator_rejects_a_project_outside_the_workspace() {
        let workspace = Fixture::new("workspace");
        let outside = Fixture::new("outside");
        let mut provider = workspace.provider();

        let result = provider.invoke(read_call(
            "outside",
            "engine_project_open",
            json!({"projectRoot":outside.root}),
        ));

        assert_eq!(result.status, CanonicalToolStatus::RejectedByEngine);
        assert_eq!(
            result.diagnostics[0].code,
            "engine_provider.project_outside_workspace"
        );
    }

    #[test]
    fn provider_session_does_not_inherit_rollback_or_approval() {
        let fixture = Fixture::new("session-isolation");
        let mut first = fixture.provider();
        let mutation = first.invoke(write_call("mutate", "engine_project_mutate", json!({
            "goal":"add config",
            "changes":[{"operation":"create_or_replace","path":"Data/config.json","content":"{}"}]
        })));
        let mut second = fixture.provider_with_session("second-session");

        let rollback = second.invoke(write_call(
            "foreign-rollback",
            "engine_project_rollback",
            json!({"rollbackRef": mutation.receipt_ref.unwrap()}),
        ));
        let unapproved = second.invoke(read_call(
            "unapproved",
            "engine_project_mutate",
            json!({
                "goal":"change config",
                "changes":[{"operation":"create_or_replace","path":"Data/config.json","content":"{\"x\":1}"}]
            }),
        ));

        assert_eq!(rollback.status, CanonicalToolStatus::RejectedByEngine);
        assert_eq!(
            rollback.diagnostics[0].code,
            "engine_provider.rollback_ref_unknown"
        );
        assert_eq!(unapproved.status, CanonicalToolStatus::RejectedByHost);
    }

    #[test]
    fn engine_tool_kernel_rejects_hidden_revision_and_unknown_input_fields() {
        let fixture = Fixture::new("hidden-input");
        let mut provider = fixture.provider();
        let result = provider.invoke(read_call(
            "hidden",
            "engine_project_inspect",
            json!({"expectedProjectDigest":"caller-owned"}),
        ));

        assert_eq!(result.status, CanonicalToolStatus::RejectedByEngine);
        assert_eq!(
            result.diagnostics[0].code,
            "engine_provider.input_schema_invalid"
        );
    }

    #[test]
    fn write_requires_host_approval_but_read_does_not() {
        let fixture = Fixture::new("approval");
        let mut provider = fixture.provider();
        let result = provider.invoke(HostToolCall {
            call_id: "denied".to_string(),
            tool_name: "engine_project_mutate".to_string(),
            arguments: json!({"goal":"x","changes":[]}),
            approved: false,
        });
        assert_eq!(result.status, CanonicalToolStatus::RejectedByHost);
        assert_eq!(
            provider
                .invoke(read_call("read", "engine_project_inspect", json!({})))
                .status,
            CanonicalToolStatus::Completed
        );
    }

    #[test]
    fn permission_matrix_and_terminal_outcomes_are_distinct() {
        for definition in tool_definitions() {
            match definition.side_effect {
                ToolSideEffect::Read => {
                    assert_eq!(definition.risk, ToolRisk::Low);
                    assert!(!definition.supports_cancellation);
                    assert!(!definition
                        .required_capabilities
                        .contains(&ToolCapability::SpawnProcess));
                }
                ToolSideEffect::Write => {
                    assert_eq!(definition.risk, ToolRisk::Elevated);
                    assert_eq!(
                        definition.required_capabilities,
                        vec![ToolCapability::MutateProject]
                    );
                    assert!(!definition.supports_cancellation);
                }
                ToolSideEffect::ProcessSpawn => {
                    assert_eq!(definition.risk, ToolRisk::Elevated);
                    assert!(!definition.supports_cancellation);
                    assert!(definition
                        .required_capabilities
                        .contains(&ToolCapability::ReadProject));
                    assert!(definition
                        .required_capabilities
                        .contains(&ToolCapability::GenerateFiles));
                    assert!(definition
                        .required_capabilities
                        .contains(&ToolCapability::SpawnProcess));
                }
            }
        }

        let fixture = Fixture::new("terminal-outcomes");
        let mut provider = fixture.provider();
        let host_rejected = provider.invoke(read_call(
            "host-rejected",
            "engine_project_mutate",
            json!({"goal":"x","changes":[]}),
        ));
        let engine_rejected = provider.invoke(read_call(
            "engine-rejected",
            "engine_project_inspect",
            json!({"unknown":true}),
        ));
        let unsupported = provider.invoke(write_call(
            "unsupported",
            "engine_project_create",
            json!({}),
        ));
        let held = read_call("cancelled", "engine_project_inspect", json!({}));
        let grant = provider.initial_grant(&held);
        provider.start_operation(&held, "cancelled-op", call_digest(&held), grant, true);
        provider.cancel("cancelled-op");
        let cancelled = provider.observe("cancelled-op").unwrap().result.unwrap();

        assert_eq!(host_rejected.status, CanonicalToolStatus::RejectedByHost);
        assert_eq!(
            engine_rejected.status,
            CanonicalToolStatus::RejectedByEngine
        );
        assert_eq!(
            unsupported.status,
            CanonicalToolStatus::UnsupportedOnAdapter
        );
        assert_eq!(cancelled.status, CanonicalToolStatus::Cancelled);
    }

    fn read_call(call_id: &str, tool_name: &str, arguments: Value) -> HostToolCall {
        HostToolCall {
            call_id: call_id.to_string(),
            tool_name: tool_name.to_string(),
            arguments,
            approved: false,
        }
    }

    fn write_call(call_id: &str, tool_name: &str, arguments: Value) -> HostToolCall {
        HostToolCall {
            approved: true,
            ..read_call(call_id, tool_name, arguments)
        }
    }

    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        fn new(label: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "aife-engine-provider-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&root).unwrap();
            fs::write(
                root.join("project.aife.json"),
                br#"{"schemaVersion":"aife-project.v2","projectId":"provider.fixture"}"#,
            )
            .unwrap();
            Self { root }
        }

        fn host(&self) -> HostSessionContext {
            HostSessionContext {
                session_id: "test-session".to_string(),
                workspace_root: self.root.clone(),
                project_root: Some(self.root.clone()),
            }
        }

        fn provider(&self) -> EngineToolProvider {
            self.provider_with_session("test-session")
        }

        fn provider_with_session(&self, session_id: &str) -> EngineToolProvider {
            EngineToolProvider::attach(HostSessionContext {
                session_id: session_id.to_string(),
                workspace_root: self.root.clone(),
                project_root: Some(self.root.clone()),
            })
            .unwrap()
        }

        fn create_project(&self, name: &str) -> PathBuf {
            let root = self.root.join(name);
            fs::create_dir_all(&root).unwrap();
            fs::write(
                root.join("project.aife.json"),
                format!(
                    "{{\"schemaVersion\":\"aife-project.v2\",\"projectId\":\"provider.{name}\"}}"
                ),
            )
            .unwrap();
            root
        }
    }

    fn file_state(path: &Path) -> (Vec<u8>, SystemTime) {
        (
            fs::read(path).unwrap(),
            fs::metadata(path).unwrap().modified().unwrap(),
        )
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}
