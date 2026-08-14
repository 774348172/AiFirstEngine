use super::*;
use editor_ui_model::{
    AssetPlacementMode, AssetQuery, EditorUiMode, InspectorValue, RuntimeRunState,
    UiCommandPayload, Vec3, WorkspaceDomainKind, WorkspaceDomainStatus,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

mod ai_capability_tool_kernel_tests;
mod ai_goal_grant_tests;
mod ai_service_tests;
mod ai_tool_catalog_tests;
mod aui_authoring_tests;
mod aui_scene_authoring_tests;
mod aui_template_tests;
mod authoring_workflow_tests;
mod authoring_workspace_tests;
mod build_service_tests;
mod fixtures;
mod goal_mutation_contract;
mod llm_patch_source_tests;
mod play_service_tests;
mod prefab_service_tests;
mod project_candidate_entry_tests;
mod project_intent_workflow_tests;
mod project_patch_tests;
mod project_preview_visual_evidence_tests;
mod project_service_tests;
mod runtime_service_tests;
mod scene_save_digest_contract_tests;
mod scene_service_tests;
mod session_smoke_tests;
