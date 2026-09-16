use code_hygiene::{generate_hygiene_report, report_to_json};
use std::path::PathBuf;

#[test]
fn workspace_report_includes_current_known_hotspots() {
    let root = workspace_crates_root();
    let report = generate_hygiene_report(&root).expect("workspace hygiene report");
    let json = report_to_json(&report);

    assert!(report.files >= 90, "expected real workspace rust files");
    assert!(
        report.total_lines >= 60_000,
        "expected current workspace size"
    );
    assert!(
        report
            .hotspots
            .iter()
            .any(|stat| stat.path == "editor_core/src/lib.rs"),
        "editor_core lib.rs should remain tracked until split"
    );
    assert!(json.contains("\"schema_version\": \"code_hygiene.report.v1\""));
}

fn workspace_crates_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace crates root")
        .to_path_buf()
}
