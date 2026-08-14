#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionDomain {
    RuntimePackage,
    World,
    Render,
    Physics2D,
    AssetRuntime,
    Ui,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionKind {
    Hydration,
    Render,
    Physics2D,
    Asset,
    Ui,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionDiagnostic {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub adapter: Option<String>,
}

impl ProjectionDiagnostic {
    pub fn new(
        severity: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
        adapter: Option<String>,
    ) -> Self {
        Self {
            severity: severity.into(),
            code: code.into(),
            message: message.into(),
            adapter,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionReport {
    pub kind: ProjectionKind,
    pub source_domain: ProjectionDomain,
    pub target_domain: ProjectionDomain,
    pub adapter_name: String,
    pub projected_count: usize,
    pub skipped_count: usize,
    pub error_count: usize,
    pub diagnostics: Vec<ProjectionDiagnostic>,
}

impl ProjectionReport {
    pub fn new(
        kind: ProjectionKind,
        source_domain: ProjectionDomain,
        target_domain: ProjectionDomain,
        adapter_name: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            source_domain,
            target_domain,
            adapter_name: adapter_name.into(),
            projected_count: 0,
            skipped_count: 0,
            error_count: 0,
            diagnostics: Vec::new(),
        }
    }

    pub fn with_counts(
        mut self,
        projected_count: usize,
        skipped_count: usize,
        error_count: usize,
    ) -> Self {
        self.projected_count = projected_count;
        self.skipped_count = skipped_count;
        self.error_count = error_count;
        self
    }

    pub fn with_diagnostics(
        mut self,
        diagnostics: impl IntoIterator<Item = ProjectionDiagnostic>,
    ) -> Self {
        self.diagnostics = diagnostics.into_iter().collect();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_report_expresses_hydration_projection() {
        let report = ProjectionReport::new(
            ProjectionKind::Hydration,
            ProjectionDomain::RuntimePackage,
            ProjectionDomain::World,
            "HydrationProjectionAdapter<RuntimeScene>",
        )
        .with_counts(2, 0, 0);

        assert_eq!(report.kind, ProjectionKind::Hydration);
        assert_eq!(report.source_domain, ProjectionDomain::RuntimePackage);
        assert_eq!(report.target_domain, ProjectionDomain::World);
        assert_eq!(report.projected_count, 2);
        assert_eq!(report.error_count, 0);
    }

    #[test]
    fn projection_report_can_carry_adapter_diagnostics() {
        let report = ProjectionReport::new(
            ProjectionKind::Render,
            ProjectionDomain::World,
            ProjectionDomain::Render,
            "RenderProjectionAdapter<SpriteRenderer2D>",
        )
        .with_counts(0, 1, 1)
        .with_diagnostics([ProjectionDiagnostic::new(
            "error",
            "missing_sprite_asset",
            "SpriteRenderer2D points to a missing sprite asset.",
            Some("RenderProjectionAdapter<SpriteRenderer2D>".to_string()),
        )]);

        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(report.diagnostics[0].severity, "error");
        assert_eq!(report.skipped_count, 1);
    }
}
