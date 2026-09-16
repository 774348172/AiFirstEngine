use editor_core::{
    AuiTemplateAsset, AuiTemplateInstantiateReport, AuiTemplateInstantiateRequest,
    AuiTemplateOperationStatus, AuiTemplateRef, AuiTemplateWorkflow,
};
use engine_runtime::aui::{
    AuiActionRef, AuiAssetManifest, AuiAssetManifestEntry, AuiBindingRef, AuiBindingTarget,
    AuiBindingValue, AuiCanvas, AuiDocument, AuiLayoutEngine, AuiNode, AuiNodeKind, AuiRect,
    AuiRendererBridge, AuiStyle,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const COMPLEX_SHOOTER_AUI_TEMPLATE_REUSE_REPORT_SCHEMA_VERSION: &str =
    "complex-shooter-aui-template-reuse-productization-report.v1";
pub const COMPLEX_SHOOTER_AUI_TEMPLATE_REUSE_SCENARIO_ID: &str =
    "complex-shooter-aui-template-reuse-productization-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComplexShooterAuiTemplateReuseStatus {
    Passed,
    Partial,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterAuiTemplateReuseMetrics {
    pub template_node_count: usize,
    pub instance_count: usize,
    pub inserted_node_count: usize,
    pub node_id_remap_count: usize,
    pub copied_binding_ref_count: usize,
    pub copied_action_ref_count: usize,
    pub copied_asset_ref_count: usize,
    pub warning_count: usize,
    pub target_document_node_count: usize,
    pub draw_command_count: usize,
    pub composition_draw_item_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterAuiTemplateReuseReport {
    pub schema_version: String,
    pub scenario_id: String,
    pub status: ComplexShooterAuiTemplateReuseStatus,
    pub project_root: String,
    pub output_root: String,
    pub template_asset_path: String,
    pub target_document_path: String,
    pub instantiate_reports: Vec<AuiTemplateInstantiateReport>,
    pub metrics: ComplexShooterAuiTemplateReuseMetrics,
    pub artifacts: Vec<String>,
    pub diagnostics: Vec<String>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ComplexShooterAuiTemplateReuseRequest {
    pub project_root: PathBuf,
    pub output_root: PathBuf,
}

impl ComplexShooterAuiTemplateReuseRequest {
    pub fn new(project_root: impl Into<PathBuf>, output_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            output_root: output_root.into(),
        }
    }
}

pub fn run_complex_shooter_aui_template_reuse_report(
    request: ComplexShooterAuiTemplateReuseRequest,
) -> ComplexShooterAuiTemplateReuseReport {
    let fixture_root = request.output_root.join("aui-template-reuse-fixture");
    let template_asset_path = fixture_root
        .join("AUI")
        .join("Templates")
        .join("equipment-slot.aui-template.json");
    let target_document_path = fixture_root.join("AUI").join("equipment-panel.aui.json");
    let mut report = ComplexShooterAuiTemplateReuseReport {
        schema_version: COMPLEX_SHOOTER_AUI_TEMPLATE_REUSE_REPORT_SCHEMA_VERSION.to_string(),
        scenario_id: COMPLEX_SHOOTER_AUI_TEMPLATE_REUSE_SCENARIO_ID.to_string(),
        status: ComplexShooterAuiTemplateReuseStatus::Failed,
        project_root: request.project_root.display().to_string(),
        output_root: request.output_root.display().to_string(),
        template_asset_path: template_asset_path.display().to_string(),
        target_document_path: target_document_path.display().to_string(),
        instantiate_reports: Vec::new(),
        metrics: ComplexShooterAuiTemplateReuseMetrics {
            template_node_count: 0,
            instance_count: 0,
            inserted_node_count: 0,
            node_id_remap_count: 0,
            copied_binding_ref_count: 0,
            copied_action_ref_count: 0,
            copied_asset_ref_count: 0,
            warning_count: 0,
            target_document_node_count: 0,
            draw_command_count: 0,
            composition_draw_item_count: 0,
        },
        artifacts: Vec::new(),
        diagnostics: Vec::new(),
        next_actions: Vec::new(),
    };

    let source = equipment_slot_source_document();
    let asset = match AuiTemplateAsset::from_document_subtree(
        &source,
        "AUI/equipment-source.aui.json",
        &template_asset_path,
        "equipment_slot",
        "equipment_slot_template",
        "Equipment Slot Template",
        0,
    ) {
        Ok(asset) => asset,
        Err(diagnostics) => {
            report.diagnostics.extend(
                diagnostics
                    .into_iter()
                    .map(|diagnostic| format!("error:{}:{}", diagnostic.code, diagnostic.message)),
            );
            recompute_report(&mut report);
            return write_report(request.output_root, report);
        }
    };
    if let Some(parent) = template_asset_path.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            report.diagnostics.push(format!(
                "error:aui_template_asset_dir_failed:{}:{}",
                parent.display(),
                error
            ));
            recompute_report(&mut report);
            return write_report(request.output_root, report);
        }
    }
    if let Err(error) = asset.save(&template_asset_path) {
        report.diagnostics.push(format!(
            "error:aui_template_asset_write_failed:{}:{}",
            template_asset_path.display(),
            error
        ));
        recompute_report(&mut report);
        return write_report(request.output_root, report);
    }
    report
        .artifacts
        .push(template_asset_path.display().to_string());
    report.metrics.template_node_count = asset.nodes.len();

    let mut target = equipment_panel_target_document();
    for index in 0..3 {
        let request = AuiTemplateInstantiateRequest {
            template_ref: AuiTemplateRef {
                asset_guid: asset.asset_guid.clone(),
                template_id: asset.template_id.clone(),
                asset_path: template_asset_path.display().to_string(),
            },
            target_document_path: target_document_path.display().to_string(),
            parent_node_id: "equipment_grid".to_string(),
            insertion_index: None,
            instance_id: format!("slot_instance_{}", index + 1),
            node_id_prefix: format!("slot{}", index + 1),
        };
        let instance_report =
            AuiTemplateWorkflow::instantiate_into_document(&asset, &request, &mut target);
        report.instantiate_reports.push(instance_report);
    }
    if let Some(parent) = target_document_path.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            report.diagnostics.push(format!(
                "error:aui_target_dir_create_failed:{}:{}",
                parent.display(),
                error
            ));
        }
    }
    match serde_json::to_string_pretty(&target)
        .map_err(|error| error.to_string())
        .and_then(|text| fs::write(&target_document_path, text).map_err(|error| error.to_string()))
    {
        Ok(()) => report
            .artifacts
            .push(target_document_path.display().to_string()),
        Err(error) => report.diagnostics.push(format!(
            "error:aui_target_document_write_failed:{}:{}",
            target_document_path.display(),
            error
        )),
    }

    let validation = AuiLayoutEngine::validate(&target, Some(&equipment_asset_manifest()));
    if !validation.ok {
        report
            .diagnostics
            .extend(validation.report_items.iter().map(|item| {
                format!(
                    "error:aui_validation:{}:{}",
                    item.code,
                    item.message.replace('\n', " ")
                )
            }));
    }
    let layout = AuiLayoutEngine::layout(&target, 1);
    let (draw_list, draw_report) = AuiLayoutEngine::extract_draw_list(&target, &layout);
    let composition = AuiRendererBridge::build_composition_frame(1, &target, &layout, &draw_list);
    report.metrics.target_document_node_count = target.nodes.len();
    report.metrics.draw_command_count = draw_report.draw_command_count;
    report.metrics.composition_draw_item_count = composition
        .stages
        .iter()
        .map(|stage| stage.draw_items.len())
        .sum();

    recompute_report(&mut report);
    write_report(request.output_root, report)
}

