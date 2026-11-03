//! Spine frames enter the existing neutral Prepare, resource leases and geometry workset.
use super::*;
use crate::render_asset_production::*;
use crate::render_contract::*;
use crate::render_projection_adapter::{spine::*, RenderProjectionAdapter};
use crate::render_resource::*;
use crate::{ids::EntityId, spine_animation::session::SpineModule, world::World};

type PreparedSpineGeometry = (
    Arc<engine_spine_runtime::Geometry>,
    Vec<crate::spine_animation::SpinePage>,
    Vec<(u32, Arc<PreparedMeshRenderer>)>,
);

#[derive(Debug, Clone)]
pub(in crate::render_geometry) struct SpineProjection {
    pub matrix: [f32; 16],
    pub visibility: Visibility,
    pub layers: Vec<String>,
    pub provenance: Provenance,
}
#[derive(Debug, Default)]
pub(super) struct SpinePreparedFrame {
    pub instances: Vec<Arc<RetainedInstance>>,
    pub textures: Vec<PreparedTextureAsset>,
    pub handles: HashSet<RenderResourceHandle>,
    pub changed: bool,
    pub error: Option<ContractError>,
    identity: String,
}
impl SpinePreparedFrame {
    pub fn retire(&mut self, frame: u64, manager: &mut RenderResourceManager) {
        for handle in self.handles.drain() {
            let _ = manager.release_resource_ownership(RenderResourceReleaseRequest {
                handle,
                frame_index: frame,
            });
        }
        // A replacement World must prepare current bindings rather than reuse
        // texture handles just released by the original reset owner.
        *self = Self::default();
    }
}
impl ProductionGeometryCache {
    pub(crate) fn project_spine(
        &mut self,
        frame: u64,
        world: &World,
        module: Option<&SpineModule>,
        loader: Option<&mut RuntimeAssetLoader>,
        manager: &mut RenderResourceManager,
    ) {
        let measurement = module.map(|m| &m.measurement);
        let started = measurement.and_then(|m| m.start());
        let mut next = SpinePreparedFrame::default();
        let result = (|| {
            let Some(module) = module else {
                return Ok(());
            };
            let sources = module
                .render_sources()
                .map_err(|_| invalid("spine.geometry"))?;
            if sources.is_empty() {
                return Ok(());
            }
            let loader = loader.ok_or_else(|| invalid("spine.runtime_package"))?;
            next.identity = format!(
                "{}:{}:{}",
                loader.package_dir().display(),
                loader.asset_revision_epoch(),
                module.registry_digest
            );
            // Owned keys prevent pointer recycling. This memo dies with the call;
            // the existing frame/packet leases retain all resulting resources.
            let mut prepared_geometry: Vec<PreparedSpineGeometry> = Vec::new();
            for (id, geometry, pages) in sources {
                let (matrix, visibility) = entity_transform(world, &id)?;
                let batches = if let Some((_, _, prepared)) = prepared_geometry
                    .iter()
                    .find(|(g, p, _)| Arc::ptr_eq(g, &geometry) && p == &pages)
                {
                    prepared.clone()
                } else {
                    let mut prepared = Vec::new();
                    for batch in SpineRendererAdapter.project(
                        &id,
                        &SpineGeometrySource {
                            geometry: &geometry,
                            pages: &pages,
                        },
                    )? {
                        let texture = if let Some(t) = next
                            .textures
                            .iter()
                            .find(|t| t.payload.asset_id == batch.texture_asset_id)
                        {
                            t.clone()
                        } else if let Some(t) = self.spine.textures.iter().find(|t| {
                            next.identity == self.spine.identity
                                && t.payload.asset_id == batch.texture_asset_id
                        }) {
                            next.textures.push(t.clone());
                            t.clone()
                        } else {
                            let payload = loader
                                .load_texture_payload(&crate::runtime_package::RuntimeAssetRef {
                                    id: batch.texture_asset_id.clone(),
                                    asset_type: "texture".into(),
                                    guid: Some(batch.texture_asset_id.clone()),
                                    sub_asset: None,
                                })
                                .map_err(|_| invalid("spine.atlas_payload"))?;
                            let t = RuntimeRenderAssetProducer.prepare_texture_payload(
                                frame,
                                payload,
                                RenderBindingKind::Texture,
                                manager,
                            )?;
                            next.textures.push(t.clone());
                            t
                        };
                        next.handles.insert(texture.handle.raw());
                        let mesh = prepare_dynamic(
                            frame,
                            RenderAssetData::Mesh(batch.geometry),
                            manager,
                            &mut next.handles,
                        )?;
                        let mut material = prepare_dynamic(
                            frame,
                            RenderAssetData::Material(batch.material),
                            manager,
                            &mut next.handles,
                        )?;
                        material.binding.slots.push(NamedRenderBinding {
                            name: "baseColorTexture".into(),
                            resource: BindingResource::Texture(texture.handle),
                        });
                        material.binding.sampler = texture.payload.sampler.clone();
                        material.textures.push(texture.clone());
                        let source = super::super::PreparedGeometry::from_assets(
                            &mesh, &material, 0, manager,
                        )?;
                        prepared.push((
                            batch.draw_order,
                            Arc::new(PreparedMeshRenderer {
                                geometry: vec![source],
                                textures: vec![texture],
                                retained_resources: vec![],
                            }),
                        ));
                    }
                    prepared_geometry.push((
                        Arc::clone(&geometry),
                        pages.clone(),
                        prepared.clone(),
                    ));
                    prepared
                };
                for (draw_order, prepared) in batches {
                    let runtime_id = world
                        .runtime_id_for_source(&id)
                        .ok_or_else(|| invalid("spine.entity_generation"))?;
                    let identity =
                        serde_json::to_vec(&("spine", runtime_id.index, runtime_id.generation))
                            .unwrap();
                    next.instances.push(Arc::new(RetainedInstance {
                        dynamic_bounds: None,
                        instance_id: (u64::from_str_radix(
                            &crate::canonical_digest::sha256_hex(&identity)[..12],
                            16,
                        )
                        .unwrap()
                            << 16)
                            | u64::from(draw_order),
                        projection: InstanceProjection::Spine(Arc::new(SpineProjection {
                            matrix,
                            visibility: visibility.clone(),
                            layers: vec!["main".into()],
                            provenance: Provenance {
                                source_entity_id: Some(id.as_str().into()),
                                source_component: Some("SpineAnimator".into()),
                                ..Provenance::unspecified(ProducerStage::RenderProjection)
                            },
                        })),
                        visibility: visibility.clone(),
                        submesh: 0,
                        prepared,
                    }));
                }
            }
            Ok(())
        })();
        next.error = result.err();
        next.changed = !next.instances.is_empty()
            || !self.spine.instances.is_empty()
            || next.error != self.spine.error;
        // The resource manager and frozen packet leases own retirement. An old
        // geometry frame keeps its backing until the existing GPU completion.
        for handle in next.handles.difference(&self.spine.handles) {
            let _ = manager.retain_resource_ownership(*handle);
        }
        for handle in self.spine.handles.difference(&next.handles) {
            let _ = manager.release_resource_ownership(RenderResourceReleaseRequest {
                handle: *handle,
                frame_index: frame,
            });
        }
        self.spine = next;
        if let Some(m) = measurement {
            m.finish(
                crate::spine_animation::measurement::Stage::Projection,
                started,
            );
        }
    }
}
fn invalid(field: &'static str) -> ContractError {
    ContractError::new(ContractErrorCode::InvalidBinding, field)
        .at_stage(ProducerStage::RenderProjection)
}
fn prepare_dynamic(
    frame: u64,
    asset: RenderAssetData,
    manager: &mut RenderResourceManager,
    handles: &mut HashSet<RenderResourceHandle>,
) -> Result<PreparedCookedRenderAsset, ContractError> {
    asset.validate()?;
    let digest = dynamic_asset_digest(&asset);
    let id = format!("spine:{digest}");
    let (kind, binding_kind, source, bytes) = match &asset {
        RenderAssetData::Mesh(m) => (
            RenderResourceKind::MeshBuffer,
            RenderBindingKind::Mesh,
            RenderResourceSource::MeshBufferDescriptor {
                vertex_count: m.vertex_count,
                index_count: m.index_count,
            },
            m.vertex_bytes.len() + m.index_bytes.len(),
        ),
        RenderAssetData::Material(m) => (
            RenderResourceKind::MaterialParams,
            RenderBindingKind::Material,
            RenderResourceSource::MaterialParamsDescriptor {
                param_count: m.parameters.len() as u32,
            },
            serde_json::to_vec(&m.parameters).unwrap().len(),
        ),
    };
    let request = RenderResourceRequest {
        key: RenderAssetKey::new(&id, &digest, kind),
        source,
        byte_len: bytes,
        reason: "spine-neutral-prepare".into(),
    };
    let handle = manager.request_resource(frame, request.clone());
    handles.insert(handle);
    let binding = RenderBindingSet::single(
        id.clone(),
        binding_kind,
        handle,
        "nearestClamp".into(),
        false,
        id.clone(),
    )?;
    Ok(PreparedCookedRenderAsset {
        asset: CookedRenderAsset {
            source_digest: digest.clone(),
            recipe: NATIVE_RENDER_COOK_RECIPE.into(),
            asset,
        },
        binding,
        textures: vec![],
        state: RenderAssetPreparationState::Prepared,
        resource_request: request,
        asset_guid: id,
        cooked_digest: digest,
        source_path: None,
        pipeline_identity: None,
    })
}
fn dynamic_asset_digest(asset: &RenderAssetData) -> String {
    let RenderAssetData::Mesh(mesh) = asset else {
        return crate::canonical_digest::sha256_prefixed(&serde_json::to_vec(asset).unwrap());
    };
    use sha2::{Digest, Sha256};
    // This process-local dynamic resource key is not a cooked package digest.
    // Keep every descriptor field, but hash raw buffers without JSON expansion.
    // Exhaustive destructuring makes new geometry fields require a decision here.
    let CookedGeometry {
        vertex_bytes,
        index_bytes,
        vertex_stride,
        attributes,
        vertex_count,
        index_count,
        index_format,
        topology,
        submeshes,
        bounds,
        coordinate_convention,
        material_slots,
    } = mesh;
    let descriptor = serde_json::to_vec(&(
        vertex_stride,
        attributes,
        vertex_count,
        index_count,
        index_format,
        topology,
        submeshes,
        bounds,
        coordinate_convention,
        material_slots,
    ))
    .unwrap();
    let mut hash = Sha256::new();
    hash.update(b"spine-neutral-mesh\0");
    for bytes in [
        vertex_bytes.as_slice(),
        index_bytes.as_slice(),
        descriptor.as_slice(),
    ] {
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    format!("sha256:{:x}", hash.finalize())
}
fn entity_transform(
    world: &World,
    id: &EntityId,
) -> Result<([f32; 16], Visibility), ContractError> {
    let mut chain = Vec::new();
    let mut cursor = Some(id.clone());
    let mut visible = true;
    while let Some(id) = cursor {
        if chain.len() >= world.entity_count() {
            return Err(invalid("spine.hierarchy"));
        }
        let meta = world.entity(&id).ok_or_else(|| invalid("spine.entity"))?;
        visible &= meta.alive && meta.enabled;
        let t = world
            .transform(&id)
            .ok_or_else(|| invalid("spine.transform"))?;
        let (s, c) = t.local_rotation.z.to_radians().sin_cos();
        chain.push([
            c * t.local_scale.x,
            s * t.local_scale.x,
            0.,
            0.,
            -s * t.local_scale.y,
            c * t.local_scale.y,
            0.,
            0.,
            0.,
            0.,
            t.local_scale.z,
            0.,
            t.local_position.x,
            t.local_position.y,
            t.local_position.z,
            1.,
        ]);
        cursor = world.hierarchy(&id).and_then(|h| h.parent_id);
    }
    let matrix = chain
        .into_iter()
        .rev()
        .fold(IDENTITY_MATRIX, multiply_matrix);
    if matrix.iter().any(|v| !v.is_finite()) {
        return Err(invalid("spine.transform"));
    }
    Ok((
        matrix,
        Visibility {
            authored: true,
            inherited: Evaluation::Known(visible),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        components::{Hierarchy, Transform},
        math::Vec3,
    };
    fn digest_mesh() -> CookedGeometry {
        SpineRendererAdapter
            .project(
                &"digest".into(),
                &SpineGeometrySource {
                    geometry: &engine_spine_runtime::Geometry {
                        vertices: vec![
                            engine_spine_runtime::Vertex {
                                a: 1.,
                                ..Default::default()
                            };
                            3
                        ],
                        indices: vec![0, 1, 2],
                        batches: vec![engine_spine_runtime::DrawBatch {
                            page: 0,
                            blend: engine_spine_runtime::BlendMode::Normal,
                            first_index: 0,
                            index_count: 3,
                        }],
                        bounds: Some([0.; 4]),
                    },
                    pages: &[crate::spine_animation::SpinePage {
                        name: "page".into(),
                        texture_asset_id: "texture".into(),
                        pma: true,
                    }],
                },
            )
            .unwrap()
            .remove(0)
            .geometry
    }
    #[test]
    fn spine374_f_dynamic_mesh_digest_hashes_framed_binary_and_every_descriptor_field() {
        let mesh = digest_mesh();
        let key = dynamic_asset_digest(&RenderAssetData::Mesh(mesh.clone()));
        assert_eq!(
            key, "sha256:0ed07f162192c5909c8c6bd6443f7ff2371879cc543855dc7f6f49a2464b8025",
            "frozen small-mesh identity must use the bounded binary digest"
        );
        let mut variants = Vec::new();
        macro_rules! change {
            ($m:ident, $body:expr) => {{
                let mut $m = mesh.clone();
                $body;
                variants.push($m);
            }};
        }
        change!(m, m.vertex_bytes[0] ^= 1);
        change!(m, m.index_bytes[0] ^= 1);
        change!(m, m.vertex_stride += 4);
        change!(m, m.attributes[0].offset += 4);
        change!(m, m.vertex_count += 1);
        change!(m, m.index_count += 3);
        change!(m, m.index_format = GeometryIndexFormat::Uint16);
        change!(m, m.submeshes[0].index_count += 3);
        change!(m, m.bounds.max[0] = 1.);
        change!(
            m,
            m.coordinate_convention =
                CoordinateConvention::World3dRightHandedYUpMetersCameraNegativeZ
        );
        change!(m, m.material_slots[0].push('x'));
        // Same concatenation of bytes, different field boundaries must not collide.
        change!(m, m.index_bytes.insert(0, m.vertex_bytes.pop().unwrap()));
        for changed in variants {
            assert_ne!(key, dynamic_asset_digest(&RenderAssetData::Mesh(changed)));
        }
        assert_eq!(key, dynamic_asset_digest(&RenderAssetData::Mesh(mesh)));
    }

    #[test]
    fn spine374_f_dynamic_digest_single_recipe_owner_cost() {
        let mut mesh = digest_mesh();
        mesh.vertex_bytes = mesh.vertex_bytes[..36].repeat(512);
        mesh.vertex_count = 512;
        mesh.index_bytes = (0..2352u32).flat_map(|i| (i % 512).to_le_bytes()).collect();
        mesh.index_count = 2352;
        mesh.submeshes[0].index_count = 2352;
        mesh.validate().unwrap();
        let asset = RenderAssetData::Mesh(mesh);
        let started = std::time::Instant::now();
        for _ in 0..100 {
            std::hint::black_box(crate::canonical_digest::sha256_prefixed(
                &serde_json::to_vec(std::hint::black_box(&asset)).unwrap(),
            ));
        }
        let old = started.elapsed();
        let started = std::time::Instant::now();
        for _ in 0..100 {
            std::hint::black_box(dynamic_asset_digest(std::hint::black_box(&asset)));
        }
        eprintln!("single recipe digest, 100 calls: JSON={old:?}, framed binary={:?}; owner diagnostic only", started.elapsed());
    }
    #[test]
    fn spine374_e_parent_mirror_nonuniform_transform_applies_once_and_inherits_disable() {
        let mut world = World::new();
        let parent: EntityId = "parent".into();
        let child: EntityId = "child".into();
        let mut p = Transform::identity();
        p.local_position = Vec3 {
            x: 10.,
            y: 2.,
            z: 0.,
        };
        p.local_rotation.z = 90.;
        p.local_scale = Vec3 {
            x: -2.,
            y: 3.,
            z: 1.,
        };
        world
            .try_spawn_with_components(
                parent.clone(),
                "parent",
                "entity",
                false,
                Hierarchy {
                    parent_id: None,
                    sibling_order: 0,
                },
                Some(p),
                None,
            )
            .unwrap();
        let mut c = Transform::identity();
        c.local_position = Vec3 {
            x: 1.,
            y: 0.,
            z: 0.,
        };
        world
            .try_spawn_with_components(
                child.clone(),
                "child",
                "entity",
                true,
                Hierarchy {
                    parent_id: Some(parent),
                    sibling_order: 0,
                },
                Some(c),
                None,
            )
            .unwrap();
        let (m, visible) = entity_transform(&world, &child).unwrap();
        assert!((m[12] - 10.).abs() < 1e-5 && m[13].abs() < 1e-5);
        assert!((m[0]).abs() < 1e-5 && (m[1] + 2.).abs() < 1e-5 && (m[4] + 3.).abs() < 1e-5);
        assert_eq!(visible.inherited, Evaluation::Known(false));
        assert!(entity_transform(&world, &"missing".into()).is_err());
    }
}
