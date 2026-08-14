use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use crate::{
    decode_rule_operation, decode_rule_statement, decode_rule_trigger, load_first_input_mapping,
    EditorSession, InputMappingAuthoringService, ProjectRelativePath,
};

use super::{
    AssetPatchOperation, AuiPatchOperation, BuildPatchOperation, InputPatchOperation,
    PatchCapability, PatchDiagnostic, PatchOperation, PatchValidationReport, PrefabPatchOperation,
    ProjectPatchDocument, RulePatchOperation, ScenePatchOperation, PROJECT_PATCH_SCHEMA_VERSION,
};

pub struct PatchValidator;

impl PatchValidator {
    pub const MAX_OPERATION_COUNT: usize = 48;

    pub fn validate(
        session: &EditorSession,
        patch: &ProjectPatchDocument,
    ) -> PatchValidationReport {
        let mut diagnostics = Vec::new();

        if patch.schema_version != PROJECT_PATCH_SCHEMA_VERSION {
            diagnostics.push(PatchDiagnostic::error(
                "project_patch.schema_unsupported",
                format!("Unsupported ProjectPatch schema: {}", patch.schema_version),
                None,
                None,
            ));
        }
        if patch.operations.is_empty() {
            diagnostics.push(PatchDiagnostic::error(
                "project_patch.operations_empty",
                "ProjectPatch requires at least one operation.",
                None,
                None,
            ));
        }
        if patch.operations.len() > Self::MAX_OPERATION_COUNT {
            diagnostics.push(PatchDiagnostic::error(
                "project_patch.operations_too_many",
                format!(
                    "ProjectPatch has {} operations; All-Domain A-min limit is {}.",
                    patch.operations.len(),
                    Self::MAX_OPERATION_COUNT
                ),
                None,
                None,
            ));
        }
        validate_required_capabilities(patch, &mut diagnostics);

        validate_operation_ids(patch, &mut diagnostics);
        validate_dependencies(patch, &mut diagnostics);
        validate_forbidden_gameplay_api(patch, &mut diagnostics);
        validate_scene(session, patch, &mut diagnostics);
        validate_input(session, patch, &mut diagnostics);
        validate_asset(session, patch, &mut diagnostics);
        validate_prefab(session, patch, &mut diagnostics);
        validate_aui(session, patch, &mut diagnostics);
        validate_rule(session, patch, &mut diagnostics);
        validate_build(session, patch, &mut diagnostics);

        if diagnostics.is_empty() {
            PatchValidationReport::accepted(patch)
        } else {
            PatchValidationReport::rejected(patch, diagnostics)
        }
    }
}

fn validate_required_capabilities(
    patch: &ProjectPatchDocument,
    diagnostics: &mut Vec<PatchDiagnostic>,
) {
    let supported = [
        PatchCapability::Scene,
        PatchCapability::Input,
        PatchCapability::Asset,
        PatchCapability::Prefab,
        PatchCapability::Aui,
        PatchCapability::Rule,
        PatchCapability::Build,
    ];
    for capability in &patch.required_capabilities {
        if !supported.contains(capability) {
            diagnostics.push(PatchDiagnostic::error(
                "project_patch.capability_unknown",
                format!("Capability {capability:?} is not recognized by ProjectPatch v2."),
                None,
                None,
            ));
        }
    }
}

fn validate_operation_ids(patch: &ProjectPatchDocument, diagnostics: &mut Vec<PatchDiagnostic>) {
    let mut ids = BTreeSet::new();
    for operation in &patch.operations {
        let operation_id = operation.operation_id().trim();
        if operation_id.is_empty() {
            diagnostics.push(PatchDiagnostic::error(
                "project_patch.operation_id_required",
                "PatchOperation operation_id is required.",
                None,
                Some(operation.target_summary()),
            ));
        } else if !ids.insert(operation_id.to_string()) {
            diagnostics.push(PatchDiagnostic::error(
                "project_patch.operation_id_duplicate",
                format!("Duplicate operation_id: {operation_id}"),
                Some(operation_id.to_string()),
                Some(operation.target_summary()),
            ));
        }
    }
}

fn validate_dependencies(patch: &ProjectPatchDocument, diagnostics: &mut Vec<PatchDiagnostic>) {
    let ids = patch
        .operations
        .iter()
        .map(|operation| operation.operation_id().to_string())
        .collect::<BTreeSet<_>>();
    for operation in &patch.operations {
        for dependency in operation.depends_on() {
            if !ids.contains(dependency) {
                diagnostics.push(PatchDiagnostic::error(
                    "project_patch.dependency_missing",
                    format!(
                        "Operation {} depends on missing operation {}.",
                        operation.operation_id(),
                        dependency
                    ),
                    Some(operation.operation_id().to_string()),
                    Some(operation.target_summary()),
                ));
            }
        }
    }
}

fn validate_forbidden_gameplay_api(
    patch: &ProjectPatchDocument,
    diagnostics: &mut Vec<PatchDiagnostic>,
) {
    let forbidden = [
        "player", "enemy", "bullet", "health", "score", "weapon", "boss", "wave",
    ];
    for operation in &patch.operations {
        let text = serde_json::to_string(operation)
            .unwrap_or_default()
            .to_ascii_lowercase();
        if let Some(found) = forbidden
            .iter()
            .find(|word| text.contains(&format!("engine.{word}")))
        {
            diagnostics.push(PatchDiagnostic::error(
                "project_patch.gameplay_api_forbidden",
                format!("Engine-specific gameplay API is forbidden: engine.{found}"),
                Some(operation.operation_id().to_string()),
                Some(operation.target_summary()),
            ));
        }
    }
}

