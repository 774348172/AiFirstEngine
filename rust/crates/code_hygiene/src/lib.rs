use std::cmp::Reverse;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub mod v2;

pub const DEFAULT_LARGEST_FILE_LIMIT: usize = 30;

const HOTSPOT_SUFFIXES: &[&str] = &[
    "editor_core/src/lib.rs",
    "editor_window_winit/src/lib.rs",
    "editor_ui_model/src/lib.rs",
    "editor_ui_renderer/src/lib.rs",
    "editor_wgpu_renderer/src/lib.rs",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HygieneFileStat {
    pub path: String,
    pub lines: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HygieneRecommendation {
    pub code: String,
    pub severity: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HygieneReport {
    pub root: String,
    pub files: usize,
    pub total_lines: usize,
    pub over_1000: usize,
    pub over_2000: usize,
    pub over_4000: usize,
    pub largest_files: Vec<HygieneFileStat>,
    pub hotspots: Vec<HygieneFileStat>,
    pub recommendations: Vec<HygieneRecommendation>,
}

pub fn generate_hygiene_report(root: impl AsRef<Path>) -> io::Result<HygieneReport> {
    generate_hygiene_report_with_limit(root, DEFAULT_LARGEST_FILE_LIMIT)
}

pub fn generate_hygiene_report_with_limit(
    root: impl AsRef<Path>,
    largest_file_limit: usize,
) -> io::Result<HygieneReport> {
    let root = root.as_ref();
    let root_display = normalize_path(root);
    let mut stats = Vec::new();
    collect_rust_file_stats(root, root, &mut stats)?;
    stats.sort_by_key(|stat| Reverse(stat.lines));

    let files = stats.len();
    let total_lines = stats.iter().map(|stat| stat.lines).sum();
    let over_1000 = stats.iter().filter(|stat| stat.lines > 1000).count();
    let over_2000 = stats.iter().filter(|stat| stat.lines > 2000).count();
    let over_4000 = stats.iter().filter(|stat| stat.lines > 4000).count();
    let largest_files = stats.iter().take(largest_file_limit).cloned().collect();
    let hotspots = stats
        .iter()
        .filter(|stat| {
            HOTSPOT_SUFFIXES
                .iter()
                .any(|suffix| path_has_suffix(&stat.path, suffix))
        })
        .cloned()
        .collect();

    let recommendations = build_recommendations(over_1000, over_2000, over_4000, &stats);

    Ok(HygieneReport {
        root: root_display,
        files,
        total_lines,
        over_1000,
        over_2000,
        over_4000,
        largest_files,
        hotspots,
        recommendations,
    })
}

pub fn report_to_json(report: &HygieneReport) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    push_json_field(
        &mut out,
        1,
        "schema_version",
        "\"code_hygiene.report.v1\"",
        true,
    );
    push_json_field(&mut out, 1, "root", &json_string(&report.root), true);
    push_json_field(&mut out, 1, "files", &report.files.to_string(), true);
    push_json_field(
        &mut out,
        1,
        "total_lines",
        &report.total_lines.to_string(),
        true,
    );
    push_json_field(
        &mut out,
        1,
        "over_1000",
        &report.over_1000.to_string(),
        true,
    );
    push_json_field(
        &mut out,
        1,
        "over_2000",
        &report.over_2000.to_string(),
        true,
    );
    push_json_field(
        &mut out,
        1,
        "over_4000",
        &report.over_4000.to_string(),
        true,
    );
    push_file_array(&mut out, 1, "largest_files", &report.largest_files, true);
    push_file_array(&mut out, 1, "hotspots", &report.hotspots, true);
    push_recommendation_array(
        &mut out,
        1,
        "recommendations",
        &report.recommendations,
        false,
    );
    out.push_str("}\n");
    out
}

pub(crate) fn collect_rust_file_stats(
    root: &Path,
    current: &Path,
    stats: &mut Vec<HygieneFileStat>,
) -> io::Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if should_skip_dir(&path) {
                continue;
            }
            collect_rust_file_stats(root, &path, stats)?;
        } else if file_type.is_file() && path.extension().is_some_and(|ext| ext == "rs") {
            let content = fs::read_to_string(&path)?;
            let lines = content.lines().count();
            let display_path = path
                .strip_prefix(root)
                .map(normalize_path)
                .unwrap_or_else(|_| normalize_path(&path));
            stats.push(HygieneFileStat {
                path: display_path,
                lines,
            });
        }
    }
    Ok(())
}

fn should_skip_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| matches!(name, "target" | ".git"))
}

pub(crate) fn build_recommendations(
    over_1000: usize,
    over_2000: usize,
    over_4000: usize,
    stats: &[HygieneFileStat],
) -> Vec<HygieneRecommendation> {
    let mut recommendations = Vec::new();
    if over_4000 > 0 {
        recommendations.push(HygieneRecommendation {
            code: "large_facade_required".to_string(),
            severity: "warning".to_string(),
            message: "Files over 4000 lines should become facade files or be split by domain."
                .to_string(),
        });
    }
    if over_2000 > 0 {
        recommendations.push(HygieneRecommendation {
            code: "domain_split_required".to_string(),
            severity: "warning".to_string(),
            message: "Files over 2000 lines need an explicit domain split plan.".to_string(),
        });
    }
    if over_1000 > 10 {
        recommendations.push(HygieneRecommendation {
            code: "test_migration_required".to_string(),
            severity: "info".to_string(),
            message:
                "Many files exceed 1000 lines; keep tests while moving them to domain modules."
                    .to_string(),
        });
    }
    if stats
        .iter()
        .any(|stat| path_has_suffix(&stat.path, "editor_core/src/lib.rs"))
    {
        recommendations.push(HygieneRecommendation {
            code: "editor_session_service_split".to_string(),
            severity: "warning".to_string(),
            message: "editor_core/src/lib.rs should stop accepting new domain implementations."
                .to_string(),
        });
    }
    recommendations
}

