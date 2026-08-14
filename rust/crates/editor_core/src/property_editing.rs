use editor_ui_model::{
    ui_command_id_for_payload, EditorAssetRef, InspectorField, InspectorModel,
    InspectorPersistence, InspectorValue, InspectorValueType, UiCommand, UiCommandPayload,
    UiCommandSource, Vec3,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyPath {
    value: String,
}

impl PropertyPath {
    pub fn parse(value: impl Into<String>) -> Result<Self, PropertyEditDiagnostic> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(PropertyEditDiagnostic::error(
                "property.path.empty",
                "Property path cannot be empty.",
            ));
        }
        if trimmed.contains("..")
            || trimmed
                .chars()
                .any(|ch| !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.')))
        {
            return Err(PropertyEditDiagnostic::error(
                "property.path.invalid",
                format!("Unsupported property path: {trimmed}"),
            ));
        }
        Ok(Self {
            value: trimmed.to_string(),
        })
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub fn segments(&self) -> Vec<&str> {
        self.value.split('.').collect()
    }
}

impl std::fmt::Display for PropertyPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PropertyValueType {
    String,
    Bool,
    Number,
    Vec2,
    Vec3,
    Vec4,
    Color,
    Enum,
    AssetRef,
    EntityRef,
    Array,
    Object,
    Curve,
    RichText,
    Json,
    Empty,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PropertyValue {
    String(String),
    Bool(bool),
    Number(f64),
    Vec3(Vec3),
    Color(PropertyColor),
    AssetRef(EditorAssetRef),
    EntityRef(String),
    Array(Vec<PropertyValue>),
    Object(Vec<PropertyNode>),
    Curve(PropertyCurve),
    RichText(PropertyRichText),
    Json(serde_json::Value),
    Empty,
}

impl PropertyValue {
    pub fn value_type(&self) -> PropertyValueType {
        match self {
            Self::String(_) => PropertyValueType::String,
            Self::Bool(_) => PropertyValueType::Bool,
            Self::Number(_) => PropertyValueType::Number,
            Self::Vec3(_) => PropertyValueType::Vec3,
            Self::Color(_) => PropertyValueType::Color,
            Self::AssetRef(_) => PropertyValueType::AssetRef,
            Self::EntityRef(_) => PropertyValueType::EntityRef,
            Self::Array(_) => PropertyValueType::Array,
            Self::Object(_) => PropertyValueType::Object,
            Self::Curve(_) => PropertyValueType::Curve,
            Self::RichText(_) => PropertyValueType::RichText,
            Self::Json(_) => PropertyValueType::Json,
            Self::Empty => PropertyValueType::Empty,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PropertyColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropertyCurve {
    pub keys: Vec<PropertyCurveKey>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PropertyCurveKey {
    pub time: f32,
    pub value: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyRichText {
    pub plain_text: String,
    pub spans: Vec<PropertyRichTextSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyRichTextSpan {
    pub start: usize,
    pub end: usize,
    pub style: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PropertyEditorKind {
    Text,
    MultilineRichText,
    Number,
    Slider,
    Toggle,
    Vec2,
    Vec3,
    Vec4,
    ColorPicker,
    Enum,
    AssetRefPicker,
    EntityRefPicker,
    Array,
    Object,
    Curve,
    Json,
    Custom,
    Readonly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyMetadata {
    pub label: String,
    pub readonly: bool,
    pub editable: bool,
    pub component_type: Option<String>,
    pub field_path: Option<String>,
    pub custom_plugin_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropertyNode {
    pub node_id: String,
    pub path: PropertyPath,
    pub value: PropertyValue,
    pub value_type: PropertyValueType,
    pub editor_kind: PropertyEditorKind,
    pub metadata: PropertyMetadata,
    pub children: Vec<PropertyNode>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PropertyTree {
    pub selected_entity_id: Option<String>,
    pub persistence: InspectorPersistence,
    pub nodes: Vec<PropertyNode>,
}

impl PropertyTree {
    pub fn from_inspector_model(model: &InspectorModel) -> Self {
        let nodes = model
            .sections
            .iter()
            .flat_map(|section| {
                section
                    .fields
                    .iter()
                    .filter_map(|field| property_node_from_inspector_field(field).ok())
            })
            .collect();
        Self {
            selected_entity_id: model.selected_entity_id.clone(),
            persistence: model.persistence,
            nodes,
        }
    }

    pub fn find(&self, path: &PropertyPath) -> Option<&PropertyNode> {
        self.nodes.iter().find(|node| &node.path == path)
    }

    pub fn summary(&self) -> PropertyTreeSummary {
        PropertyTreeSummary {
            selected_entity_id: self.selected_entity_id.clone(),
            editable_count: self
                .nodes
                .iter()
                .filter(|node| node.metadata.editable && !node.metadata.readonly)
                .count(),
            readonly_count: self
                .nodes
                .iter()
                .filter(|node| node.metadata.readonly)
                .count(),
            property_count: self.nodes.len(),
            editable_paths: self
                .nodes
                .iter()
                .filter(|node| node.metadata.editable && !node.metadata.readonly)
                .map(|node| node.path.to_string())
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyTreeSummary {
    pub selected_entity_id: Option<String>,
    pub property_count: usize,
    pub editable_count: usize,
    pub readonly_count: usize,
    pub editable_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InspectorPluginDescriptor {
    pub plugin_id: String,
    pub target_component_type: Option<String>,
    pub target_path_prefix: Option<String>,
    pub editor_kind: PropertyEditorKind,
    pub allowed_commands: Vec<PropertyEditCommandKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PropertyEditCommandKind {
    SetValue,
    InsertArrayElement,
    RemoveArrayElement,
    MoveArrayElement,
    AddCurveKey,
    RemoveCurveKey,
    MoveCurveKey,
    SetCurveKeyValue,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextCompositionState {
    pub preedit_text: String,
    pub committed_text: String,
    pub active: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RichTextBuffer {
    pub plain_text: String,
    pub spans: Vec<PropertyRichTextSpan>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropertyEditBuffer {
    pub focused_path: Option<PropertyPath>,
    pub draft_text: String,
    pub rich_text: Option<RichTextBuffer>,
    pub composition: TextCompositionState,
    pub dirty: bool,
}

impl Default for PropertyEditBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl PropertyEditBuffer {
    pub fn new() -> Self {
        Self {
            focused_path: None,
            draft_text: String::new(),
            rich_text: None,
            composition: TextCompositionState::default(),
            dirty: false,
        }
    }

    pub fn begin_edit(&mut self, node: &PropertyNode) {
        self.focused_path = Some(node.path.clone());
        self.draft_text = property_value_to_edit_text(&node.value);
        self.rich_text = match &node.value {
            PropertyValue::RichText(value) => Some(RichTextBuffer {
                plain_text: value.plain_text.clone(),
                spans: value.spans.clone(),
            }),
            _ => None,
        };
        self.composition = TextCompositionState::default();
        self.dirty = false;
    }

    pub fn input_text(&mut self, text: &str) {
        self.draft_text.push_str(text);
        self.dirty = true;
    }

    pub fn replace_text(&mut self, text: impl Into<String>) {
        self.draft_text = text.into();
        self.dirty = true;
    }

    pub fn update_composition(&mut self, preedit_text: impl Into<String>) {
        self.composition.preedit_text = preedit_text.into();
        self.composition.active = true;
    }

    pub fn commit_composition(&mut self, committed_text: impl Into<String>) {
        let committed_text = committed_text.into();
        self.draft_text.push_str(&committed_text);
        self.composition.committed_text = committed_text;
        self.composition.preedit_text.clear();
        self.composition.active = false;
        self.dirty = true;
    }

    pub fn commit(
        &mut self,
        tree: &PropertyTree,
    ) -> Result<PropertyEditCommitReport, PropertyEditDiagnostic> {
        let path = self.focused_path.clone().ok_or_else(|| {
            PropertyEditDiagnostic::error(
                "property.edit.no_focus",
                "Cannot commit property edit without focused property.",
            )
        })?;
        let node = tree.find(&path).ok_or_else(|| {
            PropertyEditDiagnostic::error(
                "property.edit.node_missing",
                format!("Cannot commit missing property: {path}"),
            )
        })?;
        let value = parse_property_value(&self.draft_text, node.value_type)?;
        let command = PropertyEditCommand::SetValue {
            target: PropertyEditTarget {
                entity_id: tree.selected_entity_id.clone(),
                persistence: tree.persistence,
                path: path.clone(),
                component_type: node.metadata.component_type.clone(),
                field_path: node.metadata.field_path.clone(),
            },
            value,
        };
        self.dirty = false;
        Ok(PropertyEditCommitReport {
            status: PropertyEditCommitStatus::Committed,
            command: Some(command),
            diagnostics: Vec::new(),
        })
    }

    pub fn cancel(&mut self) -> PropertyEditCommitReport {
        self.focused_path = None;
        self.draft_text.clear();
        self.rich_text = None;
        self.composition = TextCompositionState::default();
        self.dirty = false;
        PropertyEditCommitReport {
            status: PropertyEditCommitStatus::Cancelled,
            command: None,
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropertyEditTarget {
    pub entity_id: Option<String>,
    pub persistence: InspectorPersistence,
    pub path: PropertyPath,
    pub component_type: Option<String>,
    pub field_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PropertyEditCommand {
    SetValue {
        target: PropertyEditTarget,
        value: PropertyValue,
    },
    InsertArrayElement {
        target: PropertyEditTarget,
        index: usize,
        value: PropertyValue,
    },
    RemoveArrayElement {
        target: PropertyEditTarget,
        index: usize,
    },
    MoveArrayElement {
        target: PropertyEditTarget,
        from: usize,
        to: usize,
    },
    AddCurveKey {
        target: PropertyEditTarget,
        key: PropertyCurveKey,
    },
    RemoveCurveKey {
        target: PropertyEditTarget,
        index: usize,
    },
    MoveCurveKey {
        target: PropertyEditTarget,
        index: usize,
        time: f32,
    },
    SetCurveKeyValue {
        target: PropertyEditTarget,
        index: usize,
        value: f32,
    },
}

impl PropertyEditCommand {
    pub fn to_ui_command(
        &self,
        request_id: impl Into<String>,
    ) -> Result<UiCommand, PropertyEditDiagnostic> {
        match self {
            Self::SetValue { target, value } => {
                let entity_id = target.entity_id.clone().ok_or_else(|| {
                    PropertyEditDiagnostic::error(
                        "property.edit.entity_required",
                        "Property edit requires selected entity.",
                    )
                })?;
                if target.persistence == InspectorPersistence::TemporaryPlaySession {
                    let (component_type, field_path) =
                        runtime_component_mapping_for_target(target)?;
                    let payload = UiCommandPayload::SetRuntimeComponentFieldTemporary {
                        entity_id,
                        component_type,
                        field_path,
                        value: property_value_to_json(value),
                    };
                    return Ok(UiCommand {
                        command_id: property_command_id_for_payload(&payload).to_string(),
                        source: UiCommandSource::Inspector,
                        request_id: request_id.into(),
                        payload,
                    });
                }
                if target.persistence != InspectorPersistence::PersistentAuthoring {
                    return Err(PropertyEditDiagnostic::error(
                        "property.edit.persistence_readonly",
                        "This Inspector value is not writable through persistent authoring edits.",
                    ));
                }
                let payload = if target.path.as_str() == "transform.localPosition" {
                    UiCommandPayload::SetSceneTransform {
                        entity_id,
                        local_position: Some(property_value_to_vec3(value)?),
                        local_rotation: None,
                        local_scale: None,
                    }
                } else if target.path.as_str() == "transform.localRotation" {
                    UiCommandPayload::SetSceneTransform {
                        entity_id,
                        local_position: None,
                        local_rotation: Some(property_value_to_vec3(value)?),
                        local_scale: None,
                    }
                } else if target.path.as_str() == "transform.localScale" {
                    UiCommandPayload::SetSceneTransform {
                        entity_id,
                        local_position: None,
                        local_rotation: None,
                        local_scale: Some(property_value_to_vec3(value)?),
                    }
                } else if let (Some(component_type), Some(field_path)) =
                    (&target.component_type, &target.field_path)
                {
                    UiCommandPayload::SetSceneComponentField {
                        entity_id,
                        component_type: component_type.clone(),
                        field_path: field_path.clone(),
                        value: property_value_to_json(value),
                    }
                } else {
                    return Err(PropertyEditDiagnostic::error(
                        "property.edit.path_unsupported",
                        format!("Unsupported property edit path: {}", target.path),
                    ));
                };
                Ok(UiCommand {
                    command_id: property_command_id_for_payload(&payload).to_string(),
                    source: UiCommandSource::Inspector,
                    request_id: request_id.into(),
                    payload,
                })
            }
            _ => Err(PropertyEditDiagnostic::error(
                "property.edit.command_not_mapped",
                "Only SetValue property commands are mapped to UiCommand in the first pass.",
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PropertyEditCommitStatus {
    Committed,
    Cancelled,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropertyEditCommitReport {
    pub status: PropertyEditCommitStatus,
    pub command: Option<PropertyEditCommand>,
    pub diagnostics: Vec<PropertyEditDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyEditDiagnostic {
    pub severity: PropertyEditDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
}

impl PropertyEditDiagnostic {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: PropertyEditDiagnosticSeverity::Error,
            code: code.into(),
            message: message.into(),
            path: None,
        }
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PropertyEditDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

pub fn property_node_from_inspector_field(
    field: &InspectorField,
) -> Result<PropertyNode, PropertyEditDiagnostic> {
    let path = PropertyPath::parse(field.path.clone())?;
    let value = property_value_from_inspector_value(&field.value);
    let value_type = property_value_type_from_inspector(field.value_type.clone(), &value);
    let (component_type, field_path) = component_mapping_for_path(path.as_str());
    Ok(PropertyNode {
        node_id: field.field_id.clone(),
        path,
        value,
        value_type,
        editor_kind: editor_kind_for_field(field, value_type),
        metadata: PropertyMetadata {
            label: field.label.clone(),
            readonly: field.readonly,
            editable: field.editable,
            component_type,
            field_path,
            custom_plugin_id: None,
        },
        children: Vec::new(),
    })
}

fn property_value_from_inspector_value(value: &InspectorValue) -> PropertyValue {
    match value {
        InspectorValue::String(value) => PropertyValue::String(value.clone()),
        InspectorValue::Bool(value) => PropertyValue::Bool(*value),
        InspectorValue::Number(value) => PropertyValue::Number(*value),
        InspectorValue::Vec3(value) => PropertyValue::Vec3(*value),
        InspectorValue::AssetRef(value) => PropertyValue::AssetRef(value.clone()),
        InspectorValue::EntityRef(value) => PropertyValue::EntityRef(value.clone()),
        InspectorValue::Json(value) => json_to_property_value(value),
        InspectorValue::Empty => PropertyValue::Empty,
    }
}

fn property_value_type_from_inspector(
    value_type: InspectorValueType,
    value: &PropertyValue,
) -> PropertyValueType {
    match value_type {
        InspectorValueType::String => PropertyValueType::String,
        InspectorValueType::Bool => PropertyValueType::Bool,
        InspectorValueType::Number => PropertyValueType::Number,
        InspectorValueType::Vec3 => PropertyValueType::Vec3,
        InspectorValueType::AssetRef => PropertyValueType::AssetRef,
        InspectorValueType::EntityRef => PropertyValueType::EntityRef,
        InspectorValueType::Json => value.value_type(),
        InspectorValueType::Empty => PropertyValueType::Empty,
    }
}

fn json_to_property_value(value: &serde_json::Value) -> PropertyValue {
    match value {
        serde_json::Value::Bool(value) => PropertyValue::Bool(*value),
        serde_json::Value::Number(value) => PropertyValue::Number(value.as_f64().unwrap_or(0.0)),
        serde_json::Value::String(value) => PropertyValue::String(value.clone()),
        serde_json::Value::Array(values) => {
            PropertyValue::Array(values.iter().map(json_to_property_value).collect())
        }
        serde_json::Value::Object(values) => PropertyValue::Object(
            values
                .iter()
                .filter_map(|(key, value)| {
                    let path = PropertyPath::parse(key.clone()).ok()?;
                    let property_value = json_to_property_value(value);
                    Some(PropertyNode {
                        node_id: key.clone(),
                        path,
                        value: property_value.clone(),
                        value_type: property_value.value_type(),
                        editor_kind: editor_kind_for_value_type(property_value.value_type(), false),
                        metadata: PropertyMetadata {
                            label: key.clone(),
                            readonly: false,
                            editable: true,
                            component_type: None,
                            field_path: Some(key.clone()),
                            custom_plugin_id: None,
                        },
                        children: Vec::new(),
                    })
                })
                .collect(),
        ),
        serde_json::Value::Null => PropertyValue::Empty,
    }
}

fn editor_kind_for_field(
    field: &InspectorField,
    value_type: PropertyValueType,
) -> PropertyEditorKind {
    editor_kind_for_value_type(value_type, field.readonly || !field.editable)
}

fn editor_kind_for_value_type(value_type: PropertyValueType, readonly: bool) -> PropertyEditorKind {
    if readonly {
        return PropertyEditorKind::Readonly;
    }
    match value_type {
        PropertyValueType::String => PropertyEditorKind::Text,
        PropertyValueType::Bool => PropertyEditorKind::Toggle,
        PropertyValueType::Number => PropertyEditorKind::Number,
        PropertyValueType::Vec2 => PropertyEditorKind::Json,
        PropertyValueType::Vec3 => PropertyEditorKind::Vec3,
        PropertyValueType::Vec4 => PropertyEditorKind::Json,
        PropertyValueType::Color => PropertyEditorKind::ColorPicker,
        PropertyValueType::Enum => PropertyEditorKind::Enum,
        PropertyValueType::AssetRef => PropertyEditorKind::AssetRefPicker,
        PropertyValueType::EntityRef => PropertyEditorKind::EntityRefPicker,
        PropertyValueType::Array => PropertyEditorKind::Array,
        PropertyValueType::Object => PropertyEditorKind::Object,
        PropertyValueType::Curve => PropertyEditorKind::Curve,
        PropertyValueType::RichText => PropertyEditorKind::MultilineRichText,
        PropertyValueType::Json => PropertyEditorKind::Json,
        PropertyValueType::Empty => PropertyEditorKind::Readonly,
    }
}

fn component_mapping_for_path(path: &str) -> (Option<String>, Option<String>) {
    if let Some(field_path) = path.strip_prefix("renderable.") {
        return (Some("Renderable".to_string()), Some(field_path.to_string()));
    }
    if let Some(field_path) = path.strip_prefix("spriteRenderer2D.") {
        return (
            Some("SpriteRenderer2D".to_string()),
            Some(field_path.to_string()),
        );
    }
    if let Some(field_path) = path.strip_prefix("collider2D.") {
        return (Some("Collider2D".to_string()), Some(field_path.to_string()));
    }
    let Some(rest) = path.strip_prefix("components.") else {
        return (None, None);
    };
    let mut parts = rest.split('.');
    let Some(component_type) = parts.next() else {
        return (None, None);
    };
    let Some(field_path) = parts.next() else {
        return (Some(component_type.to_string()), None);
    };
    if parts.next().is_some() {
        return (Some(component_type.to_string()), None);
    }
    (
        Some(component_type.to_string()),
        Some(field_path.to_string()),
    )
}

fn runtime_component_mapping_for_target(
    target: &PropertyEditTarget,
) -> Result<(String, String), PropertyEditDiagnostic> {
    let path = target.path.as_str();
    if path == "transform.localPosition" {
        return Ok(("Transform".to_string(), "local_position".to_string()));
    }
    if path == "transform.localRotation" {
        return Ok(("Transform".to_string(), "local_rotation".to_string()));
    }
    if path == "transform.localScale" {
        return Ok(("Transform".to_string(), "local_scale".to_string()));
    }
    if let (Some(component_type), Some(field_path)) = (&target.component_type, &target.field_path) {
        return Ok((component_type.clone(), field_path.clone()));
    }
    Err(PropertyEditDiagnostic::error(
        "property.edit.runtime_path_unsupported",
        format!("Unsupported runtime temporary edit path: {}", target.path),
    ))
}

fn parse_property_value(
    text: &str,
    value_type: PropertyValueType,
) -> Result<PropertyValue, PropertyEditDiagnostic> {
    match value_type {
        PropertyValueType::String => Ok(PropertyValue::String(text.to_string())),
        PropertyValueType::Bool => text.parse::<bool>().map(PropertyValue::Bool).map_err(|_| {
            PropertyEditDiagnostic::error("property.value.bool_invalid", "Invalid bool value.")
        }),
        PropertyValueType::Number => text.parse::<f64>().map(PropertyValue::Number).map_err(|_| {
            PropertyEditDiagnostic::error("property.value.number_invalid", "Invalid number value.")
        }),
        PropertyValueType::Vec3 => parse_vec3(text).map(PropertyValue::Vec3),
        PropertyValueType::AssetRef => serde_json::from_str::<EditorAssetRef>(text)
            .or_else(|_| Ok::<EditorAssetRef, serde_json::Error>(EditorAssetRef::legacy(text)))
            .map(PropertyValue::AssetRef)
            .map_err(|_| {
                PropertyEditDiagnostic::error(
                    "property.value.asset_ref_invalid",
                    "Invalid structured asset reference.",
                )
            }),
        PropertyValueType::EntityRef => Ok(PropertyValue::EntityRef(text.to_string())),
        PropertyValueType::Json
        | PropertyValueType::Vec2
        | PropertyValueType::Vec4
        | PropertyValueType::Enum
        | PropertyValueType::Array
        | PropertyValueType::Object
        | PropertyValueType::Curve
        | PropertyValueType::RichText
        | PropertyValueType::Color
        | PropertyValueType::Empty => serde_json::from_str::<serde_json::Value>(text)
            .map(|value| match value_type {
                PropertyValueType::Color => json_to_color(&value)
                    .map(PropertyValue::Color)
                    .unwrap_or(PropertyValue::Json(value)),
                PropertyValueType::RichText => PropertyValue::RichText(PropertyRichText {
                    plain_text: text.to_string(),
                    spans: Vec::new(),
                }),
                _ => json_to_property_value(&value),
            })
            .map_err(|_| {
                PropertyEditDiagnostic::error(
                    "property.value.json_invalid",
                    "Invalid structured property value.",
                )
            }),
    }
}

fn parse_vec3(text: &str) -> Result<Vec3, PropertyEditDiagnostic> {
    let normalized = text
        .trim()
        .trim_start_matches('(')
        .trim_start_matches('[')
        .trim_end_matches(')')
        .trim_end_matches(']');
    let parts = normalized.split(',').map(str::trim).collect::<Vec<_>>();
    if parts.len() != 3 {
        return Err(PropertyEditDiagnostic::error(
            "property.value.vec3_invalid",
            "Vec3 requires three comma-separated values.",
        ));
    }
    Ok(Vec3 {
        x: parts[0].parse::<f32>().map_err(|_| {
            PropertyEditDiagnostic::error("property.value.vec3_invalid", "Invalid Vec3 x value.")
        })?,
        y: parts[1].parse::<f32>().map_err(|_| {
            PropertyEditDiagnostic::error("property.value.vec3_invalid", "Invalid Vec3 y value.")
        })?,
        z: parts[2].parse::<f32>().map_err(|_| {
            PropertyEditDiagnostic::error("property.value.vec3_invalid", "Invalid Vec3 z value.")
        })?,
    })
}

fn property_value_to_edit_text(value: &PropertyValue) -> String {
    match value {
        PropertyValue::String(value) => value.clone(),
        PropertyValue::Bool(value) => value.to_string(),
        PropertyValue::Number(value) => value.to_string(),
        PropertyValue::Vec3(value) => format!("{},{},{}", value.x, value.y, value.z),
        PropertyValue::AssetRef(value) => value.asset_id.clone(),
        PropertyValue::EntityRef(value) => value.clone(),
        PropertyValue::RichText(value) => value.plain_text.clone(),
        PropertyValue::Empty => String::new(),
        _ => serde_json::to_string(&property_value_to_json(value)).unwrap_or_default(),
    }
}

fn property_value_to_vec3(value: &PropertyValue) -> Result<Vec3, PropertyEditDiagnostic> {
    match value {
        PropertyValue::Vec3(value) => Ok(*value),
        _ => Err(PropertyEditDiagnostic::error(
            "property.value.vec3_required",
            "Transform field requires Vec3 value.",
        )),
    }
}

fn property_value_to_json(value: &PropertyValue) -> serde_json::Value {
    match value {
        PropertyValue::String(value) => serde_json::Value::String(value.clone()),
        PropertyValue::Bool(value) => serde_json::Value::Bool(*value),
        PropertyValue::Number(value) => serde_json::json!(value),
        PropertyValue::Vec3(value) => {
            serde_json::json!({ "x": value.x, "y": value.y, "z": value.z })
        }
        PropertyValue::Color(value) => {
            serde_json::json!({ "r": value.r, "g": value.g, "b": value.b, "a": value.a })
        }
        PropertyValue::AssetRef(value) => serde_json::to_value(value).unwrap_or_default(),
        PropertyValue::EntityRef(value) => serde_json::Value::String(value.clone()),
        PropertyValue::Array(values) => {
            serde_json::Value::Array(values.iter().map(property_value_to_json).collect())
        }
        PropertyValue::Object(nodes) => serde_json::Value::Object(
            nodes
                .iter()
                .map(|node| {
                    (
                        node.metadata.label.clone(),
                        property_value_to_json(&node.value),
                    )
                })
                .collect(),
        ),
        PropertyValue::Curve(value) => serde_json::json!({
            "keys": value.keys.iter().map(|key| serde_json::json!({
                "time": key.time,
                "value": key.value,
            })).collect::<Vec<_>>()
        }),
        PropertyValue::RichText(value) => serde_json::json!({
            "plainText": value.plain_text,
            "spans": value.spans,
        }),
        PropertyValue::Json(value) => value.clone(),
        PropertyValue::Empty => serde_json::Value::Null,
    }
}

fn json_to_color(value: &serde_json::Value) -> Option<PropertyColor> {
    let object = value.as_object()?;
    Some(PropertyColor {
        r: object.get("r")?.as_f64()? as f32,
        g: object.get("g")?.as_f64()? as f32,
        b: object.get("b")?.as_f64()? as f32,
        a: object
            .get("a")
            .and_then(|value| value.as_f64())
            .unwrap_or(1.0) as f32,
    })
}

fn property_command_id_for_payload(payload: &UiCommandPayload) -> &'static str {
    ui_command_id_for_payload(payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use editor_ui_model::{InspectorSection, InspectorValue};

    fn inspector_model() -> InspectorModel {
        InspectorModel {
            selected_entity_id: Some("entity-player".to_string()),
            title: "Player".to_string(),
            readonly: false,
            persistence: InspectorPersistence::PersistentAuthoring,
            sections: vec![
                InspectorSection {
                    section_id: "transform".to_string(),
                    title: "Transform".to_string(),
                    fields: vec![InspectorField {
                        field_id: "transform.localPosition".to_string(),
                        label: "localPosition".to_string(),
                        value: InspectorValue::Vec3(Vec3 {
                            x: 0.0,
                            y: 1.0,
                            z: 0.0,
                        }),
                        value_type: InspectorValueType::Vec3,
                        path: "transform.localPosition".to_string(),
                        readonly: false,
                        editable: true,
                    }],
                },
                InspectorSection {
                    section_id: "SpriteRenderer2D".to_string(),
                    title: "SpriteRenderer2D".to_string(),
                    fields: vec![InspectorField {
                        field_id: "components.SpriteRenderer2D.visible".to_string(),
                        label: "visible".to_string(),
                        value: InspectorValue::Bool(true),
                        value_type: InspectorValueType::Bool,
                        path: "components.SpriteRenderer2D.visible".to_string(),
                        readonly: false,
                        editable: true,
                    }],
                },
            ],
        }
    }

    #[test]
    fn property_editing_path_parses_basic_paths() {
        let path = PropertyPath::parse("transform.localPosition").unwrap();
        assert_eq!(path.segments(), vec!["transform", "localPosition"]);
        assert!(PropertyPath::parse("").is_err());
        assert!(PropertyPath::parse("bad path").is_err());
    }

    #[test]
    fn property_editing_tree_builds_from_inspector_model() {
        let tree = PropertyTree::from_inspector_model(&inspector_model());
        let summary = tree.summary();
        assert_eq!(summary.selected_entity_id.as_deref(), Some("entity-player"));
        assert_eq!(summary.property_count, 2);
        assert_eq!(summary.editable_count, 2);
        assert!(summary
            .editable_paths
            .contains(&"transform.localPosition".to_string()));
    }

    #[test]
    fn property_editing_value_covers_advanced_types() {
        let values = [
            PropertyValue::Color(PropertyColor {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            }),
            PropertyValue::Array(vec![PropertyValue::Number(1.0)]),
            PropertyValue::Object(Vec::new()),
            PropertyValue::Curve(PropertyCurve {
                keys: vec![PropertyCurveKey {
                    time: 0.0,
                    value: 1.0,
                }],
            }),
            PropertyValue::RichText(PropertyRichText {
                plain_text: "hello".to_string(),
                spans: Vec::new(),
            }),
        ];
        assert_eq!(values[0].value_type(), PropertyValueType::Color);
        assert_eq!(values[1].value_type(), PropertyValueType::Array);
        assert_eq!(values[2].value_type(), PropertyValueType::Object);
        assert_eq!(values[3].value_type(), PropertyValueType::Curve);
        assert_eq!(values[4].value_type(), PropertyValueType::RichText);
    }

    #[test]
    fn property_editing_buffer_commits_transform_value() {
        let tree = PropertyTree::from_inspector_model(&inspector_model());
        let path = PropertyPath::parse("transform.localPosition").unwrap();
        let node = tree.find(&path).unwrap();
        let mut buffer = PropertyEditBuffer::new();
        buffer.begin_edit(node);
        buffer.replace_text("2,3,4");

        let report = buffer.commit(&tree).unwrap();
        let command = report.command.unwrap();
        let ui_command = command.to_ui_command("request-1").unwrap();

        assert_eq!(ui_command.command_id, "set_scene_transform");
        assert!(matches!(
            ui_command.payload,
            UiCommandPayload::SetSceneTransform { .. }
        ));
    }

    #[test]
    fn property_editing_buffer_commits_component_value() {
        let tree = PropertyTree::from_inspector_model(&inspector_model());
        let path = PropertyPath::parse("components.SpriteRenderer2D.visible").unwrap();
        let node = tree.find(&path).unwrap();
        let mut buffer = PropertyEditBuffer::new();
        buffer.begin_edit(node);
        buffer.replace_text("false");

        let report = buffer.commit(&tree).unwrap();
        let command = report.command.unwrap();
        let ui_command = command.to_ui_command("request-2").unwrap();

        assert_eq!(ui_command.command_id, "set_scene_component_field");
        match ui_command.payload {
            UiCommandPayload::SetSceneComponentField {
                component_type,
                field_path,
                value,
                ..
            } => {
                assert_eq!(component_type, "SpriteRenderer2D");
                assert_eq!(field_path, "visible");
                assert_eq!(value, serde_json::Value::Bool(false));
            }
            other => panic!("unexpected payload: {other:?}"),
        }
    }

    #[test]
    fn property_editing_buffer_tracks_ime_composition_and_cancel() {
        let tree = PropertyTree::from_inspector_model(&inspector_model());
        let path = PropertyPath::parse("components.SpriteRenderer2D.visible").unwrap();
        let node = tree.find(&path).unwrap();
        let mut buffer = PropertyEditBuffer::new();
        buffer.begin_edit(node);
        buffer.update_composition("zhong");
        assert!(buffer.composition.active);
        buffer.commit_composition("中");
        assert!(buffer.dirty);
        let report = buffer.cancel();
        assert_eq!(report.status, PropertyEditCommitStatus::Cancelled);
        assert!(report.command.is_none());
        assert!(buffer.focused_path.is_none());
    }
}
