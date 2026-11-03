use super::*;
use serde_json::json;

fn capture_pixels(output: &std::path::Path) -> serde_json::Value {
    let mut samples = vec![];
    for (frame, channel) in [(35, 0), (80, 2), (125, 1), (170, 1)] {
        let image = image::open(output.join(format!("capture-{frame}.png")))
            .expect("required real-window capture")
            .to_rgba8();
        let mut count = 0u64;
        let mut sum_x = 0u64;
        let mut min_x = image.width();
        let mut max_x = 0;
        for (x, _, pixel) in image.enumerate_pixels() {
            if pixel[channel] > 128 && (0..3).filter(|c| *c != channel).all(|c| pixel[c] < 64) {
                count += 1;
                sum_x += u64::from(x);
                min_x = min_x.min(x);
                max_x = max_x.max(x);
            }
        }
        assert!(count > 1000, "frame {frame}: expected channel {channel}, found {count} colored pixels; black or wrong-revision capture");
        samples.push(json!({"frame":frame,"channel":channel,"count":count,"centroidX":sum_x as f64/count as f64,"minX":min_x,"maxX":max_x}));
    }
    assert!(
        samples[1]["minX"].as_u64().unwrap() > samples[0]["maxX"].as_u64().unwrap(),
        "replacement must remain visible outside the original bounds"
    );
    assert!(
        samples[1]["count"].as_u64().unwrap() * 2 > samples[0]["count"].as_u64().unwrap() * 3,
        "quad must cover more pixels than initial triangle"
    );
    assert_eq!(
        samples[1]["count"], samples[2]["count"],
        "color patch preserves geometry"
    );
    assert_eq!(samples[1]["minX"], samples[2]["minX"]);
    assert_eq!(samples[1]["maxX"], samples[2]["maxX"]);
    assert_eq!(
        samples[2]["count"], samples[3]["count"],
        "position patch preserves topology and colors"
    );
    assert!(
        samples[3]["maxX"].as_u64().unwrap() < samples[0]["minX"].as_u64().unwrap(),
        "position patch must render outside both prior bounds"
    );
    json!(samples)
}

#[test]
#[ignore = "review retained real-window captures without rebuilding or launching Player"]
fn generated_content_376_capture_pixel_oracle() {
    let output = PathBuf::from(std::env::var_os("AIFE_376_CAPTURE_REVIEW").unwrap());
    capture_pixels(&output);
}

