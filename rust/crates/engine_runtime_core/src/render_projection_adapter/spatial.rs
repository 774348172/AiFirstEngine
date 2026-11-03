//! Typed adapters in the existing RenderProjection owner.
use super::*;
use crate::ids::RuntimeEntityId;
use crate::spatial_contract::{Camera3D, MeshRenderer3D};
use crate::world::SpatialWorldState;

#[derive(Debug, Clone, PartialEq)]
pub struct MeshProjection {
    #[doc(hidden)]
    pub presentation: Option<std::sync::Arc<crate::model_lod3d::MeshPresentation3D>>,
    pub runtime_entity_id: RuntimeEntityId,
    pub component: MeshRenderer3D,
    pub world: SpatialWorldState,
    pub provenance: Provenance,
    /// Origin used by the evaluated presentation matrix; simulation is anchor-local.
    pub coordinate_origin: [f64; 3],
    pub deformation: Option<std::sync::Arc<crate::character_animation::SkinDeformation3D>>,
}

pub struct MeshRenderer3DAdapter<'a> {
    pub runtime_entity_id: RuntimeEntityId,
    pub world: &'a SpatialWorldState,
}
impl RenderProjectionAdapter<MeshRenderer3D> for MeshRenderer3DAdapter<'_> {
    type Projection = MeshProjection;
    fn project(&self, source: &SourceEntityId, component: &MeshRenderer3D) -> MeshProjection {
        MeshProjection {
            presentation: None,
            runtime_entity_id: self.runtime_entity_id,
            component: component.clone(),
            world: self.world.clone(),
            provenance: provenance(source, "MeshRenderer3D"),
            coordinate_origin: self.world.anchor,
            deformation: None,
        }
    }
}
impl MeshProjection {
    pub fn visibility(&self) -> Visibility {
        Visibility {
            authored: self.world.authored_visible,
            inherited: Evaluation::Known(self.world.inherited_visible && self.world.enabled),
        }
    }
    /// Stable across proxy allocation, renaming and submesh array reordering.
    /// GeometryFrame rejects duplicate IDs (including any hash collision).
    pub fn submesh_instance_id(&self, material_slot: &str) -> u64 {
        let bytes = serde_json::to_vec(&(
            self.runtime_entity_id.index,
            self.runtime_entity_id.generation,
            &self.component.mesh.0.guid,
            &self.component.mesh.0.id,
            material_slot,
            self.presentation
                .as_ref()
                .and_then(|p| p.source_instance.as_deref()),
        ))
        .expect("serializable identity");
        u64::from_str_radix(&crate::canonical_digest::sha256_hex(&bytes)[..16], 16)
            .expect("hex digest")
    }
    pub fn geometry_instance<'a>(
        &self,
        source: &'a crate::render_geometry::PreparedGeometry,
    ) -> Result<crate::render_geometry::GeometryInstance<'a>, ContractError> {
        let slot = source.material_slot();
        if !self.component.materials.contains_key(slot) {
            return Err(ContractError::new(
                ContractErrorCode::InvalidBinding,
                "materials.slot",
            ));
        }
        Ok(crate::render_geometry::GeometryInstance {
            instance_id: self.submesh_instance_id(slot),
            source,
            world_transform: self.world.world_matrix,
            visibility: self.visibility(),
            layers: LayerSelection::Named {
                names: self.component.layers.clone(),
            },
            provenance: self.provenance.clone(),
        })
    }
    /// Bounds are supplied by the actual validated mesh, never an entity-scale cube.
    /// Transform all corners to retain parent-induced shear and mirrored scale.
    pub fn world_bounds(
        &self,
        mesh: &CookedGeometry,
    ) -> Result<([f32; 3], [f32; 3]), ContractError> {
        mesh.validate()?;
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for bits in 0..8 {
            let p = std::array::from_fn::<_, 3, _>(|i| {
                if bits & (1 << i) == 0 {
                    mesh.bounds.min[i]
                } else {
                    mesh.bounds.max[i]
                }
            });
            let m = self.world.world_matrix;
            for i in 0..3 {
                let v = m[i] * p[0] + m[4 + i] * p[1] + m[8 + i] * p[2] + m[12 + i];
                if !v.is_finite() {
                    return Err(ContractError::new(
                        ContractErrorCode::NonFinite,
                        "worldBounds",
                    ));
                }
                min[i] = min[i].min(v);
                max[i] = max[i].max(v);
            }
        }
        Ok((min, max))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CameraProjection {
    pub lights: std::sync::Arc<Vec<super::LightProjection>>,
    pub view: RenderView,
    pub pose: CameraPose,
    pub suspended: bool,
    pub origin: [f64; 3],
    pub coordinate_origin: [f64; 3],
}
impl CameraProjection {
    pub fn executable(&self) -> Result<Option<ExecutableRenderView>, ContractError> {
        if self.suspended {
            Ok(None)
        } else {
            self.view.evaluate(self.pose).map(Some)
        }
    }
}
pub struct Camera3DAdapter<'a> {
    pub runtime_entity_id: RuntimeEntityId,
    pub world: &'a SpatialWorldState,
    pub extent: [u32; 2],
}
impl RenderProjectionAdapter<Camera3D> for Camera3DAdapter<'_> {
    type Projection = CameraProjection;
    fn project(&self, source: &SourceEntityId, component: &Camera3D) -> CameraProjection {
        let key = serde_json::to_vec(&(
            "world3d-camera",
            self.runtime_entity_id.index,
            self.runtime_entity_id.generation,
        ))
        .expect("camera identity");
        let id = u64::from_str_radix(&crate::canonical_digest::sha256_hex(&key)[..16], 16)
            .expect("hex identity");
        let mut view = RenderView::perspective(id, self.extent[0], self.extent[1], 60., 0.1, 100.);
        view.source_camera_entity_id = Some(source.as_str().into());
        view.projection = component.projection.clone();
        view.color_pipeline = component.color_pipeline.clone();
        view.layers = LayerSelection::Named {
            names: component.layers.clone(),
        };
        view.clear_policy = ClearPolicy::Color {
            color: component.clear_color,
        };
        view.provenance = provenance(source, "Camera3D");
        CameraProjection {
            lights: Default::default(),
            view,
            pose: self.world.pose,
            suspended: self.extent.contains(&0),
            origin: self.world.anchor,
            coordinate_origin: self.world.anchor,
        }
    }
}
fn provenance(source: &SourceEntityId, component: &str) -> Provenance {
    Provenance {
        source_entity_id: Some(source.as_str().into()),
        source_component: Some(component.into()),
        availability: SourceAvailability::SuppliedContext,
        ..Provenance::unspecified(ProducerStage::RenderProjection)
    }
}
