use crate::cargo_json::sha256_hex;
use crate::validation_catalog::{
    catalog_digest, AuthorizationRequirement, CostClass, EnvironmentIdentity, EvidenceIdentityKind,
    ExternalEffect, ValidationCatalog, ValidationCatalogSource, VerifierCatalogEntry,
};
use crate::validation_evidence::{evidence_is_reusable, EvidenceStore};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const PREPARE_REQUEST_SCHEMA_VERSION: &str = "construction-validation.prepare-request.v2";
pub const PREPARE_REPORT_SCHEMA_VERSION: &str = "construction-validation.prepare-report.v2";
const VALIDATION_PLAN_ROOT: &str = "target/quality-gate/validation-plans";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrepareRequest {
    pub schema_version: String,
    pub request_id: String,
    pub claim: ValidationClaim,
    pub change: ChangeIdentity,
    #[serde(default)]
    pub declared_owners: Vec<String>,
    #[serde(default)]
    pub declared_consumers: Vec<String>,
    #[serde(default)]
    pub required_capabilities: Vec<String>,
    pub environment: EnvironmentIdentity,
    pub authorization_ceiling: AuthorizationCeiling,
    pub time_budget_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangeIdentity {
    pub base_commit: Option<String>,
    pub head_commit: Option<String>,
    pub dirty_patch_digest: Option<String>,
    #[serde(default)]
    pub changed_subjects: Vec<ChangedSubjectFact>,
    #[serde(default)]
    pub identities: BTreeMap<EvidenceIdentityKind, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangedSubjectFact {
    pub path: String,
    pub owner_domain: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ValidationClaim {
    Development,
    Integration,
    Freeze,
    ReleaseActivation,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuthorizationCeiling {
    pub local_ci: bool,
    pub real_editor: bool,
    pub production_replacement: bool,
    pub real_configuration_mutation: bool,
}

impl AuthorizationCeiling {
    fn allows(self, requirement: AuthorizationRequirement) -> bool {
        match requirement {
            AuthorizationRequirement::LocalCi => self.local_ci,
            AuthorizationRequirement::RealEditor => self.real_editor,
            AuthorizationRequirement::ProductionReplacement => self.production_replacement,
            AuthorizationRequirement::RealConfigurationMutation => self.real_configuration_mutation,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrepareReport {
    pub schema_version: String,
    pub request_id: String,
    pub status: PrepareStatus,
    pub plan_ref: Option<PlanRef>,
    pub claim: ValidationClaim,
    pub affected_closure: Vec<String>,
    pub proof_obligations: Vec<ProofObligation>,
    pub stages: Vec<PlannedStage>,
    pub reused_evidence: Vec<ReusedEvidence>,
    pub eliminated_duplicates: Vec<EliminatedDuplicate>,
    pub estimated_duration_seconds: u64,
    pub cleanup_reserve_seconds: u64,
    pub required_authorization: Vec<AuthorizationRequirement>,
    pub omissions: Vec<String>,
    pub diagnostics: Vec<ValidationDiagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrepareStatus {
    Ready,
    Ineligible,
    NeedsAuthorization,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProofObligation {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlannedStage {
    pub verifier_id: String,
    pub producer_id: String,
    pub proof_obligations: Vec<String>,
    pub environment: EnvironmentIdentity,
    pub consumed_identities: BTreeMap<EvidenceIdentityKind, String>,
    pub cost_class: CostClass,
    pub estimated_duration_seconds: u64,
    pub timeout_seconds: u64,
    pub cleanup_reserve_seconds: u64,
    pub external_effects: Vec<ExternalEffect>,
    pub required_authorization: Vec<AuthorizationRequirement>,
    pub reused_evidence_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReusedEvidence {
    pub verifier_id: String,
    pub evidence_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EliminatedDuplicate {
    pub verifier_id: String,
    pub covered_by: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlanRef {
    pub plan_id: String,
    pub plan_digest: String,
    pub catalog_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValidationDiagnostic {
    pub code: String,
    pub domain: String,
    pub stage: String,
    pub observed_evidence: String,
    pub next_action: String,
}

#[derive(Debug, Clone)]
pub struct ConstructionValidationModule<C, E> {
    catalog_source: C,
    evidence_store: E,
}

impl<C, E> ConstructionValidationModule<C, E> {
    pub fn new(catalog_source: C, evidence_store: E) -> Self {
        Self {
            catalog_source,
            evidence_store,
        }
    }
}

impl<C: ValidationCatalogSource, E: EvidenceStore> ConstructionValidationModule<C, E> {
    pub fn prepare(&self, request: PrepareRequest) -> PrepareReport {
        let mut request = request;
        normalize_request(&mut request);
        if let Some(diagnostic) = validate_request(&request) {
            return empty_report(&request, PrepareStatus::Invalid, diagnostic);
        }
        let catalog = match self.catalog_source.load() {
            Ok(catalog) => catalog,
            Err(error) => {
                return empty_report(
                    &request,
                    PrepareStatus::Invalid,
                    diagnostic(
                        error.code,
                        "catalog",
                        error.message,
                        "Repair the committed validation catalog before planning.",
                    ),
                );
            }
        };
        self.prepare_with_catalog(request, catalog)
    }

    fn prepare_with_catalog(
        &self,
        request: PrepareRequest,
        catalog: ValidationCatalog,
    ) -> PrepareReport {
        let catalog_digest = match catalog_digest(&catalog) {
            Ok(digest) => digest,
            Err(error) => {
                return empty_report(
                    &request,
                    PrepareStatus::Invalid,
                    diagnostic(
                        error.code,
                        "catalog",
                        error.message,
                        "Repair the validation catalog before planning.",
                    ),
                );
            }
        };
        let affected_closure = match affected_closure(&request, &catalog) {
            Ok(closure) => closure,
            Err(diagnostic) => {
                return empty_report(&request, PrepareStatus::Invalid, diagnostic);
            }
        };
        let proof_obligations = proof_obligations(request.claim, &request.required_capabilities);
        let selected = match select_verifiers(
            &catalog,
            &affected_closure,
            &proof_obligations,
            &request.environment,
        ) {
            Ok(selected) => selected,
            Err(diagnostic) => {
                let mut report = empty_report(&request, PrepareStatus::Ineligible, diagnostic);
                report.affected_closure = affected_closure;
                report.proof_obligations = proof_obligations;
                return report;
            }
        };
        let (selected, eliminated_duplicates) = eliminate_subsumed(selected);

        let mut stages = Vec::new();
        let mut reused_evidence = Vec::new();
        let mut required_authorization = BTreeSet::new();
        for (entry, obligations) in selected {
            required_authorization.extend(entry.required_authorization.iter().copied());
            let consumed_identities = entry
                .consumed_identity_kinds
                .iter()
                .filter_map(|kind| {
                    request
                        .change
                        .identities
                        .get(kind)
                        .cloned()
                        .map(|digest| (*kind, digest))
                })
                .collect::<BTreeMap<_, _>>();
            let reusable = self
                .evidence_store
                .records_for(&entry.id)
                .into_iter()
                .filter(|record| {
                    evidence_is_reusable(
                        record,
                        &request.environment,
                        &request.change.identities,
                        &entry.consumed_identity_kinds,
                    )
                })
                .min_by(|left, right| left.evidence_ref.cmp(&right.evidence_ref));
            let reused_evidence_ref = reusable.as_ref().map(|record| record.evidence_ref.clone());
            if let Some(record) = reusable {
                reused_evidence.push(ReusedEvidence {
                    verifier_id: entry.id.clone(),
                    evidence_ref: record.evidence_ref,
                });
            }
            stages.push(PlannedStage {
                verifier_id: entry.id,
                producer_id: entry.producer_id,
                proof_obligations: obligations.into_iter().collect(),
                environment: request.environment.clone(),
                consumed_identities,
                cost_class: entry.cost_class,
                estimated_duration_seconds: entry
                    .historical_duration_seconds
                    .unwrap_or(entry.default_timeout_seconds),
                timeout_seconds: entry.default_timeout_seconds,
                cleanup_reserve_seconds: entry.cleanup_reserve_seconds,
                external_effects: entry.external_effects,
                required_authorization: entry.required_authorization,
                reused_evidence_ref,
            });
        }
        stages.sort_by(|left, right| {
            left.cost_class
                .rank()
                .cmp(&right.cost_class.rank())
                .then_with(|| left.verifier_id.cmp(&right.verifier_id))
        });
        reused_evidence.sort_by(|left, right| left.verifier_id.cmp(&right.verifier_id));

        let estimated_duration_seconds = stages
            .iter()
            .filter(|stage| stage.reused_evidence_ref.is_none())
            .map(|stage| stage.estimated_duration_seconds)
            .sum::<u64>();
        let cleanup_reserve_seconds = stages
            .iter()
            .filter(|stage| stage.reused_evidence_ref.is_none())
            .map(|stage| stage.cleanup_reserve_seconds)
            .sum::<u64>();
        let required_authorization = required_authorization.into_iter().collect::<Vec<_>>();
        let missing_authorization = required_authorization
            .iter()
            .copied()
            .filter(|requirement| !request.authorization_ceiling.allows(*requirement))
            .collect::<Vec<_>>();
        let (status, diagnostics) = if !missing_authorization.is_empty() {
            (
                PrepareStatus::NeedsAuthorization,
                vec![diagnostic(
                    "construction_validation.authorization_required",
                    "authorization",
                    format!("planning requires {missing_authorization:?}"),
                    "Obtain explicit authorization or lower the requested claim.",
                )],
            )
        } else if estimated_duration_seconds.saturating_add(cleanup_reserve_seconds)
            > request.time_budget_seconds
        {
            (
                PrepareStatus::Ineligible,
                vec![diagnostic(
                    "construction_validation.time_budget_exceeded",
                    "budget",
                    format!(
                        "estimated {}s plus cleanup {}s exceeds budget {}s",
                        estimated_duration_seconds,
                        cleanup_reserve_seconds,
                        request.time_budget_seconds
                    ),
                    "Increase the declared budget or reduce the claim without omitting proof obligations.",
                )],
            )
        } else {
            (PrepareStatus::Ready, Vec::new())
        };

        let mut report = PrepareReport {
            schema_version: PREPARE_REPORT_SCHEMA_VERSION.to_string(),
            request_id: request.request_id.clone(),
            status,
            plan_ref: None,
            claim: request.claim,
            affected_closure,
            proof_obligations,
            stages,
            reused_evidence,
            eliminated_duplicates,
            estimated_duration_seconds,
            cleanup_reserve_seconds,
            required_authorization,
            omissions: Vec::new(),
            diagnostics,
        };
        let digest = plan_digest(&request, &catalog_digest, &report);
        report.plan_ref = Some(PlanRef {
            plan_id: format!("plan-{}", &digest[7..23]),
            plan_digest: digest,
            catalog_digest,
        });
        report
    }
}

pub fn prepare_validation_plan_files(
    workspace_root: &Path,
    request_path: &Path,
    catalog_path: &Path,
    output_path: &Path,
) -> Result<PrepareReport, ValidationDiagnostic> {
    let request_source = fs::read(request_path).map_err(|error| {
        diagnostic(
            "construction_validation.request_invalid",
            "request",
            format!("failed to read request {request_path:?}: {error}"),
            "Provide a readable PrepareRequest v2 JSON file.",
        )
    })?;
    let request = serde_json::from_slice::<PrepareRequest>(&request_source).map_err(|error| {
        diagnostic(
            "construction_validation.request_invalid",
            "request",
            format!("invalid request {request_path:?}: {error}"),
            "Repair the PrepareRequest v2 JSON before planning.",
        )
    })?;
    let catalog_source = fs::read(catalog_path).map_err(|error| {
        diagnostic(
            "construction_validation.catalog_invalid",
            "catalog",
            format!("failed to read catalog {catalog_path:?}: {error}"),
            "Provide a readable repository-owned validation catalog.",
        )
    })?;
    let catalog =
        crate::validation_catalog::parse_validation_catalog(&catalog_source).map_err(|error| {
            diagnostic(
                error.code,
                "catalog",
                error.message,
                "Repair the validation catalog before planning.",
            )
        })?;
    let report = ConstructionValidationModule::new(
        crate::validation_catalog::StaticValidationCatalogSource::new(catalog),
        crate::validation_evidence::EmptyEvidenceStore,
    )
    .prepare(request);
    let output = resolve_validation_plan_output(workspace_root, output_path)?;
    let mut bytes = serde_json::to_vec_pretty(&report).map_err(|error| {
        diagnostic(
            "construction_validation.plan_write_failed",
            "report",
            error.to_string(),
            "Repair the report schema before retrying.",
        )
    })?;
    bytes.push(b'\n');
    write_plan_idempotently(workspace_root, &output, &bytes)?;
    Ok(report)
}

pub fn resolve_validation_plan_output(
    workspace_root: &Path,
    requested: &Path,
) -> Result<PathBuf, ValidationDiagnostic> {
    if requested.is_absolute()
        || requested
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(output_path_diagnostic(
            "output path must be a normalized workspace-relative path",
        ));
    }
    let allowed_root = Path::new(VALIDATION_PLAN_ROOT);
    if requested == allowed_root || !requested.starts_with(allowed_root) {
        return Err(output_path_diagnostic(format!(
            "output must be a strict child of {VALIDATION_PLAN_ROOT}"
        )));
    }
    if requested.extension().and_then(|value| value.to_str()) != Some("json") {
        return Err(output_path_diagnostic(
            "output file must use the .json extension",
        ));
    }
    let workspace_root = fs::canonicalize(workspace_root).map_err(|error| {
        output_path_diagnostic(format!("failed to resolve workspace root: {error}"))
    })?;
    let output = workspace_root.join(requested);
    reject_existing_reparse_components(&workspace_root, &output)?;
    Ok(output)
}

fn write_plan_idempotently(
    workspace_root: &Path,
    output: &Path,
    bytes: &[u8],
) -> Result<(), ValidationDiagnostic> {
    if output.exists() {
        let existing = fs::read(output).map_err(|error| {
            plan_write_diagnostic(format!("failed to read existing output: {error}"))
        })?;
        if existing == bytes {
            return Ok(());
        }
        return Err(plan_write_diagnostic(
            "output already exists with different content",
        ));
    }
    let parent = output
        .parent()
        .ok_or_else(|| plan_write_diagnostic("output has no parent directory"))?;
    fs::create_dir_all(parent).map_err(|error| {
        plan_write_diagnostic(format!("failed to create plan output directory: {error}"))
    })?;
    let workspace_root = fs::canonicalize(workspace_root).map_err(|error| {
        plan_write_diagnostic(format!("failed to resolve workspace root: {error}"))
    })?;
    reject_existing_reparse_components(&workspace_root, output)?;
    fs::write(output, bytes)
        .map_err(|error| plan_write_diagnostic(format!("failed to write prepare report: {error}")))
}

fn reject_existing_reparse_components(
    workspace_root: &Path,
    output: &Path,
) -> Result<(), ValidationDiagnostic> {
    let relative = output.strip_prefix(workspace_root).map_err(|_| {
        output_path_diagnostic("output does not belong to the canonical workspace root")
    })?;
    let mut cursor = workspace_root.to_path_buf();
    for component in relative.components() {
        cursor.push(component.as_os_str());
        if !cursor.exists() {
            continue;
        }
        let metadata = fs::symlink_metadata(&cursor).map_err(|error| {
            output_path_diagnostic(format!("failed to inspect output component: {error}"))
        })?;
        if metadata_is_link_or_reparse(&metadata) {
            return Err(output_path_diagnostic(format!(
                "output component {cursor:?} is a link or reparse point"
            )));
        }
    }
    Ok(())
}

#[cfg(windows)]
fn metadata_is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn metadata_is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn output_path_diagnostic(observed: impl Into<String>) -> ValidationDiagnostic {
    diagnostic(
        "construction_validation.output_path_invalid",
        "output",
        observed,
        "Choose a regular JSON file below target/quality-gate/validation-plans/<run-id>/.",
    )
}

fn plan_write_diagnostic(observed: impl Into<String>) -> ValidationDiagnostic {
    diagnostic(
        "construction_validation.plan_write_failed",
        "output",
        observed,
        "Choose a fresh run-owned output path or preserve the identical existing report.",
    )
}

fn normalize_request(request: &mut PrepareRequest) {
    request.environment.normalize();
    normalize_strings(&mut request.declared_owners);
    normalize_strings(&mut request.declared_consumers);
    normalize_strings(&mut request.required_capabilities);
    request.change.changed_subjects.sort();
    request.change.changed_subjects.dedup();
}

fn validate_request(request: &PrepareRequest) -> Option<ValidationDiagnostic> {
    if request.schema_version != PREPARE_REQUEST_SCHEMA_VERSION
        || request.request_id.trim().is_empty()
        || request.time_budget_seconds == 0
        || request.time_budget_seconds > 3 * 60 * 60
        || request.environment.platform.trim().is_empty()
        || request.environment.profile.trim().is_empty()
        || request.environment.composition.trim().is_empty()
    {
        return Some(diagnostic(
            "construction_validation.request_invalid",
            "request",
            "request schema, identity, environment, or time budget is invalid",
            "Provide a valid v2 request with a budget no greater than three hours.",
        ));
    }
    if request.declared_owners.is_empty() && request.change.changed_subjects.is_empty() {
        return Some(diagnostic(
            "construction_validation.request_invalid",
            "request",
            "no changed subject or declared owner was supplied",
            "Declare the changed owner domains before planning.",
        ));
    }
    None
}

fn affected_closure(
    request: &PrepareRequest,
    catalog: &ValidationCatalog,
) -> Result<Vec<String>, ValidationDiagnostic> {
    let mut known = BTreeSet::new();
    let mut adjacency = BTreeMap::<String, BTreeSet<String>>::new();
    for verifier in &catalog.verifiers {
        known.extend(verifier.owner_domains.iter().cloned());
        known.extend(verifier.consumer_domains.iter().cloned());
        for owner in &verifier.owner_domains {
            adjacency
                .entry(owner.clone())
                .or_default()
                .extend(verifier.consumer_domains.iter().cloned());
        }
    }
    let mut roots = request
        .declared_owners
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    roots.extend(
        request
            .change
            .changed_subjects
            .iter()
            .map(|subject| subject.owner_domain.clone()),
    );
    roots.extend(request.declared_consumers.iter().cloned());
    if let Some(unresolved) = roots.iter().find(|root| !known.contains(*root)) {
        return Err(diagnostic(
            "construction_validation.owner_unresolved",
            "affected_closure",
            format!("domain {unresolved:?} is absent from the validation catalog"),
            "Register the owner or correct the request before planning.",
        ));
    }
    let mut closure = roots.clone();
    let mut queue = roots.into_iter().collect::<VecDeque<_>>();
    while let Some(domain) = queue.pop_front() {
        if let Some(consumers) = adjacency.get(&domain) {
            for consumer in consumers {
                if closure.insert(consumer.clone()) {
                    queue.push_back(consumer.clone());
                }
            }
        }
    }
    Ok(closure.into_iter().collect())
}

fn proof_obligations(
    claim: ValidationClaim,
    required_capabilities: &[String],
) -> Vec<ProofObligation> {
    let mut ids = vec!["development.owner".to_string()];
    if matches!(
        claim,
        ValidationClaim::Integration | ValidationClaim::Freeze | ValidationClaim::ReleaseActivation
    ) {
        ids.push("integration.affected".to_string());
    }
    if matches!(
        claim,
        ValidationClaim::Freeze | ValidationClaim::ReleaseActivation
    ) {
        ids.push("freeze.candidate".to_string());
    }
    if claim == ValidationClaim::ReleaseActivation {
        ids.push("release.activation".to_string());
    }
    ids.extend(required_capabilities.iter().cloned());
    ids.sort();
    ids.dedup();
    ids.into_iter().map(|id| ProofObligation { id }).collect()
}

fn select_verifiers(
    catalog: &ValidationCatalog,
    affected_closure: &[String],
    obligations: &[ProofObligation],
    environment: &EnvironmentIdentity,
) -> Result<Vec<(VerifierCatalogEntry, BTreeSet<String>)>, ValidationDiagnostic> {
    let affected = affected_closure.iter().collect::<BTreeSet<_>>();
    let candidates = catalog
        .verifiers
        .iter()
        .filter(|entry| {
            entry.environment.matches(environment)
                && entry
                    .owner_domains
                    .iter()
                    .chain(&entry.consumer_domains)
                    .any(|domain| affected.contains(domain))
        })
        .collect::<Vec<_>>();
    let mut uncovered = obligations
        .iter()
        .map(|obligation| obligation.id.clone())
        .collect::<BTreeSet<_>>();
    let mut selected = BTreeMap::<String, (VerifierCatalogEntry, BTreeSet<String>)>::new();
    while !uncovered.is_empty() {
        let mut proving = candidates
            .iter()
            .filter_map(|entry| {
                let covered = entry
                    .proves
                    .iter()
                    .filter(|proof| uncovered.contains(*proof))
                    .cloned()
                    .collect::<BTreeSet<_>>();
                (!covered.is_empty()).then_some((*entry, covered))
            })
            .collect::<Vec<_>>();
        proving.sort_by(|(left, left_covered), (right, right_covered)| {
            right_covered
                .len()
                .cmp(&left_covered.len())
                .then_with(|| left.cost_class.rank().cmp(&right.cost_class.rank()))
                .then_with(|| left.id.cmp(&right.id))
        });
        let Some((entry, covered)) = proving.first() else {
            return Err(diagnostic(
                "construction_validation.proof_uncovered",
                "selection",
                format!("no affected verifier proves remaining obligations {uncovered:?}"),
                "Add a bounded verifier catalog entry or lower the claim.",
            ));
        };
        for obligation in covered {
            uncovered.remove(obligation);
        }
        selected.insert(entry.id.clone(), ((*entry).clone(), covered.clone()));
    }
    Ok(selected.into_values().collect())
}

fn eliminate_subsumed(
    selected: Vec<(VerifierCatalogEntry, BTreeSet<String>)>,
) -> (
    Vec<(VerifierCatalogEntry, BTreeSet<String>)>,
    Vec<EliminatedDuplicate>,
) {
    let mut selected = selected
        .into_iter()
        .map(|(entry, obligations)| (entry.id.clone(), (entry, obligations)))
        .collect::<BTreeMap<_, _>>();
    let ids = selected.keys().cloned().collect::<Vec<_>>();
    let mut eliminated = Vec::new();
    for covering_id in &ids {
        let Some((covering, _)) = selected.get(covering_id) else {
            continue;
        };
        let targets = covering.subsumes.clone();
        for target in targets {
            if target == *covering_id || !selected.contains_key(&target) {
                continue;
            }
            let (_, target_obligations) = selected.remove(&target).expect("selected target");
            if let Some((_, covering_obligations)) = selected.get_mut(covering_id) {
                covering_obligations.extend(target_obligations);
            }
            eliminated.push(EliminatedDuplicate {
                verifier_id: target,
                covered_by: covering_id.clone(),
                reason: "same_environment_catalog_subsumption".to_string(),
            });
        }
    }
    eliminated.sort_by(|left, right| left.verifier_id.cmp(&right.verifier_id));
    (selected.into_values().collect(), eliminated)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlanDigestPayload<'a> {
    request: &'a PrepareRequest,
    catalog_digest: &'a str,
    status: PrepareStatus,
    affected_closure: &'a [String],
    proof_obligations: &'a [ProofObligation],
    stages: &'a [PlannedStage],
    reused_evidence: &'a [ReusedEvidence],
    eliminated_duplicates: &'a [EliminatedDuplicate],
    estimated_duration_seconds: u64,
    cleanup_reserve_seconds: u64,
    required_authorization: &'a [AuthorizationRequirement],
}

fn plan_digest(request: &PrepareRequest, catalog_digest: &str, report: &PrepareReport) -> String {
    let bytes = serde_json::to_vec(&PlanDigestPayload {
        request,
        catalog_digest,
        status: report.status,
        affected_closure: &report.affected_closure,
        proof_obligations: &report.proof_obligations,
        stages: &report.stages,
        reused_evidence: &report.reused_evidence,
        eliminated_duplicates: &report.eliminated_duplicates,
        estimated_duration_seconds: report.estimated_duration_seconds,
        cleanup_reserve_seconds: report.cleanup_reserve_seconds,
        required_authorization: &report.required_authorization,
    })
    .expect("plan digest payload is serializable");
    format!("sha256:{}", sha256_hex(&bytes))
}

fn empty_report(
    request: &PrepareRequest,
    status: PrepareStatus,
    diagnostic: ValidationDiagnostic,
) -> PrepareReport {
    PrepareReport {
        schema_version: PREPARE_REPORT_SCHEMA_VERSION.to_string(),
        request_id: request.request_id.clone(),
        status,
        plan_ref: None,
        claim: request.claim,
        affected_closure: Vec::new(),
        proof_obligations: Vec::new(),
        stages: Vec::new(),
        reused_evidence: Vec::new(),
        eliminated_duplicates: Vec::new(),
        estimated_duration_seconds: 0,
        cleanup_reserve_seconds: 0,
        required_authorization: Vec::new(),
        omissions: Vec::new(),
        diagnostics: vec![diagnostic],
    }
}

fn diagnostic(
    code: impl Into<String>,
    stage: impl Into<String>,
    observed_evidence: impl Into<String>,
    next_action: impl Into<String>,
) -> ValidationDiagnostic {
    ValidationDiagnostic {
        code: code.into(),
        domain: "construction_validation".to_string(),
        stage: stage.into(),
        observed_evidence: observed_evidence.into(),
        next_action: next_action.into(),
    }
}

fn normalize_strings(values: &mut Vec<String>) {
    values.retain(|value| !value.trim().is_empty());
    values.sort();
    values.dedup();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validation_catalog::{
        EnvironmentRequirement, StaticValidationCatalogSource, VALIDATION_CATALOG_SCHEMA_VERSION,
    };
    use crate::validation_evidence::{EmptyEvidenceStore, EvidenceRecord, InMemoryEvidenceStore};

    fn entry(
        id: &str,
        owner: &str,
        consumers: &[&str],
        proves: &[&str],
        cost: CostClass,
    ) -> VerifierCatalogEntry {
        VerifierCatalogEntry {
            id: id.to_string(),
            owner_domains: vec![owner.to_string()],
            consumer_domains: consumers.iter().map(|value| value.to_string()).collect(),
            proves: proves.iter().map(|value| value.to_string()).collect(),
            producer_id: if cost == CostClass::Low {
                "quality_gate.owner_tests"
            } else {
                "quality_gate.crate_tests"
            }
            .to_string(),
            environment: EnvironmentRequirement::default(),
            subsumes: Vec::new(),
            cost_class: cost,
            default_timeout_seconds: 100,
            historical_duration_seconds: Some(if cost == CostClass::Low { 5 } else { 20 }),
            cleanup_reserve_seconds: 0,
            external_effects: Vec::new(),
            required_authorization: Vec::new(),
            consumed_identity_kinds: vec![EvidenceIdentityKind::ProductSource],
        }
    }

    fn catalog(entries: Vec<VerifierCatalogEntry>) -> ValidationCatalog {
        ValidationCatalog {
            schema_version: VALIDATION_CATALOG_SCHEMA_VERSION.to_string(),
            verifiers: entries,
        }
    }

    fn request(claim: ValidationClaim) -> PrepareRequest {
        PrepareRequest {
            schema_version: PREPARE_REQUEST_SCHEMA_VERSION.to_string(),
            request_id: "request-a".to_string(),
            claim,
            change: ChangeIdentity {
                base_commit: None,
                head_commit: None,
                dirty_patch_digest: Some("sha256:patch".to_string()),
                changed_subjects: vec![ChangedSubjectFact {
                    path: "crates/quality_gate/src/lib.rs".to_string(),
                    owner_domain: "quality_gate".to_string(),
                }],
                identities: BTreeMap::from([(
                    EvidenceIdentityKind::ProductSource,
                    "sha256:source".to_string(),
                )]),
            },
            declared_owners: Vec::new(),
            declared_consumers: Vec::new(),
            required_capabilities: Vec::new(),
            environment: EnvironmentIdentity {
                platform: "windows-x86_64".to_string(),
                profile: "debug".to_string(),
                features: Vec::new(),
                composition: "source".to_string(),
            },
            authorization_ceiling: AuthorizationCeiling::default(),
            time_budget_seconds: 300,
        }
    }

    fn module(
        entries: Vec<VerifierCatalogEntry>,
    ) -> ConstructionValidationModule<StaticValidationCatalogSource, EmptyEvidenceStore> {
        ConstructionValidationModule::new(
            StaticValidationCatalogSource::new(catalog(entries)),
            EmptyEvidenceStore,
        )
    }

    fn standard_entries() -> Vec<VerifierCatalogEntry> {
        vec![
            entry(
                "owner",
                "quality_gate",
                &["quality_gate_cli"],
                &["development.owner"],
                CostClass::Low,
            ),
            entry(
                "integration",
                "quality_gate_cli",
                &["rust_workspace"],
                &["integration.affected"],
                CostClass::Medium,
            ),
        ]
    }

    #[test]
    fn schema_unknown_field_fails_closed() {
        let source = serde_json::to_string(&request(ValidationClaim::Development)).unwrap();
        let source = source.replacen('{', "{\"unexpected\":true,", 1);
        assert!(serde_json::from_str::<PrepareRequest>(&source).is_err());
    }

    #[test]
    fn plan_only_prepare_has_no_executor_dependency() {
        let report = module(standard_entries()).prepare(request(ValidationClaim::Development));
        assert_eq!(report.status, PrepareStatus::Ready);
        assert_eq!(report.stages.len(), 1);
    }

    #[test]
    fn affected_closure_includes_transitive_consumers_and_excludes_unrelated() {
        let mut entries = standard_entries();
        entries.push(entry(
            "unrelated",
            "editor",
            &[],
            &["development.owner"],
            CostClass::Low,
        ));
        let report = module(entries).prepare(request(ValidationClaim::Integration));
        assert_eq!(
            report.affected_closure,
            vec!["quality_gate", "quality_gate_cli", "rust_workspace"]
        );
        assert!(report
            .stages
            .iter()
            .all(|stage| stage.verifier_id != "unrelated"));
    }

    #[test]
    fn affected_unresolved_owner_fails_closed() {
        let mut request = request(ValidationClaim::Development);
        request.change.changed_subjects[0].owner_domain = "missing".to_string();
        let report = module(standard_entries()).prepare(request);
        assert_eq!(report.status, PrepareStatus::Invalid);
        assert_eq!(
            report.diagnostics[0].code,
            "construction_validation.owner_unresolved"
        );
    }

    #[test]
    fn claim_levels_are_cumulative_proof_obligations() {
        assert_eq!(
            proof_obligations(ValidationClaim::Development, &[]).len(),
            1
        );
        assert_eq!(
            proof_obligations(ValidationClaim::Integration, &[]).len(),
            2
        );
        assert_eq!(proof_obligations(ValidationClaim::Freeze, &[]).len(), 3);
        assert_eq!(
            proof_obligations(ValidationClaim::ReleaseActivation, &[]).len(),
            4
        );
    }

    #[test]
    fn minimal_verifier_selection_prefers_low_cost() {
        let mut entries = standard_entries();
        entries.push(entry(
            "expensive-owner",
            "quality_gate",
            &[],
            &["development.owner"],
            CostClass::High,
        ));
        let report = module(entries).prepare(request(ValidationClaim::Development));
        assert_eq!(report.stages[0].verifier_id, "owner");
    }

    #[test]
    fn minimal_verifier_selection_prefers_one_stage_covering_more_obligations() {
        let mut entries = standard_entries();
        entries.push(entry(
            "combined",
            "quality_gate",
            &["quality_gate_cli"],
            &["development.owner", "integration.affected"],
            CostClass::Medium,
        ));
        let report = module(entries).prepare(request(ValidationClaim::Integration));
        assert_eq!(report.status, PrepareStatus::Ready);
        assert_eq!(report.stages.len(), 1);
        assert_eq!(report.stages[0].verifier_id, "combined");
    }

    #[test]
    fn minimal_verifier_uncovered_proof_is_ineligible() {
        let report = module(vec![standard_entries().remove(0)])
            .prepare(request(ValidationClaim::Integration));
        assert_eq!(report.status, PrepareStatus::Ineligible);
        assert_eq!(
            report.diagnostics[0].code,
            "construction_validation.proof_uncovered"
        );
    }

    #[test]
    fn subsumes_selected_same_environment_stage() {
        let mut entries = standard_entries();
        entries[1].subsumes.push("owner".to_string());
        let report = module(entries).prepare(request(ValidationClaim::Integration));
        assert_eq!(report.stages.len(), 1);
        assert_eq!(report.eliminated_duplicates[0].verifier_id, "owner");
        assert_eq!(report.stages[0].proof_obligations.len(), 2);
    }

    #[test]
    fn cost_budget_includes_cleanup_reserve() {
        let mut entries = standard_entries();
        entries[0].cleanup_reserve_seconds = 10;
        let mut request = request(ValidationClaim::Development);
        request.time_budget_seconds = 14;
        let report = module(entries).prepare(request);
        assert_eq!(report.status, PrepareStatus::Ineligible);
        assert_eq!(
            report.diagnostics[0].code,
            "construction_validation.time_budget_exceeded"
        );
    }

    #[test]
    fn plan_digest_is_stable_for_permuted_inputs() {
        let entries = standard_entries();
        let module = module(entries);
        let mut left = request(ValidationClaim::Integration);
        left.declared_owners = vec!["quality_gate".to_string(), "quality_gate".to_string()];
        left.environment.features = vec!["b".to_string(), "a".to_string()];
        let mut right = left.clone();
        right.declared_owners.reverse();
        right.environment.features.reverse();
        assert_eq!(
            module.prepare(left).plan_ref.unwrap().plan_digest,
            module.prepare(right).plan_ref.unwrap().plan_digest
        );
    }

    #[test]
    fn plan_digest_changes_for_semantic_input() {
        let module = module(standard_entries());
        let left = request(ValidationClaim::Development);
        let mut right = left.clone();
        right.request_id = "request-b".to_string();
        assert_ne!(
            module.prepare(left).plan_ref.unwrap().plan_digest,
            module.prepare(right).plan_ref.unwrap().plan_digest
        );
    }

    #[test]
    fn evidence_reuse_removes_stage_cost() {
        let entries = standard_entries();
        let environment = request(ValidationClaim::Development).environment;
        let evidence = EvidenceRecord {
            evidence_ref: "owner-pass".to_string(),
            verifier_id: "owner".to_string(),
            consumed_identities: BTreeMap::from([(
                EvidenceIdentityKind::ProductSource,
                "sha256:source".to_string(),
            )]),
            environment,
            report_digest: Some(format!("sha256:{}", "a".repeat(64))),
            passed: true,
        };
        let module = ConstructionValidationModule::new(
            StaticValidationCatalogSource::new(catalog(entries)),
            InMemoryEvidenceStore::new(vec![evidence]),
        );
        let report = module.prepare(request(ValidationClaim::Development));
        assert_eq!(report.status, PrepareStatus::Ready);
        assert_eq!(report.estimated_duration_seconds, 0);
        assert_eq!(report.reused_evidence[0].evidence_ref, "owner-pass");
    }

    #[test]
    fn subsumes_entry_with_different_environment_is_not_selected() {
        let mut entries = standard_entries();
        entries[1].environment.platform = Some("linux-x86_64".to_string());
        entries[1].subsumes.push("owner".to_string());
        let report = module(entries).prepare(request(ValidationClaim::Integration));
        assert_eq!(report.status, PrepareStatus::Ineligible);
        assert!(report.eliminated_duplicates.is_empty());
    }

    #[test]
    fn authorization_missing_from_ceiling_needs_authorization() {
        let mut owner = standard_entries().remove(0);
        owner.external_effects.push(ExternalEffect::LocalCi);
        owner
            .required_authorization
            .push(AuthorizationRequirement::LocalCi);
        let report = module(vec![owner]).prepare(request(ValidationClaim::Development));
        assert_eq!(report.status, PrepareStatus::NeedsAuthorization);
        assert_eq!(
            report.diagnostics[0].code,
            "construction_validation.authorization_required"
        );
    }
}
