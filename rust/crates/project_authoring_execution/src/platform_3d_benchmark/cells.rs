//! Offline foundation payload qualification, using the original prepared package.
use super::{BenchmarkError, BenchmarkResult};
use crate::PreparedRuntimePackage;
use engine_runtime::canonical_digest::sha256_prefixed;
use engine_runtime::render_contract::{CookedRenderAsset, RenderAssetData, RenderContract};
use engine_runtime::runtime_package_builder::RuntimePackageBuildInput;
use engine_runtime::spatial_contract::{RuntimePrefabV2, RuntimeSpatial};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundationCellPayload {
    pub prefab_id: String,
    pub render_instances: usize,
    pub mesh_bytes: u64,
    pub base_color_bytes: u64,
    pub unique_upload_bytes: u64,
    pub mesh_assets: BTreeSet<String>,
    pub material_assets: BTreeSet<String>,
    pub texture_assets: BTreeSet<String>,
    pub payload_digests: BTreeSet<String>,
}

/// Count actual enabled prefab render bindings and decoded payloads. Content shared
/// with the other cell contributes zero to the required unique byte threshold.
/// This is source/Cook proof; it does not establish residency, pixels or timing.
pub fn inspect_foundation_cells(
    prepared: &PreparedRuntimePackage,
) -> BenchmarkResult<Vec<FoundationCellPayload>> {
    inspect_input(prepared.runtime_package_build_input())
}

fn inspect_input(input: &RuntimePackageBuildInput) -> BenchmarkResult<Vec<FoundationCellPayload>> {
    inspect_cells(input, 2)
}

pub(super) fn inspect_world_cells(
    prepared: &PreparedRuntimePackage,
) -> BenchmarkResult<serde_json::Value> {
    let input = prepared.runtime_package_build_input();
    let cells = inspect_cells(input, 256)?;
    let invalid = |message| BenchmarkError::new("benchmark.cell_payload_ineligible", message);
    let mut textures = BTreeSet::new();
    let mut bytes = 0u64;
    for (index, cell) in cells.iter().enumerate() {
        // The ordinary source has a stable cell ID; do not infer qualification
        // from a manifest count when a real binding or unique payload is absent.
        if !cells
            .iter()
            .any(|c| c.prefab_id == format!("cell-{index:03}"))
            || cell.render_instances != 2
            || cell.texture_assets.len() != 2
            || cell.base_color_bytes != 32 * 1024 * 1024
        {
            return Err(invalid(
                "full 16x16 cells each require two actual 2048² RGBA consumers",
            ));
        }
        bytes += cell.unique_upload_bytes;
        for id in &cell.texture_assets {
            if !textures.insert(id) {
                return Err(invalid("texture identity shared across cells"));
            }
        }
    }
    if textures.len() != 512 || bytes < 8 * 1024 * 1024 * 1024 {
        return Err(invalid(
            "actual unique GPU/CPU texture dataset is below 8 GiB",
        ));
    }
    Ok(
        serde_json::json!({"scope":"BM08-world-v1-actual-Cook-payloads","cells":cells,"textureCount":textures.len(),"uniqueDecodedBytes":bytes,"qualification":"source-Cook only; runtime read/upload/residency/trajectory/performance not proved"}),
    )
}

