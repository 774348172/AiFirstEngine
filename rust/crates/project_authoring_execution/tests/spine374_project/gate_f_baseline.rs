//! Required same-binary counterfactual, using the existing bounded process owner.
use super::*;

pub(super) fn empty_scene(
    recipe: &Value,
    delivery: &std::path::Path,
    root: &std::path::Path,
) -> Value {
    let original = load_runtime_package(delivery.join("data/runtime_package"))
        .value
        .unwrap();
    let mut empty_recipe = recipe.clone();
    empty_recipe["actors"] = json!(0);
    let f = project(&empty_recipe);
    let mut input = prepare_input(&f, TargetProfile::WindowsRelease);
    assert_eq!(
        fs::read(f.source.join("RuntimeModule/src/lib.rs")).unwrap(),
        include_bytes!("gate_f_runtime.rs")
    );
    input.project.runtime_module = original.manifest.project.runtime_module.clone();
    let package = f.package(&input);
    let loaded = load_runtime_package(&package).value.unwrap();
    assert_eq!(loaded.active_scene.entities.len(), 2);
    assert!(
        loaded
            .spine_animator_registry
            .as_ref()
            .is_none_or(|r| r.controllers.is_empty())
    );
    assert_eq!(
        loaded.manifest.project.runtime_module,
        original.manifest.project.runtime_module
    );
    drop(loaded);
    drop(input);
    drop(original);
    let host = root.join("same-binary-empty-host");
    fs::create_dir_all(&host).unwrap();
    let mut identities = serde_json::Map::new();
    for name in ["Game.exe", "engine_runtime.dll"] {
        let bytes = fs::read(delivery.join(name)).unwrap();
        fs::write(host.join(name), &bytes).unwrap();
        assert_eq!(fs::read(host.join(name)).unwrap(), bytes);
        identities.insert(name.into(), json!(sha256_prefixed(&bytes)));
    }
    let module = delivery
        .join("data/bin/sample_spine374_recipes.dll")
        .canonicalize()
        .unwrap();
    let module_hash = sha256_prefixed(&fs::read(&module).unwrap());
    identities.insert("projectModule".into(), json!(module_hash));
    let report_path = root.join("empty-player.json");
    let policy = runtime_player_winit::measurement::MeasurementPolicy {
        level: runtime_player_winit::measurement::MeasurementLevel::Off,
        cpu_probe: true,
        ..Default::default()
    };
    let args = vec![
        "run-native-player".into(),
        "--package".into(),
        package.as_os_str().to_owned(),
        "--mode".into(),
        "windowed".into(),
        "--frames".into(),
        "4200".into(),
        "--runtime-report-level".into(),
        "off".into(),
        "--performance-warmup-frames".into(),
        "600".into(),
        "--performance-sample-frames".into(),
        "3600".into(),
        "--measurement-policy".into(),
        serde_json::to_string(&policy).unwrap().into(),
        "--report".into(),
        report_path.as_os_str().to_owned(),
    ];
    let process = runtime_cli::run_bounded_child_process(runtime_cli::BoundedChildProcessRequest {
        executable: host.join("Game.exe"),
        args,
        current_dir: host.clone(),
        environment: vec![(
            "AIFE_PROJECT_RUNTIME_DLL".into(),
            module.as_os_str().to_owned(),
        )],
        timeout: std::time::Duration::from_secs(300),
        stdout_capture_limit_bytes: 64 * 1024,
        stderr_capture_limit_bytes: 64 * 1024,
        priority: runtime_cli::BoundedChildProcessPriority::Normal,
    });
    fs::write(root.join("empty-process.json"),serde_json::to_vec_pretty(&json!({"purpose":"same-binary empty-scene counterfactual; not a second export","package":package,"binarySha256":identities,"process":process})).unwrap()).unwrap();
    assert_eq!(process.exit_code, Some(0), "{process:#?}");
    assert!(process.owned_process_cleanup_confirmed());
    assert_eq!(sha256_prefixed(&fs::read(&module).unwrap()), module_hash);
    for name in ["Game.exe", "engine_runtime.dll"] {
        assert_eq!(
            sha256_prefixed(&fs::read(host.join(name)).unwrap()),
            identities[name].as_str().unwrap()
        );
    }
    let player: Value = serde_json::from_slice(&fs::read(&report_path).unwrap()).unwrap();
    assert_eq!(player["exitCode"], 0, "{player}");
    player["framePerformanceSummary"].clone()
}
