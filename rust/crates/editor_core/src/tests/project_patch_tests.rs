use super::fixtures::*;
use super::*;
use editor_ui_model::{AssetKind, InputActionValueKind};

fn create_entity_patch(name: &str) -> ProjectPatchDocument {
    ProjectPatchDocument::new(
        "patch-create-entity",
        "Create entity",
        PatchSource::Test,
        vec![PatchOperation::Scene(ScenePatchOperation::CreateEntity {
            operation_id: "op-create".to_string(),
            depends_on: Vec::new(),
            parent_id: None,
            name: name.to_string(),
        })],
    )
}

#[test]
fn project_patch_model_records_scene_capability() {
    let patch = create_entity_patch("Patch Entity");

    assert_eq!(patch.schema_version, PROJECT_PATCH_SCHEMA_VERSION);
    assert_eq!(patch.required_capabilities, vec![PatchCapability::Scene]);
    assert_eq!(patch.operations[0].kind(), "Scene.CreateEntity");
}

#[test]
fn project_patch_model_records_all_domain_capabilities() {
    let patch = ProjectPatchDocument::new(
        "patch-all-domain-model",
        "All domain model",
        PatchSource::Test,
        vec![
            PatchOperation::Asset(AssetPatchOperation::ValidateAssetBrowserIndex {
                operation_id: "op-asset".to_string(),
                depends_on: Vec::new(),
                query_kind: Some(AssetKind::Sprite),
            }),
            PatchOperation::Prefab(PrefabPatchOperation::ValidateReferences {
                operation_id: "op-prefab".to_string(),
                depends_on: vec!["op-asset".to_string()],
                path: Some("Prefabs/ship.prefab.json".to_string()),
            }),
            PatchOperation::Aui(AuiPatchOperation::CreateDocument {
                operation_id: "op-aui".to_string(),
                depends_on: Vec::new(),
                path: "UI/hud.aui.json".to_string(),
                document_id: "hud".to_string(),
                width: 1280.0,
                height: 720.0,
            }),
            PatchOperation::Rule(RulePatchOperation::ValidateAsset {
                operation_id: "op-rule".to_string(),
                depends_on: Vec::new(),
                path: "Rules/fire.rule.json".to_string(),
            }),
            PatchOperation::Build(BuildPatchOperation::ExportDesktopPackage {
                operation_id: "op-build".to_string(),
                depends_on: vec!["op-rule".to_string()],
                profile_id: Some("windows-dev".to_string()),
            }),
        ],
    );

    assert_eq!(
        patch.required_capabilities,
        vec![
            PatchCapability::Asset,
            PatchCapability::Prefab,
            PatchCapability::Aui,
            PatchCapability::Rule,
            PatchCapability::Build
        ]
    );
    assert_eq!(
        patch.operations[0].kind(),
        "Asset.ValidateAssetBrowserIndex"
    );
    assert_eq!(patch.operations[1].depends_on(), &["op-asset".to_string()]);
    assert!(
        PatchReviewModel::from_patch(&patch, PatchValidationReport::accepted(&patch))
            .write_set_preview
            .iter()
            .any(|entry| entry.contains("Build.ExportDesktopPackage"))
    );
}

#[test]
fn project_patch_applier_expands_scene_operation_to_ui_command() {
    let patch = create_entity_patch("Patch Entity");

    let commands = PatchApplier::expand(&patch);

    assert_eq!(commands.len(), 1);
    assert!(matches!(
        &commands[0],
        UiCommandPayload::CreateSceneEntity { name, .. } if name == "Patch Entity"
    ));
}

#[test]
fn project_patch_scene_component_authoring_applies_and_rolls_back_exactly() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_project_editor_scene_session(&scene_path);
    let patch = ProjectPatchDocument::new(
        "patch-scene-component-authoring",
        "Create entity with project component",
        PatchSource::Test,
        vec![
            PatchOperation::Scene(ScenePatchOperation::CreateEntity {
                operation_id: "create".to_string(),
                depends_on: Vec::new(),
                parent_id: None,
                name: "C01 Entity".to_string(),
            }),
            PatchOperation::Scene(ScenePatchOperation::AddComponent {
                operation_id: "add-component".to_string(),
                depends_on: vec!["create".to_string()],
                entity_id: "entity-c01-entity".to_string(),
                component_type: "project.c01State".to_string(),
                fields: serde_json::json!({"hp": 3, "state": "ready"}),
            }),
            PatchOperation::Scene(ScenePatchOperation::SetComponentField {
                operation_id: "set-field".to_string(),
                depends_on: vec!["add-component".to_string()],
                entity_id: "entity-c01-entity".to_string(),
                component_type: "project.c01State".to_string(),
                field_path: "state".to_string(),
                value: serde_json::json!("running"),
            }),
        ],
    );

    let validation = PatchValidator::validate(&session, &patch);
    assert!(validation.accepted, "{:?}", validation.diagnostics);
    let report = session.execute_patch_as_transaction(patch);
    assert_eq!(
        report.status,
        PatchApplyStatus::Committed,
        "{:?}",
        report.validation
    );
    let entity = session
        .editor_scene_document()
        .unwrap()
        .entity("entity-c01-entity")
        .unwrap();
    let component = entity
        .components
        .iter()
        .find(|component| component.component_type == "project.c01State")
        .unwrap();
    assert_eq!(component.fields["hp"], serde_json::json!(3));
    assert_eq!(component.fields["state"], serde_json::json!("running"));

    let rollback = session
        .rollback_last_project_patch("patch-scene-component-authoring")
        .unwrap();
    assert_eq!(rollback.status, PatchApplyStatus::Committed);
    assert!(session
        .editor_scene_document()
        .unwrap()
        .entity("entity-c01-entity")
        .is_none());
}

