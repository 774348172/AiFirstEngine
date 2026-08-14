use editor_core::EditorSession;
use editor_ui_model::EditorUiModel;

pub struct EditorUiModelComposer;

impl EditorUiModelComposer {
    pub fn compose(session: &EditorSession) -> EditorUiModel {
        session.build_ui_model()
    }
}
