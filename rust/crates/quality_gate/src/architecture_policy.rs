use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::Path;

pub const ARCHITECTURE_POLICY_SCHEMA_VERSION: &str = "architecture-policy.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchitecturePolicy {
    pub schema_version: String,
    pub profiles: Vec<PolicyProfile>,
    pub domains: Vec<DomainPolicy>,
    #[serde(default)]
    pub dependency_rules: Vec<DependencyRule>,
    pub review_authorities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyProfile {
    pub id: String,
    pub mode: PolicyMode,
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyMode {
    EngineStrict,
    ProjectAdvisory,
    ProjectStrict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPolicy {
    pub id: String,
    pub owner: String,
    pub include: Vec<String>,
    #[serde(default)]
    pub facade_only: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyRule {
    pub id: String,
    pub from: String,
    pub to: String,
    pub allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchitectureDiagnostic {
    pub code: String,
    pub source_path: Option<String>,
    pub domain: Option<String>,
    pub subject: Option<String>,
    pub stage: String,
    pub observed_evidence: String,
    pub rule_id: Option<String>,
    pub classification: String,
    pub next_action: String,
}

impl ArchitectureDiagnostic {
    pub fn policy(
        code: impl Into<String>,
        evidence: impl Into<String>,
        next_action: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            source_path: None,
            domain: None,
            subject: None,
            stage: "architecture_policy".to_string(),
            observed_evidence: evidence.into(),
            rule_id: None,
            classification: "invalid_policy".to_string(),
            next_action: next_action.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyError {
    pub diagnostics: Vec<ArchitectureDiagnostic>,
}

impl fmt::Display for PolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = self
            .diagnostics
            .iter()
            .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.observed_evidence))
            .collect::<Vec<_>>()
            .join("; ");
        formatter.write_str(&message)
    }
}

impl std::error::Error for PolicyError {}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyDiff {
    pub affected_profiles: Vec<String>,
    pub affected_domains: Vec<String>,
    pub relaxations: Vec<String>,
}

impl PolicyDiff {
    pub fn requires_review(&self) -> bool {
        !self.relaxations.is_empty()
    }
}

pub fn load_policy(path: &Path) -> Result<ArchitecturePolicy, PolicyError> {
    let source = fs::read_to_string(path).map_err(|error| PolicyError {
        diagnostics: vec![ArchitectureDiagnostic::policy(
            "architecture_policy.read_failed",
            format!("{}: {error}", path.display()),
            "Restore the committed architecture policy.",
        )],
    })?;
    parse_policy(&source)
}

pub fn parse_policy(source: &str) -> Result<ArchitecturePolicy, PolicyError> {
    let policy = toml::from_str::<ArchitecturePolicy>(source).map_err(|error| PolicyError {
        diagnostics: vec![ArchitectureDiagnostic::policy(
            "architecture_policy.schema_invalid",
            error.to_string(),
            "Use only fields defined by architecture-policy.v1.",
        )],
    })?;
    validate_policy(&policy)?;
    Ok(policy)
}

pub fn validate_policy(policy: &ArchitecturePolicy) -> Result<(), PolicyError> {
    let mut diagnostics = Vec::new();
    if policy.schema_version != ARCHITECTURE_POLICY_SCHEMA_VERSION {
        diagnostics.push(ArchitectureDiagnostic::policy(
            "architecture_policy.unknown_schema",
            format!("unsupported schema {:?}", policy.schema_version),
            "Set schema_version to architecture-policy.v1.",
        ));
    }

    collect_duplicate_ids(
        policy.profiles.iter().map(|profile| profile.id.as_str()),
        "profile",
        &mut diagnostics,
    );
    collect_duplicate_ids(
        policy.domains.iter().map(|domain| domain.id.as_str()),
        "domain",
        &mut diagnostics,
    );
    collect_duplicate_ids(
        policy.dependency_rules.iter().map(|rule| rule.id.as_str()),
        "dependency rule",
        &mut diagnostics,
    );

    let modes = policy
        .profiles
        .iter()
        .map(|profile| profile.mode)
        .collect::<BTreeSet<_>>();
    for mode in [
        PolicyMode::EngineStrict,
        PolicyMode::ProjectAdvisory,
        PolicyMode::ProjectStrict,
    ] {
        if !modes.contains(&mode) {
            diagnostics.push(ArchitectureDiagnostic::policy(
                "architecture_policy.profile_missing",
                format!("missing required profile mode {mode:?}"),
                "Declare EngineStrict, ProjectAdvisory, and ProjectStrict profiles.",
            ));
        }
    }

    let mut include_owner = BTreeMap::new();
    for domain in &policy.domains {
        if domain.owner.trim().is_empty() {
            diagnostics.push(ArchitectureDiagnostic::policy(
                "architecture_policy.owner_missing",
                format!("domain {:?} has no owner", domain.id),
                "Assign a stable owner to every domain.",
            ));
        }
        for include in &domain.include {
            if let Some(previous) = include_owner.insert(include, &domain.id) {
                diagnostics.push(ArchitectureDiagnostic::policy(
                    "architecture_policy.path_overlap",
                    format!(
                        "path pattern {include:?} belongs to {previous:?} and {:?}",
                        domain.id
                    ),
                    "Make domain include patterns unambiguous.",
                ));
            }
        }
    }

    let domain_ids = policy
        .domains
        .iter()
        .map(|domain| domain.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut allowed_edges = BTreeMap::<&str, Vec<&str>>::new();
    for rule in &policy.dependency_rules {
        if !domain_ids.contains(rule.from.as_str()) || !domain_ids.contains(rule.to.as_str()) {
            diagnostics.push(ArchitectureDiagnostic::policy(
                "architecture_policy.unknown_domain",
                format!("rule {:?} references {} -> {}", rule.id, rule.from, rule.to),
                "Reference only declared domain IDs.",
            ));
        } else if rule.allowed && rule.from != rule.to {
            allowed_edges
                .entry(rule.from.as_str())
                .or_default()
                .push(rule.to.as_str());
        }
    }
    if let Some(cycle) = dependency_cycle(&allowed_edges) {
        diagnostics.push(ArchitectureDiagnostic::policy(
            "architecture_policy.dependency_cycle",
            format!("allowed domain dependency cycle: {}", cycle.join(" -> ")),
            "Remove an allowed edge so domain dependencies remain acyclic.",
        ));
    }

    if policy.review_authorities.is_empty()
        || policy
            .review_authorities
            .iter()
            .any(|authority| authority.trim().is_empty())
    {
        diagnostics.push(ArchitectureDiagnostic::policy(
            "architecture_policy.review_authority_missing",
            "review_authorities must contain non-empty committed authority IDs",
            "Declare at least one review authority.",
        ));
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(PolicyError { diagnostics })
    }
}

pub fn diff_policy(previous: &ArchitecturePolicy, candidate: &ArchitecturePolicy) -> PolicyDiff {
    let mut diff = PolicyDiff::default();
    for candidate_profile in &candidate.profiles {
        let Some(previous_profile) = previous
            .profiles
            .iter()
            .find(|profile| profile.id == candidate_profile.id)
        else {
            diff.affected_profiles.push(candidate_profile.id.clone());
            continue;
        };
        if candidate_profile != previous_profile {
            diff.affected_profiles.push(candidate_profile.id.clone());
        }
        for exclusion in &candidate_profile.exclude {
            if !previous_profile.exclude.contains(exclusion) {
                diff.relaxations.push(format!(
                    "profile {} added exclusion {}",
                    candidate_profile.id, exclusion
                ));
            }
        }
    }
    for candidate_domain in &candidate.domains {
        if previous
            .domains
            .iter()
            .find(|domain| domain.id == candidate_domain.id)
            != Some(candidate_domain)
        {
            diff.affected_domains.push(candidate_domain.id.clone());
        }
    }
    for rule in &candidate.dependency_rules {
        let was_allowed = previous.dependency_rules.iter().any(|previous_rule| {
            previous_rule.from == rule.from && previous_rule.to == rule.to && previous_rule.allowed
        });
        if rule.allowed && !was_allowed {
            diff.relaxations
                .push(format!("allowed dependency {} -> {}", rule.from, rule.to));
        }
    }
    diff.affected_profiles.sort();
    diff.affected_profiles.dedup();
    diff.affected_domains.sort();
    diff.affected_domains.dedup();
    diff.relaxations.sort();
    diff
}

fn collect_duplicate_ids<'a>(
    ids: impl Iterator<Item = &'a str>,
    kind: &str,
    diagnostics: &mut Vec<ArchitectureDiagnostic>,
) {
    let mut seen = BTreeSet::new();
    for id in ids {
        if id.trim().is_empty() || !seen.insert(id) {
            diagnostics.push(ArchitectureDiagnostic::policy(
                "architecture_policy.id_invalid",
                format!("{kind} ID {id:?} is empty or duplicated"),
                "Use a unique non-empty stable ID.",
            ));
        }
    }
}

fn dependency_cycle<'a>(edges: &BTreeMap<&'a str, Vec<&'a str>>) -> Option<Vec<&'a str>> {
    fn visit<'a>(
        node: &'a str,
        edges: &BTreeMap<&'a str, Vec<&'a str>>,
        visiting: &mut Vec<&'a str>,
        visited: &mut BTreeSet<&'a str>,
    ) -> Option<Vec<&'a str>> {
        if let Some(index) = visiting.iter().position(|current| *current == node) {
            let mut cycle = visiting[index..].to_vec();
            cycle.push(node);
            return Some(cycle);
        }
        if visited.contains(node) {
            return None;
        }
        visiting.push(node);
        for target in edges.get(node).into_iter().flatten() {
            if let Some(cycle) = visit(target, edges, visiting, visited) {
                return Some(cycle);
            }
        }
        visiting.pop();
        visited.insert(node);
        None
    }

    let mut visited = BTreeSet::new();
    for node in edges.keys() {
        if let Some(cycle) = visit(node, edges, &mut Vec::new(), &mut visited) {
            return Some(cycle);
        }
    }
    None
}

