#[cfg(windows)]
mod windows_fixture {
    use editor_core::{
        ProjectNativeModuleIdentity, ProjectRuntimeNativeModuleBuildRequest,
        ProjectRuntimeNativeModuleBuildStatus, ProjectRuntimeNativeModuleBuilder,
        ProjectRuntimeNativeModuleLoader, ProjectRuntimePackageAssembler,
        ProjectRuntimePackageAssemblyRequest, ProjectRuntimePackageAssemblyStatus,
        ProjectRuntimeTrustInspection, PROJECT_RUNTIME_NATIVE_MODULE_BUILDER_SCHEMA_VERSION,
        PROJECT_RUNTIME_NATIVE_MODULE_IDENTITY_SCHEMA_VERSION,
    };
    use engine_input::InputMappingAsset;
    use engine_runtime::aui::{
        AuiAction, AuiActionEvent, AuiBindingValue, ProjectUiStateProducerContext,
    };
    use engine_runtime::canonical_digest::sha256_prefixed;
    use engine_runtime::project_observation::CookedProjectObservationContract;
    use engine_runtime::project_runtime_module::{
        LinkedProjectRuntimeSet, ProjectRuntimeBootstrap,
    };
    use engine_runtime::project_runtime_session::{
        ProjectAuiActionBatch, ProjectRuntimeMutationPreparation, ProjectRuntimeObservationContext,
        ProjectRuntimeSessionContext, ProjectRuntimeSessionOutput,
        ProjectRuntimeSessionReportLevel, ProjectRuntimeSessionStatus,
    };
    use engine_runtime::runtime_package::{
        load_runtime_package, RuntimeProjectInfo, RuntimeProjectModuleRef, RuntimeScene,
        RUNTIME_SCENE_SCHEMA_VERSION,
    };
    use engine_runtime::runtime_package_builder::{
        RuntimePackageBuildInput, RuntimePackageBuildRequest, RuntimePackageBuildStatus,
        RuntimePackageBuilder, RuntimePackageSourceJson,
    };
    use engine_runtime::runtime_scene_hydration::RuntimeSceneHydrator;
    use engine_runtime::runtime_time::TimeContext;
    use engine_runtime::world::World;
    use engine_runtime::world_api::WorldReadApi;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    const MODULE_ID: &str = "fixture.native.runtime";
    const INTERFACE_VERSION: &str = "project-runtime-module.v1";
    const TOWER_NATIVE_RETAIN_ROOT_ENV: &str = "AIFE_E2E_TOWER_NATIVE_RETAIN_ROOT";
    const AOT_DIGEST: &str =
        "sha256:3333333333333333333333333333333333333333333333333333333333333333";

