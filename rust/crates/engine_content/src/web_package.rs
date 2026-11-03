//! Static delivery receipt for the existing RuntimePackage, not a new game format.
use crate::canonical_digest::sha256_prefixed;
use crate::runtime_package_source::RuntimePackageSource;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebPackageManifest {
    pub schema_version: String,
    pub project_id: String,
    pub module_id: String,
    pub package_content_hash: String,
    pub files: Vec<WebPackageFile>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebPackageFile {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}
impl WebPackageManifest {
    pub fn verified_source(
        &self,
        files: Vec<(String, Vec<u8>)>,
    ) -> Result<RuntimePackageSource, String> {
        if self.schema_version != "web-package.v1"
            || self.project_id.is_empty()
            || self.module_id.is_empty()
        {
            return Err("web.package.manifest_invalid".into());
        }
        let supplied_count = files.len();
        let source = RuntimePackageSource::memory(files).map_err(|e| e.to_string())?;
        let mut expected = BTreeMap::new();
        for file in &self.files {
            // resolve validates names even when the supplied file is missing.
            source.resolve(&file.path).map_err(|e| e.to_string())?;
            if expected
                .insert(file.path.to_ascii_lowercase(), ())
                .is_some()
            {
                return Err(format!("web.package.duplicate_file:{}", file.path));
            }
            let bytes = source
                .read(&file.path)
                .map_err(|e| format!("web.package.read:{}:{e}", file.path))?;
            if bytes.len() as u64 != file.bytes || sha256_prefixed(&bytes) != file.sha256 {
                return Err(format!("web.package.integrity:{}", file.path));
            }
        }
        if supplied_count != self.files.len() {
            return Err("web.package.file_count_mismatch".into());
        }
        if !expected.contains_key("manifest.json") {
            return Err("web.package.manifest_missing".into());
        }
        Ok(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn manifest() -> WebPackageManifest {
        WebPackageManifest {
            schema_version: "web-package.v1".into(),
            project_id: "test".into(),
            module_id: "test.runtime".into(),
            package_content_hash: "hash".into(),
            files: vec![WebPackageFile {
                path: "manifest.json".into(),
                bytes: 2,
                sha256: sha256_prefixed(b"{}"),
            }],
        }
    }
    #[test]
    fn delivery_rejects_corruption_missing_duplicates_and_escape() {
        let valid = manifest();
        assert!(valid
            .verified_source(vec![("manifest.json".into(), b"{}".to_vec())])
            .is_ok());
        assert!(valid
            .verified_source(vec![("manifest.json".into(), b"[]".to_vec())])
            .unwrap_err()
            .contains("integrity:manifest.json"));
        assert!(valid
            .verified_source(vec![])
            .unwrap_err()
            .contains("web.package.read:manifest.json"));
        assert!(valid
            .verified_source(vec![
                ("manifest.json".into(), b"{}".to_vec()),
                ("extra".into(), vec![])
            ])
            .unwrap_err()
            .contains("file_count_mismatch"));
        let mut duplicate = manifest();
        duplicate.files.push(duplicate.files[0].clone());
        assert!(duplicate
            .verified_source(vec![("manifest.json".into(), b"{}".to_vec())])
            .unwrap_err()
            .contains("duplicate_file"));
        let mut escape = manifest();
        escape.files[0].path = "../escape".into();
        assert!(escape.verified_source(vec![]).is_err());
    }
}