fn inspect_cells(
    input: &RuntimePackageBuildInput,
    expected_cells: usize,
) -> BenchmarkResult<Vec<FoundationCellPayload>> {
    let invalid = |message| BenchmarkError::new("benchmark.cell_payload_ineligible", message);
    if input.prefabs.len() != expected_cells {
        return Err(invalid(format!(
            "exactly {expected_cells} real prefabs required"
        )));
    }
    let mut cooked = BTreeMap::new();
    for asset in &input.assets {
        if !matches!(asset.asset_type.as_str(), "mesh" | "material") {
            continue;
        }
        let bytes = asset
            .runtime_payload
            .as_ref()
            .ok_or_else(|| invalid(asset.asset_id.clone()))?;
        let value = RenderContract::<CookedRenderAsset>::decode(
            std::str::from_utf8(bytes).map_err(|e| invalid(e.to_string()))?,
        )
        .map_err(|e| invalid(e.to_string()))?
        .data
        .asset;
        cooked.insert(asset.asset_id.as_str(), value);
    }
    let textures: BTreeMap<_, _> = input
        .texture_payloads
        .iter()
        .map(|v| (v.metadata.asset_id.as_str(), v))
        .collect();
    let mut results = Vec::new();
    let mut all_bytes = Vec::new();
    for prefab in &input.prefabs {
        let mut document: RuntimePrefabV2 =
            serde_json::from_value(prefab.document.clone()).map_err(|e| invalid(e.to_string()))?;
        document
            .validate_and_canonicalize()
            .map_err(|e| invalid(e.to_string()))?;
        let mut result = FoundationCellPayload {
            prefab_id: prefab.prefab_id.clone(),
            render_instances: 0,
            mesh_bytes: 0,
            base_color_bytes: 0,
            unique_upload_bytes: 0,
            mesh_assets: BTreeSet::new(),
            material_assets: BTreeSet::new(),
            texture_assets: BTreeSet::new(),
            payload_digests: BTreeSet::new(),
        };
        let mut payloads = BTreeMap::new();
        let hidden: BTreeSet<_> = document
            .entities
            .iter()
            .filter(|e| {
                !e.enabled()
                    || matches!(e.spatial(), Some(RuntimeSpatial::World3D {
                visibility: Some(v), ..
            }) if !v.visible)
            })
            .map(|e| e.id())
            .collect();
        let parents: BTreeMap<_, _> = document
            .entities
            .iter()
            .map(|e| (e.id(), e.parent_id()))
            .collect();
        for entity in &document.entities {
            let mut ancestor = Some(entity.id());
            let mut visible = true;
            while let Some(id) = ancestor {
                if hidden.contains(id) {
                    visible = false;
                    break;
                }
                ancestor = parents.get(id).copied().flatten();
            }
            let Some(RuntimeSpatial::World3D {
                mesh_renderer: Some(renderer),
                ..
            }) = entity.spatial()
            else {
                continue;
            };
            if !visible || !renderer.visible {
                continue;
            }
            result.render_instances += 1;
            let mesh_id = renderer.mesh.0.id.as_str();
            let Some(RenderAssetData::Mesh(mesh)) = cooked.get(mesh_id) else {
                return Err(invalid(format!("missing mesh {mesh_id}")));
            };
            if result.mesh_assets.insert(mesh_id.into()) {
                let bytes = (mesh.vertex_bytes.len() + mesh.index_bytes.len()) as u64;
                result.mesh_bytes += bytes;
                let digest = sha256_prefixed(
                    &[mesh.vertex_bytes.as_slice(), mesh.index_bytes.as_slice()].concat(),
                );
                payloads.entry(digest).or_insert(bytes);
            }
            for reference in renderer.materials.values() {
                let material_id = reference.0.id.as_str();
                result.material_assets.insert(material_id.into());
                let Some(RenderAssetData::Material(material)) = cooked.get(material_id) else {
                    return Err(invalid(format!("missing material {material_id}")));
                };
                let reference = material
                    .textures
                    .get("baseColorTexture")
                    .ok_or_else(|| invalid(format!("material {material_id} lacks baseColor")))?;
                let texture_id = reference.reference.id.as_str();
                let texture = textures
                    .get(texture_id)
                    .ok_or_else(|| invalid(format!("missing texture {texture_id}")))?;
                let expected =
                    u64::from(texture.metadata.width) * u64::from(texture.metadata.height) * 4;
                if texture.rgba8.len() as u64 != expected
                    || expected == 0
                    || texture
                        .rgba8
                        .chunks_exact(4)
                        .all(|p| p == &texture.rgba8[..4])
                {
                    return Err(invalid(format!(
                        "nonconstant actual RGBA payload required: {texture_id}"
                    )));
                }
                if result.texture_assets.insert(texture_id.into()) {
                    result.base_color_bytes += expected;
                    payloads
                        .entry(sha256_prefixed(&texture.rgba8))
                        .or_insert(expected);
                }
            }
        }
        if result.render_instances == 0 {
            return Err(invalid("no enabled render consumer".into()));
        }
        result.payload_digests = payloads.keys().cloned().collect();
        all_bytes.push(payloads);
        results.push(result);
    }
    for index in 0..results.len() {
        results[index].unique_upload_bytes = all_bytes[index]
            .iter()
            .filter(|(digest, _)| {
                all_bytes
                    .iter()
                    .enumerate()
                    .all(|(other, payloads)| other == index || !payloads.contains_key(*digest))
            })
            .map(|(_, bytes)| bytes)
            .sum();
        if results[index].unique_upload_bytes < 32 * 1024 * 1024 {
            return Err(invalid(format!(
                "{}: {} unique bytes, 32 MiB required",
                results[index].prefab_id, results[index].unique_upload_bytes
            )));
        }
    }
    Ok(results)
}

