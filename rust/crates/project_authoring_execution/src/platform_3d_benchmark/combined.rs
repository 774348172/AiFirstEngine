//! BM10 inspects one actual combined Cook, retaining original domain predicates.
use super::{BenchmarkError, BenchmarkResult};
use engine_runtime::{
    render_contract::{CookedRenderAsset, RenderAssetData, RenderContract, ValidateRenderContract},
    runtime_package_builder::RuntimePackageBuildInput,
    spatial_contract::RuntimeSpatial,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
fn invalid(message: impl Into<String>) -> BenchmarkError {
    BenchmarkError::new("benchmark.combined_payload_ineligible", message)
}
pub fn inspect_bm10_input(input: &RuntimePackageBuildInput) -> BenchmarkResult<Value> {
    if input.scenes.len() != 1 || input.scenes[0].main_camera.as_deref() != Some("camera") {
        return Err(invalid("combined scene/camera missing"));
    }
    let scene = &input.scenes[0];
    let mut ids = BTreeSet::new();
    if scene
        .entities
        .iter()
        .any(|e| !e.enabled || e.spatial.is_none() || !ids.insert(&e.id))
    {
        return Err(invalid(
            "combined consumers must be unique, enabled spatial entities",
        ));
    }
    // Each view is a subset of this exact combined output, with original payloads
    // and bindings. The strict original checks cannot be replaced by manifest counts.
    let mut view = input.clone();
    view.scenes[0].entities.retain(|e| {
        e.id.starts_with("actor-")
            || e.id.starts_with("bm05-static-")
            || matches!(e.id.as_str(), "bm05-floor" | "camera")
    });
    let characters = super::inspect_bm05_input(&view)?;
    view.scenes[0].entities = scene
        .entities
        .iter()
        .filter(|e| {
            e.id.starts_with("dynamic-")
                || e.id.starts_with("static-")
                || e.id.starts_with("motor-")
                || matches!(
                    e.id.as_str(),
                    "moving-platform" | "world-controller" | "camera"
                )
        })
        .cloned()
        .collect();
    let physics = super::inspect_bm06_input(&view)?;
    view.scenes[0].entities = scene
        .entities
        .iter()
        .filter(|e| {
            e.id.starts_with("agent-")
                || e.id.starts_with("obstacle-")
                || matches!(
                    e.id.as_str(),
                    "nav-main"
                        | "floor"
                        | "floor-visual"
                        | "door-north"
                        | "door-north-visual"
                        | "door-south"
                        | "door-south-visual"
                        | "island-gap"
                        | "island-gap-visual"
                        | "camera"
                )
        })
        .cloned()
        .collect();
    let navigation = super::inspect_bm07_input(&view)?;
    drop(view);
    let assets = input
        .assets
        .iter()
        .map(|a| (a.asset_id.as_str(), a))
        .collect::<BTreeMap<_, _>>();
    if assets.len() != input.assets.len() {
        return Err(invalid("duplicate combined asset identity"));
    }
    let payload = |id: &str| {
        assets
            .get(id)
            .and_then(|a| a.runtime_payload.as_deref())
            .ok_or_else(|| invalid(format!("missing actual combined payload {id}")))
    };
    let mut triangles = BTreeMap::new();
    for asset in input.assets.iter().filter(|a| a.asset_type == "mesh") {
        let cooked: RenderContract<CookedRenderAsset> =
            serde_json::from_slice(payload(&asset.asset_id)?)
                .map_err(|e| invalid(e.to_string()))?;
        cooked.data.validate().map_err(|e| invalid(e.to_string()))?;
        let RenderAssetData::Mesh(mesh) = cooked.data.asset else {
            return Err(invalid("non-mesh combined geometry"));
        };
        triangles.insert(asset.asset_id.as_str(), u64::from(mesh.index_count / 3));
    }
    let mut instances = 0;
    let mut all_triangles = 0u64;
    let mut lights = [0usize; 3];
    let mut shadow = [0usize; 3];
    let mut audio = 0;
    let mut particles = 0;
    for entity in &scene.entities {
        if let Some(RuntimeSpatial::World3D {
            mesh_renderer: Some(mesh),
            ..
        }) = &entity.spatial
        {
            if !mesh.visible {
                return Err(invalid(
                    "hidden declaration is not a live drawable instance",
                ));
            }
            instances += 1;
            all_triangles += triangles
                .get(mesh.mesh.0.id.as_str())
                .ok_or_else(|| invalid("unresolved combined mesh"))?;
        }
        if let Some(RuntimeSpatial::World3D {
            light: Some(light), ..
        }) = &entity.spatial
        {
            let value = serde_json::to_value(light).map_err(|e| invalid(e.to_string()))?;
            if value["enabled"] != true {
                return Err(invalid("disabled combined light"));
            }
            let index = match value["kind"]["type"].as_str() {
                Some("directional") => 0,
                Some("point") => 1,
                Some("spot") => 2,
                _ => return Err(invalid("unknown combined light kind")),
            };
            lights[index] += 1;
            shadow[index] += usize::from(value["castShadows"] == true);
        }
        let has = |kind: &str| entity.components.iter().any(|c| c.component_type == kind);
        if has("engine.audio_source") && has("engine.spatial_audio3d") {
            audio += 1;
        }
        for component in &entity.components {
            if component.component_type == "engine.particle_effect" {
                let effect =
                    engine_runtime::runtime_particles::decode_particle_effect(&component.data)
                        .map_err(invalid)?;
                if !effect.play_on_awake
                    || effect.paused
                    || effect.effect_ref.id != "bm13-particles"
                {
                    return Err(invalid(
                        "combined effect must actually start the original particle asset",
                    ));
                }
                particles += 1;
            }
        }
    }
    if instances != 50000
        || all_triangles > 10_000_000
        || lights != [1, 32, 8]
        || shadow != [1, 0, 4]
        || audio != 64
        || particles != 1
    {
        return Err(invalid(format!("combined counts differ: render={instances} triangles={all_triangles} lights={lights:?} shadows={shadow:?} spatialAudio={audio} effectBindings={particles}")));
    }
    let hud = input
        .aui_documents
        .iter()
        .flat_map(|a| a.document["nodes"].as_array().into_iter().flatten())
        .filter(|n| n["visible"] == true)
        .count();
    if hud != 100 {
        return Err(invalid(format!("actual visible HUD source nodes {hud}")));
    }
    let effect: engine_runtime::particle_effect::CookedParticleEffect =
        serde_json::from_slice(payload("bm13-particles")?).map_err(|e| invalid(e.to_string()))?;
    effect
        .description
        .validate()
        .map_err(|e| invalid(e.to_string()))?;
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
        return Err(invalid(
            "actual 10000-capacity startup particle burst missing",
        ));
    }
    let pcm = engine_runtime::audio::decode_audio_wav(payload("bm13-tone")?).map_err(invalid)?;
    if !pcm.samples.iter().any(|v| v.abs() > 0.01) {
        return Err(invalid("combined audio payload is silent"));
    }
    Ok(
        json!({"scope":"BM10-original-combined-cooked-consumers-only","renderInstances":instances,"sceneEntities":scene.entities.len(),"allRendererTriangles":all_triangles,"staticTriangleCeilingConservativelyMet":true,"characters":characters,"physics":physics,"navigation":navigation,"lights":lights,"castingLights":shadow,"hudSourceNodes":hud,"spatialAudioSourceBindings":audio,"particleCapacityAndStartupBurst":10000,"notProved":["10000 actual main-view instances","simultaneous actual Player update/deformation/queries/navigation","live particle count and spatial mixing","K3/K2","formal performance/memory/platform","whole BM10 or S5"]}),
    )
}

