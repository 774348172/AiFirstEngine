//! Lower heightfields into ordinary mesh/collision/navigation source documents.
//! Existing Cook recipes and package/resource owners retain publication authority.
use super::{assembly_error, CompilerSourceView, GameProjectCompilerError};
use engine_runtime::{
    canonical_digest::sha256_prefixed,
    render_contract::{RenderAssetData, RenderContract},
    spatial_contract::strict_json,
    terrain3d::{
        HeightfieldAsset3D, TerrainAssetIdentity3D, VegetationAsset3D, HEIGHTFIELD_RECIPE_3D,
    },
};
use serde_json::{json, Value};
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
};

fn failure(path: &str, message: impl std::fmt::Display) -> GameProjectCompilerError {
    assembly_error("terrain3d.cook_invalid", format!("{path}: {message}"))
}

pub(crate) fn expand(
    source: &CompilerSourceView,
) -> Result<Cow<'_, CompilerSourceView>, GameProjectCompilerError> {
    let mut terrains = Vec::new();
    let mut identities = BTreeSet::new();
    let mut guids = BTreeSet::new();
    let spatial = source.project_manifest_value()?["schemaVersion"].as_str()
        == Some(crate::SPATIAL_PROJECT_MANIFEST_SCHEMA_VERSION);
    for path in source.paths().filter(|p| {
        p.starts_with("Assets/") && (p.ends_with(".asset") || (spatial && p.ends_with(".json")))
    }) {
        let Some(bytes) = source.bytes(path) else {
            continue;
        };
        let Ok(probe) = serde_json::from_slice::<Value>(bytes) else {
            continue;
        };
        for (field, seen) in [("assetId", &mut identities), ("assetGuid", &mut guids)] {
            if let Some(value) = probe[field].as_str() {
                seen.insert(value.to_owned());
            }
        }
        match probe["schemaVersion"].as_str() {
            Some("terrain3d-asset.v1") => {
                let strict = strict_json(bytes).map_err(|e| failure(path, e))?;
                let terrain: HeightfieldAsset3D =
                    serde_json::from_value(strict).map_err(|e| failure(path, e))?;
                terrain.validate().map_err(|e| failure(path, e))?;
                terrains.push((path.to_owned(), terrain));
            }
            Some("vegetation3d-asset.v1") => {
                let strict = strict_json(bytes).map_err(|e| failure(path, e))?;
                let vegetation: VegetationAsset3D =
                    serde_json::from_value(strict).map_err(|e| failure(path, e))?;
                vegetation.validate().map_err(|e| failure(path, e))?;
            }
            _ => {}
        }
    }
    if terrains.is_empty() {
        return Ok(Cow::Borrowed(source));
    }
    // Claim every derived identity before constructing bytes, so any failure
    // leaves the operation snapshot and prior publication unchanged.
    for (path, terrain) in &terrains {
        for tile in &terrain.tiles {
            for identity in tile
                .lods
                .iter()
                .map(|lod| &lod.mesh)
                .chain(tile.collision.iter())
                .chain(tile.navigation.iter().map(|nav| &nav.asset))
            {
                claim(path, identity, &mut identities, &mut guids)?;
            }
        }
    }
    let mut expanded = source.clone();
    for (path, terrain) in terrains {
        // Neighbour halos consume the same immutable full-density mesh. Keep
        // it only for this terrain expansion, never across source snapshots.
        let mut collision_meshes = BTreeMap::new();
        // Path namespace is stable across height/material edits. Producer keys
        // depend on individual generated document bytes, retaining tile locality.
        let digest = sha256_prefixed(path.as_bytes());
        let token = digest.strip_prefix("sha256:").unwrap_or(&digest);
        let directory = format!("Assets/TerrainCook/{token}");
        let mut tile_manifests = Vec::new();
        for (tile_index, tile) in terrain.tiles.iter().enumerate() {
            for lod in &tile.lods {
                let geometry = terrain
                    .build_tile_mesh(&tile.key, lod.step)
                    .map_err(|e| failure(&path, e))?;
                let document = json!({
                    "schemaVersion":"mesh-asset.v1", "assetId":lod.mesh.asset_id,
                    "assetGuid":lod.mesh.asset_guid,
                    "renderData":RenderContract::new(RenderAssetData::Mesh(geometry)),
                });
                insert(
                    &mut expanded,
                    format!("{directory}/tile-{tile_index}-lod-{}.asset", lod.step),
                    &document,
                )?;
            }
            if tile.collision.is_some() || tile.navigation.is_some() {
                let geometry = build_collision(&terrain, &tile.key, &mut collision_meshes)
                    .map_err(|e| failure(&path, e))?;
                if let Some(identity) = &tile.collision {
                    let document = json!({
                        "schemaVersion":"collision3d-asset.v1", "assetId":identity.asset_id,
                        "assetGuid":identity.asset_guid,
                        "shape":{"kind":"staticMesh", "vertices":geometry.vertices,"triangles":geometry.triangles},
                    });
                    insert(
                        &mut expanded,
                        format!("{directory}/tile-{tile_index}-collision.asset"),
                        &document,
                    )?;
                }
                if let Some(nav) = &tile.navigation {
                    // Recast erodes walkable spans before trimming its padded
                    // border. Real neighbour geometry must occupy that border,
                    // otherwise a streamed tile seam becomes a false cliff.
                    // The output remains one independently owned local tile.
                    let geometry =
                        navigation_halo(&terrain, tile, &nav.profile, &mut collision_meshes)
                            .map_err(|e| failure(&path, e))?;
                    let min = geometry
                        .vertices
                        .iter()
                        .map(|p| p[1])
                        .fold(f32::INFINITY, f32::min);
                    let max = geometry
                        .vertices
                        .iter()
                        .map(|p| p[1])
                        .fold(f32::NEG_INFINITY, f32::max);
                    let margin = nav.profile.height + nav.profile.climb + 1.;
                    let document = json!({
                        "schemaVersion":"navigation3d-asset.v1", "assetId":nav.asset.asset_id,
                        "assetGuid":nav.asset.asset_guid, "profile":nav.profile,
                        "vertices":geometry.vertices,"triangles":geometry.triangles,
                        "tiles":[[0,0]],"verticalBounds":[min-margin,max+margin],"offMeshLinks":[],
                    });
                    insert(
                        &mut expanded,
                        format!("{directory}/tile-{tile_index}-navigation.asset"),
                        &document,
                    )?;
                }
            }
            tile_manifests.push(json!({"key":tile.key,"coordinate":tile.coordinate,
                "lods":tile.lods,"collision":tile.collision,"navigation":tile.navigation,
            }));
        }
        // The source identity remains a compact manifest. Runtime packages do
        // not retain height samples in addition to all cooked vertex buffers.
        let manifest = json!({"schemaVersion":"terrain3d-manifest-asset.v1",
            "assetId":terrain.asset_id,"assetGuid":terrain.asset_guid,
            "recipe":HEIGHTFIELD_RECIPE_3D,"tileWidth":terrain.tile_width(),
            "materialSlots":terrain.material_slots,"tiles":tile_manifests,
        });
        expanded.files.insert(
            path.clone(),
            serde_json::to_vec(&manifest).map_err(|e| failure(&path, e))?,
        );
    }
    Ok(Cow::Owned(expanded))
}

