use crate::cargo_json::sha256_hex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const VALIDATION_CATALOG_SCHEMA_VERSION: &str = "construction-validation.catalog.v1";

const REGISTERED_PRODUCERS: &[&str] = &[
    "quality_gate.owner_tests",
    "quality_gate.crate_tests",
    "quality_gate.format",
    "quality_gate.verify",
    "quality_gate.local_ci",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValidationCatalog {
    pub schema_version: String,
    pub verifiers: Vec<VerifierCatalogEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VerifierCatalogEntry {
    pub id: String,
    pub owner_domains: Vec<String>,
    pub consumer_domains: Vec<String>,
    pub proves: Vec<String>,
    pub producer_id: String,
    pub environment: EnvironmentRequirement,
    pub subsumes: Vec<String>,
    pub cost_class: CostClass,
    pub default_timeout_seconds: u64,
    pub historical_duration_seconds: Option<u64>,
    pub cleanup_reserve_seconds: u64,
    pub external_effects: Vec<ExternalEffect>,
    pub required_authorization: Vec<AuthorizationRequirement>,
    pub consumed_identity_kinds: Vec<EvidenceIdentityKind>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnvironmentRequirement {
    pub platform: Option<String>,
    pub profile: Option<String>,
    #[serde(default)]
    pub required_features: Vec<String>,
    pub composition: Option<String>,
}

impl EnvironmentRequirement {
    pub fn matches(&self, environment: &EnvironmentIdentity) -> bool {
        self.platform
            .as_ref()
            .is_none_or(|value| value == &environment.platform)
            && self
                .profile
                .as_ref()
                .is_none_or(|value| value == &environment.profile)
            && self
                .composition
                .as_ref()
                .is_none_or(|value| value == &environment.composition)
            && self
                .required_features
                .iter()
                .all(|feature| environment.features.binary_search(feature).is_ok())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnvironmentIdentity {
    pub platform: String,
    pub profile: String,
    pub features: Vec<String>,
    pub composition: String,
}

impl EnvironmentIdentity {
    pub fn normalize(&mut self) {
        normalize_strings(&mut self.features);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostClass {
    Low,
    Medium,
    High,
    External,
}

impl CostClass {
    pub fn rank(self) -> u8 {
        match self {
            Self::Low => 0,
            Self::Medium => 1,
            Self::High => 2,
            Self::External => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalEffect {
    OwnedProcesses,
    OwnedArtifactRoot,
    LocalCi,
    RealEditorSession,
    RealOsWindowOrGpu,
    ProductionBinaryReplacement,
    InstalledBinaryMutation,
    RealConfigurationMutation,
    OneShotExternalAcceptance,
    OwnedRecursiveCleanup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationRequirement {
    LocalCi,
    RealEditor,
    ProductionReplacement,
    RealConfigurationMutation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceIdentityKind {
    ProductSource,
    VerificationHarness,
    GeneratedContractFixture,
    ProductionBinary,
    LaunchComposition,
    ExternalConfiguration,
    PlatformToolchain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogError {
    pub code: String,
    pub message: String,
}

pub trait ValidationCatalogSource {
    fn load(&self) -> Result<ValidationCatalog, CatalogError>;
}

#[derive(Debug, Clone)]
pub struct StaticValidationCatalogSource {
    catalog: ValidationCatalog,
}

impl StaticValidationCatalogSource {
    pub fn new(catalog: ValidationCatalog) -> Self {
        Self { catalog }
    }
}

impl ValidationCatalogSource for StaticValidationCatalogSource {
    fn load(&self) -> Result<ValidationCatalog, CatalogError> {
        validate_and_normalize_catalog(self.catalog.clone())
    }
}

pub fn parse_validation_catalog(source: &[u8]) -> Result<ValidationCatalog, CatalogError> {
    let catalog = serde_json::from_slice(source).map_err(|error| CatalogError {
        code: "construction_validation.catalog_invalid".to_string(),
        message: error.to_string(),
    })?;
    validate_and_normalize_catalog(catalog)
}

pub fn catalog_digest(catalog: &ValidationCatalog) -> Result<String, CatalogError> {
    let normalized = validate_and_normalize_catalog(catalog.clone())?;
    let bytes = serde_json::to_vec(&normalized).map_err(|error| CatalogError {
        code: "construction_validation.catalog_invalid".to_string(),
        message: error.to_string(),
    })?;
    Ok(format!("sha256:{}", sha256_hex(&bytes)))
}

fn validate_and_normalize_catalog(
    mut catalog: ValidationCatalog,
) -> Result<ValidationCatalog, CatalogError> {
    if catalog.schema_version != VALIDATION_CATALOG_SCHEMA_VERSION {
        return Err(catalog_error(
            "construction_validation.catalog_invalid",
            "unsupported validation catalog schema",
        ));
    }
    for verifier in &mut catalog.verifiers {
        normalize_strings(&mut verifier.owner_domains);
        normalize_strings(&mut verifier.consumer_domains);
        normalize_strings(&mut verifier.proves);
        normalize_strings(&mut verifier.subsumes);
        normalize_strings(&mut verifier.environment.required_features);
        verifier.external_effects.sort();
        verifier.external_effects.dedup();
        verifier.required_authorization.sort();
        verifier.required_authorization.dedup();
        verifier.consumed_identity_kinds.sort();
        verifier.consumed_identity_kinds.dedup();
    }
    catalog
        .verifiers
        .sort_by(|left, right| left.id.cmp(&right.id));

    let mut ids = BTreeSet::new();
    for verifier in &catalog.verifiers {
        if verifier.id.trim().is_empty() || !ids.insert(verifier.id.clone()) {
            return Err(catalog_error(
                "construction_validation.catalog_invalid",
                "verifier ids must be non-empty and unique",
            ));
        }
        if !REGISTERED_PRODUCERS.contains(&verifier.producer_id.as_str()) {
            return Err(catalog_error(
                "construction_validation.catalog_unknown_producer",
                format!("unknown producer id {:?}", verifier.producer_id),
            ));
        }
        if verifier.default_timeout_seconds == 0
            || verifier.default_timeout_seconds > 3 * 60 * 60
            || verifier
                .historical_duration_seconds
                .is_some_and(|duration| duration > verifier.default_timeout_seconds)
        {
            return Err(catalog_error(
                "construction_validation.catalog_invalid",
                format!(
                    "verifier {:?} has an invalid duration contract",
                    verifier.id
                ),
            ));
        }
        validate_effect_authorization(verifier)?;
    }

    let by_id = catalog
        .verifiers
        .iter()
        .map(|verifier| (verifier.id.clone(), verifier))
        .collect::<BTreeMap<_, _>>();
    for verifier in &catalog.verifiers {
        for target in &verifier.subsumes {
            if target == &verifier.id || !by_id.contains_key(target) {
                return Err(catalog_error(
                    "construction_validation.catalog_invalid",
                    format!(
                        "verifier {:?} has invalid subsumes target {target:?}",
                        verifier.id
                    ),
                ));
            }
        }
    }
    if has_subsumes_cycle(&catalog.verifiers) {
        return Err(catalog_error(
            "construction_validation.subsumes_cycle",
            "catalog subsumes relation contains a cycle",
        ));
    }
    Ok(catalog)
}

fn validate_effect_authorization(verifier: &VerifierCatalogEntry) -> Result<(), CatalogError> {
    let required = verifier
        .external_effects
        .iter()
        .filter_map(|effect| match effect {
            ExternalEffect::LocalCi => Some(AuthorizationRequirement::LocalCi),
            ExternalEffect::RealEditorSession | ExternalEffect::RealOsWindowOrGpu => {
                Some(AuthorizationRequirement::RealEditor)
            }
            ExternalEffect::ProductionBinaryReplacement
            | ExternalEffect::InstalledBinaryMutation => {
                Some(AuthorizationRequirement::ProductionReplacement)
            }
            ExternalEffect::RealConfigurationMutation => {
                Some(AuthorizationRequirement::RealConfigurationMutation)
            }
            ExternalEffect::OwnedProcesses
            | ExternalEffect::OwnedArtifactRoot
            | ExternalEffect::OneShotExternalAcceptance
            | ExternalEffect::OwnedRecursiveCleanup => None,
        })
        .collect::<BTreeSet<_>>();
    let declared = verifier
        .required_authorization
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if !required.is_subset(&declared) {
        return Err(catalog_error(
            "construction_validation.catalog_invalid",
            format!(
                "verifier {:?} has external effects without matching authorization",
                verifier.id
            ),
        ));
    }
    Ok(())
}

fn has_subsumes_cycle(verifiers: &[VerifierCatalogEntry]) -> bool {
    fn visit(
        id: &str,
        graph: &BTreeMap<String, Vec<String>>,
        visiting: &mut BTreeSet<String>,
        visited: &mut BTreeSet<String>,
    ) -> bool {
        if visiting.contains(id) {
            return true;
        }
        if visited.contains(id) {
            return false;
        }
        visiting.insert(id.to_string());
        if graph.get(id).is_some_and(|targets| {
            targets
                .iter()
                .any(|target| visit(target, graph, visiting, visited))
        }) {
            return true;
        }
        visiting.remove(id);
        visited.insert(id.to_string());
        false
    }

    let graph = verifiers
        .iter()
        .map(|verifier| (verifier.id.clone(), verifier.subsumes.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    graph
        .keys()
        .any(|id| visit(id, &graph, &mut visiting, &mut visited))
}

fn normalize_strings(values: &mut Vec<String>) {
    values.retain(|value| !value.trim().is_empty());
    values.sort();
    values.dedup();
}

fn catalog_error(code: impl Into<String>, message: impl Into<String>) -> CatalogError {
    CatalogError {
        code: code.into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, producer_id: &str) -> VerifierCatalogEntry {
        VerifierCatalogEntry {
            id: id.to_string(),
            owner_domains: vec!["quality_gate".to_string()],
            consumer_domains: Vec::new(),
            proves: vec!["development.owner".to_string()],
            producer_id: producer_id.to_string(),
            environment: EnvironmentRequirement::default(),
            subsumes: Vec::new(),
            cost_class: CostClass::Low,
            default_timeout_seconds: 60,
            historical_duration_seconds: Some(5),
            cleanup_reserve_seconds: 0,
            external_effects: Vec::new(),
            required_authorization: Vec::new(),
            consumed_identity_kinds: vec![EvidenceIdentityKind::ProductSource],
        }
    }

    #[test]
    fn catalog_unknown_producer_fails_closed() {
        let catalog = ValidationCatalog {
            schema_version: VALIDATION_CATALOG_SCHEMA_VERSION.to_string(),
            verifiers: vec![entry("owner", "arbitrary.shell")],
        };
        assert_eq!(
            validate_and_normalize_catalog(catalog).unwrap_err().code,
            "construction_validation.catalog_unknown_producer"
        );
    }

    #[test]
    fn catalog_subsumes_cycle_fails_closed() {
        let mut first = entry("first", "quality_gate.owner_tests");
        first.subsumes.push("second".to_string());
        let mut second = entry("second", "quality_gate.crate_tests");
        second.subsumes.push("first".to_string());
        let catalog = ValidationCatalog {
            schema_version: VALIDATION_CATALOG_SCHEMA_VERSION.to_string(),
            verifiers: vec![first, second],
        };
        assert_eq!(
            validate_and_normalize_catalog(catalog).unwrap_err().code,
            "construction_validation.subsumes_cycle"
        );
    }

    #[test]
    fn catalog_digest_is_order_independent() {
        let first = entry("a", "quality_gate.owner_tests");
        let second = entry("b", "quality_gate.crate_tests");
        let left = ValidationCatalog {
            schema_version: VALIDATION_CATALOG_SCHEMA_VERSION.to_string(),
            verifiers: vec![first.clone(), second.clone()],
        };
        let right = ValidationCatalog {
            schema_version: VALIDATION_CATALOG_SCHEMA_VERSION.to_string(),
            verifiers: vec![second, first],
        };
        assert_eq!(
            catalog_digest(&left).unwrap(),
            catalog_digest(&right).unwrap()
        );
    }

    #[test]
    fn catalog_committed_minimal_catalog_is_valid() {
        let source = include_bytes!("../../../quality/construction-validation-catalog.v1.json");
        let catalog = parse_validation_catalog(source).unwrap();
        assert_eq!(catalog.verifiers.len(), 2);
        assert!(catalog_digest(&catalog).unwrap().starts_with("sha256:"));
    }
}
