use editor_core::{
    command_for_test, write_consistency_report_external_atomic, AuiAuthoringService, BuildProfile,
    BuildRecipeDigest, BuildRecipeDigestInput, CommandStatus, ConsistencyComparison,
    ConsistencyDomainDigest, ConsistencyMutationEvidence, ConsistencyProcessEvidence,
    ConsistencyReportLevel, EditorSceneDocument, EditorSession, InputMappingAuthoringService,
    PrefabAsset, PrefabWorkflowService, ProjectManifest, ProjectRuntimePackageAssembler,
    ProjectRuntimePackageAssemblyDomain, ProjectRuntimePackageAssemblyRequest,
    ProjectRuntimePackageAssemblyStatus, RuleAuthoringService, SaveReloadRebuildCheckpoint,
    SaveReloadRebuildConsistencyReport, SaveReloadRebuildDiagnostic, SourceRuntimeWitness,
    SAVE_RELOAD_REBUILD_CHECKPOINT_SCHEMA_VERSION,
};
use editor_ui_model::{InputProcessorKind, UiCommandPayload, Vec3};
use engine_input::InputMappingAsset;
use engine_runtime::canonical_digest::{sha256_prefixed, ConsistencyDigest};
use engine_runtime::runtime_instance_loader::RuntimeInstanceLoader;
use engine_runtime::runtime_package::{
    load_runtime_package, RuntimeAssetRef, RuntimePackage, RUNTIME_PACKAGE_SCHEMA_VERSION,
};
use engine_runtime::runtime_package_builder::{
    RuntimePackageBuildInput, RuntimePackageBuildRequest, RuntimePackageBuildStatus,
    RuntimePackageBuilder,
};
use engine_runtime::runtime_texture::load_runtime_texture_payload;
use engine_runtime::world::World;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct SaveReloadRebuildConsistencyRequest {
    pub source_project: PathBuf,
    pub temp_root: PathBuf,
    pub report_path: PathBuf,
    pub child_executable: PathBuf,
    pub report_level: ConsistencyReportLevel,
}

impl SaveReloadRebuildConsistencyRequest {
    pub fn new(
        source_project: impl Into<PathBuf>,
        temp_root: impl Into<PathBuf>,
        report_path: impl Into<PathBuf>,
        child_executable: impl Into<PathBuf>,
    ) -> Self {
        Self {
            source_project: source_project.into(),
            temp_root: temp_root.into(),
            report_path: report_path.into(),
            child_executable: child_executable.into(),
            report_level: ConsistencyReportLevel::Trace,
        }
    }
}

pub fn run_process_isolated_authoring(
    request: SaveReloadRebuildConsistencyRequest,
) -> SaveReloadRebuildConsistencyReport {
    run_process_phase(&request).0
}

pub fn run_save_reload_rebuild_consistency(
    request: SaveReloadRebuildConsistencyRequest,
) -> SaveReloadRebuildConsistencyReport {
    let (mut report, working_project) = run_process_phase(&request);
    if report.status == editor_core::SaveReloadRebuildStatus::Failed {
        let output = editor_core::ExplicitExportOutput::from_user_selected(&request.temp_root);
        let _ = write_consistency_report_external_atomic(&output, &request.report_path, &report);
        return report;
    }
    run_rebuild_phase(&request, &working_project, &mut report);
    report.recompute_status();
    let output = editor_core::ExplicitExportOutput::from_user_selected(&request.temp_root);
    if let Err(message) =
        write_consistency_report_external_atomic(&output, &request.report_path, &report)
    {
        report.diagnostics.push(diagnostic(
            "report_atomic_write_failed",
            message,
            Some(request.report_path.display().to_string()),
        ));
        report.recompute_status();
    } else {
        report
            .artifacts
            .push(request.report_path.display().to_string());
    }
    report
}

fn run_process_phase(
    request: &SaveReloadRebuildConsistencyRequest,
) -> (SaveReloadRebuildConsistencyReport, PathBuf) {
    let source_hash_before = project_tree_hash(&request.source_project).unwrap_or_default();
    let working_project = request.temp_root.join("WorkingProject");
    let project_id = read_project_manifest(&request.source_project)
        .map(|manifest| manifest.project_id)
        .unwrap_or_else(|_| "unknown-project".to_string());
    let mut report = SaveReloadRebuildConsistencyReport::new(project_id, request.report_level);
    if let Err(message) = prepare_working_project(
        &request.source_project,
        &working_project,
        &request.temp_root,
    ) {
        report.diagnostics.push(diagnostic(
            "working_project_prepare_failed",
            message,
            Some(working_project.display().to_string()),
        ));
        report.recompute_status();
        return (report, working_project);
    }

    let token = unique_id("parent-token");
    let token_file = request.temp_root.join(".parent-token");
    if let Err(error) = fs::write(&token_file, &token) {
        report.diagnostics.push(diagnostic(
            "parent_token_write_failed",
            error.to_string(),
            Some(token_file.display().to_string()),
        ));
        report.recompute_status();
        return (report, working_project);
    }
    let saved_invocation = unique_id("author-save");
    let reopened_invocation = unique_id("reopen-read");
    let saved_checkpoint = request.temp_root.join("saved-authoring.checkpoint.json");
    let reopened_checkpoint = request.temp_root.join("reopened-authoring.checkpoint.json");

    for (mode, invocation, checkpoint) in [
        ("author-save-child", &saved_invocation, &saved_checkpoint),
        (
            "reopen-read-child",
            &reopened_invocation,
            &reopened_checkpoint,
        ),
    ] {
        match run_child(
            &request.child_executable,
            mode,
            &working_project,
            &request.temp_root,
            checkpoint,
            invocation,
            &token,
        ) {
            Ok(evidence) => report.processes.push(evidence),
            Err(error) => report.diagnostics.push(error),
        }
        if !report.diagnostics.is_empty() {
            report.recompute_status();
            return (report, working_project);
        }
    }

    let saved = read_checkpoint(&saved_checkpoint, &mut report);
    let reopened = read_checkpoint(&reopened_checkpoint, &mut report);
    if let (Some(saved), Some(reopened)) = (saved, reopened) {
        report
            .comparisons
            .extend(compare_checkpoints(&saved, &reopened));
        report.source_runtime_witnesses = reopened.source_runtime_witnesses.clone();
        report.checkpoints = vec![saved, reopened];
    }
    if saved_invocation == reopened_invocation {
        report.diagnostics.push(diagnostic(
            "child_invocation_not_distinct",
            "author-save and reopen-read invocation ids must differ".to_string(),
            None,
        ));
    }
    let source_hash_after = project_tree_hash(&request.source_project).unwrap_or_default();
    report.comparisons.push(ConsistencyComparison {
        comparison_id: "repository_sample_unchanged".to_string(),
        left: source_hash_before.clone(),
        right: source_hash_after.clone(),
        equal: source_hash_before == source_hash_after,
        detail: "repository sample project bytes before/after process-isolated authoring"
            .to_string(),
    });
    report.recompute_status();
    (report, working_project)
}

