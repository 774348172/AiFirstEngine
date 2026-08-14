use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorTransform {
    pub local_position: EditorVec3,
    pub local_rotation: EditorVec3,
    pub local_scale: EditorVec3,
}

impl EditorTransform {
    pub fn identity() -> Self {
        Self {
            local_position: EditorVec3::ZERO,
            local_rotation: EditorVec3::ZERO,
            local_scale: EditorVec3::ONE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EditorVec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl EditorVec3 {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    pub const ONE: Self = Self {
        x: 1.0,
        y: 1.0,
        z: 1.0,
    };
}