#[test]
fn project_patch_scene_component_authoring_rejects_duplicate_and_missing_components() {
    let scene_path = write_editor_scene_fixture();
    let session = opened_editor_scene_session(&scene_path);
    let patch = ProjectPatchDocument::new(
        "patch-scene-component-invalid",
        "Reject invalid component lifecycle",
        PatchSource::Test,
        vec![
            PatchOperation::Scene(ScenePatchOperation::AddComponent {
                operation_id: "add-first".to_string(),
                depends_on: Vec::new(),
                entity_id: "entity-player".to_string(),
                component_type: "project.state".to_string(),
                fields: serde_json::json!({}),
            }),
            PatchOperation::Scene(ScenePatchOperation::AddComponent {
                operation_id: "add-duplicate".to_string(),
                depends_on: vec!["add-first".to_string()],
                entity_id: "entity-player".to_string(),
                component_type: "project.state".to_string(),
                fields: serde_json::json!({}),
            }),
            PatchOperation::Scene(ScenePatchOperation::RemoveComponent {
                operation_id: "remove-missing".to_string(),
                depends_on: Vec::new(),
                entity_id: "entity-player".to_string(),
                component_type: "project.missing".to_string(),
            }),
        ],
    );

    let validation = PatchValidator::validate(&session, &patch);
    assert!(!validation.accepted);
    let codes = validation
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect::<Vec<_>>();
    assert!(codes.contains(&"project_patch.scene.component_duplicate"));
    assert!(codes.contains(&"project_patch.scene.component_missing"));
}

#[test]
fn project_patch_applier_expands_all_domain_operations_to_ui_commands() {
    let patch = ProjectPatchDocument::new(
        "patch-all-domain-applier",
        "All domain applier",
        PatchSource::Test,
        vec![
            PatchOperation::Asset(AssetPatchOperation::GenerateMockImageAsset {
                operation_id: "op-asset".to_string(),
                depends_on: Vec::new(),
                prompt: "ship sprite".to_string(),
                target_folder: "Assets/Generated".to_string(),
                asset_name: "ship".to_string(),
                image_kind: "sprite".to_string(),
                width: 32,
                height: 32,
                transparent_background: true,
            }),
            PatchOperation::Prefab(PrefabPatchOperation::InstantiateInScene {
                operation_id: "op-prefab".to_string(),
                depends_on: Vec::new(),
                prefab_id: "prefab.ship".to_string(),
                parent_entity_id: None,
                local_position: None,
            }),
            PatchOperation::Aui(AuiPatchOperation::ValidateDocument {
                operation_id: "op-aui".to_string(),
                depends_on: Vec::new(),
                path: "UI/hud.aui.json".to_string(),
            }),
            PatchOperation::Rule(RulePatchOperation::BuildArtifact {
                operation_id: "op-rule".to_string(),
                depends_on: Vec::new(),
                path: "Rules/fire.rule.json".to_string(),
            }),
            PatchOperation::Build(BuildPatchOperation::OpenBuildReport {
                operation_id: "op-build".to_string(),
                depends_on: Vec::new(),
            }),
        ],
    );

    let commands = PatchApplier::expand(&patch);

    assert!(matches!(
        &commands[0],
        UiCommandPayload::GenerateMockImageAsset {
            asset_name,
            image_kind,
            ..
        } if asset_name == "ship" && image_kind == "sprite"
    ));
    assert!(matches!(
        &commands[1],
        UiCommandPayload::InstantiatePrefabInScene { prefab_id, .. } if prefab_id == "prefab.ship"
    ));
    assert!(matches!(
        &commands[2],
        UiCommandPayload::ValidateAuiDocument { path } if path == "UI/hud.aui.json"
    ));
    assert!(matches!(
        &commands[3],
        UiCommandPayload::BuildRuleArtifact { path } if path == "Rules/fire.rule.json"
    ));
    assert!(matches!(&commands[4], UiCommandPayload::OpenBuildReport));
}

