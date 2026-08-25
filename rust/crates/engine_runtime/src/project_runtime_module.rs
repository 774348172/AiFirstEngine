use crate::aui::{
    AuiSnapshotSource, ProjectUiStateIdentity, ProjectUiStateProducerContext,
    ProjectUiStateResolve, ProjectUiStateResolveError, ProjectUiStateSnapshot,
    ProjectUiStateSnapshotOutput, ProjectUiStateSnapshotProducer,
};
use crate::canonical_digest::{sha256_prefixed, CanonicalDigestError, ConsistencyDigest};
use crate::logic_executor::RustAotRule;
use crate::project_logic::ProjectLogicRunner;
use crate::project_runtime_session::{
    create_empty_project_runtime_session, ProjectRuntimeSession,
    ProjectRuntimeSessionCreateContext, ProjectRuntimeSessionFactory,
};
use crate::rule_registry::{RuleModuleRegistry, RuleRegistryError};
use crate::runtime_package::{RuntimePackage, RuntimeProjectModuleRef};
use engine_input::InputMappingAsset;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

pub const PROJECT_RUNTIME_MODULE_INTERFACE_VERSION: &str = "project-runtime-module.v2";
pub const PROJECT_RUNTIME_BIND_RECEIPT_SCHEMA_VERSION: &str = "project-runtime-bind-receipt.v2";
pub const EMPTY_PROJECT_RUNTIME_MODULE_ID: &str = "engine.empty.runtime";
pub const EMPTY_PROJECT_RUNTIME_AOT_DIGEST: &str = "sha256:engine-empty-runtime-v2";

pub struct ProjectRuntimeAotDigestSource<'a> {
    pub relative_path: &'a str,
    pub bytes: &'a [u8],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectRuntimeAotDigestPayload<'a> {
    module_id: &'a str,
    interface_version: &'a str,
    cargo_manifest: &'a str,
    cargo_package: &'a str,
    player_binary: &'a str,
    sources: Vec<ProjectRuntimeAotDigestSourceHash<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectRuntimeAotDigestSourceHash<'a> {
    relative_path: &'a str,
    content_hash: String,
}

