//! Native render data. Importers produce these same bytes; no backend objects live here.
use super::{
    ContractError, ContractErrorCode, CoordinateConvention, RenderAssetKind, RenderAssetReference,
    ValidateRenderContract,
};
use crate::canonical_digest::sha256_prefixed;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const NATIVE_RENDER_COOK_RECIPE: &str = "render-native.v1";
pub const GENERATED_RENDER_COOK_RECIPE: &str = "render-native.generated-static.v1";
pub const IMPORTED_RENDER_COOK_RECIPE: &str = "render-native.imported-static.v1";
/// Source owner of the existing built-in unlit shader, not a package file path.
/// U2 derives executable pipelines from this material recipe plus view/layout state.
pub const UNLIT_SHADER_SOURCE: &str =
    "engine_runtime/src/wgpu_backend.rs#real::create_basic_pipeline";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    tag = "kind",
    content = "payload",
    deny_unknown_fields
)]
pub enum RenderAssetData {
    Mesh(CookedGeometry),
    Material(CookedMaterial),
}

impl RenderAssetData {
    pub fn asset_kind(&self) -> RenderAssetKind {
        match self {
            Self::Mesh(_) => RenderAssetKind::Mesh,
            Self::Material(_) => RenderAssetKind::Material,
        }
    }
}

