use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderResourceKind {
    Texture,
    MeshBuffer,
    MaterialParams,
    ShaderPipeline,
    SurfaceFrameTexture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderResourceState {
    Missing,
    Requested,
    Uploading,
    Resident,
    Stale,
    PendingRelease,
    Released,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderResourceHandle {
    pub kind: RenderResourceKind,
    pub index: u64,
    pub generation: u64,
}

impl RenderResourceHandle {
    pub fn is_stale_for(&self, current: &RenderResourceHandle) -> bool {
        self.kind == current.kind
            && self.index == current.index
            && self.generation != current.generation
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderAssetKey {
    pub asset_id: String,
    pub asset_version: String,
    pub resource_kind: RenderResourceKind,
    pub platform_profile: String,
    pub quality_profile: String,
    pub usage: String,
}

impl RenderAssetKey {
    pub fn new(
        asset_id: impl Into<String>,
        asset_version: impl Into<String>,
        resource_kind: RenderResourceKind,
    ) -> Self {
        Self {
            asset_id: asset_id.into(),
            asset_version: asset_version.into(),
            resource_kind,
            platform_profile: "dev-desktop".to_string(),
            quality_profile: "default".to_string(),
            usage: "default".to_string(),
        }
    }

    fn same_asset_shape(&self, other: &Self) -> bool {
        self.asset_id == other.asset_id
            && self.resource_kind == other.resource_kind
            && self.platform_profile == other.platform_profile
            && self.quality_profile == other.quality_profile
            && self.usage == other.usage
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderResourceSource {
    TextureDescriptor {
        width: u32,
        height: u32,
        format: String,
    },
    MeshBufferDescriptor {
        vertex_count: u32,
        index_count: u32,
    },
    MaterialParamsDescriptor {
        param_count: u32,
    },
    ShaderPipelineDescriptor {
        shader_id: String,
    },
    SurfaceFrameTextureDescriptor {
        target_id: String,
        width: u32,
        height: u32,
        format: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderResourceRequest {
    pub key: RenderAssetKey,
    pub source: RenderResourceSource,
    pub byte_len: usize,
    pub reason: String,
}

impl RenderResourceRequest {
    pub fn texture(
        asset_id: impl Into<String>,
        version: impl Into<String>,
        byte_len: usize,
    ) -> Self {
        Self {
            key: RenderAssetKey::new(asset_id, version, RenderResourceKind::Texture),
            source: RenderResourceSource::TextureDescriptor {
                width: 1,
                height: 1,
                format: "Rgba8Unorm".to_string(),
            },
            byte_len,
            reason: "test_texture_request".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderResourceReleaseRequest {
    pub handle: RenderResourceHandle,
    pub frame_index: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderResourceRecord {
    pub handle: RenderResourceHandle,
    pub key: RenderAssetKey,
    pub state: RenderResourceState,
    pub bytes: usize,
    pub last_used_frame: u64,
    pub pending_release_frame: Option<u64>,
    pub owner: String,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderUploadBudget {
    pub max_bytes_per_frame: Option<usize>,
    pub uploaded_bytes_this_frame: usize,
    pub deferred_request_count: usize,
}

impl RenderUploadBudget {
    pub fn unlimited() -> Self {
        Self {
            max_bytes_per_frame: None,
            uploaded_bytes_this_frame: 0,
            deferred_request_count: 0,
        }
    }

    pub fn new(max_bytes_per_frame: usize) -> Self {
        Self {
            max_bytes_per_frame: Some(max_bytes_per_frame),
            uploaded_bytes_this_frame: 0,
            deferred_request_count: 0,
        }
    }

    fn can_upload(&self, byte_len: usize) -> bool {
        match self.max_bytes_per_frame {
            None => true,
            Some(max) => {
                self.uploaded_bytes_this_frame == 0
                    || self.uploaded_bytes_this_frame.saturating_add(byte_len) <= max
            }
        }
    }

    fn record_upload(&mut self, byte_len: usize) {
        self.uploaded_bytes_this_frame = self.uploaded_bytes_this_frame.saturating_add(byte_len);
    }

    fn record_deferred(&mut self) {
        self.deferred_request_count += 1;
    }

    fn reset_frame(&mut self) {
        self.uploaded_bytes_this_frame = 0;
        self.deferred_request_count = 0;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderResourceEventType {
    Requested,
    Created,
    Reused,
    Uploading,
    Uploaded,
    Stale,
    PendingRelease,
    Released,
    Failed,
    Deferred,
    DeviceLost,
    SurfaceLost,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderResourceDiagnostic {
    pub code: String,
    pub message: String,
}

impl RenderResourceDiagnostic {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderResourceEvent {
    pub event_type: RenderResourceEventType,
    pub resource_kind: RenderResourceKind,
    pub asset_id: String,
    pub generation: u64,
    pub state_before: RenderResourceState,
    pub state_after: RenderResourceState,
    pub bytes: usize,
    pub reason: String,
    pub diagnostic: Option<RenderResourceDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderResourceLifetimeReport {
    pub schema_version: String,
    pub frame_index: u64,
    pub created_count: usize,
    pub reused_count: usize,
    pub uploaded_bytes: usize,
    pub resident_bytes: usize,
    pub pending_release_count: usize,
    pub failed_count: usize,
    pub events: Vec<RenderResourceEvent>,
}

impl RenderResourceLifetimeReport {
    pub fn empty(frame_index: u64) -> Self {
        Self {
            schema_version: "render-resource-lifetime-report.v1".to_string(),
            frame_index,
            created_count: 0,
            reused_count: 0,
            uploaded_bytes: 0,
            resident_bytes: 0,
            pending_release_count: 0,
            failed_count: 0,
            events: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderResourceError {
    MissingHandle,
    StaleHandle {
        requested: RenderResourceHandle,
        current: RenderResourceHandle,
    },
    ReleasedHandle,
}

#[derive(Debug, Clone)]
pub struct RenderResourceManager {
    records: Vec<Option<RenderResourceRecord>>,
    key_to_handle: BTreeMap<RenderAssetKey, RenderResourceHandle>,
    frame_events: Vec<RenderResourceEvent>,
    upload_budget: RenderUploadBudget,
    safe_frame_delay: u64,
    current_frame: u64,
}

impl Default for RenderResourceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderResourceManager {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            key_to_handle: BTreeMap::new(),
            frame_events: Vec::new(),
            upload_budget: RenderUploadBudget::unlimited(),
            safe_frame_delay: 2,
            current_frame: 0,
        }
    }

    pub fn with_safe_frame_delay(safe_frame_delay: u64) -> Self {
        Self {
            safe_frame_delay,
            ..Self::new()
        }
    }

    pub fn set_upload_budget(&mut self, upload_budget: RenderUploadBudget) {
        self.upload_budget = upload_budget;
    }

    pub fn request_resource(
        &mut self,
        frame_index: u64,
        request: RenderResourceRequest,
    ) -> RenderResourceHandle {
        self.current_frame = frame_index;
        if let Some(existing) = self.key_to_handle.get(&request.key).copied() {
            if self
                .record(existing)
                .is_some_and(|record| record.state == RenderResourceState::Resident)
            {
                self.push_event(
                    RenderResourceEventType::Reused,
                    &request.key,
                    existing.generation,
                    RenderResourceState::Resident,
                    RenderResourceState::Resident,
                    request.byte_len,
                    "Resident resource reused",
                    None,
                );
                return existing;
            }
        }

        self.mark_previous_versions_stale(frame_index, &request.key);
        let handle = self.allocate_handle(request.key.resource_kind);
        let mut state = RenderResourceState::Requested;
        let mut last_error = None;
        self.push_event(
            RenderResourceEventType::Requested,
            &request.key,
            handle.generation,
            RenderResourceState::Missing,
            RenderResourceState::Requested,
            request.byte_len,
            request.reason.clone(),
            None,
        );

        if self.upload_budget.can_upload(request.byte_len) {
            self.push_event(
                RenderResourceEventType::Uploading,
                &request.key,
                handle.generation,
                RenderResourceState::Requested,
                RenderResourceState::Uploading,
                request.byte_len,
                "Uploading render resource",
                None,
            );
            self.upload_budget.record_upload(request.byte_len);
            state = RenderResourceState::Resident;
            self.push_event(
                RenderResourceEventType::Uploaded,
                &request.key,
                handle.generation,
                RenderResourceState::Uploading,
                RenderResourceState::Resident,
                request.byte_len,
                "Render resource resident",
                None,
            );
        } else {
            self.upload_budget.record_deferred();
            last_error = Some("UploadBudgetDeferred".to_string());
            self.push_event(
                RenderResourceEventType::Deferred,
                &request.key,
                handle.generation,
                RenderResourceState::Requested,
                RenderResourceState::Requested,
                request.byte_len,
                "Upload budget exhausted",
                Some(RenderResourceDiagnostic::new(
                    "UploadBudgetDeferred",
                    "Render resource upload was deferred by the per-frame upload budget.",
                )),
            );
        }

        let record = RenderResourceRecord {
            handle,
            key: request.key.clone(),
            state,
            bytes: request.byte_len,
            last_used_frame: frame_index,
            pending_release_frame: None,
            owner: "RenderThread".to_string(),
            last_error,
        };
        self.key_to_handle.insert(request.key, handle);
        self.records[handle.index as usize] = Some(record);
        handle
    }

    pub fn release_resource(
        &mut self,
        request: RenderResourceReleaseRequest,
    ) -> Result<(), RenderResourceError> {
        let record = self.record_mut_checked(request.handle)?;
        let before = record.state;
        if before == RenderResourceState::Released {
            return Err(RenderResourceError::ReleasedHandle);
        }
        record.state = RenderResourceState::PendingRelease;
        record.pending_release_frame = Some(request.frame_index);
        let key = record.key.clone();
        let bytes = record.bytes;
        let generation = record.handle.generation;
        self.push_event(
            RenderResourceEventType::PendingRelease,
            &key,
            generation,
            before,
            RenderResourceState::PendingRelease,
            bytes,
            "Released by runtime unload",
            None,
        );
        Ok(())
    }

    pub fn mark_used(
        &mut self,
        handle: RenderResourceHandle,
        frame_index: u64,
    ) -> Result<(), RenderResourceError> {
        let record = self.record_mut_checked(handle)?;
        record.last_used_frame = frame_index;
        Ok(())
    }

    pub fn end_frame(&mut self, frame_index: u64) -> RenderResourceLifetimeReport {
        self.current_frame = frame_index;
        let mut release_events = Vec::new();
        for record in self.records.iter_mut().flatten() {
            if record.state != RenderResourceState::PendingRelease {
                continue;
            }
            let Some(pending_frame) = record.pending_release_frame else {
                continue;
            };
            if frame_index >= pending_frame.saturating_add(self.safe_frame_delay) {
                let before = record.state;
                record.state = RenderResourceState::Released;
                release_events.push(RenderResourceEvent {
                    event_type: RenderResourceEventType::Released,
                    resource_kind: record.key.resource_kind,
                    asset_id: record.key.asset_id.clone(),
                    generation: record.handle.generation,
                    state_before: before,
                    state_after: RenderResourceState::Released,
                    bytes: record.bytes,
                    reason: "Safe frame delay elapsed".to_string(),
                    diagnostic: None,
                });
            }
        }
        self.frame_events.extend(release_events);
        let report = self.report(frame_index);
        self.frame_events.clear();
        self.upload_budget.reset_frame();
        report
    }

    pub fn report(&self, frame_index: u64) -> RenderResourceLifetimeReport {
        let resident_bytes = self
            .records
            .iter()
            .flatten()
            .filter(|record| record.state == RenderResourceState::Resident)
            .map(|record| record.bytes)
            .sum();
        let pending_release_count = self
            .records
            .iter()
            .flatten()
            .filter(|record| record.state == RenderResourceState::PendingRelease)
            .count();
        let failed_count = self
            .records
            .iter()
            .flatten()
            .filter(|record| record.state == RenderResourceState::Failed)
            .count();
        RenderResourceLifetimeReport {
            schema_version: "render-resource-lifetime-report.v1".to_string(),
            frame_index,
            created_count: self
                .frame_events
                .iter()
                .filter(|event| event.event_type == RenderResourceEventType::Requested)
                .count(),
            reused_count: self
                .frame_events
                .iter()
                .filter(|event| event.event_type == RenderResourceEventType::Reused)
                .count(),
            uploaded_bytes: self
                .frame_events
                .iter()
                .filter(|event| event.event_type == RenderResourceEventType::Uploaded)
                .map(|event| event.bytes)
                .sum(),
            resident_bytes,
            pending_release_count,
            failed_count,
            events: self.frame_events.clone(),
        }
    }

    pub fn record(&self, handle: RenderResourceHandle) -> Option<&RenderResourceRecord> {
        self.records
            .get(handle.index as usize)
            .and_then(|record| record.as_ref())
            .filter(|record| record.handle == handle)
    }

    pub fn validate_handle(
        &self,
        handle: RenderResourceHandle,
    ) -> Result<&RenderResourceRecord, RenderResourceError> {
        let Some(slot) = self
            .records
            .get(handle.index as usize)
            .and_then(|record| record.as_ref())
        else {
            return Err(RenderResourceError::MissingHandle);
        };
        if slot.handle != handle {
            return Err(RenderResourceError::StaleHandle {
                requested: handle,
                current: slot.handle,
            });
        }
        if slot.state == RenderResourceState::Released {
            return Err(RenderResourceError::ReleasedHandle);
        }
        Ok(slot)
    }

    pub fn mark_device_lost(&mut self, frame_index: u64) -> RenderResourceLifetimeReport {
        self.current_frame = frame_index;
        let mut events = Vec::new();
        for record in self.records.iter_mut().flatten() {
            if record.state != RenderResourceState::Resident {
                continue;
            }
            let before = record.state;
            record.state = RenderResourceState::Failed;
            record.last_error = Some("DeviceLost".to_string());
            events.push(RenderResourceEvent {
                event_type: RenderResourceEventType::DeviceLost,
                resource_kind: record.key.resource_kind,
                asset_id: record.key.asset_id.clone(),
                generation: record.handle.generation,
                state_before: before,
                state_after: RenderResourceState::Failed,
                bytes: record.bytes,
                reason: "Device lost".to_string(),
                diagnostic: Some(RenderResourceDiagnostic::new(
                    "DeviceLost",
                    "Backend device was lost; resource must be recreated from RuntimeAsset data.",
                )),
            });
        }
        self.frame_events.extend(events);
        self.report(frame_index)
    }

    pub fn mark_surface_lost(&mut self, frame_index: u64) -> RenderResourceLifetimeReport {
        self.current_frame = frame_index;
        let mut events = Vec::new();
        for record in self.records.iter_mut().flatten() {
            if record.state != RenderResourceState::Resident
                || record.key.resource_kind != RenderResourceKind::SurfaceFrameTexture
            {
                continue;
            }
            let before = record.state;
            record.state = RenderResourceState::Failed;
            record.last_error = Some("SurfaceLost".to_string());
            events.push(RenderResourceEvent {
                event_type: RenderResourceEventType::SurfaceLost,
                resource_kind: record.key.resource_kind,
                asset_id: record.key.asset_id.clone(),
                generation: record.handle.generation,
                state_before: before,
                state_after: RenderResourceState::Failed,
                bytes: record.bytes,
                reason: "Surface lost".to_string(),
                diagnostic: Some(RenderResourceDiagnostic::new(
                    "SurfaceLost",
                    "Surface frame texture was lost; non-surface resources remain resident.",
                )),
            });
        }
        self.frame_events.extend(events);
        self.report(frame_index)
    }

    fn allocate_handle(&mut self, kind: RenderResourceKind) -> RenderResourceHandle {
        if let Some((index, slot)) = self.records.iter_mut().enumerate().find(|(_, record)| {
            record
                .as_ref()
                .is_some_and(|record| record.state == RenderResourceState::Released)
        }) {
            let generation = slot
                .as_ref()
                .map(|record| record.handle.generation + 1)
                .unwrap_or(0);
            return RenderResourceHandle {
                kind,
                index: index as u64,
                generation,
            };
        }
        let index = self.records.len() as u64;
        self.records.push(None);
        RenderResourceHandle {
            kind,
            index,
            generation: 0,
        }
    }

    fn mark_previous_versions_stale(&mut self, frame_index: u64, key: &RenderAssetKey) {
        let mut stale_keys = Vec::new();
        let mut stale_events = Vec::new();
        for record in self.records.iter_mut().flatten() {
            if record.key.same_asset_shape(key)
                && record.key.asset_version != key.asset_version
                && record.state == RenderResourceState::Resident
            {
                let before = record.state;
                record.state = RenderResourceState::Stale;
                record.last_used_frame = frame_index;
                stale_keys.push(record.key.clone());
                stale_events.push(RenderResourceEvent {
                    event_type: RenderResourceEventType::Stale,
                    resource_kind: record.key.resource_kind,
                    asset_id: record.key.asset_id.clone(),
                    generation: record.handle.generation,
                    state_before: before,
                    state_after: RenderResourceState::Stale,
                    bytes: record.bytes,
                    reason: "Replaced by newer asset version".to_string(),
                    diagnostic: Some(RenderResourceDiagnostic::new(
                        "ReplacedByHotUpdate",
                        "A newer render asset generation replaced this resident resource.",
                    )),
                });
            }
        }
        for stale_key in stale_keys {
            self.key_to_handle.remove(&stale_key);
        }
        self.frame_events.extend(stale_events);
    }

    fn record_mut_checked(
        &mut self,
        handle: RenderResourceHandle,
    ) -> Result<&mut RenderResourceRecord, RenderResourceError> {
        let Some(slot) = self
            .records
            .get_mut(handle.index as usize)
            .and_then(|record| record.as_mut())
        else {
            return Err(RenderResourceError::MissingHandle);
        };
        if slot.handle != handle {
            return Err(RenderResourceError::StaleHandle {
                requested: handle,
                current: slot.handle,
            });
        }
        Ok(slot)
    }

    fn push_event(
        &mut self,
        event_type: RenderResourceEventType,
        key: &RenderAssetKey,
        generation: u64,
        state_before: RenderResourceState,
        state_after: RenderResourceState,
        bytes: usize,
        reason: impl Into<String>,
        diagnostic: Option<RenderResourceDiagnostic>,
    ) {
        self.frame_events.push(RenderResourceEvent {
            event_type,
            resource_kind: key.resource_kind,
            asset_id: key.asset_id.clone(),
            generation,
            state_before,
            state_after,
            bytes,
            reason: reason.into(),
            diagnostic,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texture_request(asset_id: &str, version: &str, bytes: usize) -> RenderResourceRequest {
        RenderResourceRequest::texture(asset_id, version, bytes)
    }

    #[test]
    fn render_resource_handle_generation_rejects_stale_handle() {
        let mut manager = RenderResourceManager::with_safe_frame_delay(1);
        let first = manager.request_resource(1, texture_request("texture-a", "v1", 4));
        manager
            .release_resource(RenderResourceReleaseRequest {
                handle: first,
                frame_index: 2,
            })
            .expect("release first");
        manager.end_frame(3);
        let second = manager.request_resource(4, texture_request("texture-b", "v1", 4));

        assert_eq!(first.index, second.index);
        assert!(first.is_stale_for(&second));
        assert!(matches!(
            manager.validate_handle(first),
            Err(RenderResourceError::StaleHandle { .. })
        ));
    }

    #[test]
    fn render_resource_lifetime_report_serializes_stably() {
        let mut manager = RenderResourceManager::new();
        manager.request_resource(1, texture_request("texture-a", "v1", 16));

        let report = manager.end_frame(1);
        let json = serde_json::to_string(&report).expect("serialize report");
        let roundtrip: RenderResourceLifetimeReport =
            serde_json::from_str(&json).expect("deserialize report");

        assert_eq!(
            roundtrip.schema_version,
            "render-resource-lifetime-report.v1"
        );
        assert_eq!(roundtrip.created_count, 1);
        assert_eq!(roundtrip.uploaded_bytes, 16);
    }

    #[test]
    fn render_resource_pool_creates_and_reuses_texture() {
        let mut manager = RenderResourceManager::new();
        let first = manager.request_resource(1, texture_request("texture-a", "v1", 16));
        let second = manager.request_resource(1, texture_request("texture-a", "v1", 16));

        let report = manager.end_frame(1);

        assert_eq!(first, second);
        assert_eq!(report.created_count, 1);
        assert_eq!(report.reused_count, 1);
        assert_eq!(
            manager.record(first).map(|record| record.state),
            Some(RenderResourceState::Resident)
        );
    }

    #[test]
    fn render_resource_pool_delays_release_until_safe_frame() {
        let mut manager = RenderResourceManager::with_safe_frame_delay(2);
        let handle = manager.request_resource(1, texture_request("texture-a", "v1", 16));
        manager
            .release_resource(RenderResourceReleaseRequest {
                handle,
                frame_index: 2,
            })
            .expect("release");

        let before_safe = manager.end_frame(3);
        assert_eq!(before_safe.pending_release_count, 1);
        assert_eq!(
            manager.record(handle).map(|record| record.state),
            Some(RenderResourceState::PendingRelease)
        );

        manager.end_frame(4);
        assert_eq!(
            manager.validate_handle(handle),
            Err(RenderResourceError::ReleasedHandle)
        );
    }

    #[test]
    fn render_resource_pool_replaces_hot_update_generation() {
        let mut manager = RenderResourceManager::new();
        let old = manager.request_resource(1, texture_request("texture-a", "v1", 16));
        let new = manager.request_resource(2, texture_request("texture-a", "v2", 20));
        let report = manager.end_frame(2);

        assert_ne!(old, new);
        assert!(report
            .events
            .iter()
            .any(|event| event.event_type == RenderResourceEventType::Stale));
        assert_eq!(
            manager.record(new).map(|record| record.state),
            Some(RenderResourceState::Resident)
        );
    }

    #[test]
    fn render_resource_upload_budget_defers_large_batch() {
        let mut manager = RenderResourceManager::new();
        manager.set_upload_budget(RenderUploadBudget::new(10));
        let first = manager.request_resource(1, texture_request("texture-a", "v1", 8));
        let second = manager.request_resource(1, texture_request("texture-b", "v1", 8));
        let report = manager.end_frame(1);

        assert_eq!(
            manager.record(first).map(|record| record.state),
            Some(RenderResourceState::Resident)
        );
        assert_eq!(
            manager.record(second).map(|record| record.state),
            Some(RenderResourceState::Requested)
        );
        assert!(report.events.iter().any(|event| {
            event.event_type == RenderResourceEventType::Deferred
                && event
                    .diagnostic
                    .as_ref()
                    .is_some_and(|diagnostic| diagnostic.code == "UploadBudgetDeferred")
        }));
    }

    #[test]
    fn render_resource_upload_budget_allows_one_large_resource_progress() {
        let mut manager = RenderResourceManager::new();
        manager.set_upload_budget(RenderUploadBudget::new(10));
        let handle = manager.request_resource(1, texture_request("texture-a", "v1", 64));
        let report = manager.end_frame(1);

        assert_eq!(
            manager.record(handle).map(|record| record.state),
            Some(RenderResourceState::Resident)
        );
        assert_eq!(report.uploaded_bytes, 64);
    }

    #[test]
    fn render_resource_device_lost_marks_resources_for_rebuild() {
        let mut manager = RenderResourceManager::new();
        let handle = manager.request_resource(1, texture_request("texture-a", "v1", 16));
        let report = manager.mark_device_lost(2);

        assert_eq!(
            manager.record(handle).map(|record| record.state),
            Some(RenderResourceState::Failed)
        );
        assert_eq!(report.failed_count, 1);
        assert!(report.events.iter().any(|event| {
            event.event_type == RenderResourceEventType::DeviceLost
                && event
                    .diagnostic
                    .as_ref()
                    .is_some_and(|diagnostic| diagnostic.code == "DeviceLost")
        }));
    }

    #[test]
    fn render_resource_surface_lost_only_marks_surface_frame_texture() {
        let mut manager = RenderResourceManager::new();
        let texture = manager.request_resource(1, texture_request("texture-a", "v1", 16));
        let surface = manager.request_resource(
            1,
            RenderResourceRequest {
                key: RenderAssetKey::new(
                    "surface-main",
                    "frame-1",
                    RenderResourceKind::SurfaceFrameTexture,
                ),
                source: RenderResourceSource::SurfaceFrameTextureDescriptor {
                    target_id: "window-main".to_string(),
                    width: 640,
                    height: 360,
                    format: "Bgra8UnormSrgb".to_string(),
                },
                byte_len: 640 * 360 * 4,
                reason: "surface_frame".to_string(),
            },
        );

        let report = manager.mark_surface_lost(2);

        assert_eq!(
            manager.record(texture).map(|record| record.state),
            Some(RenderResourceState::Resident)
        );
        assert_eq!(
            manager.record(surface).map(|record| record.state),
            Some(RenderResourceState::Failed)
        );
        assert_eq!(report.failed_count, 1);
    }

    #[test]
    fn render_resource_pool_end_to_end_headless_scene() {
        let mut manager = RenderResourceManager::with_safe_frame_delay(1);
        let texture_v1 = manager.request_resource(1, texture_request("texture-player", "v1", 16));
        let mesh_v1 = manager.request_resource(
            1,
            RenderResourceRequest {
                key: RenderAssetKey::new("mesh-player", "v1", RenderResourceKind::MeshBuffer),
                source: RenderResourceSource::MeshBufferDescriptor {
                    vertex_count: 3,
                    index_count: 3,
                },
                byte_len: 96,
                reason: "headless scene mesh".to_string(),
            },
        );
        let material_v1 = manager.request_resource(
            1,
            RenderResourceRequest {
                key: RenderAssetKey::new(
                    "material-player",
                    "v1",
                    RenderResourceKind::MaterialParams,
                ),
                source: RenderResourceSource::MaterialParamsDescriptor { param_count: 1 },
                byte_len: 64,
                reason: "headless scene material".to_string(),
            },
        );
        let reused_texture =
            manager.request_resource(1, texture_request("texture-player", "v1", 16));
        let frame_one = manager.end_frame(1);

        assert_eq!(texture_v1, reused_texture);
        assert!(frame_one.created_count >= 3);
        assert!(frame_one.reused_count >= 1);
        assert!(frame_one.uploaded_bytes >= 176);

        manager
            .release_resource(RenderResourceReleaseRequest {
                handle: mesh_v1,
                frame_index: 2,
            })
            .expect("release mesh");
        let release_frame = manager.end_frame(2);
        assert!(release_frame.pending_release_count >= 1);

        manager.end_frame(3);
        assert_eq!(
            manager.validate_handle(mesh_v1),
            Err(RenderResourceError::ReleasedHandle)
        );

        let texture_v2 = manager.request_resource(4, texture_request("texture-player", "v2", 20));
        let hot_update_frame = manager.end_frame(4);

        assert_ne!(texture_v1, texture_v2);
        assert!(hot_update_frame
            .events
            .iter()
            .any(|event| event.event_type == RenderResourceEventType::Stale));
        assert_eq!(
            manager.record(texture_v2).map(|record| record.state),
            Some(RenderResourceState::Resident)
        );
        assert_eq!(
            manager.record(material_v1).map(|record| record.state),
            Some(RenderResourceState::Resident)
        );
    }
}
