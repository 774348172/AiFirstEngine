//! Native render assets enter the existing neutral package assembler here.
#[path = "generated_render_asset.rs"]
mod generated;
use super::{
    assembly_error, CompilerSourceView, GameProjectCompilerError, ProjectAssemblyProducerReport,
    RuntimePackageSourceAsset,
};
use engine_runtime::canonical_digest::sha256_prefixed;
use engine_runtime::render_contract::*;
use engine_runtime::runtime_asset::{RuntimeAssetIndex, RuntimeAssetRecord};

/// One immutable compiler snapshot owns this lazy reference projection. It never
/// survives an assembly operation, so cache hits still validate current identities.
pub(super) struct CookContext<'a> {
    source: &'a CompilerSourceView,
    index: Option<RuntimeAssetIndex>,
    #[cfg(test)]
    index_builds: usize,
}
impl<'a> CookContext<'a> {
    pub(super) fn new(source: &'a CompilerSourceView) -> Self {
        Self {
            source,
            index: None,
            #[cfg(test)]
            index_builds: 0,
        }
    }
    pub(super) fn index(&mut self) -> Result<&RuntimeAssetIndex, String> {
        if self.index.is_none() {
            self.index = Some(build_reference_index(self.source)?);
            #[cfg(test)]
            {
                self.index_builds += 1;
            }
        }
        Ok(self.index.as_ref().unwrap())
    }
}
fn build_reference_index(source: &CompilerSourceView) -> Result<RuntimeAssetIndex, String> {
    let mut records = Vec::new();
    let mut ids = std::collections::BTreeSet::new();
    let mut guids = std::collections::BTreeSet::new();
    // Match the assembler's v3 JSON asset eligibility; legacy projects still
    // resolve only .asset inputs, never an unassembled JSON sidecar.
    let json_assets = source
        .bytes("project.aife.json")
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(bytes).ok())
        .is_some_and(|manifest| {
            manifest["schemaVersion"].as_str()
                == Some(crate::SPATIAL_PROJECT_MANIFEST_SCHEMA_VERSION)
        });
    for (candidate_path, bytes) in &source.files {
        if !candidate_path.starts_with("Assets/")
            || !(candidate_path.ends_with(".asset")
                || (json_assets && candidate_path.ends_with(".json")))
        {
            continue;
        }
        let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if value["schemaVersion"] == "model-import.v1" {
            let descriptor: crate::three_d_contract::ModelImportV1 =
                serde_json::from_value(value).map_err(|e| e.to_string())?;
            for output in descriptor.outputs {
                if !ids.insert(output.asset_id.clone()) || !guids.insert(output.asset_guid.clone())
                {
                    return Err("duplicate asset id/guid in compiler snapshot".into());
                }
                records.push(RuntimeAssetRecord {
                    asset_guid: output.asset_guid,
                    asset_id: output.asset_id,
                    asset_type: output.kind.as_str().into(),
                    sub_asset_id: Some(output.key),
                    version: "source".into(),
                    cooked_asset_id: String::new(),
                    bundle_id: String::new(),
                    loader_kind: output.kind.as_str().into(),
                    dependencies: Vec::new(),
                    hash: None,
                    size: None,
                    flags: Vec::new(),
                    source_map_debug: Some(candidate_path.clone()),
                });
            }
            continue;
        }
        let Some(id) = value["assetId"].as_str() else {
            continue;
        };
        let guid = value
            .get("assetGuid")
            .or_else(|| value.get("asset_guid"))
            .and_then(|v| v.as_str())
            .unwrap_or(id);
        if !ids.insert(id.to_string()) || !guids.insert(guid.to_string()) {
            return Err("duplicate asset id/guid in compiler snapshot".into());
        }
        let kind = value["schemaVersion"]
            .as_str()
            .unwrap_or("")
            .strip_suffix("-asset.v1")
            .or_else(|| {
                value["schemaVersion"]
                    .as_str()
                    .unwrap_or("")
                    .strip_suffix("-asset.v2")
            })
            .unwrap_or("asset");
        records.push(RuntimeAssetRecord {
            asset_guid: guid.into(),
            asset_id: id.into(),
            asset_type: kind.into(),
            sub_asset_id: None,
            version: "source".into(),
            cooked_asset_id: String::new(),
            bundle_id: String::new(),
            loader_kind: kind.into(),
            dependencies: Vec::new(),
            hash: None,
            size: None,
            flags: Vec::new(),
            source_map_debug: Some(candidate_path.clone()),
        });
    }
    Ok(RuntimeAssetIndex::new(records, Vec::new()))
}