impl ValidateRenderContract for RenderAssetData {
    fn validate(&self) -> Result<(), ContractError> {
        match self {
            Self::Mesh(value) => value.validate(),
            Self::Material(value) => value.validate(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CookedRenderAsset {
    pub source_digest: String,
    pub recipe: String,
    pub asset: RenderAssetData,
}

pub fn is_sha256_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|v| {
        v.len() == 64
            && v.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

impl ValidateRenderContract for CookedRenderAsset {
    fn validate(&self) -> Result<(), ContractError> {
        if !matches!(
            self.recipe.as_str(),
            NATIVE_RENDER_COOK_RECIPE | GENERATED_RENDER_COOK_RECIPE | IMPORTED_RENDER_COOK_RECIPE
        ) || !is_sha256_digest(&self.source_digest)
        {
            return Err(invalid("cookManifest"));
        }
        self.asset.validate()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VertexSemantic {
    Position,
    Normal,
    TexCoord0,
    Color0,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VertexFormat {
    Float32x2,
    Float32x3,
    Float32x4,
}
impl VertexFormat {
    fn byte_len(self) -> u32 {
        match self {
            Self::Float32x2 => 8,
            Self::Float32x3 => 12,
            Self::Float32x4 => 16,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VertexAttribute {
    pub semantic: VertexSemantic,
    pub format: VertexFormat,
    pub offset: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GeometryTopology {
    TriangleList,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GeometryIndexFormat {
    Uint16,
    Uint32,
}
impl GeometryIndexFormat {
    pub fn byte_len(self) -> usize {
        match self {
            Self::Uint16 => 2,
            Self::Uint32 => 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubmeshRange {
    pub first_index: u32,
    pub index_count: u32,
    pub material_slot: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GeometryBounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub sphere_center: [f32; 3],
    pub sphere_radius: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CookedGeometry {
    pub vertex_bytes: Vec<u8>,
    pub index_bytes: Vec<u8>,
    pub vertex_stride: u32,
    pub attributes: Vec<VertexAttribute>,
    pub vertex_count: u32,
    pub index_count: u32,
    pub index_format: GeometryIndexFormat,
    pub topology: GeometryTopology,
    pub submeshes: Vec<SubmeshRange>,
    pub bounds: GeometryBounds,
    pub coordinate_convention: CoordinateConvention,
    pub material_slots: Vec<String>,
}

impl ValidateRenderContract for CookedGeometry {
    fn validate(&self) -> Result<(), ContractError> {
        if self.vertex_stride == 0
            || self.vertex_stride % 4 != 0
            || self.vertex_count == 0
            || u64::from(self.vertex_stride) * u64::from(self.vertex_count)
                != self.vertex_bytes.len() as u64
        {
            return Err(invalid("geometry.vertexBytes"));
        }
        if self.index_count == 0
            || self.index_count % 3 != 0
            || u64::from(self.index_count) * self.index_format.byte_len() as u64
                != self.index_bytes.len() as u64
        {
            return Err(invalid("geometry.indexBytes"));
        }
        let position = self
            .attributes
            .iter()
            .find(|a| a.semantic == VertexSemantic::Position)
            .ok_or_else(|| invalid("geometry.attributes.position"))?;
        if position.format != VertexFormat::Float32x3 {
            return Err(invalid("geometry.attributes.position"));
        }
        for (i, a) in self.attributes.iter().enumerate() {
            let end = a
                .offset
                .checked_add(a.format.byte_len())
                .ok_or_else(|| invalid("geometry.attributes"))?;
            let format_valid = match a.semantic {
                VertexSemantic::Position | VertexSemantic::Normal => {
                    a.format == VertexFormat::Float32x3
                }
                VertexSemantic::TexCoord0 => a.format == VertexFormat::Float32x2,
                VertexSemantic::Color0 => a.format == VertexFormat::Float32x4,
            };
            if !format_valid
                || a.offset % 4 != 0
                || end > self.vertex_stride
                || self.attributes[..i].iter().any(|b| {
                    b.semantic == a.semantic
                        || (a.offset < b.offset + b.format.byte_len() && b.offset < end)
                })
            {
                return Err(invalid("geometry.attributes"));
            }
        }
        let b = &self.bounds;
        if !b
            .min
            .iter()
            .chain(&b.max)
            .chain(&b.sphere_center)
            .chain(std::iter::once(&b.sphere_radius))
            .all(|v| v.is_finite())
            || b.sphere_radius < 0.0
            || (0..3).any(|i| b.min[i] > b.max[i])
        {
            return Err(invalid("geometry.bounds"));
        }
        for vertex in self.vertex_bytes.chunks_exact(self.vertex_stride as usize) {
            for attribute in &self.attributes {
                for part in vertex[attribute.offset as usize
                    ..(attribute.offset + attribute.format.byte_len()) as usize]
                    .chunks_exact(4)
                {
                    if !f32::from_le_bytes(part.try_into().unwrap()).is_finite() {
                        return Err(invalid("geometry.vertexBytes.nonFinite"));
                    }
                }
            }
            let mut distance = 0.0f64;
            for i in 0..3 {
                let start = position.offset as usize + i * 4;
                let value = f32::from_le_bytes(vertex[start..start + 4].try_into().unwrap());
                if value < b.min[i] || value > b.max[i] {
                    return Err(invalid("geometry.bounds.aabb"));
                }
                distance += (f64::from(value) - f64::from(b.sphere_center[i])).powi(2);
            }
            if distance > f64::from(b.sphere_radius).powi(2) * (1.0 + 1e-6) {
                return Err(invalid("geometry.bounds.sphere"));
            }
        }
        for bytes in self.index_bytes.chunks_exact(self.index_format.byte_len()) {
            let index = match self.index_format {
                GeometryIndexFormat::Uint16 => {
                    u32::from(u16::from_le_bytes(bytes.try_into().unwrap()))
                }
                GeometryIndexFormat::Uint32 => u32::from_le_bytes(bytes.try_into().unwrap()),
            };
            if index >= self.vertex_count {
                return Err(invalid("geometry.indexBytes.range"));
            }
        }
        if self.material_slots.is_empty()
            || self
                .material_slots
                .iter()
                .enumerate()
                .any(|(i, s)| s.trim().is_empty() || self.material_slots[..i].contains(s))
        {
            return Err(invalid("geometry.materialSlots"));
        }
        let mut ranges = self.submeshes.iter().collect::<Vec<_>>();
        ranges.sort_by_key(|s| s.first_index);
        let mut next = 0;
        for submesh in ranges {
            if submesh.first_index != next
                || submesh.index_count == 0
                || submesh.index_count % 3 != 0
                || !self.material_slots.contains(&submesh.material_slot)
            {
                return Err(invalid("geometry.submeshes"));
            }
            next = next
                .checked_add(submesh.index_count)
                .ok_or_else(|| invalid("geometry.submeshes"))?;
        }
        if next != self.index_count {
            return Err(invalid("geometry.submeshes"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    tag = "type",
    content = "value",
    deny_unknown_fields
)]
pub enum MaterialParameter {
    Float(f32),
    Vector4([f32; 4]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MaterialPreset {
    Unlit,
    StandardLit,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MaterialBlend {
    Opaque,
    AlphaBlend,
    AlphaMask,
    PremultipliedAlpha,
    PremultipliedAdditive,
    PremultipliedMultiply,
    PremultipliedScreen,
}
impl MaterialBlend {
    pub fn is_premultiplied(self) -> bool {
        matches!(
            self,
            Self::PremultipliedAlpha
                | Self::PremultipliedAdditive
                | Self::PremultipliedMultiply
                | Self::PremultipliedScreen
        )
    }
    pub fn is_transparent(self) -> bool {
        self == Self::AlphaBlend || self.is_premultiplied()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MaterialCull {
    None,
    Back,
    Front,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MaterialDepth {
    Disabled,
    TestAndWrite,
    TestOnly,
}

pub const STANDARD_LIT_SHADER_SOURCE: &str =
    "engine_runtime/src/wgpu_backend/real/geometry/lit.rs#shader";
pub const STANDARD_LIT_TEXTURE_SLOTS: [&str; 5] = [
    "baseColorTexture",
    "normalTexture",
    "metallicRoughnessTexture",
    "occlusionTexture",
    "emissiveTexture",
];

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StandardLitParameters {
    pub surface: [crate::render_graph::OrderedF32; 4],
    pub emissive: [crate::render_graph::OrderedF32; 4],
}
impl StandardLitParameters {
    pub fn validate(&self) -> Result<(), ContractError> {
        let v = self.surface.map(crate::render_graph::OrderedF32::to_f32);
        let e = self.emissive.map(crate::render_graph::OrderedF32::to_f32);
        if v.iter().chain(&e).any(|x| !x.is_finite())
            || !(0.0..=1.0).contains(&v[0])
            || !(0.0..=1.0).contains(&v[1])
            || v[2] < 0.0
            || !(0.0..=1.0).contains(&v[3])
            || e[..3].iter().any(|x| *x < 0.0)
            || e[3] != 0.0
        {
            return Err(invalid("material.standardLit"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CookedMaterial {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decal_bias: Option<i32>,
    pub preset: MaterialPreset,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub texture_sampling: BTreeMap<String, crate::runtime_texture::TextureSampler>,
    pub parameters: BTreeMap<String, MaterialParameter>,
    pub textures: BTreeMap<String, RenderAssetReference>,
    /// High-level base-color textures are sampled as sRGB; absent preserves U1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_color_texture_sampling: Option<crate::runtime_texture::TextureSampler>,
    /// glTF OPAQUE ignores all alpha inputs. False preserves native U1 output alpha.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub opaque_alpha: bool,
    /// Native UnlitFlat variant; default preserves vertex-color multiplication.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ignore_vertex_color: bool,
    pub blend: MaterialBlend,
    pub cull: MaterialCull,
    pub depth: MaterialDepth,
    pub shader_source: String,
}

impl CookedMaterial {
    pub fn standard_lit(&self) -> Option<StandardLitParameters> {
        if self.preset != MaterialPreset::StandardLit {
            return None;
        }
        let f = |n: &str| match self.parameters[n] {
            MaterialParameter::Float(v) => v.into(),
            _ => unreachable!(),
        };
        let MaterialParameter::Vector4(e) = self.parameters["emissive"] else {
            unreachable!()
        };
        Some(StandardLitParameters {
            surface: [
                f("metallic"),
                f("roughness"),
                f("normalScale"),
                f("occlusionStrength"),
            ],
            emissive: e.map(Into::into),
        })
    }
    /// Pipeline state excludes material values and texture identities, which are bindings.
    pub fn pipeline_identity(&self) -> Result<String, ContractError> {
        self.validate()?;
        let original = sha256_prefixed(
            &serde_json::to_vec(&(
                NATIVE_RENDER_COOK_RECIPE,
                self.preset,
                self.blend,
                self.cull,
                self.depth,
                &self.shader_source,
            ))
            .unwrap(),
        );
        let original = if let Some(bias) = self.decal_bias {
            sha256_prefixed(format!("{original}|mesh-decal:{bias}").as_bytes())
        } else {
            original
        };
        let original = if self.ignore_vertex_color {
            sha256_prefixed(format!("{original}|unlit-flat").as_bytes())
        } else {
            original
        };
        Ok(if self.opaque_alpha {
            sha256_prefixed(format!("{original}|opaque-alpha-one").as_bytes())
        } else {
            original
        })
    }
}

impl ValidateRenderContract for CookedMaterial {
    fn validate(&self) -> Result<(), ContractError> {
        if self.decal_bias.is_some_and(|v| {
            !(-16..=-1).contains(&v)
                || self.preset != MaterialPreset::Unlit
                || self.blend != MaterialBlend::AlphaBlend
                || self.depth != MaterialDepth::TestOnly
        }) {
            return Err(invalid("material.decalBias"));
        }
        if self.blend.is_premultiplied()
            && (self.preset != MaterialPreset::Unlit
                || self.depth != MaterialDepth::Disabled
                || self.decal_bias.is_some())
        {
            return Err(invalid("material.premultipliedPolicy"));
        }
        if self.opaque_alpha && self.blend != MaterialBlend::Opaque {
            return Err(invalid("material.opaqueAlpha"));
        }
        if self.base_color_texture_sampling.is_some_and(|sampling| {
            !self.textures.contains_key("baseColorTexture")
                || crate::runtime_texture::TextureSampler::parse(&sampling.key()).is_none()
        }) {
            return Err(invalid("material.baseColorTextureSampling"));
        }
        if self.shader_source
            != match self.preset {
                MaterialPreset::Unlit => UNLIT_SHADER_SOURCE,
                MaterialPreset::StandardLit => STANDARD_LIT_SHADER_SOURCE,
            }
        {
            return Err(invalid("material.shaderSource"));
        }
        let lit = self.preset == MaterialPreset::StandardLit;
        if self.texture_sampling.iter().any(|(slot, sampler)| {
            !lit || !self.textures.contains_key(slot)
                || crate::runtime_texture::TextureSampler::parse(&sampler.key()).is_none()
        }) || (lit && self.base_color_texture_sampling.is_some())
        {
            return Err(invalid("material.textureSampling"));
        }
        let cutoff = self.parameters.get("alphaCutoff");
        if (self.blend == MaterialBlend::AlphaMask) != cutoff.is_some()
            || (self.blend == MaterialBlend::AlphaMask && self.depth != MaterialDepth::TestAndWrite)
        {
            return Err(invalid("material.alphaCutoff"));
        }
        for (name, value) in &self.parameters {
            match (name.as_str(), value) {
                ("alphaCutoff", MaterialParameter::Float(v))
                    if self.blend == MaterialBlend::AlphaMask
                        && v.is_finite()
                        && (0.0..=1.0).contains(v) => {}
                (
                    "metallic" | "roughness" | "normalScale" | "occlusionStrength",
                    MaterialParameter::Float(v),
                ) if lit && v.is_finite() => {}
                ("emissive", MaterialParameter::Vector4(v))
                    if lit && v.iter().all(|x| x.is_finite()) => {}
                ("baseColor", MaterialParameter::Vector4(v))
                    if v.iter()
                        .all(|v| v.is_finite() && (!lit || (0.0..=1.0).contains(v))) => {}
                _ => {
                    return Err(ContractError::new(
                        ContractErrorCode::UnsupportedMaterialParameter,
                        "material.parameters",
                    ));
                }
            }
        }
        if !self.parameters.contains_key("baseColor") {
            return Err(invalid("material.parameters.baseColor"));
        }
        if lit {
            if [
                "metallic",
                "roughness",
                "normalScale",
                "occlusionStrength",
                "emissive",
            ]
            .iter()
            .any(|n| !self.parameters.contains_key(*n))
            {
                return Err(invalid("material.standardLit"));
            }
            self.standard_lit().unwrap().validate()?;
            if self
                .textures
                .keys()
                .any(|n| !self.texture_sampling.contains_key(n))
            {
                return Err(invalid("material.textureSampling"));
            }
        }
        for (slot, reference) in &self.textures {
            if !(slot == "baseColorTexture"
                || (lit && STANDARD_LIT_TEXTURE_SLOTS.contains(&slot.as_str())))
                || reference.expected_kind != RenderAssetKind::Texture
            {
                return Err(invalid("material.textures"));
            }
            reference.validate()?;
        }
        Ok(())
    }
}

fn invalid(field: &'static str) -> ContractError {
    ContractError::new(ContractErrorCode::InvalidAssetPayload, field)
}

impl CookedGeometry {
    /// Validate only the material's submesh; unrelated Unlit slots may omit UVs.
    pub fn validate_standard_lit_inputs(
        &self,
        range: std::ops::Range<u32>,
        normal_map: bool,
        textured: bool,
    ) -> Result<(), ContractError> {
        self.validate()?;
        VerifiedMaterialGeometry {
            vertex_bytes: &self.vertex_bytes,
            index_bytes: &self.index_bytes,
            vertex_stride: self.vertex_stride,
            attributes: &self.attributes,
            index_format: self.index_format,
            index_count: self.index_count,
        }
        .validate(range, normal_map, textured)
    }
}

/// Borrowed view of already verified geometry. The original material requirements
/// are shared by cooked assets and generated buffers without cloning either backing.
pub(crate) struct VerifiedMaterialGeometry<'a> {
    pub vertex_bytes: &'a [u8],
    pub index_bytes: &'a [u8],
    pub vertex_stride: u32,
    pub attributes: &'a [VertexAttribute],
    pub index_format: GeometryIndexFormat,
    pub index_count: u32,
}

impl VerifiedMaterialGeometry<'_> {
    pub(crate) fn validate(
        &self,
        range: std::ops::Range<u32>,
        normal_map: bool,
        textured: bool,
    ) -> Result<(), ContractError> {
        if !range.start.is_multiple_of(3)
            || !range.end.is_multiple_of(3)
            || range.start >= range.end
            || range.end > self.index_count
        {
            return Err(invalid("material.indexRange"));
        }
        let g = self;
        let normal = g
            .attributes
            .iter()
            .find(|a| a.semantic == VertexSemantic::Normal)
            .ok_or_else(|| invalid("material.normalRequired"))?;
        let uv = g
            .attributes
            .iter()
            .find(|a| a.semantic == VertexSemantic::TexCoord0);
        if (textured || normal_map) && uv.is_none() {
            return Err(invalid("material.uvRequired"));
        }
        let read = |index: usize, offset: usize| {
            f32::from_le_bytes(
                g.vertex_bytes[index * g.vertex_stride as usize + offset
                    ..index * g.vertex_stride as usize + offset + 4]
                    .try_into()
                    .unwrap(),
            )
        };
        let width = g.index_format.byte_len();
        let bytes = &g.index_bytes[range.start as usize * width..range.end as usize * width];
        for index in bytes.chunks_exact(width) {
            let i = if width == 2 {
                u16::from_le_bytes(index.try_into().unwrap()) as usize
            } else {
                u32::from_le_bytes(index.try_into().unwrap()) as usize
            };
            let len: f32 = (0..3)
                .map(|j| read(i, normal.offset as usize + j * 4).powi(2))
                .sum();
            if len < 1e-12 {
                return Err(invalid("material.zeroNormal"));
            }
        }

        if normal_map {
            let uv = uv.unwrap().offset as usize;
            let width = g.index_format.byte_len();
            for triangle in bytes.chunks_exact(3 * width) {
                let ids: [usize; 3] = std::array::from_fn(|i| {
                    if width == 2 {
                        u16::from_le_bytes(triangle[i * width..(i + 1) * width].try_into().unwrap())
                            as usize
                    } else {
                        u32::from_le_bytes(triangle[i * width..(i + 1) * width].try_into().unwrap())
                            as usize
                    }
                });
                let du = read(ids[1], uv) - read(ids[0], uv);
                let dv = read(ids[1], uv + 4) - read(ids[0], uv + 4);
                let eu = read(ids[2], uv) - read(ids[0], uv);
                let ev = read(ids[2], uv + 4) - read(ids[0], uv + 4);
                if (du * ev - dv * eu).abs() < 1e-12 {
                    return Err(invalid("material.degenerateNormalUv"));
                }
            }
        }
        Ok(())
    }
}