fn actual_dll_lifetime(delivery: &std::path::Path) -> serde_json::Value {
    use engine_runtime::project_runtime_module::ProjectRuntimeBootstrap;
    use engine_runtime::project_runtime_native_adapter::linked_project_runtime_set_from_dll;
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    extern "system" {
        fn GetModuleHandleW(name: *const u16) -> *mut std::ffi::c_void;
        fn GetProcAddress(module: *mut std::ffi::c_void, name: *const u8) -> *mut std::ffi::c_void;
    }
    let dll = delivery.join("data/bin/376_generated_runtime.dll");
    let wide: Vec<_> = dll.as_os_str().encode_wide().chain(Some(0)).collect();
    let module = || unsafe { GetModuleHandleW(wide.as_ptr()) };
    assert!(module().is_null(), "must begin without a retained module");
    let linked = linked_project_runtime_set_from_dll(&dll).unwrap();
    let handle = module();
    assert!(!handle.is_null());
    let counts = unsafe {
        let symbol = GetProcAddress(handle, b"generated_content_376_worker_counts\0".as_ptr());
        assert!(
            !symbol.is_null(),
            "real project DLL must export the test probe"
        );
        std::mem::transmute::<*mut std::ffi::c_void, extern "C" fn() -> u64>(symbol)
    };
    let mut package = engine_runtime::runtime_package::load_runtime_package(
        delivery.join("data/runtime_package"),
    )
    .value
    .unwrap();
    let requested = package.manifest.project.runtime_module.clone();
    for (bad, expected) in [
        ("interface", "project_runtime.interface_version_mismatch"),
        ("digest", "project_runtime.aot_digest_mismatch"),
    ] {
        package.manifest.project.runtime_module = requested.clone();
        if bad == "interface" {
            package.manifest.project.runtime_module.interface_version =
                "project-runtime-module.v1".into();
        } else {
            package.manifest.project.runtime_module.aot_content_digest = "sha256:wrong".into();
        }
        let error = ProjectRuntimeBootstrap::bind(&package, &linked)
            .err()
            .expect("stale contract rejects before create_session");
        assert_eq!(error.code, expected);
        assert_eq!(counts(), 0);
    }
    package.manifest.project.runtime_module = requested;
    let parts = ProjectRuntimeBootstrap::bind(&package, &linked)
        .unwrap()
        .into_parts();
    assert_eq!(counts(), 1u64 << 32);
    drop(parts.project_runtime_session);
    assert_eq!(
        counts(),
        1u64 << 32,
        "UI producer still leases the same native session"
    );
    drop(parts.ui_state_producer);
    assert_eq!(
        counts(),
        (1u64 << 32) | 1,
        "destroy_session joined the real project thread"
    );
    drop(parts.project_logic);
    assert!(
        !module().is_null(),
        "linked registration still holds the DLL"
    );
    drop(linked);
    assert!(
        module().is_null(),
        "last library guard unloads only after session destruction"
    );
    json!({"realDll":dll,"oldInterfaceEarlyRejected":true,"digestEarlyRejected":true,"workerStarted":1,"workerJoined":1,"unloadedAfterDestroy":true})
}

fn first_presentations(output: &std::path::Path) -> serde_json::Value {
    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output.join("result.json")).unwrap()).unwrap();
    let stderr = result["process"]["stderrSummary"].as_str().unwrap();
    assert!(
        stderr.contains("376-WORKER joined-before-destroy-return"),
        "actual Player destroy_session must join the worker: {stderr}"
    );
    let player: serde_json::Value =
        serde_json::from_slice(&fs::read(output.join("player.json")).unwrap()).unwrap();
    let captures = player["semanticPlaytest"]["captureEvidence"]
        .as_array()
        .unwrap();
    let mut events = vec![];
    for (request, channel, range) in [(1, 0, 1..=6), (2, 2, 44..=49)] {
        let prefix = format!("376-APPLIED request={request} ");
        let event = stderr
            .lines()
            .find(|line| line.starts_with(&prefix))
            .expect("source simulation event");
        let tick: u64 = event
            .split("simulation_tick=")
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        let mut first = None;
        for frame in range.clone() {
            let image = image::open(output.join(format!("capture-{frame}.png")))
                .unwrap()
                .to_rgba8();
            let count = image
                .pixels()
                .filter(|pixel| {
                    pixel[channel] > 128 && (0..3).filter(|c| *c != channel).all(|c| pixel[c] < 64)
                })
                .count();
            if count > 1000 {
                first = Some(frame);
                break;
            }
        }
        let frame = first.expect("new pixels must appear inside the captured transition interval");
        assert!(
            frame > *range.start(),
            "must capture the immediately preceding old frame"
        );
        assert!(
            frame >= tick,
            "simulation observation and new pixels must use compatible frame identities"
        );
        let capture = captures
            .iter()
            .find(|capture| capture["presentationFrame"] == frame)
            .unwrap();
        let previous = captures
            .iter()
            .find(|capture| capture["presentationFrame"] == frame - 1)
            .unwrap();
        assert_eq!(capture["simulationTick"], frame);
        assert_ne!(capture["sha256"], previous["sha256"]);
        events.push(json!({"request":request,"appliedSimulationTick":tick,"firstNewPresentationFrame":frame,"capture":capture,"precedingCapture":previous,"sourceEvent":event}));
    }
    json!({"events":events,"sourceEvents":stderr.lines().filter(|line| line.starts_with("376-")).collect::<Vec<_>>()})
}