#[test]
fn project_patch_validator_rejects_missing_scene_entity() {
    let scene_path = write_editor_scene_fixture();
    let session = opened_editor_scene_session(&scene_path);
    let patch = ProjectPatchDocument::new(
        "patch-missing-entity",
        "Move missing entity",
        PatchSource::Test,
        vec![PatchOperation::Scene(ScenePatchOperation::SetTransform {
            operation_id: "op-move-missing".to_string(),
            depends_on: Vec::new(),
            entity_id: "missing".to_string(),
            local_position: Some(Vec3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            }),
            local_rotation: None,
            local_scale: None,
        })],
    );

    let report = PatchValidator::validate(&session, &patch);

    assert!(!report.accepted);
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "project_patch.scene.entity_missing"));
}

#[test]
fn project_patch_transaction_creates_scene_entity() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_project_editor_scene_session(&scene_path);

    let report = session.execute_patch_as_transaction(create_entity_patch("Patch Entity"));

    assert_eq!(report.status, PatchApplyStatus::Committed);
    assert_eq!(session.patch_history().entries.len(), 1);
    assert!(session
        .build_ui_model()
        .hierarchy
        .roots
        .iter()
        .any(|node| node.entity_id == "entity-patch-entity"));
}

#[test]
fn project_patch_transaction_rejects_missing_entity_without_mutation() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);
    let before_count = session.build_ui_model().hierarchy.roots.len();
    let patch = ProjectPatchDocument::new(
        "patch-missing-entity",
        "Move missing entity",
        PatchSource::Test,
        vec![PatchOperation::Scene(ScenePatchOperation::SetTransform {
            operation_id: "op-move-missing".to_string(),
            depends_on: Vec::new(),
            entity_id: "missing".to_string(),
            local_position: Some(Vec3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            }),
            local_rotation: None,
            local_scale: None,
        })],
    );

    let report = session.execute_patch_as_transaction(patch);

    assert_eq!(report.status, PatchApplyStatus::Rejected);
    assert_eq!(session.build_ui_model().hierarchy.roots.len(), before_count);
    assert!(session.patch_history().entries.is_empty());
}

#[test]
fn project_patch_input_action_and_binding_apply() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    let create = session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PatchInput".to_string(),
    }));
    assert_eq!(create.status, CommandStatus::Committed);
    let mapping_path = "Input/input.default.json".to_string();
    let create_mapping = session.execute_command(command_for_test(
        UiCommandPayload::CreateDefaultInputMapping {
            path: mapping_path.clone(),
        },
    ));
    assert_eq!(create_mapping.status, CommandStatus::Committed);

    let patch = ProjectPatchDocument::new(
        "patch-input-fire",
        "Add fire input",
        PatchSource::Test,
        vec![
            PatchOperation::Input(InputPatchOperation::AddInputAction {
                operation_id: "op-add-fire".to_string(),
                depends_on: Vec::new(),
                path: mapping_path.clone(),
                action_id: "action.patch_fire".to_string(),
                value_type: InputActionValueKind::Button,
            }),
            PatchOperation::Input(InputPatchOperation::AddInputBinding {
                operation_id: "op-bind-fire".to_string(),
                depends_on: vec!["op-add-fire".to_string()],
                path: mapping_path.clone(),
                context_id: "gameplay".to_string(),
                action_id: "action.patch_fire".to_string(),
                device_path: "keyboard/F".to_string(),
            }),
        ],
    );

    let report = session.execute_patch_as_transaction(patch);
    let mapping = InputMappingAuthoringService::load(&root, &mapping_path).unwrap();

    assert_eq!(report.status, PatchApplyStatus::Committed);
    assert!(mapping
        .actions
        .iter()
        .any(|action| action.id == "action.patch_fire"));
    assert!(mapping
        .bindings
        .iter()
        .any(|binding| binding.device_path == "keyboard/F"));
}

