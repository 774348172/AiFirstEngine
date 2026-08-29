use super::fixtures::*;
use super::*;
use editor_ui_model::InputActionValueKind;
use engine_runtime::canonical_digest::sha256_prefixed;
use std::io::Write;

fn candidate_project_session(name: &str) -> (EditorSession, PathBuf) {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    let result = session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: name.to_string(),
    }));
    assert_eq!(result.status, CommandStatus::Committed);
    (session, root)
}

fn input_candidate(
    session: &EditorSession,
    candidate_id: &str,
    source_kind: ProjectCandidateSourceKind,
    mapping_path: &str,
) -> ProjectCandidate {
    let patch = ProjectPatchDocument::new(
        format!("patch-{candidate_id}"),
        "Add candidate input",
        PatchSource::Test,
        vec![PatchOperation::Input(InputPatchOperation::AddInputAction {
            operation_id: format!("op-{candidate_id}"),
            depends_on: Vec::new(),
            path: mapping_path.to_string(),
            action_id: format!("action.{candidate_id}"),
            value_type: InputActionValueKind::Button,
        })],
    );
    let envelope = ProjectCandidateEntry::project_patch_envelope(
        session,
        candidate_id,
        source_kind,
        "candidate-entry-test",
        patch,
    )
    .unwrap();
    ProjectCandidateEntry::prepare(session, ProjectCandidatePrepareRequest { envelope }).unwrap()
}

fn approval_for(
    candidate: &ProjectCandidate,
    validation: &ProjectCandidateValidationReport,
) -> ProjectCandidateApproval {
    ProjectCandidateApproval {
        schema_version: PROJECT_CANDIDATE_APPROVAL_SCHEMA_VERSION.to_string(),
        candidate_id: candidate.envelope.candidate_id.clone(),
        candidate_digest: candidate.candidate_digest.clone(),
        validation_digest: validation.validation_digest.clone(),
        approved_by: "local-maintainer".to_string(),
        allow_replace: false,
    }
}

#[test]
fn project_candidate_entry_policy_rejects_unknown_fields_and_oversize_input() {
    let (session, _) = candidate_project_session("CandidatePolicy");
    let binding = ProjectCandidateEntry::inspect_project_binding(&session).unwrap();
    let context = ProjectPatchLlmContextSnapshot::capture(&session);
    let raw = format!(
        r#"{{"schemaVersion":"project-candidate-envelope.v1","candidateId":"policy","sourceKind":"imported_codex","sourceLabel":"test","targetProjectId":"{}","expectedBaseProjectDigest":"{}","projectPatchContextHash":"{}","payload":{{"payloadKind":"project_patch","payload":{{"schemaVersion":"project-patch.v2","patchId":"policy","title":"policy","source":"Test","intentSummary":"","targetProjectRoot":null,"requiredCapabilities":[],"operations":[],"expectedOutcome":"","riskLevel":"Low","createdAt":"0"}}}},"unknown":true}}"#,
        binding.project_id, binding.project_digest, context.context_hash
    );
    let error = ProjectCandidateEntry::from_json_string(&session, &raw).unwrap_err();
    assert_eq!(error.code, "project_candidate_entry.envelope_parse_failed");

    let oversize = " ".repeat(8 * 1024 * 1024 + 1);
    let error = ProjectCandidateEntry::from_json_string(&session, &oversize).unwrap_err();
    assert_eq!(error.code, "project_candidate_entry.input_too_large");
}

#[test]
fn project_candidate_entry_binding_rejects_project_and_base_drift() {
    let (session, root) = candidate_project_session("CandidateBinding");
    let binding = ProjectCandidateEntry::inspect_project_binding(&session).unwrap();
    assert_eq!(
        binding.project_id,
        session
            .active_project_session()
            .unwrap()
            .manifest
            .project_id
    );

    let patch =
        ProjectPatchDocument::new("binding-patch", "Binding", PatchSource::Test, Vec::new());
    let mut envelope = ProjectCandidateEntry::project_patch_envelope(
        &session,
        "binding",
        ProjectCandidateSourceKind::TestFixture,
        "binding-test",
        patch,
    )
    .unwrap();
    envelope.target_project_id = "another-project".to_string();
    let error = ProjectCandidateEntry::prepare(
        &session,
        ProjectCandidatePrepareRequest {
            envelope: envelope.clone(),
        },
    )
    .unwrap_err();
    assert_eq!(error.code, "project_candidate_entry.project_id_mismatch");

    envelope.target_project_id = binding.project_id;
    fs::write(root.join("binding-drift.txt"), "drift").unwrap();
    let error =
        ProjectCandidateEntry::prepare(&session, ProjectCandidatePrepareRequest { envelope })
            .unwrap_err();
    assert_eq!(error.code, "project_candidate_entry.base_project_drifted");
}

