use std::fs;
use std::path::Path;

#[test]
fn github_adapter_is_thin_read_only_and_bounded() {
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let workflow_path = repository_root.join(".github/workflows/rust-quality.yml");
    let workflow = fs::read_to_string(workflow_path)
        .unwrap()
        .replace("\r\n", "\n");

    assert!(workflow.contains("permissions:\n  contents: read"));
    assert!(workflow.contains("persist-credentials: false"));
    assert!(workflow.contains("runs-on: windows-latest"));
    assert!(workflow.contains("timeout-minutes:"));
    assert!(workflow.contains("cancel-in-progress: true"));
    assert!(workflow.contains("actions/upload-artifact@v4"));
    assert!(workflow.contains("cargo run -p quality_gate --locked -- verify"));
    assert_eq!(workflow.matches("cargo run -p quality_gate").count(), 1);

    for forbidden in [
        "cargo fmt",
        "cargo clippy",
        "cargo test",
        "permissions: write",
        "secrets.",
        "publish",
        "deploy",
    ] {
        assert!(
            !workflow.contains(forbidden),
            "thin CI Adapter contains forbidden contract fragment {forbidden:?}"
        );
    }

    let local_ci = include_str!("../src/local_ci.rs");
    assert!(local_ci.contains("git_worktree_add"));
    assert!(local_ci.contains("git_worktree_remove"));
    assert!(local_ci.contains("QUALITY_GATE_CI_ADAPTER"));
    assert!(local_ci.contains("local-git-worktree"));
}
