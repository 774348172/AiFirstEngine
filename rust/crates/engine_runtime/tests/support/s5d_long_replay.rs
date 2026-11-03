//! Original published simulation package, with its real project DLL and host.
//! This deliberately excludes GPU/OS-display and frame-time qualification.
use engine_runtime::{
    archetype::ComponentValue,
    component_value::RuntimeValue,
    components::ComponentTypeId,
    engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode},
    frame_loop::RuntimeFrameContext,
    ids::EntityId,
    project_observation::ProjectRuntimeObservationState,
    project_runtime_module::ProjectRuntimeBootstrap,
    project_runtime_native_adapter::linked_project_runtime_set_from_dll,
    project_runtime_session::*,
    runtime_instance_loader::RuntimeInstanceLoader,
    runtime_package::load_runtime_package,
    scene_loader::load_scene_into_world,
    world::World,
    world_api::WorldReadApi,
};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::{Arc, Mutex},
    time::Instant,
};

#[derive(Default)]
struct EventLog {
    ticks: u64,
    events: BTreeMap<u64, Vec<(String, String)>>,
}

fn collect_events(world: WorldReadApi<'_>, actors: &[EntityId], tick: u64, log: &mut EventLog) {
    if actors.is_empty() {
        return;
    }
    assert_eq!(
        tick,
        log.ticks + 1,
        "every committed event tick must be observed once"
    );
    for id in actors {
        let ComponentValue::Dynamic {
            value: RuntimeValue::Object(fields),
            ..
        } = world
            .read_component(id, &ComponentTypeId::new("engine.character3d_state"))
            .unwrap()
        else {
            panic!("missing actual character event projection");
        };
        assert_eq!(fields.get("tick"), Some(&RuntimeValue::I64(tick as i64)));
        let Some(RuntimeValue::Array(events)) = fields.get("events") else {
            panic!("missing committed event array");
        };
        if !events.is_empty() {
            log.events
                .entry(tick)
                .or_default()
                .push((id.as_str().into(), format!("{events:?}")));
        }
    }
    log.ticks = tick;
}