// One ordinary project; the Compiler owns all package/glue/native output.
#[test]
#[ignore = "bounded Windows Compiler, real DLL, OS window and restart consumer"]
fn generated_content_376_compiler_player_and_restart() {
    let root = PathBuf::from(
        std::env::var_os("AIFE_376_INTEGRATION_ROOT").expect("fresh run-owned integration root"),
    );
    assert!(root.is_absolute());
    fs::create_dir(&root).expect("integration root must be fresh");
    let project = root.join("Project");
    for directory in [
        "RuntimeModule/src",
        "Scenes",
        "Assets",
        "Input",
        "Observations",
        "BuildProfiles",
        "Tests",
    ] {
        fs::create_dir_all(project.join(directory)).unwrap();
    }
    let write = |path: &str, value: serde_json::Value| {
        fs::write(
            project.join(path),
            serde_json::to_vec_pretty(&value).unwrap(),
        )
        .unwrap()
    };
    write(
        "project.aife.json",
        json!({
            "schemaVersion":"aife-project.v3","projectId":"376.ordinary-project","projectName":"376 integration",
            "engineVersion":"0.1.0","createdAt":"2026-10-30T00:00:00Z","lastOpenedAt":null,"assetRoot":"Assets","settingsVersion":"aife-project-settings.v1",
            "defaultScene":"Scenes/main.json","observationContract":"Observations/contract.json",
            "runtimeModule":{"sourceKind":"projectRust","moduleId":"376.generated.runtime","interfaceVersion":"project-runtime-module.v2","cargoManifest":"RuntimeModule/Cargo.toml","cargoPackage":"generated_content_376_runtime","playerBinary":"generated_content_376_player","projectGameSdk":"project-game-sdk.v1"}
        }),
    );
    let sdk = project_authoring_execution::default_engine_sdk_root()
        .canonicalize()
        .unwrap();
    let sdk_path = sdk
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .replace('\\', "/");
    fs::write(project.join("RuntimeModule/Cargo.toml"),format!("[package]\nname='generated_content_376_runtime'\nversion='0.1.0'\nedition='2021'\npublish=false\n[dependencies]\nproject_game_sdk={{path='{sdk_path}/crates/project_game_sdk'}}\n")).unwrap();
    fs::write(
        project.join("RuntimeModule/src/lib.rs"),
        include_str!("../fixtures/generated_content_376_project.rs.txt"),
    )
    .unwrap();
    write(
        "BuildProfiles/windows.dev.json",
        json!({"schemaVersion":"build-profile.v1","profile":"dev","target":"windows","runtimePackageMode":"debug-readable","frameLimit":3,"headlessSurfaceGate":true,"realWindowSmoke":"optional"}),
    );
    write(
        "Assets/material.json",
        json!({"schemaVersion":"material-asset.v2","assetId":"surface-material","assetGuid":"376-material","displayName":"Vertex colors","preset":"unlit","baseColor":[1,1,1,1],"blend":"opaque","cull":"none","depth":"testAndWrite"}),
    );
    let transform = |position: [f64; 3]| json!({"componentType":"Transform3D","data":{"schemaVersion":"transform3d.v1","translation":position,"rotation":{"kind":"quaternion","xyzw":[0,0,0,1]},"scale":[1,1,1]}});
    let mut scene: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../samples/platform_3d_shadows_project/Scenes/main.json"
    ))
    .unwrap();
    let mut camera = scene["entities"][1].clone();
    camera["components"][1]["data"]["projection"]["size"] = json!(5.0);
    // Empty camera selection uses the existing all-layer convention. A newly
    // created generated renderer has no project-authored layer membership.
    camera["components"][1]["data"]["layers"] = json!([]);
    scene["entities"] = json!([
        {"id":"surface","name":"Generated surface","enabled":true,"parentId":null,"siblingOrder":0,"components":[transform([0.,0.,-6.]),{"componentType":"Visibility3D","data":{"schemaVersion":"visibility3d.v1","visible":true}}]},camera
    ]);
    scene["name"] = json!("Generated content consumer");
    write("Scenes/main.json", scene);
    write(
        "Input/default.json",
        json!({"schema_version":"input-mapping.v2","asset_id":"input.none","actions":[],"contexts":[],"bindings":[]}),
    );
    let mut observations = vec![];
    for path in [
        "stage",
        "applied",
        "collision",
        "epoch",
        "tick",
        "appliedTick",
        "movedTick",
        "rayChecks",
    ] {
        observations
            .push(json!({"path":format!("content.{path}"),"type":"integer","description":path}));
    }
    for path in ["persisted", "reloaded"] {
        observations
            .push(json!({"path":format!("content.{path}"),"type":"bool","description":path}));
    }
    write(
        "Observations/contract.json",
        json!({"schemaVersion":"project-observation-contract.v1","contractId":"generated.ordinary","observations":observations}),
    );
    let mut session = project_authoring_execution::ProjectAuthoringSession::open(&project).unwrap();
    let lease = full_lease(&mut session, "376-ordinary");
    let compiler = GameProjectCompiler::bind(&lease).unwrap();
    let started = std::time::Instant::now();
    compiler
        .check(
            &lease,
            CheckProfile::new(TargetProfile::WindowsDev)
                .with_process_approval(true)
                .with_timeout_ms(180_000),
        )
        .expect("project SDK source must compile before native build");
    let checked_ms = started.elapsed().as_millis();
    let prepared = compiler.prepare(&lease, TargetProfile::WindowsDev).unwrap();
    let prepared_ms = started.elapsed().as_millis() - checked_ms;
    fs::write(root.join("prepare.json"),serde_json::to_vec_pretty(&json!({"checkMs":checked_ms,"prepareMs":prepared_ms,"summary":prepared.prepare_summary(),"sourceIdentity":prepared.lineage().source_identity(),"revisionId":prepared.lineage().revision_id(),"glue":prepared.generated_runtime_glue_report()})).unwrap()).unwrap();
    let cache = PathBuf::from(
        std::env::var_os("AIFE_376_COMPILER_CACHE").expect("376 isolated compiler cache"),
    );
    let build_started = std::time::Instant::now();
    let built = compiler
        .build(
            &prepared,
            BuildRequest::for_project(
                TargetProfile::WindowsDev,
                &project,
                project_authoring_execution::ProjectRelativePath::parse("Build/Windows").unwrap(),
                cache,
            )
            .with_player_verification(false)
            .with_timeout_ms(1_200_000),
        )
        .unwrap();
    let export_ms = build_started.elapsed().as_millis();
    let delivery = built.delivery().package_dir();
    fs::write(
        root.join("delivery.json"),
        serde_json::to_vec_pretty(
            &json!({"delivery":delivery,"exportMs":export_ms,"report":built.desktop_export()}),
        )
        .unwrap(),
    )
    .unwrap();
    run_delivery(&root, delivery);
}

