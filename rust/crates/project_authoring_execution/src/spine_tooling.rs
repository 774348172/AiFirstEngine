//! Domain data for the existing inspect/check tools; no additional tool or source owner.
use authoring_project_context::ProjectSnapshot;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpineProjectInspection {
    pub capability: &'static str,
    /// Full product qualification still requires the ordinary-project and scale Gates.
    pub capability_status: &'static str,
    pub backend_compiled: bool,
    pub tick_rate: u32,
    pub controllers: Vec<SpineControllerInspection>,
    pub diagnostics: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpineControllerInspection {
    pub source_path: String,
    pub asset_id: String,
    pub skeleton_asset_id: String,
    pub default_action: String,
    pub actions: Vec<SpineActionInspection>,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpineActionInspection {
    pub name: String,
    pub requested_mix_in_seconds: f64,
    pub mix_in_ticks: u32,
    pub requested_fade_out_seconds: Option<f64>,
    pub fade_out_ticks: Option<u32>,
}
pub fn inspect_spine_project_snapshot(snapshot: &ProjectSnapshot) -> SpineProjectInspection {
    let mut report = SpineProjectInspection {
        capability: engine_runtime::spine_animation::SPINE_CAPABILITY,
        capability_status: if cfg!(feature = "spine") {
            "planned"
        } else {
            "unsupported"
        },
        backend_compiled: cfg!(feature = "spine"),
        tick_rate: 60,
        controllers: Vec::new(),
        diagnostics: Vec::new(),
    };
    for file in &snapshot.files {
        if file.relative_path.starts_with("Animations/")
            && file
                .relative_path
                .ends_with(".animator-description-2d.json")
        {
            match crate::neutral_assembler::inspect_spine_description(
                &file.relative_path,
                &file.bytes,
                report.tick_rate,
            ) {
                Ok(Some(controller)) => report.controllers.push(controller),
                Ok(None) => {}
                Err(error) => report
                    .diagnostics
                    .push(format!("{}: {error}", file.relative_path)),
            }
        }
    }
    report
}
