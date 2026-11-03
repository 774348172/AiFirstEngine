//! S1-H direct typed-tool consumer of retained Player provenance.
use project_authoring_execution::foundation_player_observation;
use runtime_player_winit::{NativePlayerWindowRunRequest, NativeWindowHostReport};
use serde_json::json;

mod s5a {
    use engine_tool_provider::{
        CanonicalToolResult, CanonicalToolStatus, EngineToolProvider, HostSessionContext,
        HostToolCall, NativeEngineToolProvider,
    };
    use serde_json::{json, Value};
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT: AtomicU64 = AtomicU64::new(1);
    struct Project {
        root: PathBuf,
        provider: EngineToolProvider,
        calls: u64,
    }
    impl Project {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "aife-371-s5-tools-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            for dir in ["Assets", "Models", "BuildProfiles"] {
                fs::create_dir(root.join(dir)).unwrap();
            }
            fs::write(
                root.join("project.aife.json"),
                br#"{"schemaVersion":"aife-project.v3","projectId":"371.s5.tools"}"#,
            )
            .unwrap();
            for (path, bytes) in [
                ("Assets/character.import.json", include_bytes!("../../../../samples/platform_3d_world_project/Assets/character.import.json").as_slice()),
                ("Models/character.gltf", include_bytes!("../../../../samples/platform_3d_world_project/Models/character.gltf").as_slice()),
                ("Models/character.bin", include_bytes!("../../../../samples/platform_3d_world_project/Models/character.bin").as_slice()),
                ("BuildProfiles/windows.release.json", include_bytes!("../../../../samples/platform_3d_world_project/BuildProfiles/windows.release.json").as_slice()),
            ] { fs::write(root.join(path), bytes).unwrap(); }
            let provider = EngineToolProvider::attach(HostSessionContext {
                session_id: format!("s5-{}", root.file_name().unwrap().to_string_lossy()),
                workspace_root: root.clone(),
                project_root: Some(root.clone()),
            })
            .unwrap();
            Self {
                root,
                provider,
                calls: 0,
            }
        }
        fn call(&mut self, tool: &str, arguments: Value) -> CanonicalToolResult {
            self.calls += 1;
            self.provider.invoke(HostToolCall {
                call_id: format!("s5-call-{}", self.calls),
                tool_name: tool.into(),
                arguments,
                approved: true,
            })
        }
        fn read(&self, path: &str) -> Value {
            serde_json::from_slice(&fs::read(self.root.join(path)).unwrap()).unwrap()
        }
        fn write(&self, path: &str, value: &Value) {
            fs::write(self.root.join(path), serde_json::to_vec(value).unwrap()).unwrap();
        }
    }
    impl Drop for Project {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    fn copy_source_tree(from: &std::path::Path, to: &std::path::Path) {
        fs::create_dir_all(to).unwrap();
        for item in fs::read_dir(from).unwrap() {
            let item = item.unwrap();
            if item.file_type().unwrap().is_dir() {
                copy_source_tree(&item.path(), &to.join(item.file_name()));
            } else {
                fs::copy(item.path(), to.join(item.file_name())).unwrap();
            }
        }
    }

    #[test]
    fn wrong_character_clip_and_joint_binding_locate_authored_fields_and_recover() {
        let mut project = Project::new();
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../samples/platform_3d_action_project");
        for directory in [
            "Assets",
            "Models",
            "Scenes",
            "Prefabs",
            "AUI",
            "Input",
            "BuildProfiles",
            "Observations",
            "Tests",
            "Rules",
        ] {
            if source.join(directory).is_dir() {
                copy_source_tree(&source.join(directory), &project.root.join(directory));
            }
        }
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(source.join("project.aife.json")).unwrap()).unwrap();
        manifest.as_object_mut().unwrap().remove("runtimeModule");
        manifest["projectId"] = json!("371.s5.tools");
        project.write("project.aife.json", &manifest);
        let original = project.read("Scenes/main.json");
        let actor = original["entities"]
            .as_array()
            .unwrap()
            .iter()
            .position(|e| e["id"] == "walker")
            .unwrap();
        let component = original["entities"][actor]["components"]
            .as_array()
            .unwrap()
            .iter()
            .position(|c| c["componentType"] == "engine.character3d")
            .unwrap();
        for (suffix, value) in [
            ("/graph/states/0/clip", json!("unknown-clip")),
            ("/joints/1/entityRef", json!("floor")),
            ("/retarget/mapping/sourceAsset", json!("wrong-rig")),
        ] {
            let pointer = format!("/entities/{actor}/components/{component}/data{suffix}");
            let mut invalid = original.clone();
            if suffix.starts_with("/retarget") {
                let binding = &mut invalid["entities"][actor]["components"][component]["data"];
                let reference = binding["character"].clone();
                let id = reference["id"].clone();
                binding["retarget"] = json!({"source":reference,"mapping":{"sourceAsset":id,"targetAsset":id,"joints":[{"source":"root","target":"root","translationScale":1.0}]}});
            }
            *invalid.pointer_mut(&pointer).unwrap() = value;
            project.write("Scenes/main.json", &invalid);
            let rejected = project.call(
                "engine_project_check",
                json!({"targetProfile":"windows-release"}),
            );
            assert_eq!(
                rejected.status,
                CanonicalToolStatus::RejectedByEngine,
                "{rejected:#?}"
            );
            let location = rejected.diagnostics[0]
                .source_location
                .as_ref()
                .expect("authored source location");
            assert_eq!(location.source_path, "Scenes/main.json", "{rejected:#?}");
            assert_eq!(location.field_path.as_deref(), Some(pointer.as_str()));
            assert!(!location.generated);
            assert_eq!(
                rejected.retryability,
                engine_tool_provider::ToolRetryability::RetryAfterCorrection
            );
            project.write("Scenes/main.json", &original);
            let repaired = project.call(
                "engine_project_check",
                json!({"targetProfile":"windows-release"}),
            );
            assert_eq!(
                repaired.status,
                CanonicalToolStatus::Completed,
                "{repaired:#?}"
            );
        }
    }

    #[test]
    fn inspect_and_check_bind_domain_summary_to_the_same_snapshot() {
        let mut project = Project::new();
        let inspected = project.call("engine_project_inspect", json!({}));
        assert_eq!(
            inspected.status,
            CanonicalToolStatus::Completed,
            "{inspected:#?}"
        );
        let checked = project.call(
            "engine_project_check",
            json!({"targetProfile":"windows-release"}),
        );
        assert_eq!(
            checked.status,
            CanonicalToolStatus::Completed,
            "{checked:#?}"
        );
        assert_eq!(inspected.project_revision, checked.project_revision);
        assert_eq!(
            inspected.output["spatialInspection"],
            checked.output["checkReport"]["spatialInspection"]
        );
        assert_eq!(
            inspected.output["spatialInspection"]["assets"][0]["sourcePath"],
            "Assets/character.import.json"
        );
        assert_eq!(
            inspected.output["spatialInspection"]["qualification"],
            "not_assessed_by_inspect"
        );
    }

    #[test]
    fn missing_lit_normals_preserve_source_field_and_recover_after_repair() {
        let mut project = Project::new();
        let original = project.read("Models/character.gltf");
        let mut model = original.clone();
        model["materials"][0]
            .as_object_mut()
            .unwrap()
            .remove("extensions");
        model["meshes"][0]["primitives"][0]["attributes"]
            .as_object_mut()
            .unwrap()
            .remove("NORMAL");
        project.write("Models/character.gltf", &model);
        let mut settings = project.read("Assets/character.import.json");
        settings["materialStrategy"] = json!("standardLit");
        project.write("Assets/character.import.json", &settings);
        let failed = project.call(
            "engine_project_check",
            json!({"targetProfile":"windows-release"}),
        );
        assert_ne!(failed.status, CanonicalToolStatus::Completed);
        let first = failed.diagnostics.first().expect("first cause");
        assert_eq!(
            first.code, "game_project_compiler.three_d.material.normal_required",
            "{failed:#?}"
        );
        let location = first
            .source_location
            .as_ref()
            .expect("ordinary source location");
        assert_eq!(location.source_path, "Models/character.gltf");
        assert_eq!(
            location.field_path.as_deref(),
            Some("/meshes/0/primitives/0")
        );
        assert_eq!(
            failed.retryability,
            engine_tool_provider::ToolRetryability::RetryAfterCorrection
        );
        project.write("Models/character.gltf", &original);
        let repaired = project.call(
            "engine_project_check",
            json!({"targetProfile":"windows-release"}),
        );
        assert_eq!(
            repaired.status,
            CanonicalToolStatus::Completed,
            "{repaired:#?}"
        );
    }

    #[test]
    fn cross_asset_mutation_budget_failure_and_rollback_restore_exact_source() {
        let mut project = Project::new();
        let before_model = fs::read(project.root.join("Models/character.gltf")).unwrap();
        let before_import = fs::read(project.root.join("Assets/character.import.json")).unwrap();
        let before = project.call("engine_project_inspect", json!({}));
        let mut settings: Value = serde_json::from_slice(&before_import).unwrap();
        settings["budgets"] = serde_json::to_value(
            project_authoring_execution::three_d_contract::ImportBudgets::default(),
        )
        .unwrap();
        settings["budgets"]["sourceBytes"] = json!(1);
        let mut model: Value = serde_json::from_slice(&before_model).unwrap();
        model["materials"][0]["pbrMetallicRoughness"]["baseColorFactor"] =
            json!([0.8, 0.2, 0.1, 1.0]);
        let mutation = project.call("engine_project_mutate", json!({"goal":"change material and import budget", "expectedRevisionId":before.project_revision, "changes":[
            {"operation":"create_or_replace", "path":"Assets/character.import.json", "content":serde_json::to_string(&settings).unwrap()},
            {"operation":"create_or_replace", "path":"Models/character.gltf", "content":serde_json::to_string(&model).unwrap()}
        ]}));
        assert_eq!(
            mutation.status,
            CanonicalToolStatus::Completed,
            "{mutation:#?}"
        );
        assert_eq!(mutation.output["changedPaths"].as_array().unwrap().len(), 2);
        let failed = project.call(
            "engine_project_check",
            json!({"targetProfile":"windows-release"}),
        );
        assert_ne!(failed.status, CanonicalToolStatus::Completed);
        let first = failed.diagnostics.first().unwrap();
        assert!(
            first.code.ends_with("source_budget_exceeded"),
            "{failed:#?}"
        );
        let location = first.source_location.as_ref().expect("budget source");
        assert_eq!(location.source_path, "Assets/character.import.json");
        assert_eq!(location.field_path.as_deref(), Some("/source"));
        assert_eq!(
            failed.retryability,
            engine_tool_provider::ToolRetryability::RetryAfterCorrection
        );
        let rollback = project.call(
            "engine_project_rollback",
            json!({"rollbackRef":mutation.receipt_ref.unwrap()}),
        );
        assert_eq!(
            rollback.status,
            CanonicalToolStatus::Completed,
            "{rollback:#?}"
        );
        assert_eq!(rollback.project_revision, before.project_revision);
        assert_eq!(
            fs::read(project.root.join("Models/character.gltf")).unwrap(),
            before_model
        );
        assert_eq!(
            fs::read(project.root.join("Assets/character.import.json")).unwrap(),
            before_import
        );
        let repaired = project.call(
            "engine_project_check",
            json!({"targetProfile":"windows-release"}),
        );
        assert_eq!(
            repaired.status,
            CanonicalToolStatus::Completed,
            "{repaired:#?}"
        );
    }
    #[cfg(windows)]
    #[test]
    #[ignore = "explicit owned ordinary project; real nine-tool Compiler/ProjectModule/Player workflow"]
    fn ordinary_project_nine_tools_modify_playtest_observe_and_rollback() {
        let root =
            PathBuf::from(std::env::var_os("AIFE_S5_TOOL_PROJECT").expect("owned S5 project"));
        let evidence =
            PathBuf::from(std::env::var_os("AIFE_S5_TOOL_EVIDENCE").expect("fresh S5 evidence"));
        fs::create_dir(&evidence).unwrap();
        let mut provider = EngineToolProvider::attach(HostSessionContext {
            session_id: format!("s5-real-{}", std::process::id()),
            workspace_root: root.clone(),
            project_root: Some(root.clone()),
        })
        .unwrap();
        let mut call = |name: &str, tool: &str, arguments: Value| {
            let start = std::time::Instant::now();
            let result = provider.invoke(HostToolCall {
                call_id: name.into(),
                tool_name: tool.into(),
                arguments,
                approved: true,
            });
            fs::write(
                evidence.join(format!("{name}.json")),
                serde_json::to_vec_pretty(&result).unwrap(),
            )
            .unwrap();
            println!(
                "{name}: {:?} {:.3}s",
                result.status,
                start.elapsed().as_secs_f64()
            );
            assert_eq!(
                result.status,
                CanonicalToolStatus::Completed,
                "{name}: {result:#?}"
            );
            result
        };
        let before = call("01-inspect", "engine_project_inspect", json!({}));
        call(
            "02-check",
            "engine_project_check",
            json!({"targetProfile":"windows-release"}),
        );
        let baseline = call(
            "03-build",
            "engine_project_build",
            json!({"targetProfile":"windows-release","frameLimit":3,"timeoutMs":600000}),
        );
        let played = call(
            "04-playtest",
            "engine_runtime_playtest",
            json!({"deliveryRef":baseline.output["deliveryRef"]}),
        );
        assert_eq!(played.output["overall"], "passed", "{played:#?}");
        let observed = call(
            "05-observe",
            "engine_runtime_observe",
            json!({"runRef":played.output["runRef"]}),
        );
        assert_eq!(observed.output, played.output);
        let paths = [
            "Assets/amber.json",
            "Models/character.gltf",
            "Prefabs/S4Environment/tree.prefab.json",
        ];
        let originals = paths.map(|path| fs::read(root.join(path)).unwrap());
        let mut changed: Vec<Value> = originals
            .iter()
            .map(|bytes| serde_json::from_slice(bytes).unwrap())
            .collect();
        changed[0]["baseColor"] = json!([0.3, 0.8, 0.1, 1.0]);
        changed[1]["animations"][1]["samplers"][0]["interpolation"] = json!("STEP");
        changed[2]["entities"][0]["components"][0]["data"]["translation"][1] = json!(0.05);
        let changes: Vec<Value> = paths.iter().zip(&changed).map(|(path,value)| json!({"operation":"create_or_replace", "path":path, "content":serde_json::to_string_pretty(value).unwrap()})).collect();
        let mutation = call(
            "06-mutate",
            "engine_project_mutate",
            json!({"goal":"change named material, animation sampler and loaded cell layout", "expectedRevisionId":before.project_revision, "changes":changes}),
        );
        assert_eq!(mutation.output["changedPaths"].as_array().unwrap().len(), 3);
        let modified = call(
            "07-build-modified",
            "engine_project_build",
            json!({"targetProfile":"windows-release","frameLimit":3,"timeoutMs":600000}),
        );
        assert_ne!(modified.project_revision, baseline.project_revision);
        let modified_play = call(
            "08-playtest-modified",
            "engine_runtime_playtest",
            json!({"deliveryRef":modified.output["deliveryRef"]}),
        );
        assert_eq!(
            modified_play.output["overall"], "passed",
            "{modified_play:#?}"
        );
        // Retained observation cannot adopt either the new source or new delivery.
        let retained = call(
            "09-observe-baseline",
            "engine_runtime_observe",
            json!({"runRef":played.output["runRef"]}),
        );
        assert_eq!(retained.output, played.output);
        let rolled = call(
            "10-rollback",
            "engine_project_rollback",
            json!({"rollbackRef":mutation.receipt_ref.unwrap()}),
        );
        assert_eq!(rolled.project_revision, before.project_revision);
        for (path, original) in paths.iter().zip(&originals) {
            assert_eq!(fs::read(root.join(path)).unwrap(), *original);
        }
        let restored = call(
            "11-build-restored",
            "engine_project_build",
            json!({"targetProfile":"windows-release","frameLimit":3,"timeoutMs":600000}),
        );
        assert_eq!(restored.project_revision, baseline.project_revision);
        call(
            "12-verify",
            "engine_delivery_verify",
            json!({"deliveryRef":restored.output["deliveryRef"],"mode":"windowed","frameLimit":60,"timeoutMs":60000,"screenshot":true}),
        );
        call(
            "13-run",
            "engine_runtime_run",
            json!({"mode":"headless","frameLimit":3,"timeoutMs":60000}),
        );
        fs::write(evidence.join("summary.json"), serde_json::to_vec_pretty(&json!({"status":"passed", "qualifiedTools":9, "baselineDelivery":baseline.output, "modifiedDelivery":modified.output, "restoredDelivery":restored.output, "sourceRevision":before.project_revision})).unwrap()).unwrap();
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "explicit ordinary S5 gameplay project; real Release Compiler and windowed playtest"]
    fn ordinary_gameplay_project_build_playtest_and_observe() {
        let root =
            PathBuf::from(std::env::var_os("AIFE_S5_GAMEPLAY_PROJECT").expect("ordinary project"));
        let evidence =
            PathBuf::from(std::env::var_os("AIFE_S5_GAMEPLAY_EVIDENCE").expect("fresh evidence"));
        fs::create_dir(&evidence).unwrap();
        let mut provider = EngineToolProvider::attach(HostSessionContext {
            session_id: format!("s5-gameplay-{}", std::process::id()),
            workspace_root: root.clone(),
            project_root: Some(root),
        })
        .unwrap();
        let mut call = |name: &str, tool: &str, arguments: Value| {
            let start = std::time::Instant::now();
            let result = provider.invoke(HostToolCall {
                call_id: name.into(),
                tool_name: tool.into(),
                arguments,
                approved: true,
            });
            fs::write(
                evidence.join(format!("{name}.json")),
                serde_json::to_vec_pretty(&result).unwrap(),
            )
            .unwrap();
            println!(
                "{name}: {:?} {:.3}s",
                result.status,
                start.elapsed().as_secs_f64()
            );
            assert_eq!(result.status, CanonicalToolStatus::Completed, "{result:#?}");
            result
        };
        call(
            "01-check",
            "engine_project_check",
            json!({"targetProfile":"windows-release"}),
        );
        let built = call(
            "02-build",
            "engine_project_build",
            json!({"targetProfile":"windows-release","frameLimit":3,"timeoutMs":600000}),
        );
        let played = call(
            "03-playtest",
            "engine_runtime_playtest",
            json!({"deliveryRef":built.output["deliveryRef"]}),
        );
        assert_eq!(played.output["overall"], "passed", "{played:#?}");
        let capture = played.output["foundationObservation"]["capture"]["path"]
            .as_str()
            .expect("real Player capture");
        let player: Value = serde_json::from_slice(
            &fs::read(PathBuf::from(capture).parent().unwrap().join("player.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(player["aui"]["status"], "success", "{player:#?}");
        assert!(player["aui"]["drawItemCount"].as_u64().unwrap() >= 3);
        assert!(player["aui"]["textCommandCount"].as_u64().unwrap() >= 2);
        assert_eq!(player["aui"]["glyphPresent"], true);
        let observed = call(
            "04-observe",
            "engine_runtime_observe",
            json!({"runRef":played.output["runRef"]}),
        );
        assert_eq!(observed.output, played.output);
        fs::write(
            evidence.join("summary.json"),
            serde_json::to_vec_pretty(
                &json!({"status":"passed","delivery":built.output,"run":played.output}),
            )
            .unwrap(),
        )
        .unwrap();
    }
}
fn player() -> NativeWindowHostReport {
    let mut p =
        NativeWindowHostReport::base(&NativePlayerWindowRunRequest::windowed("retained/package"));
    p.run_id = "frozen-runtime-run".into();
    p.frames_completed = 12;
    p.screenshot.requested = true;
    p.screenshot.status = runtime_player_winit::NativeWindowScreenshotStatus::Captured;
    p.screenshot.path = Some("sealed/capture-7.png".into());
    p.screenshot.byte_size = Some(16);
    p.screenshot.frame_identity = Some(serde_json::from_value(json!({
        "source":{"runId":"frozen-runtime-run","runEpoch":1,"sessionId":"session","packageContentHash":"sha256:frozen","worldEpoch":2},
        "frameIndex":7,"tickPair":[6,7],"presentationAlphaBits":0,
        "views":[{"viewId":1,"cameraEntityId":"camera","viewVersion":2,"historyVersion":3,"originBits":[0,0,0]}]
    })).unwrap());
    p.screenshot.submission = Some(
        serde_json::from_value(
            json!({"deviceEpoch":1,"serial":1,"frameIndex":7,"contentVersion":1}),
        )
        .unwrap(),
    );
    p
}
#[test]
fn foundation_missing_dimensions_are_unavailable_not_qualified() {
    let p =
        NativeWindowHostReport::base(&NativePlayerWindowRunRequest::windowed("retained/package"));
    let o = foundation_player_observation(&p).unwrap();
    assert_eq!(o["actualDisplay"]["status"], "unavailable");
    assert_eq!(o["actualDisplay"]["qualified"], false);
    assert_eq!(o["traceIdentityStatus"], "unavailable");
    assert!(o["resources"].is_null());
}
#[test]
fn foundation_capture_keeps_its_earlier_frame_and_frozen_revision() {
    let p = player();
    let o = foundation_player_observation(&p).unwrap();
    assert_eq!(o["capture"]["frameIdentity"]["frameIndex"], 7);
    assert_eq!(o["framesCompleted"], 12);
    assert_eq!(o["captureIdentityStatus"], "observed");
    assert_eq!(
        o["capture"]["frameIdentity"]["source"]["packageContentHash"],
        "sha256:frozen"
    );
    assert_eq!(o["qualification"], "not_assessed_by_observe");
}
#[test]
fn foundation_capture_rejects_wrong_run_frame_epoch_and_invalid_alpha() {
    let p = player();
    for (path, value) in [
        ("/screenshot/submission/deviceEpoch", json!(0)),
        ("/screenshot/submission/serial", json!(0)),
        ("/screenshot/frameIdentity/source/runId", json!("other-run")),
        ("/screenshot/frameIdentity/source/worldEpoch", json!(0)),
        ("/screenshot/frameIdentity/frameIndex", json!(8)),
        (
            "/screenshot/frameIdentity/presentationAlphaBits",
            json!(0x7fc00000u32),
        ),
        ("/screenshot/frameIdentity/tickPair", json!([8, 7])),
    ] {
        let mut v = serde_json::to_value(&p).unwrap();
        *v.pointer_mut(path).unwrap() = value;
        assert!(
            foundation_player_observation(&serde_json::from_value(v).unwrap()).is_err(),
            "{path}"
        );
    }
}
#[test]
fn foundation_capture_rejects_missing_submission_duplicate_views_and_nonfinite_origin() {
    let mut p = player();
    p.screenshot.submission = None;
    assert!(foundation_player_observation(&p).is_err());
    let p = player();
    let mut v = serde_json::to_value(&p).unwrap();
    let view = v["screenshot"]["frameIdentity"]["views"][0].clone();
    v["screenshot"]["frameIdentity"]["views"] = json!([view.clone(), view]);
    assert!(foundation_player_observation(&serde_json::from_value(v).unwrap()).is_err());
    let mut v = serde_json::to_value(&p).unwrap();
    v["screenshot"]["frameIdentity"]["views"][0]["originBits"][0] = json!(f64::INFINITY.to_bits());
    assert!(foundation_player_observation(&serde_json::from_value(v).unwrap()).is_err());
}
#[test]
fn foundation_observe_stays_an_independent_retained_reference_tool() {
    let tools = engine_tool_provider::tool_definitions();
    let observe = tools
        .iter()
        .find(|t| t.name == "engine_runtime_observe")
        .unwrap();
    assert_eq!(observe.input_schema["required"], json!(["runRef"]));
    assert_eq!(observe.input_schema["additionalProperties"], false);
    assert!(tools.iter().any(|t| t.name == "engine_runtime_playtest"));
    assert!(tools.iter().any(|t| t.name == "engine_project_inspect"));
}

#[test]
#[ignore = "projects an explicitly supplied retained real Player report without spawning or rebuilding"]
fn foundation_retained_real_player_projection() {
    let input = std::env::var_os("AIFE_FOUNDATION_RETAINED_PLAYER_REPORT")
        .expect("explicit retained report required");
    let output =
        std::env::var_os("AIFE_FOUNDATION_PROJECTION_OUTPUT").expect("fresh output required");
    let bytes = std::fs::read(input).unwrap();
    let player: NativeWindowHostReport = serde_json::from_slice(&bytes).unwrap();
    let projection = foundation_player_observation(&player).unwrap();
    assert_eq!(projection["runtimeRunId"], player.run_id);
    assert_eq!(projection["actualDisplay"]["qualified"], false);
    assert_eq!(projection["traceIdentityStatus"], "observed");
    if player
        .frame_performance_summary
        .as_ref()
        .and_then(|s| s.gpu.as_ref())
        .and_then(|g| g.native_present.as_ref())
        .is_some_and(|n| n.frames.iter().all(|f| f.actual_present_time_ns.is_none()))
    {
        assert_eq!(projection["actualDisplay"]["status"], "unavailable");
    }
    // A capture may precede the final Trace frame, but must agree with the
    // retained Trace identity of its own frame, including the World epoch.
    let frames = &player
        .frame_performance_summary
        .as_ref()
        .unwrap()
        .raw_cpu_frames;
    let first = frames.first().unwrap().frame_identity.as_ref().unwrap();
    let last = frames.last().unwrap().frame_identity.as_ref().unwrap();
    assert!(first.frame_index < last.frame_index);
    let mut captured = player.clone();
    captured.screenshot.status = runtime_player_winit::NativeWindowScreenshotStatus::Captured;
    captured.screenshot.frame_identity = Some(first.clone());
    captured.screenshot.submission = Some(
        serde_json::from_value(json!({
            "deviceEpoch":1,"serial":1,"frameIndex":first.frame_index,"contentVersion":1
        }))
        .unwrap(),
    );
    assert!(foundation_player_observation(&captured).is_ok());
    let mut foreign = (**first).clone();
    std::sync::Arc::make_mut(&mut foreign.source).world_epoch += 1;
    captured.screenshot.frame_identity = Some(std::sync::Arc::new(foreign));
    assert!(
        foundation_player_observation(&captured).is_err(),
        "earlier capture cannot substitute another World epoch for its retained Trace frame"
    );
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .unwrap();
    use std::io::Write;
    file.write_all(&serde_json::to_vec_pretty(&projection).unwrap())
        .unwrap();
}

#[cfg(windows)]
#[test]
#[ignore = "explicit owned ordinary 3D project; real Release build/playtest/retained observe"]
fn foundation_ordinary_project_tools_keep_the_real_run_capture_and_frozen_source() {
    use engine_tool_provider::{
        CanonicalToolStatus, HostSessionContext, HostToolCall, NativeHostAdapter,
    };
    use std::{fs, path::PathBuf};
    let root = PathBuf::from(
        std::env::var_os("AIFE_FOUNDATION_TOOL_PROJECT").expect("owned ordinary project"),
    );
    let output =
        PathBuf::from(std::env::var_os("AIFE_FOUNDATION_TOOL_EVIDENCE").expect("fresh evidence"));
    fs::create_dir(&output).unwrap();
    let mut provider = NativeHostAdapter::attach(HostSessionContext {
        session_id: format!(
            "371-foundation-real-tools-{}",
            output.file_name().unwrap().to_string_lossy()
        ),
        workspace_root: root.clone(),
        project_root: Some(root.clone()),
    })
    .unwrap();
    let mut call = |id: &str, tool: &str, arguments| {
        let result = provider.invoke(HostToolCall {
            call_id: id.into(),
            tool_name: tool.into(),
            arguments,
            approved: true,
        });
        fs::write(
            output.join(format!("{id}.json")),
            serde_json::to_vec_pretty(&result).unwrap(),
        )
        .unwrap();
        assert_eq!(
            result.status,
            CanonicalToolStatus::Completed,
            "{id}: {result:#?}"
        );
        result
    };
    call("inspect", "engine_project_inspect", json!({}));
    call("check", "engine_project_check", json!({}));
    let built = call(
        "build",
        "engine_project_build",
        json!({"targetProfile":"windows-release","frameLimit":3}),
    );
    let played = call(
        "playtest",
        "engine_runtime_playtest",
        json!({"deliveryRef":built.output["deliveryRef"]}),
    );
    assert_eq!(played.output["overall"], "passed");
    let observation = &played.output["foundationObservation"];
    assert!(played.output["runId"].as_str().is_some());
    assert!(observation["runtimeRunId"].as_str().is_some());
    assert_ne!(
        played.output["runId"], observation["runtimeRunId"],
        "the worker receipt binds the job to the original engine run"
    );
    assert_eq!(observation["captureIdentityStatus"], "observed");
    assert_eq!(observation["actualDisplay"]["qualified"], false);
    assert_eq!(
        played.output["semantic"]["captureEvidence"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    for capture in played.output["semantic"]["captureEvidence"]
        .as_array()
        .unwrap()
    {
        assert_eq!(
            capture["presentationFrame"],
            capture["spatial"]["committedFrame"]
        );
        assert!(capture["spatial"]["view"]["version"].is_u64());
    }
    // Observation must consume the retained result after live input changes.
    // This is an exclusively owned fixture, never a user's project or delivery.
    let scenario_path = root.join("Tests/tool-foundation.scenario.json");
    let before = fs::read(&scenario_path).unwrap();
    struct RestoreScenario(PathBuf, Vec<u8>);
    impl Drop for RestoreScenario {
        fn drop(&mut self) {
            fs::write(&self.0, &self.1).expect("restore owned scenario input");
        }
    }
    let mut changed: serde_json::Value = serde_json::from_slice(&before).unwrap();
    changed["assertions"][0]["equals"] = json!(999);
    let restore = RestoreScenario(scenario_path.clone(), before);
    fs::write(&scenario_path, serde_json::to_vec(&changed).unwrap()).unwrap();
    let retained = call(
        "observe",
        "engine_runtime_observe",
        json!({"runRef":played.output["runRef"]}),
    );
    drop(restore);
    assert_eq!(
        retained.output, played.output,
        "latest project input must not replace the retained run"
    );
}
