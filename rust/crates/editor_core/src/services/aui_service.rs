use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use editor_ui_model::{DiagnosticSeverity, WorkspaceSelectionTarget};
use engine_runtime::aui::{
    AuiActionEvent, AuiAssetRef, AuiBindingRef, AuiBindingTarget, AuiBindingValue, AuiNode,
    AuiNodeKind, AuiRect, AuiRuntimePresentReport, AuiRuntimePresentStatus, AuiRuntimePresenter,
    AuiStyle,
};
use serde::{Deserialize, Serialize};

use crate::{
    services::project_service::normalize_project_relative_path, AuiAuthoringDiagnostic,
    AuiAuthoringReport, AuiAuthoringService, AuiDocumentCookRequest, AuiDocumentCooker,
    AuiNodeFieldValue, AuiTemplateAsset, AuiTemplateDiagnostic, AuiTemplateDiagnosticSeverity,
    AuiTemplateInstantiateReport, AuiTemplateInstantiateRequest, AuiTemplateOperationStatus,
    AuiTemplateRef, AuiTemplateWorkflow, AuiTransactionStatus, CommandResult, CommandStatus,
    CommandTransaction, EditorSession, StateChangeSummary, UndoPolicy,
};

pub const AUI_DOCUMENT_AUTHORING_PRODUCTIZATION_REPORT_SCHEMA_VERSION: &str =
    "aui-document-authoring-productization-report.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuiDocumentAuthoringProductizationStatus {
    Passed,
    Partial,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiDocumentAuthoringProductizationReport {
    pub schema_version: String,
    pub status: AuiDocumentAuthoringProductizationStatus,
    pub source_path: Option<String>,
    pub document_id: Option<String>,
    pub node_count: usize,
    pub binding_count: usize,
    pub action_count: usize,
    pub validation_ok: bool,
    pub preview: Option<AuiRuntimePresentReport>,
    pub diagnostics: Vec<AuiAuthoringDiagnostic>,
    pub next_actions: Vec<String>,
}

impl AuiDocumentAuthoringProductizationReport {
    fn from_service(
        path: Option<String>,
        service: &AuiAuthoringService,
        preview: Option<AuiRuntimePresentReport>,
    ) -> Self {
        let authoring = service.report(None);
        Self::from_authoring_report(path, authoring, preview)
    }

    fn from_authoring_report(
        path: Option<String>,
        authoring: AuiAuthoringReport,
        preview: Option<AuiRuntimePresentReport>,
    ) -> Self {
        let preview_failed = preview
            .as_ref()
            .is_some_and(|report| report.status == AuiRuntimePresentStatus::Failed);
        let preview_partial = preview
            .as_ref()
            .is_some_and(|report| report.status == AuiRuntimePresentStatus::Partial);
        let mut next_actions = Vec::new();
        if !authoring.validation.ok {
            next_actions.push("fix_aui_document_validation".to_string());
        }
        if preview_partial {
            next_actions.push("runtime_text_glyph_present".to_string());
        }
        if preview_failed {
            next_actions.push("fix_aui_preview_present".to_string());
        }
        let status = if !authoring.validation.ok || preview_failed {
            AuiDocumentAuthoringProductizationStatus::Failed
        } else if preview_partial {
            AuiDocumentAuthoringProductizationStatus::Partial
        } else {
            AuiDocumentAuthoringProductizationStatus::Passed
        };

        Self {
            schema_version: AUI_DOCUMENT_AUTHORING_PRODUCTIZATION_REPORT_SCHEMA_VERSION.to_string(),
            status,
            source_path: path,
            document_id: Some(authoring.document_id),
            node_count: authoring.node_count,
            binding_count: authoring.binding_count,
            action_count: authoring.action_count,
            validation_ok: authoring.validation.ok,
            preview,
            diagnostics: authoring.diagnostics,
            next_actions,
        }
    }
}

