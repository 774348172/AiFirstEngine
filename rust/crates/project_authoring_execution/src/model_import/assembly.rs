use super::*;
use crate::three_d_contract::{ImportOutput, ImportOutputKind};
use engine_runtime::render_contract::*;
use engine_runtime::runtime_package::{CookedTextureAsset, COOKED_TEXTURE_SCHEMA_VERSION};
use engine_runtime::runtime_package_builder::RuntimePackageSourceTexture;
use engine_runtime::spatial_contract::*;

#[path = "mesh.rs"]
mod mesh;

/// Compiler products. IDs are taken exclusively from committed mappings.
#[derive(Serialize, Deserialize)]
pub(crate) struct CompiledImport {
    pub assets: Vec<(ImportOutput, RenderAssetData)>,
    pub textures: Vec<(ImportOutput, RuntimePackageSourceTexture)>,
    pub character: Option<(
        ImportOutput,
        engine_runtime::character_animation::CharacterAsset,
    )>,
    pub prefab: (ImportOutput, Value),
    pub source_digest: String,
    pub report: ModelImportDecodeReport,
}

pub(crate) fn compile_sources(source: &CompilerSourceView) -> Result<Vec<CompiledImport>> {
    decode_sources(source)?
        .into_iter()
        .map(|(settings, decoded)| compile(settings, decoded))
        .collect()
}

pub(super) fn fingerprint(json: &Value, locator: &str) -> Option<String> {
    let value = if let Some(scene) = locator.strip_suffix("/character") {
        json.pointer(scene)?
    } else if locator == "/materials/default" {
        &Value::Null
    } else {
        json.pointer(locator)?
    };
    Some(sha256_prefixed(
        &serde_json::to_vec(value).expect("source JSON"),
    ))
}

