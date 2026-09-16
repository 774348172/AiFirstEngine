use project_authoring_execution::{
    BuildDeliveryReport, BuildRequest, CheckProfile, DeliveryRef, DeliveryVerificationReport,
    GameProjectCompiler, GameProjectCompilerError, PreparedRuntimePackage, RunOptions,
    RuntimeExecutionReport, TargetProfile, VerifyRequest,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn game_project_compiler_interface_prepare_is_deterministic_for_same_revision() {
    let fixture = Fixture::new("deterministic", "compiler.fixture.deterministic");
    let mut session = project_authoring_execution::ProjectAuthoringSession::open(fixture.path())
        .expect("open fixture");
    let first_lease = full_lease(&mut session, "prepare-1");
    let second_lease = full_lease(&mut session, "prepare-2");
    let compiler = GameProjectCompiler::bind(&first_lease).expect("bind compiler");

    let first = compiler
        .prepare(&first_lease, TargetProfile::WindowsDev)
        .expect("prepare first lease");
    let second = compiler
        .prepare(&second_lease, TargetProfile::WindowsDev)
        .expect("prepare second lease");

    assert_eq!(first.preparation_identity(), second.preparation_identity());
    assert_eq!(first.lineage(), second.lineage());
    assert_eq!(first.source_file_count(), 2);
    assert!(first.runtime_package_identity().is_some());
    assert!(first.is_runtime_package_ready());
    assert_eq!(first.assembly_digest(), second.assembly_digest());
    assert_eq!(first.runtime_package_build_input().scenes.len(), 0);
}

#[test]
fn game_project_compiler_check_validates_without_assembling_runtime_package() {
    let fixture = Fixture::new("check-valid", "compiler.fixture.check");
    let mut session = project_authoring_execution::ProjectAuthoringSession::open(fixture.path())
        .expect("open fixture");
    let lease = full_lease(&mut session, "check-valid");
    let compiler = GameProjectCompiler::bind(&lease).expect("bind compiler");

    let report = compiler
        .check(&lease, CheckProfile::new(TargetProfile::WindowsDev))
        .expect("check fixture");

    assert_eq!(report.project_identity(), "compiler.fixture.check");
    assert_eq!(report.revision_id(), lease.snapshot().revision.revision_id);
    assert_eq!(report.target_profile(), TargetProfile::WindowsDev);
    assert_eq!(report.source_file_count(), 2);
}

#[test]
fn particle_effect_check_and_prepare_share_snapshot_program_validation() {
    let fixture = Fixture::new("particle-effect", "compiler.fixture.particles");
    fs::create_dir(fixture.path().join("Assets")).unwrap();
    let mut description: serde_json::Value = serde_json::from_str(include_str!(
        "../../engine_runtime/tests/fixtures/particle-effect.json"
    ))
    .unwrap();
    description["emitters"][0]["behaviorSource"] = "Assets/motion.particle.wgsl".into();
    fs::write(
        fixture.path().join("Assets/effect.particle-effect.json"),
        serde_json::to_vec(&description).unwrap(),
    )
    .unwrap();
    let program = "fn particle_init(p: Particle, c: ParticleContext, e: EffectParams, i: ParticleInputs) -> Particle { return p; }\nfn particle_update(p: Particle, c: ParticleContext, e: EffectParams, i: ParticleInputs) -> Particle { var result = p; result.velocity.y = e.speed; return result; }\n";
    let program_path = fixture.path().join("Assets/motion.particle.wgsl");
    fs::write(&program_path, program).unwrap();
    let mut session =
        project_authoring_execution::ProjectAuthoringSession::open(fixture.path()).unwrap();
    let good = full_lease(&mut session, "particle-good");
    let compiler = GameProjectCompiler::bind(&good).unwrap();
    compiler
        .check(&good, CheckProfile::new(TargetProfile::WindowsDev))
        .unwrap();
    // An old lease must remain valid even when the live source is now broken.
    fs::write(
        &program_path,
        program.replace("e.speed", "e.missing_parameter"),
    )
    .unwrap();
    session.refresh().unwrap();
    let bad = full_lease(&mut session, "particle-bad");
    let prepared = compiler.prepare(&good, TargetProfile::WindowsDev).unwrap();
    let asset = prepared
        .runtime_package_build_input()
        .assets
        .iter()
        .find(|a| a.asset_type == "particle-effect")
        .unwrap();
    assert!(String::from_utf8(asset.runtime_payload.clone().unwrap())
        .unwrap()
        .contains("e.speed"));
    let bad_compiler = GameProjectCompiler::bind(&bad).unwrap();
    let check = bad_compiler
        .check(&bad, CheckProfile::new(TargetProfile::WindowsDev))
        .unwrap_err();
    let prepare = bad_compiler
        .prepare(&bad, TargetProfile::WindowsDev)
        .unwrap_err();
    assert_eq!(check.source_location(), prepare.source_location());
    assert_eq!(
        check.source_location().unwrap().source_path,
        "Assets/motion.particle.wgsl"
    );
    assert_eq!(check.source_location().unwrap().line, Some(2));
}

#[test]
fn game_project_compiler_interface_prepare_consumes_r1_lease_after_live_r2_refresh() {
    let fixture = Fixture::new("lease-r1-r2", "compiler.fixture.lease");
    let source_path = fixture.path().join("Game/main.rs");
    let mut session = project_authoring_execution::ProjectAuthoringSession::open(fixture.path())
        .expect("open fixture");
    let r1_lease = full_lease(&mut session, "prepare-r1");
    let r1_revision = r1_lease.snapshot().revision.revision_id.clone();
    let r1_compiler = GameProjectCompiler::bind(&r1_lease).expect("bind r1 compiler");

    fs::write(&source_path, b"fn game() { let _revision = 2; }").expect("write r2");
    session.refresh().expect("refresh r2");
    let r2_lease = full_lease(&mut session, "prepare-r2");

    let prepared_r1 = r1_compiler
        .prepare(&r1_lease, TargetProfile::WindowsDev)
        .expect("prepare retained r1");
    let mismatch = r1_compiler
        .prepare(&r2_lease, TargetProfile::WindowsDev)
        .expect_err("r1 compiler must reject r2 lease");
    let prepared_r2 = GameProjectCompiler::bind(&r2_lease)
        .expect("bind r2 compiler")
        .prepare(&r2_lease, TargetProfile::WindowsDev)
        .expect("prepare r2");

    assert_eq!(prepared_r1.lineage().revision_id(), r1_revision);
    assert_ne!(
        prepared_r1.preparation_identity(),
        prepared_r2.preparation_identity()
    );
    assert_eq!(
        mismatch.code(),
        "game_project_compiler.operation_binding_mismatch"
    );
}

#[test]
fn game_project_compiler_generated_runtime_glue_is_bound_to_the_operation_lease() {
    let fixture = Fixture::with_manifest(
        "generated-glue-lease",
        br#"{
          "schemaVersion":"aife-project.v2",
          "projectId":"compiler.fixture.generated-glue",
          "runtimeModule":{
            "moduleId":"fixture.generated.runtime",
            "interfaceVersion":"project-runtime-module.v2",
            "cargoManifest":"RuntimeModule/Cargo.toml",
            "cargoPackage":"fixture_generated_runtime",
            "playerBinary":"fixture_generated_player",
            "projectGameSdk":"project-game-sdk.v1"
          }
        }"#,
    );
    fs::create_dir_all(fixture.path().join("RuntimeModule/src")).expect("create runtime source");
    fs::write(
        fixture.path().join("RuntimeModule/Cargo.toml"),
        b"[package]\nname='fixture_generated_runtime'\nversion='0.1.0'\n",
    )
    .expect("write runtime manifest");
    let source_path = fixture.path().join("RuntimeModule/src/lib.rs");
    fs::write(
        &source_path,
        b"pub fn project_game() { let _revision = 1; }",
    )
    .expect("write r1 runtime source");
    let mut session = project_authoring_execution::ProjectAuthoringSession::open(fixture.path())
        .expect("open generated glue fixture");
    let r1_lease = full_lease(&mut session, "generated-glue-r1");
    let r1_compiler = GameProjectCompiler::bind(&r1_lease).expect("bind r1 compiler");

    fs::write(
        &source_path,
        b"pub fn project_game() { let _revision = 2; }",
    )
    .expect("write r2 runtime source");
    session.refresh().expect("refresh r2");
    let r2_lease = full_lease(&mut session, "generated-glue-r2");

    let r1 = r1_compiler
        .prepare(&r1_lease, TargetProfile::WindowsDev)
        .expect("prepare retained r1");
    let r2 = GameProjectCompiler::bind(&r2_lease)
        .expect("bind r2 compiler")
        .prepare(&r2_lease, TargetProfile::WindowsDev)
        .expect("prepare r2");
    let r1_glue = r1
        .generated_runtime_glue_report()
        .expect("r1 generation report");
    let r2_glue = r2
        .generated_runtime_glue_report()
        .expect("r2 generation report");

    assert_eq!(r1_glue.project_game_sdk_contract, "project-game-sdk.v1");
    assert_ne!(
        r1_glue.runtime_module_source_identity,
        r2_glue.runtime_module_source_identity
    );
    assert_ne!(r1_glue.generation_digest, r2_glue.generation_digest);
}

