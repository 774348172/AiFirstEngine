//! Generated candidates use the original cache, RHI upload receipts and World safe point.
use super::*;
use crate::engine_rhi::EngineRhiBackend;
use crate::geometry_upload::{GeometryUploadState, GeometryUploadTicket};
use crate::render_geometry::ProductionGeometryCache;
use crate::render_resource::RenderResourceManager;
use crate::spatial_contract::{MeshRenderer3D, SpatialAssetRef};
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
#[path = "generated_content_collision.rs"]
mod collision;

#[derive(Debug, Clone)]
pub(super) struct Publication {
    renderer: MeshRenderer3D,
    key: Arc<str>,
    upload: Option<GeometryUploadTicket>,
}

pub(crate) struct GeneratedContentNativeFrame<'a> {
    pub frame: u64,
    pub world: &'a mut World,
    pub package: &'a str,
    pub cache: &'a mut ProductionGeometryCache,
    pub manager: &'a mut RenderResourceManager,
    pub backend: &'a mut (dyn EngineRhiBackend + 'static),
    pub physics: &'a mut crate::physics_3d::PhysicsModule,
}

pub(super) fn owns_renderer(
    publication: &Publication,
    world: &World,
    target: &(String, u64),
) -> bool {
    world.entity_generation(&EntityId::from(target.0.as_str())) == Some(target.1)
        && world
            .mesh_renderer3d(&EntityId::from(target.0.as_str()))
            .is_some_and(|r| {
                r.mesh == publication.renderer.mesh && r.materials == publication.renderer.materials
            })
}