#[test]
fn project_patch_input_multiple_bindings_rollback_restores_existing_mapping_bytes() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PatchInputRollback".to_string(),
    }));
    let mapping_path = "Input/input.default.json".to_string();
    session.execute_command(command_for_test(
        UiCommandPayload::CreateDefaultInputMapping {
            path: mapping_path.clone(),
        },
    ));
    let before = fs::read(root.join(&mapping_path)).unwrap();
    let patch = ProjectPatchDocument::new(
        "patch-input-rollback",
        "Rollback multiple bindings",
        PatchSource::Test,
        vec![
            PatchOperation::Input(InputPatchOperation::AddInputAction {
                operation_id: "add-action".to_string(),
                depends_on: Vec::new(),
                path: mapping_path.clone(),
                action_id: "action.rollback".to_string(),
                value_type: InputActionValueKind::Button,
            }),
            PatchOperation::Input(InputPatchOperation::AddInputBinding {
                operation_id: "add-binding-a".to_string(),
                depends_on: vec!["add-action".to_string()],
                path: mapping_path.clone(),
                context_id: "gameplay".to_string(),
                action_id: "action.rollback".to_string(),
                device_path: "keyboard/J".to_string(),
            }),
            PatchOperation::Input(InputPatchOperation::AddInputBinding {
                operation_id: "add-binding-b".to_string(),
                depends_on: vec!["add-binding-a".to_string()],
                path: mapping_path.clone(),
                context_id: "gameplay".to_string(),
                action_id: "action.rollback".to_string(),
                device_path: "keyboard/K".to_string(),
            }),
        ],
    );

    let apply = session.execute_patch_as_transaction(patch);
    assert_eq!(apply.status, PatchApplyStatus::Committed, "{apply:#?}");
    let rollback = session
        .rollback_last_project_patch("patch-input-rollback")
        .unwrap();

    assert_eq!(
        rollback.status,
        PatchApplyStatus::Committed,
        "{rollback:#?}"
    );
    assert_eq!(fs::read(root.join(mapping_path)).unwrap(), before);
}

#[test]
fn project_patch_history_is_cleared_when_session_switches_project() {
    let first_root = unique_editor_project_temp_dir();
    let second_root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    assert_eq!(
        session
            .execute_command(command_for_test(UiCommandPayload::CreateProject {
                path: first_root.display().to_string(),
                name: "First Patch Project".to_string(),
            }))
            .status,
        CommandStatus::Committed
    );
    let mapping_path = "Input/input.default.json".to_string();
    session.execute_command(command_for_test(
        UiCommandPayload::CreateDefaultInputMapping {
            path: mapping_path.clone(),
        },
    ));
    let apply = session.execute_patch_as_transaction(ProjectPatchDocument::new(
        "patch-before-project-switch",
        "Patch before project switch",
        PatchSource::Test,
        vec![PatchOperation::Input(InputPatchOperation::AddInputAction {
            operation_id: "add-before-switch".to_string(),
            depends_on: Vec::new(),
            path: mapping_path,
            action_id: "action.before_switch".to_string(),
            value_type: InputActionValueKind::Button,
        })],
    ));
    assert_eq!(apply.status, PatchApplyStatus::Committed, "{apply:#?}");
    assert_eq!(session.patch_history().entries.len(), 1);

    assert_eq!(
        session
            .execute_command(command_for_test(UiCommandPayload::CreateProject {
                path: second_root.display().to_string(),
                name: "Second Patch Project".to_string(),
            }))
            .status,
        CommandStatus::Committed
    );

    assert!(session.patch_history().entries.is_empty());
    assert!(session.revert_last_patch_for_test().is_none());
}

#[test]
fn project_patch_validator_rejects_duplicate_input_action() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PatchInputDuplicate".to_string(),
    }));
    let mapping_path = "Input/input.default.json".to_string();
    session.execute_command(command_for_test(
        UiCommandPayload::CreateDefaultInputMapping {
            path: mapping_path.clone(),
        },
    ));
    let patch = ProjectPatchDocument::new(
        "patch-dup-input",
        "Add duplicate fire input",
        PatchSource::Test,
        vec![
            PatchOperation::Input(InputPatchOperation::AddInputAction {
                operation_id: "op-add-a".to_string(),
                depends_on: Vec::new(),
                path: mapping_path.clone(),
                action_id: "action.dup".to_string(),
                value_type: InputActionValueKind::Button,
            }),
            PatchOperation::Input(InputPatchOperation::AddInputAction {
                operation_id: "op-add-b".to_string(),
                depends_on: Vec::new(),
                path: mapping_path.clone(),
                action_id: "action.dup".to_string(),
                value_type: InputActionValueKind::Button,
            }),
        ],
    );

    let report = PatchValidator::validate(&session, &patch);

    assert!(!report.accepted);
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "project_patch.input.action_duplicate_in_patch"));
}

