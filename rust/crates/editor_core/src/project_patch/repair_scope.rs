use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{decode_rule_operation, decode_rule_statement, decode_rule_trigger};

use super::{
    AssetPatchOperation, AuiPatchOperation, BuildPatchOperation, InputPatchOperation,
    PatchCapability, PatchOperation, PatchRiskLevel, PrefabPatchOperation, ProjectPatchDocument,
    ProjectPatchImportResult, RulePatchOperation, ScenePatchOperation,
};

pub const REPAIR_SCOPE_UNPROVABLE_MAX_OPERATIONS: usize = 8;

pub const REPAIR_SCOPE_OPERATION_COUNT_EXPANDED: &str = "repair_scope_operation_count_expanded";
pub const REPAIR_SCOPE_OPERATION_KIND_CHANGED: &str = "repair_scope_operation_kind_changed";
pub const REPAIR_SCOPE_TARGET_CHANGED: &str = "repair_scope_target_changed";
pub const REPAIR_SCOPE_UNAUTHORIZED_FIELD_CHANGED: &str = "repair_scope_unauthorized_field_changed";
pub const REPAIR_SCOPE_DEPENDENCY_EXPANDED: &str = "repair_scope_dependency_expanded";
pub const REPAIR_SCOPE_DESTRUCTIVE_OR_BUILD_EXPANDED: &str =
    "repair_scope_destructive_or_build_expanded";
pub const REPAIR_SCOPE_RISK_EXPANDED: &str = "repair_scope_risk_expanded";
pub const REPAIR_SCOPE_CAPABILITY_EXPANDED: &str = "repair_scope_capability_expanded";
pub const REPAIR_SCOPE_METADATA_CHANGED: &str = "repair_scope_metadata_changed";
pub const REPAIR_SCOPE_UNPROVABLE_LIMIT_EXCEEDED: &str = "repair_scope_unprovable_limit_exceeded";
pub const REPAIR_SCOPE_AUTHORIZATION_AMBIGUOUS: &str = "repair_scope_authorization_ambiguous";

