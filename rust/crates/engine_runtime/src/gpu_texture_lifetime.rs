use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::runtime_renderer::{RenderTarget, RuntimeRenderTargetKind, ViewportTextureDescriptor};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuTextureId(pub String);

impl From<String> for GpuTextureId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for GpuTextureId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuTextureDescriptor {
    pub texture_id: GpuTextureId,
    pub target_id: String,
    pub target_kind: RuntimeRenderTargetKind,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub color_space: String,
    pub frame_index: u64,
    pub owner: String,
}

impl GpuTextureDescriptor {
    pub fn from_viewport_descriptor(
        descriptor: &ViewportTextureDescriptor,
        target_kind: RuntimeRenderTargetKind,
    ) -> Self {
        Self {
            texture_id: GpuTextureId(descriptor.texture_id.clone()),
            target_id: descriptor.target_id.clone(),
            target_kind,
            width: descriptor.width,
            height: descriptor.height,
            format: descriptor.format.clone(),
            color_space: descriptor.color_space.clone(),
            frame_index: descriptor.frame_index,
            owner: descriptor.producer.clone(),
        }
    }

    pub fn from_surface_target(target: &RenderTarget, frame_index: u64) -> Self {
        Self {
            texture_id: GpuTextureId(format!(
                "{}::surface-frame-{}",
                target.target_id, frame_index
            )),
            target_id: target.target_id.clone(),
            target_kind: target.target_kind,
            width: target.width,
            height: target.height,
            format: target.format.clone(),
            color_space: target.color_space.clone(),
            frame_index,
            owner: "RenderThread".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GpuTextureState {
    Created,
    Configured,
    Acquired,
    Rendered,
    Presented,
    Released,
    Lost,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuTextureLifetimeEvent {
    pub frame_index: u64,
    pub texture_id: GpuTextureId,
    pub target_id: String,
    pub state: GpuTextureState,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuTextureRecord {
    pub descriptor: GpuTextureDescriptor,
    pub state: GpuTextureState,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuTextureLifetimeReport {
    pub schema_version: String,
    pub frame_index: u64,
    pub target_id: String,
    pub target_kind: RuntimeRenderTargetKind,
    pub texture_id: Option<GpuTextureId>,
    pub final_state: Option<GpuTextureState>,
    pub event_count: usize,
    pub events: Vec<GpuTextureLifetimeEvent>,
    pub last_error: Option<String>,
}

impl GpuTextureLifetimeReport {
    pub fn empty(frame_index: u64, target: &RenderTarget) -> Self {
        Self {
            schema_version: "gpu-texture-lifetime-report.v1".to_string(),
            frame_index,
            target_id: target.target_id.clone(),
            target_kind: target.target_kind,
            texture_id: None,
            final_state: None,
            event_count: 0,
            events: Vec::new(),
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeGpuTextureRegistry {
    records: BTreeMap<GpuTextureId, GpuTextureRecord>,
}

impl RuntimeGpuTextureRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_successful_frame(
        &mut self,
        descriptor: GpuTextureDescriptor,
    ) -> GpuTextureLifetimeReport {
        let mut report = GpuTextureLifetimeReport {
            schema_version: "gpu-texture-lifetime-report.v1".to_string(),
            frame_index: descriptor.frame_index,
            target_id: descriptor.target_id.clone(),
            target_kind: descriptor.target_kind,
            texture_id: Some(descriptor.texture_id.clone()),
            final_state: Some(GpuTextureState::Presented),
            event_count: 0,
            events: Vec::new(),
            last_error: None,
        };
        for state in [
            GpuTextureState::Created,
            GpuTextureState::Configured,
            GpuTextureState::Acquired,
            GpuTextureState::Rendered,
            GpuTextureState::Presented,
        ] {
            report.events.push(GpuTextureLifetimeEvent {
                frame_index: descriptor.frame_index,
                texture_id: descriptor.texture_id.clone(),
                target_id: descriptor.target_id.clone(),
                state,
                message: format!("{state:?}"),
            });
        }
        report.event_count = report.events.len();
        self.records.insert(
            descriptor.texture_id.clone(),
            GpuTextureRecord {
                descriptor,
                state: GpuTextureState::Presented,
                last_error: None,
            },
        );
        report
    }

    pub fn record_release(
        &mut self,
        texture_id: &GpuTextureId,
        frame_index: u64,
    ) -> Option<GpuTextureLifetimeEvent> {
        let record = self.records.get_mut(texture_id)?;
        record.state = GpuTextureState::Released;
        Some(GpuTextureLifetimeEvent {
            frame_index,
            texture_id: texture_id.clone(),
            target_id: record.descriptor.target_id.clone(),
            state: GpuTextureState::Released,
            message: "Released".to_string(),
        })
    }

    pub fn record_lost(
        &mut self,
        target: &RenderTarget,
        frame_index: u64,
        error: impl Into<String>,
    ) -> GpuTextureLifetimeReport {
        let error = error.into();
        GpuTextureLifetimeReport {
            schema_version: "gpu-texture-lifetime-report.v1".to_string(),
            frame_index,
            target_id: target.target_id.clone(),
            target_kind: target.target_kind,
            texture_id: None,
            final_state: Some(GpuTextureState::Lost),
            event_count: 1,
            events: vec![GpuTextureLifetimeEvent {
                frame_index,
                texture_id: GpuTextureId(format!("{}::lost-{}", target.target_id, frame_index)),
                target_id: target.target_id.clone(),
                state: GpuTextureState::Lost,
                message: error.clone(),
            }],
            last_error: Some(error),
        }
    }

    pub fn record(&self, texture_id: &GpuTextureId) -> Option<&GpuTextureRecord> {
        self.records.get(texture_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> RenderTarget {
        RenderTarget::viewport_texture("viewport-main", 640, 360)
    }

    #[test]
    fn gpu_texture_lifetime_records_create_acquire_present_release() {
        let target = target();
        let descriptor = GpuTextureDescriptor::from_surface_target(&target, 1);
        let texture_id = descriptor.texture_id.clone();
        let mut registry = RuntimeGpuTextureRegistry::new();

        let report = registry.record_successful_frame(descriptor);
        let release = registry
            .record_release(&texture_id, 2)
            .expect("release event");

        assert_eq!(report.final_state, Some(GpuTextureState::Presented));
        assert!(report
            .events
            .iter()
            .any(|event| event.state == GpuTextureState::Created));
        assert!(report
            .events
            .iter()
            .any(|event| event.state == GpuTextureState::Acquired));
        assert!(report
            .events
            .iter()
            .any(|event| event.state == GpuTextureState::Presented));
        assert_eq!(release.state, GpuTextureState::Released);
        assert_eq!(
            registry.record(&texture_id).map(|record| record.state),
            Some(GpuTextureState::Released)
        );
    }

    #[test]
    fn gpu_texture_lifetime_records_resize_recreate() {
        let mut registry = RuntimeGpuTextureRegistry::new();
        let first = GpuTextureDescriptor::from_surface_target(&target(), 1);
        let resized_target = RenderTarget::viewport_texture("viewport-main", 1280, 720);
        let second = GpuTextureDescriptor::from_surface_target(&resized_target, 2);

        let first_report = registry.record_successful_frame(first);
        let second_report = registry.record_successful_frame(second);

        assert_ne!(first_report.texture_id, second_report.texture_id);
        assert_eq!(second_report.target_id, "viewport-main");
        assert_eq!(second_report.final_state, Some(GpuTextureState::Presented));
    }

    #[test]
    fn gpu_texture_lifetime_reports_surface_lost() {
        let mut registry = RuntimeGpuTextureRegistry::new();

        let report = registry.record_lost(&target(), 7, "surface_lost");

        assert_eq!(report.final_state, Some(GpuTextureState::Lost));
        assert_eq!(report.last_error.as_deref(), Some("surface_lost"));
        assert_eq!(report.event_count, 1);
    }
}
