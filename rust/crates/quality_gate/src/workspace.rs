use crate::cargo_json::sha256_hex;
use crate::report::QualityDiagnostic;
use serde_json::Value;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub fn file_digest(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(format!("sha256:{}", sha256_hex(&bytes)))
}

pub fn combined_digest(values: &[&str]) -> String {
    format!("sha256:{}", sha256_hex(values.join("\n").as_bytes()))
}

pub struct WorkspaceAudit {
    pub manifest_set_digest: String,
    pub diagnostics: Vec<QualityDiagnostic>,
}

pub fn audit_workspace_lints(
    workspace_root: &Path,
    metadata_output: &[u8],
) -> Result<WorkspaceAudit, String> {
    let metadata: Value =
        serde_json::from_slice(metadata_output).map_err(|error| error.to_string())?;
    let packages = metadata
        .get("packages")
        .and_then(Value::as_array)
        .ok_or_else(|| "cargo metadata output is missing packages".to_string())?;
    let mut manifests = packages
        .iter()
        .map(|package| {
            package
                .get("manifest_path")
                .and_then(Value::as_str)
                .map(PathBuf::from)
                .ok_or_else(|| "cargo metadata package is missing manifest_path".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    manifests.sort();

    let root_manifest =
        fs::read_to_string(workspace_root.join("Cargo.toml")).map_err(|error| error.to_string())?;
    let root_toml: toml::Value =
        toml::from_str(&root_manifest).map_err(|error| error.to_string())?;
    let mut diagnostics = Vec::new();
    if root_toml
        .get("workspace")
        .and_then(|workspace| workspace.get("lints"))
        .is_none()
    {
        diagnostics.push(diagnostic(
            "quality_gate.workspace_lint_not_inherited",
            "root Cargo.toml does not define [workspace.lints]",
            "Define the shared lint policy in the workspace root.",
        ));
    }

    let mut canonical = Vec::new();
    for manifest in manifests {
        let content = fs::read_to_string(&manifest).map_err(|error| error.to_string())?;
        let parsed: toml::Value = toml::from_str(&content).map_err(|error| {
            format!(
                "failed to parse {} while auditing lints: {error}",
                manifest.display()
            )
        })?;
        let inherited = parsed
            .get("lints")
            .and_then(|lints| lints.get("workspace"))
            .and_then(toml::Value::as_bool)
            == Some(true);
        let relative = manifest
            .strip_prefix(workspace_root)
            .unwrap_or(&manifest)
            .to_string_lossy()
            .replace('\\', "/");
        if !inherited {
            diagnostics.push(diagnostic(
                "quality_gate.workspace_lint_not_inherited",
                format!("{relative} does not declare [lints] workspace = true"),
                "Make this workspace member inherit the root lint policy.",
            ));
        }
        canonical.push(format!("{relative}\n{}", sha256_hex(content.as_bytes())));
    }

    Ok(WorkspaceAudit {
        manifest_set_digest: format!("sha256:{}", sha256_hex(canonical.join("\n").as_bytes())),
        diagnostics,
    })
}

pub fn resolve_report_path(
    workspace_root: &Path,
    requested: Option<&Path>,
) -> Result<PathBuf, String> {
    let path = requested
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/quality-gate/quality-gate-report.v2.json"));
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err("report path may not contain parent-directory traversal".to_string());
    }
    let absolute = if path.is_absolute() {
        path
    } else {
        workspace_root.join(path)
    };
    let target = workspace_root.join("target");
    if !absolute.starts_with(&target) {
        return Err("report output must be inside the workspace target directory".to_string());
    }
    Ok(absolute)
}

fn diagnostic(
    code: impl Into<String>,
    message: impl Into<String>,
    next_action: impl Into<String>,
) -> QualityDiagnostic {
    QualityDiagnostic {
        code: code.into(),
        message: message.into(),
        next_action: next_action.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_path_is_confined_to_target() {
        let root = Path::new("workspace");
        assert!(resolve_report_path(root, Some(Path::new("target/report.json"))).is_ok());
        assert!(resolve_report_path(root, Some(Path::new("../report.json"))).is_err());
        assert!(resolve_report_path(root, Some(Path::new("report.json"))).is_err());
    }
}
