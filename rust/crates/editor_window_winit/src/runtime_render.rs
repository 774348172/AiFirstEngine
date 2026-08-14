use crate::viewport::{
    GizmoState, SceneCameraState, SelectionOutlineState, ViewportOutputKind, ViewportState,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeFrameDescriptor {
    pub frame_id: u64,
    pub viewport_id: String,
    pub target_id: Option<String>,
    pub texture_id: Option<String>,
    pub output_kind: ViewportOutputKind,
    pub clear_color: [u8; 4],
    pub test_geometry_kind: String,
    pub camera_state: SceneCameraState,
    pub selection_outline_state: SelectionOutlineState,
    pub gizmo_state: GizmoState,
    pub frame_hash: String,
}

pub struct HeadlessRuntimeRenderer;

impl HeadlessRuntimeRenderer {
    pub fn render(frame_id: u64, viewport: &ViewportState) -> RuntimeFrameDescriptor {
        let test_geometry_kind = match viewport.output_kind {
            ViewportOutputKind::Clear => "none",
            ViewportOutputKind::TestTriangle => "test-triangle",
            ViewportOutputKind::TestTexture => "test-texture",
        }
        .to_string();
        let mut frame = RuntimeFrameDescriptor {
            frame_id,
            viewport_id: viewport.viewport_id.clone(),
            target_id: None,
            texture_id: None,
            output_kind: viewport.output_kind.clone(),
            clear_color: [20, 24, 28, 255],
            test_geometry_kind,
            camera_state: viewport.camera_state.clone(),
            selection_outline_state: viewport.selection_outline_state.clone(),
            gizmo_state: viewport.gizmo_state.clone(),
            frame_hash: String::new(),
        };
        frame.frame_hash = hash_stable(&frame_without_hash(&frame));
        frame
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct RuntimeFrameDescriptorNoHash {
    frame_id: u64,
    viewport_id: String,
    target_id: Option<String>,
    texture_id: Option<String>,
    output_kind: ViewportOutputKind,
    clear_color: [u8; 4],
    test_geometry_kind: String,
    camera_state: SceneCameraState,
    selection_outline_state: SelectionOutlineState,
    gizmo_state: GizmoState,
}

fn frame_without_hash(frame: &RuntimeFrameDescriptor) -> RuntimeFrameDescriptorNoHash {
    RuntimeFrameDescriptorNoHash {
        frame_id: frame.frame_id,
        viewport_id: frame.viewport_id.clone(),
        target_id: frame.target_id.clone(),
        texture_id: frame.texture_id.clone(),
        output_kind: frame.output_kind.clone(),
        clear_color: frame.clear_color,
        test_geometry_kind: frame.test_geometry_kind.clone(),
        camera_state: frame.camera_state.clone(),
        selection_outline_state: frame.selection_outline_state.clone(),
        gizmo_state: frame.gizmo_state.clone(),
    }
}

fn hash_stable<T: Serialize>(value: &T) -> String {
    let text = serde_json::to_string(value).expect("C-min frame descriptor should serialize");
    let mut hash = 2166136261u32;
    for byte in text.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(16777619);
    }
    format!("{hash:08x}")
}
