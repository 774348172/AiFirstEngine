use super::*;
use crate::three_d_contract::{ImportMaterialStrategy, ImportOutput};
use engine_runtime::render_contract::*;
use engine_runtime::runtime_package::RuntimeAssetRef;
use engine_runtime::runtime_texture::{TextureFilter, TextureSampler, TextureWrap};

pub(super) fn reference(output: &ImportOutput, kind: RenderAssetKind) -> RenderAssetReference {
    RenderAssetReference {
        expected_kind: kind,
        reference: RuntimeAssetRef {
            id: output.asset_id.clone(),
            asset_type: kind.as_str().into(),
            guid: Some(output.asset_guid.clone()),
            sub_asset: Some(output.key.clone()),
        },
    }
}

pub(super) fn lower(
    material: gltf::Material<'_>,
    settings: &ModelImportV1,
    outputs: &BTreeMap<String, &ImportOutput>,
    report: &mut ModelImportDecodeReport,
) -> Result<CookedMaterial> {
    let path = material.index().map_or_else(
        || "/materials/default".into(),
        |i| format!("/materials/{i}"),
    );
    let invalid = |field: &str, code: &str, message: &str| {
        fail(&settings.source, &format!("{path}{field}"), code, message)
    };
    if !material.unlit() && settings.material_strategy == ImportMaterialStrategy::StrictUnlit {
        return Err(invalid("", "material.model_unsupported", "strictUnlit requires KHR_materials_unlit; select baseColorOnly explicitly to accept PBR semantic loss."));
    }
    let pbr = material.pbr_metallic_roughness();
    let lit =
        !material.unlit() && settings.material_strategy == ImportMaterialStrategy::StandardLit;
    if !material.unlit() && !lit {
        let mut discarded = vec![
            ("", "PBR lighting"),
            ("/pbrMetallicRoughness/metallicFactor", "metallic factor"),
            ("/pbrMetallicRoughness/roughnessFactor", "roughness factor"),
            ("/emissiveFactor", "emissive factor"),
        ];
        for (present, field, semantic) in [
            (
                pbr.metallic_roughness_texture().is_some(),
                "/pbrMetallicRoughness/metallicRoughnessTexture",
                "metallic/roughness texture",
            ),
            (
                material.normal_texture().is_some(),
                "/normalTexture",
                "normal texture and scale",
            ),
            (
                material.occlusion_texture().is_some(),
                "/occlusionTexture",
                "occlusion texture and strength",
            ),
            (
                material.emissive_texture().is_some(),
                "/emissiveTexture",
                "emissive texture",
            ),
        ] {
            if present {
                discarded.push((field, semantic));
            }
        }
        for (field, semantic) in discarded {
            report.notices.push(ImportDecodeNotice {
                source_path: settings.source.clone(),
                field_path: format!("{path}{field}"),
                outcome: "discarded".into(),
                message: format!("baseColorOnly explicitly discards {semantic}."),
            });
        }
        report.status = "degraded".into();
    }
    let (blend, depth) = match material.alpha_mode() {
        gltf::material::AlphaMode::Opaque => (MaterialBlend::Opaque, MaterialDepth::TestAndWrite),
        gltf::material::AlphaMode::Blend => (MaterialBlend::AlphaBlend, MaterialDepth::TestOnly),
        gltf::material::AlphaMode::Mask => (MaterialBlend::AlphaMask, MaterialDepth::TestAndWrite),
    };
    let color = pbr.base_color_factor();
    if color
        .iter()
        .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
    {
        return Err(invalid(
            "/pbrMetallicRoughness/baseColorFactor",
            "material.color_invalid",
            "Base color must be finite and in [0,1].",
        ));
    }
    let mut textures = BTreeMap::new();
    let mut sampling = None;
    if let Some(info) = pbr.base_color_texture() {
        if info.tex_coord() != 0 {
            return Err(invalid(
                "/pbrMetallicRoughness/baseColorTexture/texCoord",
                "material.uv_unsupported",
                "Only UV0 is supported.",
            ));
        }
        let texture = info.texture();
        let locator = format!("/images/{}", texture.source().index());
        let output = outputs.get(&locator).ok_or_else(|| {
            invalid(
                "/pbrMetallicRoughness/baseColorTexture",
                "mapping_missing",
                "Base-color image requires a committed texture output mapping.",
            )
        })?;
        textures.insert(
            "baseColorTexture".into(),
            reference(output, RenderAssetKind::Texture),
        );
        let resolved = resolve_sampler(texture.sampler());
        report.notices.push(ImportDecodeNotice {
            source_path: settings.source.clone(),
            field_path: format!("{path}/pbrMetallicRoughness/baseColorTexture/sampler"),
            outcome: "resolvedSampler".into(),
            message: resolved.key(),
        });
        sampling = Some(resolved);
    }
    let mut parameters = BTreeMap::from([("baseColor".into(), MaterialParameter::Vector4(color))]);
    if blend == MaterialBlend::AlphaMask {
        let cutoff = material.alpha_cutoff().unwrap_or(0.5);
        if !cutoff.is_finite() || !(0.0..=1.0).contains(&cutoff) {
            return Err(invalid(
                "/alphaCutoff",
                "material.alpha_cutoff_invalid",
                "Alpha cutoff must be finite and in [0,1].",
            ));
        }
        parameters.insert("alphaCutoff".into(), MaterialParameter::Float(cutoff));
    }
    let mut texture_sampling = BTreeMap::new();
    if lit {
        if let Some(value) = sampling.take() {
            texture_sampling.insert("baseColorTexture".into(), value);
        }
        let mut add = |slot: &str, texture: gltf::Texture<'_>, uv: u32| -> Result<()> {
            if uv != 0 {
                return Err(invalid(
                    slot,
                    "material.uv_unsupported",
                    "StandardLit supports UV0 only.",
                ));
            }
            let locator = format!("/images/{}", texture.source().index());
            let output = outputs.get(&locator).ok_or_else(|| {
                invalid(
                    slot,
                    "mapping_missing",
                    "Texture requires a committed output mapping.",
                )
            })?;
            textures.insert(slot.into(), reference(output, RenderAssetKind::Texture));
            texture_sampling.insert(slot.into(), resolve_sampler(texture.sampler()));
            Ok(())
        };
        if let Some(t) = pbr.metallic_roughness_texture() {
            add("metallicRoughnessTexture", t.texture(), t.tex_coord())?;
        }
        if let Some(t) = material.normal_texture() {
            add("normalTexture", t.texture(), t.tex_coord())?;
        }
        if let Some(t) = material.occlusion_texture() {
            add("occlusionTexture", t.texture(), t.tex_coord())?;
        }
        if let Some(t) = material.emissive_texture() {
            add("emissiveTexture", t.texture(), t.tex_coord())?;
        }
        for (name, value) in [
            ("metallic", pbr.metallic_factor()),
            ("roughness", pbr.roughness_factor()),
            (
                "normalScale",
                material.normal_texture().map_or(1., |t| t.scale()),
            ),
            (
                "occlusionStrength",
                material.occlusion_texture().map_or(1., |t| t.strength()),
            ),
        ] {
            parameters.insert(name.into(), MaterialParameter::Float(value));
        }
        let e = material.emissive_factor();
        parameters.insert(
            "emissive".into(),
            MaterialParameter::Vector4([e[0], e[1], e[2], 0.]),
        );
    }
    let cooked = CookedMaterial {
        decal_bias: None,
        preset: if lit {
            MaterialPreset::StandardLit
        } else {
            MaterialPreset::Unlit
        },
        texture_sampling,
        parameters,
        textures,
        base_color_texture_sampling: sampling,
        opaque_alpha: blend == MaterialBlend::Opaque,
        ignore_vertex_color: false,
        blend,
        cull: if material.double_sided() {
            MaterialCull::None
        } else {
            MaterialCull::Back
        },
        depth,
        shader_source: if lit {
            STANDARD_LIT_SHADER_SOURCE
        } else {
            UNLIT_SHADER_SOURCE
        }
        .into(),
    };
    cooked
        .validate()
        .map_err(|e| invalid("", "material.invalid", &e.to_string()))?;
    Ok(cooked)
}

