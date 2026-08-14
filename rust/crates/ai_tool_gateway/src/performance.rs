use serde::{Deserialize, Serialize};

pub const GATEWAY_PERFORMANCE_CONTRACT_REPORT_SCHEMA_VERSION: &str =
    "gateway-performance-contract-report.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayCacheState {
    Cold,
    Warm,
    NotApplicable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GatewayPerformanceStageSample {
    pub stage: String,
    pub total_ms: u64,
    pub ipc_wait_ms: u64,
    pub editor_queue_wait_ms: u64,
    pub worker_ms: u64,
    pub child_process_ms: u64,
    pub cache_state: GatewayCacheState,
}

impl GatewayPerformanceStageSample {
    pub fn in_process(
        stage: impl Into<String>,
        total_ms: u64,
        cache_state: GatewayCacheState,
    ) -> Self {
        Self {
            stage: stage.into(),
            total_ms,
            ipc_wait_ms: 0,
            editor_queue_wait_ms: 0,
            worker_ms: total_ms,
            child_process_ms: 0,
            cache_state,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let accounted = self
            .ipc_wait_ms
            .saturating_add(self.editor_queue_wait_ms)
            .saturating_add(self.worker_ms)
            .saturating_add(self.child_process_ms);
        if accounted > self.total_ms {
            return Err(format!(
                "Performance stage '{}' accounts for more time than its total.",
                self.stage
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GatewayPerformanceContractReport {
    pub schema_version: String,
    pub stages: Vec<GatewayPerformanceStageSample>,
    pub compile_triggered_by_handshake_or_search: bool,
    pub import_triggered_by_handshake_or_search: bool,
    pub rebuild_triggered_by_handshake_or_search: bool,
    pub total_preflight_ms: u64,
    pub competition_budget_ms: u64,
    pub remaining_budget_ms: u64,
}

impl GatewayPerformanceContractReport {
    pub fn new(stages: Vec<GatewayPerformanceStageSample>, competition_budget_ms: u64) -> Self {
        let total_preflight_ms = stages
            .iter()
            .fold(0_u64, |total, stage| total.saturating_add(stage.total_ms));
        Self {
            schema_version: GATEWAY_PERFORMANCE_CONTRACT_REPORT_SCHEMA_VERSION.to_string(),
            stages,
            compile_triggered_by_handshake_or_search: false,
            import_triggered_by_handshake_or_search: false,
            rebuild_triggered_by_handshake_or_search: false,
            total_preflight_ms,
            competition_budget_ms,
            remaining_budget_ms: competition_budget_ms.saturating_sub(total_preflight_ms),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != GATEWAY_PERFORMANCE_CONTRACT_REPORT_SCHEMA_VERSION {
            return Err("Gateway performance report schema is unsupported.".to_string());
        }
        for stage in &self.stages {
            stage.validate()?;
        }
        let computed_total = self
            .stages
            .iter()
            .fold(0_u64, |total, stage| total.saturating_add(stage.total_ms));
        if computed_total != self.total_preflight_ms
            || self.remaining_budget_ms != self.competition_budget_ms.saturating_sub(computed_total)
        {
            return Err("Gateway performance report totals are inconsistent.".to_string());
        }
        if self.compile_triggered_by_handshake_or_search
            || self.import_triggered_by_handshake_or_search
            || self.rebuild_triggered_by_handshake_or_search
        {
            return Err(
                "Handshake and search must not trigger compile, import, or rebuild.".to_string(),
            );
        }
        Ok(())
    }
}