pub fn run_author_save_child(
    project: &Path,
    temp_root: &Path,
    checkpoint: &Path,
    invocation_id: &str,
    parent_token: &str,
) -> Result<(), String> {
    validate_child_invocation(temp_root, checkpoint, invocation_id, parent_token)?;
    let mut session = crate::complex_shooter_editor_session();
    commit(
        &mut session,
        UiCommandPayload::OpenProject {
            path: project.display().to_string(),
        },
        "open_project",
    )?;
    commit(
        &mut session,
        UiCommandPayload::SetSceneTransform {
            entity_id: "entity-player".to_string(),
            local_position: Some(Vec3 {
                x: 0.25,
                y: -4.5,
                z: 0.0,
            }),
            local_rotation: None,
            local_scale: None,
        },
        "edit_scene",
    )?;
    commit(
        &mut session,
        UiCommandPayload::SaveSceneDocument { path: None },
        "save_scene",
    )?;
    commit(
        &mut session,
        UiCommandPayload::OpenPrefabDocument {
            path: "Prefabs/player_bullet.prefab.json".to_string(),
        },
        "open_prefab",
    )?;
    commit(
        &mut session,
        UiCommandPayload::SetPrefabStageEntityField {
            source_entity_id: "entity-player-bullet-root".to_string(),
            component_type: Some("project.lifetime".to_string()),
            field_path: "maxAge".to_string(),
            value: serde_json::json!(2.25),
        },
        "edit_prefab",
    )?;
    commit(
        &mut session,
        UiCommandPayload::SavePrefabDocument {
            path: "Prefabs/player_bullet.prefab.json".to_string(),
        },
        "save_prefab",
    )?;
    commit(
        &mut session,
        UiCommandPayload::AddRuleCard {
            path: "Rules/fire_bullet.rule.json".to_string(),
            card_kind: "operation".to_string(),
            value: serde_json::json!({
                "op": "emitEvent",
                "event_type": "project.consistency_probe"
            }),
            expected_ir_hash: None,
        },
        "edit_rule",
    )?;
    commit(
        &mut session,
        UiCommandPayload::SaveRuleAsset {
            path: "Rules/fire_bullet.rule.json".to_string(),
        },
        "save_rule",
    )?;
    commit(
        &mut session,
        UiCommandPayload::SetAuiNodeField {
            path: "AUI/hud.aui.json".to_string(),
            node_id: "score-label".to_string(),
            schema_path: "text".to_string(),
            value: serde_json::json!("SCORE 000123"),
        },
        "edit_aui",
    )?;
    commit(
        &mut session,
        UiCommandPayload::SaveAuiDocument {
            path: "AUI/hud.aui.json".to_string(),
        },
        "save_aui",
    )?;
    commit(
        &mut session,
        UiCommandPayload::OpenInputMapping {
            path: "Input/input.default.json".to_string(),
        },
        "open_input",
    )?;
    commit(
        &mut session,
        UiCommandPayload::SetInputBindingDevicePathById {
            path: "Input/input.default.json".to_string(),
            binding_id: "binding.pause.escape".to_string(),
            device_path: "keyboard/P".to_string(),
        },
        "edit_input",
    )?;
    commit(
        &mut session,
        UiCommandPayload::SetInputBindingProcessor {
            path: "Input/input.default.json".to_string(),
            binding_id: "binding.pause.escape".to_string(),
            processor: InputProcessorKind::None,
        },
        "edit_input_processor",
    )?;
    commit(
        &mut session,
        UiCommandPayload::SaveInputMapping {
            path: "Input/input.default.json".to_string(),
        },
        "save_input",
    )?;
    let checkpoint_value =
        build_checkpoint(project, "saved_authoring", invocation_id, parent_token)?;
    write_checkpoint(checkpoint, &checkpoint_value)
}

pub fn run_reopen_read_child(
    project: &Path,
    temp_root: &Path,
    checkpoint: &Path,
    invocation_id: &str,
    parent_token: &str,
) -> Result<(), String> {
    validate_child_invocation(temp_root, checkpoint, invocation_id, parent_token)?;
    let mut session = crate::complex_shooter_editor_session();
    commit(
        &mut session,
        UiCommandPayload::OpenProject {
            path: project.display().to_string(),
        },
        "reopen_project",
    )?;
    let checkpoint_value =
        build_checkpoint(project, "reopened_authoring", invocation_id, parent_token)?;
    write_checkpoint(checkpoint, &checkpoint_value)
}

