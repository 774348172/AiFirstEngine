// Frozen context.rs sha256:370260324570e88b439d96056178e32b07d47ba3d3a4419053c72a1c8695c7f4; original header and open schema rejection branch.
#[path = "old_context_diagnostic.rs"]
mod diagnostic;
use diagnostic::{ContextError, DiagnosticStage};
use serde::Deserialize;
const PROJECT_MANIFEST_SCHEMA_VERSION: &str = "aife-project.v2";
#[derive(Debug, Deserialize)]
struct ProjectManifestHeader {
    #[serde(rename = "schemaVersion")]
    schema_version: String,
    #[serde(rename = "projectId")]
    project_id: String,
}

pub fn open_guard(bytes: &[u8]) -> Result<(), ContextError> {
    let manifest: ProjectManifestHeader = serde_json::from_slice(bytes).unwrap();
    let manifest_path = std::path::Path::new("project.aife.json");
    if manifest.schema_version != PROJECT_MANIFEST_SCHEMA_VERSION {
        return Err(ContextError::new(
            "authoring_context.manifest_schema_unsupported",
            format!(
                "Unsupported project manifest schema: {}",
                manifest.schema_version
            ),
            DiagnosticStage::Open,
            Some(manifest_path.display().to_string()),
            format!("Migrate the manifest to {PROJECT_MANIFEST_SCHEMA_VERSION}."),
        ));
    }
    let _ = manifest.project_id;
    Ok(())
}
