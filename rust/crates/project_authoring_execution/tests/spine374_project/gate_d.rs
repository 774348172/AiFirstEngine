// Included by the existing Compiler consumer, sharing its owned source/package fixtures.
use super::*;
use engine_runtime::{
    frame_loop::*, project_runtime_session::*, spine_animation::session::*, world::World,
};
use project_runtime_sdk::{animator::AnimatorSnapshot, ProjectRuntimeAnimatorIntent as Intent};
use std::collections::{BTreeMap, VecDeque};

#[path = "gate_f_lifecycle.rs"]
mod gate_f_lifecycle;

#[test]
fn spine374_d_author_cook_inspection_and_package_binding() {
    let f = Fixture::new("d-author");
    f.authored();
    let checked = f.check().unwrap();
    assert_eq!(checked.spine_inspection.capability_status, "planned");
    let actions = &checked.spine_inspection.controllers[0].actions;
    assert_eq!(
        actions.iter().map(|a| a.mix_in_ticks).collect::<Vec<_>>(),
        [0, 6, 1]
    );
    assert_eq!(actions[2].fade_out_ticks, Some(12));
    let input = f.prepare().unwrap();
    let registry = &input.spine_animator.as_ref().unwrap().registry;
    assert_eq!(registry.controllers.len(), 1);
    assert_eq!(registry.controllers[0].masks[0].bones, ["upper"]);
    let wire = serde_json::to_value(&input.scenes[0].entities[0]).unwrap();
    assert_eq!(wire["spineAnimator"]["controllerId"], "hero-controller");
    assert!(wire["components"].as_array().unwrap().is_empty());
    let decoded: RuntimeEntity = serde_json::from_value(wire).unwrap();
    assert_eq!(
        serde_json::to_value(decoded).unwrap(),
        serde_json::to_value(&input.scenes[0].entities[0]).unwrap()
    );
    let root = f.package(&input);
    fs::rename(&f.source, f.root.join("source-offline")).unwrap();
    let package = load_runtime_package(&root).value.unwrap();
    let world = engine_runtime::scene_loader::load_scene_into_world(&package.active_scene)
        .value
        .unwrap();
    let module = SpineModule::load(&package, &world).unwrap();
    assert_eq!(module.snapshot("robot").unwrap().sockets.len(), 1);
    let player = runtime_player_winit::run_headless_native_player_from_package(
        runtime_player_winit::NativePlayerWindowRunRequest::headless_surface_gate(&root)
            .with_runtime_report_level(
                engine_runtime::windowed_player::WindowedPlayerRuntimeReportLevel::Summary,
            ),
    );
    assert_eq!(player.exit_code, 0, "{player:#?}");
    let observed = player
        .spine
        .as_ref()
        .expect("Player omitted explicit Spine Summary");
    assert_eq!(observed.instance_count, 1);
    assert!(observed.tick.is_some());
    assert!(observed.instances.is_empty());
    fs::write(
        f.root.join("player-spine-summary.json"),
        serde_json::to_vec_pretty(&player).unwrap(),
    )
    .unwrap();
    let mut broken = input.clone();
    broken.scenes[0].entities[0].components[0].data["registryDigest"] = json!("sha256:stale");
    let result = RuntimePackageBuilder::build(
        &RuntimePackageBuildRequest::dev_desktop(f.root.join("bad-package"), "main"),
        &broken,
    );
    assert_eq!(result.status, RuntimePackageBuildStatus::Failed);
}

#[test]
fn spine374_d_check_rejects_author_types_names_duplicates_masks_and_bindings() {
    for case in [
        "unknown-field",
        "parameter-type",
        "trigger-default",
        "duplicate-layer",
        "unknown-action",
        "unknown-clip",
        "unknown-bone",
        "empty-mask",
        "bad-mask",
        "additive-deform",
        "duration",
        "once",
        "entity",
        "unknown-skeleton",
        "unknown-schema",
        "unknown-backend",
    ] {
        let f = Fixture::new(case);
        let mut d = f.authored();
        match case {
            "unknown-field" => d["seek"] = json!(1),
            "parameter-type" => d["rules"][0]["all"] = json!([{"parameter":"fire","equals":true}]),
            "trigger-default" => d["parameters"]["fire"]["default"] = json!(true),
            "duplicate-layer" => d["layers"][1]["name"] = json!("base"),
            "unknown-action" => d["default"] = json!("absent"),
            "unknown-clip" => d["animations"]["idle"]["clip"] = json!("absent"),
            "unknown-bone" => d["sockets"]["hand"]["bone"] = json!("absent"),
            "empty-mask" => d["masks"]["upper"] = json!([]),
            "bad-mask" => d["masks"]["upper"] = json!(["root"]),
            "additive-deform" => d["layers"][1]["blend"] = json!("additive"),
            "duration" => d["animations"]["idle"]["mixInSeconds"] = json!(-1),
            "once" => d["animations"]["wave"]["playback"] = json!("loop"),
            "entity" => d["entity"] = json!("absent"),
            "unknown-skeleton" => d["skeleton"]["assetId"] = json!("absent"),
            "unknown-schema" => d["schema"] = json!("animator-description-2d.v3"),
            "unknown-backend" => d["backend"] = json!("unknown"),
            _ => unreachable!(),
        }
        f.write("Animations/hero.animator-description-2d.json", &d);
        assert!(f.check().is_err(), "{case} unexpectedly checked");
    }
    let f = Fixture::new("duplicate-json");
    let d = f.authored();
    let text = serde_json::to_string(&d).unwrap().replace(
        "\"moving\":{",
        "\"moving\":{\"kind\":\"bool\",\"default\":false},\"moving\":{",
    );
    fs::write(
        f.source
            .join("Animations/hero.animator-description-2d.json"),
        text,
    )
    .unwrap();
    assert!(f.check().is_err());
}