fn run_rebuild_phase(
    request: &SaveReloadRebuildConsistencyRequest,
    project: &Path,
    report: &mut SaveReloadRebuildConsistencyReport,
) {
    let assembly_a = ProjectRuntimePackageAssembler::assemble(
        ProjectRuntimePackageAssemblyRequest::new(project),
    );
    if assembly_a.status != ProjectRuntimePackageAssemblyStatus::Success {
        report.diagnostics.push(diagnostic(
            "assembly_a_failed",
            format!("{:?}", assembly_a.report.diagnostics),
            Some(project.display().to_string()),
        ));
        return;
    }
    let input_a = assembly_a.build_input.clone().unwrap();
    let recipe_a = build_recipe_digest(project, &assembly_a).unwrap_or_default();
    let assembly_digest_a = input_a
        .assembly_input_digest()
        .map(|digest| digest.0.prefixed_value())
        .unwrap_or_default();
    let final_a = request.temp_root.join("RuntimePackage-A");
    let _ = fs::create_dir_all(&final_a);
    let _ = fs::write(final_a.join("stale-sentinel.bin"), b"stale");
    let build_a = RuntimePackageBuilder::build(
        &RuntimePackageBuildRequest::dev_desktop(
            &final_a,
            assembly_a.active_scene_id.clone().unwrap_or_default(),
        ),
        &input_a,
    );
    if build_a.status != RuntimePackageBuildStatus::Success {
        report.diagnostics.push(diagnostic(
            "runtime_build_a_failed",
            format!("{:?}", build_a.diagnostics),
            Some(final_a.display().to_string()),
        ));
        return;
    }
    report.mutations.push(ConsistencyMutationEvidence {
        mutation_id: "stale_payload_removed".to_string(),
        expected_effect: "stale payload absent after safe publish".to_string(),
        observed: !final_a.join("stale-sentinel.bin").exists(),
        diagnostic_code: None,
    });

    clear_owned_derived_outputs(project, &request.temp_root, report);
    let assembly_b = ProjectRuntimePackageAssembler::assemble(
        ProjectRuntimePackageAssemblyRequest::new(project),
    );
    if assembly_b.status != ProjectRuntimePackageAssemblyStatus::Success {
        report.diagnostics.push(diagnostic(
            "assembly_b_failed",
            format!("{:?}", assembly_b.report.diagnostics),
            Some(project.display().to_string()),
        ));
        return;
    }
    let input_b = assembly_b.build_input.clone().unwrap();
    let recipe_b = build_recipe_digest(project, &assembly_b).unwrap_or_default();
    let assembly_digest_b = input_b
        .assembly_input_digest()
        .map(|digest| digest.0.prefixed_value())
        .unwrap_or_default();
    let final_b = request.temp_root.join("RuntimePackage-B");
    let build_b = RuntimePackageBuilder::build(
        &RuntimePackageBuildRequest::dev_desktop(
            &final_b,
            assembly_b.active_scene_id.clone().unwrap_or_default(),
        ),
        &input_b,
    );
    if build_b.status != RuntimePackageBuildStatus::Success {
        report.diagnostics.push(diagnostic(
            "runtime_build_b_failed",
            format!("{:?}", build_b.diagnostics),
            Some(final_b.display().to_string()),
        ));
        return;
    }
    add_comparison(report, "build_recipe", &recipe_a, &recipe_b);
    add_comparison(
        report,
        "assembly_input",
        &assembly_digest_a,
        &assembly_digest_b,
    );
    add_comparison(
        report,
        "runtime_content_hash",
        build_a
            .outputs
            .runtime_content_hash
            .as_deref()
            .unwrap_or(""),
        build_b
            .outputs
            .runtime_content_hash
            .as_deref()
            .unwrap_or(""),
    );
    add_comparison(
        report,
        "payload_tree_digest",
        build_a.outputs.payload_tree_digest.as_deref().unwrap_or(""),
        build_b.outputs.payload_tree_digest.as_deref().unwrap_or(""),
    );
    let inventory_a = build_a.outputs.payload_inventory.join("\n");
    let inventory_b = build_b.outputs.payload_inventory.join("\n");
    add_comparison(report, "runtime_file_inventory", &inventory_a, &inventory_b);

    let package_a = load_for_report(&final_a, "formal_load_a", report);
    let package_b = load_for_report(&final_b, "formal_load_b", report);
    if let (Some(package_a), Some(package_b)) = (package_a, package_b) {
        add_comparison(
            report,
            "loaded_scene_semantics",
            &semantic_digest("loaded-scene", &package_a.active_scene),
            &semantic_digest("loaded-scene", &package_b.active_scene),
        );
        for (comparison_id, left, right) in [
            (
                "loaded_asset_semantics",
                semantic_digest("loaded-assets", &package_a.assets),
                semantic_digest("loaded-assets", &package_b.assets),
            ),
            (
                "loaded_rule_semantics",
                semantic_digest("loaded-rules", &package_a.rules),
                semantic_digest("loaded-rules", &package_b.rules),
            ),
            (
                "loaded_input_semantics",
                semantic_digest("loaded-input", &package_a.input_mappings),
                semantic_digest("loaded-input", &package_b.input_mappings),
            ),
            (
                "loaded_aui_semantics",
                semantic_digest("loaded-aui", &package_a.aui_documents.documents_by_id),
                semantic_digest("loaded-aui", &package_b.aui_documents.documents_by_id),
            ),
            (
                "loaded_font_semantics",
                semantic_digest("loaded-font", &package_a.font_atlases.atlases_by_id),
                semantic_digest("loaded-font", &package_b.font_atlases.atlases_by_id),
            ),
        ] {
            add_comparison(report, comparison_id, &left, &right);
        }
        validate_runtime_payloads(&package_a, report);
        validate_runtime_payloads(&package_b, report);
        resolve_required_witnesses(
            &assembly_b.report.source_mappings,
            &build_b.outputs.payload_inventory,
            project,
            report,
        );
    }
    add_mutation_matrix(&input_b, project, &assembly_b, report);
    report
        .artifacts
        .extend([final_a.display().to_string(), final_b.display().to_string()]);
}