/// A bounded functional combined scenario does not grant a complete K3/K2 trajectory.
pub(super) fn validate_scenario(
    scenario: &runtime_player_winit::semantic_outcome::PlaytestScenario,
) -> BenchmarkResult<()> {
    let has = |tick, path: &str, value: Value| {
        scenario.assertions.iter().any(|a| {
            a.from_simulation_tick == tick
                && a.through_simulation_tick == tick
                && a.path == path
                && serde_json::to_value(&a.equals).ok() == Some(value.clone())
        })
    };
    for (tick, path, value) in [
        (120, "bm06.actualDynamicBodies", json!(1000)),
        (120, "bm06.raycastsPerTick", json!(200)),
        (120, "bm06.sweepsPerTick", json!(50)),
        (160, "bm05.actualActors", json!(100)),
        (160, "bm05.actualJoints", json!(6400)),
        (160, "bm05.actorsWithEvents", json!(100)),
        (300, "bm07.actualAgents", json!(1000)),
        (300, "bm07.actualMotorStates", json!(1000)),
        (300, "bm07.requestMixExact", json!(true)),
        (300, "bm07.noUnexpectedResults", json!(true)),
        (360, "bm10.tick", json!(360)),
        (360, "bm10.domainsUpdated", json!(true)),
        (360, "bm10.audioCommandsIssued", json!(64)),
    ] {
        if !has(tick, path, value) {
            return Err(invalid(format!(
                "combined scenario missing actual domain checkpoint {path}"
            )));
        }
    }
    for frame in [120, 160, 360] {
        if !scenario
            .captures
            .iter()
            .any(|c| c.presentation_frame == frame && c.required && !c.subjective_review)
        {
            return Err(invalid(
                "combined scenario missing required objective capture",
            ));
        }
    }
    Ok(())
}
#[cfg(test)]
#[test]
fn bm10_scenario_requires_every_real_domain_checkpoint_and_capture() {
    let source = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../samples/platform_3d_bm10_project/Tests/bm10.scenario.json"
    ));
    let scenario: runtime_player_winit::semantic_outcome::PlaytestScenario =
        serde_json::from_slice(source).unwrap();
    validate_scenario(&scenario).unwrap();
    for index in 0..scenario.assertions.len() {
        let mut bad = scenario.clone();
        bad.assertions.remove(index);
        assert!(validate_scenario(&bad).is_err());
    }
    for index in 0..scenario.captures.len() {
        let mut bad = scenario.clone();
        bad.captures[index].required = false;
        assert!(validate_scenario(&bad).is_err());
    }
}