#[test]
fn project_patch_validator_accepts_all_domain_a_min_patch() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PatchAllDomainValidate".to_string(),
    }));
    let patch = ProjectPatchDocument::new(
        "patch-all-domain-validate",
        "Validate all domain patch",
        PatchSource::Test,
        vec![
            PatchOperation::Asset(AssetPatchOperation::ValidateAssetBrowserIndex {
                operation_id: "op-asset".to_string(),
                depends_on: Vec::new(),
                query_kind: Some(AssetKind::Sprite),
            }),
            PatchOperation::Prefab(PrefabPatchOperation::ValidateReferences {
                operation_id: "op-prefab".to_string(),
                depends_on: Vec::new(),
                path: Some("Prefabs/ship.prefab.json".to_string()),
            }),
            PatchOperation::Aui(AuiPatchOperation::CreateDocument {
                operation_id: "op-aui".to_string(),
                depends_on: Vec::new(),
                path: "UI/hud.aui.json".to_string(),
                document_id: "hud".to_string(),
                width: 1280.0,
                height: 720.0,
            }),
            PatchOperation::Rule(RulePatchOperation::ValidateAsset {
                operation_id: "op-rule".to_string(),
                depends_on: Vec::new(),
                path: "Rules/fire.rule.json".to_string(),
            }),
            PatchOperation::Build(BuildPatchOperation::ExportDesktopPackage {
                operation_id: "op-build".to_string(),
                depends_on: vec!["op-rule".to_string()],
                profile_id: Some("windows-dev".to_string()),
            }),
        ],
    );

    let report = PatchValidator::validate(&session, &patch);

    assert!(report.accepted, "{:?}", report.diagnostics);
}

#[test]
fn project_patch_validator_rejects_unsafe_domain_path_and_build_order() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PatchAllDomainReject".to_string(),
    }));
    let patch = ProjectPatchDocument::new(
        "patch-invalid-domain-path",
        "Invalid domain path",
        PatchSource::Test,
        vec![
            PatchOperation::Build(BuildPatchOperation::ExportDesktopPackage {
                operation_id: "op-build".to_string(),
                depends_on: Vec::new(),
                profile_id: Some("release-store".to_string()),
            }),
            PatchOperation::Aui(AuiPatchOperation::CreateDocument {
                operation_id: "op-aui".to_string(),
                depends_on: Vec::new(),
                path: "../hud.aui.json".to_string(),
                document_id: "hud".to_string(),
                width: 1280.0,
                height: 720.0,
            }),
        ],
    );

    let report = PatchValidator::validate(&session, &patch);

    assert!(!report.accepted);
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "project_patch.aui.path_invalid"));
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "project_patch.build.profile_unsupported"));
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "project_patch.build.order_invalid"));
}

#[test]
fn project_patch_history_revert_removes_created_entity() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_project_editor_scene_session(&scene_path);
    let apply = session.execute_patch_as_transaction(create_entity_patch("Patch Entity"));
    assert_eq!(apply.status, PatchApplyStatus::Committed);

    let revert = session
        .revert_last_patch_for_test()
        .expect("inverse patch should exist");

    assert_eq!(revert.status, PatchApplyStatus::Committed);
    assert!(!session
        .build_ui_model()
        .hierarchy
        .roots
        .iter()
        .any(|node| node.entity_id == "entity-patch-entity"));
}

#[test]
fn project_patch_rollback_restores_project_file_snapshot_on_failure() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PatchFileRollback".to_string(),
    }));
    let aui_path = "UI/rollback-hud.aui.json".to_string();
    let patch = ProjectPatchDocument::new(
        "patch-aui-rollback",
        "Rollback AUI file write",
        PatchSource::Test,
        vec![
            PatchOperation::Aui(AuiPatchOperation::CreateDocument {
                operation_id: "op-create-aui".to_string(),
                depends_on: Vec::new(),
                path: aui_path.clone(),
                document_id: "rollback-hud".to_string(),
                width: 1280.0,
                height: 720.0,
            }),
            PatchOperation::Aui(AuiPatchOperation::SetBindingPath {
                operation_id: "op-bind-missing-node".to_string(),
                depends_on: vec!["op-create-aui".to_string()],
                path: aui_path.clone(),
                node_id: "missing-node".to_string(),
                target_field: "text.text".to_string(),
                binding_id: "bind.score".to_string(),
                binding_path: "game.score".to_string(),
                fallback: Some(serde_json::json!("0")),
            }),
        ],
    );

    let report = session.execute_patch_as_transaction(patch);

    assert_eq!(report.status, PatchApplyStatus::Failed);
    assert!(!root.join(&aui_path).exists());
    assert!(session.patch_history().entries.is_empty());
}

