use serde::{Deserialize, Serialize};
use super::EditorSceneDocument;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneUndoRecord {
    pub transaction_id: String,
    pub command_kind: String,
    pub before_document_snapshot: EditorSceneDocument,
    pub after_document_snapshot: EditorSceneDocument,
}

#[derive(Debug, Clone, Default)]
pub struct SceneUndoStack {
    undo_stack: Vec<SceneUndoRecord>,
    redo_stack: Vec<SceneUndoRecord>,
}

impl SceneUndoStack {
    pub fn push(&mut self, record: SceneUndoRecord) {
        self.undo_stack.push(record);
        self.redo_stack.clear();
    }

    pub fn undo(&mut self, document: &mut EditorSceneDocument) -> Option<SceneUndoRecord> {
        let record = self.undo_stack.pop()?;
        *document = record.before_document_snapshot.clone();
        document.mark_dirty(format!("undo-{}", record.transaction_id));
        self.redo_stack.push(record.clone());
        Some(record)
    }

    pub fn redo(&mut self, document: &mut EditorSceneDocument) -> Option<SceneUndoRecord> {
        let record = self.redo_stack.pop()?;
        *document = record.after_document_snapshot.clone();
        document.mark_dirty(format!("redo-{}", record.transaction_id));
        self.undo_stack.push(record.clone());
        Some(record)
    }
}


