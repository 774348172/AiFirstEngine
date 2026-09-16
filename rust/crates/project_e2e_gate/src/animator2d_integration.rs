use editor_core::{
    EditorRuntimePlayInstance, EditorRuntimePlayRequest, ProjectRuntimePackageAssembler,
    ProjectRuntimePackageAssemblyRequest, ProjectRuntimePackageAssemblyStatus,
    EDITOR_RUNTIME_PLAY_REQUEST_SCHEMA_VERSION,
};
use engine_runtime::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
use engine_runtime::frame_loop::RuntimeFrameContext;
use engine_runtime::ids::EntityId;
use engine_runtime::project_logic::ProjectLogicRunner;
use engine_runtime::project_runtime_session::{
    ProjectAuiActionBatch, ProjectRuntimeMutationBuffer, ProjectRuntimeSession,
    ProjectRuntimeSessionContext, ProjectRuntimeSessionOutput, ProjectRuntimeSessionReportLevel,
};
use engine_runtime::runtime_package::load_runtime_package;
use engine_runtime::runtime_package_builder::{
    RuntimePackageBuildRequest, RuntimePackageBuildStatus, RuntimePackageBuilder,
};
use engine_runtime::runtime_scene_hydration::RuntimeSceneHydrator;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

struct TriggerOnSecondTickSession {
    session_id: String,
    entity_id: EntityId,
    trigger_id: String,
    ticks: u64,
}

impl ProjectRuntimeSession for TriggerOnSecondTickSession {
    fn session_id(&self) -> &str {
        &self.session_id
    }

    fn handle_aui_actions(
        &mut self,
        _context: ProjectRuntimeSessionContext<'_>,
        batch: ProjectAuiActionBatch<'_>,
    ) -> ProjectRuntimeSessionOutput {
        let mut output = ProjectRuntimeSessionOutput::no_op();
        output.unhandled_action_count = batch.len();
        output
    }

    fn fixed_update(
        &mut self,
        _context: ProjectRuntimeSessionContext<'_>,
    ) -> ProjectRuntimeSessionOutput {
        self.ticks += 1;
        if self.ticks != 2 {
            return ProjectRuntimeSessionOutput::no_op();
        }
        let mut mutations = ProjectRuntimeMutationBuffer::new();
        mutations.animator2d_set_trigger(self.entity_id.clone(), self.trigger_id.clone());
        ProjectRuntimeSessionOutput::applied(mutations)
    }
}

#[derive(Clone, Copy)]
struct FixtureCase {
    name: &'static str,
    source_project: &'static str,
    target_entity: &'static str,
    controller_id: &'static str,
    idle_state: &'static str,
    active_state: &'static str,
    trigger_id: &'static str,
    idle_sprite: &'static str,
    active_sprite: &'static str,
}

#[test]
fn animator2d_generic_and_second_project_use_the_same_cooked_fixed_tick_projection_chain() {
    let cases = [
        FixtureCase {
            name: "generic-shooter",
            source_project: "complex_shooter_project",
            target_entity: "entity-player",
            controller_id: "generic-motion-controller",
            idle_state: "resting",
            active_state: "bursting",
            trigger_id: "begin-burst",
            idle_sprite: "generic-rest-frame",
            active_sprite: "generic-burst-frame",
        },
        FixtureCase {
            name: "second-switch",
            source_project: "switch_puzzle_project",
            target_entity: "entity-puzzle-switch",
            controller_id: "switch-ink-controller",
            idle_state: "sealed",
            active_state: "released",
            trigger_id: "release-seal",
            idle_sprite: "switch-sealed-frame",
            active_sprite: "switch-released-frame",
        },
    ];

    for case in cases {
        qualify_case(case);
    }
}

