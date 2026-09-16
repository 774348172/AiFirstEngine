//! Renderer-facing particle commands. Project code does not submit GPU bindings.
use crate::render_graph::OrderedF32;
use serde::{Deserialize, Serialize};

/// Immutable per-source projection. Large package data is shared between frame packets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParticleSourceFrame {
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
        if !self
            .world_to_clip
            .iter()
            .flatten()
            .chain(&self.right)
            .chain(&self.up)
            .chain(&self.forward)
            .chain(&self.origin)
            .all(|x| x.to_f32().is_finite())
        {
            return Err("particle_render.nonfinite_view".into());
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
    pub step_id: u64,
    pub delta_seconds: OrderedF32,
    pub origin: [OrderedF32; 3],
    pub emitting: bool,
    pub paused: bool,
}

#[cfg(feature = "real-wgpu")]
impl From<&ParticleRenderStep> for crate::particle_gpu::ParticleGpuStep {
    fn from(v: &ParticleRenderStep) -> Self {
        Self {
            step_id: v.step_id,
            delta_seconds: v.delta_seconds.to_f32(),
            origin: v.origin.map(|x| x.to_f32()),
            emitting: v.emitting,
            paused: v.paused,
        }
    }
}
