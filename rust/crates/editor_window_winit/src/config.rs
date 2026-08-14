use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeEditorWindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub resizable: bool,
    #[serde(alias = "dpi_scale")]
    pub scale_factor: f64,
}

impl Default for NativeEditorWindowConfig {
    fn default() -> Self {
        Self {
            title: "AI First Engine Editor".to_string(),
            width: 1280,
            height: 720,
            resizable: true,
            scale_factor: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LogicalPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PhysicalPoint {
    pub x: f64,
    pub y: f64,
}

pub fn physical_to_logical(point: PhysicalPoint, scale_factor: f64) -> LogicalPoint {
    let scale_factor = scale_factor.max(f64::EPSILON);
    LogicalPoint {
        x: point.x / scale_factor,
        y: point.y / scale_factor,
    }
}

pub fn logical_to_physical(point: LogicalPoint, scale_factor: f64) -> PhysicalPoint {
    PhysicalPoint {
        x: point.x * scale_factor,
        y: point.y * scale_factor,
    }
}
