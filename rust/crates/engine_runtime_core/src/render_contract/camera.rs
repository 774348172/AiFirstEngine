//! Executable camera projection. Matrices are column-major, clip depth is 0..1.
use super::*;
use crate::render_graph::OrderedF32;

pub const IDENTITY_MATRIX: [f32; 16] = [
    1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
];

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CameraPose {
    pub position: [f32; 3],
    /// Unit quaternion x, y, z, w. Camera scale is deliberately not a lens parameter.
    pub rotation: [f32; 4],
}

impl Default for CameraPose {
    fn default() -> Self {
        Self {
            position: [0.; 3],
            rotation: [0., 0., 0., 1.],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutableRenderView {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_pipeline: Option<crate::color_pipeline::ColorPipeline3D>,

    pub view_id: u64,
    pub source_camera_entity_id: Option<String>,
    pub view_kind: ViewKind,
    pub version: u64,
    pub domain: RenderDomain,
    pub target_kind: TargetKind,
    pub viewport: [u32; 4],
    pub target_extent: [u32; 2],
    pub view_matrix: [OrderedF32; 16],
    pub projection_matrix: [OrderedF32; 16],
    pub clear_color: Option<[OrderedF32; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth_clear: Option<OrderedF32>,
    pub depth_policy: DepthPolicy,
    pub provenance: Provenance,
}

impl ExecutableRenderView {
    /// Legacy 2D target labels remain compatible; only the typed 3D shadow view is depth-only.
    pub fn is_shadow_map(&self) -> bool {
        self.target_kind == TargetKind::ShadowMap && self.domain == RenderDomain::World3d
    }
    pub fn view_projection(&self) -> [f32; 16] {
        multiply_matrix(
            self.projection_matrix.map(OrderedF32::to_f32),
            self.view_matrix.map(OrderedF32::to_f32),
        )
    }

    pub fn validate_extent(&self, width: u32, height: u32) -> Result<(), ContractError> {
        if let Some(pipeline) = &self.color_pipeline {
            pipeline.validate()?;
            if pipeline.sky.is_some() {
                let p = self.projection_matrix.map(OrderedF32::to_f32);
                // The first analytic sky contract accepts centered canonical lenses only.
                if p[0] <= 0.
                    || p[5] <= 0.
                    || [1, 2, 3, 4, 6, 7, 8, 9, 12, 13].iter().any(|i| p[*i] != 0.)
                    || !((p[11] == -1. && p[15] == 0.) || (p[11] == 0. && p[15] == 1.))
                {
                    return Err(unsupported("colorPipeline.sky.projection"));
                }
            }
            pipeline.validate_clear(self.clear_color.map(|c| c.map(OrderedF32::to_f32)))?;
            if self.domain != RenderDomain::World3d || self.is_shadow_map() {
                return Err(unsupported("colorPipeline.domain"));
            }
        }
        let [x, y, w, h] = self.viewport;
        if w == 0
            || h == 0
            || x.checked_add(w).is_none_or(|v| v > width)
            || y.checked_add(h).is_none_or(|v| v > height)
        {
            return Err(ContractError::new(
                ContractErrorCode::InvalidViewport,
                "viewport",
            ));
        }
        finite(&self.view_matrix.map(OrderedF32::to_f32), "viewMatrix")?;
        finite(
            &self.projection_matrix.map(OrderedF32::to_f32),
            "projectionMatrix",
        )?;
        if let Some(color) = self.clear_color {
            finite(&color.map(OrderedF32::to_f32), "clearColor")?;
        }
        if self.is_shadow_map() {
            if self.view_kind != ViewKind::Shadow
                || self.domain != RenderDomain::World3d
                || self.depth_policy != DepthPolicy::TestAndWrite
                || self.clear_color.is_some()
                || self.depth_clear.is_some_and(|v| v.to_f32() != 1.)
            {
                return Err(unsupported("shadow.view"));
            }
        } else if self.depth_clear.is_some() {
            return Err(unsupported("depthClear"));
        }
        if self.domain == RenderDomain::Ui {
            return Err(ContractError::new(
                ContractErrorCode::InvalidDomain,
                "domain",
            ));
        }
        Ok(())
    }
}

impl RenderView {
    /// `size` is the full vertical world extent. Pose is supplied separately by Projection.
    pub fn orthographic(
        view_id: u64,
        width: u32,
        height: u32,
        size: f32,
        near: f32,
        far: f32,
    ) -> Self {
        let mut view = Self::perspective(view_id, width, height, 60., near, far);
        view.domain = RenderDomain::World2d;
        view.coordinate_convention = CoordinateConvention::Legacy2d;
        view.projection = Projection::Orthographic { size, near, far };
        view.depth_policy = DepthPolicy::Disabled;
        view
    }

    pub fn evaluate(&self, pose: CameraPose) -> Result<ExecutableRenderView, ContractError> {
        self.validate()?;
        finite(&pose.position, "camera.position")?;
        finite(&pose.rotation, "camera.rotation")?;
        let norm: f32 = pose.rotation.iter().map(|v| v * v).sum();
        if (norm - 1.).abs() > 1e-4 {
            return Err(ContractError::new(
                ContractErrorCode::InvalidProjection,
                "camera.rotation",
            ));
        }
        let (view, projection) = match self.projection {
            Projection::LegacyMatrix { view, projection } => {
                if pose != CameraPose::default() {
                    return Err(unsupported("legacy.cameraPose"));
                }
                (view, projection)
            }
            _ => {
                let aspect = self.viewport[2] as f32 / self.viewport[3] as f32;
                let projection = match self.projection {
                    Projection::Orthographic { size, near, far } => [
                        2. / (size * aspect),
                        0.,
                        0.,
                        0.,
                        0.,
                        2. / size,
                        0.,
                        0.,
                        0.,
                        0.,
                        1. / (near - far),
                        0.,
                        0.,
                        0.,
                        near / (near - far),
                        1.,
                    ],
                    Projection::Perspective {
                        fov_y_degrees,
                        near,
                        far,
                    } => {
                        let f = 1. / (fov_y_degrees.to_radians() * 0.5).tan();
                        [
                            f / aspect,
                            0.,
                            0.,
                            0.,
                            0.,
                            f,
                            0.,
                            0.,
                            0.,
                            0.,
                            far / (near - far),
                            -1.,
                            0.,
                            0.,
                            near * (far / (near - far)),
                            0.,
                        ]
                    }
                    Projection::LegacyMatrix { .. } => unreachable!(),
                };
                (inverse_camera_pose(pose), projection)
            }
        };
        let evaluated = ExecutableRenderView {
            color_pipeline: self.color_pipeline.clone(),
            view_id: self.view_id,
            source_camera_entity_id: self.source_camera_entity_id.clone(),
            view_kind: self.view_kind,
            version: self.version,
            domain: self.domain,
            target_kind: self.target,
            viewport: self.viewport,
            view_matrix: view.map(Into::into),
            projection_matrix: projection.map(Into::into),
            target_extent: [
                self.viewport[0].saturating_add(self.viewport[2]),
                self.viewport[1].saturating_add(self.viewport[3]),
            ],
            clear_color: match self.clear_policy {
                ClearPolicy::Color { color } => Some(color.map(Into::into)),
                ClearPolicy::Load | ClearPolicy::Depth { .. } => None,
            },
            depth_clear: match self.clear_policy {
                ClearPolicy::Depth { depth } => Some(depth.into()),
                _ => None,
            },
            depth_policy: self.depth_policy,
            provenance: self.provenance.clone(),
        };
        evaluated.validate_extent(u32::MAX, u32::MAX)?;
        finite(&evaluated.view_projection(), "viewProjection")?;
        Ok(evaluated)
    }
}

fn inverse_camera_pose(pose: CameraPose) -> [f32; 16] {
    let [x, y, z, w] = pose.rotation;
    // Transpose of the camera's world rotation, then inverse translation.
    let mut view = [
        1. - 2. * (y * y + z * z),
        2. * (x * y - z * w),
        2. * (x * z + y * w),
        0.,
        2. * (x * y + z * w),
        1. - 2. * (x * x + z * z),
        2. * (y * z - x * w),
        0.,
        2. * (x * z - y * w),
        2. * (y * z + x * w),
        1. - 2. * (x * x + y * y),
        0.,
        0.,
        0.,
        0.,
        1.,
    ];
    let p = transform_point(
        view,
        [-pose.position[0], -pose.position[1], -pose.position[2], 0.],
    );
    view[12..15].copy_from_slice(&p[..3]);
    view
}

pub fn multiply_matrix(a: [f32; 16], b: [f32; 16]) -> [f32; 16] {
    std::array::from_fn(|i| {
        (0..4)
            .map(|k| a[(i % 4) + k * 4] * b[(i / 4) * 4 + k])
            .sum()
    })
}

pub fn transform_point(m: [f32; 16], p: [f32; 4]) -> [f32; 4] {
    std::array::from_fn(|row| (0..4).map(|col| m[col * 4 + row] * p[col]).sum())
}