pub(super) fn cook(
    path: &str,
    document: &serde_json::Value,
    context: &mut CookContext<'_>,
    asset: &mut RuntimePackageSourceAsset,
    cache: Option<&crate::ProjectAssemblyArtifactCache>,
    reports: &mut Vec<ProjectAssemblyProducerReport>,
) -> Result<(), GameProjectCompilerError> {
    let source = context.source;
    let started = std::time::Instant::now();
    let invalid = |message: String| {
        assembly_error(
            "render_asset.invalid",
            format!("{path}: renderData: {message}"),
        )
    };
    if document.get("runtimeData").is_some() || document.get("sourceImage").is_some() {
        return Err(invalid(
            "native renderData cannot coexist with runtimeData/sourceImage".into(),
        ));
    }
    let high_level = matches!(
        document["schemaVersion"].as_str(),
        Some("mesh-asset.v2" | "material-asset.v2")
    );
    let recipe = if high_level {
        GENERATED_RENDER_COOK_RECIPE
    } else {
        NATIVE_RENDER_COOK_RECIPE
    };
    let source_digest = sha256_prefixed(source.bytes(path).expect("snapshot source exists"));
    let key = sha256_prefixed(format!("{recipe}|{source_digest}").as_bytes());
    let mut report = ProjectAssemblyProducerReport::uncached("render-asset-cook", 0);
    report.producer_recipe_version = recipe.into();
    report.recipe_key = Some(key.clone());
    let lookup_started = std::time::Instant::now();
    let cached = cache.and_then(|cache| {
        let lookup = cache.lookup_json::<RenderContract<CookedRenderAsset>>(
            "render-asset-cook",
            &key,
            recipe,
        );
        report.cache_status = lookup.status;
        report.miss_reason = lookup.reason;
        report.artifact_path = lookup.artifact_path.map(|p| p.display().to_string());
        let (Some(value), Some(envelope)) = (lookup.artifact, lookup.envelope) else {
            return None;
        };
        let encoded = serde_json::to_vec(&value).ok()?;
        if value.data.source_digest == source_digest
            && value.data.recipe == recipe
            && value.data.validate().is_ok()
            && envelope.dependency_digest == source_digest
            && envelope.output_digest == sha256_prefixed(&encoded)
        {
            Some((value.data.asset, encoded))
        } else {
            let _ = cache.quarantine("render-asset-cook", &key);
            report.cache_status = crate::ProjectAssemblyArtifactCacheStatus::Invalid;
            report.miss_reason = Some("typed_output_or_source_identity_mismatch".into());
            None
        }
    });
    report.lookup_duration_ms = lookup_started.elapsed().as_millis() as u64;
    let produce_started = std::time::Instant::now();
    let input = if let Some((input, _)) = &cached {
        input.clone()
    } else if high_level {
        generated::lower(document).map_err(|e| invalid(e.to_string()))?
    } else {
        RenderContract::<RenderAssetData>::decode(&document["renderData"].to_string())
            .map_err(|e| invalid(e.to_string()))?
            .data
    };
    if input.asset_kind().as_str() != asset.asset_type {
        return Err(invalid("asset type and renderData kind mismatch".into()));
    }
    if asset.asset_id.trim().is_empty()
        || asset.asset_guid.as_deref().unwrap_or("").trim().is_empty()
    {
        return Err(invalid(
            "native render assets require explicit assetId and assetGuid".into(),
        ));
    }
    if let RenderAssetData::Material(material) = &input {
        // Reuse the runtime index and typed resolver for identity consistency;
        // this temporary index is a projection of the compiler snapshot, not a DB.
        let index = context.index().map_err(&invalid)?;
        for reference in material.textures.values() {
            let RenderAssetResolution::Resolved(record) = reference
                .resolve(Some(index))
                .map_err(|e| invalid(e.to_string()))?
            else {
                unreachable!()
            };
            asset.dependencies.push(record.asset_guid.clone());
        }
        asset.dependencies.sort();
        asset.dependencies.dedup();
    }
    // Reference validity is checked against this snapshot even on a cache hit.
    // Texture pixels are owned by texture-cook; material output only stores its reference.
    if let Some((_, mut bytes)) = cached {
        // Serializer growth space is scratch, not part of a retained cooked asset.
        bytes.shrink_to_fit();
        asset.hash = Some(sha256_prefixed(&bytes));
        report.output_digest = asset.hash.clone();
        asset.runtime_payload = Some(bytes);
        report.produce_duration_ms = 0;
        report
            .substages
            .push(crate::ProjectAssemblyProducerSubstageReport::skipped(
                "lower-and-encode",
            ));
    } else {
        finish(
            input,
            recipe,
            source_digest.clone(),
            asset,
            &mut Vec::new(),
            started,
            None,
        )?;
        report.produce_duration_ms = produce_started.elapsed().as_millis() as u64;
        report.output_digest = asset.hash.clone();
        report
            .substages
            .push(crate::ProjectAssemblyProducerSubstageReport::completed(
                "lower-and-encode",
                report.produce_duration_ms,
            ));
        if let Some(cache) = cache {
            let publish_started = std::time::Instant::now();
            let value: RenderContract<CookedRenderAsset> =
                serde_json::from_slice(asset.runtime_payload.as_ref().unwrap())
                    .map_err(|e| invalid(e.to_string()))?;
            let published = cache
                .publish_json(
                    "render-asset-cook",
                    recipe,
                    &key,
                    &source_digest,
                    asset.hash.as_ref().unwrap(),
                    &value,
                )
                .map_err(|e| assembly_error("render_asset.cache_publish_failed", e.to_string()))?;
            report.cache_status = match published.status {
                crate::ProjectAssemblyArtifactPublishStatus::Produced => {
                    crate::ProjectAssemblyArtifactCacheStatus::Produced
                }
                crate::ProjectAssemblyArtifactPublishStatus::PublishRaceReused => {
                    crate::ProjectAssemblyArtifactCacheStatus::PublishRaceReused
                }
            };
            report.artifact_path = Some(published.artifact_path.display().to_string());
            report.publish_duration_ms = publish_started.elapsed().as_millis() as u64;
        }
    }
    report.duration_ms = started.elapsed().as_millis() as u64;
    reports.push(report);
    Ok(())
}