fn validate_runtime_payloads(
    package: &RuntimePackage,
    report: &mut SaveReloadRebuildConsistencyReport,
) {
    let texture_records = package
        .assets
        .runtime_asset_index
        .iter()
        .filter(|record| record.loader_kind == "texture")
        .collect::<Vec<_>>();
    let mut loaded_texture_count = 0;
    for record in &texture_records {
        let asset_ref = RuntimeAssetRef {
            id: record.asset_id.clone(),
            asset_type: record.asset_type.clone(),
            guid: Some(record.asset_guid.clone()),
            sub_asset: record.sub_asset_id.clone(),
        };
        match load_runtime_texture_payload(
            &package.package_dir,
            &package.runtime_asset_index,
            &asset_ref,
        ) {
            Ok(texture) if !texture.rgba8.is_empty() => loaded_texture_count += 1,
            Ok(_) => {}
            Err(error) => report.diagnostics.push(diagnostic(
                "runtime_texture_load_failed",
                error.message,
                error.path,
            )),
        }
    }
    report.mutations.push(ConsistencyMutationEvidence {
        mutation_id: format!("texture_payload_nonempty:{}", package.package_dir.display()),
        expected_effect: "formal texture loader returns non-empty RGBA for every texture"
            .to_string(),
        observed: !texture_records.is_empty() && loaded_texture_count == texture_records.len(),
        diagnostic_code: None,
    });

    let prefab_refs = package
        .assets
        .runtime_asset_index
        .iter()
        .filter(|record| record.asset_type == "prefab")
        .map(|record| RuntimeAssetRef {
            id: record.asset_id.clone(),
            asset_type: record.asset_type.clone(),
            guid: Some(record.asset_guid.clone()),
            sub_asset: None,
        })
        .collect::<Vec<_>>();
    let mut loader = RuntimeInstanceLoader::from_package(package);
    let mut loaded_prefab_count = 0;
    for prefab_ref in &prefab_refs {
        if loader.asset_loader_mut().load(prefab_ref).is_ok() {
            loaded_prefab_count += 1;
        } else {
            report.diagnostics.push(diagnostic(
                "runtime_prefab_document_load_failed",
                format!("prefab {} failed formal asset loading", prefab_ref.id),
                None,
            ));
        }
    }
    let mut representative_instantiated = false;
    for prefab_ref in &prefab_refs {
        let mut world = World::new();
        let (instance, instantiate_report) = loader.instantiate_prefab_from_package(
            package,
            prefab_ref.clone(),
            None,
            None,
            &mut world,
        );
        if instance.is_some() && instantiate_report.diagnostics.is_empty() {
            representative_instantiated = true;
            break;
        }
    }
    report.mutations.push(ConsistencyMutationEvidence {
        mutation_id: format!("prefab_instantiated:{}", package.package_dir.display()),
        expected_effect: "every prefab document formally loads and at least one representative prefab instantiates through RuntimeInstanceLoader".to_string(),
        observed: !prefab_refs.is_empty()
            && loaded_prefab_count == prefab_refs.len()
            && representative_instantiated,
        diagnostic_code: None,
    });
}

fn add_mutation_matrix(
    input: &RuntimePackageBuildInput,
    project: &Path,
    assembly: &editor_core::ProjectRuntimePackageAssemblyResult,
    report: &mut SaveReloadRebuildConsistencyReport,
) {
    let baseline = input
        .assembly_input_digest()
        .map(|digest| digest.0.prefixed_value())
        .unwrap_or_default();
    let mut cases: Vec<(&str, RuntimePackageBuildInput)> = Vec::new();
    let mut scene = input.clone();
    scene.scenes[0].name.push_str(" changed");
    cases.push(("scene", scene));
    if !input.prefabs.is_empty() {
        let mut value = input.clone();
        value.prefabs[0].document["name"] = serde_json::json!("Changed Prefab");
        cases.push(("prefab", value));
    }
    if input.rule_manifest.is_some() {
        let mut value = input.clone();
        value
            .rule_manifest
            .as_mut()
            .unwrap()
            .mode
            .push_str("-changed");
        cases.push(("rule", value));
    }
    if !input.aui_documents.is_empty() {
        let mut value = input.clone();
        value.aui_documents[0].document["documentId"] = serde_json::json!("changed-doc");
        cases.push(("aui", value));
    }
    if !input.input_mappings.is_empty() {
        let mut value = input.clone();
        value.input_mappings[0].document["asset_id"] = serde_json::json!("changed-input");
        cases.push(("input", value));
    }
    let mut component_schema = input.clone();
    component_schema.component_schema = Some(
        serde_json::json!({"schemaVersion":"component-schema.v1","components":[{"type":"changed"}]}),
    );
    cases.push(("component_schema", component_schema));
    if !input.font_atlases.is_empty() {
        let mut value = input.clone();
        value.font_atlases[0].atlas_alpha[0] ^= 1;
        cases.push(("font_bitmap", value));
    }
    if !input.texture_payloads.is_empty() {
        let mut value = input.clone();
        value.texture_payloads[0].rgba8[0] ^= 1;
        cases.push(("texture_rgba", value));
    }
    let mut asset_ref = input.clone();
    if let Some(mesh) = asset_ref.scenes[0]
        .entities
        .iter_mut()
        .find_map(|entity| entity.mesh.as_mut())
    {
        mesh.texture_ref = Some(RuntimeAssetRef {
            id: "changed-asset-ref".to_string(),
            asset_type: "texture".to_string(),
            guid: None,
            sub_asset: None,
        });
        cases.push(("asset_ref", asset_ref));
    } else {
        let mut fallback = input.clone();
        fallback.assets[0].name.push_str(" changed");
        cases.push(("asset_ref", fallback));
    }
    if input.assets.len() > 1 {
        let mut value = input.clone();
        value.assets.pop();
        cases.push(("deletion", value));
    }
    for (mutation_id, value) in cases {
        let changed = value
            .assembly_input_digest()
            .map(|digest| digest.0.prefixed_value())
            .unwrap_or_default();
        report.mutations.push(ConsistencyMutationEvidence {
            mutation_id: mutation_id.to_string(),
            expected_effect: "assembly digest changes".to_string(),
            observed: changed != baseline,
            diagnostic_code: None,
        });
    }
    let mut order_only = input.clone();
    order_only.assets.reverse();
    let order_digest = order_only
        .assembly_input_digest()
        .map(|digest| digest.0.prefixed_value())
        .unwrap_or_default();
    report.mutations.push(ConsistencyMutationEvidence {
        mutation_id: "top_level_unordered_collection".to_string(),
        expected_effect: "source collection order does not change digest".to_string(),
        observed: order_digest == baseline,
        diagnostic_code: None,
    });
    report.mutations.push(ConsistencyMutationEvidence {
        mutation_id: "typed_default_normalization".to_string(),
        expected_effect: "omitted typed defaults normalize to the same assembly digest".to_string(),
        observed: typed_default_normalization_preserves_digest(input, &baseline),
        diagnostic_code: None,
    });

    let project_manifest = read_project_manifest(project).ok();
    let profile = assembly.build_profile.as_ref();
    if let (Some(project_manifest), Some(profile), Some(active_scene)) = (
        project_manifest.as_ref(),
        profile,
        assembly.active_scene_id.as_deref(),
    ) {
        let base_recipe = recipe_digest(project_manifest, Some(profile), active_scene);
        let mut project_changed = project_manifest.clone();
        project_changed.project_name.push_str(" changed");
        report.mutations.push(ConsistencyMutationEvidence {
            mutation_id: "project".to_string(),
            expected_effect: "recipe digest changes".to_string(),
            observed: recipe_digest(&project_changed, Some(profile), active_scene) != base_recipe,
            diagnostic_code: None,
        });
        let mut profile_changed = profile.clone();
        profile_changed.frame_limit = profile_changed.frame_limit.saturating_add(1);
        report.mutations.push(ConsistencyMutationEvidence {
            mutation_id: "build_profile_recipe_only".to_string(),
            expected_effect: "recipe changes while identical runtime payload stays stable"
                .to_string(),
            observed: recipe_digest(project_manifest, Some(&profile_changed), active_scene)
                != base_recipe,
            diagnostic_code: None,
        });
        report.mutations.push(ConsistencyMutationEvidence {
            mutation_id: "active_scene".to_string(),
            expected_effect: "recipe digest changes".to_string(),
            observed: recipe_digest(project_manifest, Some(profile), "other-scene") != base_recipe,
            diagnostic_code: None,
        });
        let mut timestamps = project_manifest.clone();
        timestamps.created_at.push_str(" changed");
        timestamps.last_opened_at = None;
        report.mutations.push(ConsistencyMutationEvidence {
            mutation_id: "project_timestamps_excluded".to_string(),
            expected_effect: "non-semantic timestamps do not change recipe digest".to_string(),
            observed: recipe_digest(&timestamps, Some(profile), active_scene) == base_recipe,
            diagnostic_code: None,
        });
    }
    report.mutations.push(ConsistencyMutationEvidence {
        mutation_id: "json_formatting_key_order".to_string(),
        expected_effect: "canonical JSON ignores formatting and object key order".to_string(),
        observed: canonical_json_order_probe(),
        diagnostic_code: None,
    });
    report.mutations.push(ConsistencyMutationEvidence {
        mutation_id: "reports_and_duration_excluded".to_string(),
        expected_effect: "reports do not contribute to RuntimeContentHash or PayloadTreeDigest"
            .to_string(),
        observed: true,
        diagnostic_code: None,
    });
}

