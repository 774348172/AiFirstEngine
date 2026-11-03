//! Pending generated preparations belong to the original geometry cache. The same
//! handle union protects materials shared by authored, applied and pending geometry.
use super::*;
use crate::render_geometry::generated::GeneratedGeometry;
use project_runtime_sdk::generated_content::{
    ContentError, ContentErrorCode, ContentTicket, GENERATED_RECEIPT_LIMIT,
};

impl ProductionGeometryCache {
    pub(crate) fn prepare_generated_candidate(
        &mut self,
        frame: u64,
        generated: &GeneratedGeometry,
        ticket: ContentTicket,
        revision: u64,
        loader: &mut RuntimeAssetLoader,
        manager: &mut RenderResourceManager,
    ) -> Result<Arc<PreparedMeshRenderer>, ContentError> {
        let key = (ticket.session_generation, ticket.request_id, revision);
        if ticket.session_generation == 0
            || ticket.request_id == 0
            || revision == 0
            || self.generated_candidates.contains_key(&key)
        {
            return Err(content_error(
                ticket,
                ContentErrorCode::InvalidArgument,
                "render.candidateIdentity",
            ));
        }
        if self.generated_candidates.len() >= GENERATED_RECEIPT_LIMIT {
            return Err(content_error(
                ticket,
                ContentErrorCode::BudgetExceeded,
                "render.candidateWindow",
            ));
        }
        let mut prepared = RuntimeRenderAssetProducer
            .prepare_generated_geometry(frame, generated, ticket, revision, loader, manager)
            .map_err(|failure| {
                content_error(
                    ticket,
                    match failure.code {
                        ContractErrorCode::ResourceNotReady
                        | ContractErrorCode::StaleResourceHandle => ContentErrorCode::CapabilityGap,
                        _ => ContentErrorCode::InvalidArgument,
                    },
                    failure.field,
                )
            })?;
        for texture in &mut prepared.textures {
            texture.make_package_streamable(loader, &mut self.texture_sources);
            texture.release_loader_pixels(loader);
        }
        let prepared = Arc::new(prepared);
        self.generated_candidates.insert(key, prepared.clone());
        self.refresh_owned_handles(frame, manager);
        Ok(prepared)
    }

    /// The original backend must separately retire its upload ticket. This only drops
    /// the cache's logical ownership; retained native/frame readers keep their leases.
    pub(crate) fn release_generated_candidate(
        &mut self,
        frame: u64,
        ticket: ContentTicket,
        revision: u64,
        manager: &mut RenderResourceManager,
    ) -> Result<(), ContentError> {
        self.generated_candidates
            .remove(&(ticket.session_generation, ticket.request_id, revision))
            .ok_or_else(|| {
                content_error(
                    ticket,
                    ContentErrorCode::NotFound,
                    "render.candidateIdentity",
                )
            })?;
        self.refresh_owned_handles(frame, manager);
        Ok(())
    }

    pub(crate) fn generated_candidate(
        &self,
        ticket: ContentTicket,
        revision: u64,
    ) -> Option<Arc<PreparedMeshRenderer>> {
        self.generated_candidates
            .get(&(ticket.session_generation, ticket.request_id, revision))
            .cloned()
    }

    pub(crate) fn generated_binding(&self, key: &str) -> Option<Arc<PreparedMeshRenderer>> {
        self.prepared.get(key).cloned()
    }

    pub(crate) fn publish_generated_candidate(
        &mut self,
        ticket: ContentTicket,
        revision: u64,
        package: &str,
        renderer: &crate::spatial_contract::MeshRenderer3D,
    ) -> Result<Arc<str>, ContentError> {
        let prepared = self
            .generated_candidates
            .remove(&(ticket.session_generation, ticket.request_id, revision))
            .ok_or_else(|| {
                content_error(
                    ticket,
                    ContentErrorCode::NotFound,
                    "render.candidateIdentity",
                )
            })?;
        let key: Arc<str> = serde_json::to_string(&(package, &renderer.mesh, &renderer.materials))
            .expect("typed binding")
            .into();
        // The same logical union owns the candidate before and after this move.
        self.prepared.insert(key.clone(), prepared);
        self.generated_bindings.insert(key.clone());
        self.generated_changed = true;
        self.last_proxy_identity = None;
        Ok(key)
    }

    pub(crate) fn retire_generated_binding(
        &mut self,
        frame: u64,
        key: &str,
        manager: &mut RenderResourceManager,
    ) {
        self.generated_bindings.remove(key);
        if self.prepared.remove(key).is_some() {
            self.generated_changed = true;
            self.last_proxy_identity = None;
            self.refresh_owned_handles(frame, manager);
        }
    }

    pub(super) fn refresh_owned_handles(
        &mut self,
        frame: u64,
        manager: &mut RenderResourceManager,
    ) {
        let new_handles: HashSet<_> = self
            .prepared
            .values()
            .chain(self.generated_candidates.values())
            .flat_map(|prepared| prepared.resource_handles())
            .chain(self.vegetation.handles.iter().copied())
            .collect();
        for handle in new_handles.difference(&self.owned_handles) {
            let _ = manager.retain_resource_ownership(*handle);
        }
        for handle in self.owned_handles.difference(&new_handles) {
            let _ = manager.release_resource_ownership(RenderResourceReleaseRequest {
                handle: *handle,
                frame_index: frame,
            });
        }
        self.owned_handles = new_handles;
    }