#[test]
fn spine374_d_author_change_reuses_skeleton_cook_and_changes_controller_digest() {
    use project_authoring_execution::ProjectAssemblyArtifactCacheStatus;
    let f = Fixture::new("d-author-cache");
    let mut d = f.authored();
    let first = f.prepare().unwrap();
    let initial = &first.spine_animator.as_ref().unwrap().registry;
    d["animations"]["run"]["mixInSeconds"] = json!(0.11);
    f.write("Animations/hero.animator-description-2d.json", &d);
    let (second, reports) = f.prepare_reported().unwrap();
    let next = &second.spine_animator.as_ref().unwrap().registry;
    assert_eq!(initial.assets, next.assets);
    assert_ne!(initial.registry_digest, next.registry_digest);
    assert_eq!(next.controllers[0].actions[1].mix_in_ticks, 7);
    assert!(reports
        .iter()
        .filter(|r| r.producer_id == "spine-cook")
        .all(|r| r.cache_status == ProjectAssemblyArtifactCacheStatus::Hit));
    assert!(reports.iter().any(|r| r.producer_id == "spine-cook"));
}

struct Session {
    snapshots: BTreeMap<String, AnimatorSnapshot>,
    seen: Vec<(bool, AnimatorSnapshot)>,
    aui: VecDeque<Vec<Intent>>,
    fixed: VecDeque<Vec<Intent>>,
    no_op: bool,
    write: bool,
    stale: bool,
    bad_world: bool,
}
impl Session {
    fn new() -> Self {
        Self {
            snapshots: BTreeMap::new(),
            seen: Vec::new(),
            aui: VecDeque::new(),
            fixed: VecDeque::new(),
            no_op: false,
            write: false,
            stale: false,
            bad_world: false,
        }
    }
    fn output(
        &mut self,
        context: ProjectRuntimeSessionContext<'_>,
        is_aui: bool,
    ) -> ProjectRuntimeSessionOutput {
        self.seen.push((is_aui, self.snapshots["robot"].clone()));
        let mut mutations = ProjectRuntimeMutationBuffer::new();
        if self.write {
            let mut transform = context.world.read_transform(&"robot".into()).unwrap();
            transform.local_position.x = 77.;
            mutations.write_transform("robot".into(), transform);
        }
        if self.bad_world {
            mutations.write_component_field(
                "robot".into(),
                engine_runtime::components::ComponentTypeId::from("Absent"),
                engine_runtime::field_path::FieldPath::parse("x").unwrap(),
                engine_runtime::component_value::RuntimeValue::Bool(true),
            );
        }
        let commands = if is_aui {
            self.aui.pop_front()
        } else {
            self.fixed.pop_front()
        }
        .unwrap_or_default();
        for intent in commands {
            mutations.animator_command(AnimatorCommand {
                entity_id: "robot".into(),
                generation: context.world.entity_generation(&"robot".into()).unwrap()
                    + u64::from(self.stale),
                intent,
            });
        }
        let mut output = ProjectRuntimeSessionOutput::applied(mutations);
        if self.no_op {
            output.status = ProjectRuntimeSessionStatus::NoOp;
        }
        output
    }
}
impl ProjectRuntimeSession for Session {
    fn session_id(&self) -> &str {
        "spine374-d-session"
    }
    fn accept_animator_snapshots(&mut self, s: BTreeMap<String, AnimatorSnapshot>) {
        self.snapshots = s;
    }
    fn handle_aui_actions(
        &mut self,
        c: ProjectRuntimeSessionContext<'_>,
        _: ProjectAuiActionBatch<'_>,
    ) -> ProjectRuntimeSessionOutput {
        self.output(c, true)
    }
    fn fixed_update(&mut self, c: ProjectRuntimeSessionContext<'_>) -> ProjectRuntimeSessionOutput {
        self.output(c, false)
    }
}
struct Runtime {
    package: RuntimePackage,
    world: World,
    loader: engine_runtime::runtime_instance_loader::RuntimeInstanceLoader,
    frame_loop: FrameLoop,
    render: engine_runtime::render_state::RenderSceneState,
    extract: engine_runtime::render_extract::RenderExtractContext,
}
impl Runtime {
    fn new(f: &Fixture) -> Self {
        let input = f.prepare().unwrap();
        let root = f.package(&input);
        let package = load_runtime_package(&root).value.unwrap();
        let world = engine_runtime::scene_loader::load_scene_into_world(&package.active_scene)
            .value
            .unwrap();
        let loader =
            engine_runtime::runtime_instance_loader::RuntimeInstanceLoader::from_package(&package);
        Self {
            package,
            world,
            loader,
            frame_loop: FrameLoop::new("main"),
            render: Default::default(),
            extract: Default::default(),
        }
    }
    fn tick(
        &mut self,
        s: &mut Session,
        aui: bool,
        steps: usize,
    ) -> Result<RuntimeFrameOutput, RuntimeFrameSessionFault> {
        let action = [engine_runtime::aui::AuiAction {
            action_id: "animate".into(),
            node_id: "button".into(),
            event: engine_runtime::aui::AuiActionEvent::Activate,
            payload: None,
        }];
        self.frame_loop
            .tick_runtime_frame_with_project_session_delta_and_fixed_steps(
                &mut self.world,
                &mut self.render,
                &mut self.extract,
                None,
                None,
                1. / 60.,
                Some(RuntimeFrameContext::new(&self.package, &mut self.loader)),
                ProjectRuntimeFrameSession {
                    session: s,
                    actions: if aui { &action } else { &[] },
                    report_level: ProjectRuntimeSessionReportLevel::Trace,
                    observation_contract: None,
                },
                steps,
            )
    }
}