fn validate_asset(
    session: &EditorSession,
    patch: &ProjectPatchDocument,
    diagnostics: &mut Vec<PatchDiagnostic>,
) {
    if !patch
        .operations
        .iter()
        .any(|operation| matches!(operation, PatchOperation::Asset(_)))
    {
        return;
    }
    validate_project_context(
        session,
        "project_patch.asset.no_project",
        "Asset patch requires an active project.",
        diagnostics,
    );
    for operation in &patch.operations {
        let PatchOperation::Asset(asset_operation) = operation else {
            continue;
        };
        match asset_operation {
            AssetPatchOperation::RegisterExistingAsset {
                operation_id,
                path,
                expected_kind,
                ..
            } => {
                validate_project_relative_path(
                    path,
                    operation_id,
                    "project_patch.asset.path_invalid",
                    diagnostics,
                );
                if expected_kind.is_some_and(|kind| {
                    matches!(
                        kind,
                        editor_ui_model::AssetKind::Folder | editor_ui_model::AssetKind::Unknown
                    )
                }) {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.asset.expected_kind_invalid",
                        "RegisterExistingAsset expected_kind cannot be Folder or Unknown.",
                        Some(operation_id.clone()),
                        Some(path.clone()),
                    ));
                }
            }
            AssetPatchOperation::GenerateMockImageAsset {
                operation_id,
                prompt,
                target_folder,
                asset_name,
                image_kind,
                width,
                height,
                ..
            } => {
                if prompt.trim().is_empty() || asset_name.trim().is_empty() {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.asset.generation_request_invalid",
                        "GenerateMockImageAsset requires non-empty prompt and asset_name.",
                        Some(operation_id.clone()),
                        Some(target_folder.clone()),
                    ));
                }
                validate_project_relative_path(
                    target_folder,
                    operation_id,
                    "project_patch.asset.target_folder_invalid",
                    diagnostics,
                );
                if !matches!(
                    image_kind.as_str(),
                    "texture"
                        | "Texture"
                        | "sprite"
                        | "Sprite"
                        | "uiImage"
                        | "ui_image"
                        | "UiImage"
                        | "referenceImage"
                        | "reference_image"
                        | "ReferenceImage"
                ) {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.asset.image_kind_invalid",
                        format!("Unsupported image_kind: {image_kind}"),
                        Some(operation_id.clone()),
                        Some(target_folder.clone()),
                    ));
                }
                if *width == 0 || *height == 0 || *width > 4096 || *height > 4096 {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.asset.image_size_invalid",
                        "GenerateMockImageAsset width and height must be between 1 and 4096.",
                        Some(operation_id.clone()),
                        Some(target_folder.clone()),
                    ));
                }
            }
            AssetPatchOperation::ValidateAssetBrowserIndex { .. } => {}
        }
    }
}

fn validate_prefab(
    session: &EditorSession,
    patch: &ProjectPatchDocument,
    diagnostics: &mut Vec<PatchDiagnostic>,
) {
    if !patch
        .operations
        .iter()
        .any(|operation| matches!(operation, PatchOperation::Prefab(_)))
    {
        return;
    }
    validate_project_context(
        session,
        "project_patch.prefab.no_project",
        "Prefab patch requires an active project.",
        diagnostics,
    );
    for operation in &patch.operations {
        let PatchOperation::Prefab(prefab_operation) = operation else {
            continue;
        };
        match prefab_operation {
            PrefabPatchOperation::CreateFromSceneSelection {
                operation_id,
                scene_path,
                root_entity_id,
                prefab_id,
                name,
                ..
            } => {
                if root_entity_id.trim().is_empty()
                    || prefab_id.trim().is_empty()
                    || name.trim().is_empty()
                {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.prefab.create_invalid",
                        "CreateFromSceneSelection requires root_entity_id, prefab_id, and name.",
                        Some(operation_id.clone()),
                        Some(format!("prefab.id={prefab_id}")),
                    ));
                }
                if let Some(scene_path) = scene_path {
                    validate_project_relative_path(
                        scene_path,
                        operation_id,
                        "project_patch.prefab.scene_path_invalid",
                        diagnostics,
                    );
                }
            }
            PrefabPatchOperation::OpenDocument {
                operation_id, path, ..
            }
            | PrefabPatchOperation::SaveDocument {
                operation_id, path, ..
            } => validate_prefab_path(path, operation_id, diagnostics),
            PrefabPatchOperation::SetStageEntityField {
                operation_id,
                source_entity_id,
                field_path,
                value,
                ..
            } => {
                if source_entity_id.trim().is_empty()
                    || field_path.trim().is_empty()
                    || value.is_null()
                {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.prefab.stage_field_invalid",
                        "SetStageEntityField requires source_entity_id, field_path, and non-null value.",
                        Some(operation_id.clone()),
                        Some(source_entity_id.clone()),
                    ));
                }
            }
            PrefabPatchOperation::InstantiateInScene {
                operation_id,
                prefab_id,
                parent_entity_id,
                ..
            } => {
                if prefab_id.trim().is_empty() {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.prefab.prefab_id_required",
                        "InstantiateInScene requires a non-empty prefab_id.",
                        Some(operation_id.clone()),
                        None,
                    ));
                }
                if let Some(parent_entity_id) = parent_entity_id {
                    if let Some(document) = session.editor_scene_document.as_ref() {
                        if !document.has_entity(parent_entity_id) {
                            diagnostics.push(PatchDiagnostic::error(
                                "project_patch.prefab.parent_missing",
                                format!(
                                    "Prefab instantiate parent does not exist: {parent_entity_id}"
                                ),
                                Some(operation_id.clone()),
                                Some(parent_entity_id.clone()),
                            ));
                        }
                    }
                }
            }
            PrefabPatchOperation::ApplyOverrideToAsset {
                operation_id,
                instance_entity_id,
                target_source_entity_id,
                component_type,
                field_path,
                ..
            }
            | PrefabPatchOperation::RevertOverride {
                operation_id,
                instance_entity_id,
                target_source_entity_id,
                component_type,
                field_path,
                ..
            } => {
                if instance_entity_id.trim().is_empty()
                    || target_source_entity_id.trim().is_empty()
                    || component_type.trim().is_empty()
                    || field_path.trim().is_empty()
                {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.prefab.override_invalid",
                        "Prefab override operations require instance, source entity, component_type, and field_path.",
                        Some(operation_id.clone()),
                        Some(instance_entity_id.clone()),
                    ));
                }
            }
            PrefabPatchOperation::ValidateReferences {
                operation_id, path, ..
            } => {
                if let Some(path) = path {
                    validate_prefab_path(path, operation_id, diagnostics);
                }
            }
        }
    }
}

