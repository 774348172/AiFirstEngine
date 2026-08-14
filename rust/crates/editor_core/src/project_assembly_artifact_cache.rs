use engine_runtime::canonical_digest::sha256_prefixed;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const PROJECT_ASSEMBLY_ARTIFACT_ENVELOPE_SCHEMA_VERSION: &str =
    "project-assembly-artifact-envelope.v1";
pub const PROJECT_ASSEMBLY_PRODUCER_REPORT_SCHEMA_VERSION: &str =
    "project-assembly-producer-report.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectAssemblyArtifactCacheStatus {
    Disabled,
    Hit,
    Miss,
    Invalid,
    Corrupt,
    Produced,
    PublishRaceReused,
    Failed,
}

impl Default for ProjectAssemblyArtifactCacheStatus {
    fn default() -> Self {
        Self::Disabled
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectAssemblyArtifactEnvelope {
    pub schema_version: String,
    pub producer_id: String,
    pub producer_recipe_version: String,
    pub recipe_key: String,
    pub dependency_digest: String,
    pub output_digest: String,
    pub payload_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAssemblyProducerSubstageReport {
    pub stage_id: String,
    pub duration_ms: u64,
    pub skipped: bool,
}

impl ProjectAssemblyProducerSubstageReport {
    pub fn completed(stage_id: impl Into<String>, duration_ms: u64) -> Self {
        Self {
            stage_id: stage_id.into(),
            duration_ms,
            skipped: false,
        }
    }

    pub fn skipped(stage_id: impl Into<String>) -> Self {
        Self {
            stage_id: stage_id.into(),
            duration_ms: 0,
            skipped: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAssemblyProducerReport {
    pub schema_version: String,
    pub producer_id: String,
    pub producer_recipe_version: String,
    pub status: String,
    pub duration_ms: u64,
    pub recipe_duration_ms: u64,
    pub lookup_duration_ms: u64,
    pub produce_duration_ms: u64,
    pub validate_duration_ms: u64,
    pub publish_duration_ms: u64,
    pub cache_status: ProjectAssemblyArtifactCacheStatus,
    pub recipe_key: Option<String>,
    pub output_digest: Option<String>,
    pub miss_reason: Option<String>,
    pub artifact_path: Option<String>,
    #[serde(default)]
    pub substages: Vec<ProjectAssemblyProducerSubstageReport>,
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

impl ProjectAssemblyProducerReport {
    pub fn uncached(producer_id: impl Into<String>, duration_ms: u64) -> Self {
        Self {
            schema_version: PROJECT_ASSEMBLY_PRODUCER_REPORT_SCHEMA_VERSION.to_string(),
            producer_id: producer_id.into(),
            producer_recipe_version: "uncached.v1".to_string(),
            status: "success".to_string(),
            duration_ms,
            recipe_duration_ms: 0,
            lookup_duration_ms: 0,
            produce_duration_ms: duration_ms,
            validate_duration_ms: 0,
            publish_duration_ms: 0,
            cache_status: ProjectAssemblyArtifactCacheStatus::Disabled,
            recipe_key: None,
            output_digest: None,
            miss_reason: None,
            artifact_path: None,
            substages: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct ProjectAssemblyArtifactLookup<T> {
    pub status: ProjectAssemblyArtifactCacheStatus,
    pub artifact: Option<T>,
    pub envelope: Option<ProjectAssemblyArtifactEnvelope>,
    pub artifact_path: Option<PathBuf>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectAssemblyArtifactPublishStatus {
    Produced,
    PublishRaceReused,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectAssemblyArtifactPublishResult {
    pub status: ProjectAssemblyArtifactPublishStatus,
    pub artifact_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectAssemblyArtifactCacheError {
    pub code: &'static str,
    pub message: String,
}

impl std::fmt::Display for ProjectAssemblyArtifactCacheError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ProjectAssemblyArtifactCacheError {}

#[derive(Debug, Clone)]
pub struct ProjectAssemblyArtifactCache {
    root: PathBuf,
}

impl ProjectAssemblyArtifactCache {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, ProjectAssemblyArtifactCacheError> {
        let root = root.into();
        if root.as_os_str().is_empty() {
            return Err(error("artifact_cache.root_empty", "Cache root is empty."));
        }
        fs::create_dir_all(&root).map_err(|source| {
            error(
                "artifact_cache.root_unavailable",
                format!("Cannot create cache root {}: {source}", root.display()),
            )
        })?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn quarantine(
        &self,
        producer_id: &str,
        recipe_key: &str,
    ) -> Result<(), ProjectAssemblyArtifactCacheError> {
        let artifact_path = self.artifact_path(producer_id, recipe_key)?;
        if !artifact_path.exists() {
            return Ok(());
        }
        quarantine_path(&artifact_path).map_err(|source| {
            error(
                "artifact_cache.quarantine_failed",
                format!(
                    "Cannot quarantine artifact {}: {source}",
                    artifact_path.display()
                ),
            )
        })
    }

    pub fn lookup_json<T: DeserializeOwned>(
        &self,
        producer_id: &str,
        recipe_key: &str,
        expected_recipe_version: &str,
    ) -> ProjectAssemblyArtifactLookup<T> {
        let artifact_path = match self.artifact_path(producer_id, recipe_key) {
            Ok(path) => path,
            Err(source) => {
                return ProjectAssemblyArtifactLookup {
                    status: ProjectAssemblyArtifactCacheStatus::Invalid,
                    artifact: None,
                    envelope: None,
                    artifact_path: None,
                    reason: Some(source.to_string()),
                };
            }
        };
        if !artifact_path.is_dir() {
            return ProjectAssemblyArtifactLookup {
                status: ProjectAssemblyArtifactCacheStatus::Miss,
                artifact: None,
                envelope: None,
                artifact_path: Some(artifact_path),
                reason: Some("artifact_missing".to_string()),
            };
        }

        match self.read_json_artifact::<T>(
            &artifact_path,
            producer_id,
            recipe_key,
            expected_recipe_version,
        ) {
            Ok((envelope, artifact)) => ProjectAssemblyArtifactLookup {
                status: ProjectAssemblyArtifactCacheStatus::Hit,
                artifact: Some(artifact),
                envelope: Some(envelope),
                artifact_path: Some(artifact_path),
                reason: None,
            },
            Err((status, reason)) => {
                if matches!(
                    status,
                    ProjectAssemblyArtifactCacheStatus::Corrupt
                        | ProjectAssemblyArtifactCacheStatus::Invalid
                ) {
                    let _ = quarantine_path(&artifact_path);
                }
                ProjectAssemblyArtifactLookup {
                    status,
                    artifact: None,
                    envelope: None,
                    artifact_path: Some(artifact_path),
                    reason: Some(reason),
                }
            }
        }
    }

    pub fn publish_json<T: Serialize>(
        &self,
        producer_id: &str,
        producer_recipe_version: &str,
        recipe_key: &str,
        dependency_digest: &str,
        output_digest: &str,
        artifact: &T,
    ) -> Result<ProjectAssemblyArtifactPublishResult, ProjectAssemblyArtifactCacheError> {
        let artifact_path = self.artifact_path(producer_id, recipe_key)?;
        let parent = artifact_path.parent().ok_or_else(|| {
            error(
                "artifact_cache.path_invalid",
                "Artifact path has no parent directory.",
            )
        })?;
        fs::create_dir_all(parent).map_err(|source| {
            error(
                "artifact_cache.publish_failed",
                format!(
                    "Cannot create artifact parent {}: {source}",
                    parent.display()
                ),
            )
        })?;
        let payload = serde_json::to_vec(artifact).map_err(|source| {
            error(
                "artifact_cache.payload_encode_failed",
                format!("Cannot encode typed artifact: {source}"),
            )
        })?;
        let payload_digest = sha256_prefixed(&payload);
        let envelope = ProjectAssemblyArtifactEnvelope {
            schema_version: PROJECT_ASSEMBLY_ARTIFACT_ENVELOPE_SCHEMA_VERSION.to_string(),
            producer_id: producer_id.to_string(),
            producer_recipe_version: producer_recipe_version.to_string(),
            recipe_key: recipe_key.to_string(),
            dependency_digest: dependency_digest.to_string(),
            output_digest: output_digest.to_string(),
            payload_digest,
        };
        let envelope_bytes = serde_json::to_vec_pretty(&envelope).map_err(|source| {
            error(
                "artifact_cache.envelope_encode_failed",
                format!("Cannot encode artifact envelope: {source}"),
            )
        })?;
        let temp_path = parent.join(format!(
            ".tmp-{}-{}-{}",
            digest_hex(recipe_key)?,
            std::process::id(),
            unique_suffix()
        ));
        fs::create_dir(&temp_path).map_err(|source| {
            error(
                "artifact_cache.publish_failed",
                format!(
                    "Cannot create temporary artifact {}: {source}",
                    temp_path.display()
                ),
            )
        })?;
        let write_result = (|| {
            write_synced(&temp_path.join("payload.json"), &payload)?;
            write_synced(&temp_path.join("artifact-envelope.json"), &envelope_bytes)?;
            Ok::<(), ProjectAssemblyArtifactCacheError>(())
        })();
        if let Err(source) = write_result {
            let _ = fs::remove_dir_all(&temp_path);
            return Err(source);
        }

        match fs::rename(&temp_path, &artifact_path) {
            Ok(()) => Ok(ProjectAssemblyArtifactPublishResult {
                status: ProjectAssemblyArtifactPublishStatus::Produced,
                artifact_path,
            }),
            Err(rename_error) if artifact_path.is_dir() => {
                let existing = self.read_envelope(&artifact_path).map_err(|reason| {
                    error(
                        "artifact_cache.publish_race_invalid",
                        format!("Existing artifact is invalid after publish race: {reason}"),
                    )
                })?;
                let _ = fs::remove_dir_all(&temp_path);
                if existing.output_digest == output_digest
                    && existing.payload_digest == envelope.payload_digest
                {
                    Ok(ProjectAssemblyArtifactPublishResult {
                        status: ProjectAssemblyArtifactPublishStatus::PublishRaceReused,
                        artifact_path,
                    })
                } else {
                    Err(error(
                        "artifact_cache.deterministic_violation",
                        format!(
                            "Recipe key {recipe_key} produced conflicting output after race: {rename_error}"
                        ),
                    ))
                }
            }
            Err(source) => {
                let _ = fs::remove_dir_all(&temp_path);
                Err(error(
                    "artifact_cache.publish_failed",
                    format!(
                        "Cannot publish artifact {}: {source}",
                        artifact_path.display()
                    ),
                ))
            }
        }
    }

    fn artifact_path(
        &self,
        producer_id: &str,
        recipe_key: &str,
    ) -> Result<PathBuf, ProjectAssemblyArtifactCacheError> {
        if producer_id.is_empty()
            || !producer_id
                .chars()
                .all(|value| value.is_ascii_alphanumeric() || matches!(value, '-' | '_'))
        {
            return Err(error(
                "artifact_cache.producer_id_invalid",
                format!("Producer id is not a safe logical identifier: {producer_id}"),
            ));
        }
        let digest = digest_hex(recipe_key)?;
        Ok(self
            .root
            .join("assembly-artifacts-v1")
            .join(producer_id)
            .join(&digest[..2])
            .join(digest))
    }

    fn read_json_artifact<T: DeserializeOwned>(
        &self,
        artifact_path: &Path,
        producer_id: &str,
        recipe_key: &str,
        expected_recipe_version: &str,
    ) -> Result<(ProjectAssemblyArtifactEnvelope, T), (ProjectAssemblyArtifactCacheStatus, String)>
    {
        let envelope = self
            .read_envelope(artifact_path)
            .map_err(|reason| (ProjectAssemblyArtifactCacheStatus::Corrupt, reason))?;
        if envelope.schema_version != PROJECT_ASSEMBLY_ARTIFACT_ENVELOPE_SCHEMA_VERSION
            || envelope.producer_id != producer_id
            || envelope.recipe_key != recipe_key
            || envelope.producer_recipe_version != expected_recipe_version
        {
            return Err((
                ProjectAssemblyArtifactCacheStatus::Invalid,
                "artifact_identity_incompatible".to_string(),
            ));
        }
        let payload_path = artifact_path.join("payload.json");
        let payload = fs::read(&payload_path).map_err(|source| {
            (
                ProjectAssemblyArtifactCacheStatus::Corrupt,
                format!("Cannot read {}: {source}", payload_path.display()),
            )
        })?;
        if sha256_prefixed(&payload) != envelope.payload_digest {
            return Err((
                ProjectAssemblyArtifactCacheStatus::Corrupt,
                "artifact_payload_digest_mismatch".to_string(),
            ));
        }
        let artifact = serde_json::from_slice::<T>(&payload).map_err(|source| {
            (
                ProjectAssemblyArtifactCacheStatus::Corrupt,
                format!("Typed artifact decode failed: {source}"),
            )
        })?;
        Ok((envelope, artifact))
    }

    fn read_envelope(
        &self,
        artifact_path: &Path,
    ) -> Result<ProjectAssemblyArtifactEnvelope, String> {
        let path = artifact_path.join("artifact-envelope.json");
        let bytes = fs::read(&path)
            .map_err(|source| format!("Cannot read {}: {source}", path.display()))?;
        serde_json::from_slice(&bytes)
            .map_err(|source| format!("Cannot decode {}: {source}", path.display()))
    }
}

fn write_synced(path: &Path, bytes: &[u8]) -> Result<(), ProjectAssemblyArtifactCacheError> {
    use std::io::Write;
    let mut file = fs::File::create(path).map_err(|source| {
        error(
            "artifact_cache.publish_failed",
            format!("Cannot create {}: {source}", path.display()),
        )
    })?;
    file.write_all(bytes).map_err(|source| {
        error(
            "artifact_cache.publish_failed",
            format!("Cannot write {}: {source}", path.display()),
        )
    })?;
    file.sync_all().map_err(|source| {
        error(
            "artifact_cache.publish_failed",
            format!("Cannot sync {}: {source}", path.display()),
        )
    })
}

fn digest_hex(recipe_key: &str) -> Result<String, ProjectAssemblyArtifactCacheError> {
    let Some(digest) = recipe_key.strip_prefix("sha256:") else {
        return Err(error(
            "artifact_cache.recipe_key_invalid",
            "Recipe key must use the sha256: prefix.",
        ));
    };
    if digest.len() != 64 || !digest.chars().all(|value| value.is_ascii_hexdigit()) {
        return Err(error(
            "artifact_cache.recipe_key_invalid",
            "Recipe key must contain exactly 64 hexadecimal SHA-256 characters.",
        ));
    }
    Ok(digest.to_ascii_lowercase())
}

fn quarantine_path(path: &Path) -> Result<(), std::io::Error> {
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "artifact has no parent")
    })?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("artifact");
    fs::rename(
        path,
        parent.join(format!(".corrupt-{name}-{}", unique_suffix())),
    )
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn error(code: &'static str, message: impl Into<String>) -> ProjectAssemblyArtifactCacheError {
    ProjectAssemblyArtifactCacheError {
        code,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "aife-assembly-cache-{name}-{}-{}",
            std::process::id(),
            unique_suffix()
        ))
    }

    #[test]
    fn project_assembly_artifact_cache_round_trips_and_detects_corruption() {
        let root = test_root("round-trip");
        let cache = ProjectAssemblyArtifactCache::open(&root).unwrap();
        let key = sha256_prefixed(b"recipe");
        let missing = cache.lookup_json::<serde_json::Value>("font-cook", &key, "font.v1");
        assert_eq!(missing.status, ProjectAssemblyArtifactCacheStatus::Miss);

        let payload = serde_json::json!({"fontBundle": "sealed"});
        cache
            .publish_json("font-cook", "font.v1", &key, &key, "output", &payload)
            .unwrap();
        let hit = cache.lookup_json::<serde_json::Value>("font-cook", &key, "font.v1");
        assert_eq!(hit.status, ProjectAssemblyArtifactCacheStatus::Hit);
        assert_eq!(hit.artifact, Some(payload));

        fs::write(
            hit.artifact_path.unwrap().join("payload.json"),
            b"truncated",
        )
        .unwrap();
        let corrupt = cache.lookup_json::<serde_json::Value>("font-cook", &key, "font.v1");
        assert_eq!(corrupt.status, ProjectAssemblyArtifactCacheStatus::Corrupt);
        let rebuilt = cache
            .publish_json(
                "font-cook",
                "font.v1",
                &key,
                &key,
                "output",
                &serde_json::json!({"fontBundle": "rebuilt"}),
            )
            .unwrap();
        assert_eq!(
            rebuilt.status,
            ProjectAssemblyArtifactPublishStatus::Produced
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_assembly_artifact_cache_rejects_unsafe_identity() {
        let root = test_root("identity");
        let cache = ProjectAssemblyArtifactCache::open(&root).unwrap();
        let lookup = cache.lookup_json::<serde_json::Value>(
            "../font",
            &sha256_prefixed(b"recipe"),
            "font.v1",
        );
        assert_eq!(lookup.status, ProjectAssemblyArtifactCacheStatus::Invalid);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_assembly_artifact_cache_invalid_recipe_is_quarantined_and_rebuilt() {
        let root = test_root("invalid-recipe");
        let cache = ProjectAssemblyArtifactCache::open(&root).unwrap();
        let key = sha256_prefixed(b"recipe");
        cache
            .publish_json(
                "font-cook",
                "font.v1",
                &key,
                &key,
                "old-output",
                &serde_json::json!({"version": 1}),
            )
            .unwrap();
        let invalid = cache.lookup_json::<serde_json::Value>("font-cook", &key, "font.v2");
        assert_eq!(invalid.status, ProjectAssemblyArtifactCacheStatus::Invalid);
        cache
            .publish_json(
                "font-cook",
                "font.v2",
                &key,
                &key,
                "new-output",
                &serde_json::json!({"version": 2}),
            )
            .unwrap();
        let hit = cache.lookup_json::<serde_json::Value>("font-cook", &key, "font.v2");
        assert_eq!(hit.status, ProjectAssemblyArtifactCacheStatus::Hit);
        assert_eq!(hit.artifact, Some(serde_json::json!({"version": 2})));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_assembly_artifact_cache_concurrency_publishes_one_sealed_output() {
        use std::sync::{Arc, Barrier};

        let root = test_root("concurrency");
        let cache = Arc::new(ProjectAssemblyArtifactCache::open(&root).unwrap());
        let barrier = Arc::new(Barrier::new(3));
        let key = sha256_prefixed(b"same-recipe");
        let mut workers = Vec::new();
        for _ in 0..2 {
            let cache = Arc::clone(&cache);
            let barrier = Arc::clone(&barrier);
            let key = key.clone();
            workers.push(std::thread::spawn(move || {
                barrier.wait();
                cache
                    .publish_json(
                        "font-cook",
                        "font.v1",
                        &key,
                        &key,
                        "same-output",
                        &serde_json::json!({"sealed": true}),
                    )
                    .unwrap()
                    .status
            }));
        }
        barrier.wait();
        let statuses = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect::<Vec<_>>();
        assert!(statuses.contains(&ProjectAssemblyArtifactPublishStatus::Produced));
        assert!(statuses.contains(&ProjectAssemblyArtifactPublishStatus::PublishRaceReused));
        let hit = cache.lookup_json::<serde_json::Value>("font-cook", &key, "font.v1");
        assert_eq!(hit.status, ProjectAssemblyArtifactCacheStatus::Hit);
        fs::remove_dir_all(root).unwrap();
    }
}
