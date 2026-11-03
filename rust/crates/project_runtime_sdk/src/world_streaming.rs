//! High-level streaming source DTO. No backend handles or load tickets cross this contract.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeStreamingSource3D {
    pub id: String,
    pub position: [f64; 3],
    pub activation_radius: f64,
    pub prefetch_radius: f64,
    pub priority: i32,
}
impl ProjectRuntimeStreamingSource3D {
    pub fn validate(&self) -> bool {
        !self.id.trim().is_empty()
            && self.id.len() <= 128
            && self.position.iter().all(|v| v.is_finite())
            && self.activation_radius.is_finite()
            && self.activation_radius >= 0.
            && self.prefetch_radius.is_finite()
            && self.prefetch_radius >= self.activation_radius
            && self.prefetch_radius <= 100_000.
    }
}
