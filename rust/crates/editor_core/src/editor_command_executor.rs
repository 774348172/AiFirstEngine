use crate::{
    command_id_for_payload, CommandResult, EditorCommandPayload, EditorCommandRegistry,
    EditorCommandRequest, EditorSession,
};
use editor_ui_model::{UiCommand, UiCommandPayload};

pub fn ui_command_to_editor_command_request(command: UiCommand) -> EditorCommandRequest {
    EditorCommandRequest {
        command_id: command.command_id,
        source: command.source,
        request_id: command.request_id,
        payload: EditorCommandPayload::Ui(command.payload),
    }
}

pub fn execute_editor_command(
    session: &mut EditorSession,
    request: EditorCommandRequest,
) -> CommandResult {
    let payload = request.payload.into_ui_payload();
    let command_id = command_id_for_payload(&payload);
    if EditorCommandRegistry::builtin()
        .descriptor(command_id)
        .is_none()
    {
        return session.reject_unknown_editor_command(request.request_id, request.source, payload);
    }

    session.execute_ui_command_direct(UiCommand {
        command_id: command_id.to_string(),
        source: request.source,
        request_id: request.request_id,
        payload,
    })
}

pub fn execute_ui_payload_as_editor_command(
    session: &mut EditorSession,
    source: editor_ui_model::UiCommandSource,
    request_id: impl Into<String>,
    payload: UiCommandPayload,
) -> CommandResult {
    execute_editor_command(
        session,
        EditorCommandRequest {
            command_id: command_id_for_payload(&payload).to_string(),
            source,
            request_id: request_id.into(),
            payload: EditorCommandPayload::Ui(payload),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{create_default_editable_project_fixture, CommandStatus, EditorSession};
    use editor_ui_model::{UiCommandPayload, UiCommandSource};

    #[test]
    fn legacy_ui_command_maps_to_editor_command_request() {
        let command = UiCommand {
            command_id: "play".to_string(),
            source: UiCommandSource::Toolbar,
            request_id: "request".to_string(),
            payload: UiCommandPayload::Play,
        };

        let request = ui_command_to_editor_command_request(command);

        assert_eq!(request.command_id, "play");
        assert_eq!(request.source, UiCommandSource::Toolbar);
        assert_eq!(request.request_id, "request");
        assert_eq!(request.payload.as_ui_payload(), &UiCommandPayload::Play);
    }

    #[test]
    fn scene_create_entity_command_executes_through_framework() {
        let mut session = EditorSession::new();
        let fixture = create_default_editable_project_fixture();
        let open = execute_ui_payload_as_editor_command(
            &mut session,
            UiCommandSource::Test,
            "request-open",
            UiCommandPayload::OpenSceneDocument {
                path: fixture.scene_path.display().to_string(),
            },
        );
        assert_eq!(open.status, CommandStatus::Committed);

        let result = execute_ui_payload_as_editor_command(
            &mut session,
            UiCommandSource::Test,
            "request-create",
            UiCommandPayload::CreateSceneEntity {
                parent_id: None,
                name: "Created Through Framework".to_string(),
            },
        );

        assert_eq!(result.command_id, "create_scene_entity");
        assert_eq!(result.status, CommandStatus::Committed);
    }
}
