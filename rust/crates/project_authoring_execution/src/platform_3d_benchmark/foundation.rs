//! Foundation admission consumes the Compiler's actual spatial scene and AUI.
use super::{BenchmarkError, BenchmarkResult};
use crate::PreparedRuntimePackage;
use engine_runtime::{
    aui::AuiDocument, render_contract::Projection, spatial_contract::RuntimeSpatial,
};
use serde_json::{json, Value};
use std::collections::BTreeSet;

fn invalid(message: impl Into<String>) -> BenchmarkError {
    BenchmarkError::new("benchmark.foundation_payload_ineligible", message)
}

pub fn inspect_foundation_projection(prepared: &PreparedRuntimePackage) -> BenchmarkResult<Value> {
    let descriptor: Value = serde_json::from_slice(
        prepared
            .source()
            .bytes("Benchmark/foundation-manifest.json")
            .ok_or_else(|| invalid("missing BM09 source descriptor"))?,
    )
    .map_err(|e| invalid(e.to_string()))?;
    if descriptor["workload"] != "BM-09"
        || descriptor["objects"] != 1000
        || descriptor["anchorsM"] != json!([0, 10000, 1000000])
        || descriptor["relocationsPerAnchor"] != 20
        || descriptor["motionProbes"] != 10
    {
        return Err(invalid(
            "BM09 descriptor differs from frozen foundation recipe",
        ));
    }
    let input = prepared.runtime_package_build_input();
    if input.scenes.len() != 1
        || input.scenes[0].entities.len() != 1001
        || input.scenes[0].main_camera.as_deref() != Some("camera")
        || !input.aui_documents.is_empty()
    {
        return Err(invalid(
            "BM09 requires 1000 rendered spatial roots and one selected camera",
        ));
    }
    let mut rendered = 0;
    let mut ids = BTreeSet::new();
    for e in &input.scenes[0].entities {
        if !e.enabled || e.parent_id.is_some() || !ids.insert(e.id.as_str()) {
            return Err(invalid("BM09 requires live distinct roots"));
        }
        match &e.spatial {
            Some(RuntimeSpatial::World3D {
                mesh_renderer: Some(mesh),
                camera: None,
                light: None,
                ..
            }) if mesh.visible && e.id != "camera" => rendered += 1,
            Some(RuntimeSpatial::World3D {
                mesh_renderer: None,
                camera: Some(camera),
                light: None,
                ..
            }) if e.id == "camera" && camera.enabled => {}
            _ => return Err(invalid("BM09 actual render/camera content mismatch")),
        }
    }
    for i in 0..1000 {
        if !ids.contains(format!("node-{i}").as_str()) {
            return Err(invalid("missing BM09 object identity"));
        }
    }
    if rendered != 1000 {
        return Err(invalid("wrong actual rendered count"));
    }
    Ok(
        json!({"scope":"BM-09/foundation-v1-PC-cook","renderedSpatial":rendered,"cameras":1,"qualification":"unqualified"}),
    )
}

pub fn inspect_foundation_hud(prepared: &PreparedRuntimePackage) -> BenchmarkResult<Value> {
    let manifest: Value = serde_json::from_slice(
        prepared
            .source()
            .bytes("Benchmark/foundation-manifest.json")
            .ok_or_else(|| invalid("missing foundation source descriptor"))?,
    )
    .map_err(|e| invalid(e.to_string()))?;
    let track = manifest["track"]
        .as_str()
        .ok_or_else(|| invalid("missing track"))?;
    if !matches!(track, "K0" | "K1")
        || manifest["seed"] != 57739792
        || manifest["periodTicks"] != 7200
    {
        return Err(invalid("track/seed/period must match frozen BM01 catalog"));
    }
    if ![0, 1, 10, 100]
        .iter()
        .any(|v| manifest["dirtyPercent"] == *v)
    {
        return Err(invalid("dirty percent must be one frozen variant"));
    }
    let mut result = inspect_hud_input(prepared.runtime_package_build_input(), track)?;
    result["dirtyPercent"] = manifest["dirtyPercent"].clone();
    Ok(result)
}