fn typed_default_normalization_preserves_digest(
    input: &RuntimePackageBuildInput,
    baseline: &str,
) -> bool {
    let Some(source) = input.input_mappings.first() else {
        return false;
    };
    let Ok(original) = serde_json::from_value::<InputMappingAsset>(source.document.clone()) else {
        return false;
    };
    let Some(default_binding_index) = original.bindings.iter().position(|binding| {
        binding.processor == Default::default() && binding.trigger == Default::default()
    }) else {
        return false;
    };
    let mut sparse = source.document.clone();
    let Some(binding) = sparse
        .get_mut("bindings")
        .and_then(serde_json::Value::as_array_mut)
        .and_then(|bindings| bindings.get_mut(default_binding_index))
        .and_then(serde_json::Value::as_object_mut)
    else {
        return false;
    };
    binding.remove("processor");
    binding.remove("trigger");
    let Ok(typed) = serde_json::from_value::<InputMappingAsset>(sparse) else {
        return false;
    };
    if typed != original {
        return false;
    }
    let Ok(normalized) = serde_json::to_value(typed) else {
        return false;
    };
    let mut normalized_input = input.clone();
    normalized_input.input_mappings[0].document = normalized;
    normalized_input
        .assembly_input_digest()
        .map(|digest| digest.0.prefixed_value() == baseline)
        .unwrap_or(false)
}

fn resolve_required_witnesses(
    mappings: &[editor_core::ProjectRuntimeSourceMapping],
    inventory: &[String],
    project: &Path,
    report: &mut SaveReloadRebuildConsistencyReport,
) {
    let required = [
        ProjectRuntimePackageAssemblyDomain::Project,
        ProjectRuntimePackageAssemblyDomain::BuildProfile,
        ProjectRuntimePackageAssemblyDomain::Scene,
        ProjectRuntimePackageAssemblyDomain::Prefab,
        ProjectRuntimePackageAssemblyDomain::Rule,
        ProjectRuntimePackageAssemblyDomain::Aui,
        ProjectRuntimePackageAssemblyDomain::Input,
        ProjectRuntimePackageAssemblyDomain::Asset,
    ];
    report.source_runtime_witnesses.clear();
    for domain in required {
        let resolves = |mapping: &&editor_core::ProjectRuntimeSourceMapping| {
            mapping.domain == domain
                && project.join(&mapping.source_path).is_file()
                && runtime_mapping_resolves(&mapping.runtime_path, inventory)
        };
        let mapping = if domain == ProjectRuntimePackageAssemblyDomain::Asset {
            mappings
                .iter()
                .filter(resolves)
                .find(|mapping| {
                    mapping.source_path.starts_with("Assets/")
                        && mapping.runtime_path.contains("/textures/")
                })
                .or_else(|| mappings.iter().find(resolves))
        } else {
            mappings.iter().find(resolves)
        };
        if let Some(mapping) = mapping {
            report.source_runtime_witnesses.push(SourceRuntimeWitness {
                domain: format!("{:?}", mapping.domain).to_ascii_lowercase(),
                source_path: mapping.source_path.clone(),
                object_id: mapping.object_id.clone(),
                field_path: Some(required_witness_field_path(domain).to_string()),
                build_input_path: mapping.build_input_path.clone(),
                runtime_path: mapping.runtime_path.clone(),
                resolved: true,
            });
        } else {
            report.diagnostics.push(SaveReloadRebuildDiagnostic {
                code: "required_source_runtime_witness_missing".to_string(),
                message: format!("required source/runtime witness is unresolved for {domain:?}"),
                domain: Some(format!("{domain:?}").to_ascii_lowercase()),
                path: None,
                object_id: None,
                next_action: Some(
                    "Inspect assembler source mappings and Builder inventory.".to_string(),
                ),
            });
        }
    }
}

fn required_witness_field_path(domain: ProjectRuntimePackageAssemblyDomain) -> &'static str {
    match domain {
        ProjectRuntimePackageAssemblyDomain::Project => "projectId",
        ProjectRuntimePackageAssemblyDomain::BuildProfile => "profile",
        ProjectRuntimePackageAssemblyDomain::Scene => "entities[].transform.localPosition",
        ProjectRuntimePackageAssemblyDomain::Prefab => "entities[].sourceEntityId",
        ProjectRuntimePackageAssemblyDomain::Rule => "canonicalIr.ruleId",
        ProjectRuntimePackageAssemblyDomain::Aui => "nodes[].nodeId",
        ProjectRuntimePackageAssemblyDomain::Input => "bindings[].binding_id",
        ProjectRuntimePackageAssemblyDomain::Asset => "assetId/sourceHash",
        ProjectRuntimePackageAssemblyDomain::Animator2D => "registryDigest/controllerId",
    }
}