fn qualify_case(case: FixtureCase) {
    let root = temp_root(case.name);
    let project_root = root.join("project");
    copy_tree(
        &workspace_root().join("samples").join(case.source_project),
        &project_root,
    )
    .unwrap();
    install_fixture(&project_root, case);

    let assembly = ProjectRuntimePackageAssembler::assemble(
        ProjectRuntimePackageAssemblyRequest::new(&project_root),
    );
    assert_eq!(
        assembly.status,
        ProjectRuntimePackageAssemblyStatus::Success,
        "{} assembly diagnostics: {:?}",
        case.name,
        assembly.report.diagnostics
    );
    let input = assembly.build_input.expect("assembly build input");
    assert_eq!(
        input.animator2d_registry.controllers[0].id,
        case.controller_id
    );
    let registry_digest = input.animator2d_registry.registry_digest.clone();
    let package_dir = root.join("runtime-package");
    let build = RuntimePackageBuilder::build(
        &RuntimePackageBuildRequest::dev_desktop(
            &package_dir,
            assembly
                .active_scene_id
                .unwrap_or_else(|| "scene-main".to_string()),
        ),
        &input,
    );
    assert_eq!(
        build.status,
        RuntimePackageBuildStatus::Success,
        "{} build diagnostics: {:?}",
        case.name,
        build.diagnostics
    );
    let load = load_runtime_package(&package_dir);
    let package = load.value.unwrap_or_else(|| {
        panic!(
            "{} load diagnostics: {:?}",
            case.name, load.diagnostics.issues
        )
    });
    assert_eq!(package.animator2d_registry.registry_digest, registry_digest);

    let linked = if case.source_project == "complex_shooter_project" {
        crate::complex_shooter_linked_set()
    } else {
        crate::switch_puzzle_linked_set()
    };
    let mut editor_play = EditorRuntimePlayInstance::start_with_linked_modules(
        EditorRuntimePlayRequest {
            schema_version: EDITOR_RUNTIME_PLAY_REQUEST_SCHEMA_VERSION.to_string(),
            session_id: format!("animator2d-observation-{}", case.name),
            project_root: project_root.clone(),
            runtime_package_path: package_dir.clone(),
            scene_ref: Some(package.active_scene.id.clone()),
            run_profile: Some("animator2d-gate-f".to_string()),
            frame_limit: 1,
            requested_by: "Animator2DGateF".to_string(),
            preview_package_report_path: None,
        },
        &linked,
    );
    let mut editor_instance = editor_play.instance.take().expect("Editor Play instance");
    editor_instance
        .set_project_runtime_session_report_level(ProjectRuntimeSessionReportLevel::Trace);
    let trace_report = editor_instance.tick_next_descriptor_frame();
    let observation = trace_report
        .last_frame
        .as_ref()
        .and_then(|frame| {
            frame
                .animator2d_play_observations
                .iter()
                .find(|observation| observation.entity_id == case.target_entity)
        })
        .expect("Editor GameView Animator2D observation");
    assert!(observation.read_only);
    assert_eq!(observation.state_id, case.idle_state);
    assert_eq!(observation.frame_index, 0);
    drop(editor_instance);

    let mut world = engine_runtime::world::World::new();
    let mut hydrator = RuntimeSceneHydrator::from_package(&package);
    let hydration = hydrator.hydrate_active_scene(&package, &mut world);
    assert!(!hydration.has_errors(), "{} hydration failed", case.name);
    let target = EntityId::from(case.target_entity);
    assert_eq!(
        world
            .animator2d(&target)
            .map(|value| value.controller_id.as_str()),
        Some(case.controller_id)
    );

    let session = TriggerOnSecondTickSession {
        session_id: format!("animator2d.{}", case.name),
        entity_id: target.clone(),
        trigger_id: case.trigger_id.to_string(),
        ticks: 0,
    };
    let mut host = EngineHostLoop::with_project_runtime_session(
        package.active_scene.id.clone(),
        ProjectLogicRunner::empty(),
        Box::new(session),
    );
    let mut sequence = Vec::new();
    let mut states = Vec::new();
    for _ in 0..3 {
        let output = host.tick_with_runtime_context(
            EngineFrameInput::new(EngineHostMode::EditorPlay),
            &mut world,
            RuntimeFrameContext {
                package: &package,
                instance_loader: hydrator.instance_loader_mut(),
            },
        );
        sequence.push(
            world
                .sprite_renderer2d(&target)
                .and_then(|renderer| renderer.sprite_ref.clone())
                .expect("animated sprite ref"),
        );
        states.push(
            host.animator2d_module()
                .and_then(|module| module.entity_state(&target))
                .expect("Animator2D state")
                .state_id,
        );
        assert!(output.render_frame_report.is_some());
        assert!(host
            .render_scene()
            .proxy_for_source(&engine_runtime::ids::SourceEntityId::from(
                case.target_entity
            ))
            .is_some());
    }

    assert_eq!(
        sequence,
        vec![
            case.idle_sprite.to_string(),
            case.active_sprite.to_string(),
            case.active_sprite.to_string(),
        ]
    );
    assert_eq!(
        states,
        vec![
            case.idle_state.to_string(),
            case.active_state.to_string(),
            case.active_state.to_string(),
        ]
    );
}