fn inspect_hud_input(
    input: &engine_runtime::runtime_package_builder::RuntimePackageBuildInput,
    track: &str,
) -> BenchmarkResult<Value> {
    if input.scenes.len() != 1 {
        return Err(invalid("one active scene required"));
    }
    let scene = &input.scenes[0];
    if scene.entities.len() != 10001 || scene.main_camera.as_deref() != Some("camera") {
        return Err(invalid(
            "10000 live pure spatial entities and one selected camera required",
        ));
    }
    let mut ids = BTreeSet::new();
    let mut spatial = 0;
    for entity in &scene.entities {
        if !entity.enabled
            || entity.parent_id.is_some()
            || !entity.components.is_empty()
            || !ids.insert(entity.id.as_str())
        {
            return Err(invalid(
                "foundation entities must be distinct live roots without hidden workloads",
            ));
        }
        let Some(RuntimeSpatial::World3D {
            mesh_renderer: None,
            light: None,
            camera,
            visibility,
            ..
        }) = &entity.spatial
        else {
            return Err(invalid(
                "pure spatial entities cannot contain mesh/light/legacy payloads",
            ));
        };
        if visibility.as_ref().is_some_and(|v| !v.visible) {
            return Err(invalid(
                "hidden means no renderer, not disabled spatial state",
            ));
        }
        if entity.id == "camera" {
            let camera = camera.as_ref().ok_or_else(|| invalid("missing camera"))?;
            let valid = match (&camera.projection, track) {
                (
                    Projection::Perspective {
                        fov_y_degrees,
                        near,
                        far,
                    },
                    "K1",
                ) => *fov_y_degrees == 60. && *near == 0.1 && *far == 2000.,
                (Projection::Orthographic { size, near, far }, "K0") => {
                    *size == 220. && *near == 0.1 && *far == 2000.
                }
                _ => false,
            };
            if !camera.enabled || !valid {
                return Err(invalid("camera projection does not match frozen track"));
            }
        } else {
            if camera.is_some() {
                return Err(invalid("extra camera"));
            }
            spatial += 1;
        }
    }
    for i in 0..10000 {
        if !ids.contains(format!("node-{i}").as_str()) {
            return Err(invalid("missing pure spatial identity"));
        }
    }
    if input.aui_documents.len() != 1 {
        return Err(invalid("one HUD document required"));
    }
    let hud: AuiDocument = serde_json::from_value(input.aui_documents[0].document.clone())
        .map_err(|e| invalid(e.to_string()))?;
    if hud.nodes.len() != 21
        || hud.canvases.len() != 1
        || hud
            .nodes
            .iter()
            .filter(|n| n.node_id.starts_with("hud-") && n.visible)
            .count()
            != 20
    {
        return Err(invalid("exactly 20 visible HUD panels plus root required"));
    }
    Ok(
        json!({"scope":"BM-01/foundation-v1-PC-cook","track":track,"pureSpatial":spatial,"cameras":1,"lights":0,"hud":20,"qualification":"unqualified"}),
    )
}

/// Validate the leased scenario before build. A frame receipt alone cannot prove
/// a complete simulation window, so that claim uses the semantic owner.
pub(super) fn foundation_scenario(
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
        .ok_or_else(|| invalid("scenario path"))?;
    let scenario: runtime_player_winit::semantic_outcome::PlaytestScenario =
        serde_json::from_slice(
            prepared
                .source()
                .bytes(path)
                .ok_or_else(|| invalid("scenario source"))?,
        )
        .map_err(|e| invalid(e.to_string()))?;
    let expected_frames = if workload == "BM-09" { 63 } else { 7201 };
    if scenario.target != runtime_player_winit::semantic_outcome::PlaytestTarget::WindowsWindowed
        || scenario.max_simulation_ticks != expected_frames
        || scenario.max_presentation_frames != frames
        || frames != expected_frames
        || scenario.timeout_ms > timeout
    {
        return Err(invalid("foundation scenario must contain the complete recipe window (BM01 7201 or BM09 63 ticks), within request timeout"));
    }
    if workload == "BM-09" {
        validate_projection_scenario(&scenario)?;
        return Ok(scenario);
    }
    let descriptor: Value = serde_json::from_slice(
        prepared
            .source()
            .bytes("Benchmark/foundation-manifest.json")
            .ok_or_else(|| invalid("source descriptor"))?,
    )
    .map_err(|e| invalid(e.to_string()))?;
    validate_hud_scenario(
        &scenario,
        descriptor["track"].as_str().unwrap_or(""),
        descriptor["dirtyPercent"].as_u64().unwrap_or(u64::MAX),
    )?;
    Ok(scenario)
}

