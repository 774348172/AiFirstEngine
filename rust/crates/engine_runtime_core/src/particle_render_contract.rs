//! Renderer-facing particle commands. Project code does not submit GPU bindings.
use crate::render_graph::OrderedF32;
use serde::{Deserialize, Serialize};
fn zero_offset() -> [OrderedF32; 3] {
    [0.0.into(); 3]
}
#[cfg_attr(test, path = "particle_render_contract/scene.rs")]
mod scene;
pub(crate) use scene::bind_scene;
#[doc(hidden)]
pub use scene::validate_plan;

/// Column-major linear part of the evaluated emitter affine transform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParticleAffine {
    pub columns: [[OrderedF32; 3]; 3],
}
impl Default for ParticleAffine {
    fn default() -> Self {
        Self {
            columns: std::array::from_fn(|c| {
                std::array::from_fn(|r| (if c == r { 1. } else { 0. }).into())
            }),
        }
    }
}
impl ParticleAffine {
    pub fn inverse(&self) -> Result<[[f32; 3]; 3], String> {
        let m = self.columns.map(|c| c.map(OrderedF32::to_f32));
        if !m
            .iter()
            .flatten()
            .all(|v| v.is_finite() && v.abs() <= 32768.)
        {
            return Err("particle_effect.invalid_affine".into());
        }
        let cross = |a: [f32; 3], b: [f32; 3]| {
            [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ]
        };
        let rows = [cross(m[1], m[2]), cross(m[2], m[0]), cross(m[0], m[1])];
        let det: f32 = m[0].iter().zip(rows[0]).map(|(a, b)| a * b).sum();
        if !det.is_finite() || det.abs() < 1e-12 {
            return Err("particle_effect.singular_affine".into());
        }
        let inv = std::array::from_fn(|c| std::array::from_fn(|r| rows[r][c] / det));
        if !inv
            .iter()
            .flatten()
            .all(|v| v.is_finite() && v.abs() <= 32768.)
        {
            return Err("particle_effect.invalid_affine_inverse".into());
        }
        Ok(inv)
    }
}

#[cfg(test)]
mod affine_tests {
    use super::*;
    #[test]
    fn particle_affine_preserves_shear_mirror_inverse_and_rejects_degenerate_inputs() {
        let a = ParticleAffine {
            columns: [[0., 2., 0.], [-3., 0., 0.], [0.5, 0., -1.]].map(|c| c.map(Into::into)),
        };
        let inv = a.inverse().unwrap();
        for c in 0..3 {
            for r in 0..3 {
                let actual: f32 = (0..3).map(|k| a.columns[k][r].to_f32() * inv[c][k]).sum();
                assert!((actual - if c == r { 1. } else { 0. }).abs() < 1e-6);
            }
        }
        for bad in [0., f32::NAN, f32::INFINITY, 32769.] {
            let mut v = ParticleAffine::default();
            v.columns[0][0] = bad.into();
            assert!(v.inverse().is_err());
        }
        // OrderedF32 packets serialize the exact IEEE bits rather than JSON decimals.
        let old: ParticleRenderStep = serde_json::from_value(serde_json::json!({
            "stepId": 1, "deltaSeconds": 0.01f32.to_bits(), "origin": [0, 0, 0],
            "emitting": true, "paused": false
        }))
        .unwrap();
        assert_eq!(old.emitter_affine, Default::default());
    }
}

