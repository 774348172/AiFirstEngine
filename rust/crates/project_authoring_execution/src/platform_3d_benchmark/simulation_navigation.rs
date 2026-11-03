//! Inspect BM07's actual Cook and its independently executable scenario.
use super::{invalid, BenchmarkResult};
use engine_runtime::{
    navigation_3d::*, physics_3d::CharacterMotor3D,
    runtime_package_builder::RuntimePackageBuildInput,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
pub fn inspect_bm07_input(input: &RuntimePackageBuildInput) -> BenchmarkResult<Value> {
    if input.scenes.len() != 1 || input.scenes[0].main_camera.as_deref() != Some("camera") {
        return Err(invalid("BM07 requires its actual selected scene/camera"));
    }
    let mut assets = BTreeMap::new();
    for asset in input
        .assets
        .iter()
        .filter(|a| a.asset_type == "navigation3d")
    {
        let cooked: CookedNavigation3D = serde_json::from_slice(
            asset
                .runtime_payload
                .as_deref()
                .ok_or_else(|| invalid("missing real cooked navigation payload"))?,
        )
        .map_err(|e| invalid(e.to_string()))?;
        cooked.validate().map_err(|e| invalid(e.to_string()))?;
        if cooked.asset_id != asset.asset_id {
            return Err(invalid("navigation payload identity differs"));
        }
        assets.insert(asset.asset_id.as_str(), cooked);
    }
    let mut tiles = BTreeSet::new();
    let mut profile = None;
    let mut agents = 0;
    let mut obstacles = 0;
    let mut bindings = BTreeSet::new();
    let mut entities = input.scenes[0].entities.clone();
    for prefab in &input.prefabs {
        if prefab.prefab_id != "bm07-removable" {
            // Imported visual models also publish ordinary, unused prefabs.
            // Only the actual removable navigation owner contributes tiles.
            continue;
        }
        let engine_runtime::spatial_contract::RuntimePrefabDocument::Spatial(document) =
            engine_runtime::spatial_contract::decode_runtime_prefab(prefab.document.clone())
                .map_err(|e| invalid(e.to_string()))?
        else {
            return Err(invalid("BM07 tile owner requires a spatial prefab"));
        };
        for entity in document.entities {
            let engine_runtime::spatial_contract::RuntimeEntityDocument::Spatial(entity) = entity
            else {
                return Err(invalid("BM07 tile prefab must remain spatial"));
            };
            if entity.components.iter().any(|c| {
                matches!(
                    c.component_type.as_str(),
                    "engine.navigation_agent3d" | "engine.motor3d" | "engine.navigation_obstacle3d"
                )
            }) {
                return Err(invalid(
                    "original agents and obstacles must remain in the active scene",
                ));
            }
            entities.push(entity.into());
        }
    }
    for entity in &entities {
        if !entity.enabled {
            return Err(invalid("BM07 contains a disabled consumer"));
        }
        let component = |kind: &str| {
            entity
                .components
                .iter()
                .find(|c| c.component_type == kind)
                .map(|c| &c.data)
        };
        if let Some(value) = component("engine.navigation3d") {
            let binding: NavigationSurface3D =
                serde_json::from_value(value.clone()).map_err(|e| invalid(e.to_string()))?;
            binding.validate().map_err(|e| invalid(e.to_string()))?;
            if !bindings.insert(binding.navigation.0.id.clone()) {
                return Err(invalid("duplicate navigation asset binding"));
            }
            let asset = assets
                .get(binding.navigation.0.id.as_str())
                .ok_or_else(|| invalid("navigation binding has no actual cooked asset"))?;
            if asset.profile.tile_width != 32.
                || profile.as_ref().is_some_and(|p| p != &asset.profile)
            {
                return Err(invalid("BM07 requires one exact 32m profile"));
            }
            profile = Some(asset.profile.clone());
            for tile in &asset.tiles {
                engine_navigation_native::validate_tile(&tile.data)
                    .map_err(|e| invalid(format!("invalid actual native tile {e:?}")))?;
                if !tiles.insert((tile.x, tile.z)) {
                    return Err(invalid("duplicate real nav tile"));
                }
            }
        }
        if let Some(value) = component("engine.navigation_agent3d") {
            let agent: NavigationAgent3D =
                serde_json::from_value(value.clone()).map_err(|e| invalid(e.to_string()))?;
            agent.validate().map_err(|e| invalid(e.to_string()))?;
            let motor: CharacterMotor3D = serde_json::from_value(
                component("engine.motor3d")
                    .ok_or_else(|| invalid("agent lacks real motor"))?
                    .clone(),
            )
            .map_err(|e| invalid(e.to_string()))?;
            motor.validate().map_err(|e| invalid(e.to_string()))?;
            if !motor.enabled || entity.spatial.is_none() || agent.profile != "human" {
                return Err(invalid("agent motor/spatial/profile not eligible"));
            }
            agents += 1;
        }
        if let Some(value) = component("engine.navigation_obstacle3d") {
            let obstacle: NavigationObstacle3D =
                serde_json::from_value(value.clone()).map_err(|e| invalid(e.to_string()))?;
            obstacle.validate().map_err(|e| invalid(e.to_string()))?;
            if entity.spatial.is_none() {
                return Err(invalid("obstacle lacks actual spatial placement"));
            }
            obstacles += 1;
        }
    }
    if agents != 1000
        || obstacles != 50
        || tiles.len() != 64
        || bindings.len() != assets.len()
        || profile.is_none_or(|p| p.key != "human")
    {
        return Err(invalid(format!(
            "BM07 actual Cook differs: agents={agents}, obstacles={obstacles}, tiles={}",
            tiles.len()
        )));
    }
    Ok(
        json!({"scope":"BM-07/sim-v1-full-PC-cooked-navigation-bindings","actualAgents":agents,"actualMotorBindings":agents,"actualObstacles":obstacles,"actualTiles":tiles.len(),"tileMetres":32,"qualification":"cooked-input-only","notExecuted":["200 requests per second and real 70/20/10 outcomes","opposing crowd/door/motor displacement","dynamic obstacle and tile unload","window/formal/performance/platform"]}),
    )
}
pub(super) fn validate_scenario(
    scenario: &runtime_player_winit::semantic_outcome::PlaytestScenario,
) -> BenchmarkResult<()> {
    let has = |tick, path: &str, value: Value| {
        scenario.assertions.iter().any(|a| {
            a.from_simulation_tick == tick
                && a.through_simulation_tick == tick
                && a.path == format!("bm07.{path}")
                && serde_json::to_value(&a.equals).ok() == Some(value.clone())
        })
    };
    for tick in [300, 600, 900, 1200] {
        for (path, value) in [
            ("actualAgents", json!(1000)),
            ("actualMotorStates", json!(1000)),
            ("requestMixExact", json!(true)),
            ("noUnexpectedResults", json!(true)),
        ] {
            if !has(tick, path, value) {
                return Err(invalid(
                    "BM07 lacks actual population/rate/outcome checkpoint",
                ));
            }
        }
    }
    for (path, value) in [
        ("issuedReachable", json!(2800)),
        ("issuedUnreachable", json!(800)),
        ("issuedNotResident", json!(400)),
        ("requestOutcomesMet", json!(true)),
        ("opposingCrossingsObserved", json!(true)),
        ("movedObstacles", json!(50)),
        ("unloadedTileGone", json!(true)),
    ] {
        if !has(1200, path, value) {
            return Err(invalid("BM07 lacks actual itinerary result"));
        }
    }
    for tick in [300, 600, 1200] {
        if !scenario
            .captures
            .iter()
            .any(|c| c.presentation_frame == tick && c.required && !c.subjective_review)
        {
            return Err(invalid(
                "BM07 requires its three original functional captures",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bm07_scenario_rejects_missing_population_mix_outcome_itinerary_and_captures() {
        let scenario: runtime_player_winit::semantic_outcome::PlaytestScenario =
            serde_json::from_slice(include_bytes!(
                "../../../../../samples/platform_3d_bm07_project/Tests/bm07.scenario.json"
            ))
            .unwrap();
        let contract:engine_runtime::project_observation::ProjectObservationContract=serde_json::from_slice(include_bytes!("../../../../../samples/platform_3d_bm07_project/Observations/project.observations.json")).unwrap();
        scenario.validate(Some(&contract)).unwrap();
        validate_scenario(&scenario).unwrap();
        for missing in 0..scenario.assertions.len() {
            let mut bad = scenario.clone();
            bad.assertions.remove(missing);
            assert!(validate_scenario(&bad).is_err());
        }
        for missing in 0..scenario.captures.len() {
            let mut bad = scenario.clone();
            bad.captures.remove(missing);
            assert!(validate_scenario(&bad).is_err());
        }
    }
}