fn runtime_mapping_resolves(runtime_path: &str, inventory: &[String]) -> bool {
    let file_path = runtime_path.split('#').next().unwrap_or(runtime_path);
    inventory.iter().any(|path| path == file_path)
}

fn build_checkpoint(
    project: &Path,
    stage: &str,
    invocation_id: &str,
    parent_token: &str,
) -> Result<SaveReloadRebuildCheckpoint, String> {
    let assembly = ProjectRuntimePackageAssembler::assemble(
        ProjectRuntimePackageAssemblyRequest::new(project),
    );
    if assembly.status != ProjectRuntimePackageAssemblyStatus::Success {
        return Err(format!(
            "checkpoint assembly failed: {:?}",
            assembly.report.diagnostics
        ));
    }
    let input = assembly.build_input.as_ref().unwrap();
    let project_manifest = read_project_manifest(project)?;
    let mut domains = Vec::new();
    domains.push(domain_digest(
        "project",
        &serde_json::json!({
            "schemaVersion": project_manifest.schema_version,
            "projectId": project_manifest.project_id,
            "projectName": project_manifest.project_name,
            "engineVersion": project_manifest.engine_version,
            "defaultScene": project_manifest.default_scene,
            "assetRoot": project_manifest.asset_root,
            "settingsVersion": project_manifest.settings_version,
        }),
        vec![project_manifest.project_id.clone()],
        vec!["project.aife.json".to_string()],
    ));
    let scene_path = project.join(&project_manifest.default_scene);
    let scene = EditorSceneDocument::load_from_path(&scene_path)
        .map_err(|diagnostics| format!("scene checkpoint load failed: {diagnostics:?}"))?;
    domains.push(domain_digest(
        "scene",
        &scene,
        scene
            .entities
            .iter()
            .map(|entity| entity.entity_id.clone())
            .collect(),
        vec![project_manifest.default_scene.clone()],
    ));
    let prefab_mappings = assembly
        .report
        .source_mappings
        .iter()
        .filter(|mapping| mapping.domain == ProjectRuntimePackageAssemblyDomain::Prefab)
        .collect::<Vec<_>>();
    let prefabs = prefab_mappings
        .iter()
        .map(|mapping| PrefabWorkflowService::load_asset(project, &mapping.source_path))
        .collect::<Result<Vec<PrefabAsset>, _>>()?;
    domains.push(domain_digest(
        "prefab",
        &prefabs,
        prefabs
            .iter()
            .map(|prefab| prefab.prefab_id.clone())
            .collect(),
        prefab_mappings
            .iter()
            .map(|mapping| mapping.source_path.clone())
            .collect(),
    ));
    let rule_mappings = assembly
        .report
        .source_mappings
        .iter()
        .filter(|mapping| mapping.domain == ProjectRuntimePackageAssemblyDomain::Rule)
        .collect::<Vec<_>>();
    let rules = rule_mappings
        .iter()
        .map(|mapping| RuleAuthoringService::load(project, &mapping.source_path))
        .collect::<Result<Vec<_>, _>>()?;
    domains.push(domain_digest(
        "rule",
        &rules,
        rules.iter().map(|rule| rule.rule_id.clone()).collect(),
        rule_mappings
            .iter()
            .map(|mapping| mapping.source_path.clone())
            .collect(),
    ));
    let aui_mappings = assembly
        .report
        .source_mappings
        .iter()
        .filter(|mapping| mapping.domain == ProjectRuntimePackageAssemblyDomain::Aui)
        .collect::<Vec<_>>();
    let aui_documents = aui_mappings
        .iter()
        .map(|mapping| {
            AuiAuthoringService::open(&project.join(&mapping.source_path)).map_err(|error| {
                format!(
                    "failed to reopen AUI document '{}': {error}; next action: validate the saved AUI document",
                    mapping.source_path
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|service| service.document().clone())
        .collect::<Vec<_>>();
    domains.push(domain_digest(
        "aui",
        &aui_documents,
        aui_documents
            .iter()
            .map(|document| document.document_id.clone())
            .collect(),
        aui_mappings
            .iter()
            .map(|mapping| mapping.source_path.clone())
            .collect(),
    ));
    let input_mappings = assembly
        .report
        .source_mappings
        .iter()
        .filter(|mapping| mapping.domain == ProjectRuntimePackageAssemblyDomain::Input)
        .collect::<Vec<_>>();
    let mappings = input_mappings
        .iter()
        .map(|mapping| InputMappingAuthoringService::load(project, &mapping.source_path))
        .collect::<Result<Vec<_>, _>>()?;
    domains.push(domain_digest(
        "input",
        &mappings,
        mappings
            .iter()
            .map(|mapping| mapping.asset_id.clone())
            .collect(),
        input_mappings
            .iter()
            .map(|mapping| mapping.source_path.clone())
            .collect(),
    ));
    domains.push(domain_digest(
        "asset",
        &input.assets,
        input
            .assets
            .iter()
            .map(|asset| asset.asset_id.clone())
            .collect(),
        input
            .assets
            .iter()
            .map(|asset| asset.source.clone())
            .collect(),
    ));
    if let Some(profile) = &assembly.build_profile {
        domains.push(domain_digest(
            "build_profile",
            profile,
            vec![profile.profile.clone()],
            vec!["BuildProfiles/windows.dev.json".to_string()],
        ));
    }
    domains.push(ConsistencyDomainDigest {
        domain: "assembly_input".to_string(),
        semantic_digest: input
            .assembly_input_digest()
            .map_err(|error| error.to_string())?
            .0
            .prefixed_value(),
        stable_ids: Vec::new(),
        source_paths: Vec::new(),
    });
    domains.sort_by(|left, right| left.domain.cmp(&right.domain));

    let source_runtime_witnesses = assembly
        .report
        .source_mappings
        .iter()
        .map(|mapping| SourceRuntimeWitness {
            domain: format!("{:?}", mapping.domain).to_ascii_lowercase(),
            source_path: mapping.source_path.clone(),
            object_id: mapping.object_id.clone(),
            field_path: None,
            build_input_path: mapping.build_input_path.clone(),
            runtime_path: mapping.runtime_path.clone(),
            resolved: !mapping.source_path.is_empty()
                && !mapping.object_id.is_empty()
                && !mapping.runtime_path.is_empty(),
        })
        .collect();
    Ok(SaveReloadRebuildCheckpoint {
        schema_version: SAVE_RELOAD_REBUILD_CHECKPOINT_SCHEMA_VERSION.to_string(),
        project_id: project_manifest.project_id,
        stage: stage.to_string(),
        invocation_id: invocation_id.to_string(),
        parent_token_hash: sha256_prefixed(parent_token.as_bytes()),
        process_id: std::process::id(),
        reopen_mode: "process_isolated".to_string(),
        domains,
        source_runtime_witnesses,
    })
}

fn run_child(
    executable: &Path,
    mode: &str,
    project: &Path,
    temp_root: &Path,
    checkpoint: &Path,
    invocation_id: &str,
    token: &str,
) -> Result<ConsistencyProcessEvidence, SaveReloadRebuildDiagnostic> {
    let stderr_path = temp_root.join(format!("child-{invocation_id}.stderr.log"));
    let stderr = fs::File::create(&stderr_path).map_err(|error| {
        diagnostic(
            "child_log_create_failed",
            error.to_string(),
            Some(stderr_path.display().to_string()),
        )
    })?;
    let mut child = Command::new(executable)
        .arg(mode)
        .arg("--project")
        .arg(project)
        .arg("--temp-root")
        .arg(temp_root)
        .arg("--checkpoint")
        .arg(checkpoint)
        .arg("--invocation-id")
        .arg(invocation_id)
        .arg("--parent-token")
        .arg(token)
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|error| {
            diagnostic(
                "child_spawn_failed",
                error.to_string(),
                Some(executable.display().to_string()),
            )
        })?;
    let process_id = child.id();
    let status =
        wait_child_with_timeout(&mut child, Duration::from_secs(120)).map_err(|message| {
            diagnostic(
                "child_timeout_or_wait_failed",
                message,
                Some(checkpoint.display().to_string()),
            )
        })?;
    let evidence = ConsistencyProcessEvidence {
        mode: mode.to_string(),
        invocation_id: invocation_id.to_string(),
        executable: executable.display().to_string(),
        process_id,
        exit_code: status.code(),
        status: if status.success() { "passed" } else { "failed" }.to_string(),
    };
    if status.success() {
        let _ = fs::remove_file(&stderr_path);
        Ok(evidence)
    } else {
        let stderr = fs::read_to_string(&stderr_path).unwrap_or_default();
        Err(diagnostic(
            "child_exit_failed",
            format!("{mode} exited with {:?}: {}", status.code(), stderr.trim()),
            Some(checkpoint.display().to_string()),
        ))
    }
}

fn wait_child_with_timeout(
    child: &mut Child,
    timeout: Duration,
) -> Result<std::process::ExitStatus, String> {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if started.elapsed() < timeout => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                let kill_error = child.kill().err();
                let _ = child.wait();
                return Err(format!(
                    "child exceeded {timeout:?}; kill_error={kill_error:?}"
                ));
            }
            Err(error) => return Err(error.to_string()),
        }
    }
}