fn validate_aui(
    session: &EditorSession,
    patch: &ProjectPatchDocument,
    diagnostics: &mut Vec<PatchDiagnostic>,
) {
    if !patch
        .operations
        .iter()
        .any(|operation| matches!(operation, PatchOperation::Aui(_)))
    {
        return;
    }
    validate_project_context(
        session,
        "project_patch.aui.no_project",
        "AUI patch requires an active project.",
        diagnostics,
    );
    for operation in &patch.operations {
        let PatchOperation::Aui(aui_operation) = operation else {
            continue;
        };
        let operation_id = aui_operation.operation_id();
        validate_aui_path(aui_operation.path(), operation_id, diagnostics);
        match aui_operation {
            AuiPatchOperation::CreateDocument {
                document_id,
                width,
                height,
                ..
            } => {
                if document_id.trim().is_empty() || *width <= 0.0 || *height <= 0.0 {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.aui.create_invalid",
                        "CreateDocument requires document_id and positive width/height.",
                        Some(operation_id.to_string()),
                        Some(aui_operation.path().to_string()),
                    ));
                }
            }
            AuiPatchOperation::AddNode {
                parent_node_id,
                node_id,
                node_kind,
                rect,
                ..
            } => {
                if parent_node_id.trim().is_empty()
                    || node_id.trim().is_empty()
                    || node_kind.trim().is_empty()
                    || !rect.is_object()
                {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.aui.node_invalid",
                        "AddNode requires parent_node_id, node_id, node_kind, and object rect.",
                        Some(operation_id.to_string()),
                        Some(aui_operation.path().to_string()),
                    ));
                }
            }
            AuiPatchOperation::SetNodeField {
                node_id,
                schema_path,
                value,
                ..
            } => {
                if node_id.trim().is_empty() || schema_path.trim().is_empty() || value.is_null() {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.aui.node_field_invalid",
                        "SetNodeField requires node_id, schema_path, and non-null value.",
                        Some(operation_id.to_string()),
                        Some(aui_operation.path().to_string()),
                    ));
                }
            }
            AuiPatchOperation::SetBindingPath {
                node_id,
                target_field,
                binding_id,
                binding_path,
                ..
            } => {
                if node_id.trim().is_empty()
                    || target_field.trim().is_empty()
                    || binding_id.trim().is_empty()
                    || binding_path.trim().is_empty()
                {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.aui.binding_invalid",
                        "SetBindingPath requires node_id, target_field, binding_id, and binding_path.",
                        Some(operation_id.to_string()),
                        Some(aui_operation.path().to_string()),
                    ));
                }
            }
            AuiPatchOperation::SetActionRef {
                node_id,
                event,
                action_id,
                ..
            } => {
                if node_id.trim().is_empty()
                    || event.trim().is_empty()
                    || action_id.trim().is_empty()
                {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.aui.action_invalid",
                        "SetActionRef requires node_id, event, and action_id.",
                        Some(operation_id.to_string()),
                        Some(aui_operation.path().to_string()),
                    ));
                }
            }
            AuiPatchOperation::OpenDocument { .. }
            | AuiPatchOperation::ValidateDocument { .. }
            | AuiPatchOperation::SaveDocument { .. }
            | AuiPatchOperation::PreviewOverlay { .. } => {}
        }
    }
}

