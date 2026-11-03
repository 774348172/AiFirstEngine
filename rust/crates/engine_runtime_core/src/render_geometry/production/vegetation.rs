//! Decoration instances enter ordinary Prepare and the existing packet lease.
//! Asset requests live here only until the original loader can release them;
//! native resource admission/retirement stays in ProductionGeometryCache.
use super::*;
use crate::{
    render_asset_production::PackageTexturePixels,
    render_contract::{ProducerStage, RenderAssetData},
    render_resource::RenderResourceHandle,
    runtime_asset::{RuntimeAssetHandle, RuntimeAssetLoadState},
    runtime_package::RuntimeAssetRef,
    terrain3d::{VegetationAsset3D, VegetationCluster3D},
};
use std::{collections::HashMap, sync::Weak};

#[cfg(test)]
#[path = "vegetation_tests.rs"]
mod tests;

#[derive(Debug)]
struct Request {
    handle: RuntimeAssetHandle,
    asset: Option<Arc<VegetationAsset3D>>,
}

#[derive(Debug, Default)]
pub(super) struct VegetationPreparedFrame {
    pub instances: Vec<Arc<RetainedInstance>>,
    pub meshes: Vec<PreparedProjection>,
    pub textures: Vec<PreparedTextureAsset>,
    pub handles: HashSet<RenderResourceHandle>,
    pub changed: bool,
    pub error: Option<ContractError>,
    requests: BTreeMap<String, Request>,
    prepared: BTreeMap<String, Arc<PreparedMeshRenderer>>,
    identity: String,
    reset_pending: bool,
}

impl VegetationPreparedFrame {
    /// Called only after the original production owner releases its handle union.
    /// Keep loader requests until update receives the current loader, just as
    /// environment requests survive reset_world for balanced release.
    pub fn clear_resources(&mut self) {
        self.instances.clear();
        self.meshes.clear();
        self.textures.clear();
        self.handles.clear();
        self.prepared.clear();
        self.identity.clear();
        self.error = None;
        self.changed = true;
        self.reset_pending = true;
    }

