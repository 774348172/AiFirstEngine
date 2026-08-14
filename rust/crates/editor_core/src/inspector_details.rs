use crate::{
    EditorSceneEntity, PrefabInstance, PrefabOverride, PrefabWorkflowService, PropertyEditCommand,
    PropertyEditDiagnostic, PropertyEditDiagnosticSeverity, PropertyEditTarget, PropertyEditorKind,
    PropertyMetadata, PropertyNode, PropertyPath, PropertyTree, PropertyValue, PropertyValueType,
    SceneEditCommand,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const INSPECTOR_REPORT_SCHEMA_VERSION: &str = "inspector-details-report.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectableTarget {
    pub target_id: String,
    pub kind: InspectableTargetKind,
    pub entity_id: Option<String>,
    pub instance_id: Option<String>,
    #[serde(default)]
    pub readonly: bool,
}

impl InspectableTarget {
    pub fn scene_entity(entity_id: impl Into<String>) -> Self {
        let entity_id = entity_id.into();
        Self {
            target_id: entity_id.clone(),
            kind: InspectableTargetKind::SceneEntity,
            entity_id: Some(entity_id),
            instance_id: None,
            readonly: false,
        }
    }

    pub fn prefab_instance(entity_id: impl Into<String>, instance_id: impl Into<String>) -> Self {
        let entity_id = entity_id.into();
        let instance_id = instance_id.into();
        Self {
            target_id: entity_id.clone(),
            kind: InspectableTargetKind::PrefabInstance,
            entity_id: Some(entity_id),
            instance_id: Some(instance_id),
            readonly: false,
        }
    }

