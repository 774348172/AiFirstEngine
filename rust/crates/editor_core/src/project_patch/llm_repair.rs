use engine_runtime::canonical_digest::{canonical_json_bytes, sha256_prefixed};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{PatchDiagnostic, ProjectPatchImportParseStatus, ProjectPatchImportResult};

const REPAIRABLE_CODES: &[&str] = &[
    "project_patch_import.parse_failed",
    "project_patch.operation_id_required",
    "project_patch.operation_id_duplicate",
    "project_patch.dependency_missing",
    "project_patch.scene.component_field_invalid",
    "project_patch.prefab.stage_field_invalid",
    "project_patch.aui.node_field_invalid",
    "project_patch.rule.payload_invalid",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepairDecision {
    Eligible,
    NotEligible,
}

pub fn import_diagnostics(result: &ProjectPatchImportResult) -> Vec<PatchDiagnostic> {
    let mut diagnostics = result.schema_diagnostics.clone();
    diagnostics.extend(result.capability_diagnostics.clone());
    if let Some(validation) = &result.validation {
        diagnostics.extend(validation.diagnostics.clone());
    }
    diagnostics
}

pub fn project_patch_import_accepted(result: &ProjectPatchImportResult) -> bool {
    result.parse_status == ProjectPatchImportParseStatus::Parsed
        && result.schema_diagnostics.is_empty()
        && result.capability_diagnostics.is_empty()
        && result
            .validation
            .as_ref()
            .is_some_and(|validation| validation.accepted)
}

pub fn repair_decision(result: &ProjectPatchImportResult) -> RepairDecision {
    let diagnostics = import_diagnostics(result);
    if !diagnostics.is_empty()
        && diagnostics
            .iter()
            .all(|diagnostic| REPAIRABLE_CODES.contains(&diagnostic.code.as_str()))
    {
        RepairDecision::Eligible
    } else {
        RepairDecision::NotEligible
    }
}

pub fn diagnostic_fingerprint(result: &ProjectPatchImportResult) -> String {
    let mut items = import_diagnostics(result)
        .into_iter()
        .map(|diagnostic| (diagnostic.code, diagnostic.operation_id, diagnostic.target))
        .collect::<Vec<_>>();
    items.sort();
    let value = json!(items);
    let bytes = canonical_json_bytes(&value)
        .expect("ProjectPatch diagnostic fingerprint input must be canonical JSON");
    sha256_prefixed(&bytes)
}

pub fn build_project_patch_repair_prompt(
    original_prompt: &str,
    candidate: &str,
    result: &ProjectPatchImportResult,
    maximum_candidate_bytes: usize,
) -> String {
    let maximum = maximum_candidate_bytes.min(candidate.len());
    let end = candidate
        .char_indices()
        .find_map(|(index, _)| (index >= maximum).then_some(index))
        .unwrap_or(candidate.len());
    let bounded_candidate = &candidate[..end];
    let diagnostics = import_diagnostics(result)
        .into_iter()
        .take(32)
        .map(|diagnostic| {
            json!({
                "code": diagnostic.code,
                "operationId": diagnostic.operation_id,
                "target": diagnostic.target,
            })
        })
        .collect::<Vec<_>>();
    format!(
        "REPAIR_PROJECT_PATCH_ONCE\n\
         Return one corrected ProjectPatchDocument JSON object. Do not expand scope, risk, capabilities, destructive operations, or Build operations.\n\
         Original request:\n{original_prompt}\n\
         Rejected candidate:\n{bounded_candidate}\n\
         Stable diagnostics:\n{}",
        serde_json::to_string(&diagnostics).expect("bounded repair diagnostics must serialize")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rejected(code: &str) -> ProjectPatchImportResult {
        ProjectPatchImportResult {
            schema_version: "project-patch-import-result.v1".to_string(),
            source_kind: crate::ProjectPatchImportSourceKind::AiStructuredOutput,
            source_label: "fixture".to_string(),
            parse_status: ProjectPatchImportParseStatus::Rejected,
            parsed_patch: None,
            schema_diagnostics: vec![PatchDiagnostic::error(code, "redacted", None, None)],
            capability_diagnostics: Vec::new(),
            validation: None,
            review: None,
            proposal_id: None,
            next_actions: Vec::new(),
        }
    }

    #[test]
    fn llm_repair_decision_uses_exact_allowlist_and_deny_by_default() {
        assert_eq!(
            repair_decision(&rejected("project_patch_import.parse_failed")),
            RepairDecision::Eligible
        );
        for code in [
            "project_patch.operations_too_many",
            "project_patch.gameplay_api_forbidden",
            "project_patch.asset.path_outside_project",
            "unknown.code",
        ] {
            assert_eq!(
                repair_decision(&rejected(code)),
                RepairDecision::NotEligible
            );
        }
    }

    #[test]
    fn llm_repair_decision_fingerprint_is_message_independent() {
        let first = rejected("project_patch_import.parse_failed");
        let mut second = first.clone();
        second.schema_diagnostics[0].message = "different prose".to_string();
        assert_eq!(
            diagnostic_fingerprint(&first),
            diagnostic_fingerprint(&second)
        );
    }
}