// Observe the preceding committed event tick before each real FixedUpdate.
// A presentation-only observer would miss intermediate ticks at30Hz.
struct EventTap {
    inner: Box<dyn ProjectRuntimeSession>,
    actors: Vec<EntityId>,
    log: Arc<Mutex<EventLog>>,
}
impl ProjectRuntimeSession for EventTap {
    fn session_id(&self) -> &str {
        self.inner.session_id()
    }
    fn accept_animator_snapshots(
        &mut self,
        snapshots: engine_runtime::spine_animation::session::CallbackSnapshots,
    ) {
        self.inner.accept_animator_snapshots(snapshots);
    }
    fn observe_memory(&self, sample: &mut engine_runtime::resource_memory::ResourceMemorySample) {
        self.inner.observe_memory(sample);
    }
    fn mount_ui_documents(
        &mut self,
        documents: &engine_runtime::runtime_package::RuntimeAuiDocumentRegistry,
    ) -> Result<(), ProjectRuntimeSessionFactoryError> {
        self.inner.mount_ui_documents(documents)
    }
    fn retained_ui(&self) -> Option<&engine_runtime::aui_session::AuiSession> {
        self.inner.retained_ui()
    }
    fn publish_ui_geometry(&mut self, output: &engine_runtime::aui::AuiRuntimePresentOutput) {
        self.inner.publish_ui_geometry(output);
    }
    fn advance_ui_animations(
        &mut self,
        frame: u64,
        delta_us: u64,
    ) -> Result<(), project_runtime_sdk::aui::UiError> {
        self.inner.advance_ui_animations(frame, delta_us)
    }
    fn cancel_drag_animations(
        &mut self,
        document: &str,
        interaction: &engine_runtime::aui::AuiInteractionResult,
    ) {
        self.inner.cancel_drag_animations(document, interaction);
    }
    fn aui_extension_stats(&self) -> ProjectRuntimeAuiExtensionStats {
        self.inner.aui_extension_stats()
    }
    fn refresh_ui_extensions_for_frame(
        &mut self,
        frame: &engine_runtime::aui_extension::AuiCustomFrame,
        allow_publish: bool,
    ) -> Result<engine_runtime::aui_extension::AuiCustomFrameResult, String> {
        self.inner
            .refresh_ui_extensions_for_frame(frame, allow_publish)
    }
    fn finish_ui_updates(&mut self, accepted: bool) {
        self.inner.finish_ui_updates(accepted);
    }
    fn accept_spawn_receipts(&mut self, receipts: BTreeMap<u64, (EntityId, u64)>) {
        self.inner.accept_spawn_receipts(receipts);
    }
    fn lifecycle(
        &mut self,
        context: ProjectRuntimeSessionContext<'_>,
        event: ProjectRuntimeLifecycleEvent,
    ) -> ProjectRuntimeSessionOutput {
        self.inner.lifecycle(context, event)
    }
    fn handle_aui_actions(
        &mut self,
        context: ProjectRuntimeSessionContext<'_>,
        batch: ProjectAuiActionBatch<'_>,
    ) -> ProjectRuntimeSessionOutput {
        self.inner.handle_aui_actions(context, batch)
    }
    fn fixed_update(
        &mut self,
        context: ProjectRuntimeSessionContext<'_>,
    ) -> ProjectRuntimeSessionOutput {
        if context.time.fixed_frame_count > 1 {
            collect_events(
                context.world,
                &self.actors,
                context.time.fixed_frame_count - 1,
                &mut self.log.lock().unwrap(),
            );
        }
        self.inner.fixed_update(context)
    }
    fn observe(
        &self,
        context: ProjectRuntimeObservationContext<'_>,
    ) -> ProjectRuntimeObservationOutput {
        self.inner.observe(context)
    }
}

fn snapshot(world: &World, characters: &[EntityId], agents: &[EntityId]) -> Value {
    let characters: Vec<_> = characters
        .iter()
        .map(|id| {
            let pose = world
                .character_pose(id)
                .expect("actual committed character");
            json!({
                "id": id.as_str(), "tick": pose.tick,
                "owner": world.transform3d(id), "mode": format!("{:?}", pose.mode),
                "state": pose.state, "joints": pose.current.joints,
                "morphWeights": pose.current.morph_weights,
                "events": format!("{:?}", pose.events),
                "jointCount": pose.joints.len(),
                "skinVertices": pose.skin_sources.iter().map(|s| s.vertex_count).collect::<Vec<_>>()
            })
        })
        .collect();
    let agents: Vec<_> = agents
        .iter()
        .map(|id| {
            json!({"id": id.as_str(), "owner": world.transform3d(id),
                "navigation": world.navigation3d_state(id)})
        })
        .collect();
    json!({"physics": &*world.physics3d_frame(), "characters": characters, "agents": agents})
}

