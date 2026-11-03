use engine_runtime::aui::{
    AuiActionEvent, AuiAssetRef, AuiBindingTarget, AuiDocument, AuiNode, AuiNodeKind,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

pub const AUI_TEMPLATE_ASSET_SCHEMA_VERSION: &str = "aui-template-asset.v2";
const AUI_TEMPLATE_ASSET_LEGACY_SCHEMA_VERSION: &str = "aui-template-asset.v1";
pub const AUI_TEMPLATE_INSTANTIATE_REPORT_SCHEMA_VERSION: &str =
    "aui-template-instantiate-report.v2";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuiTemplateOperationStatus {
    Passed,
    Partial,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuiTemplateDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiTemplateDiagnostic {
    pub severity: AuiTemplateDiagnosticSeverity,
    pub code: String,
    pub node_id: Option<String>,
    pub message: String,
    pub suggested_action: String,
}

impl AuiTemplateDiagnostic {
    pub fn info(
        code: impl Into<String>,
        node_id: Option<String>,
        message: impl Into<String>,
        suggested_action: impl Into<String>,
    ) -> Self {
        Self {
            severity: AuiTemplateDiagnosticSeverity::Info,
            code: code.into(),
            node_id,
            message: message.into(),
            suggested_action: suggested_action.into(),
        }
    }

    pub fn warning(
        code: impl Into<String>,
        node_id: Option<String>,
        message: impl Into<String>,
        suggested_action: impl Into<String>,
    ) -> Self {
        Self {
            severity: AuiTemplateDiagnosticSeverity::Warning,
            code: code.into(),
            node_id,
            message: message.into(),
            suggested_action: suggested_action.into(),
        }
    }

    pub fn error(
        code: impl Into<String>,
        node_id: Option<String>,
        message: impl Into<String>,
        suggested_action: impl Into<String>,
    ) -> Self {
        Self {
            severity: AuiTemplateDiagnosticSeverity::Error,
            code: code.into(),
            node_id,
            message: message.into(),
            suggested_action: suggested_action.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiTemplateDependencyRef {
    pub node_id: String,
    pub field_path: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiTemplateMetadata {
    pub created_by: String,
    pub created_at_unix_ms: u64,
    pub source_node_count: usize,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuiTemplateParameterType {
    String,
    Number,
    Boolean,
    AssetRef,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "kebab-case")]
pub enum AuiTemplateParameterValue {
    String(String),
    Number(f32),
    Boolean(bool),
    AssetRef(String),
}

impl AuiTemplateParameterValue {
    fn parameter_type(&self) -> AuiTemplateParameterType {
        match self {
            Self::String(_) => AuiTemplateParameterType::String,
            Self::Number(_) => AuiTemplateParameterType::Number,
            Self::Boolean(_) => AuiTemplateParameterType::Boolean,
            Self::AssetRef(_) => AuiTemplateParameterType::AssetRef,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuiTemplateParameterDefinition {
    pub parameter_id: String,
    pub parameter_type: AuiTemplateParameterType,
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_value: Option<AuiTemplateParameterValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "target", rename_all = "kebab-case")]
pub enum AuiTemplateParameterTarget {
    Text,
    InputFieldText,
    ProgressValue,
    Visible,
    ImageAssetId,
    BindingPath { binding_id: String },
    ActionId { event: AuiActionEvent },
}

impl AuiTemplateParameterTarget {
    fn expected_type(&self) -> AuiTemplateParameterType {
        match self {
            Self::Text
            | Self::InputFieldText
            | Self::BindingPath { .. }
            | Self::ActionId { .. } => AuiTemplateParameterType::String,
            Self::ProgressValue => AuiTemplateParameterType::Number,
            Self::Visible => AuiTemplateParameterType::Boolean,
            Self::ImageAssetId => AuiTemplateParameterType::AssetRef,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiTemplateParameterBinding {
    pub parameter_id: String,
    pub node_id: String,
    pub target: AuiTemplateParameterTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiTemplateSlotDefinition {
    pub slot_id: String,
    pub host_node_id: String,
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_children: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_kinds: Vec<AuiNodeKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiTemplateNamedPartDefinition {
    pub part_name: String,
    pub node_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_kind: Option<AuiNodeKind>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuiTemplateAsset {
    pub schema_version: String,
    pub asset_guid: String,
    pub template_id: String,
    pub display_name: String,
    pub source_document_path: String,
    pub source_document_id: String,
    pub root_node_id: String,
    pub nodes: Vec<AuiNode>,
    pub asset_refs: Vec<AuiTemplateDependencyRef>,
    pub binding_refs: Vec<AuiTemplateDependencyRef>,
    pub action_refs: Vec<AuiTemplateDependencyRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<AuiTemplateParameterDefinition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameter_bindings: Vec<AuiTemplateParameterBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slots: Vec<AuiTemplateSlotDefinition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub named_parts: Vec<AuiTemplateNamedPartDefinition>,
    pub metadata: AuiTemplateMetadata,
    pub guid_source: String,
    pub asset_db_integrated: bool,
}

impl AuiTemplateAsset {
    pub fn from_document_subtree(
        document: &AuiDocument,
        source_document_path: impl Into<String>,
        template_asset_path: impl AsRef<Path>,
        root_node_id: impl Into<String>,
        template_id: impl Into<String>,
        display_name: impl Into<String>,
        created_at_unix_ms: u64,
    ) -> Result<Self, Vec<AuiTemplateDiagnostic>> {
        let source_document_path = source_document_path.into();
        let root_node_id = root_node_id.into();
        let template_id = template_id.into();
        let display_name = display_name.into();
        let mut diagnostics = Vec::new();
        if template_id.trim().is_empty() {
            diagnostics.push(AuiTemplateDiagnostic::error(
                "aui_template.template_id_required",
                None,
                "AUI template_id is required.",
                "Provide a stable template_id.",
            ));
        }
        if root_node_id.trim().is_empty() {
            diagnostics.push(AuiTemplateDiagnostic::error(
                "aui_template.root_node_required",
                None,
                "AUI template root_node_id is required.",
                "Select a concrete AUI node subtree root.",
            ));
        }
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }

        let nodes = collect_subtree_nodes(document, &root_node_id)?;
        let mut normalized_nodes = nodes;
        if let Some(root) = normalized_nodes
            .iter_mut()
            .find(|node| node.node_id == root_node_id)
        {
            root.parent = None;
        }
        let asset_refs = collect_asset_refs(&normalized_nodes);
        let binding_refs = collect_binding_refs(&normalized_nodes);
        let action_refs = collect_action_refs(&normalized_nodes);
        let asset_path = normalize_path(template_asset_path.as_ref());

        Ok(Self {
            schema_version: AUI_TEMPLATE_ASSET_SCHEMA_VERSION.to_string(),
            asset_guid: deterministic_guid("aui-template", &asset_path, &template_id),
            template_id,
            display_name,
            source_document_path,
            source_document_id: document.document_id.clone(),
            root_node_id,
            metadata: AuiTemplateMetadata {
                created_by: "editor_core.aui_template".to_string(),
                created_at_unix_ms,
                source_node_count: normalized_nodes.len(),
                notes: vec![
                    "C2 template asset; typed instantiate-by-expansion only.".to_string(),
                    "guid_source=deterministic_path_hash; asset_db_integrated=false.".to_string(),
                ],
            },
            nodes: normalized_nodes,
            asset_refs,
            binding_refs,
            action_refs,
            parameters: Vec::new(),
            parameter_bindings: Vec::new(),
            slots: Vec::new(),
            named_parts: Vec::new(),
            guid_source: "deterministic_path_hash".to_string(),
            asset_db_integrated: false,
        })
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let project_root = path.parent().unwrap_or_else(|| Path::new("."));
        let file_name = path.file_name().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "AUI template save path has no file name",
            )
        })?;
        let scope = crate::ProjectWriteScope::open(project_root)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        self.save_in_scope(&scope, Path::new(file_name))
    }

    pub fn save_in_scope(
        &self,
        scope: &crate::ProjectWriteScope,
        relative_path: impl AsRef<Path>,
    ) -> std::io::Result<()> {
        let text = serde_json::to_string_pretty(self)?;
        scope
            .write_atomic(relative_path, text.as_bytes())
            .map(|_| ())
            .map_err(|error| std::io::Error::other(error.to_string()))
    }

    pub fn open(path: &Path) -> std::io::Result<Self> {
        let text = fs::read_to_string(path)?;
        let asset = serde_json::from_str::<Self>(&text)?;
        Ok(asset)
    }

    pub fn validate(&self) -> Vec<AuiTemplateDiagnostic> {
        let mut diagnostics = Vec::new();
        if self.schema_version != AUI_TEMPLATE_ASSET_SCHEMA_VERSION
            && self.schema_version != AUI_TEMPLATE_ASSET_LEGACY_SCHEMA_VERSION
        {
            diagnostics.push(AuiTemplateDiagnostic::error(
                "aui_template.unsupported_schema_version",
                None,
                format!(
                    "Unsupported AUI template schema_version '{}'.",
                    self.schema_version
                ),
                "Regenerate the template using the current editor.",
            ));
        }
        if self.asset_guid.trim().is_empty() {
            diagnostics.push(AuiTemplateDiagnostic::error(
                "aui_template.asset_guid_required",
                None,
                "AUI template asset_guid is required.",
                "Regenerate the template asset.",
            ));
        }
        if self.nodes.is_empty() {
            diagnostics.push(AuiTemplateDiagnostic::error(
                "aui_template.nodes_required",
                None,
                "AUI template contains no nodes.",
                "Save a non-empty AUI subtree as template.",
            ));
        }
        if !self
            .nodes
            .iter()
            .any(|node| node.node_id == self.root_node_id)
        {
            diagnostics.push(AuiTemplateDiagnostic::error(
                "aui_template.root_missing",
                Some(self.root_node_id.clone()),
                format!("AUI template root node '{}' is missing.", self.root_node_id),
                "Regenerate the template from a valid AUI subtree.",
            ));
        }
        if self.schema_version == AUI_TEMPLATE_ASSET_LEGACY_SCHEMA_VERSION
            && (!self.parameters.is_empty()
                || !self.parameter_bindings.is_empty()
                || !self.slots.is_empty()
                || !self.named_parts.is_empty())
        {
            diagnostics.push(AuiTemplateDiagnostic::error(
                "aui_template.legacy_schema_has_v2_fields",
                None,
                "A legacy v1 AUI template cannot declare v2 composition fields.",
                "Upgrade schema_version to aui-template-asset.v2.",
            ));
        }

        let mut node_ids = HashSet::new();
        for node in &self.nodes {
            if !node_ids.insert(node.node_id.as_str()) {
                diagnostics.push(AuiTemplateDiagnostic::error(
                    "aui_template.duplicate_node_id",
                    Some(node.node_id.clone()),
                    format!("AUI template node id '{}' is duplicated.", node.node_id),
                    "Give every template-local node a unique id.",
                ));
            }
        }

        let mut parameters = HashMap::new();
        for parameter in &self.parameters {
            if parameter.parameter_id.trim().is_empty() {
                diagnostics.push(AuiTemplateDiagnostic::error(
                    "aui_template.parameter_id_required",
                    None,
                    "AUI template parameter_id is required.",
                    "Give every parameter a stable non-empty id.",
                ));
                continue;
            }
            if parameters
                .insert(parameter.parameter_id.as_str(), parameter)
                .is_some()
            {
                diagnostics.push(AuiTemplateDiagnostic::error(
                    "aui_template.duplicate_parameter",
                    None,
                    format!(
                        "AUI template parameter '{}' is declared more than once.",
                        parameter.parameter_id
                    ),
                    "Keep one definition for each parameter id.",
                ));
            }
            if let Some(default) = &parameter.default_value {
                if default.parameter_type() != parameter.parameter_type {
                    diagnostics.push(AuiTemplateDiagnostic::error(
                        "aui_template.default_parameter_type_mismatch",
                        None,
                        format!(
                            "Default value for parameter '{}' does not match its declared type.",
                            parameter.parameter_id
                        ),
                        "Use a default value with the declared parameter type.",
                    ));
                }
                validate_parameter_value(
                    &parameter.parameter_id,
                    default,
                    &mut diagnostics,
                    "default",
                );
            }
        }

        for binding in &self.parameter_bindings {
            let Some(parameter) = parameters.get(binding.parameter_id.as_str()) else {
                diagnostics.push(AuiTemplateDiagnostic::error(
                    "aui_template.parameter_binding_unknown_parameter",
                    Some(binding.node_id.clone()),
                    format!(
                        "Parameter binding references unknown parameter '{}'.",
                        binding.parameter_id
                    ),
                    "Declare the parameter before binding it.",
                ));
                continue;
            };
            if parameter.parameter_type != binding.target.expected_type() {
                diagnostics.push(AuiTemplateDiagnostic::error(
                    "aui_template.parameter_target_type_mismatch",
                    Some(binding.node_id.clone()),
                    format!(
                        "Parameter '{}' type does not match its target field.",
                        binding.parameter_id
                    ),
                    "Use a target compatible with the declared parameter type.",
                ));
            }
            let Some(node) = self
                .nodes
                .iter()
                .find(|node| node.node_id == binding.node_id)
            else {
                diagnostics.push(AuiTemplateDiagnostic::error(
                    "aui_template.parameter_target_node_missing",
                    Some(binding.node_id.clone()),
                    format!(
                        "Parameter '{}' targets missing node '{}'.",
                        binding.parameter_id, binding.node_id
                    ),
                    "Bind the parameter to an existing template-local node.",
                ));
                continue;
            };
            validate_parameter_target(node, &binding.target, &mut diagnostics);
        }

        let mut slot_ids = HashSet::new();
        for slot in &self.slots {
            if slot.slot_id.trim().is_empty() {
                diagnostics.push(AuiTemplateDiagnostic::error(
                    "aui_template.slot_id_required",
                    Some(slot.host_node_id.clone()),
                    "AUI template slot_id is required.",
                    "Give every content slot a stable non-empty id.",
                ));
            } else if !slot_ids.insert(slot.slot_id.as_str()) {
                diagnostics.push(AuiTemplateDiagnostic::error(
                    "aui_template.duplicate_slot",
                    Some(slot.host_node_id.clone()),
                    format!(
                        "AUI template slot '{}' is declared more than once.",
                        slot.slot_id
                    ),
                    "Keep one definition for each content slot id.",
                ));
            }
            if !self
                .nodes
                .iter()
                .any(|node| node.node_id == slot.host_node_id)
            {
                diagnostics.push(AuiTemplateDiagnostic::error(
                    "aui_template.slot_host_missing",
                    Some(slot.host_node_id.clone()),
                    format!(
                        "Content slot '{}' references missing host node '{}'.",
                        slot.slot_id, slot.host_node_id
                    ),
                    "Use an existing template-local node as the slot host.",
                ));
            }
            if slot.max_children == Some(0) {
                diagnostics.push(AuiTemplateDiagnostic::error(
                    "aui_template.slot_max_children_zero",
                    Some(slot.host_node_id.clone()),
                    format!(
                        "Content slot '{}' cannot accept any children.",
                        slot.slot_id
                    ),
                    "Remove the slot or set max_children to at least one.",
                ));
            }
        }

        let mut part_names = HashSet::new();
        for part in &self.named_parts {
            if part.part_name.trim().is_empty() {
                diagnostics.push(AuiTemplateDiagnostic::error(
                    "aui_template.named_part_name_required",
                    Some(part.node_id.clone()),
                    "AUI template named part requires part_name.",
                    "Give every named part a stable non-empty name.",
                ));
            } else if !part_names.insert(part.part_name.as_str()) {
                diagnostics.push(AuiTemplateDiagnostic::error(
                    "aui_template.duplicate_named_part",
                    Some(part.node_id.clone()),
                    format!(
                        "AUI template named part '{}' is declared more than once.",
                        part.part_name
                    ),
                    "Keep one declaration for each named part.",
                ));
            }
            let Some(node) = self.nodes.iter().find(|node| node.node_id == part.node_id) else {
                diagnostics.push(AuiTemplateDiagnostic::error(
                    "aui_template.named_part_node_missing",
                    Some(part.node_id.clone()),
                    format!(
                        "Named part '{}' references missing node '{}'.",
                        part.part_name, part.node_id
                    ),
                    "Map the named part to an existing template-local node.",
                ));
                continue;
            };
            if part.required_kind.is_some_and(|kind| node.kind != kind) {
                diagnostics.push(AuiTemplateDiagnostic::error(
                    "aui_template.named_part_kind_mismatch",
                    Some(part.node_id.clone()),
                    format!(
                        "Named part '{}' has kind {:?}, expected {:?}.",
                        part.part_name, node.kind, part.required_kind
                    ),
                    "Use a node with the required kind or correct the part contract.",
                ));
            }
        }
        diagnostics
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiTemplateRef {
    pub asset_guid: String,
    pub template_id: String,
    pub asset_path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuiTemplateInstantiateRequest {
    pub template_ref: AuiTemplateRef,
    pub target_document_path: String,
    pub parent_node_id: String,
    pub insertion_index: Option<usize>,
    pub instance_id: String,
    pub node_id_prefix: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<AuiTemplateParameterAssignment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slot_fills: Vec<AuiTemplateSlotFill>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuiTemplateParameterAssignment {
    pub parameter_id: String,
    pub value: AuiTemplateParameterValue,
}

impl AuiTemplateParameterAssignment {
    pub fn new(parameter_id: impl Into<String>, value: AuiTemplateParameterValue) -> Self {
        Self {
            parameter_id: parameter_id.into(),
            value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiTemplateSlotFill {
    pub slot_id: String,
    pub node_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiTemplateNodeIdRemap {
    pub source_node_id: String,
    pub inserted_node_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiTemplateSourceMapEntry {
    pub template_id: String,
    pub instance_id: String,
    pub source_document_path: String,
    pub source_node_id: String,
    pub inserted_node_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiTemplateNamedPartMap {
    pub part_name: String,
    pub source_node_id: String,
    pub inserted_node_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiTemplateInstantiateReport {
    pub schema_version: String,
    pub status: AuiTemplateOperationStatus,
    pub template_ref: AuiTemplateRef,
    pub instance_id: String,
    pub target_document_path: String,
    pub parent_node_id: String,
    pub inserted_node_count: usize,
    pub node_id_remap: Vec<AuiTemplateNodeIdRemap>,
    pub source_map: Vec<AuiTemplateSourceMapEntry>,
    pub named_parts: Vec<AuiTemplateNamedPartMap>,
    pub asset_ref_count: usize,
    pub binding_ref_count: usize,
    pub action_ref_count: usize,
    pub copied_binding_refs: Vec<AuiTemplateDependencyRef>,
    pub copied_action_refs: Vec<AuiTemplateDependencyRef>,
    pub copied_asset_refs: Vec<AuiTemplateDependencyRef>,
    pub override_supported: bool,
    pub linked_instance_supported: bool,
    pub runtime_instance_supported: bool,
    pub guid_source: String,
    pub asset_db_integrated: bool,
    pub diagnostics: Vec<AuiTemplateDiagnostic>,
}

impl AuiTemplateInstantiateReport {
    fn new(request: &AuiTemplateInstantiateRequest, asset: &AuiTemplateAsset) -> Self {
        Self {
            schema_version: AUI_TEMPLATE_INSTANTIATE_REPORT_SCHEMA_VERSION.to_string(),
            status: AuiTemplateOperationStatus::Failed,
            template_ref: request.template_ref.clone(),
            instance_id: request.instance_id.clone(),
            target_document_path: request.target_document_path.clone(),
            parent_node_id: request.parent_node_id.clone(),
            inserted_node_count: 0,
            node_id_remap: Vec::new(),
            source_map: Vec::new(),
            named_parts: Vec::new(),
            asset_ref_count: asset.asset_refs.len(),
            binding_ref_count: asset.binding_refs.len(),
            action_ref_count: asset.action_refs.len(),
            copied_binding_refs: Vec::new(),
            copied_action_refs: Vec::new(),
            copied_asset_refs: Vec::new(),
            override_supported: false,
            linked_instance_supported: false,
            runtime_instance_supported: false,
            guid_source: asset.guid_source.clone(),
            asset_db_integrated: asset.asset_db_integrated,
            diagnostics: Vec::new(),
        }
    }

    fn recompute_status(&mut self) {
        if self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == AuiTemplateDiagnosticSeverity::Error)
        {
            self.status = AuiTemplateOperationStatus::Failed;
        } else if self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == AuiTemplateDiagnosticSeverity::Warning)
        {
            self.status = AuiTemplateOperationStatus::Partial;
        } else {
            self.status = AuiTemplateOperationStatus::Passed;
        }
    }
}

pub struct AuiTemplateWorkflow;

impl AuiTemplateWorkflow {
    pub fn instantiate_into_document(
        asset: &AuiTemplateAsset,
        request: &AuiTemplateInstantiateRequest,
        document: &mut AuiDocument,
    ) -> AuiTemplateInstantiateReport {
        let mut report = AuiTemplateInstantiateReport::new(request, asset);
        report.diagnostics.extend(asset.validate());

        if request.template_ref.asset_guid != asset.asset_guid {
            report.diagnostics.push(AuiTemplateDiagnostic::error(
                "aui_template.asset_guid_mismatch",
                None,
                format!(
                    "Template ref guid '{}' does not match asset guid '{}'.",
                    request.template_ref.asset_guid, asset.asset_guid
                ),
                "Reload the template asset and use its asset_guid.",
            ));
        }
        if request.template_ref.template_id != asset.template_id {
            report.diagnostics.push(AuiTemplateDiagnostic::error(
                "aui_template.template_id_mismatch",
                None,
                format!(
                    "Template ref id '{}' does not match asset template_id '{}'.",
                    request.template_ref.template_id, asset.template_id
                ),
                "Reload the template asset and use its template_id.",
            ));
        }
        if !document
            .nodes
            .iter()
            .any(|node| node.node_id == request.parent_node_id)
        {
            report.diagnostics.push(AuiTemplateDiagnostic::error(
                "aui_template.parent_missing",
                Some(request.parent_node_id.clone()),
                format!(
                    "Target AUI document parent node '{}' does not exist.",
                    request.parent_node_id
                ),
                "Choose an existing AUI node as insertion parent.",
            ));
        }
        if request.instance_id.trim().is_empty() {
            report.diagnostics.push(AuiTemplateDiagnostic::error(
                "aui_template.instance_id_required",
                None,
                "AUI template instantiate requires instance_id.",
                "Provide a stable instance_id for reports.",
            ));
        }

        let parameter_definitions = asset
            .parameters
            .iter()
            .map(|definition| (definition.parameter_id.as_str(), definition))
            .collect::<HashMap<_, _>>();
        let mut resolved_parameters = asset
            .parameters
            .iter()
            .filter_map(|definition| {
                definition
                    .default_value
                    .clone()
                    .map(|value| (definition.parameter_id.clone(), value))
            })
            .collect::<HashMap<_, _>>();
        let mut assigned_parameters = HashSet::new();
        for assignment in &request.parameters {
            let Some(definition) = parameter_definitions.get(assignment.parameter_id.as_str())
            else {
                report.diagnostics.push(AuiTemplateDiagnostic::error(
                    "aui_template.unknown_parameter",
                    None,
                    format!(
                        "Instantiate request provides unknown parameter '{}'.",
                        assignment.parameter_id
                    ),
                    "Remove the assignment or declare the parameter in the template.",
                ));
                continue;
            };
            if !assigned_parameters.insert(assignment.parameter_id.as_str()) {
                report.diagnostics.push(AuiTemplateDiagnostic::error(
                    "aui_template.duplicate_parameter_assignment",
                    None,
                    format!(
                        "Instantiate request assigns parameter '{}' more than once.",
                        assignment.parameter_id
                    ),
                    "Provide one value for each parameter.",
                ));
                continue;
            }
            if assignment.value.parameter_type() != definition.parameter_type {
                report.diagnostics.push(AuiTemplateDiagnostic::error(
                    "aui_template.parameter_type_mismatch",
                    None,
                    format!(
                        "Value for parameter '{}' does not match its declared type.",
                        assignment.parameter_id
                    ),
                    "Provide a value with the declared parameter type.",
                ));
                continue;
            }
            validate_parameter_value(
                &assignment.parameter_id,
                &assignment.value,
                &mut report.diagnostics,
                "assignment",
            );
            resolved_parameters.insert(assignment.parameter_id.clone(), assignment.value.clone());
        }
        for definition in &asset.parameters {
            if definition.required && !resolved_parameters.contains_key(&definition.parameter_id) {
                report.diagnostics.push(AuiTemplateDiagnostic::error(
                    "aui_template.required_parameter_missing",
                    None,
                    format!(
                        "Required AUI template parameter '{}' was not provided.",
                        definition.parameter_id
                    ),
                    "Provide the required parameter value.",
                ));
            }
        }

        let slot_definitions = asset
            .slots
            .iter()
            .map(|slot| (slot.slot_id.as_str(), slot))
            .collect::<HashMap<_, _>>();
        let mut filled_slots = HashSet::new();
        let mut filled_node_ids = HashSet::new();
        for fill in &request.slot_fills {
            let Some(slot) = slot_definitions.get(fill.slot_id.as_str()) else {
                report.diagnostics.push(AuiTemplateDiagnostic::error(
                    "aui_template.unknown_slot",
                    None,
                    format!("Instantiate request fills unknown slot '{}'.", fill.slot_id),
                    "Remove the fill or declare the content slot in the template.",
                ));
                continue;
            };
            if !filled_slots.insert(fill.slot_id.as_str()) {
                report.diagnostics.push(AuiTemplateDiagnostic::error(
                    "aui_template.duplicate_slot_fill",
                    Some(slot.host_node_id.clone()),
                    format!("Content slot '{}' is filled more than once.", fill.slot_id),
                    "Provide one fill entry for each content slot.",
                ));
                continue;
            }
            if slot.required && fill.node_ids.is_empty() {
                report.diagnostics.push(AuiTemplateDiagnostic::error(
                    "aui_template.required_slot_empty",
                    Some(slot.host_node_id.clone()),
                    format!(
                        "Required content slot '{}' has an empty fill.",
                        fill.slot_id
                    ),
                    "Provide at least one target-document subtree root.",
                ));
            }
            if slot
                .max_children
                .is_some_and(|maximum| fill.node_ids.len() > maximum)
            {
                report.diagnostics.push(AuiTemplateDiagnostic::error(
                    "aui_template.slot_max_children_exceeded",
                    Some(slot.host_node_id.clone()),
                    format!(
                        "Content slot '{}' received {} children, exceeding its limit {:?}.",
                        fill.slot_id,
                        fill.node_ids.len(),
                        slot.max_children
                    ),
                    "Reduce the slot fill child count.",
                ));
            }
            for node_id in &fill.node_ids {
                let Some(node) = document.nodes.iter().find(|node| node.node_id == *node_id) else {
                    report.diagnostics.push(AuiTemplateDiagnostic::error(
                        "aui_template.slot_fill_node_missing",
                        Some(node_id.clone()),
                        format!(
                            "Content slot '{}' fill node '{}' is missing.",
                            fill.slot_id, node_id
                        ),
                        "Fill the slot with an existing target-document subtree root.",
                    ));
                    continue;
                };
                if !filled_node_ids.insert(node_id.as_str()) {
                    report.diagnostics.push(AuiTemplateDiagnostic::error(
                        "aui_template.slot_fill_node_reused",
                        Some(node_id.clone()),
                        format!(
                            "Target node '{}' is used by more than one slot fill.",
                            node_id
                        ),
                        "Use each target subtree root in at most one slot fill.",
                    ));
                }
                if node.parent.is_none() {
                    report.diagnostics.push(AuiTemplateDiagnostic::error(
                        "aui_template.slot_fill_root_forbidden",
                        Some(node_id.clone()),
                        format!(
                            "Root node '{}' cannot be moved into a content slot.",
                            node_id
                        ),
                        "Use a non-root target-document subtree.",
                    ));
                }
                if node_id == &request.parent_node_id
                    || document_node_is_ancestor(document, node_id, &request.parent_node_id)
                {
                    report.diagnostics.push(AuiTemplateDiagnostic::error(
                        "aui_template.slot_fill_cycle",
                        Some(node_id.clone()),
                        format!(
                            "Moving node '{}' into slot '{}' would create a parent cycle.",
                            node_id, fill.slot_id
                        ),
                        "Choose content outside the template insertion parent chain.",
                    ));
                }
                if !slot.allowed_kinds.is_empty() && !slot.allowed_kinds.contains(&node.kind) {
                    report.diagnostics.push(AuiTemplateDiagnostic::error(
                        "aui_template.slot_fill_kind_mismatch",
                        Some(node_id.clone()),
                        format!(
                            "Node '{}' kind {:?} is not allowed in content slot '{}'.",
                            node_id, node.kind, fill.slot_id
                        ),
                        "Use a node kind allowed by the content slot contract.",
                    ));
                }
            }
        }
        for slot in &asset.slots {
            if slot.required && !filled_slots.contains(slot.slot_id.as_str()) {
                report.diagnostics.push(AuiTemplateDiagnostic::error(
                    "aui_template.required_slot_missing",
                    Some(slot.host_node_id.clone()),
                    format!("Required content slot '{}' was not filled.", slot.slot_id),
                    "Provide content for the required slot.",
                ));
            }
        }
        for left in &request.slot_fills {
            for left_node in &left.node_ids {
                for right in &request.slot_fills {
                    for right_node in &right.node_ids {
                        if left_node != right_node
                            && document_node_is_ancestor(document, left_node, right_node)
                        {
                            report.diagnostics.push(AuiTemplateDiagnostic::error(
                                "aui_template.overlapping_slot_fill_subtrees",
                                Some(right_node.clone()),
                                format!(
                                    "Slot fill node '{}' is inside another filled subtree '{}'.",
                                    right_node, left_node
                                ),
                                "Fill slots with disjoint target-document subtree roots.",
                            ));
                        }
                    }
                }
            }
        }
        if report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == AuiTemplateDiagnosticSeverity::Error)
        {
            report.recompute_status();
            return report;
        }

        let existing_ids = document
            .nodes
            .iter()
            .map(|node| node.node_id.clone())
            .collect::<HashSet<_>>();
        let prefix = if request.node_id_prefix.trim().is_empty() {
            request.instance_id.as_str()
        } else {
            request.node_id_prefix.as_str()
        };
        let mut used_ids = existing_ids.clone();
        let mut remap = HashMap::new();
        let mut source_node_ids = asset
            .nodes
            .iter()
            .map(|node| node.node_id.as_str())
            .collect::<Vec<_>>();
        source_node_ids.sort_unstable();
        for source_node_id in source_node_ids {
            let inserted = make_unique_node_id(prefix, source_node_id, &mut used_ids);
            remap.insert(source_node_id.to_string(), inserted.clone());
            report.node_id_remap.push(AuiTemplateNodeIdRemap {
                source_node_id: source_node_id.to_string(),
                inserted_node_id: inserted.clone(),
            });
            report.source_map.push(AuiTemplateSourceMapEntry {
                template_id: asset.template_id.clone(),
                instance_id: request.instance_id.clone(),
                source_document_path: asset.source_document_path.clone(),
                source_node_id: source_node_id.to_string(),
                inserted_node_id: inserted,
            });
        }

        let mut inserted_nodes = Vec::new();
        for source in &asset.nodes {
            let mut node = source.clone();
            node.node_id = remap
                .get(&source.node_id)
                .cloned()
                .unwrap_or_else(|| source.node_id.clone());
            node.parent = source
                .parent
                .as_ref()
                .and_then(|parent| remap.get(parent).cloned())
                .or_else(|| {
                    (source.node_id == asset.root_node_id).then(|| request.parent_node_id.clone())
                });
            node.children = source
                .children
                .iter()
                .filter_map(|child| remap.get(child).cloned())
                .collect();
            inserted_nodes.push(node);
        }

        for binding in &asset.parameter_bindings {
            let Some(value) = resolved_parameters.get(&binding.parameter_id) else {
                continue;
            };
            let Some(inserted_node_id) = remap.get(&binding.node_id) else {
                continue;
            };
            let Some(node) = inserted_nodes
                .iter_mut()
                .find(|node| node.node_id == *inserted_node_id)
            else {
                continue;
            };
            if let Err(diagnostic) = apply_parameter_value(node, &binding.target, value) {
                report.diagnostics.push(diagnostic);
            }
        }

        for part in &asset.named_parts {
            if let Some(inserted_node_id) = remap.get(&part.node_id) {
                report.named_parts.push(AuiTemplateNamedPartMap {
                    part_name: part.part_name.clone(),
                    source_node_id: part.node_id.clone(),
                    inserted_node_id: inserted_node_id.clone(),
                });
            }
        }

        if report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == AuiTemplateDiagnosticSeverity::Error)
        {
            report.recompute_status();
            return report;
        }

        let Some(root_inserted_id) = remap.get(&asset.root_node_id).cloned() else {
            report.diagnostics.push(AuiTemplateDiagnostic::error(
                "aui_template.root_remap_missing",
                Some(asset.root_node_id.clone()),
                "AUI template root could not be remapped.",
                "Regenerate the template asset.",
            ));
            report.recompute_status();
            return report;
        };
        let copied_asset_refs = collect_asset_refs(&inserted_nodes);
        let copied_binding_refs = collect_binding_refs(&inserted_nodes);
        let copied_action_refs = collect_action_refs(&inserted_nodes);

        let mut next_document = document.clone();
        if let Some(parent) = next_document
            .nodes
            .iter_mut()
            .find(|node| node.node_id == request.parent_node_id)
        {
            let index = request
                .insertion_index
                .unwrap_or(parent.children.len())
                .min(parent.children.len());
            parent.children.insert(index, root_inserted_id);
        }
        next_document.nodes.extend(inserted_nodes);

        for fill in &request.slot_fills {
            let Some(slot) = slot_definitions.get(fill.slot_id.as_str()) else {
                continue;
            };
            let Some(host_node_id) = remap.get(&slot.host_node_id).cloned() else {
                continue;
            };
            for fill_node_id in &fill.node_ids {
                let old_parent_id = next_document
                    .nodes
                    .iter()
                    .find(|node| node.node_id == *fill_node_id)
                    .and_then(|node| node.parent.clone());
                if let Some(old_parent_id) = old_parent_id {
                    if let Some(old_parent) = next_document
                        .nodes
                        .iter_mut()
                        .find(|node| node.node_id == old_parent_id)
                    {
                        old_parent.children.retain(|child| child != fill_node_id);
                    }
                }
                if let Some(fill_node) = next_document
                    .nodes
                    .iter_mut()
                    .find(|node| node.node_id == *fill_node_id)
                {
                    fill_node.parent = Some(host_node_id.clone());
                }
                if let Some(host) = next_document
                    .nodes
                    .iter_mut()
                    .find(|node| node.node_id == host_node_id)
                {
                    host.children.push(fill_node_id.clone());
                }
            }
        }

        report.inserted_node_count = asset.nodes.len();
        report.copied_asset_refs = copied_asset_refs;
        report.copied_binding_refs = copied_binding_refs;
        report.copied_action_refs = copied_action_refs;
        report.asset_ref_count = report.copied_asset_refs.len();
        report.binding_ref_count = report.copied_binding_refs.len();
        report.action_ref_count = report.copied_action_refs.len();
        if template_has_unparameterized_binding(asset) {
            report.diagnostics.push(AuiTemplateDiagnostic::warning(
                "aui_template.binding_refs_unparameterized",
                None,
                "AUI template binding refs were copied without parameterization.",
                "Inspect copied_binding_refs and rewrite paths for each instance if needed.",
            ));
        }
        if template_has_unparameterized_action(asset) {
            report.diagnostics.push(AuiTemplateDiagnostic::warning(
                "aui_template.action_refs_unparameterized",
                None,
                "AUI template action refs were copied without parameterization.",
                "Inspect copied_action_refs and rewrite action ids for each instance if needed.",
            ));
        }
        *document = next_document;
        report.recompute_status();
        report
    }
}

fn validate_parameter_value(
    parameter_id: &str,
    value: &AuiTemplateParameterValue,
    diagnostics: &mut Vec<AuiTemplateDiagnostic>,
    source: &str,
) {
    match value {
        AuiTemplateParameterValue::Number(value) if !value.is_finite() => {
            diagnostics.push(AuiTemplateDiagnostic::error(
                "aui_template.parameter_number_not_finite",
                None,
                format!("Parameter '{parameter_id}' {source} value must be finite."),
                "Use a finite numeric value.",
            ));
        }
        AuiTemplateParameterValue::AssetRef(value) if value.trim().is_empty() => {
            diagnostics.push(AuiTemplateDiagnostic::error(
                "aui_template.parameter_asset_ref_empty",
                None,
                format!("Parameter '{parameter_id}' {source} asset ref is empty."),
                "Use a non-empty asset id.",
            ));
        }
        _ => {}
    }
}

fn validate_parameter_target(
    node: &AuiNode,
    target: &AuiTemplateParameterTarget,
    diagnostics: &mut Vec<AuiTemplateDiagnostic>,
) {
    let compatible = match target {
        AuiTemplateParameterTarget::Text => {
            matches!(node.kind, AuiNodeKind::Text | AuiNodeKind::Button)
        }
        AuiTemplateParameterTarget::InputFieldText => node.kind == AuiNodeKind::InputField,
        AuiTemplateParameterTarget::ProgressValue => node.kind == AuiNodeKind::ProgressBar,
        AuiTemplateParameterTarget::Visible => true,
        AuiTemplateParameterTarget::ImageAssetId => node.kind == AuiNodeKind::Image,
        AuiTemplateParameterTarget::BindingPath { binding_id } => {
            node.binding_refs
                .iter()
                .filter(|binding| binding.binding_id == *binding_id)
                .count()
                == 1
        }
        AuiTemplateParameterTarget::ActionId { event } => {
            node.action_refs
                .iter()
                .filter(|action| action.event == *event)
                .count()
                == 1
        }
    };
    if !compatible {
        diagnostics.push(AuiTemplateDiagnostic::error(
            "aui_template.parameter_target_field_missing",
            Some(node.node_id.clone()),
            format!(
                "Node '{}' does not expose the requested template parameter target {:?}.",
                node.node_id, target
            ),
            "Choose a compatible node field, binding id, or action event.",
        ));
    }
}

fn apply_parameter_value(
    node: &mut AuiNode,
    target: &AuiTemplateParameterTarget,
    value: &AuiTemplateParameterValue,
) -> Result<(), AuiTemplateDiagnostic> {
    match (target, value) {
        (AuiTemplateParameterTarget::Text, AuiTemplateParameterValue::String(value))
        | (AuiTemplateParameterTarget::InputFieldText, AuiTemplateParameterValue::String(value)) => {
            node.text = Some(value.clone())
        }
        (AuiTemplateParameterTarget::ProgressValue, AuiTemplateParameterValue::Number(value)) => {
            node.progress_value = Some(value.clamp(0.0, 1.0))
        }
        (AuiTemplateParameterTarget::Visible, AuiTemplateParameterValue::Boolean(value)) => {
            node.visible = *value
        }
        (AuiTemplateParameterTarget::ImageAssetId, AuiTemplateParameterValue::AssetRef(value)) => {
            node.image = Some(AuiAssetRef::new(value))
        }
        (
            AuiTemplateParameterTarget::BindingPath { binding_id },
            AuiTemplateParameterValue::String(value),
        ) => {
            let Some(binding) = node
                .binding_refs
                .iter_mut()
                .find(|binding| binding.binding_id == *binding_id)
            else {
                return Err(parameter_apply_diagnostic(node, target));
            };
            binding.path = value.clone();
        }
        (
            AuiTemplateParameterTarget::ActionId { event },
            AuiTemplateParameterValue::String(value),
        ) => {
            let Some(action) = node
                .action_refs
                .iter_mut()
                .find(|action| action.event == *event)
            else {
                return Err(parameter_apply_diagnostic(node, target));
            };
            action.action_id = value.clone();
        }
        _ => return Err(parameter_apply_diagnostic(node, target)),
    }
    Ok(())
}

fn parameter_apply_diagnostic(
    node: &AuiNode,
    target: &AuiTemplateParameterTarget,
) -> AuiTemplateDiagnostic {
    AuiTemplateDiagnostic::error(
        "aui_template.parameter_apply_failed",
        Some(node.node_id.clone()),
        format!(
            "Template parameter target {:?} could not be applied to node '{}'.",
            target, node.node_id
        ),
        "Validate the template parameter type and target field.",
    )
}

fn document_node_is_ancestor(document: &AuiDocument, ancestor: &str, descendant: &str) -> bool {
    let nodes = document
        .nodes
        .iter()
        .map(|node| (node.node_id.as_str(), node))
        .collect::<HashMap<_, _>>();
    let mut current = nodes
        .get(descendant)
        .and_then(|node| node.parent.as_deref());
    let mut visited = HashSet::new();
    while let Some(node_id) = current {
        if node_id == ancestor {
            return true;
        }
        if !visited.insert(node_id) {
            return false;
        }
        current = nodes.get(node_id).and_then(|node| node.parent.as_deref());
    }
    false
}

fn template_has_unparameterized_binding(asset: &AuiTemplateAsset) -> bool {
    asset.nodes.iter().any(|node| {
        node.binding_refs.iter().any(|binding| {
            !asset.parameter_bindings.iter().any(|parameter_binding| {
                parameter_binding.node_id == node.node_id
                    && matches!(
                        &parameter_binding.target,
                        AuiTemplateParameterTarget::BindingPath { binding_id }
                            if binding_id == &binding.binding_id
                    )
            })
        })
    })
}

fn template_has_unparameterized_action(asset: &AuiTemplateAsset) -> bool {
    asset.nodes.iter().any(|node| {
        node.action_refs.iter().any(|action| {
            !asset.parameter_bindings.iter().any(|parameter_binding| {
                parameter_binding.node_id == node.node_id
                    && matches!(
                        parameter_binding.target,
                        AuiTemplateParameterTarget::ActionId { event }
                            if event == action.event
                    )
            })
        })
    })
}

fn collect_subtree_nodes(
    document: &AuiDocument,
    root_node_id: &str,
) -> Result<Vec<AuiNode>, Vec<AuiTemplateDiagnostic>> {
    let nodes_by_id = document
        .nodes
        .iter()
        .map(|node| (node.node_id.clone(), node.clone()))
        .collect::<HashMap<_, _>>();
    if !nodes_by_id.contains_key(root_node_id) {
        return Err(vec![AuiTemplateDiagnostic::error(
            "aui_template.root_missing",
            Some(root_node_id.to_string()),
            format!("AUI node '{root_node_id}' does not exist."),
            "Select an existing AUI node as template root.",
        )]);
    }
    let mut ordered = Vec::new();
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    collect_node_recursive(
        root_node_id,
        &nodes_by_id,
        &mut visiting,
        &mut visited,
        &mut ordered,
    )?;
    Ok(ordered)
}

fn collect_node_recursive(
    node_id: &str,
    nodes_by_id: &HashMap<String, AuiNode>,
    visiting: &mut HashSet<String>,
    visited: &mut HashSet<String>,
    ordered: &mut Vec<AuiNode>,
) -> Result<(), Vec<AuiTemplateDiagnostic>> {
    if visited.contains(node_id) {
        return Ok(());
    }
    if !visiting.insert(node_id.to_string()) {
        return Err(vec![AuiTemplateDiagnostic::error(
            "aui_template.cyclic_node_tree",
            Some(node_id.to_string()),
            format!("AUI subtree contains a cycle at node '{node_id}'."),
            "Fix the AUI node children before saving the template.",
        )]);
    }
    let Some(node) = nodes_by_id.get(node_id) else {
        return Err(vec![AuiTemplateDiagnostic::error(
            "aui_template.child_missing",
            Some(node_id.to_string()),
            format!("AUI subtree references missing child node '{node_id}'."),
            "Fix the AUI node children list before saving the template.",
        )]);
    };
    ordered.push(node.clone());
    for child in &node.children {
        collect_node_recursive(child, nodes_by_id, visiting, visited, ordered)?;
    }
    visiting.remove(node_id);
    visited.insert(node_id.to_string());
    Ok(())
}

fn collect_asset_refs(nodes: &[AuiNode]) -> Vec<AuiTemplateDependencyRef> {
    nodes
        .iter()
        .filter_map(|node| {
            node.image.as_ref().map(|image| AuiTemplateDependencyRef {
                node_id: node.node_id.clone(),
                field_path: "image.assetId".to_string(),
                value: image.asset_id.clone(),
            })
        })
        .collect()
}

fn collect_binding_refs(nodes: &[AuiNode]) -> Vec<AuiTemplateDependencyRef> {
    nodes
        .iter()
        .flat_map(|node| {
            node.binding_refs
                .iter()
                .map(move |binding| AuiTemplateDependencyRef {
                    node_id: node.node_id.clone(),
                    field_path: format!(
                        "bindingRefs.{}.{}",
                        binding.binding_id,
                        binding_target_name(binding.target_field)
                    ),
                    value: binding.path.clone(),
                })
        })
        .collect()
}

fn collect_action_refs(nodes: &[AuiNode]) -> Vec<AuiTemplateDependencyRef> {
    nodes
        .iter()
        .flat_map(|node| {
            node.action_refs
                .iter()
                .map(move |action| AuiTemplateDependencyRef {
                    node_id: node.node_id.clone(),
                    field_path: format!("actionRefs.{}", action_event_name(action.event)),
                    value: action.action_id.clone(),
                })
        })
        .collect()
}

fn binding_target_name(target: AuiBindingTarget) -> &'static str {
    match target {
        AuiBindingTarget::RectOffsetX => "rect.offsetX",
        AuiBindingTarget::RectOffsetY => "rect.offsetY",
        AuiBindingTarget::TextColor => "text.color",
        AuiBindingTarget::TextText => "text.text",
        AuiBindingTarget::InputFieldText => "inputField.text",
        AuiBindingTarget::ProgressBarValue => "progress.value",
        AuiBindingTarget::ToggleChecked => "toggle.checked",
        AuiBindingTarget::SliderValue => "slider.value",
        AuiBindingTarget::PanelVisible => "panel.visible",
        AuiBindingTarget::ImageVisible => "image.visible",
        AuiBindingTarget::ImageAssetRef => "image.assetRef",
        AuiBindingTarget::Interactable => "interactable",
        AuiBindingTarget::Translation => "presentation.translation",
        AuiBindingTarget::Rotation => "presentation.rotation",
        AuiBindingTarget::Scale => "presentation.scale",
        AuiBindingTarget::Pivot => "presentation.pivot",
        AuiBindingTarget::Opacity => "presentation.opacity",
        AuiBindingTarget::Tint => "presentation.tint",
        AuiBindingTarget::Resource => "resource",
        AuiBindingTarget::LocalStyle => "localStyle",
    }
}

fn action_event_name(event: AuiActionEvent) -> &'static str {
    match event {
        AuiActionEvent::Activate => "activate",
        AuiActionEvent::Click => "click",
        AuiActionEvent::DragStart => "drag_start",
        AuiActionEvent::DragMove => "drag_move",
        AuiActionEvent::Drop => "drop",
        AuiActionEvent::Focus => "focus",
        AuiActionEvent::Blur => "blur",
        AuiActionEvent::Submit => "submit",
        AuiActionEvent::Cancel => "cancel",
        AuiActionEvent::Scroll => "scroll",
        AuiActionEvent::TextChanged => "text_changed",
        AuiActionEvent::TextSubmitted => "text_submitted",
        AuiActionEvent::TextCancelled => "text_cancelled",
        AuiActionEvent::ValueChanged => "value_changed",
    }
}

fn make_unique_node_id(
    prefix: &str,
    source_node_id: &str,
    used_ids: &mut HashSet<String>,
) -> String {
    let clean_prefix = prefix.trim().trim_end_matches(['_', '-']);
    let base = if clean_prefix.is_empty() {
        source_node_id.to_string()
    } else {
        format!("{clean_prefix}_{source_node_id}")
    };
    if used_ids.insert(base.clone()) {
        return base;
    }
    for index in 2usize.. {
        let candidate = format!("{base}_{index}");
        if used_ids.insert(candidate.clone()) {
            return candidate;
        }
    }
    unreachable!("usize iteration should always find a unique node id")
}

fn deterministic_guid(prefix: &str, asset_path: &str, template_id: &str) -> String {
    let input = format!("{prefix}:{asset_path}:{template_id}");
    format!("{prefix}-{:016x}", fnv1a64(input.as_bytes()))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod m4e_tests {
    use super::*;

    #[test]
    fn template_dependency_names_cover_m4e_control_bindings_and_actions() {
        assert_eq!(
            binding_target_name(AuiBindingTarget::ToggleChecked),
            "toggle.checked"
        );
        assert_eq!(
            binding_target_name(AuiBindingTarget::SliderValue),
            "slider.value"
        );
        assert_eq!(
            action_event_name(AuiActionEvent::ValueChanged),
            "value_changed"
        );
    }
}

#[allow(dead_code)]
fn _assert_node_kind_is_used(kind: AuiNodeKind) -> AuiNodeKind {
    kind
}