fn path_has_suffix(path: &str, suffix: &str) -> bool {
    path.replace('\\', "/")
        .ends_with(&suffix.replace('\\', "/"))
}

pub(crate) fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn push_json_field(out: &mut String, indent: usize, key: &str, value: &str, comma: bool) {
    push_indent(out, indent);
    out.push('"');
    out.push_str(key);
    out.push_str("\": ");
    out.push_str(value);
    if comma {
        out.push(',');
    }
    out.push('\n');
}

fn push_file_array(
    out: &mut String,
    indent: usize,
    key: &str,
    files: &[HygieneFileStat],
    comma: bool,
) {
    push_indent(out, indent);
    out.push('"');
    out.push_str(key);
    out.push_str("\": [\n");
    for (index, file) in files.iter().enumerate() {
        push_indent(out, indent + 1);
        out.push_str("{ \"path\": ");
        out.push_str(&json_string(&file.path));
        out.push_str(", \"lines\": ");
        out.push_str(&file.lines.to_string());
        out.push_str(" }");
        if index + 1 != files.len() {
            out.push(',');
        }
        out.push('\n');
    }
    push_indent(out, indent);
    out.push(']');
    if comma {
        out.push(',');
    }
    out.push('\n');
}

fn push_recommendation_array(
    out: &mut String,
    indent: usize,
    key: &str,
    recommendations: &[HygieneRecommendation],
    comma: bool,
) {
    push_indent(out, indent);
    out.push('"');
    out.push_str(key);
    out.push_str("\": [\n");
    for (index, recommendation) in recommendations.iter().enumerate() {
        push_indent(out, indent + 1);
        out.push_str("{ \"code\": ");
        out.push_str(&json_string(&recommendation.code));
        out.push_str(", \"severity\": ");
        out.push_str(&json_string(&recommendation.severity));
        out.push_str(", \"message\": ");
        out.push_str(&json_string(&recommendation.message));
        out.push_str(" }");
        if index + 1 != recommendations.len() {
            out.push(',');
        }
        out.push('\n');
    }
    push_indent(out, indent);
    out.push(']');
    if comma {
        out.push(',');
    }
    out.push('\n');
}

fn push_indent(out: &mut String, indent: usize) {
    for _ in 0..indent {
        out.push_str("  ");
    }
}

fn json_string(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

pub fn write_report_json(report: &HygieneReport, path: impl Into<PathBuf>) -> io::Result<()> {
    let path = path.into();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, report_to_json(report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn hygiene_report_counts_rust_files_and_thresholds() {
        let root = temp_root("counts");
        fs::create_dir_all(root.join("crate_a/src")).unwrap();
        fs::write(root.join("crate_a/src/lib.rs"), "a\nb\n").unwrap();
        fs::write(root.join("crate_a/src/big.rs"), "x\n".repeat(1001)).unwrap();

        let report = generate_hygiene_report_with_limit(&root, 10).unwrap();

        assert_eq!(report.files, 2);
        assert_eq!(report.total_lines, 1003);
        assert_eq!(report.over_1000, 1);
        assert_eq!(report.over_2000, 0);
        assert_eq!(report.largest_files[0].path, "crate_a/src/big.rs");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn hygiene_report_detects_hotspots_by_suffix() {
        let root = temp_root("hotspots");
        let hotspot = root.join("editor_core/src");
        fs::create_dir_all(&hotspot).unwrap();
        fs::write(hotspot.join("lib.rs"), "x\n").unwrap();

        let report = generate_hygiene_report(&root).unwrap();

        assert_eq!(report.hotspots.len(), 1);
        assert_eq!(report.hotspots[0].path, "editor_core/src/lib.rs");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn report_json_contains_stable_schema_and_recommendations() {
        let report = HygieneReport {
            root: "root".to_string(),
            files: 1,
            total_lines: 4001,
            over_1000: 1,
            over_2000: 1,
            over_4000: 1,
            largest_files: vec![HygieneFileStat {
                path: "editor_core/src/lib.rs".to_string(),
                lines: 4001,
            }],
            hotspots: vec![HygieneFileStat {
                path: "editor_core/src/lib.rs".to_string(),
                lines: 4001,
            }],
            recommendations: build_recommendations(
                1,
                1,
                1,
                &[HygieneFileStat {
                    path: "editor_core/src/lib.rs".to_string(),
                    lines: 4001,
                }],
            ),
        };

        let json = report_to_json(&report);

        assert!(json.contains("\"schema_version\": \"code_hygiene.report.v1\""));
        assert!(json.contains("\"large_facade_required\""));
        assert!(json.contains("\"editor_session_service_split\""));
    }

    fn temp_root(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("code_hygiene_{label}_{nanos}"))
    }
}