fn install_fixture(project_root: &Path, case: FixtureCase) {
    let source_image = if case.source_project == "complex_shooter_project" {
        "Assets/Images/tex-player-ship.png"
    } else {
        "Assets/Images/app-icon.png"
    };
    for sprite_id in [case.idle_sprite, case.active_sprite] {
        fs::write(
            project_root
                .join("Assets")
                .join(format!("{sprite_id}.asset")),
            serde_json::to_vec_pretty(&json!({
                "schemaVersion": "texture-asset.v1",
                "assetId": sprite_id,
                "sourceImage": source_image,
                "importer": { "format": "png", "colorSpace": "srgb", "sampler": "linearClamp" }
            }))
            .unwrap(),
        )
        .unwrap();
    }

    let animation_root = project_root.join("Animations");
    fs::create_dir_all(&animation_root).unwrap();
    for (clip_id, sprite_id, playback) in [
        (format!("{}-idle-clip", case.name), case.idle_sprite, "loop"),
        (
            format!("{}-active-clip", case.name),
            case.active_sprite,
            "once",
        ),
    ] {
        fs::write(
            animation_root.join(format!("{clip_id}.sprite-animation-clip-2d.json")),
            serde_json::to_vec_pretty(&json!({
                "schema": "sprite-animation-clip-2d.v1",
                "assetId": clip_id,
                "playback": playback,
                "frames": [{ "spriteRef": sprite_id, "durationTicks": 2 }]
            }))
            .unwrap(),
        )
        .unwrap();
    }
    let idle_clip = format!("{}-idle-clip", case.name);
    let active_clip = format!("{}-active-clip", case.name);
    fs::write(
        animation_root.join(format!(
            "{}.animator-controller-2d.json",
            case.controller_id
        )),
        serde_json::to_vec_pretty(&json!({
            "schema": "animator-controller-2d.v1",
            "assetId": case.controller_id,
            "parameters": [{ "id": case.trigger_id, "kind": "trigger" }],
            "entryStateId": case.idle_state,
            "states": [
                { "id": case.idle_state, "clipRef": idle_clip, "speedPermille": 1000 },
                { "id": case.active_state, "clipRef": active_clip, "speedPermille": 1000 }
            ],
            "transitions": [{
                "id": format!("{}-transition", case.name),
                "from": case.idle_state,
                "to": case.active_state,
                "when": "immediate",
                "priority": 100,
                "conditions": [{ "parameter": case.trigger_id, "op": "triggered" }]
            }]
        }))
        .unwrap(),
    )
    .unwrap();

    let scene_path = project_root.join("Scenes/main.scene.json");
    let mut scene: Value = serde_json::from_slice(&fs::read(&scene_path).unwrap()).unwrap();
    let entity = scene["entities"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|entity| entity["id"] == case.target_entity)
        .expect("fixture target entity");
    let components = entity["components"].as_array_mut().unwrap();
    if !components
        .iter()
        .any(|component| component["componentType"] == "SpriteRenderer2D")
    {
        components.push(json!({
            "componentType": "SpriteRenderer2D",
            "data": { "spriteRef": { "id": case.idle_sprite, "type": "texture" }, "visible": true }
        }));
    }
    components.push(json!({
        "componentType": "Animator2D",
        "data": { "controllerRef": case.controller_id, "enabled": true, "initialBools": {} }
    }));
    fs::write(scene_path, serde_json::to_vec_pretty(&scene).unwrap()).unwrap();
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn temp_root(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("animator2d-e2e-{name}-{stamp}"))
}

fn copy_tree(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let target = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
