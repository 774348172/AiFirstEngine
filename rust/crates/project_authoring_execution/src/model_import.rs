//! Build-side, snapshot-only glTF decoding. Stable asset publication belongs to the
//! existing assembler; this intermediate does not allocate GUIDs or write files.
use crate::game_project_compiler::{CompilerSourceView, SourceLocation};
use crate::three_d_contract::{ImportBudgets, ModelImportV1};
use engine_runtime::canonical_digest::sha256_prefixed;
use engine_runtime::render_contract::CookedGeometry;
use engine_runtime::spatial_contract::{decode, strict_json, SpatialContractError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

mod assembly;
mod bindings;
mod cache;
pub(crate) use cache::compile_cached;
mod character;
mod coordinates;
mod geometry;
mod materials;
mod planning;
pub use planning::*;
mod source;
pub(crate) use assembly::compile_sources;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSourceDependency {
    pub locator: String,
    pub source_path: String,
    pub byte_length: u64,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportDecodeNotice {
    pub source_path: String,
    pub field_path: String,
    pub outcome: String,
    pub message: String,
}

/// Build-side import evidence; check never qualifies runtime presentation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelImportDecodeReport {
    pub descriptor_path: String,
    pub source_path: String,
    pub stage: String,
    pub status: String,
    pub assembly_available: bool,
    pub recipe: String,
    pub source_digest: String,
    pub dependencies: Vec<ImportSourceDependency>,
    pub consumed: BTreeSet<String>,
    pub unconsumed: BTreeSet<String>,
    pub notices: Vec<ImportDecodeNotice>,
    pub vertices: u64,
    pub indices: u64,
    pub decoded_image_bytes: u64,
    pub source_bytes: u64,
    pub reserved_cooked_bytes: u64,
    pub geometry_digests: Vec<String>,
    pub image_digests: BTreeMap<usize, String>,
}

#[derive(Debug)]
pub(super) struct ImportFailure {
    pub source_path: String,
    pub error: SpatialContractError,
}
type Result<T> = std::result::Result<T, ImportFailure>;

fn fail(source: &str, field: &str, code: &str, message: impl Into<String>) -> ImportFailure {
    ImportFailure {
        source_path: source.into(),
        error: SpatialContractError::new(code, field, message),
    }
}

pub(super) fn compiler_error(failure: ImportFailure) -> crate::GameProjectCompilerError {
    crate::game_project_compiler::check::error(
        &failure.error.code,
        failure.error.message,
        Some(SourceLocation {
            source_path: failure.source_path,
            field_path: Some(failure.error.field_path),
            line: None,
            column: None,
            generated: false,
        }),
    )
}

pub(super) fn check_sources(source: &CompilerSourceView) -> Result<Vec<ModelImportDecodeReport>> {
    let imports = compile_sources(source)?;
    bindings::validate(source, &imports)?;
    Ok(imports.into_iter().map(|i| i.report).collect())
}

fn decode_sources(source: &CompilerSourceView) -> Result<Vec<(ModelImportV1, DecodedModel)>> {
    let mut reports = Vec::new();
    for path in source
        .paths()
        .filter(|p| p.starts_with("Assets/") && (p.ends_with(".json") || p.ends_with(".asset")))
    {
        let value: Value = serde_json::from_slice(source.bytes(path).unwrap())
            .map_err(|e| fail(path, "", "schema_invalid", e.to_string()))?;
        if value["schemaVersion"] != "model-import.v1" {
            continue;
        }
        let descriptor: ModelImportV1 = decode(value, "").map_err(|error| ImportFailure {
            source_path: path.into(),
            error,
        })?;
        let decoded = decode_model(source, path, &descriptor)?;
        reports.push((descriptor, decoded));
    }
    Ok(reports)
}

#[derive(Debug)]
pub(super) struct DecodedPrimitive {
    pub mesh_index: usize,
    pub primitive_index: usize,
    pub material_index: Option<usize>,
    pub geometry: CookedGeometry,
}
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub rgba8: Vec<u8>,
}
#[derive(Debug)]
pub(super) struct DecodedModel {
    pub document: gltf::Document,
    pub json: Value,
    pub nodes: Vec<usize>,
    pub primitives: Vec<DecodedPrimitive>,
    pub images: BTreeMap<usize, DecodedImage>,
    pub character: Option<engine_runtime::character_animation::CharacterAsset>,
    pub report: ModelImportDecodeReport,
}