#[test]
fn project_patch_asset_generates_mock_image_asset() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PatchAsset".to_string(),
    }));
    let patch = ProjectPatchDocument::new(
        "patch-asset-generate",
        "Generate asset",
        PatchSource::Test,
        vec![
            PatchOperation::Asset(AssetPatchOperation::GenerateMockImageAsset {
                operation_id: "op-generate".to_string(),
                depends_on: Vec::new(),
                prompt: "ship sprite".to_string(),
                target_folder: "Assets/Generated".to_string(),
                asset_name: "ship".to_string(),
                image_kind: "sprite".to_string(),
                width: 16,
                height: 16,
                transparent_background: true,
            }),
            PatchOperation::Asset(AssetPatchOperation::ValidateAssetBrowserIndex {
                operation_id: "op-validate".to_string(),
                depends_on: vec!["op-generate".to_string()],
                query_kind: Some(AssetKind::Texture),
            }),
        ],
    );

    let report = session.execute_patch_as_transaction(patch);

    assert_eq!(report.status, PatchApplyStatus::Committed);
    assert!(root
        .join("Assets")
        .join("Generated")
        .join("ship.png")
        .exists());
    assert!(root
        .join("Assets")
        .join("Generated")
        .join("ship.asset")
        .exists());
    assert!(root
        .join("Assets")
        .join("Generated")
        .join("ship.asset.meta.json")
        .exists());
    let database = ProjectAssetImport::load_database(&root)
        .unwrap()
        .expect("formal AssetDB");
    assert!(database.assets.iter().any(|asset| {
        asset.asset_id == "ship"
            && asset.asset_type == "texture"
            && asset.descriptor_path == "Assets/Generated/ship.asset"
    }));
}

#[test]
fn project_patch_prefab_creates_from_scene_entity() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PatchPrefab".to_string(),
    }));
    session.execute_command(command_for_test(
        UiCommandPayload::OpenProjectBrowserEntry {
            path: "Scenes/Main.scene.json".to_string(),
        },
    ));
    let patch = ProjectPatchDocument::new(
        "patch-prefab-create",
        "Create prefab",
        PatchSource::Test,
        vec![
            PatchOperation::Scene(ScenePatchOperation::CreateEntity {
                operation_id: "op-create-entity".to_string(),
                depends_on: Vec::new(),
                parent_id: None,
                name: "Patch Ship".to_string(),
            }),
            PatchOperation::Prefab(PrefabPatchOperation::CreateFromSceneSelection {
                operation_id: "op-create-prefab".to_string(),
                depends_on: vec!["op-create-entity".to_string()],
                scene_path: Some("Scenes/Main.scene.json".to_string()),
                root_entity_id: "entity-patch-ship".to_string(),
                prefab_id: "prefab-patch-ship".to_string(),
                name: "Patch Ship".to_string(),
                replace_selection_with_instance: false,
            }),
        ],
    );

    let report = session.execute_patch_as_transaction(patch);

    assert_eq!(report.status, PatchApplyStatus::Committed);
    assert!(root
        .join("Prefabs")
        .join("prefab-patch-ship.prefab.json")
        .exists());
}

#[test]
fn project_patch_aui_creates_document_and_node() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PatchAui".to_string(),
    }));
    let patch = ProjectPatchDocument::new(
        "patch-aui-create",
        "Create AUI",
        PatchSource::Test,
        vec![
            PatchOperation::Aui(AuiPatchOperation::CreateDocument {
                operation_id: "op-create-aui".to_string(),
                depends_on: Vec::new(),
                path: "UI/hud.aui.json".to_string(),
                document_id: "hud".to_string(),
                width: 1280.0,
                height: 720.0,
            }),
            PatchOperation::Aui(AuiPatchOperation::AddNode {
                operation_id: "op-add-node".to_string(),
                depends_on: vec!["op-create-aui".to_string()],
                path: "UI/hud.aui.json".to_string(),
                parent_node_id: "root".to_string(),
                node_id: "score_text".to_string(),
                node_kind: "text".to_string(),
                name: "Score Text".to_string(),
                rect: serde_json::json!({
                    "x": 16.0,
                    "y": 16.0,
                    "width": 220.0,
                    "height": 40.0
                }),
            }),
            PatchOperation::Aui(AuiPatchOperation::ValidateDocument {
                operation_id: "op-validate-aui".to_string(),
                depends_on: vec!["op-add-node".to_string()],
                path: "UI/hud.aui.json".to_string(),
            }),
        ],
    );

    let report = session.execute_patch_as_transaction(patch);

    assert_eq!(report.status, PatchApplyStatus::Committed);
    assert!(root.join("UI").join("hud.aui.json").exists());
}

