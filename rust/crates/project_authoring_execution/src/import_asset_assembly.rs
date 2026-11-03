use super::*;
use crate::model_import;
use engine_runtime::render_contract::{RenderAssetData, IMPORTED_RENDER_COOK_RECIPE};
use engine_runtime::runtime_package_builder::RuntimePackagePrefabIdentity;

pub(super) fn append(
    source: &CompilerSourceView,
    input: &mut RuntimePackageBuildInput,
    reports: &mut Vec<ProjectAssemblyProducerReport>,
    cache: Option<&ProjectAssemblyArtifactCache>,
    scope: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), GameProjectCompilerError> {
    for imported in model_import::compile_cached(source, cache, scope, reports, cancelled)
        .map_err(model_import::compiler_error)?
    {
        let path = &imported.report.descriptor_path;
        let asset_input = |output: &crate::three_d_contract::ImportOutput| {
            let mut asset = RuntimePackageSourceAsset::new(
                &output.asset_id,
                &output.asset_id,
                output.kind.as_str(),
                path,
                format!("cooked/{}.asset", output.asset_id),
            );
            asset.asset_guid = Some(output.asset_guid.clone());
            asset.sub_asset_id = Some(output.key.clone());
            asset
        };
        let dependencies = imported
            .assets
            .iter()
            .map(|(o, _)| o.asset_guid.clone())
            .chain(imported.character.iter().map(|(o, _)| o.asset_guid.clone()))
            .collect();
        for (output, data) in imported.assets {
            let mut asset = asset_input(&output);
            if let RenderAssetData::Material(material) = &data {
                asset.dependencies = material
                    .textures
                    .values()
                    .filter_map(|r| r.reference.guid.clone())
                    .collect();
                asset.dependencies.sort();
                asset.dependencies.dedup();
            }
            let asset_digest = engine_runtime::canonical_digest::sha256_prefixed(
                &serde_json::to_vec(&(IMPORTED_RENDER_COOK_RECIPE, &data))
                    .expect("asset dependencies"),
            );
            render_asset_cook::finish(
                data,
                IMPORTED_RENDER_COOK_RECIPE,
                asset_digest,
                &mut asset,
                reports,
                std::time::Instant::now(),
                cache,
            )?;
            input.assets.push(asset);
        }
        for (output, texture) in imported.textures {
            let mut asset = asset_input(&output);
            asset.hash = Some(texture.metadata.source_hash.clone());
            asset.runtime_uri = format!("cooked/textures/{}.texture.json", output.asset_id);
            input.texture_payloads.push(texture);
            input.assets.push(asset);
        }
        if let Some((output, character)) = imported.character {
            let mut asset = asset_input(&output);
            let bytes = serde_json::to_vec(&character).expect("validated character");
            asset.hash = Some(engine_runtime::canonical_digest::sha256_prefixed(&bytes));
            asset.dependencies = character
                .skins
                .iter()
                .filter_map(|skin| {
                    input
                        .assets
                        .iter()
                        .find(|a| a.asset_id == skin.mesh_asset_id)
                        .and_then(|a| a.asset_guid.clone())
                })
                .collect();
            asset.runtime_payload = Some(bytes);
            input.assets.push(asset);
        }
        let (output, prefab) = imported.prefab;
        input.prefabs.push(RuntimePackageSourcePrefab {
            identity: Some(RuntimePackagePrefabIdentity {
                asset_guid: output.asset_guid,
                sub_asset_id: output.key,
                source: path.clone(),
                dependencies,
            }),
            prefab_id: output.asset_id,
            document: prefab,
        });
    }
    Ok(())
}