#[test]
fn project_candidate_entry_project_patch_uses_common_candidate_schema() {
    assert_project_patch_provider_independence();
}

#[test]
fn project_candidate_entry_provider_independence_uses_one_candidate_schema() {
    assert_project_patch_provider_independence();
}

fn assert_project_patch_provider_independence() {
    let (mut session, _) = candidate_project_session("CandidateProviderIndependence");
    let mapping_path = "Input/input.default.json";
    assert_eq!(
        session
            .execute_command(command_for_test(
                UiCommandPayload::CreateDefaultInputMapping {
                    path: mapping_path.to_string(),
                }
            ))
            .status,
        CommandStatus::Committed
    );
    let provider = input_candidate(
        &session,
        "provider",
        ProjectCandidateSourceKind::BuiltInProvider,
        mapping_path,
    );
    let imported = input_candidate(
        &session,
        "imported",
        ProjectCandidateSourceKind::ImportedCodex,
        mapping_path,
    );
    assert_eq!(provider.schema_version, imported.schema_version);
    assert!(matches!(
        provider.prepared_payload,
        PreparedProjectCandidatePayload::ProjectPatch { .. }
    ));
    assert!(matches!(
        imported.prepared_payload,
        PreparedProjectCandidatePayload::ProjectPatch { .. }
    ));
    let provider_validation = ProjectCandidateEntry::validate(
        &session,
        &provider,
        &ProjectCandidateValidationContext::default(),
    )
    .unwrap();
    let imported_validation = ProjectCandidateEntry::validate(
        &session,
        &imported,
        &ProjectCandidateValidationContext::default(),
    )
    .unwrap();
    assert_eq!(
        provider_validation.schema_version,
        imported_validation.schema_version
    );
    assert_eq!(
        provider_validation.status,
        ProjectCandidateValidationStatus::Passed
    );
    assert_eq!(
        imported_validation.status,
        ProjectCandidateValidationStatus::Passed
    );
}

#[test]
fn project_candidate_entry_apply_and_rollback_are_explicit_and_exact_last() {
    let (mut session, root) = candidate_project_session("CandidateApplyRollback");
    let mapping_path = "Input/input.default.json";
    session.execute_command(command_for_test(
        UiCommandPayload::CreateDefaultInputMapping {
            path: mapping_path.to_string(),
        },
    ));
    let before = fs::read(root.join(mapping_path)).unwrap();
    let candidate = input_candidate(
        &session,
        "apply-rollback",
        ProjectCandidateSourceKind::ImportedCodex,
        mapping_path,
    );
    let validation = ProjectCandidateEntry::validate(
        &session,
        &candidate,
        &ProjectCandidateValidationContext::default(),
    )
    .unwrap();
    let approval = approval_for(&candidate, &validation);
    let receipt = ProjectCandidateEntry::apply(
        &mut session,
        candidate.clone(),
        validation.clone(),
        approval,
    )
    .unwrap();
    assert_ne!(
        receipt.before_project_digest,
        receipt.applied_project_digest
    );
    assert_ne!(fs::read(root.join(mapping_path)).unwrap(), before);

    let rollback = ProjectCandidateEntry::rollback(&mut session, &receipt).unwrap();
    assert_eq!(
        rollback.restored_project_digest,
        receipt.before_project_digest
    );
    assert_eq!(fs::read(root.join(mapping_path)).unwrap(), before);
    assert!(session.patch_history().entries.is_empty());
}