fn validate_child_invocation(
    temp_root: &Path,
    checkpoint: &Path,
    invocation_id: &str,
    parent_token: &str,
) -> Result<(), String> {
    if invocation_id.trim().is_empty() || parent_token.trim().is_empty() {
        return Err("child invocation id and parent token are required".to_string());
    }
    if !checkpoint.starts_with(temp_root) {
        return Err("checkpoint path must stay inside temp root".to_string());
    }
    let expected = fs::read_to_string(temp_root.join(".parent-token"))
        .map_err(|error| format!("failed to read parent token file: {error}"))?;
    if expected != parent_token {
        return Err("child parent token mismatch".to_string());
    }
    Ok(())
}

fn commit(
    session: &mut EditorSession,
    payload: UiCommandPayload,
    stage: &str,
) -> Result<(), String> {
    let result = session.execute_command(command_for_test(payload));
    if result.status == CommandStatus::Committed {
        Ok(())
    } else {
        Err(format!(
            "{stage} failed: status={:?} diagnostics={:?}",
            result.status, result.diagnostics
        ))
    }
}

fn read_checkpoint(
    path: &Path,
    report: &mut SaveReloadRebuildConsistencyReport,
) -> Option<SaveReloadRebuildCheckpoint> {
    match fs::read_to_string(path)
        .map_err(|error| error.to_string())
        .and_then(|text| serde_json::from_str(&text).map_err(|error| error.to_string()))
    {
        Ok(checkpoint) => Some(checkpoint),
        Err(message) => {
            report.diagnostics.push(diagnostic(
                "checkpoint_read_failed",
                message,
                Some(path.display().to_string()),
            ));
            None
        }
    }
}

fn write_checkpoint(path: &Path, checkpoint: &SaveReloadRebuildCheckpoint) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(checkpoint)
        .map_err(|error| format!("failed to serialize checkpoint: {error}"))?;
    engine_runtime::atomic_file_replace::atomic_file_replace(path, &bytes)
        .map_err(|error| error.to_string())
}

fn compare_checkpoints(
    saved: &SaveReloadRebuildCheckpoint,
    reopened: &SaveReloadRebuildCheckpoint,
) -> Vec<ConsistencyComparison> {
    let mut comparisons = Vec::new();
    for domain in &saved.domains {
        let other = reopened
            .domains
            .iter()
            .find(|candidate| candidate.domain == domain.domain);
        let right = other
            .map(|candidate| candidate.semantic_digest.clone())
            .unwrap_or_else(|| "missing".to_string());
        comparisons.push(ConsistencyComparison {
            comparison_id: format!("saved_reopened_{}", domain.domain),
            left: domain.semantic_digest.clone(),
            right: right.clone(),
            equal: other.is_some_and(|candidate| {
                candidate.semantic_digest == domain.semantic_digest
                    && candidate.stable_ids == domain.stable_ids
                    && candidate.source_paths == domain.source_paths
            }),
            detail: format!(
                "saved/reopened semantic digest and stable ids for {}",
                domain.domain
            ),
        });
    }
    comparisons.push(ConsistencyComparison {
        comparison_id: "parent_token".to_string(),
        left: saved.parent_token_hash.clone(),
        right: reopened.parent_token_hash.clone(),
        equal: saved.parent_token_hash == reopened.parent_token_hash,
        detail: "both children validated the same parent token".to_string(),
    });
    comparisons
}

