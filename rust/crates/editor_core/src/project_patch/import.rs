use std::fs;
use std::io::Read;
use std::path::Path;

use crate::EditorSession;

use super::{
    PatchCapability, PatchDiagnostic, PatchReviewModel, PatchValidator, ProjectPatchDocument,
    ProjectPatchImportParseStatus, ProjectPatchImportRequest, ProjectPatchImportResult,
    PROJECT_PATCH_IMPORT_REQUEST_SCHEMA_VERSION, PROJECT_PATCH_IMPORT_RESULT_SCHEMA_VERSION,
    PROJECT_PATCH_SCHEMA_VERSION,
};

pub struct ProjectPatchImportService;

const MAX_PROJECT_PATCH_IMPORT_BYTES: usize = 8 * 1024 * 1024;

impl ProjectPatchImportService {
    pub fn from_json_string(
        session: &EditorSession,
        request: ProjectPatchImportRequest,
    ) -> ProjectPatchImportResult {
        let Some(raw_json) = request.raw_json.as_deref() else {
            return ProjectPatchImportResult::rejected(
                &request,
                vec![PatchDiagnostic::error(
                    "project_patch_import.raw_json_missing",
                    "ProjectPatchImportRequest.raw_json is required for JSON string import.",
                    None,
                    Some(request.source_label.clone()),
                )],
                vec!["provide_project_patch_json".to_string()],
            );
        };
        Self::import_raw_json(session, &request, raw_json)
    }

    pub fn from_file(
        session: &EditorSession,
        request: ProjectPatchImportRequest,
    ) -> ProjectPatchImportResult {
        let Some(file_path) = request.file_path.as_deref() else {
            return ProjectPatchImportResult::rejected(
                &request,
                vec![PatchDiagnostic::error(
                    "project_patch_import.file_path_missing",
                    "ProjectPatchImportRequest.file_path is required for file import.",
                    None,
                    Some(request.source_label.clone()),
                )],
                vec!["provide_project_patch_file_path".to_string()],
            );
        };
        let path = Path::new(file_path);
        match read_bounded_regular_utf8(path) {
            Ok(raw_json) => Self::import_raw_json(session, &request, &raw_json),
            Err(error) => ProjectPatchImportResult::rejected(
                &request,
                vec![PatchDiagnostic::error(
                    "project_patch_import.file_read_failed",
                    format!("Failed to read ProjectPatch import file: {error}"),
                    None,
                    Some(path.display().to_string()),
                )],
                vec!["fix_project_patch_import_file_path".to_string()],
            ),
        }
    }

    pub fn from_fixture(
        session: &EditorSession,
        request: ProjectPatchImportRequest,
    ) -> ProjectPatchImportResult {
        Self::from_json_string(session, request)
    }

    pub fn import_raw_json(
        session: &EditorSession,
        request: &ProjectPatchImportRequest,
        raw_json: &str,
    ) -> ProjectPatchImportResult {
        let mut schema_diagnostics = validate_import_request_schema(request);
        let parsed_patch = match serde_json::from_str::<ProjectPatchDocument>(raw_json) {
            Ok(patch) => patch,
            Err(error) => {
                schema_diagnostics.push(PatchDiagnostic::error(
                    "project_patch_import.parse_failed",
                    format!("Failed to parse ProjectPatchDocument JSON: {error}"),
                    None,
                    Some(request.source_label.clone()),
                ));
                return ProjectPatchImportResult::rejected(
                    request,
                    schema_diagnostics,
                    vec!["fix_project_patch_json_shape".to_string()],
                );
            }
        };

        schema_diagnostics.extend(validate_patch_schema(request, &parsed_patch));
        let capability_diagnostics = capability_diagnostics(&parsed_patch);
        let validation = PatchValidator::validate(session, &parsed_patch);
        let review = PatchReviewModel::from_patch(&parsed_patch, validation.clone());
        let next_actions = next_actions_for_import(
            &schema_diagnostics,
            &capability_diagnostics,
            Some(&validation),
        );
        let parse_status = if schema_diagnostics.is_empty() {
            ProjectPatchImportParseStatus::Parsed
        } else {
            ProjectPatchImportParseStatus::Rejected
        };

        ProjectPatchImportResult {
            schema_version: PROJECT_PATCH_IMPORT_RESULT_SCHEMA_VERSION.to_string(),
            source_kind: request.source_kind,
            source_label: request.source_label.clone(),
            parse_status,
            parsed_patch: Some(parsed_patch),
            schema_diagnostics,
            capability_diagnostics,
            validation: Some(validation),
            review: Some(review),
            proposal_id: None,
            next_actions,
        }
    }
}