#[test]
fn game_project_compiler_interface_prepare_fails_closed_for_invalid_or_missing_manifest() {
    let invalid = Fixture::new("invalid", "compiler.fixture.invalid");
    let mut invalid_session =
        project_authoring_execution::ProjectAuthoringSession::open(invalid.path())
            .expect("open invalid fixture");
    fs::write(invalid.path().join("project.aife.json"), b"{").expect("invalidate manifest");
    invalid_session
        .refresh()
        .expect("refresh invalid revision fact");
    let invalid_lease = full_lease(&mut invalid_session, "prepare-invalid");
    let invalid_error = GameProjectCompiler::bind(&invalid_lease)
        .expect("bind invalid revision")
        .prepare(&invalid_lease, TargetProfile::WindowsDev)
        .expect_err("invalid revision must fail closed");

    let missing = Fixture::new("missing-manifest-scope", "compiler.fixture.missing");
    let mut missing_session =
        project_authoring_execution::ProjectAuthoringSession::open(missing.path())
            .expect("open missing-scope fixture");
    let empty_lease = missing_session
        .acquire_snapshot_lease("prepare-empty", Vec::new())
        .expect("acquire empty bounded lease");
    let missing_error = GameProjectCompiler::bind(&empty_lease)
        .expect("bind empty lease")
        .prepare(&empty_lease, TargetProfile::WindowsDev)
        .expect_err("manifest outside lease must fail closed");

    assert_eq!(
        invalid_error.code(),
        "game_project_compiler.source_json_invalid"
    );
    assert_eq!(
        missing_error.code(),
        "game_project_compiler.manifest_missing_from_snapshot"
    );
    assert_eq!(
        invalid_error.source_location().unwrap().source_path,
        "project.aife.json"
    );
}

