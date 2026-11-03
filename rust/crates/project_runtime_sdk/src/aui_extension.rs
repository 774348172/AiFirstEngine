//! Versioned, bounded custom AUI extension wire. Project code never receives engine internals.

use crate::aui::{UiError, UiErrorKind, UiNodeHandle};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const UI_EXTENSION_VERSION: u32 = 2;
pub const MAX_UI_EXTENSION_BATCH_NODES: usize = 256;
pub const MAX_UI_EXTENSION_WIRE_BYTES: usize = 1024 * 1024;
pub const MAX_UI_EXTENSION_PROPERTIES: usize = 128;
pub const MAX_UI_EXTENSION_IDENTIFIER_BYTES: usize = 128;
pub const MAX_UI_EXTENSION_CHILD_RECTS: usize = 4096;
pub const MAX_UI_EXTENSION_VERTICES: usize = 4096;
pub const MAX_UI_EXTENSION_INDICES: usize = 12_288;
pub const MAX_UI_EXTENSION_PRIMITIVES: usize = 4096;
pub const UI_EXTENSION_SCHEMA: &str = "aui-extension.v2;batch<=256;wire<=1048576;stages=describe|measure|layout|paint;callbackId;typedProperties<=128;identifier<=128;childRects<=4096;vertices<=4096;indices<=12288;primitives<=4096;graphics.v1:commands<=4096,items<=256;graphicsAsset:id+guid;readonly;atomic";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiExtensionInvalidation {
    Measure,
    Layout,
    Paint,
    Hit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiExtensionPropertyType {
    Bool,
    Float,
    Integer,
    Text,
    Color,
    Asset,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum UiExtensionValue {
    Bool(bool),
    Float(f32),
    Integer(i64),
    Text(String),
    Color([f32; 4]),
    Asset(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiExtensionPropertyUpdate {
    pub handle: UiNodeHandle,
    pub name: String,
    pub value: UiExtensionValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiExtensionPropertyDescriptor {
    pub name: String,
    pub property_type: UiExtensionPropertyType,
    pub invalidates: Vec<UiExtensionInvalidation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiExtensionTypeDescriptor {
    pub type_id: String,
    pub schema_version: u32,
    pub callback_id: u64,
    #[serde(default)]
    pub properties: Vec<UiExtensionPropertyDescriptor>,
    pub measure: bool,
    pub layout: bool,
    pub paint: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiExtensionConstraints {
    pub known_width: Option<f32>,
    pub known_height: Option<f32>,
    pub available_width: Option<f32>,
    pub available_height: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiExtensionChildInput {
    pub handle: UiNodeHandle,
    pub measured_size: [f32; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case", deny_unknown_fields)]
pub enum UiExtensionInput {
    Describe,
    Measure {
        constraints: UiExtensionConstraints,
    },
    Layout {
        size: [f32; 2],
        children: Vec<UiExtensionChildInput>,
    },
    Paint {
        size: [f32; 2],
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiExtensionRequest {
    pub request_id: u64,
    pub node: UiNodeHandle,
    pub type_id: String,
    pub schema_version: u32,
    pub callback_id: u64,
    #[serde(default)]
    pub properties: BTreeMap<String, UiExtensionValue>,
    pub input: UiExtensionInput,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiExtensionRequestBatch {
    pub version: u32,
    pub requests: Vec<UiExtensionRequest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiExtensionMeasureOutput {
    pub size: [f32; 2],
    pub first_baseline: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiExtensionChildRect {
    pub handle: UiNodeHandle,
    pub rect: [f32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiExtensionMeshVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
    /// Linear normalized RGBA with straight alpha.
    pub color: [f32; 4],
}

/// Numeric primitive colors are linear normalized RGBA with straight alpha.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UiExtensionPaintPrimitive {
    /// Retained path data: the runtime owns tessellation and geometry caching.
    Graphics {
        graphics: crate::aui_draw2d::Graphics2D,
    },
    /// Resolves a cooked graphics2d asset by both logical ID and GUID.
    GraphicsAsset {
        asset_id: String,
        guid: String,
    },
    Rect {
        rect: [f32; 4],
        color: [f32; 4],
    },
    Image {
        rect: [f32; 4],
        asset_id: String,
        color: [f32; 4],
    },
    Text {
        rect: [f32; 4],
        text: String,
        color: [f32; 4],
        font_size: f32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiExtensionPaintOutput {
    pub vertices: Vec<UiExtensionMeshVertex>,
    pub indices: Vec<u32>,
    pub asset_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub primitives: Vec<UiExtensionPaintPrimitive>,
}

impl UiExtensionPaintOutput {
    /// Publish local logical-pixel paths; the host validates, tessellates and retains them.
    /// Vertex RGBA uses the existing straight-alpha UI pass color convention.
    pub fn from_graphics(graphics: crate::aui_draw2d::Graphics2D) -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            asset_id: None,
            primitives: vec![UiExtensionPaintPrimitive::Graphics { graphics }],
        }
    }

    /// References only cooked package data. Both ID and GUID must match the same asset.
    pub fn from_graphics_asset(asset_id: impl Into<String>, guid: impl Into<String>) -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            asset_id: None,
            primitives: vec![UiExtensionPaintPrimitive::GraphicsAsset {
                asset_id: asset_id.into(),
                guid: guid.into(),
            }],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case", deny_unknown_fields)]
pub enum UiExtensionOutput {
    Describe {
        descriptor: UiExtensionTypeDescriptor,
    },
    Measure {
        output: UiExtensionMeasureOutput,
    },
    Layout {
        children: Vec<UiExtensionChildRect>,
    },
    Paint {
        output: UiExtensionPaintOutput,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiExtensionResponse {
    pub request_id: u64,
    pub output: UiExtensionOutput,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiExtensionResponseBatch {
    pub version: u32,
    pub responses: Vec<UiExtensionResponse>,
}

fn invalid(kind: UiErrorKind) -> UiError {
    UiError {
        kind,
        handle: None,
        property: None,
    }
}

pub fn validate_descriptor(value: &UiExtensionTypeDescriptor) -> Result<(), UiError> {
    if value.type_id.is_empty()
        || value.type_id.len() > MAX_UI_EXTENSION_IDENTIFIER_BYTES
        || value.schema_version == 0
        || value.callback_id == 0
        || value.properties.len() > MAX_UI_EXTENSION_PROPERTIES
    {
        return Err(invalid(UiErrorKind::InvalidValue));
    }
    let mut names = std::collections::BTreeSet::new();
    for property in &value.properties {
        if property.name.is_empty()
            || property.name.len() > MAX_UI_EXTENSION_IDENTIFIER_BYTES
            || !names.insert(property.name.as_str())
            || property.invalidates.is_empty()
        {
            return Err(invalid(UiErrorKind::InvalidValue));
        }
    }
    Ok(())
}

pub fn validate_properties(
    descriptor: &UiExtensionTypeDescriptor,
    properties: &BTreeMap<String, UiExtensionValue>,
) -> Result<(), UiError> {
    if properties.len() > MAX_UI_EXTENSION_PROPERTIES {
        return Err(invalid(UiErrorKind::CapacityExceeded));
    }
    for (name, value) in properties {
        let Some(property) = descriptor.properties.iter().find(|item| item.name == *name) else {
            return Err(invalid(UiErrorKind::InvalidValue));
        };
        let actual = match value {
            UiExtensionValue::Bool(_) => UiExtensionPropertyType::Bool,
            UiExtensionValue::Float(value) if value.is_finite() => UiExtensionPropertyType::Float,
            UiExtensionValue::Integer(_) => UiExtensionPropertyType::Integer,
            UiExtensionValue::Text(_) => UiExtensionPropertyType::Text,
            UiExtensionValue::Color(value) if value.iter().all(|channel| channel.is_finite()) => {
                UiExtensionPropertyType::Color
            }
            UiExtensionValue::Asset(value) if !value.is_empty() => UiExtensionPropertyType::Asset,
            _ => return Err(invalid(UiErrorKind::InvalidValue)),
        };
        if actual != property.property_type {
            return Err(invalid(UiErrorKind::WrongNodeType));
        }
    }
    Ok(())
}

pub fn validate_request_batch(value: &UiExtensionRequestBatch) -> Result<(), UiError> {
    if value.version != UI_EXTENSION_VERSION || value.requests.len() > MAX_UI_EXTENSION_BATCH_NODES
    {
        return Err(invalid(UiErrorKind::CapacityExceeded));
    }
    for request in &value.requests {
        if request.request_id == 0
            || request.type_id.is_empty()
            || request.type_id.len() > MAX_UI_EXTENSION_IDENTIFIER_BYTES
            || request.schema_version == 0
            || request.properties.len() > MAX_UI_EXTENSION_PROPERTIES
        {
            return Err(invalid(UiErrorKind::InvalidValue));
        }
        if request
            .properties
            .keys()
            .any(|name| name.is_empty() || name.len() > MAX_UI_EXTENSION_IDENTIFIER_BYTES)
        {
            return Err(invalid(UiErrorKind::InvalidValue));
        }
        if matches!(request.input, UiExtensionInput::Describe) {
            if request.callback_id != 0 {
                return Err(invalid(UiErrorKind::InvalidValue));
            }
        } else if request.callback_id == 0 {
            return Err(invalid(UiErrorKind::InvalidValue));
        }
        if let UiExtensionInput::Layout { children, .. } = &request.input {
            if children.len() > MAX_UI_EXTENSION_CHILD_RECTS {
                return Err(invalid(UiErrorKind::CapacityExceeded));
            }
        }
    }
    let bytes = serde_json::to_vec(value).map_err(|_| invalid(UiErrorKind::InvalidValue))?;
    if bytes.len() > MAX_UI_EXTENSION_WIRE_BYTES {
        return Err(invalid(UiErrorKind::CapacityExceeded));
    }
    Ok(())
}

pub fn validate_response_batch(value: &UiExtensionResponseBatch) -> Result<(), UiError> {
    if value.version != UI_EXTENSION_VERSION || value.responses.len() > MAX_UI_EXTENSION_BATCH_NODES
    {
        return Err(invalid(UiErrorKind::CapacityExceeded));
    }
    let mut ids = std::collections::BTreeSet::new();
    for response in &value.responses {
        if response.request_id == 0 || !ids.insert(response.request_id) {
            return Err(invalid(UiErrorKind::InvalidValue));
        }
        match &response.output {
            UiExtensionOutput::Describe { descriptor } => validate_descriptor(descriptor)?,
            UiExtensionOutput::Measure { output } => {
                if output.size.iter().any(|v| !v.is_finite() || *v < 0.0)
                    || output
                        .first_baseline
                        .is_some_and(|v| !v.is_finite() || v < 0.0)
                {
                    return Err(invalid(UiErrorKind::InvalidValue));
                }
            }
            UiExtensionOutput::Layout { children } => {
                if children.len() > MAX_UI_EXTENSION_CHILD_RECTS
                    || children.iter().any(|c| {
                        c.rect.iter().any(|v| !v.is_finite()) || c.rect[2] < 0.0 || c.rect[3] < 0.0
                    })
                {
                    return Err(invalid(UiErrorKind::InvalidValue));
                }
            }
            UiExtensionOutput::Paint { output } => {
                if output.vertices.len() > MAX_UI_EXTENSION_VERTICES
                    || output.indices.len() > MAX_UI_EXTENSION_INDICES
                    || output.primitives.len() > MAX_UI_EXTENSION_PRIMITIVES
                    || output
                        .indices
                        .iter()
                        .any(|&i| i as usize >= output.vertices.len())
                    || output.vertices.iter().any(|v| {
                        v.position
                            .iter()
                            .chain(v.uv.iter())
                            .chain(v.color.iter())
                            .any(|x| !x.is_finite())
                    })
                    || output.primitives.iter().any(|primitive| match primitive {
                        UiExtensionPaintPrimitive::Graphics { graphics } => {
                            graphics.validate().is_err()
                        }
                        UiExtensionPaintPrimitive::GraphicsAsset { asset_id, guid } => {
                            asset_id.is_empty()
                                || guid.is_empty()
                                || asset_id.len() > MAX_UI_EXTENSION_IDENTIFIER_BYTES
                                || guid.len() > MAX_UI_EXTENSION_IDENTIFIER_BYTES
                        }
                        UiExtensionPaintPrimitive::Rect { rect, color } => {
                            invalid_rect(*rect) || color.iter().any(|value| !value.is_finite())
                        }
                        UiExtensionPaintPrimitive::Image {
                            rect,
                            asset_id,
                            color,
                        } => {
                            invalid_rect(*rect)
                                || asset_id.is_empty()
                                || color.iter().any(|value| !value.is_finite())
                        }
                        UiExtensionPaintPrimitive::Text {
                            rect,
                            text: _,
                            color,
                            font_size,
                        } => {
                            invalid_rect(*rect)
                                || color.iter().any(|value| !value.is_finite())
                                || !font_size.is_finite()
                                || *font_size <= 0.0
                        }
                    })
                {
                    return Err(invalid(UiErrorKind::CapacityExceeded));
                }
            }
        }
    }
    let bytes = serde_json::to_vec(value).map_err(|_| invalid(UiErrorKind::InvalidValue))?;
    if bytes.len() > MAX_UI_EXTENSION_WIRE_BYTES {
        return Err(invalid(UiErrorKind::CapacityExceeded));
    }
    Ok(())
}

fn invalid_rect(rect: [f32; 4]) -> bool {
    rect.iter().any(|value| !value.is_finite()) || rect[2] < 0.0 || rect[3] < 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_and_mesh_limits_are_enforced() {
        let mut descriptor = UiExtensionTypeDescriptor {
            type_id: "sample.circle".into(),
            schema_version: 1,
            callback_id: 7,
            properties: vec![],
            measure: true,
            layout: false,
            paint: true,
        };
        assert!(validate_descriptor(&descriptor).is_ok());
        descriptor.callback_id = 0;
        assert!(validate_descriptor(&descriptor).is_err());

        let response = UiExtensionResponseBatch {
            version: UI_EXTENSION_VERSION,
            responses: vec![UiExtensionResponse {
                request_id: 1,
                output: UiExtensionOutput::Paint {
                    output: UiExtensionPaintOutput {
                        vertices: vec![],
                        indices: vec![0],
                        asset_id: None,
                        primitives: vec![],
                    },
                },
            }],
        };
        assert!(validate_response_batch(&response).is_err());
    }

    #[test]
    fn typed_properties_and_callback_identity_are_enforced() {
        let descriptor = UiExtensionTypeDescriptor {
            type_id: "sample.circle".into(),
            schema_version: 1,
            callback_id: 7,
            properties: vec![UiExtensionPropertyDescriptor {
                name: "progress".into(),
                property_type: UiExtensionPropertyType::Float,
                invalidates: vec![UiExtensionInvalidation::Paint],
            }],
            measure: true,
            layout: false,
            paint: true,
        };
        assert!(validate_properties(
            &descriptor,
            &BTreeMap::from([("progress".into(), UiExtensionValue::Float(0.5))]),
        )
        .is_ok());
        assert!(validate_properties(
            &descriptor,
            &BTreeMap::from([("progress".into(), UiExtensionValue::Integer(1))]),
        )
        .is_err());

        let mut request = UiExtensionRequestBatch {
            version: UI_EXTENSION_VERSION,
            requests: vec![UiExtensionRequest {
                request_id: 1,
                node: UiNodeHandle {
                    instance: 1,
                    slot: 0,
                    generation: 1,
                },
                type_id: "sample.circle".into(),
                schema_version: 1,
                callback_id: 7,
                properties: BTreeMap::new(),
                input: UiExtensionInput::Paint { size: [10.0, 10.0] },
            }],
        };
        assert!(validate_request_batch(&request).is_ok());
        request.requests[0].callback_id = 0;
        assert!(validate_request_batch(&request).is_err());
    }

    #[test]
    fn graphics_extension_v2_roundtrips_and_rejects_old_reader_and_invalid_refs() {
        let mut response = UiExtensionResponseBatch {
            version: UI_EXTENSION_VERSION,
            responses: vec![UiExtensionResponse {
                request_id: 1,
                output: UiExtensionOutput::Paint {
                    output: UiExtensionPaintOutput {
                        vertices: vec![],
                        indices: vec![],
                        asset_id: None,
                        primitives: vec![
                            UiExtensionPaintPrimitive::Graphics {
                                graphics: crate::aui_draw2d::Graphics2D::default(),
                            },
                            UiExtensionPaintPrimitive::GraphicsAsset {
                                asset_id: "shape".into(),
                                guid: "guid-shape".into(),
                            },
                        ],
                    },
                },
            }],
        };
        validate_response_batch(&response).unwrap();
        let decoded: UiExtensionResponseBatch =
            serde_json::from_slice(&serde_json::to_vec(&response).unwrap()).unwrap();
        assert_eq!(decoded, response);
        response.version = 1;
        assert!(validate_response_batch(&response).is_err());
        response.version = UI_EXTENSION_VERSION;
        if let UiExtensionOutput::Paint { output } = &mut response.responses[0].output {
            output.primitives[1] = UiExtensionPaintPrimitive::GraphicsAsset {
                asset_id: "shape".into(),
                guid: "".into(),
            };
        }
        assert!(validate_response_batch(&response).is_err());
    }
}