pub fn project_runtime_aot_digest<'a>(
    module_id: &str,
    interface_version: &str,
    cargo_manifest: &str,
    cargo_package: &str,
    player_binary: &str,
    sources: impl IntoIterator<Item = ProjectRuntimeAotDigestSource<'a>>,
) -> Result<String, CanonicalDigestError> {
    let mut sources = sources
        .into_iter()
        .map(|source| ProjectRuntimeAotDigestSourceHash {
            relative_path: source.relative_path,
            content_hash: sha256_prefixed(source.bytes),
        })
        .collect::<Vec<_>>();
    sources.sort_by(|left, right| left.relative_path.cmp(right.relative_path));
    let payload = ProjectRuntimeAotDigestPayload {
        module_id,
        interface_version,
        cargo_manifest,
        cargo_package,
        player_binary,
        sources,
    };
    Ok(ConsistencyDigest::sha256(
        "project-runtime-module-aot-input",
        "project-runtime-module-aot-input.v1",
        &payload,
    )?
    .prefixed_value())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeModuleDescriptor {
    pub module_id: String,
    pub interface_version: String,
    pub aot_content_digest: String,
}

impl ProjectRuntimeModuleDescriptor {
    pub fn new(module_id: impl Into<String>, aot_content_digest: impl Into<String>) -> Self {
        Self {
            module_id: module_id.into(),
            interface_version: PROJECT_RUNTIME_MODULE_INTERFACE_VERSION.to_string(),
            aot_content_digest: aot_content_digest.into(),
        }
    }

    pub fn empty() -> Self {
        Self::new(
            EMPTY_PROJECT_RUNTIME_MODULE_ID,
            EMPTY_PROJECT_RUNTIME_AOT_DIGEST,
        )
    }

    fn matches(&self, requested: &RuntimeProjectModuleRef) -> Result<(), ProjectRuntimeError> {
        if self.module_id != requested.module_id {
            return Err(ProjectRuntimeError::new(
                "project_runtime.module_id_mismatch",
                "match_descriptor",
                format!(
                    "RuntimePackage requests module '{}', but linked module is '{}'.",
                    requested.module_id, self.module_id
                ),
                "Build or select the project player/editor linked with the requested module.",
            ));
        }
        if self.interface_version != requested.interface_version {
            return Err(ProjectRuntimeError::new(
                "project_runtime.interface_version_mismatch",
                "match_descriptor",
                format!(
                    "RuntimePackage requests interface '{}', but linked module provides '{}'.",
                    requested.interface_version, self.interface_version
                ),
                "Rebuild the RuntimePackage and linked project module with the same interface version.",
            ));
        }
        if self.aot_content_digest != requested.aot_content_digest {
            return Err(ProjectRuntimeError::new(
                "project_runtime.aot_digest_mismatch",
                "match_descriptor",
                format!(
                    "RuntimePackage AOT digest '{}' does not match linked module digest '{}'.",
                    requested.aot_content_digest, self.aot_content_digest
                ),
                "Rebuild the RuntimePackage and project player from the same project module inputs.",
            ));
        }
        Ok(())
    }
}

pub trait ProjectRuntimeModule: Send + Sync {
    fn descriptor(&self) -> &ProjectRuntimeModuleDescriptor;

    fn install(
        &self,
        registration: &mut ProjectRuntimeRegistration,
    ) -> Result<(), ProjectRuntimeError>;
}

pub struct EmptyProjectRuntimeModule {
    descriptor: ProjectRuntimeModuleDescriptor,
}

impl EmptyProjectRuntimeModule {
    pub fn new() -> Self {
        Self {
            descriptor: ProjectRuntimeModuleDescriptor::empty(),
        }
    }
}

impl Default for EmptyProjectRuntimeModule {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectRuntimeModule for EmptyProjectRuntimeModule {
    fn descriptor(&self) -> &ProjectRuntimeModuleDescriptor {
        &self.descriptor
    }

    fn install(
        &self,
        registration: &mut ProjectRuntimeRegistration,
    ) -> Result<(), ProjectRuntimeError> {
        registration.set_runtime_session_factory(create_empty_project_runtime_session)?;
        registration.set_ui_state_producer_factory(create_empty_ui_state_producer)
    }
}

struct EmptyProjectUiStateProducer;

impl ProjectUiStateSnapshotProducer for EmptyProjectUiStateProducer {
    fn producer_id(&self) -> &str {
        "engine_empty_project_ui_state"
    }

    fn produce(
        &mut self,
        context: ProjectUiStateProducerContext<'_>,
    ) -> ProjectUiStateSnapshotOutput {
        ProjectUiStateSnapshotOutput::new(
            self.producer_id(),
            AuiSnapshotSource::EmptyDefaultSnapshot,
            ProjectUiStateSnapshot::new(context.frame_index),
        )
    }

    fn resolve(
        &mut self,
        context: ProjectUiStateProducerContext<'_>,
    ) -> Result<ProjectUiStateResolve, ProjectUiStateResolveError> {
        let identity = ProjectUiStateIdentity {
            producer_epoch: 1,
            visible_revision: 0,
            binding_set: context.binding_set.identity().clone(),
        };
        if context.previous_identity.as_ref() == Some(&identity) {
            return Ok(ProjectUiStateResolve::Reuse { identity });
        }
        let output = self.produce(context);
        Ok(ProjectUiStateResolve::Replace { identity, output })
    }
}

fn create_empty_ui_state_producer() -> Box<dyn ProjectUiStateSnapshotProducer> {
    Box::new(EmptyProjectUiStateProducer)
}

pub type ProjectUiStateProducerFactory =
    Arc<dyn Fn() -> Box<dyn ProjectUiStateSnapshotProducer> + Send + Sync>;

pub struct ProjectRuntimeSessionBundle {
    pub project_runtime_session: Box<dyn ProjectRuntimeSession>,
    pub ui_state_producer: Box<dyn ProjectUiStateSnapshotProducer>,
}

pub type ProjectRuntimeSessionBundleFactory = Arc<
    dyn for<'a> Fn(
            ProjectRuntimeSessionCreateContext<'a>,
        ) -> Result<
            ProjectRuntimeSessionBundle,
            crate::project_runtime_session::ProjectRuntimeSessionFactoryError,
        > + Send
        + Sync,
>;

#[derive(Clone)]
struct RegisteredProjectRule {
    artifact_id: String,
    rule: RustAotRule,
}

pub struct ProjectRuntimeRegistration {
    rules: BTreeMap<String, RegisteredProjectRule>,
    runtime_session_factory: Option<ProjectRuntimeSessionFactory>,
    ui_state_producer_factory: Option<ProjectUiStateProducerFactory>,
    session_bundle_factory: Option<ProjectRuntimeSessionBundleFactory>,
}

impl ProjectRuntimeRegistration {
    fn new() -> Self {
        Self {
            rules: BTreeMap::new(),
            runtime_session_factory: None,
            ui_state_producer_factory: None,
            session_bundle_factory: None,
        }
    }

    pub fn register_rust_aot_rule(
        &mut self,
        rule_id: impl Into<String>,
        artifact_id: impl Into<String>,
        rule: impl for<'a> Fn(
                &mut crate::logic_executor::LogicContext<'a>,
            ) -> crate::logic_executor::LogicResult
            + Send
            + Sync
            + 'static,
    ) -> Result<(), ProjectRuntimeError> {
        let rule_id = rule_id.into();
        let artifact_id = artifact_id.into();
        if rule_id.trim().is_empty() || artifact_id.trim().is_empty() {
            return Err(ProjectRuntimeError::new(
                "project_runtime.registration_failed",
                "register_rule",
                "Project rule id and artifact id are required.",
                "Regenerate the project runtime module registration.",
            ));
        }
        if self
            .rules
            .insert(
                rule_id.clone(),
                RegisteredProjectRule {
                    artifact_id,
                    rule: RustAotRule::new(rule),
                },
            )
            .is_some()
        {
            return Err(ProjectRuntimeError::new(
                "project_runtime.duplicate_rule",
                "register_rule",
                format!("Project runtime module registered duplicate rule '{rule_id}'."),
                "Remove the duplicate project rule registration.",
            )
            .with_rule_id(rule_id));
        }
        Ok(())
    }

    pub fn set_ui_state_producer_factory(
        &mut self,
        factory: impl Fn() -> Box<dyn ProjectUiStateSnapshotProducer> + Send + Sync + 'static,
    ) -> Result<(), ProjectRuntimeError> {
        if self
            .ui_state_producer_factory
            .replace(Arc::new(factory))
            .is_some()
        {
            return Err(ProjectRuntimeError::new(
                "project_runtime.registration_failed",
                "register_ui_state_producer",
                "Project runtime module registered more than one UI state producer factory.",
                "Keep exactly one ProjectUiStateSnapshotProducer factory per project module.",
            ));
        }
        Ok(())
    }

    pub fn set_runtime_session_factory(
        &mut self,
        factory: impl for<'a> Fn(
                ProjectRuntimeSessionCreateContext<'a>,
            ) -> Result<
                Box<dyn ProjectRuntimeSession>,
                crate::project_runtime_session::ProjectRuntimeSessionFactoryError,
            > + Send
            + Sync
            + 'static,
    ) -> Result<(), ProjectRuntimeError> {
        if self
            .runtime_session_factory
            .replace(Arc::new(factory))
            .is_some()
        {
            return Err(ProjectRuntimeError::new(
                "project_runtime.session_duplicate",
                "register_runtime_session",
                "Project runtime module registered more than one runtime session factory.",
                "Keep exactly one ProjectRuntimeSession factory per project module.",
            ));
        }
        Ok(())
    }

    pub fn set_runtime_session_bundle_factory(
        &mut self,
        factory: impl for<'a> Fn(
                ProjectRuntimeSessionCreateContext<'a>,
            ) -> Result<
                ProjectRuntimeSessionBundle,
                crate::project_runtime_session::ProjectRuntimeSessionFactoryError,
            > + Send
            + Sync
            + 'static,
    ) -> Result<(), ProjectRuntimeError> {
        if self
            .session_bundle_factory
            .replace(Arc::new(factory))
            .is_some()
        {
            return Err(ProjectRuntimeError::new(
                "project_runtime.session_bundle_duplicate",
                "register_runtime_session_bundle",
                "Project runtime module registered more than one session bundle factory.",
                "Keep exactly one ProjectRuntimeSessionBundle factory per project module.",
            ));
        }
        Ok(())
    }

    fn into_runtime_parts(
        self,
        package: &RuntimePackage,
    ) -> Result<RegistrationRuntimeParts, ProjectRuntimeError> {
        let mut registry = RuleModuleRegistry::new();
        for (rule_id, registered) in self.rules {
            registry.register_generated_rule_artifact_value(
                rule_id,
                registered.artifact_id,
                registered.rule,
            );
        }
        let project_logic = registry
            .build_runner_strict(&package.rules)
            .map_err(ProjectRuntimeError::from_rule_registry)?;
        let create_context = ProjectRuntimeSessionCreateContext {
            project_id: &package.manifest.project.project_id,
            module_id: &package.manifest.project.runtime_module.module_id,
        };
        let (project_runtime_session, ui_state_producer) = if let Some(factory) =
            self.session_bundle_factory
        {
            let bundle = factory(create_context).map_err(|error| {
                ProjectRuntimeError::new(
                    "project_runtime.session_factory_failed",
                    "create_runtime_session_bundle",
                    error.message,
                    "Fix the project runtime session bundle factory before launching the runtime.",
                )
            })?;
            (bundle.project_runtime_session, bundle.ui_state_producer)
        } else {
            let runtime_session_factory = self.runtime_session_factory.ok_or_else(|| {
            ProjectRuntimeError::new(
                "project_runtime.session_missing",
                "finalize_registration",
                "Project runtime module did not register a runtime session factory.",
                "Register an explicit ProjectRuntimeSession factory, including a no-op session for stateless projects.",
            )
        })?;
            let project_runtime_session =
                runtime_session_factory(create_context).map_err(|error| {
                    ProjectRuntimeError::new(
                        "project_runtime.session_factory_failed",
                        "create_runtime_session",
                        error.message,
                        "Fix the project runtime session factory before launching the runtime.",
                    )
                })?;
            let ui_state_producer = self
            .ui_state_producer_factory
            .map(|factory| factory())
            .ok_or_else(|| {
                ProjectRuntimeError::new(
                    "project_runtime.ui_producer_missing",
                    "finalize_registration",
                    "Project runtime module did not register a UI state producer factory.",
                    "Register a project UI state producer, including a no-op producer for projects without AUI bindings.",
                )
            })?;
            (project_runtime_session, ui_state_producer)
        };
        if project_runtime_session.session_id().trim().is_empty() {
            return Err(ProjectRuntimeError::new(
                "project_runtime.session_id_missing",
                "create_runtime_session",
                "Project runtime session factory returned an empty session id.",
                "Return a stable non-empty session id from ProjectRuntimeSession::session_id.",
            ));
        }
        Ok(RegistrationRuntimeParts {
            project_logic,
            project_runtime_session,
            ui_state_producer,
            registered_rule_count: registry.len(),
        })
    }
}