#[test]
fn project_candidate_entry_tamper_and_non_last_rollback_fail_closed() {
    let (mut session, _) = candidate_project_session("CandidateTamper");
    let mapping_path = "Input/input.default.json";
    session.execute_command(command_for_test(
        UiCommandPayload::CreateDefaultInputMapping {
            path: mapping_path.to_string(),
        },
    ));
    let candidate = input_candidate(
        &session,
        "tamper",
        ProjectCandidateSourceKind::ImportedCodex,
        mapping_path,
    );
    let validation = ProjectCandidateEntry::validate(
        &session,
        &candidate,
        &ProjectCandidateValidationContext::default(),
    )
    .unwrap();
    let mut tampered = candidate.clone();
    tampered.envelope.source_label.push_str("-tampered");
    let error = ProjectCandidateEntry::apply(
        &mut session,
        tampered,
        validation.clone(),
        approval_for(&candidate, &validation),
    )
    .unwrap_err();
    assert_eq!(error.code, "project_candidate_entry.candidate_tampered");

    let mut relowered_value = serde_json::to_value(&candidate).unwrap();
    relowered_value["preparedPayload"]["payload"]["patch"]["title"] =
        serde_json::Value::String("Tampered lowered patch".to_string());
    relowered_value["candidateDigest"] = serde_json::Value::String(String::new());
    let digest = sha256_prefixed(
        &engine_runtime::canonical_digest::canonical_json_bytes(&relowered_value).unwrap(),
    );
    relowered_value["candidateDigest"] = serde_json::Value::String(digest);
    let relowered: ProjectCandidate = serde_json::from_value(relowered_value).unwrap();
    let error = ProjectCandidateEntry::apply(
        &mut session,
        relowered,
        validation.clone(),
        approval_for(&candidate, &validation),
    )
    .unwrap_err();
    assert_eq!(
        error.code,
        "project_candidate_entry.lowering_binding_mismatch"
    );

    let receipt = ProjectCandidateEntry::apply(
        &mut session,
        candidate.clone(),
        validation.clone(),
        approval_for(&candidate, &validation),
    )
    .unwrap();
    let second_patch = ProjectPatchDocument::new(
        "intervening-patch",
        "Intervening patch",
        PatchSource::Test,
        vec![PatchOperation::Input(InputPatchOperation::AddInputAction {
            operation_id: "op-intervening".to_string(),
            depends_on: Vec::new(),
            path: mapping_path.to_string(),
            action_id: "action.intervening".to_string(),
            value_type: InputActionValueKind::Button,
        })],
    );
    assert_eq!(
        session.execute_patch_as_transaction(second_patch).status,
        PatchApplyStatus::Committed
    );
    let error = ProjectCandidateEntry::rollback(&mut session, &receipt).unwrap_err();
    assert_eq!(
        error.code,
        "project_candidate_entry.rollback_project_drifted"
    );
}

#[test]
fn project_candidate_entry_tamper_rejects_imported_file_source_drift() {
    let (mut session, _) = candidate_project_session("CandidateFileDrift");
    let mapping_path = "Input/input.default.json";
    session.execute_command(command_for_test(
        UiCommandPayload::CreateDefaultInputMapping {
            path: mapping_path.to_string(),
        },
    ));
    let patch = ProjectPatchDocument::new(
        "file-drift-patch",
        "File drift",
        PatchSource::ImportedPatch,
        vec![PatchOperation::Input(InputPatchOperation::AddInputAction {
            operation_id: "op-file-drift".to_string(),
            depends_on: Vec::new(),
            path: mapping_path.to_string(),
            action_id: "action.file_drift".to_string(),
            value_type: InputActionValueKind::Button,
        })],
    );
    let envelope = ProjectCandidateEntry::project_patch_envelope(
        &session,
        "file-drift",
        ProjectCandidateSourceKind::ImportedFile,
        "file-drift-test",
        patch,
    )
    .unwrap();
    let source_dir = unique_editor_project_temp_dir();
    fs::create_dir_all(&source_dir).unwrap();
    let source_path = source_dir.join("candidate.json");
    fs::write(&source_path, serde_json::to_vec(&envelope).unwrap()).unwrap();
    let candidate = ProjectCandidateEntry::from_file(&session, &source_path).unwrap();
    fs::OpenOptions::new()
        .append(true)
        .open(&source_path)
        .unwrap()
        .write_all(b"\n")
        .unwrap();
    let error = ProjectCandidateEntry::validate(
        &session,
        &candidate,
        &ProjectCandidateValidationContext::default(),
    )
    .unwrap_err();
    assert_eq!(error.code, "project_candidate_entry.source_drifted");
}