fn read_bounded_regular_utf8(path: &Path) -> Result<String, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("ProjectPatch import file cannot be inspected: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(
            "ProjectPatch import path must be a regular file and cannot be a symlink.".to_string(),
        );
    }
    if metadata.len() > MAX_PROJECT_PATCH_IMPORT_BYTES as u64 {
        return Err(format!(
            "ProjectPatch import file exceeds the {} byte limit.",
            MAX_PROJECT_PATCH_IMPORT_BYTES
        ));
    }
    let file = fs::File::open(path)
        .map_err(|error| format!("ProjectPatch import file cannot be opened: {error}"))?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take((MAX_PROJECT_PATCH_IMPORT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("ProjectPatch import file cannot be read: {error}"))?;
    if bytes.len() > MAX_PROJECT_PATCH_IMPORT_BYTES {
        return Err(format!(
            "ProjectPatch import file exceeds the {} byte limit.",
            MAX_PROJECT_PATCH_IMPORT_BYTES
        ));
    }
    String::from_utf8(bytes)
        .map_err(|error| format!("ProjectPatch import file is not valid UTF-8: {error}"))
}

fn validate_import_request_schema(request: &ProjectPatchImportRequest) -> Vec<PatchDiagnostic> {
    let mut diagnostics = Vec::new();
    if request.schema_version != PROJECT_PATCH_IMPORT_REQUEST_SCHEMA_VERSION {
        diagnostics.push(PatchDiagnostic::error(
            "project_patch_import.request_schema_unsupported",
            format!(
                "Unsupported ProjectPatchImportRequest schema: {}",
                request.schema_version
            ),
            None,
            Some(request.source_label.clone()),
        ));
    }
    if request.source_label.trim().is_empty() {
        diagnostics.push(PatchDiagnostic::error(
            "project_patch_import.source_label_required",
            "ProjectPatch import requires a non-empty source_label.",
            None,
            None,
        ));
    }
    diagnostics
}

fn validate_patch_schema(
    request: &ProjectPatchImportRequest,
    patch: &ProjectPatchDocument,
) -> Vec<PatchDiagnostic> {
    let mut diagnostics = Vec::new();
    if patch.schema_version != PROJECT_PATCH_SCHEMA_VERSION {
        diagnostics.push(PatchDiagnostic::error(
            "project_patch_import.patch_schema_unsupported",
            format!("Unsupported ProjectPatch schema: {}", patch.schema_version),
            None,
            Some(request.source_label.clone()),
        ));
    }
    if let Some(expected_patch_id) = request.expected_patch_id.as_deref() {
        if patch.patch_id != expected_patch_id {
            diagnostics.push(PatchDiagnostic::error(
                "project_patch_import.patch_id_mismatch",
                format!(
                    "Imported ProjectPatch id {} does not match expected id {}.",
                    patch.patch_id, expected_patch_id
                ),
                None,
                Some(request.source_label.clone()),
            ));
        }
    }
    diagnostics
}

fn capability_diagnostics(patch: &ProjectPatchDocument) -> Vec<PatchDiagnostic> {
    patch
        .required_capabilities
        .iter()
        .filter(|capability| {
            !matches!(
                capability,
                PatchCapability::Scene
                    | PatchCapability::Input
                    | PatchCapability::Asset
                    | PatchCapability::Prefab
                    | PatchCapability::Aui
                    | PatchCapability::Rule
                    | PatchCapability::Build
            )
        })
        .map(|capability| {
            PatchDiagnostic::error(
                "project_patch_import.capability_not_supported",
                format!("Capability {capability:?} is not supported for imported ProjectPatch v2."),
                None,
                Some(format!("project_patch.{}", patch.patch_id)),
            )
        })
        .collect()
}

fn next_actions_for_import(
    schema_diagnostics: &[PatchDiagnostic],
    capability_diagnostics: &[PatchDiagnostic],
    validation: Option<&super::PatchValidationReport>,
) -> Vec<String> {
    let mut actions = Vec::new();
    if !schema_diagnostics.is_empty() {
        actions.push("fix_project_patch_import_schema".to_string());
    }
    if !capability_diagnostics.is_empty() {
        actions.push("defer_unsupported_project_patch_capability".to_string());
    }
    if validation.is_some_and(|validation| !validation.accepted) {
        actions.push("fix_project_patch_validation_diagnostics".to_string());
    }
    actions.sort();
    actions.dedup();
    actions
}