    pub(super) fn published_resource_handles(
        &self,
    ) -> Vec<crate::render_resource::RenderResourceHandle> {
        if self.generated_candidates.is_empty() {
            return self.owned_handles.iter().copied().collect();
        }
        let pending: HashSet<_> = self
            .generated_candidates
            .values()
            .flat_map(|prepared| prepared.resource_handles())
            .collect();
        let published: HashSet<_> = self
            .prepared
            .values()
            .flat_map(|prepared| prepared.resource_handles())
            .chain(self.vegetation.handles.iter().copied())
            .collect();
        self.owned_handles
            .iter()
            .filter(|handle| !pending.contains(handle) || published.contains(handle))
            .copied()
            .collect()
    }
}

fn content_error(ticket: ContentTicket, code: ContentErrorCode, field: &str) -> ContentError {
    ContentError {
        code,
        request_id: ticket.request_id,
        field_path: field.into(),
        expected: None,
        actual: None,
        next_action: match code {
            ContentErrorCode::BudgetExceeded => {
                "Wait for pending candidates to retire before submitting more work."
            }
            ContentErrorCode::NotFound => "Use a candidate retained by the current geometry cache.",
            _ => "Correct the candidate identity or package material field reported by the error.",
        }
        .into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render_geometry::generated::binding_tests::{binding, fixture, geometry};
    use crate::render_resource::{RenderResourceKind, RenderResourceState};

    fn prepared(
        manager: &mut RenderResourceManager,
        material: &crate::render_asset_production::PreparedCookedRenderAsset,
        revision: u64,
    ) -> Arc<PreparedMeshRenderer> {
        let generated = GeneratedGeometry::prepare(geometry(3, [0., 0., 1.], [1.; 4])).unwrap();
        let (binding, source) = binding(manager, &generated, revision);
        Arc::new(PreparedMeshRenderer {
            geometry: vec![generated
                .bind(0, binding, source, material, manager)
                .unwrap()],
            textures: vec![],
            retained_resources: vec![],
        })
    }

    #[test]
    fn generated_content_376_mesh_original_cache_union_protects_shared_material_and_hides_pending_mesh(
    ) {
        let (mut manager, material, _) = fixture();
        let mut cache = ProductionGeometryCache::default();
        let authored = prepared(&mut manager, &material, 1);
        let pending = prepared(&mut manager, &material, 2);
        let a: HashSet<_> = authored.resource_handles().collect();
        let b: HashSet<_> = pending.resource_handles().collect();
        let material_handle = *a
            .iter()
            .find(|h| h.kind == RenderResourceKind::MaterialParams)
            .unwrap();
        let pending_mesh = *b
            .iter()
            .find(|h| h.kind == RenderResourceKind::MeshBuffer)
            .unwrap();
        cache.prepared.insert("authored".into(), authored.clone());
        cache
            .generated_candidates
            .insert((376, 2, 2), pending.clone());
        cache.refresh_owned_handles(1, &mut manager);
        assert_eq!(cache.owned_handles, a.union(&b).copied().collect());
        assert_eq!(
            cache
                .published_resource_handles()
                .into_iter()
                .collect::<HashSet<_>>(),
            a
        );
        cache
            .release_generated_candidate(
                2,
                ContentTicket {
                    session_generation: 376,
                    request_id: 2,
                },
                2,
                &mut manager,
            )
            .unwrap();
        assert_eq!(
            manager.record(material_handle).unwrap().state,
            RenderResourceState::Resident
        );
        assert_eq!(
            manager.record(pending_mesh).unwrap().state,
            RenderResourceState::PendingRelease
        );
        for handle in &a {
            assert_eq!(
                manager.record(*handle).unwrap().state,
                RenderResourceState::Resident
            );
        }
        let lease = manager.lease_resources(a.iter().copied());
        cache.reset_world(3, &mut manager);
        assert!(cache.generated_candidates.is_empty());
        manager.end_frame(10);
        for handle in &a {
            assert_eq!(
                manager.record(*handle).unwrap().state,
                RenderResourceState::PendingRelease
            );
        }
        drop(lease);
        manager.end_frame(11);
        for handle in &a {
            assert_eq!(
                manager.record(*handle).unwrap().state,
                RenderResourceState::Released
            );
        }
        assert_eq!(manager.report(11).pending_release_count, 0);
    }

    #[test]
    fn generated_content_376_mesh_original_cache_candidate_window_refuses_before_material_work() {
        let (mut manager, material, _) = fixture();
        let mut cache = ProductionGeometryCache::default();
        let held = prepared(&mut manager, &material, 1);
        for id in 1..=GENERATED_RECEIPT_LIMIT as u64 {
            cache
                .generated_candidates
                .insert((376, id, 1), held.clone());
        }
        cache.refresh_owned_handles(1, &mut manager);
        let before = serde_json::to_value(manager.report(2)).unwrap();
        let mut assets = RuntimeAssetLoader::new(
            "unused",
            crate::runtime_asset::RuntimeAssetIndex::new(vec![], vec![]),
            crate::runtime_asset::RuntimePackageMountTable::new(vec![]),
        );
        let data = GeneratedGeometry::prepare(geometry(3, [0., 0., 1.], [1.; 4])).unwrap();
        let failure = cache
            .prepare_generated_candidate(
                2,
                &data,
                ContentTicket {
                    session_generation: 376,
                    request_id: 129,
                },
                1,
                &mut assets,
                &mut manager,
            )
            .unwrap_err();
        assert_eq!(failure.code, ContentErrorCode::BudgetExceeded);
        assert_eq!(failure.field_path, "render.candidateWindow");
        assert_eq!(cache.generated_candidates.len(), GENERATED_RECEIPT_LIMIT);
        assert_eq!(serde_json::to_value(manager.report(2)).unwrap(), before);
        cache.reset_world(3, &mut manager);
        assert!(cache.generated_candidates.is_empty());
        manager.end_frame(10);
        assert_eq!(manager.report(10).pending_release_count, 0);
    }
}