#[test]
fn game_project_compiler_interface_rejects_cross_project_prepared_identity() {
    let first = Fixture::new("project-a", "compiler.fixture.a");
    let second = Fixture::new("project-b", "compiler.fixture.b");
    let mut first_session =
        project_authoring_execution::ProjectAuthoringSession::open(first.path())
            .expect("open first fixture");
    let mut second_session =
        project_authoring_execution::ProjectAuthoringSession::open(second.path())
            .expect("open second fixture");
    let first_lease = full_lease(&mut first_session, "prepare-a");
    let second_lease = full_lease(&mut second_session, "prepare-b");
    let compiler = GameProjectCompiler::bind(&first_lease).expect("bind first compiler");

    let error = compiler
        .prepare(&second_lease, TargetProfile::WindowsDev)
        .expect_err("compiler must reject another project binding");

    assert_eq!(
        error.code(),
        "game_project_compiler.operation_binding_mismatch"
    );

    let _run: fn(
        &GameProjectCompiler,
        &PreparedRuntimePackage,
        RunOptions,
    ) -> Result<RuntimeExecutionReport, GameProjectCompilerError> = GameProjectCompiler::run;
    let _build: fn(
        &GameProjectCompiler,
        &PreparedRuntimePackage,
        BuildRequest,
    ) -> Result<BuildDeliveryReport, GameProjectCompilerError> = GameProjectCompiler::build;
    let _verify: fn(
        &GameProjectCompiler,
        &DeliveryRef,
        VerifyRequest,
    ) -> Result<DeliveryVerificationReport, GameProjectCompilerError> = GameProjectCompiler::verify;
}

#[test]
fn game_project_compiler_assembler_owner_prepares_complex_shooter_runtime_input() {
    let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("samples")
        .join("complex_shooter_project");
    let mut session = project_authoring_execution::ProjectAuthoringSession::open(&project_root)
        .expect("open complex shooter fixture");
    let lease = full_lease(&mut session, "assembler-owner-red-test");
    let compiler = GameProjectCompiler::bind(&lease).expect("bind compiler");

    let prepared = compiler
        .prepare(&lease, TargetProfile::WindowsDev)
        .expect("prepare complex shooter snapshot");

    assert!(
        prepared.is_runtime_package_ready(),
        "F-B requires the Compiler's internal Assembler to produce a RuntimePackageBuildInput"
    );
    assert!(prepared.source_file_count() >= 20);
    let input = prepared.runtime_package_build_input();
    assert_eq!(input.scenes.len(), 1);
    assert_eq!(input.prefabs.len(), 4);
    assert_eq!(input.aui_documents.len(), 1);
    let hud: engine_runtime::aui::AuiDocument =
        serde_json::from_value(input.aui_documents[0].document.clone()).expect("canonical HUD");
    let viewport = engine_runtime::aui::AuiComputedRect {
        x: 0.0,
        y: 0.0,
        width: 1280.0,
        height: 720.0,
    };
    let mut rectangles = Vec::new();
    for node in hud.nodes.iter().filter(|node| node.parent.is_some()) {
        let rect = node.rect.resolve(viewport);
        assert!(rect.width > 0.0 && rect.height > 0.0);
        assert!(
            rect.width * rect.height < viewport.width * viewport.height / 4.0,
            "HUD child {} must not cover the game",
            node.node_id
        );
        assert!(
            rect.x >= 0.0
                && rect.y >= 0.0
                && rect.x + rect.width <= viewport.width
                && rect.y + rect.height <= viewport.height
        );
        // Backplates intentionally sit behind controls; controls themselves remain disjoint.
        if node.kind == engine_runtime::aui::AuiNodeKind::Panel {
            assert!(!node.consume_input);
            continue;
        }
        for previous in &rectangles {
            let previous: &engine_runtime::aui::AuiComputedRect = previous;
            assert!(
                rect.x + rect.width <= previous.x
                    || previous.x + previous.width <= rect.x
                    || rect.y + rect.height <= previous.y
                    || previous.y + previous.height <= rect.y,
                "HUD controls must not overlap"
            );
        }
        rectangles.push(rect);
        assert!(!node.consume_input);
    }
    let paths = hud
        .nodes
        .iter()
        .flat_map(|node| &node.binding_refs)
        .map(|binding| binding.path.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        paths,
        [
            "game.score_text",
            "game.wave_text",
            "game.enemy_count_text",
            "player.hp_ratio"
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(input.input_mappings.len(), 1);
    assert_eq!(input.assets.len(), 9);
    assert!(input.assets.iter().any(|asset| {
        asset.asset_id == "scene-main"
            && asset.asset_type == "scene"
            && asset.runtime_uri == "scenes/scene-main.json"
    }));
    assert_eq!(input.texture_payloads.len(), 7);
    assert!(input.texture_payloads.iter().all(|texture| {
        matches!(
            texture.metadata.format.as_str(),
            "rgba8UnormSrgb" | "rgba8Unorm"
        )
    }));
    assert_eq!(
        input
            .rule_manifest
            .as_ref()
            .map(|manifest| manifest.rules.len()),
        Some(5)
    );
    assert_eq!(
        prepared.assembly_digest(),
        prepared.runtime_package_identity().unwrap()
    );
    let scene = &input.scenes[0];
    assert!(scene
        .entities
        .iter()
        .any(|entity| entity.id == "entity-enemy-a"));
    assert!(scene
        .entities
        .iter()
        .any(|entity| entity.id == "entity-enemy-b"));
    assert!(!scene
        .entities
        .iter()
        .flat_map(|entity| &entity.components)
        .any(|component| { component.component_type == "engine.prefab_instance" }));
}

#[test]
fn game_project_compiler_assembler_cooks_animator2d_from_lease_sources() {
    let fixture = Fixture::new("animator2d", "compiler.fixture.animator2d");
    fs::create_dir_all(fixture.path().join("Scenes")).expect("create Scenes");
    fs::create_dir_all(fixture.path().join("Animations")).expect("create Animations");
    fs::create_dir_all(fixture.path().join("AUI")).expect("create AUI");
    fs::write(
        fixture.path().join("Scenes/Main.scene.json"),
        br#"{
          "id":"scene-main","name":"Main","entities":[{
            "id":"entity-player","name":"Player","enabled":true,"components":[{
              "componentType":"Animator2D",
              "data":{"controllerRef":"player-controller","enabled":true,"initialBools":{}}
            }]
          }]
        }"#,
    )
    .expect("write Scene");
    fs::write(
        fixture
            .path()
            .join("Animations/player-idle.sprite-animation-clip-2d.json"),
        br#"{
          "schema":"sprite-animation-clip-2d.v1","assetId":"player-idle","playback":"loop",
          "frames":[{"spriteRef":"sprite-player","durationTicks":2}]
        }"#,
    )
    .expect("write clip");
    fs::write(
        fixture
            .path()
            .join("Animations/player-controller.animator-controller-2d.json"),
        br#"{
          "schema":"animator-controller-2d.v1","assetId":"player-controller",
          "entryStateId":"idle","states":[{"id":"idle","clipRef":"player-idle"}]
        }"#,
    )
    .expect("write controller");
    fs::write(
        fixture.path().join("AUI/legacy.aui.json"),
        br#"{
          "schema_version":"aui-document.v1","document_id":"legacy-hud",
          "canvases":[],"nodes":[]
        }"#,
    )
    .expect("write legacy AUI");
    let mut session = project_authoring_execution::ProjectAuthoringSession::open(fixture.path())
        .expect("open animator fixture");
    let lease = full_lease(&mut session, "animator2d-owner");
    let prepared = GameProjectCompiler::bind(&lease)
        .expect("bind animator fixture")
        .prepare(&lease, TargetProfile::WindowsDev)
        .expect("prepare animator fixture");
    let input = prepared.runtime_package_build_input();

    assert_eq!(input.animator2d_registry.clips.len(), 1);
    assert_eq!(input.animator2d_registry.controllers.len(), 1);
    assert_eq!(
        input.aui_documents[0].document["schema_version"],
        "aui-document.v2"
    );
    let animator = input.scenes[0].entities[0]
        .animator2d
        .as_ref()
        .expect("runtime Animator2D");
    assert_eq!(animator.controller_id, "player-controller");
    assert_eq!(
        animator.registry_digest,
        input.animator2d_registry.registry_digest
    );
}

