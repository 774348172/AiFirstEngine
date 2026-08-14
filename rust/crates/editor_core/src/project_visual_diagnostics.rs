use crate::{
    EditorSession, ProjectCandidateEntry, ProjectObservationIndex, ProjectPreviewEvidence,
    ProjectPreviewFrameEvidence, ProjectReferencesInput, ProjectRelativePath,
    ProjectSourceSymbolsInput, PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION,
};
use engine_runtime::aui::AuiDocument;
use engine_runtime::canonical_digest::{canonical_json_bytes, sha256_prefixed};
use engine_runtime::visual_issue::{
    VisualIssueBundle, VisualIssueContext, VisualIssueNodeEvidence,
    VISUAL_ISSUE_BUNDLE_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const PROJECT_UI_DIAGNOSTIC_INPUT_SCHEMA_VERSION: &str = "project-ui-diagnostic-input.v1";
pub const PROJECT_VISUAL_ISSUE_BUNDLE_SCHEMA_VERSION: &str = "project-visual-issue-bundle.v1";
pub const PROJECT_VISUAL_ISSUE_ROOT: &str = "Library/AiCapability/Visual";
const PROJECT_VISUAL_ISSUE_FILE: &str = "issue-bundle.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeCaptureIssueInput {
    pub schema_version: String,
    pub frame_evidence_ref: String,
    pub symptom: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectUiLocateInput {
    pub schema_version: String,
    pub query: String,
    pub issue_bundle_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectUiExplainInput {
    pub schema_version: String,
    pub document_path: String,
    pub node_id: String,
    pub issue_bundle_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectUiOwnerTraceInput {
    pub schema_version: String,
    pub document_path: String,
    pub node_id: String,
    pub issue_bundle_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectVisualIssueBundle {
    pub schema_version: String,
    pub capture_operation_id: String,
    pub project_identity: String,
    pub project_digest: String,
    pub preview_operation_id: String,
    pub frame_evidence_ref: String,
    pub frame_digest: String,
    pub screenshot_ref: String,
    pub screenshot_digest: String,
    pub symptom: Option<String>,
    pub issue_bundle_ref: String,
    pub bundle_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectUiCandidate {
    pub document_path: String,
    pub document_id: String,
    pub node_id: String,
    pub node_name: String,
    pub node_text: Option<String>,
    pub binding_paths: Vec<String>,
    pub action_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectUiLocateResult {
    pub query: String,
    pub candidates: Vec<ProjectUiCandidate>,
    pub ambiguity_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectUiOwnerTrace {
    pub document_path: String,
    pub document_id: String,
    pub node_id: String,
    pub binding_paths: Vec<String>,
    pub action_ids: Vec<String>,
    pub referenced_objects: Vec<String>,
    pub project_source_symbols: Vec<String>,
}

pub struct ProjectVisualDiagnostics;

impl ProjectVisualDiagnostics {
    pub fn capture_issue(
        session: &EditorSession,
        operation_id: &str,
        input: &ProjectRuntimeCaptureIssueInput,
    ) -> Result<ProjectVisualIssueBundle, String> {
        validate_schema(&input.schema_version)?;
        validate_operation_id(operation_id)?;
        if let Some(symptom) = &input.symptom {
            let trimmed = symptom.trim();
            if trimmed.is_empty() || trimmed.len() > 1_024 || trimmed != symptom {
                return Err(
                    "Visual issue symptom must contain 1-1024 canonical characters when supplied."
                        .to_string(),
                );
            }
        }
        let (frame, binding) = validate_frame_evidence(session, &input.frame_evidence_ref)?;
        let issue_bundle_ref = issue_bundle_ref(operation_id)?;
        let mut bundle = ProjectVisualIssueBundle {
            schema_version: PROJECT_VISUAL_ISSUE_BUNDLE_SCHEMA_VERSION.to_string(),
            capture_operation_id: operation_id.to_string(),
            project_identity: binding.project_id,
            project_digest: binding.project_digest,
            preview_operation_id: frame.operation_id,
            frame_evidence_ref: input.frame_evidence_ref.clone(),
            frame_digest: frame.frame_digest,
            screenshot_ref: frame.screenshot_ref,
            screenshot_digest: frame.screenshot_digest,
            symptom: input.symptom.clone(),
            issue_bundle_ref: issue_bundle_ref.clone(),
            bundle_digest: String::new(),
        };
        bundle.bundle_digest = project_visual_issue_digest(&bundle)?;
        let bytes = serde_json::to_vec_pretty(&bundle).map_err(|error| error.to_string())?;
        let project = session
            .active_project_session()
            .ok_or_else(|| "Visual issue capture requires an active project.".to_string())?;
        project
            .write_scope()
            .write_atomic(&issue_bundle_ref, &bytes)
            .map_err(|error| error.to_string())?;
        read_issue_bundle(session, &issue_bundle_ref)
    }

    pub fn locate(
        session: &EditorSession,
        input: &ProjectUiLocateInput,
    ) -> Result<ProjectUiLocateResult, String> {
        validate_schema(&input.schema_version)?;
        if let Some(issue_bundle_ref) = &input.issue_bundle_ref {
            read_issue_bundle(session, issue_bundle_ref)?;
        }
        let query = input.query.trim().to_ascii_lowercase();
        if query.is_empty() || query.len() > 256 {
            return Err("UI locate query must contain 1-256 characters.".to_string());
        }
        let mut candidates = Vec::new();
        for (path, document) in load_documents(session)? {
            for node in &document.nodes {
                let matches = [
                    Some(node.node_id.as_str()),
                    Some(node.name.as_str()),
                    node.text.as_deref(),
                ]
                .into_iter()
                .flatten()
                .any(|value| value.to_ascii_lowercase().contains(&query));
                if matches {
                    candidates.push(candidate(&path, &document, node));
                }
            }
        }
        candidates.sort_by(|left, right| {
            (&left.document_path, &left.node_id).cmp(&(&right.document_path, &right.node_id))
        });
        let ambiguity_count = candidates.len().saturating_sub(1);
        Ok(ProjectUiLocateResult {
            query: input.query.clone(),
            candidates,
            ambiguity_count,
        })
    }

    pub fn explain_visibility(
        session: &EditorSession,
        input: &ProjectUiExplainInput,
    ) -> Result<VisualIssueBundle, String> {
        validate_schema(&input.schema_version)?;
        validate_bounded_field(&input.document_path, 512, "AUI document path")?;
        validate_bounded_field(&input.node_id, 256, "AUI node id")?;
        let issue = read_issue_bundle(session, &input.issue_bundle_ref)?;
        let (_path, document) = load_document(session, &input.document_path)?;
        let node = document
            .nodes
            .iter()
            .find(|node| node.node_id == input.node_id)
            .ok_or_else(|| format!("AUI node '{}' was not found.", input.node_id))?;
        let runtime_digest = sha256_prefixed(
            format!(
                "{}|{}",
                issue.project_identity,
                session
                    .active_project_session()
                    .map(|project| project.manifest.runtime_module.module_id.as_str())
                    .unwrap_or_default()
            )
            .as_bytes(),
        );
        let parent_chain = parent_chain(&document, node);
        let parent_hidden = parent_chain.iter().any(|parent_id| {
            document
                .nodes
                .iter()
                .find(|candidate| &candidate.node_id == parent_id)
                .is_some_and(|parent| !parent.visible)
        });
        let first_failure_stage = if !node.visible {
            "authored_visibility"
        } else if parent_hidden {
            "parent_or_screen_visibility"
        } else {
            "presented_frame_semantic_trace_unavailable"
        };
        let mut bundle = VisualIssueBundle {
            schema_version: VISUAL_ISSUE_BUNDLE_SCHEMA_VERSION.to_string(),
            document_id: document.document_id,
            context: VisualIssueContext {
                project_digest: issue.project_digest,
                runtime_digest,
                frame_digest: issue.frame_digest,
                screenshot_ref: issue.screenshot_ref,
                screenshot_digest: issue.screenshot_digest,
            },
            node: VisualIssueNodeEvidence {
                node_id: node.node_id.clone(),
                node_name: node.name.clone(),
                authored_visible: node.visible,
                resolved_visible: None,
                parent_chain,
                binding_paths: sorted(node.binding_refs.iter().map(|binding| binding.path.clone())),
                action_ids: sorted(
                    node.action_refs
                        .iter()
                        .map(|action| action.action_id.clone()),
                ),
                layout_rect: None,
                effective_clip_rect: None,
                clipped_by_node: None,
                draw_command_present: false,
                text_glyph_present: false,
                ui_pass_inserted: true,
                first_failure_stage: first_failure_stage.to_string(),
                diagnostic_codes: vec![
                    "exact_presented_frame_evidence_verified".to_string(),
                    "runtime_semantic_trace_not_captured".to_string(),
                ],
            },
            bundle_digest: String::new(),
        };
        bundle.bundle_digest = visual_issue_digest(&bundle)?;
        Ok(bundle)
    }

    pub fn trace_owner(
        session: &EditorSession,
        input: &ProjectUiOwnerTraceInput,
    ) -> Result<ProjectUiOwnerTrace, String> {
        validate_schema(&input.schema_version)?;
        validate_bounded_field(&input.document_path, 512, "AUI document path")?;
        validate_bounded_field(&input.node_id, 256, "AUI node id")?;
        if let Some(issue_bundle_ref) = &input.issue_bundle_ref {
            read_issue_bundle(session, issue_bundle_ref)?;
        }
        let (path, document) = load_document(session, &input.document_path)?;
        let node = document
            .nodes
            .iter()
            .find(|node| node.node_id == input.node_id)
            .ok_or_else(|| format!("AUI node '{}' was not found.", input.node_id))?;
        let binding_paths = sorted(node.binding_refs.iter().map(|binding| binding.path.clone()));
        let action_ids = sorted(
            node.action_refs
                .iter()
                .map(|action| action.action_id.clone()),
        );
        let node_id = node.node_id.clone();
        let index = ProjectObservationIndex::build(session)?;
        let mut referenced_objects = Vec::new();
        let mut project_source_symbols = Vec::new();
        for value in binding_paths.iter().chain(action_ids.iter()) {
            let references = index.references(&ProjectReferencesInput {
                schema_version: PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION.to_string(),
                symbol_or_value: value.clone(),
                continuation_token: None,
                page_size: 100,
            })?;
            referenced_objects.extend(references.references.into_iter().map(|reference| {
                format!("{}#{}", reference.project_relative_path, reference.line)
            }));
            let symbols = index.source_symbols(&ProjectSourceSymbolsInput {
                schema_version: PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION.to_string(),
                query: symbol_query(value),
                continuation_token: None,
                page_size: 100,
            })?;
            project_source_symbols.extend(
                symbols
                    .symbols
                    .into_iter()
                    .map(|symbol| format!("{}::{}", symbol.project_relative_path, symbol.name)),
            );
        }
        Ok(ProjectUiOwnerTrace {
            document_path: path,
            document_id: document.document_id,
            node_id,
            binding_paths,
            action_ids,
            referenced_objects: sorted(referenced_objects),
            project_source_symbols: sorted(project_source_symbols),
        })
    }
}

fn validate_schema(schema_version: &str) -> Result<(), String> {
    (schema_version == PROJECT_UI_DIAGNOSTIC_INPUT_SCHEMA_VERSION)
        .then_some(())
        .ok_or_else(|| "Project UI diagnostic input schema is unsupported.".to_string())
}

fn validate_operation_id(operation_id: &str) -> Result<(), String> {
    let valid = !operation_id.is_empty()
        && operation_id.len() <= 128
        && operation_id != "."
        && operation_id != ".."
        && operation_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
    valid.then_some(()).ok_or_else(|| {
        "Visual issue operation id must be one bounded path-safe ASCII segment.".to_string()
    })
}

fn validate_bounded_field(value: &str, maximum: usize, role: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > maximum || trimmed != value {
        Err(format!(
            "{role} must contain 1-{maximum} canonical characters."
        ))
    } else {
        Ok(())
    }
}

fn issue_bundle_ref(operation_id: &str) -> Result<String, String> {
    validate_operation_id(operation_id)?;
    Ok(format!(
        "{PROJECT_VISUAL_ISSUE_ROOT}/{operation_id}/{PROJECT_VISUAL_ISSUE_FILE}"
    ))
}

fn validate_frame_evidence(
    session: &EditorSession,
    frame_evidence_ref: &str,
) -> Result<
    (
        ProjectPreviewFrameEvidence,
        crate::ProjectCandidateProjectBinding,
    ),
    String,
> {
    let retained = session.project_preview_frame_result().ok_or_else(|| {
        "No retained Preview frame result exists in the current Editor session.".to_string()
    })?;
    if retained.status != crate::ProjectPreviewFrameResultStatus::Captured
        || retained.evidence_ref.as_deref() != Some(frame_evidence_ref)
    {
        return Err(
            "Frame evidence is stale or is not the currently retained Preview result.".to_string(),
        );
    }
    let binding = ProjectCandidateEntry::inspect_project_binding(session)
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    let project = session
        .active_project_session()
        .ok_or_else(|| "Frame evidence validation requires an active project.".to_string())?;
    let frame = ProjectPreviewEvidence::read_frame(project.write_scope(), frame_evidence_ref)
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    if retained.operation_id != frame.operation_id
        || retained.captured_evidence.as_ref() != Some(&frame)
    {
        return Err(
            "Frame evidence no longer matches the trusted receipt retained by the Preview operation."
                .to_string(),
        );
    }
    if frame.project_identity != binding.project_id
        || frame.project_digest != binding.project_digest
    {
        return Err(
            "Frame evidence belongs to a different project identity or project revision."
                .to_string(),
        );
    }
    Ok((frame, binding))
}

fn read_issue_bundle(
    session: &EditorSession,
    requested_issue_bundle_ref: &str,
) -> Result<ProjectVisualIssueBundle, String> {
    let normalized = ProjectRelativePath::parse(requested_issue_bundle_ref).map_err(|error| {
        format!("Visual issue bundle reference is not project-contained: {error}")
    })?;
    let project = session
        .active_project_session()
        .ok_or_else(|| "Visual issue validation requires an active project.".to_string())?;
    let bytes = project
        .write_scope()
        .read(normalized.as_path())
        .map_err(|error| error.to_string())?;
    let bundle: ProjectVisualIssueBundle =
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    if bundle.schema_version != PROJECT_VISUAL_ISSUE_BUNDLE_SCHEMA_VERSION {
        return Err("Visual issue bundle schema is unsupported.".to_string());
    }
    validate_operation_id(&bundle.capture_operation_id)?;
    let expected_ref = issue_bundle_ref(&bundle.capture_operation_id)?;
    if normalized.as_str() != expected_ref || bundle.issue_bundle_ref != expected_ref {
        return Err("Visual issue bundle is not stored at its operation-owned path.".to_string());
    }
    if project_visual_issue_digest(&bundle)? != bundle.bundle_digest {
        return Err("Visual issue bundle content no longer matches its digest.".to_string());
    }
    let (frame, binding) = validate_frame_evidence(session, &bundle.frame_evidence_ref)?;
    if bundle.project_identity != binding.project_id
        || bundle.project_digest != binding.project_digest
        || bundle.preview_operation_id != frame.operation_id
        || bundle.frame_digest != frame.frame_digest
        || bundle.screenshot_ref != frame.screenshot_ref
        || bundle.screenshot_digest != frame.screenshot_digest
    {
        return Err(
            "Visual issue bundle no longer matches its verified Preview frame evidence."
                .to_string(),
        );
    }
    Ok(bundle)
}

fn project_visual_issue_digest(bundle: &ProjectVisualIssueBundle) -> Result<String, String> {
    let mut unsigned = bundle.clone();
    unsigned.bundle_digest.clear();
    canonical_digest(&unsigned)
}

fn visual_issue_digest(bundle: &VisualIssueBundle) -> Result<String, String> {
    let mut unsigned = bundle.clone();
    unsigned.bundle_digest.clear();
    canonical_digest(&unsigned)
}

fn canonical_digest(value: &impl Serialize) -> Result<String, String> {
    let value = serde_json::to_value(value).map_err(|error| error.to_string())?;
    canonical_json_bytes(&value)
        .map(|bytes| sha256_prefixed(&bytes))
        .map_err(|error| error.to_string())
}

fn parent_chain(document: &AuiDocument, node: &engine_runtime::aui::AuiNode) -> Vec<String> {
    let mut parents = Vec::new();
    let mut current = node.parent.as_deref();
    while let Some(parent_id) = current {
        if parents.iter().any(|value| value == parent_id) {
            break;
        }
        parents.push(parent_id.to_string());
        current = document
            .nodes
            .iter()
            .find(|candidate| candidate.node_id == parent_id)
            .and_then(|parent| parent.parent.as_deref());
    }
    parents
}

fn candidate(
    path: &str,
    document: &AuiDocument,
    node: &engine_runtime::aui::AuiNode,
) -> ProjectUiCandidate {
    ProjectUiCandidate {
        document_path: path.to_string(),
        document_id: document.document_id.clone(),
        node_id: node.node_id.clone(),
        node_name: node.name.clone(),
        node_text: node.text.clone(),
        binding_paths: sorted(node.binding_refs.iter().map(|binding| binding.path.clone())),
        action_ids: sorted(
            node.action_refs
                .iter()
                .map(|action| action.action_id.clone()),
        ),
    }
}

fn load_documents(session: &EditorSession) -> Result<Vec<(String, AuiDocument)>, String> {
    let project = session
        .active_project_session()
        .ok_or_else(|| "Project UI diagnostics require an active project.".to_string())?;
    let mut paths = Vec::new();
    for relative_root in ["UI", "Assets/UI", "AUI"] {
        let root = project.project_root.join(relative_root);
        if root.exists() {
            collect_json(&root, &mut paths)?;
        }
    }
    paths.sort();
    paths.truncate(256);
    Ok(paths
        .into_iter()
        .filter_map(|path| {
            let relative = path
                .strip_prefix(&project.project_root)
                .ok()?
                .to_string_lossy()
                .replace('\\', "/");
            let bytes = fs::read(&path).ok()?;
            if bytes.len() > 1024 * 1024 {
                return None;
            }
            serde_json::from_slice::<AuiDocument>(&bytes)
                .ok()
                .map(|document| (relative, document))
        })
        .collect::<Vec<_>>())
}

fn load_document(
    session: &EditorSession,
    requested: &str,
) -> Result<(String, AuiDocument), String> {
    load_documents(session)?
        .into_iter()
        .find(|(path, _)| path.eq_ignore_ascii_case(&requested.replace('\\', "/")))
        .ok_or_else(|| format!("AUI document '{requested}' was not found."))
}

fn collect_json(root: &Path, output: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            collect_json(&entry.path(), output)?;
        } else if entry
            .path()
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        {
            output.push(entry.path());
        }
    }
    Ok(())
}

fn sorted(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut values = values.into_iter().collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn symbol_query(value: &str) -> String {
    value
        .split(['.', '/', ':'])
        .next_back()
        .unwrap_or(value)
        .replace('-', "_")
}