impl EditorSession {
    pub(crate) fn select_aui_node(
        &mut self,
        transaction: &mut CommandTransaction,
        document_path: String,
        document_id: String,
        node_id: String,
    ) -> CommandResult {
        if document_path.trim().is_empty()
            || document_id.trim().is_empty()
            || node_id.trim().is_empty()
        {
            self.push_error(
                transaction,
                "editor.aui_scene_authoring.context_required",
                "SelectAuiNode requires document_path, document_id, and node_id.",
                Some("Select a concrete AUI node from Scene View or Hierarchy."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        }

        transaction
            .read_set
            .push(format!("aui_document.{document_path}"));
        transaction
            .write_set
            .push("workspace.selection.aui_node".to_string());
        transaction.undo_policy = UndoPolicy::None;

        let before = self
            .selected_aui_node
            .as_ref()
            .map(|selection| format!("{selection:?}"));
        self.scene_selection.clear();
        self.selected_entity_id = None;
        self.selected_entity_source = None;
        self.selected_project_browser_path = Some(document_path.clone());
        self.selected_aui_node = Some(WorkspaceSelectionTarget::AuiNode {
            document_path: document_path.clone(),
            document_id: document_id.clone(),
            node_id: node_id.clone(),
        });
        transaction.state_changes.push(StateChangeSummary {
            kind: "selection.aui_node.changed".to_string(),
            path: "workspace.selection.aui_node".to_string(),
            before_summary: before,
            after_summary: Some(format!("{document_path}:{document_id}:{node_id}")),
        });
        self.push_info(
            transaction,
            "editor.aui_scene_authoring.selected",
            format!("Selected AUI node {node_id} in {document_path}."),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn create_aui_document(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
        document_id: String,
        width: f32,
        height: f32,
    ) -> CommandResult {
        let Some(session) = &self.active_project_session else {
            self.push_error(
                transaction,
                "editor.aui_authoring.no_project",
                "Cannot create an AUI document before opening a project.",
                Some("Open or create a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        if path.trim().is_empty() || document_id.trim().is_empty() || width <= 0.0 || height <= 0.0
        {
            self.push_error(
                transaction,
                "editor.aui_authoring.context_required",
                "CreateAuiDocument requires path, document_id, width, and height.",
                Some("Provide a project-relative AUI/*.aui.json path and a document id."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        }

        let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
            .with_style(AuiStyle::color("#101820cc"));
        let service = AuiAuthoringService::create_document(document_id, width, height, root);
        transaction.write_set.push(format!("aui_document.{path}"));
        match service.save_in_scope(session.write_scope(), &path) {
            Ok(()) => {
                self.selected_project_browser_path = Some(path.clone());
                transaction.state_changes.push(StateChangeSummary {
                    kind: "aui_document.created".to_string(),
                    path: format!("aui_document.{path}"),
                    before_summary: None,
                    after_summary: Some(service.document().document_id.clone()),
                });
                push_aui_productization_report(
                    self,
                    transaction,
                    &AuiDocumentAuthoringProductizationReport::from_service(
                        Some(path.clone()),
                        &service,
                        None,
                    ),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Committed)
            }
            Err(error) => {
                self.push_error(
                    transaction,
                    "editor.aui_authoring.save_failed",
                    format!("Failed to save AUI document {path}: {error}"),
                    Some("Check that the AUI folder is writable."),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Failed)
            }
        }
    }

    pub(crate) fn open_aui_document(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
    ) -> CommandResult {
        let Some(session) = &self.active_project_session else {
            self.push_error(
                transaction,
                "editor.aui_authoring.no_project",
                "Cannot open an AUI document before opening a project.",
                Some("Open or create a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        transaction.read_set.push(format!("aui_document.{path}"));
        match load_aui_authoring_service(&session.project_root, &path) {
            Ok(service) => {
                self.selected_project_browser_path = Some(path.clone());
                transaction.state_changes.push(StateChangeSummary {
                    kind: "aui_document.opened".to_string(),
                    path: "workspace.selected_asset".to_string(),
                    before_summary: None,
                    after_summary: Some(path.clone()),
                });
                push_aui_productization_report(
                    self,
                    transaction,
                    &AuiDocumentAuthoringProductizationReport::from_service(
                        Some(path),
                        &service,
                        None,
                    ),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Committed)
            }
            Err(message) => {
                self.push_error(
                    transaction,
                    "editor.aui_authoring.open_failed",
                    message,
                    Some("Select a valid AUI document."),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Failed)
            }
        }
    }

    pub(crate) fn add_aui_node(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
        parent_node_id: String,
        node_id: String,
        kind: String,
        name: String,
        rect: serde_json::Value,
    ) -> CommandResult {
        self.edit_aui_document(transaction, path, |service| {
            let node_kind = decode_node_kind(&kind)?;
            let rect = decode_rect(rect)?;
            let mut node = AuiNode::new(node_id, node_kind, rect);
            if !name.trim().is_empty() {
                node.name = name;
            }
            let tx = service.add_node(parent_node_id, node);
            transaction_from_aui(tx)
        })
    }

    pub(crate) fn set_aui_node_field(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
        node_id: String,
        schema_path: String,
        value: serde_json::Value,
    ) -> CommandResult {
        self.edit_aui_document(transaction, path, |service| {
            let value = decode_node_field_value(&schema_path, value)?;
            let tx = service.set_node_field(node_id, schema_path, value);
            transaction_from_aui(tx)
        })
    }

    pub(crate) fn set_aui_binding_path(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
        node_id: String,
        target_field: String,
        binding_id: String,
        binding_path: String,
        fallback: Option<serde_json::Value>,
    ) -> CommandResult {
        self.edit_aui_document(transaction, path, |service| {
            let binding = AuiBindingRef::new(
                binding_id,
                decode_binding_target(&target_field)?,
                binding_path,
                fallback.map(decode_binding_value).transpose()?,
            );
            let tx = service.set_binding_path(node_id, binding);
            transaction_from_aui(tx)
        })
    }

    pub(crate) fn set_aui_action_ref(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
        node_id: String,
        event: String,
        action_id: String,
        payload: Option<serde_json::Value>,
    ) -> CommandResult {
        if payload.is_some() {
            self.push_info(
                transaction,
                "editor.aui_authoring.action_payload_deferred",
                "AUI action payload was provided but runtime AuiActionRef v1 stores event/action_id only.",
            );
        }
        self.edit_aui_document(transaction, path, |service| {
            let tx = service.set_action_ref(node_id, decode_action_event(&event)?, action_id);
            transaction_from_aui(tx)
        })
    }

    pub(crate) fn validate_aui_document(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
    ) -> CommandResult {
        let Some(session) = &self.active_project_session else {
            self.push_error(
                transaction,
                "editor.aui_authoring.no_project",
                "Cannot validate an AUI document before opening a project.",
                Some("Open or create a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        transaction.read_set.push(format!("aui_document.{path}"));
        match load_aui_authoring_service(&session.project_root, &path) {
            Ok(service) => {
                let report = AuiDocumentAuthoringProductizationReport::from_service(
                    Some(path.clone()),
                    &service,
                    None,
                );
                push_aui_productization_report(self, transaction, &report);
                self.selected_project_browser_path = Some(path);
                let status = if report.validation_ok {
                    CommandStatus::Committed
                } else {
                    CommandStatus::Failed
                };
                self.finish_transaction(transaction.clone(), status)
            }
            Err(message) => {
                self.push_error(
                    transaction,
                    "editor.aui_authoring.validate_failed",
                    message,
                    Some("Fix the AUI document and validate again."),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Failed)
            }
        }
    }

    pub(crate) fn save_aui_document(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
    ) -> CommandResult {
        let Some(session) = &self.active_project_session else {
            self.push_error(
                transaction,
                "editor.aui_authoring.no_project",
                "Cannot save an AUI document before opening a project.",
                Some("Open or create a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        transaction.read_set.push(format!("aui_document.{path}"));
        transaction.write_set.push(format!("aui_document.{path}"));
        let service = match load_aui_authoring_service(&session.project_root, &path) {
            Ok(service) => service,
            Err(message) => {
                self.push_error(
                    transaction,
                    "editor.aui_authoring.load_failed",
                    message,
                    Some("Create or select a valid AUI document."),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
            }
        };
        match service.save_in_scope(session.write_scope(), &path) {
            Ok(()) => {
                self.selected_project_browser_path = Some(path.clone());
                self.push_info(
                    transaction,
                    "editor.aui_authoring.saved",
                    format!("Saved canonical AUI document {path}."),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Committed)
            }
            Err(error) => {
                self.push_error(
                    transaction,
                    "editor.aui_authoring.save_failed",
                    format!("Failed to save AUI document {path}: {error}"),
                    Some("Check that the AUI folder is writable."),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Failed)
            }
        }
    }

    pub(crate) fn preview_aui_overlay(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
    ) -> CommandResult {
        let Some(session) = &self.active_project_session else {
            self.push_error(
                transaction,
                "editor.aui_authoring.no_project",
                "Cannot preview an AUI document before opening a project.",
                Some("Open or create a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        transaction.read_set.push(format!("aui_document.{path}"));
        transaction.write_set.push("aui_preview.report".to_string());
        match load_aui_authoring_service(&session.project_root, &path) {
            Ok(service) => {
                let present = AuiRuntimePresenter::present_package_smoke(service.document(), 1);
                let report = AuiDocumentAuthoringProductizationReport::from_service(
                    Some(path.clone()),
                    &service,
                    Some(present.report),
                );
                push_aui_productization_report(self, transaction, &report);
                self.selected_project_browser_path = Some(path);
                let status = if report.status == AuiDocumentAuthoringProductizationStatus::Failed {
                    CommandStatus::Failed
                } else {
                    CommandStatus::Committed
                };
                self.finish_transaction(transaction.clone(), status)
            }
            Err(message) => {
                self.push_error(
                    transaction,
                    "editor.aui_authoring.preview_failed",
                    message,
                    Some("Fix the AUI document before previewing it."),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Failed)
            }
        }
    }

    pub(crate) fn save_aui_subtree_as_template(
        &mut self,
        transaction: &mut CommandTransaction,
        document_path: String,
        root_node_id: String,
        template_asset_path: String,
        template_id: String,
        display_name: String,
    ) -> CommandResult {
        let Some(session) = &self.active_project_session else {
            self.push_error(
                transaction,
                "editor.aui_template.no_project",
                "Cannot save an AUI template before opening a project.",
                Some("Open or create a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        if document_path.trim().is_empty()
            || root_node_id.trim().is_empty()
            || template_asset_path.trim().is_empty()
            || template_id.trim().is_empty()
        {
            self.push_error(
                transaction,
                "editor.aui_template.context_required",
                "SaveAuiSubtreeAsTemplate requires document_path, root_node_id, template_asset_path, and template_id.",
                Some("Select an AUI node and provide a template asset path."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        }
        transaction
            .read_set
            .push(format!("aui_document.{document_path}"));
        transaction
            .write_set
            .push(format!("aui_template.{template_asset_path}"));
        let service = match load_aui_authoring_service(&session.project_root, &document_path) {
            Ok(service) => service,
            Err(message) => {
                self.push_error(
                    transaction,
                    "editor.aui_template.document_load_failed",
                    message,
                    Some("Fix the source AUI document before saving a template."),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
            }
        };
        let full_template_path = session
            .project_root
            .join(normalize_project_relative_path(&template_asset_path));
        let asset = match AuiTemplateAsset::from_document_subtree(
            service.document(),
            document_path.clone(),
            &full_template_path,
            root_node_id.clone(),
            template_id.clone(),
            display_name,
            current_unix_ms(),
        ) {
            Ok(asset) => asset,
            Err(diagnostics) => {
                push_aui_template_diagnostics(self, transaction, &diagnostics);
                return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
            }
        };
        if let Err(error) = asset.save_in_scope(session.write_scope(), &template_asset_path) {
            self.push_error(
                transaction,
                "editor.aui_template.save_failed",
                format!("Failed to save AUI template {template_asset_path}: {error}"),
                Some("Check that the template folder is writable."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        }
        transaction.state_changes.push(StateChangeSummary {
            kind: "aui_template.saved".to_string(),
            path: format!("aui_template.{template_asset_path}"),
            before_summary: None,
            after_summary: Some(format!(
                "template_id={} nodes={} bindings={} actions={}",
                asset.template_id,
                asset.nodes.len(),
                asset.binding_refs.len(),
                asset.action_refs.len()
            )),
        });
        self.push_info(
            transaction,
            "editor.aui_template.saved",
            format!(
                "Saved AUI template {} from {}:{} nodes={} guid_source={}.",
                asset.template_id,
                document_path,
                root_node_id,
                asset.nodes.len(),
                asset.guid_source
            ),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn instantiate_aui_template(
        &mut self,
        transaction: &mut CommandTransaction,
        template_asset_path: String,
        template_id: String,
        target_document_path: String,
        parent_node_id: String,
        insertion_index: Option<usize>,
        instance_id: String,
        node_id_prefix: String,
    ) -> CommandResult {
        let Some(session) = &self.active_project_session else {
            self.push_error(
                transaction,
                "editor.aui_template.no_project",
                "Cannot instantiate an AUI template before opening a project.",
                Some("Open or create a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        if template_asset_path.trim().is_empty()
            || template_id.trim().is_empty()
            || target_document_path.trim().is_empty()
            || parent_node_id.trim().is_empty()
            || instance_id.trim().is_empty()
        {
            self.push_error(
                transaction,
                "editor.aui_template.context_required",
                "InstantiateAuiTemplate requires template_asset_path, template_id, target_document_path, parent_node_id, and instance_id.",
                Some("Choose a template asset and a target AUI parent node."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        }
        transaction
            .read_set
            .push(format!("aui_template.{template_asset_path}"));
        transaction
            .read_set
            .push(format!("aui_document.{target_document_path}"));
        transaction
            .write_set
            .push(format!("aui_document.{target_document_path}"));
        let full_template_path = session
            .project_root
            .join(normalize_project_relative_path(&template_asset_path));
        let asset = match AuiTemplateAsset::open(&full_template_path) {
            Ok(asset) => asset,
            Err(error) => {
                self.push_error(
                    transaction,
                    "editor.aui_template.open_failed",
                    format!("Failed to open AUI template {template_asset_path}: {error}"),
                    Some("Select a valid AUI template asset."),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
            }
        };
        let service = match load_aui_authoring_service(&session.project_root, &target_document_path)
        {
            Ok(service) => service,
            Err(message) => {
                self.push_error(
                    transaction,
                    "editor.aui_template.target_load_failed",
                    message,
                    Some("Fix the target AUI document before instantiating a template."),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
            }
        };
        let before = aui_document_summary(&service);
        let mut document = service.into_document();
        let request = AuiTemplateInstantiateRequest {
            template_ref: AuiTemplateRef {
                asset_guid: asset.asset_guid.clone(),
                template_id,
                asset_path: template_asset_path.clone(),
            },
            target_document_path: target_document_path.clone(),
            parent_node_id: parent_node_id.clone(),
            insertion_index,
            instance_id,
            node_id_prefix,
        };
        let report =
            AuiTemplateWorkflow::instantiate_into_document(&asset, &request, &mut document);
        push_aui_template_instantiate_report(self, transaction, &report);
        if report.status == AuiTemplateOperationStatus::Failed {
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        }
        let service = AuiAuthoringService::from_document(document);
        if let Err(error) = service.save_in_scope(session.write_scope(), &target_document_path) {
            self.push_error(
                transaction,
                "editor.aui_template.target_save_failed",
                format!("Failed to save target AUI document {target_document_path}: {error}"),
                Some("Check that the AUI document is writable."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        }
        transaction.state_changes.push(StateChangeSummary {
            kind: "aui_template.instantiated".to_string(),
            path: format!("aui_document.{target_document_path}"),
            before_summary: Some(before),
            after_summary: Some(aui_document_summary(&service)),
        });
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn validate_aui_template(
        &mut self,
        transaction: &mut CommandTransaction,
        template_asset_path: String,
        template_id: String,
    ) -> CommandResult {
        let Some(session) = &self.active_project_session else {
            self.push_error(
                transaction,
                "editor.aui_template.no_project",
                "Cannot validate an AUI template before opening a project.",
                Some("Open or create a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        if template_asset_path.trim().is_empty() || template_id.trim().is_empty() {
            self.push_error(
                transaction,
                "editor.aui_template.context_required",
                "ValidateAuiTemplate requires template_asset_path and template_id.",
                Some("Select a valid AUI template asset."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        }
        transaction
            .read_set
            .push(format!("aui_template.{template_asset_path}"));
        let full_template_path = session
            .project_root
            .join(normalize_project_relative_path(&template_asset_path));
        let asset = match AuiTemplateAsset::open(&full_template_path) {
            Ok(asset) => asset,
            Err(error) => {
                self.push_error(
                    transaction,
                    "editor.aui_template.open_failed",
                    format!("Failed to open AUI template {template_asset_path}: {error}"),
                    Some("Select a valid AUI template asset."),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
            }
        };
        let mut diagnostics = asset.validate();
        if asset.template_id != template_id {
            diagnostics.push(AuiTemplateDiagnostic::error(
                "aui_template.template_id_mismatch",
                None,
                format!(
                    "Requested template_id '{}' does not match asset template_id '{}'.",
                    template_id, asset.template_id
                ),
                "Use the template_id stored in the asset.",
            ));
        }
        push_aui_template_diagnostics(self, transaction, &diagnostics);
        self.push_info(
            transaction,
            "editor.aui_template.validated",
            format!(
                "Validated AUI template {} nodes={} bindings={} actions={} guid_source={}.",
                asset.template_id,
                asset.nodes.len(),
                asset.binding_refs.len(),
                asset.action_refs.len(),
                asset.guid_source
            ),
        );
        let status = if diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == AuiTemplateDiagnosticSeverity::Error)
        {
            CommandStatus::Failed
        } else {
            CommandStatus::Committed
        };
        self.finish_transaction(transaction.clone(), status)
    }

    fn edit_aui_document(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
        edit: impl FnOnce(&mut AuiAuthoringService) -> Result<(), String>,
    ) -> CommandResult {
        let Some(session) = &self.active_project_session else {
            self.push_error(
                transaction,
                "editor.aui_authoring.no_project",
                "Cannot edit an AUI document before opening a project.",
                Some("Open or create a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        transaction.read_set.push(format!("aui_document.{path}"));
        transaction.write_set.push(format!("aui_document.{path}"));
        let mut service = match load_aui_authoring_service(&session.project_root, &path) {
            Ok(service) => service,
            Err(message) => {
                self.push_error(
                    transaction,
                    "editor.aui_authoring.load_failed",
                    message,
                    Some("Create or select a valid AUI document."),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
            }
        };
        let before = aui_document_summary(&service);
        if let Err(message) = edit(&mut service) {
            self.push_error(
                transaction,
                "editor.aui_authoring.edit_failed",
                message,
                Some("Send a valid structured AUI authoring command."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        }
        if let Err(error) = service.save_in_scope(session.write_scope(), &path) {
            self.push_error(
                transaction,
                "editor.aui_authoring.save_failed",
                format!("Failed to save AUI document {path}: {error}"),
                Some("Check that the AUI folder is writable."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        }
        let after = aui_document_summary(&service);
        self.selected_project_browser_path = Some(path.clone());
        transaction.state_changes.push(StateChangeSummary {
            kind: "aui_document.edited".to_string(),
            path: format!("aui_document.{path}"),
            before_summary: Some(before),
            after_summary: Some(after),
        });
        push_aui_productization_report(
            self,
            transaction,
            &AuiDocumentAuthoringProductizationReport::from_service(Some(path), &service, None),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }
}

fn load_aui_authoring_service(
    project_root: &std::path::Path,
    relative_path: &str,
) -> Result<AuiAuthoringService, String> {
    if relative_path.trim().is_empty() {
        return Err("AUI document path is required.".to_string());
    }
    let path = project_root.join(normalize_project_relative_path(relative_path));
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("Failed to read AUI document {relative_path}: {error}"))?;
    match serde_json::from_str::<engine_runtime::aui::AuiDocument>(&text) {
        Ok(document) => Ok(AuiAuthoringService::from_document(document)),
        Err(_) => {
            let value = serde_json::from_str::<serde_json::Value>(&text)
                .map_err(|error| format!("Failed to parse AUI document JSON: {error}"))?;
            AuiDocumentCooker::cook(AuiDocumentCookRequest {
                source_path: path,
                document: value,
            })
            .map(|output| AuiAuthoringService::from_document(output.document))
            .map_err(|report| {
                report
                    .diagnostics
                    .first()
                    .map(|diagnostic| diagnostic.message.clone())
                    .unwrap_or_else(|| "Failed to normalize AUI document.".to_string())
            })
        }
    }
}

fn decode_node_kind(kind: &str) -> Result<AuiNodeKind, String> {
    match normalize_kind(kind).as_str() {
        "panel" | "canvas" => Ok(AuiNodeKind::Panel),
        "image" => Ok(AuiNodeKind::Image),
        "text" => Ok(AuiNodeKind::Text),
        "button" => Ok(AuiNodeKind::Button),
        "progressbar" | "progress_bar" | "progress-bar" => Ok(AuiNodeKind::ProgressBar),
        "toggle" => Ok(AuiNodeKind::Toggle),
        "slider" => Ok(AuiNodeKind::Slider),
        "list" => Ok(AuiNodeKind::List),
        "scrollview" | "scroll_view" | "scroll-view" => Ok(AuiNodeKind::ScrollView),
        "inputfield" | "input_field" | "input-field" => Ok(AuiNodeKind::InputField),
        "custom" => Ok(AuiNodeKind::Custom),
        other => Err(format!("Unsupported AUI node kind '{other}'.")),
    }
}

fn normalize_kind(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(' ', "")
}

fn decode_rect(value: serde_json::Value) -> Result<AuiRect, String> {
    if value.is_null() {
        return Err("AUI rect is required.".to_string());
    }
    if let (Some(x), Some(y), Some(width), Some(height)) = (
        number_field(&value, "x"),
        number_field(&value, "y"),
        number_field(&value, "width"),
        number_field(&value, "height"),
    ) {
        return Ok(AuiRect::fixed_position(x, y, width, height));
    }
    if value
        .get("stretch")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(AuiRect::stretch_full());
    }
    serde_json::from_value::<AuiRect>(value)
        .map_err(|error| format!("Invalid AUI rect payload: {error}"))
}

fn decode_node_field_value(
    schema_path: &str,
    value: serde_json::Value,
) -> Result<AuiNodeFieldValue, String> {
    match schema_path {
        "text" => Ok(AuiNodeFieldValue::Text(
            value
                .as_str()
                .ok_or_else(|| "AUI text field requires a string value.".to_string())?
                .to_string(),
        )),
        "visible" => Ok(AuiNodeFieldValue::Visible(value.as_bool().ok_or_else(
            || "AUI visible field requires a bool value.".to_string(),
        )?)),
        "progressValue" => Ok(AuiNodeFieldValue::ProgressValue(
            value
                .as_f64()
                .ok_or_else(|| "AUI progressValue requires a number value.".to_string())?
                as f32,
        )),
        "image.assetId" => Ok(AuiNodeFieldValue::ImageAsset(
            value
                .as_str()
                .ok_or_else(|| "AUI image.assetId requires a string value.".to_string())?
                .to_string(),
        )),
        "name" => Ok(AuiNodeFieldValue::Name(
            value
                .as_str()
                .ok_or_else(|| "AUI name field requires a string value.".to_string())?
                .to_string(),
        )),
        "interactable" => Ok(AuiNodeFieldValue::Interactable(
            value
                .as_bool()
                .ok_or_else(|| "AUI interactable field requires a bool value.".to_string())?,
        )),
        "consumeInput" => Ok(AuiNodeFieldValue::ConsumeInput(
            value
                .as_bool()
                .ok_or_else(|| "AUI consumeInput field requires a bool value.".to_string())?,
        )),
        "rect" => Ok(AuiNodeFieldValue::Rect(decode_rect(value)?)),
        "style" => Ok(AuiNodeFieldValue::Style(decode_style(value)?)),
        other => Err(format!("Unsupported AUI node field '{other}'.")),
    }
}

fn decode_style(value: serde_json::Value) -> Result<AuiStyle, String> {
    if value.is_null() {
        return Err("AUI style field requires an object value.".to_string());
    }
    serde_json::from_value::<AuiStyle>(value)
        .map_err(|error| format!("Invalid AUI style payload: {error}"))
}

fn decode_binding_target(value: &str) -> Result<AuiBindingTarget, String> {
    match value.trim() {
        "text.text" | "text" | "TextText" => Ok(AuiBindingTarget::TextText),
        "progress.value" | "progressValue" | "ProgressBarValue" => {
            Ok(AuiBindingTarget::ProgressBarValue)
        }
        "panel.visible" | "PanelVisible" => Ok(AuiBindingTarget::PanelVisible),
        "image.visible" | "ImageVisible" => Ok(AuiBindingTarget::ImageVisible),
        "image.assetRef" | "image.assetId" | "ImageAssetRef" => Ok(AuiBindingTarget::ImageAssetRef),
        other => Err(format!("Unsupported AUI binding target '{other}'.")),
    }
}

fn decode_binding_value(value: serde_json::Value) -> Result<AuiBindingValue, String> {
    match value {
        serde_json::Value::Bool(value) => Ok(AuiBindingValue::Bool(value)),
        serde_json::Value::Number(value) => value
            .as_f64()
            .map(|value| AuiBindingValue::Number(value as f32))
            .ok_or_else(|| "AUI numeric fallback must fit f64.".to_string()),
        serde_json::Value::String(value) => Ok(AuiBindingValue::String(value)),
        serde_json::Value::Object(map) => {
            if let Some(asset_id) = map.get("assetId").or_else(|| map.get("asset_id")) {
                return Ok(AuiBindingValue::AssetRef(AuiAssetRef::new(
                    asset_id
                        .as_str()
                        .ok_or_else(|| "AUI asset fallback id must be a string.".to_string())?,
                )));
            }
            if let Some(color) = map.get("color").and_then(serde_json::Value::as_str) {
                return Ok(AuiBindingValue::Color(color.to_string()));
            }
            Err(
                "Unsupported AUI fallback object. Use bool, number, string, {color}, or {assetId}."
                    .to_string(),
            )
        }
        _ => Err("Unsupported AUI fallback value.".to_string()),
    }
}

fn decode_action_event(value: &str) -> Result<AuiActionEvent, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "click" => Ok(AuiActionEvent::Click),
        "drag_start" | "dragstart" | "drag-start" => Ok(AuiActionEvent::DragStart),
        "drag_move" | "dragmove" | "drag-move" => Ok(AuiActionEvent::DragMove),
        "drop" => Ok(AuiActionEvent::Drop),
        "focus" => Ok(AuiActionEvent::Focus),
        "blur" => Ok(AuiActionEvent::Blur),
        "cancel" => Ok(AuiActionEvent::Cancel),
        "scroll" => Ok(AuiActionEvent::Scroll),
        other => Err(format!("Unsupported AUI action event '{other}'.")),
    }
}

fn number_field(value: &serde_json::Value, field: &str) -> Option<f32> {
    value
        .get(field)
        .and_then(serde_json::Value::as_f64)
        .map(|value| value as f32)
}

fn transaction_from_aui(transaction: crate::AuiTransaction) -> Result<(), String> {
    if transaction.status == AuiTransactionStatus::Committed {
        return Ok(());
    }
    Err(transaction
        .diagnostics
        .first()
        .map(|diagnostic| diagnostic.message.clone())
        .unwrap_or_else(|| "AUI authoring transaction was rejected.".to_string()))
}

fn aui_document_summary(service: &AuiAuthoringService) -> String {
    let document = service.document();
    let binding_count: usize = document
        .nodes
        .iter()
        .map(|node| node.binding_refs.len())
        .sum();
    let action_count: usize = document
        .nodes
        .iter()
        .map(|node| node.action_refs.len())
        .sum();
    format!(
        "document_id={} nodes={} bindings={} actions={}",
        document.document_id,
        document.nodes.len(),
        binding_count,
        action_count
    )
}

fn push_aui_productization_report(
    session: &EditorSession,
    transaction: &mut CommandTransaction,
    report: &AuiDocumentAuthoringProductizationReport,
) {
    if report.validation_ok {
        session.push_info(
            transaction,
            "editor.aui_authoring.report",
            format!(
                "AUI authoring report {:?}: document_id={:?} nodes={} preview={:?}",
                report.status,
                report.document_id,
                report.node_count,
                report.preview.as_ref().map(|preview| preview.status)
            ),
        );
    } else {
        session.push_error(
            transaction,
            "editor.aui_authoring.validation_failed",
            format!(
                "AUI document {:?} failed validation.",
                report.document_id.as_deref().unwrap_or("unknown")
            ),
            Some("Inspect AUI validation diagnostics."),
        );
    }

    if let Some(preview) = &report.preview {
        for diagnostic in &preview.diagnostics {
            let code = format!("editor.aui_preview.{}", diagnostic.code);
            if diagnostic.severity
                == engine_runtime::aui::AuiRuntimePresentDiagnosticSeverity::Error
            {
                session.push_error(
                    transaction,
                    &code,
                    diagnostic.message.clone(),
                    Some("Inspect the AUI preview report."),
                );
            } else {
                transaction.diagnostics.push(session.make_diagnostic(
                    transaction,
                    DiagnosticSeverity::Warning,
                    &code,
                    diagnostic.message.clone(),
                    Some("Preview is partial; continue with runtime text/glyph validation."),
                ));
            }
        }
    }
}

fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn push_aui_template_instantiate_report(
    session: &EditorSession,
    transaction: &mut CommandTransaction,
    report: &AuiTemplateInstantiateReport,
) {
    push_aui_template_diagnostics(session, transaction, &report.diagnostics);
    session.push_info(
        transaction,
        "editor.aui_template.instantiate_report",
        format!(
            "AUI template instantiate {:?}: template={} inserted_nodes={} remaps={} bindings={} actions={} guid_source={} asset_db_integrated={}.",
            report.status,
            report.template_ref.template_id,
            report.inserted_node_count,
            report.node_id_remap.len(),
            report.binding_ref_count,
            report.action_ref_count,
            report.guid_source,
            report.asset_db_integrated
        ),
    );
}

fn push_aui_template_diagnostics(
    session: &EditorSession,
    transaction: &mut CommandTransaction,
    diagnostics: &[AuiTemplateDiagnostic],
) {
    for diagnostic in diagnostics {
        let code = diagnostic.code.as_str();
        match diagnostic.severity {
            AuiTemplateDiagnosticSeverity::Info => {
                session.push_info(transaction, code, diagnostic.message.clone());
            }
            AuiTemplateDiagnosticSeverity::Warning => {
                transaction.diagnostics.push(session.make_diagnostic(
                    transaction,
                    DiagnosticSeverity::Warning,
                    code,
                    diagnostic.message.clone(),
                    Some(diagnostic.suggested_action.as_str()),
                ));
            }
            AuiTemplateDiagnosticSeverity::Error => {
                session.push_error(
                    transaction,
                    code,
                    diagnostic.message.clone(),
                    Some(diagnostic.suggested_action.as_str()),
                );
            }
        }
    }
}
