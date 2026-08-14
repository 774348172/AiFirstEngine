#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDiagnostic {
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub path: String,
    pub stage: Option<String>,
    pub message: String,
    pub next_action: Option<String>,
}

impl RuntimeDiagnostic {
    pub fn error(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            code: "runtime.error".to_string(),
            path: path.into(),
            stage: None,
            message: message.into(),
            next_action: None,
        }
    }

    pub fn warning(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Warning,
            code: "runtime.warning".to_string(),
            path: path.into(),
            stage: None,
            message: message.into(),
            next_action: None,
        }
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = code.into();
        self
    }

    pub fn with_stage(mut self, stage: impl Into<String>) -> Self {
        self.stage = Some(stage.into());
        self
    }

    pub fn with_next_action(mut self, next_action: impl Into<String>) -> Self {
        self.next_action = Some(next_action.into());
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeDiagnostics {
    pub issues: Vec<RuntimeDiagnostic>,
}

impl RuntimeDiagnostics {
    pub fn new() -> Self {
        Self { issues: Vec::new() }
    }

    pub fn error(&mut self, path: impl Into<String>, message: impl Into<String>) {
        self.issues.push(RuntimeDiagnostic::error(path, message));
    }

    pub fn warning(&mut self, path: impl Into<String>, message: impl Into<String>) {
        self.issues.push(RuntimeDiagnostic::warning(path, message));
    }

    pub fn push(&mut self, diagnostic: RuntimeDiagnostic) {
        self.issues.push(diagnostic);
    }

    pub fn has_errors(&self) -> bool {
        self.issues
            .iter()
            .any(|issue| issue.severity == DiagnosticSeverity::Error)
    }

    pub fn is_ok(&self) -> bool {
        !self.has_errors()
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeLoadResult<T> {
    pub value: Option<T>,
    pub diagnostics: RuntimeDiagnostics,
}

impl<T> RuntimeLoadResult<T> {
    pub fn ok(value: T, diagnostics: RuntimeDiagnostics) -> Self {
        Self {
            value: Some(value),
            diagnostics,
        }
    }

    pub fn failed(diagnostics: RuntimeDiagnostics) -> Self {
        Self {
            value: None,
            diagnostics,
        }
    }
}
