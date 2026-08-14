use editor_ui_model::{
    RuleAuthoringCommand, RuleAuthoringDiagnostic, RuleAuthoringDiagnosticSeverity,
    RuleAuthoringDocument, RuleAuthoringModel, RuleAuthoringReport, RuleAuthoringStageEvidence,
    RuleAuthoringStageStatus, RuleAuthoringStatus, RuleCardAuthoringModel, RuleCardAuthoringReport,
    RuleCardDiagnosticRef, RuleCardFieldModel, RuleCardFieldValueKind, RuleCardKind, RuleCardModel,
    RuleCardSourceMapping, RuleCardValidationState, RuleGraphPreviewEdge, RuleGraphPreviewEdgeKind,
    RuleGraphPreviewGroup, RuleGraphPreviewModel, RuleGraphPreviewNode, RuleGraphPreviewNodeKind,
    RuleGraphPreviewNodeStatus, RULE_AUTHORING_REPORT_SCHEMA_VERSION,
    RULE_CARD_AUTHORING_REPORT_SCHEMA_VERSION, RULE_GRAPH_PREVIEW_SCHEMA_VERSION,
};
use engine_runtime::project_rule_asset::{
    read_project_rule_asset_json, runtime_rule_manifest_from_assets, ProjectRuleAsset,
    ProjectRuleAssetSourceKind, ProjectRuleAssetValidationStatus,
};
use engine_runtime::rule_artifact::{
    validate_runtime_rule_manifest_artifacts, RuleArtifactManifest, RuleArtifactRegistry,
};
use engine_runtime::rule_compiler::{
    generate_static_registry_source, RuleCompileDiagnostic, RuleCompileRequest, RuleCompileStatus,
    RuleCompiler,
};
use engine_runtime::rule_ir::{
    ProjectRuleIr, ProjectRulePhase, RuleIrDiagnostic, RuleOperation, RuleStatement, RuleTrigger,
};
use engine_runtime::runtime_package::RuntimeRuleModuleKind;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const RULE_AUTHORING_DEFAULT_GENERATED_ROOT: &str = "target/generated-rules";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RuleAuthoringEditCommand {
    SetTrigger(RuleTrigger),
    AddStatement(RuleStatement),
    UpdateStatement {
        index: usize,
        statement: RuleStatement,
    },
    RemoveStatement {
        index: usize,
    },
    AddOperation(RuleOperation),
    UpdateOperation {
        index: usize,
        operation: RuleOperation,
    },
    RemoveOperation {
        index: usize,
    },
}

pub struct RuleAuthoringService;

