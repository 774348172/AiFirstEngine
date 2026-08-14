use crate::EditorSession;
use engine_runtime::canonical_digest::sha256_prefixed;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

pub const PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION: &str = "project-observation-input.v1";
pub const PROJECT_OBSERVATION_RESULT_SCHEMA_VERSION: &str = "project-observation-result.v1";
const MAX_INDEX_FILES: usize = 4096;
const MAX_TEXT_FILE_BYTES: u64 = 256 * 1024;
const MAX_PAGE_SIZE: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectSearchInput {
    pub schema_version: String,
    pub query: String,
    pub kinds: Vec<String>,
    pub continuation_token: Option<String>,
    pub page_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectObjectReadInput {
    pub schema_version: String,
    pub object_ref: String,
    pub max_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectReferencesInput {
    pub schema_version: String,
    pub symbol_or_value: String,
    pub continuation_token: Option<String>,
    pub page_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectSourceSymbolsInput {
    pub schema_version: String,
    pub query: String,
    pub continuation_token: Option<String>,
    pub page_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectDiagnosticsInput {
    pub schema_version: String,
    pub code_or_text: Option<String>,
    pub continuation_token: Option<String>,
    pub page_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectEvidenceReadInput {
    pub schema_version: String,
    pub evidence_ref: String,
    pub max_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectObservationItem {
    pub object_ref: String,
    pub kind: String,
    pub name: String,
    pub project_relative_path: String,
    pub summary: String,
    pub match_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectObservationPage {
    pub items: Vec<ProjectObservationItem>,
    pub next_continuation_token: Option<String>,
    pub scanned_file_count: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectObjectEvidence {
    pub object_ref: String,
    pub kind: String,
    pub project_relative_path: String,
    pub content_digest: String,
    pub content: Value,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectReferenceEvidence {
    pub object_ref: String,
    pub project_relative_path: String,
    pub line: usize,
    pub column: usize,
    pub preview: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectReferencePage {
    pub references: Vec<ProjectReferenceEvidence>,
    pub next_continuation_token: Option<String>,
    pub scanned_file_count: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectSourceSymbol {
    pub symbol_ref: String,
    pub name: String,
    pub symbol_kind: String,
    pub project_relative_path: String,
    pub line: usize,
    pub declaration: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectSourceSymbolPage {
    pub symbols: Vec<ProjectSourceSymbol>,
    pub next_continuation_token: Option<String>,
    pub scanned_file_count: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "observationKind",
    content = "observation",
    rename_all = "snake_case"
)]
pub enum ProjectObservationResult {
    Search(ProjectObservationPage),
    Object(ProjectObjectEvidence),
    References(ProjectReferencePage),
    SourceSymbols(ProjectSourceSymbolPage),
    Diagnostics(ProjectObservationPage),
    Evidence(ProjectObjectEvidence),
}

pub struct ProjectObservationIndex {
    root: PathBuf,
    files: Vec<IndexedFile>,
    truncated: bool,
}

struct IndexedFile {
    relative_path: String,
    kind: String,
    name: String,
}

impl ProjectObservationIndex {
    pub fn build(session: &EditorSession) -> Result<Self, String> {
        let project = session
            .active_project_session()
            .ok_or_else(|| "project_observation.no_active_project".to_string())?;
        let root = project
            .project_root
            .canonicalize()
            .map_err(|error| format!("project_observation.root_invalid: {error}"))?;
        let mut files = Vec::new();
        let mut truncated = false;
        for entry in [
            "project.aife.json",
            "Assets",
            "Scenes",
            "Prefabs",
            "AUI",
            "Rules",
            "Input",
            "Settings",
            "RuntimeModule",
        ] {
            collect_files(&root, &root.join(entry), &mut files, &mut truncated)?;
            if truncated {
                break;
            }
        }
        files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        files.dedup_by(|left, right| left.relative_path == right.relative_path);
        Ok(Self {
            root,
            files,
            truncated,
        })
    }

    pub fn search(&self, input: &ProjectSearchInput) -> Result<ProjectObservationPage, String> {
        validate_input(&input.schema_version, input.page_size)?;
        let query = input.query.trim().to_lowercase();
        if query.is_empty() {
            return Err("project_observation.query_empty".to_string());
        }
        let kinds = input
            .kinds
            .iter()
            .map(|kind| kind.to_lowercase())
            .collect::<BTreeSet<_>>();
        let mut items = Vec::new();
        for file in &self.files {
            if !kinds.is_empty() && !kinds.contains(&file.kind) {
                continue;
            }
            let mut match_fields = Vec::new();
            if file.name.to_lowercase().contains(&query) {
                match_fields.push("name".to_string());
            }
            if file.relative_path.to_lowercase().contains(&query) {
                match_fields.push("path".to_string());
            }
            let content = self.read_text(&file.relative_path).unwrap_or_default();
            if content.to_lowercase().contains(&query) {
                match_fields.push("content".to_string());
            }
            if match_fields.is_empty() {
                continue;
            }
            items.push(ProjectObservationItem {
                object_ref: object_ref(&file.relative_path),
                kind: file.kind.clone(),
                name: file.name.clone(),
                project_relative_path: file.relative_path.clone(),
                summary: summarize_text(&content, &query),
                match_fields,
            });
        }
        Ok(page_items(
            items,
            input.continuation_token.as_deref(),
            input.page_size,
            self.files.len(),
            self.truncated,
        ))
    }

    pub fn read_object(
        &self,
        input: &ProjectObjectReadInput,
    ) -> Result<ProjectObjectEvidence, String> {
        validate_schema(&input.schema_version)?;
        let max_bytes = input.max_bytes.clamp(1, MAX_TEXT_FILE_BYTES as usize);
        let relative = input
            .object_ref
            .strip_prefix("project-object:")
            .ok_or_else(|| "project_observation.object_ref_invalid".to_string())?;
        self.read_evidence(relative, max_bytes, false)
    }

    pub fn references(
        &self,
        input: &ProjectReferencesInput,
    ) -> Result<ProjectReferencePage, String> {
        validate_input(&input.schema_version, input.page_size)?;
        let needle = input.symbol_or_value.trim();
        if needle.is_empty() {
            return Err("project_observation.reference_empty".to_string());
        }
        let mut references = Vec::new();
        for file in &self.files {
            let content = self.read_text(&file.relative_path).unwrap_or_default();
            for (line_index, line) in content.lines().enumerate() {
                for (column, _) in line.match_indices(needle) {
                    references.push(ProjectReferenceEvidence {
                        object_ref: object_ref(&file.relative_path),
                        project_relative_path: file.relative_path.clone(),
                        line: line_index + 1,
                        column: column + 1,
                        preview: bounded_preview(line),
                    });
                }
            }
        }
        references.sort_by(|left, right| {
            (&left.project_relative_path, left.line, left.column).cmp(&(
                &right.project_relative_path,
                right.line,
                right.column,
            ))
        });
        let (references, next) = paginate(
            references,
            input.continuation_token.as_deref(),
            input.page_size,
        );
        Ok(ProjectReferencePage {
            references,
            next_continuation_token: next,
            scanned_file_count: self.files.len(),
            truncated: self.truncated,
        })
    }

    pub fn source_symbols(
        &self,
        input: &ProjectSourceSymbolsInput,
    ) -> Result<ProjectSourceSymbolPage, String> {
        validate_input(&input.schema_version, input.page_size)?;
        let query = input.query.trim().to_lowercase();
        let mut symbols = Vec::new();
        for file in self
            .files
            .iter()
            .filter(|file| file.relative_path.ends_with(".rs"))
        {
            let content = self.read_text(&file.relative_path).unwrap_or_default();
            collect_rust_symbols(&file.relative_path, &content, &query, &mut symbols);
        }
        symbols.sort_by(|left, right| {
            (&left.project_relative_path, left.line, &left.name).cmp(&(
                &right.project_relative_path,
                right.line,
                &right.name,
            ))
        });
        let (symbols, next) = paginate(
            symbols,
            input.continuation_token.as_deref(),
            input.page_size,
        );
        Ok(ProjectSourceSymbolPage {
            symbols,
            next_continuation_token: next,
            scanned_file_count: self.files.len(),
            truncated: self.truncated,
        })
    }

    pub fn diagnostics(
        &self,
        input: &ProjectDiagnosticsInput,
    ) -> Result<ProjectObservationPage, String> {
        validate_input(&input.schema_version, input.page_size)?;
        let query = input
            .code_or_text
            .as_deref()
            .unwrap_or("")
            .trim()
            .to_lowercase();
        let mut items = Vec::new();
        for file in &self.files {
            let lower = file.relative_path.to_lowercase();
            if !(lower.contains("report") || lower.contains("diagnostic")) {
                continue;
            }
            let content = self.read_text(&file.relative_path).unwrap_or_default();
            if !query.is_empty() && !content.to_lowercase().contains(&query) {
                continue;
            }
            items.push(ProjectObservationItem {
                object_ref: object_ref(&file.relative_path),
                kind: "diagnostic".to_string(),
                name: file.name.clone(),
                project_relative_path: file.relative_path.clone(),
                summary: summarize_text(&content, &query),
                match_fields: vec!["diagnostic".to_string()],
            });
        }
        Ok(page_items(
            items,
            input.continuation_token.as_deref(),
            input.page_size,
            self.files.len(),
            self.truncated,
        ))
    }

    pub fn read_evidence_input(
        &self,
        input: &ProjectEvidenceReadInput,
    ) -> Result<ProjectObjectEvidence, String> {
        validate_schema(&input.schema_version)?;
        let relative = input
            .evidence_ref
            .strip_prefix("project-evidence:")
            .ok_or_else(|| "project_observation.evidence_ref_invalid".to_string())?;
        if !(relative.starts_with("Library/Reports/")
            || relative.starts_with("Library/AiToolKernel/"))
        {
            return Err("project_observation.evidence_scope_rejected".to_string());
        }
        self.read_evidence(
            relative,
            input.max_bytes.clamp(1, MAX_TEXT_FILE_BYTES as usize),
            true,
        )
    }

    fn read_text(&self, relative: &str) -> Result<String, String> {
        let path = safe_existing_path(&self.root, relative)?;
        let metadata = fs::metadata(&path).map_err(|error| error.to_string())?;
        if metadata.len() > MAX_TEXT_FILE_BYTES {
            return Err("project_observation.file_oversize".to_string());
        }
        fs::read_to_string(path).map_err(|error| error.to_string())
    }

    fn read_evidence(
        &self,
        relative: &str,
        max_bytes: usize,
        evidence: bool,
    ) -> Result<ProjectObjectEvidence, String> {
        let path = safe_existing_path(&self.root, relative)?;
        let bytes = fs::read(&path).map_err(|error| error.to_string())?;
        let truncated = bytes.len() > max_bytes;
        let visible = &bytes[..bytes.len().min(max_bytes)];
        let content = match serde_json::from_slice::<Value>(visible) {
            Ok(value) => value,
            Err(_) => Value::String(String::from_utf8_lossy(visible).into_owned()),
        };
        Ok(ProjectObjectEvidence {
            object_ref: if evidence {
                format!("project-evidence:{relative}")
            } else {
                object_ref(relative)
            },
            kind: classify_path(relative),
            project_relative_path: relative.to_string(),
            content_digest: sha256_prefixed(&bytes),
            content,
            truncated,
        })
    }
}

fn collect_files(
    root: &Path,
    path: &Path,
    files: &mut Vec<IndexedFile>,
    truncated: &mut bool,
) -> Result<(), String> {
    if files.len() >= MAX_INDEX_FILES {
        *truncated = true;
        return Ok(());
    }
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    if metadata.file_type().is_symlink() {
        return Ok(());
    }
    if metadata.is_dir() {
        let mut children = fs::read_dir(path)
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        children.sort_by_key(|entry| entry.file_name());
        for child in children {
            collect_files(root, &child.path(), files, truncated)?;
            if *truncated {
                break;
            }
        }
        return Ok(());
    }
    if !metadata.is_file() || metadata.len() > MAX_TEXT_FILE_BYTES {
        return Ok(());
    }
    let relative = path
        .strip_prefix(root)
        .map_err(|_| "project_observation.path_outside_root".to_string())?
        .to_string_lossy()
        .replace('\\', "/");
    files.push(IndexedFile {
        name: path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string(),
        kind: classify_path(&relative),
        relative_path: relative,
    });
    Ok(())
}

fn safe_existing_path(root: &Path, relative: &str) -> Result<PathBuf, String> {
    if relative.is_empty()
        || Path::new(relative).is_absolute()
        || relative.split(['/', '\\']).any(|part| part == "..")
    {
        return Err("project_observation.path_invalid".to_string());
    }
    let path = root.join(relative);
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("project_observation.path_unavailable: {error}"))?;
    if !canonical.starts_with(root) {
        return Err("project_observation.path_outside_root".to_string());
    }
    Ok(canonical)
}

fn classify_path(relative: &str) -> String {
    let lower = relative.to_lowercase();
    if lower.starts_with("scenes/") {
        "scene"
    } else if lower.starts_with("prefabs/") {
        "prefab"
    } else if lower.starts_with("aui/") {
        "aui"
    } else if lower.starts_with("rules/") {
        "rule"
    } else if lower.starts_with("assets/") {
        "asset"
    } else if lower.starts_with("runtimemodule/") {
        "source"
    } else if lower.starts_with("input/") {
        "input"
    } else if lower.starts_with("settings/") || lower == "project.aife.json" {
        "project"
    } else {
        "file"
    }
    .to_string()
}

fn object_ref(relative: &str) -> String {
    format!("project-object:{relative}")
}

fn validate_schema(schema_version: &str) -> Result<(), String> {
    if schema_version != PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION {
        return Err("project_observation.schema_unsupported".to_string());
    }
    Ok(())
}

fn validate_input(schema_version: &str, page_size: usize) -> Result<(), String> {
    validate_schema(schema_version)?;
    if page_size == 0 || page_size > MAX_PAGE_SIZE {
        return Err("project_observation.page_size_invalid".to_string());
    }
    Ok(())
}

fn page_items(
    items: Vec<ProjectObservationItem>,
    continuation: Option<&str>,
    page_size: usize,
    scanned: usize,
    truncated: bool,
) -> ProjectObservationPage {
    let (items, next) = paginate(items, continuation, page_size);
    ProjectObservationPage {
        items,
        next_continuation_token: next,
        scanned_file_count: scanned,
        truncated,
    }
}

fn paginate<T>(
    items: Vec<T>,
    continuation: Option<&str>,
    page_size: usize,
) -> (Vec<T>, Option<String>) {
    let offset = continuation
        .and_then(|token| token.strip_prefix("offset:"))
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0)
        .min(items.len());
    let end = offset.saturating_add(page_size).min(items.len());
    let next = (end < items.len()).then(|| format!("offset:{end}"));
    (
        items.into_iter().skip(offset).take(end - offset).collect(),
        next,
    )
}

fn summarize_text(content: &str, query: &str) -> String {
    if content.is_empty() {
        return "indexed project object".to_string();
    }
    let line = if query.is_empty() {
        content.lines().next()
    } else {
        content
            .lines()
            .find(|line| line.to_lowercase().contains(query))
    }
    .unwrap_or_default();
    bounded_preview(line)
}

fn bounded_preview(line: &str) -> String {
    line.chars().take(240).collect()
}

fn collect_rust_symbols(
    relative_path: &str,
    content: &str,
    query: &str,
    symbols: &mut Vec<ProjectSourceSymbol>,
) {
    for (line_index, line) in content.lines().enumerate() {
        let trimmed = line.trim_start();
        let (kind, remainder) = ["fn ", "struct ", "enum ", "trait ", "type ", "const "]
            .into_iter()
            .find_map(|prefix| {
                trimmed
                    .strip_prefix(prefix)
                    .or_else(|| trimmed.strip_prefix(&format!("pub {prefix}")))
                    .map(|rest| (prefix.trim().to_string(), rest))
            })
            .unwrap_or_else(|| (String::new(), ""));
        if kind.is_empty() {
            continue;
        }
        let name = remainder
            .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
            .next()
            .unwrap_or_default();
        if name.is_empty() || (!query.is_empty() && !name.to_lowercase().contains(query)) {
            continue;
        }
        symbols.push(ProjectSourceSymbol {
            symbol_ref: format!("project-symbol:{relative_path}:{}:{name}", line_index + 1),
            name: name.to_string(),
            symbol_kind: kind,
            project_relative_path: relative_path.to_string(),
            line: line_index + 1,
            declaration: bounded_preview(trimmed),
        });
    }
}