fn retire_upload(
    upload: &mut Option<GeometryUploadTicket>,
    backend: &mut (dyn EngineRhiBackend + 'static),
) -> bool {
    let Some(ticket) = *upload else { return true };
    match backend.release_geometry_upload(ticket) {
        Ok(()) => {
            *upload = None;
            true
        }
        Err(e)
            if matches!(
                e.as_str(),
                "geometry.stale_upload_ticket" | "geometry.unknown_upload_ticket"
            ) =>
        {
            // A destroyed/replaced owner has no live pin for this old receipt.
            *upload = None;
            true
        }
        Err(_) => false, // Keep the actual pin and retry through its original owner.
    }
}

impl RuntimeInstanceLoader {
    /// Called before the original runtime frame advances simulation or extraction.
    /// Production Submit stays gated separately until the complete SDK surface is available.
    pub(crate) fn advance_generated_content_native(
        &mut self,
        context: GeneratedContentNativeFrame<'_>,
    ) {
        let GeneratedContentNativeFrame {
            frame,
            world,
            package,
            cache,
            manager,
            backend,
            physics,
        } = context;
        let same_world = self.generated_content.check_world(world, 0).is_ok();
        world.propagate_spatial();
        // Remove dead/foreign/closed ownership before CPU polling can clear canonical readers.
        for (key, binding) in &mut self.generated_content.bindings {
            let closing = !same_world || !self.generated_content.accepting;
            #[cfg(not(target_arch = "wasm32"))]
            if (closing
                || world.entity_generation(&key.0.as_str().into()) != Some(key.1)
                || binding
                    .collision
                    .as_ref()
                    .is_some_and(|owned| !physics.generated_static_native_live(owned)))
                && binding
                    .collision
                    .as_ref()
                    .is_some_and(|owned| physics.retire_generated_static(world, owned).is_ok())
            {
                binding.collision = None;
                self.asset_loader
                    .release_prepared_cpu(std::mem::take(&mut binding.collision_cpu_charge));
            }
            let Some(publication) = binding.published.as_mut() else {
                continue;
            };
            if closing || !owns_renderer(publication, world, key) {
                if closing && same_world && owns_renderer(publication, world, key) {
                    world.remove_component_value(
                        &key.0.as_str().into(),
                        &crate::components::ComponentTypeId::mesh_renderer3d(),
                    );
                }
                binding.canonical = None;
                binding.layout = None;
                if retire_upload(&mut publication.upload, backend) {
                    cache.retire_generated_binding(frame, &publication.key, manager);
                    binding.published = None;
                }
                continue;
            }
            // Explicit backend replacement retains the same canonical/prepared inputs.
            if let Some(ticket) = publication.upload {
                match backend.geometry_upload_state(ticket) {
                    Err(e)
                        if matches!(
                            e.as_str(),
                            "geometry.stale_upload_ticket" | "geometry.unknown_upload_ticket"
                        ) =>
                    {
                        publication.upload = None
                    }
                    Ok(GeometryUploadState::Abandoned) => {
                        retire_upload(&mut publication.upload, backend);
                    }
                    // A transient poll error is not evidence that the actual owner dropped its pin.
                    Err(_) => continue,
                    _ => {}
                }
            }
            if publication.upload.is_none() {
                if let Some(prepared) = cache.generated_binding(&publication.key) {
                    publication.upload = backend
                        .begin_geometry_upload(&prepared.geometry, &prepared.textures)
                        .ok();
                }
            }
        }
        self.advance_generated_content_cpu(world);
        let requests: Vec<_> = self.generated_content.records.keys().copied().collect();
        for id in requests {
            let ticket = self.generated_content.records[&id].receipt.ticket;
            let revision = self.generated_content.records[&id]
                .receipt
                .candidate_revision;
            if self.generated_content.records[&id]
                .receipt
                .status
                .terminal()
            {
                let record = self.generated_content.records.get_mut(&id).unwrap();
                if retire_upload(&mut record.upload, backend)
                    && cache.generated_candidate(ticket, revision).is_some()
                {
                    let _ = cache.release_generated_candidate(frame, ticket, revision, manager);
                }
                continue;
            }
            if self.generated_content.records[&id].candidate.is_none() {
                continue;
            }
            if let Err(failure) = self.validate_generated_content_commit(ticket, world) {
                let _ = self
                    .generated_content
                    .fail(ticket, failure, &mut self.asset_loader);
                continue;
            }
            #[cfg(not(target_arch = "wasm32"))]
            if self.generated_content.records[&id]
                .prepared_collision
                .is_none()
            {
                let change = self
                    .generated_content
                    .records
                    .get_mut(&id)
                    .unwrap()
                    .candidate
                    .as_mut()
                    .unwrap()
                    .collision
                    .take();
                let decode_bytes = change.as_ref().map_or(0, cpu::collision_bytes);
                let descriptor = match change {
                    Some(CollisionUpdate3D::Replace { body }) => {
                        Some(Some(collision::native_body(body)))
                    }
                    Some(CollisionUpdate3D::Clear) => Some(None),
                    _ => None,
                };
                if let Some(descriptor) = descriptor {
                    let target = self.generated_content.records[&id]
                        .receipt
                        .target
                        .entity_id
                        .as_str()
                        .into();
                    match physics.prepare_static_collision(
                        world,
                        &target,
                        descriptor,
                        &mut self.asset_loader,
                    ) {
                        Ok(prepared) => {
                            let record = self.generated_content.records.get_mut(&id).unwrap();
                            record.prepared_collision = Some(prepared);
                            // The original byte attachments are gone; decoded native descriptors
                            // keep their original payload charge until replacement/release.
                            record.cpu_charge -= decode_bytes;
                            self.asset_loader.release_prepared_cpu(decode_bytes);
                        }
                        Err(failure) => {
                            let _ = self.generated_content.fail(
                                ticket,
                                collision::failure(id, failure),
                                &mut self.asset_loader,
                            );
                            continue;
                        }
                    }
                }
            }
            let candidate = self.generated_content.records[&id]
                .candidate
                .as_ref()
                .unwrap();
            // wasm keeps an explicit gap; no native Prepared is inferred there.
            #[cfg(target_arch = "wasm32")]
            if candidate
                .collision
                .as_ref()
                .is_some_and(|c| !matches!(c, CollisionUpdate3D::Keep))
                || (candidate.geometry.is_none() && !candidate.release)
            {
                let failure = error(
                    ContentErrorCode::CapabilityGap,
                    id,
                    "capability.generatedContent.collisionPublication",
                );
                let _ = self
                    .generated_content
                    .fail(ticket, failure, &mut self.asset_loader);
                continue;
            }
            if let Some(geometry) = candidate.geometry.clone() {
                let prepared = match cache.generated_candidate(ticket, revision) {
                    Some(prepared) => Ok(prepared),
                    None => cache.prepare_generated_candidate(
                        frame,
                        &geometry,
                        ticket,
                        revision,
                        &mut self.asset_loader,
                        manager,
                    ),
                };
                let prepared = match prepared {
                    Ok(prepared) => prepared,
                    Err(failure) => {
                        let _ =
                            self.generated_content
                                .fail(ticket, failure, &mut self.asset_loader);
                        continue;
                    }
                };
                if self.generated_content.records[&id].upload.is_none() {
                    match backend.begin_geometry_upload(&prepared.geometry, &prepared.textures) {
                        Ok(upload) => {
                            self.generated_content.records.get_mut(&id).unwrap().upload =
                                Some(upload)
                        }
                        Err(_) => {
                            let _ =
                                cache.release_generated_candidate(frame, ticket, revision, manager);
                            let failure =
                                error(ContentErrorCode::CapabilityGap, id, "render.nativeUpload");
                            let _ = self.generated_content.fail(
                                ticket,
                                failure,
                                &mut self.asset_loader,
                            );
                            continue;
                        }
                    }
                }
                let upload = self.generated_content.records[&id].upload.unwrap();
                match backend.geometry_upload_state(upload) {
                    Ok(GeometryUploadState::Ready) => {}
                    Ok(GeometryUploadState::Uploading | GeometryUploadState::Submitted) => continue,
                    Err(e)
                        if matches!(
                            e.as_str(),
                            "geometry.stale_upload_ticket" | "geometry.unknown_upload_ticket"
                        ) =>
                    {
                        self.generated_content.records.get_mut(&id).unwrap().upload = None;
                        continue;
                    }
                    Ok(GeometryUploadState::Abandoned) => {
                        retire_upload(
                            &mut self.generated_content.records.get_mut(&id).unwrap().upload,
                            backend,
                        );
                        continue;
                    }
                    Err(_) => {
                        let failure = error(
                            ContentErrorCode::CapabilityGap,
                            id,
                            "render.nativeReadiness",
                        );
                        let _ =
                            self.generated_content
                                .fail(ticket, failure, &mut self.asset_loader);
                        continue;
                    }
                }
            }
            if let Err(failure) = self.publish_generated_geometry(
                ticket,
                GeneratedContentNativeFrame {
                    frame,
                    world,
                    package,
                    cache,
                    manager,
                    backend,
                    physics,
                },
            ) {
                let _ = self
                    .generated_content
                    .fail(ticket, failure, &mut self.asset_loader);
            }
        }
        self.generated_content
            .bindings
            .retain(|(id, generation), b| {
                b.published.is_some()
                    || {
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            b.collision.is_some()
                        }
                        #[cfg(target_arch = "wasm32")]
                        {
                            false
                        }
                    }
                    || b.pending.is_some()
                    || world.entity_generation(&id.as_str().into()) == Some(*generation)
            });
        self.generated_content
            .reap_cpu_backings(&mut self.asset_loader);
    }

    fn publish_generated_geometry(
        &mut self,
        ticket: ContentTicket,
        context: GeneratedContentNativeFrame<'_>,
    ) -> Result<(), ContentError> {
        let GeneratedContentNativeFrame {
            frame,
            world,
            package,
            cache,
            manager,
            backend,
            physics,
        } = context;
        self.validate_generated_content_commit(ticket, world)?;
        let record = &self.generated_content.records[&ticket.request_id];
        let target = record.receipt.target.clone();
        let revision = record.receipt.candidate_revision;
        let key = target_key(&target);
        let next_epoch = self
            .generated_content
            .commit_epoch
            .checked_add(1)
            .ok_or_else(|| {
                error(
                    ContentErrorCode::BudgetExceeded,
                    ticket.request_id,
                    "commitEpoch",
                )
            })?;
        if record
            .candidate
            .as_ref()
            .is_some_and(|c| c.geometry.is_some())
        {
            if cache.generated_candidate(ticket, revision).is_none() {
                return Err(error(
                    ContentErrorCode::NotFound,
                    ticket.request_id,
                    "render.candidateIdentity",
                ));
            }
            if record.upload.is_none_or(|upload| {
                backend.geometry_upload_state(upload) != Ok(GeometryUploadState::Ready)
            }) {
                return Err(error(
                    ContentErrorCode::CapabilityGap,
                    ticket.request_id,
                    "render.nativeReadiness",
                ));
            }
        }
        let changes_geometry = record
            .candidate
            .as_ref()
            .is_some_and(|c| c.geometry.is_some() || c.release);
        // Retirement pressure preserves both the old public binding and new Ready candidate.
        if let Some(old) = self
            .generated_content
            .bindings
            .get_mut(&key)
            .and_then(|b| b.published.as_mut())
            .filter(|_| changes_geometry)
        {
            if !retire_upload(&mut old.upload, backend) {
                return Ok(());
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let release = self.generated_content.records[&ticket.request_id]
                .candidate
                .as_ref()
                .unwrap()
                .release;
            let mut changed = false;
            let mut next = None;
            if let Some(prepared) = self
                .generated_content
                .records
                .get_mut(&ticket.request_id)
                .unwrap()
                .prepared_collision
                .take()
            {
                next = physics
                    .commit_static_collision(world, prepared)
                    .map_err(|failure| collision::failure(ticket.request_id, failure))?;
                changed = true;
            } else if release {
                if let Some(owned) = self.generated_content.bindings[&key].collision.as_ref() {
                    physics
                        .retire_generated_static(world, owned)
                        .map_err(|failure| collision::failure(ticket.request_id, failure))?;
                    changed = true;
                }
            }
            if changed {
                let binding = self.generated_content.bindings.get_mut(&key).unwrap();
                self.asset_loader
                    .release_prepared_cpu(std::mem::take(&mut binding.collision_cpu_charge));
                binding.collision = next;
                if binding.collision.is_some() {
                    binding.collision_cpu_charge = std::mem::take(
                        &mut self
                            .generated_content
                            .records
                            .get_mut(&ticket.request_id)
                            .unwrap()
                            .cpu_charge,
                    );
                }
                binding.collision_revision = revision;
            }
        }
        let record = self
            .generated_content
            .records
            .get_mut(&ticket.request_id)
            .unwrap();
        let candidate = record.candidate.take().expect("CPU prepared candidate");
        let mut publication = None;
        if let Some(geometry) = candidate.geometry.as_ref() {
            let identity = geometry.binding_request(ticket, revision).key.asset_id;
            let mut renderer = world
                .mesh_renderer3d(&target.entity_id.as_str().into())
                .cloned()
                .unwrap_or(MeshRenderer3D {
                    schema_version: "mesh-renderer3d.v1".into(),
                    mesh: SpatialAssetRef(RuntimeAssetRef {
                        id: identity.clone(),
                        asset_type: "mesh".into(),
                        guid: Some(identity.clone()),
                        sub_asset: None,
                    }),
                    materials: BTreeMap::new(),
                    visible: true,
                    layers: Vec::new(),
                });
            renderer.mesh = SpatialAssetRef(RuntimeAssetRef {
                id: identity.clone(),
                asset_type: "mesh".into(),
                guid: Some(identity),
                sub_asset: None,
            });
            renderer.materials = geometry
                .materials()
                .map(|(slot, reference)| {
                    (
                        slot.into(),
                        SpatialAssetRef(RuntimeAssetRef {
                            id: reference.id.clone(),
                            asset_type: reference.asset_type.clone(),
                            guid: reference.guid.clone(),
                            sub_asset: reference.sub_asset.clone(),
                        }),
                    )
                })
                .collect();
            let published_key =
                cache.publish_generated_candidate(ticket, revision, package, &renderer)
                    .expect("candidate identity checked before fallible native activation; no concurrent cache mutation");
            publication = Some(Publication {
                renderer: renderer.clone(),
                key: published_key,
                upload: record.upload.take(),
            });
            world.insert_component_value(
                target.entity_id.as_str().into(),
                crate::archetype::ComponentValue::MeshRenderer3D(renderer),
            );
        } else if candidate.release {
            if let Some(old) = self.generated_content.bindings[&key].published.as_ref() {
                if owns_renderer(old, world, &key) {
                    world.remove_component_value(
                        &target.entity_id.as_str().into(),
                        &crate::components::ComponentTypeId::mesh_renderer3d(),
                    );
                }
            }
        }
        let binding = self.generated_content.bindings.get_mut(&key).unwrap();
        if changes_geometry {
            if let Some(old) = binding.published.take() {
                cache.retire_generated_binding(frame, &old.key, manager);
            }
            binding.layout = candidate.geometry.as_ref().map(|geometry| {
                let (vertex_count, vertex_stride, attributes) = geometry.sdk_layout();
                Layout {
                    vertex_count,
                    vertex_stride,
                    attributes,
                }
            });
            binding.canonical = candidate.geometry;
            binding.published = publication;
        }
        binding.applied = revision;
        binding.pending = None;
        self.generated_content.commit_epoch = next_epoch;
        let record = self
            .generated_content
            .records
            .get_mut(&ticket.request_id)
            .unwrap();
        record.receipt.status = ContentStatus::Applied;
        record.receipt.commit_epoch = Some(next_epoch);
        self.generated_content
            .reclaim_terminal(ticket.request_id, &mut self.asset_loader);
        self.generated_content.update_pending_receipts(&target);
        Ok(())
    }
}
