use engine_tool_provider::codex_config::{
    generate_codex_config_artifact, CodexConfigArtifactRequest,
    CODEX_CONFIG_ARTIFACT_SCHEMA_VERSION,
};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn isolated_codex_config_generates_an_unapplied_headless_mcp_artifact() {
    let root = std::env::temp_dir().join(format!(
        "aife-codex-config-artifact-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let workspace = root.join("workspace");
    let project = workspace.join("game");
    let binary = root.join("bin/ai_engine_tool_provider_mcp.exe");
    let output = root.join("artifacts");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(binary.parent().unwrap()).unwrap();
    fs::write(&binary, b"fixture-binary").unwrap();

    let receipt = generate_codex_config_artifact(&CodexConfigArtifactRequest {
        output_root: output.clone(),
        mcp_binary: binary,
        workspace_root: workspace,
        project_root: Some(project),
    })
    .unwrap();

    assert_eq!(receipt.schema_version, CODEX_CONFIG_ARTIFACT_SCHEMA_VERSION);
    assert!(!receipt.applied_to_user_config);
    assert!(receipt
        .artifact_path
        .starts_with(output.canonicalize().unwrap()));
    let config = fs::read_to_string(&receipt.artifact_path).unwrap();
    assert!(config.contains("[mcp_servers.ai_first_game_engine]"));
    assert!(config.contains("ai_engine_tool_provider_mcp.exe"));
    assert!(config.contains("--workspace-root"));
    assert!(config.contains("--project-root"));
    assert!(!config.contains("gateway"));
    assert!(!config.contains("editor"));
    assert!(receipt.artifact_digest.starts_with("sha256:"));

    let _ = fs::remove_dir_all(root);
}