#[test]
fn project_candidate_entry_rollback_rejects_non_last_project_patch_without_file_drift() {
    let (mut session, root) = candidate_project_session("CandidateExactLast");
    let scene_path = root.join("Scenes/Main.scene.json");
    assert_eq!(
        session.open_scene_document_for_test(&scene_path).status,
        CommandStatus::Committed
    );
    let patch = ProjectPatchDocument::new(
        "exact-last-candidate-patch",
        "Exact last candidate",
        PatchSource::ImportedPatch,
        vec![PatchOperation::Scene(ScenePatchOperation::CreateEntity {
            operation_id: "op-exact-last-candidate".to_string(),
            depends_on: Vec::new(),
            parent_id: None,
            name: "Candidate Entity".to_string(),
        })],
    );
    let envelope = ProjectCandidateEntry::project_patch_envelope(
        &session,
        "exact-last-candidate",
        ProjectCandidateSourceKind::ImportedCodex,
        "exact-last-test",
        patch,
    )
    .unwrap();
    let candidate =
        ProjectCandidateEntry::prepare(&session, ProjectCandidatePrepareRequest { envelope })
            .unwrap();
    let validation = ProjectCandidateEntry::validate(
        &session,
        &candidate,
        &ProjectCandidateValidationContext::default(),
    )
    .unwrap();
    let approval = approval_for(&candidate, &validation);
    let receipt =
        ProjectCandidateEntry::apply(&mut session, candidate, validation, approval).unwrap();
    assert_ne!(
        receipt.before_project_digest, receipt.applied_project_digest,
        "Scene ProjectPatch must persist a new project digest"
    );

    let intervening = ProjectPatchDocument::new(
        "intervening-scene-patch",
        "Intervening non-file patch",
        PatchSource::Test,
        vec![PatchOperation::Asset(
            AssetPatchOperation::ValidateAssetBrowserIndex {
                operation_id: "op-intervening-asset-validation".to_string(),
                depends_on: Vec::new(),
                query_kind: None,
            },
        )],
    );
    assert_eq!(
        session.execute_patch_as_transaction(intervening).status,
        PatchApplyStatus::Committed
    );
    let error = ProjectCandidateEntry::rollback(&mut session, &receipt).unwrap_err();
    assert_eq!(error.code, "project_candidate_entry.project_patch_not_last");
}

#[test]
fn project_candidate_entry_source_patch_lowers_and_validates_through_controlled_pipeline() {
    let (mut session, root) = candidate_project_session("CandidateSourcePatch");
    let store = unique_editor_project_temp_dir();
    fs::create_dir_all(&store).unwrap();
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("project.aife.json")).unwrap()).unwrap();
    manifest["runtimeModule"] = serde_json::json!({
        "sourceKind": "projectRust",
        "moduleId": "candidate.source.runtime",
        "interfaceVersion": PROJECT_RUNTIME_MODULE_INTERFACE_VERSION,
        "cargoManifest": "RuntimeModule/Cargo.toml",
        "cargoPackage": "candidate_source_runtime",
        "playerBinary": "candidate_source_player"
    });
    let source_patch = ControlledSourcePatchDocument {
        schema_version: CONTROLLED_SOURCE_PATCH_SCHEMA_VERSION.to_string(),
        patch_id: "candidate_source_patch".to_string(),
        operations: vec![
            ControlledSourcePatchOperation::CreateOrReplace {
                path: "RuntimeModule/Cargo.toml".to_string(),
                text: "[package]\nname = \"candidate_source_runtime\"\nversion = \"0.0.3\"\nedition = \"2021\"\n\n[dependencies]\nengine_runtime = \"=0.0.3\"\n"
                    .to_string(),
            },
            ControlledSourcePatchOperation::CreateOrReplace {
                path: "RuntimeModule/src/lib.rs".to_string(),
                text: "pub fn candidate_value() -> u32 {\n    7\n}\n".to_string(),
            },
            ControlledSourcePatchOperation::CreateOrReplace {
                path: "project.aife.json".to_string(),
                text: serde_json::to_string_pretty(&manifest).unwrap(),
            },
        ],
    };
    let binding = ProjectCandidateEntry::inspect_project_binding(&session).unwrap();
    let envelope = ProjectCandidateEnvelope {
        schema_version: PROJECT_CANDIDATE_ENVELOPE_SCHEMA_VERSION.to_string(),
        candidate_id: "source-entry".to_string(),
        source_kind: ProjectCandidateSourceKind::ImportedCodex,
        source_label: "source-entry-test".to_string(),
        target_project_id: binding.project_id,
        expected_base_project_digest: binding.project_digest,
        project_patch_context_hash: None,
        payload: ProjectCandidatePayload::ControlledSourcePatch {
            request: ControlledSourcePatchPrepareRequest {
                revision_id: "source_entry_revision".to_string(),
                project_root: root,
                candidate_store_root: store,
                source_patch,
            },
        },
    };
    let candidate =
        ProjectCandidateEntry::prepare(&session, ProjectCandidatePrepareRequest { envelope })
            .unwrap();
    assert!(matches!(
        candidate.prepared_payload,
        PreparedProjectCandidatePayload::ControlledSourcePatch { .. }
    ));
    let sdk_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_path_buf();
    let validation = ProjectCandidateEntry::validate(
        &session,
        &candidate,
        &ProjectCandidateValidationContext {
            controlled_source_patch: Some(
                ControlledSourcePatchValidationRequest::compile_tests_only(sdk_root),
            ),
            cancellation: None,
        },
    )
    .unwrap();
    assert_eq!(
        validation.status,
        ProjectCandidateValidationStatus::Passed,
        "{validation:#?}"
    );
    assert!(matches!(
        validation.payload_validation,
        ProjectCandidateValidationPayload::ControlledSourcePatch { .. }
    ));
    let approval = approval_for(&candidate, &validation);
    let receipt =
        ProjectCandidateEntry::apply(&mut session, candidate, validation, approval).unwrap();
    assert_ne!(
        receipt.before_project_digest,
        receipt.applied_project_digest
    );
    let rollback = ProjectCandidateEntry::rollback(&mut session, &receipt).unwrap();
    assert_eq!(
        rollback.restored_project_digest,
        receipt.before_project_digest
    );
}

