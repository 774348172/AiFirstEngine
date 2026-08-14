use engine_runtime::aui::{AuiActionEvent, AuiBindingTarget, AuiDocument, AuiNode, AuiNodeKind};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

pub const AUI_TEMPLATE_ASSET_SCHEMA_VERSION: &str = "aui-template-asset.v1";
pub const AUI_TEMPLATE_INSTANTIATE_REPORT_SCHEMA_VERSION: &str =
    "aui-template-instantiate-report.v1";

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
                    "C-min template asset; instantiate-by-expansion only.".to_string(),
                    "guid_source=deterministic_path_hash; asset_db_integrated=false.".to_string(),
                ],
            },
            nodes: normalized_nodes,
            asset_refs,
            binding_refs,
            action_refs,
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
        if self.schema_version != AUI_TEMPLATE_ASSET_SCHEMA_VERSION {
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
        diagnostics
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiTemplateRef {
    pub asset_guid: String,
    pub template_id: String,
    pub asset_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiTemplateInstantiateRequest {
    pub template_ref: AuiTemplateRef,
    pub target_document_path: String,
    pub parent_node_id: String,
    pub insertion_index: Option<usize>,
    pub instance_id: String,
    pub node_id_prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiTemplateNodeIdRemap {
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
        for node in &asset.nodes {
            let inserted = make_unique_node_id(prefix, &node.node_id, &mut used_ids);
            remap.insert(node.node_id.clone(), inserted.clone());
            report.node_id_remap.push(AuiTemplateNodeIdRemap {
                source_node_id: node.node_id.clone(),
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
        if let Some(parent) = document
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
        document.nodes.extend(inserted_nodes);

        report.inserted_node_count = asset.nodes.len();
        report.copied_asset_refs = remap_dependencies(&asset.asset_refs, &remap);
        report.copied_binding_refs = remap_dependencies(&asset.binding_refs, &remap);
        report.copied_action_refs = remap_dependencies(&asset.action_refs, &remap);
        if !report.copied_binding_refs.is_empty() {
            report.diagnostics.push(AuiTemplateDiagnostic::warning(
                "aui_template.binding_refs_unparameterized",
                None,
                "AUI template binding refs were copied without parameterization.",
                "Inspect copied_binding_refs and rewrite paths for each instance if needed.",
            ));
        }
        if !report.copied_action_refs.is_empty() {
            report.diagnostics.push(AuiTemplateDiagnostic::warning(
                "aui_template.action_refs_unparameterized",
                None,
                "AUI template action refs were copied without parameterization.",
                "Inspect copied_action_refs and rewrite action ids for each instance if needed.",
            ));
        }
        report.recompute_status();
        report
    }
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

fn remap_dependencies(
    refs: &[AuiTemplateDependencyRef],
    remap: &HashMap<String, String>,
) -> Vec<AuiTemplateDependencyRef> {
    refs.iter()
        .map(|dependency| AuiTemplateDependencyRef {
            node_id: remap
                .get(&dependency.node_id)
                .cloned()
                .unwrap_or_else(|| dependency.node_id.clone()),
            field_path: dependency.field_path.clone(),
            value: dependency.value.clone(),
        })
        .collect()
}

fn binding_target_name(target: AuiBindingTarget) -> &'static str {
    match target {
        AuiBindingTarget::TextText => "text.text",
        AuiBindingTarget::InputFieldText => "inputField.text",
        AuiBindingTarget::ProgressBarValue => "progress.value",
        AuiBindingTarget::PanelVisible => "panel.visible",
        AuiBindingTarget::ImageVisible => "image.visible",
        AuiBindingTarget::ImageAssetRef => "image.assetRef",
    }
}

fn action_event_name(event: AuiActionEvent) -> &'static str {
    match event {
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

#[allow(dead_code)]
fn _assert_node_kind_is_used(kind: AuiNodeKind) -> AuiNodeKind {
    kind
}
