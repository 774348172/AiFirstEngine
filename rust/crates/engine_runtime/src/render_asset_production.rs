use serde::{Deserialize, Serialize};

use crate::render_resource::{
    RenderAssetKey, RenderResourceHandle, RenderResourceKind, RenderResourceManager,
    RenderResourceRequest, RenderResourceSource,
};
use crate::runtime_asset::RuntimeAssetRecord;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeRenderAssetKind {
    Texture,
    Material,
    Mesh,
    AuiImage,
    FontAtlas,
    ParticleTexture,
    RenderTarget,
}

impl RuntimeRenderAssetKind {
    pub fn producer_name(self) -> &'static str {
        match self {
            Self::Texture => "TextureProducer",
            Self::Material => "MaterialProducer",
            Self::Mesh => "MeshProducer",
            Self::AuiImage => "TextureProducer",
            Self::FontAtlas => "FontAtlasProducer",
            Self::ParticleTexture => "TextureProducer",
            Self::RenderTarget => "RenderTargetProducer",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeRenderAssetUsage {
    Sprite2DTexture,
    MeshAlbedoTexture,
    AuiImageTexture,
    FontAtlasTexture,
    ParticleTexture,
    MaterialBinding,
    MeshGeometry,
    SurfaceRenderTarget,
    OffscreenRenderTarget,
}

impl RuntimeRenderAssetUsage {
    pub fn as_key_usage(self) -> &'static str {
        match self {
            Self::Sprite2DTexture => "sprite-2d-texture",
            Self::MeshAlbedoTexture => "mesh-albedo-texture",
            Self::AuiImageTexture => "aui-image-texture",
            Self::FontAtlasTexture => "font-atlas-texture",
            Self::ParticleTexture => "particle-texture",
            Self::MaterialBinding => "material-binding",
            Self::MeshGeometry => "mesh-geometry",
            Self::SurfaceRenderTarget => "surface-render-target",
            Self::OffscreenRenderTarget => "offscreen-render-target",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRenderAssetId(pub String);

impl RuntimeRenderAssetId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRenderAssetRequest {
    pub request_id: String,
    pub frame_index: u64,
    pub kind: RuntimeRenderAssetKind,
    pub usage: RuntimeRenderAssetUsage,
    pub asset_ref: Option<String>,
    pub source_entity_id: Option<String>,
    pub source_component: Option<String>,
    pub descriptor: Option<RuntimeRenderAssetDescriptor>,
}

impl RuntimeRenderAssetRequest {
    pub fn from_asset(
        frame_index: u64,
        kind: RuntimeRenderAssetKind,
        usage: RuntimeRenderAssetUsage,
        asset_ref: impl Into<String>,
    ) -> Self {
        let asset_ref = asset_ref.into();
        Self {
            request_id: format!(
                "render-asset:{frame_index}:{}:{asset_ref}",
                usage.as_key_usage()
            ),
            frame_index,
            kind,
            usage,
            asset_ref: Some(asset_ref),
            source_entity_id: None,
            source_component: None,
            descriptor: None,
        }
    }

    pub fn render_target(
        frame_index: u64,
        target_id: impl Into<String>,
        width: u32,
        height: u32,
        format: impl Into<String>,
        usage: RuntimeRenderAssetUsage,
    ) -> Self {
        let target_id = target_id.into();
        Self {
            request_id: format!(
                "render-asset:{frame_index}:{}:{target_id}",
                usage.as_key_usage()
            ),
            frame_index,
            kind: RuntimeRenderAssetKind::RenderTarget,
            usage,
            asset_ref: Some(target_id.clone()),
            source_entity_id: None,
            source_component: None,
            descriptor: Some(RuntimeRenderAssetDescriptor::RenderTarget(
                RenderTargetDescriptor {
                    target_id,
                    width,
                    height,
                    format: format.into(),
                    clear_color: [0, 0, 0, 255],
                    sample_count: 1,
                    sampled: matches!(usage, RuntimeRenderAssetUsage::OffscreenRenderTarget),
                    present: matches!(usage, RuntimeRenderAssetUsage::SurfaceRenderTarget),
                },
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "descriptorKind")]
pub enum RuntimeRenderAssetDescriptor {
    Texture(TextureDescriptor),
    Material(MaterialDescriptor),
    Mesh(MeshDescriptor),
    FontAtlas(FontAtlasDescriptor),
    RenderTarget(RenderTargetDescriptor),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextureDescriptor {
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub color_space: String,
    pub byte_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialDescriptor {
    pub material_model: String,
    pub shader_key: String,
    pub scalar_param_count: u32,
    pub vector_param_count: u32,
    pub texture_slot_count: u32,
    pub blend_mode: String,
    pub cull_mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeshDescriptor {
    pub vertex_layout: String,
    pub vertex_count: u32,
    pub index_count: u32,
    pub index_format: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FontAtlasDescriptor {
    pub font_key: String,
    pub atlas_generation: u64,
    pub width: u32,
    pub height: u32,
    pub glyph_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderTargetDescriptor {
    pub target_id: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub clear_color: [u8; 4],
    pub sample_count: u32,
    pub sampled: bool,
    pub present: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeRenderAssetStatus {
    Ready,
    Deferred,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRenderAssetHandle {
    pub asset_id: RuntimeRenderAssetId,
    pub resource_handle: Option<RenderResourceHandle>,
    pub binding: Option<RenderBindingSet>,
    pub status: RuntimeRenderAssetStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRenderAsset {
    pub asset_id: RuntimeRenderAssetId,
    pub kind: RuntimeRenderAssetKind,
    pub usage: RuntimeRenderAssetUsage,
    pub payload: RuntimeRenderAssetPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "payloadKind")]
pub enum RuntimeRenderAssetPayload {
    Texture(TextureRenderAsset),
    Material(MaterialRenderAsset),
    Mesh(MeshRenderAsset),
    AuiImage(AuiImageRenderAsset),
    FontAtlas(FontAtlasRenderAsset),
    ParticleTexture(ParticleTextureRenderAsset),
    RenderTarget(RenderTargetRenderAsset),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextureRenderAsset {
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub color_space: String,
    pub mip_count: u32,
    pub byte_len: usize,
    pub sampler: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialRenderAsset {
    pub material_model: String,
    pub shader_key: String,
    pub scalar_param_count: u32,
    pub vector_param_count: u32,
    pub texture_slot_count: u32,
    pub blend_mode: String,
    pub cull_mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeshRenderAsset {
    pub vertex_layout: String,
    pub vertex_count: u32,
    pub index_count: u32,
    pub index_format: String,
    pub bounds: [i32; 6],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiImageRenderAsset {
    pub texture_asset_ref: String,
    pub nine_slice: Option<[u32; 4]>,
    pub tint: [u8; 4],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FontAtlasRenderAsset {
    pub atlas_texture: TextureRenderAsset,
    pub font_key: String,
    pub atlas_generation: u64,
    pub glyph_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticleTextureRenderAsset {
    pub texture_asset_ref: String,
    pub sampler: String,
    pub flipbook_columns: u32,
    pub flipbook_rows: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderTargetRenderAsset {
    pub target_id: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub clear_color: [u8; 4],
    pub sample_count: u32,
    pub sampled: bool,
    pub present: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderBindingSet {
    pub binding_id: String,
    pub binding_kind: RenderBindingKind,
    pub resources: Vec<RenderResourceHandle>,
    pub material_handle: Option<RenderResourceHandle>,
    pub sampler: String,
    pub fallback_used: bool,
    pub debug_label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderBindingKind {
    Texture,
    Material,
    Mesh,
    FontAtlas,
    RenderTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeRenderAssetProductionStage {
    ResolveSource,
    LoadRuntimeAsset,
    Decode,
    ProduceTypedAsset,
    CreateResourceRequest,
    CreateOrReuseResource,
    CreateBinding,
    Ready,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeRenderAssetProductionCode {
    MissingAssetRef,
    MissingRuntimeAsset,
    UnsupportedFormat,
    DecodeFailed,
    InvalidDescriptor,
    UploadFailed,
    BindingFailed,
    FallbackUsed,
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeRenderAssetProductionSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRenderAssetProductionEvent {
    pub request_id: String,
    pub kind: RuntimeRenderAssetKind,
    pub usage: RuntimeRenderAssetUsage,
    pub asset_ref: Option<String>,
    pub asset_id: Option<String>,
    pub source_entity_id: Option<String>,
    pub source_component: Option<String>,
    pub producer: String,
    pub stage: RuntimeRenderAssetProductionStage,
    pub code: RuntimeRenderAssetProductionCode,
    pub severity: RuntimeRenderAssetProductionSeverity,
    pub resource_handle: Option<RenderResourceHandle>,
    pub fallback_used: bool,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRenderAssetProductionReport {
    pub frame_index: u64,
    pub request_count: usize,
    pub produced_count: usize,
    pub reused_count: usize,
    pub failed_count: usize,
    pub uploaded_bytes: usize,
    pub fallback_count: usize,
    pub events: Vec<RuntimeRenderAssetProductionEvent>,
}

impl RuntimeRenderAssetProductionReport {
    pub fn from_events(
        frame_index: u64,
        request_count: usize,
        events: Vec<RuntimeRenderAssetProductionEvent>,
    ) -> Self {
        let produced_count = events
            .iter()
            .filter(|event| event.code == RuntimeRenderAssetProductionCode::Ready)
            .count();
        let failed_count = events
            .iter()
            .filter(|event| event.severity == RuntimeRenderAssetProductionSeverity::Error)
            .count();
        let fallback_count = events.iter().filter(|event| event.fallback_used).count();
        Self {
            frame_index,
            request_count,
            produced_count,
            reused_count: 0,
            failed_count,
            uploaded_bytes: 0,
            fallback_count,
            events,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeRenderAssetProductionOutput {
    pub typed_asset: Option<RuntimeRenderAsset>,
    pub resource_request: Option<RenderResourceRequest>,
    pub handle: RuntimeRenderAssetHandle,
    pub report: RuntimeRenderAssetProductionReport,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeRenderAssetProducer;

impl RuntimeRenderAssetProducer {
    pub fn new() -> Self {
        Self
    }

    pub fn produce_from_record(
        &self,
        request: &RuntimeRenderAssetRequest,
        record: &RuntimeAssetRecord,
        manager: &mut RenderResourceManager,
    ) -> RuntimeRenderAssetProductionOutput {
        if request.asset_ref.as_deref().unwrap_or_default().is_empty() {
            return self.failed(
                request,
                RuntimeRenderAssetProductionStage::ResolveSource,
                RuntimeRenderAssetProductionCode::MissingAssetRef,
                "Runtime render asset production requires a non-empty asset_ref.",
            );
        }

        let Some(typed_asset) = self.build_typed_asset(request, Some(record)) else {
            return self.failed(
                request,
                RuntimeRenderAssetProductionStage::ProduceTypedAsset,
                RuntimeRenderAssetProductionCode::UnsupportedFormat,
                format!(
                    "Runtime asset '{}' with type '{}' cannot produce {:?}.",
                    record.asset_id, record.asset_type, request.kind
                ),
            );
        };
        self.finalize_typed_asset(request, typed_asset, manager)
    }

    pub fn produce_from_descriptor(
        &self,
        request: &RuntimeRenderAssetRequest,
        manager: &mut RenderResourceManager,
    ) -> RuntimeRenderAssetProductionOutput {
        let Some(typed_asset) = self.build_typed_asset(request, None) else {
            return self.failed(
                request,
                RuntimeRenderAssetProductionStage::ProduceTypedAsset,
                RuntimeRenderAssetProductionCode::InvalidDescriptor,
                "Runtime render asset descriptor is missing or invalid for this kind.",
            );
        };
        self.finalize_typed_asset(request, typed_asset, manager)
    }

    fn finalize_typed_asset(
        &self,
        request: &RuntimeRenderAssetRequest,
        typed_asset: RuntimeRenderAsset,
        manager: &mut RenderResourceManager,
    ) -> RuntimeRenderAssetProductionOutput {
        let Some(resource_request) = resource_request_for_asset(&typed_asset) else {
            return self.failed(
                request,
                RuntimeRenderAssetProductionStage::CreateResourceRequest,
                RuntimeRenderAssetProductionCode::InvalidDescriptor,
                "Runtime render asset could not create a render resource request.",
            );
        };
        let resource_handle =
            manager.request_resource(request.frame_index, resource_request.clone());
        let binding = binding_for_asset(&typed_asset, resource_handle);
        let events = vec![
            event(
                request,
                RuntimeRenderAssetProductionStage::ProduceTypedAsset,
                RuntimeRenderAssetProductionCode::Ready,
                RuntimeRenderAssetProductionSeverity::Info,
                None,
                false,
                "Produced backend-neutral typed render asset.",
            ),
            event(
                request,
                RuntimeRenderAssetProductionStage::CreateOrReuseResource,
                RuntimeRenderAssetProductionCode::Ready,
                RuntimeRenderAssetProductionSeverity::Info,
                Some(resource_handle),
                false,
                "RenderResourceManager returned a resource handle.",
            ),
            event(
                request,
                RuntimeRenderAssetProductionStage::CreateBinding,
                RuntimeRenderAssetProductionCode::Ready,
                RuntimeRenderAssetProductionSeverity::Info,
                Some(resource_handle),
                binding
                    .as_ref()
                    .is_some_and(|binding| binding.fallback_used),
                "Created renderer-facing binding set.",
            ),
        ];
        let mut report =
            RuntimeRenderAssetProductionReport::from_events(request.frame_index, 1, events);
        report.uploaded_bytes = resource_request.byte_len;
        RuntimeRenderAssetProductionOutput {
            typed_asset: Some(typed_asset),
            resource_request: Some(resource_request),
            handle: RuntimeRenderAssetHandle {
                asset_id: RuntimeRenderAssetId::new(asset_id_for_request(request)),
                resource_handle: Some(resource_handle),
                binding,
                status: RuntimeRenderAssetStatus::Ready,
            },
            report,
        }
    }

    fn failed(
        &self,
        request: &RuntimeRenderAssetRequest,
        stage: RuntimeRenderAssetProductionStage,
        code: RuntimeRenderAssetProductionCode,
        message: impl Into<String>,
    ) -> RuntimeRenderAssetProductionOutput {
        let events = vec![event(
            request,
            stage,
            code,
            RuntimeRenderAssetProductionSeverity::Error,
            None,
            false,
            message,
        )];
        RuntimeRenderAssetProductionOutput {
            typed_asset: None,
            resource_request: None,
            handle: RuntimeRenderAssetHandle {
                asset_id: RuntimeRenderAssetId::new(asset_id_for_request(request)),
                resource_handle: None,
                binding: None,
                status: RuntimeRenderAssetStatus::Failed,
            },
            report: RuntimeRenderAssetProductionReport::from_events(request.frame_index, 1, events),
        }
    }

    fn build_typed_asset(
        &self,
        request: &RuntimeRenderAssetRequest,
        record: Option<&RuntimeAssetRecord>,
    ) -> Option<RuntimeRenderAsset> {
        let asset_id = RuntimeRenderAssetId::new(asset_id_for_request(request));
        let payload = match request.kind {
            RuntimeRenderAssetKind::Texture => {
                let descriptor = texture_descriptor(request, record)?;
                RuntimeRenderAssetPayload::Texture(TextureRenderAsset {
                    width: descriptor.width,
                    height: descriptor.height,
                    format: descriptor.format,
                    color_space: descriptor.color_space,
                    mip_count: 1,
                    byte_len: descriptor.byte_len,
                    sampler: "linearClamp".to_string(),
                })
            }
            RuntimeRenderAssetKind::AuiImage => {
                let texture_ref = request.asset_ref.clone()?;
                RuntimeRenderAssetPayload::AuiImage(AuiImageRenderAsset {
                    texture_asset_ref: texture_ref,
                    nine_slice: None,
                    tint: [255, 255, 255, 255],
                })
            }
            RuntimeRenderAssetKind::ParticleTexture => {
                let texture_ref = request.asset_ref.clone()?;
                RuntimeRenderAssetPayload::ParticleTexture(ParticleTextureRenderAsset {
                    texture_asset_ref: texture_ref,
                    sampler: "linearClamp".to_string(),
                    flipbook_columns: 1,
                    flipbook_rows: 1,
                })
            }
            RuntimeRenderAssetKind::Material => {
                let descriptor = material_descriptor(request)?;
                RuntimeRenderAssetPayload::Material(MaterialRenderAsset {
                    material_model: descriptor.material_model,
                    shader_key: descriptor.shader_key,
                    scalar_param_count: descriptor.scalar_param_count,
                    vector_param_count: descriptor.vector_param_count,
                    texture_slot_count: descriptor.texture_slot_count,
                    blend_mode: descriptor.blend_mode,
                    cull_mode: descriptor.cull_mode,
                })
            }
            RuntimeRenderAssetKind::Mesh => {
                let descriptor = mesh_descriptor(request, record)?;
                RuntimeRenderAssetPayload::Mesh(MeshRenderAsset {
                    vertex_layout: descriptor.vertex_layout,
                    vertex_count: descriptor.vertex_count,
                    index_count: descriptor.index_count,
                    index_format: descriptor.index_format,
                    bounds: [0, 0, 0, 1, 1, 1],
                })
            }
            RuntimeRenderAssetKind::FontAtlas => {
                let descriptor = font_atlas_descriptor(request)?;
                RuntimeRenderAssetPayload::FontAtlas(FontAtlasRenderAsset {
                    atlas_texture: TextureRenderAsset {
                        width: descriptor.width,
                        height: descriptor.height,
                        format: "Rgba8Unorm".to_string(),
                        color_space: "srgb".to_string(),
                        mip_count: 1,
                        byte_len: descriptor.width as usize * descriptor.height as usize * 4,
                        sampler: "linearClamp".to_string(),
                    },
                    font_key: descriptor.font_key,
                    atlas_generation: descriptor.atlas_generation,
                    glyph_count: descriptor.glyph_count,
                })
            }
            RuntimeRenderAssetKind::RenderTarget => {
                let descriptor = render_target_descriptor(request)?;
                if descriptor.width == 0 || descriptor.height == 0 || descriptor.sample_count != 1 {
                    return None;
                }
                RuntimeRenderAssetPayload::RenderTarget(RenderTargetRenderAsset {
                    target_id: descriptor.target_id,
                    width: descriptor.width,
                    height: descriptor.height,
                    format: descriptor.format,
                    clear_color: descriptor.clear_color,
                    sample_count: descriptor.sample_count,
                    sampled: descriptor.sampled,
                    present: descriptor.present,
                })
            }
        };
        Some(RuntimeRenderAsset {
            asset_id,
            kind: request.kind,
            usage: request.usage,
            payload,
        })
    }
}

fn texture_descriptor(
    request: &RuntimeRenderAssetRequest,
    record: Option<&RuntimeAssetRecord>,
) -> Option<TextureDescriptor> {
    if let Some(RuntimeRenderAssetDescriptor::Texture(descriptor)) = &request.descriptor {
        return Some(descriptor.clone());
    }
    let record = record?;
    if record.asset_type != "texture" && record.loader_kind != "texture" {
        return None;
    }
    let byte_len = record.size.unwrap_or(4).max(4) as usize;
    Some(TextureDescriptor {
        width: 1,
        height: (byte_len / 4).max(1) as u32,
        format: "Rgba8Unorm".to_string(),
        color_space: "srgb".to_string(),
        byte_len,
    })
}

fn material_descriptor(request: &RuntimeRenderAssetRequest) -> Option<MaterialDescriptor> {
    if let Some(RuntimeRenderAssetDescriptor::Material(descriptor)) = &request.descriptor {
        return Some(descriptor.clone());
    }
    Some(MaterialDescriptor {
        material_model: "unlit".to_string(),
        shader_key: "default.basic".to_string(),
        scalar_param_count: 0,
        vector_param_count: 1,
        texture_slot_count: 1,
        blend_mode: "alphaBlend".to_string(),
        cull_mode: "none".to_string(),
    })
}

fn mesh_descriptor(
    request: &RuntimeRenderAssetRequest,
    record: Option<&RuntimeAssetRecord>,
) -> Option<MeshDescriptor> {
    if let Some(RuntimeRenderAssetDescriptor::Mesh(descriptor)) = &request.descriptor {
        return Some(descriptor.clone());
    }
    let record = record?;
    if !matches!(
        (record.asset_type.as_str(), record.loader_kind.as_str()),
        ("mesh", _) | ("model", _) | (_, "mesh") | (_, "model")
    ) {
        return None;
    }
    let byte_len = record.size.unwrap_or(96).max(36);
    Some(MeshDescriptor {
        vertex_layout: "position3Color4Uv2".to_string(),
        vertex_count: (byte_len / 32).max(3) as u32,
        index_count: (byte_len / 4).max(3) as u32,
        index_format: "u32".to_string(),
    })
}

fn font_atlas_descriptor(request: &RuntimeRenderAssetRequest) -> Option<FontAtlasDescriptor> {
    if let Some(RuntimeRenderAssetDescriptor::FontAtlas(descriptor)) = &request.descriptor {
        return Some(descriptor.clone());
    }
    Some(FontAtlasDescriptor {
        font_key: request
            .asset_ref
            .clone()
            .unwrap_or_else(|| "default-font".to_string()),
        atlas_generation: 1,
        width: 256,
        height: 256,
        glyph_count: 0,
    })
}

fn render_target_descriptor(request: &RuntimeRenderAssetRequest) -> Option<RenderTargetDescriptor> {
    if let Some(RuntimeRenderAssetDescriptor::RenderTarget(descriptor)) = &request.descriptor {
        return Some(descriptor.clone());
    }
    None
}

fn resource_request_for_asset(asset: &RuntimeRenderAsset) -> Option<RenderResourceRequest> {
    let (resource_kind, source, byte_len) = match &asset.payload {
        RuntimeRenderAssetPayload::Texture(texture) => (
            RenderResourceKind::Texture,
            RenderResourceSource::TextureDescriptor {
                width: texture.width,
                height: texture.height,
                format: texture.format.clone(),
            },
            texture.byte_len,
        ),
        RuntimeRenderAssetPayload::AuiImage(_) | RuntimeRenderAssetPayload::ParticleTexture(_) => (
            RenderResourceKind::Texture,
            RenderResourceSource::TextureDescriptor {
                width: 1,
                height: 1,
                format: "Rgba8Unorm".to_string(),
            },
            4,
        ),
        RuntimeRenderAssetPayload::FontAtlas(font) => (
            RenderResourceKind::Texture,
            RenderResourceSource::TextureDescriptor {
                width: font.atlas_texture.width,
                height: font.atlas_texture.height,
                format: font.atlas_texture.format.clone(),
            },
            font.atlas_texture.byte_len,
        ),
        RuntimeRenderAssetPayload::Material(material) => (
            RenderResourceKind::MaterialParams,
            RenderResourceSource::MaterialParamsDescriptor {
                param_count: material
                    .scalar_param_count
                    .saturating_add(material.vector_param_count)
                    .max(1),
            },
            64,
        ),
        RuntimeRenderAssetPayload::Mesh(mesh) => (
            RenderResourceKind::MeshBuffer,
            RenderResourceSource::MeshBufferDescriptor {
                vertex_count: mesh.vertex_count,
                index_count: mesh.index_count,
            },
            mesh.vertex_count as usize * 32 + mesh.index_count as usize * 4,
        ),
        RuntimeRenderAssetPayload::RenderTarget(target) => (
            RenderResourceKind::SurfaceFrameTexture,
            RenderResourceSource::SurfaceFrameTextureDescriptor {
                target_id: target.target_id.clone(),
                width: target.width,
                height: target.height,
                format: target.format.clone(),
            },
            target.width as usize * target.height as usize * 4,
        ),
    };
    Some(RenderResourceRequest {
        key: RenderAssetKey {
            asset_id: asset.asset_id.0.clone(),
            asset_version: "v1".to_string(),
            resource_kind,
            platform_profile: "dev-desktop".to_string(),
            quality_profile: "default".to_string(),
            usage: asset.usage.as_key_usage().to_string(),
        },
        source,
        byte_len,
        reason: format!("RuntimeRenderAssetProduction {:?}", asset.kind),
    })
}

fn binding_for_asset(
    asset: &RuntimeRenderAsset,
    resource_handle: RenderResourceHandle,
) -> Option<RenderBindingSet> {
    let binding_kind = match asset.payload {
        RuntimeRenderAssetPayload::Texture(_)
        | RuntimeRenderAssetPayload::AuiImage(_)
        | RuntimeRenderAssetPayload::ParticleTexture(_) => RenderBindingKind::Texture,
        RuntimeRenderAssetPayload::Material(_) => RenderBindingKind::Material,
        RuntimeRenderAssetPayload::Mesh(_) => RenderBindingKind::Mesh,
        RuntimeRenderAssetPayload::FontAtlas(_) => RenderBindingKind::FontAtlas,
        RuntimeRenderAssetPayload::RenderTarget(_) => RenderBindingKind::RenderTarget,
    };
    Some(RenderBindingSet {
        binding_id: format!("binding:{}", asset.asset_id.0),
        binding_kind,
        resources: vec![resource_handle],
        material_handle: None,
        sampler: "linearClamp".to_string(),
        fallback_used: false,
        debug_label: format!("{:?} {:?}", asset.kind, asset.usage),
    })
}

fn asset_id_for_request(request: &RuntimeRenderAssetRequest) -> String {
    request
        .asset_ref
        .clone()
        .unwrap_or_else(|| request.request_id.clone())
}

fn event(
    request: &RuntimeRenderAssetRequest,
    stage: RuntimeRenderAssetProductionStage,
    code: RuntimeRenderAssetProductionCode,
    severity: RuntimeRenderAssetProductionSeverity,
    resource_handle: Option<RenderResourceHandle>,
    fallback_used: bool,
    message: impl Into<String>,
) -> RuntimeRenderAssetProductionEvent {
    RuntimeRenderAssetProductionEvent {
        request_id: request.request_id.clone(),
        kind: request.kind,
        usage: request.usage,
        asset_ref: request.asset_ref.clone(),
        asset_id: request.asset_ref.clone(),
        source_entity_id: request.source_entity_id.clone(),
        source_component: request.source_component.clone(),
        producer: request.kind.producer_name().to_string(),
        stage,
        code,
        severity,
        resource_handle,
        fallback_used,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn render_asset_production_report_covers_all_v1_kinds_and_usages() {
        let kinds = [
            RuntimeRenderAssetKind::Texture,
            RuntimeRenderAssetKind::Material,
            RuntimeRenderAssetKind::Mesh,
            RuntimeRenderAssetKind::AuiImage,
            RuntimeRenderAssetKind::FontAtlas,
            RuntimeRenderAssetKind::ParticleTexture,
            RuntimeRenderAssetKind::RenderTarget,
        ];
        let usages = [
            RuntimeRenderAssetUsage::Sprite2DTexture,
            RuntimeRenderAssetUsage::MeshAlbedoTexture,
            RuntimeRenderAssetUsage::AuiImageTexture,
            RuntimeRenderAssetUsage::FontAtlasTexture,
            RuntimeRenderAssetUsage::ParticleTexture,
            RuntimeRenderAssetUsage::MaterialBinding,
            RuntimeRenderAssetUsage::MeshGeometry,
            RuntimeRenderAssetUsage::SurfaceRenderTarget,
            RuntimeRenderAssetUsage::OffscreenRenderTarget,
        ];

        let events = kinds
            .iter()
            .zip(usages.iter().cycle())
            .map(|(kind, usage)| {
                let request = RuntimeRenderAssetRequest::from_asset(
                    1,
                    *kind,
                    *usage,
                    format!("{kind:?}-asset"),
                );
                event(
                    &request,
                    RuntimeRenderAssetProductionStage::Ready,
                    RuntimeRenderAssetProductionCode::Ready,
                    RuntimeRenderAssetProductionSeverity::Info,
                    None,
                    false,
                    "ready",
                )
            })
            .collect::<Vec<_>>();

        let report = RuntimeRenderAssetProductionReport::from_events(1, kinds.len(), events);

        assert_eq!(report.request_count, 7);
        assert_eq!(report.produced_count, 7);
        assert_eq!(report.failed_count, 0);
    }

    #[test]
    fn render_asset_production_report_counts_failed_and_fallback_events() {
        let request = RuntimeRenderAssetRequest::from_asset(
            1,
            RuntimeRenderAssetKind::Texture,
            RuntimeRenderAssetUsage::Sprite2DTexture,
            "missing-texture",
        );
        let report = RuntimeRenderAssetProductionReport::from_events(
            1,
            2,
            vec![
                event(
                    &request,
                    RuntimeRenderAssetProductionStage::Failed,
                    RuntimeRenderAssetProductionCode::MissingRuntimeAsset,
                    RuntimeRenderAssetProductionSeverity::Error,
                    None,
                    false,
                    "missing",
                ),
                event(
                    &request,
                    RuntimeRenderAssetProductionStage::CreateBinding,
                    RuntimeRenderAssetProductionCode::FallbackUsed,
                    RuntimeRenderAssetProductionSeverity::Warning,
                    None,
                    true,
                    "fallback",
                ),
            ],
        );

        assert_eq!(report.failed_count, 1);
        assert_eq!(report.fallback_count, 1);
    }

    #[test]
    fn render_asset_texture_production_uses_unified_texture_path() {
        let producer = RuntimeRenderAssetProducer::new();
        let mut manager = RenderResourceManager::new();
        let request = RuntimeRenderAssetRequest::from_asset(
            1,
            RuntimeRenderAssetKind::Texture,
            RuntimeRenderAssetUsage::Sprite2DTexture,
            "texture-a",
        );

        let output =
            producer.produce_from_record(&request, &record("texture", "texture"), &mut manager);

        assert_eq!(output.handle.status, RuntimeRenderAssetStatus::Ready);
        assert_eq!(
            output.resource_request.as_ref().unwrap().key.resource_kind,
            RenderResourceKind::Texture
        );
        assert_eq!(
            output.resource_request.as_ref().unwrap().key.usage,
            "sprite-2d-texture"
        );
    }

    #[test]
    fn render_asset_aui_image_texture_usage_uses_texture_producer() {
        let producer = RuntimeRenderAssetProducer::new();
        let mut manager = RenderResourceManager::new();
        let request = RuntimeRenderAssetRequest::from_asset(
            2,
            RuntimeRenderAssetKind::AuiImage,
            RuntimeRenderAssetUsage::AuiImageTexture,
            "ui-button",
        );

        let output =
            producer.produce_from_record(&request, &record("texture", "texture"), &mut manager);

        assert_eq!(output.handle.status, RuntimeRenderAssetStatus::Ready);
        assert_eq!(output.report.events[0].producer, "TextureProducer");
        assert_eq!(
            output.resource_request.as_ref().unwrap().key.resource_kind,
            RenderResourceKind::Texture
        );
    }

    #[test]
    fn render_asset_particle_texture_usage_uses_texture_producer() {
        let producer = RuntimeRenderAssetProducer::new();
        let mut manager = RenderResourceManager::new();
        let request = RuntimeRenderAssetRequest::from_asset(
            3,
            RuntimeRenderAssetKind::ParticleTexture,
            RuntimeRenderAssetUsage::ParticleTexture,
            "particle-spark",
        );

        let output =
            producer.produce_from_record(&request, &record("texture", "texture"), &mut manager);

        assert_eq!(output.handle.status, RuntimeRenderAssetStatus::Ready);
        assert_eq!(output.report.events[0].producer, "TextureProducer");
        assert_eq!(
            output.resource_request.as_ref().unwrap().key.usage,
            "particle-texture"
        );
    }

    #[test]
    fn render_asset_material_production_creates_minimal_binding() {
        let producer = RuntimeRenderAssetProducer::new();
        let mut manager = RenderResourceManager::new();
        let request = RuntimeRenderAssetRequest::from_asset(
            4,
            RuntimeRenderAssetKind::Material,
            RuntimeRenderAssetUsage::MaterialBinding,
            "material-basic",
        );

        let output =
            producer.produce_from_record(&request, &record("material", "material"), &mut manager);

        assert_eq!(output.handle.status, RuntimeRenderAssetStatus::Ready);
        assert_eq!(
            output.handle.binding.as_ref().unwrap().binding_kind,
            RenderBindingKind::Material
        );
    }

    #[test]
    fn render_asset_material_binding_fallback_is_reportable() {
        let request = RuntimeRenderAssetRequest::from_asset(
            4,
            RuntimeRenderAssetKind::Material,
            RuntimeRenderAssetUsage::MaterialBinding,
            "material-basic",
        );
        let report = RuntimeRenderAssetProductionReport::from_events(
            4,
            1,
            vec![event(
                &request,
                RuntimeRenderAssetProductionStage::CreateBinding,
                RuntimeRenderAssetProductionCode::FallbackUsed,
                RuntimeRenderAssetProductionSeverity::Warning,
                None,
                true,
                "Default material binding used.",
            )],
        );

        assert_eq!(report.fallback_count, 1);
    }

    #[test]
    fn render_asset_mesh_production_creates_mesh_buffer_request() {
        let producer = RuntimeRenderAssetProducer::new();
        let mut manager = RenderResourceManager::new();
        let request = RuntimeRenderAssetRequest::from_asset(
            5,
            RuntimeRenderAssetKind::Mesh,
            RuntimeRenderAssetUsage::MeshGeometry,
            "mesh-ship",
        );

        let output = producer.produce_from_record(&request, &record("mesh", "mesh"), &mut manager);

        assert_eq!(output.handle.status, RuntimeRenderAssetStatus::Ready);
        assert_eq!(
            output.resource_request.as_ref().unwrap().key.resource_kind,
            RenderResourceKind::MeshBuffer
        );
    }

    #[test]
    fn render_asset_mesh_binding_is_draw_consumable() {
        let producer = RuntimeRenderAssetProducer::new();
        let mut manager = RenderResourceManager::new();
        let request = RuntimeRenderAssetRequest::from_asset(
            5,
            RuntimeRenderAssetKind::Mesh,
            RuntimeRenderAssetUsage::MeshGeometry,
            "mesh-ship",
        );

        let output = producer.produce_from_record(&request, &record("mesh", "mesh"), &mut manager);

        assert_eq!(
            output.handle.binding.as_ref().unwrap().binding_kind,
            RenderBindingKind::Mesh
        );
    }

    #[test]
    fn render_asset_font_atlas_production_creates_texture_request() {
        let producer = RuntimeRenderAssetProducer::new();
        let mut manager = RenderResourceManager::new();
        let request = RuntimeRenderAssetRequest {
            descriptor: Some(RuntimeRenderAssetDescriptor::FontAtlas(
                FontAtlasDescriptor {
                    font_key: "default".to_string(),
                    atlas_generation: 2,
                    width: 128,
                    height: 128,
                    glyph_count: 12,
                },
            )),
            ..RuntimeRenderAssetRequest::from_asset(
                6,
                RuntimeRenderAssetKind::FontAtlas,
                RuntimeRenderAssetUsage::FontAtlasTexture,
                "font-default",
            )
        };

        let output = producer.produce_from_descriptor(&request, &mut manager);

        assert_eq!(output.handle.status, RuntimeRenderAssetStatus::Ready);
        assert_eq!(
            output.resource_request.as_ref().unwrap().key.resource_kind,
            RenderResourceKind::Texture
        );
        assert_eq!(
            output.handle.binding.as_ref().unwrap().binding_kind,
            RenderBindingKind::FontAtlas
        );
    }

    #[test]
    fn render_asset_render_target_production_creates_surface_handle() {
        let producer = RuntimeRenderAssetProducer::new();
        let mut manager = RenderResourceManager::new();
        let request = RuntimeRenderAssetRequest::render_target(
            7,
            "surface-main",
            640,
            360,
            "Bgra8UnormSrgb",
            RuntimeRenderAssetUsage::SurfaceRenderTarget,
        );

        let output = producer.produce_from_descriptor(&request, &mut manager);

        assert_eq!(output.handle.status, RuntimeRenderAssetStatus::Ready);
        assert_eq!(
            output.resource_request.as_ref().unwrap().key.resource_kind,
            RenderResourceKind::SurfaceFrameTexture
        );
    }

    #[test]
    fn render_asset_render_target_invalid_descriptor_reports_error() {
        let producer = RuntimeRenderAssetProducer::new();
        let mut manager = RenderResourceManager::new();
        let request = RuntimeRenderAssetRequest::render_target(
            7,
            "surface-main",
            0,
            360,
            "Bgra8UnormSrgb",
            RuntimeRenderAssetUsage::SurfaceRenderTarget,
        );

        let output = producer.produce_from_descriptor(&request, &mut manager);

        assert_eq!(output.handle.status, RuntimeRenderAssetStatus::Failed);
        assert_eq!(output.report.failed_count, 1);
        assert_eq!(
            output.report.events[0].code,
            RuntimeRenderAssetProductionCode::InvalidDescriptor
        );
    }

    #[test]
    fn render_asset_production_all_v1_types_have_success_and_failure_paths() {
        let producer = RuntimeRenderAssetProducer::new();
        let mut manager = RenderResourceManager::new();
        let requests = vec![
            (
                RuntimeRenderAssetRequest::from_asset(
                    8,
                    RuntimeRenderAssetKind::Texture,
                    RuntimeRenderAssetUsage::Sprite2DTexture,
                    "texture-a",
                ),
                Some(record("texture", "texture")),
            ),
            (
                RuntimeRenderAssetRequest::from_asset(
                    8,
                    RuntimeRenderAssetKind::Material,
                    RuntimeRenderAssetUsage::MaterialBinding,
                    "material-a",
                ),
                Some(record("material", "material")),
            ),
            (
                RuntimeRenderAssetRequest::from_asset(
                    8,
                    RuntimeRenderAssetKind::Mesh,
                    RuntimeRenderAssetUsage::MeshGeometry,
                    "mesh-a",
                ),
                Some(record("mesh", "mesh")),
            ),
            (
                RuntimeRenderAssetRequest::from_asset(
                    8,
                    RuntimeRenderAssetKind::AuiImage,
                    RuntimeRenderAssetUsage::AuiImageTexture,
                    "aui-a",
                ),
                Some(record("texture", "texture")),
            ),
            (
                RuntimeRenderAssetRequest::from_asset(
                    8,
                    RuntimeRenderAssetKind::FontAtlas,
                    RuntimeRenderAssetUsage::FontAtlasTexture,
                    "font-a",
                ),
                None,
            ),
            (
                RuntimeRenderAssetRequest::from_asset(
                    8,
                    RuntimeRenderAssetKind::ParticleTexture,
                    RuntimeRenderAssetUsage::ParticleTexture,
                    "particle-a",
                ),
                Some(record("texture", "texture")),
            ),
        ];

        for (request, record) in requests {
            let output = if let Some(record) = record {
                producer.produce_from_record(&request, &record, &mut manager)
            } else {
                producer.produce_from_descriptor(&request, &mut manager)
            };
            assert_eq!(output.handle.status, RuntimeRenderAssetStatus::Ready);
            assert_eq!(output.report.failed_count, 0);
        }

        let bad = RuntimeRenderAssetRequest::from_asset(
            8,
            RuntimeRenderAssetKind::Texture,
            RuntimeRenderAssetUsage::Sprite2DTexture,
            "",
        );
        let failed =
            producer.produce_from_record(&bad, &record("texture", "texture"), &mut manager);
        assert_eq!(failed.handle.status, RuntimeRenderAssetStatus::Failed);
    }
}