#[test]
fn game_project_compiler_animator2d_description_lease_reaches_package() {
    let fixture = Fixture::new(
        "animator-description",
        "compiler.fixture.animator-description",
    );
    for dir in ["Scenes", "Animations", "Assets"] {
        fs::create_dir_all(fixture.path().join(dir)).unwrap();
    }
    fs::write(fixture.path().join("Scenes/Main.scene.json"),br#"{"id":"main","name":"Main","entities":[{"id":"robot","name":"Robot","components":[{"componentType":"SpriteRenderer2D","data":{"spriteRef":"frame"}}]}]}"#).unwrap();
    fs::write(fixture.path().join("Animations/robot.animator-description-2d.json"),br#"{"schema":"animator-description-2d.v1","assetId":"robot-animations","entity":"robot","default":"idle","animations":{"idle":{"frames":["frame"],"loop":true}}}"#).unwrap();
    fs::write(fixture.path().join("Assets/frame.asset"),br#"{"schemaVersion":"texture-asset.v1","assetId":"frame","sourceImage":"Assets/frame.png"}"#).unwrap();
    let mut png_bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_bytes, 1, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&[255, 255, 255, 255])
            .unwrap();
    }
    fs::write(fixture.path().join("Assets/frame.png"), png_bytes).unwrap();
    let mut session =
        project_authoring_execution::ProjectAuthoringSession::open(fixture.path()).unwrap();
    let lease = full_lease(&mut session, "animator-description");
    let prepared = GameProjectCompiler::bind(&lease)
        .unwrap()
        .prepare(&lease, TargetProfile::WindowsDev)
        .unwrap();
    let input = prepared.runtime_package_build_input();
    assert_eq!(
        input.scenes[0].entities[0]
            .animator2d
            .as_ref()
            .unwrap()
            .controller_id,
        "robot-animations"
    );
    assert_eq!(
        input.animator2d_registry.clips[0].id,
        "robot-animations.idle"
    );
    assert_eq!(input.texture_payloads.len(), 1);
    assert_eq!(
        fs::read_dir(fixture.path().join("Animations"))
            .unwrap()
            .count(),
        1,
        "derived assets must stay internal"
    );
}

fn full_lease(
    session: &mut project_authoring_execution::ProjectAuthoringSession,
    owner: &str,
) -> authoring_project_context::ProjectSnapshotLease {
    let paths = session
        .source_inventory()
        .expect("capture source inventory")
        .entries
        .into_iter()
        .map(|entry| entry.relative_path)
        .collect();
    session
        .acquire_snapshot_lease(owner, paths)
        .expect("acquire full snapshot lease")
}

#[test]
fn check_reports_source_location_and_rejects_missing_reference() {
    let fixture = Fixture::new("check-reference", "compiler.fixture.reference");
    let mut session =
        project_authoring_execution::ProjectAuthoringSession::open(fixture.path()).unwrap();
    fs::write(fixture.path().join("project.aife.json"), br#"{"schemaVersion":"aife-project.v2","projectId":"compiler.fixture.reference","defaultScene":"Scenes/missing.scene.json"}"#).unwrap();
    session.refresh().unwrap();
    let lease = full_lease(&mut session, "missing-reference");
    let compiler = GameProjectCompiler::bind(&lease).unwrap();
    let error = compiler
        .check(&lease, CheckProfile::new(TargetProfile::WindowsDev))
        .unwrap_err();
    assert_eq!(
        error.source_location().unwrap().source_path,
        "project.aife.json"
    );
    assert_eq!(
        error.source_location().unwrap().field_path.as_deref(),
        Some("/defaultScene")
    );
    assert!(compiler.prepare(&lease, TargetProfile::WindowsDev).is_err());
}

struct Fixture {
    root: PathBuf,
}

#[test]
fn prepare_texture_cache_tracks_shared_dependencies_corruption_and_revision_lineage() {
    use project_authoring_execution::ProjectAssemblyArtifactCacheStatus as Status;
    let fixture = Fixture::new("incremental-texture", "compiler.incremental.texture");
    fs::create_dir_all(fixture.path().join("Assets")).unwrap();
    let png = |pixel: [u8; 4]| {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&pixel)
                .unwrap();
        }
        bytes
    };
    fs::write(
        fixture.path().join("Assets/shared.png"),
        png([1, 2, 3, 255]),
    )
    .unwrap();
    for id in ["a", "b"] {
        fs::write(fixture.path().join(format!("Assets/{id}.asset")), serde_json::to_vec(&serde_json::json!({"schemaVersion":"texture-asset.v1","assetId":id,"sourceImage":"Assets/shared.png"})).unwrap()).unwrap();
    }
    let cache = fixture.path().join("Library/test-cache");
    let mut session =
        project_authoring_execution::ProjectAuthoringSession::open(fixture.path()).unwrap();
    let lease = full_lease(&mut session, "first");
    let compiler = GameProjectCompiler::bind(&lease).unwrap();
    let first = compiler
        .prepare_with_artifact_cache(&lease, TargetProfile::WindowsDev, &cache)
        .unwrap();
    assert!(first
        .producer_reports()
        .iter()
        .all(|r| r.cache_status == Status::Miss));
    let second = compiler
        .prepare_with_artifact_cache(&lease, TargetProfile::WindowsDev, &cache)
        .unwrap();
    assert_eq!(second.prepare_summary()["reusedCount"], 2);
    fs::create_dir_all(fixture.path().join("RuntimeModule/src")).unwrap();
    fs::write(
        fixture.path().join("RuntimeModule/src/unrelated.rs"),
        "// source changed\n",
    )
    .unwrap();
    session.refresh().unwrap();
    let changed_lease = full_lease(&mut session, "rust-change");
    let compiler = GameProjectCompiler::bind(&changed_lease).unwrap();
    let third = compiler
        .prepare_with_artifact_cache(&changed_lease, TargetProfile::WindowsDev, &cache)
        .unwrap();
    assert_ne!(first.lineage().revision_id(), third.lineage().revision_id());
    assert_eq!(third.prepare_summary()["reusedCount"], 2);
    let payload =
        Path::new(third.producer_reports()[0].artifact_path.as_ref().unwrap()).join("payload.json");
    fs::write(payload, "broken").unwrap();
    let repaired = compiler
        .prepare_with_artifact_cache(&changed_lease, TargetProfile::WindowsDev, &cache)
        .unwrap();
    assert_eq!(repaired.producer_reports()[0].cache_status, Status::Corrupt);
    assert_eq!(repaired.producer_reports()[1].cache_status, Status::Hit);
    fs::write(
        fixture.path().join("Assets/shared.png"),
        png([4, 5, 6, 255]),
    )
    .unwrap();
    session.refresh().unwrap();
    let image_lease = full_lease(&mut session, "image-change");
    let compiler = GameProjectCompiler::bind(&image_lease).unwrap();
    let changed = compiler
        .prepare_with_artifact_cache(&image_lease, TargetProfile::WindowsDev, &cache)
        .unwrap();
    assert!(changed
        .producer_reports()
        .iter()
        .all(|r| r.cache_status == Status::Miss));
    assert_ne!(first.assembly_digest(), changed.assembly_digest());
}