#[test]
fn project_patch_rule_creates_and_validates_asset() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PatchRule".to_string(),
    }));
    let patch = ProjectPatchDocument::new(
        "patch-rule-create",
        "Create rule",
        PatchSource::Test,
        vec![
            PatchOperation::Rule(RulePatchOperation::CreateAsset {
                operation_id: "op-create-rule".to_string(),
                depends_on: Vec::new(),
                path: "Rules/fire.rule.json".to_string(),
                rule_id: "project.rule.fire".to_string(),
                display_name: "Fire".to_string(),
                phase: Some("PostPhysics".to_string()),
            }),
            PatchOperation::Rule(RulePatchOperation::ValidateAsset {
                operation_id: "op-validate-rule".to_string(),
                depends_on: vec!["op-create-rule".to_string()],
                path: "Rules/fire.rule.json".to_string(),
            }),
            PatchOperation::Rule(RulePatchOperation::BuildProjectManifest {
                operation_id: "op-build-rule-manifest".to_string(),
                depends_on: vec!["op-validate-rule".to_string()],
                path: "Rules/rule-manifest.json".to_string(),
            }),
        ],
    );

    let report = session.execute_patch_as_transaction(patch);

    assert_eq!(report.status, PatchApplyStatus::Committed);
    assert!(root.join("Rules").join("fire.rule.json").exists());
    let manifest: engine_runtime::runtime_package::RuntimeRuleManifest = serde_json::from_slice(
        &std::fs::read(root.join("Rules").join("rule-manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest.rules.len(), 1);
    assert_eq!(manifest.rules[0].rule_id, "project.rule.fire");
    assert_eq!(
        manifest.rules[0].phase,
        engine_runtime::runtime_package::RuntimeRulePhase::PostPhysics
    );
}

#[test]
fn project_patch_build_failure_keeps_confirmed_source_asset_edits() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PatchBuild".to_string(),
    }));
    let aui_path = "UI/build-keeps-source.aui.json";
    let patch = ProjectPatchDocument::new(
        "patch-build-keeps-source",
        "Build failure keeps source",
        PatchSource::Test,
        vec![
            PatchOperation::Aui(AuiPatchOperation::CreateDocument {
                operation_id: "op-create-aui".to_string(),
                depends_on: Vec::new(),
                path: aui_path.to_string(),
                document_id: "build-keeps-source".to_string(),
                width: 1280.0,
                height: 720.0,
            }),
            PatchOperation::Build(BuildPatchOperation::OpenBuildOutput {
                operation_id: "op-open-build-output".to_string(),
                depends_on: vec!["op-create-aui".to_string()],
            }),
        ],
    );

    let report = session.execute_patch_as_transaction(patch);

    assert_eq!(report.status, PatchApplyStatus::Failed);
    assert_eq!(
        report.operation_results[0].status,
        PatchOperationApplyStatus::Committed
    );
    assert_eq!(
        report.operation_results[1].status,
        PatchOperationApplyStatus::Rejected
    );
    assert!(root.join(aui_path).exists());
}

#[test]
fn project_patch_productization_report_marks_unsupported_capability_partial() {
    let mut patch = create_entity_patch("Patch Entity");
    patch.required_capabilities = vec![PatchCapability::Scene, PatchCapability::Aui];
    let validation = PatchValidationReport::accepted(&patch);
    let review = PatchReviewModel::from_patch(&patch, validation.clone());
    let report = ProjectPatchProductizationReport::from_parts(
        "project-patch-productization-test",
        &patch,
        validation,
        review,
        None,
        PatchHistorySummary {
            applied_count: 0,
            last_patch_id: None,
            last_status: None,
            reversible_count: 0,
            diagnostics: Vec::new(),
        },
        Vec::new(),
    );

    assert_eq!(report.status, ProjectPatchProductizationStatus::Partial);
    assert_eq!(
        report.supported_capabilities,
        vec![PatchCapability::Scene, PatchCapability::Aui]
    );
    assert!(report.unsupported_capabilities.is_empty());
    assert!(!report
        .next_actions
        .contains(&"aui_authoring_productization_or_patch_capability_v2".to_string()));
}

#[test]
fn project_patch_import_from_json_builds_review_model() {
    let scene_path = write_editor_scene_fixture();
    let session = opened_editor_scene_session(&scene_path);
    let patch = create_entity_patch("Imported Patch Entity");
    let raw_json = serde_json::to_string(&patch).unwrap();

    let result = ProjectPatchImportService::from_json_string(
        &session,
        ProjectPatchImportRequest::json_string("test-json", raw_json),
    );

    assert_eq!(result.parse_status, ProjectPatchImportParseStatus::Parsed);
    assert!(result.schema_diagnostics.is_empty());
    assert!(result.capability_diagnostics.is_empty());
    assert!(result
        .validation
        .as_ref()
        .is_some_and(|report| report.accepted));
    let review = result.review.expect("review should be built");
    assert_eq!(review.patch_id, "patch-create-entity");
    assert_eq!(review.operation_count, 1);
    assert_eq!(review.touched_domains, vec![PatchCapability::Scene]);
}