const DIAGNOSTIC_OPERATION_ID_REQUIRED: &str = "project_patch.operation_id_required";
const DIAGNOSTIC_OPERATION_ID_DUPLICATE: &str = "project_patch.operation_id_duplicate";
const DIAGNOSTIC_DEPENDENCY_MISSING: &str = "project_patch.dependency_missing";
const DIAGNOSTIC_SCENE_FIELD_INVALID: &str = "project_patch.scene.component_field_invalid";
const DIAGNOSTIC_PREFAB_FIELD_INVALID: &str = "project_patch.prefab.stage_field_invalid";
const DIAGNOSTIC_AUI_FIELD_INVALID: &str = "project_patch.aui.node_field_invalid";
const DIAGNOSTIC_RULE_PAYLOAD_INVALID: &str = "project_patch.rule.payload_invalid";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairScopeValidationStatus {
    Passed,
    Rejected,
    ScopeUnprovableRestricted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepairScopePolicy {
    pub maximum_operation_count: usize,
}

impl RepairScopePolicy {
    pub const fn new(maximum_operation_count: usize) -> Self {
        Self {
            maximum_operation_count,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairScopeValidation {
    pub status: RepairScopeValidationStatus,
    pub initial_operation_count: Option<usize>,
    pub repaired_operation_count: usize,
    pub changed_slots: Vec<usize>,
    pub diagnostic_codes: Vec<String>,
    pub rejection_code: Option<String>,
}

impl RepairScopeValidation {
    pub fn accepted(&self) -> bool {
        self.status != RepairScopeValidationStatus::Rejected
    }

    fn rejected(
        initial_operation_count: Option<usize>,
        repaired_operation_count: usize,
        changed_slots: Vec<usize>,
        diagnostic_codes: Vec<String>,
        rejection_code: &'static str,
    ) -> Self {
        Self {
            status: RepairScopeValidationStatus::Rejected,
            initial_operation_count,
            repaired_operation_count,
            changed_slots,
            diagnostic_codes,
            rejection_code: Some(rejection_code.to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SemanticRepairKind {
    None,
    SceneComponentField,
    PrefabStageField,
    AuiNodeField,
    RuleTrigger,
    RuleStatement,
    RuleOperation,
}

#[derive(Debug, Clone, PartialEq)]
struct PatchOperationScopeClaim {
    kind: &'static str,
    immutable_target_anchor: Value,
    destructive_or_build: bool,
    semantic_repair: SemanticRepairKind,
}

#[derive(Debug, Clone, Copy, Default)]
struct SlotAuthorization {
    operation_id: bool,
    dependencies: bool,
    semantic: bool,
}

pub fn validate_repair_scope(
    initial_import: &ProjectPatchImportResult,
    repaired: &ProjectPatchDocument,
    policy: RepairScopePolicy,
) -> RepairScopeValidation {
    let diagnostic_codes = diagnostic_codes(initial_import);
    match initial_import.parsed_patch.as_ref() {
        Some(initial) => validate_parseable(initial, repaired, policy, diagnostic_codes),
        None => validate_unprovable(repaired, policy, diagnostic_codes),
    }
}

fn validate_parseable(
    initial: &ProjectPatchDocument,
    repaired: &ProjectPatchDocument,
    policy: RepairScopePolicy,
    diagnostic_codes: Vec<String>,
) -> RepairScopeValidation {
    let initial_count = initial.operations.len();
    let repaired_count = repaired.operations.len();
    let changed_slots = changed_slots(initial, repaired);
    let reject = |code| {
        RepairScopeValidation::rejected(
            Some(initial_count),
            repaired_count,
            changed_slots.clone(),
            diagnostic_codes.clone(),
            code,
        )
    };

    if repaired_count > policy.maximum_operation_count || repaired_count != initial_count {
        return reject(REPAIR_SCOPE_OPERATION_COUNT_EXPANDED);
    }
    if metadata_changed(initial, repaired)
        || initial.target_project_root != repaired.target_project_root
    {
        return reject(REPAIR_SCOPE_METADATA_CHANGED);
    }
    if risk_rank(repaired.risk_level) > risk_rank(initial.risk_level) {
        return reject(REPAIR_SCOPE_RISK_EXPANDED);
    }
    if repaired
        .required_capabilities
        .iter()
        .any(|capability| !initial.required_capabilities.contains(capability))
        || repaired.required_capabilities.len() > initial.required_capabilities.len()
    {
        return reject(REPAIR_SCOPE_CAPABILITY_EXPANDED);
    }
    if !capabilities_cover_operations(&repaired.required_capabilities, &repaired.operations) {
        return reject(REPAIR_SCOPE_UNAUTHORIZED_FIELD_CHANGED);
    }

    let authorizations = slot_authorizations(initial, &diagnostic_codes);
    if authorization_is_ambiguous(initial, &diagnostic_codes, &authorizations) {
        return reject(REPAIR_SCOPE_AUTHORIZATION_AMBIGUOUS);
    }
    for (slot, (initial_operation, repaired_operation)) in initial
        .operations
        .iter()
        .zip(&repaired.operations)
        .enumerate()
    {
        let initial_claim = scope_claim(initial_operation);
        let repaired_claim = scope_claim(repaired_operation);
        if initial_claim.kind != repaired_claim.kind {
            return reject(REPAIR_SCOPE_OPERATION_KIND_CHANGED);
        }
        if initial_claim.immutable_target_anchor != repaired_claim.immutable_target_anchor {
            return reject(REPAIR_SCOPE_TARGET_CHANGED);
        }
        let authorization = authorizations[slot];
        if authorization.dependencies
            && (repaired_operation.depends_on().len() > initial_operation.depends_on().len()
                || !dependencies_resolve(repaired_operation, repaired))
        {
            return reject(REPAIR_SCOPE_DEPENDENCY_EXPANDED);
        }
        if !operations_equal_after_authorized_changes(
            initial_operation,
            repaired_operation,
            initial_claim.semantic_repair,
            authorization,
        ) {
            return reject(if initial_claim.destructive_or_build {
                REPAIR_SCOPE_DESTRUCTIVE_OR_BUILD_EXPANDED
            } else {
                REPAIR_SCOPE_UNAUTHORIZED_FIELD_CHANGED
            });
        }
    }

    RepairScopeValidation {
        status: RepairScopeValidationStatus::Passed,
        initial_operation_count: Some(initial_count),
        repaired_operation_count: repaired_count,
        changed_slots,
        diagnostic_codes,
        rejection_code: None,
    }
}

fn authorization_is_ambiguous(
    initial: &ProjectPatchDocument,
    diagnostic_codes: &[String],
    authorizations: &[SlotAuthorization],
) -> bool {
    let has_code = |code: &str| diagnostic_codes.iter().any(|item| item == code);
    let any_id = authorizations
        .iter()
        .any(|authorization| authorization.operation_id);
    let any_dependency = authorizations
        .iter()
        .any(|authorization| authorization.dependencies);
    let has_semantic_for = |matches_operation: fn(&PatchOperation) -> bool| {
        initial
            .operations
            .iter()
            .zip(authorizations)
            .any(|(operation, authorization)| {
                authorization.semantic && matches_operation(operation)
            })
    };
    ((has_code(DIAGNOSTIC_OPERATION_ID_REQUIRED) || has_code(DIAGNOSTIC_OPERATION_ID_DUPLICATE))
        && !any_id)
        || (has_code(DIAGNOSTIC_DEPENDENCY_MISSING) && !any_dependency)
        || (has_code(DIAGNOSTIC_SCENE_FIELD_INVALID)
            && !has_semantic_for(|operation| matches!(operation, PatchOperation::Scene(_))))
        || (has_code(DIAGNOSTIC_PREFAB_FIELD_INVALID)
            && !has_semantic_for(|operation| matches!(operation, PatchOperation::Prefab(_))))
        || (has_code(DIAGNOSTIC_AUI_FIELD_INVALID)
            && !has_semantic_for(|operation| matches!(operation, PatchOperation::Aui(_))))
        || (has_code(DIAGNOSTIC_RULE_PAYLOAD_INVALID)
            && !has_semantic_for(|operation| matches!(operation, PatchOperation::Rule(_))))
}

fn validate_unprovable(
    repaired: &ProjectPatchDocument,
    policy: RepairScopePolicy,
    diagnostic_codes: Vec<String>,
) -> RepairScopeValidation {
    let repaired_count = repaired.operations.len();
    let reject = |code| {
        RepairScopeValidation::rejected(
            None,
            repaired_count,
            (0..repaired_count).collect(),
            diagnostic_codes.clone(),
            code,
        )
    };
    let maximum = policy
        .maximum_operation_count
        .min(REPAIR_SCOPE_UNPROVABLE_MAX_OPERATIONS);
    if repaired_count > maximum {
        return reject(REPAIR_SCOPE_UNPROVABLE_LIMIT_EXCEEDED);
    }
    if repaired.risk_level != PatchRiskLevel::Low {
        return reject(REPAIR_SCOPE_RISK_EXPANDED);
    }
    if repaired
        .operations
        .iter()
        .any(|operation| scope_claim(operation).destructive_or_build)
    {
        return reject(REPAIR_SCOPE_DESTRUCTIVE_OR_BUILD_EXPANDED);
    }
    if !capabilities_exactly_match_operations(&repaired.required_capabilities, &repaired.operations)
    {
        return reject(REPAIR_SCOPE_UNAUTHORIZED_FIELD_CHANGED);
    }
    RepairScopeValidation {
        status: RepairScopeValidationStatus::ScopeUnprovableRestricted,
        initial_operation_count: None,
        repaired_operation_count: repaired_count,
        changed_slots: (0..repaired_count).collect(),
        diagnostic_codes,
        rejection_code: None,
    }
}

fn diagnostic_codes(initial_import: &ProjectPatchImportResult) -> Vec<String> {
    let mut codes = super::import_diagnostics(initial_import)
        .into_iter()
        .map(|diagnostic| diagnostic.code)
        .collect::<Vec<_>>();
    codes.sort();
    codes.dedup();
    codes
}

fn metadata_changed(initial: &ProjectPatchDocument, repaired: &ProjectPatchDocument) -> bool {
    initial.schema_version != repaired.schema_version
        || initial.patch_id != repaired.patch_id
        || initial.title != repaired.title
        || initial.source != repaired.source
        || initial.intent_summary != repaired.intent_summary
        || initial.expected_outcome != repaired.expected_outcome
        || initial.created_at != repaired.created_at
}

fn changed_slots(initial: &ProjectPatchDocument, repaired: &ProjectPatchDocument) -> Vec<usize> {
    let common = initial.operations.len().min(repaired.operations.len());
    let mut slots = (0..common)
        .filter(|slot| initial.operations[*slot] != repaired.operations[*slot])
        .collect::<Vec<_>>();
    slots.extend(common..initial.operations.len().max(repaired.operations.len()));
    slots
}

fn slot_authorizations(
    initial: &ProjectPatchDocument,
    diagnostic_codes: &[String],
) -> Vec<SlotAuthorization> {
    let has_code = |code: &str| diagnostic_codes.iter().any(|item| item == code);
    let initial_ids = initial
        .operations
        .iter()
        .map(|operation| operation.operation_id().to_string())
        .collect::<BTreeSet<_>>();
    let mut seen_ids = BTreeSet::new();
    initial
        .operations
        .iter()
        .map(|operation| {
            let operation_id = operation.operation_id();
            let duplicate = !operation_id.trim().is_empty() && !seen_ids.insert(operation_id);
            let semantic = semantic_repair_is_authorized(operation, has_code);
            SlotAuthorization {
                operation_id: (has_code(DIAGNOSTIC_OPERATION_ID_REQUIRED)
                    && operation_id.trim().is_empty())
                    || (has_code(DIAGNOSTIC_OPERATION_ID_DUPLICATE) && duplicate),
                dependencies: has_code(DIAGNOSTIC_DEPENDENCY_MISSING)
                    && operation
                        .depends_on()
                        .iter()
                        .any(|dependency| !initial_ids.contains(dependency)),
                semantic,
            }
        })
        .collect()
}

fn semantic_repair_is_authorized(
    operation: &PatchOperation,
    has_code: impl Fn(&str) -> bool,
) -> bool {
    match operation {
        PatchOperation::Scene(ScenePatchOperation::SetComponentField { field_path, .. }) => {
            has_code(DIAGNOSTIC_SCENE_FIELD_INVALID) && field_path.trim().is_empty()
        }
        PatchOperation::Prefab(PrefabPatchOperation::SetStageEntityField {
            source_entity_id,
            field_path,
            value,
            ..
        }) => {
            has_code(DIAGNOSTIC_PREFAB_FIELD_INVALID)
                && !source_entity_id.trim().is_empty()
                && (field_path.trim().is_empty() || value.is_null())
        }
        PatchOperation::Aui(AuiPatchOperation::SetNodeField {
            node_id,
            schema_path,
            value,
            ..
        }) => {
            has_code(DIAGNOSTIC_AUI_FIELD_INVALID)
                && !node_id.trim().is_empty()
                && (schema_path.trim().is_empty() || value.is_null())
        }
        PatchOperation::Rule(operation) => {
            has_code(DIAGNOSTIC_RULE_PAYLOAD_INVALID) && rule_payload_is_invalid(operation)
        }
        PatchOperation::Scene(_)
        | PatchOperation::Input(_)
        | PatchOperation::Asset(_)
        | PatchOperation::Prefab(_)
        | PatchOperation::Aui(_)
        | PatchOperation::Build(_) => false,
    }
}

fn rule_payload_is_invalid(operation: &RulePatchOperation) -> bool {
    match operation {
        RulePatchOperation::SetTrigger { trigger, .. } => {
            decode_rule_trigger(trigger.clone()).is_err()
        }
        RulePatchOperation::AddStatement { statement, .. }
        | RulePatchOperation::UpdateStatement { statement, .. } => {
            decode_rule_statement(statement.clone()).is_err()
        }
        RulePatchOperation::AddOperation { operation, .. }
        | RulePatchOperation::UpdateOperation { operation, .. } => {
            decode_rule_operation(operation.clone()).is_err()
        }
        RulePatchOperation::CreateAsset { .. }
        | RulePatchOperation::OpenAsset { .. }
        | RulePatchOperation::RemoveStatement { .. }
        | RulePatchOperation::RemoveOperation { .. }
        | RulePatchOperation::ValidateAsset { .. }
        | RulePatchOperation::BuildArtifact { .. }
        | RulePatchOperation::BuildProjectManifest { .. } => false,
    }
}

fn operations_equal_after_authorized_changes(
    initial: &PatchOperation,
    repaired: &PatchOperation,
    semantic_repair: SemanticRepairKind,
    authorization: SlotAuthorization,
) -> bool {
    let mut initial = serde_json::to_value(initial).expect("PatchOperation must serialize");
    let mut repaired = serde_json::to_value(repaired).expect("PatchOperation must serialize");
    let Some(initial_fields) = operation_fields_mut(&mut initial) else {
        return false;
    };
    let Some(repaired_fields) = operation_fields_mut(&mut repaired) else {
        return false;
    };
    if authorization.operation_id {
        initial_fields.remove("operationId");
        repaired_fields.remove("operationId");
    }
    if authorization.dependencies {
        initial_fields.remove("dependsOn");
        repaired_fields.remove("dependsOn");
    }
    if authorization.semantic {
        for field in semantic_mutable_fields(semantic_repair) {
            initial_fields.remove(*field);
            repaired_fields.remove(*field);
        }
    }
    initial == repaired
}

fn operation_fields_mut(value: &mut Value) -> Option<&mut serde_json::Map<String, Value>> {
    value.get_mut("operation")?.as_object_mut()
}

fn semantic_mutable_fields(kind: SemanticRepairKind) -> &'static [&'static str] {
    match kind {
        SemanticRepairKind::None => &[],
        SemanticRepairKind::SceneComponentField | SemanticRepairKind::PrefabStageField => {
            &["fieldPath", "value"]
        }
        SemanticRepairKind::AuiNodeField => &["schemaPath", "value"],
        SemanticRepairKind::RuleTrigger => &["trigger"],
        SemanticRepairKind::RuleStatement => &["statement"],
        SemanticRepairKind::RuleOperation => &["operation"],
    }
}

fn dependencies_resolve(operation: &PatchOperation, repaired: &ProjectPatchDocument) -> bool {
    let ids = repaired
        .operations
        .iter()
        .map(PatchOperation::operation_id)
        .collect::<BTreeSet<_>>();
    operation.depends_on().iter().all(|dependency| {
        ids.contains(dependency.as_str()) && dependency != operation.operation_id()
    })
}

fn capabilities_cover_operations(
    capabilities: &[PatchCapability],
    operations: &[PatchOperation],
) -> bool {
    operation_capabilities(operations)
        .iter()
        .all(|capability| capabilities.contains(capability))
}

fn capabilities_exactly_match_operations(
    capabilities: &[PatchCapability],
    operations: &[PatchOperation],
) -> bool {
    let derived = operation_capabilities(operations);
    capabilities.len() == derived.len()
        && capabilities
            .iter()
            .all(|capability| derived.contains(capability))
}

fn operation_capabilities(operations: &[PatchOperation]) -> Vec<PatchCapability> {
    let ordered = [
        PatchCapability::Scene,
        PatchCapability::Input,
        PatchCapability::Asset,
        PatchCapability::Prefab,
        PatchCapability::Aui,
        PatchCapability::Rule,
        PatchCapability::Build,
    ];
    ordered
        .into_iter()
        .filter(|capability| {
            operations.iter().any(|operation| {
                matches!(
                    (capability, operation),
                    (PatchCapability::Scene, PatchOperation::Scene(_))
                        | (PatchCapability::Input, PatchOperation::Input(_))
                        | (PatchCapability::Asset, PatchOperation::Asset(_))
                        | (PatchCapability::Prefab, PatchOperation::Prefab(_))
                        | (PatchCapability::Aui, PatchOperation::Aui(_))
                        | (PatchCapability::Rule, PatchOperation::Rule(_))
                        | (PatchCapability::Build, PatchOperation::Build(_))
                )
            })
        })
        .collect()
}

fn risk_rank(risk: PatchRiskLevel) -> u8 {
    match risk {
        PatchRiskLevel::Low => 0,
        PatchRiskLevel::Medium => 1,
        PatchRiskLevel::High => 2,
    }
}

fn scope_claim(operation: &PatchOperation) -> PatchOperationScopeClaim {
    match operation {
        PatchOperation::Scene(operation) => scene_scope_claim(operation),
        PatchOperation::Input(operation) => input_scope_claim(operation),
        PatchOperation::Asset(operation) => asset_scope_claim(operation),
        PatchOperation::Prefab(operation) => prefab_scope_claim(operation),
        PatchOperation::Aui(operation) => aui_scope_claim(operation),
        PatchOperation::Rule(operation) => rule_scope_claim(operation),
        PatchOperation::Build(operation) => build_scope_claim(operation),
    }
}

fn claim(
    kind: &'static str,
    immutable_target_anchor: Value,
    destructive_or_build: bool,
    semantic_repair: SemanticRepairKind,
) -> PatchOperationScopeClaim {
    PatchOperationScopeClaim {
        kind,
        immutable_target_anchor,
        destructive_or_build,
        semantic_repair,
    }
}

fn scene_scope_claim(operation: &ScenePatchOperation) -> PatchOperationScopeClaim {
    match operation {
        ScenePatchOperation::CreateEntity {
            parent_id, name, ..
        } => claim(
            "Scene.CreateEntity",
            json!([parent_id, name]),
            false,
            SemanticRepairKind::None,
        ),
        ScenePatchOperation::DeleteEntity { entity_id, .. } => claim(
            "Scene.DeleteEntity",
            json!(entity_id),
            true,
            SemanticRepairKind::None,
        ),
        ScenePatchOperation::RenameEntity { entity_id, .. } => claim(
            "Scene.RenameEntity",
            json!(entity_id),
            false,
            SemanticRepairKind::None,
        ),
        ScenePatchOperation::SetTransform { entity_id, .. } => claim(
            "Scene.SetTransform",
            json!(entity_id),
            false,
            SemanticRepairKind::None,
        ),
        ScenePatchOperation::AddComponent {
            entity_id,
            component_type,
            ..
        } => claim(
            "Scene.AddComponent",
            json!([entity_id, component_type]),
            false,
            SemanticRepairKind::None,
        ),
        ScenePatchOperation::RemoveComponent {
            entity_id,
            component_type,
            ..
        } => claim(
            "Scene.RemoveComponent",
            json!([entity_id, component_type]),
            true,
            SemanticRepairKind::None,
        ),
        ScenePatchOperation::SetComponentField {
            entity_id,
            component_type,
            ..
        } => claim(
            "Scene.SetComponentField",
            json!([entity_id, component_type]),
            false,
            SemanticRepairKind::SceneComponentField,
        ),
        ScenePatchOperation::PlaceAssetIntoScene {
            asset_id,
            asset_type,
            asset_guid,
            target_parent_id,
            ..
        } => claim(
            "Scene.PlaceAssetIntoScene",
            json!([asset_id, asset_type, asset_guid, target_parent_id]),
            false,
            SemanticRepairKind::None,
        ),
    }
}

fn input_scope_claim(operation: &InputPatchOperation) -> PatchOperationScopeClaim {
    match operation {
        InputPatchOperation::CreateDefaultInputMapping { path, .. } => claim(
            "Input.CreateDefaultInputMapping",
            json!(path),
            false,
            SemanticRepairKind::None,
        ),
        InputPatchOperation::DeleteInputMapping { path, .. } => claim(
            "Input.DeleteInputMapping",
            json!(path),
            false,
            SemanticRepairKind::None,
        ),
        InputPatchOperation::AddInputAction {
            path, action_id, ..
        } => claim(
            "Input.AddInputAction",
            json!([path, action_id]),
            false,
            SemanticRepairKind::None,
        ),
        InputPatchOperation::AddInputBinding {
            path,
            context_id,
            action_id,
            ..
        } => claim(
            "Input.AddInputBinding",
            json!([path, context_id, action_id]),
            false,
            SemanticRepairKind::None,
        ),
        InputPatchOperation::RemoveInputAction {
            path, action_id, ..
        } => claim(
            "Input.RemoveInputAction",
            json!([path, action_id]),
            true,
            SemanticRepairKind::None,
        ),
        InputPatchOperation::RemoveInputBinding {
            path,
            binding_index,
            ..
        } => claim(
            "Input.RemoveInputBinding",
            json!([path, binding_index]),
            true,
            SemanticRepairKind::None,
        ),
        InputPatchOperation::SetInputBindingDevicePath {
            path,
            binding_index,
            ..
        } => claim(
            "Input.SetInputBindingDevicePath",
            json!([path, binding_index]),
            false,
            SemanticRepairKind::None,
        ),
        InputPatchOperation::SetInputBindingProcessor {
            path,
            binding_index,
            ..
        } => claim(
            "Input.SetInputBindingProcessor",
            json!([path, binding_index]),
            false,
            SemanticRepairKind::None,
        ),
    }
}

fn asset_scope_claim(operation: &AssetPatchOperation) -> PatchOperationScopeClaim {
    match operation {
        AssetPatchOperation::RegisterExistingAsset { path, .. } => claim(
            "Asset.RegisterExistingAsset",
            json!(path),
            false,
            SemanticRepairKind::None,
        ),
        AssetPatchOperation::GenerateMockImageAsset {
            target_folder,
            asset_name,
            ..
        } => claim(
            "Asset.GenerateMockImageAsset",
            json!([target_folder, asset_name]),
            false,
            SemanticRepairKind::None,
        ),
        AssetPatchOperation::ValidateAssetBrowserIndex { query_kind, .. } => claim(
            "Asset.ValidateAssetBrowserIndex",
            json!(query_kind),
            false,
            SemanticRepairKind::None,
        ),
    }
}

fn prefab_scope_claim(operation: &PrefabPatchOperation) -> PatchOperationScopeClaim {
    match operation {
        PrefabPatchOperation::CreateFromSceneSelection {
            scene_path,
            root_entity_id,
            prefab_id,
            ..
        } => claim(
            "Prefab.CreateFromSceneSelection",
            json!([scene_path, root_entity_id, prefab_id]),
            false,
            SemanticRepairKind::None,
        ),
        PrefabPatchOperation::OpenDocument { path, .. } => claim(
            "Prefab.OpenDocument",
            json!(path),
            false,
            SemanticRepairKind::None,
        ),
        PrefabPatchOperation::SetStageEntityField {
            source_entity_id,
            component_type,
            ..
        } => claim(
            "Prefab.SetStageEntityField",
            json!([source_entity_id, component_type]),
            false,
            SemanticRepairKind::PrefabStageField,
        ),
        PrefabPatchOperation::SaveDocument { path, .. } => claim(
            "Prefab.SaveDocument",
            json!(path),
            false,
            SemanticRepairKind::None,
        ),
        PrefabPatchOperation::InstantiateInScene {
            prefab_id,
            parent_entity_id,
            ..
        } => claim(
            "Prefab.InstantiateInScene",
            json!([prefab_id, parent_entity_id]),
            false,
            SemanticRepairKind::None,
        ),
        PrefabPatchOperation::ApplyOverrideToAsset {
            instance_entity_id,
            target_source_entity_id,
            component_type,
            field_path,
            ..
        } => claim(
            "Prefab.ApplyOverrideToAsset",
            json!([
                instance_entity_id,
                target_source_entity_id,
                component_type,
                field_path
            ]),
            false,
            SemanticRepairKind::None,
        ),
        PrefabPatchOperation::RevertOverride {
            instance_entity_id,
            target_source_entity_id,
            component_type,
            field_path,
            ..
        } => claim(
            "Prefab.RevertOverride",
            json!([
                instance_entity_id,
                target_source_entity_id,
                component_type,
                field_path
            ]),
            false,
            SemanticRepairKind::None,
        ),
        PrefabPatchOperation::ValidateReferences { path, .. } => claim(
            "Prefab.ValidateReferences",
            json!(path),
            false,
            SemanticRepairKind::None,
        ),
    }
}

fn aui_scope_claim(operation: &AuiPatchOperation) -> PatchOperationScopeClaim {
    match operation {
        AuiPatchOperation::CreateDocument {
            path, document_id, ..
        } => claim(
            "Aui.CreateDocument",
            json!([path, document_id]),
            false,
            SemanticRepairKind::None,
        ),
        AuiPatchOperation::OpenDocument { path, .. } => claim(
            "Aui.OpenDocument",
            json!(path),
            false,
            SemanticRepairKind::None,
        ),
        AuiPatchOperation::AddNode {
            path,
            parent_node_id,
            node_id,
            ..
        } => claim(
            "Aui.AddNode",
            json!([path, parent_node_id, node_id]),
            false,
            SemanticRepairKind::None,
        ),
        AuiPatchOperation::SetNodeField { path, node_id, .. } => claim(
            "Aui.SetNodeField",
            json!([path, node_id]),
            false,
            SemanticRepairKind::AuiNodeField,
        ),
        AuiPatchOperation::SetBindingPath {
            path,
            node_id,
            target_field,
            binding_id,
            ..
        } => claim(
            "Aui.SetBindingPath",
            json!([path, node_id, target_field, binding_id]),
            false,
            SemanticRepairKind::None,
        ),
        AuiPatchOperation::SetActionRef {
            path,
            node_id,
            event,
            ..
        } => claim(
            "Aui.SetActionRef",
            json!([path, node_id, event]),
            false,
            SemanticRepairKind::None,
        ),
        AuiPatchOperation::ValidateDocument { path, .. } => claim(
            "Aui.ValidateDocument",
            json!(path),
            false,
            SemanticRepairKind::None,
        ),
        AuiPatchOperation::SaveDocument { path, .. } => claim(
            "Aui.SaveDocument",
            json!(path),
            false,
            SemanticRepairKind::None,
        ),
        AuiPatchOperation::PreviewOverlay { path, .. } => claim(
            "Aui.PreviewOverlay",
            json!(path),
            false,
            SemanticRepairKind::None,
        ),
    }
}

fn rule_scope_claim(operation: &RulePatchOperation) -> PatchOperationScopeClaim {
    match operation {
        RulePatchOperation::CreateAsset { path, rule_id, .. } => claim(
            "Rule.CreateAsset",
            json!([path, rule_id]),
            false,
            SemanticRepairKind::None,
        ),
        RulePatchOperation::OpenAsset { path, .. } => claim(
            "Rule.OpenAsset",
            json!(path),
            false,
            SemanticRepairKind::None,
        ),
        RulePatchOperation::SetTrigger { path, .. } => claim(
            "Rule.SetTrigger",
            json!(path),
            false,
            SemanticRepairKind::RuleTrigger,
        ),
        RulePatchOperation::AddStatement { path, .. } => claim(
            "Rule.AddStatement",
            json!([path, "append"]),
            false,
            SemanticRepairKind::RuleStatement,
        ),
        RulePatchOperation::UpdateStatement {
            path,
            statement_index,
            ..
        } => claim(
            "Rule.UpdateStatement",
            json!([path, statement_index]),
            false,
            SemanticRepairKind::RuleStatement,
        ),
        RulePatchOperation::RemoveStatement {
            path,
            statement_index,
            ..
        } => claim(
            "Rule.RemoveStatement",
            json!([path, statement_index]),
            true,
            SemanticRepairKind::None,
        ),
        RulePatchOperation::AddOperation { path, .. } => claim(
            "Rule.AddOperation",
            json!([path, "append"]),
            false,
            SemanticRepairKind::RuleOperation,
        ),
        RulePatchOperation::UpdateOperation {
            path,
            operation_index,
            ..
        } => claim(
            "Rule.UpdateOperation",
            json!([path, operation_index]),
            false,
            SemanticRepairKind::RuleOperation,
        ),
        RulePatchOperation::RemoveOperation {
            path,
            operation_index,
            ..
        } => claim(
            "Rule.RemoveOperation",
            json!([path, operation_index]),
            true,
            SemanticRepairKind::None,
        ),
        RulePatchOperation::ValidateAsset { path, .. } => claim(
            "Rule.ValidateAsset",
            json!(path),
            false,
            SemanticRepairKind::None,
        ),
        RulePatchOperation::BuildArtifact { path, .. } => claim(
            "Rule.BuildArtifact",
            json!(path),
            false,
            SemanticRepairKind::None,
        ),
        RulePatchOperation::BuildProjectManifest { path, .. } => claim(
            "Rule.BuildProjectManifest",
            json!(path),
            false,
            SemanticRepairKind::None,
        ),
    }
}

fn build_scope_claim(operation: &BuildPatchOperation) -> PatchOperationScopeClaim {
    match operation {
        BuildPatchOperation::ExportDesktopPackage { profile_id, .. } => claim(
            "Build.ExportDesktopPackage",
            json!(profile_id),
            true,
            SemanticRepairKind::None,
        ),
        BuildPatchOperation::OpenBuildReport { .. } => claim(
            "Build.OpenBuildReport",
            json!("build.report"),
            true,
            SemanticRepairKind::None,
        ),
        BuildPatchOperation::OpenBuildOutput { .. } => claim(
            "Build.OpenBuildOutput",
            json!("build.output"),
            true,
            SemanticRepairKind::None,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        PatchDiagnostic, PatchSource, PatchValidationReport, ProjectPatchImportParseStatus,
        ProjectPatchImportSourceKind, PROJECT_PATCH_IMPORT_RESULT_SCHEMA_VERSION,
        PROJECT_PATCH_SCHEMA_VERSION,
    };

    fn scene_operation(operation_id: &str, name: &str) -> PatchOperation {
        PatchOperation::Scene(ScenePatchOperation::CreateEntity {
            operation_id: operation_id.to_string(),
            depends_on: Vec::new(),
            parent_id: None,
            name: name.to_string(),
        })
    }

    fn patch(operations: Vec<PatchOperation>) -> ProjectPatchDocument {
        ProjectPatchDocument {
            schema_version: PROJECT_PATCH_SCHEMA_VERSION.to_string(),
            patch_id: "repair-scope".to_string(),
            title: "Repair scope".to_string(),
            source: PatchSource::AiAssistant,
            intent_summary: "bounded repair".to_string(),
            target_project_root: Some("project".to_string()),
            required_capabilities: operation_capabilities(&operations),
            operations,
            expected_outcome: "valid patch".to_string(),
            risk_level: PatchRiskLevel::Low,
            created_at: "0".to_string(),
        }
    }

    fn import(
        initial: Option<ProjectPatchDocument>,
        diagnostics: Vec<PatchDiagnostic>,
    ) -> ProjectPatchImportResult {
        let validation = initial
            .as_ref()
            .map(|patch| PatchValidationReport::rejected(patch, diagnostics.clone()));
        ProjectPatchImportResult {
            schema_version: PROJECT_PATCH_IMPORT_RESULT_SCHEMA_VERSION.to_string(),
            source_kind: ProjectPatchImportSourceKind::AiStructuredOutput,
            source_label: "fixture".to_string(),
            parse_status: if initial.is_some() {
                ProjectPatchImportParseStatus::Parsed
            } else {
                ProjectPatchImportParseStatus::Rejected
            },
            parsed_patch: initial,
            schema_diagnostics: if validation.is_none() {
                diagnostics
            } else {
                Vec::new()
            },
            capability_diagnostics: Vec::new(),
            validation,
            review: None,
            proposal_id: None,
            next_actions: Vec::new(),
        }
    }

    fn diagnostic(code: &str) -> PatchDiagnostic {
        PatchDiagnostic::error(
            code,
            "fixture",
            Some("forged".to_string()),
            Some("forged".to_string()),
        )
    }

    fn assert_rejected(
        initial: ProjectPatchDocument,
        repaired: ProjectPatchDocument,
        diagnostics: Vec<PatchDiagnostic>,
        code: &str,
    ) {
        let result = validate_repair_scope(
            &import(Some(initial), diagnostics),
            &repaired,
            RepairScopePolicy::new(48),
        );
        assert_eq!(result.status, RepairScopeValidationStatus::Rejected);
        assert_eq!(result.rejection_code.as_deref(), Some(code));
    }

    #[test]
    fn repair_scope_rejects_count_reorder_kind_target_and_metadata_changes() {
        let one_operation = patch(vec![scene_operation("a", "A")]);
        let mut expanded = one_operation.clone();
        expanded.operations.push(scene_operation("b", "B"));
        assert_rejected(
            one_operation.clone(),
            expanded,
            Vec::new(),
            REPAIR_SCOPE_OPERATION_COUNT_EXPANDED,
        );
        let expanded_to_global_maximum = patch(
            (0..48)
                .map(|index| scene_operation(&format!("op-{index}"), "A"))
                .collect(),
        );
        assert_rejected(
            one_operation,
            expanded_to_global_maximum,
            Vec::new(),
            REPAIR_SCOPE_OPERATION_COUNT_EXPANDED,
        );

        let initial = patch(vec![scene_operation("a", "A"), scene_operation("b", "B")]);
        let mut expanded = initial.clone();
        expanded.operations.push(scene_operation("c", "C"));
        expanded.required_capabilities = operation_capabilities(&expanded.operations);
        assert_rejected(
            initial.clone(),
            expanded,
            Vec::new(),
            REPAIR_SCOPE_OPERATION_COUNT_EXPANDED,
        );

        let mut reordered = initial.clone();
        reordered.operations.swap(0, 1);
        assert_rejected(
            initial.clone(),
            reordered,
            Vec::new(),
            REPAIR_SCOPE_TARGET_CHANGED,
        );

        let mut kind_changed = initial.clone();
        kind_changed.operations[0] = PatchOperation::Scene(ScenePatchOperation::RenameEntity {
            operation_id: "a".to_string(),
            depends_on: Vec::new(),
            entity_id: "entity".to_string(),
            name: "renamed".to_string(),
        });
        assert_rejected(
            initial.clone(),
            kind_changed,
            Vec::new(),
            REPAIR_SCOPE_OPERATION_KIND_CHANGED,
        );

        let mut metadata = initial.clone();
        metadata.title = "changed".to_string();
        assert_rejected(initial, metadata, Vec::new(), REPAIR_SCOPE_METADATA_CHANGED);
    }

    #[test]
    fn repair_scope_rejects_same_kind_target_change_in_every_domain() {
        let cases = vec![
            (
                PatchOperation::Scene(ScenePatchOperation::RenameEntity {
                    operation_id: "op".into(),
                    depends_on: Vec::new(),
                    entity_id: "entity-a".into(),
                    name: "Name".into(),
                }),
                PatchOperation::Scene(ScenePatchOperation::RenameEntity {
                    operation_id: "op".into(),
                    depends_on: Vec::new(),
                    entity_id: "entity-b".into(),
                    name: "Name".into(),
                }),
            ),
            (
                PatchOperation::Input(InputPatchOperation::RemoveInputAction {
                    operation_id: "op".into(),
                    depends_on: Vec::new(),
                    path: "Input/default.json".into(),
                    action_id: "action.a".into(),
                }),
                PatchOperation::Input(InputPatchOperation::RemoveInputAction {
                    operation_id: "op".into(),
                    depends_on: Vec::new(),
                    path: "Input/default.json".into(),
                    action_id: "action.b".into(),
                }),
            ),
            (
                PatchOperation::Asset(AssetPatchOperation::RegisterExistingAsset {
                    operation_id: "op".into(),
                    depends_on: Vec::new(),
                    path: "Assets/a.asset".into(),
                    expected_kind: None,
                }),
                PatchOperation::Asset(AssetPatchOperation::RegisterExistingAsset {
                    operation_id: "op".into(),
                    depends_on: Vec::new(),
                    path: "Assets/b.asset".into(),
                    expected_kind: None,
                }),
            ),
            (
                PatchOperation::Prefab(PrefabPatchOperation::OpenDocument {
                    operation_id: "op".into(),
                    depends_on: Vec::new(),
                    path: "Prefabs/a.prefab.json".into(),
                }),
                PatchOperation::Prefab(PrefabPatchOperation::OpenDocument {
                    operation_id: "op".into(),
                    depends_on: Vec::new(),
                    path: "Prefabs/b.prefab.json".into(),
                }),
            ),
            (
                PatchOperation::Aui(AuiPatchOperation::OpenDocument {
                    operation_id: "op".into(),
                    depends_on: Vec::new(),
                    path: "UI/a.aui.json".into(),
                }),
                PatchOperation::Aui(AuiPatchOperation::OpenDocument {
                    operation_id: "op".into(),
                    depends_on: Vec::new(),
                    path: "UI/b.aui.json".into(),
                }),
            ),
            (
                PatchOperation::Rule(RulePatchOperation::OpenAsset {
                    operation_id: "op".into(),
                    depends_on: Vec::new(),
                    path: "Rules/a.rule.json".into(),
                }),
                PatchOperation::Rule(RulePatchOperation::OpenAsset {
                    operation_id: "op".into(),
                    depends_on: Vec::new(),
                    path: "Rules/b.rule.json".into(),
                }),
            ),
            (
                PatchOperation::Build(BuildPatchOperation::ExportDesktopPackage {
                    operation_id: "op".into(),
                    depends_on: Vec::new(),
                    profile_id: Some("windows-dev".into()),
                }),
                PatchOperation::Build(BuildPatchOperation::ExportDesktopPackage {
                    operation_id: "op".into(),
                    depends_on: Vec::new(),
                    profile_id: Some("other".into()),
                }),
            ),
        ];
        for (initial_operation, repaired_operation) in cases {
            assert_rejected(
                patch(vec![initial_operation]),
                patch(vec![repaired_operation]),
                Vec::new(),
                REPAIR_SCOPE_TARGET_CHANGED,
            );
        }
    }

    #[test]
    fn repair_scope_rejects_risk_and_capability_expansion() {
        let initial = patch(vec![scene_operation("op", "A")]);
        let mut risk = initial.clone();
        risk.risk_level = PatchRiskLevel::High;
        assert_rejected(
            initial.clone(),
            risk,
            Vec::new(),
            REPAIR_SCOPE_RISK_EXPANDED,
        );

        let mut capability = initial.clone();
        capability
            .required_capabilities
            .push(PatchCapability::Build);
        assert_rejected(
            initial.clone(),
            capability,
            Vec::new(),
            REPAIR_SCOPE_CAPABILITY_EXPANDED,
        );

        let mut duplicate = initial.clone();
        duplicate.required_capabilities.push(PatchCapability::Scene);
        assert_rejected(
            initial,
            duplicate,
            Vec::new(),
            REPAIR_SCOPE_CAPABILITY_EXPANDED,
        );
    }

    #[test]
    fn repair_scope_rejects_undiagnosed_field_and_forged_diagnostic_target() {
        let initial = patch(vec![PatchOperation::Scene(
            ScenePatchOperation::SetComponentField {
                operation_id: "set".to_string(),
                depends_on: Vec::new(),
                entity_id: "entity-a".to_string(),
                component_type: "Transform".to_string(),
                field_path: "position.x".to_string(),
                value: json!(1),
            },
        )]);
        let mut repaired = initial.clone();
        let PatchOperation::Scene(ScenePatchOperation::SetComponentField { value, .. }) =
            &mut repaired.operations[0]
        else {
            unreachable!()
        };
        *value = json!(2);
        assert_rejected(
            initial,
            repaired,
            vec![diagnostic(DIAGNOSTIC_SCENE_FIELD_INVALID)],
            REPAIR_SCOPE_AUTHORIZATION_AMBIGUOUS,
        );
    }

    #[test]
    fn repair_scope_allows_only_structurally_invalid_authorized_fields() {
        let initial = patch(vec![PatchOperation::Aui(AuiPatchOperation::SetNodeField {
            operation_id: "set".to_string(),
            depends_on: Vec::new(),
            path: "UI/hud.aui.json".to_string(),
            node_id: "score".to_string(),
            schema_path: String::new(),
            value: Value::Null,
        })]);
        let mut repaired = initial.clone();
        let PatchOperation::Aui(AuiPatchOperation::SetNodeField {
            schema_path, value, ..
        }) = &mut repaired.operations[0]
        else {
            unreachable!()
        };
        *schema_path = "text".to_string();
        *value = json!("Score");
        let result = validate_repair_scope(
            &import(
                Some(initial),
                vec![diagnostic(DIAGNOSTIC_AUI_FIELD_INVALID)],
            ),
            &repaired,
            RepairScopePolicy::new(48),
        );
        assert_eq!(result.status, RepairScopeValidationStatus::Passed);
        assert_eq!(result.changed_slots, vec![0]);
    }

    #[test]
    fn repair_scope_allows_typed_scene_prefab_and_rule_payload_repairs() {
        let cases = vec![
            (
                PatchOperation::Scene(ScenePatchOperation::SetComponentField {
                    operation_id: "op".into(),
                    depends_on: Vec::new(),
                    entity_id: "entity".into(),
                    component_type: "Transform".into(),
                    field_path: String::new(),
                    value: Value::Null,
                }),
                PatchOperation::Scene(ScenePatchOperation::SetComponentField {
                    operation_id: "op".into(),
                    depends_on: Vec::new(),
                    entity_id: "entity".into(),
                    component_type: "Transform".into(),
                    field_path: "position.x".into(),
                    value: json!(1),
                }),
                DIAGNOSTIC_SCENE_FIELD_INVALID,
            ),
            (
                PatchOperation::Prefab(PrefabPatchOperation::SetStageEntityField {
                    operation_id: "op".into(),
                    depends_on: Vec::new(),
                    source_entity_id: "source".into(),
                    component_type: Some("Transform".into()),
                    field_path: String::new(),
                    value: Value::Null,
                }),
                PatchOperation::Prefab(PrefabPatchOperation::SetStageEntityField {
                    operation_id: "op".into(),
                    depends_on: Vec::new(),
                    source_entity_id: "source".into(),
                    component_type: Some("Transform".into()),
                    field_path: "position.x".into(),
                    value: json!(1),
                }),
                DIAGNOSTIC_PREFAB_FIELD_INVALID,
            ),
            (
                PatchOperation::Rule(RulePatchOperation::SetTrigger {
                    operation_id: "op".into(),
                    depends_on: Vec::new(),
                    path: "Rules/a.rule.json".into(),
                    trigger: json!({}),
                    expected_ir_hash: None,
                }),
                PatchOperation::Rule(RulePatchOperation::SetTrigger {
                    operation_id: "op".into(),
                    depends_on: Vec::new(),
                    path: "Rules/a.rule.json".into(),
                    trigger: json!({"kind": "always"}),
                    expected_ir_hash: None,
                }),
                DIAGNOSTIC_RULE_PAYLOAD_INVALID,
            ),
        ];
        for (initial_operation, repaired_operation, code) in cases {
            let result = validate_repair_scope(
                &import(Some(patch(vec![initial_operation])), vec![diagnostic(code)]),
                &patch(vec![repaired_operation]),
                RepairScopePolicy::new(48),
            );
            assert_eq!(result.status, RepairScopeValidationStatus::Passed);
        }
    }

    #[test]
    fn repair_scope_recomputes_missing_and_duplicate_id_slots() {
        let initial = patch(vec![
            scene_operation("dup", "A"),
            scene_operation("dup", "B"),
            scene_operation("", "C"),
        ]);
        let mut repaired = initial.clone();
        for (slot, id) in [(1, "b"), (2, "c")] {
            let PatchOperation::Scene(ScenePatchOperation::CreateEntity { operation_id, .. }) =
                &mut repaired.operations[slot]
            else {
                unreachable!()
            };
            *operation_id = id.to_string();
        }
        let result = validate_repair_scope(
            &import(
                Some(initial),
                vec![
                    diagnostic(DIAGNOSTIC_OPERATION_ID_DUPLICATE),
                    diagnostic(DIAGNOSTIC_OPERATION_ID_REQUIRED),
                ],
            ),
            &repaired,
            RepairScopePolicy::new(48),
        );
        assert_eq!(result.status, RepairScopeValidationStatus::Passed);
        assert_eq!(result.changed_slots, vec![1, 2]);
    }

    #[test]
    fn repair_scope_dependency_fix_cannot_expand_or_target_missing_operation() {
        let mut initial = patch(vec![scene_operation("a", "A"), scene_operation("b", "B")]);
        let PatchOperation::Scene(ScenePatchOperation::CreateEntity { depends_on, .. }) =
            &mut initial.operations[1]
        else {
            unreachable!()
        };
        *depends_on = vec!["missing".to_string()];
        let mut expanded = initial.clone();
        let PatchOperation::Scene(ScenePatchOperation::CreateEntity { depends_on, .. }) =
            &mut expanded.operations[1]
        else {
            unreachable!()
        };
        *depends_on = vec!["a".to_string(), "missing-2".to_string()];
        assert_rejected(
            initial.clone(),
            expanded,
            vec![diagnostic(DIAGNOSTIC_DEPENDENCY_MISSING)],
            REPAIR_SCOPE_DEPENDENCY_EXPANDED,
        );

        let mut repaired = initial.clone();
        let PatchOperation::Scene(ScenePatchOperation::CreateEntity { depends_on, .. }) =
            &mut repaired.operations[1]
        else {
            unreachable!()
        };
        *depends_on = vec!["a".to_string()];
        let result = validate_repair_scope(
            &import(
                Some(initial),
                vec![diagnostic(DIAGNOSTIC_DEPENDENCY_MISSING)],
            ),
            &repaired,
            RepairScopePolicy::new(48),
        );
        assert_eq!(result.status, RepairScopeValidationStatus::Passed);
    }

    #[test]
    fn repair_scope_rejects_destructive_and_build_retargeting() {
        let initial = patch(vec![PatchOperation::Build(
            BuildPatchOperation::ExportDesktopPackage {
                operation_id: "build".to_string(),
                depends_on: Vec::new(),
                profile_id: Some("windows-dev".to_string()),
            },
        )]);
        let mut repaired = initial.clone();
        let PatchOperation::Build(BuildPatchOperation::ExportDesktopPackage { profile_id, .. }) =
            &mut repaired.operations[0]
        else {
            unreachable!()
        };
        *profile_id = Some("other".to_string());
        assert_rejected(initial, repaired, Vec::new(), REPAIR_SCOPE_TARGET_CHANGED);
    }

    #[test]
    fn repair_scope_parse_failed_is_restricted_and_capped_at_eight() {
        let parse_failed = import(None, vec![diagnostic("project_patch_import.parse_failed")]);
        let accepted = patch(vec![scene_operation("a", "A")]);
        let result = validate_repair_scope(&parse_failed, &accepted, RepairScopePolicy::new(48));
        assert_eq!(
            result.status,
            RepairScopeValidationStatus::ScopeUnprovableRestricted
        );

        let too_many = patch(
            (0..9)
                .map(|index| scene_operation(&format!("op-{index}"), "A"))
                .collect(),
        );
        assert_eq!(
            validate_repair_scope(&parse_failed, &too_many, RepairScopePolicy::new(48))
                .rejection_code
                .as_deref(),
            Some(REPAIR_SCOPE_UNPROVABLE_LIMIT_EXCEEDED)
        );

        let destructive = patch(vec![PatchOperation::Scene(
            ScenePatchOperation::DeleteEntity {
                operation_id: "delete".to_string(),
                depends_on: Vec::new(),
                entity_id: "entity".to_string(),
            },
        )]);
        assert_eq!(
            validate_repair_scope(&parse_failed, &destructive, RepairScopePolicy::new(48))
                .rejection_code
                .as_deref(),
            Some(REPAIR_SCOPE_DESTRUCTIVE_OR_BUILD_EXPANDED)
        );

        let mut high_risk = accepted.clone();
        high_risk.risk_level = PatchRiskLevel::High;
        assert_eq!(
            validate_repair_scope(&parse_failed, &high_risk, RepairScopePolicy::new(48))
                .rejection_code
                .as_deref(),
            Some(REPAIR_SCOPE_RISK_EXPANDED)
        );

        let mut mismatched_capability = accepted;
        mismatched_capability.required_capabilities = vec![PatchCapability::Asset];
        assert_eq!(
            validate_repair_scope(
                &parse_failed,
                &mismatched_capability,
                RepairScopePolicy::new(48)
            )
            .rejection_code
            .as_deref(),
            Some(REPAIR_SCOPE_UNAUTHORIZED_FIELD_CHANGED)
        );
    }

    #[test]
    fn scope_claim_is_exhaustive_for_every_current_operation_variant() {
        let common = |id: &str| (id.to_string(), Vec::<String>::new());
        let (id, deps) = common("op");
        let operations = vec![
            scene_operation("scene-create", "Entity"),
            PatchOperation::Scene(ScenePatchOperation::DeleteEntity {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                entity_id: "e".into(),
            }),
            PatchOperation::Scene(ScenePatchOperation::RenameEntity {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                entity_id: "e".into(),
                name: "n".into(),
            }),
            PatchOperation::Scene(ScenePatchOperation::SetTransform {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                entity_id: "e".into(),
                local_position: None,
                local_rotation: None,
                local_scale: None,
            }),
            PatchOperation::Scene(ScenePatchOperation::AddComponent {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                entity_id: "e".into(),
                component_type: "T".into(),
                fields: json!({}),
            }),
            PatchOperation::Scene(ScenePatchOperation::RemoveComponent {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                entity_id: "e".into(),
                component_type: "T".into(),
            }),
            PatchOperation::Scene(ScenePatchOperation::SetComponentField {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                entity_id: "e".into(),
                component_type: "T".into(),
                field_path: "x".into(),
                value: json!(1),
            }),
            PatchOperation::Scene(ScenePatchOperation::PlaceAssetIntoScene {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                asset_id: "a".into(),
                asset_type: "Texture".into(),
                asset_guid: None,
                target_parent_id: None,
                local_position: None,
                placement_mode: editor_ui_model::AssetPlacementMode::WorldOrigin,
            }),
            PatchOperation::Input(InputPatchOperation::CreateDefaultInputMapping {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                path: "Input/default.json".into(),
            }),
            PatchOperation::Input(InputPatchOperation::AddInputAction {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                path: "Input/default.json".into(),
                action_id: "jump".into(),
                value_type: editor_ui_model::InputActionValueKind::Button,
            }),
            PatchOperation::Input(InputPatchOperation::AddInputBinding {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                path: "Input/default.json".into(),
                context_id: "game".into(),
                action_id: "jump".into(),
                device_path: "Keyboard/Space".into(),
            }),
            PatchOperation::Input(InputPatchOperation::RemoveInputAction {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                path: "Input/default.json".into(),
                action_id: "jump".into(),
            }),
            PatchOperation::Input(InputPatchOperation::RemoveInputBinding {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                path: "Input/default.json".into(),
                binding_index: 0,
            }),
            PatchOperation::Input(InputPatchOperation::SetInputBindingDevicePath {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                path: "Input/default.json".into(),
                binding_index: 0,
                device_path: "Keyboard/Space".into(),
            }),
            PatchOperation::Asset(AssetPatchOperation::RegisterExistingAsset {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                path: "Assets/a.asset".into(),
                expected_kind: None,
            }),
            PatchOperation::Asset(AssetPatchOperation::GenerateMockImageAsset {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                prompt: "p".into(),
                target_folder: "Assets".into(),
                asset_name: "a".into(),
                image_kind: "sprite".into(),
                width: 16,
                height: 16,
                transparent_background: true,
            }),
            PatchOperation::Asset(AssetPatchOperation::ValidateAssetBrowserIndex {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                query_kind: None,
            }),
            PatchOperation::Prefab(PrefabPatchOperation::CreateFromSceneSelection {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                scene_path: None,
                root_entity_id: "e".into(),
                prefab_id: "p".into(),
                name: "P".into(),
                replace_selection_with_instance: false,
            }),
            PatchOperation::Prefab(PrefabPatchOperation::OpenDocument {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                path: "Prefabs/p.prefab.json".into(),
            }),
            PatchOperation::Prefab(PrefabPatchOperation::SetStageEntityField {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                source_entity_id: "e".into(),
                component_type: Some("T".into()),
                field_path: "x".into(),
                value: json!(1),
            }),
            PatchOperation::Prefab(PrefabPatchOperation::SaveDocument {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                path: "Prefabs/p.prefab.json".into(),
            }),
            PatchOperation::Prefab(PrefabPatchOperation::InstantiateInScene {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                prefab_id: "p".into(),
                parent_entity_id: None,
                local_position: None,
            }),
            PatchOperation::Prefab(PrefabPatchOperation::ApplyOverrideToAsset {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                instance_entity_id: "i".into(),
                target_source_entity_id: "e".into(),
                component_type: "T".into(),
                field_path: "x".into(),
            }),
            PatchOperation::Prefab(PrefabPatchOperation::RevertOverride {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                instance_entity_id: "i".into(),
                target_source_entity_id: "e".into(),
                component_type: "T".into(),
                field_path: "x".into(),
            }),
            PatchOperation::Prefab(PrefabPatchOperation::ValidateReferences {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                path: None,
            }),
            PatchOperation::Aui(AuiPatchOperation::CreateDocument {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                path: "UI/a.aui.json".into(),
                document_id: "a".into(),
                width: 100.0,
                height: 100.0,
            }),
            PatchOperation::Aui(AuiPatchOperation::OpenDocument {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                path: "UI/a.aui.json".into(),
            }),
            PatchOperation::Aui(AuiPatchOperation::AddNode {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                path: "UI/a.aui.json".into(),
                parent_node_id: "root".into(),
                node_id: "n".into(),
                node_kind: "Text".into(),
                name: "N".into(),
                rect: json!({}),
            }),
            PatchOperation::Aui(AuiPatchOperation::SetNodeField {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                path: "UI/a.aui.json".into(),
                node_id: "n".into(),
                schema_path: "text".into(),
                value: json!("x"),
            }),
            PatchOperation::Aui(AuiPatchOperation::SetBindingPath {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                path: "UI/a.aui.json".into(),
                node_id: "n".into(),
                target_field: "text".into(),
                binding_id: "b".into(),
                binding_path: "score".into(),
                fallback: None,
            }),
            PatchOperation::Aui(AuiPatchOperation::SetActionRef {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                path: "UI/a.aui.json".into(),
                node_id: "n".into(),
                event: "click".into(),
                action_id: "a".into(),
                payload: None,
            }),
            PatchOperation::Aui(AuiPatchOperation::ValidateDocument {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                path: "UI/a.aui.json".into(),
            }),
            PatchOperation::Aui(AuiPatchOperation::SaveDocument {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                path: "UI/a.aui.json".into(),
            }),
            PatchOperation::Aui(AuiPatchOperation::PreviewOverlay {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                path: "UI/a.aui.json".into(),
            }),
            PatchOperation::Rule(RulePatchOperation::CreateAsset {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                path: "Rules/a.rule.json".into(),
                rule_id: "a".into(),
                display_name: "A".into(),
                phase: None,
            }),
            PatchOperation::Rule(RulePatchOperation::OpenAsset {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                path: "Rules/a.rule.json".into(),
            }),
            PatchOperation::Rule(RulePatchOperation::SetTrigger {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                path: "Rules/a.rule.json".into(),
                trigger: json!({}),
                expected_ir_hash: None,
            }),
            PatchOperation::Rule(RulePatchOperation::AddStatement {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                path: "Rules/a.rule.json".into(),
                statement: json!({}),
                expected_ir_hash: None,
            }),
            PatchOperation::Rule(RulePatchOperation::UpdateStatement {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                path: "Rules/a.rule.json".into(),
                statement_index: 0,
                statement: json!({}),
                expected_ir_hash: None,
            }),
            PatchOperation::Rule(RulePatchOperation::RemoveStatement {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                path: "Rules/a.rule.json".into(),
                statement_index: 0,
                expected_ir_hash: None,
            }),
            PatchOperation::Rule(RulePatchOperation::AddOperation {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                path: "Rules/a.rule.json".into(),
                operation: json!({}),
                expected_ir_hash: None,
            }),
            PatchOperation::Rule(RulePatchOperation::UpdateOperation {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                path: "Rules/a.rule.json".into(),
                operation_index: 0,
                operation: json!({}),
                expected_ir_hash: None,
            }),
            PatchOperation::Rule(RulePatchOperation::RemoveOperation {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                path: "Rules/a.rule.json".into(),
                operation_index: 0,
                expected_ir_hash: None,
            }),
            PatchOperation::Rule(RulePatchOperation::ValidateAsset {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                path: "Rules/a.rule.json".into(),
            }),
            PatchOperation::Rule(RulePatchOperation::BuildArtifact {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                path: "Rules/a.rule.json".into(),
            }),
            PatchOperation::Rule(RulePatchOperation::BuildProjectManifest {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                path: "Rules/rule-manifest.json".into(),
            }),
            PatchOperation::Build(BuildPatchOperation::ExportDesktopPackage {
                operation_id: id.clone(),
                depends_on: deps.clone(),
                profile_id: None,
            }),
            PatchOperation::Build(BuildPatchOperation::OpenBuildReport {
                operation_id: id.clone(),
                depends_on: deps.clone(),
            }),
            PatchOperation::Build(BuildPatchOperation::OpenBuildOutput {
                operation_id: id,
                depends_on: deps,
            }),
        ];
        assert_eq!(operations.len(), 49);
        for operation in operations {
            let claim = scope_claim(&operation);
            assert_eq!(claim.kind, operation.kind());
        }
    }
}
