//! CPU candidate preparation inside the existing neutral geometry owner. No backend readiness.
use super::*;
use project_runtime_sdk::generated_content::{
    Attribute3D, AttributePatch3D, ContentError, Geometry3D,
};
use project_runtime_sdk::ProjectRuntimeSpatialAssetRef;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub(crate) struct GeneratedGeometry {
    buffers: Arc<GeometryBuffers>,
    bounds: GeometryBounds,
    sections: Arc<[SubmeshRange]>,
    materials: Arc<BTreeMap<String, ProjectRuntimeSpatialAssetRef>>,
}

impl GeneratedGeometry {
    pub(crate) fn sdk_layout(
        &self,
    ) -> (
        u32,
        u32,
        Vec<project_runtime_sdk::generated_content::AttributeLayout3D>,
    ) {
        use project_runtime_sdk::generated_content::AttributeLayout3D;
        (
            self.buffers.vertex_count(),
            self.buffers.vertex_stride(),
            self.buffers
                .attributes()
                .iter()
                .map(|a| AttributeLayout3D {
                    semantic: match a.semantic {
                        VertexSemantic::Position => Attribute3D::Position,
                        VertexSemantic::Normal => Attribute3D::Normal,
                        VertexSemantic::TexCoord0 => Attribute3D::TexCoord0,
                        VertexSemantic::Color0 => Attribute3D::Color0,
                    },
                    offset: a.offset,
                })
                .collect(),
        )
    }

    pub(crate) fn materials(&self) -> impl Iterator<Item = (&str, &ProjectRuntimeSpatialAssetRef)> {
        self.materials
            .iter()
            .map(|(slot, reference)| (slot.as_str(), reference))
    }

    pub(crate) fn sections(&self) -> &[SubmeshRange] {
        &self.sections
    }