const CHECK_GAME: &str = r#"
use project_game_sdk::*;
pub struct Session;
impl ProjectGameSession for Session { fn session_id(&self) -> &str { "check.session" } }
pub fn project_game() -> ProjectGameDefinition<Session> {
    ProjectGameDefinition::new(|_: &SessionCreateRequest| Ok(Session))
}
"#;

fn rust_check_fixture() -> Fixture {
    let mut manifest: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../samples/switch_puzzle_project/project.aife.json"
    ))
    .unwrap();
    manifest["projectId"] = "compiler.check.rust".into();
    manifest
        .as_object_mut()
        .unwrap()
        .remove("observationContract");
    let fixture = Fixture::with_manifest("rust-check", &serde_json::to_vec(&manifest).unwrap());
    fs::create_dir_all(fixture.path().join("Scenes")).unwrap();
    fs::write(fixture.path().join("Scenes/Main.scene.json"), b"{}").unwrap();
    fs::create_dir_all(fixture.path().join("RuntimeModule/src")).unwrap();
    fs::write(
        fixture.path().join("RuntimeModule/Cargo.toml"),
        include_str!("../../../../samples/switch_puzzle_project/RuntimeModule/Cargo.toml"),
    )
    .unwrap();
    fs::write(fixture.path().join("RuntimeModule/src/lib.rs"), CHECK_GAME).unwrap();
    fixture
}

