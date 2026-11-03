//! Spatial consumption of the existing source-side Prefab instance contract.
use super::*;
use crate::three_d_contract::{SpatialPrefabInstance, SpatialPrefabOverride, SpatialSourceScene};
use engine_runtime::spatial_contract::*;
use serde_json::json;

pub(super) fn expand(
    source: &SpatialSourceScene,
    runtime: &mut RuntimeSceneV2,
    prefabs: &[RuntimePackageSourcePrefab],
) -> ContractResult<()> {
    let mut entities = Vec::new();
    for (i, (original, placement)) in source.entities.iter().zip(&runtime.entities).enumerate() {
        let Some((component_index, component)) = original
            .components
            .iter()
            .enumerate()
            .find(|(_, c)| c.component_type == "engine.prefab_instance")
        else {
            entities.push(placement.clone());
            continue;
        };
        let path = format!("/entities/{i}/components/{component_index}/data");
        let result = (|| -> ContractResult<Vec<RuntimeEntityDocument>> {
            let instance: SpatialPrefabInstance = decode(component.data.clone(), "")?;
            instance.validate()?;
            let prefab = prefabs
                .iter()
                .find(|p| p.prefab_id == instance.source.0.id)
                .ok_or_else(|| invalid("/source", "Referenced prefab is absent."))?;
            if let Some(identity) = &prefab.identity {
                if instance.source.0.guid.as_deref() != Some(&identity.asset_guid)
                    || instance.source.0.sub_asset.as_deref() != Some(&identity.sub_asset_id)
                {
                    return Err(invalid(
                        "/source",
                        "Imported prefab requires its exact committed GUID and subAsset key.",
                    ));
                }
            } else if instance
                .source
                .0
                .guid
                .as_deref()
                .is_some_and(|guid| guid != prefab.prefab_id)
                || instance.source.0.sub_asset.is_some()
            {
                return Err(invalid(
                    "/source",
                    "Native prefab reference identity does not match.",
                ));
            }
            let mut prefab: RuntimePrefabV2 = decode(prefab.document.clone(), "/source")?;
            prefab.validate_and_canonicalize()?;
            for (index, value) in instance.overrides.iter().enumerate() {
                let target = prefab
                    .entities
                    .iter_mut()
                    .find(|e| e.id() == value.target_source_entity_id)
                    .ok_or_else(|| {
                        invalid(
                            &format!("/overrides/{index}/targetSourceEntityId"),
                            "Override target was removed or is not part of this prefab.",
                        )
                    })?;
                if target.id() == prefab.root_entity_id && value.component_type == "Transform3D" {
                    return Err(invalid(
                        &format!("/overrides/{index}"),
                        "Place the instance root with its source scene Transform3D.",
                    ));
                }
                apply(target, value).map_err(|e| e.at(&format!("/overrides/{index}")))?;
            }
            let RuntimeEntityDocument::Spatial(placement) = placement else {
                return Err(invalid(
                    "",
                    "Spatial instance needs a spatial placement root.",
                ));
            };
            let mut result = Vec::new();
            for entity in prefab.entities {
                let RuntimeEntityDocument::Spatial(mut entity) = entity else {
                    return Err(invalid(
                        "/source",
                        "Spatial instances currently require an entirely spatial prefab.",
                    ));
                };
                let is_root = entity.id == prefab.root_entity_id;
                entity.id = prefab_runtime_id(&placement.id, &prefab.root_entity_id, &entity.id);
                if is_root {
                    entity.name = placement.name.clone();
                    entity.enabled &= placement.enabled;
                    entity.parent_id = placement.parent_id.clone();
                    entity.sibling_order = placement.sibling_order;
                    let RuntimeSpatial::World3D {
                        transform,
                        visibility,
                        ..
                    } = &placement.spatial;
                    let RuntimeSpatial::World3D {
                        transform: dest,
                        visibility: dest_visible,
                        ..
                    } = &mut entity.spatial;
                    *dest = transform.clone();
                    if let Some(visibility) = visibility {
                        *dest_visible = Some(Visibility3D {
                            schema_version: "visibility3d.v1".into(),
                            visible: visibility.visible
                                && dest_visible.as_ref().is_none_or(|v| v.visible),
                        });
                    }
                    entity.components.extend(placement.components.clone());
                } else {
                    entity.parent_id = entity.parent_id.as_ref().map(|parent| {
                        prefab_runtime_id(&placement.id, &prefab.root_entity_id, parent)
                    });
                }
                result.push(RuntimeEntityDocument::Spatial(entity));
            }
            Ok(result)
        })()
        .map_err(|e| e.at(&path))?;
        entities.extend(result);
    }
    runtime.entities = entities;
    runtime.validate_and_canonicalize().map_err(|e| {
        invalid(
            "/entities",
            &format!("Expanded instance validation failed: {e}"),
        )
    })
}
fn invalid(path: &str, message: &str) -> SpatialContractError {
    SpatialContractError::new("prefab_override_invalid", path, message)
}
fn apply(entity: &mut RuntimeEntityDocument, value: &SpatialPrefabOverride) -> ContractResult<()> {
    let RuntimeEntityDocument::Spatial(entity) = entity else {
        return Err(invalid(
            "/targetSourceEntityId",
            "Override target is not spatial.",
        ));
    };
    let RuntimeSpatial::World3D {
        transform,
        mesh_renderer,
        visibility,
        ..
    } = &mut entity.spatial;
    match value.component_type.as_str() {
        "MeshRenderer3D" => {
            let renderer = mesh_renderer
                .as_mut()
                .ok_or_else(|| invalid("/componentType", "Target has no MeshRenderer3D."))?;
            let slot = value
                .field_path
                .strip_prefix("materials.")
                .ok_or_else(|| invalid("/fieldPath", "Use materials.<persistent slot name>."))?;
            let material = renderer.materials.get_mut(slot).ok_or_else(|| {
                invalid(
                    "/fieldPath",
                    "Named material slot was removed or does not exist.",
                )
            })?;
            *material = decode(value.value.clone(), "/value")?;
        }
        "Transform3D" => {
            if !["translation", "rotation", "scale"].contains(&value.field_path.as_str()) {
                return Err(invalid(
                    "/fieldPath",
                    "Expected translation, rotation or scale.",
                ));
            }
            let mut data = json!({"schemaVersion":"transform3d.v1","translation":transform.translation,"rotation":{"kind":"quaternion","xyzw":transform.rotation_xyzw},"scale":transform.scale});
            data[&value.field_path] = value.value.clone();
            *transform = decode::<Transform3D>(data, "/value")?.canonicalize()?;
        }
        "Visibility3D" if value.field_path == "visible" => {
            let visible = decode::<bool>(value.value.clone(), "/value")?;
            *visibility = Some(Visibility3D {
                schema_version: "visibility3d.v1".into(),
                visible,
            });
        }
        kind if kind.starts_with("project.") => {
            let component = entity
                .components
                .iter_mut()
                .find(|c| c.component_type == kind)
                .ok_or_else(|| invalid("/componentType", "Project component is absent."))?;
            let pointer = format!(
                "/{}",
                value
                    .field_path
                    .split('.')
                    .map(|s| s.replace('~', "~0").replace('/', "~1"))
                    .collect::<Vec<_>>()
                    .join("/")
            );
            let target = component
                .data
                .pointer_mut(&pointer)
                .ok_or_else(|| invalid("/fieldPath", "Project component field does not exist."))?;
            *target = value.value.clone();
        }
        _ => {
            return Err(invalid(
                "/componentType",
                "Unsupported spatial prefab override.",
            ))
        }
    }
    entity
        .validate_and_canonicalize()
        .map_err(|e| invalid("/value", &e.to_string()))
}
