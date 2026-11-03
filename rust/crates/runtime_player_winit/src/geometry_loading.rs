//! Native frame preparation using the existing texture owner and upload receipts.
use engine_runtime::render_asset_production::PreparedTextureAsset;
use engine_runtime::wgpu_backend::real::RealWgpuBackend;
use engine_runtime::wgpu_backend::real::{GeometryUploadState, GeometryUploadTicket};
use std::sync::Arc;

#[derive(Default)]
pub(super) struct GeometryTextureUpload {
    current: Option<(Arc<Vec<PreparedTextureAsset>>, GeometryUploadTicket)>,
}

impl GeometryTextureUpload {
    pub(super) fn prepare(
        &mut self,
        backend: &mut RealWgpuBackend,
        textures: &Arc<Vec<PreparedTextureAsset>>,
        frame_index: u64,
    ) -> Result<bool, String> {
        if textures.is_empty() && self.current.is_none() {
            return Ok(true);
        }
        if self
            .current
            .as_ref()
            .is_none_or(|(old, _)| !Arc::ptr_eq(old, textures) && old.as_ref() != textures.as_ref())
        {
            // All old submitted uses must complete before replacing a potentially
            // large static set. The new ticket pins shared resources before the
            // old one is released; unchanged textures are not uploaded again.
            backend.poll_gpu_completion()?;
            if self.current.is_some() && backend.gpu_lifecycle_stats().in_flight != 0 {
                return Ok(false);
            }
            let ticket = backend.begin_geometry_upload(&[], textures)?;
            if let Some((_, old)) = &self.current {
                if let Err(error) = backend.release_geometry_upload(*old) {
                    backend.release_geometry_upload(ticket)?;
                    return Err(error);
                }
            }
            self.current = Some((Arc::clone(textures), ticket));
        }
        let ticket = self.current.as_ref().unwrap().1;
        backend.poll_gpu_completion()?;
        if backend.geometry_upload_state(ticket)? == GeometryUploadState::Ready {
            return Ok(true);
        }
        // Upload and loading-clear submissions share the two-in-flight native
        // preparation profile. Polling cannot manufacture an allowance/Present.
        if backend.gpu_lifecycle_stats().in_flight >= 2 {
            return Ok(false);
        }
        // The native caller binds an actual presentation allowance. The argument
        // remains the current logical frame for offscreen owner tests.
        backend.pump_geometry_uploads(frame_index)?;
        match backend.geometry_upload_state(ticket)? {
            GeometryUploadState::Ready => Ok(true),
            GeometryUploadState::Uploading | GeometryUploadState::Submitted => Ok(false),
            GeometryUploadState::Abandoned => Err("geometry.upload_device_abandoned".into()),
        }
    }

    pub(super) fn release(&mut self, backend: &mut RealWgpuBackend) -> Result<(), String> {
        if let Some((_, ticket)) = self.current.as_ref() {
            backend.release_geometry_upload(*ticket)?;
            self.current = None;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_runtime::render_asset_production::RuntimeRenderAssetProducer;
    use engine_runtime::render_contract::RenderBindingKind;
    use engine_runtime::render_resource::RenderResourceManager;
    use engine_runtime::runtime_texture::RuntimeTexturePayload;
    use engine_runtime::wgpu_backend::real::GeometryUploadBudget;

    #[test]
    fn native_geometry_preparation_segments_instead_of_synchronously_registering() {
        let mut backend = RealWgpuBackend::new_offscreen(8, 8).expect("real GPU required");
        backend
            .set_geometry_upload_budget(GeometryUploadBudget {
                resident_bytes: 1024 * 1024,
                staging_bytes: 16,
                bytes_per_frame: 16,
                max_tickets: 4,
            })
            .unwrap();
        let mut manager = RenderResourceManager::new();
        let texture = RuntimeRenderAssetProducer::new()
            .prepare_texture_payload(
                1,
                RuntimeTexturePayload {
                    asset_id: "native-loading".into(),
                    cooked_asset_id: "native-loading.cooked".into(),
                    width: 4,
                    height: 4,
                    format: "rgba8Unorm".into(),
                    color_space: "linear".into(),
                    mip_count: 1,
                    rgba8: vec![255; 64].into(),
                    sampler: "nearestClamp".into(),
                    source_hash: "native-loading-source".into(),
                },
                RenderBindingKind::Texture,
                &mut manager,
            )
            .unwrap();
        let textures = Arc::new(vec![texture]);
        let mut upload = GeometryTextureUpload::default();
        backend.pump_geometry_uploads(1).unwrap();
        assert!(
            !upload.prepare(&mut backend, &textures, 1).unwrap(),
            "a 64-byte texture cannot be ready in a 16-byte presentation allowance"
        );
        assert!(backend.geometry_upload_snapshot().frame_uploaded_bytes <= 16);
        assert!(!upload.prepare(&mut backend, &textures, 1).unwrap());
        assert_eq!(backend.geometry_upload_snapshot().frame_uploaded_bytes, 16);
        for frame in 2..=4 {
            backend
                .shutdown_gpu(std::time::Duration::from_secs(2))
                .unwrap();
            backend.bind_geometry_upload_presentation(frame).unwrap();
            // A game frame stays frozen at 1 while loading presentations advance.
            let _ = upload.prepare(&mut backend, &textures, 1).unwrap();
            assert_eq!(backend.geometry_upload_snapshot().frame_uploaded_bytes, 16);
        }
        backend
            .shutdown_gpu(std::time::Duration::from_secs(2))
            .unwrap();
        assert!(upload.prepare(&mut backend, &textures, 1).unwrap());
        assert_eq!(backend.geometry_upload_snapshot().uploaded_bytes, 64);
        assert!(backend.bind_geometry_upload_presentation(3).is_err());
        // Equal values in another Arc reuse the actual allocation and receipt.
        assert!(upload
            .prepare(&mut backend, &Arc::new(textures.as_ref().clone()), 1)
            .unwrap());
        assert_eq!(backend.geometry_upload_snapshot().uploaded_bytes, 64);
        upload.release(&mut backend).unwrap();
        assert_eq!(backend.geometry_upload_snapshot().tickets, 0);
        assert_eq!(backend.geometry_residency().textures, 0);
    }
}
