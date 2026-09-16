use crate::architecture_policy::{ArchitectureDiagnostic, ArchitecturePolicy, DomainPolicy};
use crate::cargo_json::sha256_hex;
use quote::ToTokens;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use syn::visit::{self, Visit};

pub const ARCHITECTURE_INVENTORY_SCHEMA_VERSION: &str = "architecture-inventory.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchitectureInventory {
    pub schema_version: String,
    pub profile_id: String,
    pub files: Vec<InventoryFile>,
    pub crate_dependencies: Vec<CrateDependency>,
    pub digest: String,
    pub diagnostics: Vec<ArchitectureDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryFile {
    pub path: String,
    pub domain: String,
    pub owner: String,
    pub kind: InventoryFileKind,
    pub content_digest: String,
    pub symbols: Vec<String>,
    pub imports: Vec<String>,
    pub impls: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InventoryFileKind {
    Rust,
    CargoManifest,
    BuildScript,
    Workflow,
    Policy,
    Schema,
    WorkspaceConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrateDependency {
    pub package: String,
    pub dependency: String,
}

pub fn build_inventory(
    workspace_root: &Path,
    policy: &ArchitecturePolicy,
    profile_id: &str,
    cargo_metadata: &[u8],
) -> Result<ArchitectureInventory, Vec<ArchitectureDiagnostic>> {
    let Some(profile) = policy
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
    else {
        return Err(vec![ArchitectureDiagnostic::policy(
            "architecture_inventory.unknown_profile",
            format!("profile {profile_id:?} is not declared"),
            "Use a committed architecture policy profile.",
        )]);
    };
    let metadata = parse_metadata(cargo_metadata)?;
    let mut candidates = BTreeSet::new();
    collect_files(
        workspace_root,
        workspace_root,
        workspace_root,
        &mut candidates,
    )
    .map_err(|diagnostic| vec![*diagnostic])?;
    if let Some(repository_root) = workspace_root.parent() {
        let workflow_root = repository_root.join(".github/workflows");
        if workflow_root.is_dir() {
            collect_files(
                repository_root,
                &workflow_root,
                repository_root,
                &mut candidates,
            )
            .map_err(|diagnostic| vec![*diagnostic])?;
        }
    }

    let mut files = Vec::new();
    let mut diagnostics = Vec::new();
    for relative in candidates {
        if !matches_any(&relative, &profile.include) || matches_any(&relative, &profile.exclude) {
            continue;
        }
        if !classifiable(&relative) {
            continue;
        }
        let absolute = if relative.starts_with(".github/") {
            workspace_root
                .parent()
                .unwrap_or(workspace_root)
                .join(&relative)
        } else {
            workspace_root.join(&relative)
        };
        let Some(domain) = domain_for_path(&relative, &policy.domains) else {
            diagnostics.push(ArchitectureDiagnostic {
                code: "architecture_inventory.owner_missing".to_string(),
                source_path: Some(relative),
                domain: None,
                subject: None,
                stage: "architecture_inventory".to_string(),
                observed_evidence: "included path has no matching domain owner".to_string(),
                rule_id: None,
                classification: "unowned".to_string(),
                next_action: "Assign the path to exactly one committed domain.".to_string(),
            });
            continue;
        };
        match inventory_file(&absolute, &relative, domain) {
            Ok(file) => files.push(file),
            Err(diagnostic) => diagnostics.push(*diagnostic),
        }
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let mut crate_dependencies = metadata;
    crate_dependencies.sort_by(|left, right| {
        (&left.package, &left.dependency).cmp(&(&right.package, &right.dependency))
    });
    let canonical = serde_json::to_vec(&(&files, &crate_dependencies)).map_err(|error| {
        vec![ArchitectureDiagnostic::policy(
            "architecture_inventory.serialize_failed",
            error.to_string(),
            "Repair inventory serialization.",
        )]
    })?;
    let digest = format!("sha256:{}", sha256_hex(&canonical));
    Ok(ArchitectureInventory {
        schema_version: ARCHITECTURE_INVENTORY_SCHEMA_VERSION.to_string(),
        profile_id: profile_id.to_string(),
        files,
        crate_dependencies,
        digest,
        diagnostics,
    })
}

fn parse_metadata(bytes: &[u8]) -> Result<Vec<CrateDependency>, Vec<ArchitectureDiagnostic>> {
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        vec![ArchitectureDiagnostic::policy(
            "architecture_inventory.metadata_invalid",
            error.to_string(),
            "Run cargo metadata --locked --format-version 1 --no-deps.",
        )]
    })?;
    let packages = value
        .get("packages")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            vec![ArchitectureDiagnostic::policy(
                "architecture_inventory.metadata_invalid",
                "cargo metadata is missing packages",
                "Run cargo metadata --locked --format-version 1 --no-deps.",
            )]
        })?;
    let mut dependencies = Vec::new();
    for package in packages {
        let Some(name) = package.get("name").and_then(Value::as_str) else {
            continue;
        };
        for dependency in package
            .get("dependencies")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(dependency_name) = dependency.get("name").and_then(Value::as_str) {
                dependencies.push(CrateDependency {
                    package: name.to_string(),
                    dependency: dependency_name.to_string(),
                });
            }
        }
    }
    Ok(dependencies)
}