#[test]
fn project_patch_import_reports_invalid_json_without_mutation() {
    let scene_path = write_editor_scene_fixture();
    let session = opened_editor_scene_session(&scene_path);

    let result = ProjectPatchImportService::from_json_string(
        &session,
        ProjectPatchImportRequest::json_string("bad-json", "{not-json"),
    );

    assert_eq!(result.parse_status, ProjectPatchImportParseStatus::Rejected);
    assert!(result.parsed_patch.is_none());
    assert!(result.validation.is_none());
    assert!(result
        .schema_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "project_patch_import.parse_failed"));
    assert!(session.patch_history().entries.is_empty());
}

#[test]
fn project_patch_import_file_input_is_bounded_and_regular() {
    let scene_path = write_editor_scene_fixture();
    let session = opened_editor_scene_session(&scene_path);
    let directory = unique_editor_project_temp_dir();
    fs::create_dir_all(&directory).unwrap();
    let directory_result = ProjectPatchImportService::from_file(
        &session,
        ProjectPatchImportRequest::file_path("directory", directory.display().to_string()),
    );
    assert_eq!(
        directory_result.parse_status,
        ProjectPatchImportParseStatus::Rejected
    );
    assert!(directory_result
        .schema_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "project_patch_import.file_read_failed"));

    let oversized = directory.join("oversized.json");
    fs::File::create(&oversized)
        .unwrap()
        .set_len(8 * 1024 * 1024 + 1)
        .unwrap();
    let oversized_result = ProjectPatchImportService::from_file(
        &session,
        ProjectPatchImportRequest::file_path("oversized", oversized.display().to_string()),
    );
    assert_eq!(
        oversized_result.parse_status,
        ProjectPatchImportParseStatus::Rejected
    );
    assert!(oversized_result
        .schema_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("exceeds")));
}

#[test]
fn project_patch_import_reports_schema_mismatch() {
    let scene_path = write_editor_scene_fixture();
    let session = opened_editor_scene_session(&scene_path);
    let mut patch = create_entity_patch("Imported Patch Entity");
    patch.schema_version = "project-patch.v0".to_string();
    let raw_json = serde_json::to_string(&patch).unwrap();

    let result = ProjectPatchImportService::from_json_string(
        &session,
        ProjectPatchImportRequest::json_string("schema-mismatch", raw_json),
    );

    assert_eq!(result.parse_status, ProjectPatchImportParseStatus::Rejected);
    assert!(result.parsed_patch.is_some());
    assert!(result
        .schema_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "project_patch_import.patch_schema_unsupported"));
    assert!(result
        .validation
        .as_ref()
        .is_some_and(|report| !report.accepted));
}

#[test]
fn project_patch_import_accepts_all_domain_capability() {
    let scene_path = write_editor_scene_fixture();
    let session = opened_editor_scene_session(&scene_path);
    let mut patch = create_entity_patch("Imported Patch Entity");
    patch.required_capabilities = vec![PatchCapability::Scene, PatchCapability::Aui];
    let raw_json = serde_json::to_string(&patch).unwrap();

    let result = ProjectPatchImportService::from_json_string(
        &session,
        ProjectPatchImportRequest::json_string("unsupported-capability", raw_json),
    );

    assert_eq!(result.parse_status, ProjectPatchImportParseStatus::Parsed);
    assert!(result.capability_diagnostics.is_empty());
    assert!(result
        .validation
        .as_ref()
        .is_some_and(|report| report.accepted));
    assert!(!result
        .next_actions
        .contains(&"defer_unsupported_project_patch_capability".to_string()));
}

#[test]
fn project_patch_import_report_summarizes_apply_and_history() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_project_editor_scene_session(&scene_path);
    let patch = create_entity_patch("Imported Patch Entity");
    let raw_json = serde_json::to_string(&patch).unwrap();
    let import_result = ProjectPatchImportService::from_json_string(
        &session,
        ProjectPatchImportRequest::test_fixture("fixture", raw_json),
    );
    let imported_patch = import_result
        .parsed_patch
        .as_ref()
        .expect("parsed patch")
        .clone();
    let apply_report = session.execute_patch_as_transaction(imported_patch);

    let report = ProjectPatchImportProductizationReport::from_parts(
        "project-patch-import-test",
        import_result,
        Some(apply_report),
        summarize_patch_history(&session.patch_history().entries),
        Vec::new(),
    );

    assert_eq!(
        report.schema_version,
        PROJECT_PATCH_IMPORT_PRODUCTIZATION_REPORT_SCHEMA_VERSION
    );
    assert_eq!(report.status, ProjectPatchImportProductizationStatus::Pass);
    assert_eq!(report.parse_status, ProjectPatchImportParseStatus::Parsed);
    assert_eq!(report.history_summary.applied_count, 1);
    assert_eq!(report.supported_capabilities, vec![PatchCapability::Scene]);
    assert!(report.unsupported_capabilities.is_empty());
}