fn validate_rule(
    session: &EditorSession,
    patch: &ProjectPatchDocument,
    diagnostics: &mut Vec<PatchDiagnostic>,
) {
    if !patch
        .operations
        .iter()
        .any(|operation| matches!(operation, PatchOperation::Rule(_)))
    {
        return;
    }
    validate_project_context(
        session,
        "project_patch.rule.no_project",
        "Rule patch requires an active project.",
        diagnostics,
    );
    for operation in &patch.operations {
        let PatchOperation::Rule(rule_operation) = operation else {
            continue;
        };
        let operation_id = rule_operation.operation_id();
        if !matches!(
            rule_operation,
            RulePatchOperation::BuildProjectManifest { .. }
        ) {
            validate_rule_path(rule_operation.path(), operation_id, diagnostics);
        }
        match rule_operation {
            RulePatchOperation::CreateAsset {
                rule_id,
                display_name,
                phase,
                ..
            } => {
                if rule_id.trim().is_empty() || display_name.trim().is_empty() {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.rule.create_invalid",
                        "CreateAsset requires rule_id and display_name.",
                        Some(operation_id.to_string()),
                        Some(rule_operation.path().to_string()),
                    ));
                }
                if phase.as_deref().is_some_and(|phase| {
                    !matches!(
                        phase,
                        "FixedUpdate" | "Update" | "PostPhysics" | "EventHandler"
                    )
                }) {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.rule.create_phase_invalid",
                        "CreateAsset phase must be FixedUpdate, Update, PostPhysics, or EventHandler.",
                        Some(operation_id.to_string()),
                        Some(rule_operation.path().to_string()),
                    ));
                }
            }
            RulePatchOperation::SetTrigger { trigger, .. } => {
                validate_rule_payload(
                    operation_id,
                    rule_operation.path(),
                    decode_rule_trigger(trigger.clone()).map(|_| ()),
                    diagnostics,
                );
            }
            RulePatchOperation::AddStatement { statement, .. }
            | RulePatchOperation::UpdateStatement { statement, .. } => {
                validate_rule_payload(
                    operation_id,
                    rule_operation.path(),
                    decode_rule_statement(statement.clone()).map(|_| ()),
                    diagnostics,
                );
            }
            RulePatchOperation::AddOperation { operation, .. }
            | RulePatchOperation::UpdateOperation { operation, .. } => {
                validate_rule_payload(
                    operation_id,
                    rule_operation.path(),
                    decode_rule_operation(operation.clone()).map(|_| ()),
                    diagnostics,
                );
            }
            RulePatchOperation::RemoveStatement { .. }
            | RulePatchOperation::RemoveOperation { .. }
            | RulePatchOperation::OpenAsset { .. }
            | RulePatchOperation::ValidateAsset { .. }
            | RulePatchOperation::BuildArtifact { .. } => {}
            RulePatchOperation::BuildProjectManifest { path, .. } => {
                if path.replace('\\', "/") != "Rules/rule-manifest.json" {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.rule.manifest_path_invalid",
                        "BuildProjectManifest must target Rules/rule-manifest.json.",
                        Some(operation_id.to_string()),
                        Some(path.clone()),
                    ));
                }
            }
        }
        validate_expected_ir_hash(rule_operation, diagnostics);
    }
}

fn validate_build(
    session: &EditorSession,
    patch: &ProjectPatchDocument,
    diagnostics: &mut Vec<PatchDiagnostic>,
) {
    if !patch
        .operations
        .iter()
        .any(|operation| matches!(operation, PatchOperation::Build(_)))
    {
        return;
    }
    validate_project_context(
        session,
        "project_patch.build.no_project",
        "Build patch requires an active project.",
        diagnostics,
    );
    let mut seen_build = false;
    for operation in &patch.operations {
        match operation {
            PatchOperation::Build(build_operation) => {
                seen_build = true;
                if let BuildPatchOperation::ExportDesktopPackage {
                    operation_id,
                    profile_id,
                    ..
                } = build_operation
                {
                    if profile_id
                        .as_deref()
                        .is_some_and(|profile| profile != "windows-dev")
                    {
                        diagnostics.push(PatchDiagnostic::error(
                            "project_patch.build.profile_unsupported",
                            "Build.ExportDesktopPackage only supports default or windows-dev profile in A-min.",
                            Some(operation_id.clone()),
                            profile_id.clone(),
                        ));
                    }
                }
            }
            _ if seen_build => diagnostics.push(PatchDiagnostic::error(
                "project_patch.build.order_invalid",
                "Build operations must run after project mutation operations.",
                Some(operation.operation_id().to_string()),
                Some(operation.target_summary()),
            )),
            _ => {}
        }
    }
}

fn validate_project_context(
    session: &EditorSession,
    code: &str,
    message: &str,
    diagnostics: &mut Vec<PatchDiagnostic>,
) {
    if session.active_project_session.is_none() {
        diagnostics.push(PatchDiagnostic::error(
            code,
            message,
            None,
            Some("project_session".to_string()),
        ));
    }
}

