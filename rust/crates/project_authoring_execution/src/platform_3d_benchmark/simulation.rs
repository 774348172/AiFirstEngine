//! Final simulation recipe admission counts real cooked payloads and live bindings.
use super::{BenchmarkError, BenchmarkResult};
use crate::PreparedRuntimePackage;
use engine_runtime::{
    character_animation::{CharacterAsset, CharacterBinding3D},
    render_contract::{CookedRenderAsset, RenderAssetData, RenderContract},
    runtime_package_builder::RuntimePackageBuildInput,
    spatial_contract::RuntimeSpatial,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
#[path = "simulation_navigation.rs"]
mod navigation;
pub use navigation::inspect_bm07_input;
#[path = "simulation_precision.rs"]
mod precision;
pub use precision::inspect_bm09_input;

fn invalid(message: impl Into<String>) -> BenchmarkError {
    BenchmarkError::new("benchmark.sim_payload_ineligible", message)
}
pub fn inspect_simulation_cook(
    prepared: &PreparedRuntimePackage,
    workload: &str,
) -> BenchmarkResult<Value> {
    let descriptor: Value = serde_json::from_slice(
        prepared
            .source()
            .bytes("Benchmark/sim-manifest.json")
            .ok_or_else(|| invalid("missing frozen simulation manifest"))?,
    )
    .map_err(|e| invalid(e.to_string()))?;
    let catalog = super::target_catalog()?;
    let target = catalog["workloads"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["id"] == workload)
        .ok_or_else(|| invalid("unknown simulation workload"))?;
    if descriptor["workload"] != workload
        || descriptor["stageRecipe"] != "sim-v1"
        || descriptor["seed"] != target["seed"]
        || descriptor["target"] != target["pc"]
        || descriptor["track"] != target["trajectory"]
    {
        return Err(invalid(
            "simulation descriptor must retain the original PC target, seed and trajectory",
        ));
    }
    match workload {
        "BM-05" => inspect_bm05_input(prepared.runtime_package_build_input()),
        "BM-06" => inspect_bm06_input(prepared.runtime_package_build_input()),
        "BM-07" => inspect_bm07_input(prepared.runtime_package_build_input()),
        "BM-09" => inspect_bm09_input(prepared.runtime_package_build_input()),
        _ => Err(invalid(
            "simulation consumer not implemented for this workload",
        )),
    }
}

/// Count real cooked body, joint and motor bindings. Runtime queries remain a Player claim.
pub fn inspect_bm06_input(input: &RuntimePackageBuildInput) -> BenchmarkResult<Value> {
    use engine_runtime::physics_3d::{
        Body3D, BodyMotion3D, CharacterMotor3D, CollisionShape3D, Joint3D,
    };
    if input.scenes.len() != 1 || input.scenes[0].main_camera.as_deref() != Some("camera") {
        return Err(invalid("BM06 requires its actual selected scene/camera"));
    }
    let scene = &input.scenes[0];
    let mut bodies = BTreeMap::new();
    let mut ids = BTreeSet::new();
    let mut counts = [[0usize; 3]; 2];
    let mut ccd = [0usize; 2];
    let mut joints = Vec::new();
    let mut motors = 0;
    let mut platforms = 0;
    for entity in &scene.entities {
        if !entity.enabled || !ids.insert(entity.id.as_str()) {
            return Err(invalid("BM06 disabled/duplicate entity"));
        }
        for component in &entity.components {
            match component.component_type.as_str() {
                "engine.body3d" => {
                    let body: Body3D = serde_json::from_value(component.data.clone())
                        .map_err(|e| invalid(e.to_string()))?;
                    body.validate().map_err(|e| invalid(e.to_string()))?;
                    if !body.enabled
                        || entity.spatial.is_none()
                        || bodies.insert(entity.id.as_str(), body.motion).is_some()
                    {
                        return Err(invalid("BM06 requires unique live spatial body bindings"));
                    }
                    if body.motion == BodyMotion3D::Kinematic
                        && entity.id == "moving-platform"
                        && matches!(body.shape, CollisionShape3D::Box { .. })
                    {
                        platforms += 1;
                        continue;
                    }
                    let population = match body.motion {
                        BodyMotion3D::Dynamic => 0,
                        BodyMotion3D::Static => 1,
                        _ => return Err(invalid("unexpected BM06 kinematic body")),
                    };
                    let shape = match body.shape {
                        CollisionShape3D::Box { .. } => 0,
                        CollisionShape3D::Sphere { .. } => 1,
                        CollisionShape3D::Capsule { .. } => 2,
                        _ => {
                            return Err(invalid(
                                "BM06 shape distribution must use actual primitive colliders",
                            ))
                        }
                    };
                    counts[population][shape] += 1;
                    ccd[population] += usize::from(body.ccd);
                }
                "engine.joint3d" => {
                    let joint: Joint3D = serde_json::from_value(component.data.clone())
                        .map_err(|e| invalid(e.to_string()))?;
                    joint.validate().map_err(|e| invalid(e.to_string()))?;
                    joints.push((entity.id.as_str(), joint.other.entity_ref));
                }
                "engine.motor3d" => {
                    let motor: CharacterMotor3D = serde_json::from_value(component.data.clone())
                        .map_err(|e| invalid(e.to_string()))?;
                    motor.validate().map_err(|e| invalid(e.to_string()))?;
                    if !motor.enabled || entity.spatial.is_none() {
                        return Err(invalid("BM06 disabled/nonspatial motor"));
                    }
                    motors += 1;
                }
                _ => {}
            }
        }
    }
    if counts != [[700, 200, 100], [1400, 400, 200]]
        || ccd != [100, 0]
        || motors != 2
        || joints.len() != 100
    {
        return Err(invalid(format!(
            "actual BM06 differs: shapes={counts:?}, ccd={ccd:?}, motors={motors}, joints={}",
            joints.len()
        )));
    }
    let mut owners = BTreeSet::new();
    for (owner, other) in &joints {
        if !owners.insert(*owner)
            || *owner == other
            || bodies.get(owner) != Some(&BodyMotion3D::Dynamic)
            || bodies.get(other.as_str()) != Some(&BodyMotion3D::Dynamic)
        {
            return Err(invalid("all 100 BM06 joints must actually bind distinct dynamic body owners to other live dynamic bodies"));
        }
    }
    Ok(
        json!({"scope":"BM-06/sim-v1-full-PC-cooked-body-bindings","dynamicBodies":1000,"staticColliders":2000,
        "actualShapes":counts,"actualCcd":ccd,"actualJoints":joints.len(),"actualMotors":motors,"auxiliaryKinematicPlatforms":platforms,
        "qualification":"cooked-input-only","notExecuted":["actual-Player-200-rays-50-sweeps-per-tick","full-motor-obstacle-cell-itinerary","formal-performance","Android-device"]}),
    )
}

/// Offline consumer seam; takes actual Compiler output, never manifest counts as proof.
pub fn inspect_bm05_input(input: &RuntimePackageBuildInput) -> BenchmarkResult<Value> {
    if input.scenes.len() != 1 || input.scenes[0].main_camera.as_deref() != Some("camera") {
        return Err(invalid(
            "BM05 needs one actual scene and its selected camera",
        ));
    }
    let scene = &input.scenes[0];
    let entities: BTreeMap<_, _> = scene.entities.iter().map(|e| (e.id.as_str(), e)).collect();
    if entities.len() != scene.entities.len() {
        return Err(invalid("duplicate scene identity"));
    }
    let mut meshes = BTreeMap::new();
    let mut rigs = BTreeMap::new();
    for asset in &input.assets {
        match asset.asset_type.as_str() {
            "mesh" => {
                let payload = asset
                    .runtime_payload
                    .as_ref()
                    .ok_or_else(|| invalid("missing cooked mesh"))?;
                let RenderAssetData::Mesh(mesh) = RenderContract::<CookedRenderAsset>::decode(
                    std::str::from_utf8(payload).map_err(|e| invalid(e.to_string()))?,
                )
                .map_err(|e| invalid(e.to_string()))?
                .data
                .asset
                else {
                    return Err(invalid("mesh points at non-mesh payload"));
                };
                let triangles =
                    mesh.index_bytes.len() as u64 / mesh.index_format.byte_len() as u64 / 3;
                meshes.insert(asset.asset_id.as_str(), (mesh.vertex_count, triangles));
            }
            "character3d" => {
                let character: CharacterAsset = serde_json::from_slice(
                    asset
                        .runtime_payload
                        .as_ref()
                        .ok_or_else(|| invalid("missing cooked rig"))?,
                )
                .map_err(|e| invalid(e.to_string()))?;
                character.validate().map_err(|e| invalid(e.to_string()))?;
                if character.joints.len() != 64
                    || character.clips.len() != 3
                    || character.skins.len() != 1
                {
                    return Err(invalid(
                        "each of ten rigs requires 64 joints, three clips, one actual skin",
                    ));
                }
                let skin = &character.skins[0];
                if skin.joints.len() != 64
                    || skin.influences.len() != 15000
                    || skin
                        .influences
                        .iter()
                        .any(|v| v.weights.iter().any(|w| *w <= 0.))
                {
                    return Err(invalid("15000 vertices must each use four positive influences and all 64 skin joints"));
                }
                if skin.joints.iter().copied().collect::<BTreeSet<_>>().len() != 64 {
                    return Err(invalid("duplicate skin joints"));
                }
                let referenced: BTreeSet<_> =
                    skin.influences.iter().flat_map(|v| v.joints).collect();
                if referenced.len() != 64 {
                    return Err(invalid(
                        "all 64 joints must actually influence mesh vertices",
                    ));
                }
                if character
                    .clips
                    .iter()
                    .any(|clip| clip.tracks.is_empty() || clip.events.is_empty())
                {
                    return Err(invalid(
                        "clips must contain actual tracks and gameplay events",
                    ));
                }
                rigs.insert(asset.asset_id.as_str(), character);
            }
            _ => {}
        }
    }
    if rigs.len() != 10
        || rigs
            .values()
            .map(|r| &r.source_digest)
            .collect::<BTreeSet<_>>()
            .len()
            != 10
    {
        return Err(invalid("exactly ten distinct cooked rig sources required"));
    }
    for rig in rigs.values() {
        if meshes.get(rig.skins[0].mesh_asset_id.as_str()) != Some(&(15000, 25000)) {
            return Err(invalid(
                "each rig mesh must actually decode to 15000 vertices/25000 triangles",
            ));
        }
    }
    let skin_mesh_ids: BTreeSet<_> = rigs
        .values()
        .map(|r| r.skins[0].mesh_asset_id.as_str())
        .collect();
    let mut characters = 0;
    let mut masked = 0;
    let mut static_instances = 0;
    let mut static_triangles = 0;
    let mut all_targets = BTreeSet::new();
    let mut used_rigs = BTreeMap::<&str, usize>::new();
    for e in &scene.entities {
        if !e.enabled {
            return Err(invalid("disabled entities do not qualify for BM05"));
        }
        let binding = e
            .components
            .iter()
            .find(|c| c.component_type == "engine.character3d");
        if let Some(c) = binding {
            let binding: CharacterBinding3D =
                serde_json::from_value(c.data.clone()).map_err(|e| invalid(e.to_string()))?;
            binding.validate().map_err(|e| invalid(e.to_string()))?;
            let rig = rigs
                .get(binding.character.0.id.as_str())
                .ok_or_else(|| invalid("bound rig missing"))?;
            if !binding.play_on_awake
                || binding.joints.len() != 64
                || binding.visual_interval != 1
                || binding.visual_max_distance.is_some()
            {
                return Err(invalid(
                    "100 characters must play all 64 joints at full visual rate",
                ));
            }
            let graph = binding
                .graph
                .as_ref()
                .ok_or_else(|| invalid("missing actual two-clip graph"))?;
            graph.validate(rig).map_err(|e| invalid(e.to_string()))?;
            let base = graph
                .states
                .iter()
                .find(|s| s.key == graph.initial)
                .ok_or_else(|| invalid("initial clip missing"))?;
            if graph.layers.len() != 1
                || graph.layers[0].weight <= 0.
                || graph.layers[0].clip == base.clip
            {
                return Err(invalid(
                    "base and a different positive-weight second clip must both be active",
                ));
            }
            let layer = &graph.layers[0];
            if layer.additive {
                if layer.mask.is_empty() || layer.mask.len() >= 64 {
                    return Err(invalid("masked additive requires a proper joint subset"));
                }
                masked += 1;
            } else if !layer.mask.is_empty() {
                return Err(invalid("non-additive target uses the full body"));
            }
            for (i, target) in binding.joints.iter().enumerate() {
                let joint = entities
                    .get(target.entity_ref.as_str())
                    .ok_or_else(|| invalid("missing live skeleton target"))?;
                if joint.spatial.is_none() || !all_targets.insert(target.entity_ref.clone()) {
                    return Err(invalid("missing spatial joint or aliased skeleton"));
                }
                let parent = rig.joints[i]
                    .parent
                    .map(|p| binding.joints[p as usize].entity_ref.as_str())
                    .unwrap_or(e.id.as_str());
                if joint.parent_id.as_deref() != Some(parent) {
                    return Err(invalid(
                        "actual joint hierarchy disagrees with cooked skeleton",
                    ));
                }
            }
            let skin = &rig.skins[0];
            let mesh_entity =
                entities[binding.joints[skin.mesh_joint as usize].entity_ref.as_str()];
            if !matches!(&mesh_entity.spatial,Some(RuntimeSpatial::World3D{mesh_renderer:Some(mesh),..}) if mesh.visible&&mesh.mesh.0.id==skin.mesh_asset_id)
            {
                return Err(invalid(
                    "actual character skin target must bind its visible cooked mesh",
                ));
            }
            *used_rigs.entry(rig.asset_id.as_str()).or_default() += 1;
            characters += 1;
        }
        if let Some(RuntimeSpatial::World3D {
            mesh_renderer: Some(mesh),
            ..
        }) = &e.spatial
        {
            if !mesh.visible {
                return Err(invalid("hidden mesh cannot qualify for all-visible BM05"));
            }
            if !skin_mesh_ids.contains(mesh.mesh.0.id.as_str()) {
                static_instances += 1;
                static_triangles += meshes
                    .get(mesh.mesh.0.id.as_str())
                    .ok_or_else(|| invalid("missing static mesh payload"))?
                    .1;
            }
        }
    }
    if (characters, masked, static_instances, static_triangles) != (100, 20, 1000, 1000000)
        || used_rigs.len() != 10
        || used_rigs.values().any(|n| *n != 10)
        || all_targets.len() != 6400
    {
        return Err(invalid(format!("actual BM05 differs: characters={characters}, masked={masked}, static={static_instances}/{static_triangles}, used rigs={used_rigs:?}, joints={}",all_targets.len())));
    }
    Ok(
        json!({"scope":"BM-05/sim-v1-full-PC-cooked-payloads-and-bindings","characters":characters,"rigs":10,"bonesPerRig":64,"actualBoundJoints":6400,"verticesPerCharacter":15000,"trianglesPerCharacter":25000,"positiveInfluencesPerVertex":4,"clipsPerRig":3,"simultaneousClips":2,"maskedAdditiveCharacters":masked,"staticInstances":static_instances,"staticTriangles":static_triangles,"qualification":"cooked-input-only","notExecuted":["actual-Player-deformation-and-pixels","full-K3-cycle","formal-performance","Android-device"]}),
    )
}

pub(super) fn simulation_scenario(
    prepared: &PreparedRuntimePackage,
    workload: &str,
    frames: u64,
    timeout: u64,
) -> BenchmarkResult<runtime_player_winit::semantic_outcome::PlaytestScenario> {
    prepared
        .load_playtest_scenario()
        .map_err(|e| invalid(e.to_string()))?;
    let manifest = prepared
        .source()
        .project_manifest_value()
        .map_err(|e| invalid(e.to_string()))?;
    let path = manifest["playtestScenario"]
        .as_str()
        .ok_or_else(|| invalid("simulation scenario path missing"))?;
    let scenario: runtime_player_winit::semantic_outcome::PlaytestScenario =
        serde_json::from_slice(
            prepared
                .source()
                .bytes(path)
                .ok_or_else(|| invalid("simulation scenario source missing"))?,
        )
        .map_err(|e| invalid(e.to_string()))?;
    let expected_ticks = match workload {
        "BM-05" => 720,
        "BM-06" | "BM-07" => 1200,
        "BM-09" => 660,
        _ => return Err(invalid("unsupported simulation scenario workload")),
    };
    if scenario.target != runtime_player_winit::semantic_outcome::PlaytestTarget::WindowsWindowed
        || scenario.max_simulation_ticks != expected_ticks
        || scenario.max_presentation_frames != frames
        || frames != expected_ticks
        || scenario.timeout_ms > timeout
    {
        return Err(invalid(format!("{workload} requires its complete leased {expected_ticks}-tick functional scenario; this does not qualify full K3/performance")));
    }
    if workload == "BM-05" {
        validate_bm05_scenario(&scenario)?;
    } else if workload == "BM-06" {
        validate_bm06_scenario(&scenario)?;
    } else if workload == "BM-09" {
        precision::validate_scenario(&scenario)?;
    } else {
        navigation::validate_scenario(&scenario)?;
    }
    Ok(scenario)
}
fn validate_bm06_scenario(
    scenario: &runtime_player_winit::semantic_outcome::PlaytestScenario,
) -> BenchmarkResult<()> {
    let has = |tick, path: &str, value: Value| {
        scenario.assertions.iter().any(|a| {
            a.from_simulation_tick == tick
                && a.through_simulation_tick == tick
                && a.path == path
                && serde_json::to_value(&a.equals).ok().as_ref() == Some(&value)
        })
    };
    for tick in [120, 360, 600, 900, 1200] {
        for (path, count) in [("actualDynamicBodies", 1000), ("actualMotorStates", 2)] {
            if !has(tick, &format!("bm06.{path}"), json!(count)) {
                return Err(invalid(
                    "BM06 requires actual body and motor states in all three input segments",
                ));
            }
        }
    }
    for tick in [900, 1200] {
        if !has(tick, "bm06.drivenAwakeTargetMet", json!(true)) {
            return Err(invalid("BM06 must check the >=90% awake driven segment"));
        }
    }
    for tick in [120, 600, 1200] {
        for (path, value) in [
            ("raycastsPerTick", json!(200)),
            ("sweepsPerTick", json!(50)),
            ("queryBatchHasHit", json!(true)),
            ("querySnapshotIsPriorTick", json!(true)),
        ] {
            if !has(tick, &format!("bm06.{path}"), value) {
                return Err(invalid(
                    "BM06 requires real ordered per-tick query consumption",
                ));
            }
        }
    }
    for (tick, phase) in [
        (61, "step"),
        (241, "slope"),
        (361, "steep"),
        (481, "ceiling"),
        (601, "platform"),
        (602, "teleport"),
        (901, "crossCell"),
        (1081, "returnCell"),
    ] {
        if !has(tick, &format!("bm06.motors.{phase}Count"), json!(2)) {
            return Err(invalid(
                "BM06 requires both motors to complete the actual obstacle/cell itinerary",
            ));
        }
    }
    for tick in [120, 600, 1200] {
        if !scenario
            .captures
            .iter()
            .any(|c| c.presentation_frame == tick && c.required && !c.subjective_review)
        {
            return Err(invalid(
                "BM06 requires all three comparable functional captures",
            ));
        }
    }
    Ok(())
}
fn validate_bm05_scenario(
    scenario: &runtime_player_winit::semantic_outcome::PlaytestScenario,
) -> BenchmarkResult<()> {
    let has = |tick, path: &str, value: &Value| {
        scenario.assertions.iter().any(|a| {
            a.from_simulation_tick == tick
                && a.through_simulation_tick == tick
                && a.path == path
                && serde_json::to_value(&a.equals).ok().as_ref() == Some(value)
        })
    };
    for (tick, path, value) in [
        (160, "actualActors", json!(100)),
        (160, "actualJoints", json!(6400)),
        (160, "actorsWithEvents", json!(100)),
        (340, "actualActors", json!(100)),
        (340, "actualJoints", json!(6400)),
        (340, "offscreenActors", json!(75)),
        (340, "offscreenActorsWithNewEvents", json!(75)),
        (380, "pausedPoseUnchanged", json!(true)),
        (410, "pausedStepChangedPose", json!(true)),
        (600, "actualActors", json!(100)),
        (600, "actualJoints", json!(6400)),
        (600, "offscreenActors", json!(0)),
        (600, "actualRootMotionModes", json!(10)),
        (600, "rootMotionActorsMoved", json!(10)),
        (600, "motorActorsMoved", json!(10)),
    ] {
        if !has(tick, &format!("bm05.{path}"), &value) {
            return Err(invalid(format!(
                "missing actual BM05 checkpoint {tick}/{path}"
            )));
        }
    }
    for tick in [160, 340, 410, 600] {
        if !scenario
            .captures
            .iter()
            .any(|c| c.presentation_frame == tick && c.required && !c.subjective_review)
        {
            return Err(invalid(
                "BM05 requires the four comparable functional captures",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bm06_scenario_requires_sorted_observations_queries_awake_and_both_motor_routes() {
        let scenario: runtime_player_winit::semantic_outcome::PlaytestScenario =
            serde_json::from_slice(include_bytes!(
                "../../../../../samples/platform_3d_bm06_project/Tests/bm06.scenario.json"
            ))
            .unwrap();
        let contract: engine_runtime::project_observation::ProjectObservationContract = serde_json::from_slice(include_bytes!("../../../../../samples/platform_3d_bm06_project/Observations/project.observations.json")).unwrap();
        scenario.validate(Some(&contract)).unwrap();
        validate_bm06_scenario(&scenario).unwrap();
        let mut unsorted = scenario.clone();
        unsorted.assertions.reverse();
        assert!(unsorted.validate(Some(&contract)).is_err());
        for missing in 0..scenario.assertions.len() {
            let mut incomplete = scenario.clone();
            incomplete.assertions.remove(missing);
            assert!(
                validate_bm06_scenario(&incomplete).is_err(),
                "missing actual BM06 assertion {missing}"
            );
        }
        for missing in 0..scenario.captures.len() {
            let mut incomplete = scenario.clone();
            incomplete.captures.remove(missing);
            assert!(validate_bm06_scenario(&incomplete).is_err());
        }
    }
    #[test]
    fn bm05_scenario_rejects_missing_actual_joint_event_pause_and_motion_assertions() {
        let scenario: runtime_player_winit::semantic_outcome::PlaytestScenario =
            serde_json::from_slice(include_bytes!(
                "../../../../../samples/platform_3d_bm05_project/Tests/bm05.scenario.json"
            ))
            .unwrap();
        let contract: engine_runtime::project_observation::ProjectObservationContract =
            serde_json::from_slice(include_bytes!(
                "../../../../../samples/platform_3d_bm05_project/Observations/project.observations.json"
            ))
            .unwrap();
        scenario.validate(Some(&contract)).unwrap();
        validate_bm05_scenario(&scenario).unwrap();
        for missing in 0..scenario.assertions.len() {
            let mut incomplete = scenario.clone();
            incomplete.assertions.remove(missing);
            assert!(
                validate_bm05_scenario(&incomplete).is_err(),
                "missing assertion {missing}"
            );
        }
        for missing in 0..scenario.captures.len() {
            let mut incomplete = scenario.clone();
            incomplete.captures.remove(missing);
            assert!(
                validate_bm05_scenario(&incomplete).is_err(),
                "missing capture {missing}"
            );
        }
    }
}