impl Ord for PolicyMode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}

impl PartialOrd for PolicyMode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_policy() -> ArchitecturePolicy {
        ArchitecturePolicy {
            schema_version: ARCHITECTURE_POLICY_SCHEMA_VERSION.to_string(),
            profiles: vec![
                profile("engine", PolicyMode::EngineStrict),
                profile("project-advisory", PolicyMode::ProjectAdvisory),
                profile("project-strict", PolicyMode::ProjectStrict),
            ],
            domains: vec![DomainPolicy {
                id: "tooling".to_string(),
                owner: "engine-quality".to_string(),
                include: vec!["crates/quality_gate/**".to_string()],
                facade_only: Vec::new(),
            }],
            dependency_rules: Vec::new(),
            review_authorities: vec!["local-maintainer".to_string()],
        }
    }

    fn profile(id: &str, mode: PolicyMode) -> PolicyProfile {
        PolicyProfile {
            id: id.to_string(),
            mode,
            include: vec!["crates/**".to_string()],
            exclude: vec!["target/**".to_string()],
        }
    }

    #[test]
    fn architecture_policy_rejects_unknown_fields() {
        let mut source = toml::to_string(&valid_policy()).unwrap();
        source.push_str("unknown = true\n");
        assert!(parse_policy(&source).is_err());
    }

    #[test]
    fn architecture_policy_reports_relaxations() {
        let previous = valid_policy();
        let mut candidate = previous.clone();
        candidate.profiles[0]
            .exclude
            .push("crates/legacy/**".to_string());
        let diff = diff_policy(&previous, &candidate);
        assert!(diff.requires_review());
        assert_eq!(diff.affected_profiles, vec!["engine"]);
    }
}