fn validate_prefab_path(path: &str, operation_id: &str, diagnostics: &mut Vec<PatchDiagnostic>) {
    validate_project_relative_path(
        path,
        operation_id,
        "project_patch.prefab.path_invalid",
        diagnostics,
    );
    let lower = path.replace('\\', "/").to_ascii_lowercase();
    if !lower.starts_with("prefabs/") || !lower.ends_with(".prefab.json") {
        diagnostics.push(PatchDiagnostic::error(
            "project_patch.prefab.path_not_prefab",
            "Prefab path must be under Prefabs/ and end with .prefab.json.",
            Some(operation_id.to_string()),
            Some(path.to_string()),
        ));
    }
}

fn validate_aui_path(path: &str, operation_id: &str, diagnostics: &mut Vec<PatchDiagnostic>) {
    validate_project_relative_path(
        path,
        operation_id,
        "project_patch.aui.path_invalid",
        diagnostics,
    );
    let lower = path.replace('\\', "/").to_ascii_lowercase();
    let in_supported_root = lower.starts_with("ui/") || lower.starts_with("assets/ui/");
    if !in_supported_root || !lower.ends_with(".aui.json") {
        diagnostics.push(PatchDiagnostic::error(
            "project_patch.aui.path_not_aui",
            "AUI path must be under UI/ or Assets/UI/ and end with .aui.json.",
            Some(operation_id.to_string()),
            Some(path.to_string()),
        ));
    }
}

fn validate_rule_path(path: &str, operation_id: &str, diagnostics: &mut Vec<PatchDiagnostic>) {
    validate_project_relative_path(
        path,
        operation_id,
        "project_patch.rule.path_invalid",
        diagnostics,
    );
    let lower = path.replace('\\', "/").to_ascii_lowercase();
    if !lower.starts_with("rules/") || !lower.ends_with(".rule.json") {
        diagnostics.push(PatchDiagnostic::error(
            "project_patch.rule.path_not_rule",
            "Rule path must be under Rules/ and end with .rule.json.",
            Some(operation_id.to_string()),
            Some(path.to_string()),
        ));
    }
}

fn validate_project_relative_path(
    path: &str,
    operation_id: &str,
    code: &str,
    diagnostics: &mut Vec<PatchDiagnostic>,
) {
    if path.trim().is_empty() {
        diagnostics.push(PatchDiagnostic::error(
            code,
            "Project-relative path is required.",
            Some(operation_id.to_string()),
            Some(path.to_string()),
        ));
        return;
    }
    let path = Path::new(path);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::Prefix(_) | Component::RootDir
            )
        })
    {
        diagnostics.push(PatchDiagnostic::error(
            code,
            "Path must be project-relative and cannot escape the project root.",
            Some(operation_id.to_string()),
            Some(path.display().to_string()),
        ));
    }
}

fn validate_rule_payload(
    operation_id: &str,
    path: &str,
    result: Result<(), String>,
    diagnostics: &mut Vec<PatchDiagnostic>,
) {
    if let Err(message) = result {
        diagnostics.push(PatchDiagnostic::error(
            "project_patch.rule.payload_invalid",
            message,
            Some(operation_id.to_string()),
            Some(path.to_string()),
        ));
    }
}

fn validate_expected_ir_hash(
    operation: &RulePatchOperation,
    diagnostics: &mut Vec<PatchDiagnostic>,
) {
    let expected_ir_hash = match operation {
        RulePatchOperation::SetTrigger {
            expected_ir_hash, ..
        }
        | RulePatchOperation::AddStatement {
            expected_ir_hash, ..
        }
        | RulePatchOperation::UpdateStatement {
            expected_ir_hash, ..
        }
        | RulePatchOperation::RemoveStatement {
            expected_ir_hash, ..
        }
        | RulePatchOperation::AddOperation {
            expected_ir_hash, ..
        }
        | RulePatchOperation::UpdateOperation {
            expected_ir_hash, ..
        }
        | RulePatchOperation::RemoveOperation {
            expected_ir_hash, ..
        } => expected_ir_hash,
        RulePatchOperation::CreateAsset { .. }
        | RulePatchOperation::OpenAsset { .. }
        | RulePatchOperation::ValidateAsset { .. }
        | RulePatchOperation::BuildArtifact { .. }
        | RulePatchOperation::BuildProjectManifest { .. } => return,
    };
    if expected_ir_hash
        .as_deref()
        .is_some_and(|hash| hash.trim().is_empty())
    {
        diagnostics.push(PatchDiagnostic::error(
            "project_patch.rule.expected_ir_hash_invalid",
            "expected_ir_hash must be omitted or non-empty.",
            Some(operation.operation_id().to_string()),
            Some(operation.path().to_string()),
        ));
    }
}

