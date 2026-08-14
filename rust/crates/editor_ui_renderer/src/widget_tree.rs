use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{ControlClassSet, ControlPseudoStateSet, HitTarget, UiColor, UiRect, UiUvRect};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WidgetId(String);

impl WidgetId {
    pub fn semantic(value: impl Into<String>) -> Result<Self, WidgetTreeError> {
        let value = value.into();
        if value.is_empty()
            || value.starts_with(|character: char| character.is_ascii_digit())
            || !value
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "._-/:".contains(character))
        {
            return Err(WidgetTreeError::InvalidId(value));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn scoped(scope: &str, key: &str) -> Result<Self, WidgetTreeError> {
        let mut hash = 0xcbf29ce484222325_u64;
        for byte in key.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        Self::semantic(format!("{scope}/{hash:016x}"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WidgetRole {
    Root,
    Container,
    Panel,
    Button,
    IconButton,
    Label,
    Image,
    Viewport,
    Scroll,
    Overlay,
    TextInput,
    Tab,
    Toggle,
    Splitter,
}

impl WidgetRole {
    pub fn can_activate(self) -> bool {
        matches!(
            self,
            Self::Button
                | Self::IconButton
                | Self::TextInput
                | Self::Viewport
                | Self::Tab
                | Self::Toggle
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WidgetVisibility {
    Visible,
    Hidden,
    Collapsed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WidgetDirection {
    Row,
    Column,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditorWidgetLayoutStyle {
    pub direction: WidgetDirection,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub min_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_width: Option<f32>,
    pub max_height: Option<f32>,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub gap: f32,
    pub absolute: bool,
    pub inset_left: Option<f32>,
    pub inset_top: Option<f32>,
    pub clip: bool,
    pub z_index: i32,
}

impl Default for EditorWidgetLayoutStyle {
    fn default() -> Self {
        Self {
            direction: WidgetDirection::Column,
            width: None,
            height: None,
            min_width: None,
            min_height: None,
            max_width: None,
            max_height: None,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            gap: 0.0,
            absolute: false,
            inset_left: None,
            inset_top: None,
            clip: false,
            z_index: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WidgetPaint {
    Rect {
        color: UiColor,
        corner_radius: f32,
    },
    Text {
        text: String,
        color: UiColor,
        size: f32,
    },
    Image {
        texture_id: Option<String>,
        fallback_color: UiColor,
        source_uv: UiUvRect,
        tint: UiColor,
    },
    Viewport {
        scene_id: Option<String>,
        frame: u64,
        texture_id: Option<String>,
        target_id: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditorWidgetAction {
    Activate,
    Submit,
    Cancel,
    Focus,
    Resize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivationPolicy {
    #[default]
    ReleaseInside,
    Press,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorCommandBinding {
    pub action: EditorWidgetAction,
    pub command_id: String,
    pub target: HitTarget,
    pub reason_disabled: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WidgetLocalState {
    pub scroll_x: f32,
    pub scroll_y: f32,
    pub expanded: bool,
    pub text_cursor: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditorWidgetDeclaration {
    pub id: WidgetId,
    pub role: WidgetRole,
    pub style: EditorWidgetLayoutStyle,
    pub visibility: WidgetVisibility,
    pub enabled: bool,
    pub control_classes: ControlClassSet,
    pub model_pseudo_states: ControlPseudoStateSet,
    pub activation_policy: ActivationPolicy,
    pub binding: Option<EditorCommandBinding>,
    pub hit_region_id: Option<String>,
    pub paint: Vec<WidgetPaint>,
    pub children: Vec<Self>,
}

impl EditorWidgetDeclaration {
    pub fn new(id: WidgetId, role: WidgetRole) -> Self {
        Self {
            id,
            role,
            style: EditorWidgetLayoutStyle::default(),
            visibility: WidgetVisibility::Visible,
            enabled: true,
            control_classes: ControlClassSet::default(),
            model_pseudo_states: ControlPseudoStateSet::empty(),
            activation_policy: ActivationPolicy::ReleaseInside,
            binding: None,
            hit_region_id: None,
            paint: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn with_absolute_rect(mut self, rect: UiRect, z_index: i32) -> Self {
        self.style.absolute = true;
        self.style.inset_left = Some(rect.x);
        self.style.inset_top = Some(rect.y);
        self.style.width = Some(rect.width.max(0.0));
        self.style.height = Some(rect.height.max(0.0));
        self.style.z_index = z_index;
        self
    }

    pub fn with_interaction(
        mut self,
        hit_region_id: String,
        enabled: bool,
        binding: EditorCommandBinding,
    ) -> Self {
        self.hit_region_id = Some(hit_region_id);
        self.enabled = enabled;
        self.binding = Some(binding);
        self
    }

    pub fn with_control_style(
        mut self,
        classes: impl IntoIterator<Item = impl Into<String>>,
        model_pseudo_states: ControlPseudoStateSet,
        activation_policy: ActivationPolicy,
    ) -> Self {
        self.control_classes = ControlClassSet::new(classes);
        self.model_pseudo_states = model_pseudo_states;
        self.activation_policy = activation_policy;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditorWidgetNode {
    pub id: WidgetId,
    pub role: WidgetRole,
    pub parent: Option<WidgetId>,
    pub children: Vec<WidgetId>,
    pub style: EditorWidgetLayoutStyle,
    pub visibility: WidgetVisibility,
    pub enabled: bool,
    pub control_classes: ControlClassSet,
    pub model_pseudo_states: ControlPseudoStateSet,
    pub activation_policy: ActivationPolicy,
    pub binding: Option<EditorCommandBinding>,
    pub hit_region_id: Option<String>,
    pub paint: Vec<WidgetPaint>,
    pub local_state: WidgetLocalState,
    pub logical_rect: UiRect,
    pub effective_clip: Option<UiRect>,
    pub resolved_z: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditorWidgetTree {
    pub root: WidgetId,
    pub nodes: BTreeMap<WidgetId, EditorWidgetNode>,
}

impl EditorWidgetTree {
    pub fn validate(&self) -> Result<(), WidgetTreeError> {
        let root = self
            .nodes
            .get(&self.root)
            .ok_or_else(|| WidgetTreeError::MissingRoot(self.root.clone()))?;
        if root.parent.is_some() || root.role != WidgetRole::Root {
            return Err(WidgetTreeError::InvalidRoot(self.root.clone()));
        }
        let mut visited = BTreeSet::new();
        self.visit(&self.root, &mut visited)?;
        if visited.len() != self.nodes.len() {
            let id = self
                .nodes
                .keys()
                .find(|id| !visited.contains(*id))
                .cloned()
                .expect("node count differs");
            return Err(WidgetTreeError::Orphan(id));
        }
        for node in self.nodes.values() {
            if let Some(binding) = &node.binding {
                let legal = node.role.can_activate()
                    || (node.role == WidgetRole::Splitter
                        && binding.action == EditorWidgetAction::Resize);
                if !legal {
                    return Err(WidgetTreeError::IllegalActivation(node.id.clone()));
                }
            }
        }
        Ok(())
    }

    fn visit(
        &self,
        id: &WidgetId,
        visited: &mut BTreeSet<WidgetId>,
    ) -> Result<(), WidgetTreeError> {
        if !visited.insert(id.clone()) {
            return Err(WidgetTreeError::Cycle(id.clone()));
        }
        let node = self
            .nodes
            .get(id)
            .ok_or_else(|| WidgetTreeError::MissingNode(id.clone()))?;
        for child_id in &node.children {
            let child = self
                .nodes
                .get(child_id)
                .ok_or_else(|| WidgetTreeError::MissingNode(child_id.clone()))?;
            if child.parent.as_ref() != Some(id) {
                return Err(WidgetTreeError::ParentChildMismatch(child_id.clone()));
            }
            self.visit(child_id, visited)?;
        }
        Ok(())
    }

    pub fn node(&self, id: &WidgetId) -> Option<&EditorWidgetNode> {
        self.nodes.get(id)
    }
    pub fn node_mut(&mut self, id: &WidgetId) -> Option<&mut EditorWidgetNode> {
        self.nodes.get_mut(id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WidgetTreeError {
    InvalidId(String),
    DuplicateId(WidgetId),
    MissingRoot(WidgetId),
    InvalidRoot(WidgetId),
    MissingNode(WidgetId),
    ParentChildMismatch(WidgetId),
    Cycle(WidgetId),
    Orphan(WidgetId),
    IllegalActivation(WidgetId),
}

impl std::fmt::Display for WidgetTreeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for WidgetTreeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widget_tree_rejects_duplicate_or_non_semantic_identity() {
        assert!(WidgetId::semantic("0").is_err());
        assert!(WidgetId::semantic("toolbar/play").is_ok());
    }

    #[test]
    fn widget_invariant_rejects_activation_on_non_control() {
        let root_id = WidgetId::semantic("root").unwrap();
        let root = EditorWidgetNode {
            id: root_id.clone(),
            role: WidgetRole::Root,
            parent: None,
            children: Vec::new(),
            style: EditorWidgetLayoutStyle::default(),
            visibility: WidgetVisibility::Visible,
            enabled: true,
            control_classes: ControlClassSet::default(),
            model_pseudo_states: ControlPseudoStateSet::empty(),
            activation_policy: ActivationPolicy::ReleaseInside,
            binding: Some(EditorCommandBinding {
                action: EditorWidgetAction::Activate,
                command_id: "bad".into(),
                target: HitTarget::Viewport,
                reason_disabled: None,
            }),
            hit_region_id: None,
            paint: Vec::new(),
            local_state: WidgetLocalState::default(),
            logical_rect: UiRect {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            effective_clip: None,
            resolved_z: 0,
        };
        let tree = EditorWidgetTree {
            root: root_id.clone(),
            nodes: [(root_id, root)].into(),
        };
        assert!(matches!(
            tree.validate(),
            Err(WidgetTreeError::IllegalActivation(_))
        ));
    }
}
