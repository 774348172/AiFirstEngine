//! BM09 admission verifies real cross-domain Cook; precision is measured in Player.
use super::{invalid, BenchmarkResult};
use engine_runtime::{
    character_animation::{CharacterAsset, CharacterBinding3D},
    navigation_3d::{CookedNavigation3D, NavigationSurface3D},
    physics_3d::Body3D,
    runtime_package_builder::RuntimePackageBuildInput,
    spatial_contract::RuntimeSpatial,
};
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub fn inspect_bm09_input(input: &RuntimePackageBuildInput) -> BenchmarkResult<Value> {
    if input.scenes.len() != 1 || input.scenes[0].main_camera.as_deref() != Some("camera") {
        return Err(invalid("BM09 requires its actual scene/camera"));
    }
    let scene = &input.scenes[0];
    let mut ids = BTreeSet::new();
    let (mut objects, mut characters, mut bodies, mut agents, mut surfaces) = (0, 0, 0, 0, 0);
    for entity in &scene.entities {
        if !entity.enabled || !ids.insert(entity.id.as_str()) || entity.spatial.is_none() {
            return Err(invalid("BM09 requires distinct live spatial entities"));
        }
        if entity.id.starts_with("node-") {
            if !matches!(&entity.spatial,Some(RuntimeSpatial::World3D {mesh_renderer:Some(mesh),..}) if mesh.visible)
            {
                return Err(invalid("BM09 spatial probe lacks its real render consumer"));
            }
            objects += 1;
        }
        for component in &entity.components {
            match component.component_type.as_str() {
                "engine.character3d" => {
                    let binding: CharacterBinding3D =
                        serde_json::from_value(component.data.clone())
                            .map_err(|e| invalid(e.to_string()))?;
                    binding.validate().map_err(|e| invalid(e.to_string()))?;
                    let asset = input
                        .assets
                        .iter()
                        .find(|a| {
                            a.asset_id == binding.character.0.id && a.asset_type == "character3d"
                        })
                        .ok_or_else(|| invalid("BM09 character lacks its actual cooked rig"))?;
                    let cooked: CharacterAsset = serde_json::from_slice(
                        asset
                            .runtime_payload
                            .as_deref()
                            .ok_or_else(|| invalid("missing character payload"))?,
                    )
                    .map_err(|e| invalid(e.to_string()))?;
                    cooked.validate().map_err(|e| invalid(e.to_string()))?;
                    if binding.joints.len() != 3
                        || cooked.joints.len() != 3
                        || !entity
                            .components
                            .iter()
                            .any(|c| c.component_type == "engine.motor3d")
                    {
                        return Err(invalid(
                            "BM09 character must retain three joints and a real motor",
                        ));
                    }
                    for joint in &binding.joints {
                        if !scene.entities.iter().any(|e| e.id == joint.entity_ref) {
                            return Err(invalid("missing actual character joint"));
                        }
                    }
                    characters += 1;
                }
                "engine.body3d" => {
                    let body: Body3D = serde_json::from_value(component.data.clone())
                        .map_err(|e| invalid(e.to_string()))?;
                    body.validate().map_err(|e| invalid(e.to_string()))?;
                    if body.enabled {
                        bodies += 1;
                    }
                }
                "engine.navigation_agent3d" => {
                    if !entity
                        .components
                        .iter()
                        .any(|c| c.component_type == "engine.motor3d")
                    {
                        return Err(invalid("BM09 path agent lacks its actual motor"));
                    }
                    agents += 1;
                }
                "engine.navigation3d" => {
                    let binding: NavigationSurface3D =
                        serde_json::from_value(component.data.clone())
                            .map_err(|e| invalid(e.to_string()))?;
                    binding.validate().map_err(|e| invalid(e.to_string()))?;
                    let asset = input
                        .assets
                        .iter()
                        .find(|a| {
                            a.asset_id == binding.navigation.0.id && a.asset_type == "navigation3d"
                        })
                        .ok_or_else(|| invalid("missing real navigation asset"))?;
                    let cooked: CookedNavigation3D = serde_json::from_slice(
                        asset
                            .runtime_payload
                            .as_deref()
                            .ok_or_else(|| invalid("missing native tiles"))?,
                    )
                    .map_err(|e| invalid(e.to_string()))?;
                    cooked.validate().map_err(|e| invalid(e.to_string()))?;
                    if cooked.tiles.len() != 2 {
                        return Err(invalid("BM09 requires two actual adjoining native tiles"));
                    }
                    for tile in &cooked.tiles {
                        engine_navigation_native::validate_tile(&tile.data)
                            .map_err(|e| invalid(format!("invalid native tile {e:?}")))?;
                    }
                    surfaces += 1;
                }
                _ => {}
            }
        }
    }
    if (objects, characters, bodies, agents, surfaces) != (1000, 10, 100, 4, 1) {
        return Err(invalid(format!("BM09 actual domains differ: objects={objects}, characters={characters}, bodies={bodies}, agents={agents}, surfaces={surfaces}")));
    }
    Ok(
        json!({"scope":"BM09-sim-v1-actual-cross-domain-Cook","objects":objects,"characters":characters,"colliders":bodies,"pathAgents":agents,"nativeTiles":2,"qualification":"cook-only; actual queries, paths, rebase precision and zero-anchor comparison require Player"}),
    )
}

