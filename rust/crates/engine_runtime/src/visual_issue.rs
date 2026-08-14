use crate::aui::{AuiDocument, AuiNode, AuiRuntimePresentOutput};
use crate::canonical_digest::{canonical_json_bytes, sha256_prefixed};
use serde::{Deserialize, Serialize};

pub const VISUAL_ISSUE_BUNDLE_SCHEMA_VERSION: &str = "visual-issue-bundle.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VisualIssueContext {
    pub project_digest: String,
    pub runtime_digest: String,
    pub frame_digest: String,
    pub screenshot_ref: String,
    pub screenshot_digest: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VisualIssueNodeEvidence {
    pub node_id: String,
    pub node_name: String,
    pub authored_visible: bool,
    pub resolved_visible: Option<bool>,
    pub parent_chain: Vec<String>,
    pub binding_paths: Vec<String>,
    pub action_ids: Vec<String>,
    pub layout_rect: Option<[f32; 4]>,
    pub effective_clip_rect: Option<[f32; 4]>,
    pub clipped_by_node: Option<String>,
    pub draw_command_present: bool,
    pub text_glyph_present: bool,
    pub ui_pass_inserted: bool,
    pub first_failure_stage: String,
    pub diagnostic_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VisualIssueBundle {
    pub schema_version: String,
    pub document_id: String,
    pub context: VisualIssueContext,
    pub node: VisualIssueNodeEvidence,
    pub bundle_digest: String,
}

impl VisualIssueBundle {
    pub fn capture(
        authored: &AuiDocument,
        present: &AuiRuntimePresentOutput,
        node_id: &str,
        context: VisualIssueContext,
    ) -> Result<Self, String> {
        validate_context(&context)?;
        let authored_node = authored
            .nodes
            .iter()
            .find(|node| node.node_id == node_id)
            .ok_or_else(|| format!("AUI node '{node_id}' is absent from authored document."))?;
        let resolved_node = present
            .resolved_document
            .nodes
            .iter()
            .find(|node| node.node_id == node_id);
        let computed = present
            .layout
            .computed_nodes
            .iter()
            .find(|node| node.node_id == node_id);
        let draw_command_present = present
            .overlay
            .draw_items
            .iter()
            .any(|item| item.node_id == node_id);
        let parent_chain = parent_chain(authored, authored_node);
        let parent_hidden = parent_chain.iter().any(|parent_id| {
            present
                .resolved_document
                .nodes
                .iter()
                .find(|node| &node.node_id == parent_id)
                .is_some_and(|node| !node.visible)
        });
        let off_screen = computed.is_some_and(|node| {
            node.rect.width <= 0.0
                || node.rect.height <= 0.0
                || node.rect.x + node.rect.width <= 0.0
                || node.rect.y + node.rect.height <= 0.0
        });
        let text_glyph_present =
            authored_node.text.as_deref().is_none_or(str::is_empty) || present.report.glyph_present;
        let first_failure_stage = if !authored_node.visible {
            "authored_visibility"
        } else if parent_hidden {
            "parent_or_screen_visibility"
        } else if resolved_node.is_some_and(|node| !node.visible) {
            "binding_visibility"
        } else if computed.is_none() {
            "layout_missing"
        } else if off_screen {
            "layout_off_screen"
        } else if computed.is_some_and(|node| {
            node.clipped_by_node.is_some() && node.effective_clip_rect.is_none()
        }) {
            "clip_culled"
        } else if !draw_command_present {
            "draw_missing"
        } else if !text_glyph_present {
            "glyph_missing"
        } else if !present.report.ui_pass_inserted {
            "present_missing"
        } else {
            "visible"
        };
        let mut diagnostic_codes = present
            .report
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.clone())
            .collect::<Vec<_>>();
        diagnostic_codes.sort();
        diagnostic_codes.dedup();
        let node = VisualIssueNodeEvidence {
            node_id: authored_node.node_id.clone(),
            node_name: authored_node.name.clone(),
            authored_visible: authored_node.visible,
            resolved_visible: resolved_node.map(|node| node.visible),
            parent_chain,
            binding_paths: sorted(
                authored_node
                    .binding_refs
                    .iter()
                    .map(|binding| binding.path.clone()),
            ),
            action_ids: sorted(
                authored_node
                    .action_refs
                    .iter()
                    .map(|action| action.action_id.clone()),
            ),
            layout_rect: computed.map(|node| rect(node.rect)),
            effective_clip_rect: computed.and_then(|node| node.effective_clip_rect.map(rect)),
            clipped_by_node: computed.and_then(|node| node.clipped_by_node.clone()),
            draw_command_present,
            text_glyph_present,
            ui_pass_inserted: present.report.ui_pass_inserted,
            first_failure_stage: first_failure_stage.to_string(),
            diagnostic_codes,
        };
        let mut bundle = Self {
            schema_version: VISUAL_ISSUE_BUNDLE_SCHEMA_VERSION.to_string(),
            document_id: authored.document_id.clone(),
            context,
            node,
            bundle_digest: String::new(),
        };
        bundle.bundle_digest = digest(&bundle)?;
        Ok(bundle)
    }
}