fn write_report(
    output_root: PathBuf,
    mut report: ComplexShooterAuiTemplateReuseReport,
) -> ComplexShooterAuiTemplateReuseReport {
    let artifact_path = output_root
        .join("reports")
        .join("complex-shooter-aui-template-reuse-productization-report.json");
    if let Some(parent) = artifact_path.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            report.diagnostics.push(format!(
                "error:aui_template_report_dir_create_failed:{}:{}",
                parent.display(),
                error
            ));
        }
    }
    recompute_report(&mut report);
    match serde_json::to_string_pretty(&report)
        .map_err(|error| error.to_string())
        .and_then(|text| fs::write(&artifact_path, text).map_err(|error| error.to_string()))
    {
        Ok(()) => report.artifacts.push(artifact_path.display().to_string()),
        Err(error) => report.diagnostics.push(format!(
            "error:aui_template_report_write_failed:{}:{}",
            artifact_path.display(),
            error
        )),
    }
    recompute_report(&mut report);
    report
}

fn recompute_report(report: &mut ComplexShooterAuiTemplateReuseReport) {
    report.metrics.instance_count = report.instantiate_reports.len();
    report.metrics.inserted_node_count = report
        .instantiate_reports
        .iter()
        .map(|item| item.inserted_node_count)
        .sum();
    report.metrics.node_id_remap_count = report
        .instantiate_reports
        .iter()
        .map(|item| item.node_id_remap.len())
        .sum();
    report.metrics.copied_binding_ref_count = report
        .instantiate_reports
        .iter()
        .map(|item| item.copied_binding_refs.len())
        .sum();
    report.metrics.copied_action_ref_count = report
        .instantiate_reports
        .iter()
        .map(|item| item.copied_action_refs.len())
        .sum();
    report.metrics.copied_asset_ref_count = report
        .instantiate_reports
        .iter()
        .map(|item| item.copied_asset_refs.len())
        .sum();
    report.metrics.warning_count = report
        .instantiate_reports
        .iter()
        .flat_map(|item| item.diagnostics.iter())
        .filter(|diagnostic| {
            diagnostic.severity == editor_core::AuiTemplateDiagnosticSeverity::Warning
        })
        .count();
    report.next_actions.clear();
    if report.metrics.warning_count == 0 {
        report
            .next_actions
            .push("verify_aui_template_dependency_warnings".to_string());
    }
    if report.metrics.composition_draw_item_count == 0 {
        report
            .next_actions
            .push("fix_aui_template_present_smoke".to_string());
    }
    report.next_actions.sort();
    report.next_actions.dedup();
    if report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.starts_with("error:"))
        || report
            .instantiate_reports
            .iter()
            .any(|item| item.status == AuiTemplateOperationStatus::Failed)
    {
        report.status = ComplexShooterAuiTemplateReuseStatus::Failed;
    } else if !report.next_actions.is_empty() {
        report.status = ComplexShooterAuiTemplateReuseStatus::Partial;
    } else {
        report.status = ComplexShooterAuiTemplateReuseStatus::Passed;
    }
}