pub(super) fn validate_scenario(
    s: &runtime_player_winit::semantic_outcome::PlaytestScenario,
) -> BenchmarkResult<()> {
    let has = |tick, path: &str, value: Value| {
        s.assertions.iter().any(|a| {
            a.from_simulation_tick == tick
                && a.through_simulation_tick == tick
                && a.path == path
                && serde_json::to_value(&a.equals).ok() == Some(value.clone())
        })
    };
    for tick in [29, 59, 329, 629, 660] {
        for (path, value) in [
            ("tick", json!(tick)),
            ("spatialRead", json!(1000)),
            ("charactersRead", json!(10)),
            ("jointsRead", json!(30)),
            ("bodiesRead", json!(100)),
            ("pathsRead", json!(4)),
            ("queriesRead", json!(198)),
            ("domainsValid", json!(true)),
        ] {
            if !has(tick, &format!("bm09.{path}"), value) {
                return Err(invalid(
                    "BM09 requires every actual cross-domain checkpoint",
                ));
            }
        }
    }
    if !has(660, "bm09.rebases", json!(20)) {
        return Err(invalid("BM09 must finish twenty actual rebases"));
    }
    for tick in [29, 329, 629] {
        if !s
            .captures
            .iter()
            .any(|c| c.presentation_frame == tick && c.required && !c.subjective_review)
        {
            return Err(invalid("BM09 requires before/middle/after captures"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn precision_cook_rejects_missing_real_domain_consumers() {
        use crate::{GameProjectCompiler, ProjectAuthoringSession, TargetProfile};
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../samples/platform_3d_bm09_project");
        let mut session = ProjectAuthoringSession::open(root).unwrap();
        let paths = session
            .source_inventory()
            .unwrap()
            .entries
            .into_iter()
            .map(|e| e.relative_path)
            .collect();
        let lease = session
            .acquire_snapshot_lease("bm09-domain-admission", paths)
            .unwrap();
        let prepared = GameProjectCompiler::bind(&lease)
            .unwrap()
            .prepare(&lease, TargetProfile::WindowsDev)
            .unwrap();
        let baseline = prepared.runtime_package_build_input();
        inspect_bm09_input(baseline).unwrap();
        for missing in [
            "node-0000",
            "actor-00",
            "collider-001",
            "agent-0",
            "navigation",
            "actor-00-hand",
        ] {
            let mut input = baseline.clone();
            input.scenes[0].entities.retain(|e| e.id != missing);
            assert!(
                inspect_bm09_input(&input).is_err(),
                "missing actual consumer {missing}"
            );
        }
        for kind in ["character3d", "navigation3d"] {
            let mut input = baseline.clone();
            input.assets.retain(|a| a.asset_type != kind);
            assert!(
                inspect_bm09_input(&input).is_err(),
                "missing actual cooked asset {kind}"
            );
        }
    }
    #[test]
    fn precision_scenario_rejects_each_missing_domain_and_capture() {
        let scenario: runtime_player_winit::semantic_outcome::PlaytestScenario =
            serde_json::from_slice(include_bytes!(
                "../../../../../samples/platform_3d_bm09_project/Tests/bm09.scenario.json"
            ))
            .unwrap();
        let contract:engine_runtime::project_observation::ProjectObservationContract=serde_json::from_slice(include_bytes!("../../../../../samples/platform_3d_bm09_project/Observations/project.observations.json")).unwrap();
        scenario.validate(Some(&contract)).unwrap();
        validate_scenario(&scenario).unwrap();
        for i in 0..scenario.assertions.len() {
            let mut incomplete = scenario.clone();
            incomplete.assertions.remove(i);
            assert!(
                validate_scenario(&incomplete).is_err(),
                "missing assertion {i}"
            );
        }
        for i in 0..scenario.captures.len() {
            let mut incomplete = scenario.clone();
            incomplete.captures.remove(i);
            assert!(
                validate_scenario(&incomplete).is_err(),
                "missing capture {i}"
            );
        }
    }
}