#[test]
fn check_real_cargo_uses_lease_reports_rust_span_and_does_not_edit_sources() {
    let fixture = rust_check_fixture();
    let mut session =
        project_authoring_execution::ProjectAuthoringSession::open(fixture.path()).unwrap();
    let valid = full_lease(&mut session, "check-r1");
    let compiler = GameProjectCompiler::bind(&valid).unwrap();
    let options = CheckProfile::new(TargetProfile::WindowsDev).with_process_approval(true);
    let source_path = fixture.path().join("RuntimeModule/src/lib.rs");
    let invalid_source = format!("{CHECK_GAME}\npub fn bad() {{ let _: u32 = \"wrong\"; }}\n");
    fs::write(&source_path, &invalid_source).unwrap();
    session.refresh().unwrap();
    let invalid = full_lease(&mut session, "check-r2");
    let report = compiler
        .check(&valid, options.clone())
        .expect("retained R1 should still compile");
    assert_eq!(report.rust_check, "passed");
    assert!(compiler.check(&invalid, options.clone()).is_err());
    let error = GameProjectCompiler::bind(&invalid)
        .unwrap()
        .check(&invalid, options)
        .unwrap_err();
    let diagnostic = error
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code == "E0308")
        .expect("real rustc type error");
    let at = diagnostic.location.as_ref().unwrap();
    assert_eq!(at.source_path, "RuntimeModule/src/lib.rs");
    assert!(at.line.unwrap() > 1);
    assert!(at.column.unwrap() > 0);
    assert!(!at.generated);
    assert_eq!(fs::read_to_string(source_path).unwrap(), invalid_source);
    assert!(!fixture.path().join("RuntimeModule/Cargo.lock").exists());
    assert!(!fixture.path().join("target").exists());
}

#[test]
fn check_rust_approval_missing_cargo_and_timeout_never_pass() {
    let fixture = rust_check_fixture();
    let mut session =
        project_authoring_execution::ProjectAuthoringSession::open(fixture.path()).unwrap();
    let lease = full_lease(&mut session, "check-process-failures");
    let compiler = GameProjectCompiler::bind(&lease).unwrap();
    assert_eq!(
        compiler
            .check(&lease, CheckProfile::new(TargetProfile::WindowsDev))
            .unwrap_err()
            .code(),
        "game_project_compiler.check_process_approval_required"
    );
    let error = compiler
        .check(
            &lease,
            CheckProfile::new(TargetProfile::WindowsDev)
                .with_process_approval(true)
                .with_cargo_executable(fixture.path().join("missing-cargo.exe")),
        )
        .unwrap_err();
    assert!(error.message().contains("SpawnFailed"), "{error:?}");
    let error = compiler
        .check(
            &lease,
            CheckProfile::new(TargetProfile::WindowsDev)
                .with_process_approval(true)
                .with_timeout_ms(1),
        )
        .unwrap_err();
    assert!(error.message().contains("Timeout"), "{error:?}");
}

#[test]
fn check_generated_contract_errors_are_not_mislabelled_as_user_source() {
    let fixture = rust_check_fixture();
    fs::write(
        fixture.path().join("RuntimeModule/src/lib.rs"),
        "pub fn project_game() -> u32 { 0 }\n",
    )
    .unwrap();
    let mut session =
        project_authoring_execution::ProjectAuthoringSession::open(fixture.path()).unwrap();
    let lease = full_lease(&mut session, "check-generated-contract");
    let error = GameProjectCompiler::bind(&lease)
        .unwrap()
        .check(
            &lease,
            CheckProfile::new(TargetProfile::WindowsDev).with_process_approval(true),
        )
        .unwrap_err();
    assert!(
        error.diagnostics().iter().any(|diagnostic| {
            diagnostic.location.as_ref().is_some_and(|at| {
                at.generated && at.source_path.starts_with("generated/RuntimeGlue/")
            })
        }),
        "{error:?}"
    );
}