pub(super) fn compile(
    settings: ModelImportV1,
    mut decoded: DecodedModel,
) -> Result<CompiledImport> {
    let descriptor = decoded.report.descriptor_path.clone();
    let mapping_error =
        |path: &str, message: String| fail(&descriptor, path, "mapping_invalid", message);
    let mut outputs = BTreeMap::new();
    for (i, output) in settings.outputs.iter().enumerate() {
        let path = format!("/outputs/{i}");
        if outputs
            .insert(output.source_locator.clone(), output)
            .is_some()
        {
            return Err(mapping_error(
                &path,
                "Each source locator must have exactly one committed output.".into(),
            ));
        }
        if fingerprint(&decoded.json, &output.source_locator).as_deref()
            != Some(&output.source_fingerprint)
        {
            return Err(mapping_error(&format!("{path}/sourceFingerprint"),format!("Source locator changed or is absent; review its mapping. Current fingerprint: {:?}.",fingerprint(&decoded.json,&output.source_locator))));
        }
    }
    let mut used = BTreeSet::new();
    let mut required = |locator: &str, kind: ImportOutputKind| -> Result<ImportOutput> {
        let output = outputs.get(locator).ok_or_else(|| {
            mapping_error(
                "/outputs",
                format!("Missing committed {} mapping for {locator}.", kind.as_str()),
            )
        })?;
        if output.kind != kind {
            return Err(mapping_error(
                "/outputs",
                format!("{locator} requires {} output.", kind.as_str()),
            ));
        }
        used.insert(locator.to_string());
        Ok((*output).clone())
    };
    let conversion = coordinates::Conversion::new(&settings.coordinates);
    let normalized_bytes = decoded.report.vertices * 48
        + decoded.report.indices * 4
        + decoded
            .images
            .values()
            .map(|image| image.rgba8.len() as u64)
            .sum::<u64>();
    if normalized_bytes > settings.budgets.cooked_bytes {
        return Err(fail(
            &settings.source,
            "",
            "import_cooked_budget_exceeded",
            "Normalized geometry and images exceed cookedBytes before output allocation.",
        ));
    }
    let mut assets = Vec::new();
    let mut textures = Vec::new();
    let material_indices: BTreeSet<_> = decoded
        .primitives
        .iter()
        .map(|p| p.material_index)
        .collect();
    let mut material_refs = BTreeMap::new();
    for index in material_indices {
        let material = match index {
            Some(index) => decoded.document.materials().nth(index).unwrap(),
            None => decoded
                .document
                .meshes()
                .flat_map(|m| m.primitives())
                .find(|p| p.material().index().is_none())
                .unwrap()
                .material(),
        };
        let locator = index.map_or_else(
            || "/materials/default".into(),
            |i| format!("/materials/{i}"),
        );
        let output = required(&locator, ImportOutputKind::Material)?;
        let cooked = materials::lower(material, &settings, &outputs, &mut decoded.report)?;
        if cooked.preset == MaterialPreset::StandardLit {
            for primitive in decoded
                .primitives
                .iter()
                .filter(|p| p.material_index == index)
            {
                primitive
                    .geometry
                    .validate_standard_lit_inputs(
                        0..primitive.geometry.index_count,
                        cooked.textures.contains_key("normalTexture"),
                        !cooked.textures.is_empty(),
                    )
                    .map_err(|e| {
                        fail(
                            &settings.source,
                            &locator,
                            "material.geometry_invalid",
                            &e.to_string(),
                        )
                    })?;
            }
        }
        for reference in cooked.textures.values() {
            let image_output = settings
                .outputs
                .iter()
                .find(|o| o.asset_id == reference.reference.id)
                .unwrap();
            required(&image_output.source_locator, ImportOutputKind::Texture)?;
        }
        material_refs.insert(
            index,
            materials::reference(&output, RenderAssetKind::Material).reference,
        );
        assets.push((output, RenderAssetData::Material(cooked)));
    }
    let mut mesh_renderers = BTreeMap::new();
    let mut meshes: BTreeMap<usize, Vec<&DecodedPrimitive>> = BTreeMap::new();
    for primitive in &decoded.primitives {
        meshes
            .entry(primitive.mesh_index)
            .or_default()
            .push(primitive);
    }
    for (index, primitives) in meshes {
        let output = required(&format!("/meshes/{index}"), ImportOutputKind::Mesh)?;
        let geometry = mesh::merge(&settings, &output, &primitives, &conversion)?;
        let mut materials = BTreeMap::new();
        let slots: BTreeMap<_, _> = output
            .material_slots
            .iter()
            .map(|slot| (slot.source_locator.as_str(), slot))
            .collect();
        for p in &primitives {
            let locator = format!("/meshes/{index}/primitives/{}", p.primitive_index);
            let slot = slots[locator.as_str()];
            materials.insert(
                slot.name.clone(),
                SpatialAssetRef(material_refs[&p.material_index].clone()),
            );
        }
        mesh_renderers.insert(
            index,
            MeshRenderer3D {
                schema_version: "mesh-renderer3d.v1".into(),
                mesh: SpatialAssetRef(
                    materials::reference(&output, RenderAssetKind::Mesh).reference,
                ),
                materials,
                visible: true,
                layers: vec!["world".into()],
            },
        );
        assets.push((output, RenderAssetData::Mesh(geometry)));
    }
    let prefab_output = required(
        &format!("/scenes/{}", settings.scene),
        ImportOutputKind::Prefab,
    )?;
    let character = if let Some(mut asset) = decoded.character.take() {
        let output = required(
            &format!("/scenes/{}/character", settings.scene),
            ImportOutputKind::Character,
        )?;
        asset.asset_id = output.asset_id.clone();
        for skin in &mut asset.skins {
            let node_index = decoded.nodes[usize::from(skin.mesh_joint)];
            let mesh_index = decoded
                .document
                .nodes()
                .nth(node_index)
                .unwrap()
                .mesh()
                .unwrap()
                .index();
            skin.mesh_asset_id = outputs[&format!("/meshes/{mesh_index}")].asset_id.clone();
        }
        asset.validate().map_err(|error| ImportFailure {
            source_path: settings.source.clone(),
            error,
        })?;
        Some((output, asset))
    } else {
        None
    };
    // Images are emitted only when supported material semantics use them;
    // decoding still validates the complete consumed dependency closure.
    for (index, image) in decoded.images {
        let locator = format!("/images/{index}");
        if used.contains(&locator) {
            let output = outputs[&locator];
            let mut texture = texture_payload(output, image);
            if settings.texture_compression.as_deref() == Some("bc3") {
                let mut spaces = BTreeSet::new();
                for (_, asset) in &assets {
                    if let RenderAssetData::Material(material) = asset {
                        for (slot, reference) in &material.textures {
                            if reference.reference.id == output.asset_id {
                                spaces.insert(matches!(
                                    slot.as_str(),
                                    "baseColorTexture" | "emissiveTexture"
                                ));
                            }
                        }
                    }
                }
                if spaces.len() != 1 {
                    return Err(fail(&settings.source, &output.source_locator, "texture_compression_color_conflict", "Compressed images used as both color and data require separate authored image outputs."));
                }
                texture =
                    crate::neutral_assembler::texture_cook::bc3(texture, *spaces.first().unwrap())
                        .map_err(|message| {
                            fail(
                                &settings.source,
                                &output.source_locator,
                                "texture_compression_invalid",
                                message,
                            )
                        })?;
            }
            textures.push((output.clone(), texture));
        }
    }
    if used.len() != outputs.len() {
        return Err(mapping_error(
            "/outputs",
            "Mapping contains outputs outside the selected supported scene closure.".into(),
        ));
    }
    let character_ref = character.as_ref().map(|(output,_)| serde_json::json!({"id":output.asset_id,"guid":output.asset_guid,"type":"character3d"}));
    let mut nodes = BTreeMap::new();
    for (i, mapping) in settings.nodes.iter().enumerate() {
        if nodes
            .insert(mapping.source_locator.clone(), mapping)
            .is_some()
            || fingerprint(&decoded.json, &mapping.source_locator).as_deref()
                != Some(&mapping.source_fingerprint)
        {
            return Err(mapping_error(&format!("/nodes/{i}"),format!("Node locator must be unique and its fingerprint must match; current fingerprint: {:?}.",fingerprint(&decoded.json,&mapping.source_locator))));
        }
    }
    if nodes.len() != decoded.nodes.len()
        || decoded
            .nodes
            .iter()
            .any(|i| !nodes.contains_key(&format!("/nodes/{i}")))
    {
        return Err(mapping_error(
            "/nodes",
            "Every selected node requires exactly one committed mapping, without unused nodes."
                .into(),
        ));
    }
    let root_id = format!("{}:root", prefab_output.key);
    if settings.nodes.iter().any(|n| n.entity_id == root_id) {
        return Err(mapping_error(
            "/nodes/entityId",
            "Node identity collides with the prefab's stable synthetic root.".into(),
        ));
    }
    let spatial = |transform, mesh_renderer| RuntimeSpatial::World3D {
        transform,
        visibility: None,
        mesh_renderer,
        camera: None,
        light: None,
    };
    let mut entities = vec![RuntimeEntityDocument::Spatial(RuntimeEntityV2 {
        schema_version: RUNTIME_ENTITY_V2.into(),
        id: root_id.clone(),
        name: settings.display_name.clone(),
        enabled: true,
        parent_id: None,
        sibling_order: 0,
        spatial: spatial(Transform3D::identity().canonicalize().unwrap(), None),
        components: character_ref.as_ref().map(|reference| vec![ProjectDataComponent {component_type:"engine.character3d".into(),data:serde_json::json!({"schemaVersion":"character3d-binding.v1","character":reference,"joints":decoded.nodes.iter().map(|i|serde_json::json!({"entityRef":nodes[&format!("/nodes/{i}")].entity_id})).collect::<Vec<_>>(),"graph":null,"playOnAwake":false})}]).unwrap_or_default(),
    })];
    let mut parents = BTreeMap::new();
    for index in &decoded.nodes {
        let node = decoded.document.nodes().nth(*index).unwrap();
        for (order, child) in node.children().enumerate() {
            parents.insert(
                child.index(),
                (
                    nodes[&format!("/nodes/{index}")].entity_id.clone(),
                    order as i32,
                ),
            );
        }
    }
    for (order, node) in decoded
        .document
        .scenes()
        .nth(settings.scene as usize)
        .unwrap()
        .nodes()
        .enumerate()
    {
        parents.insert(node.index(), (root_id.clone(), order as i32));
    }
    for index in &decoded.nodes {
        let node = decoded.document.nodes().nth(*index).unwrap();
        let mapping = nodes[&format!("/nodes/{index}")];
        let (parent, order) = &parents[index];
        entities.push(RuntimeEntityDocument::Spatial(RuntimeEntityV2 {
            schema_version: RUNTIME_ENTITY_V2.into(),
            id: mapping.entity_id.clone(),
            name: node.name().unwrap_or(&mapping.key).into(),
            enabled: true,
            parent_id: Some(parent.clone()),
            sibling_order: *order,
            spatial: spatial(
                conversion.node(node.clone(), &settings.source)?,
                node.mesh().map(|m| mesh_renderers[&m.index()].clone()),
            ),
            components: Vec::new(),
        }));
    }
    let mut prefab = RuntimePrefabV2 {
        schema_version: RUNTIME_PREFAB_V2.into(),
        id: prefab_output.asset_id.clone(),
        name: settings.display_name.clone(),
        root_entity_id: root_id,
        entities,
    };
    prefab
        .validate_and_canonicalize()
        .map_err(|error| ImportFailure {
            source_path: descriptor.clone(),
            error,
        })?;
    // Keep the same JSON value passed to RuntimePackageBuilder; its pretty writer
    // must not diverge from the import payload budget (including float encoding).
    let prefab = serde_json::to_value(prefab).expect("validated prefab");
    let source_digest = sha256_prefixed(
        &serde_json::to_vec(&(
            IMPORTED_RENDER_COOK_RECIPE,
            &decoded.report.source_digest,
            &settings,
        ))
        .unwrap(),
    );
    // Count the actual serialized payload, not merely its raw vertex storage.
    // This writer allocates no output and fails before the byte ceiling is crossed.
    let mut budget = PayloadBudget {
        used: 0,
        limit: settings.budgets.cooked_bytes,
    };
    if let Some((_, asset)) = &character {
        serde_json::to_writer(&mut budget, asset).map_err(|e| {
            fail(
                &settings.source,
                "",
                "import_cooked_budget_exceeded",
                e.to_string(),
            )
        })?;
    }
    for (_, asset) in &assets {
        serde_json::to_writer(
            &mut budget,
            &RenderContract::new(BorrowedCookedAsset {
                source_digest: &source_digest,
                recipe: IMPORTED_RENDER_COOK_RECIPE,
                asset,
            }),
        )
        .map_err(|e| {
            fail(
                &settings.source,
                "",
                "import_cooked_budget_exceeded",
                e.to_string(),
            )
        })?;
    }
    serde_json::to_writer_pretty(&mut budget, &prefab).map_err(|e| {
        fail(
            &settings.source,
            "",
            "import_cooked_budget_exceeded",
            e.to_string(),
        )
    })?;
    for (_, texture) in &textures {
        serde_json::to_writer_pretty(&mut budget, &texture.metadata).map_err(|e| {
            fail(
                &settings.source,
                "",
                "import_cooked_budget_exceeded",
                e.to_string(),
            )
        })?;
        budget.charge(texture.rgba8.len()).map_err(|e| {
            fail(
                &settings.source,
                "",
                "import_cooked_budget_exceeded",
                e.to_string(),
            )
        })?;
    }
    decoded.report.reserved_cooked_bytes = budget.used;
    decoded.report.stage = "assembly".into();
    decoded.report.assembly_available = true;
    decoded.report.recipe = IMPORTED_RENDER_COOK_RECIPE.into();
    decoded.report.geometry_digests = assets
        .iter()
        .filter_map(|(_, a)| match a {
            RenderAssetData::Mesh(m) => Some(sha256_prefixed(&serde_json::to_vec(m).unwrap())),
            _ => None,
        })
        .collect();
    Ok(CompiledImport {
        character,
        assets,
        textures,
        prefab: (prefab_output, prefab),
        source_digest,
        report: decoded.report,
    })
}