fn navigation_halo(
    source: &HeightfieldAsset3D,
    tile: &engine_runtime::terrain3d::HeightfieldTile3D,
    profile: &engine_runtime::navigation_3d::AgentProfile3D,
    collision_meshes: &mut BTreeMap<String, engine_runtime::terrain3d::TerrainCollisionMesh3D>,
) -> Result<engine_runtime::terrain3d::TerrainCollisionMesh3D, String> {
    let width = source.tile_width();
    let padding = ((profile.radius / profile.cell_size).ceil() + 3.) * profile.cell_size;
    let mut output = engine_runtime::terrain3d::TerrainCollisionMesh3D {
        vertices: Vec::new(),
        triangles: Vec::new(),
    };
    for neighbour in &source.tiles {
        let delta = [
            (i64::from(neighbour.coordinate[0]) - i64::from(tile.coordinate[0])) as f64
                * f64::from(width),
            (i64::from(neighbour.coordinate[1]) - i64::from(tile.coordinate[1])) as f64
                * f64::from(width),
        ];
        if delta
            .iter()
            .any(|d| *d + f64::from(width) < -f64::from(padding) || *d > f64::from(width + padding))
        {
            continue;
        }
        let mesh = build_collision(source, &neighbour.key, collision_meshes)?;
        for triangle in &mesh.triangles {
            let points = triangle.map(|index| {
                let mut p = mesh.vertices[index as usize];
                p[0] += delta[0] as f32;
                p[2] += delta[1] as f32;
                p
            });
            if [0, 2].iter().any(|axis| {
                points
                    .iter()
                    .map(|p| p[*axis])
                    .fold(f32::NEG_INFINITY, f32::max)
                    < -padding
                    || points
                        .iter()
                        .map(|p| p[*axis])
                        .fold(f32::INFINITY, f32::min)
                        > width + padding
            }) {
                continue;
            }
            let base = u32::try_from(output.vertices.len())
                .map_err(|_| "Navigation halo vertex index overflow.".to_string())?;
            output.vertices.extend(points);
            output.triangles.push([base, base + 1, base + 2]);
        }
    }
    Ok(output)
}