struct RegistrationRuntimeParts {
    project_logic: ProjectLogicRunner,
    project_runtime_session: Box<dyn ProjectRuntimeSession>,
    ui_state_producer: Box<dyn ProjectUiStateSnapshotProducer>,
    registered_rule_count: usize,
}

#[derive(Default)]
pub struct LinkedProjectRuntimeSet {
    modules: BTreeMap<String, Arc<dyn ProjectRuntimeModule>>,
}

impl LinkedProjectRuntimeSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn singleton(module: Arc<dyn ProjectRuntimeModule>) -> Result<Self, ProjectRuntimeError> {
        let mut set = Self::new();
        set.add(module)?;
        Ok(set)
    }

    pub fn explicit_empty() -> Self {
        Self::singleton(Arc::new(EmptyProjectRuntimeModule::new()))
            .expect("built-in empty project runtime module descriptor is valid")
    }

    pub fn add(
        &mut self,
        module: Arc<dyn ProjectRuntimeModule>,
    ) -> Result<&mut Self, ProjectRuntimeError> {
        let module_id = module.descriptor().module_id.clone();
        if module_id.trim().is_empty() {
            return Err(ProjectRuntimeError::new(
                "project_runtime.registration_failed",
                "link_module",
                "Linked project runtime module id is required.",
                "Regenerate the project module descriptor.",
            ));
        }
        if self.modules.insert(module_id.clone(), module).is_some() {
            return Err(ProjectRuntimeError::new(
                "project_runtime.duplicate_linked_module_id",
                "link_module",
                format!("Linked project runtime set contains duplicate module id '{module_id}'."),
                "Ensure each statically linked project module id is unique.",
            ));
        }
        Ok(self)
    }

    pub fn len(&self) -> usize {
        self.modules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }

    pub fn only_descriptor(&self) -> Result<&ProjectRuntimeModuleDescriptor, ProjectRuntimeError> {
        if self.modules.len() != 1 {
            return Err(ProjectRuntimeError::new(
                "project_runtime.singleton_descriptor_required",
                "describe_module",
                format!(
                    "Project runtime host requires exactly one linked module; found {}.",
                    self.modules.len()
                ),
                "Generate a project-specific host with exactly one linked runtime module.",
            ));
        }
        Ok(self
            .modules
            .values()
            .next()
            .expect("singleton module set contains one module")
            .descriptor())
    }

    pub fn descriptor_for_module_id(
        &self,
        module_id: &str,
    ) -> Option<&ProjectRuntimeModuleDescriptor> {
        self.modules
            .get(module_id)
            .map(|module| module.descriptor())
    }

    fn resolve(
        &self,
        requested: &RuntimeProjectModuleRef,
    ) -> Result<Arc<dyn ProjectRuntimeModule>, ProjectRuntimeError> {
        if let Some(module) = self.modules.get(&requested.module_id) {
            return Ok(Arc::clone(module));
        }
        if self.modules.len() == 1 {
            return Ok(Arc::clone(
                self.modules
                    .values()
                    .next()
                    .expect("singleton linked set contains one module"),
            ));
        }
        Err(ProjectRuntimeError::new(
            "project_runtime.module_not_linked",
            "resolve_module",
            format!(
                "RuntimePackage requests project module '{}', which is not linked into this host.",
                requested.module_id
            ),
            "Rebuild and relaunch the editor/player with the requested project module linked.",
        ))
    }
}