#[test]
fn check_unknown_schema_and_missing_texture_input_have_actionable_locations() {
    let fixture = Fixture::new("source-failures", "compiler.source.failures");
    let mut session =
        project_authoring_execution::ProjectAuthoringSession::open(fixture.path()).unwrap();
    fs::write(
        fixture.path().join("project.aife.json"),
        br#"{"schemaVersion":"unknown","projectId":"compiler.source.failures"}"#,
    )
    .unwrap();
    session.refresh().unwrap();
    let lease = full_lease(&mut session, "schema-failure");
    let error = GameProjectCompiler::bind(&lease)
        .unwrap()
        .check(&lease, CheckProfile::new(TargetProfile::WindowsDev))
        .unwrap_err();
    assert_eq!(
        error.source_location().unwrap().field_path.as_deref(),
        Some("/schemaVersion")
    );
    fs::write(
        fixture.path().join("project.aife.json"),
        br#"{"schemaVersion":"aife-project.v2","projectId":"compiler.source.failures"}"#,
    )
    .unwrap();
    fs::create_dir_all(fixture.path().join("Assets")).unwrap();
    fs::write(fixture.path().join("Assets/image.asset"), br#"{"schemaVersion":"texture-asset.v1","assetId":"image","sourceImage":"Assets/missing.png"}"#).unwrap();
    session.refresh().unwrap();
    let lease = full_lease(&mut session, "texture-failure");
    let compiler = GameProjectCompiler::bind(&lease).unwrap();
    let error = compiler
        .check(&lease, CheckProfile::new(TargetProfile::WindowsDev))
        .unwrap_err();
    assert_eq!(
        error.source_location().unwrap().source_path,
        "Assets/image.asset"
    );
    assert_eq!(
        error.source_location().unwrap().field_path.as_deref(),
        Some("/sourceImage")
    );
    assert!(compiler.prepare(&lease, TargetProfile::WindowsDev).is_err());
}

#[test]
fn check_invalid_json_profile_schema_and_cross_project_fail_closed() {
    let fixture = Fixture::new("invalid-check", "compiler.invalid.check");
    let mut session =
        project_authoring_execution::ProjectAuthoringSession::open(fixture.path()).unwrap();
    fs::write(fixture.path().join("project.aife.json"), b"{\ninvalid").unwrap();
    session.refresh().unwrap();
    let lease = full_lease(&mut session, "check-invalid-json");
    let error = GameProjectCompiler::bind(&lease)
        .unwrap()
        .check(&lease, CheckProfile::new(TargetProfile::WindowsDev))
        .unwrap_err();
    assert_eq!(error.source_location().unwrap().line, Some(2));
    assert!(error.code().contains("source_json_invalid"));

    fs::write(
        fixture.path().join("project.aife.json"),
        br#"{"schemaVersion":"aife-project.v2","projectId":"compiler.invalid.check"}"#,
    )
    .unwrap();
    fs::create_dir_all(fixture.path().join("BuildProfiles")).unwrap();
    fs::write(
        fixture.path().join("BuildProfiles/windows.dev.json"),
        br#"{"target":"android","profile":"dev"}"#,
    )
    .unwrap();
    session.refresh().unwrap();
    let lease = full_lease(&mut session, "check-invalid-profile");
    let compiler = GameProjectCompiler::bind(&lease).unwrap();
    let error = compiler
        .check(&lease, CheckProfile::new(TargetProfile::WindowsDev))
        .unwrap_err();
    assert_eq!(
        error.source_location().unwrap().source_path,
        "BuildProfiles/windows.dev.json"
    );
    assert!(compiler
        .check(&lease, CheckProfile::new(TargetProfile::AndroidDev))
        .unwrap_err()
        .code()
        .contains("unsupported"));

    let other = Fixture::new("cross-check", "compiler.other.check");
    let mut other_session =
        project_authoring_execution::ProjectAuthoringSession::open(other.path()).unwrap();
    let other_lease = full_lease(&mut other_session, "cross-project-check");
    assert!(compiler
        .check(&other_lease, CheckProfile::new(TargetProfile::WindowsDev))
        .is_err());
}

#[test]
fn playtest_scenario_load_uses_retained_manifest_scenario_scene_and_observations() {
    use sha2::{Digest, Sha256};
    let fixture = playtest_fixture("immutable");
    let mut session =
        project_authoring_execution::ProjectAuthoringSession::open(fixture.path()).unwrap();
    let lease = full_lease(&mut session, "playtest-r1");
    let compiler = GameProjectCompiler::bind(&lease).unwrap();
    let prepared = compiler.prepare(&lease, TargetProfile::WindowsDev).unwrap();
    let first = prepared.load_playtest_scenario().unwrap();
    let original = fs::read(fixture.path().join("Tests/default.json")).unwrap();
    assert_eq!(first.source_path(), "Tests/default.json");
    assert_eq!(
        first.source_digest(),
        format!("sha256:{:x}", Sha256::digest(&original))
    );
    assert_eq!(first.lineage(), prepared.lineage());
    assert_eq!(
        first.preparation_identity(),
        prepared.preparation_identity()
    );
    assert_eq!(first.scenario().scenario_id, "scenario.ready");
    fs::write(
        fixture.path().join("Tests/default.json"),
        b"invalid live scenario",
    )
    .unwrap();
    fs::write(
        fixture.path().join("Observations/contract.json"),
        b"invalid live contract",
    )
    .unwrap();
    fs::write(
        fixture.path().join("Scenes/Main.scene.json"),
        b"invalid live scene",
    )
    .unwrap();
    fs::write(
        fixture.path().join("project.aife.json"),
        b"invalid live manifest",
    )
    .unwrap();
    session.refresh().unwrap();
    assert_eq!(prepared.load_playtest_scenario().unwrap(), first);
    assert_eq!(
        compiler
            .prepare(&lease, TargetProfile::WindowsDev)
            .unwrap()
            .load_playtest_scenario()
            .unwrap(),
        first
    );
}

#[test]
fn playtest_scenario_rejects_missing_unsafe_or_non_string_manifest_references() {
    let fixture = playtest_fixture("references");
    for reference in [
        serde_json::Value::Null,
        serde_json::json!(42),
        serde_json::json!("../outside.json"),
        serde_json::json!("C:/outside.json"),
        serde_json::json!("Tests/missing.json"),
    ] {
        let manifest = serde_json::json!({"schemaVersion": "aife-project.v2", "projectId": "compiler.playtest.fixture", "playtestScenario": reference, "observationContract": "Observations/contract.json"});
        fs::write(
            fixture.path().join("project.aife.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        let mut session =
            project_authoring_execution::ProjectAuthoringSession::open(fixture.path()).unwrap();
        let lease = full_lease(&mut session, "playtest-reference");
        let prepared = GameProjectCompiler::bind(&lease)
            .unwrap()
            .prepare(&lease, TargetProfile::WindowsDev)
            .unwrap();
        let error = prepared.load_playtest_scenario().unwrap_err();
        assert!(error.code().contains("playtest_"));
        assert!(error.source_location().is_some());
    }
}

#[test]
fn playtest_scenario_rejects_invalid_json_unknown_scene_path_type_and_size() {
    let fixture = playtest_fixture("validation");
    let source = fs::read(fixture.path().join("Tests/default.json")).unwrap();
    let mut invalids = vec![(b"{\ninvalid".to_vec(), "playtest_json_invalid", None)];
    for (field, value, expected) in [
        (
            "initialSceneId",
            serde_json::json!("unknown"),
            "playtest_scene_missing",
        ),
        (
            "path",
            serde_json::json!("hud.text"),
            "playtest.observation_path_unknown",
        ),
        (
            "equals",
            serde_json::json!(1),
            "playtest.observation_type_mismatch",
        ),
    ] {
        let mut json: serde_json::Value = serde_json::from_slice(&source).unwrap();
        if field == "initialSceneId" {
            json[field] = value;
        } else {
            json["assertions"][0][field] = value;
        }
        invalids.push((serde_json::to_vec(&json).unwrap(), expected, Some(field)));
    }
    invalids.push((
        vec![b' '; runtime_player_winit::semantic_outcome::MAX_PLAYTEST_SCENARIO_BYTES + 1],
        "playtest_source_too_large",
        None,
    ));
    for (bytes, code, field) in invalids {
        fs::write(fixture.path().join("Tests/default.json"), bytes).unwrap();
        let mut session =
            project_authoring_execution::ProjectAuthoringSession::open(fixture.path()).unwrap();
        let lease = full_lease(&mut session, "playtest-invalid");
        let prepared = GameProjectCompiler::bind(&lease)
            .unwrap()
            .prepare(&lease, TargetProfile::WindowsDev)
            .unwrap();
        let error = prepared.load_playtest_scenario().unwrap_err();
        assert!(error.code().ends_with(code), "{}", error.code());
        let location = error.source_location().unwrap();
        assert_eq!(location.source_path, "Tests/default.json");
        if let Some(field) = field {
            assert!(location.field_path.as_ref().unwrap().ends_with(field));
        }
        if code == "playtest_json_invalid" {
            assert_eq!(location.line, Some(2));
        }
    }
}

#[test]
fn playtest_scenario_changed_source_gets_new_digest_and_does_not_require_playtest_for_prepare() {
    let fixture = playtest_fixture("new-digest");
    let mut session =
        project_authoring_execution::ProjectAuthoringSession::open(fixture.path()).unwrap();
    let lease = full_lease(&mut session, "playtest-before");
    let first = GameProjectCompiler::bind(&lease)
        .unwrap()
        .prepare(&lease, TargetProfile::WindowsDev)
        .unwrap()
        .load_playtest_scenario()
        .unwrap();
    let mut json: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.path().join("Tests/default.json")).unwrap())
            .unwrap();
    json["assertions"][0]["equals"] = serde_json::json!(false);
    fs::write(
        fixture.path().join("Tests/default.json"),
        serde_json::to_vec(&json).unwrap(),
    )
    .unwrap();
    session.refresh().unwrap();
    let lease = full_lease(&mut session, "playtest-after");
    let second = GameProjectCompiler::bind(&lease)
        .unwrap()
        .prepare(&lease, TargetProfile::WindowsDev)
        .unwrap()
        .load_playtest_scenario()
        .unwrap();
    assert_ne!(first.source_digest(), second.source_digest());
    assert_ne!(
        first.lineage().revision_id(),
        second.lineage().revision_id()
    );
    let ordinary = Fixture::new("no-playtest", "compiler.ordinary");
    let mut session =
        project_authoring_execution::ProjectAuthoringSession::open(ordinary.path()).unwrap();
    let lease = full_lease(&mut session, "ordinary-prepare");
    let prepared = GameProjectCompiler::bind(&lease)
        .unwrap()
        .prepare(&lease, TargetProfile::WindowsDev)
        .unwrap();
    assert!(prepared.is_runtime_package_ready());
    assert!(prepared
        .load_playtest_scenario()
        .unwrap_err()
        .code()
        .ends_with("playtest_reference_missing"));
}

#[test]
fn compiler_test_fixtures_remain_isolated_when_clock_samples_repeat() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let first = Fixture::with_manifest_at_stamp("clock-collision", b"first", stamp);
    let second = Fixture::with_manifest_at_stamp("clock-collision", b"second", stamp);
    assert_ne!(
        first.path(),
        second.path(),
        "Repeated timestamps must not alias test projects"
    );
    assert_eq!(
        fs::read(first.path().join("project.aife.json")).unwrap(),
        b"first"
    );
}