    pub fn runtime_readonly(target_id: impl Into<String>) -> Self {
        let target_id = target_id.into();
        Self {
            target_id,
            kind: InspectableTargetKind::RuntimeObjectReadonly,
            entity_id: None,
            instance_id: None,
            readonly: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InspectableTargetKind {
    SceneEntity,
    PrefabInstance,
    RuntimeObjectReadonly,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentSchema {
    pub component_type: String,
    pub display_name: String,
    #[serde(default)]
    pub fields: Vec<FieldSchema>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectSchema {
    pub object_type: String,
    pub display_name: String,
    #[serde(default)]
    pub fields: Vec<FieldSchema>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldSchema {
    pub field_path: String,
    pub label: String,
    pub value_type: PropertyValueType,
    pub editor_kind: PropertyEditorKind,
    #[serde(default)]
    pub readonly: bool,
    #[serde(default)]
    pub constraints: Vec<FieldConstraint>,
    #[serde(default)]
    pub enum_options: Vec<EnumOption>,
    #[serde(default)]
    pub asset_filter: Option<AssetFilter>,
    #[serde(default)]
    pub child_fields: Vec<FieldSchema>,
    #[serde(default)]
    pub custom_plugin_id: Option<String>,
}

impl FieldSchema {
    pub fn new(
        field_path: impl Into<String>,
        label: impl Into<String>,
        value_type: PropertyValueType,
        editor_kind: PropertyEditorKind,
    ) -> Self {
        Self {
            field_path: field_path.into(),
            label: label.into(),
            value_type,
            editor_kind,
            readonly: false,
            constraints: Vec::new(),
            enum_options: Vec::new(),
            asset_filter: None,
            child_fields: Vec::new(),
            custom_plugin_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FieldConstraint {
    Required,
    Min(f64),
    Max(f64),
    Regex(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnumOption {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetFilter {
    #[serde(default)]
    pub asset_types: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentSchemaRegistry {
    component_schemas: BTreeMap<String, ComponentSchema>,
    object_schemas: BTreeMap<String, ObjectSchema>,
}

impl ComponentSchemaRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_component_schema(
        &mut self,
        schema: ComponentSchema,
    ) -> Vec<InspectorDiagnostic> {
        let mut diagnostics = validate_schema_terms(&schema.component_type);
        for field in &schema.fields {
            diagnostics.extend(validate_field_schema(field, &schema.component_type));
        }
        self.component_schemas
            .insert(schema.component_type.clone(), schema);
        diagnostics
    }

    pub fn register_object_schema(&mut self, schema: ObjectSchema) -> Vec<InspectorDiagnostic> {
        let mut diagnostics = validate_schema_terms(&schema.object_type);
        for field in &schema.fields {
            diagnostics.extend(validate_field_schema(field, &schema.object_type));
        }
        self.object_schemas
            .insert(schema.object_type.clone(), schema);
        diagnostics
    }

    pub fn component_schema(&self, component_type: &str) -> Option<&ComponentSchema> {
        self.component_schemas.get(component_type)
    }

    pub fn object_schema(&self, object_type: &str) -> Option<&ObjectSchema> {
        self.object_schemas.get(object_type)
    }

    pub fn require_component_schema(
        &self,
        component_type: &str,
    ) -> Result<&ComponentSchema, InspectorDiagnostic> {
        self.component_schema(component_type).ok_or_else(|| {
            InspectorDiagnostic::warning(
                InspectorDiagnosticCode::MissingSchema,
                format!("Missing component schema: {component_type}"),
            )
            .with_component_type(component_type)
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectorSourceData {
    pub target: InspectableTarget,
    pub display_name: String,
    #[serde(default)]
    pub object_type: Option<String>,
    #[serde(default)]
    pub values: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub components: Vec<InspectableComponentData>,
    #[serde(default)]
    pub prefab_overrides: Vec<PrefabOverride>,
}

impl InspectorSourceData {
    pub fn from_scene_entity(entity: &EditorSceneEntity) -> Self {
        let target = if PrefabInstance::from_scene_entity(entity).is_ok() {
            let instance_id = entity
                .components
                .iter()
                .find_map(|component| {
                    component
                        .fields
                        .get("instanceId")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                })
                .unwrap_or_else(|| format!("prefab-instance-{}", entity.entity_id));
            InspectableTarget::prefab_instance(entity.entity_id.clone(), instance_id)
        } else {
            InspectableTarget::scene_entity(entity.entity_id.clone())
        };
        let mut values = BTreeMap::new();
        values.insert("name".to_string(), serde_json::json!(entity.name));
        values.insert("enabled".to_string(), serde_json::json!(entity.enabled));
        if let Some(transform) = entity.transform {
            values.insert("transform".to_string(), serde_json::json!(transform));
        }
        if let Some(mesh) = &entity.mesh {
            values.insert("mesh".to_string(), serde_json::json!(mesh));
        }
        let components = entity
            .components
            .iter()
            .map(|component| InspectableComponentData {
                component_type: component.component_type.clone(),
                fields: component.fields.clone(),
            })
            .collect();
        Self {
            target,
            display_name: entity.name.clone(),
            object_type: Some("engine.scene_entity".to_string()),
            values,
            components,
            prefab_overrides: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectableComponentData {
    pub component_type: String,
    pub fields: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyHandle {
    pub target: InspectableTarget,
    pub component_type: Option<String>,
    pub path: PropertyPath,
    pub value_type: PropertyValueType,
    pub readonly: bool,
    pub override_state: PropertyOverrideState,
}

impl PropertyHandle {
    pub fn new(
        target: InspectableTarget,
        component_type: Option<String>,
        path: PropertyPath,
        value_type: PropertyValueType,
        readonly: bool,
        override_state: PropertyOverrideState,
    ) -> Self {
        Self {
            readonly: readonly || target.readonly,
            target,
            component_type,
            path,
            value_type,
            override_state,
        }
    }

    pub fn state(&self) -> PropertyHandleState {
        if self.readonly {
            PropertyHandleState::Readonly
        } else {
            PropertyHandleState::Editable
        }
    }

    pub fn read_from_value(
        &self,
        source: &serde_json::Value,
    ) -> Result<PropertyValue, InspectorDiagnostic> {
        let value = json_get_path(source, self.path.as_str()).ok_or_else(|| {
            InspectorDiagnostic::warning(
                InspectorDiagnosticCode::MissingField,
                format!("Missing property field: {}", self.path),
            )
            .with_path(self.path.as_str())
        })?;
        Ok(property_value_from_json(value, self.value_type))
    }

    pub fn validate_write(&self, value: &PropertyValue) -> Result<(), InspectorDiagnostic> {
        if self.readonly {
            return Err(InspectorDiagnostic::error(
                InspectorDiagnosticCode::ReadonlyTarget,
                format!("Property is readonly: {}", self.path),
            )
            .with_path(self.path.as_str()));
        }
        if !property_value_matches(value, self.value_type) {
            return Err(InspectorDiagnostic::error(
                InspectorDiagnosticCode::InvalidValue,
                format!(
                    "Invalid value type for {}: expected {:?}, got {:?}",
                    self.path,
                    self.value_type,
                    value.value_type()
                ),
            )
            .with_path(self.path.as_str()));
        }
        Ok(())
    }

    pub fn describe_write(
        &self,
        value: PropertyValue,
    ) -> Result<PropertyEditCommand, InspectorDiagnostic> {
        self.validate_write(&value)?;
        Ok(PropertyEditCommand::SetValue {
            target: PropertyEditTarget {
                entity_id: self.target.entity_id.clone(),
                persistence: editor_ui_model::InspectorPersistence::PersistentAuthoring,
                path: self.path.clone(),
                component_type: self.component_type.clone(),
                field_path: self
                    .component_type
                    .as_ref()
                    .map(|_| self.path.as_str().to_string()),
            },
            value,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PropertyHandleState {
    Editable,
    Readonly,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PropertyOverrideState {
    None,
    Inherited,
    Overridden,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyTreeBuildResult {
    pub tree: PropertyTree,
    pub handles: Vec<PropertyHandle>,
    pub report: InspectorReport,
}

pub struct PropertyTreeBuilder;

impl PropertyTreeBuilder {
    pub fn build(
        source: &InspectorSourceData,
        registry: &ComponentSchemaRegistry,
    ) -> PropertyTreeBuildResult {
        let mut report = InspectorReport::new();
        let mut nodes = Vec::new();
        let mut handles = Vec::new();

        if let Some(object_type) = &source.object_type {
            if let Some(schema) = registry.object_schema(object_type) {
                for field in &schema.fields {
                    push_schema_node(
                        &mut nodes,
                        &mut handles,
                        &mut report,
                        &source.target,
                        None,
                        Some(&source.values),
                        field,
                        &source.prefab_overrides,
                    );
                }
            } else {
                report.push(
                    InspectorDiagnostic::warning(
                        InspectorDiagnosticCode::MissingSchema,
                        format!("Missing object schema: {object_type}"),
                    )
                    .with_component_type(object_type),
                );
                push_json_fallback_nodes(
                    &mut nodes,
                    &mut handles,
                    &mut report,
                    &source.target,
                    None,
                    &source.values,
                );
            }
        }

        for component in &source.components {
            if let Some(schema) = registry.component_schema(&component.component_type) {
                let component_fields = component.fields.as_object().map(|fields| {
                    fields
                        .iter()
                        .map(|(key, value)| (key.clone(), value.clone()))
                        .collect::<BTreeMap<_, _>>()
                });
                for field in &schema.fields {
                    push_schema_node(
                        &mut nodes,
                        &mut handles,
                        &mut report,
                        &source.target,
                        Some(component.component_type.as_str()),
                        component_fields.as_ref(),
                        field,
                        &source.prefab_overrides,
                    );
                }
            } else {
                report.push(
                    InspectorDiagnostic::warning(
                        InspectorDiagnosticCode::MissingSchema,
                        format!("Missing component schema: {}", component.component_type),
                    )
                    .with_component_type(&component.component_type),
                );
                push_component_fallback_node(
                    &mut nodes,
                    &mut handles,
                    &mut report,
                    &source.target,
                    component,
                );
            }
        }

        report.property_count = nodes.len();
        report.editable_count = nodes
            .iter()
            .filter(|node| node.metadata.editable && !node.metadata.readonly)
            .count();
        report.readonly_count = nodes.iter().filter(|node| node.metadata.readonly).count();
        report.override_count = handles
            .iter()
            .filter(|handle| handle.override_state == PropertyOverrideState::Overridden)
            .count();

        PropertyTreeBuildResult {
            tree: PropertyTree {
                selected_entity_id: source.target.entity_id.clone(),
                persistence: editor_ui_model::InspectorPersistence::PersistentAuthoring,
                nodes,
            },
            handles,
            report,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyEditorWidgetDescriptor {
    pub widget_id: String,
    pub path: String,
    pub editor_kind: PropertyEditorKind,
    pub value_type: PropertyValueType,
    pub readonly: bool,
    #[serde(default)]
    pub enum_options: Vec<EnumOption>,
    #[serde(default)]
    pub asset_filter: Option<AssetFilter>,
    #[serde(default)]
    pub custom_plugin_id: Option<String>,
}

impl PropertyEditorWidgetDescriptor {
    pub fn from_field_schema(
        path: &PropertyPath,
        schema: &FieldSchema,
        readonly: bool,
    ) -> Result<Self, InspectorDiagnostic> {
        if !editor_kind_supports_value_type(schema.editor_kind, schema.value_type) {
            return Err(InspectorDiagnostic::warning(
                InspectorDiagnosticCode::UnsupportedEditorKind,
                format!(
                    "Editor kind {:?} does not match value type {:?}.",
                    schema.editor_kind, schema.value_type
                ),
            )
            .with_path(path.as_str()));
        }
        Ok(Self {
            widget_id: format!("widget.{}", path.as_str()),
            path: path.as_str().to_string(),
            editor_kind: if readonly {
                PropertyEditorKind::Readonly
            } else {
                schema.editor_kind
            },
            value_type: schema.value_type,
            readonly,
            enum_options: schema.enum_options.clone(),
            asset_filter: schema.asset_filter.clone(),
            custom_plugin_id: schema.custom_plugin_id.clone(),
        })
    }
}

pub struct PropertyTransactionRouter;

impl PropertyTransactionRouter {
    pub fn route(
        target: &InspectableTarget,
        command: PropertyEditCommand,
    ) -> Result<PropertyTransactionRoute, InspectorDiagnostic> {
        match target.kind {
            InspectableTargetKind::RuntimeObjectReadonly => Err(InspectorDiagnostic::error(
                InspectorDiagnosticCode::ReadonlyTarget,
                "Runtime inspector target is readonly.",
            )),
            InspectableTargetKind::SceneEntity => scene_route(command),
            InspectableTargetKind::PrefabInstance => prefab_route(target, command),
        }
    }

    pub fn apply_prefab_override(
        instance_entity: &mut EditorSceneEntity,
        command: PropertyEditCommand,
    ) -> Result<PropertyTransactionRoute, InspectorDiagnostic> {
        let target = InspectableTarget::prefab_instance(
            instance_entity.entity_id.clone(),
            PrefabInstance::from_scene_entity(instance_entity)
                .map(|instance| instance.instance_id)
                .unwrap_or_else(|_| format!("prefab-instance-{}", instance_entity.entity_id)),
        );
        let route = Self::route(&target, command)?;
        if let PropertyTransactionRoute::PrefabOverride { override_value } = &route {
            PrefabWorkflowService::write_override_to_instance_entity(
                instance_entity,
                override_value.clone(),
            )
            .map_err(prefab_diagnostic_to_inspector)?;
        }
        Ok(route)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PropertyTransactionRoute {
    SceneEdit { command: SceneEditCommand },
    PrefabOverride { override_value: PrefabOverride },
    Rejected { diagnostic: InspectorDiagnostic },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectorReport {
    pub schema_version: String,
    pub property_count: usize,
    pub editable_count: usize,
    pub readonly_count: usize,
    pub invalid_schema_count: usize,
    pub failed_edit_count: usize,
    pub override_count: usize,
    pub diagnostics: Vec<InspectorDiagnostic>,
}

impl Default for InspectorReport {
    fn default() -> Self {
        Self::new()
    }
}

impl InspectorReport {
    pub fn new() -> Self {
        Self {
            schema_version: INSPECTOR_REPORT_SCHEMA_VERSION.to_string(),
            property_count: 0,
            editable_count: 0,
            readonly_count: 0,
            invalid_schema_count: 0,
            failed_edit_count: 0,
            override_count: 0,
            diagnostics: Vec::new(),
        }
    }

    pub fn push(&mut self, diagnostic: InspectorDiagnostic) {
        if matches!(
            diagnostic.code,
            InspectorDiagnosticCode::MissingSchema
                | InspectorDiagnosticCode::InvalidSchema
                | InspectorDiagnosticCode::UnsupportedEditorKind
        ) {
            self.invalid_schema_count += 1;
        }
        if matches!(
            diagnostic.code,
            InspectorDiagnosticCode::InvalidValue | InspectorDiagnosticCode::ReadonlyTarget
        ) {
            self.failed_edit_count += 1;
        }
        self.diagnostics.push(diagnostic);
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectorDiagnostic {
    pub severity: InspectorDiagnosticSeverity,
    pub code: InspectorDiagnosticCode,
    pub message: String,
    pub component_type: Option<String>,
    pub path: Option<String>,
}

impl InspectorDiagnostic {
    pub fn info(code: InspectorDiagnosticCode, message: impl Into<String>) -> Self {
        Self::new(InspectorDiagnosticSeverity::Info, code, message)
    }

    pub fn warning(code: InspectorDiagnosticCode, message: impl Into<String>) -> Self {
        Self::new(InspectorDiagnosticSeverity::Warning, code, message)
    }

    pub fn error(code: InspectorDiagnosticCode, message: impl Into<String>) -> Self {
        Self::new(InspectorDiagnosticSeverity::Error, code, message)
    }

    fn new(
        severity: InspectorDiagnosticSeverity,
        code: InspectorDiagnosticCode,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            code,
            message: message.into(),
            component_type: None,
            path: None,
        }
    }

    pub fn with_component_type(mut self, component_type: impl Into<String>) -> Self {
        self.component_type = Some(component_type.into());
        self
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InspectorDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InspectorDiagnosticCode {
    MissingSchema,
    InvalidSchema,
    MissingField,
    InvalidValue,
    ReadonlyTarget,
    UnsupportedEditorKind,
    RouteFailed,
}

fn push_schema_node(
    nodes: &mut Vec<PropertyNode>,
    handles: &mut Vec<PropertyHandle>,
    report: &mut InspectorReport,
    target: &InspectableTarget,
    component_type: Option<&str>,
    source: Option<&BTreeMap<String, serde_json::Value>>,
    schema: &FieldSchema,
    overrides: &[PrefabOverride],
) {
    let path = match PropertyPath::parse(&schema.field_path) {
        Ok(path) => path,
        Err(error) => {
            report.push(property_edit_diagnostic_to_inspector(error));
            return;
        }
    };
    let readonly = schema.readonly || target.readonly;
    let value_json =
        source.and_then(|source| json_map_get_path(source, schema.field_path.as_str()));
    let value = value_json
        .map(|value| property_value_from_json(value, schema.value_type))
        .unwrap_or(PropertyValue::Empty);
    let override_state = override_state_for(component_type, schema.field_path.as_str(), overrides);
    let handle = PropertyHandle::new(
        target.clone(),
        component_type.map(str::to_string),
        path.clone(),
        schema.value_type,
        readonly,
        override_state,
    );
    if let Err(diagnostic) =
        PropertyEditorWidgetDescriptor::from_field_schema(&path, schema, readonly)
    {
        report.push(diagnostic);
    }
    let children = schema
        .child_fields
        .iter()
        .filter_map(|child| {
            child_node(target, component_type, value_json, child, overrides, report)
        })
        .collect::<Vec<_>>();
    nodes.push(PropertyNode {
        node_id: node_id_for(component_type, path.as_str()),
        path,
        value: value.clone(),
        value_type: value.value_type(),
        editor_kind: if readonly {
            PropertyEditorKind::Readonly
        } else {
            schema.editor_kind
        },
        metadata: PropertyMetadata {
            label: schema.label.clone(),
            readonly,
            editable: !readonly,
            component_type: component_type.map(str::to_string),
            field_path: Some(schema.field_path.clone()),
            custom_plugin_id: schema.custom_plugin_id.clone(),
        },
        children,
    });
    handles.push(handle);
}

fn child_node(
    target: &InspectableTarget,
    component_type: Option<&str>,
    parent_json: Option<&serde_json::Value>,
    schema: &FieldSchema,
    overrides: &[PrefabOverride],
    report: &mut InspectorReport,
) -> Option<PropertyNode> {
    let path = match PropertyPath::parse(&schema.field_path) {
        Ok(path) => path,
        Err(error) => {
            report.push(property_edit_diagnostic_to_inspector(error));
            return None;
        }
    };
    let value_json = parent_json.and_then(|value| json_get_path(value, schema.field_path.as_str()));
    let value = value_json
        .map(|value| property_value_from_json(value, schema.value_type))
        .unwrap_or(PropertyValue::Empty);
    let readonly = schema.readonly || target.readonly;
    let _override_state = override_state_for(component_type, schema.field_path.as_str(), overrides);
    Some(PropertyNode {
        node_id: node_id_for(component_type, path.as_str()),
        path,
        value: value.clone(),
        value_type: value.value_type(),
        editor_kind: if readonly {
            PropertyEditorKind::Readonly
        } else {
            schema.editor_kind
        },
        metadata: PropertyMetadata {
            label: schema.label.clone(),
            readonly,
            editable: !readonly,
            component_type: component_type.map(str::to_string),
            field_path: Some(schema.field_path.clone()),
            custom_plugin_id: schema.custom_plugin_id.clone(),
        },
        children: Vec::new(),
    })
}

fn push_component_fallback_node(
    nodes: &mut Vec<PropertyNode>,
    handles: &mut Vec<PropertyHandle>,
    report: &mut InspectorReport,
    target: &InspectableTarget,
    component: &InspectableComponentData,
) {
    let path = match PropertyPath::parse(component.component_type.replace('.', "_")) {
        Ok(path) => path,
        Err(error) => {
            report.push(property_edit_diagnostic_to_inspector(error));
            return;
        }
    };
    let value = property_value_from_json(&component.fields, PropertyValueType::Json);
    let readonly = target.readonly;
    nodes.push(PropertyNode {
        node_id: node_id_for(Some(component.component_type.as_str()), path.as_str()),
        path: path.clone(),
        value: value.clone(),
        value_type: PropertyValueType::Json,
        editor_kind: if readonly {
            PropertyEditorKind::Readonly
        } else {
            PropertyEditorKind::Json
        },
        metadata: PropertyMetadata {
            label: component.component_type.clone(),
            readonly,
            editable: !readonly,
            component_type: Some(component.component_type.clone()),
            field_path: None,
            custom_plugin_id: None,
        },
        children: json_children_for(&path, &component.fields, Some(&component.component_type)),
    });
    handles.push(PropertyHandle::new(
        target.clone(),
        Some(component.component_type.clone()),
        path,
        PropertyValueType::Json,
        readonly,
        PropertyOverrideState::None,
    ));
}

fn push_json_fallback_nodes(
    nodes: &mut Vec<PropertyNode>,
    handles: &mut Vec<PropertyHandle>,
    report: &mut InspectorReport,
    target: &InspectableTarget,
    component_type: Option<&str>,
    values: &BTreeMap<String, serde_json::Value>,
) {
    for (key, value) in values {
        let path = match PropertyPath::parse(key.clone()) {
            Ok(path) => path,
            Err(error) => {
                report.push(property_edit_diagnostic_to_inspector(error));
                continue;
            }
        };
        let property_value = property_value_from_json(value, PropertyValueType::Json);
        let readonly = target.readonly;
        nodes.push(PropertyNode {
            node_id: node_id_for(component_type, path.as_str()),
            path: path.clone(),
            value: property_value.clone(),
            value_type: property_value.value_type(),
            editor_kind: if readonly {
                PropertyEditorKind::Readonly
            } else {
                PropertyEditorKind::Json
            },
            metadata: PropertyMetadata {
                label: key.clone(),
                readonly,
                editable: !readonly,
                component_type: component_type.map(str::to_string),
                field_path: Some(key.clone()),
                custom_plugin_id: None,
            },
            children: json_children_for(&path, value, component_type),
        });
        handles.push(PropertyHandle::new(
            target.clone(),
            component_type.map(str::to_string),
            path,
            PropertyValueType::Json,
            readonly,
            PropertyOverrideState::None,
        ));
    }
}

fn scene_route(
    command: PropertyEditCommand,
) -> Result<PropertyTransactionRoute, InspectorDiagnostic> {
    match command {
        PropertyEditCommand::SetValue { target, value } => {
            let entity_id = target.entity_id.ok_or_else(|| {
                InspectorDiagnostic::error(
                    InspectorDiagnosticCode::RouteFailed,
                    "Scene property edit requires entity id.",
                )
            })?;
            let value_json = property_value_to_json(&value);
            if target.path.as_str() == "transform.localPosition" {
                return Ok(PropertyTransactionRoute::SceneEdit {
                    command: SceneEditCommand::SetTransform {
                        entity_id,
                        local_position: Some(json_to_editor_vec3(value_json)?),
                        local_rotation: None,
                        local_scale: None,
                    },
                });
            }
            if target.path.as_str() == "transform.localRotation" {
                return Ok(PropertyTransactionRoute::SceneEdit {
                    command: SceneEditCommand::SetTransform {
                        entity_id,
                        local_position: None,
                        local_rotation: Some(json_to_editor_vec3(value_json)?),
                        local_scale: None,
                    },
                });
            }
            if target.path.as_str() == "transform.localScale" {
                return Ok(PropertyTransactionRoute::SceneEdit {
                    command: SceneEditCommand::SetTransform {
                        entity_id,
                        local_position: None,
                        local_rotation: None,
                        local_scale: Some(json_to_editor_vec3(value_json)?),
                    },
                });
            }
            let component_type = target.component_type.ok_or_else(|| {
                InspectorDiagnostic::error(
                    InspectorDiagnosticCode::RouteFailed,
                    "Component property edit requires component type.",
                )
            })?;
            let field_path = target
                .field_path
                .unwrap_or_else(|| target.path.as_str().to_string());
            Ok(PropertyTransactionRoute::SceneEdit {
                command: SceneEditCommand::SetComponentField {
                    entity_id,
                    component_type,
                    field_path,
                    value: value_json,
                },
            })
        }
        _ => Err(InspectorDiagnostic::error(
            InspectorDiagnosticCode::RouteFailed,
            "Only SetValue is routed by Inspector Details v1.",
        )),
    }
}

fn prefab_route(
    target: &InspectableTarget,
    command: PropertyEditCommand,
) -> Result<PropertyTransactionRoute, InspectorDiagnostic> {
    match command {
        PropertyEditCommand::SetValue {
            target: edit_target,
            value,
        } => {
            let component_type = edit_target.component_type.unwrap_or_else(|| {
                if edit_target.path.as_str().starts_with("transform.") {
                    "engine.transform".to_string()
                } else {
                    "engine.object".to_string()
                }
            });
            let field_path = edit_target.field_path.unwrap_or_else(|| {
                edit_target
                    .path
                    .as_str()
                    .trim_start_matches("transform.")
                    .to_string()
            });
            let source_entity_id = target.entity_id.clone().ok_or_else(|| {
                InspectorDiagnostic::error(
                    InspectorDiagnosticCode::RouteFailed,
                    "Prefab property edit requires entity id.",
                )
            })?;
            Ok(PropertyTransactionRoute::PrefabOverride {
                override_value: PrefabOverride::new(
                    source_entity_id,
                    component_type,
                    field_path,
                    property_value_to_json(&value),
                ),
            })
        }
        _ => Err(InspectorDiagnostic::error(
            InspectorDiagnosticCode::RouteFailed,
            "Only SetValue is routed by Inspector Details v1.",
        )),
    }
}

fn validate_schema_terms(value: &str) -> Vec<InspectorDiagnostic> {
    let disallowed = [
        "player", "enemy", "bullet", "health", "damage", "score", "wave", "weapon", "boss", "drop",
    ];
    let lower = value.to_ascii_lowercase();
    if disallowed.iter().any(|term| lower.contains(term)) {
        vec![InspectorDiagnostic::warning(
            InspectorDiagnosticCode::InvalidSchema,
            format!("Schema name should stay engine-foundation neutral: {value}"),
        )]
    } else {
        Vec::new()
    }
}

fn validate_field_schema(field: &FieldSchema, owner: &str) -> Vec<InspectorDiagnostic> {
    let mut diagnostics = Vec::new();
    diagnostics.extend(validate_schema_terms(&field.field_path));
    if let Err(error) = PropertyPath::parse(&field.field_path) {
        diagnostics.push(property_edit_diagnostic_to_inspector(error).with_component_type(owner));
    }
    if !editor_kind_supports_value_type(field.editor_kind, field.value_type) {
        diagnostics.push(
            InspectorDiagnostic::warning(
                InspectorDiagnosticCode::UnsupportedEditorKind,
                format!(
                    "Editor kind {:?} does not support {:?}.",
                    field.editor_kind, field.value_type
                ),
            )
            .with_component_type(owner)
            .with_path(&field.field_path),
        );
    }
    diagnostics
}

fn property_edit_diagnostic_to_inspector(error: PropertyEditDiagnostic) -> InspectorDiagnostic {
    let severity = match error.severity {
        PropertyEditDiagnosticSeverity::Info => InspectorDiagnosticSeverity::Info,
        PropertyEditDiagnosticSeverity::Warning => InspectorDiagnosticSeverity::Warning,
        PropertyEditDiagnosticSeverity::Error => InspectorDiagnosticSeverity::Error,
    };
    InspectorDiagnostic {
        severity,
        code: InspectorDiagnosticCode::InvalidSchema,
        message: error.message,
        component_type: None,
        path: error.path,
    }
}

fn prefab_diagnostic_to_inspector(error: crate::PrefabDiagnostic) -> InspectorDiagnostic {
    InspectorDiagnostic::error(InspectorDiagnosticCode::RouteFailed, error.message)
        .with_path(error.field_path.unwrap_or_default())
}

fn editor_kind_supports_value_type(
    editor_kind: PropertyEditorKind,
    value_type: PropertyValueType,
) -> bool {
    matches!(
        (editor_kind, value_type),
        (PropertyEditorKind::Text, PropertyValueType::String)
            | (
                PropertyEditorKind::MultilineRichText,
                PropertyValueType::RichText
            )
            | (PropertyEditorKind::Number, PropertyValueType::Number)
            | (PropertyEditorKind::Slider, PropertyValueType::Number)
            | (PropertyEditorKind::Toggle, PropertyValueType::Bool)
            | (PropertyEditorKind::Vec2, PropertyValueType::Vec2)
            | (PropertyEditorKind::Vec3, PropertyValueType::Vec3)
            | (PropertyEditorKind::Vec4, PropertyValueType::Vec4)
            | (PropertyEditorKind::ColorPicker, PropertyValueType::Color)
            | (PropertyEditorKind::Enum, PropertyValueType::Enum)
            | (
                PropertyEditorKind::AssetRefPicker,
                PropertyValueType::AssetRef
            )
            | (
                PropertyEditorKind::EntityRefPicker,
                PropertyValueType::EntityRef
            )
            | (PropertyEditorKind::Array, PropertyValueType::Array)
            | (PropertyEditorKind::Object, PropertyValueType::Object)
            | (PropertyEditorKind::Curve, PropertyValueType::Curve)
            | (PropertyEditorKind::Json, _)
            | (PropertyEditorKind::Custom, _)
            | (PropertyEditorKind::Readonly, _)
    )
}

fn property_value_matches(value: &PropertyValue, expected: PropertyValueType) -> bool {
    expected == PropertyValueType::Json
        || expected == PropertyValueType::Vec2
        || expected == PropertyValueType::Vec4
        || expected == PropertyValueType::Enum
        || value.value_type() == expected
}

fn property_value_from_json(
    value: &serde_json::Value,
    expected: PropertyValueType,
) -> PropertyValue {
    match expected {
        PropertyValueType::String => value
            .as_str()
            .map(|value| PropertyValue::String(value.to_string()))
            .unwrap_or_else(|| PropertyValue::Json(value.clone())),
        PropertyValueType::Bool => value
            .as_bool()
            .map(PropertyValue::Bool)
            .unwrap_or_else(|| PropertyValue::Json(value.clone())),
        PropertyValueType::Number => value
            .as_f64()
            .map(PropertyValue::Number)
            .unwrap_or_else(|| PropertyValue::Json(value.clone())),
        PropertyValueType::Vec3 => serde_json::from_value::<editor_ui_model::Vec3>(value.clone())
            .map(PropertyValue::Vec3)
            .unwrap_or_else(|_| PropertyValue::Json(value.clone())),
        PropertyValueType::AssetRef => {
            serde_json::from_value::<editor_ui_model::EditorAssetRef>(value.clone())
                .map(PropertyValue::AssetRef)
                .unwrap_or_else(|_| PropertyValue::Json(value.clone()))
        }
        PropertyValueType::EntityRef => value
            .as_str()
            .map(|value| PropertyValue::EntityRef(value.to_string()))
            .unwrap_or_else(|| PropertyValue::Json(value.clone())),
        PropertyValueType::Array => value
            .as_array()
            .map(|values| {
                PropertyValue::Array(
                    values
                        .iter()
                        .map(|value| property_value_from_json(value, PropertyValueType::Json))
                        .collect(),
                )
            })
            .unwrap_or_else(|| PropertyValue::Json(value.clone())),
        PropertyValueType::Object => PropertyValue::Object(json_children_for(
            &PropertyPath::parse("object").expect("static property path"),
            value,
            None,
        )),
        PropertyValueType::Empty => PropertyValue::Empty,
        _ => PropertyValue::Json(value.clone()),
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
        PropertyValue::Curve(value) => serde_json::json!(value),
        PropertyValue::RichText(value) => serde_json::json!(value),
        PropertyValue::Json(value) => value.clone(),
        PropertyValue::Empty => serde_json::Value::Null,
    }
}

fn json_children_for(
    parent_path: &PropertyPath,
    value: &serde_json::Value,
    component_type: Option<&str>,
) -> Vec<PropertyNode> {
    let Some(object) = value.as_object() else {
        return Vec::new();
    };
    object
        .iter()
        .filter_map(|(key, value)| {
            let path = PropertyPath::parse(format!("{}.{}", parent_path.as_str(), key)).ok()?;
            let property_value = property_value_from_json(value, PropertyValueType::Json);
            Some(PropertyNode {
                node_id: node_id_for(component_type, path.as_str()),
                path,
                value: property_value.clone(),
                value_type: property_value.value_type(),
                editor_kind: PropertyEditorKind::Json,
                metadata: PropertyMetadata {
                    label: key.clone(),
                    readonly: false,
                    editable: true,
                    component_type: component_type.map(str::to_string),
                    field_path: Some(key.clone()),
                    custom_plugin_id: None,
                },
                children: Vec::new(),
            })
        })
        .collect()
}

fn json_map_get_path<'a>(
    values: &'a BTreeMap<String, serde_json::Value>,
    path: &str,
) -> Option<&'a serde_json::Value> {
    let mut segments = path.split('.').filter(|segment| !segment.is_empty());
    let first = segments.next()?;
    let mut current = values.get(first)?;
    for segment in segments {
        current = current.get(segment)?;
    }
    Some(current)
}

fn json_get_path<'a>(value: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let mut current = value;
    for segment in path.split('.').filter(|segment| !segment.is_empty()) {
        current = current.get(segment)?;
    }
    Some(current)
}

fn override_state_for(
    component_type: Option<&str>,
    field_path: &str,
    overrides: &[PrefabOverride],
) -> PropertyOverrideState {
    let Some(component_type) = component_type else {
        return PropertyOverrideState::None;
    };
    if overrides.iter().any(|override_value| {
        override_value.component_type == component_type && override_value.field_path == field_path
    }) {
        PropertyOverrideState::Overridden
    } else {
        PropertyOverrideState::Inherited
    }
}

fn json_to_editor_vec3(value: serde_json::Value) -> Result<crate::EditorVec3, InspectorDiagnostic> {
    serde_json::from_value(value).map_err(|_| {
        InspectorDiagnostic::error(
            InspectorDiagnosticCode::InvalidValue,
            "Expected Vec3-compatible JSON value.",
        )
    })
}

fn node_id_for(component_type: Option<&str>, path: &str) -> String {
    match component_type {
        Some(component_type) => format!("{}.{}", component_type, path),
        None => path.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EditorSceneComponent, EditorSceneEntity, EditorTransform, EditorVec3,
        PREFAB_INSTANCE_COMPONENT_TYPE,
    };

    fn registry() -> ComponentSchemaRegistry {
        let mut registry = ComponentSchemaRegistry::new();
        registry.register_object_schema(ObjectSchema {
            object_type: "engine.scene_entity".to_string(),
            display_name: "Scene Entity".to_string(),
            fields: vec![
                FieldSchema::new(
                    "name",
                    "Name",
                    PropertyValueType::String,
                    PropertyEditorKind::Text,
                ),
                FieldSchema::new(
                    "transform.localPosition",
                    "Local Position",
                    PropertyValueType::Vec3,
                    PropertyEditorKind::Vec3,
                ),
            ],
        });
        registry.register_component_schema(ComponentSchema {
            component_type: "project.motion".to_string(),
            display_name: "Motion".to_string(),
            fields: vec![
                FieldSchema::new(
                    "speed",
                    "Speed",
                    PropertyValueType::Number,
                    PropertyEditorKind::Number,
                ),
                FieldSchema::new(
                    "flags",
                    "Flags",
                    PropertyValueType::Object,
                    PropertyEditorKind::Object,
                ),
            ],
        });
        registry
    }

    fn entity() -> EditorSceneEntity {
        EditorSceneEntity {
            schema_version: crate::EDITOR_SCENE_DOCUMENT_SCHEMA_VERSION.to_string(),
            entity_id: "entity-a".to_string(),
            name: "Entity A".to_string(),
            kind: "entity".to_string(),
            enabled: true,
            parent_id: None,
            sibling_order: 0,
            transform: Some(EditorTransform {
                local_position: EditorVec3 {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                },
                local_rotation: EditorVec3::ZERO,
                local_scale: EditorVec3::ONE,
            }),
            mesh: None,
            components: vec![EditorSceneComponent {
                component_type: "project.motion".to_string(),
                fields: serde_json::json!({ "speed": 2.0, "flags": { "enabled": true } }),
            }],
        }
    }

    #[test]
    fn inspector_schema_registry_registers_and_resolves_schema() {
        let registry = registry();
        let schema = registry.component_schema("project.motion").unwrap();
        assert_eq!(schema.display_name, "Motion");
        assert_eq!(schema.fields.len(), 2);
    }

    #[test]
    fn collider2d_inspector_schema_registers_engine_fields() {
        let mut registry = ComponentSchemaRegistry::new();
        let diagnostics = registry.register_component_schema(ComponentSchema {
            component_type: "engine.collider2d".to_string(),
            display_name: "Collider2D".to_string(),
            fields: vec![
                FieldSchema::new(
                    "shape",
                    "Shape",
                    PropertyValueType::Enum,
                    PropertyEditorKind::Enum,
                ),
                FieldSchema::new(
                    "halfExtents.x",
                    "Half Extents X",
                    PropertyValueType::Number,
                    PropertyEditorKind::Number,
                ),
                FieldSchema::new(
                    "halfExtents.y",
                    "Half Extents Y",
                    PropertyValueType::Number,
                    PropertyEditorKind::Number,
                ),
                FieldSchema::new(
                    "radius",
                    "Radius",
                    PropertyValueType::Number,
                    PropertyEditorKind::Number,
                ),
                FieldSchema::new(
                    "offset",
                    "Offset",
                    PropertyValueType::Vec2,
                    PropertyEditorKind::Vec2,
                ),
                FieldSchema::new(
                    "enabled",
                    "Enabled",
                    PropertyValueType::Bool,
                    PropertyEditorKind::Toggle,
                ),
                FieldSchema::new(
                    "isSensor",
                    "Sensor",
                    PropertyValueType::Bool,
                    PropertyEditorKind::Toggle,
                ),
            ],
        });
        let schema = registry
            .component_schema("engine.collider2d")
            .expect("collider2d schema should be registered");

        assert!(diagnostics.is_empty());
        assert!(schema
            .fields
            .iter()
            .any(|field| field.field_path == "halfExtents.x"));
        assert!(schema
            .fields
            .iter()
            .any(|field| field.editor_kind == PropertyEditorKind::Toggle));
    }

    #[test]
    fn inspector_schema_missing_schema_returns_diagnostic() {
        let registry = ComponentSchemaRegistry::new();
        let diagnostic = registry
            .require_component_schema("project.unknown")
            .unwrap_err();
        assert_eq!(diagnostic.code, InspectorDiagnosticCode::MissingSchema);
    }

    #[test]
    fn inspector_schema_warns_on_gameplay_specific_api_terms() {
        let mut registry = ComponentSchemaRegistry::new();
        let diagnostics = registry.register_component_schema(ComponentSchema {
            component_type: "project.health".to_string(),
            display_name: "Invalid".to_string(),
            fields: Vec::new(),
        });
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == InspectorDiagnosticCode::InvalidSchema));
    }

    #[test]
    fn property_handle_reads_and_validates_values() {
        let handle = PropertyHandle::new(
            InspectableTarget::scene_entity("entity-a"),
            Some("project.motion".to_string()),
            PropertyPath::parse("speed").unwrap(),
            PropertyValueType::Number,
            false,
            PropertyOverrideState::None,
        );
        let value = handle
            .read_from_value(&serde_json::json!({ "speed": 3.0 }))
            .unwrap();
        assert_eq!(value, PropertyValue::Number(3.0));
        assert!(handle
            .validate_write(&PropertyValue::String("bad".to_string()))
            .is_err());
    }

    #[test]
    fn property_handle_rejects_readonly_write() {
        let handle = PropertyHandle::new(
            InspectableTarget::runtime_readonly("runtime-a"),
            None,
            PropertyPath::parse("name").unwrap(),
            PropertyValueType::String,
            false,
            PropertyOverrideState::None,
        );
        assert!(handle
            .describe_write(PropertyValue::String("New".to_string()))
            .is_err());
    }

    #[test]
    fn property_tree_builder_builds_tree_from_schema_and_source() {
        let source = InspectorSourceData::from_scene_entity(&entity());
        let result = PropertyTreeBuilder::build(&source, &registry());
        assert!(result
            .tree
            .nodes
            .iter()
            .any(|node| node.path.as_str() == "name"));
        assert!(result
            .tree
            .nodes
            .iter()
            .any(|node| node.metadata.component_type.as_deref() == Some("project.motion")));
        assert_eq!(result.report.invalid_schema_count, 0);
    }

    #[test]
    fn property_tree_builder_uses_json_fallback_for_missing_schema() {
        let source = InspectorSourceData::from_scene_entity(&entity());
        let result = PropertyTreeBuilder::build(&source, &ComponentSchemaRegistry::new());
        assert!(result.report.invalid_schema_count >= 1);
        assert!(result
            .tree
            .nodes
            .iter()
            .any(|node| node.editor_kind == PropertyEditorKind::Json));
    }

    #[test]
    fn property_widget_maps_field_schema_to_descriptor() {
        let schema = FieldSchema::new(
            "speed",
            "Speed",
            PropertyValueType::Number,
            PropertyEditorKind::Slider,
        );
        let descriptor = PropertyEditorWidgetDescriptor::from_field_schema(
            &PropertyPath::parse("speed").unwrap(),
            &schema,
            false,
        )
        .unwrap();
        assert_eq!(descriptor.editor_kind, PropertyEditorKind::Slider);
    }

    #[test]
    fn transaction_router_routes_scene_entity_edit() {
        let target = InspectableTarget::scene_entity("entity-a");
        let command = PropertyEditCommand::SetValue {
            target: PropertyEditTarget {
                entity_id: Some("entity-a".to_string()),
                persistence: editor_ui_model::InspectorPersistence::PersistentAuthoring,
                path: PropertyPath::parse("speed").unwrap(),
                component_type: Some("project.motion".to_string()),
                field_path: Some("speed".to_string()),
            },
            value: PropertyValue::Number(5.0),
        };
        let route = PropertyTransactionRouter::route(&target, command).unwrap();
        assert!(matches!(
            route,
            PropertyTransactionRoute::SceneEdit {
                command: SceneEditCommand::SetComponentField { .. }
            }
        ));
    }

    #[test]
    fn transaction_router_routes_prefab_instance_edit_to_override() {
        let target = InspectableTarget::prefab_instance("entity-a", "instance-a");
        let command = PropertyEditCommand::SetValue {
            target: PropertyEditTarget {
                entity_id: Some("entity-a".to_string()),
                persistence: editor_ui_model::InspectorPersistence::PersistentAuthoring,
                path: PropertyPath::parse("speed").unwrap(),
                component_type: Some("project.motion".to_string()),
                field_path: Some("speed".to_string()),
            },
            value: PropertyValue::Number(6.0),
        };
        let route = PropertyTransactionRouter::route(&target, command).unwrap();
        match route {
            PropertyTransactionRoute::PrefabOverride { override_value } => {
                assert_eq!(override_value.component_type, "project.motion");
                assert_eq!(override_value.field_path, "speed");
            }
            other => panic!("unexpected route: {other:?}"),
        }
    }

    #[test]
    fn prefab_instance_property_edit_creates_prefab_override() {
        let mut entity = entity();
        entity.components.push(EditorSceneComponent {
            component_type: PREFAB_INSTANCE_COMPONENT_TYPE.to_string(),
            fields: serde_json::json!({
                "source": { "id": "prefab-a", "type": "prefab" },
                "instanceId": "instance-a",
                "overrides": []
            }),
        });
        let command = PropertyEditCommand::SetValue {
            target: PropertyEditTarget {
                entity_id: Some("entity-a".to_string()),
                persistence: editor_ui_model::InspectorPersistence::PersistentAuthoring,
                path: PropertyPath::parse("speed").unwrap(),
                component_type: Some("project.motion".to_string()),
                field_path: Some("speed".to_string()),
            },
            value: PropertyValue::Number(7.0),
        };
        let route = PropertyTransactionRouter::apply_prefab_override(&mut entity, command).unwrap();
        assert!(matches!(
            route,
            PropertyTransactionRoute::PrefabOverride { .. }
        ));
        let parsed = PrefabInstance::from_scene_entity(&entity).unwrap();
        assert_eq!(parsed.overrides.len(), 1);
    }

    #[test]
    fn inspector_report_counts_properties_and_diagnostics() {
        let mut report = InspectorReport::new();
        report.property_count = 3;
        report.editable_count = 2;
        report.readonly_count = 1;
        report.push(InspectorDiagnostic::warning(
            InspectorDiagnosticCode::MissingSchema,
            "missing",
        ));
        report.push(InspectorDiagnostic::error(
            InspectorDiagnosticCode::ReadonlyTarget,
            "readonly",
        ));
        assert_eq!(report.invalid_schema_count, 1);
        assert_eq!(report.failed_edit_count, 1);
    }
}
