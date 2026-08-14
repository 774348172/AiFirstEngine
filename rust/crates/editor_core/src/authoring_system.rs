use crate::{
    command_for_test, create_default_editable_project_fixture, CommandStatus,
    EditableProjectLoopDiagnosticSeverity, EditorSceneComponent, EditorSession, EditorTransform,
};
use editor_ui_model::{AssetPlacementMode, UiCommandPayload, Vec3};
use engine_runtime::input_mapping::InputMappingAsset;
use serde::{Deserialize, Serialize};

pub const EDITOR_AUTHORING_REPORT_SCHEMA_VERSION: &str = "editor-authoring-report.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorAuthoringDiagnostic {
    pub severity: EditableProjectLoopDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub source_stage: String,
}

impl EditorAuthoringDiagnostic {
    pub fn error(
        code: impl Into<String>,
        source_stage: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: EditableProjectLoopDiagnosticSeverity::Error,
            code: code.into(),
            message: message.into(),
            source_stage: source_stage.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorAuthoringReport {
    pub schema_version: String,
    pub project_id: String,
    pub opened_scene: bool,
    pub scene_id: Option<String>,
    pub created_entity_id: Option<String>,
    pub placed_asset_entity_id: Option<String>,
    pub prefab_entity_id: Option<String>,
    pub collider_entity_id: Option<String>,
    pub inspector_edit_applied: bool,
    pub input_mapping_validated: bool,
    pub dirty_before_save: Option<bool>,
    pub dirty_after_save: Option<bool>,
    pub console_entry_count: usize,
    pub diagnostics: Vec<EditorAuthoringDiagnostic>,
}

impl EditorAuthoringReport {
    pub fn new(project_id: impl Into<String>) -> Self {
        Self {
            schema_version: EDITOR_AUTHORING_REPORT_SCHEMA_VERSION.to_string(),
            project_id: project_id.into(),
            opened_scene: false,
            scene_id: None,
            created_entity_id: None,
            placed_asset_entity_id: None,
            prefab_entity_id: None,
            collider_entity_id: None,
            inspector_edit_applied: false,
            input_mapping_validated: false,
            dirty_before_save: None,
            dirty_after_save: None,
            console_entry_count: 0,
            diagnostics: Vec::new(),
        }
    }
}

pub fn run_editor_authoring_system_headless() -> EditorAuthoringReport {
    let fixture = create_default_editable_project_fixture();
    let mut session = EditorSession::new();
    let mut report = EditorAuthoringReport::new(fixture.project_id.clone());

    let open = session.execute_command(command_for_test(UiCommandPayload::OpenSceneDocument {
        path: fixture.scene_path.display().to_string(),
    }));
    report.opened_scene = open.status == CommandStatus::Committed;
    if !report.opened_scene {
        report.diagnostics.push(EditorAuthoringDiagnostic::error(
            "editor_authoring.open_scene_failed",
            "open_scene",
            "Editable Scene did not open.",
        ));
        return finalize_report(report, &session);
    }
    report.scene_id = session.build_ui_model().hierarchy.scene_id.clone();

    let create = session.execute_command(command_for_test(UiCommandPayload::CreateSceneEntity {
        parent_id: None,
        name: "Authoring Entity".to_string(),
    }));
    if create.status == CommandStatus::Committed {
        report.created_entity_id = session
            .build_ui_model()
            .hierarchy
            .selected_entity_id
            .clone();
    } else {
        report.diagnostics.push(EditorAuthoringDiagnostic::error(
            "editor_authoring.create_entity_failed",
            "create_entity",
            "Generic authoring Entity was not created.",
        ));
    }

    let place = session.execute_command(command_for_test(UiCommandPayload::PlaceAssetIntoScene {
        asset_id: "model-authoring".to_string(),
        asset_type: "model".to_string(),
        asset_guid: Some("guid-model-authoring".to_string()),
        target_parent_id: None,
        local_position: Some(Vec3 {
            x: 2.0,
            y: 0.0,
            z: 0.0,
        }),
        placement_mode: AssetPlacementMode::WorldOrigin,
    }));
    if place.status == CommandStatus::Committed {
        report.placed_asset_entity_id = session
            .build_ui_model()
            .hierarchy
            .selected_entity_id
            .clone();
    } else {
        report.diagnostics.push(EditorAuthoringDiagnostic::error(
            "editor_authoring.place_asset_failed",
            "place_asset",
            "Asset placement did not commit.",
        ));
    }

    let prefab = session.execute_scene_edit_for_test(crate::SceneEditCommand::CreateEntity {
        parent_id: None,
        name: "Authoring Prefab Instance".to_string(),
        mesh: None,
        components: vec![EditorSceneComponent {
            component_type: "engine.prefab_instance".to_string(),
            fields: serde_json::json!({
                "source": {
                    "id": "prefab.authoring",
                    "type": "prefab"
                }
            }),
        }],
        local_transform: EditorTransform::identity(),
        sibling_order: None,
    });
    if prefab.status == CommandStatus::Committed {
        report.prefab_entity_id = session
            .build_ui_model()
            .hierarchy
            .selected_entity_id
            .clone();
    } else {
        report.diagnostics.push(EditorAuthoringDiagnostic::error(
            "editor_authoring.prefab_instance_failed",
            "prefab_authoring",
            "Prefab instance authoring Entity was not created.",
        ));
    }

    let collider = session.execute_scene_edit_for_test(crate::SceneEditCommand::CreateEntity {
        parent_id: None,
        name: "Authoring Collider2D".to_string(),
        mesh: None,
        components: vec![EditorSceneComponent {
            component_type: "engine.collider2d".to_string(),
            fields: serde_json::json!({
                "shape": "aabb",
                "halfExtents": { "x": 0.5, "y": 0.5 },
                "isSensor": true
            }),
        }],
        local_transform: EditorTransform::identity(),
        sibling_order: None,
    });
    if collider.status == CommandStatus::Committed {
        report.collider_entity_id = session
            .build_ui_model()
            .hierarchy
            .selected_entity_id
            .clone();
    } else {
        report.diagnostics.push(EditorAuthoringDiagnostic::error(
            "editor_authoring.collider2d_failed",
            "collider2d_authoring",
            "Collider2D authoring Entity was not created.",
        ));
    }

    if let Some(collider_entity_id) = report.collider_entity_id.clone() {
        let edit =
            session.execute_command(command_for_test(UiCommandPayload::SetSceneComponentField {
                entity_id: collider_entity_id,
                component_type: "engine.collider2d".to_string(),
                field_path: "isSensor".to_string(),
                value: serde_json::json!(false),
            }));
        report.inspector_edit_applied = edit.status == CommandStatus::Committed;
        if !report.inspector_edit_applied {
            report.diagnostics.push(EditorAuthoringDiagnostic::error(
                "editor_authoring.inspector_edit_failed",
                "schema_inspector",
                "Inspector component field edit did not commit.",
            ));
        }
    }

    let input_report = InputMappingAsset::explicit_empty("input.authoring-check").validate();
    report.input_mapping_validated = !input_report.has_errors();
    if !report.input_mapping_validated {
        report.diagnostics.push(EditorAuthoringDiagnostic::error(
            "editor_authoring.input_mapping_invalid",
            "input_mapping_authoring",
            "Explicit project InputMappingAsset did not validate.",
        ));
    }

    report.dirty_before_save = session.scene_dirty();
    let save = session.execute_command(command_for_test(UiCommandPayload::SaveSceneDocument {
        path: None,
    }));
    if save.status != CommandStatus::Committed {
        report.diagnostics.push(EditorAuthoringDiagnostic::error(
            "editor_authoring.save_failed",
            "save_scene",
            "Authoring Scene did not save.",
        ));
    }
    report.dirty_after_save = session.scene_dirty();

    finalize_report(report, &session)
}

fn finalize_report(
    mut report: EditorAuthoringReport,
    session: &EditorSession,
) -> EditorAuthoringReport {
    report.console_entry_count = session.build_ui_model().console.entries.len();
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_authoring_report_serializes_for_ai() {
        let mut report = EditorAuthoringReport::new("project-main");
        report.opened_scene = true;
        report.scene_id = Some("scene-main".to_string());

        let json = serde_json::to_string_pretty(&report).expect("report should serialize");

        assert!(json.contains(EDITOR_AUTHORING_REPORT_SCHEMA_VERSION));
        assert!(json.contains("openedScene"));
        assert!(json.contains("inputMappingValidated"));
    }

    #[test]
    fn editor_authoring_system_headless_runs_c_min_loop() {
        let report = run_editor_authoring_system_headless();

        assert!(report.opened_scene);
        assert_eq!(report.scene_id.as_deref(), Some("scene-main"));
        assert_eq!(
            report.created_entity_id.as_deref(),
            Some("entity-authoring-entity")
        );
        assert_eq!(
            report.placed_asset_entity_id.as_deref(),
            Some("entity-model-authoring")
        );
        assert_eq!(
            report.prefab_entity_id.as_deref(),
            Some("entity-authoring-prefab-instance")
        );
        assert_eq!(
            report.collider_entity_id.as_deref(),
            Some("entity-authoring-collider2d")
        );
        assert!(report.inspector_edit_applied);
        assert!(report.input_mapping_validated);
        assert_eq!(report.dirty_before_save, Some(true));
        assert_eq!(report.dirty_after_save, Some(false));
        assert!(report.console_entry_count > 0);
        assert!(report.diagnostics.is_empty());
    }
}
