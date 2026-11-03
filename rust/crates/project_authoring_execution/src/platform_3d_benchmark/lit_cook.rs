//! BM-04 admission inspects real Cook output, never the source manifest's claims.
use super::{BenchmarkError, BenchmarkResult};
use crate::PreparedRuntimePackage;
use engine_runtime::{
    canonical_digest::sha256_prefixed,
    color_pipeline::AntiAliasing3D,
    environment_lighting::EnvironmentMap,
    render_contract::{
        CookedRenderAsset, MaterialBlend, MaterialPreset, Projection, RenderAssetData,
        RenderContract, VertexSemantic,
    },
    runtime_package_builder::RuntimePackageBuildInput,
    runtime_texture::texture_mip_byte_len,
    spatial_contract::{LightKind, RuntimeSpatial},
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LitCookEvidence {
    pub scope: &'static str,
    pub instances: usize,
    pub meshes: usize,
    pub materials: usize,
    pub textures: usize,
    pub instance_triangles: u64,
    pub mask_instances: usize,
    pub blend_instances: usize,
    pub lights: [usize; 3],
    pub shadow_lights: [usize; 3],
    pub texture_bytes: u64,
    pub geometry_bytes: u64,
    pub environment_bytes: u64,
    pub environment_source_face_size: u32,
    pub payload_digests: BTreeMap<String, String>,
    pub qualification: &'static str,
}

pub fn inspect_lit_cook(prepared: &PreparedRuntimePackage) -> BenchmarkResult<LitCookEvidence> {
    inspect_prepared(prepared, false)
}

/// Explicitly reduced development fixture; this entry never admits the PC target.
pub fn inspect_lit_diagnostic(
    prepared: &PreparedRuntimePackage,
) -> BenchmarkResult<LitCookEvidence> {
    inspect_prepared(prepared, true)
}

fn inspect_prepared(
    prepared: &PreparedRuntimePackage,
    mini: bool,
) -> BenchmarkResult<LitCookEvidence> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Descriptor {
        asset_id: String,
        source_cubemap: String,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct CubeHeader {
        schema_version: String,
        face_size: u32,
    }
    let source = prepared.source();
    let descriptor: Descriptor = serde_json::from_slice(
        source
            .bytes("Assets/studio.json")
            .ok_or_else(|| invalid("environment descriptor source"))?,
    )
    .map_err(|e| invalid(e.to_string()))?;
    if descriptor.asset_id != "studio" {
        return Err(invalid("environment source identity"));
    }
    let bytes = source
        .bytes(&descriptor.source_cubemap)
        .ok_or_else(|| invalid("cubemap source"))?;
    // Prepare already validates all six faces and binds the source bytes to the
    // derived payload. This borrowed header check does not allocate a JSON tree.
    let header: CubeHeader = serde_json::from_slice(bytes).map_err(|e| invalid(e.to_string()))?;
    if header.schema_version != "cube-radiance.v1"
        || header.face_size != if mini { 4 } else { 1024 }
    {
        return Err(invalid("original cubemap source extent"));
    }
    let mut result = inspect(prepared.runtime_package_build_input(), mini)?;
    result.environment_source_face_size = header.face_size;
    result.payload_digests.insert(
        format!("source:{}", descriptor.source_cubemap),
        sha256_prefixed(bytes),
    );
    Ok(result)
}

fn invalid(message: impl Into<String>) -> BenchmarkError {
    BenchmarkError::new("benchmark.lit_payload_ineligible", message)
}

fn inspect(input: &RuntimePackageBuildInput, mini: bool) -> BenchmarkResult<LitCookEvidence> {
    let expected: (usize, usize, usize, usize, u32) = if mini {
        (20, 2, 4, 6, 4)
    } else {
        (10000, 100, 128, 96, 2048)
    };
    let mut result = LitCookEvidence {
        scope: if mini {
            "diagnostic-mini-ineligible"
        } else {
            "BM-04/lit-v1-PC-cook"
        },
        instances: 0,
        meshes: 0,
        materials: 0,
        textures: 0,
        instance_triangles: 0,
        mask_instances: 0,
        blend_instances: 0,
        lights: [0; 3],
        shadow_lights: [0; 3],
        texture_bytes: 0,
        geometry_bytes: 0,
        environment_bytes: 0,
        environment_source_face_size: 0,
        payload_digests: BTreeMap::new(),
        qualification: "unqualified",
    };
    let mut meshes = BTreeMap::new();
    let mut materials = BTreeMap::new();
    let mut geometry_hashes = BTreeSet::new();
    let mut environments = BTreeSet::new();
    for asset in &input.assets {
        if !matches!(
            asset.asset_type.as_str(),
            "mesh" | "material" | "environment"
        ) {
            continue;
        }
        let bytes = asset
            .runtime_payload
            .as_ref()
            .ok_or_else(|| invalid(&asset.asset_id))?;
        result
            .payload_digests
            .insert(asset.asset_id.clone(), sha256_prefixed(bytes));
        if asset.asset_type == "environment" {
            let map = EnvironmentMap::from_bytes(bytes).map_err(invalid)?;
            if map.words()[1] != if mini { 16 } else { 64 } {
                return Err(invalid("IBL derived extent"));
            }
            result.environment_bytes += bytes.len() as u64;
            environments.insert(asset.asset_id.as_str());
            continue;
        }
        let cooked = RenderContract::<CookedRenderAsset>::decode(
            std::str::from_utf8(bytes).map_err(|e| invalid(e.to_string()))?,
        )
        .map_err(|e| invalid(e.to_string()))?
        .data;
        match cooked.asset {
            RenderAssetData::Mesh(mesh) => {
                if mesh.vertex_count != 600
                    || mesh.index_count != 3000
                    || ![VertexSemantic::Normal, VertexSemantic::TexCoord0]
                        .iter()
                        .all(|s| mesh.attributes.iter().any(|a| a.semantic == *s))
                    || !geometry_hashes.insert(sha256_prefixed(&mesh.vertex_bytes))
                {
                    return Err(invalid(format!(
                        "wrong or duplicate mesh {}",
                        asset.asset_id
                    )));
                }
                result.geometry_bytes += (mesh.vertex_bytes.len() + mesh.index_bytes.len()) as u64;
                meshes.insert(asset.asset_id.as_str(), mesh);
            }
            RenderAssetData::Material(material) => {
                if material.preset != MaterialPreset::StandardLit || material.textures.len() != 4 {
                    return Err(invalid(format!(
                        "StandardLit four slots required: {}",
                        asset.asset_id
                    )));
                }
                let mut slots = Vec::new();
                for slot in [
                    "baseColorTexture",
                    "normalTexture",
                    "metallicRoughnessTexture",
                    "occlusionTexture",
                ] {
                    let texture = material.textures.get(slot).ok_or_else(|| invalid(slot))?;
                    let sampler = material
                        .texture_sampling
                        .get(slot)
                        .ok_or_else(|| invalid("mip sampler missing"))?;
                    if sampler.min_filter
                        != engine_runtime::runtime_texture::TextureFilter::LinearMipmapLinear
                    {
                        return Err(invalid("trilinear mip sampling required"));
                    }
                    slots.push(texture.reference.id.clone());
                }
                if slots[2] != slots[3] || slots[0] == slots[1] || slots[1] == slots[2] {
                    return Err(invalid("baseColor/normal/shared ORM semantics"));
                }
                materials.insert(asset.asset_id.as_str(), (material, slots));
            }
        }
    }
    let mut textures = BTreeMap::new();
    let mut pixel_hashes = BTreeSet::new();
    for texture in &input.texture_payloads {
        let m = &texture.metadata;
        let mip_count = expected.4.ilog2() + 1;
        let digest = sha256_prefixed(&texture.rgba8);
        let base_len = texture_mip_byte_len(&m.format, m.width, m.height, 1).unwrap_or(0);
        let constant = !mini
            && texture.rgba8.get(..base_len).is_none_or(|base| {
                base.chunks_exact(16)
                    .all(|block| Some(block) == base.get(..16))
            });
        if m.width != expected.4
            || m.height != expected.4
            || m.mip_count != mip_count
            || !matches!(m.format.as_str(), "bc3RgbaUnorm" | "bc3RgbaUnormSrgb")
            || texture_mip_byte_len(&m.format, m.width, m.height, m.mip_count)
                != Some(texture.rgba8.len())
            || m.byte_length != texture.rgba8.len()
            || constant
            || !pixel_hashes.insert(digest.clone())
        {
            return Err(invalid(format!(
                "wrong extent/mip/format/shared payload {}",
                m.asset_id
            )));
        }
        result.texture_bytes += texture.rgba8.len() as u64;
        result.payload_digests.insert(m.asset_id.clone(), digest);
        textures.insert(m.asset_id.as_str(), m);
    }
    let mut bound_meshes = BTreeSet::new();
    let mut bound_materials = BTreeSet::new();
    let mut bound_textures = BTreeSet::new();
    let mut camera_count = 0;
    for scene in &input.scenes {
        let parents: BTreeMap<_, _> = scene
            .entities
            .iter()
            .map(|e| (e.id.as_str(), e.parent_id.as_deref()))
            .collect();
        let hidden: BTreeSet<_> = scene
            .entities
            .iter()
            .filter(|e| {
                !e.enabled
                    || matches!(&e.spatial,
            Some(RuntimeSpatial::World3D { visibility: Some(v), .. }) if !v.visible)
            })
            .map(|e| e.id.as_str())
            .collect();
        for entity in &scene.entities {
            let mut ancestor = Some(entity.id.as_str());
            let mut seen = BTreeSet::new();
            let mut visible = true;
            while let Some(id) = ancestor {
                if !seen.insert(id) {
                    return Err(invalid("scene parent cycle"));
                }
                if hidden.contains(id) {
                    visible = false;
                    break;
                }
                ancestor = parents.get(id).copied().flatten();
            }
            if !visible {
                continue;
            }
            let Some(RuntimeSpatial::World3D {
                mesh_renderer,
                camera,
                light,
                ..
            }) = &entity.spatial
            else {
                continue;
            };
            if let Some(renderer) = mesh_renderer.as_ref().filter(|r| r.visible) {
                let mesh = meshes
                    .get(renderer.mesh.0.id.as_str())
                    .ok_or_else(|| invalid("unresolved mesh"))?;
                if renderer.layers != ["world"]
                    || renderer.materials.len() != mesh.material_slots.len()
                {
                    return Err(invalid("renderer slots/layers"));
                }
                bound_meshes.insert(renderer.mesh.0.id.as_str());
                result.instances += 1;
                result.instance_triangles += u64::from(mesh.index_count / 3);
                for slot in &mesh.material_slots {
                    let reference = renderer
                        .materials
                        .get(slot)
                        .ok_or_else(|| invalid("unbound submesh slot"))?;
                    let (material, slots) = materials
                        .get(reference.0.id.as_str())
                        .ok_or_else(|| invalid("unresolved material"))?;
                    bound_materials.insert(reference.0.id.as_str());
                    result.mask_instances +=
                        usize::from(material.blend == MaterialBlend::AlphaMask);
                    result.blend_instances +=
                        usize::from(material.blend == MaterialBlend::AlphaBlend);
                    for (index, texture) in slots.iter().enumerate() {
                        let m = textures
                            .get(texture.as_str())
                            .ok_or_else(|| invalid("unresolved texture"))?;
                        let (format, space) = if index == 0 {
                            ("bc3RgbaUnormSrgb", "srgb")
                        } else {
                            ("bc3RgbaUnorm", "linear")
                        };
                        if m.format != format || m.color_space != space {
                            return Err(invalid("slot color space"));
                        }
                        bound_textures.insert(texture.as_str());
                    }
                }
            }
            if let Some(light) = light.as_ref().filter(|l| l.enabled) {
                if light.layers != ["world"] {
                    return Err(invalid("light layers"));
                }
                let index = match light.kind {
                    LightKind::Directional { .. } => 0,
                    LightKind::Point { .. } => 1,
                    LightKind::Spot { .. } => 2,
                };
                result.lights[index] += 1;
                result.shadow_lights[index] += usize::from(light.cast_shadows);
            }
            if let Some(camera) = camera.as_ref().filter(|c| c.enabled) {
                let color = camera
                    .color_pipeline
                    .as_ref()
                    .ok_or_else(|| invalid("HDR pipeline"))?;
                let ibl = color.ibl.as_ref().ok_or_else(|| invalid("IBL missing"))?;
                if scene.main_camera.as_deref() != Some(entity.id.as_str())
                    || camera.layers != ["world"]
                    || camera.projection
                        != (Projection::Perspective {
                            fov_y_degrees: 60.,
                            near: 0.1,
                            far: 2000.,
                        })
                    || color.anti_aliasing != AntiAliasing3D::Fxaa
                    || color.sky.is_none()
                    || color.fog.is_none()
                    || color.ambient_diffuse.iter().any(|v| v.to_f32() != 0.)
                    || color.exposure_ev.to_f32() != 0.
                    || ibl.intensity.to_f32() != 1.
                    || !environments.contains(ibl.environment.0.id.as_str())
                {
                    return Err(invalid("main camera/IBL/sky/fog recipe"));
                }
                camera_count += 1;
            }
        }
    }
    result.meshes = meshes.len();
    result.materials = materials.len();
    result.textures = textures.len();
    if (
        result.instances,
        result.meshes,
        result.materials,
        result.textures,
    ) != (expected.0, expected.1, expected.2, expected.3)
        || (
            bound_meshes.len(),
            bound_materials.len(),
            bound_textures.len(),
        ) != (expected.1, expected.2, expected.3)
        || result.instance_triangles != expected.0 as u64 * 1000
        || result.mask_instances != expected.0 / 10
        || result.blend_instances != expected.0 / 20
        || result.lights != [1, 128, 16]
        || result.shadow_lights != [1, 0, 8]
        || camera_count != 1
        || environments.len() != 1
    {
        return Err(invalid(format!(
            "actual cooked/bound recipe differs: {result:?}"
        )));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lit_inspector_rejects_hidden_wrong_slots_spaces_mips_lights_and_alpha() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../samples/platform_3d_lit_project");
        let mut session = crate::ProjectAuthoringSession::open(root).unwrap();
        let paths = session
            .source_inventory()
            .unwrap()
            .entries
            .into_iter()
            .map(|e| e.relative_path)
            .collect();
        let lease = session
            .acquire_snapshot_lease("lit-negative", paths)
            .unwrap();
        let prepared = crate::GameProjectCompiler::bind(&lease)
            .unwrap()
            .prepare(&lease, crate::TargetProfile::WindowsDev)
            .unwrap();
        let baseline = prepared.runtime_package_build_input();
        inspect(baseline, true).unwrap();
        for case in [
            "hidden",
            "slots",
            "space",
            "mips",
            "lights",
            "alpha",
            "shared-geometry",
        ] {
            let mut input = baseline.clone();
            match case {
                "hidden" => input.scenes[0].entities[0].enabled = false,
                "space" => input.texture_payloads[0].metadata.color_space = "wrong".into(),
                "mips" => input.texture_payloads[0].metadata.mip_count = 1,
                "lights" => {
                    for entity in &mut input.scenes[0].entities {
                        if let Some(RuntimeSpatial::World3D { light: Some(l), .. }) =
                            &mut entity.spatial
                        {
                            if l.cast_shadows {
                                l.cast_shadows = false;
                                break;
                            }
                        }
                    }
                }
                "slots" | "alpha" => {
                    let asset = input
                        .assets
                        .iter_mut()
                        .find(|a| a.asset_type == "material")
                        .unwrap();
                    let mut value: serde_json::Value =
                        serde_json::from_slice(asset.runtime_payload.as_ref().unwrap()).unwrap();
                    let m = &mut value["data"]["asset"]["payload"];
                    if case == "slots" {
                        m["textures"]
                            .as_object_mut()
                            .unwrap()
                            .remove("normalTexture");
                    } else {
                        m["blend"] = "opaque".into();
                        m["parameters"]
                            .as_object_mut()
                            .unwrap()
                            .remove("alphaCutoff");
                    }
                    asset.runtime_payload = Some(serde_json::to_vec(&value).unwrap());
                }
                "shared-geometry" => {
                    let mut assets = input.assets.iter_mut().filter(|a| a.asset_type == "mesh");
                    let a = assets.next().unwrap().runtime_payload.clone();
                    assets.next().unwrap().runtime_payload = a;
                }
                _ => unreachable!(),
            }
            assert!(inspect(&input, true).is_err(), "accepted {case}");
        }
    }
}