#[test]
fn project_candidate_entry_asset_import_binds_expected_source_and_validates() {
    let (mut session, root) = candidate_project_session("CandidateAssetImport");
    let external = unique_editor_project_temp_dir();
    let store = unique_editor_project_temp_dir();
    fs::create_dir_all(&external).unwrap();
    fs::create_dir_all(&store).unwrap();
    let source = external.join("candidate.png");
    let mut png_bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_bytes, 1, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[0, 255, 0, 255]).unwrap();
    }
    fs::write(&source, &png_bytes).unwrap();
    let source_hash = sha256_prefixed(&png_bytes);
    let binding = ProjectCandidateEntry::inspect_project_binding(&session).unwrap();
    let envelope = ProjectCandidateEnvelope {
        schema_version: PROJECT_CANDIDATE_ENVELOPE_SCHEMA_VERSION.to_string(),
        candidate_id: "asset-entry".to_string(),
        source_kind: ProjectCandidateSourceKind::ImportedFile,
        source_label: "asset-entry-test".to_string(),
        target_project_id: binding.project_id,
        expected_base_project_digest: binding.project_digest,
        project_patch_context_hash: None,
        payload: ProjectCandidatePayload::AssetImport {
            request: ProjectAssetImportPrepareRequest {
                import_id: "asset_entry_import".to_string(),
                revision_id: "asset_entry_revision".to_string(),
                project_root: root,
                candidate_store_root: store,
                source_path: source,
                target_directory: "Assets/Imported".to_string(),
                asset_id: "texture_candidate_entry".to_string(),
                display_name: "Candidate Entry".to_string(),
                conflict_policy: AssetImportConflictPolicy::RejectExisting,
                source_metadata: AssetImportSourceMetadata::local_file(),
                license: AssetLicenseMetadata::project_owned(),
                texture_settings: TextureImportSettings::default(),
            },
            expected_source_hash: source_hash,
        },
    };
    let candidate =
        ProjectCandidateEntry::prepare(&session, ProjectCandidatePrepareRequest { envelope })
            .unwrap();
    let validation = ProjectCandidateEntry::validate(
        &session,
        &candidate,
        &ProjectCandidateValidationContext::default(),
    )
    .unwrap();
    assert_eq!(validation.status, ProjectCandidateValidationStatus::Passed);
    assert!(matches!(
        validation.payload_validation,
        ProjectCandidateValidationPayload::AssetImport { .. }
    ));
    let approval = approval_for(&candidate, &validation);
    let receipt =
        ProjectCandidateEntry::apply(&mut session, candidate, validation, approval).unwrap();
    assert_ne!(
        receipt.before_project_digest,
        receipt.applied_project_digest
    );
    let rollback = ProjectCandidateEntry::rollback(&mut session, &receipt).unwrap();
    assert_eq!(
        rollback.restored_project_digest,
        receipt.before_project_digest
    );
}
