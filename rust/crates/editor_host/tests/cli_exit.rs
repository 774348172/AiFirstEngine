use std::process::Command;

#[test]
fn invalid_isolated_project_launch_profile_exits_nonzero() {
    let output = Command::new(env!("CARGO_BIN_EXE_editor_host"))
        .args(["--real-window", "--isolated-project-launch-root"])
        .output()
        .expect("run editor_host");

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("editor_host.isolated_project_launch_root_missing"));
    assert!(output.stdout.is_empty());
}