/// Immutable per-source projection. Large package data is shared between frame packets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParticleSourceFrame {
    /// Stable simulation coordinate anchor, encoded losslessly for packet Eq.
    #[serde(default)]
    pub simulation_origin_bits: [u64; 3],
    pub instance: u64,
    pub epoch: u64,
    pub assets: std::sync::Arc<str>,
    pub parameters: std::sync::Arc<str>,
    pub textures: Vec<Option<crate::render_resource::RenderResourceHandle>>,
    pub meshes: Vec<Option<crate::render_resource::RenderResourceHandle>>,
    pub step: ParticleRenderStep,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticleRenderView {
    #[serde(default)]
    pub emitter_affine: ParticleAffine,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fog: Option<crate::color_pipeline::DistanceFog3D>,
    #[serde(default = "zero_offset")]
    pub camera_position: [OrderedF32; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene_view: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth_target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewport: Option<[u32; 4]>,
    #[serde(default = "zero_offset")]
    pub simulation_to_view: [OrderedF32; 3],
    /// Column-major world-to-clip matrix, WGPU depth range 0..1.
    pub world_to_clip: [[OrderedF32; 4]; 4],
    pub right: [OrderedF32; 3],
    pub up: [OrderedF32; 3],
    pub forward: [OrderedF32; 3],
    pub origin: [OrderedF32; 3],
}
impl Default for ParticleRenderView {
    fn default() -> Self {
        Self {
            emitter_affine: Default::default(),
            fog: None,
            camera_position: [0.0.into(); 3],
            scene_view: None,
            depth_target: None,
            viewport: None,
            simulation_to_view: [0.0.into(); 3],
            world_to_clip: std::array::from_fn(|c| {
                std::array::from_fn(|r| (if c == r { 1.0 } else { 0.0 }).into())
            }),
            right: [1.0.into(), 0.0.into(), 0.0.into()],
            up: [0.0.into(), 1.0.into(), 0.0.into()],
            forward: [0.0.into(), 0.0.into(), 1.0.into()],
            origin: [0.0.into(); 3],
        }
    }
}
impl ParticleRenderView {
    pub fn from_scene_view(
        view: Option<&crate::render_state::RenderViewState>,
        width: u32,
        height: u32,
        origin: [OrderedF32; 3],
    ) -> Self {
        let mut result = Self {
            origin,
            ..Default::default()
        };
        if let Some(view) = view {
            let v = &view.view_matrix;
            let p = &view.projection_matrix;
            result.world_to_clip = std::array::from_fn(|c| {
                std::array::from_fn(|r| {
                    (0..4)
                        .map(|k| p[k * 4 + r] * v[c * 4 + k])
                        .sum::<f32>()
                        .into()
                })
            });
            result.right = [v[0].into(), v[4].into(), v[8].into()];
            result.up = [v[1].into(), v[5].into(), v[9].into()];
            result.forward = [(-v[2]).into(), (-v[6]).into(), (-v[10]).into()];
        } else {
            let half_height = crate::runtime_renderer::DEFAULT_2D_ORTHOGRAPHIC_HALF_HEIGHT;
            result.world_to_clip[0][0] =
                (1.0 / (half_height * width.max(1) as f32 / height.max(1) as f32)).into();
            result.world_to_clip[1][1] = (1.0 / half_height).into();
            result.world_to_clip[2][2] = (-0.001).into();
            result.world_to_clip[3][2] = 0.5.into();
            result.forward = [0.0.into(), 0.0.into(), (-1.0).into()];
        }
        result
    }
    pub fn validate(&self) -> Result<(), String> {
        self.emitter_affine.inverse()?;
        if let Some(fog) = &self.fog {
            crate::color_pipeline::ColorPipeline3D {
                fog: Some(fog.clone()),
                ..Default::default()
            }
            .validate()
            .map_err(|e| format!("particle_render.invalid_fog:{e}"))?;
            if self.scene_view.is_none() {
                return Err("particle_render.unbound_fog".into());
            }
        }
        if !self
            .world_to_clip
            .iter()
            .flatten()
            .chain(&self.right)
            .chain(&self.up)
            .chain(&self.forward)
            .chain(&self.origin)
            .chain(&self.simulation_to_view)
            .chain(&self.camera_position)
            .all(|x| x.to_f32().is_finite())
        {
            return Err("particle_render.nonfinite_view".into());
        }
        if self.depth_target.as_ref().is_some_and(|v| v.is_empty())
            || self.depth_target.is_some() != self.viewport.is_some()
            || self.depth_target.is_some() != self.scene_view.is_some()
            || self.viewport.is_some_and(|v| v[2] == 0 || v[3] == 0)
        {
            return Err("particle_render.invalid_depth_view".into());
        }
        for axis in [&self.right, &self.up, &self.forward] {
            if (axis.iter().map(|x| x.to_f32().powi(2)).sum::<f32>() - 1.0).abs() > 1e-4 {
                return Err("particle_render.invalid_camera_basis".into());
            }
        }
        for (a, b) in [
            (&self.right, &self.up),
            (&self.right, &self.forward),
            (&self.up, &self.forward),
        ] {
            if a.iter()
                .zip(b)
                .map(|(x, y)| x.to_f32() * y.to_f32())
                .sum::<f32>()
                .abs()
                > 1e-4
            {
                return Err("particle_render.invalid_camera_basis".into());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticleRenderStep {
    #[serde(default)]
    pub emitter_affine: ParticleAffine,
    pub step_id: u64,
    pub delta_seconds: OrderedF32,
    pub origin: [OrderedF32; 3],
    pub emitting: bool,
    pub paused: bool,
}