fn validate_projection_scenario(
    scenario: &runtime_player_winit::semantic_outcome::PlaytestScenario,
) -> BenchmarkResult<()> {
    let has = |tick: u64, path: &str, value: Value| {
        scenario.assertions.iter().any(|a| {
            a.from_simulation_tick == tick
                && a.through_simulation_tick == tick
                && a.path == path
                && serde_json::to_value(&a.equals).ok().as_ref() == Some(&value)
        })
    };
    for anchor in 0..3 {
        for round in [0, 1, 20] {
            let tick = anchor * 21 + round + 1;
            for (path, value) in [
                ("anchor", json!(anchor)),
                ("relocations", json!(round)),
                ("positionMatching", json!(1000)),
                ("cameraX", json!(-(round as f64) * 16.)),
                ("positionWithinBudget", json!(true)),
            ] {
                if !has(tick, &format!("projection.{path}"), value) {
                    return Err(invalid("BM09 missing actual relocation checkpoint"));
                }
            }
        }
        if !scenario.captures.iter().any(|c| {
            c.presentation_frame == (anchor + 1) * 21 && c.required && !c.subjective_review
        }) {
            return Err(invalid("BM09 needs three comparable anchor captures"));
        }
    }
    Ok(())
}

fn validate_hud_scenario(
    scenario: &runtime_player_winit::semantic_outcome::PlaytestScenario,
    track: &str,
    dirty: u64,
) -> BenchmarkResult<()> {
    let has = |tick: u64, path: &str, value: Value| {
        scenario.assertions.iter().any(|a| {
            a.from_simulation_tick == tick
                && a.through_simulation_tick == tick
                && a.path == path
                && serde_json::to_value(&a.equals).ok().as_ref() == Some(&value)
        })
    };
    for local in [0, 1, 900, 1800, 2700, 3600, 4500, 5400, 6300, 7200] {
        for (path, value) in [
            ("foundation.phaseTick", json!(local)),
            ("foundation.committedMatching", json!(10000)),
        ] {
            if !has(local + 1, path, value) {
                return Err(invalid("missing complete committed-state checkpoint"));
            }
        }
    }
    for tick in [1, 7201] {
        if !has(tick, "foundation.dirtyTarget", json!(dirty * 100)) {
            return Err(invalid("dirty variant checkpoint mismatch"));
        }
    }
    let catalog = super::target_catalog()?;
    for (key, local) in [0, 1800, 3600, 5400, 7200].into_iter().enumerate() {
        for (axis, name) in ["x", "y", "z"].iter().enumerate() {
            let position = if track == "K1" {
                catalog["tracks"]["K1"]["keys"][key]["position"][axis]
                    .as_f64()
                    .unwrap()
            } else {
                [0., 0., 150.][axis]
            };
            if !has(
                local + 1,
                &format!("foundation.camera.{name}"),
                json!(position),
            ) {
                return Err(invalid("missing frozen camera checkpoint"));
            }
        }
    }
    if !scenario
        .captures
        .iter()
        .any(|c| c.presentation_frame == 7201 && c.required && !c.subjective_review)
    {
        return Err(invalid("terminal HUD capture required"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_runtime::{aui::*, runtime_package::RuntimeProjectInfo, runtime_package_builder::*};
    #[test]
    fn projection_scenario_rejects_missing_committed_checkpoint_or_anchor_capture() {
        let mut assertions = Vec::new();
        for anchor in 0..3 {
            for round in [0, 1, 20] {
                let tick = anchor * 21 + round + 1;
                for (path, value) in [
                    ("anchor", json!(anchor)),
                    ("relocations", json!(round)),
                    ("positionMatching", json!(1000)),
                    ("cameraX", json!(-(round as f64) * 16.)),
                    ("positionWithinBudget", json!(true)),
                ] {
                    assertions.push(json!({"assertionId":format!("{anchor}-{round}-{path}"),"fromSimulationTick":tick,"throughSimulationTick":tick,"path":format!("projection.{path}"),"equals":value}));
                }
            }
        }
        let scenario:runtime_player_winit::semantic_outcome::PlaytestScenario=serde_json::from_value(json!({
            "schemaVersion":"playtest-scenario.v2","scenarioId":"bm09-unit","initialSceneId":"main","target":"windows-windowed",
            "maxSimulationTicks":63,"maxPresentationFrames":63,"timeoutMs":120000,"inputs":[],"assertions":assertions,
            "captures":(0..3).map(|a|json!({"captureId":format!("anchor-{a}"),"presentationFrame":(a+1)*21,"required":true,"subjectiveReview":false})).collect::<Vec<_>>()
        })).unwrap();
        validate_projection_scenario(&scenario).unwrap();
        let mut missing = scenario.clone();
        missing.assertions.pop();
        assert!(validate_projection_scenario(&missing).is_err());
        let mut capture = scenario.clone();
        capture.captures[2].presentation_frame = 62;
        assert!(validate_projection_scenario(&capture).is_err());
        let mut subjective = scenario;
        subjective.captures[0].subjective_review = true;
        assert!(validate_projection_scenario(&subjective).is_err());
    }
    fn input() -> RuntimePackageBuildInput {
        let engine_runtime::spatial_contract::RuntimeSceneDocument::Spatial(scene) =
            engine_runtime::spatial_contract::decode_runtime_scene(include_bytes!(
                "../../../engine_runtime/tests/fixtures/three_d_a0/runtime-scene-v2.json"
            ))
            .unwrap()
        else {
            panic!()
        };
        let mut scene: engine_runtime::runtime_package::RuntimeScene = scene.into();
        let mut node = scene.entities[0].clone();
        let mut camera = scene.entities[1].clone();
        if let Some(RuntimeSpatial::World3D { mesh_renderer, .. }) = &mut node.spatial {
            *mesh_renderer = None;
        }
        if let Some(RuntimeSpatial::World3D {
            camera: Some(c), ..
        }) = &mut camera.spatial
        {
            c.projection = Projection::Perspective {
                fov_y_degrees: 60.,
                near: 0.1,
                far: 2000.,
            };
        }
        scene.entities = (0..10000)
            .map(|i| {
                let mut n = node.clone();
                n.id = format!("node-{i}");
                n
            })
            .collect();
        scene.entities.push(camera);
        let mut input = RuntimePackageBuildInput::new(RuntimeProjectInfo::explicit_empty(
            "foundation",
            "Foundation",
            "1",
        ));
        input.scenes.push(scene);
        let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
        root.children = (0..20).map(|i| format!("hud-{i}")).collect();
        let mut nodes = vec![root];
        for i in 0..20 {
            nodes.push(AuiNode::new(
                format!("hud-{i}"),
                AuiNodeKind::Panel,
                AuiRect::fixed_position(0., 0., 24., 24.),
            ));
        }
        let hud = AuiDocument::new(
            "hud",
            vec![AuiCanvas::screen_overlay("main", 1920., 1080., "root")],
            nodes,
        );
        input.aui_documents.push(RuntimePackageSourceJson {
            id: "hud".into(),
            document: serde_json::to_value(hud).unwrap(),
        });
        input
    }
    #[test]
    fn hud_actual_input_rejects_missing_disabled_wrong_domain_camera_and_hud() {
        let baseline = input();
        inspect_hud_input(&baseline, "K1").unwrap();
        for case in 0..6 {
            let mut input = baseline.clone();
            match case {
                0 => {
                    input.scenes[0].entities.pop();
                }
                1 => input.scenes[0].entities[0].enabled = false,
                2 => input.scenes[0].entities[0].spatial = None,
                3 => input.scenes[0].entities[0].id = "node-1".into(),
                4 => input.aui_documents[0].document["nodes"][1]["visible"] = json!(false),
                5 => input.scenes[0].main_camera = None,
                _ => unreachable!(),
            }
            assert!(
                inspect_hud_input(&input, "K1").is_err(),
                "bad actual case {case}"
            );
        }
        assert!(
            inspect_hud_input(&baseline, "K0").is_err(),
            "perspective does not qualify K0"
        );
    }
}