fn validate_context(context: &VisualIssueContext) -> Result<(), String> {
    for (role, value) in [
        ("project digest", &context.project_digest),
        ("runtime digest", &context.runtime_digest),
        ("frame digest", &context.frame_digest),
        ("screenshot digest", &context.screenshot_digest),
    ] {
        if !value.starts_with("sha256:") {
            return Err(format!("Visual issue {role} must be a sha256 digest."));
        }
    }
    if context.screenshot_ref.trim().is_empty() {
        return Err("Visual issue screenshot_ref is required.".to_string());
    }
    Ok(())
}

fn parent_chain(document: &AuiDocument, node: &AuiNode) -> Vec<String> {
    let mut parents = Vec::new();
    let mut current = node.parent.as_deref();
    while let Some(parent_id) = current {
        if parents.iter().any(|value| value == parent_id) {
            break;
        }
        parents.push(parent_id.to_string());
        current = document
            .nodes
            .iter()
            .find(|candidate| candidate.node_id == parent_id)
            .and_then(|parent| parent.parent.as_deref());
    }
    parents
}

fn rect(rect: crate::aui::AuiComputedRect) -> [f32; 4] {
    [rect.x, rect.y, rect.width, rect.height]
}

fn sorted(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut values = values.into_iter().collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn digest(bundle: &VisualIssueBundle) -> Result<String, String> {
    let mut unsigned = bundle.clone();
    unsigned.bundle_digest.clear();
    let value = serde_json::to_value(unsigned).map_err(|error| error.to_string())?;
    canonical_json_bytes(&value)
        .map(|bytes| sha256_prefixed(&bytes))
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aui::{AuiCanvas, AuiNodeKind, AuiRect, AuiRuntimePresenter};

    #[test]
    fn visual_issue_bundle_reports_first_semantic_visibility_failure() {
        let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
            .with_children(["start-button"]);
        let mut button = AuiNode::new(
            "start-button",
            AuiNodeKind::Button,
            AuiRect::fixed_position(20.0, 20.0, 200.0, 60.0),
        )
        .with_parent("root")
        .with_text("Start Game");
        button.visible = false;
        let document = AuiDocument::new(
            "main-menu",
            vec![AuiCanvas::screen_overlay("main", 1280.0, 720.0, "root")],
            vec![root, button],
        );
        let present = AuiRuntimePresenter::present_package_smoke(&document, 1);
        let bundle = VisualIssueBundle::capture(
            &document,
            &present,
            "start-button",
            VisualIssueContext {
                project_digest: "sha256:project".to_string(),
                runtime_digest: "sha256:runtime".to_string(),
                frame_digest: "sha256:frame".to_string(),
                screenshot_ref: "project-evidence:Library/Reports/frame.png".to_string(),
                screenshot_digest: "sha256:screenshot".to_string(),
            },
        )
        .unwrap();
        assert_eq!(bundle.node.first_failure_stage, "authored_visibility");
        assert!(!bundle.node.draw_command_present);
        assert!(bundle.bundle_digest.starts_with("sha256:"));
    }
}
