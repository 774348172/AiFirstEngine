use super::*;
use crate::game_project_compiler::CompilerSourceView;
use engine_runtime::render_contract::{
    RenderAssetData, RenderAssetReference, RenderAssetResolution, RenderContract,
    ValidateRenderContract,
};
use engine_runtime::runtime_asset::{RuntimeAssetIndex, RuntimeAssetRecord};
use std::collections::BTreeMap;

pub(crate) struct SourceContractFailure {
    pub source_path: String,
    pub error: SpatialContractError,
}
fn at(path: &str, error: SpatialContractError) -> SourceContractFailure {
    SourceContractFailure {
        source_path: path.into(),
        error,
    }
}

pub(crate) fn validate_source(
    source: &CompilerSourceView,
    project_schema: &str,
) -> Result<(), SourceContractFailure> {
    let expanded = crate::neutral_assembler::terrain_cook::expand(source).map_err(|e| {
        at(
            "Assets/",
            SpatialContractError::new("terrain3d.cook_invalid", "/", format!("{e:?}")),
        )
    })?;
    let source = &expanded;
    let v3 = project_schema == crate::SPATIAL_PROJECT_MANIFEST_SCHEMA_VERSION;
    let mut entities = Vec::new();
    let mut materials = Vec::new();
    let mut records = Vec::new();
    let mut slots = BTreeMap::new();
    let mut ids = BTreeSet::new();
    let mut guids = BTreeSet::new();
    for path in source.paths().filter(|p| {
        ["Scenes/", "Prefabs/", "Assets/"]
            .iter()
            .any(|prefix| p.starts_with(prefix))
            && (p.ends_with(".json") || p.ends_with(".asset"))
    }) {
        let bytes = source.bytes(path).unwrap();
        let value: Value = serde_json::from_slice(bytes).map_err(|e| {
            at(
                path,
                SpatialContractError::new("schema_invalid", "", e.to_string()),
            )
        })?;
        let schema = value
            .get("schemaVersion")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let is_new = [
            AUTHORING_SCENE_V2,
            AUTHORING_PREFAB_V2,
            "mesh-asset.v2",
            "material-asset.v2",
            "model-import.v1",
            "environment-asset.v1",
        ]
        .contains(&schema);
        if is_new && !v3 {
            return Err(at(path,SpatialContractError::new("project_upgrade_required", "/schemaVersion", "New spatial sources require aife-project.v3; old compilers must reject them at project open.")));
        }
        let value = if is_new {
            strict_json(bytes).map_err(|e| at(path, e))?
        } else {
            value
        };
        let schema = value
            .get("schemaVersion")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let outcome = (|| -> ContractResult<()> {
            match schema {
                AUTHORING_SCENE_V2 => {
                    if !path.starts_with("Scenes/") {
                        return Err(SpatialContractError::new(
                            "source_domain_invalid",
                            "/schemaVersion",
                            "Scene contracts belong in Scenes/.",
                        ));
                    }
                    let scene: SpatialSourceScene = decode(value.clone(), "")?;
                    scene.canonicalize_placements()?;
                    entities.push((path, scene.entities));
                }
                AUTHORING_PREFAB_V2 => {
                    if !path.starts_with("Prefabs/") {
                        return Err(SpatialContractError::new(
                            "source_domain_invalid",
                            "/schemaVersion",
                            "Prefab contracts belong in Prefabs/.",
                        ));
                    }
                    let prefab: SpatialSourcePrefab = decode(value.clone(), "")?;
                    prefab.canonicalize()?;
                    entities.push((path, prefab.entities));
                }
                "mesh-asset.v2" => {
                    let mesh: MeshAssetV2 = decode(value.clone(), "")?;
                    mesh.validate()?;
                    slots.insert(mesh.asset_guid.clone(), mesh.material_slots());
                }
                "material-asset.v2" => {
                    let material: MaterialAssetV2 = decode(value.clone(), "")?;
                    material.validate()?;
                    materials.push((path, material));
                }
                "model-import.v1" => {
                    let import: ModelImportV1 = decode(value.clone(), "")?;
                    import.validate()?;
                    let bytes = source.bytes(&import.source).ok_or_else(|| {
                        SpatialContractError::new(
                            "model_source_missing",
                            "/source",
                            "Model source must exist in the captured source inventory.",
                        )
                    })?;
                    if bytes.len() as u64 > import.budgets.source_bytes {
                        return Err(SpatialContractError::new(
                            "source_budget_exceeded",
                            "/source",
                            "Primary model already exceeds the total source budget.",
                        ));
                    }
                    for output in &import.outputs {
                        if output.kind == ImportOutputKind::Mesh {
                            slots.insert(
                                output.asset_guid.clone(),
                                output
                                    .material_slots
                                    .iter()
                                    .map(|s| s.name.clone())
                                    .collect(),
                            );
                        }
                        records.push(record(
                            &output.asset_id,
                            &output.asset_guid,
                            output.kind.as_str(),
                            Some(output.key.clone()),
                            path,
                        ));
                    }
                }
                _ => {
                    if path.starts_with("Scenes/") || path.starts_with("Prefabs/") {
                        reject_legacy_document_extension(&value)?;
                    }
                    if (schema.starts_with("mesh-asset.") && value.get("source").is_some())
                        || (schema.starts_with("material-asset.") && value.get("preset").is_some())
                    {
                        return Err(SpatialContractError::new(
                            "envelope_upgrade_required",
                            "/schemaVersion",
                            "High-level mesh/material declarations require v2.",
                        ));
                    }
                    if schema.starts_with("model-import.") {
                        return Err(SpatialContractError::new(
                            "schema_unsupported",
                            "/schemaVersion",
                            "Expected model-import.v1.",
                        ));
                    }
                    if v3 && schema == "mesh-asset.v1" {
                        if let Some(render_data) = value.get("renderData") {
                            let asset: RenderContract<RenderAssetData> =
                                decode(render_data.clone(), "/renderData")?;
                            asset.data.validate().map_err(|e| {
                                SpatialContractError::new(
                                    "asset_invalid",
                                    "/renderData",
                                    format!("{e:?}"),
                                )
                            })?;
                            if let RenderAssetData::Mesh(mesh) = asset.data {
                                slots.insert(asset_guid(&value, path).into(), mesh.material_slots);
                            }
                        }
                    }
                }
            }
            if is_new
                && schema != AUTHORING_SCENE_V2
                && schema != AUTHORING_PREFAB_V2
                && !path.starts_with("Assets/")
            {
                return Err(SpatialContractError::new(
                    "source_domain_invalid",
                    "/schemaVersion",
                    "Asset contracts belong in Assets/.",
                ));
            }
            if v3 && path.starts_with("Assets/") {
                let id = value.get("assetId").and_then(Value::as_str).unwrap_or(path);
                let kind = schema
                    .strip_suffix("-asset.v1")
                    .or_else(|| schema.strip_suffix("-asset.v2"))
                    .unwrap_or("asset");
                records.push(record(id, asset_guid(&value, path), kind, None, path));
            }
            Ok(())
        })();
        outcome.map_err(|e| at(path, e))?;
    }
    if !v3 {
        return Ok(());
    }
    for record in &records {
        if !ids.insert(&record.asset_id) || !guids.insert(&record.asset_guid) {
            return Err(at(
                record.source_map_debug.as_deref().unwrap_or("Assets/"),
                SpatialContractError::new(
                    "asset_identity_duplicate",
                    "/assetGuid",
                    "Source and planned outputs must have unique assetId and assetGuid.",
                ),
            ));
        }
    }
    let index = RuntimeAssetIndex::new(records, Vec::new());
    for (path, material) in materials {
        if let Some(texture) = material.base_color_texture {
            resolve(
                &index,
                &texture.texture,
                RenderAssetKind::Texture,
                "/baseColorTexture/texture",
            )
            .map_err(|e| at(path, e))?;
        }
    }
    for (path, items) in entities {
        for (i, entity) in items.iter().enumerate() {
            for (j, c) in entity
                .components
                .iter()
                .enumerate()
                .filter(|(_, c)| c.component_type == "Camera3D")
            {
                let camera: Camera3D = decode(c.data.clone(), "").map_err(|e| at(path, e))?;
                if let Some(ibl) = camera.color_pipeline.as_ref().and_then(|p| p.ibl.as_ref()) {
                    resolve(
                        &index,
                        &ibl.environment,
                        RenderAssetKind::Environment,
                        &format!("/entities/{i}/components/{j}/data/colorPipeline/ibl/environment"),
                    )
                    .map_err(|e| at(path, e))?;
                }
            }
            for (j, component) in entity
                .components
                .iter()
                .enumerate()
                .filter(|(_, c)| c.component_type == "engine.prefab_instance")
            {
                let instance: SpatialPrefabInstance =
                    decode(component.data.clone(), "").map_err(|e| at(path, e))?;
                for (n, value) in instance
                    .overrides
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| v.component_type == "MeshRenderer3D")
                {
                    let field = format!("/entities/{i}/components/{j}/data/overrides/{n}/value");
                    let reference: SpatialAssetRef =
                        decode(value.value.clone(), &field).map_err(|e| at(path, e))?;
                    resolve(&index, &reference, RenderAssetKind::Material, &field)
                        .map_err(|e| at(path, e))?;
                }
            }
            let mut renderers = Vec::new();
            for (j, component) in entity.components.iter().enumerate() {
                let field = format!("/entities/{i}/components/{j}/data");
                if component.component_type == "MeshRenderer3D" {
                    renderers.push((
                        field.clone(),
                        decode::<MeshRenderer3D>(component.data.clone(), "")
                            .map_err(|e| at(path, e))?,
                    ));
                }
                if component.component_type == "engine.model_lod3d" {
                    let lod: engine_runtime::model_lod3d::ModelLod3D =
                        decode(component.data.clone(), "").map_err(|e| at(path, e))?;
                    lod.validate().map_err(|e| at(path, e.at(&field)))?;
                    for (n, level) in lod.levels.into_iter().enumerate() {
                        renderers.push((format!("{field}/levels/{n}/renderer"), level.renderer));
                    }
                }
                if component.component_type == "engine.vegetation3d" {
                    let binding: engine_runtime::terrain3d::VegetationBinding3D =
                        decode(component.data.clone(), "").map_err(|e| at(path, e))?;
                    let reference = engine_runtime::runtime_package::RuntimeAssetRef {
                        id: binding.cluster_asset.asset_id,
                        asset_type: "vegetation3d".into(),
                        guid: Some(binding.cluster_asset.asset_guid),
                        sub_asset: None,
                    };
                    if index.resolve(&reference).is_err() {
                        return Err(at(
                            path,
                            SpatialContractError::new(
                                "vegetation3d.asset_missing",
                                &field,
                                "Vegetation binding must resolve its exact source asset.",
                            ),
                        ));
                    }
                }
            }
            for (field, renderer) in renderers {
                let result = (|| -> ContractResult<()> {
                    let mesh = resolve(&index, &renderer.mesh, RenderAssetKind::Mesh, "/mesh")?;
                    let used_slots = slots.get(&mesh.asset_guid).ok_or_else(|| SpatialContractError::new("mesh_slots_unavailable", "/mesh", "Mesh requires validated renderData, generated source or committed import slot mapping."))?;
                    renderer.validate_slots(used_slots)?;
                    for (slot, material) in &renderer.materials {
                        resolve(
                            &index,
                            material,
                            RenderAssetKind::Material,
                            &format!("/materials/{}", pointer_token(slot)),
                        )?;
                    }
                    Ok(())
                })();
                result.map_err(|e| at(path, e.at(&field)))?;
            }
        }
    }
    Ok(())
}
fn asset_guid<'a>(value: &'a Value, fallback: &'a str) -> &'a str {
    value
        .get("assetGuid")
        .or_else(|| value.get("asset_guid"))
        .or_else(|| value.get("assetId"))
        .and_then(Value::as_str)
        .unwrap_or(fallback)
}
fn record(id: &str, guid: &str, kind: &str, sub: Option<String>, path: &str) -> RuntimeAssetRecord {
    RuntimeAssetRecord {
        asset_guid: guid.into(),
        asset_id: id.into(),
        asset_type: kind.into(),
        sub_asset_id: sub,
        version: "contract-only".into(),
        cooked_asset_id: String::new(),
        bundle_id: String::new(),
        loader_kind: kind.into(),
        dependencies: Vec::new(),
        hash: None,
        size: None,
        flags: Vec::new(),
        source_map_debug: Some(path.into()),
    }
}
fn resolve<'a>(
    index: &'a RuntimeAssetIndex,
    reference: &SpatialAssetRef,
    kind: RenderAssetKind,
    path: &str,
) -> ContractResult<&'a RuntimeAssetRecord> {
    match (RenderAssetReference {
        expected_kind: kind,
        reference: reference.0.clone(),
    })
    .resolve(Some(index))
    {
        Ok(RenderAssetResolution::Resolved(record)) => Ok(record),
        other => Err(SpatialContractError::new(
            "asset_reference_invalid",
            path,
            format!("{other:?}"),
        )),
    }
}
