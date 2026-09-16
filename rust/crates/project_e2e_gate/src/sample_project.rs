use crate::report::ComplexProjectE2eDiagnostic;
use editor_core::EditorSceneDocument;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SampleProjectSummary {
    pub project_root: PathBuf,
    pub project_name: String,
    pub default_scene: String,
    pub scene_count: usize,
    pub entity_count: usize,
    pub prefab_count: usize,
    pub asset_count: usize,
    pub rule_count: usize,
    pub input_action_count: usize,
    pub aui_document_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectManifestView {
    project_name: String,
    default_scene: String,
}

pub fn load_sample_project_summary(
    project_root: impl AsRef<Path>,
) -> Result<SampleProjectSummary, Vec<ComplexProjectE2eDiagnostic>> {
    let project_root = project_root.as_ref();
    let mut diagnostics = Vec::new();

    let manifest_path = project_root.join("project.aife.json");
    let manifest_text = match fs::read_to_string(&manifest_path) {
        Ok(text) => text,
        Err(error) => {
            return Err(vec![ComplexProjectE2eDiagnostic::error(
                "SampleProjectManifestReadFailed",
                format!("failed to read project manifest: {error}"),
            )
            .with_path(manifest_path.display().to_string())]);
        }
    };
    let manifest = match serde_json::from_str::<ProjectManifestView>(&manifest_text) {
        Ok(manifest) => manifest,
        Err(error) => {
            return Err(vec![ComplexProjectE2eDiagnostic::error(
                "SampleProjectManifestParseFailed",
                format!("failed to parse project manifest: {error}"),
            )
            .with_path(manifest_path.display().to_string())]);
        }
    };

    let scene_count = count_json_files(&project_root.join("Scenes"));
    let prefab_count = count_json_files(&project_root.join("Prefabs"));
    let asset_count = count_files(&project_root.join("Assets"));
    let aui_document_count = count_json_files(&project_root.join("AUI"));

    let scene_path = project_root.join(&manifest.default_scene);
    let scene = EditorSceneDocument::load_from_path(&scene_path).map_err(|scene_diagnostics| {
        scene_diagnostics
            .into_iter()
            .map(|diagnostic| {
                ComplexProjectE2eDiagnostic::error(diagnostic.code, diagnostic.message)
                    .with_path(scene_path.display().to_string())
            })
            .collect::<Vec<_>>()
    })?;
    let entity_count = scene.entities.len();

    let rule_count = read_json_value(
        project_root.join("Rules").join("rule-manifest.json"),
        &mut diagnostics,
    )
    .and_then(|value| {
        value
            .get("rules")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len)
    })
    .unwrap_or_default();
    let input_action_count = read_json_value(
        project_root.join("Input").join("input.default.json"),
        &mut diagnostics,
    )
    .and_then(|value| {
        value
            .get("actions")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len)
    })
    .unwrap_or_default();

    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == "error")
    {
        return Err(diagnostics);
    }

    Ok(SampleProjectSummary {
        project_root: project_root.to_path_buf(),
        project_name: manifest.project_name,
        default_scene: manifest.default_scene,
        scene_count,
        entity_count,
        prefab_count,
        asset_count,
        rule_count,
        input_action_count,
        aui_document_count,
    })
}

fn read_json_value(
    path: PathBuf,
    diagnostics: &mut Vec<ComplexProjectE2eDiagnostic>,
) -> Option<serde_json::Value> {
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            diagnostics.push(
                ComplexProjectE2eDiagnostic::warning(
                    "SampleProjectOptionalJsonReadFailed",
                    format!("failed to read optional project json: {error}"),
                )
                .with_path(path.display().to_string()),
            );
            return None;
        }
    };
    match serde_json::from_str(&text) {
        Ok(value) => Some(value),
        Err(error) => {
            diagnostics.push(
                ComplexProjectE2eDiagnostic::error(
                    "SampleProjectJsonParseFailed",
                    format!("failed to parse project json: {error}"),
                )
                .with_path(path.display().to_string()),
            );
            None
        }
    }
}

fn count_json_files(path: &Path) -> usize {
    count_files_with(path, |path| {
        path.extension().is_some_and(|ext| ext == "json")
    })
}

fn count_files(path: &Path) -> usize {
    count_files_with(path, |_| true)
}

fn count_files_with(path: &Path, predicate: impl Fn(&Path) -> bool + Copy) -> usize {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .map(|path| {
            if path.is_dir() {
                count_files_with(&path, predicate)
            } else if predicate(&path) {
                1
            } else {
                0
            }
        })
        .sum()
}