fn validate_scene(
    session: &EditorSession,
    patch: &ProjectPatchDocument,
    diagnostics: &mut Vec<PatchDiagnostic>,
) {
    let has_scene_operation = patch
        .operations
        .iter()
        .any(|operation| matches!(operation, PatchOperation::Scene(_)));
    if !has_scene_operation {
        return;
    }
    let Some(document) = session.editor_scene_document.as_ref() else {
        diagnostics.push(PatchDiagnostic::error(
            "project_patch.scene_not_loaded",
            "Scene patch requires an open editable Scene document.",
            None,
            Some("editor_scene_document".to_string()),
        ));
        return;
    };

    let mut deleted_entities = BTreeSet::new();
    let mut created_names = BTreeSet::new();
    let mut available_components = document
        .entities
        .iter()
        .map(|entity| {
            (
                entity.entity_id.clone(),
                entity
                    .components
                    .iter()
                    .map(|component| component.component_type.clone())
                    .collect::<BTreeSet<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for operation in &patch.operations {
        let PatchOperation::Scene(scene_operation) = operation else {
            continue;
        };
        match scene_operation {
            ScenePatchOperation::CreateEntity {
                operation_id,
                parent_id,
                name,
                ..
            } => {
                if name.trim().is_empty() {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.scene.entity_name_required",
                        "CreateEntity requires a non-empty name.",
                        Some(operation_id.clone()),
                        None,
                    ));
                }
                if !created_names.insert(name.clone()) {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.scene.entity_name_duplicate_in_patch",
                        format!("Patch creates duplicate entity name: {name}"),
                        Some(operation_id.clone()),
                        None,
                    ));
                }
                if let Some(parent_id) = parent_id {
                    if !available_components.contains_key(parent_id) {
                        diagnostics.push(PatchDiagnostic::error(
                            "project_patch.scene.parent_missing",
                            format!("CreateEntity parent does not exist: {parent_id}"),
                            Some(operation_id.clone()),
                            Some(parent_id.clone()),
                        ));
                    }
                }
                if !name.trim().is_empty() {
                    let entity_id = next_virtual_entity_id(
                        available_components.keys().map(String::as_str),
                        name,
                    );
                    available_components.insert(entity_id, BTreeSet::new());
                }
            }
            ScenePatchOperation::DeleteEntity {
                operation_id,
                entity_id,
                ..
            } => {
                if !available_components.contains_key(entity_id) {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.scene.entity_missing",
                        format!("DeleteEntity target does not exist: {entity_id}"),
                        Some(operation_id.clone()),
                        Some(entity_id.clone()),
                    ));
                }
                deleted_entities.insert(entity_id.clone());
                available_components.remove(entity_id);
            }
            ScenePatchOperation::RenameEntity {
                operation_id,
                entity_id,
                name,
                ..
            } => {
                validate_existing_entity(
                    available_components.contains_key(entity_id),
                    operation_id,
                    entity_id,
                    diagnostics,
                );
                if name.trim().is_empty() {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.scene.entity_name_required",
                        "RenameEntity requires a non-empty name.",
                        Some(operation_id.clone()),
                        Some(entity_id.clone()),
                    ));
                }
            }
            ScenePatchOperation::SetTransform {
                operation_id,
                entity_id,
                ..
            } => validate_existing_entity(
                available_components.contains_key(entity_id),
                operation_id,
                entity_id,
                diagnostics,
            ),
            ScenePatchOperation::AddComponent {
                operation_id,
                entity_id,
                component_type,
                fields,
                ..
            } => {
                validate_existing_entity(
                    available_components.contains_key(entity_id),
                    operation_id,
                    entity_id,
                    diagnostics,
                );
                if component_type.trim().is_empty() || !fields.is_object() {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.scene.component_invalid",
                        "AddComponent requires a non-empty component_type and object fields.",
                        Some(operation_id.clone()),
                        Some(entity_id.clone()),
                    ));
                } else if let Some(components) = available_components.get_mut(entity_id) {
                    if !components.insert(component_type.clone()) {
                        diagnostics.push(PatchDiagnostic::error(
                            "project_patch.scene.component_duplicate",
                            format!("Entity {entity_id} already has component {component_type}."),
                            Some(operation_id.clone()),
                            Some(entity_id.clone()),
                        ));
                    }
                }
            }
            ScenePatchOperation::RemoveComponent {
                operation_id,
                entity_id,
                component_type,
                ..
            } => {
                validate_existing_entity(
                    available_components.contains_key(entity_id),
                    operation_id,
                    entity_id,
                    diagnostics,
                );
                if component_type.trim().is_empty() {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.scene.component_invalid",
                        "RemoveComponent requires a non-empty component_type.",
                        Some(operation_id.clone()),
                        Some(entity_id.clone()),
                    ));
                } else if let Some(components) = available_components.get_mut(entity_id) {
                    if !components.remove(component_type) {
                        diagnostics.push(PatchDiagnostic::error(
                            "project_patch.scene.component_missing",
                            format!("Entity {entity_id} does not have component {component_type}."),
                            Some(operation_id.clone()),
                            Some(entity_id.clone()),
                        ));
                    }
                }
            }
            ScenePatchOperation::SetComponentField {
                operation_id,
                entity_id,
                component_type,
                field_path,
                ..
            } => {
                validate_existing_entity(
                    available_components.contains_key(entity_id),
                    operation_id,
                    entity_id,
                    diagnostics,
                );
                if component_type.trim().is_empty() || field_path.trim().is_empty() {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.scene.component_field_invalid",
                        "SetComponentField requires non-empty component_type and field_path.",
                        Some(operation_id.clone()),
                        Some(entity_id.clone()),
                    ));
                } else if available_components
                    .get(entity_id)
                    .is_some_and(|components| !components.contains(component_type))
                {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.scene.component_missing",
                        format!("Entity {entity_id} does not have component {component_type}."),
                        Some(operation_id.clone()),
                        Some(entity_id.clone()),
                    ));
                }
            }
            ScenePatchOperation::PlaceAssetIntoScene {
                operation_id,
                asset_id,
                asset_type,
                target_parent_id,
                ..
            } => {
                if asset_id.trim().is_empty() || asset_type.trim().is_empty() {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.scene.asset_ref_invalid",
                        "PlaceAssetIntoScene requires non-empty asset_id and asset_type.",
                        Some(operation_id.clone()),
                        Some(asset_id.clone()),
                    ));
                }
                if let Some(parent_id) = target_parent_id {
                    if !available_components.contains_key(parent_id) {
                        diagnostics.push(PatchDiagnostic::error(
                            "project_patch.scene.parent_missing",
                            format!("PlaceAssetIntoScene parent does not exist: {parent_id}"),
                            Some(operation_id.clone()),
                            Some(parent_id.clone()),
                        ));
                    }
                }
            }
        }
    }

    for operation in &patch.operations {
        let PatchOperation::Scene(scene_operation) = operation else {
            continue;
        };
        let target = match scene_operation {
            ScenePatchOperation::DeleteEntity { .. } | ScenePatchOperation::CreateEntity { .. } => {
                None
            }
            ScenePatchOperation::RenameEntity { entity_id, .. }
            | ScenePatchOperation::SetTransform { entity_id, .. }
            | ScenePatchOperation::AddComponent { entity_id, .. }
            | ScenePatchOperation::RemoveComponent { entity_id, .. }
            | ScenePatchOperation::SetComponentField { entity_id, .. } => Some(entity_id),
            ScenePatchOperation::PlaceAssetIntoScene {
                target_parent_id, ..
            } => target_parent_id.as_ref(),
        };
        if let Some(entity_id) = target {
            if deleted_entities.contains(entity_id) {
                diagnostics.push(PatchDiagnostic::error(
                    "project_patch.scene.update_deleted_entity",
                    format!("Patch deletes and updates the same entity: {entity_id}"),
                    Some(operation.operation_id().to_string()),
                    Some(entity_id.clone()),
                ));
            }
        }
    }
}

