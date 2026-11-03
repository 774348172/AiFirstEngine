use super::*;
use crate::render_state::{
    RenderTargetKind, RenderViewId, RenderViewKind, RenderViewState, Viewport,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Projection {
    LegacyMatrix {
        view: [f32; 16],
        projection: [f32; 16],
    },
    Orthographic {
        size: f32,
        near: f32,
        far: f32,
    },
    Perspective {
        fov_y_degrees: f32,
        near: f32,
        far: f32,
    },
}

impl ValidateRenderContract for Projection {
    fn validate(&self) -> Result<(), ContractError> {
        let valid = match self {
            Self::LegacyMatrix { view, projection } => {
                finite(view, "projection.view")?;
                finite(projection, "projection.projection")?;
                true
            }
            Self::Orthographic { size, near, far } => {
                finite(&[*size, *near, *far], "projection")?;
                *size > 0.0 && near < far
            }
            Self::Perspective {
                fov_y_degrees,
                near,
                far,
            } => {
                finite(&[*fov_y_degrees, *near, *far], "projection")?;
                *fov_y_degrees > 0.0 && *fov_y_degrees < 180.0 && *near > 0.0 && near < far
            }
        };
        if valid {
            Ok(())
        } else {
            Err(ContractError::new(
                ContractErrorCode::InvalidProjection,
                "projection",
            ))
        }
    }
}

// Typed wire mirrors keep serde changes out of existing RenderViewState/ABI types.
macro_rules! legacy_kind {
    ($name:ident, $legacy:ident, $($variant:ident),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub enum $name { $($variant),+ }
        impl From<&$legacy> for $name {
            fn from(value: &$legacy) -> Self { match value { $($legacy::$variant => Self::$variant),+ } }
        }
        impl From<$name> for $legacy {
            fn from(value: $name) -> Self { match value { $($name::$variant => Self::$variant),+ } }
        }
    };
}
legacy_kind!(
    ViewKind,
    RenderViewKind,
    Game,
    SceneView,
    Preview,
    Shadow,
    Reflection
);
legacy_kind!(
    TargetKind,
    RenderTargetKind,
    Window,
    ViewportTexture,
    RenderTexture,
    ShadowMap,
    PreviewTexture
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DepthPolicy {
    Disabled,
    TestAndWrite,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ClearPolicy {
    Color {
        color: [f32; 4],
    },
    Load,
    /// Depth-only shadow attachment clear; later atlas tiles use Load.
    Depth {
        depth: f32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenderView {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_pipeline: Option<crate::color_pipeline::ColorPipeline3D>,

    pub view_id: u64,
    pub source_camera_entity_id: Option<String>,
    pub view_kind: ViewKind,
    pub domain: RenderDomain,
    pub projection: Projection,
    /// x, y, width, height in pixels. New typed views must have an active extent.
    pub viewport: [u32; 4],
    pub target: TargetKind,
    pub clear_policy: ClearPolicy,
    pub depth_policy: DepthPolicy,
    pub layers: LayerSelection,
    pub coordinate_convention: CoordinateConvention,
    pub version: u64,
    pub provenance: Provenance,
}

impl RenderView {
    pub fn from_legacy(view: &RenderViewState) -> Self {
        if let Some(native) = &view.spatial {
            return native.view.clone();
        }
        Self {
            color_pipeline: None,
            view_id: view.view_id.0,
            source_camera_entity_id: view.source_entity_id.as_ref().map(ToString::to_string),
            view_kind: (&view.view_kind).into(),
            domain: RenderDomain::World2d,
            projection: Projection::LegacyMatrix {
                view: view.view_matrix,
                projection: view.projection_matrix,
            },
            viewport: [
                view.viewport.x,
                view.viewport.y,
                view.viewport.width,
                view.viewport.height,
            ],
            target: (&view.target).into(),
            clear_policy: ClearPolicy::Color {
                color: view.clear_color,
            },
            depth_policy: DepthPolicy::Disabled,
            layers: LayerSelection::Legacy {
                mask: view.layer_mask.clone(),
            },
            coordinate_convention: CoordinateConvention::Legacy2d,
            version: view.version,
            provenance: Provenance {
                source_entity_id: view.source_entity_id.as_ref().map(ToString::to_string),
                ..Provenance::legacy(ProducerStage::RenderProjection)
            },
        }
    }

    pub fn perspective(
        view_id: u64,
        width: u32,
        height: u32,
        fov_y_degrees: f32,
        near: f32,
        far: f32,
    ) -> Self {
        Self {
            color_pipeline: None,
            view_id,
            source_camera_entity_id: None,
            view_kind: ViewKind::Game,
            domain: RenderDomain::World3d,
            projection: Projection::Perspective {
                fov_y_degrees,
                near,
                far,
            },
            viewport: [0, 0, width, height],
            target: TargetKind::Window,
            clear_policy: ClearPolicy::Color {
                color: [0.0, 0.0, 0.0, 1.0],
            },
            depth_policy: DepthPolicy::TestAndWrite,
            layers: LayerSelection::Named { names: vec![] },
            coordinate_convention: CoordinateConvention::World3dRightHandedYUpMetersCameraNegativeZ,
            version: 0,
            provenance: Provenance::unspecified(ProducerStage::ContractValidation),
        }
    }

    pub fn execution_support(&self) -> ExecutionSupport {
        if self.to_legacy().is_ok() {
            ExecutionSupport::LegacyCompatible
        } else if !matches!(self.projection, Projection::LegacyMatrix { .. })
            && self.validate().is_ok()
        {
            ExecutionSupport::NativeGeometry
        } else {
            ExecutionSupport::ContractOnly
        }
    }

    pub fn to_legacy(&self) -> Result<LegacyConversion<'_, RenderViewState>, ContractError> {
        let Projection::LegacyMatrix { view, projection } = &self.projection else {
            return Err(unsupported("projection"));
        };
        let LayerSelection::Legacy { mask } = &self.layers else {
            return Err(unsupported("layers"));
        };
        let ClearPolicy::Color { color } = &self.clear_policy else {
            return Err(unsupported("clearPolicy"));
        };
        if self.domain != RenderDomain::World2d
            || self.coordinate_convention != CoordinateConvention::Legacy2d
            || self.depth_policy != DepthPolicy::Disabled
        {
            return Err(unsupported("view"));
        }
        Ok(LegacyConversion {
            payload: RenderViewState {
                spatial: None,
                view_id: RenderViewId(self.view_id),
                source_entity_id: self.source_camera_entity_id.clone().map(Into::into),
                view_kind: self.view_kind.into(),
                viewport: Viewport {
                    x: self.viewport[0],
                    y: self.viewport[1],
                    width: self.viewport[2],
                    height: self.viewport[3],
                },
                target: self.target.into(),
                view_matrix: *view,
                projection_matrix: *projection,
                clear_color: *color,
                layer_mask: mask.clone(),
                version: self.version,
            },
            provenance: &self.provenance,
        })
    }
}

impl ValidateRenderContract for RenderView {
    fn validate(&self) -> Result<(), ContractError> {
        self.projection.validate()?;
        if let Some(pipeline) = &self.color_pipeline {
            pipeline.validate()?;
            pipeline.validate_clear(match self.clear_policy {
                ClearPolicy::Color { color } => Some(color),
                _ => None,
            })?;
            if self.domain != RenderDomain::World3d || self.target == TargetKind::ShadowMap {
                return Err(unsupported("colorPipeline.domain"));
            }
        }
        self.layers.validate()?;
        if let ClearPolicy::Color { color } = &self.clear_policy {
            finite(color, "clearPolicy.color")?;
        }
        if self.target == TargetKind::ShadowMap
            && !matches!(self.projection, Projection::LegacyMatrix { .. })
        {
            if self.view_kind != ViewKind::Shadow
                || self.domain != RenderDomain::World3d
                || self.depth_policy != DepthPolicy::TestAndWrite
                || !matches!(
                    self.clear_policy,
                    ClearPolicy::Depth { depth: 1.0 } | ClearPolicy::Load
                )
            {
                return Err(unsupported("shadow.view"));
            }
        } else if matches!(self.clear_policy, ClearPolicy::Depth { .. }) {
            return Err(unsupported("clearPolicy.depth"));
        }
        let legacy = matches!(self.projection, Projection::LegacyMatrix { .. });
        if !legacy
            && (self.viewport[2] == 0
                || self.viewport[3] == 0
                || self.viewport[0].checked_add(self.viewport[2]).is_none()
                || self.viewport[1].checked_add(self.viewport[3]).is_none())
        {
            return Err(ContractError::new(
                ContractErrorCode::InvalidViewport,
                "viewport",
            ));
        }
        let domain_ok = match self.domain {
            RenderDomain::World2d => {
                self.coordinate_convention == CoordinateConvention::Legacy2d
                    && !matches!(self.projection, Projection::Perspective { .. })
            }
            RenderDomain::World3d => {
                self.coordinate_convention
                    == CoordinateConvention::World3dRightHandedYUpMetersCameraNegativeZ
                    && !legacy
            }
            RenderDomain::Ui => false, // UI composition remains owned by UiProjection, not a new World view.
        };
        if !domain_ok {
            return Err(ContractError::new(
                ContractErrorCode::InvalidDomain,
                "domain",
            ));
        }
        Ok(())
    }
}
