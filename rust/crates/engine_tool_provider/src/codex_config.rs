use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

pub const CODEX_CONFIG_ARTIFACT_SCHEMA_VERSION: &str = "engine-tool-codex-config-artifact.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexConfigArtifactRequest {
    pub output_root: PathBuf,
    pub mcp_binary: PathBuf,
    pub workspace_root: PathBuf,
    pub project_root: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CodexConfigArtifactReceipt {
    pub schema_version: String,
    pub artifact_path: PathBuf,
    pub artifact_digest: String,
    pub mcp_binary: PathBuf,
    pub workspace_root: PathBuf,
    pub project_root: Option<PathBuf>,
    pub applied_to_user_config: bool,
}

pub fn generate_codex_config_artifact(
    request: &CodexConfigArtifactRequest,
) -> Result<CodexConfigArtifactReceipt, String> {
    let mcp_binary = canonical_file(&request.mcp_binary, "MCP binary")?;
    let workspace_root = canonical_directory(&request.workspace_root, "workspace root")?;
    let project_root = request
        .project_root
        .as_ref()
        .map(|path| canonical_directory(path, "project root"))
        .transpose()?;
    if project_root
        .as_ref()
        .is_some_and(|project| !project.starts_with(&workspace_root))
    {
        return Err("Project root must be contained by the workspace root.".to_string());
    }
    fs::create_dir_all(&request.output_root).map_err(|error| error.to_string())?;
    let output_root = request
        .output_root
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let artifact_path = output_root.join("ai-first-game-engine-mcp.toml");
    let mut args = vec![
        "--workspace-root".to_string(),
        workspace_root.display().to_string(),
    ];
    if let Some(project_root) = &project_root {
        args.push("--project-root".to_string());
        args.push(project_root.display().to_string());
    }
    let args = args
        .iter()
        .map(|argument| toml_string(argument))
        .collect::<Vec<_>>()
        .join(", ");
    let body = format!(
        "[mcp_servers.ai_first_game_engine]\ncommand = {}\nargs = [{}]\n",
        toml_string(&mcp_binary.display().to_string()),
        args
    );
    fs::write(&artifact_path, body.as_bytes()).map_err(|error| error.to_string())?;
    Ok(CodexConfigArtifactReceipt {
        schema_version: CODEX_CONFIG_ARTIFACT_SCHEMA_VERSION.to_string(),
        artifact_path,
        artifact_digest: sha256_prefixed(body.as_bytes()),
        mcp_binary,
        workspace_root,
        project_root,
        applied_to_user_config: false,
    })
}

fn canonical_file(path: &Path, role: &str) -> Result<PathBuf, String> {
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("{role} cannot be resolved: {error}"))?;
    if !canonical.is_file() {
        return Err(format!("{role} is not a file."));
    }
    Ok(canonical)
}

fn canonical_directory(path: &Path, role: &str) -> Result<PathBuf, String> {
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("{role} cannot be resolved: {error}"))?;
    if !canonical.is_dir() {
        return Err(format!("{role} is not a directory."));
    }
    Ok(canonical)
}

fn toml_string(value: &str) -> String {
    serde_json::to_string(value).expect("string must serialize")
}

fn sha256_prefixed(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("sha256:{digest:x}")
}