pub struct ProjectRuntimeBootstrap;

impl ProjectRuntimeBootstrap {
    pub fn bind(
        package: &RuntimePackage,
        linked_modules: &LinkedProjectRuntimeSet,
    ) -> Result<BoundProjectRuntime, ProjectRuntimeError> {
        let requested = &package.manifest.project.runtime_module;
        let module = linked_modules.resolve(requested)?;
        module.descriptor().matches(requested)?;

        let mut registration = ProjectRuntimeRegistration::new();
        module.install(&mut registration).map_err(|error| {
            if error.stage.is_empty() {
                ProjectRuntimeError {
                    stage: "install_module".to_string(),
                    ..error
                }
            } else {
                error
            }
        })?;
        let parts = registration.into_runtime_parts(package)?;
        let default_input_mapping = package.default_input_mapping.clone().ok_or_else(|| {
            ProjectRuntimeError::new(
                "project_runtime.default_input_missing",
                "bind_input",
                "RuntimePackage v2 does not contain its declared default InputMappingAsset.",
                "Add an explicit project InputMappingAsset (or input.none) and rebuild.",
            )
        })?;
        let receipt = ProjectRuntimeBindReceipt {
            schema_version: PROJECT_RUNTIME_BIND_RECEIPT_SCHEMA_VERSION.to_string(),
            project_id: package.manifest.project.project_id.clone(),
            module_id: requested.module_id.clone(),
            interface_version: requested.interface_version.clone(),
            aot_content_digest: requested.aot_content_digest.clone(),
            registered_rule_count: parts.registered_rule_count,
            required_rule_count: package
                .rules
                .rules
                .iter()
                .filter(|rule| rule.enabled)
                .count(),
            producer_id: parts.ui_state_producer.producer_id().to_string(),
            session_id: parts.project_runtime_session.session_id().to_string(),
            session_status: "ready".to_string(),
            default_input_mapping_id: default_input_mapping.asset_id.clone(),
            status: "passed".to_string(),
        };
        Ok(BoundProjectRuntime {
            project_logic: parts.project_logic,
            project_runtime_session: parts.project_runtime_session,
            ui_state_producer: parts.ui_state_producer,
            default_input_mapping,
            receipt,
        })
    }
}

