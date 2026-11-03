//! Original world payload admission uses the ordinary Compiler and Player.
use super::{BenchmarkError, BenchmarkResult};
use crate::PreparedRuntimePackage;
use engine_runtime::{
    navigation_3d::CookedNavigation3D,
    particle_effect::CookedParticleEffect,
    physics_3d::{CollisionShape3D, CookedCollision3D},
    render_contract::{CookedRenderAsset, RenderAssetData, RenderContract, ValidateRenderContract},
    runtime_package_builder::RuntimePackageBuildInput,
    terrain3d::{VegetationAsset3D, VegetationBinding3D},
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

fn inspect_bm13_raw(input: &RuntimePackageBuildInput) -> Result<Value, String> {
    let assets = input
        .assets
        .iter()
        .map(|a| (a.asset_id.as_str(), a))
        .collect::<BTreeMap<_, _>>();
    if assets.len() != input.assets.len() || input.scenes.len() != 1 {
        return Err("duplicate assets or scene count".into());
    }
    let mut required = vec![
        "bm13-tone".to_string(),
        "bm13-particles".to_string(),
        "bm13-tree-coarse".to_string(),
    ];
    for i in 0..9 {
        for suffix in ["fine", "coarse", "collision", "nav"] {
            required.push(format!("bm13-ground-{i}-{suffix}"));
        }
    }
    for i in 0..4 {
        required.push(format!("bm13-forest-{i}"));
    }
    for id in &required {
        if !assets.contains_key(id.as_str()) {
            return Err(format!("missing actual {id}"));
        }
    }
    let entities = &input.scenes[0].entities;
    let sources = entities
        .iter()
        .filter(|e| {
            e.enabled
                && e.spatial.is_some()
                && e.components
                    .iter()
                    .any(|c| c.component_type == "engine.audio_source")
                && e.components
                    .iter()
                    .any(|c| c.component_type == "engine.spatial_audio3d")
        })
        .count();
    if sources != 64 {
        return Err(format!("actual spatial sources {sources}"));
    }
    let component = |entity: &str, kind: &str| {
        entities
            .iter()
            .find(|e| e.id == entity && e.enabled && e.spatial.is_some())
            .and_then(|e| e.components.iter().find(|c| c.component_type == kind))
            .map(|c| &c.data)
            .ok_or_else(|| format!("missing actual binding {entity}/{kind}"))
    };
    for i in 0..9 {
        let entity = format!("terrain-{i}");
        if component(&entity, "engine.body3d")?["shape"]["source"]["id"]
            != format!("bm13-ground-{i}-collision")
            || component(&entity, "engine.navigation3d")?["navigation"]["id"]
                != format!("bm13-ground-{i}-nav")
            || component(&entity, "engine.model_lod3d")?["levels"]
                .as_array()
                .map(Vec::len)
                != Some(2)
        {
            return Err("terrain binding mismatch".into());
        }
    }
    let payload = |id: &str| {
        assets
            .get(id)
            .and_then(|a| a.runtime_payload.as_deref())
            .ok_or_else(|| format!("missing real cooked bytes {id}"))
    };
    let mut triangles = 0u64;
    for i in 0..9 {
        for suffix in ["fine", "coarse"] {
            let id = format!("bm13-ground-{i}-{suffix}");
            let cooked: RenderContract<CookedRenderAsset> =
                serde_json::from_slice(payload(&id)?).map_err(|e| e.to_string())?;
            cooked.data.validate().map_err(|e| e.to_string())?;
            let RenderAssetData::Mesh(mesh) = cooked.data.asset else {
                return Err("non-mesh terrain output".into());
            };
            if suffix == "fine" {
                if mesh.vertex_count != 327680 || mesh.index_count != 786432 {
                    return Err("full density terrain lost".into());
                }
                triangles += u64::from(mesh.index_count / 3);
            } else if mesh.index_count >= 786432 {
                return Err("terrain coarse LOD did not reduce geometry".into());
            }
        }
        let collision: CookedCollision3D =
            serde_json::from_slice(payload(&format!("bm13-ground-{i}-collision"))?)
                .map_err(|e| e.to_string())?;
        collision.validate().map_err(|e| e.to_string())?;
        let CollisionShape3D::StaticMesh {
            vertices,
            triangles,
            ..
        } = collision.shape
        else {
            return Err("terrain needs real collision mesh".into());
        };
        if vertices.len() != 327680 || triangles.len() != 262144 {
            return Err("terrain collision reduced".into());
        }
        let navigation: CookedNavigation3D =
            serde_json::from_slice(payload(&format!("bm13-ground-{i}-nav"))?)
                .map_err(|e| e.to_string())?;
        navigation.validate().map_err(|e| e.to_string())?;
        if navigation.tiles.len() != 1 {
            return Err("missing real native nav tile".into());
        }
        engine_navigation_native::validate_tile(&navigation.tiles[0].data)
            .map_err(|e| format!("native tile {e:?}"))?;
    }
    let mut vegetation = 0;
    let mut vegetation_lods = 0;
    let mut bound_assets = BTreeSet::new();
    let mut source_ids = BTreeSet::new();
    for entity in entities.iter().filter(|e| e.enabled && e.spatial.is_some()) {
        let Some(component) = entity
            .components
            .iter()
            .find(|c| c.component_type == "engine.vegetation3d")
        else {
            continue;
        };
        let binding: VegetationBinding3D =
            serde_json::from_value(component.data.clone()).map_err(|e| e.to_string())?;
        let id = binding.cluster_asset.asset_id.as_str();
        if !binding.promoted_instance_ids.is_empty()
            || !bound_assets.insert(id.to_string())
            || assets
                .get(id)
                .is_none_or(|a| a.asset_type != "vegetation3d")
        {
            return Err("missing, duplicated or promoted vegetation binding".into());
        }
        let asset: VegetationAsset3D =
            serde_json::from_slice(payload(id)?).map_err(|e| e.to_string())?;
        asset
            .validate_binding(&binding)
            .map_err(|e| e.to_string())?;
        for instance in asset.clusters.iter().flat_map(|c| &c.instances) {
            if !source_ids.insert(instance.source_id.clone()) {
                return Err("duplicated vegetation source across partitions".into());
            }
        }
        for cluster in &asset.clusters {
            if cluster.lods.len() != 1
                || cluster.lods[0].distance != 48.
                || cluster.lods[0].mesh.reference.id != "bm13-tree-coarse"
            {
                return Err(
                    "original vegetation needs a real coarse representation at 48 metres".into(),
                );
            }
            let mut counts = Vec::new();
            for (_, mesh) in cluster.mesh_levels() {
                let cooked: RenderContract<CookedRenderAsset> =
                    serde_json::from_slice(payload(&mesh.reference.id)?)
                        .map_err(|e| e.to_string())?;
                cooked.data.validate().map_err(|e| e.to_string())?;
                let RenderAssetData::Mesh(data) = cooked.data.asset else {
                    return Err("vegetation LOD is not a mesh".into());
                };
                counts.push(data.index_count);
            }
            if counts[1] >= counts[0] {
                return Err("vegetation coarse LOD did not reduce actual geometry".into());
            }
            vegetation_lods += cluster.instances.len();
        }
        vegetation += asset
            .clusters
            .iter()
            .map(|c| c.instances.len())
            .sum::<usize>();
    }
    if bound_assets.len()
        != input
            .assets
            .iter()
            .filter(|a| a.asset_type == "vegetation3d")
            .count()
    {
        return Err("unbound vegetation payload is not a consumer".into());
    }
    if vegetation != 50000 {
        return Err("original vegetation population required".into());
    }
    let effect: CookedParticleEffect =
        serde_json::from_slice(payload("bm13-particles")?).map_err(|e| e.to_string())?;
    effect.description.validate().map_err(|e| e.to_string())?;
    if effect.description.budget.max_particles != 10000
        || effect
            .description
            .emitters
            .iter()
            .map(|e| e.capacity)
            .sum::<u32>()
            != 10000
        || effect
            .description
            .emitters
            .iter()
            .flat_map(|e| &e.bursts)
            .map(|b| b.count)
            .sum::<u32>()
            != 10000
    {
        return Err("real particle capacity/burst differs".into());
    }
    let audio = engine_runtime::audio::decode_audio_wav(payload("bm13-tone")?)
        .map_err(|e| e.to_string())?;
    if !audio.samples.iter().any(|s| s.abs() > 0.01) {
        return Err("silent PCM is not audio-load evidence".into());
    }
    Ok(
        json!({"scope":"original-BM13-cooked-payloads-only","terrainTiles":9,"samplesPerTileSide":257,"terrainFineTriangles":triangles,"terrainVisualLods":18,"fullDensityCollisions":9,"realNativeNavigationTiles":9,"vegetationInstances":vegetation,"vegetationSourcesWithRealCoarseLod":vegetation_lods,"spatialAudioSourceBindings":sources,"nonSilentPcmSamples":audio.samples.len(),"particleCapacityAndStartupBurst":10000,"notProved":["10000 visible vegetation","runtime vegetation LOD","active particles","actual spatial mixing","independent residency","K2 trajectory","window/performance/platform","whole BM13 or S5"]}),
    )
}

pub fn inspect_bm13_input(input: &RuntimePackageBuildInput) -> BenchmarkResult<Value> {
    inspect_bm13_raw(input)
        .map_err(|message| BenchmarkError::new("benchmark.world_payload_ineligible", message))
}
pub(super) fn inspect_world_cook(
    prepared: &PreparedRuntimePackage,
    workload: &str,
) -> BenchmarkResult<Value> {
    let invalid =
        |message: String| BenchmarkError::new("benchmark.world_payload_ineligible", message);
    let descriptor: Value = serde_json::from_slice(
        prepared
            .source()
            .bytes("Benchmark/world-manifest.json")
            .ok_or_else(|| invalid("missing world manifest".into()))?,
    )
    .map_err(|e| invalid(e.to_string()))?;
    let catalog = super::target_catalog()?;
    let target = catalog["workloads"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["id"] == workload)
        .ok_or_else(|| invalid("unknown workload".into()))?;
    if descriptor["workload"] != workload
        || descriptor["stageRecipe"] != "world-v1"
        || descriptor["seed"] != target["seed"]
        || descriptor["track"] != target["trajectory"]
        || descriptor["target"] != target["pc"]
    {
        return Err(invalid(
            "world manifest must retain the original target, seed and trajectory".into(),
        ));
    }
    match workload {
        "BM-08" => {
            let track: Value = serde_json::from_slice(
                prepared
                    .source()
                    .bytes("Benchmark/k2-input.json")
                    .ok_or_else(|| invalid("missing frozen K2 input".into()))?,
            )
            .map_err(|e| invalid(e.to_string()))?;
            if track != catalog["tracks"]["K2-16"] {
                return Err(invalid("canonical K2 input changed".into()));
            }
            super::cells::inspect_world_cells(prepared)
        }
        "BM-13" => inspect_bm13_input(prepared.runtime_package_build_input()),
        "BM-10" => super::inspect_bm10_input(prepared.runtime_package_build_input()),
        _ => Err(invalid(
            "world consumer is not implemented for this workload".into(),
        )),
    }
}
pub(super) fn world_scenario(
    prepared: &PreparedRuntimePackage,
    workload: &str,
    frames: u64,
    timeout: u64,
) -> BenchmarkResult<runtime_player_winit::semantic_outcome::PlaytestScenario> {
    let invalid =
        |message: String| BenchmarkError::new("benchmark.world_scenario_ineligible", message);
    prepared
        .load_playtest_scenario()
        .map_err(|e| invalid(e.to_string()))?;
    let manifest = prepared
        .source()
        .project_manifest_value()
        .map_err(|e| invalid(e.to_string()))?;
    let path = manifest["playtestScenario"]
        .as_str()
        .ok_or_else(|| invalid("world scenario missing".into()))?;
    let scenario: runtime_player_winit::semantic_outcome::PlaytestScenario =
        serde_json::from_slice(
            prepared
                .source()
                .bytes(path)
                .ok_or_else(|| invalid("leased world scenario missing".into()))?,
        )
        .map_err(|e| invalid(e.to_string()))?;
    if workload == "BM-08" {
        super::cells::validate_world_scenario(&scenario, frames, timeout)?;
        return Ok(scenario);
    }
    if workload == "BM-10" {
        if frames != 360
            || scenario.max_simulation_ticks != 360
            || scenario.max_presentation_frames != frames
            || scenario.timeout_ms > timeout
            || scenario.target
                != runtime_player_winit::semantic_outcome::PlaytestTarget::WindowsWindowed
        {
            return Err(invalid(
                "BM10 requires the complete bounded combined 360-tick Windows functional scenario"
                    .into(),
            ));
        }
        super::combined::validate_scenario(&scenario)?;
        return Ok(scenario);
    }
    if workload != "BM-13"
        || frames != 120
        || scenario.max_simulation_ticks != 120
        || scenario.max_presentation_frames != frames
        || scenario.timeout_ms > timeout
        || scenario.target
            != runtime_player_winit::semantic_outcome::PlaytestTarget::WindowsWindowed
        || !scenario.assertions.iter().any(|a| {
            a.from_simulation_tick == 120
                && a.through_simulation_tick == 120
                && a.path == "bm13.tick"
                && serde_json::to_value(&a.equals).ok() == Some(json!(120))
        })
        || !scenario
            .captures
            .iter()
            .any(|c| c.presentation_frame == 120 && c.required && !c.subjective_review)
    {
        return Err(invalid("BM13 original payload smoke requires its complete 120-tick assertion and capture; this does not qualify K2/LOD/residency/performance".into()));
    }
    Ok(scenario)
}