fn collect_files(
    base: &Path,
    current: &Path,
    confinement_root: &Path,
    output: &mut BTreeSet<String>,
) -> Result<(), Box<ArchitectureDiagnostic>> {
    for entry in fs::read_dir(current).map_err(|error| inventory_io(current, error))? {
        let entry = entry.map_err(|error| inventory_io(current, error))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| inventory_io(&path, error))?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if matches!(name, ".git" | "target" | "vendor" | "generated") {
                continue;
            }
            if !path.starts_with(confinement_root) {
                continue;
            }
            collect_files(base, &path, confinement_root, output)?;
        } else if file_type.is_file() {
            let relative = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            output.insert(relative);
        }
    }
    Ok(())
}

fn inventory_file(
    absolute: &Path,
    relative: &str,
    domain: &DomainPolicy,
) -> Result<InventoryFile, Box<ArchitectureDiagnostic>> {
    let bytes = fs::read(absolute).map_err(|error| inventory_io(absolute, error))?;
    let kind = file_kind(relative)?;
    let mut symbols = Vec::new();
    let mut imports = Vec::new();
    let mut impls = Vec::new();
    if kind == InventoryFileKind::Rust || kind == InventoryFileKind::BuildScript {
        let source = std::str::from_utf8(&bytes).map_err(|error| {
            Box::new(ArchitectureDiagnostic {
                code: "architecture_inventory.rust_not_utf8".to_string(),
                source_path: Some(relative.to_string()),
                domain: Some(domain.id.clone()),
                subject: None,
                stage: "architecture_inventory".to_string(),
                observed_evidence: error.to_string(),
                rule_id: None,
                classification: "parse_failed".to_string(),
                next_action: "Store Rust source as UTF-8.".to_string(),
            })
        })?;
        let syntax = syn::parse_file(source).map_err(|error| {
            Box::new(ArchitectureDiagnostic {
                code: "architecture_inventory.rust_parse_failed".to_string(),
                source_path: Some(relative.to_string()),
                domain: Some(domain.id.clone()),
                subject: None,
                stage: "architecture_inventory".to_string(),
                observed_evidence: error.to_string(),
                rule_id: None,
                classification: "parse_failed".to_string(),
                next_action: "Repair Rust syntax before architecture inventory.".to_string(),
            })
        })?;
        let mut visitor = RustInventoryVisitor::default();
        visitor.visit_file(&syntax);
        symbols = visitor.symbols.into_iter().collect();
        imports = visitor.imports.into_iter().collect();
        impls = visitor.impls.into_iter().collect();
    }
    Ok(InventoryFile {
        path: relative.to_string(),
        domain: domain.id.clone(),
        owner: domain.owner.clone(),
        kind,
        content_digest: format!("sha256:{}", sha256_hex(&bytes)),
        symbols,
        imports,
        impls,
    })
}

fn file_kind(relative: &str) -> Result<InventoryFileKind, Box<ArchitectureDiagnostic>> {
    if relative.ends_with("build.rs") {
        Ok(InventoryFileKind::BuildScript)
    } else if relative.ends_with(".rs") {
        Ok(InventoryFileKind::Rust)
    } else if relative.ends_with("Cargo.toml") {
        Ok(InventoryFileKind::CargoManifest)
    } else if matches!(relative, "Cargo.lock" | "rust-toolchain.toml") {
        Ok(InventoryFileKind::WorkspaceConfig)
    } else if relative.starts_with(".github/workflows/") {
        Ok(InventoryFileKind::Workflow)
    } else if relative.contains("quality/") && relative.ends_with(".toml") {
        Ok(InventoryFileKind::Policy)
    } else if relative.ends_with(".json") {
        Ok(InventoryFileKind::Schema)
    } else {
        Err(Box::new(ArchitectureDiagnostic {
            code: "architecture_inventory.kind_unknown".to_string(),
            source_path: Some(relative.to_string()),
            domain: None,
            subject: None,
            stage: "architecture_inventory".to_string(),
            observed_evidence: "included file kind is not classified".to_string(),
            rule_id: None,
            classification: "unknown".to_string(),
            next_action: "Classify the file kind or narrow the committed include scope."
                .to_string(),
        }))
    }
}

fn classifiable(relative: &str) -> bool {
    relative.ends_with(".rs")
        || relative.ends_with("Cargo.toml")
        || matches!(relative, "Cargo.lock" | "rust-toolchain.toml")
        || relative.starts_with(".github/workflows/")
        || (relative.contains("quality/") && relative.ends_with(".toml"))
        || relative.ends_with(".json")
}