#[test]
fn spine374_d_frame_loop_initializes_before_aui_advances_once_and_defers_events() {
    let f = Fixture::new("d-frame");
    f.authored();
    let mut r = Runtime::new(&f);
    let mut s = Session::new();
    s.aui.push_back(vec![Intent::Play {
        name: "idle".into(),
    }]);
    r.tick(&mut s, true, 1).unwrap();
    assert_eq!(s.seen.len(), 2);
    assert!(s.seen[0].0);
    assert_eq!(s.seen[0].1.pose_revision, 0);
    assert!(s.seen.iter().all(|(_, s)| s.events.is_empty()));
    assert_eq!(
        r.frame_loop
            .spine_module()
            .unwrap()
            .snapshot("robot")
            .unwrap()
            .pose_revision,
        1
    );
    // A zero-step render frame can deliver the prior tick; it cannot evaluate a pose again.
    r.tick(&mut s, true, 0).unwrap();
    let seen = &s.seen[2].1;
    assert_eq!(seen.events.len(), 1);
    assert_eq!(seen.events[0].payload.name, "zero");
    assert_eq!(seen.events[0].tick, 1);
    assert_eq!(
        r.frame_loop
            .spine_module()
            .unwrap()
            .snapshot("robot")
            .unwrap()
            .pose_revision,
        1
    );
    r.tick(&mut s, false, 15).unwrap();
    let module = r.frame_loop.spine_module().unwrap();
    assert_eq!(module.tick_index(), Some(16));
    assert_eq!(module.snapshot("robot").unwrap().pose_revision, 16);
    r.tick(&mut s, false, 1).unwrap();
    let beats: Vec<_> = s
        .seen
        .iter()
        .flat_map(|(_, s)| &s.events)
        .filter(|e| e.payload.name == "beat")
        .collect();
    assert_eq!(beats.len(), 1);
    assert_eq!(beats[0].tick, 16);
    assert!(module_observation_off(&r));
    assert_eq!(
        r.frame_loop
            .spine_module()
            .unwrap()
            .observe(engine_runtime::render_contract::DiagnosticLevel::Summary)
            .unwrap()
            .instances
            .len(),
        0
    );
    assert_eq!(
        r.frame_loop
            .spine_module()
            .unwrap()
            .observe(engine_runtime::render_contract::DiagnosticLevel::Trace)
            .unwrap()
            .instances
            .len(),
        1
    );
}
fn module_observation_off(r: &Runtime) -> bool {
    r.frame_loop
        .spine_module()
        .unwrap()
        .observe(engine_runtime::render_contract::DiagnosticLevel::Off)
        .is_none()
}

#[test]
fn spine374_d_fixed_clock_mismatch_is_rejected_before_aui_callback() {
    let f = Fixture::new("d-clock");
    f.authored();
    let mut r = Runtime::new(&f);
    let mut s = Session::new();
    r.frame_loop
        .runtime_time_mut()
        .set_fixed_delta_time(1. / 30.);
    let fault = r.tick(&mut s, true, 1).unwrap_err();
    assert_eq!(fault.report.status, "animator.runtime.tick_rate_mismatch");
    assert!(s.seen.is_empty());
}

