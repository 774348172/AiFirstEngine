use crate::canonical_digest::{CanonicalDigestError, ConsistencyDigest};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

pub const PROJECT_OBSERVATION_CONTRACT_SCHEMA_VERSION: &str = "project-observation-contract.v1";
pub const PROJECT_OBSERVATION_CONTRACT_DIGEST_KIND: &str = "project-observation-contract";
pub const MAX_PROJECT_OBSERVATIONS: usize = 64;
pub const MAX_PROJECT_OBSERVATION_PATH_BYTES: usize = 128;
pub const MAX_PROJECT_OBSERVATION_DESCRIPTION_BYTES: usize = 256;
pub const MAX_PROJECT_OBSERVATION_ALLOWED_VALUES: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectObservationType {
    Bool,
    Integer,
    Number,
    String,
}

impl ProjectObservationType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::Integer => "integer",
            Self::Number => "number",
            Self::String => "string",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ProjectObservationValue {
    Bool(bool),
    Integer(i64),
    Number(f64),
    String(String),
}

impl ProjectObservationValue {
    pub fn value_type(&self) -> ProjectObservationType {
        match self {
            Self::Bool(_) => ProjectObservationType::Bool,
            Self::Integer(_) => ProjectObservationType::Integer,
            Self::Number(_) => ProjectObservationType::Number,
            Self::String(_) => ProjectObservationType::String,
        }
    }