fn domain_for_path<'a>(path: &str, domains: &'a [DomainPolicy]) -> Option<&'a DomainPolicy> {
    domains
        .iter()
        .find(|domain| matches_any(path, &domain.include))
}

pub fn matches_pattern(path: &str, pattern: &str) -> bool {
    let path = path.replace('\\', "/");
    let pattern = pattern.replace('\\', "/");
    if let Some(prefix) = pattern.strip_suffix("/**") {
        path == prefix || path.starts_with(&format!("{prefix}/"))
    } else if let Some(suffix) = pattern.strip_prefix("**/") {
        path == suffix || path.ends_with(&format!("/{suffix}"))
    } else {
        path == pattern
    }
}

fn matches_any(path: &str, patterns: &[String]) -> bool {
    patterns
        .iter()
        .any(|pattern| matches_pattern(path, pattern))
}

fn inventory_io(path: &Path, error: std::io::Error) -> Box<ArchitectureDiagnostic> {
    Box::new(ArchitectureDiagnostic {
        code: "architecture_inventory.io_failed".to_string(),
        source_path: Some(path.to_string_lossy().replace('\\', "/")),
        domain: None,
        subject: None,
        stage: "architecture_inventory".to_string(),
        observed_evidence: error.to_string(),
        rule_id: None,
        classification: "io_failed".to_string(),
        next_action: "Restore readable committed source files.".to_string(),
    })
}

#[derive(Default)]
struct RustInventoryVisitor {
    symbols: BTreeSet<String>,
    imports: BTreeSet<String>,
    impls: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for RustInventoryVisitor {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.symbols.insert(format!("fn:{}", node.sig.ident));
        visit::visit_item_fn(self, node);
    }

    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        self.symbols.insert(format!("struct:{}", node.ident));
        visit::visit_item_struct(self, node);
    }

    fn visit_item_enum(&mut self, node: &'ast syn::ItemEnum) {
        self.symbols.insert(format!("enum:{}", node.ident));
        visit::visit_item_enum(self, node);
    }

    fn visit_item_trait(&mut self, node: &'ast syn::ItemTrait) {
        self.symbols.insert(format!("trait:{}", node.ident));
        visit::visit_item_trait(self, node);
    }

    fn visit_item_use(&mut self, node: &'ast syn::ItemUse) {
        self.imports.insert(node.tree.to_token_stream().to_string());
        visit::visit_item_use(self, node);
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        self.impls
            .insert(node.self_ty.to_token_stream().to_string());
        visit::visit_item_impl(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::architecture_policy::{
        ArchitecturePolicy, DomainPolicy, PolicyMode, PolicyProfile,
        ARCHITECTURE_POLICY_SCHEMA_VERSION,
    };
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn architecture_inventory_uses_syn_symbols_and_metadata_edges() {
        let root = temp_root();
        fs::create_dir_all(root.join("crates/sample/src")).unwrap();
        fs::write(
            root.join("crates/sample/src/lib.rs"),
            "use std::fmt; pub struct Sample; impl Sample { pub fn run() {} }",
        )
        .unwrap();
        fs::write(
            root.join("crates/sample/Cargo.toml"),
            "[package]\nname='sample'\nversion='0.1.0'\n",
        )
        .unwrap();
        let policy = test_policy();
        let metadata = br#"{"packages":[{"name":"sample","dependencies":[{"name":"serde"}]}]}"#;

        let inventory = build_inventory(&root, &policy, "engine", metadata).unwrap();

        let rust = inventory
            .files
            .iter()
            .find(|file| file.path.ends_with("lib.rs"))
            .unwrap();
        assert!(rust.symbols.contains(&"struct:Sample".to_string()));
        assert!(rust.imports.iter().any(|import| import.contains("std")));
        assert_eq!(inventory.crate_dependencies[0].dependency, "serde");
        fs::remove_dir_all(root).unwrap();
    }

    fn test_policy() -> ArchitecturePolicy {
        ArchitecturePolicy {
            schema_version: ARCHITECTURE_POLICY_SCHEMA_VERSION.to_string(),
            profiles: vec![
                profile("engine", PolicyMode::EngineStrict),
                profile("advisory", PolicyMode::ProjectAdvisory),
                profile("strict", PolicyMode::ProjectStrict),
            ],
            domains: vec![DomainPolicy {
                id: "sample".to_string(),
                owner: "test".to_string(),
                include: vec!["crates/sample/**".to_string()],
                facade_only: Vec::new(),
            }],
            dependency_rules: Vec::new(),
            review_authorities: vec!["test".to_string()],
        }
    }

    fn profile(id: &str, mode: PolicyMode) -> PolicyProfile {
        PolicyProfile {
            id: id.to_string(),
            mode,
            include: vec!["crates/**".to_string()],
            exclude: Vec::new(),
        }
    }

    fn temp_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("architecture_inventory_{nanos}"))
    }
}