pub(super) fn validate_world_scenario(
    scenario: &runtime_player_winit::semantic_outcome::PlaytestScenario,
    frames: u64,
    timeout: u64,
) -> BenchmarkResult<()> {
    use serde_json::json;
    let invalid = || {
        BenchmarkError::new("benchmark.world_scenario_ineligible", "BM08 requires its complete actual route assertions and captures; no timing/read/upload/residency qualification is inferred")
    };
    if frames != 40000
        || scenario.max_simulation_ticks != frames
        || scenario.max_presentation_frames != frames
        || scenario.timeout_ms > timeout
        || scenario.target
            != runtime_player_winit::semantic_outcome::PlaytestTarget::WindowsWindowed
    {
        return Err(invalid());
    }
    for (path, value) in [
        ("bm08.tick", json!(40000)),
        ("bm08.completed", json!(true)),
        ("bm08.uniqueVisited", json!(73)),
        ("bm08.firstFourRevisited", json!(true)),
        ("bm08.residentBounded", json!(true)),
        ("bm08.trackTick", json!(36000)),
    ] {
        if !scenario.assertions.iter().any(|a| {
            a.from_simulation_tick == frames
                && a.through_simulation_tick == frames
                && a.path == path
                && serde_json::to_value(&a.equals).ok() == Some(value.clone())
        }) {
            return Err(invalid());
        }
    }
    for frame in [2500, 25000, 40000] {
        if !scenario
            .captures
            .iter()
            .any(|c| c.presentation_frame == frame && c.required && !c.subjective_review)
        {
            return Err(invalid());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GameProjectCompiler, ProjectAuthoringSession, TargetProfile};

    #[test]
    fn full_world_route_rejects_missing_assertions_captures_and_short_windows() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../samples/platform_3d_bm08_project");
        let scenario: runtime_player_winit::semantic_outcome::PlaytestScenario =
            serde_json::from_slice(&std::fs::read(root.join("Tests/bm08.scenario.json")).unwrap())
                .unwrap();
        validate_world_scenario(&scenario, 40000, 600000).unwrap();
        assert!(validate_world_scenario(&scenario, 39999, 600000).is_err());
        for index in 0..scenario.assertions.len() {
            let mut missing = scenario.clone();
            missing.assertions.remove(index);
            assert!(validate_world_scenario(&missing, 40000, 600000).is_err());
        }
        for index in 0..scenario.captures.len() {
            let mut missing = scenario.clone();
            missing.captures.remove(index);
            assert!(validate_world_scenario(&missing, 40000, 600000).is_err());
        }
    }

    fn prepared(root: &std::path::Path) -> PreparedRuntimePackage {
        let mut session = ProjectAuthoringSession::open(root).unwrap();
        let paths = session
            .source_inventory()
            .unwrap()
            .entries
            .into_iter()
            .map(|e| e.relative_path)
            .collect();
        let lease = session
            .acquire_snapshot_lease("371-cell-payload-test", paths)
            .unwrap();
        GameProjectCompiler::bind(&lease)
            .unwrap()
            .prepare(&lease, TargetProfile::WindowsDev)
            .unwrap()
    }

    #[test]
    fn functional_cells_cannot_claim_frozen_foundation_payload() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../samples/platform_3d_streaming_project");
        let prepared = prepared(&root);
        assert_eq!(
            inspect_foundation_cells(&prepared).unwrap_err().code,
            "benchmark.cell_payload_ineligible"
        );
    }

    #[test]
    #[ignore = "requires the generated run-owned 64 MiB foundation project"]
    fn foundation_payload_rejects_shared_hidden_missing_and_truncated_resources() {
        let root = std::env::var_os("AIFE_CELL_PLAYER_PROJECT")
            .map(std::path::PathBuf::from)
            .unwrap();
        let prepared = prepared(&root);
        let baseline = prepared.runtime_package_build_input();
        let cells = inspect_input(baseline).unwrap();
        assert!(cells
            .iter()
            .all(|c| c.render_instances == 2 && c.unique_upload_bytes >= 32 * 1024 * 1024));
        for variant in [
            "shared",
            "constant",
            "truncated",
            "hidden",
            "unbound",
            "missing",
        ] {
            let mut input = baseline.clone();
            match variant {
                "shared" => {
                    input.texture_payloads[2].rgba8 = input.texture_payloads[0].rgba8.clone()
                }
                "constant" => input.texture_payloads[0].rgba8.fill(255),
                "truncated" => {
                    input.texture_payloads[0].rgba8.pop();
                }
                "hidden" => {
                    input.prefabs[0].document["entities"][0]["enabled"] = serde_json::json!(false);
                }
                "unbound" => {
                    for entity in input.prefabs[0].document["entities"]
                        .as_array_mut()
                        .unwrap()
                    {
                        entity["spatial"]["meshRenderer"] = serde_json::Value::Null;
                    }
                }
                "missing" => input.texture_payloads.clear(),
                _ => unreachable!(),
            }
            assert_eq!(
                inspect_input(&input).unwrap_err().code,
                "benchmark.cell_payload_ineligible",
                "{variant}"
            );
        }
    }
}