    fn update(
        &mut self,
        frame: u64,
        scene: &RenderSceneState,
        package_identity: &str,
        loader: Option<&mut RuntimeAssetLoader>,
        manager: &mut RenderResourceManager,
        backings: &mut HashMap<RenderResourceHandle, Weak<super::super::GeometryBuffers>>,
        texture_sources: &mut HashMap<RenderResourceHandle, Weak<PackageTexturePixels>>,
    ) {
        let mut instances = Vec::new();
        let mut meshes = Vec::new();
        let mut textures = Vec::new();
        let mut handles = HashSet::new();
        let mut active_requests = BTreeSet::new();
        let mut active_prepared = BTreeSet::new();
        let mut error = None;
        let mut sources = 0usize;
        let Some(loader) = loader else {
            let required=scene.proxies().any(|proxy|matches!(&proxy.payload,RenderProxyPayload::Mesh3D(p) if p.presentation.as_ref().is_some_and(|m|m.vegetation.is_some())));
            self.changed = !self.instances.is_empty() || required || self.error.is_some();
            self.instances.clear();
            self.meshes.clear();
            self.textures.clear();
            self.handles.clear();
            self.prepared.clear();
            self.error = required.then(|| {
                invalid(
                    ContractErrorCode::MissingCookedPayload,
                    "vegetation.runtimePackage",
                )
            });
            self.reset_pending = true;
            return;
        };
        let identity = format!("{package_identity}:{}", loader.asset_revision_epoch());
        if self.reset_pending || identity != self.identity {
            for (_, request) in std::mem::take(&mut self.requests) {
                let _ = loader.release(&request.handle);
            }
            self.prepared.clear();
            self.reset_pending = false;
            self.identity = identity;
        }
        // A replacement loader cannot inherit another loader's request generation.
        self.requests.retain(|_, request| {
            loader
                .current_handle(&request.handle)
                .is_some_and(|h| h.state != RuntimeAssetLoadState::Released)
        });
        for proxy in scene.proxies() {
            let RenderProxyPayload::Mesh3D(owner) = &proxy.payload else {
                continue;
            };
            let owner = Arc::new(owner.as_ref().clone());
            let Some(presentation) = &owner.presentation else {
                continue;
            };
            let Some(binding) = &presentation.vegetation else {
                continue;
            };
            let result = (|| -> Result<(), ContractError> {
                let key = serde_json::to_string(&(&self.identity, &binding.cluster_asset))
                    .expect("typed vegetation reference");
                active_requests.insert(key.clone());
                if !self.requests.contains_key(&key) {
                    let reference = RuntimeAssetRef {
                        id: binding.cluster_asset.asset_id.clone(),
                        asset_type: "vegetation3d".into(),
                        guid: Some(binding.cluster_asset.asset_guid.clone()),
                        sub_asset: None,
                    };
                    let handle = loader.request_async(&reference).map_err(|_| {
                        invalid(
                            ContractErrorCode::MissingCookedPayload,
                            "vegetation.request",
                        )
                    })?;
                    self.requests.insert(
                        key.clone(),
                        Request {
                            handle,
                            asset: None,
                        },
                    );
                }
                let request = self.requests.get_mut(&key).unwrap();
                let state = loader.get_handle_state(&request.handle);
                match state {
                    Some(RuntimeAssetLoadState::Loading) => {
                        return Err(invalid(
                            ContractErrorCode::ResourceNotReady,
                            "vegetation.pending",
                        ))
                    }
                    Some(RuntimeAssetLoadState::Ready) => {}
                    _ => {
                        return Err(invalid(
                            ContractErrorCode::MissingCookedPayload,
                            "vegetation.payload",
                        ))
                    }
                }
                if request.asset.is_none() {
                    let bytes = loader.get_asset_bytes(&request.handle).ok_or_else(|| {
                        invalid(
                            ContractErrorCode::MissingCookedPayload,
                            "vegetation.payload",
                        )
                    })?;
                    let value = crate::spatial_contract::strict_json(&bytes).map_err(|_| {
                        invalid(ContractErrorCode::InvalidSchema, "vegetation.payload")
                    })?;
                    let asset: VegetationAsset3D = serde_json::from_value(value).map_err(|_| {
                        invalid(ContractErrorCode::InvalidSchema, "vegetation.payload")
                    })?;
                    asset.validate_binding(binding).map_err(|_| {
                        invalid(ContractErrorCode::InvalidBinding, "vegetation.binding")
                    })?;
                    request.asset = Some(Arc::new(asset));
                }
                let asset = request.asset.as_ref().unwrap().clone();
                let draws = asset
                    .project_instances(binding, presentation.time_seconds)
                    .map_err(|_| {
                        invalid(ContractErrorCode::InvalidBinding, "vegetation.instances")
                    })?;
                sources = sources.checked_add(draws.len()).ok_or_else(|| {
                    invalid(ContractErrorCode::InvalidBinding, "vegetation.frameBudget")
                })?;
                if sources > 65_536 {
                    return Err(invalid(
                        ContractErrorCode::InvalidBinding,
                        "vegetation.frameBudget",
                    ));
                }
                let mut clusters = BTreeMap::new();
                let used_clusters: BTreeSet<_> =
                    draws.iter().map(|draw| draw.cluster_key.as_str()).collect();
                for cluster in &asset.clusters {
                    if !used_clusters.contains(cluster.key.as_str()) {
                        continue;
                    }
                    let mut representations = Vec::new();
                    for (level, (distance, mesh)) in cluster.mesh_levels().enumerate() {
                        let prepare_key =
                            serde_json::to_string(&(&self.identity, mesh, &cluster.material))
                                .expect("typed vegetation mesh/material");
                        active_prepared.insert(prepare_key.clone());
                        if !self.prepared.contains_key(&prepare_key) {
                            let mut prepared = prepare_cluster(
                                frame,
                                cluster,
                                mesh,
                                loader,
                                manager,
                                &mut handles,
                                &mut textures,
                            )?;
                            ProductionGeometryCache::share_mesh_backings(
                                backings,
                                &mut prepared,
                                Some(loader),
                            );
                            for texture in &mut prepared.textures {
                                texture.make_package_streamable(loader, texture_sources);
                            }
                            self.prepared
                                .insert(prepare_key.clone(), Arc::new(prepared));
                        }
                        let prepared = self.prepared[&prepare_key].clone();
                        // The shared declared bound must cover every actual LOD mesh.
                        for geometry in &prepared.geometry {
                            if (0..3).any(|i| {
                                geometry.bounds.min[i] < cluster.mesh_bounds[0][i] - 0.0001
                                    || geometry.bounds.max[i] > cluster.mesh_bounds[1][i] + 0.0001
                            }) {
                                return Err(invalid(
                                    ContractErrorCode::InvalidBinding,
                                    "vegetation.meshBounds",
                                ));
                            }
                        }
                        handles.extend(prepared.geometry.iter().flat_map(|g| g.resource_handles()));
                        textures.extend(prepared.textures.iter().cloned());
                        let representation = if cluster.lods.is_empty() {
                            owner.clone()
                        } else {
                            let mut projected = owner.as_ref().clone();
                            let metadata = Arc::make_mut(projected.presentation.as_mut().unwrap());
                            metadata.range =
                                Some((distance, cluster.lods.get(level).map(|l| l.distance)));
                            metadata.source_instance = Some(format!("vegetation-lod:{level}"));
                            Arc::new(projected)
                        };
                        meshes.push((representation.clone(), prepared.clone()));
                        representations.push((representation, prepared));
                    }
                    clusters.insert(cluster.key.as_str(), representations);
                }
                drop(used_clusters);
                for draw in draws {
                    let matrix = multiply(owner.world.world_matrix, draw.matrix)?;
                    let source: Arc<str> = draw.source_id.into();
                    for (level, (representation, prepared)) in
                        clusters[draw.cluster_key.as_str()].iter().enumerate()
                    {
                        for (submesh, geometry) in prepared.geometry.iter().enumerate() {
                            let bytes = serde_json::to_vec(&(
                                "vegetation",
                                owner.runtime_entity_id.index,
                                owner.runtime_entity_id.generation,
                                &binding.cluster_asset,
                                draw.cluster_key.as_str(),
                                source.as_ref(),
                                geometry.material_slot(),
                            ))
                            .expect("stable vegetation source identity");
                            // Keep the original base identity; lower-detail representations
                            // have distinct draw identities but the same authored source id.
                            let bytes = if level == 0 {
                                bytes
                            } else {
                                serde_json::to_vec(&(bytes, level))
                                    .expect("vegetation representation identity")
                            };
                            let digest = crate::canonical_digest::sha256_hex(&bytes);
                            let instance_id =
                                u64::from_str_radix(&digest[..16], 16).expect("sha256 hex");
                            instances.push(Arc::new(RetainedInstance {
                                projection: InstanceProjection::Vegetation {
                                    owner: representation.clone(),
                                    matrix,
                                    source: source.clone(),
                                },
                                // Ordinary bounds use the real cooked mesh and the
                                // actual sheared matrix; no second instance transform.
                                dynamic_bounds: None,
                                prepared: prepared.clone(),
                                submesh,
                                instance_id,
                                visibility: owner.visibility(),
                            }));
                        }
                    }
                }
                Ok(())
            })();
            if let Err(failure) = result {
                error.get_or_insert(failure);
            }
        }
        for key in self
            .requests
            .keys()
            .filter(|k| !active_requests.contains(*k))
            .cloned()
            .collect::<Vec<_>>()
        {
            let request = self.requests.remove(&key).unwrap();
            let _ = loader.release(&request.handle);
        }
        self.prepared.retain(|key, _| active_prepared.contains(key));
        let mut texture_handles = HashSet::new();
        textures.retain(|texture| texture_handles.insert(texture.handle));
        for texture in &textures {
            texture.release_loader_pixels(loader);
        }
        let changed = error != self.error
            || !same_instances(&self.instances, &instances)
            || self.handles != handles;
        if changed {
            self.instances = instances;
            self.meshes = meshes;
            self.textures = textures;
            self.handles = handles;
        }
        self.error = error;
        self.changed = changed;
    }
}