#[test]
#[ignore = "resume retained ordinary Compiler delivery after a scenario-only repair"]
fn generated_content_376_retained_delivery_player_and_restart() {
    let root = PathBuf::from(std::env::var_os("AIFE_376_INTEGRATION_ROOT").unwrap());
    assert!(root.is_absolute());
    fs::create_dir(&root).expect("fresh retained-delivery run root");
    let delivery = PathBuf::from(std::env::var_os("AIFE_376_RETAINED_DELIVERY").unwrap());
    fs::write(root.join("reused-delivery.json"), serde_json::to_vec_pretty(&json!({"delivery":delivery,"reason":"scenario-only repair; published binaries remain unchanged"})).unwrap()).unwrap();
    run_delivery(&root, &delivery);
}

fn run_delivery(root: &std::path::Path, delivery: &std::path::Path) {
    let exe = delivery.join("Game.exe");
    fs::write(
        root.join("dll-lifetime.json"),
        serde_json::to_vec_pretty(&actual_dll_lifetime(delivery)).unwrap(),
    )
    .unwrap();
    let package = delivery.join("data/runtime_package");
    // Override only the child environment. The installed user's LOCALAPPDATA is untouched.
    let local_data = root.join("IsolatedLocalAppData");
    fs::create_dir(&local_data).unwrap();
    for (index, reloaded) in [(1, false), (2, true)] {
        let scenario = json!({"schemaVersion":"playtest-scenario.v2","scenarioId":format!("376-window-restart-{index}"),"initialSceneId":"main","target":"windows-windowed","maxSimulationTicks":300,"maxPresentationFrames":500,"timeoutMs":90000,"inputs":[],
            "assertions":[
                {"assertionId":"complete","fromSimulationTick":300,"throughSimulationTick":300,"path":"content.stage","equals":12},
                {"assertionId":"persisted","fromSimulationTick":300,"throughSimulationTick":300,"path":"content.persisted","equals":true},
                {"assertionId":"reloaded","fromSimulationTick":300,"throughSimulationTick":300,"path":"content.reloaded","equals":reloaded},
                {"assertionId":"applied","fromSimulationTick":300,"throughSimulationTick":300,"path":"content.applied","equals":5},
                {"assertionId":"collision","fromSimulationTick":300,"throughSimulationTick":300,"path":"content.collision","equals":5},
                {"assertionId":"rays","fromSimulationTick":300,"throughSimulationTick":300,"path":"content.rayChecks","equals":6}
            ],"captures":[{"captureId":"initial","presentationFrame":35,"required":true,"subjectiveReview":false},{"captureId":"moved","presentationFrame":80,"required":true,"subjectiveReview":false},{"captureId":"patched","presentationFrame":125,"required":true,"subjectiveReview":false},{"captureId":"position-patched","presentationFrame":170,"required":true,"subjectiveReview":false}]
        });
        let mut scenario = scenario;
        for frame in (1..=6).chain(44..=49) {
            scenario["captures"].as_array_mut().unwrap().push(json!({"captureId":format!("boundary-{frame}"),"presentationFrame":frame,"required":true,"subjectiveReview":false}));
        }
        scenario["captures"]
            .as_array_mut()
            .unwrap()
            .sort_by_key(|capture| capture["presentationFrame"].as_u64().unwrap());
        let scenario_path = root.join(format!("scenario-{index}.json"));
        fs::write(
            &scenario_path,
            serde_json::to_vec_pretty(&scenario).unwrap(),
        )
        .unwrap();
        let output = root.join(format!("player-{index}"));
        let result =
            runtime_cli::run_bounded_child_process(runtime_cli::BoundedChildProcessRequest {
                executable: exe.clone(),
                args: vec![
                    "playtest".into(),
                    "--package".into(),
                    package.clone().into_os_string(),
                    "--scenario".into(),
                    scenario_path.into_os_string(),
                    "--output-dir".into(),
                    output.into_os_string(),
                ],
                current_dir: delivery.into(),
                environment: vec![("LOCALAPPDATA".into(), local_data.clone().into_os_string())],
                timeout: std::time::Duration::from_secs(120),
                stdout_capture_limit_bytes: 1024 * 1024,
                stderr_capture_limit_bytes: 1024 * 1024,
                priority: runtime_cli::BoundedChildProcessPriority::Normal,
            });
        fs::write(
            root.join(format!("player-{index}-process.json")),
            serde_json::to_vec_pretty(&result).unwrap(),
        )
        .unwrap();
        assert_eq!(result.exit_code, Some(0), "Player {index}: {result:?}");
        let output = root.join(format!("player-{index}"));
        fs::write(
            root.join(format!("player-{index}-pixels.json")),
            serde_json::to_vec_pretty(&capture_pixels(&output)).unwrap(),
        )
        .unwrap();
        fs::write(
            root.join(format!("player-{index}-presentations.json")),
            serde_json::to_vec_pretty(&first_presentations(&output)).unwrap(),
        )
        .unwrap();
    }
}
