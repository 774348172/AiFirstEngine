use crate::{
    build_recommendations, collect_rust_file_stats, normalize_path, HygieneRecommendation,
    HygieneReport,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cmp::Reverse;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const HYGIENE_REPORT_V2_SCHEMA_VERSION: &str = "code_hygiene.report.v2";
pub const HYGIENE_REPORT_V1_SCHEMA_VERSION: &str = "code_hygiene.report.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HygieneReportV2 {
    pub schema_version: String,
    pub root: String,
    pub files: usize,
    pub total_lines: usize,
    pub over_1000: usize,
    pub over_2000: usize,
    pub over_4000: usize,
    pub file_evidence: Vec<HygieneFileEvidence>,
    pub largest_files: Vec<HygieneFileStatV2>,
    pub hotspots: Vec<HygieneFileStatV2>,
    pub recommendations: Vec<HygieneRecommendationV2>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HygieneFileEvidence {
    pub path: String,
    pub lines: usize,
    pub fingerprint: String,
    pub content_digest: String,
    pub risk_band: HygieneRiskBand,
    pub domain: String,
    pub owner: String,
    pub baseline_delta: i64,
    pub changed_classification: ChangedClassification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HygieneRiskBand {
    Normal,
    Review,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangedClassification {
    Unchanged,
    NonSemantic,
    Semantic,
    ReviewRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HygieneFileStatV2 {
    pub path: String,
    pub lines: usize,
    pub risk_band: HygieneRiskBand,
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HygieneRecommendationV2 {
    pub code: String,
    pub severity: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HygieneReportDocument {
    V1(HygieneReportSummary),
    V2(HygieneReportV2),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HygieneReportSummary {
    pub schema_version: String,
    pub files: usize,
    pub total_lines: usize,
    pub recommendation_count: usize,
}

pub fn generate_hygiene_report_v2(root: impl AsRef<Path>) -> io::Result<HygieneReportV2> {
    let root = root.as_ref();
    let mut stats = Vec::new();
    collect_rust_file_stats(root, root, &mut stats)?;
    stats.sort_by_key(|stat| Reverse(stat.lines));
    let files = stats.len();
    let total_lines = stats.iter().map(|stat| stat.lines).sum();
    let over_1000 = stats.iter().filter(|stat| stat.lines > 1000).count();
    let over_2000 = stats.iter().filter(|stat| stat.lines > 2000).count();
    let over_4000 = stats.iter().filter(|stat| stat.lines > 4000).count();
    let recommendations = build_recommendations(over_1000, over_2000, over_4000, &stats)
        .into_iter()
        .map(recommendation_v2)
        .collect();
    let mut file_evidence = Vec::with_capacity(stats.len());
    for stat in &stats {
        let bytes = fs::read(root.join(&stat.path))?;
        let content_digest = digest(&bytes);
        file_evidence.push(HygieneFileEvidence {
            path: stat.path.clone(),
            lines: stat.lines,
            fingerprint: content_digest.clone(),
            content_digest,
            risk_band: risk_band(stat.lines),
            domain: domain_for_path(&stat.path),
            owner: owner_for_path(&stat.path),
            baseline_delta: 0,
            changed_classification: ChangedClassification::Unchanged,
        });
    }
    let largest_files = file_evidence.iter().take(30).map(stat_v2).collect();
    let hotspots = file_evidence
        .iter()
        .filter(|file| file.lines > 1000)
        .map(stat_v2)
        .collect();
    Ok(HygieneReportV2 {
        schema_version: HYGIENE_REPORT_V2_SCHEMA_VERSION.to_string(),
        root: normalize_path(root),
        files,
        total_lines,
        over_1000,
        over_2000,
        over_4000,
        file_evidence,
        largest_files,
        hotspots,
        recommendations,
    })
}

pub fn report_v2_to_json(report: &HygieneReportV2) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(report).map(|mut json| {
        json.push('\n');
        json
    })
}

pub fn write_report_v2_json(report: &HygieneReportV2, path: impl Into<PathBuf>) -> io::Result<()> {
    let path = path.into();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = report_v2_to_json(report).map_err(io::Error::other)?;
    fs::write(path, json)
}

pub fn read_report_json(source: &str) -> Result<HygieneReportDocument, String> {
    let value: serde_json::Value =
        serde_json::from_str(source).map_err(|error| error.to_string())?;
    let schema = value
        .get("schema_version")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "hygiene report is missing schema_version".to_string())?;
    match schema {
        HYGIENE_REPORT_V2_SCHEMA_VERSION => serde_json::from_value(value)
            .map(HygieneReportDocument::V2)
            .map_err(|error| error.to_string()),
        HYGIENE_REPORT_V1_SCHEMA_VERSION => {
            let files = required_usize(&value, "files")?;
            let total_lines = required_usize(&value, "total_lines")?;
            let recommendation_count = value
                .get("recommendations")
                .and_then(serde_json::Value::as_array)
                .map_or(0, Vec::len);
            Ok(HygieneReportDocument::V1(HygieneReportSummary {
                schema_version: schema.to_string(),
                files,
                total_lines,
                recommendation_count,
            }))
        }
        other => Err(format!("unsupported hygiene report schema {other:?}")),
    }
}

pub fn legacy_summary(report: &HygieneReport) -> HygieneReportSummary {
    HygieneReportSummary {
        schema_version: HYGIENE_REPORT_V1_SCHEMA_VERSION.to_string(),
        files: report.files,
        total_lines: report.total_lines,
        recommendation_count: report.recommendations.len(),
    }
}

fn required_usize(value: &serde_json::Value, key: &str) -> Result<usize, String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .and_then(|number| usize::try_from(number).ok())
        .ok_or_else(|| format!("hygiene report field {key:?} is missing or invalid"))
}

fn stat_v2(file: &HygieneFileEvidence) -> HygieneFileStatV2 {
    HygieneFileStatV2 {
        path: file.path.clone(),
        lines: file.lines,
        risk_band: file.risk_band,
        fingerprint: file.fingerprint.clone(),
    }
}

fn recommendation_v2(value: HygieneRecommendation) -> HygieneRecommendationV2 {
    HygieneRecommendationV2 {
        code: value.code,
        severity: value.severity,
        message: value.message,
    }
}

fn risk_band(lines: usize) -> HygieneRiskBand {
    match lines {
        0..=1000 => HygieneRiskBand::Normal,
        1001..=2000 => HygieneRiskBand::Review,
        2001..=4000 => HygieneRiskBand::High,
        _ => HygieneRiskBand::Critical,
    }
}

fn domain_for_path(path: &str) -> String {
    path.split('/').next().unwrap_or("unknown").to_string()
}

fn owner_for_path(path: &str) -> String {
    format!("workspace:{}", domain_for_path(path))
}

fn digest(bytes: &[u8]) -> String {
    let hash = Sha256::digest(bytes);
    format!("sha256:{hash:x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn v2_emits_file_fingerprint_risk_and_owner() {
        let root = temp_root();
        fs::create_dir_all(root.join("editor_core/src")).unwrap();
        fs::write(root.join("editor_core/src/lib.rs"), "x\n".repeat(2001)).unwrap();
        let report = generate_hygiene_report_v2(&root).unwrap();
        let file = &report.file_evidence[0];
        assert_eq!(file.risk_band, HygieneRiskBand::High);
        assert_eq!(file.domain, "editor_core");
        assert!(file.fingerprint.starts_with("sha256:"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn v1_reader_remains_compatible() {
        let source = r#"{"schema_version":"code_hygiene.report.v1","files":2,"total_lines":4,"recommendations":[]}"#;
        let document = read_report_json(source).unwrap();
        assert!(matches!(document, HygieneReportDocument::V1(_)));
    }

    fn temp_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("code_hygiene_v2_{nanos}"))
    }
}