fn preserve(dir: &Path, name: &str, value: &impl serde::Serialize) {
    std::fs::write(dir.join(name), serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

#[test]
#[ignore = "explicit original BM05/06/07 delivery; three full7200tick CPU host replays"]
fn s5d_original_simulation_package_replays_7200_ticks_at_30_60_120_hz() {
    replay(&[60, 30, 120], 7200, None);
}

#[test]
#[ignore = "bounded real BM07 regression for presentation-only event sampling"]
fn s5d_bm07_original_scenario_at_30_hz() {
    replay(&[30], 1200, Some("platform-3d-bm07"));
}

fn replay(rates: &[u64], tick_limit: u64, required_project: Option<&str>) {
    let delivery = std::path::PathBuf::from(
        std::env::var_os("AIFE_S5_REPLAY_DELIVERY").expect("original Compiler delivery"),
    );
    let evidence = std::path::PathBuf::from(
        std::env::var_os("AIFE_S5_REPLAY_EVIDENCE").expect("fresh role-owned evidence"),
    );
    std::fs::create_dir(&evidence).expect("fresh evidence identity");
    let package = load_runtime_package(delivery.join("data/runtime_package"))
        .value
        .expect("original runtime package");
    let project = &package.manifest.project.project_id;
    assert!(matches!(
        project.as_str(),
        "platform-3d-bm05" | "platform-3d-bm06" | "platform-3d-bm07"
    ));
    if let Some(required) = required_project {
        assert_eq!(project, required);
    }
    let scenario_path = std::path::PathBuf::from(
        std::env::var_os("AIFE_S5_REPLAY_SCENARIO").expect("original project scenario"),
    );
    let scenario: Value = serde_json::from_slice(&std::fs::read(&scenario_path).unwrap()).unwrap();
    let assertions = scenario["assertions"]
        .as_array()
        .expect("scenario assertions");
    assert!(!assertions.is_empty());
    let mut assertion_ids = BTreeSet::new();
    for assertion in assertions {
        let id = assertion["assertionId"].as_str().unwrap();
        assert!(
            assertion_ids.insert(id),
            "duplicate scenario assertion {id}"
        );
        assert!(assertion["path"].is_string() && assertion.get("equals").is_some());
        let from = assertion["fromSimulationTick"].as_u64().unwrap();
        let through = assertion["throughSimulationTick"].as_u64().unwrap();
        assert!(from <= through && through <= tick_limit);
    }
    preserve(&evidence, "scenario-input.json", &scenario);
    let module = std::fs::read_dir(delivery.join("data/bin"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|ext| ext == "dll"))
        .expect("published project DLL; missing composition must not be rebuilt here");
    let linked = linked_project_runtime_set_from_dll(&module).unwrap();
    let entities_with = |component: &str| -> Vec<EntityId> {
        package
            .active_scene
            .entities
            .iter()
            .filter(|entity| {
                entity
                    .components
                    .iter()
                    .any(|c| c.component_type == component)
            })
            .map(|entity| EntityId::from(entity.id.clone()))
            .collect()
    };
    let characters = entities_with("engine.character3d");
    let agents = entities_with("engine.navigation_agent3d");
    let dynamics: Vec<_> = package
        .active_scene
        .entities
        .iter()
        .filter(|entity| entity.id.starts_with("dynamic-"))
        .collect();
    match project.as_str() {
        "platform-3d-bm05" => assert_eq!(characters.len(), 100),
        "platform-3d-bm06" => assert_eq!(dynamics.len(), 1000),
        "platform-3d-bm07" => assert_eq!(agents.len(), 1000),
        _ => unreachable!(),
    }
    let mut reference = BTreeMap::<u64, Value>::new();
    let mut reference_events = None;
    let mut rows = Vec::new();
    // Start with60Hz as the unchanged original consumer. Each frequency gets a
    // fresh native session, clocks, World, Jolt/Detour and loader.
    for (run_index, &rate) in rates.iter().enumerate() {
        let parts = ProjectRuntimeBootstrap::bind(&package, &linked)
            .unwrap()
            .into_parts();
        let events = Arc::new(Mutex::new(EventLog::default()));
        let session = EventTap {
            inner: parts.project_runtime_session,
            actors: characters.clone(),
            log: events.clone(),
        };
        let mut host = EngineHostLoop::with_project_runtime_session(
            package.active_scene.id.clone(),
            parts.project_logic,
            Box::new(session),
        );
        host.set_project_runtime_session_report_level(ProjectRuntimeSessionReportLevel::Summary);
        let loaded = load_scene_into_world(&package.active_scene);
        let mut world = loaded
            .value
            .unwrap_or_else(|| panic!("{:?}", loaded.diagnostics));
        let mut loader = RuntimeInstanceLoader::from_package(&package);
        assert!(loader.asset_loader_mut().mount_bundle("startup"));
        let started = Instant::now();
        let mut previous_tick = 0;
        let mut zero_tick_frames = 0;
        let mut multiple_tick_frames = 0;
        let mut checkpoints = 0;
        let mut passed_assertions = BTreeSet::new();
        for frame in 1..=rate * (tick_limit / 60 + 1) {
            let old_physics = world.physics3d_frame();
            let old_poses: Vec<_> = characters
                .iter()
                .map(|id| world.character_pose(id))
                .collect();
            let output = host.tick_with_runtime_context(
                EngineFrameInput::new(EngineHostMode::ExportedGame)
                    .with_unscaled_delta_time(1. / rate as f32)
                    .with_deferred_render_submission(),
                &mut world,
                RuntimeFrameContext::new(&package, &mut loader),
            );
            assert!(
                output.render_submission_report.is_none() && !output.render_built,
                "CPU replay must not submit or present a second GPU workload"
            );
            let geometry_error = host
                .retained_geometry_snapshot()
                .and_then(|snapshot| snapshot.preparation_error());
            if output.host_control.is_some()
                || matches!(
                    host.project_observation_state(),
                    Some(ProjectRuntimeObservationState::ContractViolated { .. })
                )
                || output
                    .project_runtime_session_report
                    .as_ref()
                    .is_some_and(|r| r.terminal_fault)
                || geometry_error.as_ref().is_some_and(|error| {
                    error.code
                        != engine_runtime::render_contract::ContractErrorCode::ResourceNotReady
                })
            {
                preserve(
                    &evidence,
                    &format!("{rate}-first-runtime-failure.json"),
                    &json!({"frame": frame, "previousTick": previous_tick,
                        "report": output.project_runtime_session_report,
                        "observation": output.project_observation_state,
                        "hostControl": format!("{:?}", output.host_control),
                        "geometryError": format!("{geometry_error:?}"),
                        "runtimeTrace": format!("{:?}", output.runtime_trace)}),
                );
                preserve(
                    &evidence,
                    &format!("{rate}-first-runtime-failure-state.json"),
                    &snapshot(&world, &characters, &agents),
                );
                panic!("original{project} {rate}Hz runtime fault at frame{frame}");
            }
            let tick = world.physics3d_frame().tick;
            for assertion in assertions {
                let from = assertion["fromSimulationTick"].as_u64().unwrap();
                let through = assertion["throughSimulationTick"].as_u64().unwrap();
                if !(from..=through).contains(&tick) {
                    continue;
                }
                let id = assertion["assertionId"].as_str().unwrap();
                let path = assertion["path"].as_str().unwrap();
                let actual = match host.project_observation_state() {
                    Some(ProjectRuntimeObservationState::Published { snapshot }) => snapshot
                        .values
                        .get(path)
                        .map(|v| serde_json::to_value(v).unwrap()),
                    _ => None,
                };
                if actual.as_ref() != assertion.get("equals") {
                    preserve(
                        &evidence,
                        &format!("{rate}-first-business-failure.json"),
                        &json!({"project": project, "rate": rate, "frame": frame,
                            "tick": tick, "assertion": assertion, "actual": actual,
                            "observation": host.project_observation_state()}),
                    );
                    panic!("original{project} {rate}Hz tick{tick} assertion{id}: expected{} actual{actual:?}", assertion["equals"]);
                }
                passed_assertions.insert(id);
            }
            assert!(
                tick >= previous_tick && tick <= previous_tick + 2,
                "rate={rate} frame={frame} tick={tick} previous={previous_tick}"
            );
            if tick == previous_tick {
                zero_tick_frames += 1;
                assert!(
                    Arc::ptr_eq(&old_physics, &world.physics3d_frame()),
                    "zero-tick presentation changed committed physics"
                );
                for (id, old) in characters.iter().zip(&old_poses) {
                    if let Some(old) = old {
                        assert!(
                            Arc::ptr_eq(old, &world.character_pose(id).unwrap()),
                            "zero-tick presentation advanced animation for{}",
                            id.as_str()
                        );
                    }
                }
            } else if tick - previous_tick == 2 {
                multiple_tick_frames += 1;
            }
            if tick != previous_tick && tick.is_multiple_of(60) {
                let state = snapshot(&world, &characters, &agents);
                if run_index == 0 {
                    reference.insert(tick, state);
                } else if reference.get(&tick) != Some(&state) {
                    preserve(
                        &evidence,
                        &format!("{rate}-{tick}-reference.json"),
                        &reference[&tick],
                    );
                    preserve(&evidence, &format!("{rate}-{tick}-actual.json"), &state);
                    panic!("original{project} committed state diverged at tick{tick}/{rate}Hz; first snapshots preserved");
                }
                checkpoints += 1;
                if tick.is_multiple_of(600) {
                    eprintln!(
                        "original{project} {rate}Hz frame={frame} tick={tick}/{tick_limit} elapsed={:.3}s",
                        started.elapsed().as_secs_f64()
                    );
                }
            }
            previous_tick = tick;
            assert!(
                started.elapsed().as_secs() < 1200,
                "one frequency exceeded1200s exploration bound"
            );
            if tick == tick_limit {
                assert_eq!(checkpoints, tick_limit / 60);
                assert_eq!(
                    passed_assertions, assertion_ids,
                    "every original business assertion must be exercised"
                );
                assert!(
                    host.retained_geometry_snapshot()
                        .is_some_and(|snapshot| snapshot.preparation_error().is_none()),
                    "the original CPU presentation snapshot must finish preparation"
                );
                let mut log = events.lock().unwrap();
                collect_events(WorldReadApi::new(&world), &characters, tick, &mut log);
                if !characters.is_empty() {
                    assert_eq!(log.ticks, tick_limit);
                    assert!(
                        !log.events.is_empty(),
                        "original animation must produce gameplay events"
                    );
                }
                let actual_events = json!(log.events);
                if run_index == 0 {
                    reference_events = Some(actual_events.clone());
                } else if reference_events.as_ref() != Some(&actual_events) {
                    preserve(
                        &evidence,
                        &format!("{rate}-events-reference.json"),
                        &reference_events,
                    );
                    preserve(
                        &evidence,
                        &format!("{rate}-events-actual.json"),
                        &actual_events,
                    );
                    panic!("original{project} committed animation events diverged at{rate}Hz; complete tick event logs preserved");
                }
                let row = json!({"project": project, "rate": rate, "frames": frame,
                    "ticks": tick, "checkpoints": checkpoints, "zeroTickFrames": zero_tick_frames,
                    "multipleTickFrames": multiple_tick_frames, "elapsedSeconds": started.elapsed().as_secs_f64(),
                    "eventTicksObserved":log.ticks, "nonemptyEventTicks":log.events.len(),
                    "passedBusinessAssertions": passed_assertions,
                    "finalObservation": host.project_observation_state()});
                preserve(&evidence, &format!("{rate}-completed.json"), &row);
                rows.push(row);
                break;
            }
        }
        assert_eq!(
            previous_tick, tick_limit,
            "frequency{rate} did not complete the required simulation"
        );
        if rate == 120 {
            assert!(zero_tick_frames >= tick_limit - 1);
        }
        if rate == 30 {
            assert_eq!(multiple_tick_frames, tick_limit / 2);
        }
    }
    preserve(
        &evidence,
        "completed.json",
        &json!({"project": project, "runs": rows, "tickLimit": tick_limit,
        "scenarioSource": scenario_path, "businessAssertionCount": assertions.len(),
        "scope": "Original package and project DLL with current CPU host and original scenario business assertions. GPU, OS display, release timings and formal repetitions are not qualified."}),
    );
}