fn equipment_slot_source_document() -> AuiDocument {
    let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_children(["equipment_slot"]);
    let slot = AuiNode::new(
        "equipment_slot",
        AuiNodeKind::Button,
        AuiRect::fixed_position(0.0, 0.0, 96.0, 96.0),
    )
    .with_parent("root")
    .with_children(["slot_bg", "slot_icon", "slot_name"])
    .with_style(AuiStyle::color("#202830"))
    .with_action(AuiActionRef::click("ui.open_equipment_detail"));
    let slot_bg = AuiNode::new(
        "slot_bg",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(0.0, 0.0, 96.0, 96.0),
    )
    .with_parent("equipment_slot")
    .with_style(AuiStyle::color("#2c3542"));
    let slot_icon = AuiNode::new(
        "slot_icon",
        AuiNodeKind::Image,
        AuiRect::fixed_position(16.0, 12.0, 64.0, 48.0),
    )
    .with_parent("equipment_slot")
    .with_image("tex-sword")
    .with_binding(AuiBindingRef::new(
        "bind.icon",
        AuiBindingTarget::ImageAssetRef,
        "equipment.icon",
        Some(AuiBindingValue::AssetRef(
            engine_runtime::aui::AuiAssetRef::new("tex-sword"),
        )),
    ));
    let slot_name = AuiNode::new(
        "slot_name",
        AuiNodeKind::Text,
        AuiRect::fixed_position(8.0, 68.0, 80.0, 20.0),
    )
    .with_parent("equipment_slot")
    .with_text("Sword")
    .with_style(AuiStyle::text("#ffffff", 14.0))
    .with_binding(AuiBindingRef::new(
        "bind.name",
        AuiBindingTarget::TextText,
        "equipment.name",
        Some(AuiBindingValue::String("Sword".to_string())),
    ));
    AuiDocument::new(
        "equipment-source",
        vec![AuiCanvas::screen_overlay("main", 1280.0, 720.0, "root")],
        vec![root, slot, slot_bg, slot_icon, slot_name],
    )
}

fn equipment_panel_target_document() -> AuiDocument {
    let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_children(["equipment_grid"]);
    let grid = AuiNode::new(
        "equipment_grid",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(40.0, 40.0, 360.0, 120.0),
    )
    .with_parent("root")
    .with_style(AuiStyle::color("#101820"));
    AuiDocument::new(
        "equipment-panel",
        vec![AuiCanvas::screen_overlay("main", 1280.0, 720.0, "root")],
        vec![root, grid],
    )
}

fn equipment_asset_manifest() -> AuiAssetManifest {
    AuiAssetManifest::new(
        "equipment-assets",
        vec![AuiAssetManifestEntry::image(
            "tex-sword",
            "asset://ui/equipment/sword.png",
            vec![
                "slot1_slot_icon".to_string(),
                "slot2_slot_icon".to_string(),
                "slot3_slot_icon".to_string(),
            ],
        )],
    )
}
