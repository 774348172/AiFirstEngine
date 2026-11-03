use crate::canonical_digest::file_hash_inventory_digest;
use crate::runtime_package_path::{RuntimePackagePath, RuntimePackagePathClaims};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const RELEASE_PACKAGE_MANIFEST_SCHEMA_VERSION: &str = "release-package-manifest.v1";
pub const RELEASE_PAYLOAD_HASH_SCHEMA_VERSION: &str = "release-payload-inventory.v1";
pub const RELEASE_PACKAGE_MANIFEST_FILE_NAME: &str = "package-manifest.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReleasePackageManifest {
    pub schema_version: String,
    pub application: ReleasePackageApplication,
    pub target: ReleasePackageTarget,
    pub launch: ReleasePackageLaunch,
    pub entrypoint: String,
    pub runtime_package: String,
    pub runtime_content_hash: String,
    pub release_payload_hash: String,
    pub files: Vec<ReleasePackageFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReleasePackageApplication {
    pub display_name: String,
    pub executable_name: String,
    pub company_name: String,
    pub file_description: String,
    pub display_version: String,
    pub windows_file_version: [u16; 4],
    pub windows_product_version: [u16; 4],
    pub copyright: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReleasePackageTarget {
    pub platform: String,
    pub architecture: String,
    pub profile: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReleasePackageLaunch {
    pub user_frame_limit: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReleasePackageFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
    pub roles: Vec<ReleasePackageFileRole>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReleasePackageFileRole {
    Entrypoint,
    Runtime,
    Launcher,
    RuntimePayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleasePackageManifestDiagnostic {
    pub code: &'static str,
    pub path: String,
    pub message: String,
    pub next_action: &'static str,
}

pub fn release_payload_hash(files: &[ReleasePackageFile]) -> String {
    file_hash_inventory_digest(
        "release-payload",
        RELEASE_PAYLOAD_HASH_SCHEMA_VERSION,
        files
            .iter()
            .map(|file| (file.path.as_str(), file.sha256.as_str())),
    )
    .prefixed_value()
}

pub fn validate_release_package_manifest(
    manifest: &ReleasePackageManifest,
) -> Vec<ReleasePackageManifestDiagnostic> {
    let mut diagnostics = Vec::new();
    if manifest.schema_version != RELEASE_PACKAGE_MANIFEST_SCHEMA_VERSION {
        push_diagnostic(
            &mut diagnostics,
            "release_manifest_invalid",
            "schemaVersion",
            format!(
                "schemaVersion must be {RELEASE_PACKAGE_MANIFEST_SCHEMA_VERSION}, got {}",
                manifest.schema_version
            ),
            "Rebuild the release package with a supported manifest schema.",
        );
    }
    for (path, value) in [
        (
            "application.displayName",
            manifest.application.display_name.as_str(),
        ),
        (
            "application.executableName",
            manifest.application.executable_name.as_str(),
        ),
        (
            "application.companyName",
            manifest.application.company_name.as_str(),
        ),
        (
            "application.fileDescription",
            manifest.application.file_description.as_str(),
        ),
        (
            "application.displayVersion",
            manifest.application.display_version.as_str(),
        ),
    ] {
        if value.trim().is_empty() {
            push_diagnostic(
                &mut diagnostics,
                "release_manifest_invalid",
                path,
                format!("{path} must not be empty"),
                "Rebuild from a complete BuildProfile v2 application identity.",
            );
        }
    }
    if manifest.target.platform != "windows"
        || manifest.target.architecture != "x86_64"
        || manifest.target.profile != "release"
    {
        push_diagnostic(
            &mut diagnostics,
            "release_manifest_invalid",
            "target",
            "target must be windows/x86_64/release".to_string(),
            "Rebuild with the supported Windows release profile.",
        );
    }
    if manifest.launch.user_frame_limit == Some(0) {
        push_diagnostic(
            &mut diagnostics,
            "release_manifest_invalid",
            "launch.userFrameLimit",
            "launch.userFrameLimit must be null or greater than zero".to_string(),
            "Use null for an unlimited user session or a positive test-only frame limit.",
        );
    }
    validate_hash(
        &mut diagnostics,
        "runtimeContentHash",
        &manifest.runtime_content_hash,
    );
    validate_hash(
        &mut diagnostics,
        "releasePayloadHash",
        &manifest.release_payload_hash,
    );
    if let Err(error) = RuntimePackagePath::parse(manifest.entrypoint.clone()) {
        push_path_diagnostic(&mut diagnostics, "entrypoint", error);
    }
    if let Err(error) = RuntimePackagePath::parse(manifest.runtime_package.clone()) {
        push_path_diagnostic(&mut diagnostics, "runtimePackage", error);
    }

    let mut claims = RuntimePackagePathClaims::default();
    let mut entrypoint_role_paths = Vec::new();
    let mut runtime_role_count = 0;
    for (index, file) in manifest.files.iter().enumerate() {
        let field = format!("files[{index}].path");
        match RuntimePackagePath::parse(file.path.clone()) {
            Ok(path) => {
                if let Err(error) = claims.claim(&path) {
                    push_path_diagnostic(&mut diagnostics, &field, error);
                }
                if path.windows_collision_key()
                    == RELEASE_PACKAGE_MANIFEST_FILE_NAME.to_ascii_lowercase()
                {
                    push_diagnostic(
                        &mut diagnostics,
                        "release_manifest_invalid",
                        &field,
                        "package-manifest.json cannot be part of its own payload inventory"
                            .to_string(),
                        "Exclude package-manifest.json from files and releasePayloadHash.",
                    );
                }
            }
            Err(error) => push_path_diagnostic(&mut diagnostics, &field, error),
        }
        validate_hash(
            &mut diagnostics,
            &format!("files[{index}].sha256"),
            &file.sha256,
        );
        if file.roles.is_empty() {
            push_diagnostic(
                &mut diagnostics,
                "release_manifest_invalid",
                &format!("files[{index}].roles"),
                "release payload file must declare at least one role".to_string(),
                "Assign a versioned release file role.",
            );
        }
        let unique_roles = file.roles.iter().copied().collect::<BTreeSet<_>>();
        if unique_roles.len() != file.roles.len() {
            push_diagnostic(
                &mut diagnostics,
                "release_manifest_invalid",
                &format!("files[{index}].roles"),
                "release payload file roles must be unique".to_string(),
                "Remove duplicate file roles.",
            );
        }
        if unique_roles.contains(&ReleasePackageFileRole::Entrypoint) {
            entrypoint_role_paths.push(file.path.as_str());
        }
        if unique_roles.contains(&ReleasePackageFileRole::Runtime) {
            runtime_role_count += 1;
        }
    }
    if entrypoint_role_paths.len() != 1
        || entrypoint_role_paths.first().copied() != Some(manifest.entrypoint.as_str())
    {
        push_diagnostic(
            &mut diagnostics,
            "release_entrypoint_missing",
            "entrypoint",
            format!(
                "entrypoint must match the unique entrypoint role file, found {:?}",
                entrypoint_role_paths
            ),
            "Repair entrypoint and files[].roles without guessing a file name.",
        );
    }
    if runtime_role_count == 0 {
        push_diagnostic(
            &mut diagnostics,
            "release_manifest_invalid",
            "files[].roles",
            "release package must contain a runtime role".to_string(),
            "Assign runtime to the direct entrypoint or a future dedicated runtime file.",
        );
    }
    let computed_payload_hash = release_payload_hash(&manifest.files);
    if computed_payload_hash != manifest.release_payload_hash {
        push_diagnostic(
            &mut diagnostics,
            "release_payload_hash_mismatch",
            "releasePayloadHash",
            format!(
                "releasePayloadHash expected {computed_payload_hash}, got {}",
                manifest.release_payload_hash
            ),
            "Rebuild the manifest from the canonical payload inventory.",
        );
    }
    diagnostics
}

fn validate_hash(diagnostics: &mut Vec<ReleasePackageManifestDiagnostic>, path: &str, value: &str) {
    let valid = value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    });
    if !valid {
        push_diagnostic(
            diagnostics,
            "release_manifest_invalid",
            path,
            format!("{path} must use sha256:<64 lowercase hex>"),
            "Recompute the release package hash with the canonical SHA-256 encoder.",
        );
    }
}

fn push_path_diagnostic(
    diagnostics: &mut Vec<ReleasePackageManifestDiagnostic>,
    path: &str,
    error: crate::runtime_package_path::RuntimePackagePathError,
) {
    push_diagnostic(
        diagnostics,
        if error.code == "runtime_package_path_collision" {
            "release_path_collision"
        } else {
            "release_path_escape"
        },
        path,
        error.to_string(),
        "Use normalized package-relative forward-slash paths within the release root.",
    );
}

fn push_diagnostic(
    diagnostics: &mut Vec<ReleasePackageManifestDiagnostic>,
    code: &'static str,
    path: &str,
    message: String,
    next_action: &'static str,
) {
    diagnostics.push(ReleasePackageManifestDiagnostic {
        code,
        path: path.to_string(),
        message,
        next_action,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_package_manifest_accepts_b_min_roles_and_deterministic_hash() {
        let manifest = fixture_manifest();
        assert!(validate_release_package_manifest(&manifest).is_empty());
        let reversed = ReleasePackageManifest {
            files: manifest.files.iter().cloned().rev().collect(),
            ..manifest.clone()
        };
        assert_eq!(
            release_payload_hash(&manifest.files),
            release_payload_hash(&reversed.files)
        );
    }

    #[test]
    fn release_package_manifest_rejects_escape_collision_roles_and_hash_mutation() {
        let mut manifest = fixture_manifest();
        manifest.files[1].path = "../runtime/manifest.json".to_string();
        manifest.files.push(ReleasePackageFile {
            path: "complexshooter.EXE".to_string(),
            size: 1,
            sha256: format!("sha256:{}", "c".repeat(64)),
            roles: vec![ReleasePackageFileRole::Entrypoint],
        });
        manifest.release_payload_hash = format!("sha256:{}", "d".repeat(64));
        let diagnostics = validate_release_package_manifest(&manifest);
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "release_path_escape"));
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "release_path_collision"));
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "release_entrypoint_missing"));
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "release_payload_hash_mismatch"));
    }

    fn fixture_manifest() -> ReleasePackageManifest {
        let files = vec![
            ReleasePackageFile {
                path: "ComplexShooter.exe".to_string(),
                size: 100,
                sha256: format!("sha256:{}", "a".repeat(64)),
                roles: vec![
                    ReleasePackageFileRole::Entrypoint,
                    ReleasePackageFileRole::Runtime,
                ],
            },
            ReleasePackageFile {
                path: "data/runtime_package/manifest.json".to_string(),
                size: 200,
                sha256: format!("sha256:{}", "b".repeat(64)),
                roles: vec![ReleasePackageFileRole::RuntimePayload],
            },
        ];
        ReleasePackageManifest {
            schema_version: RELEASE_PACKAGE_MANIFEST_SCHEMA_VERSION.to_string(),
            application: ReleasePackageApplication {
                display_name: "Complex Shooter".to_string(),
                executable_name: "ComplexShooter".to_string(),
                company_name: "AI First Engine Studio".to_string(),
                file_description: "Complex Shooter".to_string(),
                display_version: "1.0.0".to_string(),
                windows_file_version: [1, 0, 0, 0],
                windows_product_version: [1, 0, 0, 0],
                copyright: "Copyright AI First Engine Studio".to_string(),
            },
            target: ReleasePackageTarget {
                platform: "windows".to_string(),
                architecture: "x86_64".to_string(),
                profile: "release".to_string(),
            },
            launch: ReleasePackageLaunch {
                user_frame_limit: None,
            },
            entrypoint: "ComplexShooter.exe".to_string(),
            runtime_package: "data/runtime_package".to_string(),
            runtime_content_hash: format!("sha256:{}", "e".repeat(64)),
            release_payload_hash: release_payload_hash(&files),
            files,
        }
    }
}