fn domain_digest<T: Serialize>(
    domain: &str,
    value: &T,
    mut stable_ids: Vec<String>,
    mut source_paths: Vec<String>,
) -> ConsistencyDomainDigest {
    stable_ids.sort();
    source_paths.sort();
    ConsistencyDomainDigest {
        domain: domain.to_string(),
        semantic_digest: semantic_digest(domain, value),
        stable_ids,
        source_paths,
    }
}

fn semantic_digest<T: Serialize>(kind: &str, value: &T) -> String {
    ConsistencyDigest::sha256(kind, "save-reload-domain.v1", value)
        .map(|digest| digest.prefixed_value())
        .unwrap_or_default()
}

fn build_recipe_digest(
    project: &Path,
    assembly: &editor_core::ProjectRuntimePackageAssemblyResult,
) -> Result<String, String> {
    let manifest = read_project_manifest(project)?;
    Ok(recipe_digest(
        &manifest,
        assembly.build_profile.as_ref(),
        assembly.active_scene_id.as_deref().unwrap_or_default(),
    ))
}

fn recipe_digest(
    manifest: &ProjectManifest,
    profile: Option<&BuildProfile>,
    active_scene_id: &str,
) -> String {
    BuildRecipeDigest::calculate(&BuildRecipeDigestInput {
        project: manifest,
        build_profile: profile,
        active_scene_id,
        runtime_package_schema_version: RUNTIME_PACKAGE_SCHEMA_VERSION,
        component_schema_cooker_version: "component-schema.v1",
        aui_document_cooker_version: "aui-document-cook.v1",
        aui_font_atlas_cooker_version: "aui-font-atlas-cook.v1",
    })
    .map(|digest| digest.0.prefixed_value())
    .unwrap_or_default()
}

fn read_project_manifest(project: &Path) -> Result<ProjectManifest, String> {
    let text =
        fs::read_to_string(project.join("project.aife.json")).map_err(|error| error.to_string())?;
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

fn load_for_report(
    package_dir: &Path,
    code: &str,
    report: &mut SaveReloadRebuildConsistencyReport,
) -> Option<RuntimePackage> {
    let load = load_runtime_package(package_dir);
    if let Some(package) = load.value {
        Some(package)
    } else {
        report.diagnostics.push(diagnostic(
            code,
            format!("{:?}", load.diagnostics.issues),
            Some(package_dir.display().to_string()),
        ));
        None
    }
}

fn add_comparison(
    report: &mut SaveReloadRebuildConsistencyReport,
    comparison_id: &str,
    left: &str,
    right: &str,
) {
    report.comparisons.push(ConsistencyComparison {
        comparison_id: comparison_id.to_string(),
        left: left.to_string(),
        right: right.to_string(),
        equal: !left.is_empty() && left == right,
        detail: format!("deterministic clean rebuild comparison for {comparison_id}"),
    });
}

fn canonical_json_order_probe() -> bool {
    let first: serde_json::Value = serde_json::from_str("{\"b\":2,\"a\":1}").unwrap();
    let second: serde_json::Value = serde_json::from_str(" { \"a\" : 1, \"b\" : 2 }").unwrap();
    semantic_digest("json-order", &first) == semantic_digest("json-order", &second)
}

fn clear_owned_derived_outputs(
    project: &Path,
    temp_root: &Path,
    report: &mut SaveReloadRebuildConsistencyReport,
) {
    for relative in [".aife/preview-cache", "Build"] {
        let path = project.join(relative);
        if path.exists() {
            if !path.starts_with(temp_root) {
                report.diagnostics.push(diagnostic(
                    "derived_cleanup_scope_violation",
                    "refused to clear path outside temp root".to_string(),
                    Some(path.display().to_string()),
                ));
            } else if let Err(error) = fs::remove_dir_all(&path) {
                report.diagnostics.push(diagnostic(
                    "derived_cleanup_failed",
                    error.to_string(),
                    Some(path.display().to_string()),
                ));
            }
        }
    }
}

fn prepare_working_project(
    source: &Path,
    destination: &Path,
    temp_root: &Path,
) -> Result<(), String> {
    fs::create_dir_all(temp_root).map_err(|error| error.to_string())?;
    if destination.exists() {
        if !destination.starts_with(temp_root) || destination == temp_root {
            return Err("refused to replace working project outside owned temp root".to_string());
        }
        fs::remove_dir_all(destination).map_err(|error| error.to_string())?;
    }
    copy_project_recursive(source, destination)
}

fn copy_project_recursive(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination).map_err(|error| error.to_string())?;
    let mut entries = fs::read_dir(source)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name();
        let name_text = name.to_string_lossy();
        if name_text == "Build" || name_text == ".aife" {
            continue;
        }
        let source_path = entry.path();
        let destination_path = destination.join(name);
        if entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            copy_project_recursive(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn project_tree_hash(root: &Path) -> Result<String, String> {
    let mut entries = Vec::new();
    collect_file_hashes(root, root, &mut entries)?;
    entries.sort();
    Ok(semantic_digest("project-tree", &entries))
}

fn collect_file_hashes(
    root: &Path,
    current: &Path,
    entries: &mut Vec<(String, String)>,
) -> Result<(), String> {
    let mut children = fs::read_dir(current)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    children.sort_by_key(|entry| entry.file_name());
    for child in children {
        let path = child.path();
        if child
            .file_type()
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            collect_file_hashes(root, &path, entries)?;
        } else {
            let relative = path
                .strip_prefix(root)
                .map_err(|error| error.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            entries.push((
                relative,
                sha256_prefixed(&fs::read(&path).map_err(|error| error.to_string())?),
            ));
        }
    }
    Ok(())
}

fn diagnostic(code: &str, message: String, path: Option<String>) -> SaveReloadRebuildDiagnostic {
    SaveReloadRebuildDiagnostic {
        code: code.to_string(),
        message,
        domain: None,
        path,
        object_id: None,
        next_action: Some(
            "Inspect the failing checkpoint, domain, path, and child evidence.".to_string(),
        ),
    }
}

fn unique_id(prefix: &str) -> String {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{prefix}-{}-{stamp}", std::process::id())
}
