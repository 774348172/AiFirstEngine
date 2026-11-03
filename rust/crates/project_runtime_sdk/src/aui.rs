//! Typed AUI access contracts. These Rust interfaces stay inside a module;
//! generated ABI adapters must exchange validated data, never trait objects.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiNodeHandle {
    pub instance: u64,
    pub slot: u32,
    pub generation: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UiNodeType {
    Node,
    Text,
    Button,
    Slider,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UiProperty {
    Text,
    Translation,
    Interactable,
    Visibility,
    Active,
    Value,
    Rotation,
    Scale,
    Pivot,
    Opacity,
    Tint,
    Resource,
    LocalStyle,
}

impl UiProperty {
    /// Presentation changes never alter a node's layout occupation.
    pub fn is_presentation(self) -> bool {
        matches!(
            self,
            Self::Translation
                | Self::Rotation
                | Self::Scale
                | Self::Pivot
                | Self::Opacity
                | Self::Tint
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiResourceRef {
    pub asset_id: String,
    pub guid: String,
}

/// Paint-only overrides with linear normalized RGBA and straight alpha.
/// Font/layout changes use their existing authoring contracts.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiLocalStyle {
    pub background: Option<[f32; 4]>,
    pub foreground: Option<[f32; 4]>,
    pub border: Option<[f32; 4]>,
    pub radius: Option<f32>,
}

impl UiLocalStyle {
    pub fn validate(&self) -> bool {
        [self.background, self.foreground, self.border]
            .into_iter()
            .flatten()
            .all(|color| crate::aui_presentation::validate_rgba(color).is_ok())
            && self.radius.is_none_or(|v| v.is_finite() && v >= 0.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiPropertyOwner {
    Binding,
    Controller,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UiVisibility {
    Visible,
    Hidden,
    Collapsed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UiValue {
    Text(String),
    Translation([f32; 2]),
    Bool(bool),
    Visibility(UiVisibility),
    Number(f32),
    Vector([f32; 2]),
    Color([f32; 4]),
    Resource(Option<UiResourceRef>),
    LocalStyle(UiLocalStyle),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UiCallbackId(pub u64);

impl UiCallbackId {
    pub fn new(value: u64) -> Result<Self, UiError> {
        (value != 0).then_some(Self(value)).ok_or(UiError {
            kind: UiErrorKind::InvalidValue,
            handle: None,
            property: None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiChangeOrigin {
    User,
    Programmatic,
    Binding,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UiNotificationKind {
    Animation {
        handle: crate::aui_animation::AnimationHandle,
        outcome: crate::aui_animation::AnimationOutcome,
    },
    Mount,
    Enable,
    Disable,
    Dispose,
    Changed {
        property: UiProperty,
        old_value: UiValue,
        new_value: UiValue,
        origin: UiChangeOrigin,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiNotification {
    pub sequence: u64,
    pub target: UiNodeHandle,
    pub kind: UiNotificationKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiSubscriptionScope {
    Instance,
    Active,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "property", rename_all = "snake_case")]
pub enum UiEventFilter {
    All,
    Lifecycle,
    Changed(Option<UiProperty>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiSubscriptionToken {
    pub instance: u64,
    pub id: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiEventRequest {
    pub callback_id: UiCallbackId,
    pub notification: UiNotification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiErrorKind {
    AnimationConflict,
    InvalidHandle,
    WrongNodeType,
    WrongValueType,
    BindingOwned,
    BindingUnavailable,
    OwnershipValueRequired,
    InvalidValue,
    CapacityExceeded,
    Disposed,
    UnknownNode,
    Unavailable,
    TemplateMissing,
    HierarchyCycle,
    IncompatibleRoot,
    ControlledLayout,
    LayoutUnavailable,
    NonInvertibleTransform,
}

/// Compact failure context; formatting/source mapping belongs to the consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiError {
    pub kind: UiErrorKind,
    pub handle: Option<UiNodeHandle>,
    pub property: Option<UiProperty>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiReparentMode {
    KeepLocal,
    KeepScreen,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UiStructureOperation {
    Instantiate {
        template: String,
        parent: UiNodeHandle,
        sibling_index: Option<u32>,
    },
    Delete {
        target: UiNodeHandle,
    },
    Reparent {
        target: UiNodeHandle,
        new_parent: UiNodeHandle,
        sibling_index: Option<u32>,
        mode: UiReparentMode,
    },
    SetSiblingIndex {
        target: UiNodeHandle,
        sibling_index: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiStructureTransaction {
    pub version: u32,
    pub operations: Vec<UiStructureOperation>,
}

impl UiStructureTransaction {
    pub const VERSION: u32 = 1;

    pub fn new(operations: Vec<UiStructureOperation>) -> Self {
        Self {
            version: Self::VERSION,
            operations,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiTemplateNodeRef {
    pub local_id: String,
    pub handle: UiNodeHandle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiTemplateInstance {
    pub template_instance_id: u64,
    pub root: UiNodeHandle,
    pub nodes: Vec<UiTemplateNodeRef>,
}

impl UiTemplateInstance {
    pub fn node(&self, local_id: &str) -> Option<UiNodeRef> {
        self.nodes
            .iter()
            .find(|node| node.local_id == local_id)
            .map(|node| UiNodeRef::new(node.handle))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiStructureResult {
    pub instantiated: Vec<UiTemplateInstance>,
}

pub trait UiRead {
    fn node_type(&self, handle: UiNodeHandle) -> Result<UiNodeType, UiError>;
    fn read(&self, handle: UiNodeHandle, property: UiProperty) -> Result<UiValue, UiError>;
}

/// Resolves identities in the current session; handles may be kept, contexts may not.
pub trait UiAccess: UiRead {
    fn resolve(&self, document: &str, node: &str) -> Result<UiNodeHandle, UiError>;
}

pub trait UiEvents {
    fn subscribe(
        &self,
        target: UiNodeHandle,
        filter: UiEventFilter,
        scope: UiSubscriptionScope,
        callback_id: UiCallbackId,
    ) -> Result<UiSubscriptionToken, UiError>;

    fn cancel(&self, token: UiSubscriptionToken) -> Result<bool, UiError>;
}

pub trait UiStructureAccess {
    fn transact_structure(
        &self,
        transaction: UiStructureTransaction,
    ) -> Result<UiStructureResult, UiError>;
}

pub trait UiGeometryAccess {
    fn local_to_screen(&self, target: UiNodeHandle, point: [f32; 2]) -> Result<[f32; 2], UiError>;

    fn screen_to_local(&self, target: UiNodeHandle, point: [f32; 2]) -> Result<[f32; 2], UiError>;
}

pub trait UiInvalidationAccess {
    fn request_measure(&self, target: UiNodeHandle) -> Result<(), UiError>;
    fn request_repaint(&self, target: UiNodeHandle) -> Result<(), UiError>;
}

pub trait UiRuntimeAccess:
    UiAccess
    + UiEvents
    + UiStructureAccess
    + UiGeometryAccess
    + UiInvalidationAccess
    + crate::aui_wire::UiPropertyStage
    + crate::aui_animation::UiAnimationAccess
{
}

impl<T> UiRuntimeAccess for T where
    T: UiAccess
        + UiEvents
        + UiStructureAccess
        + UiGeometryAccess
        + UiInvalidationAccess
        + crate::aui_wire::UiPropertyStage
        + crate::aui_animation::UiAnimationAccess
        + ?Sized
{
}

/// Writes are staged in a callback/transaction; the owner chooses the commit boundary.
pub trait UiWrite: UiRead {
    fn write(
        &mut self,
        handle: UiNodeHandle,
        property: UiProperty,
        value: UiValue,
    ) -> Result<(), UiError>;

    fn write_without_notify(
        &mut self,
        handle: UiNodeHandle,
        property: UiProperty,
        value: UiValue,
    ) -> Result<(), UiError>;
}

pub trait UiOwnershipWrite {
    fn transfer_property_ownership(
        &mut self,
        handle: UiNodeHandle,
        property: UiProperty,
        owner: UiPropertyOwner,
    ) -> Result<(), UiError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiNodeRef(UiNodeHandle);

impl UiNodeRef {
    /// Handles are validated on every access, including references from generated code.
    pub fn new(handle: UiNodeHandle) -> Self {
        Self(handle)
    }

    pub fn handle(self) -> UiNodeHandle {
        self.0
    }

    pub fn take_property_ownership(
        self,
        ui: &mut impl UiOwnershipWrite,
        property: UiProperty,
    ) -> Result<(), UiError> {
        ui.transfer_property_ownership(self.0, property, UiPropertyOwner::Controller)
    }

    pub fn release_property_to_binding(
        self,
        ui: &mut impl UiOwnershipWrite,
        property: UiProperty,
    ) -> Result<(), UiError> {
        ui.transfer_property_ownership(self.0, property, UiPropertyOwner::Binding)
    }

    pub fn set_translation(self, ui: &mut impl UiWrite, value: [f32; 2]) -> Result<(), UiError> {
        ui.write(self.0, UiProperty::Translation, UiValue::Translation(value))
    }

    pub fn set_rotation(self, ui: &mut impl UiWrite, radians: f32) -> Result<(), UiError> {
        ui.write(self.0, UiProperty::Rotation, UiValue::Number(radians))
    }
    pub fn set_scale(self, ui: &mut impl UiWrite, value: [f32; 2]) -> Result<(), UiError> {
        ui.write(self.0, UiProperty::Scale, UiValue::Vector(value))
    }
    pub fn set_pivot(self, ui: &mut impl UiWrite, value: [f32; 2]) -> Result<(), UiError> {
        ui.write(self.0, UiProperty::Pivot, UiValue::Vector(value))
    }
    pub fn set_opacity(self, ui: &mut impl UiWrite, value: f32) -> Result<(), UiError> {
        ui.write(self.0, UiProperty::Opacity, UiValue::Number(value))
    }
    pub fn set_tint(self, ui: &mut impl UiWrite, value: [f32; 4]) -> Result<(), UiError> {
        ui.write(self.0, UiProperty::Tint, UiValue::Color(value))
    }
    pub fn set_resource(
        self,
        ui: &mut impl UiWrite,
        value: Option<UiResourceRef>,
    ) -> Result<(), UiError> {
        ui.write(self.0, UiProperty::Resource, UiValue::Resource(value))
    }
    pub fn set_local_style(
        self,
        ui: &mut impl UiWrite,
        value: UiLocalStyle,
    ) -> Result<(), UiError> {
        ui.write(self.0, UiProperty::LocalStyle, UiValue::LocalStyle(value))
    }

    pub fn set_translation_without_notify(
        self,
        ui: &mut impl UiWrite,
        value: [f32; 2],
    ) -> Result<(), UiError> {
        ui.write_without_notify(self.0, UiProperty::Translation, UiValue::Translation(value))
    }

    pub fn set_interactable(self, ui: &mut impl UiWrite, value: bool) -> Result<(), UiError> {
        ui.write(self.0, UiProperty::Interactable, UiValue::Bool(value))
    }

    pub fn set_interactable_without_notify(
        self,
        ui: &mut impl UiWrite,
        value: bool,
    ) -> Result<(), UiError> {
        ui.write_without_notify(self.0, UiProperty::Interactable, UiValue::Bool(value))
    }

    pub fn set_visibility(self, ui: &mut impl UiWrite, value: UiVisibility) -> Result<(), UiError> {
        ui.write(self.0, UiProperty::Visibility, UiValue::Visibility(value))
    }

    pub fn set_visibility_without_notify(
        self,
        ui: &mut impl UiWrite,
        value: UiVisibility,
    ) -> Result<(), UiError> {
        ui.write_without_notify(self.0, UiProperty::Visibility, UiValue::Visibility(value))
    }

    pub fn set_active(self, ui: &mut impl UiWrite, value: bool) -> Result<(), UiError> {
        ui.write(self.0, UiProperty::Active, UiValue::Bool(value))
    }

    pub fn set_active_without_notify(
        self,
        ui: &mut impl UiWrite,
        value: bool,
    ) -> Result<(), UiError> {
        ui.write_without_notify(self.0, UiProperty::Active, UiValue::Bool(value))
    }

    pub fn local_to_screen(
        self,
        ui: &impl UiGeometryAccess,
        point: [f32; 2],
    ) -> Result<[f32; 2], UiError> {
        ui.local_to_screen(self.0, point)
    }

    pub fn screen_to_local(
        self,
        ui: &impl UiGeometryAccess,
        point: [f32; 2],
    ) -> Result<[f32; 2], UiError> {
        ui.screen_to_local(self.0, point)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiTextRef(UiNodeRef);

impl UiTextRef {
    pub fn try_new(ui: &impl UiRead, handle: UiNodeHandle) -> Result<Self, UiError> {
        require_type(ui, handle, UiNodeType::Text)?;
        Ok(Self(UiNodeRef(handle)))
    }

    pub fn node(self) -> UiNodeRef {
        self.0
    }

    pub fn set_text(self, ui: &mut impl UiWrite, value: impl Into<String>) -> Result<(), UiError> {
        ui.write(self.0 .0, UiProperty::Text, UiValue::Text(value.into()))
    }

    pub fn set_text_without_notify(
        self,
        ui: &mut impl UiWrite,
        value: impl Into<String>,
    ) -> Result<(), UiError> {
        ui.write_without_notify(self.0 .0, UiProperty::Text, UiValue::Text(value.into()))
    }

    pub fn text(self, ui: &impl UiRead) -> Result<String, UiError> {
        match ui.read(self.0 .0, UiProperty::Text)? {
            UiValue::Text(value) => Ok(value),
            _ => Err(UiError {
                kind: UiErrorKind::WrongValueType,
                handle: Some(self.0 .0),
                property: Some(UiProperty::Text),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiButtonRef(UiNodeRef);

impl UiButtonRef {
    pub fn try_new(ui: &impl UiRead, handle: UiNodeHandle) -> Result<Self, UiError> {
        require_type(ui, handle, UiNodeType::Button)?;
        Ok(Self(UiNodeRef(handle)))
    }

    pub fn node(self) -> UiNodeRef {
        self.0
    }

    pub fn set_interactable(self, ui: &mut impl UiWrite, value: bool) -> Result<(), UiError> {
        self.0.set_interactable(ui, value)
    }

    pub fn set_interactable_without_notify(
        self,
        ui: &mut impl UiWrite,
        value: bool,
    ) -> Result<(), UiError> {
        self.0.set_interactable_without_notify(ui, value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiSliderRef(UiNodeRef);

impl UiSliderRef {
    pub fn try_new(ui: &impl UiRead, handle: UiNodeHandle) -> Result<Self, UiError> {
        require_type(ui, handle, UiNodeType::Slider)?;
        Ok(Self(UiNodeRef(handle)))
    }

    pub fn node(self) -> UiNodeRef {
        self.0
    }

    pub fn set_value(self, ui: &mut impl UiWrite, value: f32) -> Result<(), UiError> {
        ui.write(self.0 .0, UiProperty::Value, UiValue::Number(value))
    }

    pub fn set_value_without_notify(
        self,
        ui: &mut impl UiWrite,
        value: f32,
    ) -> Result<(), UiError> {
        ui.write_without_notify(self.0 .0, UiProperty::Value, UiValue::Number(value))
    }

    pub fn value(self, ui: &impl UiRead) -> Result<f32, UiError> {
        match ui.read(self.0 .0, UiProperty::Value)? {
            UiValue::Number(value) if value.is_finite() => Ok(value),
            UiValue::Number(_) => Err(UiError {
                kind: UiErrorKind::InvalidValue,
                handle: Some(self.0 .0),
                property: Some(UiProperty::Value),
            }),
            _ => Err(UiError {
                kind: UiErrorKind::WrongValueType,
                handle: Some(self.0 .0),
                property: Some(UiProperty::Value),
            }),
        }
    }
}

fn require_type(
    ui: &impl UiRead,
    handle: UiNodeHandle,
    expected: UiNodeType,
) -> Result<(), UiError> {
    if ui.node_type(handle)? == expected {
        Ok(())
    } else {
        Err(UiError {
            kind: UiErrorKind::WrongNodeType,
            handle: Some(handle),
            property: None,
        })
    }
}
