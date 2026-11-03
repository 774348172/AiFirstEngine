use super::RuntimeRenderAssetProducer;
use crate::canonical_digest::sha256_prefixed;
use crate::render_contract::*;
use crate::render_resource::*;
use crate::runtime_asset::{RuntimeAssetIndex, RuntimeAssetRecord};
use crate::runtime_asset_loader::RuntimeAssetLoader;
use crate::runtime_texture::{RuntimeTexturePayload, VerifiedTexturePayload};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderAssetPreparationState {
    /// CPU payload verified and the logical resource manager accepted the request.
    /// GPU upload/DrawGeometry execution is a separate backend operation.
    Prepared,
    Deferred,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedTextureAsset {
    pub payload: std::sync::Arc<RuntimeTexturePayload>,
    verified: VerifiedTexturePayload,
    upload_source: Option<std::sync::Arc<super::PackageTexturePixels>>,
    pub handle: TextureHandle,
    pub binding: RenderBindingSet,
    pub state: RenderAssetPreparationState,
}

impl PreparedTextureAsset {
    /// Acquire CPU pixels for an upload or an explicit CPU reader. Package-backed
    /// production descriptors may reload their original verified source here;
    /// their public `payload` retains metadata only. Keep the returned Arc alive
    /// until the consumer has finished reading its pixels.
    #[doc(hidden)]
    pub fn acquire_upload_payload(&self) -> Result<std::sync::Arc<RuntimeTexturePayload>, String> {
        self.validate_upload_descriptor()?;
        match &self.upload_source {
            Some(source) => source.acquire(),
            None => Ok(self.payload.clone()),
        }
    }

    #[doc(hidden)]
    pub fn validate_upload_descriptor(&self) -> Result<(), String> {
        let byte_len = match &self.upload_source {
            Some(source) => {
                if self.content_identity_internal() != source.identity {
                    return Err("texture.source_identity_mismatch".into());
                }
                source.byte_len
            }
            None => self.payload.rgba8.len(),
        };
        validate_texture_layout(&self.payload, byte_len).map_err(|e| e.to_string())
    }

    #[doc(hidden)]
    pub fn upload_byte_len(&self) -> usize {
        self.upload_source
            .as_ref()
            .map_or(self.payload.rgba8.len(), |s| s.byte_len)
    }

    /// Drop the shared pending CPU backing after actual GPU completion. Existing
    /// readers and other uploads retain their own Arc leases independently.
    #[doc(hidden)]
    pub fn release_upload_payload(&self) {
        if let Some(source) = &self.upload_source {
            source.release();
        }
    }

    fn content_identity_internal(&self) -> String {
        self.verified.identity_for(&self.payload)
    }

    pub(crate) fn make_package_streamable(
        &mut self,
        loader: &RuntimeAssetLoader,
        sources: &mut std::collections::HashMap<
            RenderResourceHandle,
            std::sync::Weak<super::PackageTexturePixels>,
        >,
    ) {
        let identity = self.content_identity_internal();
        let source = sources
            .get(&self.handle.raw())
            .and_then(std::sync::Weak::upgrade)
            .filter(|s| s.identity == identity)
            .or_else(|| {
                super::PackageTexturePixels::new(loader, &self.verified).map(std::sync::Arc::new)
            });
        if let Some(source) = source {
            sources.insert(self.handle.raw(), std::sync::Arc::downgrade(&source));
            self.verified = source.descriptor.clone();
            self.payload = self.verified.payload().clone();
            self.upload_source = Some(source);
        }
    }

    pub(crate) fn release_loader_pixels(&self, loader: &mut RuntimeAssetLoader) {
        if let Some(source) = &self.upload_source {
            source.release_loader_pixels(loader);
        }
    }

    pub(crate) fn observe_memory(&self, sample: &mut crate::resource_memory::ResourceMemorySample) {
        sample.cpu_arc(&self.payload, "assets", &["snapshot"]);
        self.payload.observe_memory(sample, "snapshot");
        if let Some(source) = &self.upload_source {
            source.observe_memory(sample);
        }
    }

    #[cfg(feature = "real-wgpu")]
    #[doc(hidden)]
    pub fn content_identity(&self) -> String {
        self.verified.identity_for(&self.payload)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedCookedRenderAsset {
    pub asset: CookedRenderAsset,
    pub binding: RenderBindingSet,
    pub state: RenderAssetPreparationState,
    pub textures: Vec<PreparedTextureAsset>,
    pub resource_request: RenderResourceRequest,
    pub asset_guid: String,
    pub cooked_digest: String,
    pub source_path: Option<String>,
    pub pipeline_identity: Option<String>,
}

/// One validated MeshRenderer binding snapshot, consumed by the existing geometry backend.
#[derive(Debug, Clone)]
pub struct PreparedMeshRenderer {
    pub geometry: Vec<crate::render_geometry::PreparedGeometry>,
    pub textures: Vec<PreparedTextureAsset>,
    /// All validated bindings prepared for this renderer, including declared
    /// material slots that no current submesh draws. The existing cache owns
    /// their retirement along with resources referenced by geometry.
    pub retained_resources: Vec<RenderResourceHandle>,
}

impl PreparedMeshRenderer {
    pub(crate) fn resource_handles(&self) -> impl Iterator<Item = RenderResourceHandle> + '_ {
        self.geometry
            .iter()
            .flat_map(|g| g.resource_handles())
            .chain(self.retained_resources.iter().copied())
    }
}

impl RuntimeRenderAssetProducer {
    /// Assemble a generated candidate through the same package-material producer.
    /// The original geometry cache must adopt all returned handles before publishing;
    /// this private seam does not claim native readiness or release shared resources.
    pub(crate) fn prepare_generated_geometry(
        &self,
        frame_index: u64,
        generated: &crate::render_geometry::generated::GeneratedGeometry,
        ticket: project_runtime_sdk::generated_content::ContentTicket,
        revision: u64,
        loader: &mut RuntimeAssetLoader,
        manager: &mut RenderResourceManager,
    ) -> Result<PreparedMeshRenderer, ContractError> {
        // As in cell preparation, malformed candidates never mutate the live manager.
        let mut staged = manager.clone();
        let mut materials = std::collections::BTreeMap::new();
        let mut textures = Vec::new();
        let mut handles = BTreeSet::new();
        for (slot, asset) in generated.materials() {
            let reference = RenderAssetReference {
                expected_kind: RenderAssetKind::Material,
                reference: crate::runtime_package::RuntimeAssetRef {
                    id: asset.id.clone(),
                    asset_type: asset.asset_type.clone(),
                    guid: asset.guid.clone(),
                    sub_asset: asset.sub_asset.clone(),
                },
            };
            let record = resolve(&reference, loader.asset_index())?;
            if record.asset_id != asset.id || record.sub_asset_id != asset.sub_asset {
                return Err(error(
                    ContractErrorCode::AssetIdentityConflict,
                    "material.assetRef",
                ));
            }
            let prepared =
                self.prepare_cooked_from_package(frame_index, &reference, loader, &mut staged)?;
            textures.extend(prepared.textures.iter().cloned());
            handles.extend(prepared.binding.resources());
            materials.insert(slot, prepared);
        }
        let request = generated.binding_request(ticket, revision);
        let identity = request.key.asset_id.clone();
        let mesh = staged.request_resource(frame_index, request);
        handles.insert(mesh);
        let binding = std::sync::Arc::new(RenderBindingSet::single(
            identity.clone(),
            RenderBindingKind::Mesh,
            mesh,
            "linearClamp".into(),
            false,
            identity.clone(),
        )?);
        let provenance = std::sync::Arc::new(Provenance {
            asset_id: Some(identity),
            availability: SourceAvailability::SuppliedContext,
            ..Provenance::unspecified(ProducerStage::AssetBinding)
        });
        let geometry = generated
            .sections()
            .iter()
            .enumerate()
            .map(|(index, section)| {
                generated.bind(
                    index,
                    binding.clone(),
                    provenance.clone(),
                    &materials[section.material_slot.as_str()],
                    &staged,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let prepared = PreparedMeshRenderer {
            geometry,
            textures,
            retained_resources: handles.into_iter().collect(),
        };
        *manager = staged;
        Ok(prepared)
    }

    pub fn prepare_mesh_renderer(
        &self,
        frame_index: u64,
        renderer: &crate::spatial_contract::MeshRenderer3D,
        loader: &mut RuntimeAssetLoader,
        manager: &mut RenderResourceManager,
    ) -> Result<PreparedMeshRenderer, ContractError> {
        let reference =
            |kind, asset: &crate::spatial_contract::SpatialAssetRef| RenderAssetReference {
                expected_kind: kind,
                reference: asset.0.clone(),
            };
        let mesh = self.prepare_cooked_from_package(
            frame_index,
            &reference(RenderAssetKind::Mesh, &renderer.mesh),
            loader,
            manager,
        )?;
        let RenderAssetData::Mesh(data) = &mesh.asset.asset else {
            unreachable!()
        };
        renderer
            .validate_slots(&data.material_slots)
            .map_err(|_| error(ContractErrorCode::InvalidBinding, "meshRenderer.materials"))?;
        let mut materials = std::collections::BTreeMap::new();
        let mut textures = Vec::new();
        let mut retained_resources: BTreeSet<_> = mesh.binding.resources().collect();
        for (slot, asset) in &renderer.materials {
            let prepared = self.prepare_cooked_from_package(
                frame_index,
                &reference(RenderAssetKind::Material, asset),
                loader,
                manager,
            )?;
            textures.extend(prepared.textures.iter().cloned());
            retained_resources.extend(prepared.binding.resources());
            materials.insert(slot, prepared);
        }
        let geometry = data
            .submeshes
            .iter()
            .enumerate()
            .map(|(index, submesh)| {
                crate::render_geometry::PreparedGeometry::from_assets(
                    &mesh,
                    &materials[&submesh.material_slot],
                    index,
                    manager,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(PreparedMeshRenderer {
            geometry,
            textures,
            retained_resources: retained_resources.into_iter().collect(),
        })
    }

    pub fn prepare_cooked_from_package(
        &self,
        frame_index: u64,
        reference: &RenderAssetReference,
        loader: &mut RuntimeAssetLoader,
        manager: &mut RenderResourceManager,
    ) -> Result<PreparedCookedRenderAsset, ContractError> {
        let record = resolve(reference, loader.asset_index())?.clone();
        if !matches!(
            reference.expected_kind,
            RenderAssetKind::Mesh | RenderAssetKind::Material
        ) {
            return Err(error(
                ContractErrorCode::AssetTypeMismatch,
                "reference.expectedKind",
            ));
        }
        let cooked_record = loader
            .asset_index()
            .cooked_asset(&record.cooked_asset_id)
            .ok_or_else(|| error(ContractErrorCode::MissingCookedPayload, "cookedAssetId"))?
            .clone();
        let loaded = loader
            .load(&reference.reference)
            .map_err(|_| error(ContractErrorCode::MissingCookedPayload, "cookedAssetId"))?;
        let bytes = loader.acquire_render_asset_bytes(&loaded);
        let decoded = loader.decoded_payload(&loaded);
        // Owned Arc keeps bytes alive; balance the loader/dependency references on
        // success and every decode/validation failure, without a second cache.
        loader
            .release(&loaded)
            .map_err(|_| error(ContractErrorCode::StaleResourceHandle, "runtimeAssetHandle"))?;
        let bytes =
            bytes.ok_or_else(|| error(ContractErrorCode::MissingCookedPayload, "cookedAssetId"))?;
        let digest = sha256_prefixed(&bytes);
        if record.hash.as_deref() != Some(&digest)
            || cooked_record.hash.as_deref() != Some(&digest)
            || record.size != Some(bytes.len() as u64)
            || cooked_record.size != Some(bytes.len() as u64)
            || record.bundle_id != cooked_record.bundle_id
        {
            return Err(error(
                ContractErrorCode::AssetDigestMismatch,
                "runtimeManifest",
            ));
        }
        let asset = match decoded.as_deref() {
            Some(crate::runtime_asset::RuntimeDecodedPayload::Render(asset)) => asset.clone(),
            _ => {
                let text = std::str::from_utf8(&bytes)
                    .map_err(|_| error(ContractErrorCode::InvalidSchema, "cookedPayload"))?;
                RenderContract::<CookedRenderAsset>::decode(text)
                    .map_err(|e| e.compact().at_stage(ProducerStage::AssetDecode))?
                    .data
            }
        };
        if asset.asset.asset_kind() != reference.expected_kind {
            return Err(error(
                ContractErrorCode::AssetTypeMismatch,
                "cookedPayload.kind",
            ));
        }

        // Validate all dependencies before requesting any resource. Texture bytes
        // use the existing package texture decoder and the same preparation seam.
        let mut pending_textures = Vec::new();
        let mut dependency_guids = BTreeSet::new();
        if let RenderAssetData::Material(material) = &asset.asset {
            for (slot, texture) in &material.textures {
                let dependency = resolve(texture, loader.asset_index())?;
                dependency_guids.insert(dependency.asset_guid.clone());
                let mut payload = loader
                    .load_verified_texture_payload(&texture.reference)
                    .map_err(|_| {
                        error(ContractErrorCode::MissingCookedPayload, "material.textures")
                    })?;
                if let Some(sampling) = material.texture_sampling.get(slot) {
                    payload = payload.with_material_sampling(
                        sampling.key(),
                        matches!(slot.as_str(), "baseColorTexture" | "emissiveTexture"),
                    )?;
                } else if let Some(sampling) = material.base_color_texture_sampling {
                    payload = payload.with_sampling(sampling.key())?;
                }
                validate_texture_payload(payload.payload())?;
                pending_textures.push((slot.clone(), payload));
            }
        }
        if dependency_guids != record.dependencies.iter().cloned().collect() {
            return Err(error(
                ContractErrorCode::AssetIdentityConflict,
                "runtimeManifest.dependencies",
            ));
        }
        let (kind, source, byte_len, binding_kind, usage) = match &asset.asset {
            RenderAssetData::Mesh(mesh) => (
                RenderResourceKind::MeshBuffer,
                RenderResourceSource::MeshBufferDescriptor {
                    vertex_count: mesh.vertex_count,
                    index_count: mesh.index_count,
                },
                mesh.vertex_bytes.len() + mesh.index_bytes.len(),
                RenderBindingKind::Mesh,
                "mesh-geometry",
            ),
            RenderAssetData::Material(material) => (
                RenderResourceKind::MaterialParams,
                RenderResourceSource::MaterialParamsDescriptor {
                    param_count: material.parameters.len() as u32,
                },
                serde_json::to_vec(&material.parameters).unwrap().len(),
                RenderBindingKind::Material,
                "material-binding",
            ),
        };
        let resource_request = RenderResourceRequest {
            key: RenderAssetKey {
                asset_id: record.asset_guid.clone(),
                asset_version: format!("{}:{digest}", record.version),
                resource_kind: kind,
                platform_profile: "dev-desktop".into(),
                quality_profile: "default".into(),
                usage: usage.into(),
            },
            source,
            byte_len,
            reason: "validated-native-render-payload".into(),
        };
        let handle = manager.request_resource(frame_index, resource_request.clone());
        let mut binding = RenderBindingSet::single(
            format!("binding:{}", record.asset_guid),
            binding_kind,
            handle,
            "linearClamp".into(),
            false,
            record.asset_id.clone(),
        )?;
        let mut textures = Vec::new();
        for (slot, payload) in pending_textures {
            let prepared = self.prepare_verified_texture_payload(
                frame_index,
                payload,
                RenderBindingKind::Texture,
                manager,
            )?;
            // Unlit v1 has one sampled slot; preserve its actual sampler.
            binding.sampler = prepared.payload.sampler.clone();
            binding.slots.push(NamedRenderBinding {
                name: slot,
                resource: BindingResource::Texture(prepared.handle),
            });
            textures.push(prepared);
        }
        let state = preparation_state(&binding, manager)?;
        let pipeline_identity = match &asset.asset {
            RenderAssetData::Material(m) => Some(m.pipeline_identity()?),
            _ => None,
        };
        Ok(PreparedCookedRenderAsset {
            asset,
            binding,
            state,
            textures,
            resource_request,
            asset_guid: record.asset_guid,
            cooked_digest: digest,
            source_path: record.source_map_debug,
            pipeline_identity,
        })
    }

    /// Sprite, AUI image, particle texture and font atlas pixels share this CPU
    /// preparation path. Existing backend upload ownership remains unchanged.
    pub fn prepare_texture_payload(
        &self,
        frame_index: u64,
        payload: RuntimeTexturePayload,
        binding_kind: RenderBindingKind,
        manager: &mut RenderResourceManager,
    ) -> Result<PreparedTextureAsset, ContractError> {
        validate_texture_payload(&payload)?;
        if !matches!(
            binding_kind,
            RenderBindingKind::Texture | RenderBindingKind::FontAtlas
        ) {
            return Err(error(
                ContractErrorCode::ResourceTypeMismatch,
                "texture.bindingKind",
            ));
        }
        self.prepare_verified_texture_payload(
            frame_index,
            VerifiedTexturePayload::new(payload),
            binding_kind,
            manager,
        )
    }

    fn prepare_verified_texture_payload(
        &self,
        frame_index: u64,
        verified: VerifiedTexturePayload,
        binding_kind: RenderBindingKind,
        manager: &mut RenderResourceManager,
    ) -> Result<PreparedTextureAsset, ContractError> {
        let payload = verified.payload().clone();
        validate_texture_payload(&payload)?;
        // The worker receipt binds exact immutable pixels; metadata stays part of
        // the identity so sampling variants and new pixels cannot alias.
        let digest = verified.identity_for(&payload);
        let handle = manager.request_resource(
            frame_index,
            RenderResourceRequest {
                key: RenderAssetKey {
                    // Sampling/format variants coexist; pixel revisions still stale
                    // previous versions within the same sampling shape.
                    usage: format!(
                        "sampled-texture:{}:{}:{}",
                        payload.format, payload.color_space, payload.sampler
                    ),
                    ..RenderAssetKey::new(&payload.asset_id, digest, RenderResourceKind::Texture)
                },
                source: RenderResourceSource::TextureDescriptor {
                    width: payload.width,
                    height: payload.height,
                    format: payload.format.clone(),
                },
                byte_len: payload.rgba8.len(),
                reason: "validated-texture-payload".into(),
            },
        );
        let binding = RenderBindingSet::single(
            format!("binding:{}", payload.asset_id),
            binding_kind,
            handle,
            payload.sampler.clone(),
            false,
            payload.asset_id.clone(),
        )?;
        let state = preparation_state(&binding, manager)?;
        Ok(PreparedTextureAsset {
            payload,
            verified,
            upload_source: None,
            handle: TextureHandle::try_from(handle)?,
            binding,
            state,
        })
    }
}

#[doc(hidden)]
pub fn validate_texture_payload(payload: &RuntimeTexturePayload) -> Result<(), ContractError> {
    validate_texture_layout(payload, payload.rgba8.len())
}

fn validate_texture_layout(
    payload: &RuntimeTexturePayload,
    actual_byte_len: usize,
) -> Result<(), ContractError> {
    let byte_len = crate::runtime_texture::texture_mip_byte_len(
        &payload.format,
        payload.width,
        payload.height,
        payload.mip_count,
    );
    let sampler = crate::runtime_texture::TextureSampler::parse(&payload.sampler);
    if payload.asset_id.trim().is_empty()
        || payload.width == 0
        || payload.height == 0
        || byte_len != Some(actual_byte_len)
        || sampler.is_some_and(|s| {
            s.min_filter.uses_mips()
                && payload.mip_count != 32 - payload.width.max(payload.height).leading_zeros()
        })
        || crate::runtime_texture::texture_block_layout(&payload.format).is_none()
        || (payload.format.starts_with("bc3") && (payload.width % 4 != 0 || payload.height % 4 != 0
            || payload.mip_count != 32 - payload.width.max(payload.height).leading_zeros()
            || payload.color_space != if payload.format.ends_with("Srgb") { "srgb" } else { "linear" }))
        || !matches!(payload.color_space.as_str(), "srgb" | "linear")
        // Native v1 accepted rgba8Unorm with srgb metadata but sampled raw UNORM.
        // Preserve that input/byte meaning. StandardLit normalizes both fields per slot.
        || (payload.format == "rgba8UnormSrgb" && payload.color_space != "srgb")
        || crate::runtime_texture::TextureSampler::parse(&payload.sampler).is_none()
    {
        return Err(error(
            ContractErrorCode::InvalidAssetPayload,
            "texture.payload",
        ));
    }
    Ok(())
}

fn preparation_state(
    binding: &RenderBindingSet,
    manager: &RenderResourceManager,
) -> Result<RenderAssetPreparationState, ContractError> {
    match binding.validate_residency(manager) {
        Ok(()) => Ok(RenderAssetPreparationState::Prepared),
        Err(e) if e.code == ContractErrorCode::ResourceNotReady => {
            Ok(RenderAssetPreparationState::Deferred)
        }
        Err(e) => Err(e),
    }
}

fn resolve<'a>(
    reference: &RenderAssetReference,
    index: &'a RuntimeAssetIndex,
) -> Result<&'a RuntimeAssetRecord, ContractError> {
    match reference.resolve(Some(index))? {
        RenderAssetResolution::Resolved(record) => Ok(record),
        RenderAssetResolution::Unresolved => unreachable!("index supplied"),
    }
}
fn error(code: ContractErrorCode, field: &'static str) -> ContractError {
    ContractError::new(code, field).at_stage(ProducerStage::AssetDecode)
}
