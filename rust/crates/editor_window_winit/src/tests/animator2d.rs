use crate::EditorUiModelComposer;
use editor_core::EditorSession;
use editor_ui_model::{
    Animator2DAuthoringCommand, Animator2DAuthoringStatus, Animator2DConditionModel,
    Animator2DStateModel, Animator2DTransitionModel, Animator2DTransitionTimingModel,
};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn animator2d_authoring_controls_are_reachable_without_an_editable_graph() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let controller_path = std::env::temp_dir()
        .join(format!("animator2d-window-{stamp}"))
        .join("controller.animator2d.json");
    let mut session = EditorSession::new();

    for command in [
        Animator2DAuthoringCommand::CreateController {
            path: controller_path.display().to_string(),
            asset_id: "controller".to_string(),
        },
        Animator2DAuthoringCommand::UpsertState {
            state: Animator2DStateModel {
                id: "idle".to_string(),
                clip_ref: "idle-clip".to_string(),
                speed_permille: 1000,
            },
        },
        Animator2DAuthoringCommand::SetEntryState {
            state_id: "idle".to_string(),
        },
        Animator2DAuthoringCommand::UpsertTransition {
            transition: Animator2DTransitionModel {
                id: "idle-loop".to_string(),
                from: "idle".to_string(),
                to: "idle".to_string(),
                timing: Animator2DTransitionTimingModel::ClipEnd,
                priority: 0,
                conditions: Vec::<Animator2DConditionModel>::new(),
            },
        },
    ] {
        assert_eq!(
            session.execute_animator2d_authoring_command(command).status,
            Animator2DAuthoringStatus::Applied
        );
    }

    let model = EditorUiModelComposer::compose(&session).animator2d_authoring;
    assert_eq!(model.controller.as_ref().unwrap().states.len(), 1);
    assert_eq!(model.relationship_edges.len(), 1);
    assert!(!model.relationship_graph_editable);
    assert!(model.sprite_picker_enabled);
    assert!(model.controller_picker_enabled);
    for command_id in [
        "animator2d.save",
        "animator2d.preview.play",
        "animator2d.preview.pause",
        "animator2d.preview.restart",
        "animator2d.preview.step",
        "animator2d.preview.close",
    ] {
        assert!(model
            .controls
            .iter()
            .any(|control| control.command_id == command_id));
    }
}