    fn rust_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("project_e2e_gate lives under rust/crates")
            .to_path_buf()
    }

    fn identity(fixture_root: &Path) -> ProjectNativeModuleIdentity {
        let manifest = fs::read(fixture_root.join("Cargo.toml")).unwrap();
        let lock = fs::read(fixture_root.join("Cargo.lock")).unwrap();
        ProjectNativeModuleIdentity {
            schema_version: PROJECT_RUNTIME_NATIVE_MODULE_IDENTITY_SCHEMA_VERSION.to_string(),
            project_runtime_abi_digest: format!(
                "sha256:{}",
                project_runtime_abi::project_runtime_abi_digest_hex()
            ),
            project_runtime_sdk_digest: format!(
                "sha256:{}",
                project_runtime_sdk::project_runtime_contract_digest_hex()
            ),
            project_id: "fixture-project".to_string(),
            module_id: MODULE_ID.to_string(),
            logical_interface_version: INTERFACE_VERSION.to_string(),
            aot_content_digest: AOT_DIGEST.to_string(),
            normalized_manifest_digest: sha256_prefixed(&manifest),
            normalized_dependency_digest: sha256_prefixed(
                br#"["project_runtime_abi","project_runtime_sdk","serde"]"#,
            ),
            dependency_lock_digest: sha256_prefixed(&lock),
            toolchain_identity: "current-rustc".to_string(),
            target_triple: "host".to_string(),
            profile: "release".to_string(),
            features: Vec::new(),
            builder_schema_version: PROJECT_RUNTIME_NATIVE_MODULE_BUILDER_SCHEMA_VERSION
                .to_string(),
        }
    }

    fn runtime_package(root: &Path) -> engine_runtime::runtime_package::RuntimePackage {
        let package_root = root.join("runtime-package");
        let mut input = RuntimePackageBuildInput::new(RuntimeProjectInfo::new(
            "fixture-project",
            "Native Module Fixture",
            "0.1.0",
            RuntimeProjectModuleRef::new(MODULE_ID, INTERFACE_VERSION, AOT_DIGEST),
        ));
        input.scenes.push(RuntimeScene {
            schema_version: RUNTIME_SCENE_SCHEMA_VERSION.to_string(),
            id: "scene-main".to_string(),
            name: "Main".to_string(),
            gravity: 0.0,
            background: "#000000".to_string(),
            sky_color: "#000000".to_string(),
            entities: Vec::new(),
        });
        let input_none = InputMappingAsset::explicit_empty("input.none");
        input.input_mappings.push(RuntimePackageSourceJson {
            id: input_none.asset_id.clone(),
            document: serde_json::to_value(input_none).unwrap(),
        });
        let report = RuntimePackageBuilder::build(
            &RuntimePackageBuildRequest::dev_desktop(&package_root, "scene-main"),
            &input,
        );
        assert_eq!(report.status, RuntimePackageBuildStatus::Success);
        load_runtime_package(&package_root).value.unwrap()
    }

    fn commit(output: ProjectRuntimeSessionOutput, world: &mut World) {
        match output
            .prepare_mutations(world)
            .expect("prepare Tower mutations")
        {
            ProjectRuntimeMutationPreparation::Prepared(batch) => {
                batch.commit(world).expect("commit Tower mutations");
            }
            ProjectRuntimeMutationPreparation::Dropped(_) => {}
        }
    }

    #[test]
    fn tower_hydrated_world_exposes_native_session_component_ids() {
        use engine_runtime::components::ComponentTypeId;
        use engine_runtime::query::QuerySpec;

        let rust_root = rust_root();
        let project_root = rust_root
            .parent()
            .unwrap()
            .join("samples/tower_defense_project");
        let assembly = ProjectRuntimePackageAssembler::assemble(
            ProjectRuntimePackageAssemblyRequest::new(&project_root),
        );
        assert_eq!(
            assembly.status,
            ProjectRuntimePackageAssemblyStatus::Success
        );
        let input = assembly.build_input.unwrap();
        let package_root = std::env::temp_dir().join(format!(
            "aife-tower-hydration-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let build = RuntimePackageBuilder::build(
            &RuntimePackageBuildRequest::dev_desktop(
                &package_root,
                assembly.active_scene_id.unwrap(),
            ),
            &input,
        );
        assert_eq!(build.status, RuntimePackageBuildStatus::Success);
        let package = load_runtime_package(&package_root).value.unwrap();
        let mut world = World::new();
        let hydration =
            RuntimeSceneHydrator::from_package(&package).hydrate_active_scene(&package, &mut world);
        assert!(!hydration.has_errors());
        let world = WorldReadApi::new(&world);
        for component in ["tower.matchConfig", "tower.uiContent", "tower.matchView"] {
            let entities = world.query(&QuerySpec::all([ComponentTypeId::from(component)]));
            assert_eq!(entities.len(), 1, "{component}: {entities:?}");
        }
        fs::remove_dir_all(package_root).unwrap();
    }

    #[test]
    fn project_runtime_native_module_minimal_fixture_build_load_bootstrap_and_destroy() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let owned_root = std::env::temp_dir().join(format!("aife-native-module-e2e-{stamp}"));
        fs::create_dir_all(&owned_root).unwrap();
        let rust_root = rust_root();
        let fixture_root = rust_root.join("fixtures/project_runtime_native_module_minimal");
        let report =
            ProjectRuntimeNativeModuleBuilder::prepare(&ProjectRuntimeNativeModuleBuildRequest {
                source_crate_root: fixture_root.clone(),
                engine_sdk_root: rust_root,
                build_root: owned_root.join("build"),
                identity: identity(&fixture_root),
                cargo_executable: None,
                metadata_hard_deadline_ms: 60_000,
                build_hard_deadline_ms: 180_000,
                capture_limit_bytes: 1024 * 1024,
                prepared_runtime_glue: None,
            });
        assert_eq!(
            report.status,
            ProjectRuntimeNativeModuleBuildStatus::Success,
            "{:?}",
            report.diagnostics
        );
        assert_eq!(report.build_scope, "project_module_only");
        for forbidden in ["engine_runtime", "engine_input", "editor_core"] {
            assert!(!report
                .dependency_packages
                .iter()
                .any(|name| name == forbidden));
        }
        assert!(!report
            .dependency_packages
            .iter()
            .any(|name| name.starts_with("editor_")));
        let artifact = report.artifact.expect("qualified native module artifact");
        let adapter =
            ProjectRuntimeNativeModuleLoader::load(&artifact).expect("load sealed fixture DLL");
        let package = runtime_package(&owned_root);
        let first_linked = LinkedProjectRuntimeSet::singleton(Arc::new(adapter.clone())).unwrap();
        let bound =
            ProjectRuntimeBootstrap::bind(&package, &first_linked).expect("bootstrap fixture DLL");
        assert_eq!(bound.receipt().module_id, MODULE_ID);
        let mut parts = bound.into_parts();
        let world = World::new();
        let time = TimeContext::from_delta(1, 1.0 / 60.0, true);
        let action = AuiAction {
            action_id: "fixture.click".to_string(),
            node_id: "fixture.button".to_string(),
            event: AuiActionEvent::Click,
            payload: None,
        };
        let actions = [action];
        let action_output = parts.project_runtime_session.handle_aui_actions(
            ProjectRuntimeSessionContext {
                frame_index: 1,
                time,
                world: WorldReadApi::new(&world),
            },
            ProjectAuiActionBatch::new(&actions),
        );
        assert_eq!(action_output.handled_action_count, 1);
        let fixed = parts
            .project_runtime_session
            .fixed_update(ProjectRuntimeSessionContext {
                frame_index: 2,
                time,
                world: WorldReadApi::new(&world),
            });
        assert_eq!(fixed.status, ProjectRuntimeSessionStatus::NoOp);
        let ui = parts
            .ui_state_producer
            .produce(ProjectUiStateProducerContext::new(2, &package, &world));
        assert_eq!(
            ui.snapshot.values.get("fixture.action_count"),
            Some(&AuiBindingValue::Number(1.0))
        );
        assert_eq!(
            ui.snapshot.values.get("fixture.fixed_count"),
            Some(&AuiBindingValue::Number(1.0))
        );
        let observation = parts
            .project_runtime_session
            .observe(ProjectRuntimeObservationContext {
                frame_index: 2,
                time,
                world: WorldReadApi::new(&world),
                contract: &CookedProjectObservationContract {
                    schema_version: "fixture.v1".to_string(),
                    contract_id: "fixture".to_string(),
                    contract_digest: "sha256:fixture".to_string(),
                    observations: Vec::new(),
                },
                report_level: ProjectRuntimeSessionReportLevel::Summary,
            });
        assert_eq!(observation.len(), 2);
        drop(parts);
        drop(first_linked);

        let second_linked = LinkedProjectRuntimeSet::singleton(Arc::new(adapter.clone())).unwrap();
        let second =
            ProjectRuntimeBootstrap::bind(&package, &second_linked).expect("second session");
        let mut second = second.into_parts();
        let ui = second
            .ui_state_producer
            .produce(ProjectUiStateProducerContext::new(3, &package, &world));
        assert_eq!(
            ui.snapshot.values.get("fixture.destroy_count"),
            Some(&AuiBindingValue::Number(1.0)),
            "the first successful session must be destroyed exactly once"
        );
        let panic_action = [AuiAction {
            action_id: "fixture.panic".to_string(),
            node_id: "fixture.button".to_string(),
            event: AuiActionEvent::Click,
            payload: None,
        }];
        let panic_output = second.project_runtime_session.handle_aui_actions(
            ProjectRuntimeSessionContext {
                frame_index: 3,
                time,
                world: WorldReadApi::new(&world),
            },
            ProjectAuiActionBatch::new(&panic_action),
        );
        assert_eq!(panic_output.status, ProjectRuntimeSessionStatus::Faulted);
        let reentry = second
            .project_runtime_session
            .fixed_update(ProjectRuntimeSessionContext {
                frame_index: 4,
                time,
                world: WorldReadApi::new(&world),
            });
        assert_eq!(reentry.status, ProjectRuntimeSessionStatus::Faulted);
        drop(second);
        drop(second_linked);
        drop(adapter);
        fs::remove_dir_all(&owned_root).unwrap();
        assert!(!owned_root.exists());
    }

    #[test]
    fn tower_native_runtime_module_build_load_bootstrap_and_headless_consumer() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let retain_root = std::env::var_os(TOWER_NATIVE_RETAIN_ROOT_ENV).map(PathBuf::from);
        let owned_root = retain_root
            .clone()
            .unwrap_or_else(|| std::env::temp_dir().join(format!("aife-tower-native-e2e-{stamp}")));
        if retain_root.is_some() {
            assert!(owned_root.is_absolute(), "retained root must be absolute");
            assert!(
                !owned_root.exists(),
                "retained root must not already exist: {}",
                owned_root.display()
            );
        }
        fs::create_dir_all(&owned_root).unwrap();
        let rust_root = rust_root();
        let project_root = rust_root
            .parent()
            .expect("rust root has repository parent")
            .join("samples/tower_defense_project");
        let module_root = project_root.join("RuntimeModule");

        let assembly = ProjectRuntimePackageAssembler::assemble(
            ProjectRuntimePackageAssemblyRequest::new(&project_root)
                .with_artifact_cache_root(owned_root.join("package-cache")),
        );
        assert_eq!(
            assembly.status,
            ProjectRuntimePackageAssemblyStatus::Success,
            "{:?}",
            assembly.report.diagnostics
        );
        let input = assembly.build_input.expect("Tower assembly build input");
        let active_scene_id = assembly.active_scene_id.expect("Tower active scene");
        let package_root = owned_root.join("runtime-package");
        let package_build = RuntimePackageBuilder::build(
            &RuntimePackageBuildRequest::dev_desktop(&package_root, active_scene_id),
            &input,
        );
        assert_eq!(
            package_build.status,
            RuntimePackageBuildStatus::Success,
            "{:?}",
            package_build.diagnostics
        );
        let package = load_runtime_package(&package_root).value.unwrap();
        let module_ref = &package.manifest.project.runtime_module;
        let lock = fs::read(module_root.join("Cargo.lock")).unwrap();
        let inspection =
            ProjectRuntimeTrustInspection::inspect(&project_root, &rust_root, "tower-native-e2e")
                .expect("Tower production trust inspection");
        let rustc = Command::new("rustc")
            .args(["--version", "--verbose"])
            .output()
            .expect("rustc identity");
        assert!(rustc.status.success(), "rustc identity must succeed");
        let toolchain_identity = String::from_utf8(rustc.stdout)
            .expect("rustc identity utf8")
            .trim()
            .to_string();
        let identity = ProjectNativeModuleIdentity {
            schema_version: PROJECT_RUNTIME_NATIVE_MODULE_IDENTITY_SCHEMA_VERSION.to_string(),
            project_runtime_abi_digest: format!(
                "sha256:{}",
                project_runtime_abi::project_runtime_abi_digest_hex()
            ),
            project_runtime_sdk_digest: format!(
                "sha256:{}",
                project_runtime_sdk::project_runtime_contract_digest_hex()
            ),
            project_id: package.manifest.project.project_id.clone(),
            module_id: module_ref.module_id.clone(),
            logical_interface_version: module_ref.interface_version.clone(),
            aot_content_digest: module_ref.aot_content_digest.clone(),
            normalized_manifest_digest: inspection.request.normalized_manifest_digest,
            normalized_dependency_digest: inspection.request.normalized_dependency_digest,
            dependency_lock_digest: sha256_prefixed(&lock),
            toolchain_identity,
            target_triple: "host".to_string(),
            profile: "release".to_string(),
            features: Vec::new(),
            builder_schema_version: PROJECT_RUNTIME_NATIVE_MODULE_BUILDER_SCHEMA_VERSION
                .to_string(),
        };
        let report =
            ProjectRuntimeNativeModuleBuilder::prepare(&ProjectRuntimeNativeModuleBuildRequest {
                source_crate_root: module_root,
                engine_sdk_root: rust_root,
                build_root: owned_root.join("native-build"),
                identity,
                cargo_executable: None,
                metadata_hard_deadline_ms: 60_000,
                build_hard_deadline_ms: 180_000,
                capture_limit_bytes: 1024 * 1024,
                prepared_runtime_glue: None,
            });
        assert_eq!(
            report.status,
            ProjectRuntimeNativeModuleBuildStatus::Success,
            "{:?}",
            report.diagnostics
        );
        assert_eq!(report.build_scope, "project_module_only");
        for forbidden in ["engine_runtime", "engine_input", "editor_core"] {
            assert!(!report
                .dependency_packages
                .iter()
                .any(|name| name == forbidden));
        }
        assert!(!report
            .dependency_packages
            .iter()
            .any(|name| name.starts_with("editor_")));

        let artifact = report.artifact.expect("Tower module artifact");
        let adapter =
            ProjectRuntimeNativeModuleLoader::load(&artifact).expect("load Tower module DLL");
        let linked = LinkedProjectRuntimeSet::singleton(Arc::new(adapter)).unwrap();
        let bound = ProjectRuntimeBootstrap::bind(&package, &linked)
            .expect("bind loaded Tower module to RuntimePackage");
        let mut parts = bound.into_parts();
        let mut world = World::new();
        let mut hydrator = RuntimeSceneHydrator::from_package(&package);
        let hydration = hydrator.hydrate_active_scene(&package, &mut world);
        assert!(
            !hydration.has_errors(),
            "{:?}",
            hydration.instantiate_report.diagnostics
        );

        let time = TimeContext::from_delta(1, 1.0 / 30.0, true);
        let pre_bootstrap_ui = parts.ui_state_producer.produce(
            ProjectUiStateProducerContext::new(3, &package, &world)
                .with_active_binding_paths(["tower.round_text".to_string()]),
        );
        assert_eq!(
            pre_bootstrap_ui.snapshot.values.get("tower.round_text"),
            Some(&AuiBindingValue::String("第1轮".to_string())),
            "loaded UI producer must read the hydrated tower.matchView before session bootstrap"
        );
        let bootstrap = parts
            .project_runtime_session
            .fixed_update(ProjectRuntimeSessionContext {
                frame_index: 1,
                time,
                world: WorldReadApi::new(&world),
            });
        assert!(
            matches!(
                bootstrap.status,
                ProjectRuntimeSessionStatus::Applied | ProjectRuntimeSessionStatus::NoOp
            ),
            "{bootstrap:?}"
        );
        commit(bootstrap, &mut world);

        let contract = package
            .manifest
            .observation_contract
            .as_ref()
            .expect("Tower observation contract");
        let ready_observation =
            parts
                .project_runtime_session
                .observe(ProjectRuntimeObservationContext {
                    frame_index: 1,
                    time,
                    world: WorldReadApi::new(&world),
                    contract,
                    report_level: ProjectRuntimeSessionReportLevel::Summary,
                });
        assert_eq!(
            ready_observation,
            engine_runtime::project_runtime_session::ProjectRuntimeObservationOutput::empty()
                .with_value(
                    "tower.phase",
                    engine_runtime::project_observation::ProjectObservationValue::String(
                        "organizing".to_string(),
                    ),
                )
                .with_value(
                    "tower.round",
                    engine_runtime::project_observation::ProjectObservationValue::Integer(1),
                ),
            "a no-op first fixed call is valid only for an already-ready session"
        );

        for (frame_index, action_id) in [(2, "td.recruit"), (3, "td.start-round")] {
            let actions = [AuiAction {
                action_id: action_id.to_string(),
                node_id: format!("node-{action_id}"),
                event: AuiActionEvent::Click,
                payload: None,
            }];
            let output = parts.project_runtime_session.handle_aui_actions(
                ProjectRuntimeSessionContext {
                    frame_index,
                    time,
                    world: WorldReadApi::new(&world),
                },
                ProjectAuiActionBatch::new(&actions),
            );
            assert_eq!(output.handled_action_count, 1, "{action_id}: {output:?}");
            assert_eq!(output.rejected_action_count, 0, "{action_id}: {output:?}");
            commit(output, &mut world);
        }

        let fixed = parts
            .project_runtime_session
            .fixed_update(ProjectRuntimeSessionContext {
                frame_index: 4,
                time,
                world: WorldReadApi::new(&world),
            });
        assert_eq!(fixed.status, ProjectRuntimeSessionStatus::Applied);
        assert!(
            fixed.mutations.len() > 1,
            "enemy Sprite2D mutations are retained"
        );
        commit(fixed, &mut world);

        let ui = parts.ui_state_producer.produce(
            ProjectUiStateProducerContext::new(4, &package, &world).with_active_binding_paths([
                "tower.round_text".to_string(),
                "tower.phase_text".to_string(),
                "tower.military_grain_text".to_string(),
            ]),
        );
        assert_eq!(
            ui.snapshot.values.get("tower.phase_text"),
            Some(&AuiBindingValue::String("战斗".to_string()))
        );
        assert!(ui.snapshot.values.contains_key("tower.round_text"));

        let observation = parts
            .project_runtime_session
            .observe(ProjectRuntimeObservationContext {
                frame_index: 4,
                time,
                world: WorldReadApi::new(&world),
                contract,
                report_level: ProjectRuntimeSessionReportLevel::Summary,
            });
        assert_eq!(
            observation,
            engine_runtime::project_runtime_session::ProjectRuntimeObservationOutput::empty()
                .with_value(
                    "tower.phase",
                    engine_runtime::project_observation::ProjectObservationValue::String(
                        "combat".to_string(),
                    ),
                )
                .with_value(
                    "tower.round",
                    engine_runtime::project_observation::ProjectObservationValue::Integer(1),
                )
        );

        drop(parts);
        drop(linked);
        if retain_root.is_some() {
            println!("AIFE_RETAINED_TOWER_NATIVE_ROOT={}", owned_root.display());
            println!(
                "AIFE_RETAINED_TOWER_NATIVE_ARTIFACT_ROOT={}",
                artifact.artifact_root.display()
            );
        } else {
            fs::remove_dir_all(&owned_root).unwrap();
            assert!(!owned_root.exists());
        }
    }
}

#[cfg(not(windows))]
#[test]
fn project_runtime_native_module_minimal_fixture_is_windows_only() {
    assert!(cfg!(not(windows)));
}
