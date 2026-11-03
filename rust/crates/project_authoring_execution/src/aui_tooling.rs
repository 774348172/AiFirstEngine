use authoring_project_context::ProjectSnapshot;
use engine_runtime::aui::{
    AuiAssetManifest, AuiAssetManifestEntry, AuiComputedRect, AuiDocument, AuiLayoutEngine,
    AuiNodeKind, AuiValidationSeverity,
};
use engine_runtime::aui_layout_schema::{LayoutMode, AUI_LAYOUT_SCHEMA_VERSION};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const AUI_PROJECT_INSPECTION_SCHEMA_VERSION: &str = "aui-project-inspection.v1";
pub const AUI_MIGRATION_PREVIEW_SCHEMA_VERSION: &str = "aui-migration-preview.v1";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuiInspectionMode {
    #[default]
    Summary,
    Trace,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuiInspectionRequest {
    #[serde(default)]
    pub mode: AuiInspectionMode,
    #[serde(default)]
    pub document_id: Option<String>,
    #[serde(default)]
    pub node_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiProjectToolDiagnostic {
    pub source_path: String,
    pub domain: &'static str,
    pub stage: &'static str,
    pub code: String,
    pub severity: &'static str,
    pub node_id: Option<String>,
    pub asset_id: Option<String>,
    pub message: String,
    pub next_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiMigrationPreview {
    pub schema_version: &'static str,
    pub source_schema_version: String,
    pub target_schema_version: String,
    pub rewrite_required: bool,
    pub document_id_preserved: bool,
    pub stable_node_ids_preserved: bool,
    pub binding_ids_preserved: bool,
    pub action_ids_preserved: bool,
    pub source_node_count: usize,
    pub target_node_count: usize,
    pub changes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiDocumentInspection {
    pub source_path: String,
    pub document_id: String,
    pub source_schema_version: String,
    pub cooked_schema_version: String,
    pub node_count: usize,
    pub canvas_count: usize,
    pub binding_count: usize,
    pub action_count: usize,
    pub capabilities: Vec<String>,
    pub migration: AuiMigrationPreview,
    pub diagnostics: Vec<AuiProjectToolDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiNodeInspectionTrace {
    pub source_path: String,
    pub document_id: String,
    pub node_id: String,
    pub kind: String,
    pub ancestor_path: Vec<String>,
    pub layout_mode: String,
    pub has_slot_constraints: bool,
    pub has_text_layout: bool,
    pub final_rect: Option<AuiComputedRect>,
    pub style_source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiProjectInspectionReport {
    pub schema_version: &'static str,
    pub status: &'static str,
    pub project_identity: String,
    pub project_revision: String,
    pub document_count: usize,
    pub node_count: usize,
    pub migration_candidate_count: usize,
    pub error_count: usize,
    pub warning_count: usize,
    pub documents: Vec<AuiDocumentInspection>,
    pub diagnostics: Vec<AuiProjectToolDiagnostic>,
    pub trace: Vec<AuiNodeInspectionTrace>,
}

impl AuiProjectInspectionReport {
    pub fn ok(&self) -> bool {
        self.error_count == 0
    }
}

pub fn inspect_aui_project_snapshot(snapshot: &ProjectSnapshot) -> AuiProjectInspectionReport {
    inspect_aui_project_snapshot_with_request(snapshot, &AuiInspectionRequest::default())
}

pub fn inspect_aui_project_snapshot_with_request(
    snapshot: &ProjectSnapshot,
    request: &AuiInspectionRequest,
) -> AuiProjectInspectionReport {
    let mut documents = Vec::new();
    let mut diagnostics = Vec::new();
    let mut document_owners = BTreeMap::<String, String>::new();

    let mut files = snapshot
        .files
        .iter()
        .filter(|file| {
            file.relative_path.starts_with("AUI/") && file.relative_path.ends_with(".json")
        })
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));

    for file in &files {
        match inspect_document(&file.relative_path, &file.bytes) {
            Ok(document) => {
                if let Some(first_path) =
                    document_owners.insert(document.document_id.clone(), file.relative_path.clone())
                {
                    diagnostics.push(AuiProjectToolDiagnostic {
                        source_path: file.relative_path.clone(),
                        domain: "aui",
                        stage: "inspect",
                        code: "aui_tooling.duplicate_document_id".to_string(),
                        severity: "error",
                        node_id: None,
                        asset_id: None,
                        message: format!(
                            "AUI document id '{}' is also owned by '{}'.",
                            document.document_id, first_path
                        ),
                        next_action: "Keep documentId unique across project AUI source documents."
                            .to_string(),
                    });
                }
                diagnostics.extend(document.diagnostics.iter().cloned());
                documents.push(document);
            }
            Err(diagnostic) => diagnostics.push(*diagnostic),
        }
    }

    let error_count = diagnostics
        .iter()
        .filter(|item| item.severity == "error")
        .count();
    let warning_count = diagnostics
        .iter()
        .filter(|item| item.severity == "warning")
        .count();
    let trace = if request.mode == AuiInspectionMode::Trace {
        build_trace(&files, request)
    } else {
        Vec::new()
    };
    AuiProjectInspectionReport {
        schema_version: AUI_PROJECT_INSPECTION_SCHEMA_VERSION,
        status: if error_count == 0 { "passed" } else { "failed" },
        project_identity: snapshot.portable_project_identity.clone(),
        project_revision: snapshot.revision.revision_id.clone(),
        document_count: documents.len(),
        node_count: documents.iter().map(|document| document.node_count).sum(),
        migration_candidate_count: documents
            .iter()
            .filter(|document| document.migration.rewrite_required)
            .count(),
        error_count,
        warning_count,
        documents,
        diagnostics,
        trace,
    }
}

fn build_trace(
    files: &[&authoring_project_context::SnapshotFile],
    request: &AuiInspectionRequest,
) -> Vec<AuiNodeInspectionTrace> {
    let mut trace = Vec::new();
    for file in files {
        let Ok(source) = serde_json::from_slice::<Value>(&file.bytes) else {
            continue;
        };
        let Ok((_, cooked_value)) =
            crate::neutral_assembler::cook_aui_document(&file.relative_path, source)
        else {
            continue;
        };
        let Ok(document) = serde_json::from_value::<AuiDocument>(cooked_value) else {
            continue;
        };
        if request
            .document_id
            .as_deref()
            .is_some_and(|id| id != document.document_id)
        {
            continue;
        }
        let computed = AuiLayoutEngine::layout(&document, 0)
            .computed_nodes
            .into_iter()
            .map(|node| (node.node_id, node.rect))
            .collect::<BTreeMap<_, _>>();
        let parents = document
            .nodes
            .iter()
            .map(|node| (node.node_id.as_str(), node.parent.as_deref()))
            .collect::<BTreeMap<_, _>>();
        for node in &document.nodes {
            if request
                .node_id
                .as_deref()
                .is_some_and(|id| id != node.node_id)
            {
                continue;
            }
            trace.push(AuiNodeInspectionTrace {
                source_path: file.relative_path.clone(),
                document_id: document.document_id.clone(),
                node_id: node.node_id.clone(),
                kind: format!("{:?}", node.kind),
                ancestor_path: ancestor_path(&node.node_id, &parents),
                layout_mode: format!(
                    "{:?}",
                    node.layout
                        .as_ref()
                        .map(|layout| layout.mode)
                        .unwrap_or(LayoutMode::Anchor)
                ),
                has_slot_constraints: node.slot.is_some(),
                has_text_layout: node.text_layout.is_some(),
                final_rect: computed.get(&node.node_id).copied(),
                style_source: if node.style_overrides.is_some() {
                    "inline_override"
                } else if node.style_ref.is_some() {
                    "style_ref"
                } else if node.theme.is_some() {
                    "node_theme"
                } else if document.theme.is_some() {
                    "document_theme"
                } else if node.style.is_some() {
                    "legacy_inline"
                } else {
                    "engine_default"
                }
                .to_string(),
            });
        }
    }
    trace.sort_by(|left, right| {
        (&left.source_path, &left.node_id).cmp(&(&right.source_path, &right.node_id))
    });
    trace
}

fn ancestor_path(node_id: &str, parents: &BTreeMap<&str, Option<&str>>) -> Vec<String> {
    let mut path = Vec::new();
    let mut current = Some(node_id);
    let mut remaining = parents.len().saturating_add(1);
    while let Some(node) = current {
        if remaining == 0 {
            break;
        }
        path.push(node.to_string());
        current = parents.get(node).copied().flatten();
        remaining -= 1;
    }
    path.reverse();
    path
}

fn inspect_document(
    source_path: &str,
    bytes: &[u8],
) -> Result<AuiDocumentInspection, Box<AuiProjectToolDiagnostic>> {
    let source: Value = serde_json::from_slice(bytes).map_err(|error| {
        Box::new(AuiProjectToolDiagnostic {
            source_path: source_path.to_string(),
            domain: "aui",
            stage: "parse",
            code: "aui_tooling.source_json_invalid".to_string(),
            severity: "error",
            node_id: None,
            asset_id: None,
            message: error.to_string(),
            next_action: "Repair the AUI JSON and run project check again.".to_string(),
        })
    })?;
    let source_schema_version = source_schema_version(&source);
    let source_identity = source_identity(&source);
    let (_, cooked_value) =
        crate::neutral_assembler::cook_aui_document(source_path, source.clone()).map_err(
            |error| {
                Box::new(AuiProjectToolDiagnostic {
                    source_path: source_path.to_string(),
                    domain: "aui",
                    stage: "cook",
                    code: error.code().to_string(),
                    severity: "error",
                    node_id: None,
                    asset_id: None,
                    message: error.message().to_string(),
                    next_action: error.next_action().to_string(),
                })
            },
        )?;
    let cooked: AuiDocument = serde_json::from_value(cooked_value).map_err(|error| {
        Box::new(AuiProjectToolDiagnostic {
            source_path: source_path.to_string(),
            domain: "aui",
            stage: "cook",
            code: "aui_tooling.cooked_document_invalid".to_string(),
            severity: "error",
            node_id: None,
            asset_id: None,
            message: error.to_string(),
            next_action: "Repair the first cooker diagnostic and check a fresh snapshot."
                .to_string(),
        })
    })?;

    let manifest = synthetic_asset_manifest(&cooked);
    let validation = AuiLayoutEngine::validate(&cooked, Some(&manifest));
    let document_diagnostics = validation
        .report_items
        .into_iter()
        .map(|item| AuiProjectToolDiagnostic {
            source_path: source_path.to_string(),
            domain: "aui",
            stage: "validate",
            code: format!("aui.{code}", code = item.code),
            severity: match item.severity {
                AuiValidationSeverity::Info => "info",
                AuiValidationSeverity::Warning => "warning",
                AuiValidationSeverity::Error => "error",
            },
            node_id: item.node_id,
            asset_id: item.asset_id,
            message: item.message,
            next_action: item.suggested_fix.unwrap_or_else(|| {
                "Repair the AUI source and run project check again.".to_string()
            }),
        })
        .collect::<Vec<_>>();

    let target_identity = cooked_identity(&cooked);
    let rewrite_required = source_schema_version != AUI_LAYOUT_SCHEMA_VERSION;
    let mut changes = Vec::new();
    if rewrite_required {
        changes.push(format!(
            "schema:{}->{}",
            source_schema_version, AUI_LAYOUT_SCHEMA_VERSION
        ));
    }
    if source.get("root").is_some() {
        changes.push("legacy_tree_flattened_to_nodes".to_string());
    }
    if cooked.nodes.iter().any(|node| node.layout.is_some()) {
        changes.push("typed_layout_preserved".to_string());
    }
    changes.sort();

    Ok(AuiDocumentInspection {
        source_path: source_path.to_string(),
        document_id: cooked.document_id.clone(),
        source_schema_version: source_schema_version.clone(),
        cooked_schema_version: cooked.schema_version.clone(),
        node_count: cooked.nodes.len(),
        canvas_count: cooked.canvases.len(),
        binding_count: cooked
            .nodes
            .iter()
            .map(|node| node.binding_refs.len())
            .sum(),
        action_count: cooked.nodes.iter().map(|node| node.action_refs.len()).sum(),
        capabilities: capabilities(&cooked),
        migration: AuiMigrationPreview {
            schema_version: AUI_MIGRATION_PREVIEW_SCHEMA_VERSION,
            source_schema_version,
            target_schema_version: AUI_LAYOUT_SCHEMA_VERSION.to_string(),
            rewrite_required,
            document_id_preserved: source_identity.document_id == cooked.document_id,
            stable_node_ids_preserved: source_identity.node_ids == target_identity.node_ids,
            binding_ids_preserved: source_identity.binding_ids == target_identity.binding_ids,
            action_ids_preserved: source_identity.action_ids == target_identity.action_ids,
            source_node_count: source_identity.node_ids.len(),
            target_node_count: target_identity.node_ids.len(),
            changes,
        },
        diagnostics: document_diagnostics,
    })
}

fn source_schema_version(source: &Value) -> String {
    source
        .get("schema_version")
        .or_else(|| source.get("schemaVersion"))
        .and_then(Value::as_str)
        .unwrap_or("aui-document.legacy")
        .to_string()
}

#[derive(Default)]
struct SourceIdentity {
    document_id: String,
    node_ids: BTreeSet<String>,
    binding_ids: BTreeSet<String>,
    action_ids: BTreeSet<String>,
}

fn source_identity(source: &Value) -> SourceIdentity {
    let mut identity = SourceIdentity {
        document_id: source
            .get("document_id")
            .or_else(|| source.get("documentId"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        ..SourceIdentity::default()
    };
    if let Some(nodes) = source.get("nodes").and_then(Value::as_array) {
        for node in nodes {
            collect_node_identity(node, &mut identity);
        }
    } else if let Some(root) = source.get("root") {
        collect_legacy_tree_identity(root, &mut identity);
    }
    identity
}

fn cooked_identity(document: &AuiDocument) -> SourceIdentity {
    SourceIdentity {
        document_id: document.document_id.clone(),
        node_ids: document
            .nodes
            .iter()
            .map(|node| node.node_id.clone())
            .collect(),
        binding_ids: document
            .nodes
            .iter()
            .flat_map(|node| {
                node.binding_refs
                    .iter()
                    .map(|binding| binding.binding_id.clone())
            })
            .collect(),
        action_ids: document
            .nodes
            .iter()
            .flat_map(|node| {
                node.action_refs
                    .iter()
                    .map(|action| action.action_id.clone())
            })
            .collect(),
    }
}

fn collect_node_identity(node: &Value, identity: &mut SourceIdentity) {
    if let Some(node_id) = node
        .get("node_id")
        .or_else(|| node.get("nodeId"))
        .and_then(Value::as_str)
    {
        identity.node_ids.insert(node_id.to_string());
    }
    if let Some(bindings) = node
        .get("binding_refs")
        .or_else(|| node.get("bindings"))
        .and_then(Value::as_array)
    {
        for binding in bindings {
            if let Some(binding_id) = binding
                .get("binding_id")
                .or_else(|| binding.get("bindingId"))
                .and_then(Value::as_str)
            {
                identity.binding_ids.insert(binding_id.to_string());
            }
        }
    }
    if let Some(actions) = node
        .get("action_refs")
        .or_else(|| node.get("actions"))
        .and_then(Value::as_array)
    {
        for action in actions {
            if let Some(action_id) = action
                .get("action_id")
                .or_else(|| action.get("actionId"))
                .and_then(Value::as_str)
            {
                identity.action_ids.insert(action_id.to_string());
            }
        }
    }
}

fn collect_legacy_tree_identity(node: &Value, identity: &mut SourceIdentity) {
    collect_node_identity(node, identity);
    if let Some(children) = node.get("children").and_then(Value::as_array) {
        for child in children {
            collect_legacy_tree_identity(child, identity);
        }
    }
}

fn synthetic_asset_manifest(document: &AuiDocument) -> AuiAssetManifest {
    let assets = document
        .nodes
        .iter()
        .filter_map(|node| {
            node.image
                .as_ref()
                .map(|image| (&image.asset_id, &node.node_id))
        })
        .fold(
            BTreeMap::<String, Vec<String>>::new(),
            |mut entries, (asset_id, node_id)| {
                entries
                    .entry(asset_id.clone())
                    .or_default()
                    .push(node_id.clone());
                entries
            },
        )
        .into_iter()
        .map(|(asset_id, nodes)| AuiAssetManifestEntry::image(asset_id.clone(), asset_id, nodes))
        .collect();
    AuiAssetManifest::new(format!("{}.inspection", document.document_id), assets)
}

fn capabilities(document: &AuiDocument) -> Vec<String> {
    let mut values = BTreeSet::new();
    values.insert("anchor_layout".to_string());
    for node in &document.nodes {
        if let Some(layout) = &node.layout {
            values.insert(format!("layout.{:?}", layout.mode).to_ascii_lowercase());
        }
        if node.text_layout.is_some() {
            values.insert("paragraph_text".to_string());
        }
        if !node.text_spans.is_empty() {
            values.insert("rich_text".to_string());
        }
        if node.style_ref.is_some() || node.theme.is_some() || document.theme.is_some() {
            values.insert("theme".to_string());
        }
        if node.viewport_condition.is_some() {
            values.insert("viewport_condition".to_string());
        }
        if !node.binding_refs.is_empty() {
            values.insert("binding".to_string());
        }
        if !node.action_refs.is_empty() {
            values.insert("action".to_string());
        }
        match node.kind {
            AuiNodeKind::Toggle => {
                values.insert("toggle".to_string());
            }
            AuiNodeKind::Slider => {
                values.insert("slider".to_string());
            }
            AuiNodeKind::InputField => {
                values.insert("text_entry".to_string());
            }
            AuiNodeKind::List => {
                values.insert("list".to_string());
            }
            AuiNodeKind::ScrollView => {
                values.insert("scroll".to_string());
            }
            _ => {}
        }
    }
    values.into_iter().collect()
}
