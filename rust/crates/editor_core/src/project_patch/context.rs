use engine_runtime::canonical_digest::{canonical_json_bytes, sha256_prefixed};
use serde::{Deserialize, Serialize};

use crate::EditorSession;

use super::{project_patch_json_schema_hash, PatchValidator};

pub const PROJECT_PATCH_LLM_CONTEXT_SCHEMA_VERSION: &str = "project-patch-llm-context.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPatchLlmContextSnapshot {
    pub schema_version: String,
    pub project_id: Option<String>,
    pub active_scene_path: Option<String>,
    pub active_scene_id: Option<String>,
    pub selected_entity_id: Option<String>,
    pub active_authoring_step: String,
    pub authoring_summary: String,
    pub missing_required_items: Vec<String>,
    pub supported_capabilities: Vec<String>,
    pub max_operation_count: usize,
    pub project_patch_schema_hash: String,
    pub context_hash: String,
}

impl ProjectPatchLlmContextSnapshot {
    pub fn capture(session: &EditorSession) -> Self {
        let ui_model = session.build_ui_model();
        let ai_context = &ui_model.authoring_workflow.ai_context;
        let project = session.active_project_session.as_ref();
        let active_scene_path = session.scene_path.as_ref().and_then(|path| {
            let safe_path = if path.is_relative() {
                Some(path.as_path())
            } else if let Some(project) = project {
                path.strip_prefix(&project.project_root).ok()
            } else {
                path.file_name().map(std::path::Path::new)
            }?;
            Some(safe_path.to_string_lossy().replace('\\', "/"))
        });
        let mut snapshot = Self {
            schema_version: PROJECT_PATCH_LLM_CONTEXT_SCHEMA_VERSION.to_string(),
            project_id: project.map(|project| project.manifest.project_id.clone()),
            active_scene_path,
            active_scene_id: session
                .editor_scene_document
                .as_ref()
                .map(|scene| scene.scene_id.clone()),
            selected_entity_id: session.scene_selection.primary_entity_id.clone(),
            active_authoring_step: ai_context.active_step.as_str().to_string(),
            authoring_summary: ai_context.summary.clone(),
            missing_required_items: ai_context.missing_required_items.clone(),
            supported_capabilities: ["Scene", "Input", "Asset", "Prefab", "AUI", "Rule", "Build"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            max_operation_count: PatchValidator::MAX_OPERATION_COUNT,
            project_patch_schema_hash: project_patch_json_schema_hash(),
            context_hash: String::new(),
        };
        snapshot.context_hash = snapshot.semantic_hash();
        snapshot
    }

    pub fn semantic_hash(&self) -> String {
        let mut value =
            serde_json::to_value(self).expect("ProjectPatch LLM context snapshot must serialize");
        value
            .as_object_mut()
            .expect("ProjectPatch LLM context snapshot must be an object")
            .remove("contextHash");
        let bytes = canonical_json_bytes(&value)
            .expect("ProjectPatch LLM context snapshot must be canonical JSON");
        sha256_prefixed(&bytes)
    }

    pub fn prompt_json(&self) -> String {
        serde_json::to_string(self).expect("ProjectPatch LLM context snapshot must serialize")
    }
}
