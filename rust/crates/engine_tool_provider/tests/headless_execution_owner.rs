use engine_tool_provider::{
    CanonicalToolStatus, EngineToolProvider, HostSessionContext, HostToolCall,
    NativeEngineToolProvider,
};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(windows)]
#[test]
#[ignore = "local Windows imported model through ordinary Provider/Host/DLL/window"]
fn three_d_a2_imported_box_window_uses_project_package() {
    use sha2::{Digest, Sha256};
    let root = unique_temp_dir("provider-three-d-a22");
    let _guard = TestDirectoryGuard(root.clone());
    let project = root.join("project");
    copy_source_tree(
        &workspace_root().join("samples/three_d_lab_project"),
        &project,
    );
    let project = fs::canonicalize(project).unwrap();
    let evidence = std::env::var_os("AIFE_A22_EVIDENCE_ROOT").map(PathBuf::from);
    if let Some(path) = &evidence {
        fs::create_dir_all(path).unwrap();
    }
    let save = |name: &str, value: &Value| {
        if let Some(path) = &evidence {
            fs::write(
                path.join(format!("{name}.json")),
                serde_json::to_vec_pretty(value).unwrap(),
            )
            .unwrap();
        }
    };
    let mut provider = EngineToolProvider::attach(HostSessionContext {
        session_id: "three-d-a22".into(),
        workspace_root: project.clone(),
        project_root: Some(project.clone()),
    })
    .unwrap();
    let inspect = provider.invoke(approved_call(
        "import-inspect",
        "engine_project_inspect",
        json!({}),
    ));
    save("inspect", &serde_json::to_value(&inspect).unwrap());
    assert_completed("import inspect", &inspect);
    let check = provider.invoke(approved_call(
        "import-check",
        "engine_project_check",
        json!({}),
    ));
    save("check", &serde_json::to_value(&check).unwrap());
    assert_completed("import check", &check);
    let imports = check.output["checkReport"]["modelImports"]
        .as_array()
        .unwrap();
    assert_eq!(imports.len(), 1);
    assert_eq!(imports[0]["status"], "degraded");
    assert_eq!(imports[0]["assemblyAvailable"], true);
    assert_eq!(imports[0]["vertices"], 24);
    assert_eq!(imports[0]["indices"], 36);
    let build = provider.invoke(approved_call(
        "import-build",
        "engine_project_build",
        json!({"targetProfile":"windows-dev","frameLimit":3}),
    ));
    save("build", &serde_json::to_value(&build).unwrap());
    assert_completed("import build", &build);
    assert_project_delivery(&project, &build.output);
    let package = PathBuf::from(build.output["packageDir"].as_str().unwrap());
    let verify=provider.invoke(approved_call("import-window","engine_delivery_verify",json!({"deliveryRef":build.output["deliveryRef"],"mode":"windowed","frameLimit":120,"timeoutMs":30000,"screenshot":true})));
    save("verify", &serde_json::to_value(&verify).unwrap());
    if let Some(path) = &evidence {
        copy_source_tree(&package.join("reports"), &path.join("reports"));
        fs::copy(
            package.join("package-manifest.json"),
            path.join("package-manifest.json"),
        )
        .unwrap();
        fs::copy(
            package.join("data/runtime_package/manifest.json"),
            path.join("runtime-manifest.json"),
        )
        .unwrap();
    }
    assert_completed("import window", &verify);
    assert_eq!(verify.output["status"], "passed");
    assert_eq!(
        verify.output["artifactIdentity"],
        build.output["artifactIdentity"]
    );
    assert_eq!(
        verify.output["deliveryIdentity"],
        build.output["deliveryIdentity"]
    );
    let report: Value = serde_json::from_slice(
        &fs::read(package.join("reports/exported-player-process-verification-report.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(report["screenshotStatus"], "captured");
    assert_eq!(report["childPresentStatus"], "presented");
    let child: Value = serde_json::from_slice(
        &fs::read(package.join("reports/windowed-player-run-report.json")).unwrap(),
    )
    .unwrap();
    let diagnostics = child["diagnostics"].as_array().unwrap();
    assert!(diagnostics.iter().any(
        |d| d["code"] == "native_host.engine_runtime.execution_owner"
            && d["layer"] == "engine_runtime.dll"
    ));
    assert!(diagnostics
        .iter()
        .any(|d| d["code"] == "native_host.graphics.backend_selected" && d["message"] == "Vulkan"));
    let manifest: Value =
        serde_json::from_slice(&fs::read(package.join("package-manifest.json")).unwrap()).unwrap();
    let mut binaries = serde_json::Map::new();
    for (relative, field) in [
        ("Game.exe", "playerArtifactHash"),
        ("engine_runtime.dll", "engineRuntimeHash"),
        (
            "data/bin/sample_three_d_lab_runtime.dll",
            "projectRuntimeModuleHash",
        ),
    ] {
        let hash = format!(
            "sha256:{:x}",
            Sha256::digest(fs::read(package.join(relative)).unwrap())
        );
        assert_eq!(manifest[field], hash);
        binaries.insert(relative.into(), json!(hash));
    }
    save("binaries", &json!(binaries));
    // The real Host above loaded this package. Inspect its published identity
    // records without adding a Runtime dependency to the Provider test target.
    let runtime_index: Vec<Value> = serde_json::from_slice(
        &fs::read(package.join("data/runtime_package/assets/runtime-asset-index.json")).unwrap(),
    )
    .unwrap();
    save("runtime-asset-index", &json!(runtime_index));
    for (guid, key) in [
        ("guid-imported-box-mesh", "box-mesh"),
        ("guid-imported-box-red", "box-red"),
        ("guid-imported-box-prefab", "box-prefab"),
    ] {
        let record = runtime_index
            .iter()
            .find(|record| record["assetGuid"] == guid)
            .unwrap();
        assert_eq!(record["subAssetId"], key);
        assert_eq!(record["sourceMapDebug"], "Assets/khronos-box.import.json");
    }
    let image_path = PathBuf::from(report["screenshotPath"].as_str().unwrap());
    if let Some(path) = &evidence {
        fs::copy(&image_path, path.join("imported-box-window.png")).unwrap();
    }
    let mut reader = png::Decoder::new(fs::File::open(image_path).unwrap())
        .read_info()
        .unwrap();
    let mut pixels = vec![0; reader.output_buffer_size()];
    let image = reader.next_frame(&mut pixels).unwrap();
    assert_eq!(
        (image.width, image.height, image.color_type),
        (1280, 720, png::ColorType::Rgba)
    );
    let pixel = |x: usize, y: usize| &pixels[(y * 1280 + x) * 4..(y * 1280 + x) * 4 + 4];
    // Source Box ±0.5, scale 1.4, centre (2.45,1.4,0), camera z=6:
    // 60-degree perspective projects its front-face interior around (920,195).
    // Native window target encodes linear red 0.8 to sRGB ~231.
    for y in 188..196 {
        for x in 914..922 {
            let actual = pixel(x, y);
            assert!(
                actual
                    .iter()
                    .zip([231u8, 0, 0, 255])
                    .all(|(a, b)| a.abs_diff(b) <= 3),
                "imported Box pixel {x},{y}: {actual:?}"
            );
        }
    }
    let glyphs = (32..80)
        .flat_map(|y| (32..240).map(move |x| (x, y)))
        .filter(|&(x, y)| pixel(x, y)[..3].iter().all(|c| *c > 230))
        .count();
    assert!(glyphs > 250, "AUI glyphs missing: {glyphs}");
    save(
        "pixel-results",
        &json!({"importedBoxRoi":[914,188,8,8],"expectedSrgb":[231,0,0,255],"tolerance":3,"headingGlyphPixels":glyphs,"frames":120}),
    );
}

#[cfg(windows)]
#[test]
#[ignore = "local Windows ordinary 3D project build and real Host/DLL/window qualification"]
fn three_d_a1_generated_window_uses_project_package() {
    use sha2::{Digest, Sha256};
    let root = unique_temp_dir("provider-three-d-a13");
    let _guard = TestDirectoryGuard(root.clone());
    let project = root.join("project");
    copy_source_tree(
        &workspace_root().join("samples/three_d_lab_project"),
        &project,
    );
    let project = fs::canonicalize(project).unwrap();
    let mut provider = EngineToolProvider::attach(HostSessionContext {
        session_id: "three-d-a13".into(),
        workspace_root: project.clone(),
        project_root: Some(project.clone()),
    })
    .unwrap();
    let evidence = std::env::var_os("AIFE_A13_EVIDENCE_ROOT").map(PathBuf::from);
    if let Some(path) = &evidence {
        fs::create_dir_all(path).unwrap();
    }
    let mut screenshots = Vec::new();
    let mut identities = Vec::new();
    for (label, camera_x) in [("baseline", 0.0), ("camera-shift", 1.0)] {
        let scene_path = project.join("Scenes/Main.scene.json");
        let mut scene: Value = serde_json::from_slice(&fs::read(&scene_path).unwrap()).unwrap();
        let camera = scene["entities"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|e| e["id"] == "camera")
            .unwrap();
        camera["components"][0]["data"]["translation"][0] = json!(camera_x);
        fs::write(&scene_path, serde_json::to_vec_pretty(&scene).unwrap()).unwrap();
        let inspect = provider.invoke(approved_call(
            &format!("{label}-inspect"),
            "engine_project_inspect",
            json!({}),
        ));
        assert_completed(label, &inspect);
        let check = provider.invoke(approved_call(
            &format!("{label}-check"),
            "engine_project_check",
            json!({}),
        ));
        assert_completed(label, &check);
        let build = provider.invoke(approved_call(
            &format!("{label}-build"),
            "engine_project_build",
            json!({"targetProfile":"windows-dev","frameLimit":3}),
        ));
        assert_completed(label, &build);
        assert_project_delivery(&project, &build.output);
        let package = PathBuf::from(build.output["packageDir"].as_str().unwrap());
        let verify = provider.invoke(approved_call(&format!("{label}-window"), "engine_delivery_verify", json!({"deliveryRef":build.output["deliveryRef"],"mode":"windowed","frameLimit":120,"timeoutMs":30000,"screenshot":true})));
        // Preserve first failure before the run-owned project is removed.
        if let Some(evidence) = &evidence {
            fs::write(evidence.join(format!("{label}-tools.json")), serde_json::to_vec_pretty(&json!({"inspect":inspect.output,"check":check.output,"build":build.output,"verify":verify})).unwrap()).unwrap();
            copy_source_tree(
                &package.join("reports"),
                &evidence.join(format!("{label}-reports")),
            );
            fs::copy(
                package.join("package-manifest.json"),
                evidence.join(format!("{label}-package.json")),
            )
            .unwrap();
            fs::copy(
                package.join("data/runtime_package/manifest.json"),
                evidence.join(format!("{label}-runtime.json")),
            )
            .unwrap();
        }
        assert_completed(label, &verify);
        assert_eq!(verify.output["status"], "passed", "{verify:#?}");
        assert_eq!(
            verify.output["artifactIdentity"],
            build.output["artifactIdentity"]
        );
        assert_eq!(
            verify.output["deliveryIdentity"],
            build.output["deliveryIdentity"]
        );
        let report: Value = serde_json::from_slice(
            &fs::read(package.join("reports/exported-player-process-verification-report.json"))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(report["screenshotStatus"], "captured", "{report:#?}");
        assert_eq!(report["childPresentStatus"], "presented", "{report:#?}");
        let child: Value = serde_json::from_slice(
            &fs::read(package.join("reports/windowed-player-run-report.json")).unwrap(),
        )
        .unwrap();
        let diagnostics = child["diagnostics"].as_array().unwrap();
        assert!(diagnostics.iter().any(|d| d["code"]
            == "native_host.engine_runtime.execution_owner"
            && d["layer"] == "engine_runtime.dll"));
        assert!(diagnostics
            .iter()
            .any(|d| d["code"] == "native_host.graphics.backend_selected"
                && d["message"] == "Vulkan"));
        let manifest: Value =
            serde_json::from_slice(&fs::read(package.join("package-manifest.json")).unwrap())
                .unwrap();
        let mut binaries = serde_json::Map::new();
        for (path, field) in [
            ("Game.exe", "playerArtifactHash"),
            ("engine_runtime.dll", "engineRuntimeHash"),
            (
                "data/bin/sample_three_d_lab_runtime.dll",
                "projectRuntimeModuleHash",
            ),
        ] {
            let digest = format!(
                "sha256:{:x}",
                Sha256::digest(fs::read(package.join(path)).unwrap())
            );
            assert_eq!(manifest[field], digest, "{path}");
            binaries.insert(path.into(), json!(digest));
        }
        let screenshot = PathBuf::from(report["screenshotPath"].as_str().unwrap());
        if let Some(evidence) = &evidence {
            fs::copy(&screenshot, evidence.join(format!("{label}.png"))).unwrap();
            fs::write(
                evidence.join(format!("{label}-binaries.json")),
                serde_json::to_vec_pretty(&binaries).unwrap(),
            )
            .unwrap();
        }
        let decoder = png::Decoder::new(fs::File::open(&screenshot).unwrap());
        let mut reader = decoder.read_info().unwrap();
        let mut bytes = vec![0; reader.output_buffer_size()];
        let image = reader.next_frame(&mut bytes).unwrap();
        assert_eq!(
            (image.width, image.height, image.color_type),
            (1280, 720, png::ColorType::Rgba)
        );
        bytes.truncate(image.buffer_size());
        screenshots.push(bytes);
        identities.push(build.output["deliveryIdentity"].clone());
        if label == "baseline" {
            let run = provider.invoke(approved_call(
                "first-ordinary-window-run",
                "engine_runtime_run",
                json!({"mode":"windowed","frameLimit":3,"timeoutMs":30000}),
            ));
            if let Some(evidence) = &evidence {
                fs::write(
                    evidence.join("ordinary-run.json"),
                    serde_json::to_vec_pretty(&run).unwrap(),
                )
                .unwrap();
            }
            assert_completed("ordinary window run", &run);
            assert_project_delivery(&project, &run.output);
            assert_eq!(run.output["mode"], "windowed");
            assert_eq!(run.output["playerExitCode"], 0);
            let run_package = PathBuf::from(run.output["packageDir"].as_str().unwrap());
            let run_report: Value = serde_json::from_slice(
                &fs::read(
                    run_package.join("reports/exported-player-process-verification-report.json"),
                )
                .unwrap(),
            )
            .unwrap();
            assert_eq!(run_report["mode"], "windowed");
            assert_eq!(run_report["childPresentStatus"], "presented");
            if let Some(evidence) = &evidence {
                copy_source_tree(
                    &run_package.join("reports"),
                    &evidence.join("ordinary-run-reports"),
                );
            }
        }
    }
    let pixel = |image: usize, x: usize, y: usize| -> &[u8] {
        &screenshots[image][(y * 1280 + x) * 4..(y * 1280 + x) * 4 + 4]
    };
    // Independent projection arithmetic: front face z=1, camera z=6, 60-degree
    // vertical FOV gives x=478..727/y=235..485; sample well inside each UV quadrant.
    for (x, y, expected) in [
        (520, 280, [255, 0, 0, 255]),
        (650, 280, [0, 255, 0, 255]),
        (520, 420, [0, 0, 255, 255]),
        (650, 420, [255, 255, 255, 255]),
        (780, 320, [255, 0, 255, 255]),
        (640, 550, [80, 97, 118, 255]),
    ] {
        for dy in 0..4 {
            for dx in 0..4 {
                let actual = pixel(0, x + dx, y + dy);
                assert!(
                    actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 3),
                    "ROI {x},{y}: {actual:?} != {expected:?}"
                );
            }
        }
    }
    // Header is actual AUI glyph coverage; overlay remains fixed while world moves.
    let glyphs = (32..80)
        .flat_map(|y| (32..240).map(move |x| (x, y)))
        .filter(|&(x, y)| pixel(0, x, y)[..3].iter().all(|c| *c > 230))
        .count();
    assert!(glyphs > 250, "missing AUI heading: {glyphs}");
    let red_center = |image: usize| {
        let mut sum = 0usize;
        let mut count = 0usize;
        for y in 180..480 {
            // Camera translation also reveals the right side of the cube.
            // Measure the independently predicted front-face region only.
            for x in if image == 0 { 400..610 } else { 250..490 } {
                let p = pixel(image, x, y);
                if p[0] >= 252 && p[1] <= 3 && p[2] <= 3 {
                    sum += x;
                    count += 1;
                }
            }
        }
        assert!(count > 5000, "missing textured cube");
        sum as f64 / count as f64
    };
    let displacement = red_center(0) - red_center(1);
    assert!(
        (displacement - 124.7).abs() <= 1.0,
        "camera source shift: {displacement}"
    );
    assert_ne!(identities[0], identities[1]);
    assert_ne!(
        pixel(0, 640, 550),
        pixel(0, 10, 150),
        "plane must differ from background"
    );
    if let Some(evidence) = &evidence {
        fs::write(evidence.join("pixel-results.json"), serde_json::to_vec_pretty(&json!({"headingGlyphPixels":glyphs,"cameraDisplacementPx":displacement,"roiTolerance":3,"edgeTolerancePx":1})).unwrap()).unwrap();
    }
}

#[test]
fn project_game_sdk_generated_glue_headless_run_build_delivery_verify() {
    let source = workspace_root().join("samples/complex_shooter_project");
    let root = unique_temp_dir("provider-headless-owner");
    let project = root.join("project");
    let _guard = TestDirectoryGuard(root);
    copy_source_tree(&source, &project);
    let project = fs::canonicalize(project).expect("canonical copied project root");

    let mut provider = EngineToolProvider::attach(HostSessionContext {
        session_id: "headless-owner".to_string(),
        workspace_root: project.clone(),
        project_root: Some(project.clone()),
    })
    .expect("attach provider without Editor");

    let run = provider.invoke(approved_call(
        "headless-run",
        "engine_runtime_run",
        json!({"mode":"headless","frameLimit":1,"timeoutMs":30000}),
    ));
    assert_completed("run", &run);
    assert_project_delivery(&project, &run.output);

    let build = provider.invoke(approved_call(
        "headless-build",
        "engine_project_build",
        json!({"targetProfile":"windows-dev","frameLimit":1}),
    ));
    assert_completed("build", &build);
    assert_project_delivery(&project, &build.output);
    let delivery_ref = build.output["deliveryRef"]
        .as_str()
        .expect("build returns an opaque deliveryRef")
        .to_string();

    let verify = provider.invoke(approved_call(
        "delivery-verify",
        "engine_delivery_verify",
        json!({
            "deliveryRef":delivery_ref,
            "mode":"headless",
            "frameLimit":1,
            "timeoutMs":30000,
            "screenshot":false
        }),
    ));
    assert_completed("delivery verification", &verify);
    assert_eq!(verify.output["status"], "passed");
    assert_eq!(verify.output["processExitCode"], 0);
    assert_eq!(verify.output["childPlayerExitCode"], 0);
    let absent = provider.invoke(approved_call(
        "no-scenario",
        "engine_runtime_playtest",
        json!({"deliveryRef":delivery_ref}),
    ));
    // The packaged delivery carries the project's default playtest scenario;
    // omitting an explicit scenario therefore executes that declared default.
    assert_completed("default delivery playtest", &absent);
}

#[test]
fn switch_puzzle_semantic_outcome_uses_its_own_bool_contract() {
    let root = unique_temp_dir("provider-puzzle");
    let project = root.join("project");
    let _guard = TestDirectoryGuard(root);
    copy_source_tree(
        &workspace_root().join("samples/switch_puzzle_project"),
        &project,
    );
    let manifest_path = project.join("project.aife.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["playtestScenario"] = "Tests/default.json".into();
    fs::write(manifest_path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
    fs::create_dir_all(project.join("Tests")).unwrap();
    fs::write(project.join("Tests/default.json"), serde_json::to_vec_pretty(&json!({
        "schemaVersion":"playtest-scenario.v1", "scenarioId":"puzzle.keyboard", "initialSceneId":"scene-main",
        "target":"windows-headless", "maxSimulationTicks":3, "maxPresentationFrames":6,"timeoutMs":10000,
        "inputs":[{"simulationTick":1,"keyDown":["Enter"]},{"simulationTick":2,"keyUp":["Enter"]}],
        "assertions":[{"assertionId":"solved","fromSimulationTick":3,"throughSimulationTick":3,"path":"puzzle.solved","equals":true}]
    })).unwrap()).unwrap();
    let project = fs::canonicalize(project).unwrap();
    let mut provider = EngineToolProvider::attach(HostSessionContext {
        session_id: "puzzle-owner".into(),
        workspace_root: project.clone(),
        project_root: Some(project.clone()),
    })
    .unwrap();
    let tested = provider.invoke(approved_call(
        "puzzle-playtest",
        "engine_runtime_playtest",
        json!({}),
    ));
    assert_completed("puzzle playtest", &tested);
    assert_eq!(tested.output["semantic"]["assertions"][0]["actual"], true);
    assert_eq!(tested.output["outcome"]["gameplay"], "passed");
    let aui = &tested.output["semantic"]["aui"];
    assert_eq!(aui["documentId"], "puzzle-hud");
    assert_eq!(aui["simulationTick"], 3);
    assert_eq!(aui["nodesTruncated"], false);
    assert!(aui["nodes"].as_array().unwrap().iter().any(|node| {
        node["nodeId"] == "toggle-button" && node["rect"]["width"].as_f64().unwrap() > 0.0
    }));
    let delivery = project
        .join("Library/EngineTools/Deliveries")
        .join(&tested.operation_id)
        .join("Playtest/Windows/dev");
    assert!(delivery.join("Game.exe").is_file());
    assert!(
        !delivery
            .join("reports/exported-player-process-verification-report.json")
            .exists(),
        "playtest must execute the declared semantic scenario, not an additional headless preflight"
    );
    assert_eq!(tested.output["processExitCode"], 0);
    let read = provider.invoke(HostToolCall {
        call_id: "puzzle-observe".into(),
        tool_name: "engine_runtime_observe".into(),
        arguments: json!({"runRef":tested.output["runRef"]}),
        approved: false,
    });
    assert_completed("read with no process approval", &read);
    assert_eq!(read.output, tested.output);
    fs::create_dir(project.join("second")).unwrap();
    fs::write(
        project.join("second/project.aife.json"),
        br#"{"schemaVersion":"aife-project.v2","projectId":"second"}"#,
    )
    .unwrap();
    let rebound = provider.invoke(approved_call(
        "rebind",
        "engine_project_open",
        json!({"projectRoot":"second"}),
    ));
    assert_completed("project rebind", &rebound);
    let expired = provider.invoke(approved_call(
        "old-run",
        "engine_runtime_observe",
        json!({"runRef":tested.output["runRef"]}),
    ));
    assert_eq!(
        expired.diagnostics[0].code,
        "engine_provider.run_ref_unknown"
    );
    if let Some(root) = std::env::var_os("AIFE_312_EVIDENCE") {
        fs::write(
            PathBuf::from(root).join("native-puzzle.json"),
            serde_json::to_vec_pretty(&json!({"playtest":tested,"observe":read})).unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn neon_m5_menu_uses_project_aui_ordered_input_and_sealed_observe() {
    let root = unique_temp_dir("provider-neon-m5");
    let project = root.join("project");
    let _guard = TestDirectoryGuard(root);
    copy_source_tree(&workspace_root().join("samples/neon_lab_project"), &project);
    let project = fs::canonicalize(project).unwrap();
    let mut provider = EngineToolProvider::attach(HostSessionContext {
        session_id: "neon-m5-menu".into(),
        workspace_root: project.clone(),
        project_root: Some(project),
    })
    .unwrap();
    let inspected = provider.invoke(approved_call(
        "neon-m5-inspect",
        "engine_project_inspect",
        json!({"aui":{"mode":"trace","documentId":"neon-hud","nodeId":"heading"}}),
    ));
    assert_completed("neon inspect", &inspected);
    assert_eq!(
        inspected.output["auiInspection"]["documents"][0]["sourceSchemaVersion"],
        "aui-document.v3"
    );
    assert_eq!(
        inspected.output["auiInspection"]["trace"][0]["nodeId"],
        "heading"
    );
    assert_eq!(
        inspected.output["auiInspection"]["trace"][0]["hasTextLayout"],
        true
    );

    let checked = provider.invoke(approved_call(
        "neon-m5-check",
        "engine_project_check",
        json!({}),
    ));
    assert_completed("neon check", &checked);
    let tested = provider.invoke(approved_call(
        "neon-m5-playtest",
        "engine_runtime_playtest",
        json!({}),
    ));
    assert_completed("neon menu playtest", &tested);
    let semantic = &tested.output["semantic"];
    assert_eq!(semantic["scenarioId"], "neon.m5.menu");
    assert_eq!(semantic["assertions"][0]["actual"], "Play");
    assert_eq!(semantic["injectedTransitionCount"], 4);
    assert_eq!(semantic["aui"]["documentId"], "neon-hud");
    assert!(semantic["auiEvents"]
        .as_array()
        .unwrap()
        .iter()
        .any(|event| {
            event["actionCount"].as_u64().unwrap() > 0 && event["inputConsumed"] == true
        }));
    let observed = provider.invoke(approved_call(
        "neon-m5-observe",
        "engine_runtime_observe",
        json!({"runRef":tested.output["runRef"]}),
    ));
    assert_completed("neon sealed observe", &observed);
    assert_eq!(observed.output, tested.output);
}

#[test]
fn neon_m5_menu_font_theme_and_viewport_matrix_uses_ordinary_project_sources() {
    struct MatrixCase {
        label: &'static str,
        width: u64,
        height: u64,
        font_size: f64,
        theme_id: Option<&'static str>,
    }

    let root = unique_temp_dir("provider-neon-m5-matrix");
    let _guard = TestDirectoryGuard(root.clone());
    for case in [
        MatrixCase {
            label: "baseline-landscape",
            width: 1280,
            height: 720,
            font_size: 32.0,
            theme_id: None,
        },
        MatrixCase {
            label: "large-font-wide-theme",
            width: 1600,
            height: 900,
            font_size: 44.0,
            theme_id: Some("neon-matrix-wide"),
        },
        MatrixCase {
            label: "compact-font-portrait-theme",
            width: 720,
            height: 1280,
            font_size: 24.0,
            theme_id: Some("neon-matrix-portrait"),
        },
    ] {
        let project = root.join(case.label);
        copy_source_tree(&workspace_root().join("samples/neon_lab_project"), &project);
        configure_neon_matrix_project(
            &project,
            case.width,
            case.height,
            case.font_size,
            case.theme_id,
        );
        let project = fs::canonicalize(project).unwrap();
        let mut provider = EngineToolProvider::attach(HostSessionContext {
            session_id: format!("neon-m5-matrix-{}", case.label),
            workspace_root: project.clone(),
            project_root: Some(project),
        })
        .unwrap();

        let inspected = provider.invoke(approved_call(
            &format!("{}-inspect", case.label),
            "engine_project_inspect",
            json!({"aui":{"mode":"trace","documentId":"neon-hud","nodeId":"heading"}}),
        ));
        assert_completed(case.label, &inspected);
        assert_eq!(
            inspected.output["auiInspection"]["trace"][0]["nodeId"],
            "heading"
        );

        let checked = provider.invoke(approved_call(
            &format!("{}-check", case.label),
            "engine_project_check",
            json!({}),
        ));
        assert_completed(case.label, &checked);

        let tested = provider.invoke(approved_call(
            &format!("{}-playtest", case.label),
            "engine_runtime_playtest",
            json!({}),
        ));
        assert_completed(case.label, &tested);
        let semantic = &tested.output["semantic"];
        assert_eq!(semantic["assertions"][0]["actual"], "Play");
        let action_count = semantic["auiEvents"]
            .as_array()
            .unwrap()
            .iter()
            .map(|event| event["actionCount"].as_u64().unwrap())
            .sum::<u64>();
        assert_eq!(action_count, 1, "{} start action count", case.label);

        let nodes = semantic["aui"]["nodes"].as_array().unwrap();
        for node_id in ["heading", "description", "start"] {
            let node = nodes
                .iter()
                .find(|node| node["nodeId"] == node_id)
                .unwrap_or_else(|| panic!("{} missing {node_id}", case.label));
            assert_eq!(node["visible"], true, "{} {node_id}", case.label);
            assert_rect_inside_viewport(node, case.width as f64, case.height as f64, case.label);
        }
        let heading = nodes
            .iter()
            .find(|node| node["nodeId"] == "heading")
            .unwrap();
        let description = nodes
            .iter()
            .find(|node| node["nodeId"] == "description")
            .unwrap();
        assert!(heading["text"].as_str().unwrap().chars().count() >= 8);
        assert!(description["text"].as_str().unwrap().chars().count() >= 20);
        assert_eq!(heading["fontSize"].as_f64().unwrap(), case.font_size);
        if let Some(theme_id) = case.theme_id {
            assert!(heading["styleSources"]
                .as_array()
                .unwrap()
                .iter()
                .any(|source| source == &format!("{theme_id}:menu-heading")));
        }

        let observed = provider.invoke(approved_call(
            &format!("{}-observe", case.label),
            "engine_runtime_observe",
            json!({"runRef":tested.output["runRef"]}),
        ));
        assert_completed(case.label, &observed);
        assert_eq!(observed.output, tested.output);
    }
}

#[cfg(windows)]
#[test]
#[ignore = "local Windows GPU/window qualification with ordinary project builds"]
fn neon_m5_default_and_project_theme_window_pixels_match_package_geometry() {
    let root = unique_temp_dir("provider-neon-m5-gpu");
    let _guard = TestDirectoryGuard(root.clone());
    for (label, theme_id, font_size) in [
        ("default", None, 32.0),
        ("project-theme", Some("neon-m5-gpu"), 44.0),
    ] {
        let project = root.join(label);
        copy_source_tree(&workspace_root().join("samples/neon_lab_project"), &project);
        configure_neon_matrix_project(&project, 1280, 720, font_size, theme_id);
        let project = fs::canonicalize(project).unwrap();
        let mut provider = EngineToolProvider::attach(HostSessionContext {
            session_id: format!("neon-m5-gpu-{label}"),
            workspace_root: project.clone(),
            project_root: Some(project.clone()),
        })
        .unwrap();
        let build = provider.invoke(approved_call(
            &format!("{label}-build"),
            "engine_project_build",
            json!({"targetProfile":"windows-dev","frameLimit":3}),
        ));
        assert_completed(label, &build);
        let package = PathBuf::from(build.output["packageDir"].as_str().unwrap());
        let tested = provider.invoke(approved_call(
            &format!("{label}-playtest"),
            "engine_runtime_playtest",
            json!({"deliveryRef":build.output["deliveryRef"]}),
        ));
        assert_completed(label, &tested);
        assert_eq!(
            tested.output["artifactIdentity"],
            build.output["artifactIdentity"]
        );
        assert_eq!(
            tested.output["deliveryIdentity"],
            build.output["deliveryIdentity"]
        );
        let heading = tested.output["semantic"]["aui"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["nodeId"] == "heading")
            .unwrap()
            .clone();
        assert_rect_inside_viewport(&heading, 1280.0, 720.0, label);
        assert_eq!(heading["fontSize"], font_size);

        let verified = provider.invoke(approved_call(
            &format!("{label}-gpu-verify"),
            "engine_delivery_verify",
            json!({
                "deliveryRef":build.output["deliveryRef"], "mode":"windowed",
                "frameLimit":3, "timeoutMs":30000, "screenshot":true
            }),
        ));
        assert_completed(label, &verified);
        assert_eq!(verified.output["status"], "passed", "{verified:#?}");
        assert_eq!(
            verified.output["artifactIdentity"],
            tested.output["artifactIdentity"]
        );
        assert_eq!(
            verified.output["deliveryIdentity"],
            tested.output["deliveryIdentity"]
        );
        let report_path = package.join("reports/exported-player-process-verification-report.json");
        let report: Value = serde_json::from_slice(&fs::read(&report_path).unwrap()).unwrap();
        assert_eq!(report["screenshotStatus"], "captured", "{report:#?}");
        assert_eq!(report["childPresentStatus"], "presented", "{report:#?}");
        let screenshot = PathBuf::from(report["screenshotPath"].as_str().unwrap());
        let bright = assert_neon_menu_pixels(&screenshot);
        assert!(bright > if theme_id.is_some() { 5_000 } else { 2_500 });
        let child_report_path = package.join("reports/windowed-player-run-report.json");
        let child_report: Value =
            serde_json::from_slice(&fs::read(&child_report_path).unwrap()).unwrap();
        assert!(child_report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |item| item["code"] == "native_host.graphics.backend_selected"
                    && item["message"] == "Vulkan"
            ));
        if let Ok(evidence_root) = std::env::var("AIFE_M5_EVIDENCE_ROOT") {
            let evidence_root = PathBuf::from(evidence_root);
            fs::create_dir_all(&evidence_root).unwrap();
            fs::copy(&screenshot, evidence_root.join(format!("{label}.png"))).unwrap();
            fs::copy(
                &report_path,
                evidence_root.join(format!("{label}-verification.json")),
            )
            .unwrap();
            fs::copy(
                &child_report_path,
                evidence_root.join(format!("{label}-window.json")),
            )
            .unwrap();
            fs::write(
                evidence_root.join(format!("{label}-identity.json")),
                serde_json::to_vec_pretty(&json!({
                    "build":build.output,
                    "playtest":tested.output,
                    "verify":verified.output,
                    "heading":heading,
                    "brightHeadingPixels":bright
                }))
                .unwrap(),
            )
            .unwrap();
        }
        eprintln!(
            "{label}: {} / {}",
            screenshot.display(),
            report_path.display()
        );
    }
}

#[cfg(windows)]
fn assert_neon_menu_pixels(path: &Path) -> usize {
    let decoder = png::Decoder::new(fs::File::open(path).unwrap());
    let mut reader = decoder.read_info().unwrap();
    let mut bytes = vec![0; reader.output_buffer_size()];
    let image = reader.next_frame(&mut bytes).unwrap();
    assert_eq!((image.width, image.height), (1280, 720));
    assert_eq!(image.color_type, png::ColorType::Rgba);
    let pixel = |x: usize, y: usize| -> &[u8] {
        let offset = (y * image.width as usize + x) * 4;
        &bytes[offset..offset + 4]
    };
    let background = pixel(10, 700);
    let button = pixel(400, 480);
    assert!(background[0] < 30 && background[1] < 40 && background[2] < 55);
    assert!(
        button[1] > 170 && button[2] > 140,
        "start button: {button:?}"
    );
    let bright = (195..270)
        .flat_map(|y| (320..850).map(move |x| (x, y)))
        .filter(|&(x, y)| pixel(x, y)[..3].iter().all(|channel| *channel > 180))
        .count();
    assert!(bright > 2_500, "missing heading glyph pixels: {bright}");
    bright
}

#[cfg(windows)]
#[test]
#[ignore = "reads retained M5-C window evidence without rebuilding the project"]
fn neon_m5_retained_window_pixels_are_distinct_by_theme() {
    let evidence = PathBuf::from(std::env::var("AIFE_M5_EVIDENCE_ROOT").unwrap());
    let default = assert_neon_menu_pixels(&evidence.join("default.png"));
    let project = assert_neon_menu_pixels(&evidence.join("project-theme.png"));
    assert!(
        project > default + 1_000,
        "theme metrics did not change glyph footprint"
    );
}

#[cfg(windows)]
#[test]
#[ignore = "explicit M5-D independent Windows dev delivery qualification"]
fn aui_m5_independent_deliveries_run_without_project_sources() {
    let delivery_root = PathBuf::from(std::env::var("AIFE_M5D_DELIVERY_ROOT").unwrap());
    assert!(!delivery_root.exists(), "use a fresh delivery root");
    fs::create_dir_all(&delivery_root).unwrap();
    let source_root = unique_temp_dir("provider-aui-m5d-source");
    let _guard = TestDirectoryGuard(source_root.clone());
    let mut neon_pixels = Vec::new();
    for (label, source_name, theme_id, mode) in [
        ("neon-default", "neon_lab_project", None, "windowed"),
        (
            "neon-project-theme",
            "neon_lab_project",
            Some("neon-m5-gpu"),
            "windowed",
        ),
        ("settings", "aui_c2_settings_project", None, "headless-gate"),
        (
            "virtual-list",
            "aui_virtual_list_project",
            None,
            "headless-gate",
        ),
    ] {
        let project = source_root.join(label);
        copy_source_tree(
            &workspace_root().join("samples").join(source_name),
            &project,
        );
        if source_name == "neon_lab_project" {
            configure_neon_matrix_project(
                &project,
                1280,
                720,
                if theme_id.is_some() { 44.0 } else { 32.0 },
                theme_id,
            );
        }
        let project = fs::canonicalize(project).unwrap();
        let mut provider = EngineToolProvider::attach(HostSessionContext {
            session_id: format!("aui-m5d-{label}"),
            workspace_root: project.clone(),
            project_root: Some(project.clone()),
        })
        .unwrap();
        let checked = provider.invoke(approved_call(
            &format!("{label}-check"),
            "engine_project_check",
            json!({}),
        ));
        assert_completed(label, &checked);
        let built = provider.invoke(approved_call(
            &format!("{label}-build"),
            "engine_project_build",
            json!({"targetProfile":"windows-dev","frameLimit":3}),
        ));
        assert_completed(label, &built);
        let original = PathBuf::from(built.output["packageDir"].as_str().unwrap());
        let verified = provider.invoke(approved_call(
            &format!("{label}-verify"),
            "engine_delivery_verify",
            json!({
                "deliveryRef":built.output["deliveryRef"],
                "mode":if mode == "windowed" { "windowed" } else { "headless" },
                "frameLimit":3, "timeoutMs":30000, "screenshot":mode == "windowed"
            }),
        ));
        assert_completed(label, &verified);
        assert_eq!(verified.output["status"], "passed", "{verified:#?}");
        assert_eq!(
            built.output["artifactIdentity"],
            verified.output["artifactIdentity"]
        );
        assert_eq!(
            built.output["deliveryIdentity"],
            verified.output["deliveryIdentity"]
        );
        drop(provider);

        let copied = delivery_root.join(label);
        let (file_count, byte_count) = copy_delivery_tree(&original, &copied);
        assert!(file_count > 10 && byte_count > 0);
        assert!(copied.join("Game.exe").is_file());
        assert!(copied.join("engine_runtime.dll").is_file());
        assert!(copied.join("data/runtime_package/manifest.json").is_file());
        fs::rename(
            &project,
            source_root.join(format!("{label}-source-isolated")),
        )
        .unwrap();
        assert!(
            !project.exists(),
            "original project path must be unavailable"
        );

        let isolated_report = copied.join("reports/m5d-isolated-verification.json");
        let mut command = Command::new(copied.join("Game.exe"));
        command
            .current_dir(&copied)
            .arg("verify-exported-player")
            .arg("--package")
            .arg(&copied)
            .arg("--mode")
            .arg(mode)
            .arg("--frames")
            .arg("3")
            .arg("--report")
            .arg(&isolated_report);
        if mode == "windowed" {
            command
                .arg("--screenshot")
                .arg("--screenshot-path")
                .arg(copied.join("reports/m5d-isolated.png"));
        }
        let process = command.output().expect("launch copied Game.exe");
        assert!(
            process.status.success(),
            "{label} isolated player: {} / {}",
            String::from_utf8_lossy(&process.stdout),
            String::from_utf8_lossy(&process.stderr)
        );
        let report: Value = serde_json::from_slice(&fs::read(&isolated_report).unwrap()).unwrap();
        assert_eq!(report["status"], "passed", "{label}: {report:#?}");
        assert_eq!(report["childFramesCompleted"], 3);
        assert_eq!(report["childPlayerExitCode"], 0);
        if mode == "windowed" {
            assert_eq!(report["childPresentStatus"], "presented");
            neon_pixels.push(assert_neon_menu_pixels(
                &copied.join("reports/m5d-isolated.png"),
            ));
        }
        let evidence = json!({
            "label":label, "sourcePathUnavailable":!project.exists(),
            "check":checked.output, "build":built.output, "verify":verified.output,
            "copiedPackage":copied, "copiedFileCount":file_count, "copiedBytes":byte_count,
            "isolatedVerify":report
        });
        fs::write(
            delivery_root.join(format!("{label}-qualification.json")),
            serde_json::to_vec_pretty(&evidence).unwrap(),
        )
        .unwrap();
        eprintln!(
            "M5-D {label}: {} files / {} bytes / isolated passed",
            file_count, byte_count
        );
    }
    assert_eq!(neon_pixels.len(), 2);
    assert!(neon_pixels[1] > neon_pixels[0] + 1_000);
}

#[cfg(windows)]
fn copy_delivery_tree(source: &Path, destination: &Path) -> (usize, u64) {
    fs::create_dir_all(destination).unwrap();
    let mut files = 0;
    let mut bytes = 0;
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let kind = entry.file_type().unwrap();
        assert!(!kind.is_symlink(), "delivery must not contain symlinks");
        let target = destination.join(entry.file_name());
        if kind.is_dir() {
            let nested = copy_delivery_tree(&entry.path(), &target);
            files += nested.0;
            bytes += nested.1;
        } else {
            bytes += fs::copy(entry.path(), target).unwrap();
            files += 1;
        }
    }
    (files, bytes)
}

#[test]
fn aui_m5_settings_project_grid_flex_modal_and_observe() {
    let root = unique_temp_dir("provider-settings-m5");
    let project = root.join("project");
    let _guard = TestDirectoryGuard(root);
    copy_source_tree(
        &workspace_root().join("samples/aui_c2_settings_project"),
        &project,
    );
    let project = fs::canonicalize(project).unwrap();
    let mut provider = EngineToolProvider::attach(HostSessionContext {
        session_id: "settings-m5".into(),
        workspace_root: project.clone(),
        project_root: Some(project),
    })
    .unwrap();
    for (node_id, expected_mode) in [("grid", "Grid"), ("form", "Flex")] {
        let inspected = provider.invoke(approved_call(
            &format!("settings-inspect-{node_id}"),
            "engine_project_inspect",
            json!({"aui":{"mode":"trace","documentId":"settings-inventory","nodeId":node_id}}),
        ));
        assert_completed("settings inspect", &inspected);
        assert_eq!(
            inspected.output["auiInspection"]["trace"][0]["layoutMode"],
            expected_mode
        );
    }
    let checked = provider.invoke(approved_call(
        "settings-check",
        "engine_project_check",
        json!({}),
    ));
    assert_completed("settings check", &checked);
    let tested = provider.invoke(approved_call(
        "settings-playtest",
        "engine_runtime_playtest",
        json!({}),
    ));
    assert_completed("settings playtest", &tested);
    assert_eq!(tested.output["semantic"]["assertions"][0]["actual"], true);
    assert_eq!(tested.output["semantic"]["assertions"][1]["actual"], true);
    assert_eq!(tested.output["semantic"]["assertions"][2]["actual"], 80);
    assert_eq!(
        tested.output["semantic"]["assertions"][3]["actual"],
        "测试 العربية"
    );
    assert_eq!(
        tested.output["semantic"]["aui"]["documentId"],
        "settings-inventory"
    );
    for node_id in [
        "grid",
        "form",
        "name",
        "enabled",
        "volume",
        "inventory",
        "close",
    ] {
        assert!(
            tested.output["semantic"]["aui"]["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|node| {
                    node["nodeId"] == node_id
                        && node["visible"] == true
                        && node["rect"]["width"].as_f64().unwrap() > 0.0
                        && node["rect"]["height"].as_f64().unwrap() > 0.0
                }),
            "missing or collapsed {node_id}: {:#?}",
            tested.output["semantic"]["aui"]
        );
    }
    assert!(
        tested.output["semantic"]["aui"]["scrollOffsets"]["inventory"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert!(tested.output["semantic"]["aui"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|node| {
            node["nodeId"] == "name" && node["text"].as_str().unwrap().contains("测试 العربية")
        }));
    assert!(tested.output["semantic"]["aui"]["focusedNodeId"]
        .as_str()
        .is_some());
    assert!(tested.output["semantic"]["auiEvents"]
        .as_array()
        .unwrap()
        .iter()
        .any(|event| { event["actionCount"].as_u64().unwrap() > 0 }));
    let observed = provider.invoke(approved_call(
        "settings-observe",
        "engine_runtime_observe",
        json!({"runRef":tested.output["runRef"]}),
    ));
    assert_completed("settings observe", &observed);
    assert_eq!(observed.output, tested.output);
}

#[test]
fn aui_m5_virtual_list_project_is_bounded_selectable_and_sealed() {
    let root = unique_temp_dir("provider-virtual-list-m5");
    let project = root.join("project");
    let _guard = TestDirectoryGuard(root);
    copy_source_tree(
        &workspace_root().join("samples/aui_virtual_list_project"),
        &project,
    );
    let project = fs::canonicalize(project).unwrap();
    let mut provider = EngineToolProvider::attach(HostSessionContext {
        session_id: "virtual-list-m5".into(),
        workspace_root: project.clone(),
        project_root: Some(project.clone()),
    })
    .unwrap();
    let inspected = provider.invoke(approved_call(
        "virtual-list-inspect",
        "engine_project_inspect",
        json!({"aui":{"mode":"trace","documentId":"virtual-list-lab","nodeId":"fixed-list"}}),
    ));
    assert_completed("virtual list inspect", &inspected);
    assert_eq!(
        inspected.output["auiInspection"]["trace"][0]["kind"],
        "List"
    );
    assert_eq!(
        inspected.output["auiInspection"]["trace"][0]["layoutMode"],
        "Overlay"
    );
    let capabilities = inspected.output["auiInspection"]["documents"][0]["capabilities"]
        .as_array()
        .unwrap();
    assert!(capabilities.iter().any(|value| value == "list"));
    assert!(capabilities
        .iter()
        .any(|value| value == "viewport_condition"));

    let checked = provider.invoke(approved_call(
        "virtual-list-check",
        "engine_project_check",
        json!({}),
    ));
    assert_completed("virtual list check", &checked);
    let tested = provider.invoke(approved_call(
        "virtual-list-playtest",
        "engine_runtime_playtest",
        json!({}),
    ));
    assert_completed("virtual list playtest", &tested);
    let semantic = &tested.output["semantic"];
    assert_eq!(semantic["scenarioId"], "aui-c2.virtual-list");
    assert_eq!(semantic["assertions"][0]["actual"], 2);
    let selected_key = semantic["assertions"][1]["actual"]
        .as_str()
        .expect("selected stable key");
    assert_eq!(selected_key, "item-00012");
    assert_eq!(semantic["aui"]["documentId"], "virtual-list-lab");
    let lists = semantic["aui"]["virtualLists"].as_array().unwrap();
    assert_eq!(lists.len(), 2);
    let fixed = lists
        .iter()
        .find(|list| list["listNodeId"] == "fixed-list")
        .unwrap();
    assert_eq!(fixed["totalItemCount"], 10_000);
    assert!(fixed["liveInstanceCount"].as_u64().unwrap() <= 24);
    assert!(fixed["firstMaterializedIndex"].as_u64().unwrap() > 0);
    assert_eq!(fixed["selectedKey"], selected_key);
    assert!(fixed["anchorKey"].as_str().unwrap().starts_with("item-"));
    let variable = lists
        .iter()
        .find(|list| list["listNodeId"] == "variable-list")
        .unwrap();
    assert_eq!(variable["totalItemCount"], 10_000);
    assert!(variable["liveInstanceCount"].as_u64().unwrap() <= 24);
    let texts = semantic["aui"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|node| node["text"].as_str())
        .collect::<Vec<_>>();
    assert!(texts.iter().any(|text| text.contains("能源电池")));
    assert!(texts.iter().any(|text| text.contains("العنصر")));
    assert!(texts.iter().any(|text| text.contains("unbroken-token")));

    let observed = provider.invoke(approved_call(
        "virtual-list-observe",
        "engine_runtime_observe",
        json!({"runRef":tested.output["runRef"]}),
    ));
    assert_completed("virtual list sealed observe", &observed);
    assert_eq!(observed.output, tested.output);

    let scenario_path = project.join("Tests/virtual-list.scenario.json");
    let mut scenario: Value = serde_json::from_slice(&fs::read(&scenario_path).unwrap()).unwrap();
    scenario["assertions"][1]["equals"] = "item-99999".into();
    fs::write(
        &scenario_path,
        serde_json::to_vec_pretty(&scenario).unwrap(),
    )
    .unwrap();
    let mut negative_provider = EngineToolProvider::attach(HostSessionContext {
        session_id: "virtual-list-m5-negative".into(),
        workspace_root: project.clone(),
        project_root: Some(project),
    })
    .unwrap();
    let negative = negative_provider.invoke(approved_call(
        "virtual-list-wrong-key",
        "engine_runtime_playtest",
        json!({}),
    ));
    assert_eq!(
        negative.status,
        CanonicalToolStatus::Failed,
        "{negative:#?}"
    );
    assert_eq!(
        negative.output["semantic"]["assertions"][1]["status"],
        "failed"
    );
    assert_eq!(
        negative.output["semantic"]["assertions"][1]["actual"],
        selected_key
    );
}

fn approved_call(call_id: &str, tool_name: &str, arguments: Value) -> HostToolCall {
    HostToolCall {
        call_id: call_id.to_string(),
        tool_name: tool_name.to_string(),
        arguments,
        approved: true,
    }
}

fn assert_completed(label: &str, result: &engine_tool_provider::CanonicalToolResult) {
    assert_eq!(
        result.status,
        CanonicalToolStatus::Completed,
        "{label} failed: {result:#?}"
    );
}

fn assert_project_delivery(project: &Path, output: &Value) {
    let package = PathBuf::from(
        output["packageDir"]
            .as_str()
            .expect("execution returns packageDir"),
    );
    assert!(package.starts_with(project));
    assert!(package.join("Game.exe").is_file());
    assert!(package.join("package-manifest.json").is_file());
    assert!(package.join("data/runtime_package/manifest.json").is_file());
}

fn copy_source_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create copied project directory");
    for entry in fs::read_dir(source).expect("read source project directory") {
        let entry = entry.expect("read source project entry");
        let name = entry.file_name();
        if matches!(
            name.to_str(),
            Some("Build" | "Library" | "target" | ".aife" | ".git")
        ) {
            continue;
        }
        let source_path = entry.path();
        let destination_path = destination.join(name);
        if entry.file_type().expect("inspect source entry").is_dir() {
            copy_source_tree(&source_path, &destination_path);
        } else {
            fs::copy(source_path, destination_path).expect("copy source project file");
        }
    }
}

fn configure_neon_matrix_project(
    project: &Path,
    width: u64,
    height: u64,
    font_size: f64,
    theme_id: Option<&str>,
) {
    let profile_path = project.join("BuildProfiles/windows.dev.json");
    let mut profile: Value = serde_json::from_slice(&fs::read(&profile_path).unwrap()).unwrap();
    profile["gameViewTarget"]["extent"]["width"] = width.into();
    profile["gameViewTarget"]["extent"]["height"] = height.into();
    fs::write(profile_path, serde_json::to_vec_pretty(&profile).unwrap()).unwrap();

    let scenario_path = project.join("Tests/aui-m5-menu.scenario.json");
    let mut scenario: Value = serde_json::from_slice(&fs::read(&scenario_path).unwrap()).unwrap();
    scenario["maxSimulationTicks"] = 2.into();
    scenario["maxPresentationFrames"] = 4.into();
    scenario["assertions"][0]["throughSimulationTick"] = 2.into();
    let panel_x = width as f64 * 0.5 - 300.0;
    let panel_y = height as f64 * 0.5 - 195.0;
    let pointer = json!([
        (panel_x + 54.0 + 114.0).round() as i32,
        (panel_y + 248.0 + 26.0).round() as i32
    ]);
    scenario["inputs"][0]["events"][0]["position"] = pointer.clone();
    scenario["inputs"][1]["events"][0]["position"] = pointer;
    fs::write(scenario_path, serde_json::to_vec_pretty(&scenario).unwrap()).unwrap();

    let document_path = project.join("AUI/neon.aui.json");
    let mut document: Value = serde_json::from_slice(&fs::read(&document_path).unwrap()).unwrap();
    if let Some(theme_id) = theme_id {
        document["theme"] = json!({"id":theme_id,"type":"aui-theme"});
        let heading = document["nodes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|node| node["node_id"] == "heading")
            .unwrap();
        heading["styleRef"] = "menu-heading".into();
        heading["style"]
            .as_object_mut()
            .unwrap()
            .remove("font_size");
        fs::write(
            project.join(format!("Assets/{theme_id}.asset")),
            serde_json::to_vec_pretty(&json!({
                "schemaVersion":"aui-theme-asset.v1",
                "assetId":theme_id,
                "tokens":{},
                "styles":{
                    "menu-heading":{
                        "role":"text",
                        "metrics":{"fontSize":{"source":"value","value":font_size}},
                        "paint":{"foreground":{"source":"value","value":"#E9FAFFFF"}}
                    }
                }
            }))
            .unwrap(),
        )
        .unwrap();
    }
    fs::write(document_path, serde_json::to_vec_pretty(&document).unwrap()).unwrap();
}

fn assert_rect_inside_viewport(node: &Value, width: f64, height: f64, label: &str) {
    let rect = &node["rect"];
    let x = rect["x"].as_f64().unwrap();
    let y = rect["y"].as_f64().unwrap();
    let node_width = rect["width"].as_f64().unwrap();
    let node_height = rect["height"].as_f64().unwrap();
    assert!(
        x >= 0.0
            && y >= 0.0
            && node_width > 0.0
            && node_height > 0.0
            && x + node_width <= width
            && y + node_height <= height,
        "{label} node outside {width}x{height}: {node:#?}"
    );
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("aife-{label}-{}-{nonce}", std::process::id()))
}

struct TestDirectoryGuard(PathBuf);

impl Drop for TestDirectoryGuard {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("retained failed headless owner root: {}", self.0.display());
            return;
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(windows)]
#[test]
#[ignore = "local Windows actual 3D SDK/input/semantic/capture and sealed Provider loop"]
fn three_d_a3_semantic_loop_and_sealed_observe() {
    let root = unique_temp_dir("provider-three-d-a32");
    let _guard = TestDirectoryGuard(root.clone());
    let project = root.join("project");
    copy_source_tree(
        &workspace_root().join("samples/three_d_lab_project"),
        &project,
    );
    let project = fs::canonicalize(project).unwrap();
    let evidence = std::env::var_os("AIFE_A32_EVIDENCE_ROOT").map(PathBuf::from);
    if let Some(path) = &evidence {
        fs::create_dir_all(path).unwrap();
    }
    let save = |name: &str, value: &Value| {
        if let Some(path) = &evidence {
            fs::write(
                path.join(format!("{name}.json")),
                serde_json::to_vec_pretty(value).unwrap(),
            )
            .unwrap();
        }
    };
    let mut provider = EngineToolProvider::attach(HostSessionContext {
        session_id: "three-d-a32".into(),
        workspace_root: project.clone(),
        project_root: Some(project.clone()),
    })
    .unwrap();
    let scene_path = project.join("Scenes/Main.scene.json");
    let original = fs::read(&scene_path).unwrap();
    let mut bad: Value = serde_json::from_slice(&original).unwrap();
    let camera = bad["entities"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|e| e["id"] == "camera")
        .unwrap();
    let lens = camera["components"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|c| c["componentType"] == "Camera3D")
        .unwrap();
    lens["data"]["projection"]["far"] = json!(0.01);
    fs::write(&scene_path, serde_json::to_vec_pretty(&bad).unwrap()).unwrap();
    let broken = provider.invoke(approved_call(
        "broken-inspect",
        "engine_project_inspect",
        json!({}),
    ));
    save("broken-inspect", &serde_json::to_value(&broken).unwrap());
    assert_completed("broken inspect", &broken);
    assert!(!broken.output["spatialInspection"]["diagnostics"]
        .as_array()
        .unwrap()
        .is_empty());
    let rejected = provider.invoke(approved_call(
        "broken-check",
        "engine_project_check",
        json!({}),
    ));
    save("broken-check", &serde_json::to_value(&rejected).unwrap());
    assert_eq!(rejected.status, CanonicalToolStatus::RejectedByEngine);
    assert!(serde_json::to_string(&rejected)
        .unwrap()
        .contains("projection"));
    fs::write(&scene_path, &original).unwrap();
    let inspect = provider.invoke(approved_call(
        "repaired-inspect",
        "engine_project_inspect",
        json!({}),
    ));
    save("repaired-inspect", &serde_json::to_value(&inspect).unwrap());
    assert_completed("repaired inspect", &inspect);
    assert!(inspect.output["spatialInspection"]["diagnostics"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        inspect.output["spatialInspection"]["scenes"][0]["mainCamera"],
        "camera"
    );
    assert_ne!(broken.output["revision"], inspect.output["revision"]);
    let check = provider.invoke(approved_call(
        "repaired-check",
        "engine_project_check",
        json!({}),
    ));
    save("repaired-check", &serde_json::to_value(&check).unwrap());
    assert_completed("repaired check", &check);
    let build = provider.invoke(approved_call(
        "repaired-build",
        "engine_project_build",
        json!({"targetProfile":"windows-dev","frameLimit":3}),
    ));
    save("build", &serde_json::to_value(&build).unwrap());
    assert_completed("build", &build);
    assert_project_delivery(&project, &build.output);
    let tested = provider.invoke(approved_call(
        "spatial-playtest",
        "engine_runtime_playtest",
        json!({"deliveryRef":build.output["deliveryRef"]}),
    ));
    save("playtest", &serde_json::to_value(&tested).unwrap());
    let package = PathBuf::from(build.output["packageDir"].as_str().unwrap());
    if let Some(path) = &evidence {
        copy_source_tree(&package.join("reports"), &path.join("package-reports"));
    }
    // Preserve raw failure and capture artifacts before asserting the outcome.
    if let Some(captures) = tested.output["semantic"]["captureEvidence"].as_array() {
        for c in captures {
            if let (Some(path), Some(source)) = (&evidence, c["path"].as_str()) {
                fs::copy(
                    source,
                    path.join(format!("{}.png", c["captureId"].as_str().unwrap())),
                )
                .unwrap();
            }
        }
    }
    if let Some(first) = tested.output["semantic"]["captureEvidence"]
        .as_array()
        .and_then(|c| c.first())
    {
        let worker = Path::new(first["path"].as_str().unwrap()).parent().unwrap();
        if let Some(path) = &evidence {
            copy_source_tree(worker, &path.join("worker"));
        }
        let player: Value =
            serde_json::from_slice(&fs::read(worker.join("player.json")).unwrap()).unwrap();
        assert!(player["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "engine_runtime.execution_owner"
                && d["layer"] == "engine_runtime.dll"));
        assert!(player["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "graphics.backend_selected" && d["message"] == "Vulkan"));
    }
    assert_completed("spatial playtest", &tested);
    assert_eq!(
        tested.output["deliveryIdentity"],
        build.output["deliveryIdentity"]
    );
    assert_eq!(
        tested.output["artifactIdentity"],
        build.output["artifactIdentity"]
    );
    let manifest: Value =
        serde_json::from_slice(&fs::read(package.join("package-manifest.json")).unwrap()).unwrap();
    let mut binaries = serde_json::Map::new();
    for (file, key) in [
        ("Game.exe", "playerArtifactHash"),
        ("engine_runtime.dll", "engineRuntimeHash"),
        (
            "data/bin/sample_three_d_lab_runtime.dll",
            "projectRuntimeModuleHash",
        ),
    ] {
        use sha2::{Digest, Sha256};
        let digest = format!(
            "sha256:{:x}",
            Sha256::digest(fs::read(package.join(file)).unwrap())
        );
        assert_eq!(manifest[key], digest);
        binaries.insert(file.into(), json!(digest));
    }
    save("binaries", &json!(binaries));
    save("package-manifest", &manifest);
    let semantic = &tested.output["semantic"];
    assert_eq!(semantic["overall"], "passed", "{tested:#?}");
    assert_eq!(semantic["assertions"].as_array().unwrap().len(), 13);
    assert_eq!(semantic["captureEvidence"].as_array().unwrap().len(), 7);
    assert_eq!(semantic["injectedTransitionCount"], 17);
    for capture in semantic["captureEvidence"].as_array().unwrap() {
        assert_eq!(
            capture["presentationFrame"],
            capture["spatial"]["committedFrame"]
        );
        assert!(capture["spatial"]["view"]["version"].is_u64());
    }
    let pixels = |id: &str| {
        let c = semantic["captureEvidence"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["captureId"] == id)
            .unwrap();
        let decoder = png::Decoder::new(fs::File::open(c["path"].as_str().unwrap()).unwrap());
        let mut reader = decoder.read_info().unwrap();
        let mut bytes = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut bytes).unwrap();
        assert_eq!((info.width, info.height), (1280, 720));
        // Exclude the HUD: focus/hover feedback must not manufacture a world-image difference.
        bytes[(220 * 1280 * 4)..info.buffer_size()].to_vec()
    };
    let baseline = pixels("frame-1");
    assert_ne!(baseline, pixels("frame-5"));
    assert_ne!(pixels("frame-5"), pixels("frame-8"));
    assert_ne!(pixels("frame-8"), pixels("frame-11"));
    assert_ne!(pixels("frame-14"), pixels("frame-17"));
    assert_eq!(baseline, pixels("frame-20"));
    let verify=provider.invoke(approved_call("same-delivery-verify","engine_delivery_verify",json!({"deliveryRef":build.output["deliveryRef"],"mode":"windowed","frameLimit":120,"timeoutMs":30000,"screenshot":true})));
    save(
        "same-delivery-verify",
        &serde_json::to_value(&verify).unwrap(),
    );
    assert_completed("same delivery verify", &verify);
    assert_eq!(verify.output["status"], "passed");
    assert_eq!(
        verify.output["deliveryIdentity"],
        build.output["deliveryIdentity"]
    );
    if let Some(path) = &evidence {
        copy_source_tree(&package.join("reports"), &path.join("verify-reports"));
    }
    let scenario_path = project.join("Tests/spatial-controls.scenario.json");
    let mut scenario: Value = serde_json::from_slice(&fs::read(&scenario_path).unwrap()).unwrap();
    scenario["assertions"][0]["equals"] = json!(999.0);
    scenario["spatialObservation"]["level"] = json!("trace");
    fs::write(
        &scenario_path,
        serde_json::to_vec_pretty(&scenario).unwrap(),
    )
    .unwrap();
    let observed = provider.invoke(approved_call(
        "sealed-observe",
        "engine_runtime_observe",
        json!({"runRef":tested.output["runRef"]}),
    ));
    save("sealed-observe", &serde_json::to_value(&observed).unwrap());
    assert_completed("sealed observe", &observed);
    assert_eq!(observed.output, tested.output);
    let negative = provider.invoke(approved_call(
        "wrong-expectation",
        "engine_runtime_playtest",
        json!({}),
    ));
    save(
        "wrong-expectation",
        &serde_json::to_value(&negative).unwrap(),
    );
    assert_eq!(
        negative.status,
        CanonicalToolStatus::Failed,
        "{negative:#?}"
    );
    assert_eq!(
        negative.output["semantic"]["assertions"][0]["status"],
        "failed"
    );
    scenario["inputs"][0]["events"] = json!([{"type":"keyUp","key":"Enter"}]);
    fs::write(
        &scenario_path,
        serde_json::to_vec_pretty(&scenario).unwrap(),
    )
    .unwrap();
    let invalid = provider.invoke(approved_call(
        "invalid-input",
        "engine_runtime_playtest",
        json!({}),
    ));
    save("invalid-input", &serde_json::to_value(&invalid).unwrap());
    assert_eq!(invalid.status, CanonicalToolStatus::RejectedByEngine);
    assert!(serde_json::to_string(&invalid)
        .unwrap()
        .contains("events_invalid"));
}