impl RuleAuthoringService {
    pub fn build_project_manifest(
        project_root: &Path,
        relative_path: &str,
    ) -> Result<engine_runtime::runtime_package::RuntimeRuleManifest, String> {
        if relative_path.replace('\\', "/") != "Rules/rule-manifest.json" {
            return Err("Project Rule manifest path must be Rules/rule-manifest.json.".to_string());
        }
        let paths = scan_rule_asset_paths(project_root);
        if paths.is_empty() {
            return Err("Project Rule manifest requires at least one RuleAsset.".to_string());
        }
        let mut assets = Vec::with_capacity(paths.len());
        for path in paths {
            let asset = Self::load(project_root, &path)?;
            let validation = asset.validate();
            if validation.status != ProjectRuleAssetValidationStatus::Success {
                return Err(format!(
                    "RuleAsset validation failed before manifest build: {path}"
                ));
            }
            assets.push(asset);
        }
        let manifest = runtime_rule_manifest_from_assets(assets.iter(), "");
        let validation = validate_runtime_rule_manifest_artifacts(None, &manifest);
        if !validation.is_ok() {
            return Err(format!(
                "Runtime Rule manifest validation failed: {}",
                validation
                    .issues
                    .iter()
                    .map(|issue| issue.code)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        let bytes = serde_json::to_vec_pretty(&manifest)
            .map_err(|error| format!("Failed to serialize Runtime Rule manifest: {error}"))?;
        let scope =
            crate::ProjectWriteScope::open(project_root).map_err(|error| error.to_string())?;
        scope
            .write_atomic(relative_path, &bytes)
            .map_err(|error| format!("Failed to save Runtime Rule manifest: {error}"))?;
        Ok(manifest)
    }

    pub fn create_asset(
        project_root: &Path,
        relative_path: &str,
        rule_id: &str,
        display_name: &str,
    ) -> Result<ProjectRuleAsset, String> {
        let scope =
            crate::ProjectWriteScope::open(project_root).map_err(|error| error.to_string())?;
        Self::create_asset_in_scope(&scope, relative_path, rule_id, display_name)
    }

    pub fn create_asset_in_scope(
        scope: &crate::ProjectWriteScope,
        relative_path: &str,
        rule_id: &str,
        display_name: &str,
    ) -> Result<ProjectRuleAsset, String> {
        Self::create_asset_with_phase_in_scope(
            scope,
            relative_path,
            rule_id,
            display_name,
            ProjectRulePhase::Update,
        )
    }

    pub fn create_asset_with_phase_in_scope(
        scope: &crate::ProjectWriteScope,
        relative_path: &str,
        rule_id: &str,
        display_name: &str,
        phase: ProjectRulePhase,
    ) -> Result<ProjectRuleAsset, String> {
        if rule_id.trim().is_empty() {
            return Err("Rule id cannot be empty.".to_string());
        }
        let display_name = if display_name.trim().is_empty() {
            rule_id
        } else {
            display_name
        };
        let ir = ProjectRuleIr::new(rule_id.to_string(), phase);
        let asset_id = rule_asset_id_from_path_or_rule(relative_path, rule_id);
        let asset = ProjectRuleAsset::new(
            asset_id,
            display_name.to_string(),
            ProjectRuleAssetSourceKind::UserAuthored,
            ir,
        );
        Self::save_in_scope(scope, relative_path, &asset)?;
        Ok(asset)
    }

    pub fn load(project_root: &Path, relative_path: &str) -> Result<ProjectRuleAsset, String> {
        let path = project_root.join(normalize_project_relative_path(relative_path));
        read_project_rule_asset_json(path).map_err(|error| {
            format!(
                "Failed to load rule asset {}: {}",
                error.path, error.message
            )
        })
    }

    pub fn save(
        project_root: &Path,
        relative_path: &str,
        asset: &ProjectRuleAsset,
    ) -> Result<(), String> {
        let scope =
            crate::ProjectWriteScope::open(project_root).map_err(|error| error.to_string())?;
        Self::save_in_scope(&scope, relative_path, asset)
    }

    pub fn save_in_scope(
        scope: &crate::ProjectWriteScope,
        relative_path: &str,
        asset: &ProjectRuleAsset,
    ) -> Result<(), String> {
        let text = serde_json::to_string_pretty(asset)
            .map_err(|error| format!("Failed to serialize rule asset {relative_path}: {error}"))?;
        scope
            .write_atomic(relative_path, text.as_bytes())
            .map(|_| ())
            .map_err(|error| format!("Failed to save rule asset {relative_path}: {error}"))
    }

    pub fn apply(
        asset: &mut ProjectRuleAsset,
        command: RuleAuthoringEditCommand,
        expected_ir_hash: Option<&str>,
    ) -> Result<Vec<String>, String> {
        if let Some(expected) = expected_ir_hash {
            let actual = asset.ir_hash();
            if expected != actual {
                return Err(format!(
                    "Rule IR hash mismatch: expected {expected}, actual {actual}"
                ));
            }
        }

        let changed_path = match command {
            RuleAuthoringEditCommand::SetTrigger(trigger) => {
                asset.canonical_ir.trigger = trigger;
                "canonicalIr.trigger".to_string()
            }
            RuleAuthoringEditCommand::AddStatement(statement) => {
                let index = asset.canonical_ir.statements.len();
                asset.canonical_ir.statements.push(statement);
                format!("canonicalIr.statements[{index}]")
            }
            RuleAuthoringEditCommand::UpdateStatement { index, statement } => {
                let Some(existing) = asset.canonical_ir.statements.get_mut(index) else {
                    return Err(format!("Rule statement index out of range: {index}"));
                };
                *existing = statement;
                format!("canonicalIr.statements[{index}]")
            }
            RuleAuthoringEditCommand::RemoveStatement { index } => {
                if index >= asset.canonical_ir.statements.len() {
                    return Err(format!("Rule statement index out of range: {index}"));
                }
                asset.canonical_ir.statements.remove(index);
                format!("canonicalIr.statements[{index}]")
            }
            RuleAuthoringEditCommand::AddOperation(operation) => {
                let index = asset.canonical_ir.operations.len();
                asset.canonical_ir.operations.push(operation);
                format!("canonicalIr.operations[{index}]")
            }
            RuleAuthoringEditCommand::UpdateOperation { index, operation } => {
                let Some(existing) = asset.canonical_ir.operations.get_mut(index) else {
                    return Err(format!("Rule operation index out of range: {index}"));
                };
                *existing = operation;
                format!("canonicalIr.operations[{index}]")
            }
            RuleAuthoringEditCommand::RemoveOperation { index } => {
                if index >= asset.canonical_ir.operations.len() {
                    return Err(format!("Rule operation index out of range: {index}"));
                }
                asset.canonical_ir.operations.remove(index);
                format!("canonicalIr.operations[{index}]")
            }
        };
        asset.rule_id = asset.canonical_ir.rule_id.clone();
        Ok(vec![changed_path])
    }

    pub fn validate(asset: &ProjectRuleAsset) -> RuleAuthoringReport {
        let validation = asset.validate();
        let diagnostics = validation
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic_from_ir(asset, diagnostic))
            .collect::<Vec<_>>();
        let status = if validation.status == ProjectRuleAssetValidationStatus::Success {
            RuleAuthoringStatus::Valid
        } else {
            RuleAuthoringStatus::Invalid
        };
        base_report(asset, status, diagnostics, Vec::new())
    }

    pub fn build(project_root: &Path, relative_path: &str) -> Result<RuleAuthoringReport, String> {
        let asset = Self::load(project_root, relative_path)?;
        Ok(Self::build_loaded(project_root, relative_path, &asset))
    }

    pub fn build_loaded(
        project_root: &Path,
        relative_path: &str,
        asset: &ProjectRuleAsset,
    ) -> RuleAuthoringReport {
        let generated_root = project_root.join(RULE_AUTHORING_DEFAULT_GENERATED_ROOT);
        let request = RuleCompileRequest::dev_desktop(generated_root);
        let compile = RuleCompiler::compile(&request, &asset.canonical_ir, None);
        let registry_source = generate_static_registry_source(&[asset.canonical_ir.clone()]);
        let artifact_manifest = RuleArtifactManifest::from_compile_reports(
            &[compile.clone()],
            RuntimeRuleModuleKind::StaticRegistry,
        );
        let runtime_manifest = runtime_rule_manifest_from_assets([asset], "");
        let artifact_lifecycle_report = RuleArtifactRegistry::from_manifest(&artifact_manifest)
            .map(|registry| registry.validate_runtime_manifest(&runtime_manifest))
            .unwrap_or_else(|report| report);
        let runtime_manifest_report =
            validate_runtime_rule_manifest_artifacts(None, &runtime_manifest);

        let mut diagnostics = compile
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic_from_compile(asset, diagnostic))
            .collect::<Vec<_>>();
        diagnostics.extend(artifact_lifecycle_report.issues.iter().map(|issue| {
            diagnostic_from_parts(
                asset,
                issue.code,
                &issue.message,
                Some(issue.path.clone()),
                Some("Fix the rule artifact manifest and rebuild.".to_string()),
            )
        }));
        diagnostics.extend(runtime_manifest_report.issues.iter().map(|issue| {
            diagnostic_from_parts(
                asset,
                issue.code,
                &issue.message,
                Some(issue.path.clone()),
                Some("Fix the runtime rule manifest before export.".to_string()),
            )
        }));

        let mut report = base_report(
            asset,
            if compile.status == RuleCompileStatus::Success && diagnostics.is_empty() {
                RuleAuthoringStatus::Built
            } else {
                RuleAuthoringStatus::Failed
            },
            diagnostics,
            Vec::new(),
        );
        report.generated_rust_source = if let Some(path) = compile.generated_source_path.clone() {
            RuleAuthoringStageEvidence {
                status: RuleAuthoringStageStatus::Produced,
                path: Some(path),
                artifact_id: compile.artifact_id.clone(),
                summary: "generated_rust_source=produced".to_string(),
                skip_reason: None,
                next_action: None,
            }
        } else {
            RuleAuthoringStageEvidence {
                status: RuleAuthoringStageStatus::Blocked,
                path: None,
                artifact_id: None,
                summary: "generated_rust_source=blocked".to_string(),
                skip_reason: None,
                next_action: Some("Fix rule validation diagnostics.".to_string()),
            }
        };
        report.static_registry_source = RuleAuthoringStageEvidence {
            status: RuleAuthoringStageStatus::Produced,
            path: Some(format!(
                "{}/generated_registry.rs",
                RULE_AUTHORING_DEFAULT_GENERATED_ROOT
            )),
            artifact_id: compile.artifact_id.clone(),
            summary: format!(
                "static_registry_source=produced module={} rule_count={}",
                registry_source.module_name,
                registry_source.rule_ids.len()
            ),
            skip_reason: None,
            next_action: None,
        };
        report.artifact_lifecycle = RuleAuthoringStageEvidence {
            status: if artifact_lifecycle_report.is_ok() {
                RuleAuthoringStageStatus::Validated
            } else {
                RuleAuthoringStageStatus::Blocked
            },
            path: Some(relative_path.to_string()),
            artifact_id: compile.artifact_id.clone(),
            summary: format!(
                "artifact_lifecycle={} issue_count={}",
                if artifact_lifecycle_report.is_ok() {
                    "validated"
                } else {
                    "blocked"
                },
                artifact_lifecycle_report.issues.len()
            ),
            skip_reason: None,
            next_action: (!artifact_lifecycle_report.is_ok())
                .then(|| "Fix rule artifact lifecycle diagnostics.".to_string()),
        };
        report.runtime_package_manifest = RuleAuthoringStageEvidence {
            status: if runtime_manifest_report.is_ok() {
                RuleAuthoringStageStatus::Ready
            } else {
                RuleAuthoringStageStatus::Blocked
            },
            path: Some("Rules/rule-manifest.json".to_string()),
            artifact_id: compile.artifact_id,
            summary: format!(
                "runtime_package_manifest={} issue_count={}",
                if runtime_manifest_report.is_ok() {
                    "ready"
                } else {
                    "blocked"
                },
                runtime_manifest_report.issues.len()
            ),
            skip_reason: None,
            next_action: (!runtime_manifest_report.is_ok())
                .then(|| "Fix runtime rule manifest diagnostics.".to_string()),
        };
        report.cargo_build = RuleAuthoringStageEvidence::skipped(
            "skipped_by_v1",
            "Run project export or CI build for full cargo/player validation.",
        );
        report.next_actions = next_actions_for_report(&report);
        report
    }

    pub fn document(
        project_root: &Path,
        relative_path: &str,
    ) -> Result<RuleAuthoringDocument, String> {
        let asset = Self::load(project_root, relative_path)?;
        Ok(document_from_asset(Some(relative_path.to_string()), &asset))
    }

    pub fn build_model(
        project_root: Option<&Path>,
        selected_path: Option<String>,
    ) -> RuleAuthoringModel {
        Self::build_model_with_selection(project_root, selected_path, None, None)
    }

    pub fn build_model_with_selection(
        project_root: Option<&Path>,
        selected_path: Option<String>,
        selected_card_id: Option<String>,
        selected_graph_node_id: Option<String>,
    ) -> RuleAuthoringModel {
        let Some(project_root) = project_root else {
            return RuleAuthoringModel::empty();
        };
        let paths = scan_rule_asset_paths(project_root);
        let selected_path = selected_path
            .filter(|path| is_rule_asset_relative_path(path))
            .or_else(|| paths.first().cloned());
        let loaded_asset = selected_path
            .as_deref()
            .and_then(|path| Self::load(project_root, path).ok());
        let document = match (&selected_path, &loaded_asset) {
            (Some(path), Some(asset)) => document_from_asset(Some(path.clone()), asset),
            _ => RuleAuthoringDocument::empty(),
        };
        let has_project = true;
        let has_rule = selected_path.is_some();
        let commands = rule_authoring_commands(has_project, has_rule);
        let card_authoring = card_authoring_model_from_asset(
            Some(project_root),
            selected_path.as_deref(),
            paths.len(),
            &document,
            loaded_asset.as_ref(),
            selected_card_id,
            selected_graph_node_id,
        );
        RuleAuthoringModel {
            project_root: Some(project_root.display().to_string()),
            selected_path,
            rule_count: paths.len(),
            document,
            card_authoring,
            commands,
            empty_message: if paths.is_empty() {
                "No rule assets yet.".to_string()
            } else {
                String::new()
            },
        }
    }
}

pub fn scan_rule_asset_paths(project_root: &Path) -> Vec<String> {
    let rule_dir = project_root.join("Rules");
    let Ok(read_dir) = fs::read_dir(rule_dir) else {
        return Vec::new();
    };
    let mut paths = read_dir
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| is_rule_asset_path(path))
        .map(|path| {
            path.strip_prefix(project_root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

pub fn is_rule_asset_relative_path(path: &str) -> bool {
    let lower = path.replace('\\', "/").to_ascii_lowercase();
    lower.starts_with("rules/") && is_rule_asset_file_name(&lower)
}

pub fn decode_rule_trigger(value: serde_json::Value) -> Result<RuleTrigger, String> {
    serde_json::from_value(value).map_err(|error| format!("Failed to parse RuleTrigger: {error}"))
}

pub fn decode_rule_statement(value: serde_json::Value) -> Result<RuleStatement, String> {
    serde_json::from_value(value).map_err(|error| format!("Failed to parse RuleStatement: {error}"))
}

pub fn decode_rule_operation(value: serde_json::Value) -> Result<RuleOperation, String> {
    serde_json::from_value(value).map_err(|error| format!("Failed to parse RuleOperation: {error}"))
}

pub fn explain_rule_diagnostic(
    rule_name: &str,
    code: &str,
    path: Option<&str>,
    message: &str,
    suggestion: Option<&str>,
) -> (String, Option<String>) {
    let path = path.unwrap_or("rule");
    match code {
        "InvalidFieldPath" => (
            format!(
                "Rule {rule_name} has an invalid field path at {path}. Check the component field name and use a simple dot path."
            ),
            suggestion
                .map(str::to_string)
                .or_else(|| Some("Use simple dot paths without array indexes.".to_string())),
        ),
        "MissingActionId" => (
            format!(
                "Rule {rule_name} has an action trigger or condition without action_id at {path}. Set a stable Input action id before validation."
            ),
            suggestion
                .map(str::to_string)
                .or_else(|| Some("Set action_id to an existing Input action.".to_string())),
        ),
        "MissingPrefabRef" => (
            format!(
                "Rule {rule_name} instantiates a prefab without prefab_ref.id at {path}. Select a prefab asset from the project library."
            ),
            suggestion
                .map(str::to_string)
                .or_else(|| Some("Set prefab_ref.id to a valid Prefab asset id.".to_string())),
        ),
        _ => (
            format!("Rule {rule_name} reported {code} at {path}: {message}"),
            suggestion.map(str::to_string),
        ),
    }
}

fn document_from_asset(
    asset_path: Option<String>,
    asset: &ProjectRuleAsset,
) -> RuleAuthoringDocument {
    let report = RuleAuthoringService::validate(asset);
    RuleAuthoringDocument {
        asset_path,
        asset_id: Some(asset.asset_id.clone()),
        rule_id: Some(asset.rule_id.clone()),
        display_name: Some(asset.display_name.clone()),
        dirty: false,
        selected_statement_path: None,
        selected_operation_path: None,
        human_summary: report.human_summary.clone(),
        report,
    }
}

fn base_report(
    asset: &ProjectRuleAsset,
    status: RuleAuthoringStatus,
    diagnostics: Vec<RuleAuthoringDiagnostic>,
    changed_paths: Vec<String>,
) -> RuleAuthoringReport {
    let mut report = RuleAuthoringReport {
        schema_version: RULE_AUTHORING_REPORT_SCHEMA_VERSION.to_string(),
        status,
        asset_id: Some(asset.asset_id.clone()),
        rule_id: Some(asset.rule_id.clone()),
        ir_hash: Some(asset.ir_hash()),
        human_summary: human_summary_for_asset(asset),
        diagnostics,
        changed_paths,
        next_actions: Vec::new(),
        generated_rust_source: RuleAuthoringStageEvidence {
            status: RuleAuthoringStageStatus::NotRequested,
            path: None,
            artifact_id: None,
            summary: "generated_rust_source=not_requested".to_string(),
            skip_reason: None,
            next_action: Some("Run build_rule_artifact.".to_string()),
        },
        static_registry_source: RuleAuthoringStageEvidence {
            status: RuleAuthoringStageStatus::NotRequested,
            path: None,
            artifact_id: None,
            summary: "static_registry_source=not_requested".to_string(),
            skip_reason: None,
            next_action: Some("Run build_rule_artifact.".to_string()),
        },
        artifact_lifecycle: RuleAuthoringStageEvidence {
            status: RuleAuthoringStageStatus::NotRequested,
            path: None,
            artifact_id: None,
            summary: "artifact_lifecycle=not_requested".to_string(),
            skip_reason: None,
            next_action: Some("Run build_rule_artifact.".to_string()),
        },
        runtime_package_manifest: RuleAuthoringStageEvidence {
            status: RuleAuthoringStageStatus::NotRequested,
            path: None,
            artifact_id: None,
            summary: "runtime_package_manifest=not_requested".to_string(),
            skip_reason: None,
            next_action: Some("Run export after rule build.".to_string()),
        },
        cargo_build: RuleAuthoringStageEvidence::skipped(
            "skipped_by_v1",
            "Run project export or CI build for full cargo/player validation.",
        ),
    };
    report.next_actions = next_actions_for_report(&report);
    report
}

fn card_authoring_model_from_asset(
    project_root: Option<&Path>,
    selected_path: Option<&str>,
    rule_count: usize,
    document: &RuleAuthoringDocument,
    asset: Option<&ProjectRuleAsset>,
    selected_card_id: Option<String>,
    selected_graph_node_id: Option<String>,
) -> RuleCardAuthoringModel {
    let Some(asset) = asset else {
        let mut model = RuleCardAuthoringModel::empty();
        model.project_root = project_root.map(|path| path.display().to_string());
        model.selected_path = selected_path.map(str::to_string);
        model.rule_count = rule_count;
        return model;
    };

    let cards = rule_cards_from_asset(selected_path, asset, &document.report);
    let graph_preview = rule_graph_preview_from_asset(
        selected_path,
        asset,
        &document.report,
        selected_graph_node_id,
    );
    let report_summary = RuleCardAuthoringReport {
        schema_version: RULE_CARD_AUTHORING_REPORT_SCHEMA_VERSION.to_string(),
        status: document.report.status.clone(),
        asset_path: selected_path.map(str::to_string),
        rule_id: Some(asset.rule_id.clone()),
        ir_hash: Some(asset.ir_hash()),
        card_count: cards.len(),
        graph_node_count: graph_preview.nodes.len(),
        graph_edge_count: graph_preview.edges.len(),
        editable_card_count: cards
            .iter()
            .filter(|card| card.fields.iter().any(|field| field.editable))
            .count(),
        read_only_graph: graph_preview.read_only,
        changed_paths: document.report.changed_paths.clone(),
        diagnostics: document.report.diagnostics.clone(),
        next_actions: rule_card_next_actions(&document.report),
        source_mappings: graph_preview.source_mappings.clone(),
    };

    RuleCardAuthoringModel {
        project_root: project_root.map(|path| path.display().to_string()),
        selected_path: selected_path.map(str::to_string),
        rule_count,
        document: document.clone(),
        selected_card_id,
        cards,
        graph_preview,
        commands: rule_card_authoring_commands(true),
        report_summary,
    }
}

fn rule_cards_from_asset(
    asset_path: Option<&str>,
    asset: &ProjectRuleAsset,
    report: &RuleAuthoringReport,
) -> Vec<RuleCardModel> {
    let mut cards = Vec::new();
    let trigger_source = "canonicalIr.trigger";
    cards.push(RuleCardModel {
        card_id: "card:trigger".to_string(),
        kind: RuleCardKind::Trigger,
        asset_path: asset_path.map(str::to_string),
        rule_id: Some(asset.rule_id.clone()),
        source_path: trigger_source.to_string(),
        title: "Trigger".to_string(),
        summary: trigger_summary(&asset.canonical_ir.trigger),
        human_explanation: format!(
            "This rule starts when {}.",
            trigger_summary(&asset.canonical_ir.trigger)
        ),
        fields: trigger_fields(&asset.canonical_ir.trigger),
        allowed_commands: card_commands(true, false),
        diagnostics: diagnostics_for_source(report, trigger_source),
    });

    for (index, statement) in asset.canonical_ir.statements.iter().enumerate() {
        let source_path = format!("canonicalIr.statements[{index}]");
        cards.push(RuleCardModel {
            card_id: format!("card:statement:{index}"),
            kind: RuleCardKind::Statement,
            asset_path: asset_path.map(str::to_string),
            rule_id: Some(asset.rule_id.clone()),
            source_path: source_path.clone(),
            title: format!("Statement {}", index + 1),
            summary: statement_summary(statement),
            human_explanation: statement_human_explanation(statement),
            fields: vec![field_model(
                "statement.json",
                "Statement JSON",
                &source_path,
                RuleCardFieldValueKind::Json,
                json_preview(statement),
                true,
            )],
            allowed_commands: card_commands(true, true),
            diagnostics: diagnostics_for_source(report, &source_path),
        });
    }

    for (index, operation) in asset.canonical_ir.operations.iter().enumerate() {
        let source_path = format!("canonicalIr.operations[{index}]");
        cards.push(RuleCardModel {
            card_id: format!("card:operation:{index}"),
            kind: RuleCardKind::Operation,
            asset_path: asset_path.map(str::to_string),
            rule_id: Some(asset.rule_id.clone()),
            source_path: source_path.clone(),
            title: format!("Operation {}", index + 1),
            summary: operation_summary(operation),
            human_explanation: operation_human_explanation(operation),
            fields: operation_fields(operation, &source_path),
            allowed_commands: card_commands(true, true),
            diagnostics: diagnostics_for_source(report, &source_path),
        });
    }

    for (index, diagnostic) in report.diagnostics.iter().enumerate() {
        let source_path = diagnostic
            .path
            .as_deref()
            .map(canonical_rule_source_path)
            .unwrap_or_else(|| "canonicalIr".to_string());
        cards.push(RuleCardModel {
            card_id: format!("card:diagnostic:{index}"),
            kind: RuleCardKind::Diagnostic,
            asset_path: asset_path.map(str::to_string),
            rule_id: Some(asset.rule_id.clone()),
            source_path: source_path.clone(),
            title: format!("Diagnostic {}", diagnostic.code),
            summary: diagnostic.message.clone(),
            human_explanation: diagnostic.human_explanation.clone(),
            fields: vec![field_model(
                "diagnostic.message",
                "Message",
                &source_path,
                RuleCardFieldValueKind::String,
                diagnostic.message.clone(),
                false,
            )],
            allowed_commands: vec![RuleAuthoringCommand::new(
                "open_rule_diagnostics",
                "Open Diagnostics",
                true,
                None,
            )],
            diagnostics: vec![RuleCardDiagnosticRef {
                code: diagnostic.code.clone(),
                source_path: diagnostic.path.clone(),
                severity: diagnostic.severity.clone(),
            }],
        });
    }

    cards
}

fn rule_graph_preview_from_asset(
    asset_path: Option<&str>,
    asset: &ProjectRuleAsset,
    report: &RuleAuthoringReport,
    selected_node_id: Option<String>,
) -> RuleGraphPreviewModel {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let trigger_node = graph_node(
        "node:trigger",
        Some("card:trigger"),
        "canonicalIr.trigger",
        RuleGraphPreviewNodeKind::Trigger,
        trigger_summary(&asset.canonical_ir.trigger),
        report,
        selected_node_id.as_deref(),
    );
    nodes.push(trigger_node);

    for (index, statement) in asset.canonical_ir.statements.iter().enumerate() {
        let source_path = format!("canonicalIr.statements[{index}]");
        nodes.push(graph_node(
            &format!("node:statement:{index}"),
            Some(&format!("card:statement:{index}")),
            &source_path,
            RuleGraphPreviewNodeKind::Statement,
            statement_summary(statement),
            report,
            selected_node_id.as_deref(),
        ));
    }

    for (index, operation) in asset.canonical_ir.operations.iter().enumerate() {
        let source_path = format!("canonicalIr.operations[{index}]");
        nodes.push(graph_node(
            &format!("node:operation:{index}"),
            Some(&format!("card:operation:{index}")),
            &source_path,
            RuleGraphPreviewNodeKind::Operation,
            operation_summary(operation),
            report,
            selected_node_id.as_deref(),
        ));
    }

    let statement_count = asset.canonical_ir.statements.len();
    let operation_count = asset.canonical_ir.operations.len();
    if statement_count > 0 {
        edges.push(graph_edge("node:trigger", "node:statement:0", "exec"));
        for index in 1..statement_count {
            edges.push(graph_edge(
                &format!("node:statement:{}", index - 1),
                &format!("node:statement:{index}"),
                "exec",
            ));
        }
        if operation_count > 0 {
            edges.push(graph_edge(
                &format!("node:statement:{}", statement_count - 1),
                "node:operation:0",
                "exec",
            ));
        }
    } else if operation_count > 0 {
        edges.push(graph_edge("node:trigger", "node:operation:0", "exec"));
    }
    for index in 1..operation_count {
        edges.push(graph_edge(
            &format!("node:operation:{}", index - 1),
            &format!("node:operation:{index}"),
            "exec",
        ));
    }

    for (index, diagnostic) in report.diagnostics.iter().enumerate() {
        let source_path = diagnostic
            .path
            .as_deref()
            .map(canonical_rule_source_path)
            .unwrap_or_else(|| "canonicalIr".to_string());
        let target_node_id = node_id_for_source_path(&source_path);
        let diagnostic_node_id = format!("node:diagnostic:{index}");
        nodes.push(RuleGraphPreviewNode {
            node_id: diagnostic_node_id.clone(),
            card_id: Some(format!("card:diagnostic:{index}")),
            source_path: source_path.clone(),
            kind: RuleGraphPreviewNodeKind::Diagnostic,
            label: diagnostic.code.clone(),
            status: match diagnostic.severity {
                RuleAuthoringDiagnosticSeverity::Error => RuleGraphPreviewNodeStatus::Error,
                RuleAuthoringDiagnosticSeverity::Warning => RuleGraphPreviewNodeStatus::Warning,
                RuleAuthoringDiagnosticSeverity::Info => RuleGraphPreviewNodeStatus::Normal,
            },
            diagnostic_refs: vec![diagnostic.code.clone()],
        });
        if let Some(target_node_id) = target_node_id {
            edges.push(RuleGraphPreviewEdge {
                edge_id: format!("edge:diagnostic:{index}"),
                from_node_id: diagnostic_node_id,
                to_node_id: target_node_id,
                kind: RuleGraphPreviewEdgeKind::DiagnosticTarget,
                label: "diagnostic".to_string(),
            });
        }
    }

    let source_mappings = nodes
        .iter()
        .map(|node| RuleCardSourceMapping {
            source_path: node.source_path.clone(),
            card_id: node.card_id.clone(),
            node_id: Some(node.node_id.clone()),
        })
        .collect::<Vec<_>>();
    let authoring_node_ids = nodes
        .iter()
        .filter(|node| node.kind != RuleGraphPreviewNodeKind::Diagnostic)
        .map(|node| node.node_id.clone())
        .collect::<Vec<_>>();

    RuleGraphPreviewModel {
        schema_version: RULE_GRAPH_PREVIEW_SCHEMA_VERSION.to_string(),
        asset_path: asset_path.map(str::to_string),
        rule_id: Some(asset.rule_id.clone()),
        ir_hash: Some(asset.ir_hash()),
        nodes,
        edges,
        groups: vec![RuleGraphPreviewGroup {
            group_id: format!("phase:{}", asset.canonical_ir.phase.as_str()),
            label: format!("Phase {}", asset.canonical_ir.phase.as_str()),
            node_ids: authoring_node_ids,
        }],
        selected_node_id,
        source_mappings,
        read_only: true,
    }
}

fn rule_card_authoring_commands(rule_loaded: bool) -> Vec<RuleAuthoringCommand> {
    vec![
        RuleAuthoringCommand::new(
            "select_rule_card",
            "Select Card",
            rule_loaded,
            (!rule_loaded).then(|| "Select a rule asset first.".to_string()),
        ),
        RuleAuthoringCommand::new(
            "set_rule_card_field",
            "Edit Card Field",
            rule_loaded,
            (!rule_loaded).then(|| "Select a rule asset first.".to_string()),
        ),
        RuleAuthoringCommand::new(
            "add_rule_card",
            "Add Card",
            rule_loaded,
            (!rule_loaded).then(|| "Select a rule asset first.".to_string()),
        ),
        RuleAuthoringCommand::new(
            "remove_rule_card",
            "Remove Card",
            rule_loaded,
            (!rule_loaded).then(|| "Select a rule asset first.".to_string()),
        ),
        RuleAuthoringCommand::new(
            "select_rule_graph_node",
            "Select Graph Node",
            rule_loaded,
            (!rule_loaded).then(|| "Select a rule asset first.".to_string()),
        ),
        RuleAuthoringCommand::new(
            "refresh_rule_graph_preview",
            "Refresh Graph Preview",
            rule_loaded,
            (!rule_loaded).then(|| "Select a rule asset first.".to_string()),
        ),
        RuleAuthoringCommand::new(
            "duplicate_rule_card",
            "Duplicate Card",
            false,
            Some(
                "DuplicateRuleCard is disabled in v1 until duplicate edit commands exist."
                    .to_string(),
            ),
        ),
        RuleAuthoringCommand::new(
            "move_rule_card",
            "Move Card",
            false,
            Some("MoveRuleCard is disabled in v1 until reorder edit commands exist.".to_string()),
        ),
    ]
}

fn card_commands(can_edit: bool, can_remove: bool) -> Vec<RuleAuthoringCommand> {
    vec![
        RuleAuthoringCommand::new("select_rule_card", "Select Card", true, None),
        RuleAuthoringCommand::new(
            "set_rule_card_field",
            "Edit Card Field",
            can_edit,
            (!can_edit).then(|| "This card is read-only.".to_string()),
        ),
        RuleAuthoringCommand::new(
            "remove_rule_card",
            "Remove Card",
            can_remove,
            (!can_remove).then(|| "This card cannot be removed in v1.".to_string()),
        ),
    ]
}

fn rule_card_next_actions(report: &RuleAuthoringReport) -> Vec<String> {
    let mut actions = report.next_actions.clone();
    actions.push("refresh_rule_graph_preview".to_string());
    if matches!(
        report.status,
        RuleAuthoringStatus::Ready | RuleAuthoringStatus::Dirty | RuleAuthoringStatus::Valid
    ) {
        actions.push("set_rule_card_field".to_string());
    }
    actions.sort();
    actions.dedup();
    actions
}

fn trigger_fields(trigger: &RuleTrigger) -> Vec<RuleCardFieldModel> {
    let mut fields = vec![field_model(
        "trigger.kind",
        "Trigger Kind",
        "canonicalIr.trigger.kind",
        RuleCardFieldValueKind::Enum,
        trigger_kind(trigger).to_string(),
        true,
    )];
    match trigger {
        RuleTrigger::Always => {}
        RuleTrigger::ActionPressed { action_id } => fields.push(field_model(
            "trigger.actionId",
            "Action Id",
            "canonicalIr.trigger.actionId",
            RuleCardFieldValueKind::String,
            action_id.clone(),
            true,
        )),
        RuleTrigger::EventReceived { event_type } => fields.push(field_model(
            "trigger.eventType",
            "Event Type",
            "canonicalIr.trigger.eventType",
            RuleCardFieldValueKind::String,
            event_type.clone(),
            true,
        )),
    }
    fields[0].enum_options = vec![
        "always".to_string(),
        "actionPressed".to_string(),
        "eventReceived".to_string(),
    ];
    fields
}

fn operation_fields(operation: &RuleOperation, source_path: &str) -> Vec<RuleCardFieldModel> {
    let mut fields = vec![field_model(
        "operation.json",
        "Operation JSON",
        source_path,
        RuleCardFieldValueKind::Json,
        json_preview(operation),
        true,
    )];
    match operation {
        RuleOperation::WriteComponentField {
            entity_id,
            component_type,
            field_path,
            ..
        } => {
            fields.push(field_model(
                "operation.entityId",
                "Entity Id",
                &format!("{source_path}.entityId"),
                RuleCardFieldValueKind::String,
                entity_id.clone(),
                true,
            ));
            fields.push(field_model(
                "operation.componentType",
                "Component Type",
                &format!("{source_path}.componentType"),
                RuleCardFieldValueKind::String,
                component_type.clone(),
                true,
            ));
            fields.push(field_model(
                "operation.fieldPath",
                "Field Path",
                &format!("{source_path}.fieldPath"),
                RuleCardFieldValueKind::String,
                field_path.clone(),
                true,
            ));
        }
        RuleOperation::InstantiatePrefab { prefab_ref, .. } => fields.push(field_model(
            "operation.prefabRef.id",
            "Prefab Id",
            &format!("{source_path}.prefabRef.id"),
            RuleCardFieldValueKind::AssetRef,
            prefab_ref.id.clone(),
            true,
        )),
        RuleOperation::SpawnEntity {
            entity_id,
            name,
            kind,
            ..
        } => {
            fields.push(field_model(
                "operation.entityId",
                "Entity Id",
                &format!("{source_path}.entityId"),
                RuleCardFieldValueKind::String,
                entity_id.clone(),
                true,
            ));
            fields.push(field_model(
                "operation.name",
                "Name",
                &format!("{source_path}.name"),
                RuleCardFieldValueKind::String,
                name.clone(),
                true,
            ));
            fields.push(field_model(
                "operation.kind",
                "Kind",
                &format!("{source_path}.kind"),
                RuleCardFieldValueKind::String,
                kind.clone(),
                true,
            ));
        }
        RuleOperation::DespawnEntity { entity_id } => fields.push(field_model(
            "operation.entityId",
            "Entity Id",
            &format!("{source_path}.entityId"),
            RuleCardFieldValueKind::String,
            entity_id.clone(),
            true,
        )),
        RuleOperation::DespawnPrefabInstance { instance_id } => fields.push(field_model(
            "operation.instanceId",
            "Instance Id",
            &format!("{source_path}.instanceId"),
            RuleCardFieldValueKind::Number,
            instance_id.to_string(),
            true,
        )),
        RuleOperation::EmitEvent { event_type, .. } => fields.push(field_model(
            "operation.eventType",
            "Event Type",
            &format!("{source_path}.eventType"),
            RuleCardFieldValueKind::String,
            event_type.clone(),
            true,
        )),
    }
    fields
}

fn field_model(
    field_id: &str,
    label: &str,
    field_path: &str,
    value_kind: RuleCardFieldValueKind,
    value_preview: String,
    editable: bool,
) -> RuleCardFieldModel {
    RuleCardFieldModel {
        field_id: field_id.to_string(),
        label: label.to_string(),
        field_path: field_path.to_string(),
        value_kind,
        value_preview,
        editable,
        enum_options: Vec::new(),
        asset_ref_options: Vec::new(),
        validation_state: RuleCardValidationState::Unknown,
    }
}

fn graph_node(
    node_id: &str,
    card_id: Option<&str>,
    source_path: &str,
    kind: RuleGraphPreviewNodeKind,
    label: String,
    report: &RuleAuthoringReport,
    selected_node_id: Option<&str>,
) -> RuleGraphPreviewNode {
    let diagnostic_refs = diagnostics_for_source(report, source_path)
        .into_iter()
        .map(|diagnostic| diagnostic.code)
        .collect::<Vec<_>>();
    let mut status = if diagnostic_refs.is_empty() {
        RuleGraphPreviewNodeStatus::Normal
    } else {
        RuleGraphPreviewNodeStatus::Error
    };
    if selected_node_id == Some(node_id) {
        status = RuleGraphPreviewNodeStatus::Selected;
    }
    RuleGraphPreviewNode {
        node_id: node_id.to_string(),
        card_id: card_id.map(str::to_string),
        source_path: source_path.to_string(),
        kind,
        label,
        status,
        diagnostic_refs,
    }
}

fn graph_edge(from_node_id: &str, to_node_id: &str, label: &str) -> RuleGraphPreviewEdge {
    RuleGraphPreviewEdge {
        edge_id: format!("edge:{from_node_id}->{to_node_id}"),
        from_node_id: from_node_id.to_string(),
        to_node_id: to_node_id.to_string(),
        kind: RuleGraphPreviewEdgeKind::ExecutionOrder,
        label: label.to_string(),
    }
}

fn diagnostics_for_source(
    report: &RuleAuthoringReport,
    source_path: &str,
) -> Vec<RuleCardDiagnosticRef> {
    report
        .diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic
                .path
                .as_deref()
                .map(canonical_rule_source_path)
                .is_some_and(|path| path.starts_with(source_path))
        })
        .map(|diagnostic| RuleCardDiagnosticRef {
            code: diagnostic.code.clone(),
            source_path: diagnostic.path.clone(),
            severity: diagnostic.severity.clone(),
        })
        .collect()
}

fn canonical_rule_source_path(path: &str) -> String {
    if path.starts_with("canonicalIr.") {
        return path.to_string();
    }
    if path == "trigger" || path.starts_with("trigger.") {
        return format!("canonicalIr.{path}");
    }
    if path.starts_with("statements[") || path.starts_with("operations[") {
        return format!("canonicalIr.{path}");
    }
    path.to_string()
}

fn node_id_for_source_path(source_path: &str) -> Option<String> {
    if source_path.starts_with("canonicalIr.trigger") {
        return Some("node:trigger".to_string());
    }
    if let Some(index) = indexed_source_path(source_path, "canonicalIr.statements[") {
        return Some(format!("node:statement:{index}"));
    }
    if let Some(index) = indexed_source_path(source_path, "canonicalIr.operations[") {
        return Some(format!("node:operation:{index}"));
    }
    None
}

fn indexed_source_path(source_path: &str, prefix: &str) -> Option<usize> {
    let rest = source_path.strip_prefix(prefix)?;
    let end = rest.find(']')?;
    rest[..end].parse::<usize>().ok()
}

fn trigger_kind(trigger: &RuleTrigger) -> &'static str {
    match trigger {
        RuleTrigger::Always => "always",
        RuleTrigger::ActionPressed { .. } => "actionPressed",
        RuleTrigger::EventReceived { .. } => "eventReceived",
    }
}

fn trigger_summary(trigger: &RuleTrigger) -> String {
    match trigger {
        RuleTrigger::Always => "always".to_string(),
        RuleTrigger::ActionPressed { action_id } => format!("action pressed: {action_id}"),
        RuleTrigger::EventReceived { event_type } => format!("event received: {event_type}"),
    }
}

fn statement_summary(statement: &RuleStatement) -> String {
    match statement {
        RuleStatement::Operation { operation } => {
            format!("operation: {}", operation_summary(operation))
        }
        RuleStatement::When { condition, .. } => {
            format!("when {}", condition_summary(condition))
        }
        RuleStatement::ForEachQuery { query, .. } => {
            format!("for each entity with [{}]", query.all.join(", "))
        }
    }
}

fn statement_human_explanation(statement: &RuleStatement) -> String {
    match statement {
        RuleStatement::Operation { operation } => operation_human_explanation(operation),
        RuleStatement::When {
            condition,
            statements,
        } => format!(
            "Runs {} child statement(s) when {}.",
            statements.len(),
            condition_summary(condition)
        ),
        RuleStatement::ForEachQuery { query, statements } => format!(
            "Runs {} child statement(s) for entities matching all=[{}].",
            statements.len(),
            query.all.join(", ")
        ),
    }
}

fn condition_summary(condition: &engine_runtime::rule_ir::RuleCondition) -> String {
    match condition {
        engine_runtime::rule_ir::RuleCondition::Always => "always".to_string(),
        engine_runtime::rule_ir::RuleCondition::ActionPressed { action_id } => {
            format!("action pressed: {action_id}")
        }
        engine_runtime::rule_ir::RuleCondition::EventReceived { event_type } => {
            format!("event received: {event_type}")
        }
    }
}

fn operation_summary(operation: &RuleOperation) -> String {
    match operation {
        RuleOperation::WriteComponentField {
            component_type,
            field_path,
            ..
        } => format!("write {component_type}.{field_path}"),
        RuleOperation::SpawnEntity { name, kind, .. } => format!("spawn {kind} entity {name}"),
        RuleOperation::InstantiatePrefab { prefab_ref, .. } => {
            format!("instantiate prefab {}", prefab_ref.id)
        }
        RuleOperation::DespawnEntity { entity_id } => format!("despawn entity {entity_id}"),
        RuleOperation::DespawnPrefabInstance { instance_id } => {
            format!("despawn prefab instance {instance_id}")
        }
        RuleOperation::EmitEvent { event_type, .. } => format!("emit event {event_type}"),
    }
}

fn operation_human_explanation(operation: &RuleOperation) -> String {
    match operation {
        RuleOperation::WriteComponentField {
            entity_id,
            component_type,
            field_path,
            ..
        } => format!("Writes field {component_type}.{field_path} on entity {entity_id}."),
        RuleOperation::SpawnEntity {
            entity_id, name, ..
        } => {
            format!("Spawns entity {entity_id} named {name}.")
        }
        RuleOperation::InstantiatePrefab { prefab_ref, .. } => {
            format!("Instantiates prefab asset {}.", prefab_ref.id)
        }
        RuleOperation::DespawnEntity { entity_id } => format!("Despawns entity {entity_id}."),
        RuleOperation::DespawnPrefabInstance { instance_id } => {
            format!("Despawns prefab instance {instance_id}.")
        }
        RuleOperation::EmitEvent { event_type, .. } => format!("Emits event {event_type}."),
    }
}

fn json_preview<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "<unserializable>".to_string())
}