    pub fn is_valid_scalar(&self) -> bool {
        !matches!(self, Self::Number(value) if !value.is_finite())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectObservationEntry {
    pub path: String,
    #[serde(rename = "type")]
    pub value_type: ProjectObservationType,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_values: Option<Vec<ProjectObservationValue>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectObservationContract {
    pub schema_version: String,
    pub contract_id: String,
    pub observations: Vec<ProjectObservationEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CookedProjectObservationContract {
    pub schema_version: String,
    pub contract_id: String,
    pub contract_digest: String,
    pub observations: Vec<ProjectObservationEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeObservationSnapshot {
    pub schema_version: String,
    pub runtime_frame: u64,
    pub session_id: String,
    pub contract_id: String,
    pub contract_digest: String,
    pub declared_types: BTreeMap<String, ProjectObservationType>,
    pub values: BTreeMap<String, ProjectObservationValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeObservationDiagnostic {
    pub code: String,
    pub path: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum ProjectRuntimeObservationState {
    NotProducedYet {
        session_id: String,
        contract_id: String,
        contract_digest: String,
        declared_types: BTreeMap<String, ProjectObservationType>,
    },
    Published {
        snapshot: ProjectRuntimeObservationSnapshot,
    },
    ContractViolated {
        runtime_frame: u64,
        session_id: String,
        contract_id: String,
        contract_digest: String,
        declared_types: BTreeMap<String, ProjectObservationType>,
        diagnostics: Vec<ProjectRuntimeObservationDiagnostic>,
    },
}

impl ProjectRuntimeObservationState {
    pub fn not_produced_yet(
        session_id: impl Into<String>,
        contract: &CookedProjectObservationContract,
    ) -> Self {
        Self::NotProducedYet {
            session_id: session_id.into(),
            contract_id: contract.contract_id.clone(),
            contract_digest: contract.contract_digest.clone(),
            declared_types: declared_types(contract),
        }
    }

    pub fn session_id(&self) -> &str {
        match self {
            Self::NotProducedYet { session_id, .. } | Self::ContractViolated { session_id, .. } => {
                session_id
            }
            Self::Published { snapshot } => &snapshot.session_id,
        }
    }

    pub fn contract_digest(&self) -> &str {
        match self {
            Self::NotProducedYet {
                contract_digest, ..
            }
            | Self::ContractViolated {
                contract_digest, ..
            } => contract_digest,
            Self::Published { snapshot } => &snapshot.contract_digest,
        }
    }

    pub fn runtime_frame(&self) -> Option<u64> {
        match self {
            Self::NotProducedYet { .. } => None,
            Self::Published { snapshot } => Some(snapshot.runtime_frame),
            Self::ContractViolated { runtime_frame, .. } => Some(*runtime_frame),
        }
    }

    pub fn contract_id(&self) -> &str {
        match self {
            Self::NotProducedYet { contract_id, .. }
            | Self::ContractViolated { contract_id, .. } => contract_id,
            Self::Published { snapshot } => &snapshot.contract_id,
        }
    }

    pub fn declared_types(&self) -> &BTreeMap<String, ProjectObservationType> {
        match self {
            Self::NotProducedYet { declared_types, .. }
            | Self::ContractViolated { declared_types, .. } => declared_types,
            Self::Published { snapshot } => &snapshot.declared_types,
        }
    }

    pub fn actual_value(&self, path: &str) -> Option<&ProjectObservationValue> {
        match self {
            Self::Published { snapshot } => snapshot.values.get(path),
            Self::NotProducedYet { .. } | Self::ContractViolated { .. } => None,
        }
    }
}

fn declared_types(
    contract: &CookedProjectObservationContract,
) -> BTreeMap<String, ProjectObservationType> {
    contract
        .observations
        .iter()
        .map(|entry| (entry.path.clone(), entry.value_type))
        .collect()
}

pub fn validate_project_observation_values(
    contract: &CookedProjectObservationContract,
    runtime_frame: u64,
    session_id: &str,
    values: BTreeMap<String, ProjectObservationValue>,
) -> ProjectRuntimeObservationState {
    let mut diagnostics = Vec::new();
    let declarations = contract
        .observations
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect::<BTreeMap<_, _>>();

    for (path, value) in &values {
        let Some(entry) = declarations.get(path.as_str()) else {
            diagnostics.push(ProjectRuntimeObservationDiagnostic {
                code: "project_observation.value_undeclared".to_string(),
                path: Some(path.clone()),
                message: format!("Runtime session produced undeclared observation path {path}."),
            });
            continue;
        };
        if !value.is_valid_scalar() || value.value_type() != entry.value_type {
            diagnostics.push(ProjectRuntimeObservationDiagnostic {
                code: "project_observation.value_type_mismatch".to_string(),
                path: Some(path.clone()),
                message: format!(
                    "Observation path {path} must produce a finite {} value.",
                    entry.value_type.as_str()
                ),
            });
            continue;
        }
        if entry
            .allowed_values
            .as_ref()
            .is_some_and(|allowed| !allowed.contains(value))
        {
            diagnostics.push(ProjectRuntimeObservationDiagnostic {
                code: "project_observation.value_not_allowed".to_string(),
                path: Some(path.clone()),
                message: format!("Observation path {path} produced a value outside allowedValues."),
            });
        }
    }
    for path in declarations.keys() {
        if !values.contains_key(*path) {
            diagnostics.push(ProjectRuntimeObservationDiagnostic {
                code: "project_observation.value_missing".to_string(),
                path: Some((*path).to_string()),
                message: format!("Runtime session did not produce observation path {path}."),
            });
        }
    }

    if diagnostics.is_empty() {
        ProjectRuntimeObservationState::Published {
            snapshot: ProjectRuntimeObservationSnapshot {
                schema_version: "project-runtime-observation-snapshot.v1".to_string(),
                runtime_frame,
                session_id: session_id.to_string(),
                contract_id: contract.contract_id.clone(),
                contract_digest: contract.contract_digest.clone(),
                declared_types: declared_types(contract),
                values,
            },
        }
    } else {
        ProjectRuntimeObservationState::ContractViolated {
            runtime_frame,
            session_id: session_id.to_string(),
            contract_id: contract.contract_id.clone(),
            contract_digest: contract.contract_digest.clone(),
            declared_types: declared_types(contract),
            diagnostics,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectObservationContractDiagnostic {
    pub code: &'static str,
    pub contract_id: Option<String>,
    pub path: Option<String>,
    pub message: String,
    pub next_action: &'static str,
}

impl ProjectObservationContractDiagnostic {
    fn new(
        code: &'static str,
        contract_id: Option<&str>,
        path: Option<&str>,
        message: impl Into<String>,
        next_action: &'static str,
    ) -> Self {
        Self {
            code,
            contract_id: contract_id.map(str::to_string),
            path: path.map(str::to_string),
            message: message.into(),
            next_action,
        }
    }
}

impl ProjectObservationContract {
    pub fn validate(&self) -> Result<(), Vec<ProjectObservationContractDiagnostic>> {
        let mut diagnostics = Vec::new();
        if self.schema_version != PROJECT_OBSERVATION_CONTRACT_SCHEMA_VERSION {
            diagnostics.push(ProjectObservationContractDiagnostic::new(
                "project_observation.contract_schema_unsupported",
                Some(&self.contract_id),
                None,
                format!(
                    "Observation contract schema must be {}, got {}.",
                    PROJECT_OBSERVATION_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
                "Set schemaVersion to project-observation-contract.v1.",
            ));
        }
        if !is_stable_dotted_id(&self.contract_id)
            || self.contract_id.len() > MAX_PROJECT_OBSERVATION_PATH_BYTES
        {
            diagnostics.push(ProjectObservationContractDiagnostic::new(
                "project_observation.contract_id_invalid",
                Some(&self.contract_id),
                None,
                "contractId must be a non-empty dotted stable id no longer than 128 bytes.",
                "Use dot-separated ASCII identifier segments for contractId.",
            ));
        }
        if self.observations.len() > MAX_PROJECT_OBSERVATIONS {
            diagnostics.push(ProjectObservationContractDiagnostic::new(
                "project_observation.contract_limit_exceeded",
                Some(&self.contract_id),
                None,
                format!(
                    "Observation contract declares {} paths; the v1 limit is {}.",
                    self.observations.len(),
                    MAX_PROJECT_OBSERVATIONS
                ),
                "Reduce the contract to stable milestone values only.",
            ));
        }

        let mut paths = HashSet::new();
        for entry in &self.observations {
            if entry.path.len() > MAX_PROJECT_OBSERVATION_PATH_BYTES
                || !is_stable_dotted_id(&entry.path)
            {
                diagnostics.push(ProjectObservationContractDiagnostic::new(
                    "project_observation.contract_path_invalid",
                    Some(&self.contract_id),
                    Some(&entry.path),
                    "Observation path must be a non-empty dotted stable id no longer than 128 bytes.",
                    "Use dot-separated ASCII identifier segments without empty segments.",
                ));
            }
            if !paths.insert(entry.path.as_str()) {
                diagnostics.push(ProjectObservationContractDiagnostic::new(
                    "project_observation.contract_path_duplicate",
                    Some(&self.contract_id),
                    Some(&entry.path),
                    format!(
                        "Observation path is declared more than once: {}.",
                        entry.path
                    ),
                    "Keep exactly one declaration for each public observation path.",
                ));
            }
            if entry.description.is_empty()
                || entry.description.len() > MAX_PROJECT_OBSERVATION_DESCRIPTION_BYTES
            {
                diagnostics.push(ProjectObservationContractDiagnostic::new(
                    "project_observation.contract_description_invalid",
                    Some(&self.contract_id),
                    Some(&entry.path),
                    "Observation description must contain 1 to 256 UTF-8 bytes.",
                    "Provide a concise description of the authoritative project value.",
                ));
            }
            if let Some(allowed_values) = &entry.allowed_values {
                if allowed_values.len() > MAX_PROJECT_OBSERVATION_ALLOWED_VALUES {
                    diagnostics.push(ProjectObservationContractDiagnostic::new(
                        "project_observation.contract_allowed_values_limit_exceeded",
                        Some(&self.contract_id),
                        Some(&entry.path),
                        format!(
                            "Observation path {} declares {} allowed values; the v1 limit is {}.",
                            entry.path,
                            allowed_values.len(),
                            MAX_PROJECT_OBSERVATION_ALLOWED_VALUES
                        ),
                        "Reduce allowedValues to the stable public value set.",
                    ));
                }
                for value in allowed_values {
                    if !value.is_valid_scalar() || value.value_type() != entry.value_type {
                        diagnostics.push(ProjectObservationContractDiagnostic::new(
                            "project_observation.contract_allowed_value_type_mismatch",
                            Some(&self.contract_id),
                            Some(&entry.path),
                            format!(
                                "allowedValues for {} must contain only finite {} values.",
                                entry.path,
                                entry.value_type.as_str()
                            ),
                            "Make every allowed value match the declared observation type.",
                        ));
                    }
                }
            }
        }

        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        }
    }

    pub fn contract_digest(&self) -> Result<String, CanonicalDigestError> {
        Ok(ConsistencyDigest::sha256(
            PROJECT_OBSERVATION_CONTRACT_DIGEST_KIND,
            PROJECT_OBSERVATION_CONTRACT_SCHEMA_VERSION,
            self,
        )?
        .prefixed_value())
    }

    pub fn cook(
        &self,
    ) -> Result<CookedProjectObservationContract, Vec<ProjectObservationContractDiagnostic>> {
        self.validate()?;
        let contract_digest = self.contract_digest().map_err(|error| {
            vec![ProjectObservationContractDiagnostic::new(
                "project_observation.contract_digest_failed",
                Some(&self.contract_id),
                None,
                format!("Failed to calculate observation contract digest: {error}"),
                "Remove values that cannot be canonically encoded.",
            )]
        })?;
        Ok(CookedProjectObservationContract {
            schema_version: self.schema_version.clone(),
            contract_id: self.contract_id.clone(),
            contract_digest,
            observations: self.observations.clone(),
        })
    }
}

impl CookedProjectObservationContract {
    pub fn validate(&self) -> Result<(), Vec<ProjectObservationContractDiagnostic>> {
        let source = ProjectObservationContract {
            schema_version: self.schema_version.clone(),
            contract_id: self.contract_id.clone(),
            observations: self.observations.clone(),
        };
        source.validate()?;
        let expected = source.contract_digest().map_err(|error| {
            vec![ProjectObservationContractDiagnostic::new(
                "project_observation.contract_digest_failed",
                Some(&self.contract_id),
                None,
                format!("Failed to calculate observation contract digest: {error}"),
                "Rebuild the RuntimePackage from a valid source contract.",
            )]
        })?;
        if self.contract_digest != expected {
            return Err(vec![ProjectObservationContractDiagnostic::new(
                "project_observation.contract_digest_mismatch",
                Some(&self.contract_id),
                None,
                format!(
                    "Cooked observation contract digest {} does not match {}.",
                    self.contract_digest, expected
                ),
                "Rebuild the RuntimePackage from the project source contract.",
            )]);
        }
        Ok(())
    }
}

fn is_stable_dotted_id(value: &str) -> bool {
    !value.is_empty()
        && value.split('.').all(|segment| {
            let mut chars = segment.chars();
            chars
                .next()
                .is_some_and(|character| character.is_ascii_alphabetic())
                && chars.all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
                })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_contract() -> ProjectObservationContract {
        ProjectObservationContract {
            schema_version: PROJECT_OBSERVATION_CONTRACT_SCHEMA_VERSION.to_string(),
            contract_id: "sample.runtime-observations".to_string(),
            observations: vec![
                ProjectObservationEntry {
                    path: "sample.phase".to_string(),
                    value_type: ProjectObservationType::String,
                    description: "Current authoritative phase".to_string(),
                    allowed_values: Some(vec![
                        ProjectObservationValue::String("ready".to_string()),
                        ProjectObservationValue::String("finished".to_string()),
                    ]),
                },
                ProjectObservationEntry {
                    path: "sample.round".to_string(),
                    value_type: ProjectObservationType::Integer,
                    description: "Current one-based round".to_string(),
                    allowed_values: None,
                },
            ],
        }
    }

    fn diagnostic_codes(contract: &ProjectObservationContract) -> Vec<&'static str> {
        contract
            .validate()
            .expect_err("contract should be rejected")
            .into_iter()
            .map(|diagnostic| diagnostic.code)
            .collect()
    }

    #[test]
    fn project_observation_contract_validates_and_cooks_typed_scalars() {
        let contract = valid_contract();
        contract.validate().unwrap();

        let cooked = contract.cook().unwrap();

        assert_eq!(cooked.contract_id, contract.contract_id);
        assert!(cooked.contract_digest.starts_with("sha256:"));
        cooked.validate().unwrap();
        let roundtrip: CookedProjectObservationContract =
            serde_json::from_value(serde_json::to_value(&cooked).unwrap()).unwrap();
        assert_eq!(roundtrip, cooked);
    }

    #[test]
    fn project_observation_contract_rejects_unknown_schema_and_duplicate_or_invalid_paths() {
        let mut contract = valid_contract();
        contract.schema_version = "project-observation-contract.v9".to_string();
        contract.observations[0].path = "sample..phase".to_string();
        contract.observations[1].path = "sample..phase".to_string();

        let codes = diagnostic_codes(&contract);

        assert!(codes.contains(&"project_observation.contract_schema_unsupported"));
        assert!(codes.contains(&"project_observation.contract_path_invalid"));
        assert!(codes.contains(&"project_observation.contract_path_duplicate"));
    }

    #[test]
    fn project_observation_contract_rejects_limits_and_allowed_value_type_mismatch() {
        let mut contract = valid_contract();
        contract.observations = (0..=MAX_PROJECT_OBSERVATIONS)
            .map(|index| ProjectObservationEntry {
                path: format!("sample.value{index}"),
                value_type: ProjectObservationType::Integer,
                description: "value".to_string(),
                allowed_values: (index == 0)
                    .then(|| vec![ProjectObservationValue::String("wrong".to_string())]),
            })
            .collect();

        let codes = diagnostic_codes(&contract);

        assert!(codes.contains(&"project_observation.contract_limit_exceeded"));
        assert!(codes.contains(&"project_observation.contract_allowed_value_type_mismatch"));
    }

    #[test]
    fn project_observation_cooked_contract_rejects_digest_drift() {
        let mut cooked = valid_contract().cook().unwrap();
        cooked.observations[0].description = "Changed after cooking".to_string();

        let diagnostics = cooked.validate().unwrap_err();

        assert_eq!(
            diagnostics[0].code,
            "project_observation.contract_digest_mismatch"
        );
    }
}