fn resolve_sampler(sampler: gltf::texture::Sampler<'_>) -> TextureSampler {
    let min_filter = match sampler.min_filter() {
        None | Some(gltf::texture::MinFilter::Linear) => TextureFilter::Linear,
        Some(gltf::texture::MinFilter::Nearest) => TextureFilter::Nearest,
        Some(gltf::texture::MinFilter::NearestMipmapNearest) => TextureFilter::NearestMipmapNearest,
        Some(gltf::texture::MinFilter::LinearMipmapNearest) => TextureFilter::LinearMipmapNearest,
        Some(gltf::texture::MinFilter::NearestMipmapLinear) => TextureFilter::NearestMipmapLinear,
        Some(gltf::texture::MinFilter::LinearMipmapLinear) => TextureFilter::LinearMipmapLinear,
    };
    let mag_filter = match sampler.mag_filter() {
        Some(gltf::texture::MagFilter::Nearest) => TextureFilter::Nearest,
        _ => TextureFilter::Linear,
    };
    let wrap = |value| match value {
        gltf::texture::WrappingMode::ClampToEdge => TextureWrap::ClampToEdge,
        gltf::texture::WrappingMode::MirroredRepeat => TextureWrap::MirroredRepeat,
        gltf::texture::WrappingMode::Repeat => TextureWrap::Repeat,
    };
    TextureSampler {
        min_filter,
        mag_filter,
        wrap_s: wrap(sampler.wrap_s()),
        wrap_t: wrap(sampler.wrap_t()),
    }
}