impl ProductionGeometryCache {
    pub(crate) fn project_vegetation(
        &mut self,
        frame: u64,
        scene: &RenderSceneState,
        package_identity: &str,
        loader: Option<&mut RuntimeAssetLoader>,
        manager: &mut RenderResourceManager,
    ) {
        self.vegetation.update(
            frame,
            scene,
            package_identity,
            loader,
            manager,
            &mut self.mesh_backings,
            &mut self.texture_sources,
        );
    }
}

fn prepare_cluster(
    frame: u64,
    cluster: &VegetationCluster3D,
    mesh: &crate::render_contract::RenderAssetReference,
    loader: &mut RuntimeAssetLoader,
    manager: &mut RenderResourceManager,
    handles: &mut HashSet<RenderResourceHandle>,
    textures: &mut Vec<PreparedTextureAsset>,
) -> Result<PreparedMeshRenderer, ContractError> {
    let mesh =
        RuntimeRenderAssetProducer.prepare_cooked_from_package(frame, mesh, loader, manager)?;
    handles.extend(mesh.binding.resources());
    let RenderAssetData::Mesh(data) = &mesh.asset.asset else {
        return Err(invalid(
            ContractErrorCode::AssetTypeMismatch,
            "vegetation.mesh",
        ));
    };
    if (0..3).any(|i| {
        data.bounds.min[i] < cluster.mesh_bounds[0][i] - 0.0001
            || data.bounds.max[i] > cluster.mesh_bounds[1][i] + 0.0001
    }) {
        return Err(invalid(
            ContractErrorCode::InvalidBinding,
            "vegetation.meshBounds",
        ));
    }
    // All actual mesh slots deliberately share this cluster's material. This
    // is the ordinary prepare_mesh_renderer composition, with pending handles
    // returned even if native residency is not ready yet.
    let material = RuntimeRenderAssetProducer.prepare_cooked_from_package(
        frame,
        &cluster.material,
        loader,
        manager,
    )?;
    handles.extend(material.binding.resources());
    textures.extend(material.textures.iter().cloned());
    let geometry = data
        .submeshes
        .iter()
        .enumerate()
        .map(|(index, _)| {
            super::super::PreparedGeometry::from_assets(&mesh, &material, index, manager)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PreparedMeshRenderer {
        geometry,
        textures: material.textures,
        retained_resources: mesh
            .binding
            .resources()
            .chain(material.binding.resources())
            .collect(),
    })
}
fn same_instances(a: &[Arc<RetainedInstance>], b: &[Arc<RetainedInstance>]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| {
            a.instance_id == b.instance_id
                && a.submesh == b.submesh
                && Arc::ptr_eq(&a.prepared, &b.prepared)
                && match (&a.projection, &b.projection) {
                    (
                        InstanceProjection::Vegetation {
                            owner: a,
                            matrix: am,
                            source: asid,
                        },
                        InstanceProjection::Vegetation {
                            owner: b,
                            matrix: bm,
                            source: bsid,
                        },
                    ) => a == b && am == bm && asid == bsid,
                    _ => false,
                }
        })
}
fn multiply(a: [f32; 16], b: [f32; 16]) -> Result<[f32; 16], ContractError> {
    let result = std::array::from_fn(|i| {
        let row = i % 4;
        let column = i / 4;
        (0..4)
            .map(|k| a[k * 4 + row] * b[column * 4 + k])
            .sum::<f32>()
    });
    if result.iter().any(|v| !v.is_finite()) {
        return Err(invalid(ContractErrorCode::NonFinite, "vegetation.matrix"));
    }
    Ok(result)
}
fn invalid(code: ContractErrorCode, field: &'static str) -> ContractError {
    ContractError::new(code, field).at_stage(ProducerStage::RenderProjection)
}