    pub(crate) fn cpu_backings(&self) -> [(usize, usize, std::sync::Weak<Vec<u8>>); 2] {
        [self.buffers.vertex_storage(), self.buffers.index_storage()].map(|storage| {
            (
                Arc::as_ptr(storage) as usize,
                storage.capacity(),
                Arc::downgrade(storage),
            )
        })
    }
    pub(crate) fn binding_request(
        &self,
        ticket: project_runtime_sdk::generated_content::ContentTicket,
        revision: u64,
    ) -> crate::render_resource::RenderResourceRequest {
        use crate::render_resource::*;
        RenderResourceRequest {
            // Different candidates must not stale the currently applied logical resource.
            key: RenderAssetKey::new(
                format!(
                    "runtime-generated:{}:{}:{revision}",
                    ticket.session_generation, ticket.request_id
                ),
                "1",
                RenderResourceKind::MeshBuffer,
            ),
            source: RenderResourceSource::MeshBufferDescriptor {
                vertex_count: self.buffers.vertex_count(),
                index_count: self.buffers.index_count(),
            },
            byte_len: self.buffers.vertex_byte_len() + self.buffers.index_byte_len(),
            reason: "generated-content-candidate".into(),
        }
    }
    /// Consume original resource-owner bindings and an existing prepared material.
    /// The canonical buffers stay shared through native upload, frames and recovery.
    pub(crate) fn bind(
        &self,
        section: usize,
        geometry_binding: Arc<RenderBindingSet>,
        geometry_source: Arc<Provenance>,
        material: &PreparedCookedRenderAsset,
        manager: &RenderResourceManager,
    ) -> Result<PreparedGeometry, ContractError> {
        if geometry_binding.binding_kind != RenderBindingKind::Mesh
            || material.binding.binding_kind != RenderBindingKind::Material
        {
            return Err(invalid("bindingKind"));
        }
        if material.state != RenderAssetPreparationState::Prepared {
            return Err(ContractError::new(
                ContractErrorCode::ResourceNotReady,
                "preparedAsset",
            ));
        }
        geometry_binding.validate_residency(manager)?;
        material.binding.validate_residency(manager)?;
        let submesh = self
            .sections
            .get(section)
            .ok_or_else(|| invalid("submesh"))?
            .clone();
        let expected = &self.materials[&submesh.material_slot];
        if expected
            .guid
            .as_ref()
            .is_some_and(|guid| guid != &material.asset_guid)
        {
            return Err(ContractError::new(
                ContractErrorCode::AssetIdentityConflict,
                "material.assetRef",
            ));
        }
        let RenderAssetData::Material(params) = &material.asset.asset else {
            return Err(invalid("material"));
        };
        params.validate()?;
        for slot in STANDARD_LIT_TEXTURE_SLOTS {
            if params.textures.contains_key(slot) != material.binding.texture(slot).is_some() {
                return Err(invalid("material.textures"));
            }
        }
        if params.preset == MaterialPreset::StandardLit {
            self.buffers.validate_standard_lit_inputs(
                submesh.first_index..submesh.first_index + submesh.index_count,
                params.textures.contains_key("normalTexture"),
                !params.textures.is_empty(),
            )?;
        }
        let prepared_material = prepared_material::PreparedMaterial::from(params.clone());
        prepared_material.values()?;
        Ok(PreparedGeometry {
            buffers: self.buffers.clone(),
            geometry_binding,
            material_binding: Arc::new(material.binding.clone()),
            geometry_source,
            material_source: asset_provenance(material),
            material: prepared_material,
            bounds: self.bounds.clone(),
            submesh,
            convention: CoordinateConvention::World3dRightHandedYUpMetersCameraNegativeZ,
        })
    }
    #[cfg(test)]
    pub(crate) fn vertex_pointer(&self) -> *const u8 {
        self.buffers.vertex_bytes().as_ptr()
    }
    pub(crate) fn owned_vertex_bytes(&self) -> usize {
        self.buffers.vertex_storage().capacity()
    }
    pub(crate) fn prepare(geometry: Geometry3D) -> Result<Self, ContentError> {
        geometry.validate()?;
        let position = geometry
            .attributes
            .iter()
            .find(|a| a.semantic == Attribute3D::Position)
            .expect("validated position attribute")
            .offset;
        let bounds = bounds(&geometry.vertex_bytes, geometry.vertex_stride, position)?;
        let sections: Arc<[SubmeshRange]> = geometry
            .sections
            .iter()
            .map(|section| SubmeshRange {
                first_index: section.first_index,
                index_count: section.index_count,
                material_slot: section.material_slot.clone(),
            })
            .collect::<Vec<_>>()
            .into();
        let materials: BTreeMap<_, _> = geometry
            .sections
            .into_iter()
            .map(|section| (section.material_slot, section.material))
            .collect();
        let cooked = CookedGeometry {
            vertex_bytes: geometry.vertex_bytes,
            index_bytes: geometry.index_bytes,
            vertex_stride: geometry.vertex_stride,
            attributes: geometry
                .attributes
                .into_iter()
                .map(|a| {
                    let (semantic, format) = attribute(a.semantic);
                    VertexAttribute {
                        semantic,
                        format,
                        offset: a.offset,
                    }
                })
                .collect(),
            vertex_count: geometry.vertex_count,
            index_count: geometry.index_count,
            index_format: match geometry.index_format {
                project_runtime_sdk::generated_content::IndexFormat3D::Uint16 => {
                    GeometryIndexFormat::Uint16
                }
                project_runtime_sdk::generated_content::IndexFormat3D::Uint32 => {
                    GeometryIndexFormat::Uint32
                }
            },
            topology: GeometryTopology::TriangleList,
            submeshes: sections.to_vec(),
            bounds: bounds.clone(),
            coordinate_convention: CoordinateConvention::World3dRightHandedYUpMetersCameraNegativeZ,
            material_slots: materials.keys().cloned().collect(),
        };
        let buffers = GeometryBuffers::from_cooked_owned(cooked)
            .map_err(|e| ContentError::invalid(e.field))?;
        Ok(Self {
            buffers: Arc::new(buffers),
            bounds,
            sections,
            materials: Arc::new(materials),
        })
    }
    pub(crate) fn patch(&self, patch: AttributePatch3D) -> Result<Self, ContentError> {
        patch.validate()?;
        if patch
            .first_vertex
            .checked_add(patch.vertex_count)
            .is_none_or(|end| end > self.buffers.vertex_count())
        {
            return Err(ContentError::invalid("patch.range"));
        }
        let semantic = attribute(patch.semantic).0;
        let offset = self
            .buffers
            .attributes()
            .iter()
            .find(|a| a.semantic == semantic)
            .ok_or_else(|| ContentError::invalid("patch.attribute"))?
            .offset as usize;
        // The session owner must reserve this full candidate backing before calling prepare/patch.
        let mut vertices = self.buffers.vertex_bytes().to_vec();
        let stride = self.buffers.vertex_stride() as usize;
        let width = patch.semantic.byte_len();
        for (vertex, bytes) in patch.bytes.chunks_exact(width).enumerate() {
            let start = (patch.first_vertex as usize + vertex) * stride + offset;
            vertices[start..start + width].copy_from_slice(bytes);
        }
        let bounds = if patch.semantic == Attribute3D::Position {
            bounds(&vertices, self.buffers.vertex_stride(), offset as u32)?
        } else {
            self.bounds.clone()
        };
        let buffers = self
            .buffers
            .with_vertex_bytes(vertices)
            .map_err(|e| ContentError::invalid(e.field))?;
        Ok(Self {
            buffers: Arc::new(buffers),
            bounds,
            sections: self.sections.clone(),
            materials: self.materials.clone(),
        })
    }
}

