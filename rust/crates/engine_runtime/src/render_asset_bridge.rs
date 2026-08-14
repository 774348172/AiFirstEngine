//! Historical module name for asset projection into render resources.
//!
//! Current architecture reads this as `AssetProjection`. The module reports
//! `ProjectionReport` evidence and should not be extended as an independent
//! bridge family.

use serde::{Deserialize, Serialize};

use crate::ids::SourceEntityId;
use crate::projection::{ProjectionDiagnostic, ProjectionDomain, ProjectionKind, ProjectionReport};
use crate::render_resource::{
    RenderAssetKey, RenderResourceDiagnostic, RenderResourceHandle, RenderResourceKind,
    RenderResourceManager, RenderResourceRequest, RenderResourceSource,
};
use crate::render_state::RenderProxyId;
use crate::runtime_asset::{RuntimeAssetHandle, RuntimeAssetRecord};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderAssetUsage {
    SpriteTexture,
    SpriteMaterial,
    MeshGeometry,
    AuiTexture,
    FontAtlas,
}

impl RenderAssetUsage {
    pub fn as_profile_usage(self) -> &'static str {
        match self {
            Self::SpriteTexture => "sprite-texture",
            Self::SpriteMaterial => "sprite-material",
            Self::MeshGeometry => "mesh-geometry",
            Self::AuiTexture => "aui-texture",
            Self::FontAtlas => "font-atlas",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderAssetPrepareRequest {
    pub request_id: String,
    pub frame_index: u64,
    pub asset_ref: String,
    pub expected_asset_type: String,
    pub usage: RenderAssetUsage,
    pub source_entity_id: Option<String>,
    pub source_proxy_id: Option<String>,
    pub source_component: Option<String>,
    pub material_ref: Option<String>,
}

impl RenderAssetPrepareRequest {
    pub fn sprite_texture(
        frame_index: u64,
        asset_ref: impl Into<String>,
        source_entity_id: Option<&SourceEntityId>,
        source_proxy_id: Option<RenderProxyId>,
    ) -> Self {
        let asset_ref = asset_ref.into();
        Self {
            request_id: format!("prepare:sprite-texture:{frame_index}:{asset_ref}"),
            frame_index,
            asset_ref,
            expected_asset_type: "texture".to_string(),
            usage: RenderAssetUsage::SpriteTexture,
            source_entity_id: source_entity_id.map(ToString::to_string),
            source_proxy_id: source_proxy_id.map(|id| id.to_string()),
            source_component: Some("SpriteRenderer2D".to_string()),
            material_ref: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PreparedRenderAssetStatus {
    Ready,
    Deferred,
    MissingRuntimeAsset,
    UnsupportedFormat,
    DecodeFailed,
    UploadFailed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedRenderAsset {
    pub asset_ref: String,
    pub asset_id: String,
    pub cooked_asset_id: String,
    pub resource_kind: RenderResourceKind,
    pub resource_handle: Option<RenderResourceHandle>,
    pub status: PreparedRenderAssetStatus,
    pub byte_size: u64,
    pub version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpriteSampler {
    LinearClamp,
    NearestClamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpriteBlendMode {
    Opaque,
    AlphaBlend,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum SpriteMaterialHandle {
    DefaultSpriteBasic,
    RenderResource { handle: RenderResourceHandle },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpriteMaterialBinding {
    pub texture: Option<RenderResourceHandle>,
    pub material: SpriteMaterialHandle,
    pub sampler: SpriteSampler,
    pub blend_mode: SpriteBlendMode,
    pub fallback_used: bool,
}

impl SpriteMaterialBinding {
    pub fn default_sprite_basic(texture: Option<RenderResourceHandle>) -> Self {
        Self {
            texture,
            material: SpriteMaterialHandle::DefaultSpriteBasic,
            sampler: SpriteSampler::LinearClamp,
            blend_mode: SpriteBlendMode::AlphaBlend,
            fallback_used: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderAssetPrepareStage {
    ResolveAssetRef,
    LoadRuntimeAsset,
    DecodeRuntimeAsset,
    CreateRenderResourceRequest,
    UploadGpuResource,
    BindMaterial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderAssetPrepareCode {
    MissingAssetRef,
    MissingRuntimeAssetIndexEntry,
    MissingCookedAsset,
    UnsupportedTextureFormat,
    UnsupportedFormat,
    DecodeFailed,
    UploadBudgetDeferred,
    RenderResourceCreateFailed,
    MissingMaterial,
    FallbackMaterialUsed,
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderAssetPrepareSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderAssetPrepareEvent {
    pub request_id: String,
    pub asset_ref: String,
    pub asset_id: Option<String>,
    pub cooked_asset_id: Option<String>,
    pub stage: RenderAssetPrepareStage,
    pub severity: RenderAssetPrepareSeverity,
    pub code: RenderAssetPrepareCode,
    pub message: String,
    pub source_entity_id: Option<String>,
    pub source_proxy_id: Option<String>,
}

impl RenderAssetPrepareEvent {
    fn ready(
        request: Option<&RenderAssetPrepareRequest>,
        record: &RuntimeAssetRecord,
        stage: RenderAssetPrepareStage,
        message: impl Into<String>,
    ) -> Self {
        Self {
            request_id: request
                .map(|request| request.request_id.clone())
                .unwrap_or_else(|| format!("prepare:{}", record.asset_id)),
            asset_ref: request
                .map(|request| request.asset_ref.clone())
                .unwrap_or_else(|| record.asset_id.clone()),
            asset_id: Some(record.asset_id.clone()),
            cooked_asset_id: Some(record.cooked_asset_id.clone()),
            stage,
            severity: RenderAssetPrepareSeverity::Info,
            code: RenderAssetPrepareCode::Ready,
            message: message.into(),
            source_entity_id: request.and_then(|request| request.source_entity_id.clone()),
            source_proxy_id: request.and_then(|request| request.source_proxy_id.clone()),
        }
    }

    fn failed(
        request: Option<&RenderAssetPrepareRequest>,
        asset_ref: impl Into<String>,
        stage: RenderAssetPrepareStage,
        code: RenderAssetPrepareCode,
        message: impl Into<String>,
    ) -> Self {
        let asset_ref = asset_ref.into();
        Self {
            request_id: request
                .map(|request| request.request_id.clone())
                .unwrap_or_else(|| format!("prepare:{asset_ref}")),
            asset_ref,
            asset_id: None,
            cooked_asset_id: None,
            stage,
            severity: RenderAssetPrepareSeverity::Error,
            code,
            message: message.into(),
            source_entity_id: request.and_then(|request| request.source_entity_id.clone()),
            source_proxy_id: request.and_then(|request| request.source_proxy_id.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderAssetPrepareReport {
    pub frame_index: u64,
    pub request_count: usize,
    pub ready_count: usize,
    pub deferred_count: usize,
    pub failed_count: usize,
    pub uploaded_bytes: u64,
    pub events: Vec<RenderAssetPrepareEvent>,
}

impl RenderAssetPrepareReport {
    pub fn empty(frame_index: u64) -> Self {
        Self {
            frame_index,
            request_count: 0,
            ready_count: 0,
            deferred_count: 0,
            failed_count: 0,
            uploaded_bytes: 0,
            events: Vec::new(),
        }
    }

    pub fn from_events(
        frame_index: u64,
        request_count: usize,
        events: Vec<RenderAssetPrepareEvent>,
    ) -> Self {
        let ready_count = events
            .iter()
            .filter(|event| event.code == RenderAssetPrepareCode::Ready)
            .count();
        let deferred_count = events
            .iter()
            .filter(|event| event.code == RenderAssetPrepareCode::UploadBudgetDeferred)
            .count();
        let failed_count = events
            .iter()
            .filter(|event| event.severity == RenderAssetPrepareSeverity::Error)
            .count();
        Self {
            frame_index,
            request_count,
            ready_count,
            deferred_count,
            failed_count,
            uploaded_bytes: 0,
            events,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderAssetBridgeProfile {
    pub platform_profile: String,
    pub quality_profile: String,
    pub usage: String,
}

impl Default for RenderAssetBridgeProfile {
    fn default() -> Self {
        Self {
            platform_profile: "dev-desktop".to_string(),
            quality_profile: "default".to_string(),
            usage: "default".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderAssetBridgeReport {
    pub ok: bool,
    pub request_count: usize,
    pub diagnostics: Vec<RenderResourceDiagnostic>,
    #[serde(default)]
    pub prepare_report: Option<RenderAssetPrepareReport>,
}

impl RenderAssetBridgeReport {
    pub fn ok(request_count: usize) -> Self {
        Self {
            ok: true,
            request_count,
            diagnostics: Vec::new(),
            prepare_report: None,
        }
    }

    pub fn failed(diagnostic: RenderResourceDiagnostic) -> Self {
        Self {
            ok: false,
            request_count: 0,
            diagnostics: vec![diagnostic],
            prepare_report: None,
        }
    }

    pub fn from_prepare_report(report: RenderAssetPrepareReport) -> Self {
        let ok = report.failed_count == 0;
        let diagnostics = report
            .events
            .iter()
            .filter(|event| event.severity == RenderAssetPrepareSeverity::Error)
            .map(|event| {
                RenderResourceDiagnostic::new(format!("{:?}", event.code), event.message.clone())
            })
            .collect();
        Self {
            ok,
            request_count: report.request_count,
            diagnostics,
            prepare_report: Some(report),
        }
    }

    pub fn projection_summary(&self) -> ProjectionReport {
        let diagnostics = self
            .diagnostics
            .iter()
            .map(|diagnostic| {
                ProjectionDiagnostic::new(
                    "error",
                    diagnostic.code.clone(),
                    diagnostic.message.clone(),
                    Some("AssetProjectionAdapter<RuntimeAsset>".to_string()),
                )
            })
            .collect::<Vec<_>>();
        ProjectionReport::new(
            ProjectionKind::Asset,
            ProjectionDomain::AssetRuntime,
            ProjectionDomain::Render,
            "AssetProjectionAdapter<RuntimeAsset>",
        )
        .with_counts(self.request_count, 0, self.diagnostics.len())
        .with_diagnostics(diagnostics)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderAssetBridgeOutput {
    pub requests: Vec<RenderResourceRequest>,
    pub report: RenderAssetBridgeReport,
    pub prepared_assets: Vec<PreparedRenderAsset>,
}

#[derive(Debug, Clone, Default)]
pub struct RenderAssetBridge {
    profile: RenderAssetBridgeProfile,
}

impl RenderAssetBridge {
    pub fn new(profile: RenderAssetBridgeProfile) -> Self {
        Self { profile }
    }

    pub fn build_request_from_handle(
        &self,
        handle: &RuntimeAssetHandle,
    ) -> RenderAssetBridgeOutput {
        let Some(kind) = kind_from_runtime_type(&handle.asset_type, &handle.loader_kind) else {
            return unsupported_format(&handle.asset_id, &handle.asset_type);
        };
        let byte_len = estimated_byte_len(kind, None);
        let request = RenderResourceRequest {
            key: self.key(
                &handle.asset_id,
                normalized_version(&handle.version, handle.generation),
                kind,
            ),
            source: source_for_kind(kind, byte_len),
            byte_len,
            reason: format!("RuntimeAssetHandle {}", handle.handle_id),
        };
        RenderAssetBridgeOutput {
            requests: vec![request],
            report: RenderAssetBridgeReport::ok(1),
            prepared_assets: vec![prepared_from_handle(handle, kind, None)],
        }
    }

    pub fn build_request_from_record(
        &self,
        record: &RuntimeAssetRecord,
    ) -> RenderAssetBridgeOutput {
        let Some(kind) = kind_from_runtime_type(&record.asset_type, &record.loader_kind) else {
            return unsupported_format(&record.asset_id, &record.asset_type);
        };
        let byte_len = estimated_byte_len(kind, record.size);
        let request = RenderResourceRequest {
            key: self.key(
                &record.asset_id,
                normalized_record_version(&record.version),
                kind,
            ),
            source: source_for_kind(kind, byte_len),
            byte_len,
            reason: format!("RuntimeAssetRecord {}", record.asset_guid),
        };
        RenderAssetBridgeOutput {
            requests: vec![request],
            report: RenderAssetBridgeReport::ok(1),
            prepared_assets: vec![prepared_from_record(record, kind, None)],
        }
    }

    pub fn prepare_from_record(
        &self,
        frame_index: u64,
        request: &RenderAssetPrepareRequest,
        record: &RuntimeAssetRecord,
    ) -> RenderAssetBridgeOutput {
        if request.asset_ref.is_empty() {
            return prepare_failed(
                frame_index,
                Some(request),
                "",
                RenderAssetPrepareStage::ResolveAssetRef,
                RenderAssetPrepareCode::MissingAssetRef,
                "Sprite2D render asset prepare requires a non-empty asset_ref.",
            );
        }
        let Some(kind) = kind_from_runtime_type(&record.asset_type, &record.loader_kind) else {
            return prepare_failed(
                frame_index,
                Some(request),
                &request.asset_ref,
                RenderAssetPrepareStage::CreateRenderResourceRequest,
                RenderAssetPrepareCode::UnsupportedFormat,
                format!(
                    "Runtime asset '{}' with type '{}' cannot produce a render resource request.",
                    record.asset_id, record.asset_type
                ),
            );
        };
        let byte_len = estimated_byte_len(kind, record.size);
        let render_request = RenderResourceRequest {
            key: self.key_with_usage(
                &record.asset_id,
                normalized_record_version(&record.version),
                kind,
                request.usage,
            ),
            source: source_for_kind(kind, byte_len),
            byte_len,
            reason: format!("RenderAssetPrepare {}", request.request_id),
        };
        let event = RenderAssetPrepareEvent::ready(
            Some(request),
            record,
            RenderAssetPrepareStage::CreateRenderResourceRequest,
            "Render asset prepare created a render resource request.",
        );
        let report = RenderAssetPrepareReport::from_events(frame_index, 1, vec![event]);
        RenderAssetBridgeOutput {
            requests: vec![render_request],
            report: RenderAssetBridgeReport::from_prepare_report(report),
            prepared_assets: vec![prepared_from_record(record, kind, None)],
        }
    }

    pub fn prepare_record_with_resource_manager(
        &self,
        frame_index: u64,
        request: &RenderAssetPrepareRequest,
        record: &RuntimeAssetRecord,
        manager: &mut RenderResourceManager,
    ) -> RenderAssetBridgeOutput {
        let mut output = self.prepare_from_record(frame_index, request, record);
        let mut events = output
            .report
            .prepare_report
            .as_ref()
            .map(|report| report.events.clone())
            .unwrap_or_default();
        for (index, render_request) in output.requests.clone().into_iter().enumerate() {
            let handle = manager.request_resource(frame_index, render_request);
            if let Some(prepared) = output.prepared_assets.get_mut(index) {
                prepared.resource_handle = Some(handle);
                prepared.status = PreparedRenderAssetStatus::Ready;
            }
            events.push(RenderAssetPrepareEvent::ready(
                Some(request),
                record,
                RenderAssetPrepareStage::UploadGpuResource,
                "Render resource manager returned a resident resource handle.",
            ));
        }
        let mut report = RenderAssetPrepareReport::from_events(frame_index, 1, events);
        report.uploaded_bytes = output
            .prepared_assets
            .iter()
            .map(|asset| asset.byte_size)
            .sum();
        output.report = RenderAssetBridgeReport::from_prepare_report(report);
        output
    }

    fn key(
        &self,
        asset_id: &str,
        asset_version: impl Into<String>,
        resource_kind: RenderResourceKind,
    ) -> RenderAssetKey {
        RenderAssetKey {
            asset_id: asset_id.to_string(),
            asset_version: asset_version.into(),
            resource_kind,
            platform_profile: self.profile.platform_profile.clone(),
            quality_profile: self.profile.quality_profile.clone(),
            usage: self.profile.usage.clone(),
        }
    }

    fn key_with_usage(
        &self,
        asset_id: &str,
        asset_version: impl Into<String>,
        resource_kind: RenderResourceKind,
        usage: RenderAssetUsage,
    ) -> RenderAssetKey {
        RenderAssetKey {
            asset_id: asset_id.to_string(),
            asset_version: asset_version.into(),
            resource_kind,
            platform_profile: self.profile.platform_profile.clone(),
            quality_profile: self.profile.quality_profile.clone(),
            usage: usage.as_profile_usage().to_string(),
        }
    }
}

fn unsupported_format(asset_id: &str, asset_type: &str) -> RenderAssetBridgeOutput {
    RenderAssetBridgeOutput {
        requests: Vec::new(),
        report: RenderAssetBridgeReport::failed(RenderResourceDiagnostic::new(
            "UnsupportedFormat",
            format!("Runtime asset '{asset_id}' with type '{asset_type}' cannot produce a render resource request."),
        )),
        prepared_assets: Vec::new(),
    }
}

fn prepare_failed(
    frame_index: u64,
    request: Option<&RenderAssetPrepareRequest>,
    asset_ref: impl Into<String>,
    stage: RenderAssetPrepareStage,
    code: RenderAssetPrepareCode,
    message: impl Into<String>,
) -> RenderAssetBridgeOutput {
    let event = RenderAssetPrepareEvent::failed(request, asset_ref, stage, code, message);
    let report = RenderAssetPrepareReport::from_events(frame_index, 1, vec![event]);
    RenderAssetBridgeOutput {
        requests: Vec::new(),
        report: RenderAssetBridgeReport::from_prepare_report(report),
        prepared_assets: Vec::new(),
    }
}

fn prepared_from_handle(
    handle: &RuntimeAssetHandle,
    kind: RenderResourceKind,
    resource_handle: Option<RenderResourceHandle>,
) -> PreparedRenderAsset {
    PreparedRenderAsset {
        asset_ref: handle.asset_id.clone(),
        asset_id: handle.asset_id.clone(),
        cooked_asset_id: handle.cooked_asset_id.clone(),
        resource_kind: kind,
        resource_handle,
        status: if resource_handle.is_some() {
            PreparedRenderAssetStatus::Ready
        } else {
            PreparedRenderAssetStatus::Deferred
        },
        byte_size: estimated_byte_len(kind, None) as u64,
        version: normalized_version(&handle.version, handle.generation),
    }
}

fn prepared_from_record(
    record: &RuntimeAssetRecord,
    kind: RenderResourceKind,
    resource_handle: Option<RenderResourceHandle>,
) -> PreparedRenderAsset {
    PreparedRenderAsset {
        asset_ref: record.asset_id.clone(),
        asset_id: record.asset_id.clone(),
        cooked_asset_id: record.cooked_asset_id.clone(),
        resource_kind: kind,
        resource_handle,
        status: if resource_handle.is_some() {
            PreparedRenderAssetStatus::Ready
        } else {
            PreparedRenderAssetStatus::Deferred
        },
        byte_size: estimated_byte_len(kind, record.size) as u64,
        version: normalized_record_version(&record.version),
    }
}

fn kind_from_runtime_type(asset_type: &str, loader_kind: &str) -> Option<RenderResourceKind> {
    match (asset_type, loader_kind) {
        ("texture", _) | (_, "texture") => Some(RenderResourceKind::Texture),
        ("mesh", _) | ("model", _) | (_, "mesh") | (_, "model") => {
            Some(RenderResourceKind::MeshBuffer)
        }
        ("material", _) | (_, "material") => Some(RenderResourceKind::MaterialParams),
        ("shader", _) | ("pipeline", _) | (_, "shader") | (_, "pipeline") => {
            Some(RenderResourceKind::ShaderPipeline)
        }
        _ => None,
    }
}

fn source_for_kind(kind: RenderResourceKind, byte_len: usize) -> RenderResourceSource {
    match kind {
        RenderResourceKind::Texture => RenderResourceSource::TextureDescriptor {
            width: 1,
            height: (byte_len.max(4) / 4) as u32,
            format: "Rgba8Unorm".to_string(),
        },
        RenderResourceKind::MeshBuffer => RenderResourceSource::MeshBufferDescriptor {
            vertex_count: (byte_len / 32).max(1) as u32,
            index_count: (byte_len / 4).max(1) as u32,
        },
        RenderResourceKind::MaterialParams => {
            RenderResourceSource::MaterialParamsDescriptor { param_count: 1 }
        }
        RenderResourceKind::ShaderPipeline => RenderResourceSource::ShaderPipelineDescriptor {
            shader_id: "shader-pipeline-placeholder".to_string(),
        },
        RenderResourceKind::SurfaceFrameTexture => {
            RenderResourceSource::SurfaceFrameTextureDescriptor {
                target_id: "surface-placeholder".to_string(),
                width: 1,
                height: 1,
                format: "Bgra8UnormSrgb".to_string(),
            }
        }
    }
}

fn estimated_byte_len(kind: RenderResourceKind, record_size: Option<u64>) -> usize {
    record_size.map(|size| size as usize).unwrap_or(match kind {
        RenderResourceKind::Texture => 4,
        RenderResourceKind::MeshBuffer => 36,
        RenderResourceKind::MaterialParams => 64,
        RenderResourceKind::ShaderPipeline => 128,
        RenderResourceKind::SurfaceFrameTexture => 4,
    })
}

fn normalized_version(version: &str, generation: u64) -> String {
    if version.is_empty() {
        return format!("generation-{generation}");
    }
    version.to_string()
}

fn normalized_record_version(version: &str) -> String {
    if version.is_empty() {
        return "unversioned".to_string();
    }
    version.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_asset::{RuntimeAssetLoadState, RuntimeAssetRecord};

    fn texture_handle() -> RuntimeAssetHandle {
        RuntimeAssetHandle {
            handle_id: 1,
            asset_guid: "guid-texture-a".to_string(),
            asset_id: "texture-a".to_string(),
            asset_type: "texture".to_string(),
            sub_asset_id: None,
            cooked_asset_id: "cooked-texture-a".to_string(),
            bundle_id: "bundle-main".to_string(),
            runtime_resource_id: Some("runtime-texture-a".to_string()),
            state: RuntimeAssetLoadState::Ready,
            generation: 7,
            ref_count: 1,
            loader_kind: "texture".to_string(),
            version: "v1".to_string(),
        }
    }

    fn record(asset_type: &str, loader_kind: &str) -> RuntimeAssetRecord {
        RuntimeAssetRecord {
            asset_guid: format!("guid-{asset_type}"),
            asset_id: format!("{asset_type}-a"),
            asset_type: asset_type.to_string(),
            sub_asset_id: None,
            version: "v1".to_string(),
            cooked_asset_id: format!("cooked-{asset_type}"),
            bundle_id: "bundle-main".to_string(),
            loader_kind: loader_kind.to_string(),
            dependencies: Vec::new(),
            hash: None,
            size: Some(128),
            flags: Vec::new(),
            source_map_debug: None,
        }
    }

    #[test]
    fn render_asset_bridge_builds_texture_request_from_runtime_asset() {
        let bridge = RenderAssetBridge::default();
        let output = bridge.build_request_from_handle(&texture_handle());

        assert!(output.report.ok);
        assert_eq!(output.requests.len(), 1);
        let request = &output.requests[0];
        assert_eq!(request.key.asset_id, "texture-a");
        assert_eq!(request.key.asset_version, "v1");
        assert_eq!(request.key.resource_kind, RenderResourceKind::Texture);
        assert!(matches!(
            request.source,
            RenderResourceSource::TextureDescriptor { .. }
        ));
    }

    #[test]
    fn render_asset_bridge_report_exposes_projection_summary() {
        let bridge = RenderAssetBridge::default();
        let output = bridge.build_request_from_handle(&texture_handle());

        let projection = output.report.projection_summary();

        assert_eq!(projection.kind, ProjectionKind::Asset);
        assert_eq!(projection.source_domain, ProjectionDomain::AssetRuntime);
        assert_eq!(projection.target_domain, ProjectionDomain::Render);
        assert_eq!(projection.projected_count, 1);
    }

    #[test]
    fn render_asset_bridge_builds_mesh_and_material_requests_from_records() {
        let bridge = RenderAssetBridge::default();
        let mesh = bridge.build_request_from_record(&record("mesh", "mesh"));
        let material = bridge.build_request_from_record(&record("material", "material"));

        assert_eq!(
            mesh.requests[0].key.resource_kind,
            RenderResourceKind::MeshBuffer
        );
        assert_eq!(
            material.requests[0].key.resource_kind,
            RenderResourceKind::MaterialParams
        );
    }

    #[test]
    fn render_asset_bridge_reports_unsupported_format() {
        let bridge = RenderAssetBridge::default();
        let output = bridge.build_request_from_record(&record("audio", "audio"));

        assert!(!output.report.ok);
        assert!(output.requests.is_empty());
        assert_eq!(output.report.diagnostics[0].code, "UnsupportedFormat");
    }

    #[test]
    fn render_asset_prepare_request_carries_sprite_source_context() {
        let request = RenderAssetPrepareRequest::sprite_texture(
            12,
            "texture-player",
            Some(&crate::ids::SourceEntityId::from("entity-player")),
            Some(crate::render_state::RenderProxyId(7)),
        );

        assert_eq!(request.frame_index, 12);
        assert_eq!(request.asset_ref, "texture-player");
        assert_eq!(request.expected_asset_type, "texture");
        assert_eq!(request.usage, RenderAssetUsage::SpriteTexture);
        assert_eq!(
            request.source_component.as_deref(),
            Some("SpriteRenderer2D")
        );
        assert_eq!(request.source_entity_id.as_deref(), Some("entity-player"));
        assert_eq!(request.source_proxy_id.as_deref(), Some("proxy-7"));
    }

    #[test]
    fn render_asset_prepare_builds_texture_request_and_report() {
        let bridge = RenderAssetBridge::default();
        let request = RenderAssetPrepareRequest::sprite_texture(
            3,
            "texture-a",
            Some(&crate::ids::SourceEntityId::from("entity-a")),
            Some(crate::render_state::RenderProxyId(11)),
        );

        let output = bridge.prepare_from_record(3, &request, &record("texture", "texture"));

        assert!(output.report.ok);
        assert_eq!(output.requests.len(), 1);
        assert_eq!(output.prepared_assets.len(), 1);
        assert_eq!(
            output.requests[0].key.resource_kind,
            RenderResourceKind::Texture
        );
        assert_eq!(output.requests[0].key.usage, "sprite-texture");
        let prepare_report = output.report.prepare_report.expect("prepare report");
        assert_eq!(prepare_report.frame_index, 3);
        assert_eq!(prepare_report.ready_count, 1);
        assert_eq!(prepare_report.failed_count, 0);
        assert_eq!(prepare_report.events[0].asset_ref, "texture-a");
        assert_eq!(
            prepare_report.events[0].source_entity_id.as_deref(),
            Some("entity-a")
        );
        assert_eq!(
            prepare_report.events[0].source_proxy_id.as_deref(),
            Some("proxy-11")
        );
    }

    #[test]
    fn render_asset_prepare_reports_missing_asset_ref() {
        let bridge = RenderAssetBridge::default();
        let mut request = RenderAssetPrepareRequest::sprite_texture(1, "", None, None);
        request.request_id = "prepare-empty".to_string();

        let output = bridge.prepare_from_record(1, &request, &record("texture", "texture"));

        assert!(!output.report.ok);
        assert!(output.requests.is_empty());
        let prepare_report = output.report.prepare_report.expect("prepare report");
        assert_eq!(prepare_report.failed_count, 1);
        assert_eq!(
            prepare_report.events[0].code,
            RenderAssetPrepareCode::MissingAssetRef
        );
        assert_eq!(
            prepare_report.events[0].stage,
            RenderAssetPrepareStage::ResolveAssetRef
        );
    }

    #[test]
    fn render_asset_prepare_reports_unsupported_format() {
        let bridge = RenderAssetBridge::default();
        let request = RenderAssetPrepareRequest::sprite_texture(1, "audio-a", None, None);

        let output = bridge.prepare_from_record(1, &request, &record("audio", "audio"));

        assert!(!output.report.ok);
        assert!(output.requests.is_empty());
        let prepare_report = output.report.prepare_report.expect("prepare report");
        assert_eq!(prepare_report.failed_count, 1);
        assert_eq!(
            prepare_report.events[0].code,
            RenderAssetPrepareCode::UnsupportedFormat
        );
    }

    #[test]
    fn render_asset_prepare_with_resource_manager_returns_handle() {
        let bridge = RenderAssetBridge::default();
        let request = RenderAssetPrepareRequest::sprite_texture(5, "texture-a", None, None);
        let mut manager = RenderResourceManager::new();

        let output = bridge.prepare_record_with_resource_manager(
            5,
            &request,
            &record("texture", "texture"),
            &mut manager,
        );

        let prepared = output.prepared_assets.first().expect("prepared asset");
        let handle = prepared.resource_handle.expect("resource handle");
        assert_eq!(prepared.status, PreparedRenderAssetStatus::Ready);
        assert_eq!(
            manager.record(handle).map(|record| record.state),
            Some(crate::render_resource::RenderResourceState::Resident)
        );
        let prepare_report = output.report.prepare_report.expect("prepare report");
        assert_eq!(prepare_report.ready_count, 2);
        assert!(prepare_report.uploaded_bytes > 0);
    }

    #[test]
    fn sprite_material_binding_defaults_to_sprite_basic_alpha_blend() {
        let binding = SpriteMaterialBinding::default_sprite_basic(None);

        assert_eq!(binding.texture, None);
        assert_eq!(binding.material, SpriteMaterialHandle::DefaultSpriteBasic);
        assert_eq!(binding.sampler, SpriteSampler::LinearClamp);
        assert_eq!(binding.blend_mode, SpriteBlendMode::AlphaBlend);
        assert!(binding.fallback_used);
    }
}