fn next_virtual_entity_id<'a>(entity_ids: impl Iterator<Item = &'a str>, name: &str) -> String {
    let existing = entity_ids.collect::<BTreeSet<_>>();
    let slug = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    let base = if slug.is_empty() {
        "entity".to_string()
    } else {
        format!("entity-{slug}")
    };
    if !existing.contains(base.as_str()) {
        return base;
    }
    for index in 2.. {
        let candidate = format!("{base}-{index}");
        if !existing.contains(candidate.as_str()) {
            return candidate;
        }
    }
    unreachable!("unbounded entity id generation should not terminate")
}

fn validate_existing_entity(
    exists: bool,
    operation_id: &str,
    entity_id: &str,
    diagnostics: &mut Vec<PatchDiagnostic>,
) {
    if !exists {
        diagnostics.push(PatchDiagnostic::error(
            "project_patch.scene.entity_missing",
            format!("Scene entity does not exist: {entity_id}"),
            Some(operation_id.to_string()),
            Some(entity_id.to_string()),
        ));
    }
}

fn validate_input(
    session: &EditorSession,
    patch: &ProjectPatchDocument,
    diagnostics: &mut Vec<PatchDiagnostic>,
) {
    let Some(project_session) = session.active_project_session.as_ref() else {
        if patch
            .operations
            .iter()
            .any(|operation| matches!(operation, PatchOperation::Input(_)))
        {
            diagnostics.push(PatchDiagnostic::error(
                "project_patch.input.no_project",
                "Input patch requires an active project.",
                None,
                Some("project_session".to_string()),
            ));
        }
        return;
    };

    let mut action_ids_by_path = BTreeMap::<String, BTreeSet<String>>::new();
    for operation in &patch.operations {
        let PatchOperation::Input(input_operation) = operation else {
            continue;
        };
        let path = input_operation.path();
        validate_project_relative_path(
            path,
            operation.operation_id(),
            "project_patch.input.path_invalid",
            diagnostics,
        );
        let relative_path = ProjectRelativePath::parse(path).ok();
        let path_exists = relative_path.as_ref().is_some_and(|relative_path| {
            project_session
                .write_scope()
                .try_exists(relative_path.as_path())
                .unwrap_or(false)
        });
        match input_operation {
            InputPatchOperation::CreateDefaultInputMapping { operation_id, .. } => {
                if path_exists {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.input.mapping_already_exists",
                        "CreateDefaultInputMapping cannot overwrite an existing mapping.",
                        Some(operation_id.clone()),
                        Some(path.to_string()),
                    ));
                }
            }
            InputPatchOperation::DeleteInputMapping { operation_id, .. } => {
                if !path_exists {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.input.mapping_missing",
                        "DeleteInputMapping target does not exist.",
                        Some(operation_id.clone()),
                        Some(path.to_string()),
                    ));
                }
            }
            InputPatchOperation::AddInputAction {
                operation_id,
                action_id,
                ..
            } => {
                if action_id.trim().is_empty() {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.input.action_required",
                        "AddInputAction requires a non-empty action_id.",
                        Some(operation_id.clone()),
                        Some(path.to_string()),
                    ));
                }
                let ids = action_ids_by_path.entry(path.to_string()).or_default();
                if !ids.insert(action_id.clone()) {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.input.action_duplicate_in_patch",
                        format!("Patch creates duplicate input action: {action_id}"),
                        Some(operation_id.clone()),
                        Some(path.to_string()),
                    ));
                }
                if let Ok(mapping) =
                    InputMappingAuthoringService::load(&project_session.project_root, path)
                {
                    if mapping.actions.iter().any(|action| action.id == *action_id) {
                        diagnostics.push(PatchDiagnostic::error(
                            "project_patch.input.action_already_exists",
                            format!("Input action already exists: {action_id}"),
                            Some(operation_id.clone()),
                            Some(path.to_string()),
                        ));
                    }
                }
            }
            InputPatchOperation::AddInputBinding {
                operation_id,
                action_id,
                device_path,
                ..
            } => {
                if action_id.trim().is_empty() || device_path.trim().is_empty() {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.input.binding_invalid",
                        "AddInputBinding requires non-empty action_id and device_path.",
                        Some(operation_id.clone()),
                        Some(path.to_string()),
                    ));
                }
                let action_exists_in_patch = action_ids_by_path
                    .get(path)
                    .is_some_and(|ids| ids.contains(action_id));
                let action_exists_in_file =
                    InputMappingAuthoringService::load(&project_session.project_root, path)
                        .map(|mapping| mapping.actions.iter().any(|action| action.id == *action_id))
                        .unwrap_or(false);
                if !action_exists_in_patch && !action_exists_in_file {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.input.action_missing_for_binding",
                        format!("Input binding references missing action: {action_id}"),
                        Some(operation_id.clone()),
                        Some(path.to_string()),
                    ));
                }
            }
            InputPatchOperation::RemoveInputAction {
                operation_id,
                action_id,
                ..
            } => {
                let action_exists_in_file =
                    InputMappingAuthoringService::load(&project_session.project_root, path)
                        .map(|mapping| mapping.actions.iter().any(|action| action.id == *action_id))
                        .unwrap_or(false);
                if !action_exists_in_file {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.input.action_missing",
                        format!("RemoveInputAction target does not exist: {action_id}"),
                        Some(operation_id.clone()),
                        Some(path.to_string()),
                    ));
                }
            }
            InputPatchOperation::RemoveInputBinding {
                operation_id,
                binding_index,
                ..
            } => {
                if let Ok(mapping) =
                    InputMappingAuthoringService::load(&project_session.project_root, path)
                {
                    if *binding_index >= mapping.bindings.len() {
                        diagnostics.push(PatchDiagnostic::error(
                            "project_patch.input.binding_index_out_of_range",
                            format!("Input binding index out of range: {binding_index}"),
                            Some(operation_id.clone()),
                            Some(path.to_string()),
                        ));
                    }
                }
            }
            InputPatchOperation::SetInputBindingDevicePath {
                operation_id,
                binding_index,
                device_path,
                ..
            } => {
                if device_path.trim().is_empty() {
                    diagnostics.push(PatchDiagnostic::error(
                        "project_patch.input.device_path_required",
                        "SetInputBindingDevicePath requires a non-empty device_path.",
                        Some(operation_id.clone()),
                        Some(path.to_string()),
                    ));
                }
                if let Ok(mapping) =
                    InputMappingAuthoringService::load(&project_session.project_root, path)
                {
                    if *binding_index >= mapping.bindings.len() {
                        diagnostics.push(PatchDiagnostic::error(
                            "project_patch.input.binding_index_out_of_range",
                            format!("Input binding index out of range: {binding_index}"),
                            Some(operation_id.clone()),
                            Some(path.to_string()),
                        ));
                    }
                }
            }
            InputPatchOperation::SetInputBindingProcessor {
                operation_id,
                binding_index,
                ..
            } => {
                if let Ok(mapping) =
                    InputMappingAuthoringService::load(&project_session.project_root, path)
                {
                    if *binding_index >= mapping.bindings.len() {
                        diagnostics.push(PatchDiagnostic::error(
                            "project_patch.input.binding_index_out_of_range",
                            format!("Input binding index out of range: {binding_index}"),
                            Some(operation_id.clone()),
                            Some(path.to_string()),
                        ));
                    }
                }
            }
        }
    }

    if load_first_input_mapping(&project_session.project_root).is_none() {
        let creates_mapping = patch.operations.iter().any(|operation| {
            matches!(
                operation,
                PatchOperation::Input(InputPatchOperation::CreateDefaultInputMapping { .. })
            )
        });
        let edits_mapping = patch.operations.iter().any(|operation| {
            matches!(
                operation,
                PatchOperation::Input(
                    InputPatchOperation::AddInputAction { .. }
                        | InputPatchOperation::AddInputBinding { .. }
                        | InputPatchOperation::RemoveInputAction { .. }
                        | InputPatchOperation::RemoveInputBinding { .. }
                        | InputPatchOperation::SetInputBindingDevicePath { .. }
                        | InputPatchOperation::SetInputBindingProcessor { .. }
                )
            )
        });
        if edits_mapping && !creates_mapping {
            diagnostics.push(PatchDiagnostic::error(
                "project_patch.input.mapping_missing",
                "Input edits require an existing mapping or CreateDefaultInputMapping in the same patch.",
                None,
                Some("Input".to_string()),
            ));
        }
    }
}