/// Both native authoring and model import use the same cooked contract and producer.
pub(super) fn finish(
    input: RenderAssetData,
    recipe: &str,
    source_digest: String,
    asset: &mut RuntimePackageSourceAsset,
    reports: &mut Vec<ProjectAssemblyProducerReport>,
    started: std::time::Instant,
    cache: Option<&crate::ProjectAssemblyArtifactCache>,
) -> Result<(), GameProjectCompilerError> {
    let invalid = |message| assembly_error("render_asset.invalid", message);
    let cooked = CookedRenderAsset {
        source_digest: source_digest.clone(),
        recipe: recipe.into(),
        asset: input,
    };
    cooked.validate().map_err(|e| invalid(e.to_string()))?;
    let key = sha256_prefixed(format!("{recipe}|{source_digest}").as_bytes());
    let mut report = ProjectAssemblyProducerReport::uncached("render-asset-cook", 0);
    report.producer_recipe_version = recipe.into();
    report.recipe_key = Some(key.clone());
    let lookup_started = std::time::Instant::now();
    let cached = cache.and_then(|cache| {
        let lookup = cache.lookup_bytes("render-asset-cook", &key, recipe);
        report.cache_status = lookup.status;
        report.miss_reason = lookup.reason;
        report.artifact_path = lookup.artifact_path.map(|p| p.display().to_string());
        let (Some(bytes), Some(envelope)) = (lookup.artifact, lookup.envelope) else {
            return None;
        };
        let valid = serde_json::from_slice::<RenderContract<CookedRenderAsset>>(&bytes).is_ok_and(
            |value| {
                value.data.source_digest == source_digest
                    && value.data.recipe == recipe
                    && value.data.validate().is_ok()
                    && envelope.dependency_digest == source_digest
                    && envelope.output_digest == sha256_prefixed(&bytes)
            },
        );
        if valid {
            Some(bytes)
        } else {
            let _ = cache.quarantine("render-asset-cook", &key);
            report.cache_status = crate::ProjectAssemblyArtifactCacheStatus::Invalid;
            report.miss_reason = Some("typed_output_or_source_identity_mismatch".into());
            None
        }
    });
    report.lookup_duration_ms = lookup_started.elapsed().as_millis() as u64;
    if let Some(mut bytes) = cached {
        bytes.shrink_to_fit();
        asset.hash = Some(sha256_prefixed(&bytes));
        asset.runtime_payload = Some(bytes);
        report.output_digest = asset.hash.clone();
        report.produce_duration_ms = 0;
        report
            .substages
            .push(crate::ProjectAssemblyProducerSubstageReport::skipped(
                "encode",
            ));
        report.duration_ms = started.elapsed().as_millis() as u64;
        reports.push(report);
        return Ok(());
    }
    let produce_started = std::time::Instant::now();
    let mut bytes =
        serde_json::to_vec(&RenderContract::new(cooked)).map_err(|e| invalid(e.to_string()))?;
    bytes.shrink_to_fit();
    let digest = sha256_prefixed(&bytes);
    asset.hash = Some(digest.clone());
    report.produce_duration_ms = produce_started.elapsed().as_millis() as u64;
    report.output_digest = Some(digest.clone());
    if let Some(cache) = cache {
        let publish_started = std::time::Instant::now();
        let published = cache
            .publish_bytes(
                "render-asset-cook",
                recipe,
                &key,
                &source_digest,
                &digest,
                &bytes,
            )
            .map_err(|e| assembly_error("render_asset.cache_publish_failed", e.to_string()))?;
        report.cache_status = match published.status {
            crate::ProjectAssemblyArtifactPublishStatus::Produced => {
                crate::ProjectAssemblyArtifactCacheStatus::Produced
            }
            crate::ProjectAssemblyArtifactPublishStatus::PublishRaceReused => {
                crate::ProjectAssemblyArtifactCacheStatus::PublishRaceReused
            }
        };
        report.artifact_path = Some(published.artifact_path.display().to_string());
        report.publish_duration_ms = publish_started.elapsed().as_millis() as u64;
    }
    asset.runtime_payload = Some(bytes);
    report.duration_ms = started.elapsed().as_millis() as u64;
    reports.push(report);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_runtime::render_asset_production::*;
    use engine_runtime::render_resource::*;
    use engine_runtime::runtime_asset_loader::RuntimeAssetLoader;
    use engine_runtime::runtime_package::*;
    use engine_runtime::runtime_package_builder::*;
    use std::collections::BTreeMap;

    fn snapshot() -> CompilerSourceView {
        let mesh: serde_json::Value = serde_json::from_str(include_str!(
            "../../engine_runtime/tests/fixtures/render_mesh_v1.json"
        ))
        .unwrap();
        let material: serde_json::Value = serde_json::from_str(include_str!(
            "../../engine_runtime/tests/fixtures/render_material_v1.json"
        ))
        .unwrap();
        CompilerSourceView { files: BTreeMap::from([
            ("Assets/mesh.asset".into(), serde_json::to_vec(&serde_json::json!({"schemaVersion":"mesh-asset.v1", "assetId":"mesh", "assetGuid":"g-mesh", "renderData":mesh})).unwrap()),
            ("Assets/material.asset".into(), serde_json::to_vec(&serde_json::json!({"schemaVersion":"material-asset.v1", "assetId":"material", "assetGuid":"g-material", "renderData":material})).unwrap()),
        ]) }
    }
    #[test]
    fn material_reference_projection_is_built_once_per_immutable_snapshot() {
        let source = snapshot();
        let mut context = CookContext::new(&source);
        for _ in 0..260 {
            context.index().unwrap();
        }
        assert_eq!(
            context.index_builds, 1,
            "260 materials must not rescan all mesh JSON"
        );
        // A new operation must observe a duplicate introduced into the next snapshot.
        let mut changed = source.clone();
        changed.files.insert(
            "Assets/duplicate.asset".into(),
            changed.files["Assets/mesh.asset"].clone(),
        );
        let mut next = CookContext::new(&changed);
        assert!(next
            .index()
            .unwrap_err()
            .contains("duplicate asset id/guid"));
    }
    #[test]
    fn vegetation_dependency_projection_reuses_the_same_immutable_snapshot_index() {
        let mut source = snapshot();
        let instance = serde_json::json!({"sourceId":"tree", "translation":[0,0,0],"yawRadians":0,"scale":1,"windPhase":0});
        let instances = (0..64)
            .map(|i| {
                let mut v = instance.clone();
                v["sourceId"] = format!("tree-{i}").into();
                v
            })
            .collect::<Vec<_>>();
        let cluster = serde_json::json!({"key":"trees","mesh":{"expectedKind":"mesh","reference":{"id":"mesh","type":"mesh","guid":"g-mesh"}},"material":{"expectedKind":"material","reference":{"id":"material","type":"material","guid":"g-material"}},"meshBounds":[[-1,0,-1],[1,3,1]],"wind":{"direction":[1,0],"amplitude":0.1,"frequencyHz":1},"instances":instances});
        let clusters = (0..64)
            .map(|i| {
                let mut c = cluster.clone();
                c["key"] = format!("cluster-{i}").into();
                for v in c["instances"].as_array_mut().unwrap() {
                    v["sourceId"] = format!("{i}-{}", v["sourceId"].as_str().unwrap()).into();
                }
                c
            })
            .collect::<Vec<_>>();
        source.files.insert("Assets/trees.asset".into(),serde_json::to_vec(&serde_json::json!({"schemaVersion":"vegetation3d-asset.v1","assetId":"trees","assetGuid":"g-trees","clusters":clusters})).unwrap());
        let mut context = CookContext::new(&source);
        let mut input =
            RuntimePackageBuildInput::new(RuntimeProjectInfo::explicit_empty("p", "P", "1"));
        super::super::append_asset_with_context(
            &mut context,
            "Assets/trees.asset",
            &source,
            &mut input,
            None,
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(
            context.index_builds, 1,
            "128 vegetation dependencies must share the existing snapshot reference index"
        );
        assert_eq!(input.assets[0].dependencies, vec!["g-material", "g-mesh"]);
        let mut changed = source.clone();
        changed.files.remove("Assets/material.asset");
        let mut context = CookContext::new(&changed);
        assert!(super::super::append_asset_with_context(
            &mut context,
            "Assets/trees.asset",
            &changed,
            &mut input,
            None,
            &mut Vec::new()
        )
        .is_err());
    }
    fn cached_input(
        source: &CompilerSourceView,
        cache: &crate::ProjectAssemblyArtifactCache,
    ) -> (RuntimePackageBuildInput, Vec<ProjectAssemblyProducerReport>) {
        let mut input =
            RuntimePackageBuildInput::new(RuntimeProjectInfo::explicit_empty("p", "P", "1"));
        let mut reports = Vec::new();
        for path in ["Assets/mesh.asset", "Assets/material.asset"] {
            super::super::append_asset(path, source, &mut input, Some(cache), &mut reports)
                .unwrap();
        }
        (input, reports)
    }
    fn test_cache() -> crate::ProjectAssemblyArtifactCache {
        crate::ProjectAssemblyArtifactCache::open(std::env::temp_dir().join(format!(
            "aife-371-render-cache-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        )))
        .unwrap()
    }
    #[test]
    fn imported_render_cook_reuses_exact_output_and_rebuilds_changed_or_corrupt_input() {
        use crate::ProjectAssemblyArtifactCacheStatus as Status;
        let cache = test_cache();
        let mut source = snapshot();
        let run = |source: &CompilerSourceView,
                   cache: Option<&crate::ProjectAssemblyArtifactCache>| {
            let document: serde_json::Value =
                serde_json::from_slice(&source.files["Assets/material.asset"]).unwrap();
            let data: RenderContract<RenderAssetData> =
                serde_json::from_value(document["renderData"].clone()).unwrap();
            let source_digest = sha256_prefixed(
                &serde_json::to_vec(&(IMPORTED_RENDER_COOK_RECIPE, &data.data)).unwrap(),
            );
            let mut asset = RuntimePackageSourceAsset::new(
                "material",
                "Material",
                "material",
                "model.gltf",
                "cooked/material.asset",
            );
            let mut reports = Vec::new();
            finish(
                data.data,
                IMPORTED_RENDER_COOK_RECIPE,
                source_digest,
                &mut asset,
                &mut reports,
                std::time::Instant::now(),
                cache,
            )
            .unwrap();
            (asset, reports.remove(0))
        };
        let (uncached, _) = run(&source, None);
        let (first, cold) = run(&source, Some(&cache));
        let (second, warm) = run(&source, Some(&cache));
        assert_eq!(cold.cache_status, Status::Produced);
        assert_eq!(warm.cache_status, Status::Hit);
        assert_eq!(first.runtime_payload, uncached.runtime_payload);
        assert_eq!(first.runtime_payload, second.runtime_payload);
        mutate(&mut source, "Assets/material.asset", |v| {
            v["renderData"]["data"]["payload"]["parameters"]["baseColor"]["value"][0] =
                serde_json::json!(0.7);
        });
        let (changed, rebuilt) = run(&source, Some(&cache));
        assert_eq!(rebuilt.cache_status, Status::Produced);
        assert_ne!(changed.hash, first.hash);
        let root = std::path::PathBuf::from(rebuilt.artifact_path.unwrap());
        std::fs::write(root.join("payload.bin"), b"{}").unwrap();
        let (repaired, repair_report) = run(&source, Some(&cache));
        assert_eq!(repair_report.cache_status, Status::Produced);
        assert!(repair_report.miss_reason.is_some());
        assert_eq!(repaired.runtime_payload, changed.runtime_payload);
        std::fs::remove_dir_all(cache.root()).unwrap();
    }
    #[test]
    fn retained_render_payload_does_not_keep_serialization_growth_capacity() {
        let cache = test_cache();
        let source = snapshot();
        for (input, _) in [cached_input(&source, &cache), cached_input(&source, &cache)] {
            for asset in &input.assets {
                let payload = asset.runtime_payload.as_ref().unwrap();
                assert_eq!(
                    payload.capacity(),
                    payload.len(),
                    "{} retains unused serializer backing",
                    asset.asset_id
                );
                assert_eq!(
                    asset.hash.as_deref(),
                    Some(sha256_prefixed(payload).as_str())
                );
                RenderContract::<CookedRenderAsset>::decode(std::str::from_utf8(payload).unwrap())
                    .unwrap();
            }
        }
        std::fs::remove_dir_all(cache.root()).unwrap();
    }

    #[test]
    fn native_cook_cache_reuses_unchanged_and_only_rebuilds_changed_asset() {
        use crate::ProjectAssemblyArtifactCacheStatus as Status;
        let cache = test_cache();
        let mut source = snapshot();
        let (first, cold) = cached_input(&source, &cache);
        assert!(
            cold.iter().all(|r| r.cache_status == Status::Produced),
            "{cold:#?}"
        );
        let (second, warm) = cached_input(&source, &cache);
        assert!(
            warm.iter().all(|r| r.cache_status == Status::Hit),
            "{warm:#?}"
        );
        for (a, b) in first.assets.iter().zip(&second.assets) {
            assert_eq!(a.runtime_payload, b.runtime_payload);
            assert_eq!(a.dependencies, b.dependencies);
        }
        mutate(&mut source, "Assets/material.asset", |v| {
            v["renderData"]["data"]["payload"]["parameters"]["baseColor"]["value"][0] =
                serde_json::json!(0.7);
        });
        let (changed, partial) = cached_input(&source, &cache);
        assert_eq!(partial[0].cache_status, Status::Hit);
        assert_eq!(partial[1].cache_status, Status::Produced);
        assert_eq!(changed.assets[0].hash, first.assets[0].hash);
        assert_ne!(changed.assets[1].hash, first.assets[1].hash);
        std::fs::remove_dir_all(cache.root()).unwrap();
    }
    #[test]
    fn native_cached_material_revalidates_current_texture_identity() {
        let cache = test_cache();
        let mut source = snapshot();
        source.files.insert(
            "Assets/t.asset".into(),
            br#"{"schemaVersion":"texture-asset.v1","assetId":"texture","assetGuid":"g-texture"}"#
                .to_vec(),
        );
        mutate(&mut source, "Assets/material.asset", |v| {
            v["renderData"]["data"]["payload"]["textures"]["baseColorTexture"] = serde_json::json!({"expectedKind":"texture","reference":{"id":"texture","type":"texture","guid":"g-texture"}});
        });
        cached_input(&source, &cache);
        cached_input(&source, &cache);
        source.files.remove("Assets/t.asset");
        let mut input =
            RuntimePackageBuildInput::new(RuntimeProjectInfo::explicit_empty("p", "P", "1"));
        assert!(super::super::append_asset(
            "Assets/material.asset",
            &source,
            &mut input,
            Some(&cache),
            &mut Vec::new()
        )
        .is_err());
        std::fs::remove_dir_all(cache.root()).unwrap();
    }
    #[test]
    fn native_cook_corrupt_cache_is_quarantined_and_rebuilt() {
        use crate::ProjectAssemblyArtifactCacheStatus as Status;
        let cache = test_cache();
        let source = snapshot();
        let (first, reports) = cached_input(&source, &cache);
        let root = std::path::PathBuf::from(reports[0].artifact_path.as_ref().unwrap());
        std::fs::write(root.join("payload.json"), b"{}").unwrap();
        let (second, repaired) = cached_input(&source, &cache);
        assert_eq!(repaired[0].cache_status, Status::Produced);
        assert!(repaired[0].miss_reason.is_some());
        assert_eq!(repaired[1].cache_status, Status::Hit);
        for (a, b) in first.assets.iter().zip(&second.assets) {
            assert_eq!(a.runtime_payload, b.runtime_payload);
            assert_eq!(a.dependencies, b.dependencies);
        }
        std::fs::remove_dir_all(cache.root()).unwrap();
    }
    fn input(
        snapshot: &CompilerSourceView,
    ) -> (RuntimePackageBuildInput, Vec<ProjectAssemblyProducerReport>) {
        let mut input = RuntimePackageBuildInput::new(RuntimeProjectInfo::explicit_empty(
            "render-u1",
            "Render",
            "1",
        ));
        input.input_mappings.push(RuntimePackageSourceJson { id:"input.none".into(), document:serde_json::json!({"schema_version":"input-mapping.v1","asset_id":"input.none","actions":[],"contexts":[],"bindings":[]}) });
        input.scenes.push(RuntimeScene {
            main_camera: None,
            schema_version: RUNTIME_SCENE_SCHEMA_VERSION.into(),
            id: "main".into(),
            name: "Main".into(),
            gravity: 0.0,
            background: "#000000".into(),
            sky_color: "#000000".into(),
            entities: Vec::new(),
        });
        let mut reports = Vec::new();
        for path in snapshot.files.keys().filter(|p| p.ends_with(".asset")) {
            super::super::append_asset(path, snapshot, &mut input, None, &mut reports).unwrap();
        }
        (input, reports)
    }
    fn package(input: &RuntimePackageBuildInput) -> (std::path::PathBuf, RuntimePackage) {
        let root = std::env::temp_dir().join(format!(
            "aife-358-u1-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let report = RuntimePackageBuilder::build(
            &RuntimePackageBuildRequest::dev_desktop(&root, "main"),
            input,
        );
        assert_eq!(
            report.status,
            RuntimePackageBuildStatus::Success,
            "{report:?}"
        );
        let loaded = load_runtime_package(&root);
        assert!(loaded.diagnostics.is_ok(), "{:?}", loaded.diagnostics);
        (root, loaded.value.unwrap())
    }
    fn loader(root: &std::path::Path, package: &RuntimePackage) -> RuntimeAssetLoader {
        let mut value = RuntimeAssetLoader::new(
            root,
            package.runtime_asset_index.clone(),
            package.runtime_asset_mount_table.clone(),
        );
        assert!(value.mount_bundle("startup"));
        value
    }
    fn reference(id: &str, kind: RenderAssetKind) -> RenderAssetReference {
        RenderAssetReference {
            expected_kind: kind,
            reference: RuntimeAssetRef {
                id: id.into(),
                asset_type: kind.as_str().into(),
                guid: Some(format!("g-{id}")),
                sub_asset: None,
            },
        }
    }
    fn mutate(
        snapshot: &mut CompilerSourceView,
        path: &str,
        change: impl FnOnce(&mut serde_json::Value),
    ) {
        let mut value: serde_json::Value = serde_json::from_slice(&snapshot.files[path]).unwrap();
        change(&mut value);
        snapshot
            .files
            .insert(path.into(), serde_json::to_vec(&value).unwrap());
    }
    fn cook_error(snapshot: &CompilerSourceView, path: &str) -> GameProjectCompilerError {
        let mut input =
            RuntimePackageBuildInput::new(RuntimeProjectInfo::explicit_empty("p", "P", "1"));
        super::super::append_asset(path, snapshot, &mut input, None, &mut Vec::new()).unwrap_err()
    }

    #[test]
    fn native_mesh_material_cook_package_load_prepare_roundtrip_and_reuse() {
        let source = snapshot();
        let (input, reports) = input(&source);
        assert_eq!(reports.len(), 2);
        let (root, package) = package(&input);
        let mut loader = loader(&root, &package);
        let mut manager = RenderResourceManager::new();
        for (id, kind) in [
            ("mesh", RenderAssetKind::Mesh),
            ("material", RenderAssetKind::Material),
        ] {
            let reference = reference(id, kind);
            let producer = RuntimeRenderAssetProducer::new();
            let prepared = producer
                .prepare_cooked_from_package(0, &reference, &mut loader, &mut manager)
                .unwrap();
            assert_eq!(prepared.state, RenderAssetPreparationState::Prepared);
            assert_eq!(
                prepared.asset.source_digest,
                sha256_prefixed(&source.files[&format!("Assets/{id}.asset")])
            );
            assert_eq!(
                prepared.source_path.as_deref(),
                Some(format!("Assets/{id}.asset").as_str())
            );
            assert_eq!(prepared.binding.slots.len(), 1);
            assert!(reports
                .iter()
                .any(|r| r.output_digest.as_ref() == Some(&prepared.cooked_digest)));
            match &prepared.asset.asset {
                RenderAssetData::Mesh(mesh) => {
                    assert_eq!(mesh.index_bytes, vec![0, 0, 1, 0, 2, 0]);
                    assert_eq!(prepared.resource_request.byte_len, 42);
                }
                RenderAssetData::Material(material) => {
                    assert_eq!(
                        prepared.pipeline_identity,
                        Some(material.pipeline_identity().unwrap())
                    );
                }
            }
            let reused = producer
                .prepare_cooked_from_package(1, &reference, &mut loader, &mut manager)
                .unwrap();
            assert_eq!(prepared.binding, reused.binding);
        }
        assert_eq!(manager.report(1).reused_count, 2);
        assert_eq!(loader.decoded_cache_len(), 0);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn native_cook_rejects_wrong_kind_mixed_formats_missing_guid_and_bad_geometry() {
        let mut source = snapshot();
        mutate(&mut source, "Assets/mesh.asset", |v| {
            v["schemaVersion"] = "material-asset.v1".into()
        });
        assert!(cook_error(&source, "Assets/mesh.asset")
            .message()
            .contains("mismatch"));
        let mut source = snapshot();
        mutate(&mut source, "Assets/mesh.asset", |v| {
            v["runtimeData"] = serde_json::json!({})
        });
        assert!(cook_error(&source, "Assets/mesh.asset")
            .message()
            .contains("coexist"));
        let mut source = snapshot();
        mutate(&mut source, "Assets/mesh.asset", |v| {
            v.as_object_mut()
                .unwrap()
                .remove("assetGuid")
                .map(|_| ())
                .unwrap()
        });
        assert!(cook_error(&source, "Assets/mesh.asset")
            .message()
            .contains("assetGuid"));
        let mut source = snapshot();
        mutate(&mut source, "Assets/mesh.asset", |v| {
            v["renderData"]["data"]["payload"]["indexBytes"] = serde_json::json!([0, 0, 1, 0, 8, 0])
        });
        assert!(cook_error(&source, "Assets/mesh.asset")
            .message()
            .contains("indexBytes.range"));
    }

    #[test]
    fn material_dependency_identity_missing_wrong_type_guid_and_subasset_fail_at_cook() {
        for (id, kind, guid, subasset) in [
            ("missing", "texture", "g-missing", None),
            ("mesh", "texture", "g-mesh", None),
            ("texture", "texture", "wrong", None),
            ("texture", "texture", "g-texture", Some("absent")),
        ] {
            let mut source = snapshot();
            source.files.insert("Assets/t.asset".into(), br#"{"schemaVersion":"texture-asset.v1","assetId":"texture","assetGuid":"g-texture"}"#.to_vec());
            mutate(&mut source, "Assets/material.asset", |v| {
                v["renderData"]["data"]["payload"]["textures"]["baseColorTexture"] = serde_json::json!({"expectedKind":"texture","reference":{"id":id,"type":kind,"guid":guid,"sub_asset":subasset}})
            });
            assert!(cook_error(&source, "Assets/material.asset")
                .message()
                .contains("Asset"));
        }
    }

    #[test]
    fn native_json_dependency_eligibility_matches_project_assembler_version() {
        let mut source = snapshot();
        source.files.insert(
            "Assets/t.json".into(),
            br#"{"schemaVersion":"texture-asset.v1","assetId":"texture","assetGuid":"g-texture"}"#
                .to_vec(),
        );
        mutate(&mut source, "Assets/material.asset", |v| {
            v["renderData"]["data"]["payload"]["textures"]["baseColorTexture"] = serde_json::json!({"expectedKind":"texture","reference":{"id":"texture","type":"texture","guid":"g-texture"}});
        });
        assert!(cook_error(&source, "Assets/material.asset")
            .message()
            .contains("Asset"));
        source.files.insert(
            "project.aife.json".into(),
            br#"{"schemaVersion":"aife-project.v3","projectId":"v3"}"#.to_vec(),
        );
        let (_, reports) = input(&source);
        assert_eq!(reports.len(), 2);
    }

    #[test]
    fn material_named_texture_dependency_reaches_real_package_binding() {
        let mut source = snapshot();
        source.files.insert(
            "Assets/t.asset".into(),
            br#"{"schemaVersion":"texture-asset.v1","assetId":"texture","assetGuid":"g-texture"}"#
                .to_vec(),
        );
        mutate(&mut source, "Assets/material.asset", |v| {
            v["renderData"]["data"]["payload"]["textures"]["baseColorTexture"] = serde_json::json!({"expectedKind":"texture","reference":{"id":"texture","type":"texture","guid":"g-texture"}})
        });
        let (mut input, _) = input(&source);
        input.texture_payloads.push(RuntimePackageSourceTexture {
            metadata: CookedTextureAsset {
                schema_version: COOKED_TEXTURE_SCHEMA_VERSION.into(),
                asset_id: "texture".into(),
                cooked_asset_id: "cooked-texture".into(),
                width: 1,
                height: 1,
                format: "rgba8Unorm".into(),
                color_space: "srgb".into(),
                mip_count: 1,
                byte_length: 4,
                pixel_data_path: "cooked/textures/texture.rgba".into(),
                sampler: "nearestClamp".into(),
                source_hash: sha256_prefixed(&[1, 2, 3, 255]),
            },
            rgba8: vec![1, 2, 3, 255].into(),
        });
        let (root, package) = package(&input);
        let mut loader = loader(&root, &package);
        let mut manager = RenderResourceManager::new();
        let prepared = RuntimeRenderAssetProducer::new()
            .prepare_cooked_from_package(
                0,
                &reference("material", RenderAssetKind::Material),
                &mut loader,
                &mut manager,
            )
            .unwrap();
        assert_eq!(
            prepared.textures[0].payload.rgba8.as_slice(),
            &[1, 2, 3, 255]
        );
        assert_eq!(
            prepared.binding.texture("baseColorTexture"),
            Some(prepared.textures[0].handle)
        );
        assert_eq!(
            package
                .runtime_asset_index
                .record_by_guid("g-material")
                .unwrap()
                .dependencies,
            vec!["g-texture"]
        );
        assert_eq!(loader.decoded_cache_len(), 0);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn native_runtime_rejects_tampering_missing_payload_type_and_manifest_mismatch() {
        let (input, _) = input(&snapshot());
        let (root, package) = package(&input);
        let producer = RuntimeRenderAssetProducer::new();
        let mut manager = RenderResourceManager::new();
        let mut load = loader(&root, &package);
        let wrong = reference("mesh", RenderAssetKind::Material);
        assert_eq!(
            producer
                .prepare_cooked_from_package(0, &wrong, &mut load, &mut manager)
                .unwrap_err()
                .code,
            ContractErrorCode::AssetTypeMismatch
        );
        let path = root.join("cooked/mesh.asset");
        let original = std::fs::read(&path).unwrap();
        let mut changed = original.clone();
        changed.push(b' ');
        std::fs::write(&path, &changed).unwrap();
        assert_eq!(
            producer
                .prepare_cooked_from_package(
                    0,
                    &reference("mesh", RenderAssetKind::Mesh),
                    &mut load,
                    &mut manager
                )
                .unwrap_err()
                .code,
            ContractErrorCode::AssetDigestMismatch
        );
        assert_eq!(load.decoded_cache_len(), 0);
        std::fs::remove_file(&path).unwrap();
        assert_eq!(
            producer
                .prepare_cooked_from_package(
                    0,
                    &reference("mesh", RenderAssetKind::Mesh),
                    &mut load,
                    &mut manager
                )
                .unwrap_err()
                .code,
            ContractErrorCode::MissingCookedPayload
        );
        assert_eq!(manager.report(0).created_count, 0);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn native_changed_content_invalidates_package_digest_and_previous_binding() {
        let mut source = snapshot();
        let (first, _) = input(&source);
        let (first_root, first_package) = package(&first);
        mutate(&mut source, "Assets/material.asset", |v| {
            v["renderData"]["data"]["payload"]["parameters"]["baseColor"]["value"][0] = 0.9.into()
        });
        let (second, _) = input(&source);
        let (second_root, second_package) = package(&second);
        assert_ne!(
            first_package.manifest.content_hash,
            second_package.manifest.content_hash
        );
        let mut manager = RenderResourceManager::new();
        let producer = RuntimeRenderAssetProducer::new();
        let reference = reference("material", RenderAssetKind::Material);
        let old = producer
            .prepare_cooked_from_package(
                0,
                &reference,
                &mut loader(&first_root, &first_package),
                &mut manager,
            )
            .unwrap();
        let new = producer
            .prepare_cooked_from_package(
                1,
                &reference,
                &mut loader(&second_root, &second_package),
                &mut manager,
            )
            .unwrap();
        assert_ne!(old.cooked_digest, new.cooked_digest);
        assert_eq!(old.pipeline_identity, new.pipeline_identity);
        assert_eq!(
            old.binding.validate_residency(&manager).unwrap_err().code,
            ContractErrorCode::StaleResourceHandle
        );
        new.binding.validate_residency(&manager).unwrap();
        std::fs::remove_dir_all(first_root).unwrap();
        std::fs::remove_dir_all(second_root).unwrap();
    }
}
