use crate::{
    ProjectEditorCompositionArtifact, ProjectEditorCompositionDiagnostic,
    ProjectEditorCompositionHandoffTicket, ProjectEditorCompositionIdentity,
    ProjectEditorCompositionLaunchReceipt, ProjectEditorCompositionLaunchStatus,
    PROJECT_EDITOR_COMPOSITION_HANDOFF_TICKET_SCHEMA_VERSION,
    PROJECT_EDITOR_COMPOSITION_LAUNCH_RECEIPT_SCHEMA_VERSION,
};
use engine_runtime::canonical_digest::sha256_prefixed;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const DEFAULT_EDITOR_COMPOSITION_HANDOFF_TIMEOUT_MS: u64 = 30_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorCompositionHandoffRequest {
    pub old_editor_instance_id: String,
    pub running_identity_digest: Option<String>,
    pub artifact: ProjectEditorCompositionArtifact,
    pub project_root: PathBuf,
    pub project_id: String,
    pub ticket_root: PathBuf,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorCompositionCandidateProcessState {
    Running,
    Exited(i32),
}

pub trait EditorCompositionClock: Send + Sync {
    fn now_epoch_ms(&self) -> u64;
}

pub trait EditorCompositionWorkspaceAdapter: Send + Sync {
    fn save_recoverable_state(&self, project_root: &Path) -> Result<String, String>;
}

pub trait EditorCompositionProcessAdapter: Send + Sync {
    fn launch_candidate(&self, executable: &Path, ticket_path: &Path) -> Result<u32, String>;
    fn candidate_state(
        &self,
        process_id: u32,
    ) -> Result<EditorCompositionCandidateProcessState, String>;
    fn terminate_owned_candidate(&self, process_id: u32) -> Result<(), String>;
}

pub trait EditorCompositionExitAdapter: Send + Sync {
    fn request_graceful_exit(&self) -> Result<(), String>;
}

struct ActiveHandoff {
    ticket: ProjectEditorCompositionHandoffTicket,
    ticket_path: PathBuf,
    candidate_process_id: u32,
}

pub struct EditorProjectCompositionLauncher {
    clock: Arc<dyn EditorCompositionClock>,
    workspace: Arc<dyn EditorCompositionWorkspaceAdapter>,
    process: Arc<dyn EditorCompositionProcessAdapter>,
    exit: Arc<dyn EditorCompositionExitAdapter>,
    active: Option<ActiveHandoff>,
    graceful_exit_requested: bool,
}

impl EditorProjectCompositionLauncher {
    pub fn new(
        clock: Arc<dyn EditorCompositionClock>,
        workspace: Arc<dyn EditorCompositionWorkspaceAdapter>,
        process: Arc<dyn EditorCompositionProcessAdapter>,
        exit: Arc<dyn EditorCompositionExitAdapter>,
    ) -> Self {
        Self {
            clock,
            workspace,
            process,
            exit,
            active: None,
            graceful_exit_requested: false,
        }
    }

    pub fn handoff(
        &mut self,
        request: EditorCompositionHandoffRequest,
    ) -> ProjectEditorCompositionLaunchReceipt {
        if self.active.is_some() {
            return failure_receipt(
                &request,
                "project_editor_composition.handoff_already_active",
                "A project Editor composition handoff is already active.",
            );
        }
        let identity_digest = match request.artifact.descriptor.identity.digest() {
            Ok(value) => value,
            Err(error) => {
                return failure_receipt(
                    &request,
                    "project_editor_composition.identity_invalid",
                    error.to_string(),
                );
            }
        };
        if request.running_identity_digest.as_deref() == Some(identity_digest.as_str()) {
            return ProjectEditorCompositionLaunchReceipt {
                schema_version: PROJECT_EDITOR_COMPOSITION_LAUNCH_RECEIPT_SCHEMA_VERSION
                    .to_string(),
                status: ProjectEditorCompositionLaunchStatus::Ready,
                nonce: "same-composition".to_string(),
                old_editor_instance_id: request.old_editor_instance_id,
                new_editor_instance_id: None,
                project_id: request.project_id,
                composition_identity_digest: identity_digest,
                candidate_process_id: None,
                diagnostics: Vec::new(),
            };
        }
        if let Err(error) = validate_handoff_request(&request, &identity_digest) {
            return failure_receipt(
                &request,
                "project_editor_composition.handoff_request_invalid",
                error,
            );
        }
        let workspace_state_ref = match self.workspace.save_recoverable_state(&request.project_root)
        {
            Ok(value) if !value.trim().is_empty() => value,
            Ok(_) => {
                return failure_receipt(
                    &request,
                    "project_editor_composition.workspace_state_invalid",
                    "Workspace Adapter returned an empty recoverable state reference.",
                );
            }
            Err(error) => {
                return failure_receipt(
                    &request,
                    "project_editor_composition.workspace_state_failed",
                    error,
                );
            }
        };
        let now = self.clock.now_epoch_ms();
        let expires_at = now.saturating_add(request.timeout_ms);
        let nonce = sha256_prefixed(
            format!(
                "{}\0{}\0{}\0{}",
                request.old_editor_instance_id, request.project_id, identity_digest, now
            )
            .as_bytes(),
        )
        .trim_start_matches("sha256:")
        .to_string();
        let ticket_path = request.ticket_root.join(format!("handoff-{nonce}.json"));
        let acknowledgement_path = request.ticket_root.join(format!("ack-{nonce}.json"));
        let ticket = ProjectEditorCompositionHandoffTicket {
            schema_version: PROJECT_EDITOR_COMPOSITION_HANDOFF_TICKET_SCHEMA_VERSION.to_string(),
            nonce: nonce.clone(),
            old_editor_instance_id: request.old_editor_instance_id.clone(),
            expected_identity: request.artifact.descriptor.identity.clone(),
            expected_identity_digest: identity_digest.clone(),
            project_root: request.project_root.clone(),
            project_id: request.project_id.clone(),
            artifact_executable_path: request.artifact.executable_path.clone(),
            artifact_executable_hash: request.artifact.descriptor.executable_hash.clone(),
            workspace_state_ref,
            created_at: now,
            expires_at,
            acknowledgement_path,
        };
        if let Err(error) = write_json_create_new(&ticket_path, &ticket) {
            return failure_receipt(
                &request,
                "project_editor_composition.handoff_ticket_invalid",
                error,
            );
        }
        let candidate_process_id = match self
            .process
            .launch_candidate(&request.artifact.executable_path, &ticket_path)
        {
            Ok(value) => value,
            Err(error) => {
                let _ = remove_owned_file(&request.ticket_root, &ticket_path);
                return failure_receipt(
                    &request,
                    "project_editor_composition.launch_failed",
                    error,
                );
            }
        };
        self.active = Some(ActiveHandoff {
            ticket,
            ticket_path,
            candidate_process_id,
        });
        ProjectEditorCompositionLaunchReceipt {
            schema_version: PROJECT_EDITOR_COMPOSITION_LAUNCH_RECEIPT_SCHEMA_VERSION.to_string(),
            status: ProjectEditorCompositionLaunchStatus::Pending,
            nonce,
            old_editor_instance_id: request.old_editor_instance_id,
            new_editor_instance_id: None,
            project_id: request.project_id,
            composition_identity_digest: identity_digest,
            candidate_process_id: Some(candidate_process_id),
            diagnostics: Vec::new(),
        }
    }

    pub fn poll(&mut self) -> Option<ProjectEditorCompositionLaunchReceipt> {
        let active = self.active.as_ref()?;
        let now = self.clock.now_epoch_ms();
        if now > active.ticket.expires_at {
            let active = self.active.take().expect("active handoff exists");
            let _ = self
                .process
                .terminate_owned_candidate(active.candidate_process_id);
            let cleanup = cleanup_handoff_files(&active);
            return Some(terminal_receipt(
                &active,
                ProjectEditorCompositionLaunchStatus::TimedOut,
                "project_editor_composition.readiness_timeout",
                "Candidate Editor did not acknowledge readiness before the handoff deadline.",
                cleanup,
            ));
        }
        match self.process.candidate_state(active.candidate_process_id) {
            Ok(EditorCompositionCandidateProcessState::Running) => {}
            Ok(EditorCompositionCandidateProcessState::Exited(code)) => {
                let active = self.active.take().expect("active handoff exists");
                let cleanup = cleanup_handoff_files(&active);
                return Some(terminal_receipt(
                    &active,
                    ProjectEditorCompositionLaunchStatus::Failed,
                    "project_editor_composition.launch_failed",
                    format!("Candidate Editor exited before readiness acknowledgement with code {code}."),
                    cleanup,
                ));
            }
            Err(error) => {
                let active = self.active.take().expect("active handoff exists");
                let _ = self
                    .process
                    .terminate_owned_candidate(active.candidate_process_id);
                let cleanup = cleanup_handoff_files(&active);
                return Some(terminal_receipt(
                    &active,
                    ProjectEditorCompositionLaunchStatus::Failed,
                    "project_editor_composition.launch_failed",
                    error,
                    cleanup,
                ));
            }
        }
        if !active.ticket.acknowledgement_path.exists() {
            return None;
        }
        let active = self.active.take().expect("active handoff exists");
        let receipt = match read_and_validate_ack(&active) {
            Ok(receipt) => receipt,
            Err(error) => {
                let _ = self
                    .process
                    .terminate_owned_candidate(active.candidate_process_id);
                let cleanup = cleanup_handoff_files(&active);
                return Some(terminal_receipt(
                    &active,
                    ProjectEditorCompositionLaunchStatus::Failed,
                    "project_editor_composition.handoff_ticket_invalid",
                    error,
                    cleanup,
                ));
            }
        };
        if !self.graceful_exit_requested {
            if let Err(error) = self.exit.request_graceful_exit() {
                let cleanup = cleanup_handoff_files(&active);
                return Some(terminal_receipt(
                    &active,
                    ProjectEditorCompositionLaunchStatus::Failed,
                    "project_editor_composition.graceful_exit_failed",
                    error,
                    cleanup,
                ));
            }
            self.graceful_exit_requested = true;
        }
        let _ = cleanup_handoff_files(&active);
        Some(receipt)
    }

    pub fn cancel(&mut self) -> Option<ProjectEditorCompositionLaunchReceipt> {
        let active = self.active.take()?;
        let _ = self
            .process
            .terminate_owned_candidate(active.candidate_process_id);
        let cleanup = cleanup_handoff_files(&active);
        Some(terminal_receipt(
            &active,
            ProjectEditorCompositionLaunchStatus::Cancelled,
            "project_editor_composition.handoff_cancelled",
            "Project Editor composition handoff was cancelled by the user.",
            cleanup,
        ))
    }

    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorCompositionCandidateReadiness {
    pub ticket_path: PathBuf,
    pub current_executable_path: PathBuf,
    pub current_executable_hash: String,
    pub running_identity: ProjectEditorCompositionIdentity,
    pub project_root: PathBuf,
    pub new_editor_instance_id: String,
    pub candidate_process_id: u32,
    pub now_epoch_ms: u64,
}

pub fn prepare_editor_composition_candidate_readiness(
    ticket_path: PathBuf,
    current_executable_path: PathBuf,
    running_identity: ProjectEditorCompositionIdentity,
    new_editor_instance_id: String,
    candidate_process_id: u32,
    now_epoch_ms: u64,
) -> Result<EditorCompositionCandidateReadiness, String> {
    let ticket: ProjectEditorCompositionHandoffTicket =
        serde_json::from_slice(&fs::read(&ticket_path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    let current_executable_hash =
        sha256_prefixed(&fs::read(&current_executable_path).map_err(|error| error.to_string())?);
    let readiness = EditorCompositionCandidateReadiness {
        ticket_path,
        current_executable_path,
        current_executable_hash,
        running_identity,
        project_root: ticket.project_root,
        new_editor_instance_id,
        candidate_process_id,
        now_epoch_ms,
    };
    validate_candidate_readiness(&readiness)?;
    Ok(readiness)
}

pub fn acknowledge_editor_composition_candidate(
    readiness: &EditorCompositionCandidateReadiness,
) -> Result<ProjectEditorCompositionLaunchReceipt, String> {
    let ticket = validate_candidate_readiness(readiness)?;
    let ticket_parent = readiness
        .ticket_path
        .parent()
        .ok_or_else(|| "Handoff ticket has no parent directory.".to_string())?;
    let ticket_parent = fs::canonicalize(ticket_parent).map_err(|error| error.to_string())?;
    let ack_parent = ticket
        .acknowledgement_path
        .parent()
        .ok_or_else(|| "Acknowledgement path has no parent.".to_string())?;
    if fs::canonicalize(ack_parent).map_err(|error| error.to_string())? != ticket_parent {
        return Err("Acknowledgement path escapes the controlled ticket root.".to_string());
    }
    let receipt = ProjectEditorCompositionLaunchReceipt {
        schema_version: PROJECT_EDITOR_COMPOSITION_LAUNCH_RECEIPT_SCHEMA_VERSION.to_string(),
        status: ProjectEditorCompositionLaunchStatus::Ready,
        nonce: ticket.nonce,
        old_editor_instance_id: ticket.old_editor_instance_id,
        new_editor_instance_id: Some(readiness.new_editor_instance_id.clone()),
        project_id: ticket.project_id,
        composition_identity_digest: ticket.expected_identity_digest,
        candidate_process_id: Some(readiness.candidate_process_id),
        diagnostics: Vec::new(),
    };
    write_json_create_new(&ticket.acknowledgement_path, &receipt)?;
    Ok(receipt)
}

fn validate_candidate_readiness(
    readiness: &EditorCompositionCandidateReadiness,
) -> Result<ProjectEditorCompositionHandoffTicket, String> {
    let ticket_parent = readiness
        .ticket_path
        .parent()
        .ok_or_else(|| "Handoff ticket has no parent directory.".to_string())?;
    let ticket_parent = fs::canonicalize(ticket_parent).map_err(|error| error.to_string())?;
    let ticket_path =
        fs::canonicalize(&readiness.ticket_path).map_err(|error| error.to_string())?;
    if ticket_path.parent() != Some(ticket_parent.as_path()) {
        return Err("Handoff ticket must be a direct child of its controlled root.".to_string());
    }
    let ticket: ProjectEditorCompositionHandoffTicket =
        serde_json::from_slice(&fs::read(&ticket_path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    if ticket.schema_version != PROJECT_EDITOR_COMPOSITION_HANDOFF_TICKET_SCHEMA_VERSION
        || ticket.expected_identity.validate().is_err()
        || ticket
            .expected_identity
            .digest()
            .map_err(|error| error.to_string())?
            != ticket.expected_identity_digest
        || ticket.expected_identity != readiness.running_identity
        || fs::canonicalize(&ticket.project_root).map_err(|error| error.to_string())?
            != fs::canonicalize(&readiness.project_root).map_err(|error| error.to_string())?
        || fs::canonicalize(&ticket.artifact_executable_path).map_err(|error| error.to_string())?
            != fs::canonicalize(&readiness.current_executable_path)
                .map_err(|error| error.to_string())?
        || ticket.artifact_executable_hash != readiness.current_executable_hash
        || readiness.now_epoch_ms > ticket.expires_at
    {
        return Err(
            "Handoff ticket identity, artifact, project, or expiry validation failed.".to_string(),
        );
    }
    Ok(ticket)
}

fn validate_handoff_request(
    request: &EditorCompositionHandoffRequest,
    identity_digest: &str,
) -> Result<(), String> {
    request
        .artifact
        .descriptor
        .identity
        .validate()
        .map_err(|error| error.to_string())?;
    if request.timeout_ms == 0
        || request.old_editor_instance_id.trim().is_empty()
        || request.project_id.trim().is_empty()
        || request.project_id != request.artifact.descriptor.identity.project_id
        || request.artifact.descriptor.identity_digest != identity_digest
        || request
            .artifact
            .descriptor
            .executable_hash
            .trim()
            .is_empty()
    {
        return Err("Handoff request identity fields are missing or inconsistent.".to_string());
    }
    let project_root =
        fs::canonicalize(&request.project_root).map_err(|error| error.to_string())?;
    let ticket_root = fs::canonicalize(&request.ticket_root).map_err(|error| error.to_string())?;
    if ticket_root.starts_with(&project_root) {
        return Err("Handoff ticket root cannot be inside the project root.".to_string());
    }
    let executable =
        fs::canonicalize(&request.artifact.executable_path).map_err(|error| error.to_string())?;
    let actual_hash = sha256_prefixed(&fs::read(executable).map_err(|error| error.to_string())?);
    if actual_hash != request.artifact.descriptor.executable_hash {
        return Err("Composition executable hash does not match the sealed artifact.".to_string());
    }
    Ok(())
}

fn read_and_validate_ack(
    active: &ActiveHandoff,
) -> Result<ProjectEditorCompositionLaunchReceipt, String> {
    let receipt: ProjectEditorCompositionLaunchReceipt = serde_json::from_slice(
        &fs::read(&active.ticket.acknowledgement_path).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    if receipt.schema_version != PROJECT_EDITOR_COMPOSITION_LAUNCH_RECEIPT_SCHEMA_VERSION
        || receipt.status != ProjectEditorCompositionLaunchStatus::Ready
        || receipt.nonce != active.ticket.nonce
        || receipt.old_editor_instance_id != active.ticket.old_editor_instance_id
        || receipt.project_id != active.ticket.project_id
        || receipt.composition_identity_digest != active.ticket.expected_identity_digest
        || receipt.candidate_process_id != Some(active.candidate_process_id)
        || receipt
            .new_editor_instance_id
            .as_deref()
            .is_none_or(str::is_empty)
    {
        return Err(
            "Candidate readiness acknowledgement did not exactly match the active handoff."
                .to_string(),
        );
    }
    Ok(receipt)
}

fn cleanup_handoff_files(active: &ActiveHandoff) -> String {
    let root = active
        .ticket_path
        .parent()
        .expect("validated ticket path has a parent");
    let first = remove_owned_file(root, &active.ticket.acknowledgement_path);
    let second = remove_owned_file(root, &active.ticket_path);
    if first.is_ok() && second.is_ok() {
        "removed".to_string()
    } else {
        "retained_by_host_policy".to_string()
    }
}

fn remove_owned_file(root: &Path, path: &Path) -> Result<(), String> {
    if path.parent() != Some(root) {
        return Err("Refused to remove a handoff file outside its owner root.".to_string());
    }
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn write_json_create_new(path: &Path, value: &impl serde::Serialize) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| error.to_string())?;
    file.write_all(&bytes).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())
}

fn failure_receipt(
    request: &EditorCompositionHandoffRequest,
    code: &str,
    message: impl Into<String>,
) -> ProjectEditorCompositionLaunchReceipt {
    ProjectEditorCompositionLaunchReceipt {
        schema_version: PROJECT_EDITOR_COMPOSITION_LAUNCH_RECEIPT_SCHEMA_VERSION.to_string(),
        status: ProjectEditorCompositionLaunchStatus::Failed,
        nonce: String::new(),
        old_editor_instance_id: request.old_editor_instance_id.clone(),
        new_editor_instance_id: None,
        project_id: request.project_id.clone(),
        composition_identity_digest: request.artifact.descriptor.identity_digest.clone(),
        candidate_process_id: None,
        diagnostics: vec![diagnostic(
            code,
            message,
            "Keep the current Editor open and retry after fixing the reported handoff input.",
        )],
    }
}

fn terminal_receipt(
    active: &ActiveHandoff,
    status: ProjectEditorCompositionLaunchStatus,
    code: &str,
    message: impl Into<String>,
    cleanup: String,
) -> ProjectEditorCompositionLaunchReceipt {
    ProjectEditorCompositionLaunchReceipt {
        schema_version: PROJECT_EDITOR_COMPOSITION_LAUNCH_RECEIPT_SCHEMA_VERSION.to_string(),
        status,
        nonce: active.ticket.nonce.clone(),
        old_editor_instance_id: active.ticket.old_editor_instance_id.clone(),
        new_editor_instance_id: None,
        project_id: active.ticket.project_id.clone(),
        composition_identity_digest: active.ticket.expected_identity_digest.clone(),
        candidate_process_id: Some(active.candidate_process_id),
        diagnostics: vec![diagnostic(
            code,
            message,
            format!("Keep the current Editor open. Handoff cleanup: {cleanup}."),
        )],
    }
}

fn diagnostic(
    code: &str,
    message: impl Into<String>,
    next_action: impl Into<String>,
) -> ProjectEditorCompositionDiagnostic {
    ProjectEditorCompositionDiagnostic {
        code: code.to_string(),
        stage: "handoff".to_string(),
        message: message.into(),
        path: None,
        expected_identity: None,
        actual_identity: None,
        next_action: next_action.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        GeneratedCompositionLockLineage, ProjectEditorCompositionDescriptor,
        ProjectEditorCompositionResolvedIdentity,
        GENERATED_COMPOSITION_LOCK_LINEAGE_SCHEMA_VERSION,
        PROJECT_EDITOR_COMPOSITION_ARTIFACT_SCHEMA_VERSION,
        PROJECT_EDITOR_COMPOSITION_DESCRIPTOR_SCHEMA_VERSION,
        PROJECT_EDITOR_COMPOSITION_IDENTITY_SCHEMA_VERSION,
    };
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(1);

    struct FakeClock(AtomicU64);
    impl EditorCompositionClock for FakeClock {
        fn now_epoch_ms(&self) -> u64 {
            self.0.load(Ordering::SeqCst)
        }
    }

    struct FakeWorkspace(AtomicUsize);
    impl EditorCompositionWorkspaceAdapter for FakeWorkspace {
        fn save_recoverable_state(&self, _: &Path) -> Result<String, String> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok("workspace-state.json".to_string())
        }
    }

    struct FakeProcess {
        launches: AtomicUsize,
        terminations: AtomicUsize,
        state_error: Mutex<Option<String>>,
        state: Mutex<EditorCompositionCandidateProcessState>,
    }
    impl EditorCompositionProcessAdapter for FakeProcess {
        fn launch_candidate(&self, _: &Path, _: &Path) -> Result<u32, String> {
            self.launches.fetch_add(1, Ordering::SeqCst);
            Ok(41)
        }
        fn candidate_state(
            &self,
            _: u32,
        ) -> Result<EditorCompositionCandidateProcessState, String> {
            if let Some(error) = self.state_error.lock().unwrap().clone() {
                return Err(error);
            }
            Ok(*self.state.lock().unwrap())
        }
        fn terminate_owned_candidate(&self, _: u32) -> Result<(), String> {
            self.terminations.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    struct FakeExit(AtomicUsize);
    impl EditorCompositionExitAdapter for FakeExit {
        fn request_graceful_exit(&self) -> Result<(), String> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    struct Fixture {
        root: PathBuf,
        request: EditorCompositionHandoffRequest,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn identity() -> ProjectEditorCompositionIdentity {
        ProjectEditorCompositionIdentity {
            schema_version: PROJECT_EDITOR_COMPOSITION_IDENTITY_SCHEMA_VERSION.to_string(),
            project_id: "fixture.project".to_string(),
            module_id: "fixture.runtime".to_string(),
            interface_version: "project-runtime-module.v2".to_string(),
            aot_content_digest: format!("sha256:{}", "a".repeat(64)),
            editor_build_identity: format!("sha256:{}", "b".repeat(64)),
            engine_sdk_digest: format!("sha256:{}", "c".repeat(64)),
            toolchain_identity: "rustc-test".to_string(),
            target_triple: "x86_64-pc-windows-msvc".to_string(),
            profile: "release".to_string(),
            normalized_manifest_digest: format!("sha256:{}", "d".repeat(64)),
            normalized_dependency_digest: format!("sha256:{}", "e".repeat(64)),
            dependency_lock_digest: format!("sha256:{}", "f".repeat(64)),
        }
    }

    fn resolved_identity(
        identity: &ProjectEditorCompositionIdentity,
    ) -> ProjectEditorCompositionResolvedIdentity {
        ProjectEditorCompositionResolvedIdentity::new(
            identity.digest().unwrap(),
            &GeneratedCompositionLockLineage {
                schema_version: GENERATED_COMPOSITION_LOCK_LINEAGE_SCHEMA_VERSION.to_string(),
                lock_input_digest: format!("sha256:{}", "1".repeat(64)),
                raw_lock_digest: format!("sha256:{}", "2".repeat(64)),
                resolved_graph_digest: format!("sha256:{}", "3".repeat(64)),
            },
        )
        .unwrap()
    }

    fn fixture() -> Fixture {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let fixture_id = NEXT_FIXTURE_ID.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!("aife-262-handoff-{stamp}-{fixture_id}"));
        let project = root.join("project");
        let tickets = root.join("tickets");
        let artifact_root = root.join("artifact");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&tickets).unwrap();
        fs::create_dir_all(&artifact_root).unwrap();
        let executable = artifact_root.join("editor.exe");
        fs::write(&executable, b"sealed-editor").unwrap();
        let identity = identity();
        let descriptor = ProjectEditorCompositionDescriptor {
            schema_version: PROJECT_EDITOR_COMPOSITION_DESCRIPTOR_SCHEMA_VERSION.to_string(),
            identity: identity.clone(),
            identity_digest: identity.digest().unwrap(),
            resolved_identity: resolved_identity(&identity),
            executable_hash: sha256_prefixed(b"sealed-editor"),
            created_at: 1,
        };
        Fixture {
            root,
            request: EditorCompositionHandoffRequest {
                old_editor_instance_id: "old-editor".to_string(),
                running_identity_digest: None,
                artifact: ProjectEditorCompositionArtifact {
                    schema_version: PROJECT_EDITOR_COMPOSITION_ARTIFACT_SCHEMA_VERSION.to_string(),
                    executable_path: executable,
                    descriptor_path: artifact_root.join("composition-descriptor.json"),
                    build_report_path: artifact_root.join("build-report.json"),
                    descriptor,
                },
                project_root: project,
                project_id: "fixture.project".to_string(),
                ticket_root: tickets,
                timeout_ms: 100,
            },
        }
    }

    fn adapters() -> (
        Arc<FakeClock>,
        Arc<FakeWorkspace>,
        Arc<FakeProcess>,
        Arc<FakeExit>,
    ) {
        (
            Arc::new(FakeClock(AtomicU64::new(10))),
            Arc::new(FakeWorkspace(AtomicUsize::new(0))),
            Arc::new(FakeProcess {
                launches: AtomicUsize::new(0),
                terminations: AtomicUsize::new(0),
                state_error: Mutex::new(None),
                state: Mutex::new(EditorCompositionCandidateProcessState::Running),
            }),
            Arc::new(FakeExit(AtomicUsize::new(0))),
        )
    }

    #[test]
    fn editor_project_composition_launcher_same_identity_skips_relaunch() {
        let mut fixture = fixture();
        fixture.request.running_identity_digest =
            Some(fixture.request.artifact.descriptor.identity_digest.clone());
        let (clock, workspace, process, exit) = adapters();
        let mut owner = EditorProjectCompositionLauncher::new(
            clock,
            workspace.clone(),
            process.clone(),
            exit.clone(),
        );

        let receipt = owner.handoff(fixture.request.clone());

        assert_eq!(receipt.status, ProjectEditorCompositionLaunchStatus::Ready);
        assert_eq!(process.launches.load(Ordering::SeqCst), 0);
        assert_eq!(workspace.0.load(Ordering::SeqCst), 0);
        assert_eq!(exit.0.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn editor_project_composition_launcher_exact_ready_ack_requests_exit_once() {
        let fixture = fixture();
        let (clock, workspace, process, exit) = adapters();
        let mut owner = EditorProjectCompositionLauncher::new(
            clock,
            workspace.clone(),
            process.clone(),
            exit.clone(),
        );
        let pending = owner.handoff(fixture.request.clone());
        assert_eq!(
            pending.status,
            ProjectEditorCompositionLaunchStatus::Pending
        );
        assert!(owner.poll().is_none());
        assert_eq!(exit.0.load(Ordering::SeqCst), 0);
        let active = owner.active.as_ref().unwrap();

        let readiness = prepare_editor_composition_candidate_readiness(
            active.ticket_path.clone(),
            fixture.request.artifact.executable_path.clone(),
            fixture.request.artifact.descriptor.identity.clone(),
            "new-editor".to_string(),
            41,
            11,
        )
        .unwrap();
        assert!(!active.ticket.acknowledgement_path.exists());
        acknowledge_editor_composition_candidate(&readiness).unwrap();

        let ready = owner.poll().unwrap();
        assert_eq!(ready.status, ProjectEditorCompositionLaunchStatus::Ready);
        assert_eq!(ready.new_editor_instance_id.as_deref(), Some("new-editor"));
        assert_eq!(workspace.0.load(Ordering::SeqCst), 1);
        assert_eq!(process.launches.load(Ordering::SeqCst), 1);
        assert_eq!(exit.0.load(Ordering::SeqCst), 1);
        assert!(!owner.is_active());
    }

    #[test]
    fn editor_project_composition_launcher_wrong_ack_timeout_and_cancel_keep_old_editor() {
        let fixture = fixture();
        let (clock, workspace, process, exit) = adapters();
        let mut owner = EditorProjectCompositionLauncher::new(
            clock.clone(),
            workspace,
            process.clone(),
            exit.clone(),
        );
        owner.handoff(fixture.request.clone());
        let active = owner.active.as_ref().unwrap();
        let wrong = ProjectEditorCompositionLaunchReceipt {
            schema_version: PROJECT_EDITOR_COMPOSITION_LAUNCH_RECEIPT_SCHEMA_VERSION.to_string(),
            status: ProjectEditorCompositionLaunchStatus::Ready,
            nonce: "replayed".to_string(),
            old_editor_instance_id: "old-editor".to_string(),
            new_editor_instance_id: Some("new".to_string()),
            project_id: fixture.request.project_id.clone(),
            composition_identity_digest: fixture
                .request
                .artifact
                .descriptor
                .identity_digest
                .clone(),
            candidate_process_id: Some(41),
            diagnostics: Vec::new(),
        };
        write_json_create_new(&active.ticket.acknowledgement_path, &wrong).unwrap();
        assert_eq!(
            owner.poll().unwrap().status,
            ProjectEditorCompositionLaunchStatus::Failed
        );
        assert_eq!(exit.0.load(Ordering::SeqCst), 0);

        owner.handoff(fixture.request.clone());
        clock.0.store(200, Ordering::SeqCst);
        assert_eq!(
            owner.poll().unwrap().status,
            ProjectEditorCompositionLaunchStatus::TimedOut
        );
        assert_eq!(exit.0.load(Ordering::SeqCst), 0);

        clock.0.store(10, Ordering::SeqCst);
        owner.handoff(fixture.request.clone());
        assert_eq!(
            owner.cancel().unwrap().status,
            ProjectEditorCompositionLaunchStatus::Cancelled
        );
        assert_eq!(exit.0.load(Ordering::SeqCst), 0);
        assert_eq!(process.terminations.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn editor_project_composition_launcher_state_error_terminates_owned_candidate() {
        let fixture = fixture();
        let (clock, workspace, process, exit) = adapters();
        *process.state_error.lock().unwrap() = Some("state query failed".to_string());
        let mut owner =
            EditorProjectCompositionLauncher::new(clock, workspace, process.clone(), exit.clone());

        owner.handoff(fixture.request.clone());
        let receipt = owner.poll().unwrap();

        assert_eq!(receipt.status, ProjectEditorCompositionLaunchStatus::Failed);
        assert_eq!(process.terminations.load(Ordering::SeqCst), 1);
        assert_eq!(exit.0.load(Ordering::SeqCst), 0);
        assert!(!owner.is_active());
    }
}
