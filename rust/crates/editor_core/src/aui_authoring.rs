use engine_runtime::aui::{
    AuiActionEvent, AuiActionRef, AuiAssetManifest, AuiBindingRef, AuiCanvas, AuiDocument,
    AuiInputSubmitBehavior, AuiLayoutEngine, AuiNavigationRef, AuiNode, AuiRect, AuiStyle,
    AuiValidationReport,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub const AUI_AUTHORING_REPORT_SCHEMA_VERSION: &str = "aui-authoring-report.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuiTransactionStatus {
    Committed,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiAuthoringDiagnostic {
    pub code: String,
    pub message: String,
    pub path: Option<String>,
}

impl AuiAuthoringDiagnostic {
    fn new(code: impl Into<String>, message: impl Into<String>, path: Option<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            path,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiTransaction {
    pub transaction_id: String,
    pub status: AuiTransactionStatus,
    pub path: Option<String>,
    pub diagnostics: Vec<AuiAuthoringDiagnostic>,
}

impl AuiTransaction {
    fn committed(transaction_id: impl Into<String>, path: Option<String>) -> Self {
        Self {
            transaction_id: transaction_id.into(),
            status: AuiTransactionStatus::Committed,
            path,
            diagnostics: Vec::new(),
        }
    }

    fn rejected(
        transaction_id: impl Into<String>,
        path: Option<String>,
        diagnostic: AuiAuthoringDiagnostic,
    ) -> Self {
        Self {
            transaction_id: transaction_id.into(),
            status: AuiTransactionStatus::Rejected,
            path,
            diagnostics: vec![diagnostic],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuiAuthoringReport {
    pub schema_version: String,
    pub document_id: String,
    pub source_path: Option<String>,
    pub canvas_count: usize,
    pub node_count: usize,
    pub binding_count: usize,
    pub action_count: usize,
    pub transaction_count: usize,
    pub validation: AuiValidationReport,
    pub diagnostics: Vec<AuiAuthoringDiagnostic>,
}

#[derive(Debug, Clone)]
pub enum AuiNodeFieldValue {
    Text(String),
    Visible(bool),
    ProgressValue(f32),
    ImageAsset(String),
    Name(String),
    Interactable(bool),
    ConsumeInput(bool),
    Rect(AuiRect),
    Style(AuiStyle),
    Binding(AuiBindingRef),
    Focusable(Option<bool>),
    Placeholder(Option<String>),
    MaxLength(Option<usize>),
    ReadOnly(bool),
    SubmitBehavior(AuiInputSubmitBehavior),
    Navigation(AuiNavigationRef),
}

#[derive(Debug, Clone)]
pub enum AuiCanvasFieldValue {
    Visible(bool),
    ScreenId(Option<String>),
    DefaultFocusNodeId(Option<String>),
    CancelActionId(Option<String>),
    SubmitActionId(Option<String>),
}

#[derive(Debug, Clone)]
pub struct AuiAuthoringService {
    document: AuiDocument,
    transaction_counter: usize,
    diagnostics: Vec<AuiAuthoringDiagnostic>,
}

impl AuiAuthoringService {
    pub fn create_document(
        document_id: impl Into<String>,
        width: f32,
        height: f32,
        root: AuiNode,
    ) -> Self {
        let root_id = root.node_id.clone();
        Self {
            document: AuiDocument::new(
                document_id,
                vec![AuiCanvas::screen_overlay("main", width, height, root_id)],
                vec![root],
            ),
            transaction_counter: 0,
            diagnostics: Vec::new(),
        }
    }

    pub fn open(path: &Path) -> std::io::Result<Self> {
        let text = fs::read_to_string(path)?;
        let document = serde_json::from_str::<AuiDocument>(&text)?;
        Ok(Self::from_document(document))
    }

    pub fn from_document(document: AuiDocument) -> Self {
        Self {
            document,
            transaction_counter: 0,
            diagnostics: Vec::new(),
        }
    }

    pub fn document(&self) -> &AuiDocument {
        &self.document
    }

    pub fn into_document(self) -> AuiDocument {
        self.document
    }

    pub fn add_node(&mut self, parent_id: impl Into<String>, mut node: AuiNode) -> AuiTransaction {
        self.transaction_counter += 1;
        let tx_id = format!("aui-tx-{}", self.transaction_counter);
        let parent_id = parent_id.into();
        let Some(parent) = self
            .document
            .nodes
            .iter_mut()
            .find(|candidate| candidate.node_id == parent_id)
        else {
            return AuiTransaction::rejected(
                tx_id,
                Some(format!("nodes.{parent_id}.children")),
                AuiAuthoringDiagnostic::new(
                    "aui_authoring.parent_missing",
                    format!("Parent node '{parent_id}' does not exist."),
                    Some(format!("nodes.{parent_id}")),
                ),
            );
        };
        node.parent = Some(parent_id.clone());
        parent.children.push(node.node_id.clone());
        self.document.nodes.push(node);
        AuiTransaction::committed(tx_id, Some(format!("nodes.{parent_id}.children")))
    }

    pub fn set_node_field(
        &mut self,
        node_id: impl Into<String>,
        schema_path: impl Into<String>,
        value: AuiNodeFieldValue,
    ) -> AuiTransaction {
        self.transaction_counter += 1;
        let tx_id = format!("aui-tx-{}", self.transaction_counter);
        let node_id = node_id.into();
        let schema_path = schema_path.into();
        let Some(node) = self
            .document
            .nodes
            .iter_mut()
            .find(|candidate| candidate.node_id == node_id)
        else {
            return AuiTransaction::rejected(
                tx_id,
                Some(schema_path.clone()),
                AuiAuthoringDiagnostic::new(
                    "aui_authoring.node_missing",
                    format!("Node '{node_id}' does not exist."),
                    Some(schema_path),
                ),
            );
        };

        let result = match (schema_path.as_str(), value) {
            ("text", AuiNodeFieldValue::Text(value)) => {
                node.text = Some(value);
                Ok(())
            }
            ("visible", AuiNodeFieldValue::Visible(value)) => {
                node.visible = value;
                Ok(())
            }
            ("progressValue", AuiNodeFieldValue::ProgressValue(value)) => {
                node.progress_value = Some(value.clamp(0.0, 1.0));
                Ok(())
            }
            ("image.assetId", AuiNodeFieldValue::ImageAsset(value)) => {
                node.image = Some(engine_runtime::aui::AuiAssetRef::new(value));
                Ok(())
            }
            ("name", AuiNodeFieldValue::Name(value)) => {
                node.name = value;
                Ok(())
            }
            ("interactable", AuiNodeFieldValue::Interactable(value)) => {
                node.interactable = value;
                Ok(())
            }
            ("consumeInput", AuiNodeFieldValue::ConsumeInput(value)) => {
                node.consume_input = value;
                Ok(())
            }
            ("rect", AuiNodeFieldValue::Rect(value)) => {
                node.rect = value;
                Ok(())
            }
            ("style", AuiNodeFieldValue::Style(value)) => {
                node.style = Some(value);
                Ok(())
            }
            ("bindingRefs", AuiNodeFieldValue::Binding(value)) => {
                node.binding_refs.push(value);
                Ok(())
            }
            ("focusable", AuiNodeFieldValue::Focusable(value)) => {
                node.focusable = value;
                Ok(())
            }
            ("placeholder", AuiNodeFieldValue::Placeholder(value)) => {
                node.placeholder = value;
                Ok(())
            }
            ("maxLength", AuiNodeFieldValue::MaxLength(value)) => {
                node.max_length = value;
                Ok(())
            }
            ("readOnly", AuiNodeFieldValue::ReadOnly(value)) => {
                node.read_only = value;
                Ok(())
            }
            ("submitBehavior", AuiNodeFieldValue::SubmitBehavior(value)) => {
                node.submit_behavior = value;
                Ok(())
            }
            ("navigation", AuiNodeFieldValue::Navigation(value)) => {
                node.navigation = value;
                Ok(())
            }
            _ => Err(AuiAuthoringDiagnostic::new(
                "aui_authoring.unsupported_schema_path",
                format!("Unsupported AUI schema path '{schema_path}'."),
                Some(schema_path.clone()),
            )),
        };

        match result {
            Ok(()) => AuiTransaction::committed(tx_id, Some(schema_path)),
            Err(diagnostic) => AuiTransaction::rejected(tx_id, Some(schema_path), diagnostic),
        }
    }

    pub fn set_canvas_field(
        &mut self,
        canvas_id: impl Into<String>,
        schema_path: impl Into<String>,
        value: AuiCanvasFieldValue,
    ) -> AuiTransaction {
        self.transaction_counter += 1;
        let tx_id = format!("aui-tx-{}", self.transaction_counter);
        let canvas_id = canvas_id.into();
        let schema_path = schema_path.into();
        let Some(canvas) = self
            .document
            .canvases
            .iter_mut()
            .find(|candidate| candidate.canvas_id == canvas_id)
        else {
            return AuiTransaction::rejected(
                tx_id,
                Some(schema_path.clone()),
                AuiAuthoringDiagnostic::new(
                    "aui_authoring.canvas_missing",
                    format!("Canvas '{canvas_id}' does not exist."),
                    Some(schema_path),
                ),
            );
        };

        let result = match (schema_path.as_str(), value) {
            ("canvasVisible", AuiCanvasFieldValue::Visible(value)) => {
                canvas.visible = value;
                Ok(())
            }
            ("screenId", AuiCanvasFieldValue::ScreenId(value)) => {
                canvas.screen_id = value;
                Ok(())
            }
            ("defaultFocusNodeId", AuiCanvasFieldValue::DefaultFocusNodeId(value)) => {
                canvas.default_focus_node_id = value;
                Ok(())
            }
            ("cancelActionId", AuiCanvasFieldValue::CancelActionId(value)) => {
                canvas.cancel_action_id = value;
                Ok(())
            }
            ("submitActionId", AuiCanvasFieldValue::SubmitActionId(value)) => {
                canvas.submit_action_id = value;
                Ok(())
            }
            _ => Err(AuiAuthoringDiagnostic::new(
                "aui_authoring.unsupported_canvas_schema_path",
                format!("Unsupported AUI canvas schema path '{schema_path}'."),
                Some(schema_path.clone()),
            )),
        };

        match result {
            Ok(()) => AuiTransaction::committed(tx_id, Some(schema_path)),
            Err(diagnostic) => AuiTransaction::rejected(tx_id, Some(schema_path), diagnostic),
        }
    }

    pub fn set_binding_path(
        &mut self,
        node_id: impl Into<String>,
        binding: AuiBindingRef,
    ) -> AuiTransaction {
        self.transaction_counter += 1;
        let tx_id = format!("aui-tx-{}", self.transaction_counter);
        let node_id = node_id.into();
        let Some(node) = self
            .document
            .nodes
            .iter_mut()
            .find(|candidate| candidate.node_id == node_id)
        else {
            return AuiTransaction::rejected(
                tx_id,
                Some(format!("nodes.{node_id}.bindingRefs")),
                AuiAuthoringDiagnostic::new(
                    "aui_authoring.node_missing",
                    format!("Node '{node_id}' does not exist."),
                    Some(format!("nodes.{node_id}")),
                ),
            );
        };

        if let Some(existing) = node
            .binding_refs
            .iter_mut()
            .find(|existing| existing.binding_id == binding.binding_id)
        {
            *existing = binding;
        } else {
            node.binding_refs.push(binding);
        }

        AuiTransaction::committed(tx_id, Some(format!("nodes.{node_id}.bindingRefs")))
    }

    pub fn set_action_ref(
        &mut self,
        node_id: impl Into<String>,
        event: AuiActionEvent,
        action_id: impl Into<String>,
    ) -> AuiTransaction {
        self.transaction_counter += 1;
        let tx_id = format!("aui-tx-{}", self.transaction_counter);
        let node_id = node_id.into();
        let action_id = action_id.into();
        let Some(node) = self
            .document
            .nodes
            .iter_mut()
            .find(|candidate| candidate.node_id == node_id)
        else {
            return AuiTransaction::rejected(
                tx_id,
                Some(format!("nodes.{node_id}.actionRefs")),
                AuiAuthoringDiagnostic::new(
                    "aui_authoring.node_missing",
                    format!("Node '{node_id}' does not exist."),
                    Some(format!("nodes.{node_id}")),
                ),
            );
        };

        if let Some(existing) = node
            .action_refs
            .iter_mut()
            .find(|existing| existing.event == event)
        {
            existing.action_id = action_id;
        } else {
            node.action_refs.push(AuiActionRef { event, action_id });
        }

        AuiTransaction::committed(tx_id, Some(format!("nodes.{node_id}.actionRefs")))
    }

    pub fn validate(&self, manifest: Option<&AuiAssetManifest>) -> AuiValidationReport {
        AuiLayoutEngine::validate(&self.document, manifest)
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let project_root = path.parent().unwrap_or_else(|| Path::new("."));
        let file_name = path.file_name().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "AUI save path has no file name",
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
        let text = serde_json::to_string_pretty(&self.document)?;
        scope
            .write_atomic(relative_path, text.as_bytes())
            .map(|_| ())
            .map_err(|error| std::io::Error::other(error.to_string()))
    }

    pub fn report(&self, manifest: Option<&AuiAssetManifest>) -> AuiAuthoringReport {
        AuiAuthoringReport {
            schema_version: AUI_AUTHORING_REPORT_SCHEMA_VERSION.to_string(),
            document_id: self.document.document_id.clone(),
            source_path: None,
            canvas_count: self.document.canvases.len(),
            node_count: self.document.nodes.len(),
            binding_count: self
                .document
                .nodes
                .iter()
                .map(|node| node.binding_refs.len())
                .sum(),
            action_count: self
                .document
                .nodes
                .iter()
                .map(|node| node.action_refs.len())
                .sum(),
            transaction_count: self.transaction_counter,
            validation: self.validate(manifest),
            diagnostics: self.diagnostics.clone(),
        }
    }
}