pub struct BoundProjectRuntime {
    project_logic: ProjectLogicRunner,
    project_runtime_session: Box<dyn ProjectRuntimeSession>,
    ui_state_producer: Box<dyn ProjectUiStateSnapshotProducer>,
    default_input_mapping: InputMappingAsset,
    receipt: ProjectRuntimeBindReceipt,
}

impl BoundProjectRuntime {
    pub fn project_logic(&self) -> &ProjectLogicRunner {
        &self.project_logic
    }

    pub fn into_project_logic(self) -> ProjectLogicRunner {
        self.project_logic
    }

    pub fn project_runtime_session(&self) -> &dyn ProjectRuntimeSession {
        self.project_runtime_session.as_ref()
    }

    pub fn project_runtime_session_mut(&mut self) -> &mut dyn ProjectRuntimeSession {
        self.project_runtime_session.as_mut()
    }

    pub fn ui_state_producer_mut(&mut self) -> &mut dyn ProjectUiStateSnapshotProducer {
        self.ui_state_producer.as_mut()
    }

    pub fn default_input_mapping(&self) -> &InputMappingAsset {
        &self.default_input_mapping
    }

    pub fn receipt(&self) -> &ProjectRuntimeBindReceipt {
        &self.receipt
    }

    pub fn into_parts(self) -> BoundProjectRuntimeParts {
        BoundProjectRuntimeParts {
            project_logic: self.project_logic,
            project_runtime_session: self.project_runtime_session,
            ui_state_producer: self.ui_state_producer,
            default_input_mapping: self.default_input_mapping,
            receipt: self.receipt,
        }
    }
}

pub struct BoundProjectRuntimeParts {
    pub project_logic: ProjectLogicRunner,
    pub project_runtime_session: Box<dyn ProjectRuntimeSession>,
    pub ui_state_producer: Box<dyn ProjectUiStateSnapshotProducer>,
    pub default_input_mapping: InputMappingAsset,
    pub receipt: ProjectRuntimeBindReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRuntimeBindReceipt {
    pub schema_version: String,
    pub project_id: String,
    pub module_id: String,
    pub interface_version: String,
    pub aot_content_digest: String,
    pub registered_rule_count: usize,
    pub required_rule_count: usize,
    pub producer_id: String,
    pub session_id: String,
    pub session_status: String,
    pub default_input_mapping_id: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRuntimeError {
    pub code: &'static str,
    pub stage: String,
    pub message: String,
    pub next_action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_id: Option<String>,
}

impl ProjectRuntimeError {
    pub fn new(
        code: &'static str,
        stage: impl Into<String>,
        message: impl Into<String>,
        next_action: impl Into<String>,
    ) -> Self {
        Self {
            code,
            stage: stage.into(),
            message: message.into(),
            next_action: next_action.into(),
            rule_id: None,
            artifact_id: None,
        }
    }

    pub fn with_rule_id(mut self, rule_id: impl Into<String>) -> Self {
        self.rule_id = Some(rule_id.into());
        self
    }

    fn from_rule_registry(error: RuleRegistryError) -> Self {
        let code = match error.code {
            "missing_registered_rule" => "project_runtime.missing_linked_rule",
            "missing_rule_ir_hash"
            | "missing_rule_artifact_id"
            | "missing_rule_artifact_identity"
            | "missing_rule_module_for_artifact"
            | "rule_artifact_id_mismatch"
            | "missing_registered_rule_artifact"
            | "registered_rule_artifact_mismatch" => "project_runtime.rule_artifact_mismatch",
            "unsupported_rule_module_kind" => "project_runtime.unsupported_rule_module_kind",
            "unsupported_rule_executor" => "project_runtime.unsupported_rule_executor",
            _ => "project_runtime.registration_failed",
        };
        let artifact_id = error.artifact_id.clone();
        let mut mapped = Self::new(
            code,
            "validate_rule_registry",
            error.message,
            "Rebuild the project runtime module and RuntimePackage from the same rule artifacts.",
        )
        .with_rule_id(error.rule_id);
        mapped.artifact_id = artifact_id;
        mapped
    }
}

impl fmt::Display for ProjectRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} [{}]: {}",
            self.code, self.stage, self.message
        )
    }
}

