use crate::input_action::{
    ActionPhase, ActionSnapshot, ActionValue, Axis2, InputActionState, PointerPosition,
};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{HashMap, HashSet};

pub const INPUT_MAPPING_SCHEMA_VERSION: &str = "input-mapping.v2";
pub const LEGACY_INPUT_MAPPING_SCHEMA_VERSION: &str = "input-mapping.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RuntimePointerButton {
    Primary,
    Secondary,
    Middle,
}

impl RuntimePointerButton {
    pub fn as_device_button(self) -> &'static str {
        match self {
            Self::Primary => "Left",
            Self::Secondary => "Right",
            Self::Middle => "Middle",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RuntimePointerDeviceKind {
    Mouse,
    Touch,
    Pen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RuntimePointerPhase {
    Down,
    Move,
    Up,
    Held,
    Cancel,
    Leave,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RuntimePointerEvent {
    pub device_kind: RuntimePointerDeviceKind,
    pub pointer_id: u64,
    pub phase: RuntimePointerPhase,
    pub x: f32,
    pub y: f32,
    pub button: Option<RuntimePointerButton>,
    pub hover_capable: bool,
}

impl RuntimePointerEvent {
    pub fn mouse(
        phase: RuntimePointerPhase,
        pointer_id: u64,
        x: f32,
        y: f32,
        button: Option<RuntimePointerButton>,
    ) -> Self {
        Self {
            device_kind: RuntimePointerDeviceKind::Mouse,
            pointer_id,
            phase,
            x,
            y,
            button,
            hover_capable: true,
        }
    }

    pub fn touch(phase: RuntimePointerPhase, pointer_id: u64, x: f32, y: f32) -> Self {
        Self {
            device_kind: RuntimePointerDeviceKind::Touch,
            pointer_id,
            phase,
            x,
            y,
            button: Some(RuntimePointerButton::Primary),
            hover_capable: false,
        }
    }

    pub fn pen(
        phase: RuntimePointerPhase,
        pointer_id: u64,
        x: f32,
        y: f32,
        button: Option<RuntimePointerButton>,
        hover_capable: bool,
    ) -> Self {
        Self {
            device_kind: RuntimePointerDeviceKind::Pen,
            pointer_id,
            phase,
            x,
            y,
            button,
            hover_capable,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RuntimeInputEvent {
    Pointer {
        pointer: RuntimePointerEvent,
    },
    PointerDown {
        x: f32,
        y: f32,
        button: RuntimePointerButton,
    },
    PointerMove {
        x: f32,
        y: f32,
    },
    PointerUp {
        x: f32,
        y: f32,
        button: RuntimePointerButton,
    },
    PointerHeld {
        x: f32,
        y: f32,
        button: RuntimePointerButton,
    },
    MouseWheel {
        delta: f32,
    },
    KeyDown {
        key: String,
    },
    KeyUp {
        key: String,
    },
    KeyHeld {
        key: String,
    },
    TextInput {
        text: String,
    },
    ImePreedit {
        text: String,
        cursor_start: usize,
        cursor_end: usize,
    },
    ImeCommit {
        text: String,
    },
    ImeCancel,
    GamepadButtonDown {
        gamepad_id: u32,
        button: String,
    },
    GamepadButtonUp {
        gamepad_id: u32,
        button: String,
    },
    GamepadButtonHeld {
        gamepad_id: u32,
        button: String,
    },
    GamepadAxis2d {
        gamepad_id: u32,
        axis: String,
        x: f32,
        y: f32,
    },
}

impl RuntimeInputEvent {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Pointer { pointer } => match pointer.phase {
                RuntimePointerPhase::Down => "PointerDown",
                RuntimePointerPhase::Move => "PointerMove",
                RuntimePointerPhase::Up => "PointerUp",
                RuntimePointerPhase::Held => "PointerHeld",
                RuntimePointerPhase::Cancel => "PointerCancel",
                RuntimePointerPhase::Leave => "PointerLeave",
            },
            Self::PointerDown { .. } => "PointerDown",
            Self::PointerMove { .. } => "PointerMove",
            Self::PointerUp { .. } => "PointerUp",
            Self::PointerHeld { .. } => "PointerHeld",
            Self::MouseWheel { .. } => "MouseWheel",
            Self::KeyDown { .. } => "KeyDown",
            Self::KeyUp { .. } => "KeyUp",
            Self::KeyHeld { .. } => "KeyHeld",
            Self::TextInput { .. } => "TextInput",
            Self::ImePreedit { .. } => "ImePreedit",
            Self::ImeCommit { .. } => "ImeCommit",
            Self::ImeCancel => "ImeCancel",
            Self::GamepadButtonDown { .. } => "GamepadButtonDown",
            Self::GamepadButtonUp { .. } => "GamepadButtonUp",
            Self::GamepadButtonHeld { .. } => "GamepadButtonHeld",
            Self::GamepadAxis2d { .. } => "GamepadAxis2d",
        }
    }

    pub fn is_pointer_event(&self) -> bool {
        matches!(
            self,
            Self::Pointer { .. }
                | Self::PointerDown { .. }
                | Self::PointerMove { .. }
                | Self::PointerUp { .. }
                | Self::PointerHeld { .. }
        )
    }

    pub fn pointer_event(&self) -> Option<RuntimePointerEvent> {
        match self {
            Self::Pointer { pointer } => Some(*pointer),
            Self::PointerDown { x, y, button } => Some(RuntimePointerEvent::mouse(
                RuntimePointerPhase::Down,
                0,
                *x,
                *y,
                Some(*button),
            )),
            Self::PointerMove { x, y } => Some(RuntimePointerEvent::mouse(
                RuntimePointerPhase::Move,
                0,
                *x,
                *y,
                None,
            )),
            Self::PointerUp { x, y, button } => Some(RuntimePointerEvent::mouse(
                RuntimePointerPhase::Up,
                0,
                *x,
                *y,
                Some(*button),
            )),
            Self::PointerHeld { x, y, button } => Some(RuntimePointerEvent::mouse(
                RuntimePointerPhase::Held,
                0,
                *x,
                *y,
                Some(*button),
            )),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeInputFrame {
    pub frame_id: u64,
    pub viewport_id: String,
    pub events: Vec<RuntimeInputEvent>,
    pub modifiers: Vec<String>,
    pub pointer_position: Option<PointerPosition>,
}

impl RuntimeInputFrame {
    pub fn new(frame_id: u64, viewport_id: impl Into<String>) -> Self {
        Self {
            frame_id,
            viewport_id: viewport_id.into(),
            events: Vec::new(),
            modifiers: Vec::new(),
            pointer_position: None,
        }
    }

    pub fn filter_consumed_pointer_events(&self, consumed_event_indices: &[usize]) -> Self {
        if consumed_event_indices.is_empty() {
            return self.clone();
        }
        let consumed: HashSet<usize> = consumed_event_indices.iter().copied().collect();
        let mut filtered = self.clone();
        filtered.events = self
            .events
            .iter()
            .enumerate()
            .filter_map(|(index, event)| {
                if consumed.contains(&index) && event.is_pointer_event() {
                    None
                } else {
                    Some(event.clone())
                }
            })
            .collect();
        filtered
    }

    pub fn filter_consumed_events(&self, consumed_event_indices: &[usize]) -> Self {
        if consumed_event_indices.is_empty() {
            return self.clone();
        }
        let consumed: HashSet<usize> = consumed_event_indices.iter().copied().collect();
        let mut filtered = self.clone();
        filtered.events = self
            .events
            .iter()
            .enumerate()
            .filter_map(|(index, event)| {
                if consumed.contains(&index) {
                    None
                } else {
                    Some(event.clone())
                }
            })
            .collect();
        filtered
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputActionValueType {
    Button,
    Axis1,
    Axis2,
    Pointer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputControlDeviceKind {
    Keyboard,
    Mouse,
    Gamepad,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputControlCatalogEntry {
    pub device_path: String,
    pub label: String,
    pub device_kind: InputControlDeviceKind,
    pub compatible_value_types: Vec<InputActionValueType>,
    pub selectable: bool,
    pub capture_supported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputControlCatalog {
    pub schema_version: String,
    pub controls: Vec<InputControlCatalogEntry>,
}

impl InputControlCatalog {
    pub fn supported() -> Self {
        let button = vec![InputActionValueType::Button];
        Self {
            schema_version: "input-control-catalog.v1".to_string(),
            controls: vec![
                InputControlCatalogEntry {
                    device_path: "keyboard/*".to_string(),
                    label: "Keyboard key".to_string(),
                    device_kind: InputControlDeviceKind::Keyboard,
                    compatible_value_types: vec![
                        InputActionValueType::Button,
                        InputActionValueType::Axis2,
                    ],
                    selectable: false,
                    capture_supported: true,
                },
                InputControlCatalogEntry {
                    device_path: "mouse/Left".to_string(),
                    label: "Mouse left button".to_string(),
                    device_kind: InputControlDeviceKind::Mouse,
                    compatible_value_types: button.clone(),
                    selectable: true,
                    capture_supported: true,
                },
                InputControlCatalogEntry {
                    device_path: "mouse/Right".to_string(),
                    label: "Mouse right button".to_string(),
                    device_kind: InputControlDeviceKind::Mouse,
                    compatible_value_types: button.clone(),
                    selectable: true,
                    capture_supported: true,
                },
                InputControlCatalogEntry {
                    device_path: "mouse/Middle".to_string(),
                    label: "Mouse middle button".to_string(),
                    device_kind: InputControlDeviceKind::Mouse,
                    compatible_value_types: button.clone(),
                    selectable: true,
                    capture_supported: true,
                },
                InputControlCatalogEntry {
                    device_path: "mouse/Position".to_string(),
                    label: "Mouse position".to_string(),
                    device_kind: InputControlDeviceKind::Mouse,
                    compatible_value_types: vec![InputActionValueType::Pointer],
                    selectable: true,
                    capture_supported: true,
                },
                InputControlCatalogEntry {
                    device_path: "mouse/Wheel".to_string(),
                    label: "Mouse wheel".to_string(),
                    device_kind: InputControlDeviceKind::Mouse,
                    compatible_value_types: vec![InputActionValueType::Axis1],
                    selectable: true,
                    capture_supported: true,
                },
                InputControlCatalogEntry {
                    device_path: "gamepad/South".to_string(),
                    label: "Gamepad south button".to_string(),
                    device_kind: InputControlDeviceKind::Gamepad,
                    compatible_value_types: button,
                    selectable: true,
                    capture_supported: false,
                },
                InputControlCatalogEntry {
                    device_path: "gamepad/LeftStick".to_string(),
                    label: "Gamepad left stick".to_string(),
                    device_kind: InputControlDeviceKind::Gamepad,
                    compatible_value_types: vec![InputActionValueType::Axis2],
                    selectable: true,
                    capture_supported: false,
                },
            ],
        }
    }

    pub fn supports_device_path(&self, device_path: &str) -> bool {
        let lower = device_path.to_ascii_lowercase();
        self.controls.iter().any(|control| {
            let catalog_path = control.device_path.to_ascii_lowercase();
            catalog_path
                .strip_suffix('*')
                .is_some_and(|prefix| lower.starts_with(prefix) && lower.len() > prefix.len())
                || catalog_path == lower
        })
    }

    pub fn compatible_value_types(&self, device_path: &str) -> Option<&[InputActionValueType]> {
        let lower = device_path.to_ascii_lowercase();
        self.controls
            .iter()
            .find(|control| {
                let catalog_path = control.device_path.to_ascii_lowercase();
                catalog_path
                    .strip_suffix('*')
                    .is_some_and(|prefix| lower.starts_with(prefix) && lower.len() > prefix.len())
                    || catalog_path == lower
            })
            .map(|control| control.compatible_value_types.as_slice())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputActionDefinition {
    pub id: String,
    pub value_type: InputActionValueType,
}

impl InputActionDefinition {
    pub fn new(id: impl Into<String>, value_type: InputActionValueType) -> Self {
        Self {
            id: id.into(),
            value_type,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputContextDefinition {
    pub id: String,
    pub priority: i32,
    pub consume_input: bool,
    pub enabled_by_default: bool,
}

impl InputContextDefinition {
    pub fn new(id: impl Into<String>, priority: i32) -> Self {
        Self {
            id: id.into(),
            priority,
            consume_input: false,
            enabled_by_default: true,
        }
    }

    pub fn with_consume_input(mut self, consume_input: bool) -> Self {
        self.consume_input = consume_input;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InputProcessorPreset {
    None,
    Deadzone { threshold: f32 },
    Normalize,
    Scale { factor: f32 },
    Invert,
}

impl Default for InputProcessorPreset {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InputTriggerPreset {
    Down,
    Pressed,
    Released,
    Hold { seconds: f32 },
    Tap { max_seconds: f32 },
}

impl Default for InputTriggerPreset {
    fn default() -> Self {
        Self::Pressed
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputBindingDefinition {
    #[serde(default)]
    pub binding_id: String,
    pub context_id: String,
    pub action_id: String,
    pub device_path: String,
    #[serde(default)]
    pub processor: InputProcessorPreset,
    #[serde(default)]
    pub trigger: InputTriggerPreset,
}

impl InputBindingDefinition {
    pub fn new(
        context_id: impl Into<String>,
        action_id: impl Into<String>,
        device_path: impl Into<String>,
    ) -> Self {
        Self {
            binding_id: String::new(),
            context_id: context_id.into(),
            action_id: action_id.into(),
            device_path: device_path.into(),
            processor: InputProcessorPreset::default(),
            trigger: InputTriggerPreset::default(),
        }
    }

    pub fn button(action_id: impl Into<String>, key: impl Into<String>) -> Self {
        Self::new("gameplay", action_id, format!("keyboard/{}", key.into()))
    }

    pub fn pointer(action_id: impl Into<String>) -> Self {
        Self::new("gameplay", action_id, "mouse/Position")
    }

    pub fn mouse_wheel(action_id: impl Into<String>) -> Self {
        Self::new("gameplay", action_id, "mouse/Wheel")
    }

    pub fn axis2_wasd(action_id: impl Into<String>) -> Vec<Self> {
        let action_id = action_id.into();
        [
            ("keyboard/D", InputProcessorPreset::None),
            ("keyboard/A", InputProcessorPreset::Invert),
            ("keyboard/W", InputProcessorPreset::None),
            ("keyboard/S", InputProcessorPreset::Invert),
        ]
        .into_iter()
        .map(|(path, processor)| Self {
            binding_id: String::new(),
            context_id: "gameplay".to_string(),
            action_id: action_id.clone(),
            device_path: path.to_string(),
            processor,
            trigger: InputTriggerPreset::Pressed,
        })
        .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlatformInputOverride {
    pub platform: String,
    pub binding_overrides: Vec<InputBindingDefinition>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InputMappingAsset {
    pub schema_version: String,
    pub asset_id: String,
    pub actions: Vec<InputActionDefinition>,
    pub contexts: Vec<InputContextDefinition>,
    pub bindings: Vec<InputBindingDefinition>,
    #[serde(default)]
    pub platform_overrides: Vec<PlatformInputOverride>,
}

#[derive(Deserialize)]
struct InputMappingAssetDocument {
    schema_version: String,
    asset_id: String,
    actions: Vec<InputActionDefinition>,
    contexts: Vec<InputContextDefinition>,
    bindings: Vec<InputBindingDefinition>,
    #[serde(default)]
    platform_overrides: Vec<PlatformInputOverride>,
}

impl<'de> Deserialize<'de> for InputMappingAsset {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let document = InputMappingAssetDocument::deserialize(deserializer)?;
        let mut mapping = Self {
            schema_version: document.schema_version,
            asset_id: document.asset_id,
            actions: document.actions,
            contexts: document.contexts,
            bindings: document.bindings,
            platform_overrides: document.platform_overrides,
        };
        if matches!(
            mapping.schema_version.as_str(),
            INPUT_MAPPING_SCHEMA_VERSION | LEGACY_INPUT_MAPPING_SCHEMA_VERSION
        ) {
            mapping.normalize();
        }
        Ok(mapping)
    }
}

impl InputMappingAsset {
    pub fn new(
        asset_id: impl Into<String>,
        actions: Vec<InputActionDefinition>,
        contexts: Vec<InputContextDefinition>,
        bindings: Vec<InputBindingDefinition>,
    ) -> Self {
        let mut mapping = Self {
            schema_version: INPUT_MAPPING_SCHEMA_VERSION.to_string(),
            asset_id: asset_id.into(),
            actions,
            contexts,
            bindings,
            platform_overrides: Vec::new(),
        };
        mapping.normalize();
        mapping
    }

    pub fn normalize(&mut self) {
        self.schema_version = INPUT_MAPPING_SCHEMA_VERSION.to_string();
        assign_missing_binding_ids(&self.asset_id, "default", &mut self.bindings);
        for platform_override in &mut self.platform_overrides {
            assign_missing_binding_ids(
                &self.asset_id,
                &format!("platform:{}", platform_override.platform),
                &mut platform_override.binding_overrides,
            );
        }
    }

    pub fn gameplay_default() -> Self {
        let mut bindings = InputBindingDefinition::axis2_wasd("action.move");
        bindings.push(InputBindingDefinition::button("action.fire", "Space"));
        bindings.push(InputBindingDefinition::new(
            "gameplay",
            "action.fire",
            "mouse/Left",
        ));
        bindings.push(InputBindingDefinition::pointer("action.pointer"));
        Self::new(
            "input.default",
            vec![
                InputActionDefinition::new("action.move", InputActionValueType::Axis2),
                InputActionDefinition::new("action.fire", InputActionValueType::Button),
                InputActionDefinition::new("action.pointer", InputActionValueType::Pointer),
            ],
            vec![InputContextDefinition::new("gameplay", 0)],
            bindings,
        )
    }

    pub fn explicit_empty(asset_id: impl Into<String>) -> Self {
        Self::new(asset_id, Vec::new(), Vec::new(), Vec::new())
    }

    pub fn validate(&self) -> InputMappingReport {
        let mut diagnostics = Vec::new();
        if self.schema_version != INPUT_MAPPING_SCHEMA_VERSION {
            diagnostics.push(InputMappingDiagnostic::error(
                "input_mapping.schema_version",
                format!(
                    "Expected schema_version {}, got {}.",
                    INPUT_MAPPING_SCHEMA_VERSION, self.schema_version
                ),
            ));
        }

        let action_ids: HashSet<&str> = self
            .actions
            .iter()
            .map(|action| action.id.as_str())
            .collect();
        let context_ids: HashSet<&str> = self
            .contexts
            .iter()
            .map(|context| context.id.as_str())
            .collect();
        let catalog = InputControlCatalog::supported();
        let mut binding_ids = HashSet::new();
        let mut binding_keys: HashMap<(String, String), String> = HashMap::new();
        for binding in &self.bindings {
            if binding.binding_id.is_empty() {
                diagnostics.push(InputMappingDiagnostic::error(
                    "input_mapping.missing_binding_id",
                    "Binding is missing stable binding_id.",
                ));
            } else if !binding_ids.insert(binding.binding_id.as_str()) {
                diagnostics.push(InputMappingDiagnostic::error(
                    "input_mapping.duplicate_binding_id",
                    format!("Duplicate binding_id '{}'.", binding.binding_id),
                ));
            }
            if !action_ids.contains(binding.action_id.as_str()) {
                diagnostics.push(
                    InputMappingDiagnostic::error(
                        "input_mapping.unknown_action",
                        format!("Binding references unknown action '{}'.", binding.action_id),
                    )
                    .with_source(binding, "action_id")
                    .with_fix("Select an existing action or create it before saving."),
                );
            }
            if !context_ids.contains(binding.context_id.as_str()) {
                diagnostics.push(
                    InputMappingDiagnostic::error(
                        "input_mapping.unknown_context",
                        format!(
                            "Binding references unknown context '{}'.",
                            binding.context_id
                        ),
                    )
                    .with_source(binding, "context_id")
                    .with_fix("Select an existing context or create it before saving."),
                );
            }
            if !is_supported_device_path(&binding.device_path) {
                diagnostics.push(
                    InputMappingDiagnostic::error(
                        "input_mapping.unsupported_device_path",
                        format!("Unsupported device_path '{}'.", binding.device_path),
                    )
                    .with_source(binding, "device_path")
                    .with_fix("Choose a path from InputControlCatalog."),
                );
            }
            if let Some(action_type) = self.action_type(&binding.action_id) {
                if catalog
                    .compatible_value_types(&binding.device_path)
                    .is_some_and(|types| !types.contains(&action_type))
                {
                    diagnostics.push(
                        InputMappingDiagnostic::warning(
                            "input_mapping.action_value_type_device_mismatch",
                            format!(
                                "Action '{}' ({action_type:?}) is incompatible with device_path '{}'.",
                                binding.action_id, binding.device_path
                            ),
                        )
                        .with_source(binding, "device_path")
                        .with_fix("Choose a compatible device path or change the action value type."),
                    );
                }
            }
            let key = (
                binding.context_id.clone(),
                binding.device_path.to_ascii_lowercase(),
            );
            if let Some(first_binding_id) = binding_keys.insert(key, binding.binding_id.clone()) {
                diagnostics.push(
                    InputMappingDiagnostic::warning(
                        "input_mapping.duplicate_binding_in_same_context",
                        format!(
                            "Binding '{}' duplicates device_path '{}' in context '{}'; first binding is '{}'.",
                            binding.binding_id,
                            binding.device_path,
                            binding.context_id,
                            first_binding_id
                        ),
                    )
                    .with_source(binding, "device_path")
                    .with_fix("Remove one binding or assign a different device path."),
                );
            }
        }

        for action in &self.actions {
            if !self
                .bindings
                .iter()
                .any(|binding| binding.action_id == action.id)
            {
                diagnostics.push(
                    InputMappingDiagnostic::warning(
                        "input_mapping.action_without_binding",
                        format!("Action '{}' has no bindings.", action.id),
                    )
                    .with_action(&action.id)
                    .with_fix("Add at least one binding or remove the unused action."),
                );
            }
        }

        let mut shadowed = HashSet::new();
        let mut shadowing = HashSet::new();
        for higher in &self.bindings {
            if !self.context_consumes_input(&higher.context_id) {
                continue;
            }
            let higher_priority = self.context_priority(&higher.context_id);
            for lower in &self.bindings {
                if higher.binding_id == lower.binding_id
                    || higher.context_id == lower.context_id
                    || higher_priority <= self.context_priority(&lower.context_id)
                    || !higher.device_path.eq_ignore_ascii_case(&lower.device_path)
                {
                    continue;
                }
                if shadowed.insert(lower.binding_id.clone()) {
                    diagnostics.push(
                        InputMappingDiagnostic::warning(
                            "input_mapping.hidden_by_higher_priority_consuming_context",
                            format!(
                                "Binding '{}' is hidden by '{}' from higher-priority consuming context '{}'.",
                                lower.binding_id, higher.binding_id, higher.context_id
                            ),
                        )
                        .with_source(lower, "device_path")
                        .with_fix("Change context priority/consume_input or use a different device path."),
                    );
                }
                if shadowing.insert(higher.binding_id.clone()) {
                    diagnostics.push(
                        InputMappingDiagnostic::warning(
                            "input_mapping.hides_lower_priority_binding",
                            format!(
                                "Binding '{}' hides lower-priority bindings for '{}'.",
                                higher.binding_id, higher.device_path
                            ),
                        )
                        .with_source(higher, "device_path")
                        .with_fix("Keep this only when the consuming context is intentional."),
                    );
                }
            }
        }

        InputMappingReport { diagnostics }
    }

    fn action_type(&self, action_id: &str) -> Option<InputActionValueType> {
        self.actions
            .iter()
            .find(|action| action.id == action_id)
            .map(|action| action.value_type)
    }

    fn context_priority(&self, context_id: &str) -> i32 {
        self.contexts
            .iter()
            .find(|context| context.id == context_id)
            .map(|context| context.priority)
            .unwrap_or_default()
    }

    fn context_consumes_input(&self, context_id: &str) -> bool {
        self.contexts
            .iter()
            .find(|context| context.id == context_id)
            .map(|context| context.consume_input)
            .unwrap_or(false)
    }
}

fn assign_missing_binding_ids(
    asset_id: &str,
    scope: &str,
    bindings: &mut [InputBindingDefinition],
) {
    let mut occurrences: HashMap<(String, String, String), usize> = HashMap::new();
    for binding in bindings {
        let key = (
            binding.context_id.clone(),
            binding.action_id.clone(),
            binding.device_path.to_ascii_lowercase(),
        );
        let occurrence = occurrences.entry(key).or_default();
        if binding.binding_id.is_empty() {
            binding.binding_id = deterministic_binding_id(asset_id, scope, binding, *occurrence);
        }
        *occurrence += 1;
    }
}

fn deterministic_binding_id(
    asset_id: &str,
    scope: &str,
    binding: &InputBindingDefinition,
    occurrence: usize,
) -> String {
    let source = format!(
        "{asset_id}\u{1f}{scope}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{occurrence}",
        binding.context_id,
        binding.action_id,
        binding.device_path.to_ascii_lowercase()
    );
    let hash = source
        .as_bytes()
        .iter()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        });
    format!("binding.{hash:016x}")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputDiagnosticSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputMappingDiagnostic {
    pub severity: InputDiagnosticSeverity,
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub context_id: Option<String>,
    #[serde(default)]
    pub action_id: Option<String>,
    #[serde(default)]
    pub binding_id: Option<String>,
    #[serde(default)]
    pub field_path: Option<String>,
    #[serde(default)]
    pub suggested_fix: Option<String>,
}

impl InputMappingDiagnostic {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: InputDiagnosticSeverity::Error,
            code: code.into(),
            message: message.into(),
            context_id: None,
            action_id: None,
            binding_id: None,
            field_path: None,
            suggested_fix: None,
        }
    }

    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: InputDiagnosticSeverity::Warning,
            code: code.into(),
            message: message.into(),
            context_id: None,
            action_id: None,
            binding_id: None,
            field_path: None,
            suggested_fix: None,
        }
    }

    fn with_source(mut self, binding: &InputBindingDefinition, field_path: &str) -> Self {
        self.context_id = Some(binding.context_id.clone());
        self.action_id = Some(binding.action_id.clone());
        self.binding_id = Some(binding.binding_id.clone());
        self.field_path = Some(field_path.to_string());
        self
    }

    fn with_action(mut self, action_id: &str) -> Self {
        self.action_id = Some(action_id.to_string());
        self
    }

    fn with_fix(mut self, suggested_fix: &str) -> Self {
        self.suggested_fix = Some(suggested_fix.to_string());
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputMappingReport {
    pub diagnostics: Vec<InputMappingDiagnostic>,
}

impl InputMappingReport {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == InputDiagnosticSeverity::Error)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputResolveResult {
    pub action_snapshot: ActionSnapshot,
    pub diagnostics: Vec<InputMappingDiagnostic>,
}

pub struct InputResolver;

impl InputResolver {
    pub fn resolve(frame: &RuntimeInputFrame, mapping: &InputMappingAsset) -> InputResolveResult {
        let validation = mapping.validate();
        let mut actions_by_id: HashMap<String, InputActionState> = HashMap::new();
        let mut consumed_inputs: HashSet<String> = HashSet::new();
        let mut bindings = mapping.bindings.iter().collect::<Vec<_>>();
        bindings.sort_by(|a, b| {
            mapping
                .context_priority(&b.context_id)
                .cmp(&mapping.context_priority(&a.context_id))
        });

        for binding in bindings {
            let input_key = input_identity(&binding.device_path);
            if consumed_inputs.contains(&input_key) {
                continue;
            }
            let Some(action_type) = mapping.action_type(&binding.action_id) else {
                continue;
            };
            let Some(state) = resolve_binding(frame, binding, action_type) else {
                continue;
            };

            actions_by_id
                .entry(binding.action_id.clone())
                .and_modify(|existing| merge_action(existing, &state))
                .or_insert(state);

            if mapping.context_consumes_input(&binding.context_id) {
                consumed_inputs.insert(input_key);
            }
        }

        let mut actions = actions_by_id.into_values().collect::<Vec<_>>();
        actions.sort_by(|a, b| a.action_id.cmp(&b.action_id));
        InputResolveResult {
            action_snapshot: ActionSnapshot::with_actions(frame.frame_id, actions),
            diagnostics: validation.diagnostics,
        }
    }
}

fn resolve_binding(
    frame: &RuntimeInputFrame,
    binding: &InputBindingDefinition,
    action_type: InputActionValueType,
) -> Option<InputActionState> {
    match action_type {
        InputActionValueType::Button => resolve_button(frame, binding),
        InputActionValueType::Axis1 => resolve_axis1(frame, binding),
        InputActionValueType::Axis2 => resolve_axis2(frame, binding),
        InputActionValueType::Pointer => resolve_pointer(frame, binding),
    }
}

fn resolve_button(
    frame: &RuntimeInputFrame,
    binding: &InputBindingDefinition,
) -> Option<InputActionState> {
    for event in &frame.events {
        if let RuntimeInputEvent::Pointer { pointer } = event {
            let Some(button) = pointer.button else {
                continue;
            };
            let device_prefix = match pointer.device_kind {
                RuntimePointerDeviceKind::Mouse => "mouse",
                RuntimePointerDeviceKind::Touch => "touch",
                RuntimePointerDeviceKind::Pen => "pen",
            };
            if !binding
                .device_path
                .eq_ignore_ascii_case(&format!("{device_prefix}/{}", button.as_device_button()))
            {
                continue;
            }
            let phase = match (pointer.phase, &binding.trigger) {
                (RuntimePointerPhase::Down, InputTriggerPreset::Down)
                | (RuntimePointerPhase::Down, InputTriggerPreset::Pressed)
                | (RuntimePointerPhase::Down, InputTriggerPreset::Tap { .. }) => {
                    ActionPhase::Pressed
                }
                (RuntimePointerPhase::Down, InputTriggerPreset::Hold { .. })
                | (RuntimePointerPhase::Held, InputTriggerPreset::Pressed)
                | (RuntimePointerPhase::Held, InputTriggerPreset::Hold { .. }) => ActionPhase::Held,
                (RuntimePointerPhase::Up, InputTriggerPreset::Released) => ActionPhase::Released,
                _ => continue,
            };
            return Some(InputActionState::button(binding.action_id.clone(), phase));
        }
        let matches = match event {
            RuntimeInputEvent::KeyDown { key }
            | RuntimeInputEvent::KeyUp { key }
            | RuntimeInputEvent::KeyHeld { key } => binding
                .device_path
                .eq_ignore_ascii_case(&format!("keyboard/{key}")),
            RuntimeInputEvent::PointerDown { button, .. }
            | RuntimeInputEvent::PointerUp { button, .. }
            | RuntimeInputEvent::PointerHeld { button, .. } => binding
                .device_path
                .eq_ignore_ascii_case(&format!("mouse/{}", button.as_device_button())),
            RuntimeInputEvent::GamepadButtonDown { button, .. }
            | RuntimeInputEvent::GamepadButtonUp { button, .. }
            | RuntimeInputEvent::GamepadButtonHeld { button, .. } => binding
                .device_path
                .eq_ignore_ascii_case(&format!("gamepad/{button}")),
            RuntimeInputEvent::Pointer { .. }
            | RuntimeInputEvent::PointerMove { .. }
            | RuntimeInputEvent::MouseWheel { .. }
            | RuntimeInputEvent::TextInput { .. }
            | RuntimeInputEvent::ImePreedit { .. }
            | RuntimeInputEvent::ImeCommit { .. }
            | RuntimeInputEvent::ImeCancel
            | RuntimeInputEvent::GamepadAxis2d { .. } => false,
        };
        if !matches {
            continue;
        }

        let phase = match (event, &binding.trigger) {
            (RuntimeInputEvent::KeyDown { .. }, InputTriggerPreset::Down)
            | (RuntimeInputEvent::KeyDown { .. }, InputTriggerPreset::Pressed)
            | (RuntimeInputEvent::PointerDown { .. }, InputTriggerPreset::Down)
            | (RuntimeInputEvent::PointerDown { .. }, InputTriggerPreset::Pressed)
            | (RuntimeInputEvent::GamepadButtonDown { .. }, InputTriggerPreset::Down)
            | (RuntimeInputEvent::GamepadButtonDown { .. }, InputTriggerPreset::Pressed) => {
                ActionPhase::Pressed
            }
            (RuntimeInputEvent::KeyHeld { .. }, InputTriggerPreset::Pressed)
            | (RuntimeInputEvent::KeyHeld { .. }, InputTriggerPreset::Hold { .. })
            | (RuntimeInputEvent::PointerHeld { .. }, InputTriggerPreset::Pressed)
            | (RuntimeInputEvent::PointerHeld { .. }, InputTriggerPreset::Hold { .. })
            | (RuntimeInputEvent::GamepadButtonHeld { .. }, InputTriggerPreset::Pressed)
            | (RuntimeInputEvent::GamepadButtonHeld { .. }, InputTriggerPreset::Hold { .. }) => {
                ActionPhase::Held
            }
            (RuntimeInputEvent::KeyUp { .. }, InputTriggerPreset::Released)
            | (RuntimeInputEvent::PointerUp { .. }, InputTriggerPreset::Released)
            | (RuntimeInputEvent::GamepadButtonUp { .. }, InputTriggerPreset::Released) => {
                ActionPhase::Released
            }
            (RuntimeInputEvent::KeyDown { .. }, InputTriggerPreset::Hold { .. })
            | (RuntimeInputEvent::PointerDown { .. }, InputTriggerPreset::Hold { .. })
            | (RuntimeInputEvent::GamepadButtonDown { .. }, InputTriggerPreset::Hold { .. }) => {
                ActionPhase::Held
            }
            (RuntimeInputEvent::KeyDown { .. }, InputTriggerPreset::Tap { .. })
            | (RuntimeInputEvent::PointerDown { .. }, InputTriggerPreset::Tap { .. })
            | (RuntimeInputEvent::GamepadButtonDown { .. }, InputTriggerPreset::Tap { .. }) => {
                ActionPhase::Pressed
            }
            _ => continue,
        };
        return Some(InputActionState::button(binding.action_id.clone(), phase));
    }
    None
}

fn resolve_axis2(
    frame: &RuntimeInputFrame,
    binding: &InputBindingDefinition,
) -> Option<InputActionState> {
    let mut axis = Axis2 { x: 0.0, y: 0.0 };
    for event in &frame.events {
        let mut value = match event {
            RuntimeInputEvent::KeyDown { key } | RuntimeInputEvent::KeyHeld { key }
                if binding
                    .device_path
                    .eq_ignore_ascii_case(&format!("keyboard/{key}")) =>
            {
                match binding.device_path.to_ascii_lowercase().as_str() {
                    "keyboard/a" | "keyboard/d" => Axis2 { x: 1.0, y: 0.0 },
                    "keyboard/w" | "keyboard/s" => Axis2 { x: 0.0, y: 1.0 },
                    _ => Axis2 { x: 0.0, y: 0.0 },
                }
            }
            RuntimeInputEvent::GamepadAxis2d {
                axis: axis_name,
                x,
                y,
                ..
            } if binding
                .device_path
                .eq_ignore_ascii_case(&format!("gamepad/{axis_name}")) =>
            {
                Axis2 { x: *x, y: *y }
            }
            _ => continue,
        };
        value = apply_axis2_processor(value, &binding.processor);
        axis.x += value.x;
        axis.y += value.y;
    }
    if axis.x == 0.0 && axis.y == 0.0 {
        return None;
    }
    Some(InputActionState::axis2(
        binding.action_id.clone(),
        axis.x.clamp(-1.0, 1.0),
        axis.y.clamp(-1.0, 1.0),
    ))
}

fn resolve_pointer(
    frame: &RuntimeInputFrame,
    binding: &InputBindingDefinition,
) -> Option<InputActionState> {
    if !binding.device_path.eq_ignore_ascii_case("mouse/Position") {
        return None;
    }
    frame.pointer_position.map(|position| {
        InputActionState::pointer(binding.action_id.clone(), position.x, position.y)
    })
}

fn resolve_axis1(
    frame: &RuntimeInputFrame,
    binding: &InputBindingDefinition,
) -> Option<InputActionState> {
    if !binding.device_path.eq_ignore_ascii_case("mouse/Wheel") {
        return None;
    }
    let mut value = 0.0;
    for event in &frame.events {
        if let RuntimeInputEvent::MouseWheel { delta } = event {
            value += *delta;
        }
    }
    if value == 0.0 {
        return None;
    }
    Some(InputActionState::axis1(binding.action_id.clone(), value))
}

fn merge_action(existing: &mut InputActionState, incoming: &InputActionState) {
    match (&mut existing.value, &incoming.value) {
        (ActionValue::Axis2 { value }, ActionValue::Axis2 { value: incoming }) => {
            value.x = (value.x + incoming.x).clamp(-1.0, 1.0);
            value.y = (value.y + incoming.y).clamp(-1.0, 1.0);
        }
        _ => {
            *existing = incoming.clone();
        }
    }
}

fn apply_axis2_processor(value: Axis2, processor: &InputProcessorPreset) -> Axis2 {
    match processor {
        InputProcessorPreset::None => value,
        InputProcessorPreset::Invert => Axis2 {
            x: -value.x,
            y: -value.y,
        },
        InputProcessorPreset::Scale { factor } => Axis2 {
            x: value.x * factor,
            y: value.y * factor,
        },
        InputProcessorPreset::Deadzone { threshold } => {
            if value.x.abs() < *threshold && value.y.abs() < *threshold {
                Axis2 { x: 0.0, y: 0.0 }
            } else {
                value
            }
        }
        InputProcessorPreset::Normalize => {
            let length = (value.x * value.x + value.y * value.y).sqrt();
            if length == 0.0 {
                value
            } else {
                Axis2 {
                    x: value.x / length,
                    y: value.y / length,
                }
            }
        }
    }
}

fn input_identity(device_path: &str) -> String {
    device_path.to_ascii_lowercase()
}

#[cfg(test)]
mod runtime_text_ime_gamepad_tests {
    use super::*;

    #[test]
    fn text_input_and_ime_events_do_not_resolve_as_gameplay_actions() {
        let mut frame = RuntimeInputFrame::new(1, "game-view");
        frame.events = vec![
            RuntimeInputEvent::TextInput {
                text: "a".to_string(),
            },
            RuntimeInputEvent::ImePreedit {
                text: "ni".to_string(),
                cursor_start: 0,
                cursor_end: 2,
            },
            RuntimeInputEvent::ImeCommit {
                text: "nihao".to_string(),
            },
            RuntimeInputEvent::ImeCancel,
        ];
        let result = InputResolver::resolve(&frame, &InputMappingAsset::gameplay_default());

        assert_eq!(result.action_snapshot.action_count(), 0);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn gamepad_button_and_axis2_bindings_resolve_actions() {
        let mapping = InputMappingAsset::new(
            "input.gamepad",
            vec![
                InputActionDefinition::new("action.submit", InputActionValueType::Button),
                InputActionDefinition::new("action.move", InputActionValueType::Axis2),
            ],
            vec![InputContextDefinition::new("gameplay", 0)],
            vec![
                InputBindingDefinition::new("gameplay", "action.submit", "gamepad/South"),
                InputBindingDefinition::new("gameplay", "action.move", "gamepad/LeftStick"),
            ],
        );
        let mut frame = RuntimeInputFrame::new(1, "game-view");
        frame.events = vec![
            RuntimeInputEvent::GamepadButtonDown {
                gamepad_id: 0,
                button: "South".to_string(),
            },
            RuntimeInputEvent::GamepadAxis2d {
                gamepad_id: 0,
                axis: "LeftStick".to_string(),
                x: 0.5,
                y: -0.25,
            },
        ];
        let result = InputResolver::resolve(&frame, &mapping);

        assert!(result.action_snapshot.button_pressed("action.submit"));
        assert!(result
            .action_snapshot
            .axis2("action.move")
            .is_some_and(|axis| axis.x == 0.5 && axis.y == -0.25));
    }
}

fn is_supported_device_path(device_path: &str) -> bool {
    InputControlCatalog::supported().supports_device_path(device_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_mapping_asset_serializes() {
        let mapping = InputMappingAsset::gameplay_default();

        let json = serde_json::to_string(&mapping).expect("serialize mapping");
        let restored: InputMappingAsset = serde_json::from_str(&json).expect("deserialize mapping");

        assert_eq!(restored.schema_version, INPUT_MAPPING_SCHEMA_VERSION);
        assert_eq!(restored.asset_id, "input.default");
        assert!(restored
            .actions
            .iter()
            .any(|action| action.id == "action.fire"));
        assert!(restored
            .bindings
            .iter()
            .all(|binding| !binding.binding_id.is_empty()));
    }

    #[test]
    fn input_mapping_v1_migrates_binding_ids_deterministically() {
        let legacy = r#"{
            "schema_version":"input-mapping.v1",
            "asset_id":"input.legacy",
            "actions":[{"id":"action.fire","value_type":"Button"}],
            "contexts":[{"id":"gameplay","priority":0,"consume_input":false,"enabled_by_default":true}],
            "bindings":[
                {"context_id":"gameplay","action_id":"action.fire","device_path":"keyboard/Space"},
                {"context_id":"gameplay","action_id":"action.fire","device_path":"keyboard/Space"}
            ]
        }"#;

        let first: InputMappingAsset = serde_json::from_str(legacy).unwrap();
        let second: InputMappingAsset = serde_json::from_str(legacy).unwrap();

        assert_eq!(first.schema_version, INPUT_MAPPING_SCHEMA_VERSION);
        assert_eq!(first.bindings[0].binding_id, second.bindings[0].binding_id);
        assert_ne!(first.bindings[0].binding_id, first.bindings[1].binding_id);
        assert!(!first.validate().has_errors());
    }

    #[test]
    fn input_control_catalog_matches_runtime_supported_paths() {
        let catalog = InputControlCatalog::supported();

        assert!(catalog.supports_device_path("keyboard/Escape"));
        assert!(catalog.supports_device_path("mouse/Position"));
        assert!(catalog.supports_device_path("gamepad/South"));
        assert!(catalog.supports_device_path("gamepad/LeftStick"));
        assert!(!catalog.supports_device_path("gamepad/North"));
        assert!(
            !catalog
                .controls
                .iter()
                .find(|control| control.device_path == "gamepad/South")
                .unwrap()
                .capture_supported
        );
    }

    #[test]
    fn input_mapping_asset_validates_action_and_context_refs() {
        let mapping = InputMappingAsset::new(
            "input.invalid",
            vec![InputActionDefinition::new(
                "action.fire",
                InputActionValueType::Button,
            )],
            vec![InputContextDefinition::new("gameplay", 0)],
            vec![
                InputBindingDefinition::new("gameplay", "action.missing", "keyboard/Space"),
                InputBindingDefinition::new("missing", "action.fire", "keyboard/Space"),
            ],
        );

        let report = mapping.validate();

        assert!(report.has_errors());
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "input_mapping.unknown_action"));
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "input_mapping.unknown_context"));
    }

    #[test]
    fn input_mapping_rejects_unknown_device_path() {
        let mapping = InputMappingAsset::new(
            "input.invalid_device",
            vec![InputActionDefinition::new(
                "action.fire",
                InputActionValueType::Button,
            )],
            vec![InputContextDefinition::new("gameplay", 0)],
            vec![InputBindingDefinition::new(
                "gameplay",
                "action.fire",
                "flightstick/Trigger",
            )],
        );

        let report = mapping.validate();

        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "input_mapping.unsupported_device_path"));
    }

    #[test]
    fn input_mapping_reports_context_shadow_and_type_conflict() {
        let mapping = InputMappingAsset::new(
            "input.conflict",
            vec![
                InputActionDefinition::new("action.menu", InputActionValueType::Button),
                InputActionDefinition::new("action.gameplay", InputActionValueType::Button),
                InputActionDefinition::new("action.pointer.bad", InputActionValueType::Pointer),
            ],
            vec![
                InputContextDefinition::new("menu", 10).with_consume_input(true),
                InputContextDefinition::new("gameplay", 0),
            ],
            vec![
                InputBindingDefinition::new("menu", "action.menu", "keyboard/Space"),
                InputBindingDefinition::new("gameplay", "action.gameplay", "keyboard/Space"),
                InputBindingDefinition::new("gameplay", "action.pointer.bad", "mouse/Left"),
            ],
        );

        let report = mapping.validate();

        for code in [
            "input_mapping.hidden_by_higher_priority_consuming_context",
            "input_mapping.hides_lower_priority_binding",
            "input_mapping.action_value_type_device_mismatch",
        ] {
            assert!(
                report
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code == code),
                "missing diagnostic {code}"
            );
        }
        assert!(report.diagnostics.iter().all(|diagnostic| {
            diagnostic.code == "input_mapping.action_without_binding"
                || diagnostic.binding_id.is_some()
        }));
    }

    #[test]
    fn runtime_input_frame_keeps_events_in_order() {
        let mut frame = RuntimeInputFrame::new(1, "game-view");
        frame.events.push(RuntimeInputEvent::KeyDown {
            key: "Space".to_string(),
        });
        frame.events.push(RuntimeInputEvent::KeyUp {
            key: "Space".to_string(),
        });

        assert_eq!(frame.events[0].kind(), "KeyDown");
        assert_eq!(frame.events[1].kind(), "KeyUp");
    }

    #[test]
    fn runtime_pointer_event_reports_device_capability_and_phase() {
        let mouse = RuntimeInputEvent::Pointer {
            pointer: RuntimePointerEvent::mouse(
                RuntimePointerPhase::Down,
                0,
                12.0,
                24.0,
                Some(RuntimePointerButton::Primary),
            ),
        };
        let touch = RuntimeInputEvent::Pointer {
            pointer: RuntimePointerEvent::touch(RuntimePointerPhase::Move, 42, 30.0, 40.0),
        };

        assert_eq!(mouse.kind(), "PointerDown");
        assert!(mouse.pointer_event().unwrap().hover_capable);
        assert_eq!(touch.kind(), "PointerMove");
        let touch = touch.pointer_event().unwrap();
        assert_eq!(touch.device_kind, RuntimePointerDeviceKind::Touch);
        assert_eq!(touch.pointer_id, 42);
        assert!(!touch.hover_capable);

        let pen = RuntimePointerEvent::pen(RuntimePointerPhase::Move, 9, 50.0, 60.0, None, false);
        assert_eq!(pen.device_kind, RuntimePointerDeviceKind::Pen);
        assert!(!pen.hover_capable);
    }

    #[test]
    fn runtime_pointer_cancel_is_filtered_as_pointer_input() {
        let mut frame = RuntimeInputFrame::new(1, "game-view");
        frame.events.push(RuntimeInputEvent::Pointer {
            pointer: RuntimePointerEvent::touch(RuntimePointerPhase::Cancel, 7, 8.0, 9.0),
        });
        frame.events.push(RuntimeInputEvent::KeyDown {
            key: "Space".to_string(),
        });

        let filtered = frame.filter_consumed_events(&[0]);

        assert_eq!(filtered.events.len(), 1);
        assert_eq!(filtered.events[0].kind(), "KeyDown");
    }

    #[test]
    fn aui_input_filter_removes_only_consumed_pointer_events() {
        let mut frame = RuntimeInputFrame::new(1, "game-view");
        frame.events.push(RuntimeInputEvent::PointerDown {
            x: 32.0,
            y: 64.0,
            button: RuntimePointerButton::Primary,
        });
        frame.events.push(RuntimeInputEvent::KeyDown {
            key: "Space".to_string(),
        });
        frame
            .events
            .push(RuntimeInputEvent::MouseWheel { delta: -1.0 });
        frame
            .events
            .push(RuntimeInputEvent::PointerMove { x: 40.0, y: 72.0 });

        let filtered = frame.filter_consumed_pointer_events(&[0, 1, 2, 3]);

        assert_eq!(filtered.events.len(), 2);
        assert_eq!(filtered.events[0].kind(), "KeyDown");
        assert_eq!(filtered.events[1].kind(), "MouseWheel");
    }

    #[test]
    fn aui_input_filter_removes_consumed_pointer_wheel_and_key_events() {
        let mut frame = RuntimeInputFrame::new(1, "game-view");
        frame.events.push(RuntimeInputEvent::PointerDown {
            x: 32.0,
            y: 64.0,
            button: RuntimePointerButton::Primary,
        });
        frame.events.push(RuntimeInputEvent::KeyDown {
            key: "Space".to_string(),
        });
        frame
            .events
            .push(RuntimeInputEvent::MouseWheel { delta: -1.0 });
        frame
            .events
            .push(RuntimeInputEvent::PointerMove { x: 40.0, y: 72.0 });
        frame.events.push(RuntimeInputEvent::KeyUp {
            key: "Space".to_string(),
        });

        let filtered = frame.filter_consumed_events(&[0, 1, 2, 3, 4]);

        assert!(filtered.events.is_empty());
    }

    #[test]
    fn space_key_generates_fire_action_snapshot() {
        let mut frame = RuntimeInputFrame::new(1, "game-view");
        frame.events.push(RuntimeInputEvent::KeyDown {
            key: "Space".to_string(),
        });
        let mapping = InputMappingAsset::new(
            "input.test",
            vec![InputActionDefinition::new(
                "action.fire",
                InputActionValueType::Button,
            )],
            vec![InputContextDefinition::new("gameplay", 0)],
            vec![InputBindingDefinition::button("action.fire", "Space")],
        );

        let result = InputResolver::resolve(&frame, &mapping);

        assert!(!result
            .diagnostics
            .iter()
            .any(|d| d.severity == InputDiagnosticSeverity::Error));
        assert!(result.action_snapshot.button_pressed("action.fire"));
    }

    #[test]
    fn wasd_generates_move_axis2_action_snapshot() {
        let mut frame = RuntimeInputFrame::new(1, "game-view");
        frame.events.push(RuntimeInputEvent::KeyDown {
            key: "D".to_string(),
        });
        let mapping = InputMappingAsset::new(
            "input.test",
            vec![InputActionDefinition::new(
                "action.move",
                InputActionValueType::Axis2,
            )],
            vec![InputContextDefinition::new("gameplay", 0)],
            InputBindingDefinition::axis2_wasd("action.move"),
        );

        let result = InputResolver::resolve(&frame, &mapping);

        assert_eq!(
            result.action_snapshot.axis2("action.move"),
            Some(Axis2 { x: 1.0, y: 0.0 })
        );
    }

    #[test]
    fn wasd_combines_diagonal_axis2_action_snapshot() {
        let mut frame = RuntimeInputFrame::new(1, "game-view");
        frame.events.push(RuntimeInputEvent::KeyDown {
            key: "W".to_string(),
        });
        frame.events.push(RuntimeInputEvent::KeyDown {
            key: "D".to_string(),
        });
        let mapping = InputMappingAsset::new(
            "input.test",
            vec![InputActionDefinition::new(
                "action.move",
                InputActionValueType::Axis2,
            )],
            vec![InputContextDefinition::new("gameplay", 0)],
            InputBindingDefinition::axis2_wasd("action.move"),
        );

        let result = InputResolver::resolve(&frame, &mapping);

        assert_eq!(
            result.action_snapshot.axis2("action.move"),
            Some(Axis2 { x: 1.0, y: 1.0 })
        );
    }

    #[test]
    fn pointer_move_generates_pointer_action_snapshot() {
        let mut frame = RuntimeInputFrame::new(1, "game-view");
        frame
            .events
            .push(RuntimeInputEvent::PointerMove { x: 32.0, y: 64.0 });
        frame.pointer_position = Some(PointerPosition { x: 32.0, y: 64.0 });
        let mapping = InputMappingAsset::new(
            "input.test",
            vec![InputActionDefinition::new(
                "action.pointer",
                InputActionValueType::Pointer,
            )],
            vec![InputContextDefinition::new("gameplay", 0)],
            vec![InputBindingDefinition::pointer("action.pointer")],
        );

        let result = InputResolver::resolve(&frame, &mapping);

        assert_eq!(
            result.action_snapshot.pointer("action.pointer"),
            Some(PointerPosition { x: 32.0, y: 64.0 })
        );
    }

    #[test]
    fn context_priority_resolves_before_lower_priority() {
        let mut frame = RuntimeInputFrame::new(1, "game-view");
        frame.events.push(RuntimeInputEvent::KeyDown {
            key: "Space".to_string(),
        });
        let mapping = InputMappingAsset::new(
            "input.test",
            vec![
                InputActionDefinition::new("action.fire", InputActionValueType::Button),
                InputActionDefinition::new("action.ui_confirm", InputActionValueType::Button),
            ],
            vec![
                InputContextDefinition::new("gameplay", 0),
                InputContextDefinition::new("ui", 100).with_consume_input(true),
            ],
            vec![
                InputBindingDefinition::new("gameplay", "action.fire", "keyboard/Space"),
                InputBindingDefinition::new("ui", "action.ui_confirm", "keyboard/Space"),
            ],
        );

        let result = InputResolver::resolve(&frame, &mapping);

        assert!(result.action_snapshot.button_pressed("action.ui_confirm"));
        assert!(!result.action_snapshot.button_pressed("action.fire"));
    }

    #[test]
    fn resolve_result_reports_unknown_action() {
        let mut frame = RuntimeInputFrame::new(1, "game-view");
        frame.events.push(RuntimeInputEvent::KeyDown {
            key: "Space".to_string(),
        });
        let mapping = InputMappingAsset::new(
            "input.test",
            Vec::new(),
            vec![InputContextDefinition::new("gameplay", 0)],
            vec![InputBindingDefinition::new(
                "gameplay",
                "action.missing",
                "keyboard/Space",
            )],
        );

        let result = InputResolver::resolve(&frame, &mapping);

        assert!(result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "input_mapping.unknown_action"));
        assert_eq!(result.action_snapshot.action_count(), 0);
    }
}