struct Budget<'a> {
    limits: &'a ImportBudgets,
    source: &'a str,
    source_bytes: u64,
    vertices: u64,
    indices: u64,
    image_bytes: u64,
    cooked_bytes: u64,
}
fn charge(
    current: &mut u64,
    amount: u64,
    maximum: u64,
    source: &str,
    field: &str,
    code: &str,
) -> Result<()> {
    let next = current.checked_add(amount).filter(|v| *v <= maximum)
        .ok_or_else(|| fail(source, field, code, format!("Import budget exceeded: {amount} additional, {current} used, {maximum} allowed.")))?;
    *current = next;
    Ok(())
}

pub(super) fn decode_model(
    source: &CompilerSourceView,
    descriptor_path: &str,
    settings: &ModelImportV1,
) -> Result<DecodedModel> {
    decode_model_cached(
        source,
        descriptor_path,
        settings,
        &mut cache::ImportCache::disabled(),
    )
}

fn decode_model_cached(
    source: &CompilerSourceView,
    descriptor_path: &str,
    settings: &ModelImportV1,
    cache: &mut cache::ImportCache<'_>,
) -> Result<DecodedModel> {
    cache.check_cancelled(descriptor_path)?;
    settings.validate().map_err(|error| ImportFailure {
        source_path: descriptor_path.into(),
        error,
    })?;
    let bytes = source.bytes(&settings.source).ok_or_else(|| {
        fail(
            descriptor_path,
            "/source",
            "model_source_missing",
            "Model is absent from the operation snapshot.",
        )
    })?;
    let mut budget = Budget {
        limits: &settings.budgets,
        source: &settings.source,
        source_bytes: 0,
        vertices: 0,
        indices: 0,
        image_bytes: 0,
        cooked_bytes: 0,
    };
    charge(
        &mut budget.source_bytes,
        bytes.len() as u64,
        settings.budgets.source_bytes,
        &settings.source,
        "",
        "source_budget_exceeded",
    )?;
    let (json_bytes, bin) = source::container(bytes, &settings.source)?;
    let json = strict_json(json_bytes).map_err(|error| ImportFailure {
        source_path: settings.source.clone(),
        error,
    })?;
    source::version(&json, &settings.source)?;
    // Bound the complete document's typed node allocation, including unselected
    // nodes. Source bytes already bound the initial JSON parse; traversal below
    // separately bounds the selected hierarchy depth and consumed closure.
    if json["nodes"].as_array().map_or(0, Vec::len) as u64 > settings.budgets.nodes {
        return Err(fail(
            &settings.source,
            &format!("/nodes/{}", settings.budgets.nodes),
            "import_node_budget_exceeded",
            "Document node count exceeds the import budget before typed parsing.",
        ));
    }
    // Structural references are checked by the mature parser before typed iterators
    // are used. Buffer spans and supported semantics still require our checks.
    let root = serde_json::from_value(json.clone())
        .map_err(|e| fail(&settings.source, "", "import_schema_invalid", e.to_string()))?;
    let document = gltf::Document::from_json(root).map_err(|e| {
        let field = match &e {
            gltf::Error::Validation(errors) => errors
                .first()
                .map(|(path, _)| {
                    format!(
                        "/{}",
                        path.as_str()
                            .split(" = ")
                            .next()
                            .unwrap_or_default()
                            .replace("[\"", "/")
                            .replace("\"]", "")
                            .replace('[', "/")
                            .replace(']', "")
                            .replace('.', "/")
                    )
                })
                .unwrap_or_default(),
            _ => String::new(),
        };
        fail(
            &settings.source,
            &field,
            "import_schema_invalid",
            e.to_string(),
        )
    })?;
    let scene = document
        .scenes()
        .nth(settings.scene as usize)
        .ok_or_else(|| {
            fail(
                &settings.source,
                "/scenes",
                "import_scene_missing",
                "Selected scene does not exist.",
            )
        })?;
    let mut consumed = BTreeSet::from([format!("/scenes/{}", settings.scene)]);
    let mut nodes = Vec::new();
    let mut meshes = BTreeSet::new();
    let mut seen = BTreeSet::new();
    for node in scene.nodes() {
        visit(
            node,
            1,
            &mut seen,
            &mut nodes,
            &mut meshes,
            &mut consumed,
            &budget,
        )?;
    }
    let mut notices = Vec::new();
    let mut materials = BTreeSet::new();
    let mut textures = BTreeSet::new();
    let mut images = BTreeSet::new();
    let mut buffer_indices = BTreeSet::new();
    for mesh_index in meshes {
        let mesh = document.meshes().nth(mesh_index).unwrap();
        for primitive in mesh.primitives() {
            let path = format!("/meshes/{mesh_index}/primitives/{}", primitive.index());
            consumed.insert(path.clone());
            geometry::validate_primitive(&primitive, &path, &mut budget)?;
            if settings.material_strategy
                == crate::three_d_contract::ImportMaterialStrategy::StandardLit
                && !primitive.material().unlit()
                && primitive.get(&gltf::Semantic::Normals).is_none()
            {
                return Err(fail(
                    &settings.source,
                    &path,
                    "material.normal_required",
                    "StandardLit requires authored normals; no fabricated +Z normals.",
                ));
            }
            let material = primitive.material();
            let pbr = material.pbr_metallic_roughness();
            if settings.material_strategy
                == crate::three_d_contract::ImportMaterialStrategy::StandardLit
                && !material.unlit()
                && (pbr.base_color_texture().is_some()
                    || pbr.metallic_roughness_texture().is_some()
                    || material.normal_texture().is_some()
                    || material.occlusion_texture().is_some()
                    || material.emissive_texture().is_some())
                && primitive.get(&gltf::Semantic::TexCoords(0)).is_none()
            {
                return Err(fail(
                    &settings.source,
                    &path,
                    "material.uv_required",
                    "Textured StandardLit requires authored TEXCOORD_0; no fabricated zero UVs.",
                ));
            }
            if let Some(index) = primitive.material().index() {
                materials.insert(index);
            }
            for (_, accessor) in primitive
                .attributes()
                .chain(primitive.indices().map(|a| (gltf::Semantic::Positions, a)))
            {
                let view = geometry::validate_accessor(&accessor, &budget)?;
                consumed.insert(format!("/accessors/{}", accessor.index()));
                consumed.insert(format!("/bufferViews/{}", view.index()));
                buffer_indices.insert(view.buffer().index());
            }
        }
    }
    for index in materials {
        consumed.insert(format!("/materials/{index}"));
        let material = document.materials().nth(index).unwrap();
        let pbr = material.pbr_metallic_roughness();
        textures.extend(pbr.base_color_texture().map(|t| t.texture().index()));
        textures.extend(
            pbr.metallic_roughness_texture()
                .map(|t| t.texture().index()),
        );
        textures.extend(material.normal_texture().map(|t| t.texture().index()));
        textures.extend(material.occlusion_texture().map(|t| t.texture().index()));
        textures.extend(material.emissive_texture().map(|t| t.texture().index()));
    }
    for index in textures {
        consumed.insert(format!("/textures/{index}"));
        let texture = document.textures().nth(index).unwrap();
        images.insert(texture.source().index());
        if let Some(index) = texture.sampler().index() {
            consumed.insert(format!("/samplers/{index}"));
        }
    }
    for &index in &images {
        consumed.insert(format!("/images/{index}"));
        if let gltf::image::Source::View { view, .. } =
            document.images().nth(index).unwrap().source()
        {
            consumed.insert(format!("/bufferViews/{}", view.index()));
            buffer_indices.insert(view.buffer().index());
        }
    }
    character::collect_dependencies(
        &document,
        &seen,
        &mut consumed,
        &mut buffer_indices,
        &mut budget,
    )?;
    for &index in &buffer_indices {
        consumed.insert(format!("/buffers/{index}"));
    }
    source::extensions(&json, "", &consumed, &settings.source, &mut notices)?;
    for &index in &nodes {
        if document.nodes().nth(index).unwrap().camera().is_some() {
            notices.push(ImportDecodeNotice {
                source_path: settings.source.clone(),
                field_path: format!("/nodes/{index}/camera"),
                outcome: "notImported".into(),
                message: "Source camera does not replace the project's explicit main camera."
                    .into(),
            });
        }
    }
    let mut dependencies = vec![ImportSourceDependency {
        locator: "".into(),
        source_path: settings.source.clone(),
        byte_length: bytes.len() as u64,
        digest: sha256_prefixed(bytes),
    }];
    let mut source_cache =
        source::SourceBytes::new(source, &settings.source, bin, &mut dependencies);
    let mut buffers = BTreeMap::new();
    for index in buffer_indices {
        let buffer = document.buffers().nth(index).unwrap();
        let payload = source_cache.buffer(&buffer, &mut budget)?;
        buffers.insert(index, payload);
    }
    let mut character = character::decode(&document, &nodes, &buffers, settings, &mut budget)?;
    let mut primitives = Vec::new();
    for mesh in document
        .meshes()
        .filter(|m| consumed.contains(&format!("/meshes/{}", m.index())))
    {
        for primitive in mesh.primitives() {
            cache.check_cancelled(&settings.source)?;
            primitives.push(DecodedPrimitive {
                mesh_index: mesh.index(),
                primitive_index: primitive.index(),
                material_index: primitive.material().index(),
                geometry: cache.geometry(
                    &json,
                    &primitive,
                    &buffers,
                    &settings.source,
                    &format!("/meshes/{}/primitives/{}", mesh.index(), primitive.index()),
                )?,
            });
        }
    }
    let mut decoded_images = BTreeMap::new();
    for index in images {
        cache.check_cancelled(&settings.source)?;
        let image = document.images().nth(index).unwrap();
        let payload = source_cache.image(&image, &buffers, &mut budget)?;
        decoded_images.insert(
            index,
            cache.image(
                &payload,
                image.source(),
                &format!("/images/{index}"),
                &mut budget,
            )?,
        );
    }
    let mut unconsumed = BTreeSet::new();
    for kind in [
        "scenes",
        "nodes",
        "meshes",
        "materials",
        "textures",
        "images",
        "samplers",
        "buffers",
        "bufferViews",
        "accessors",
        "animations",
        "skins",
        "cameras",
    ] {
        for index in 0..json[kind].as_array().map_or(0, Vec::len) {
            let path = format!("/{kind}/{index}");
            if !consumed.contains(&path) {
                unconsumed.insert(path);
            }
        }
    }
    let source_digest =
        sha256_prefixed(&serde_json::to_vec(&dependencies).expect("dependency records"));
    let report = ModelImportDecodeReport {
        descriptor_path: descriptor_path.into(),
        source_path: settings.source.clone(),
        stage: "decode".into(),
        status: if notices.iter().any(|n| n.outcome == "ignored") {
            "degraded"
        } else {
            "accepted"
        }
        .into(),
        assembly_available: false,
        recipe: "gltf-static.decode.v1".into(),
        source_digest,
        dependencies,
        consumed,
        unconsumed,
        notices,
        vertices: budget.vertices,
        indices: budget.indices,
        decoded_image_bytes: budget.image_bytes,
        source_bytes: budget.source_bytes,
        reserved_cooked_bytes: budget.cooked_bytes,
        geometry_digests: primitives
            .iter()
            .map(|p| {
                sha256_prefixed(
                    &serde_json::to_vec(&(
                        p.mesh_index,
                        p.primitive_index,
                        p.material_index,
                        p.geometry.vertex_stride,
                        &p.geometry.attributes,
                        p.geometry.index_format,
                        sha256_prefixed(&p.geometry.vertex_bytes),
                        sha256_prefixed(&p.geometry.index_bytes),
                    ))
                    .expect("geometry identity"),
                )
            })
            .collect(),
        image_digests: decoded_images
            .iter()
            .map(|(i, p)| {
                (
                    *i,
                    sha256_prefixed(
                        &serde_json::to_vec(&(p.width, p.height, sha256_prefixed(&p.rgba8)))
                            .expect("image identity"),
                    ),
                )
            })
            .collect(),
    };
    if let Some(asset) = &mut character {
        asset.source_digest = report.source_digest.clone();
    }
    Ok(DecodedModel {
        document,
        json,
        nodes,
        primitives,
        images: decoded_images,
        character,
        report,
    })
}

fn visit(
    node: gltf::Node<'_>,
    depth: u64,
    seen: &mut BTreeSet<usize>,
    nodes: &mut Vec<usize>,
    meshes: &mut BTreeSet<usize>,
    consumed: &mut BTreeSet<String>,
    budget: &Budget<'_>,
) -> Result<()> {
    let path = format!("/nodes/{}", node.index());
    if depth > budget.limits.depth || nodes.len() as u64 >= budget.limits.nodes {
        return Err(fail(
            budget.source,
            &path,
            "import_node_budget_exceeded",
            "Selected node count or hierarchy depth exceeds the import budget.",
        ));
    }
    if !seen.insert(node.index()) {
        return Err(fail(
            budget.source,
            &path,
            "import_hierarchy_invalid",
            "Selected scene must be a tree without cycles or multiply parented nodes.",
        ));
    }
    nodes.push(node.index());
    consumed.insert(path);
    if let Some(mesh) = node.mesh() {
        meshes.insert(mesh.index());
        consumed.insert(format!("/meshes/{}", mesh.index()));
    }
    for child in node.children() {
        visit(child, depth + 1, seen, nodes, meshes, consumed, budget)?;
    }
    Ok(())
}