fn diagnostic_from_ir(
    asset: &ProjectRuleAsset,
    diagnostic: &RuleIrDiagnostic,
) -> RuleAuthoringDiagnostic {
    diagnostic_from_parts(
        asset,
        &diagnostic.code,
        &diagnostic.message,
        diagnostic.path.clone(),
        diagnostic.suggestion.clone(),
    )
}

fn diagnostic_from_compile(
    asset: &ProjectRuleAsset,
    diagnostic: &RuleCompileDiagnostic,
) -> RuleAuthoringDiagnostic {
    diagnostic_from_parts(
        asset,
        &diagnostic.code,
        &diagnostic.message,
        diagnostic.path.clone(),
        diagnostic.suggestion.clone(),
    )
}

fn diagnostic_from_parts(
    asset: &ProjectRuleAsset,
    code: &str,
    message: &str,
    path: Option<String>,
    suggestion: Option<String>,
) -> RuleAuthoringDiagnostic {
    let (human_explanation, suggested_fix) = explain_rule_diagnostic(
        &asset.display_name,
        code,
        path.as_deref(),
        message,
        suggestion.as_deref(),
    );
    RuleAuthoringDiagnostic {
        severity: RuleAuthoringDiagnosticSeverity::Error,
        code: code.to_string(),
        path,
        message: message.to_string(),
        human_explanation,
        suggested_fix,
    }
}

