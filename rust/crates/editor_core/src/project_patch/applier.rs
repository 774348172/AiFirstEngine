use editor_ui_model::UiCommandPayload;

use super::{
    AssetPatchOperation, AuiPatchOperation, BuildPatchOperation, InputBindingProcessorPatch,
    InputPatchOperation, PatchOperation, PrefabPatchOperation, ProjectPatchDocument,
    RulePatchOperation, ScenePatchOperation,
};

pub struct PatchApplier;

impl PatchApplier {
    pub fn expand(patch: &ProjectPatchDocument) -> Vec<UiCommandPayload> {
        patch
            .operations
            .iter()
            .map(Self::expand_operation)
            .collect()
    }

    pub fn expand_operation(operation: &PatchOperation) -> UiCommandPayload {
        match operation {
            PatchOperation::Scene(operation) => match operation {
                ScenePatchOperation::CreateEntity {
                    parent_id, name, ..
                } => UiCommandPayload::CreateSceneEntity {
                    parent_id: parent_id.clone(),
                    name: name.clone(),
                },
                ScenePatchOperation::DeleteEntity { entity_id, .. } => {
                    UiCommandPayload::DeleteSceneEntity {
                        entity_id: entity_id.clone(),
                    }
                }
                ScenePatchOperation::RenameEntity {
                    entity_id, name, ..
                } => UiCommandPayload::RenameSceneEntity {
                    entity_id: entity_id.clone(),
                    name: name.clone(),
                },
                ScenePatchOperation::SetTransform {
                    entity_id,
                    local_position,
                    local_rotation,
                    local_scale,
                    ..
                } => UiCommandPayload::SetSceneTransform {
                    entity_id: entity_id.clone(),
                    local_position: *local_position,
                    local_rotation: *local_rotation,
                    local_scale: *local_scale,
                },
                ScenePatchOperation::AddComponent {
                    entity_id,
                    component_type,
                    fields,
                    ..
                } => UiCommandPayload::AddSceneComponent {
                    entity_id: entity_id.clone(),
                    component_type: component_type.clone(),
                    fields: fields.clone(),
                },
                ScenePatchOperation::RemoveComponent {
                    entity_id,
                    component_type,
                    ..
                } => UiCommandPayload::RemoveSceneComponent {
                    entity_id: entity_id.clone(),
                    component_type: component_type.clone(),
                },
                ScenePatchOperation::SetComponentField {
                    entity_id,
                    component_type,
                    field_path,
                    value,
                    ..
                } => UiCommandPayload::SetSceneComponentField {
                    entity_id: entity_id.clone(),
                    component_type: component_type.clone(),
                    field_path: field_path.clone(),
                    value: value.clone(),
                },
                ScenePatchOperation::PlaceAssetIntoScene {
                    asset_id,
                    asset_type,
                    asset_guid,
                    target_parent_id,
                    local_position,
                    placement_mode,
                    ..
                } => UiCommandPayload::PlaceAssetIntoScene {
                    asset_id: asset_id.clone(),
                    asset_type: asset_type.clone(),
                    asset_guid: asset_guid.clone(),
                    target_parent_id: target_parent_id.clone(),
                    local_position: *local_position,
                    placement_mode: *placement_mode,
                },
            },
            PatchOperation::Input(operation) => match operation {
                InputPatchOperation::CreateDefaultInputMapping { path, .. } => {
                    UiCommandPayload::CreateDefaultInputMapping { path: path.clone() }
                }
                InputPatchOperation::DeleteInputMapping { path, .. } => {
                    UiCommandPayload::DeleteInputMapping { path: path.clone() }
                }
                InputPatchOperation::AddInputAction {
                    path,
                    action_id,
                    value_type,
                    ..
                } => UiCommandPayload::AddInputAction {
                    path: path.clone(),
                    action_id: action_id.clone(),
                    value_type: *value_type,
                },
                InputPatchOperation::AddInputBinding {
                    path,
                    context_id,
                    action_id,
                    device_path,
                    ..
                } => UiCommandPayload::AddInputBinding {
                    path: path.clone(),
                    context_id: context_id.clone(),
                    action_id: action_id.clone(),
                    device_path: device_path.clone(),
                },
                InputPatchOperation::RemoveInputAction {
                    path, action_id, ..
                } => UiCommandPayload::RemoveInputAction {
                    path: path.clone(),
                    action_id: action_id.clone(),
                },
                InputPatchOperation::RemoveInputBinding {
                    path,
                    binding_index,
                    ..
                } => UiCommandPayload::RemoveInputBinding {
                    path: path.clone(),
                    binding_index: *binding_index,
                },
                InputPatchOperation::SetInputBindingDevicePath {
                    path,
                    binding_index,
                    device_path,
                    ..
                } => UiCommandPayload::SetInputBindingDevicePath {
                    path: path.clone(),
                    binding_index: *binding_index,
                    device_path: device_path.clone(),
                },
                InputPatchOperation::SetInputBindingProcessor {
                    path,
                    binding_index,
                    processor,
                    ..
                } => UiCommandPayload::SetInputBindingProcessorByIndex {
                    path: path.clone(),
                    binding_index: *binding_index,
                    processor: processor_to_ui(processor),
                },
            },
            PatchOperation::Asset(operation) => match operation {
                AssetPatchOperation::RegisterExistingAsset {
                    path,
                    expected_kind,
                    ..
                } => UiCommandPayload::RegisterExistingAsset {
                    path: path.clone(),
                    expected_kind: *expected_kind,
                },
                AssetPatchOperation::GenerateMockImageAsset {
                    prompt,
                    target_folder,
                    asset_name,
                    image_kind,
                    width,
                    height,
                    transparent_background,
                    ..
                } => UiCommandPayload::GenerateMockImageAsset {
                    prompt: prompt.clone(),
                    target_folder: target_folder.clone(),
                    asset_name: asset_name.clone(),
                    image_kind: image_kind.clone(),
                    width: *width,
                    height: *height,
                    transparent_background: *transparent_background,
                },
                AssetPatchOperation::ValidateAssetBrowserIndex { query_kind, .. } => {
                    UiCommandPayload::ValidateAssetBrowserIndex {
                        query_kind: *query_kind,
                    }
                }
            },
            PatchOperation::Prefab(operation) => match operation {
                PrefabPatchOperation::CreateFromSceneSelection {
                    scene_path,
                    root_entity_id,
                    prefab_id,
                    name,
                    replace_selection_with_instance,
                    ..
                } => UiCommandPayload::CreatePrefabFromSelection {
                    scene_path: scene_path.clone(),
                    root_entity_id: root_entity_id.clone(),
                    prefab_id: prefab_id.clone(),
                    name: name.clone(),
                    replace_selection_with_instance: *replace_selection_with_instance,
                },
                PrefabPatchOperation::OpenDocument { path, .. } => {
                    UiCommandPayload::OpenPrefabDocument { path: path.clone() }
                }
                PrefabPatchOperation::SetStageEntityField {
                    source_entity_id,
                    component_type,
                    field_path,
                    value,
                    ..
                } => UiCommandPayload::SetPrefabStageEntityField {
                    source_entity_id: source_entity_id.clone(),
                    component_type: component_type.clone(),
                    field_path: field_path.clone(),
                    value: value.clone(),
                },
                PrefabPatchOperation::SaveDocument { path, .. } => {
                    UiCommandPayload::SavePrefabDocument { path: path.clone() }
                }
                PrefabPatchOperation::InstantiateInScene {
                    prefab_id,
                    parent_entity_id,
                    local_position,
                    ..
                } => UiCommandPayload::InstantiatePrefabInScene {
                    prefab_id: prefab_id.clone(),
                    parent_entity_id: parent_entity_id.clone(),
                    local_position: *local_position,
                },
                PrefabPatchOperation::ApplyOverrideToAsset {
                    instance_entity_id,
                    target_source_entity_id,
                    component_type,
                    field_path,
                    ..
                } => UiCommandPayload::ApplyPrefabOverrideToAsset {
                    instance_entity_id: instance_entity_id.clone(),
                    target_source_entity_id: target_source_entity_id.clone(),
                    component_type: component_type.clone(),
                    field_path: field_path.clone(),
                },
                PrefabPatchOperation::RevertOverride {
                    instance_entity_id,
                    target_source_entity_id,
                    component_type,
                    field_path,
                    ..
                } => UiCommandPayload::RevertPrefabOverride {
                    instance_entity_id: instance_entity_id.clone(),
                    target_source_entity_id: target_source_entity_id.clone(),
                    component_type: component_type.clone(),
                    field_path: field_path.clone(),
                },
                PrefabPatchOperation::ValidateReferences { path, .. } => {
                    UiCommandPayload::ValidatePrefabReferences { path: path.clone() }
                }
            },
            PatchOperation::Aui(operation) => match operation {
                AuiPatchOperation::CreateDocument {
                    path,
                    document_id,
                    width,
                    height,
                    ..
                } => UiCommandPayload::CreateAuiDocument {
                    path: path.clone(),
                    document_id: document_id.clone(),
                    width: *width,
                    height: *height,
                },
                AuiPatchOperation::OpenDocument { path, .. } => {
                    UiCommandPayload::OpenAuiDocument { path: path.clone() }
                }
                AuiPatchOperation::AddNode {
                    path,
                    parent_node_id,
                    node_id,
                    node_kind,
                    name,
                    rect,
                    ..
                } => UiCommandPayload::AddAuiNode {
                    path: path.clone(),
                    parent_node_id: parent_node_id.clone(),
                    node_id: node_id.clone(),
                    kind: node_kind.clone(),
                    name: name.clone(),
                    rect: rect.clone(),
                },
                AuiPatchOperation::SetNodeField {
                    path,
                    node_id,
                    schema_path,
                    value,
                    ..
                } => UiCommandPayload::SetAuiNodeField {
                    path: path.clone(),
                    node_id: node_id.clone(),
                    schema_path: schema_path.clone(),
                    value: value.clone(),
                },
                AuiPatchOperation::SetBindingPath {
                    path,
                    node_id,
                    target_field,
                    binding_id,
                    binding_path,
                    fallback,
                    ..
                } => UiCommandPayload::SetAuiBindingPath {
                    path: path.clone(),
                    node_id: node_id.clone(),
                    target_field: target_field.clone(),
                    binding_id: binding_id.clone(),
                    binding_path: binding_path.clone(),
                    fallback: fallback.clone(),
                },
                AuiPatchOperation::SetActionRef {
                    path,
                    node_id,
                    event,
                    action_id,
                    payload,
                    ..
                } => UiCommandPayload::SetAuiActionRef {
                    path: path.clone(),
                    node_id: node_id.clone(),
                    event: event.clone(),
                    action_id: action_id.clone(),
                    payload: payload.clone(),
                },
                AuiPatchOperation::ValidateDocument { path, .. } => {
                    UiCommandPayload::ValidateAuiDocument { path: path.clone() }
                }
                AuiPatchOperation::SaveDocument { path, .. } => {
                    UiCommandPayload::SaveAuiDocument { path: path.clone() }
                }
                AuiPatchOperation::PreviewOverlay { path, .. } => {
                    UiCommandPayload::PreviewAuiOverlay { path: path.clone() }
                }
            },
            PatchOperation::Rule(operation) => match operation {
                RulePatchOperation::CreateAsset {
                    path,
                    rule_id,
                    display_name,
                    phase,
                    ..
                } => UiCommandPayload::CreateRuleAsset {
                    path: path.clone(),
                    rule_id: rule_id.clone(),
                    display_name: display_name.clone(),
                    phase: phase.clone(),
                },
                RulePatchOperation::OpenAsset { path, .. } => {
                    UiCommandPayload::OpenRuleAsset { path: path.clone() }
                }
                RulePatchOperation::SetTrigger {
                    path,
                    trigger,
                    expected_ir_hash,
                    ..
                } => UiCommandPayload::SetRuleTrigger {
                    path: path.clone(),
                    trigger: trigger.clone(),
                    expected_ir_hash: expected_ir_hash.clone(),
                },
                RulePatchOperation::AddStatement {
                    path,
                    statement,
                    expected_ir_hash,
                    ..
                } => UiCommandPayload::AddRuleStatement {
                    path: path.clone(),
                    statement: statement.clone(),
                    expected_ir_hash: expected_ir_hash.clone(),
                },
                RulePatchOperation::UpdateStatement {
                    path,
                    statement_index,
                    statement,
                    expected_ir_hash,
                    ..
                } => UiCommandPayload::UpdateRuleStatement {
                    path: path.clone(),
                    statement_index: *statement_index,
                    statement: statement.clone(),
                    expected_ir_hash: expected_ir_hash.clone(),
                },
                RulePatchOperation::RemoveStatement {
                    path,
                    statement_index,
                    expected_ir_hash,
                    ..
                } => UiCommandPayload::RemoveRuleStatement {
                    path: path.clone(),
                    statement_index: *statement_index,
                    expected_ir_hash: expected_ir_hash.clone(),
                },
                RulePatchOperation::AddOperation {
                    path,
                    operation,
                    expected_ir_hash,
                    ..
                } => UiCommandPayload::AddRuleOperation {
                    path: path.clone(),
                    operation: operation.clone(),
                    expected_ir_hash: expected_ir_hash.clone(),
                },
                RulePatchOperation::UpdateOperation {
                    path,
                    operation_index,
                    operation,
                    expected_ir_hash,
                    ..
                } => UiCommandPayload::UpdateRuleOperation {
                    path: path.clone(),
                    operation_index: *operation_index,
                    operation: operation.clone(),
                    expected_ir_hash: expected_ir_hash.clone(),
                },
                RulePatchOperation::RemoveOperation {
                    path,
                    operation_index,
                    expected_ir_hash,
                    ..
                } => UiCommandPayload::RemoveRuleOperation {
                    path: path.clone(),
                    operation_index: *operation_index,
                    expected_ir_hash: expected_ir_hash.clone(),
                },
                RulePatchOperation::ValidateAsset { path, .. } => {
                    UiCommandPayload::ValidateRuleAsset { path: path.clone() }
                }
                RulePatchOperation::BuildArtifact { path, .. } => {
                    UiCommandPayload::BuildRuleArtifact { path: path.clone() }
                }
                RulePatchOperation::BuildProjectManifest { path, .. } => {
                    UiCommandPayload::BuildProjectRuleManifest { path: path.clone() }
                }
            },
            PatchOperation::Build(operation) => match operation {
                BuildPatchOperation::ExportDesktopPackage { profile_id, .. } => {
                    UiCommandPayload::ExportDesktopPackage {
                        profile_id: profile_id.clone(),
                    }
                }
                BuildPatchOperation::OpenBuildReport { .. } => UiCommandPayload::OpenBuildReport,
                BuildPatchOperation::OpenBuildOutput { .. } => UiCommandPayload::OpenBuildOutput,
            },
        }
    }
}

fn processor_to_ui(processor: &InputBindingProcessorPatch) -> editor_ui_model::InputProcessorKind {
    match processor {
        InputBindingProcessorPatch::None => editor_ui_model::InputProcessorKind::None,
        InputBindingProcessorPatch::Deadzone { threshold } => {
            editor_ui_model::InputProcessorKind::Deadzone {
                threshold: *threshold,
            }
        }
        InputBindingProcessorPatch::Normalize => editor_ui_model::InputProcessorKind::Normalize,
        InputBindingProcessorPatch::Scale { factor } => {
            editor_ui_model::InputProcessorKind::Scale { factor: *factor }
        }
        InputBindingProcessorPatch::Invert => editor_ui_model::InputProcessorKind::Invert,
    }
}