fn playtest_fixture(label: &str) -> Fixture {
    let fixture = Fixture::with_manifest(label, br#"{"schemaVersion":"aife-project.v2","projectId":"compiler.playtest.fixture","playtestScenario":"Tests/default.json","observationContract":"Observations/contract.json"}"#);
    for dir in ["Tests", "Scenes", "Observations"] {
        fs::create_dir_all(fixture.path().join(dir)).unwrap();
    }
    fs::write(
        fixture.path().join("Scenes/Main.scene.json"),
        br#"{"id":"scene-main","entities":[]}"#,
    )
    .unwrap();
    fs::write(fixture.path().join("Observations/contract.json"), br#"{"schemaVersion":"project-observation-contract.v1","contractId":"game.observations","observations":[{"path":"game.ready","type":"bool","description":"Ready"}]}"#).unwrap();
    fs::write(fixture.path().join("Tests/default.json"), br#"{
        "schemaVersion":"playtest-scenario.v1", "scenarioId":"scenario.ready", "initialSceneId":"scene-main", "target":"windows-headless",
        "maxSimulationTicks":3, "maxPresentationFrames":6, "timeoutMs":1000,
        "assertions":[{"assertionId":"ready","fromSimulationTick":2,"throughSimulationTick":3,"path":"game.ready","equals":true}]
    }"#).unwrap();
    fixture
}

impl Fixture {
    fn new(label: &str, project_id: &str) -> Self {
        let manifest =
            format!(r#"{{"schemaVersion":"aife-project.v2","projectId":"{project_id}"}}"#);
        let fixture = Self::with_manifest(label, manifest.as_bytes());
        fs::create_dir_all(fixture.path().join("Game")).expect("create source directory");
        fs::write(
            fixture.path().join("Game/main.rs"),
            b"fn game() { let _revision = 1; }",
        )
        .expect("write source");
        fixture
    }

    fn with_manifest(label: &str, manifest: &[u8]) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        Self::with_manifest_at_stamp(label, manifest, unique)
    }

    fn with_manifest_at_stamp(label: &str, manifest: &[u8], unique: u128) -> Self {
        static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let sequence = SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "aife-game-project-compiler-{label}-{}-{unique}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("exclusively create fixture");
        fs::write(root.join("project.aife.json"), manifest).expect("write manifest");
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
