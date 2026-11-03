//! Retained local Graphics geometry. Raw/serialized extension meshes stay untrusted.
use crate::render_graph::{OrderedF32, RenderDrawVertex};
use project_runtime_sdk::aui_extension::UiExtensionPaintOutput;
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UiLocalGeometry {
    pub id: u64,
    pub vertices: Vec<RenderDrawVertex>,
    pub indices: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AuiMeshPayload {
    source: Arc<UiExtensionPaintOutput>,
    #[serde(skip)]
    pub(crate) local: Option<Arc<UiLocalGeometry>>,
}
impl PartialEq for AuiMeshPayload {
    fn eq(&self, other: &Self) -> bool {
        if Arc::ptr_eq(&self.source, &other.source) {
            return true;
        }
        self.source == other.source && self.local == other.local
    }
}
impl std::ops::Deref for AuiMeshPayload {
    type Target = UiExtensionPaintOutput;
    fn deref(&self) -> &Self::Target {
        &self.source
    }
}
impl From<UiExtensionPaintOutput> for AuiMeshPayload {
    fn from(source: UiExtensionPaintOutput) -> Self {
        Self {
            source: Arc::new(source),
            local: None,
        }
    }
}
impl AuiMeshPayload {
    pub(crate) fn graphics(source: UiExtensionPaintOutput) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let local = UiLocalGeometry {
            id: NEXT.fetch_add(1, Ordering::Relaxed),
            vertices: source
                .vertices
                .iter()
                .map(|v| RenderDrawVertex::new(v.position, v.color, v.uv))
                .collect(),
            indices: source.indices.clone(),
        };
        Self {
            source: Arc::new(source),
            local: Some(Arc::new(local)),
        }
    }
}

/// Immutable CPU packet; backend regenerates these parameters for every submission.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UiRetainedDraw {
    pub geometry: Arc<UiLocalGeometry>,
    pub parameters: [OrderedF32; 16],
    pub scissor: [u32; 4],
    pub target_extent: [u32; 2],
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_mesh_serialization_keeps_old_shape_without_granting_gpu_trust() {
        let source = UiExtensionPaintOutput {
            vertices: vec![project_runtime_sdk::aui_extension::UiExtensionMeshVertex {
                position: [0., 0.],
                color: [1.; 4],
                uv: [0.; 2],
            }],
            indices: vec![],
            asset_id: None,
            primitives: vec![],
        };
        let expected = serde_json::to_value(&source).unwrap();
        let mesh = AuiMeshPayload::graphics(source);
        assert!(Arc::ptr_eq(&mesh.source, &mesh.clone().source));
        assert_eq!(serde_json::to_value(&mesh).unwrap(), expected);
        let decoded: AuiMeshPayload = serde_json::from_value(expected).unwrap();
        assert!(
            decoded.local.is_none(),
            "serialized mesh forged retained eligibility"
        );
        assert!(mesh.local.is_some());
    }
}
