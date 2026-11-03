//! Bounded ordinary-project composition consumer, separate from original-scale qualification.
use project_authoring_execution::{
    default_project_runtime_player_build_root, BuildRequest, GameProjectCompiler,
    ProjectAuthoringSession, ProjectRelativePath, TargetProfile,
};
use std::{fs, path::PathBuf, time::Instant};

#[test]
#[ignore = "explicit run-owned lit K1 project; builds actual DLL and consumes the declared bounded scenario"]
fn ordinary_lit_project_executes_k1_through_deferred_commit_and_observation() {
    let source =
        PathBuf::from(std::env::var_os("S2D_K1_SOURCE").expect("generated ordinary project"));
    let evidence = PathBuf::from(std::env::var_os("S2D_K1_EVIDENCE").expect("fresh evidence root"));
    fs::create_dir(&evidence).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(source.join("project.aife.json")).unwrap()).unwrap();
    assert_eq!(
        manifest["runtimeModule"]["cargoPackage"],
        "platform_3d_lit_k1_runtime"
    );
    let mut session = ProjectAuthoringSession::open(&source).unwrap();
    let paths = session
        .source_inventory()
        .unwrap()
        .entries
        .into_iter()
        .map(|e| e.relative_path)
        .collect();
    let lease = session.acquire_snapshot_lease("371-lit-k1", paths).unwrap();
    let compiler = GameProjectCompiler::bind(&lease).unwrap();
    let started = Instant::now();
    let cache = std::env::var_os("S2D_LIT_COOK_CACHE").map(PathBuf::from);
    let prepared = if let Some(cache) = &cache {
        compiler.prepare_with_artifact_cache(&lease, TargetProfile::WindowsRelease, cache)
    } else {
        compiler.prepare(&lease, TargetProfile::WindowsRelease)
    }
    .unwrap();
    let prepare_ms = started.elapsed().as_millis();
    fs::write(
        evidence.join("prepare.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "prepareMs": prepare_ms, "cacheRoot": cache,
            "summary": prepared.prepare_summary(),
        }))
        .unwrap(),
    )
    .unwrap();
    // Prepare validates the exact leased scenario before paying native build cost.
    prepared.load_playtest_scenario().unwrap();
    let scenario_source = std::env::var_os("S2D_K1_SCENARIO")
        .map(PathBuf::from)
        .unwrap_or_else(|| source.join("Tests/k1.scenario.json"));
    // An explicit diagnostic scenario does not qualify the Compiler's leased
    // default scenario. Keep its identity and actual target/tick scope visible.
    let scenario: runtime_player_winit::semantic_outcome::PlaytestScenario =
        serde_json::from_slice(&fs::read(&scenario_source).unwrap()).unwrap();
    let scenario_scope = serde_json::json!({
        "source": scenario_source, "target": scenario.target,
        "maxSimulationTicks": scenario.max_simulation_ticks,
        "maxPresentationFrames": scenario.max_presentation_frames,
    });
    let build_output = std::env::var("S2D_K1_BUILD_OUTPUT").unwrap_or_else(|_| "Build/K1".into());
    let started = Instant::now();
    let built = compiler
        .build(
            &prepared,
            BuildRequest::for_project(
                TargetProfile::WindowsRelease,
                &source,
                ProjectRelativePath::parse(&build_output).unwrap(),
                default_project_runtime_player_build_root(),
            )
            .with_player_verification(false)
            .with_timeout_ms(1_200_000),
        )
        .unwrap();
    let build_ms = started.elapsed().as_millis();
    fs::write(
        evidence.join("build.json"),
        serde_json::to_vec_pretty(built.desktop_export().unwrap()).unwrap(),
    )
    .unwrap();
    let started = Instant::now();
    // Exercise the bounded process owner explicitly for this Release diagnostic.
    // Compiler playtest_delivery also accepts sealed Windows Dev/Release inputs.
    let report =
        runtime_cli::run_bounded_semantic_playtest(runtime_cli::SemanticPlaytestProcessRequest {
            player_executable: built.delivery().package_dir().join("Game.exe"),
            runtime_package: built.delivery().package_dir().join("data/runtime_package"),
            scenario,
            output_dir: evidence.join("cycle"),
        })
        .unwrap();
    let runtime_ms = started.elapsed().as_millis();
    fs::write(evidence.join("timings.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "prepareMs":prepare_ms,"buildMs":build_ms,"runtimeMs":runtime_ms,
        "scenario":scenario_scope,
        "scope":"Windows Release ordinary lit project; exact declared diagnostic scenario only; not Compiler Dev playtest qualification, original-scale or full-period GPU qualification"
    })).unwrap()).unwrap();
    fs::write(
        evidence.join("playtest.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(&report).unwrap()["overall"],
        "passed",
        "{report:#?}"
    );
}
