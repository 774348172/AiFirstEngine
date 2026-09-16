use super::*;
use runtime_player_winit::semantic_outcome::{PlaytestScenario, MAX_PLAYTEST_SCENARIO_BYTES};

/// Retained test input, bound to the same immutable project source as the prepared package.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedPlaytestScenario {
    scenario: PlaytestScenario,
    source_path: String,
    source_digest: String,
    preparation_identity: String,
    lineage: ProjectArtifactLineage,
}

#[derive(Debug, Clone)]
pub struct ProjectPlaytestReport {
    delivery: DeliveryRef,
    scenario: PreparedPlaytestScenario,
    process: runtime_cli::SemanticPlaytestProcessReport,
    evidence: runtime_cli::SemanticPlaytestEvidenceRef,
}

impl ProjectPlaytestReport {
    pub fn delivery(&self) -> &DeliveryRef {
        &self.delivery
    }
    pub fn scenario(&self) -> &PreparedPlaytestScenario {
        &self.scenario
    }
    pub fn process(&self) -> &runtime_cli::SemanticPlaytestProcessReport {
        &self.process
    }
    pub fn evidence(&self) -> &runtime_cli::SemanticPlaytestEvidenceRef {
        &self.evidence
    }
}

impl GameProjectCompiler {
    pub fn playtest(
        &self,
        prepared: &PreparedRuntimePackage,
        build: BuildRequest,
        output_dir: PathBuf,
    ) -> Result<ProjectPlaytestReport, GameProjectCompilerError> {
        let scenario = prepared.load_playtest_scenario()?;
        let built = self.build(prepared, build)?;
        self.playtest_delivery(&built.delivery, &scenario, output_dir)
    }

    pub fn playtest_delivery(
        &self,
        delivery: &DeliveryRef,
        scenario: &PreparedPlaytestScenario,
        output_dir: PathBuf,
    ) -> Result<ProjectPlaytestReport, GameProjectCompilerError> {
        self.ensure_lineage(&delivery.lineage, GameProjectCompilerStage::Verify)?;
        if delivery.preparation_identity != scenario.preparation_identity
            || delivery.lineage != scenario.lineage
        {
            return Err(playtest_execution_error(
                "scenario_delivery_mismatch",
                "Scenario and delivery must originate from the same prepared input.",
            ));
        }
        if delivery.lineage.target_profile != TargetProfile::WindowsDev {
            return Err(playtest_execution_error(
                "delivery_target_unsupported",
                "Only Windows Dev delivery is qualified.",
            ));
        }
        delivery.ensure_manifest_identity()?;
        let package = delivery.package_dir.join("data/runtime_package");
        let player = delivery.package_dir.join("Game.exe");
        let manifest = runtime_cli::validate_desktop_dev_package(&delivery.package_dir).map_err(
            |diagnostic| {
                playtest_execution_error(
                    "delivery_changed",
                    format!(
                        "{}: {} ({})",
                        diagnostic.code,
                        diagnostic.message,
                        diagnostic.path.as_deref().unwrap_or_default(),
                    ),
                )
            },
        )?;
        if manifest.player_artifact_hash.as_deref() != Some(delivery.artifact_identity.as_str())
            || manifest.runtime_package_digest.as_deref()
                != Some(delivery.runtime_package_digest.as_str())
        {
            return Err(playtest_execution_error(
                "delivery_changed",
                "The specified delivery no longer matches its retained Player/package identity.",
            ));
        }
        let mut process = runtime_cli::run_bounded_semantic_playtest(
            runtime_cli::SemanticPlaytestProcessRequest {
                player_executable: player,
                runtime_package: package,
                scenario: scenario.scenario.clone(),
                output_dir: output_dir.clone(),
            },
        )
        .map_err(|e| playtest_execution_error("execution_failed", e))?;
        if process.player_digest != delivery.artifact_identity
            || process.package_digest != delivery.runtime_package_digest
        {
            process.outcome.technical =
                runtime_player_winit::semantic_outcome::OutcomeStatus::Failed;
        }
        process.outcome.delivery = process.outcome.overall(&scenario.scenario, false);
        process.overall = process.outcome.overall(&scenario.scenario, true);
        let result = output_dir.join("result.json");
        std::fs::write(
            &result,
            serde_json::to_vec_pretty(&process)
                .map_err(|e| playtest_execution_error("report_invalid", e.to_string()))?,
        )
        .map_err(|e| playtest_execution_error("report_write_failed", e.to_string()))?;
        let evidence = runtime_cli::retain_semantic_playtest_evidence(&result)
            .map_err(|e| playtest_execution_error("evidence_missing", e))?;
        Ok(ProjectPlaytestReport {
            delivery: delivery.clone(),
            scenario: scenario.clone(),
            process,
            evidence,
        })
    }