fn texture_payload(output: &ImportOutput, image: DecodedImage) -> RuntimePackageSourceTexture {
    RuntimePackageSourceTexture {
        metadata: CookedTextureAsset {
            schema_version: COOKED_TEXTURE_SCHEMA_VERSION.into(),
            asset_id: output.asset_id.clone(),
            cooked_asset_id: format!("cooked-{}", output.asset_id),
            width: image.width,
            height: image.height,
            format: "rgba8Unorm".into(),
            color_space: "srgb".into(),
            mip_count: 1,
            byte_length: image.rgba8.len(),
            pixel_data_path: format!("cooked/textures/{}.rgba", output.asset_id),
            sampler: "linear:linear:repeat:repeat".into(),
            source_hash: sha256_prefixed(&image.rgba8),
        },
        rgba8: image.rgba8.into(),
    }
}

struct PayloadBudget {
    used: u64,
    limit: u64,
}
// Same CookedRenderAsset wire shape, borrowed while measuring; never clone large buffers.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BorrowedCookedAsset<'a> {
    source_digest: &'a str,
    recipe: &'a str,
    asset: &'a RenderAssetData,
}
impl PayloadBudget {
    fn charge(&mut self, bytes: usize) -> std::io::Result<()> {
        self.used = self
            .used
            .checked_add(bytes as u64)
            .filter(|n| *n <= self.limit)
            .ok_or_else(|| {
                std::io::Error::other("Serialized import payload exceeds cookedBytes.")
            })?;
        Ok(())
    }
}
impl std::io::Write for PayloadBudget {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.charge(bytes.len())?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
