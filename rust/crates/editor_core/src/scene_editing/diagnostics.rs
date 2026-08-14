use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SceneEditDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneEditDiagnostic {
    pub severity: SceneEditDiagnosticSeverity,
    pub code: String,
    pub layer: String,
    pub message: String,
    pub path: Option<String>,
    pub entity_id: Option<String>,
}

impl SceneEditDiagnostic {
    pub fn info(
        code: impl Into<String>,
        layer: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(SceneEditDiagnosticSeverity::Info, code, layer, message)
    }

    pub fn warning(
        code: impl Into<String>,
        layer: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(SceneEditDiagnosticSeverity::Warning, code, layer, message)
    }

    pub fn error(
        code: impl Into<String>,
        layer: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(SceneEditDiagnosticSeverity::Error, code, layer, message)
    }

    fn new(
        severity: SceneEditDiagnosticSeverity,
        code: impl Into<String>,
        layer: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            layer: layer.into(),
            message: message.into(),
            path: None,
            entity_id: None,
        }
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn with_entity_id(mut self, entity_id: impl Into<String>) -> Self {
        self.entity_id = Some(entity_id.into());
        self
    }
}