    pub fn observe_playtest(
        &self,
        report: &ProjectPlaytestReport,
    ) -> Result<runtime_cli::SemanticPlaytestProcessReport, GameProjectCompilerError> {
        self.ensure_lineage(&report.delivery.lineage, GameProjectCompilerStage::Verify)?;
        let process = runtime_cli::read_semantic_playtest_evidence(&report.evidence)
            .map_err(|e| playtest_execution_error("evidence_invalid", e))?;
        if process.player_digest != report.delivery.artifact_identity
            || process.package_digest != report.delivery.runtime_package_digest
        {
            return Err(playtest_execution_error(
                "evidence_delivery_mismatch",
                "Evidence is not from the specified delivery.",
            ));
        }
        Ok(process)
    }
}

fn playtest_execution_error(code: &str, message: impl Into<String>) -> GameProjectCompilerError {
    compiler_error(&format!("game_project_compiler.playtest_{code}"), message, GameProjectCompilerStage::Verify,
        "Inspect the specified scenario/delivery/evidence; no automatic rebuild or latest-run fallback was performed.")
}

impl PreparedPlaytestScenario {
    pub fn scenario(&self) -> &PlaytestScenario {
        &self.scenario
    }
    pub fn source_path(&self) -> &str {
        &self.source_path
    }
    pub fn source_digest(&self) -> &str {
        &self.source_digest
    }
    pub fn preparation_identity(&self) -> &str {
        &self.preparation_identity
    }
    pub fn lineage(&self) -> &ProjectArtifactLineage {
        &self.lineage
    }
}

impl PreparedRuntimePackage {
    pub fn load_playtest_scenario(
        &self,
    ) -> Result<PreparedPlaytestScenario, GameProjectCompilerError> {
        let manifest = self.source.project_manifest_value()?;
        let path = manifest
            .get("playtestScenario")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                scenario_error(
                    "playtest_reference_missing",
                    PROJECT_MANIFEST_PATH,
                    Some("/playtestScenario"),
                    "Declare the default playtestScenario source path in the project manifest.",
                )
            })?;
        let relative = crate::ProjectRelativePath::parse(path).map_err(|error| {
            scenario_error(
                "playtest_reference_invalid",
                PROJECT_MANIFEST_PATH,
                Some("/playtestScenario"),
                error.to_string(),
            )
        })?;
        let path = relative.as_str();
        let bytes = self.source.bytes(path).ok_or_else(|| scenario_error("playtest_source_missing", path, None, "The prepared SourceView does not contain this scenario; prepare a new snapshot after correcting the manifest reference."))?;
        if bytes.len() > MAX_PLAYTEST_SCENARIO_BYTES {
            return Err(scenario_error(
                "playtest_source_too_large",
                path,
                None,
                "Scenario JSON exceeds the 1 MiB input limit.",
            ));
        }
        let scenario: PlaytestScenario = serde_json::from_slice(bytes).map_err(|error| {
            let mut result = scenario_error("playtest_json_invalid", path, None, error.to_string());
            if let Some(location) = &mut result.source_location {
                location.line = Some(error.line() as u64);
                location.column = Some(error.column() as u64);
            }
            result
        })?;
        scenario
            .validate(
                self.runtime_package_build_input
                    .observation_contract
                    .as_ref(),
            )
            .map_err(|error| {
                scenario_error(&error.code, path, Some(&error.field_path), error.message)
            })?;
        if !matches!(
            self.lineage.target_profile,
            TargetProfile::WindowsDev | TargetProfile::WindowsRelease
        ) {
            return Err(scenario_error(
                "playtest_target_unsupported",
                path,
                Some("/target"),
                "Playtest scenarios currently target Windows only.",
            ));
        }
        if !self
            .runtime_package_build_input
            .scenes
            .iter()
            .any(|scene| scene.id == scenario.initial_scene_id)
        {
            return Err(scenario_error(
                "playtest_scene_missing",
                path,
                Some("/initialSceneId"),
                "Initial scene is not present in this prepared RuntimePackage.",
            ));
        }
        Ok(PreparedPlaytestScenario {
            scenario,
            source_path: path.to_string(),
            source_digest: format!("sha256:{:x}", Sha256::digest(bytes)),
            preparation_identity: self.preparation_identity.clone(),
            lineage: self.lineage.clone(),
        })
    }
}

fn scenario_error(
    code: &str,
    path: &str,
    field: Option<&str>,
    message: impl Into<String>,
) -> GameProjectCompilerError {
    let mut error = compiler_error(
        &format!("game_project_compiler.{code}"),
        message,
        GameProjectCompilerStage::Prepare,
        "Correct the indicated project scenario or manifest field and prepare a fresh snapshot.",
    );
    error.source_location = Some(SourceLocation {
        source_path: path.into(),
        field_path: field.map(str::to_string),
        line: None,
        column: None,
        generated: false,
    });
    error
}