fn attribute(semantic: Attribute3D) -> (VertexSemantic, VertexFormat) {
    match semantic {
        Attribute3D::Position => (VertexSemantic::Position, VertexFormat::Float32x3),
        Attribute3D::Normal => (VertexSemantic::Normal, VertexFormat::Float32x3),
        Attribute3D::TexCoord0 => (VertexSemantic::TexCoord0, VertexFormat::Float32x2),
        Attribute3D::Color0 => (VertexSemantic::Color0, VertexFormat::Float32x4),
    }
}

/// Reconstruct the only candidate culling truth from its validated position stream.
fn bounds(vertices: &[u8], stride: u32, position: u32) -> Result<GeometryBounds, ContentError> {
    let read = |vertex: &[u8], axis: usize| {
        let offset = position as usize + axis * 4;
        f32::from_le_bytes(
            vertex[offset..offset + 4]
                .try_into()
                .expect("validated position layout"),
        )
    };
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for vertex in vertices.chunks_exact(stride as usize) {
        for axis in 0..3 {
            let value = read(vertex, axis);
            min[axis] = min[axis].min(value);
            max[axis] = max[axis].max(value);
        }
    }
    let sphere_center: [f32; 3] = std::array::from_fn(|axis| min[axis] * 0.5 + max[axis] * 0.5);
    let mut distance2 = 0f64;
    for vertex in vertices.chunks_exact(stride as usize) {
        let distance: f64 = sphere_center
            .iter()
            .enumerate()
            .map(|(axis, center)| (f64::from(read(vertex, axis)) - f64::from(*center)).powi(2))
            .sum();
        distance2 = distance2.max(distance);
    }
    let sphere_radius = distance2.sqrt() as f32;
    if !sphere_radius.is_finite() {
        return Err(ContentError::invalid("geometry.bounds.sphere"));
    }
    Ok(GeometryBounds {
        min,
        max,
        sphere_center,
        sphere_radius,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use project_runtime_sdk::generated_content::{GeometryBuilder3D, GeometrySection3D};
    fn geometry() -> Geometry3D {
        GeometryBuilder3D::new(&[[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]])
            .uv(&[[0., 0.], [1., 0.], [0., 1.]])
            .colors(&[[1., 1., 1., 1.]; 3])
            .triangles_u16(
                &[0, 1, 2],
                vec![GeometrySection3D {
                    first_index: 0,
                    index_count: 3,
                    material_slot: "surface".into(),
                    material: ProjectRuntimeSpatialAssetRef {
                        id: "material".into(),
                        asset_type: "material".into(),
                        guid: Some("material-guid".into()),
                        sub_asset: None,
                    },
                }],
            )
            .unwrap()
    }

    #[test]
    fn generated_content_376_mesh_cpu_prepare_moves_input_backing_into_existing_buffers() {
        let geometry = geometry();
        let vertex = geometry.vertex_bytes.as_ptr();
        let index = geometry.index_bytes.as_ptr();
        let prepared = GeneratedGeometry::prepare(geometry).unwrap();
        assert_eq!(prepared.buffers.vertex_bytes().as_ptr(), vertex);
        assert_eq!(prepared.buffers.index_bytes().as_ptr(), index);
        assert_eq!(prepared.buffers.vertex_count(), 3);
        assert_eq!(prepared.buffers.index_count(), 3);
        assert_eq!(prepared.bounds.min, [0., 0., 0.]);
        assert_eq!(prepared.bounds.max, [1., 1., 0.]);
        assert_eq!(prepared.bounds.sphere_center, [0.5, 0.5, 0.]);
        assert!((prepared.bounds.sphere_radius - 0.5f32.sqrt()).abs() < 1e-6);
        assert_eq!(prepared.sections[0].material_slot, "surface");
        assert_eq!(
            prepared.materials["surface"].guid.as_deref(),
            Some("material-guid")
        );
        let alias = prepared.clone();
        assert!(Arc::ptr_eq(&alias.buffers, &prepared.buffers));
    }

    #[test]
    fn generated_content_376_mesh_cpu_position_patch_rebuilds_bounds_and_shares_unchanged_index() {
        let old = GeneratedGeometry::prepare(geometry()).unwrap();
        let old_bytes = old.buffers.vertex_bytes().to_vec();
        let changed = old
            .patch(AttributePatch3D::positions(1, &[[100., -20., 3.]]).unwrap())
            .unwrap();
        assert_eq!(changed.bounds.min, [0., -20., 0.]);
        assert_eq!(changed.bounds.max, [100., 1., 3.]);
        assert_ne!(changed.bounds, old.bounds);
        assert_eq!(
            changed.buffers.index_bytes().as_ptr(),
            old.buffers.index_bytes().as_ptr()
        );
        assert_ne!(
            changed.buffers.vertex_bytes().as_ptr(),
            old.buffers.vertex_bytes().as_ptr()
        );
        assert_eq!(changed.buffers.attributes(), old.buffers.attributes());
        assert_eq!(old.buffers.vertex_bytes(), old_bytes);
        assert_eq!(old.bounds.max, [1., 1., 0.]);
        assert!(Arc::ptr_eq(&old.sections, &changed.sections));
        assert!(Arc::ptr_eq(&old.materials, &changed.materials));
    }

    #[test]
    fn generated_content_376_mesh_cpu_uv_and_color_patch_preserve_bounds_and_other_attributes() {
        let old = GeneratedGeometry::prepare(geometry()).unwrap();
        let changed = old
            .patch(AttributePatch3D::uv(1, &[[0.25, 0.75]]).unwrap())
            .unwrap();
        let changed = changed
            .patch(AttributePatch3D::colors(2, &[[0., 1., 0., 1.]]).unwrap())
            .unwrap();
        assert_eq!(changed.bounds, old.bounds);
        assert_eq!(changed.buffers.vertex_stride(), old.buffers.vertex_stride());
        assert_eq!(
            changed.buffers.index_bytes().as_ptr(),
            old.buffers.index_bytes().as_ptr()
        );
        for vertex in 0..3 {
            let offset = vertex * old.buffers.vertex_stride() as usize;
            assert_eq!(
                &changed.buffers.vertex_bytes()[offset..offset + 12],
                &old.buffers.vertex_bytes()[offset..offset + 12]
            );
        }
        let mut invalid = AttributePatch3D::positions(0, &[[0., 0., 0.]]).unwrap();
        invalid.bytes[..4].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(old.patch(invalid).is_err());
        assert!(old
            .patch(AttributePatch3D::positions(3, &[[0., 0., 0.]]).unwrap())
            .is_err());
    }

    #[test]
    fn generated_content_376_mesh_cpu_existing_contract_accepts_u32_sections_and_nonzero_position_offset(
    ) {
        let material = geometry().sections.remove(0).material;
        let mut geometry =
            GeometryBuilder3D::new(&[[-2., 0., 1.], [3., 0., 1.], [3., 4., 1.], [-2., 4., 1.]])
                .normals(&[[0., 0., 1.]; 4])
                .triangles_u32(
                    &[0, 1, 2, 0, 2, 3],
                    vec![
                        GeometrySection3D {
                            first_index: 0,
                            index_count: 3,
                            material_slot: "a".into(),
                            material: material.clone(),
                        },
                        GeometrySection3D {
                            first_index: 3,
                            index_count: 3,
                            material_slot: "b".into(),
                            material,
                        },
                    ],
                )
                .unwrap();
        // A valid layout need not put position first. Repack both streams with no semantic change.
        for vertex in geometry.vertex_bytes.chunks_exact_mut(24) {
            vertex.rotate_left(12);
        }
        for attribute in &mut geometry.attributes {
            attribute.offset = 12 - attribute.offset;
        }
        let prepared = GeneratedGeometry::prepare(geometry).unwrap();
        assert_eq!(prepared.buffers.index_format(), GeometryIndexFormat::Uint32);
        assert_eq!(prepared.bounds.min, [-2., 0., 1.]);
        assert_eq!(prepared.bounds.max, [3., 4., 1.]);
        assert_eq!(prepared.sections.len(), 2);
        assert_eq!(prepared.materials.len(), 2);
        let changed = prepared
            .patch(AttributePatch3D::normals(2, &[[0., 1., 0.]]).unwrap())
            .unwrap();
        assert_eq!(changed.bounds, prepared.bounds);
        assert_eq!(
            changed.buffers.index_bytes().as_ptr(),
            prepared.buffers.index_bytes().as_ptr()
        );
        let changed = changed
            .patch(AttributePatch3D::positions(3, &[[-10., 8., -5.]]).unwrap())
            .unwrap();
        assert_eq!(changed.bounds.min, [-10., 0., -5.]);
        assert_eq!(changed.bounds.max, [3., 8., 1.]);
    }

    #[test]
    fn generated_content_376_mesh_cpu_rejects_invalid_index_and_unrepresentable_bounds() {
        let mut invalid = geometry();
        invalid.index_bytes[..2].copy_from_slice(&9u16.to_le_bytes());
        assert!(GeneratedGeometry::prepare(invalid).is_err());
        let mut large = geometry();
        large.vertex_bytes[..4].copy_from_slice(&(-f32::MAX).to_le_bytes());
        let second = large.vertex_stride as usize;
        large.vertex_bytes[second..second + 4].copy_from_slice(&f32::MAX.to_le_bytes());
        // A representable bounding sphere can still cover +/-MAX along one axis.
        let prepared = GeneratedGeometry::prepare(large).unwrap();
        assert!(prepared.bounds.sphere_radius.is_finite());
        let mut diagonal = geometry();
        for vertex in 0..3 {
            for axis in 0..3 {
                let offset = vertex * diagonal.vertex_stride as usize + axis * 4;
                let value = if vertex == 0 { -f32::MAX } else { f32::MAX };
                diagonal.vertex_bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        assert!(GeneratedGeometry::prepare(diagonal).is_err());
    }
}

#[cfg(test)]
#[path = "generated_binding_tests.rs"]
pub(crate) mod binding_tests;
