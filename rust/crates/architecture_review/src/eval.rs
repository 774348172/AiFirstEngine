use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const ARCHITECTURE_EVAL_CORPUS_SCHEMA_VERSION: &str = "architecture-eval-corpus.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchitectureEvalCorpus {
    pub schema_version: String,
    pub cases: Vec<ArchitectureEvalCase>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchitectureEvalCase {
    pub id: String,
    pub category: String,
    pub expected_blocking: bool,
    pub observed_blocking: bool,
    pub finding_ids: Vec<String>,
    pub coverage_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchitectureEvalReport {
    pub schema_version: String,
    pub cases: usize,
    pub blocking_precision: f64,
    pub finding_stability_digest: String,
    pub coverage_ratio: f64,
    pub false_positive_ids: Vec<String>,
    pub false_negative_ids: Vec<String>,
}

pub fn read_corpus(source: &str) -> Result<ArchitectureEvalCorpus, String> {
    let corpus: ArchitectureEvalCorpus =
        serde_json::from_str(source).map_err(|error| error.to_string())?;
    if corpus.schema_version != ARCHITECTURE_EVAL_CORPUS_SCHEMA_VERSION {
        return Err(format!(
            "unsupported eval corpus schema {:?}",
            corpus.schema_version
        ));
    }
    let mut ids = BTreeSet::new();
    for case in &corpus.cases {
        if case.id.trim().is_empty()
            || case.category.trim().is_empty()
            || !ids.insert(case.id.as_str())
        {
            return Err(format!("invalid or duplicate eval case {:?}", case.id));
        }
    }
    Ok(corpus)
}

pub fn evaluate_corpus(corpus: &ArchitectureEvalCorpus) -> ArchitectureEvalReport {
    let mut true_positive = 0_usize;
    let mut false_positive_ids = Vec::new();
    let mut false_negative_ids = Vec::new();
    let mut stable = Vec::new();
    let mut coverage_complete = 0_usize;
    for case in &corpus.cases {
        match (case.expected_blocking, case.observed_blocking) {
            (true, true) => true_positive += 1,
            (false, true) => false_positive_ids.push(case.id.clone()),
            (true, false) => false_negative_ids.push(case.id.clone()),
            (false, false) => {}
        }
        if case.coverage_complete {
            coverage_complete += 1;
        }
        let mut finding_ids = case.finding_ids.clone();
        finding_ids.sort();
        stable.push(format!(
            "{}:{}:{}",
            case.id,
            case.observed_blocking,
            finding_ids.join(",")
        ));
    }
    stable.sort();
    let predicted_blocking = true_positive + false_positive_ids.len();
    let blocking_precision = if predicted_blocking == 0 {
        1.0
    } else {
        true_positive as f64 / predicted_blocking as f64
    };
    let coverage_ratio = if corpus.cases.is_empty() {
        0.0
    } else {
        coverage_complete as f64 / corpus.cases.len() as f64
    };
    let hash = Sha256::digest(stable.join("\n").as_bytes());
    ArchitectureEvalReport {
        schema_version: "architecture-eval-report.v1".to_string(),
        cases: corpus.cases.len(),
        blocking_precision,
        finding_stability_digest: format!("sha256:{hash:x}"),
        coverage_ratio,
        false_positive_ids,
        false_negative_ids,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn architecture_eval_corpus_tracks_precision_stability_and_coverage() {
        let source = include_str!("../../../quality/architecture-eval-corpus.v1.json");
        let corpus = read_corpus(source).unwrap();
        let report = evaluate_corpus(&corpus);
        assert_eq!(report.cases, 8);
        assert_eq!(report.blocking_precision, 1.0);
        assert!(report.false_positive_ids.is_empty());
        assert!(report.false_negative_ids.is_empty());
        assert!(report.finding_stability_digest.starts_with("sha256:"));
    }
}