fn human_summary_for_asset(asset: &ProjectRuleAsset) -> String {
    let trigger = match &asset.canonical_ir.trigger {
        RuleTrigger::Always => "always runs".to_string(),
        RuleTrigger::ActionPressed { action_id } => {
            format!("runs when input action {action_id} is pressed")
        }
        RuleTrigger::EventReceived { event_type } => {
            format!("runs when event {event_type} is received")
        }
    };
    let action_count = if asset.canonical_ir.statements.is_empty() {
        asset.canonical_ir.operations.len()
    } else {
        asset.canonical_ir.statements.len()
    };
    format!(
        "Rule {}: {} and contains {} authoring step(s).",
        asset.display_name, trigger, action_count
    )
}

fn next_actions_for_report(report: &RuleAuthoringReport) -> Vec<String> {
    let mut actions = Vec::new();
    if !report.diagnostics.is_empty() {
        actions.push("fix_rule_diagnostics".to_string());
    }
    if matches!(report.status, RuleAuthoringStatus::Valid) {
        actions.push("build_rule_artifact".to_string());
    }
    if matches!(report.status, RuleAuthoringStatus::Built) {
        actions.push("export_runtime_package".to_string());
    }
    if actions.is_empty() {
        actions.push("validate_rule_asset".to_string());
    }
    actions
}

