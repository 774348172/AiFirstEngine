use crate::render_graph::{RenderGraph, RenderGraphDiagnostic, RenderGraphDiagnosticSeverity};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderGraphReport {
    pub frame_index: u64,
    pub graph_id: String,
    pub pass_count: usize,
    pub resource_count: usize,
    pub output_target: Option<String>,
    pub warning_count: usize,
    pub error_count: usize,
    pub diagnostics: Vec<RenderGraphDiagnostic>,
}

impl RenderGraphReport {
    pub fn from_graph(graph: &RenderGraph) -> Self {
        let diagnostics = graph.validate();
        let warning_count = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == RenderGraphDiagnosticSeverity::Warning)
            .count();
        let error_count = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == RenderGraphDiagnosticSeverity::Error)
            .count();

        Self {
            frame_index: graph.frame_index,
            graph_id: graph.graph_id.clone(),
            pass_count: graph.passes.len(),
            resource_count: graph.resources.len(),
            output_target: graph.output_target.clone(),
            warning_count,
            error_count,
            diagnostics,
        }
    }

    pub fn has_errors(&self) -> bool {
        self.error_count > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render_graph::{RenderGraph, RenderResource};

    #[test]
    fn report_is_json_serializable_and_counts_errors() {
        let mut graph = RenderGraph::new("graph-1", 7);
        graph.output_target = Some("surface-main".to_string());
        graph
            .resources
            .push(RenderResource::surface_backbuffer("surface-main", 320, 200));

        let report = RenderGraphReport::from_graph(&graph);
        let json = serde_json::to_string(&report).expect("serialize graph report");

        assert_eq!(report.frame_index, 7);
        assert_eq!(report.error_count, 0);
        assert!(json.contains("passCount"));
    }
}