impl std::error::Error for ProjectRuntimeError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aui::{
        AuiSnapshotSource, ProjectUiStateProducerContext, ProjectUiStateSnapshot,
        ProjectUiStateSnapshotOutput,
    };
    use crate::project_runtime_session::{
        ProjectAuiActionBatch, ProjectRuntimeSessionContext, ProjectRuntimeSessionFactoryError,
        ProjectRuntimeSessionOutput,
    };
    use crate::runtime_package::{
        load_runtime_package, RuntimeProjectInfo, RuntimeProjectModuleRef, RuntimeScene,
        RUNTIME_SCENE_SCHEMA_VERSION,
    };
    use crate::runtime_package_builder::{
        RuntimePackageBuildInput, RuntimePackageBuildRequest, RuntimePackageBuildStatus,
        RuntimePackageBuilder, RuntimePackageSourceJson,
    };
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestProducer;

    impl ProjectUiStateSnapshotProducer for TestProducer {
        fn producer_id(&self) -> &str {
            "test-project-ui"
        }

        fn produce(
            &mut self,
            context: ProjectUiStateProducerContext<'_>,
        ) -> ProjectUiStateSnapshotOutput {
            ProjectUiStateSnapshotOutput::new(
                self.producer_id(),
                AuiSnapshotSource::ProjectProducer,
                ProjectUiStateSnapshot::new(context.frame_index),
            )
        }
    }

    fn test_producer() -> Box<dyn ProjectUiStateSnapshotProducer> {
        Box::new(TestProducer)
    }

    struct TestModule {
        descriptor: ProjectRuntimeModuleDescriptor,
    }

    impl TestModule {
        fn exact() -> Self {
            Self {
                descriptor: ProjectRuntimeModuleDescriptor::new(
                    "sample.test.runtime",
                    "sha256:test-runtime-v1",
                ),
            }
        }
    }

    impl ProjectRuntimeModule for TestModule {
        fn descriptor(&self) -> &ProjectRuntimeModuleDescriptor {
            &self.descriptor
        }

        fn install(
            &self,
            registration: &mut ProjectRuntimeRegistration,
        ) -> Result<(), ProjectRuntimeError> {
            registration.set_runtime_session_factory(create_test_session)?;
            registration.set_ui_state_producer_factory(test_producer)
        }
    }

    struct TestSession {
        session_id: String,
    }

    impl ProjectRuntimeSession for TestSession {
        fn session_id(&self) -> &str {
            &self.session_id
        }

        fn handle_aui_actions(
            &mut self,
            _context: ProjectRuntimeSessionContext<'_>,
            _batch: ProjectAuiActionBatch<'_>,
        ) -> ProjectRuntimeSessionOutput {
            ProjectRuntimeSessionOutput::no_op()
        }

        fn fixed_update(
            &mut self,
            _context: ProjectRuntimeSessionContext<'_>,
        ) -> ProjectRuntimeSessionOutput {
            ProjectRuntimeSessionOutput::no_op()
        }
    }

    static NEXT_TEST_SESSION_ID: AtomicU64 = AtomicU64::new(1);

    fn create_test_session(
        _context: ProjectRuntimeSessionCreateContext<'_>,
    ) -> Result<Box<dyn ProjectRuntimeSession>, ProjectRuntimeSessionFactoryError> {
        let sequence = NEXT_TEST_SESSION_ID.fetch_add(1, Ordering::Relaxed);
        Ok(Box::new(TestSession {
            session_id: format!("test-session-{sequence}"),
        }))
    }

    fn fail_test_session(
        _context: ProjectRuntimeSessionCreateContext<'_>,
    ) -> Result<Box<dyn ProjectRuntimeSession>, ProjectRuntimeSessionFactoryError> {
        Err(ProjectRuntimeSessionFactoryError::new(
            "intentional factory failure",
        ))
    }

    fn create_empty_id_session(
        _context: ProjectRuntimeSessionCreateContext<'_>,
    ) -> Result<Box<dyn ProjectRuntimeSession>, ProjectRuntimeSessionFactoryError> {
        Ok(Box::new(TestSession {
            session_id: " ".to_string(),
        }))
    }

    enum TestModuleRegistration {
        Missing,
        Duplicate,
        FactoryError,
        EmptyId,
    }

    struct SessionRegistrationTestModule {
        descriptor: ProjectRuntimeModuleDescriptor,
        registration: TestModuleRegistration,
    }

    impl SessionRegistrationTestModule {
        fn new(registration: TestModuleRegistration) -> Self {
            Self {
                descriptor: TestModule::exact().descriptor,
                registration,
            }
        }
    }

    impl ProjectRuntimeModule for SessionRegistrationTestModule {
        fn descriptor(&self) -> &ProjectRuntimeModuleDescriptor {
            &self.descriptor
        }

        fn install(
            &self,
            registration: &mut ProjectRuntimeRegistration,
        ) -> Result<(), ProjectRuntimeError> {
            match self.registration {
                TestModuleRegistration::Missing => {}
                TestModuleRegistration::Duplicate => {
                    registration.set_runtime_session_factory(create_test_session)?;
                    registration.set_runtime_session_factory(create_test_session)?;
                }
                TestModuleRegistration::FactoryError => {
                    registration.set_runtime_session_factory(fail_test_session)?;
                }
                TestModuleRegistration::EmptyId => {
                    registration.set_runtime_session_factory(create_empty_id_session)?;
                }
            }
            registration.set_ui_state_producer_factory(test_producer)
        }
    }

    fn package() -> RuntimePackage {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let package_dir = std::env::temp_dir()
            .join(format!("project-runtime-module-{stamp}"))
            .join("runtime-package");
        let mut input = RuntimePackageBuildInput::new(RuntimeProjectInfo::new(
            "project-test",
            "Test Project",
            "0.0.2",
            RuntimeProjectModuleRef::new(
                "sample.test.runtime",
                PROJECT_RUNTIME_MODULE_INTERFACE_VERSION,
                "sha256:test-runtime-v1",
            ),
        ));
        input.scenes.push(RuntimeScene {
            schema_version: RUNTIME_SCENE_SCHEMA_VERSION.to_string(),
            id: "scene-main".to_string(),
            name: "Main".to_string(),
            gravity: 0.0,
            background: "#000000".to_string(),
            sky_color: "#000000".to_string(),
            entities: Vec::new(),
        });
        let input_none = InputMappingAsset::explicit_empty("input.none");
        input.input_mappings.push(RuntimePackageSourceJson {
            id: input_none.asset_id.clone(),
            document: serde_json::to_value(input_none).unwrap(),
        });
        let request = RuntimePackageBuildRequest::dev_desktop(&package_dir, "scene-main");
        let report = RuntimePackageBuilder::build(&request, &input);
        assert_eq!(report.status, RuntimePackageBuildStatus::Success);
        let loaded = load_runtime_package(&package_dir);
        assert!(
            loaded.diagnostics.is_ok(),
            "{:?}",
            loaded.diagnostics.issues
        );
        loaded.value.unwrap()
    }

    #[test]
    fn bootstrap_binds_exact_linked_module_once() {
        let package = package();
        let linked = LinkedProjectRuntimeSet::singleton(Arc::new(TestModule::exact())).unwrap();

        let bound = ProjectRuntimeBootstrap::bind(&package, &linked).unwrap();

        assert_eq!(bound.receipt().project_id, "project-test");
        assert_eq!(bound.receipt().module_id, "sample.test.runtime");
        assert_eq!(bound.receipt().registered_rule_count, 0);
        assert_eq!(bound.receipt().default_input_mapping_id, "input.none");
    }

    #[test]
    fn project_runtime_session_missing_factory_fails() {
        let linked = LinkedProjectRuntimeSet::singleton(Arc::new(
            SessionRegistrationTestModule::new(TestModuleRegistration::Missing),
        ))
        .unwrap();

        let error = ProjectRuntimeBootstrap::bind(&package(), &linked)
            .err()
            .expect("missing session factory must fail before binding");

        assert_eq!(error.code, "project_runtime.session_missing");
    }

    #[test]
    fn project_runtime_session_duplicate_factory_fails() {
        let linked = LinkedProjectRuntimeSet::singleton(Arc::new(
            SessionRegistrationTestModule::new(TestModuleRegistration::Duplicate),
        ))
        .unwrap();

        let error = ProjectRuntimeBootstrap::bind(&package(), &linked)
            .err()
            .expect("duplicate session factory must fail");

        assert_eq!(error.code, "project_runtime.session_duplicate");
    }

    #[test]
    fn project_runtime_session_factory_error_fails() {
        let linked = LinkedProjectRuntimeSet::singleton(Arc::new(
            SessionRegistrationTestModule::new(TestModuleRegistration::FactoryError),
        ))
        .unwrap();

        let error = ProjectRuntimeBootstrap::bind(&package(), &linked)
            .err()
            .expect("session factory error must fail");

        assert_eq!(error.code, "project_runtime.session_factory_failed");
        assert!(error.message.contains("intentional factory failure"));
    }

    #[test]
    fn project_runtime_session_empty_id_fails() {
        let linked = LinkedProjectRuntimeSet::singleton(Arc::new(
            SessionRegistrationTestModule::new(TestModuleRegistration::EmptyId),
        ))
        .unwrap();

        let error = ProjectRuntimeBootstrap::bind(&package(), &linked)
            .err()
            .expect("empty session id must fail");

        assert_eq!(error.code, "project_runtime.session_id_missing");
    }

    #[test]
    fn project_runtime_session_two_binds_create_distinct_sessions() {
        let linked = LinkedProjectRuntimeSet::singleton(Arc::new(TestModule::exact())).unwrap();

        let first = ProjectRuntimeBootstrap::bind(&package(), &linked).unwrap();
        let second = ProjectRuntimeBootstrap::bind(&package(), &linked).unwrap();

        assert_ne!(
            first.project_runtime_session().session_id(),
            second.project_runtime_session().session_id()
        );
    }

    #[test]
    fn project_runtime_session_bind_receipt_exposes_identity_and_status() {
        let linked = LinkedProjectRuntimeSet::singleton(Arc::new(TestModule::exact())).unwrap();

        let bound = ProjectRuntimeBootstrap::bind(&package(), &linked).unwrap();

        assert_eq!(
            bound.receipt().session_id,
            bound.project_runtime_session().session_id()
        );
        assert_eq!(bound.receipt().session_status, "ready");
        assert_eq!(
            bound.receipt().schema_version,
            PROJECT_RUNTIME_BIND_RECEIPT_SCHEMA_VERSION
        );
    }

    #[test]
    fn project_runtime_session_explicit_no_op_session_binds() {
        let mut package = package();
        package.manifest.project.runtime_module = RuntimeProjectModuleRef::explicit_empty();
        let linked = LinkedProjectRuntimeSet::explicit_empty();

        let bound = ProjectRuntimeBootstrap::bind(&package, &linked).unwrap();

        assert_eq!(
            bound.project_runtime_session().session_id(),
            crate::project_runtime_session::EMPTY_PROJECT_RUNTIME_SESSION_ID
        );
        assert_eq!(bound.receipt().session_status, "ready");
    }

    #[test]
    fn bootstrap_rejects_missing_module_before_runtime_creation() {
        let error = ProjectRuntimeBootstrap::bind(&package(), &LinkedProjectRuntimeSet::new())
            .err()
            .expect("unlinked module must fail closed");

        assert_eq!(error.code, "project_runtime.module_not_linked");
    }

    #[test]
    fn singleton_host_rejects_wrong_project_as_module_id_mismatch() {
        let mut wrong = TestModule::exact();
        wrong.descriptor.module_id = "sample.other.runtime".to_string();
        let linked = LinkedProjectRuntimeSet::singleton(Arc::new(wrong)).unwrap();

        let error = ProjectRuntimeBootstrap::bind(&package(), &linked)
            .err()
            .expect("wrong singleton project module must fail closed");

        assert_eq!(error.code, "project_runtime.module_id_mismatch");
    }

    #[test]
    fn linked_set_rejects_duplicate_module_id() {
        let mut linked = LinkedProjectRuntimeSet::new();
        linked.add(Arc::new(TestModule::exact())).unwrap();

        let error = linked
            .add(Arc::new(TestModule::exact()))
            .err()
            .expect("duplicate module id must fail");

        assert_eq!(error.code, "project_runtime.duplicate_linked_module_id");
    }

    #[test]
    fn linked_set_selects_descriptor_by_exact_module_id() {
        let mut linked = LinkedProjectRuntimeSet::new();
        linked.add(Arc::new(TestModule::exact())).unwrap();
        let mut unrelated = TestModule::exact();
        unrelated.descriptor.module_id = "sample.other.runtime".to_string();
        linked.add(Arc::new(unrelated)).unwrap();

        assert_eq!(
            linked
                .descriptor_for_module_id("sample.test.runtime")
                .unwrap()
                .module_id,
            "sample.test.runtime"
        );
        assert!(linked
            .descriptor_for_module_id("sample.missing.runtime")
            .is_none());
    }

    #[test]
    fn bootstrap_rejects_interface_and_digest_mismatch() {
        let package = package();
        let mut wrong_interface = TestModule::exact();
        wrong_interface.descriptor.interface_version = "project-runtime-module.v999".to_string();
        let linked = LinkedProjectRuntimeSet::singleton(Arc::new(wrong_interface)).unwrap();
        assert_eq!(
            ProjectRuntimeBootstrap::bind(&package, &linked)
                .err()
                .expect("interface mismatch must fail")
                .code,
            "project_runtime.interface_version_mismatch"
        );

        let mut wrong_digest = TestModule::exact();
        wrong_digest.descriptor.aot_content_digest = "sha256:other".to_string();
        let linked = LinkedProjectRuntimeSet::singleton(Arc::new(wrong_digest)).unwrap();
        assert_eq!(
            ProjectRuntimeBootstrap::bind(&package, &linked)
                .err()
                .expect("digest mismatch must fail")
                .code,
            "project_runtime.aot_digest_mismatch"
        );
    }

    #[test]
    fn bootstrap_rejects_missing_package_default_input() {
        let mut package = package();
        package.default_input_mapping = None;
        let linked = LinkedProjectRuntimeSet::singleton(Arc::new(TestModule::exact())).unwrap();

        let error = ProjectRuntimeBootstrap::bind(&package, &linked)
            .err()
            .expect("missing package input must fail closed");

        assert_eq!(error.code, "project_runtime.default_input_missing");
    }
}
