use super::*;
use engine_runtime::canonical_digest::sha256_prefixed;
use project_authoring_execution::{
    BuildRequest, ExecutionMode, ProjectRelativePath, VerifyRequest,
};
use std::time::Instant;

#[path = "gate_f_memory.rs"]
#[cfg(feature = "real-wgpu")]
mod memory;

#[path = "gate_f_baseline.rs"]
mod baseline;
#[path = "gate_f_budget.rs"]
mod budget;
#[path = "gate_f_release_budget.rs"]
mod release_budget;

fn measured_process(
    delivery: &std::path::Path,
    root: &std::path::Path,
    policy: runtime_player_winit::measurement::MeasurementPolicy,
) -> (Value, u32) {
    fs::create_dir_all(root).unwrap();
    let report = runtime_cli::verify_exported_player_process_with_options(
        runtime_cli::ExportedPlayerProcessVerificationRequest {
            exported_package_dir: delivery.to_path_buf(),
            mode: "windowed".into(),
            frame_limit: 4200,
            report_path: Some(root.join("process.json")),
            timeout_ms: 300_000,
            screenshot: false,
            screenshot_path: None,
        },
        runtime_cli::ExportedPlayerProcessVerificationOptions {
            measurement: Some(policy),
            runtime_report_level: Some("off".into()),
            performance_warmup_frames: 600,
            performance_sample_frames: 3600,
            ..Default::default()
        },
    );
    fs::write(
        root.join("process.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    assert_eq!(report.process_exit_code, Some(0), "{report:#?}");
    assert_eq!(report.child_frames_completed, Some(4200), "{report:#?}");
    let child: Value =
        serde_json::from_slice(&fs::read(&report.child_report_path).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(&report).unwrap()["status"],
        "passed",
        "{report:#?}"
    );
    fs::write(
        root.join("requested-policy.json"),
        serde_json::to_vec_pretty(&policy).unwrap(),
    )
    .unwrap();
    (
        child["framePerformanceSummary"].clone(),
        report.process_id.unwrap(),
    )
}

fn delivery_files(root: &std::path::Path) -> std::collections::BTreeMap<String, String> {
    fn visit(
        base: &std::path::Path,
        dir: &std::path::Path,
        files: &mut std::collections::BTreeMap<String, String>,
    ) {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            assert!(
                !kind.is_symlink(),
                "delivery must not depend on a linked source"
            );
            if kind.is_dir() {
                visit(base, &entry.path(), files);
            } else {
                files.insert(
                    entry
                        .path()
                        .strip_prefix(base)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                    sha256_prefixed(&fs::read(entry.path()).unwrap()),
                );
            }
        }
    }
    let mut files = std::collections::BTreeMap::new();
    visit(root, root, &mut files);
    files
}

fn uninstrumented_delivery(delivery: &std::path::Path, fixture: &Fixture) {
    let identities = delivery_files(delivery);
    let copy = fixture.root.join("final-standalone");
    fs::create_dir_all(&copy).unwrap();
    for name in identities.keys() {
        let out = copy.join(name);
        fs::create_dir_all(out.parent().unwrap()).unwrap();
        fs::copy(delivery.join(name), out).unwrap();
    }
    assert_eq!(delivery_files(&copy), identities);
    runtime_cli::validate_desktop_dev_package(&copy).unwrap();
    assert!(!copy.join("RuntimeModule").exists() && !copy.join("Scenes").exists());
    // The entire authored project and original export are now unavailable at
    // their recorded paths. Only this fixture's source is renamed.
    fs::rename(&fixture.source, fixture.root.join("source-offline")).unwrap();
    let report = runtime_cli::verify_exported_player_process_with_options(
        runtime_cli::ExportedPlayerProcessVerificationRequest {
            exported_package_dir: copy.clone(),
            mode: "windowed".into(),
            frame_limit: 4200,
            report_path: Some(fixture.root.join("final-uninstrumented-process.json")),
            timeout_ms: 300_000,
            screenshot: false,
            screenshot_path: None,
        },
        runtime_cli::ExportedPlayerProcessVerificationOptions {
            measurement: Some(Default::default()),
            runtime_report_level: Some("off".into()),
            performance_warmup_frames: 0,
            performance_sample_frames: 0,
            ..Default::default()
        },
    );
    fs::write(
        fixture.root.join("final-delivery.json"),
        serde_json::to_vec_pretty(&json!({
            "files":identities,"delivery":copy,"sourceUnavailable":!fixture.source.exists(),
            "measurement":"off; cpuProbe false; zero sample frames","verification":report
        }))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(&report).unwrap()["status"],
        "passed",
        "{report:#?}"
    );
    let child: Value =
        serde_json::from_slice(&fs::read(&report.child_report_path).unwrap()).unwrap();
    assert!(child["framePerformanceSummary"].is_null());
    // Verification may add its own reports; every delivered payload stays exact.
    for (name, digest) in identities {
        assert_eq!(sha256_prefixed(&fs::read(copy.join(name)).unwrap()), digest);
    }
}

const CONTRACT: &str =
    include_str!("../../../engine_spine_runtime/contracts/qualification-recipes.v1.json");
fn skeleton(id: &str) -> &'static str {
    match id {
        "single" => include_str!("../../../engine_spine_runtime/tests/fixtures/recipe-single.json"),
        "regular" => {
            include_str!("../../../engine_spine_runtime/tests/fixtures/recipe-regular.json")
        }
        "clipping" => {
            include_str!("../../../engine_spine_runtime/tests/fixtures/recipe-clipping.json")
        }
        _ => panic!("unknown frozen recipe"),
    }
}
fn project(recipe: &Value) -> Fixture {
    let id = recipe["id"].as_str().unwrap();
    let f = Fixture::new(&format!("f-recipe-{id}"));
    fs::write(f.source.join("Assets/hero.skeleton.json"), skeleton(id)).unwrap();
    let layers = recipe["activeLayersPerActor"].as_u64().unwrap();
    let actors = recipe["actors"].as_u64().unwrap();
    let mut animations = serde_json::Map::new();
    for layer in 0..layers {
        for clip in 0..4 {
            animations.insert(format!("layer{layer}-clip{clip}"), json!({"clip":format!("clip{clip}"),
                "layer":format!("layer{layer}"),"playback":"loop","speedPermille":1000,"mixInSeconds":0.5}));
        }
    }
    let mut description = json!({
        "schema":"animator-description-2d.v2","backend":"spine",
        "skeleton":{"assetId":"hero"},"default":"layer0-clip0","parameters":{},
        "layers":(0..layers).map(|i| json!({"name":format!("layer{i}"),"blend":"replace"})).collect::<Vec<_>>(),
        "masks":{},"animations":animations,"rules":[],"sockets":{}});
    // Use the existing author binding route; cooked digest/index fields belong
    // to the assembler and are not handwritten into author scene data.
    for actor in 0..actors {
        description["assetId"] = json!(format!("recipe-controller-{actor:02}"));
        description["entity"] = json!(format!("actor-{actor}"));
        f.write(
            &format!("Animations/recipe-{actor:02}.animator-description-2d.json"),
            &description,
        );
    }
    let transform = |x: f64, y: f64| {
        json!({"localPosition":{"x":x,"y":y,"z":0},
        "localRotation":{"x":0,"y":0,"z":0},"localScale":{"x":1,"y":1,"z":1}})
    };
    let mut entities = vec![
        json!({"id":"camera","name":"camera","kind":"camera","enabled":true,
        "transform":transform(0.,0.),"components":[{"componentType":"project.camera2d","data":{"orthographicSize":4}}]}),
        json!({"id":"recipe","name":"recipe","transform":transform(0.,0.),"components":[{
            "componentType":"project.spine374Recipe","data":{"actors":actors,"layers":layers,
            "maxChain":recipe["maxMixingEntriesPerLayer"]}}]}),
    ];
    for actor in 0..actors {
        entities.push(json!({"id":format!("actor-{actor}"),"name":format!("actor-{actor}"),"enabled":true,
            "transform":transform((actor%8) as f64*0.4-1.4,(actor/8) as f64*0.4-1.4),"components":[]}));
    }
    f.write(
        "Scenes/main.scene.json",
        &json!({"schemaVersion":"scene.v1","id":"main","name":id,"entities":entities}),
    );
    let sdk = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../project_game_sdk")
        .canonicalize()
        .unwrap();
    fs::create_dir_all(f.source.join("RuntimeModule/src")).unwrap();
    fs::write(f.source.join("RuntimeModule/Cargo.toml"), format!(
        "[package]\nname = \"spine374_recipe_runtime\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[lib]\npath = \"src/lib.rs\"\n[dependencies]\nproject_game_sdk = {{ path = {:?} }}\n", sdk.to_string_lossy().replace('\\',"/"))).unwrap();
    fs::write(
        f.source.join("RuntimeModule/src/lib.rs"),
        include_str!("gate_f_runtime.rs"),
    )
    .unwrap();
    f.write("project.aife.json", &json!({"schemaVersion":"aife-project.v2","projectId":"spine374-f-recipes",
        "projectName":"374 frozen qualification recipes","engineVersion":"0.1.0",
        "assetRoot":"Assets","settingsVersion":"aife-project-settings.v1",
        "createdAt":"2026-10-24T01:52:11Z","lastOpenedAt":null,
        "defaultScene":"Scenes/main.scene.json","runtimeModule":{"sourceKind":"projectRust","moduleId":"sample.spine374.recipes",
        "interfaceVersion":"project-runtime-module.v2","cargoManifest":"RuntimeModule/Cargo.toml","cargoPackage":"spine374_recipe_runtime",
        "playerBinary":"spine374_recipe_player","projectGameSdk":"project-game-sdk.v1"}}));
    f.write("BuildProfiles/windows.release.json", &json!({
        "schemaVersion":"build-profile.v2","profile":"release","target":"windows",
        "runtimePackageMode":"debug-readable","frameLimit":4200,"headlessSurfaceGate":true,
        "realWindowSmoke":"optional","gameViewTarget":{"extent":{"width":1280,"height":720},
        "scalePolicy":"contain"},"architecture":"x86_64",
        "release":{"layout":"compiler-delivery-v1","includeReports":true,"includeDebugSymbols":false}}));
    let manifest: project_authoring_execution::ProjectManifest =
        serde_json::from_slice(&fs::read(f.source.join("project.aife.json")).unwrap()).unwrap();
    manifest.runtime_module.validate().unwrap();
    f
}

fn prepare_input(f: &Fixture, target: TargetProfile) -> RuntimePackageBuildInput {
    let mut session = ProjectAuthoringSession::open(&f.source).unwrap();
    let paths = session
        .source_inventory()
        .unwrap()
        .entries
        .into_iter()
        .map(|e| e.relative_path)
        .collect();
    let lease = session
        .acquire_snapshot_lease("spine374-f-input", paths)
        .unwrap();
    let compiler = GameProjectCompiler::bind(&lease).unwrap();
    // This is the cheap Cook/load consumer. Project Rust compilation belongs to
    // the explicit Release build, as in the existing Gate E sample consumer.
    compiler
        .prepare_with_artifact_cache(&lease, target, &f.root.join("cache"))
        .unwrap()
        .runtime_package_build_input()
        .clone()
}

#[cfg(feature = "real-wgpu")]
#[test]
fn spine374_f_offscreen_simulation_and_world_replacement_preserve_frozen_packets() {
    use engine_runtime::project_runtime_session::*;
    use engine_runtime::{
        engine_host_loop::*, frame_loop::RuntimeFrameContext, render_contract::DiagnosticLevel,
    };
    use std::sync::{Arc, Mutex};
    struct Capture(Arc<Mutex<Vec<project_runtime_sdk::animator::AnimatorSnapshot>>>);
    impl ProjectRuntimeSession for Capture {
        fn session_id(&self) -> &str {
            "spine374-f-offscreen"
        }
        fn accept_animator_snapshots(
            &mut self,
            snapshots: engine_runtime::spine_animation::session::CallbackSnapshots,
        ) {
            if let Some(snapshot) = snapshots.get("robot") {
                self.0.lock().unwrap().push(snapshot.clone());
            }
        }
        fn fixed_update(
            &mut self,
            _: ProjectRuntimeSessionContext<'_>,
        ) -> ProjectRuntimeSessionOutput {
            ProjectRuntimeSessionOutput::no_op()
        }
        fn handle_aui_actions(
            &mut self,
            _: ProjectRuntimeSessionContext<'_>,
            _: ProjectAuiActionBatch<'_>,
        ) -> ProjectRuntimeSessionOutput {
            ProjectRuntimeSessionOutput::no_op()
        }
    }
    let f = Fixture::new("f-offscreen-world");
    f.authored();
    let mut scene: Value =
        serde_json::from_slice(&fs::read(f.source.join("Scenes/main.scene.json")).unwrap())
            .unwrap();
    scene["entities"][0]["transform"]["localPosition"]["x"] = json!(10000);
    f.write("Scenes/main.scene.json", &scene);
    let root = f.package(&prepare_input(&f, TargetProfile::WindowsDev));
    let package = load_runtime_package(&root).value.unwrap();
    let mut world = engine_runtime::scene_loader::load_scene_into_world(&package.active_scene)
        .value
        .unwrap();
    let mut loader =
        engine_runtime::runtime_instance_loader::RuntimeInstanceLoader::from_package(&package);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mut host = EngineHostLoop::with_project_runtime_session(
        "main",
        engine_runtime::project_logic::ProjectLogicRunner::empty(),
        Box::new(Capture(seen.clone())),
    );
    let input = || {
        EngineFrameInput::new(EngineHostMode::ExportedGame)
            .with_unscaled_delta_time(1. / 60.)
            .with_fixed_step_count(1)
            .with_deferred_render_submission()
    };
    for _ in 0..16 {
        host.tick_with_runtime_context(
            input(),
            &mut world,
            RuntimeFrameContext::new(&package, &mut loader),
        );
    }
    let observed = host.observe_spine(DiagnosticLevel::Trace).unwrap();
    assert_eq!(observed.tick, Some(16));
    assert_eq!(observed.instances["robot"].pose_revision, 16);
    assert!(
        observed.queued_event_count > 0,
        "offscreen actors stopped producing content events"
    );
    assert!(seen
        .lock()
        .unwrap()
        .iter()
        .flat_map(|s| &s.events)
        .any(|e| e.payload.name == "zero"));
    let frozen = host.retained_geometry_snapshot().unwrap().clone();
    let (old_handle, bytes) = frozen.prepared_geometries().next().unwrap().upload_source();
    let original_bytes = bytes.vertex_bytes().to_vec();
    drop(bytes);
    let old_generation = world.entity_generation(&"robot".into());
    world = engine_runtime::scene_loader::load_scene_into_world(&package.active_scene)
        .value
        .unwrap();
    assert_eq!(world.entity_generation(&"robot".into()), old_generation);
    host.tick_with_runtime_context(
        input(),
        &mut world,
        RuntimeFrameContext::new(&package, &mut loader),
    );
    let observed = host.observe_spine(DiagnosticLevel::Trace).unwrap();
    assert_eq!(observed.instances["robot"].pose_revision, 1);
    assert_eq!(
        observed.queued_event_count, 1,
        "previous World's pending events crossed the boundary"
    );
    assert!(
        seen.lock().unwrap().last().unwrap().events.is_empty(),
        "new World callback received the old beat"
    );
    let (new_handle, _) = host
        .retained_geometry_snapshot()
        .unwrap()
        .prepared_geometries()
        .next()
        .unwrap()
        .upload_source();
    assert_ne!(old_handle, new_handle);
    world.try_despawn_entity(&"robot".into()).unwrap();
    host.tick_with_runtime_context(
        input(),
        &mut world,
        RuntimeFrameContext::new(&package, &mut loader),
    );
    assert_eq!(
        host.retained_geometry_snapshot()
            .unwrap()
            .prepared_geometries()
            .count(),
        0
    );
    assert_eq!(
        frozen
            .prepared_geometries()
            .next()
            .unwrap()
            .upload_source()
            .1
            .vertex_bytes(),
        original_bytes
    );
}

#[test]
fn spine374_f_recipe_projects_keep_frozen_load_and_shared_control_data() {
    use engine_runtime::spine_animation::control;
    let contract: Value = serde_json::from_str(CONTRACT).unwrap();
    let mut receipts = Vec::new();
    for recipe in contract["recipes"].as_array().unwrap() {
        let f = project(recipe);
        let input = prepare_input(&f, TargetProfile::WindowsRelease);
        let target = input.project.game_view_target.unwrap();
        assert_eq!((target.extent.width, target.extent.height), (1280, 720));
        let registry = &input.spine_animator.as_ref().unwrap().registry;
        assert_eq!(registry.assets.len(), 1);
        let actors = recipe["actors"].as_u64().unwrap() as usize;
        assert_eq!(registry.controllers.len(), actors);
        let bound = input.scenes[0]
            .entities
            .iter()
            .filter_map(|e| {
                engine_runtime::spine_animation::SpineAnimatorBinding::from_entity(e).unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(bound.len(), actors);
        assert!(bound
            .iter()
            .all(|b| b.registry_digest == registry.registry_digest));
        let root = f.package(&input);
        let package = load_runtime_package(&root).value.unwrap();
        let world = engine_runtime::scene_loader::load_scene_into_world(&package.active_scene)
            .value
            .unwrap();
        let module =
            engine_runtime::spine_animation::session::SpineModule::load(&package, &world).unwrap();
        let observed = module
            .observe(engine_runtime::render_contract::DiagnosticLevel::Summary)
            .unwrap();
        assert_eq!(observed.instance_count, actors);
        assert_eq!(
            observed.compiled_table_count, 1,
            "identical actor controllers duplicated immutable Spine data"
        );
        if actors == 64 {
            let mut alternative = package.clone();
            let altered = alternative.spine_animator_registry.as_mut().unwrap();
            altered.controllers[1].actions[0].requested_mix_in_seconds = 0.25;
            altered.controllers[1].actions[0].mix_in_ticks = 15;
            altered.seal().unwrap();
            for entity in &mut alternative.active_scene.entities {
                for component in &mut entity.components {
                    if component.component_type == "SpineAnimator" {
                        component.data["registryDigest"] = json!(altered.registry_digest);
                    }
                }
            }
            let changed =
                engine_runtime::spine_animation::session::SpineModule::load(&alternative, &world)
                    .unwrap();
            assert_eq!(
                changed
                    .observe(engine_runtime::render_contract::DiagnosticLevel::Summary)
                    .unwrap()
                    .compiled_table_count,
                2,
                "different transition definitions were merged into one table"
            );
        }
        let id = recipe["id"].as_str().unwrap();
        let raw: Value = serde_json::from_str(skeleton(id)).unwrap();
        assert_eq!(
            raw["bones"].as_array().unwrap().len() as u64,
            recipe["bonesPerActor"]
        );
        let attachments = raw["skins"][0]["attachments"]
            .as_object()
            .unwrap()
            .values()
            .flat_map(|slot| slot.as_object().unwrap().values())
            .collect::<Vec<_>>();
        let meshes = attachments
            .iter()
            .filter(|a| a["type"] == "mesh")
            .collect::<Vec<_>>();
        assert_eq!(meshes.len() as u64, recipe["meshSlotsPerActor"]);
        assert_eq!(
            attachments
                .iter()
                .filter(|a| a["type"] == "clipping")
                .count() as u64,
            recipe["clippingSlotsPerActor"]
        );
        assert_eq!(
            meshes
                .iter()
                .map(|a| a["uvs"].as_array().unwrap().len() / 2)
                .sum::<usize>() as u64,
            recipe["rawVerticesPerActor"]
        );
        assert_eq!(
            meshes
                .iter()
                .filter(|a| a["vertices"].as_array().unwrap().len()
                    > a["uvs"].as_array().unwrap().len())
                .count() as u64,
            recipe["weightedMeshSlotsPerActor"]
        );
        let checked = engine_spine_runtime::import::validate(
            skeleton(id),
            ATLAS,
            &std::collections::BTreeMap::from([("probe.png".into(), "texture-probe".into())]),
        )
        .unwrap();
        let c = &registry.controllers[0];
        let table = c
            .compile(&checked, 60, registry.assets[0].pixels_per_unit)
            .unwrap();
        let mut instances = (0..actors)
            .map(|_| table.instance().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(std::rc::Rc::strong_count(&table), actors + 1);
        let layers = recipe["activeLayersPerActor"].as_u64().unwrap() as usize;
        let chain = recipe["maxMixingEntriesPerLayer"].as_u64().unwrap() as usize;
        let switches = contract["interruptionTicksByMaxChain"][chain.to_string()]
            .as_array()
            .unwrap();
        let mut maxima = vec![0usize; layers];
        let mut event_count = 0usize;
        for tick in 1..=361 {
            let mut intents = Vec::new();
            if let Some(clip) = switches
                .iter()
                .position(|t| t.as_u64() == Some((tick - 1) % 120))
            {
                for layer in 0..layers {
                    let name = format!("layer{layer}-clip{clip}");
                    let index = c.actions.iter().position(|a| a.name == name).unwrap();
                    intents.push(control::Intent::Play(index as u16));
                }
            }
            for instance in &mut instances {
                event_count += instance.advance(tick, &intents).unwrap().events.len();
                for (layer, max) in maxima.iter_mut().enumerate() {
                    *max = (*max).max(instance.live_entry_count(layer));
                    assert!(*max <= chain);
                }
            }
        }
        assert_eq!(
            maxima,
            vec![chain; layers],
            "recipe never exercised its declared chain"
        );
        let geometry = instances[0].geometry().unwrap();
        assert!(!geometry.indices.is_empty());
        assert!(event_count > 0);
        receipts.push(
            json!({"recipe":id,"skeletonSha256":sha256_prefixed(skeleton(id).as_bytes()),
            "actors":actors,"compiledTables":1,"maximumLiveMixEntries":maxima,"events":event_count,
            "geometryVertices":geometry.vertices.len(),"geometryIndices":geometry.indices.len(),
            "qualification":"input-and-owner-only","project":f.source}),
        );
    }
    let parent = std::env::var_os("SPINE374_EVIDENCE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    fs::create_dir_all(&parent).unwrap();
    fs::write(
        parent.join("f-recipe-load-receipts.json"),
        serde_json::to_vec_pretty(&receipts).unwrap(),
    )
    .unwrap();
}

pub(super) fn release_recipes() {
    let contract: Value = serde_json::from_str(CONTRACT).unwrap();
    for recipe in contract["recipes"].as_array().unwrap() {
        let f = project(recipe);
        let mut session = ProjectAuthoringSession::open(&f.source).unwrap();
        let paths = session
            .source_inventory()
            .unwrap()
            .entries
            .into_iter()
            .map(|e| e.relative_path)
            .collect();
        let lease = session
            .acquire_snapshot_lease("spine374-f-release", paths)
            .unwrap();
        let compiler = GameProjectCompiler::bind(&lease).unwrap();
        let started = Instant::now();
        let prepared = compiler
            .prepare_with_artifact_cache(
                &lease,
                TargetProfile::WindowsRelease,
                &f.root.join("cache"),
            )
            .unwrap();
        let prepare_ms = started.elapsed().as_millis();
        let started = Instant::now();
        let built = compiler
            .build(
                &prepared,
                BuildRequest::for_project(
                    TargetProfile::WindowsRelease,
                    &f.source,
                    ProjectRelativePath::parse("Build/Playable").unwrap(),
                    project_authoring_execution::default_project_runtime_player_build_root(),
                )
                .with_frame_limit(4200)
                .with_player_verification(false)
                .with_timeout_ms(1_200_000),
            )
            .unwrap();
        let build_ms = started.elapsed().as_millis();
        fs::write(
            f.root.join("build.json"),
            serde_json::to_vec_pretty(built.desktop_export().unwrap()).unwrap(),
        )
        .unwrap();
        let delivery = built.delivery().package_dir().to_path_buf();
        let started = Instant::now();
        let composition = compiler
            .verify(
                built.delivery(),
                VerifyRequest::new(ExecutionMode::Windowed, 4200, 300_000, false),
            )
            .unwrap();
        fs::write(
            f.root.join("composition.json"),
            serde_json::to_vec_pretty(composition.process_verification()).unwrap(),
        )
        .unwrap();
        let composition_ms = started.elapsed().as_millis();
        assert_eq!(
            serde_json::to_value(composition.process_verification()).unwrap()["status"],
            "passed",
            "{composition:#?}"
        );
        // Publication is complete. Release preparation and source snapshots must
        // not compete with the Player for the qualification memory reserve.
        drop(built);
        drop(prepared);
        drop(compiler);
        drop(lease);
        drop(session);
        let mut process_ids = std::collections::BTreeSet::new();
        for repetition in 1..=3 {
            let started = Instant::now();
            let repeat_root = f.root.join(format!("repeat-{repetition}"));
            fs::create_dir_all(&repeat_root).unwrap();
            // The Compiler's existing verify delegates to this same original
            // process consumer. Use its already-supported measurement options;
            // gameplay diagnostics stay Off, with no ETW or screenshots.
            let report = runtime_cli::verify_exported_player_process_with_options(
                runtime_cli::ExportedPlayerProcessVerificationRequest {
                    exported_package_dir: delivery.clone(),
                    mode: "windowed".into(),
                    frame_limit: 4200,
                    report_path: Some(repeat_root.join("process.json")),
                    timeout_ms: 300_000,
                    screenshot: false,
                    screenshot_path: None,
                },
                runtime_cli::ExportedPlayerProcessVerificationOptions {
                    measurement: Some(runtime_player_winit::measurement::MeasurementPolicy {
                        level: runtime_player_winit::measurement::MeasurementLevel::Trace,
                        native_events: false,
                        first_frame: Some(1),
                        end_frame: Some(4201),
                        ..Default::default()
                    }),
                    runtime_report_level: Some("off".into()),
                    performance_warmup_frames: 600,
                    performance_sample_frames: 3600,
                    ..Default::default()
                },
            );
            assert_eq!(
                serde_json::to_value(&report).unwrap()["status"],
                "passed",
                "{report:#?}"
            );
            let child: Value =
                serde_json::from_slice(&fs::read(&report.child_report_path).unwrap()).unwrap();
            let summary = &child["framePerformanceSummary"];
            let cpu = summary["rawCpuFrames"].as_array();
            let gpu = summary["gpu"]["samples"].as_array();
            let mut missing = Vec::new();
            if cpu.is_none_or(|frames| frames.len() != 4200) {
                missing.push("4200 raw CPU samples including warmup");
            }
            if gpu.is_none_or(|frames| frames.len() != 3600) {
                missing.push("3600 raw GPU samples");
            }
            if cpu.is_none_or(|frames| {
                frames
                    .iter()
                    .filter(|frame| frame["sampled"] == true)
                    .any(|frame| !frame["spine"].is_object())
            }) {
                missing.push("Spine control / official apply+world / vertices+clipping / projection CPU stages");
            }
            if gpu.is_none_or(|frames| frames.iter().any(|frame| !frame["spineDrawMs"].is_number()))
            {
                missing.push("Spine pass GPU timestamps");
            }
            fs::write(f.root.join(format!("repeat-{repetition}.json")), serde_json::to_vec_pretty(&json!({
                "recipe":recipe,"contractSha256":sha256_prefixed(CONTRACT.as_bytes()),"profile":"WindowsRelease",
                "prepareMs":prepare_ms,"buildMs":build_ms,"compositionMs":composition_ms,
                "playerMs":started.elapsed().as_millis(),
                "runtimeReportLevel":"off","measurement":"bounded typed raw samples; no gameplay Trace, ETW or screenshots",
                "qualification":false,"missingEligibility":missing,
                "qualificationStagesFollowingThisEligibilityReceipt":["same-binary empty-scene private-memory baseline",
                    "CPU/GPU/upload/resident/system-memory budgets and memory-growth fit",
                    "three independent repetitions of all recipes","Summary/Trace cost comparison",
                    "final uninstrumented artifact identity"],
                "verification":report,"player":child})).unwrap()).unwrap();
            assert_eq!(report.child_frames_completed, Some(4200));
            assert_eq!(summary["observedSampleFrames"].as_u64(), Some(3600));
            assert!(missing.is_empty(), "spine374.f.measurement_ineligible: {missing:?}; stop before expensive repetitions, baseline and budget qualification");
            let cpu: Vec<engine_runtime::windowed_player::WindowedPlayerCpuFrameSample> =
                serde_json::from_value(summary["rawCpuFrames"].clone()).unwrap();
            let gpu: Vec<engine_runtime::windowed_player::WindowedPlayerGpuFrameSample> =
                serde_json::from_value(summary["gpu"]["samples"].clone()).unwrap();
            let observed = budget::evaluate(recipe, &cpu, &gpu, None);
            fs::write(
                repeat_root.join("observed-budget.json"),
                serde_json::to_vec_pretty(&observed).unwrap(),
            )
            .unwrap();
            assert!(
                observed.missing.iter().all(|m| m.contains("empty-scene")),
                "spine374.f.measurement_ineligible: {observed:#?}"
            );
            // Complete this first eligible recipe's mandatory counterfactual
            // even when an observed limit failed. It distinguishes feature
            // memory from common measurement/Player costs. The final verdict
            // still fails before later repetitions or recipes.
            let off_policy = runtime_player_winit::measurement::MeasurementPolicy {
                level: runtime_player_winit::measurement::MeasurementLevel::Off,
                cpu_probe: true,
                ..Default::default()
            };
            let (off, off_id) = measured_process(&delivery, &repeat_root.join("off"), off_policy);
            assert!(process_ids.insert(off_id));
            assert!(process_ids.insert(report.process_id.unwrap()));
            let empty = baseline::empty_scene(recipe, &delivery, &repeat_root);
            let verdict = release_budget::evaluate(recipe, &observed, &off, &empty);
            fs::write(
                repeat_root.join("budget.json"),
                serde_json::to_vec_pretty(&verdict).unwrap(),
            )
            .unwrap();
            assert!(verdict.passed, "spine374.f.budget_failed: {verdict:#?}");
            if repetition == 1 {
                let (summary, summary_id) = measured_process(
                    &delivery,
                    &repeat_root.join("summary"),
                    runtime_player_winit::measurement::MeasurementPolicy {
                        level: runtime_player_winit::measurement::MeasurementLevel::Summary,
                        ..Default::default()
                    },
                );
                assert!(process_ids.insert(summary_id));
                assert_eq!(summary["observedSampleFrames"], 3600);
                assert_eq!(summary["gpu"]["status"], "complete");
                assert!(summary["rawCpuFrames"]
                    .as_array()
                    .is_none_or(|a| a.is_empty()));
                let delta = |value: &Value| {
                    value["p95CpuFrameMs"].as_f64().unwrap()
                        - off["p95CpuFrameMs"].as_f64().unwrap()
                };
                fs::write(repeat_root.join("measurement-cost.json"),serde_json::to_vec_pretty(&json!({
                    "off":off,"summary":summary,"trace":child["framePerformanceSummary"],
                    "summaryCpuP95DeltaMs":delta(&summary),"traceCpuP95DeltaMs":delta(&child["framePerformanceSummary"]),
                    "offProcessId":off_id,"summaryProcessId":summary_id,"traceProcessId":report.process_id,
                    "scope":"same Release and workload; independent processes; costs reported separately, no frozen cost threshold"
                })).unwrap()).unwrap();
            }
        }
        uninstrumented_delivery(&delivery, &f);
    }
    // Eligibility alone is not the formal budget verdict. Keep this Gate red
    // until the remaining contract evaluation is actually implemented and run.
    panic!("spine374.f.final_qualification_pending: per-recipe budgets do not replace Off/Summary/Trace cost comparison, final uninstrumented delivery and hardware qualification");
}