fn rule_authoring_commands(project_open: bool, rule_loaded: bool) -> Vec<RuleAuthoringCommand> {
    vec![
        RuleAuthoringCommand::new(
            "create_rule_asset",
            "Create Rule",
            project_open,
            (!project_open).then(|| "Open a project first.".to_string()),
        ),
        RuleAuthoringCommand::new(
            "open_rule_asset",
            "Open",
            rule_loaded,
            (!rule_loaded).then(|| "Select a rule asset first.".to_string()),
        ),
        RuleAuthoringCommand::new(
            "validate_rule_asset",
            "Validate",
            rule_loaded,
            (!rule_loaded).then(|| "Select a rule asset first.".to_string()),
        ),
        RuleAuthoringCommand::new(
            "build_rule_artifact",
            "Build",
            rule_loaded,
            (!rule_loaded).then(|| "Select a rule asset first.".to_string()),
        ),
    ]
}

fn rule_asset_id_from_path_or_rule(relative_path: &str, rule_id: &str) -> String {
    let path_id = Path::new(relative_path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(rule_id)
        .replace([' ', '-'], "_");
    format!("asset.rule.{path_id}")
}

fn is_rule_asset_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(is_rule_asset_file_name)
}

fn is_rule_asset_file_name(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    lower.ends_with(".rule.json") || lower.ends_with(".rules.json")
}