fn claim(
    path: &str,
    identity: &TerrainAssetIdentity3D,
    ids: &mut BTreeSet<String>,
    guids: &mut BTreeSet<String>,
) -> Result<(), GameProjectCompilerError> {
    if !ids.insert(identity.asset_id.clone()) || !guids.insert(identity.asset_guid.clone()) {
        return Err(failure(
            path,
            format!(
                "Derived asset identity already exists: {} / {}",
                identity.asset_id, identity.asset_guid
            ),
        ));
    }
    Ok(())
}
fn insert(
    source: &mut CompilerSourceView,
    path: String,
    document: &Value,
) -> Result<(), GameProjectCompilerError> {
    if source.files.contains_key(&path) {
        return Err(failure(
            &path,
            "Generated terrain path collides with an existing source file.",
        ));
    }
    let bytes = serde_json::to_vec(document).map_err(|e| failure(&path, e))?;
    source.files.insert(path, bytes);
    Ok(())
}

#[cfg(test)]
std::thread_local! { static COLLISION_BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
fn build_collision<'a>(
    source: &HeightfieldAsset3D,
    key: &str,
    meshes: &'a mut BTreeMap<String, engine_runtime::terrain3d::TerrainCollisionMesh3D>,
) -> Result<&'a engine_runtime::terrain3d::TerrainCollisionMesh3D, String> {
    if !meshes.contains_key(key) {
        #[cfg(test)]
        COLLISION_BUILDS.with(|count| count.set(count.get() + 1));
        meshes.insert(
            key.to_owned(),
            source
                .build_tile_collision(key)
                .map_err(|e| e.to_string())?,
        );
    }
    Ok(meshes.get(key).unwrap())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn source() -> CompilerSourceView {
        let identity = |id: String| json!({"assetId":id,"assetGuid":format!("guid-{id}")});
        let tiles=(0..9).map(|i| json!({
            "key":i.to_string(),"coordinate":[i%3,i/3],"heights":vec![0.;81],
            "lods":[{"step":1,"mesh":identity(format!("fine-{i}"))},{"step":4,"mesh":identity(format!("coarse-{i}"))}],
            "collision":identity(format!("collision-{i}")),
            "navigation":{"asset":identity(format!("nav-{i}")),"profile":{"key":"human","radius":0.3,"height":1.8,"climb":0.3,"maxSlopeDegrees":45,"cellSize":0.2,"cellHeight":0.1,"tileWidth":16}}
        })).collect::<Vec<_>>();
        let terrain = json!({"schemaVersion":"terrain3d-asset.v1","assetId":"terrain","assetGuid":"guid-terrain","quads":8,"sampleSpacing":2,"materialSlots":["surface"],"tiles":tiles});
        CompilerSourceView {
            files: std::collections::BTreeMap::from([
                (
                    "project.aife.json".into(),
                    br#"{"schemaVersion":"aife-project.v3"}"#.to_vec(),
                ),
                (
                    "Assets/terrain.asset".into(),
                    serde_json::to_vec(&terrain).unwrap(),
                ),
            ]),
        }
    }
    #[test]
    fn navigation_halos_build_each_collision_once_and_keep_original_output() {
        let source = source();
        COLLISION_BUILDS.with(|count| count.set(0));
        let output = expand(&source).unwrap();
        let digest = sha256_prefixed(&serde_json::to_vec(&output.files).unwrap());
        assert_eq!(
            digest, "sha256:01716a062b9ad2744603c31b6a39ace8217db1fae89c426bea120d4e1686b4e8",
            "all original mesh/collision/navigation/manifest bytes must remain identical"
        );
        assert_eq!(
            COLLISION_BUILDS.with(|count| count.get()),
            9,
            "one immutable terrain operation must reuse full-density neighbour geometry"
        );
        let mut changed = source.clone();
        let mut terrain: Value =
            serde_json::from_slice(&changed.files["Assets/terrain.asset"]).unwrap();
        terrain["tiles"][4]["heights"][40] = json!(0.2);
        changed.files.insert(
            "Assets/terrain.asset".into(),
            serde_json::to_vec(&terrain).unwrap(),
        );
        COLLISION_BUILDS.with(|count| count.set(0));
        let changed_output = expand(&changed).unwrap();
        assert_ne!(
            sha256_prefixed(&serde_json::to_vec(&changed_output.files).unwrap()),
            digest,
            "a new source snapshot must rebuild its own meshes"
        );
        assert_eq!(COLLISION_BUILDS.with(|count| count.get()), 9);
    }
}
