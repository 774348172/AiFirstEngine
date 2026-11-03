//! Source validation uses the same decoded rigs and graph validator as the runtime.
use super::{assembly::CompiledImport, fail, CompilerSourceView, ImportFailure, Result};
use engine_runtime::character_animation::{
    CharacterAsset, CharacterBinding3D, SampledCharacterPose,
};
use serde_json::Value;
use std::collections::BTreeMap;

pub(super) fn validate(source: &CompilerSourceView, imports: &[CompiledImport]) -> Result<()> {
    let mut authored = Vec::new();
    for path in source
        .paths()
        .filter(|p| p.starts_with("Assets/") && (p.ends_with(".json") || p.ends_with(".asset")))
    {
        #[derive(serde::Deserialize)]
        struct Header {
            #[serde(rename = "schemaVersion", default)]
            schema: Option<String>,
        }
        let bytes = source.bytes(path).unwrap();
        let header: Header = serde_json::from_slice(bytes)
            .map_err(|e| fail(path, "", "schema_invalid", e.to_string()))?;
        if header.schema.as_deref() == Some("character3d.v1") {
            let value: Value = serde_json::from_slice(bytes)
                .map_err(|e| fail(path, "", "schema_invalid", e.to_string()))?;
            let guid = value["assetGuid"].as_str().map(str::to_string);
            // assetGuid is source identity, not a field in the cooked character contract.
            let mut cooked = value;
            cooked.as_object_mut().unwrap().remove("assetGuid");
            let asset: CharacterAsset = serde_json::from_value(cooked)
                .map_err(|e| fail(path, "", "character_invalid", e.to_string()))?;
            asset.validate().map_err(|error| ImportFailure {
                source_path: path.into(),
                error,
            })?;
            authored.push((asset, guid));
        }
    }
    let mut assets: BTreeMap<&str, (&CharacterAsset, Option<&str>)> = imports
        .iter()
        .filter_map(|i| i.character.as_ref())
        .map(|(output, asset)| {
            (
                asset.asset_id.as_str(),
                (asset, Some(output.asset_guid.as_str())),
            )
        })
        .collect();
    assets.extend(
        authored
            .iter()
            .map(|(a, guid)| (a.asset_id.as_str(), (a, guid.as_deref()))),
    );

    for path in source
        .paths()
        .filter(|p| (p.starts_with("Scenes/") || p.starts_with("Prefabs/")) && p.ends_with(".json"))
    {
        let document: Value = serde_json::from_slice(source.bytes(path).unwrap())
            .map_err(|e| fail(path, "", "schema_invalid", e.to_string()))?;
        let Some(entities) = document["entities"].as_array() else {
            continue;
        };
        let targets: BTreeMap<&str, &Value> = entities
            .iter()
            .filter_map(|e| e["id"].as_str().map(|id| (id, e)))
            .collect();
        for (i, entity) in entities.iter().enumerate() {
            let Some(components) = entity["components"].as_array() else {
                continue;
            };
            for (j, component) in components
                .iter()
                .enumerate()
                .filter(|(_, c)| c["componentType"] == "engine.character3d")
            {
                let field = format!("/entities/{i}/components/{j}/data");
                let binding: CharacterBinding3D = serde_json::from_value(component["data"].clone())
                    .map_err(|e| fail(path, &field, "character_invalid", e.to_string()))?;
                binding.validate().map_err(|e| {
                    fail(
                        path,
                        &format!(
                            "{field}{}",
                            e.field_path
                                .strip_prefix("/binding")
                                .unwrap_or(&e.field_path)
                        ),
                        "character_invalid",
                        e.message,
                    )
                })?;
                let lookup = |reference: &engine_runtime::spatial_contract::SpatialAssetRef,
                              suffix: &str| {
                    assets.get(reference.0.id.as_str()).copied().filter(|(_, guid)| reference.0.guid.as_deref().is_none_or(|g| Some(g) == *guid))
                        .map(|(asset, _)| asset)
                        .ok_or_else(|| fail(path, &format!("{field}{suffix}"), "character_invalid", format!("Expected a captured character with matching id/GUID; actual {} / {:?}.", reference.0.id, reference.0.guid)))
                };
                let asset = lookup(&binding.character, "/character")?;
                if binding.joints.len() != asset.joints.len() {
                    return Err(fail(
                        path,
                        &format!("{field}/joints"),
                        "character_invalid",
                        format!(
                            "Expected {} ordered rig targets; actual {}.",
                            asset.joints.len(),
                            binding.joints.len()
                        ),
                    ));
                }
                for (index, joint) in binding.joints.iter().enumerate() {
                    let target = targets.get(joint.entity_ref.as_str());
                    let has_transform = target
                        .and_then(|v| v["components"].as_array())
                        .is_some_and(|cs| cs.iter().any(|c| c["componentType"] == "Transform3D"));
                    let expected_parent = asset.joints[index]
                        .parent
                        .map(|parent| binding.joints[parent as usize].entity_ref.as_str());
                    if !has_transform
                        || expected_parent.is_some_and(|parent| {
                            target.and_then(|v| v["parentId"].as_str()) != Some(parent)
                        })
                    {
                        return Err(fail(path, &format!("{field}/joints/{index}/entityRef"), "character_invalid", format!("Expected a Transform3D target matching rig joint {} and parent {:?}; actual {}.", asset.joints[index].key, expected_parent, joint.entity_ref)));
                    }
                }
                let animation_asset = if let Some(retarget) = &binding.retarget {
                    let source_asset = lookup(&retarget.source, "/retarget/source")?;
                    for (name, expected, actual) in [
                        (
                            "sourceAsset",
                            source_asset.asset_id.as_str(),
                            retarget.mapping.source_asset.as_str(),
                        ),
                        (
                            "targetAsset",
                            asset.asset_id.as_str(),
                            retarget.mapping.target_asset.as_str(),
                        ),
                    ] {
                        if expected != actual {
                            return Err(fail(
                                path,
                                &format!("{field}/retarget/mapping/{name}"),
                                "character_invalid",
                                format!("Expected captured rig {expected}; actual {actual}."),
                            ));
                        }
                    }
                    let pose = SampledCharacterPose {
                        joints: source_asset.joints.iter().map(|j| j.bind.clone()).collect(),
                        morph_weights: vec![],
                    };
                    retarget
                        .mapping
                        .apply(source_asset, asset, &pose)
                        .map_err(|e| {
                            fail(
                                path,
                                &format!(
                                    "{field}/retarget/mapping{}",
                                    e.field_path
                                        .strip_prefix("/retarget")
                                        .unwrap_or(&e.field_path)
                                ),
                                "character_invalid",
                                e.message,
                            )
                        })?;
                    source_asset
                } else {
                    asset
                };
                if let Some(graph) = &binding.graph {
                    for (index, state) in graph.states.iter().enumerate() {
                        if !animation_asset.clips.iter().any(|c| c.key == state.clip) {
                            return Err(fail(
                                path,
                                &format!("{field}/graph/states/{index}/clip"),
                                "character_invalid",
                                format!(
                                    "Expected a clip from rig {}; actual {}.",
                                    animation_asset.asset_id, state.clip
                                ),
                            ));
                        }
                    }
                    graph.validate(animation_asset).map_err(|e| {
                        fail(
                            path,
                            &format!("{field}/graph{}", e.field_path),
                            "character_invalid",
                            e.message,
                        )
                    })?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn direct_assembly_rejects_wrong_clip_and_rig_then_accepts_corrected_source() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../samples/platform_3d_action_project");
        let files = [
            "Assets/character.import.json",
            "Models/character.gltf",
            "Models/character.bin",
            "Scenes/main.json",
        ]
        .into_iter()
        .map(|path| (path.to_string(), std::fs::read(root.join(path)).unwrap()))
        .collect();
        let mut source = CompilerSourceView { files };
        let original: Value = serde_json::from_slice(&source.files["Scenes/main.json"]).unwrap();
        let actor = original["entities"]
            .as_array()
            .unwrap()
            .iter()
            .position(|e| e["id"] == "walker")
            .unwrap();
        let component = original["entities"][actor]["components"]
            .as_array()
            .unwrap()
            .iter()
            .position(|c| c["componentType"] == "engine.character3d")
            .unwrap();
        let field = format!("/entities/{actor}/components/{component}/data");
        let compile = |source: &CompilerSourceView| {
            super::super::compile_cached(source, None, "s5-binding-test", &mut Vec::new(), &|| {
                false
            })
        };
        assert!(compile(&source).is_ok());
        for (suffix, change) in [
            ("/graph/states/0/clip", json!("unknown-clip")),
            ("/retarget/mapping/sourceAsset", json!("wrong-rig")),
        ] {
            let mut invalid = original.clone();
            if suffix.starts_with("/retarget") {
                let reference = invalid
                    .pointer(&format!("{field}/character"))
                    .unwrap()
                    .clone();
                let id = reference["id"].clone();
                invalid.pointer_mut(&field).unwrap()["retarget"] = json!({"source": reference, "mapping": {"sourceAsset":id,"targetAsset":id,"joints":[{"source":"root","target":"root","translationScale":1.0}]}});
            }
            let pointer = format!("{field}{suffix}");
            *invalid.pointer_mut(&pointer).unwrap() = change;
            source.files.insert(
                "Scenes/main.json".into(),
                serde_json::to_vec(&invalid).unwrap(),
            );
            let error = compile(&source)
                .err()
                .expect("invalid source rejected before publication");
            assert_eq!(error.source_path, "Scenes/main.json");
            assert_eq!(error.error.field_path, pointer);
            source.files.insert(
                "Scenes/main.json".into(),
                serde_json::to_vec(&original).unwrap(),
            );
            assert!(compile(&source).is_ok());
        }
    }
}