fn normalize_project_relative_path(path: &str) -> PathBuf {
    PathBuf::from(path.replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_runtime::rule_ir::{RuleOperation, RuleRuntimeValue};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn rule_authoring_create_validate_and_builds_report() {
        let root = temp_project();
        let mut asset = RuleAuthoringService::create_asset(
            &root,
            "Rules/fire.rule.json",
            "project.rule.fire",
            "Fire Projectile",
        )
        .unwrap();
        RuleAuthoringService::apply(
            &mut asset,
            RuleAuthoringEditCommand::SetTrigger(RuleTrigger::ActionPressed {
                action_id: "fire".to_string(),
            }),
            None,
        )
        .unwrap();
        RuleAuthoringService::apply(
            &mut asset,
            RuleAuthoringEditCommand::AddOperation(RuleOperation::InstantiatePrefab {
                prefab_ref: engine_runtime::runtime_package::RuntimeAssetRef {
                    id: "asset.prefab.projectile".to_string(),
                    asset_type: "prefab".to_string(),
                    guid: None,
                    sub_asset: None,
                },
                parent_entity: None,
                target_scene_instance: None,
            }),
            None,
        )
        .unwrap();
        RuleAuthoringService::save(&root, "Rules/fire.rule.json", &asset).unwrap();

        let report = RuleAuthoringService::build(&root, "Rules/fire.rule.json").unwrap();

        assert_eq!(report.status, RuleAuthoringStatus::Built);
        assert!(report.human_summary.contains("Fire Projectile"));
        assert_eq!(
            report.generated_rust_source.status,
            RuleAuthoringStageStatus::Produced
        );
        assert_eq!(
            report.cargo_build.status,
            RuleAuthoringStageStatus::SkippedByV1
        );
    }

    #[test]
    fn rule_authoring_rejects_expected_hash_mismatch() {
        let mut asset = ProjectRuleAsset::new(
            "asset.rule.test",
            "Test",
            ProjectRuleAssetSourceKind::UserAuthored,
            ProjectRuleIr::new("project.rule.test", ProjectRulePhase::Update),
        );

        let result = RuleAuthoringService::apply(
            &mut asset,
            RuleAuthoringEditCommand::AddOperation(RuleOperation::EmitEvent {
                event_type: "project.event".to_string(),
                payload: None,
            }),
            Some("wrong-hash"),
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("hash mismatch"));
    }

    #[test]
    fn rule_authoring_explains_diagnostics_with_templates() {
        let mut asset = ProjectRuleAsset::new(
            "asset.rule.bad",
            "Bad Rule",
            ProjectRuleAssetSourceKind::UserAuthored,
            ProjectRuleIr::new("project.rule.bad", ProjectRulePhase::Update),
        );
        asset
            .canonical_ir
            .operations
            .push(RuleOperation::WriteComponentField {
                entity_id: "entity-a".to_string(),
                component_type: "Transform".to_string(),
                field_path: "items[0].count".to_string(),
                value: RuleRuntimeValue::I64 { value: 1 },
            });

        let report = RuleAuthoringService::validate(&asset);

        assert_eq!(report.status, RuleAuthoringStatus::Invalid);
        let diagnostic = report
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "InvalidFieldPath")
            .expect("InvalidFieldPath diagnostic");
        assert!(diagnostic.human_explanation.contains("Bad Rule"));
        assert!(diagnostic.human_explanation.contains("invalid field path"));
        assert!(diagnostic.suggested_fix.is_some());

        let (fallback, _) =
            explain_rule_diagnostic("Rule A", "UnknownCode", Some("path"), "message", None);
        assert!(fallback.contains("UnknownCode"));
    }

    #[test]
    fn rule_authoring_model_scans_rule_assets_without_manifest_as_user_asset() {
        let root = temp_project();
        RuleAuthoringService::create_asset(
            &root,
            "Rules/move.rule.json",
            "project.rule.move",
            "Move",
        )
        .unwrap();
        fs::write(root.join("Rules").join("rule-manifest.json"), "{}").unwrap();

        let model = RuleAuthoringService::build_model(Some(&root), None);

        assert_eq!(model.rule_count, 1);
        assert_eq!(model.selected_path.as_deref(), Some("Rules/move.rule.json"));
        assert_eq!(model.document.rule_id.as_deref(), Some("project.rule.move"));
    }

    #[test]
    fn project_rule_manifest_build_is_deterministic_and_rejects_noncanonical_path() {
        let root = temp_project();
        RuleAuthoringService::create_asset(
            &root,
            "Rules/zeta.rule.json",
            "project.rule.zeta",
            "Zeta",
        )
        .unwrap();
        RuleAuthoringService::create_asset(
            &root,
            "Rules/alpha.rule.json",
            "project.rule.alpha",
            "Alpha",
        )
        .unwrap();

        let first = RuleAuthoringService::build_project_manifest(&root, "Rules/rule-manifest.json")
            .unwrap();
        let first_bytes = fs::read(root.join("Rules/rule-manifest.json")).unwrap();
        let second =
            RuleAuthoringService::build_project_manifest(&root, "Rules/rule-manifest.json")
                .unwrap();
        let second_bytes = fs::read(root.join("Rules/rule-manifest.json")).unwrap();

        assert_eq!(first_bytes, second_bytes);
        assert_eq!(first.rules.len(), second.rules.len());
        assert_eq!(first.rules[0].rule_id, second.rules[0].rule_id);
        assert_eq!(first.rules[1].rule_id, second.rules[1].rule_id);
        assert_eq!(first.rules[0].rule_id, "project.rule.alpha");
        assert_eq!(first.rules[1].rule_id, "project.rule.zeta");
        assert!(RuleAuthoringService::build_project_manifest(&root, "Rules/other.json").is_err());
    }

    #[test]
    fn rule_card_authoring_model_derives_cards_and_read_only_graph_preview() {
        let root = temp_project();
        let mut asset = RuleAuthoringService::create_asset(
            &root,
            "Rules/fire.rule.json",
            "project.rule.fire",
            "Fire",
        )
        .unwrap();
        RuleAuthoringService::apply(
            &mut asset,
            RuleAuthoringEditCommand::SetTrigger(RuleTrigger::ActionPressed {
                action_id: "action.fire".to_string(),
            }),
            None,
        )
        .unwrap();
        RuleAuthoringService::apply(
            &mut asset,
            RuleAuthoringEditCommand::AddOperation(RuleOperation::EmitEvent {
                event_type: "project.fire".to_string(),
                payload: None,
            }),
            None,
        )
        .unwrap();
        RuleAuthoringService::save(&root, "Rules/fire.rule.json", &asset).unwrap();

        let model = RuleAuthoringService::build_model_with_selection(
            Some(&root),
            Some("Rules/fire.rule.json".to_string()),
            Some("card:operation:0".to_string()),
            Some("node:operation:0".to_string()),
        );

        assert_eq!(model.card_authoring.cards.len(), 2);
        assert!(model.card_authoring.cards.iter().any(
            |card| card.card_id == "card:trigger" && card.source_path == "canonicalIr.trigger"
        ));
        assert!(model
            .card_authoring
            .cards
            .iter()
            .any(|card| card.card_id == "card:operation:0"
                && card.source_path == "canonicalIr.operations[0]"));
        assert!(model.card_authoring.graph_preview.read_only);
        assert!(model
            .card_authoring
            .graph_preview
            .nodes
            .iter()
            .any(|node| node.node_id == "node:operation:0"
                && node.status == RuleGraphPreviewNodeStatus::Selected));
        assert!(model
            .card_authoring
            .graph_preview
            .edges
            .iter()
            .any(
                |edge| edge.from_node_id == "node:trigger" && edge.to_node_id == "node:operation:0"
            ));
        assert!(model
            .card_authoring
            .commands
            .iter()
            .any(|command| command.command_id == "duplicate_rule_card" && !command.enabled));
    }

    #[test]
    fn rule_graph_preview_is_derived_read_only_and_keeps_source_mapping() {
        let root = temp_project();
        let mut asset = RuleAuthoringService::create_asset(
            &root,
            "Rules/event.rule.json",
            "project.rule.event",
            "Event",
        )
        .unwrap();
        RuleAuthoringService::apply(
            &mut asset,
            RuleAuthoringEditCommand::AddOperation(RuleOperation::EmitEvent {
                event_type: "project.event".to_string(),
                payload: None,
            }),
            None,
        )
        .unwrap();
        RuleAuthoringService::save(&root, "Rules/event.rule.json", &asset).unwrap();

        let model = RuleAuthoringService::build_model_with_selection(
            Some(&root),
            Some("Rules/event.rule.json".to_string()),
            None,
            Some("node:trigger".to_string()),
        );
        let preview = &model.card_authoring.graph_preview;

        assert!(preview.read_only);
        assert_eq!(preview.schema_version, RULE_GRAPH_PREVIEW_SCHEMA_VERSION);
        assert_eq!(preview.selected_node_id.as_deref(), Some("node:trigger"));
        assert!(preview.source_mappings.iter().any(|mapping| {
            mapping.source_path == "canonicalIr.operations[0]"
                && mapping.card_id.as_deref() == Some("card:operation:0")
                && mapping.node_id.as_deref() == Some("node:operation:0")
        }));
    }

    fn temp_project() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("rule-authoring-{stamp}"));
        fs::create_dir_all(&root).unwrap();
        root
    }
}