#[test]
fn spine374_d_noop_invalid_batch_and_generation_do_not_partially_commit() {
    for case in [
        "noop",
        "unknown-action",
        "stale",
        "unknown-skin",
        "wrong-parameter",
        "bad-world",
    ] {
        let f = Fixture::new(case);
        f.authored();
        let mut r = Runtime::new(&f);
        let mut s = Session::new();
        s.write = true;
        let intent = match case {
            "unknown-action" => Intent::Play {
                name: "absent".into(),
            },
            "unknown-skin" => Intent::SetSkin {
                name: "absent".into(),
            },
            "wrong-parameter" => Intent::SetBool {
                name: "fire".into(),
                value: true,
            },
            _ => Intent::Play { name: "run".into() },
        };
        s.aui.push_back(vec![
            Intent::SetBool {
                name: "moving".into(),
                value: true,
            },
            intent,
        ]);
        s.no_op = case == "noop";
        s.stale = case == "stale";
        s.bad_world = case == "bad-world";
        let result = r.tick(&mut s, true, if case == "noop" { 0 } else { 1 });
        if case == "noop" {
            assert!(result.is_ok());
        } else {
            assert!(result.unwrap_err().report.terminal_fault);
        }
        assert_eq!(
            r.world.transform(&"robot".into()).unwrap().local_position.x,
            0.,
            "{case}"
        );
        let module = r.frame_loop.spine_module().unwrap();
        assert_eq!(module.tick_index(), None);
        assert_eq!(
            module
                .observe(engine_runtime::render_contract::DiagnosticLevel::Summary)
                .unwrap()
                .queued_command_count,
            0
        );
    }
}

#[test]
fn spine374_d_valid_commands_pause_resume_skin_sockets_and_retirement() {
    let f = Fixture::new("d-commands");
    f.authored();
    let mut r = Runtime::new(&f);
    let mut s = Session::new();
    s.aui.push_back(vec![
        Intent::SetBool {
            name: "moving".into(),
            value: true,
        },
        Intent::SetSkin {
            name: "default".into(),
        },
        Intent::Play {
            name: "idle".into(),
        },
        Intent::Restart { name: "run".into() },
    ]);
    r.tick(&mut s, true, 1).unwrap();
    let m = r.frame_loop.spine_module().unwrap();
    let hand = m.snapshot("robot").unwrap().sockets["hand"].clone();
    assert!(hand.position[0] > 1.0);
    assert_eq!(hand.tick, 1);
    s.fixed.push_back(vec![Intent::SetPaused { paused: true }]);
    r.tick(&mut s, false, 2).unwrap();
    assert_eq!(
        r.frame_loop
            .spine_module()
            .unwrap()
            .snapshot("robot")
            .unwrap()
            .sockets["hand"]
            .position,
        hand.position
    );
    s.fixed.push_back(vec![
        Intent::SetPaused { paused: false },
        Intent::Resume {},
        Intent::SetTrigger {
            name: "fire".into(),
        },
    ]);
    r.tick(&mut s, false, 1).unwrap();
    s.fixed.push_back(vec![
        Intent::FadeOutLayer {
            layer: "upper".into(),
            duration_ticks: 2,
        },
        Intent::ClearLayer {
            layer: "upper".into(),
        },
    ]);
    r.tick(&mut s, false, 1).unwrap();
    assert!(
        r.frame_loop
            .spine_module()
            .unwrap()
            .geometry("robot")
            .unwrap()
            .indices
            .len()
            > 0
    );
    r.world.try_despawn_entity(&"robot".into()).unwrap();
    let mut module = SpineModule::load(&r.package, &r.world).unwrap();
    assert!(module.callback_snapshots(&r.world).unwrap().is_empty());
    // An old handle cannot be reused even if the authored source name is respawned.
    let old = hand.entity_generation;
    r.world
        .try_spawn_entity(
            "robot".into(),
            "Robot",
            "entity",
            true,
            engine_runtime::components::Hierarchy {
                parent_id: None,
                sibling_order: 0,
            },
        )
        .unwrap();
    let new = r.world.entity_generation(&"robot".into()).unwrap();
    assert_ne!(old, new);
    let snapshots = module.callback_snapshots(&r.world).unwrap();
    assert_eq!(snapshots["robot"].entity_generation, new);
    assert!(snapshots["robot"].events.is_empty());
    assert!(module
        .preflight(
            &r.world,
            6,
            &[AnimatorCommand {
                entity_id: "robot".into(),
                generation: old,
                intent: Intent::Resume {}
            }]
        )
        .is_err());
}
