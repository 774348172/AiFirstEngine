use crate::archetype::ComponentValue;
use crate::aui::{
    AuiAssetRef, AuiBindingValue, AuiSnapshotSource, ProjectUiBindingSet,
    ProjectUiBindingSetIdentity, ProjectUiStateIdentity, ProjectUiStateProducerContext,
    ProjectUiStateResolve, ProjectUiStateResolveError, ProjectUiStateSnapshot,
    ProjectUiStateSnapshotOutput, ProjectUiStateSnapshotProducer,
};
use crate::component_value::RuntimeValue;
use crate::components::{ComponentTypeId, Transform};
use crate::field_path::FieldPath;
use crate::ids::EntityId;
use crate::logic_executor::{
    ExecutorKind, LogicContext, LogicError, LogicFailureLocation, LogicResult, LogicStatus,
};
use crate::math::Vec3;
use crate::project_observation::ProjectObservationValue;
use crate::project_runtime_module::{
    LinkedProjectRuntimeSet, ProjectRuntimeError, ProjectRuntimeModule,
    ProjectRuntimeModuleDescriptor, ProjectRuntimeRegistration, ProjectRuntimeSessionBundle,
};
use crate::project_runtime_session::{
    ProjectAuiActionBatch, ProjectRuntimeMutationBuffer, ProjectRuntimeObservationContext,
    ProjectRuntimeObservationOutput, ProjectRuntimeSession, ProjectRuntimeSessionContext,
    ProjectRuntimeSessionFactoryError, ProjectRuntimeSessionOutput, ProjectRuntimeSessionStatus,
};
use crate::query::QuerySpec;
use crate::runtime_time::TimeContext;
use crate::world_api::WorldReadApi;
use project_runtime_abi::{
    ProjectRuntimeAbiStatus, ProjectRuntimeApi, ProjectRuntimeByteBuffer, ProjectRuntimeByteSlice,
    ProjectRuntimeCallContext, ProjectRuntimeHostApi, ProjectRuntimeOpaqueHandle,
    ProjectRuntimeTimeContext, PROJECT_RUNTIME_ABI_MAJOR, PROJECT_RUNTIME_API_STRUCT_SIZE,
    PROJECT_RUNTIME_CALL_CONTEXT_STRUCT_SIZE, PROJECT_RUNTIME_HOST_API_STRUCT_SIZE,
};
use project_runtime_sdk::{
    call_json, call_json_once_with_buffer, ffi_boundary, project_runtime_contract_digest,
    read_input, ProjectRuntimeAuiAction, ProjectRuntimeAuiActionRequest,
    ProjectRuntimeCollisionPair, ProjectRuntimeDeferredMutation, ProjectRuntimeFrameRequest,
    ProjectRuntimeInputAction, ProjectRuntimeModuleDescriptor as SdkModuleDescriptor,
    ProjectRuntimeObservationOutput as SdkObservationOutput, ProjectRuntimeRuleOutput,
    ProjectRuntimeRuleRequest, ProjectRuntimeSessionCreateRequest,
    ProjectRuntimeSessionCreateResponse, ProjectRuntimeSessionOutput as SdkSessionOutput,
    ProjectRuntimeStatus, ProjectRuntimeTime, ProjectRuntimeTransform, ProjectRuntimeUiBindingSet,
    ProjectRuntimeUiStateIdentity, ProjectRuntimeUiStateResolveOutput,
    ProjectRuntimeUiStateResolveRequest, ProjectRuntimeValue, ProjectRuntimeWorldQueryRequest,
    ProjectRuntimeWorldQueryResponse, ProjectRuntimeWorldReadRequest,
    ProjectRuntimeWorldReadResponse, PROJECT_RUNTIME_DEFAULT_STATEFUL_OUTPUT_CAPACITY_BYTES,
};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, Weak};

#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeRuleDescriptor {
    rule_id: String,
    artifact_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeModuleDescriptor {
    module_id: String,
    interface_version: String,
    aot_content_digest: String,
    ui_state_producer_id: String,
    rules: Vec<NativeRuleDescriptor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeCallStatus {
    Applied,
    NoOp,
    Unhandled,
    Rejected,
    Faulted,
}

impl From<ProjectRuntimeStatus> for NativeCallStatus {
    fn from(status: ProjectRuntimeStatus) -> Self {
        match status {
            ProjectRuntimeStatus::Applied => Self::Applied,
            ProjectRuntimeStatus::NoOp => Self::NoOp,
            ProjectRuntimeStatus::Unhandled => Self::Unhandled,
            ProjectRuntimeStatus::Rejected => Self::Rejected,
            ProjectRuntimeStatus::Faulted => Self::Faulted,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeAdapterError {
    operation: String,
    message: String,
}

impl NativeAdapterError {
    fn failed(message: impl Into<String>) -> Self {
        Self {
            operation: "native-module".to_string(),
            message: message.into(),
        }
    }
}

impl From<NativeAdapterError> for project_runtime_sdk::ProjectRuntimeSdkError {
    fn from(error: NativeAdapterError) -> Self {
        Self {
            status: ProjectRuntimeAbiStatus::FAILED,
            message: format!("{}: {}", error.operation, error.message),
        }
    }
}
fn sdk_error(error: NativeAdapterError) -> project_runtime_sdk::ProjectRuntimeSdkError {
    error.into()
}

const NATIVE_CALL_FAILED: &str = "project_runtime.native_module_call_failed";
const NATIVE_TERMINAL_FAULT: &str = "project_runtime.native_module_terminal_fault";

#[derive(Clone)]
pub struct LoadedProjectRuntimeModuleAdapter {
    api: Arc<ProjectRuntimeApi>,
    descriptor: ProjectRuntimeModuleDescriptor,
    native_descriptor: NativeModuleDescriptor,
    producer_id: String,
    _lifetime_guard: Option<Arc<dyn Send + Sync>>,
}

impl LoadedProjectRuntimeModuleAdapter {
    pub fn new(api: ProjectRuntimeApi) -> Result<Self, ProjectRuntimeError> {
        Self::new_inner(api, None)
    }

    /// Keeps a native library or equivalent owner alive for every adapter clone and session.
    pub fn new_with_lifetime_guard<T>(
        api: ProjectRuntimeApi,
        lifetime_guard: Arc<T>,
    ) -> Result<Self, ProjectRuntimeError>
    where
        T: Send + Sync + 'static,
    {
        Self::new_inner(api, Some(lifetime_guard))
    }

    fn new_inner(
        api: ProjectRuntimeApi,
        lifetime_guard: Option<Arc<dyn Send + Sync>>,
    ) -> Result<Self, ProjectRuntimeError> {
        validate_api(&api)?;
        let api = Arc::new(api);
        let descriptor_call = required_call(api.descriptor, "descriptor")?;
        let descriptor: SdkModuleDescriptor = call_json(
            descriptor_call,
            api.module_context,
            ProjectRuntimeOpaqueHandle::NULL,
            None,
            &(),
        )
        .map_err(|error| abi_error("descriptor", error.message))?;
        validate_descriptor(&descriptor)?;
        let native_descriptor = NativeModuleDescriptor {
            module_id: descriptor.module_id.clone(),
            interface_version: descriptor.interface_version.clone(),
            aot_content_digest: descriptor.aot_content_digest.clone(),
            ui_state_producer_id: descriptor.ui_state_producer_id.clone(),
            rules: descriptor
                .rules
                .iter()
                .map(|rule| NativeRuleDescriptor {
                    rule_id: rule.rule_id.clone(),
                    artifact_id: rule.artifact_id.clone(),
                })
                .collect(),
        };
        Ok(Self {
            descriptor: ProjectRuntimeModuleDescriptor {
                module_id: native_descriptor.module_id.clone(),
                interface_version: native_descriptor.interface_version.clone(),
                aot_content_digest: native_descriptor.aot_content_digest.clone(),
            },
            producer_id: native_descriptor.ui_state_producer_id.clone(),
            native_descriptor,
            api,
            _lifetime_guard: lifetime_guard,
        })
    }
}

pub fn linked_project_runtime_set_from_api(
    api: ProjectRuntimeApi,
) -> Result<LinkedProjectRuntimeSet, ProjectRuntimeError> {
    LinkedProjectRuntimeSet::singleton(Arc::new(LoadedProjectRuntimeModuleAdapter::new(api)?))
}

#[cfg(windows)]
pub fn linked_project_runtime_set_from_dll(
    path: &std::path::Path,
) -> Result<LinkedProjectRuntimeSet, ProjectRuntimeError> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::System::LibraryLoader::{
        GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR,
        LOAD_LIBRARY_SEARCH_SYSTEM32,
    };
    let wide = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let handle = unsafe {
        LoadLibraryExW(
            wide.as_ptr(),
            std::ptr::null_mut(),
            LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32,
        )
    };
    if handle.is_null() {
        return Err(ProjectRuntimeError::new(
            "project_runtime.native_module_load_failed",
            "load_library",
            "LoadLibraryExW failed.",
            "Repair the staged project module DLL.",
        ));
    }
    let guard = Arc::new(NativeLibraryGuard(handle));
    let symbol = unsafe {
        GetProcAddress(
            handle,
            project_runtime_abi::PROJECT_RUNTIME_ENTRY_SYMBOL.as_ptr(),
        )
    };
    let Some(symbol) = symbol else {
        return Err(ProjectRuntimeError::new(
            "project_runtime.native_module_symbol_missing",
            "resolve_symbol",
            "Project module entry symbol is missing.",
            "Rebuild the project module with the v1 ABI entry.",
        ));
    };
    let entry: project_runtime_abi::ProjectRuntimeEntry = unsafe { std::mem::transmute(symbol) };
    let api = unsafe { entry() };
    if api.is_null() {
        return Err(ProjectRuntimeError::new(
            "project_runtime.native_module_entry_null",
            "validate_api",
            "Project module entry returned null.",
            "Repair the project module ABI facade.",
        ));
    }
    let api = unsafe { *api };
    LinkedProjectRuntimeSet::singleton(Arc::new(
        LoadedProjectRuntimeModuleAdapter::new_with_lifetime_guard(api, guard)?,
    ))
}

#[cfg(windows)]
struct NativeLibraryGuard(windows_sys::Win32::Foundation::HMODULE);

#[cfg(windows)]
unsafe impl Send for NativeLibraryGuard {}

#[cfg(windows)]
unsafe impl Sync for NativeLibraryGuard {}

#[cfg(windows)]
impl Drop for NativeLibraryGuard {
    fn drop(&mut self) {
        unsafe { windows_sys::Win32::Foundation::FreeLibrary(self.0) };
    }
}

impl ProjectRuntimeModule for LoadedProjectRuntimeModuleAdapter {
    fn descriptor(&self) -> &ProjectRuntimeModuleDescriptor {
        &self.descriptor
    }

    fn install(
        &self,
        registration: &mut ProjectRuntimeRegistration,
    ) -> Result<(), ProjectRuntimeError> {
        // One registration becomes one bound runtime; rules borrow its existing session lease.
        let rule_session = Arc::new(OnceLock::<Weak<NativeProjectRuntimeSessionLease>>::new());
        for rule in &self.native_descriptor.rules {
            let rule_session = Arc::clone(&rule_session);
            let rule_id = rule.rule_id.clone();
            let callback_rule_id = rule_id.clone();
            registration.register_rust_aot_rule(
                rule_id,
                rule.artifact_id.clone(),
                move |context| {
                    let Some(lease) = rule_session.get().and_then(Weak::upgrade) else {
                        return LogicResult::failed(
                            &callback_rule_id,
                            ExecutorKind::RustAot,
                            NATIVE_CALL_FAILED,
                            "native rule session is not alive",
                        );
                    };
                    invoke_rule(&lease.api, lease.handle, &callback_rule_id, context)
                },
            )?;
        }

        let session_api = Arc::clone(&self.api);
        let session_lifetime_guard = self._lifetime_guard.clone();
        let producer_id = self.producer_id.clone();
        registration.set_runtime_session_bundle_factory(move |context| {
            LoadedProjectRuntimeSession::create_bundle(
                Arc::clone(&session_api),
                session_lifetime_guard.clone(),
                producer_id.clone(),
                context.project_id,
                context.module_id,
                &rule_session,
            )
        })
    }
}

struct NativeProjectRuntimeSessionLease {
    api: Arc<ProjectRuntimeApi>,
    _lifetime_guard: Option<Arc<dyn Send + Sync>>,
    handle: ProjectRuntimeOpaqueHandle,
}

impl Drop for NativeProjectRuntimeSessionLease {
    fn drop(&mut self) {
        destroy_session(&self.api, self.handle);
    }
}

struct LoadedProjectRuntimeSession {
    lease: Arc<NativeProjectRuntimeSessionLease>,
    session_id: String,
    terminal_fault: bool,
    call_buffer: Vec<u8>,
}

impl LoadedProjectRuntimeSession {
    fn create_bundle(
        api: Arc<ProjectRuntimeApi>,
        lifetime_guard: Option<Arc<dyn Send + Sync>>,
        producer_id: String,
        project_id: &str,
        module_id: &str,
        rule_session: &OnceLock<Weak<NativeProjectRuntimeSessionLease>>,
    ) -> Result<ProjectRuntimeSessionBundle, ProjectRuntimeSessionFactoryError> {
        let call = required_call(api.create_session, "create_session")
            .map_err(|error| ProjectRuntimeSessionFactoryError::new(error.message))?;
        let response: ProjectRuntimeSessionCreateResponse = call_json(
            call,
            api.module_context,
            ProjectRuntimeOpaqueHandle::NULL,
            None,
            &ProjectRuntimeSessionCreateRequest {
                project_id: project_id.to_string(),
                module_id: module_id.to_string(),
            },
        )
        .map_err(|error| ProjectRuntimeSessionFactoryError::new(error.message))?;
        let handle = ProjectRuntimeOpaqueHandle::from(response);
        if handle.is_null() {
            return Err(ProjectRuntimeSessionFactoryError::new(
                "native module returned a null session handle",
            ));
        }
        let session_id_call = required_call(api.session_id, "session_id")
            .map_err(|error| ProjectRuntimeSessionFactoryError::new(error.message))?;
        let session_id: String =
            match call_json::<(), String>(session_id_call, api.module_context, handle, None, &()) {
                Ok(value) if !value.trim().is_empty() => value,
                Ok(_) => {
                    destroy_session(&api, handle);
                    return Err(ProjectRuntimeSessionFactoryError::new(
                        "native module returned an empty session id",
                    ));
                }
                Err(error) => {
                    destroy_session(&api, handle);
                    return Err(ProjectRuntimeSessionFactoryError::new(error.message));
                }
            };
        let lease = Arc::new(NativeProjectRuntimeSessionLease {
            api,
            _lifetime_guard: lifetime_guard,
            handle,
        });
        rule_session.set(Arc::downgrade(&lease)).map_err(|_| {
            ProjectRuntimeSessionFactoryError::new("native rule registration already has a session")
        })?;
        let session = Self {
            lease: Arc::clone(&lease),
            session_id,
            terminal_fault: false,
            call_buffer: vec![0; PROJECT_RUNTIME_DEFAULT_STATEFUL_OUTPUT_CAPACITY_BYTES],
        };
        let producer = LoadedProjectUiStateProducer {
            lease,
            producer_id,
            registered_binding_set: None,
        };
        Ok(ProjectRuntimeSessionBundle {
            project_runtime_session: Box::new(session),
            ui_state_producer: Box::new(producer),
        })
    }

    fn invoke_session(
        &mut self,
        call: Option<project_runtime_abi::ProjectRuntimeModuleCall>,
        context: ProjectRuntimeSessionContext<'_>,
        request: &impl serde::Serialize,
    ) -> ProjectRuntimeSessionOutput {
        if self.terminal_fault {
            return terminal_output(NATIVE_TERMINAL_FAULT);
        }
        let Ok(call) = required_call(call, "session_callback") else {
            self.terminal_fault = true;
            return terminal_output(NATIVE_CALL_FAILED);
        };
        let time = context.time;
        let frame_index = context.frame_index;
        let mut host = ReadOnlyWorldHost {
            world: context.world,
        };
        let result: Result<SdkSessionOutput, _> = with_host_context(&mut host, |host_context| {
            let call_context = abi_call_context(host_context, frame_index, time);
            call_json_once_with_buffer(
                call,
                self.lease.api.module_context,
                self.lease.handle,
                Some(&call_context),
                request,
                &mut self.call_buffer,
            )
        });
        match result.and_then(convert_session_output) {
            Ok(output) => {
                if output.status == ProjectRuntimeSessionStatus::Faulted {
                    self.terminal_fault = true;
                }
                output
            }
            Err(_) => {
                self.terminal_fault = true;
                terminal_output(NATIVE_CALL_FAILED)
            }
        }
    }
}

impl ProjectRuntimeSession for LoadedProjectRuntimeSession {
    fn session_id(&self) -> &str {
        &self.session_id
    }

    fn handle_aui_actions(
        &mut self,
        context: ProjectRuntimeSessionContext<'_>,
        batch: ProjectAuiActionBatch<'_>,
    ) -> ProjectRuntimeSessionOutput {
        let request = ProjectRuntimeAuiActionRequest {
            frame: frame_request(context.frame_index, context.time),
            actions: batch
                .actions()
                .iter()
                .map(|action| ProjectRuntimeAuiAction {
                    action_id: action.action_id.clone(),
                    node_id: action.node_id.clone(),
                    event: format!("{:?}", action.event),
                    payload: action.payload.clone(),
                })
                .collect(),
        };
        self.invoke_session(self.lease.api.handle_aui_actions, context, &request)
    }

    fn fixed_update(
        &mut self,
        context: ProjectRuntimeSessionContext<'_>,
    ) -> ProjectRuntimeSessionOutput {
        let request = frame_request(context.frame_index, context.time);
        self.invoke_session(self.lease.api.fixed_update, context, &request)
    }

    fn observe(
        &self,
        context: ProjectRuntimeObservationContext<'_>,
    ) -> ProjectRuntimeObservationOutput {
        if self.terminal_fault {
            return ProjectRuntimeObservationOutput::empty();
        }
        let Ok(call) = required_call(self.lease.api.observe, "observe") else {
            return ProjectRuntimeObservationOutput::empty();
        };
        let mut host = ReadOnlyWorldHost {
            world: context.world,
        };
        let result: Result<SdkObservationOutput, _> =
            with_host_context(&mut host, |host_context| {
                let call_context =
                    abi_call_context(host_context, context.frame_index, context.time);
                call_json(
                    call,
                    self.lease.api.module_context,
                    self.lease.handle,
                    Some(&call_context),
                    &frame_request(context.frame_index, context.time),
                )
            });
        let mut output = ProjectRuntimeObservationOutput::empty();
        if let Ok(result) = result {
            for (path, value) in result.values {
                if let Some(value) = observation_value(value) {
                    output.insert(path, value);
                }
            }
        }
        output
    }
}

struct LoadedProjectUiStateProducer {
    lease: Arc<NativeProjectRuntimeSessionLease>,
    producer_id: String,
    registered_binding_set: Option<ProjectUiBindingSetIdentity>,
}

impl ProjectUiStateSnapshotProducer for LoadedProjectUiStateProducer {
    fn producer_id(&self) -> &str {
        &self.producer_id
    }

    fn produce(
        &mut self,
        context: ProjectUiStateProducerContext<'_>,
    ) -> ProjectUiStateSnapshotOutput {
        match self.resolve(context) {
            Ok(
                ProjectUiStateResolve::Replace { output, .. }
                | ProjectUiStateResolve::Uncacheable { output },
            ) => output,
            Ok(ProjectUiStateResolve::Reuse { .. }) | Err(_) => ProjectUiStateSnapshotOutput::new(
                self.producer_id(),
                AuiSnapshotSource::ProjectProducer,
                ProjectUiStateSnapshot::new(0),
            ),
        }
    }

    fn resolve(
        &mut self,
        context: ProjectUiStateProducerContext<'_>,
    ) -> Result<ProjectUiStateResolve, ProjectUiStateResolveError> {
        let call = required_call(self.lease.api.resolve_ui_state, "resolve_ui_state")
            .map_err(|error| ProjectUiStateResolveError::new(NATIVE_CALL_FAILED, error.message))?;
        let mut replacement_identity = None;
        let binding_set = match context.binding_set {
            ProjectUiBindingSet::Known(identity) => {
                if self.registered_binding_set.as_ref() != Some(&identity) {
                    return Err(ProjectUiStateResolveError::new(
                        "project_ui_state.binding_set_unknown",
                        "native producer has not registered this binding set",
                    ));
                }
                ProjectRuntimeUiBindingSet::Known {
                    digest: identity.digest,
                }
            }
            ProjectUiBindingSet::Replace {
                identity,
                active_binding_paths,
            } => {
                replacement_identity = Some(identity.clone());
                ProjectRuntimeUiBindingSet::Replace {
                    digest: identity.digest,
                    active_binding_paths,
                }
            }
        };
        let mut host = ReadOnlyWorldHost {
            world: WorldReadApi::new(context.world),
        };
        let time = TimeContext::from_delta(
            context.frame_index,
            crate::runtime_time::DEFAULT_FIXED_DELTA_TIME,
            false,
        );
        let result: Result<ProjectRuntimeUiStateResolveOutput, _> =
            with_host_context(&mut host, |host_context| {
                let call_context = abi_call_context(host_context, context.frame_index, time);
                call_json(
                    call,
                    self.lease.api.module_context,
                    self.lease.handle,
                    Some(&call_context),
                    &ProjectRuntimeUiStateResolveRequest {
                        frame: frame_request(context.frame_index, time),
                        previous_identity: context.previous_identity.map(|identity| {
                            ProjectRuntimeUiStateIdentity {
                                producer_epoch: identity.producer_epoch,
                                visible_revision: identity.visible_revision,
                                binding_set_digest: identity.binding_set.digest,
                            }
                        }),
                        binding_set,
                    },
                )
            });
        let result = result
            .map_err(|error| ProjectUiStateResolveError::new(NATIVE_CALL_FAILED, error.message))?;
        if let Some(identity) = replacement_identity {
            self.registered_binding_set = Some(identity);
        }
        let convert_identity = |identity: ProjectRuntimeUiStateIdentity| ProjectUiStateIdentity {
            producer_epoch: identity.producer_epoch,
            visible_revision: identity.visible_revision,
            binding_set: ProjectUiBindingSetIdentity {
                digest: identity.binding_set_digest,
            },
        };
        let output = |producer_id: String,
                      values: BTreeMap<String, ProjectRuntimeValue>,
                      frame_index: u64| {
            let mut snapshot = ProjectUiStateSnapshot::new(frame_index);
            for (path, value) in values {
                if let Some(value) = aui_value(value) {
                    snapshot.values.insert(path, value);
                }
            }
            ProjectUiStateSnapshotOutput::new(
                producer_id,
                AuiSnapshotSource::ProjectProducer,
                snapshot,
            )
        };
        Ok(match result {
            ProjectRuntimeUiStateResolveOutput::Reuse { identity } => {
                ProjectUiStateResolve::Reuse {
                    identity: convert_identity(identity),
                }
            }
            ProjectRuntimeUiStateResolveOutput::Replace {
                identity,
                producer_id,
                values,
            } => ProjectUiStateResolve::Replace {
                identity: convert_identity(identity),
                output: output(producer_id, values, context.frame_index),
            },
            ProjectRuntimeUiStateResolveOutput::Uncacheable {
                producer_id,
                values,
            } => ProjectUiStateResolve::Uncacheable {
                output: output(producer_id, values, context.frame_index),
            },
        })
    }
}

fn invoke_rule(
    api: &ProjectRuntimeApi,
    session: ProjectRuntimeOpaqueHandle,
    rule_id: &str,
    context: &mut LogicContext<'_>,
) -> LogicResult {
    let Ok(call) = required_call(api.invoke_rule, "invoke_rule") else {
        return LogicResult::failed(
            rule_id,
            ExecutorKind::RustAot,
            NATIVE_CALL_FAILED,
            "native rule callback is missing",
        );
    };
    let frame_index = context.frame_index;
    let time = *context.time();
    let input_actions = context
        .action_snapshot()
        .map(|snapshot| {
            snapshot
                .actions
                .iter()
                .map(|action| {
                    let (phase, axis1, axis2) = match action.value {
                        engine_input::ActionValue::Button { phase } => {
                            (Some(phase.as_str().to_string()), None, None)
                        }
                        engine_input::ActionValue::Axis1 { value } => {
                            (None, Some(value.value), None)
                        }
                        engine_input::ActionValue::Axis2 { value } => {
                            (None, None, Some([value.x, value.y]))
                        }
                        engine_input::ActionValue::Pointer { position } => (
                            Some("pointer".to_string()),
                            None,
                            Some([position.x, position.y]),
                        ),
                    };
                    ProjectRuntimeInputAction {
                        action_id: action.action_id.clone(),
                        phase,
                        axis1,
                        axis2,
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    let collision_pairs = context
        .collision_pairs()
        .iter()
        .map(|pair| ProjectRuntimeCollisionPair {
            entity_a: pair.entity_a.to_string(),
            entity_b: pair.entity_b.to_string(),
            is_sensor_pair: pair.is_sensor_pair,
        })
        .collect();
    let mut host = LogicWorldHost { context };
    let response: Result<ProjectRuntimeRuleOutput, _> =
        with_host_context(&mut host, |host_context| {
            let call_context = abi_call_context(host_context, frame_index, time);
            call_json(
                call,
                api.module_context,
                session,
                Some(&call_context),
                &ProjectRuntimeRuleRequest {
                    rule_id: rule_id.to_string(),
                    frame: frame_request(frame_index, time),
                    input_actions,
                    collision_pairs,
                },
            )
        });
    let response = match response {
        Ok(response) => response,
        Err(error) => {
            return LogicResult::failed(
                rule_id,
                ExecutorKind::RustAot,
                NATIVE_CALL_FAILED,
                error.message,
            )
        }
    };
    let status = NativeCallStatus::from(response.status);
    if status == NativeCallStatus::Faulted {
        return LogicResult::failed(
            rule_id,
            ExecutorKind::RustAot,
            NATIVE_TERMINAL_FAULT,
            response.diagnostics.join("; "),
        );
    }
    if status == NativeCallStatus::Rejected {
        return LogicResult::failed(
            rule_id,
            ExecutorKind::RustAot,
            NATIVE_CALL_FAILED,
            response.diagnostics.join("; "),
        );
    }
    if matches!(status, NativeCallStatus::NoOp | NativeCallStatus::Unhandled) {
        return LogicResult::skipped(rule_id, ExecutorKind::RustAot);
    }
    let mut result = LogicResult::applied(rule_id, ExecutorKind::RustAot);
    for mutation in response.mutations {
        match apply_rule_mutation(host.context, &mutation) {
            Ok(Some(write)) => result.writes.push(write),
            Ok(None) => {}
            Err(error) => {
                result.status = LogicStatus::Failed;
                result.errors.push(error);
                result.failure_location = mutation_failure_location(&mutation);
                return result;
            }
        }
    }
    result
}

fn convert_session_output(
    output: SdkSessionOutput,
) -> Result<ProjectRuntimeSessionOutput, project_runtime_sdk::ProjectRuntimeSdkError> {
    let mut mutations = ProjectRuntimeMutationBuffer::new();
    for mutation in output.mutations {
        append_deferred_mutation(&mut mutations, mutation)
            .map_err(|message| sdk_error(NativeAdapterError::failed(message)))?;
    }
    Ok(ProjectRuntimeSessionOutput {
        status: session_status(output.status),
        handled_action_count: usize::try_from(output.handled_action_count).map_err(|_| {
            sdk_error(NativeAdapterError::failed(
                "handled action count exceeds host range",
            ))
        })?,
        unhandled_action_count: usize::try_from(output.unhandled_action_count).map_err(|_| {
            sdk_error(NativeAdapterError::failed(
                "unhandled action count exceeds host range",
            ))
        })?,
        rejected_action_count: usize::try_from(output.rejected_action_count).map_err(|_| {
            sdk_error(NativeAdapterError::failed(
                "rejected action count exceeds host range",
            ))
        })?,
        mutations,
        diagnostics: if output.diagnostics.is_empty() {
            Vec::new()
        } else {
            vec![NATIVE_CALL_FAILED]
        },
    })
}

#[cfg(test)]
mod particle_intent_tests {
    use super::*;
    #[test]
    fn particle_native_wire_controls_commit_once_with_captured_generation() {
        use crate::{
            archetype::ComponentValue,
            components::Hierarchy,
            runtime_particles::{ParticleAction, ParticleEffect},
        };
        let mut world = crate::world::World::new();
        world.spawn_entity(
            "effect".into(),
            "Effect",
            "particle",
            true,
            Hierarchy {
                parent_id: None,
                sibling_order: 0,
            },
        );
        world.insert_component_value(
            "effect".into(),
            ComponentValue::ParticleEffect(ParticleEffect {
                effect_ref: crate::runtime_package::RuntimeAssetRef {
                    id: "effect".into(),
                    asset_type: "particle-effect".into(),
                    guid: None,
                    sub_asset: None,
                },
                play_on_awake: false,
                paused: false,
                parameters: Default::default(),
            }),
        );
        let mut buffer = ProjectRuntimeMutationBuffer::new();
        for intent in [
            serde_json::json!({"operation":"play"}),
            serde_json::json!({"operation":"restart"}),
            serde_json::json!({"operation":"stop_emitting"}),
            serde_json::json!({"operation":"clear"}),
            serde_json::json!({"operation":"set_paused","paused":true}),
            serde_json::json!({"operation":"set_parameter","name":"speed","value":{"type":"float","value":2.0}}),
        ] {
            let wire = serde_json::json!({"kind":"particle_effect","entity_id":"effect","generation":0,"intent":intent});
            append_deferred_mutation(&mut buffer, serde_json::from_value(wire).unwrap()).unwrap();
        }
        let report = buffer.prepare(&world).unwrap().commit(&mut world).unwrap();
        assert_eq!(report.committed_count, 6);
        assert_eq!(report.particle_commands.len(), 6);
        assert_eq!(report.particle_commands[0].action, ParticleAction::Play);
        assert_eq!(report.particle_commands[3].action, ParticleAction::Clear);
        assert!(
            serde_json::from_value::<project_runtime_sdk::ProjectRuntimeParticleValue>(
                serde_json::json!({"type":"float","value":"wrong"})
            )
            .is_err()
        );
        assert!(
            serde_json::from_value::<project_runtime_sdk::ProjectRuntimeParticleIntent>(
                serde_json::json!({"operation":"play","unknown":true})
            )
            .is_err()
        );
        assert!(report
            .particle_commands
            .iter()
            .all(|c| Some(c.runtime_id) == world.runtime_id_for_source(&"effect".into())));
    }
}

fn append_deferred_mutation(
    output: &mut ProjectRuntimeMutationBuffer,
    mutation: ProjectRuntimeDeferredMutation,
) -> Result<(), String> {
    match mutation {
        ProjectRuntimeDeferredMutation::ParticleEffect {
            entity_id,
            generation,
            intent,
        } => {
            use crate::runtime_particles::ParticleAction as A;
            use project_runtime_sdk::ProjectRuntimeParticleIntent as I;
            let action = match intent {
                I::Play {} => A::Play,
                I::Restart {} => A::Restart,
                I::StopEmitting {} => A::StopEmitting,
                I::Clear {} => A::Clear,
                I::SetPaused { paused } => A::SetPaused(paused),
                I::SetParameter { name, value } => A::SetParameter {
                    name,
                    value: serde_json::to_string(&value).map_err(|e| e.to_string())?,
                },
            };
            output.particle_command(EntityId::from(entity_id), generation, action);
        }
        ProjectRuntimeDeferredMutation::AudioSource { entity_id, intent } => {
            use crate::runtime_audio::AudioSourceAction as Action;
            use project_runtime_sdk::ProjectRuntimeAudioSourceIntent as Intent;
            let action = match intent {
                Intent::Play {} => Action::Play,
                Intent::Stop {} => Action::Stop,
                Intent::SetPaused { paused } => Action::SetPaused(paused),
            };
            output.audio_source_command(EntityId::from(entity_id), action);
        }
        ProjectRuntimeDeferredMutation::Animator2D { entity_id, intent } => {
            use crate::animator2d::Animator2DCommand as Command;
            use project_runtime_sdk::ProjectRuntimeAnimator2DIntent as Intent;
            let entity_id = EntityId::from(entity_id);
            output.animator2d_command(match intent {
                Intent::SetBool { name, value } => Command::SetBool {
                    entity_id,
                    parameter_id: name,
                    value,
                },
                Intent::Play { name } => Command::Play {
                    entity_id,
                    state_id: name,
                },
                Intent::Resume => Command::Resume { entity_id },
                Intent::SetPaused { paused } => Command::SetPaused { entity_id, paused },
            });
        }
        ProjectRuntimeDeferredMutation::WriteTransform {
            entity_id,
            transform,
        } => {
            output.write_transform(EntityId::from(entity_id), engine_transform(transform));
        }
        ProjectRuntimeDeferredMutation::WriteComponentField {
            entity_id,
            component_type,
            field_path,
            value,
        } => output.write_component_field(
            EntityId::from(entity_id),
            ComponentTypeId::from(component_type),
            FieldPath::parse(field_path).map_err(|error| error.code.to_string())?,
            runtime_value(value)?,
        ),
        ProjectRuntimeDeferredMutation::ReplaceDynamicComponent {
            entity_id,
            component_type,
            fields,
        } => {
            let component_type = ComponentTypeId::from(component_type);
            output.replace_component(
                EntityId::from(entity_id),
                component_type.clone(),
                ComponentValue::Dynamic {
                    component_type,
                    value: RuntimeValue::Object(
                        fields
                            .into_iter()
                            .map(|(key, value)| runtime_value(value).map(|value| (key, value)))
                            .collect::<Result<_, _>>()?,
                    ),
                },
            );
        }
        ProjectRuntimeDeferredMutation::InstantiatePrefab {
            prefab_id,
            position,
        } => {
            output.push_gameplay_command(
                crate::gameplay_command::GameplayCommand::InstantiatePrefab {
                    prefab_ref: crate::runtime_package::RuntimeAssetRef {
                        id: prefab_id,
                        asset_type: "prefab".to_string(),
                        guid: None,
                        sub_asset: None,
                    },
                    parent_entity: None,
                    target_scene_instance: None,
                    position: position.map(|[x, y, z]| crate::math::Vec3 { x, y, z }),
                },
            );
        }
        ProjectRuntimeDeferredMutation::DespawnEntity { entity_id } => {
            output.push_gameplay_command(crate::gameplay_command::GameplayCommand::DespawnEntity {
                entity_id: EntityId::from(entity_id),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod animator_intent_tests {
    use super::*;

    #[test]
    fn audio_source_sdk_json_intents_capture_identity_and_commit_in_order() {
        use crate::audio::AudioSource;
        use crate::components::Hierarchy;
        use crate::runtime_audio::AudioSourceAction as Action;
        use crate::world::World;
        let mut world = World::new();
        let entity_id = EntityId::from("speaker");
        let runtime_id = world.spawn_entity(
            entity_id.clone(),
            "Speaker",
            "audio",
            true,
            Hierarchy {
                parent_id: None,
                sibling_order: 0,
            },
        );
        world.insert_component_value(
            entity_id.clone(),
            ComponentValue::AudioSource(AudioSource {
                clip_ref: crate::runtime_package::RuntimeAssetRef {
                    id: "audio-test".into(),
                    asset_type: "audio".into(),
                    guid: None,
                    sub_asset: None,
                },
                volume: 0.5,
            }),
        );
        let dirty_before = world.dirty_records().len();
        let mut output = ProjectRuntimeMutationBuffer::new();
        for intent in [
            serde_json::json!({"operation":"play"}),
            serde_json::json!({"operation":"set_paused","paused":true}),
            serde_json::json!({"operation":"set_paused","paused":false}),
            serde_json::json!({"operation":"stop"}),
        ] {
            let wire =
                serde_json::json!({"kind":"audio_source","entity_id":"speaker","intent":intent});
            append_deferred_mutation(&mut output, serde_json::from_value(wire).unwrap()).unwrap();
        }
        let prepared = output.prepare(&world).unwrap();
        assert_eq!(world.dirty_records().len(), dirty_before);
        let report = prepared.commit(&mut world).unwrap();
        assert_eq!(report.committed_count, 4);
        assert_eq!(
            report
                .audio_source_commands
                .iter()
                .map(|command| command.action.clone())
                .collect::<Vec<_>>(),
            vec![
                Action::Play,
                Action::SetPaused(true),
                Action::SetPaused(false),
                Action::Stop
            ]
        );
        assert!(report
            .audio_source_commands
            .iter()
            .all(|command| command.entity_id == entity_id && command.runtime_id == runtime_id));
        assert_eq!(world.dirty_records().len(), dirty_before);
    }

    #[test]
    fn animator2d_sdk_json_intents_reach_existing_session_commit_in_order() {
        let mut output = ProjectRuntimeMutationBuffer::new();
        for intent in [
            serde_json::json!({"operation":"set_bool","name":"moving","value":true}),
            serde_json::json!({"operation":"play","name":"attack"}),
            serde_json::json!({"operation":"set_paused","paused":true}),
            serde_json::json!({"operation":"resume"}),
        ] {
            let wire = serde_json::json!({"kind":"animator2d","entity_id":"robot","intent":intent});
            append_deferred_mutation(&mut output, serde_json::from_value(wire).unwrap()).unwrap();
        }
        let mut world = crate::world::World::new();
        let report = output.prepare(&world).unwrap().commit(&mut world).unwrap();
        use crate::animator2d::Animator2DCommand as C;
        assert!(
            matches!(&report.animator2d_commands[..],[C::SetBool {value:true,..},C::Play {state_id,..},C::SetPaused {paused:true,..},C::Resume {..}] if state_id == "attack")
        );
    }
}

fn apply_rule_mutation(
    context: &mut LogicContext<'_>,
    mutation: &ProjectRuntimeDeferredMutation,
) -> Result<Option<crate::logic_executor::LogicWrite>, LogicError> {
    match mutation {
        ProjectRuntimeDeferredMutation::ParticleEffect { .. } => Err(LogicError {
            code: "particle_effect.session_callback_required",
            message: "Submit ParticleEffect intents from a FixedUpdate or AUI session callback."
                .into(),
        }),
        ProjectRuntimeDeferredMutation::AudioSource { .. } => Err(LogicError {
            code: "audio_source.session_callback_required",
            message: "Submit AudioSource intents from a FixedUpdate or AUI session callback."
                .into(),
        }),
        ProjectRuntimeDeferredMutation::Animator2D { .. } => Err(LogicError {
            code: "animator2d.session_callback_required",
            message: "Submit Animator2D intents from a FixedUpdate or AUI session callback.".into(),
        }),
        ProjectRuntimeDeferredMutation::WriteTransform {
            entity_id,
            transform,
        } => context
            .write_component(
                EntityId::from(entity_id.as_str()),
                ComponentTypeId::transform(),
                ComponentValue::Transform(engine_transform(transform.clone())),
            )
            .map(Some)
            .map_err(LogicError::from),
        ProjectRuntimeDeferredMutation::WriteComponentField {
            entity_id,
            component_type,
            field_path,
            value,
        } => context
            .write_component_field(
                EntityId::from(entity_id.as_str()),
                ComponentTypeId::from(component_type.as_str()),
                &FieldPath::parse(field_path.clone()).map_err(|error| LogicError {
                    code: error.code,
                    message: error.code.into(),
                })?,
                runtime_value(value.clone()).map_err(|message| LogicError {
                    code: NATIVE_CALL_FAILED,
                    message,
                })?,
            )
            .map(Some)
            .map_err(LogicError::from),
        ProjectRuntimeDeferredMutation::ReplaceDynamicComponent { .. } => Err(LogicError {
            code: NATIVE_CALL_FAILED,
            message: "replace_dynamic_component is session-deferred only".into(),
        }),
        ProjectRuntimeDeferredMutation::InstantiatePrefab {
            prefab_id,
            position,
        } => {
            context.enqueue_command(
                crate::gameplay_command::GameplayCommand::InstantiatePrefab {
                    prefab_ref: crate::runtime_package::RuntimeAssetRef {
                        id: prefab_id.clone(),
                        asset_type: "prefab".to_string(),
                        guid: None,
                        sub_asset: None,
                    },
                    parent_entity: None,
                    target_scene_instance: None,
                    position: position.map(|[x, y, z]| crate::math::Vec3 { x, y, z }),
                },
            );
            Ok(None)
        }
        ProjectRuntimeDeferredMutation::DespawnEntity { entity_id } => {
            context.request_despawn_entity(EntityId::from(entity_id.as_str()));
            Ok(None)
        }
    }
}

fn mutation_failure_location(
    mutation: &ProjectRuntimeDeferredMutation,
) -> Option<LogicFailureLocation> {
    match mutation {
        ProjectRuntimeDeferredMutation::WriteComponentField {
            entity_id,
            component_type,
            field_path,
            ..
        } => Some(LogicFailureLocation {
            entity_id: entity_id.as_str().into(),
            component_type: component_type.as_str().into(),
            field_path: Some(field_path.clone()),
        }),
        ProjectRuntimeDeferredMutation::WriteTransform { entity_id, .. } => {
            Some(LogicFailureLocation {
                entity_id: entity_id.as_str().into(),
                component_type: ComponentTypeId::transform(),
                field_path: None,
            })
        }
        _ => None,
    }
}

fn validate_api(api: &ProjectRuntimeApi) -> Result<(), ProjectRuntimeError> {
    if api.struct_size < PROJECT_RUNTIME_API_STRUCT_SIZE
        || api.abi_major != PROJECT_RUNTIME_ABI_MAJOR
    {
        return Err(abi_error(
            "validate_api",
            "native module ABI version or struct size mismatch",
        ));
    }
    if api.contract_digest != project_runtime_contract_digest() {
        return Err(abi_error(
            "validate_api",
            "native module ABI/SDK digest mismatch",
        ));
    }
    let required_capabilities = project_runtime_abi::PROJECT_RUNTIME_CAP_SESSIONS;
    if api.capabilities & required_capabilities != required_capabilities {
        return Err(abi_error(
            "validate_api",
            "native module does not provide the required v1 capabilities",
        ));
    }
    for (name, call) in [
        ("descriptor", api.descriptor),
        ("create_session", api.create_session),
        ("destroy_session", api.destroy_session),
        ("session_id", api.session_id),
    ] {
        required_call(call, name)?;
    }
    for (name, capability, call) in [
        (
            "invoke_rule",
            project_runtime_abi::PROJECT_RUNTIME_CAP_RULES,
            api.invoke_rule,
        ),
        (
            "handle_aui_actions",
            project_runtime_abi::PROJECT_RUNTIME_CAP_AUI_ACTIONS,
            api.handle_aui_actions,
        ),
        (
            "fixed_update",
            project_runtime_abi::PROJECT_RUNTIME_CAP_FIXED_UPDATE,
            api.fixed_update,
        ),
        (
            "resolve_ui_state",
            project_runtime_abi::PROJECT_RUNTIME_CAP_UI_STATE,
            api.resolve_ui_state,
        ),
        (
            "observe",
            project_runtime_abi::PROJECT_RUNTIME_CAP_OBSERVATIONS,
            api.observe,
        ),
    ] {
        let declared = api.capabilities & capability != 0;
        if declared != call.is_some() {
            return Err(abi_error(
                "validate_api",
                format!("native module capability and callback '{name}' do not match"),
            ));
        }
    }
    Ok(())
}

fn validate_descriptor(descriptor: &SdkModuleDescriptor) -> Result<(), ProjectRuntimeError> {
    if descriptor.module_id.trim().is_empty()
        || descriptor.interface_version.trim().is_empty()
        || descriptor.aot_content_digest.trim().is_empty()
        || descriptor.ui_state_producer_id.trim().is_empty()
    {
        return Err(abi_error(
            "descriptor",
            "native module descriptor contains an empty required field",
        ));
    }
    if descriptor
        .rules
        .iter()
        .any(|rule| rule.rule_id.trim().is_empty() || rule.artifact_id.trim().is_empty())
    {
        return Err(abi_error(
            "descriptor",
            "native module rule descriptor contains an empty field",
        ));
    }
    Ok(())
}

fn required_call(
    call: Option<project_runtime_abi::ProjectRuntimeModuleCall>,
    stage: &'static str,
) -> Result<project_runtime_abi::ProjectRuntimeModuleCall, ProjectRuntimeError> {
    call.ok_or_else(|| {
        abi_error(
            stage,
            format!("native module is missing required callback '{stage}'"),
        )
    })
}

fn abi_error(stage: &'static str, message: impl Into<String>) -> ProjectRuntimeError {
    ProjectRuntimeError::new(
        "project_runtime.native_module_invalid",
        stage,
        message,
        "Rebuild the project native module against the current ProjectRuntimeAbi/SDK.",
    )
}

fn destroy_session(api: &ProjectRuntimeApi, handle: ProjectRuntimeOpaqueHandle) {
    if let Some(call) = api.destroy_session {
        let _result: Result<serde_json::Value, _> =
            call_json(call, api.module_context, handle, None, &());
    }
}

fn terminal_output(diagnostic: &'static str) -> ProjectRuntimeSessionOutput {
    let mut output = ProjectRuntimeSessionOutput::no_op();
    output.status = ProjectRuntimeSessionStatus::Faulted;
    output.diagnostics.push(diagnostic);
    output
}

fn session_status(status: ProjectRuntimeStatus) -> ProjectRuntimeSessionStatus {
    match status {
        ProjectRuntimeStatus::Applied => ProjectRuntimeSessionStatus::Applied,
        ProjectRuntimeStatus::NoOp => ProjectRuntimeSessionStatus::NoOp,
        ProjectRuntimeStatus::Unhandled => ProjectRuntimeSessionStatus::Unhandled,
        ProjectRuntimeStatus::Rejected => ProjectRuntimeSessionStatus::Rejected,
        ProjectRuntimeStatus::Faulted => ProjectRuntimeSessionStatus::Faulted,
    }
}

fn frame_request(frame_index: u64, time: TimeContext) -> ProjectRuntimeFrameRequest {
    ProjectRuntimeFrameRequest {
        frame_index,
        time: ProjectRuntimeTime {
            time: time.time,
            delta_time: time.delta_time,
            unscaled_time: time.unscaled_time,
            unscaled_delta_time: time.unscaled_delta_time,
            fixed_time: time.fixed_time,
            fixed_delta_time: time.fixed_delta_time,
            frame_count: time.frame_count,
            fixed_frame_count: time.fixed_frame_count,
            time_scale: time.time_scale,
            in_fixed_step: time.in_fixed_step,
        },
    }
}

fn abi_call_context(
    host_context: ProjectRuntimeOpaqueHandle,
    frame_index: u64,
    time: TimeContext,
) -> ProjectRuntimeCallContext {
    ProjectRuntimeCallContext {
        struct_size: PROJECT_RUNTIME_CALL_CONTEXT_STRUCT_SIZE,
        reserved: 0,
        host_context,
        host_api: std::ptr::from_ref(&HOST_API),
        frame_index,
        time: ProjectRuntimeTimeContext {
            time: time.time,
            delta_time: time.delta_time,
            unscaled_time: time.unscaled_time,
            unscaled_delta_time: time.unscaled_delta_time,
            fixed_time: time.fixed_time,
            fixed_delta_time: time.fixed_delta_time,
            frame_count: time.frame_count,
            fixed_frame_count: time.fixed_frame_count,
            time_scale: time.time_scale,
            in_fixed_step: u32::from(time.in_fixed_step),
        },
    }
}

fn engine_transform(value: ProjectRuntimeTransform) -> Transform {
    Transform {
        local_position: Vec3 {
            x: value.position[0],
            y: value.position[1],
            z: value.position[2],
        },
        local_rotation: Vec3 {
            x: value.rotation[0],
            y: value.rotation[1],
            z: value.rotation[2],
        },
        local_scale: Vec3 {
            x: value.scale[0],
            y: value.scale[1],
            z: value.scale[2],
        },
    }
}

fn runtime_value(value: ProjectRuntimeValue) -> Result<RuntimeValue, String> {
    Ok(match value {
        ProjectRuntimeValue::Null => RuntimeValue::Null,
        ProjectRuntimeValue::Bool(value) => RuntimeValue::Bool(value),
        ProjectRuntimeValue::Integer(value) => RuntimeValue::I64(value),
        ProjectRuntimeValue::Number(value) => RuntimeValue::F64(value),
        ProjectRuntimeValue::String(value) => RuntimeValue::String(value),
        ProjectRuntimeValue::Vec2(value) => RuntimeValue::Vec2 {
            x: value[0],
            y: value[1],
        },
        ProjectRuntimeValue::Vec3(value) => RuntimeValue::Vec3(Vec3 {
            x: value[0],
            y: value[1],
            z: value[2],
        }),
        ProjectRuntimeValue::Color(value) => RuntimeValue::Color {
            r: value[0],
            g: value[1],
            b: value[2],
            a: value[3],
        },
        ProjectRuntimeValue::EntityRef(value) => RuntimeValue::EntityRef(EntityId::from(value)),
        ProjectRuntimeValue::AssetRef(value) => RuntimeValue::AssetRef(value),
        ProjectRuntimeValue::Object(values) => RuntimeValue::Object(
            values
                .into_iter()
                .map(|(key, value)| runtime_value(value).map(|value| (key, value)))
                .collect::<Result<_, _>>()?,
        ),
        ProjectRuntimeValue::Array(values) => RuntimeValue::Array(
            values
                .into_iter()
                .map(runtime_value)
                .collect::<Result<_, _>>()?,
        ),
    })
}

fn project_value(value: RuntimeValue) -> ProjectRuntimeValue {
    match value {
        RuntimeValue::Null => ProjectRuntimeValue::Null,
        RuntimeValue::Bool(value) => ProjectRuntimeValue::Bool(value),
        RuntimeValue::I64(value) => ProjectRuntimeValue::Integer(value),
        RuntimeValue::F64(value) => ProjectRuntimeValue::Number(value),
        RuntimeValue::String(value) => ProjectRuntimeValue::String(value),
        RuntimeValue::Vec2 { x, y } => ProjectRuntimeValue::Vec2([x, y]),
        RuntimeValue::Vec3(value) => ProjectRuntimeValue::Vec3([value.x, value.y, value.z]),
        RuntimeValue::Color { r, g, b, a } => ProjectRuntimeValue::Color([r, g, b, a]),
        RuntimeValue::EntityRef(value) => ProjectRuntimeValue::EntityRef(value.to_string()),
        RuntimeValue::AssetRef(value) => ProjectRuntimeValue::AssetRef(value),
        RuntimeValue::Object(values) => ProjectRuntimeValue::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, project_value(value)))
                .collect(),
        ),
        RuntimeValue::Array(values) => {
            ProjectRuntimeValue::Array(values.into_iter().map(project_value).collect())
        }
    }
}

fn component_value(value: ComponentValue) -> Option<ProjectRuntimeValue> {
    match value {
        ComponentValue::Transform(value) => Some(ProjectRuntimeValue::Object(BTreeMap::from([
            (
                "position".to_string(),
                ProjectRuntimeValue::Vec3([
                    value.local_position.x,
                    value.local_position.y,
                    value.local_position.z,
                ]),
            ),
            (
                "rotation".to_string(),
                ProjectRuntimeValue::Vec3([
                    value.local_rotation.x,
                    value.local_rotation.y,
                    value.local_rotation.z,
                ]),
            ),
            (
                "scale".to_string(),
                ProjectRuntimeValue::Vec3([
                    value.local_scale.x,
                    value.local_scale.y,
                    value.local_scale.z,
                ]),
            ),
        ]))),
        ComponentValue::Dynamic { value, .. } => Some(project_value(value)),
        ComponentValue::SpriteRenderer2D(value) => {
            Some(ProjectRuntimeValue::Object(BTreeMap::from([
                (
                    "spriteRef".to_string(),
                    value
                        .sprite_ref
                        .map_or(ProjectRuntimeValue::Null, ProjectRuntimeValue::AssetRef),
                ),
                (
                    "materialRef".to_string(),
                    value
                        .material_ref
                        .map_or(ProjectRuntimeValue::Null, ProjectRuntimeValue::AssetRef),
                ),
                ("color".to_string(), ProjectRuntimeValue::Color(value.color)),
                ("flipX".to_string(), ProjectRuntimeValue::Bool(value.flip_x)),
                ("flipY".to_string(), ProjectRuntimeValue::Bool(value.flip_y)),
                (
                    "sortingLayer".to_string(),
                    ProjectRuntimeValue::Integer(i64::from(value.sorting_layer)),
                ),
                (
                    "orderInLayer".to_string(),
                    ProjectRuntimeValue::Integer(i64::from(value.order_in_layer)),
                ),
                (
                    "sortZ".to_string(),
                    ProjectRuntimeValue::Number(f64::from(value.sort_z)),
                ),
                (
                    "visible".to_string(),
                    ProjectRuntimeValue::Bool(value.visible),
                ),
            ])))
        }
        _ => None,
    }
}

fn aui_value(value: ProjectRuntimeValue) -> Option<AuiBindingValue> {
    match value {
        ProjectRuntimeValue::Bool(value) => Some(AuiBindingValue::Bool(value)),
        ProjectRuntimeValue::Integer(value) => Some(AuiBindingValue::Number(value as f32)),
        ProjectRuntimeValue::Number(value) => Some(AuiBindingValue::Number(value as f32)),
        ProjectRuntimeValue::String(value) => Some(AuiBindingValue::String(value)),
        ProjectRuntimeValue::AssetRef(value) => {
            Some(AuiBindingValue::AssetRef(AuiAssetRef::new(value)))
        }
        ProjectRuntimeValue::Color(value) => Some(AuiBindingValue::Color(format!(
            "rgba({},{},{},{})",
            value[0], value[1], value[2], value[3]
        ))),
        _ => None,
    }
}

fn observation_value(value: ProjectRuntimeValue) -> Option<ProjectObservationValue> {
    match value {
        ProjectRuntimeValue::Bool(value) => Some(ProjectObservationValue::Bool(value)),
        ProjectRuntimeValue::Integer(value) => Some(ProjectObservationValue::Integer(value)),
        ProjectRuntimeValue::Number(value) => Some(ProjectObservationValue::Number(value)),
        ProjectRuntimeValue::String(value) => Some(ProjectObservationValue::String(value)),
        _ => None,
    }
}

trait HostWorldRead {
    fn query(
        &mut self,
        request: ProjectRuntimeWorldQueryRequest,
    ) -> ProjectRuntimeWorldQueryResponse;
    fn read(
        &mut self,
        request: ProjectRuntimeWorldReadRequest,
    ) -> Option<ProjectRuntimeWorldReadResponse>;
}

struct LogicWorldHost<'borrow, 'world> {
    context: &'borrow mut LogicContext<'world>,
}

impl HostWorldRead for LogicWorldHost<'_, '_> {
    fn query(
        &mut self,
        request: ProjectRuntimeWorldQueryRequest,
    ) -> ProjectRuntimeWorldQueryResponse {
        let spec = QuerySpec::all(request.all.into_iter().map(ComponentTypeId::from))
            .excluding(request.none.into_iter().map(ComponentTypeId::from));
        ProjectRuntimeWorldQueryResponse {
            entity_ids: self
                .context
                .query(spec)
                .into_iter()
                .map(|id| id.to_string())
                .collect(),
        }
    }

    fn read(
        &mut self,
        request: ProjectRuntimeWorldReadRequest,
    ) -> Option<ProjectRuntimeWorldReadResponse> {
        self.context
            .read_component(
                &EntityId::from(request.entity_id),
                &ComponentTypeId::from(request.component_type),
            )
            .ok()
            .and_then(component_value)
            .map(|value| ProjectRuntimeWorldReadResponse { value })
    }
}

struct ReadOnlyWorldHost<'world> {
    world: WorldReadApi<'world>,
}

impl HostWorldRead for ReadOnlyWorldHost<'_> {
    fn query(
        &mut self,
        request: ProjectRuntimeWorldQueryRequest,
    ) -> ProjectRuntimeWorldQueryResponse {
        let spec = QuerySpec::all(request.all.into_iter().map(ComponentTypeId::from))
            .excluding(request.none.into_iter().map(ComponentTypeId::from));
        ProjectRuntimeWorldQueryResponse {
            entity_ids: self
                .world
                .query(&spec)
                .into_iter()
                .map(|id| id.to_string())
                .collect(),
        }
    }

    fn read(
        &mut self,
        request: ProjectRuntimeWorldReadRequest,
    ) -> Option<ProjectRuntimeWorldReadResponse> {
        self.world
            .read_component(
                &EntityId::from(request.entity_id),
                &ComponentTypeId::from(request.component_type),
            )
            .ok()
            .and_then(component_value)
            .map(|value| ProjectRuntimeWorldReadResponse { value })
    }
}

struct ActiveHostContext {
    token: u64,
    context: *mut (dyn HostWorldRead + 'static),
}

thread_local! {
    static ACTIVE_HOST_CONTEXTS: RefCell<Vec<ActiveHostContext>> = const { RefCell::new(Vec::new()) };
}

static NEXT_HOST_CONTEXT: AtomicU64 = AtomicU64::new(1);

struct HostContextGuard {
    token: u64,
}

impl Drop for HostContextGuard {
    fn drop(&mut self) {
        ACTIVE_HOST_CONTEXTS.with(|contexts| {
            let popped = contexts.borrow_mut().pop();
            debug_assert_eq!(popped.map(|entry| entry.token), Some(self.token));
        });
    }
}

fn with_host_context<T>(
    context: &mut dyn HostWorldRead,
    callback: impl FnOnce(ProjectRuntimeOpaqueHandle) -> T,
) -> T {
    let token = NEXT_HOST_CONTEXT.fetch_add(1, Ordering::Relaxed);
    let pointer: *mut (dyn HostWorldRead + '_) = context;
    // SAFETY: the erased pointer is stored only until the guard drops before `context` can expire.
    let pointer: *mut (dyn HostWorldRead + 'static) = unsafe { std::mem::transmute(pointer) };
    ACTIVE_HOST_CONTEXTS.with(|contexts| {
        contexts.borrow_mut().push(ActiveHostContext {
            token,
            context: pointer,
        })
    });
    let _guard = HostContextGuard { token };
    callback(ProjectRuntimeOpaqueHandle {
        value: token,
        generation: 1,
    })
}

fn with_active_host<T>(
    handle: ProjectRuntimeOpaqueHandle,
    callback: impl FnOnce(&mut dyn HostWorldRead) -> T,
) -> Result<T, ProjectRuntimeAbiStatus> {
    if handle.generation != 1 {
        return Err(ProjectRuntimeAbiStatus::INVALID_HANDLE);
    }
    ACTIVE_HOST_CONTEXTS.with(|contexts| {
        let contexts = contexts.borrow();
        let entry = contexts
            .iter()
            .rev()
            .find(|entry| entry.token == handle.value)
            .ok_or(ProjectRuntimeAbiStatus::INVALID_HANDLE)?;
        // SAFETY: `with_host_context` keeps the referent alive and removes it after the module call.
        Ok(callback(unsafe { &mut *entry.context }))
    })
}

unsafe extern "C" fn host_world_query(
    host_context: ProjectRuntimeOpaqueHandle,
    request: ProjectRuntimeByteSlice,
    output: *mut ProjectRuntimeByteBuffer,
) -> ProjectRuntimeAbiStatus {
    ffi_boundary(output, || {
        // SAFETY: the module borrows request bytes for this callback only.
        let request: ProjectRuntimeWorldQueryRequest = unsafe { read_input(request) }?;
        with_active_host(host_context, |host| host.query(request))
    })
}

unsafe extern "C" fn host_world_read_component(
    host_context: ProjectRuntimeOpaqueHandle,
    request: ProjectRuntimeByteSlice,
    output: *mut ProjectRuntimeByteBuffer,
) -> ProjectRuntimeAbiStatus {
    ffi_boundary(output, || {
        // SAFETY: the module borrows request bytes for this callback only.
        let request: ProjectRuntimeWorldReadRequest = unsafe { read_input(request) }?;
        with_active_host(host_context, |host| host.read(request))?
            .ok_or(ProjectRuntimeAbiStatus::FAILED)
    })
}

static HOST_API: ProjectRuntimeHostApi = ProjectRuntimeHostApi {
    struct_size: PROJECT_RUNTIME_HOST_API_STRUCT_SIZE,
    reserved: 0,
    world_query: Some(host_world_query),
    world_read_component: Some(host_world_read_component),
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aui::{AuiAction, AuiActionEvent};
    use crate::components::Hierarchy;
    use crate::physics2d::{CollisionPair, Shape2D};
    use crate::project_observation::CookedProjectObservationContract;
    use crate::project_runtime_module::{
        LinkedProjectRuntimeSet, ProjectRuntimeBootstrap, PROJECT_RUNTIME_MODULE_INTERFACE_VERSION,
    };
    use crate::project_runtime_session::{
        ProjectAuiActionBatch, ProjectRuntimeObservationContext, ProjectRuntimeSessionReportLevel,
    };
    use crate::runtime_package::{
        load_runtime_package, RuntimeProjectInfo, RuntimeProjectModuleRef, RuntimeScene,
        RUNTIME_SCENE_SCHEMA_VERSION,
    };
    use crate::runtime_package_builder::{
        RuntimePackageBuildInput, RuntimePackageBuildRequest, RuntimePackageBuildStatus,
        RuntimePackageBuilder, RuntimePackageSourceJson,
    };
    use crate::world::World;
    use engine_input::{ActionPhase, ActionSnapshot, InputActionState, InputMappingAsset};
    use project_runtime_abi::{
        ProjectRuntimeModuleCall, PROJECT_RUNTIME_ABI_MINOR, PROJECT_RUNTIME_CAP_AUI_ACTIONS,
        PROJECT_RUNTIME_CAP_DEFERRED_MUTATIONS, PROJECT_RUNTIME_CAP_FIXED_UPDATE,
        PROJECT_RUNTIME_CAP_OBSERVATIONS, PROJECT_RUNTIME_CAP_RULES, PROJECT_RUNTIME_CAP_SESSIONS,
        PROJECT_RUNTIME_CAP_UI_STATE, PROJECT_RUNTIME_CAP_WORLD_READ,
    };
    use project_runtime_sdk::{call_host_json, ProjectRuntimeRuleDescriptor};
    use std::sync::atomic::{AtomicBool, AtomicU64};
    use std::time::{SystemTime, UNIX_EPOCH};

    const MODULE_ID: &str = "sample.test.runtime";

    #[test]
    fn native_call_status_maps_all_sdk_statuses() {
        let cases = [
            (ProjectRuntimeStatus::Applied, NativeCallStatus::Applied),
            (ProjectRuntimeStatus::NoOp, NativeCallStatus::NoOp),
            (ProjectRuntimeStatus::Unhandled, NativeCallStatus::Unhandled),
            (ProjectRuntimeStatus::Rejected, NativeCallStatus::Rejected),
            (ProjectRuntimeStatus::Faulted, NativeCallStatus::Faulted),
        ];
        for (sdk, native) in cases {
            assert_eq!(NativeCallStatus::from(sdk), native);
        }
    }

    unsafe extern "C" fn write_then_fail_rule(
        _module: ProjectRuntimeOpaqueHandle,
        _session: ProjectRuntimeOpaqueHandle,
        _context: *const ProjectRuntimeCallContext,
        _request: ProjectRuntimeByteSlice,
        output: *mut ProjectRuntimeByteBuffer,
    ) -> ProjectRuntimeAbiStatus {
        write(
            output,
            ProjectRuntimeRuleOutput {
                status: ProjectRuntimeStatus::Applied,
                mutations: vec![
                    ProjectRuntimeDeferredMutation::WriteTransform {
                        entity_id: "failure-fx".into(),
                        transform: ProjectRuntimeTransform {
                            position: [1.0, 0.0, 0.0],
                            rotation: [0.0; 3],
                            scale: [1.0; 3],
                        },
                    },
                    ProjectRuntimeDeferredMutation::WriteComponentField {
                        entity_id: "failure-fx".into(),
                        component_type: "engine.sprite_renderer2d".into(),
                        field_path: "unsupportedColor".into(),
                        value: ProjectRuntimeValue::Color([1.0; 4]),
                    },
                ],
                diagnostics: Vec::new(),
            },
        )
    }

    #[test]
    fn native_failed_field_keeps_original_code_location_and_prior_write() {
        let mut api = fake_api();
        api.invoke_rule = Some(write_then_fail_rule);
        let mut world = World::new();
        let id = EntityId::from("failure-fx");
        world.spawn_entity(
            id.clone(),
            "FX",
            "visual",
            true,
            Hierarchy {
                parent_id: None,
                sibling_order: 0,
            },
        );
        world.insert_transform(id.clone(), Transform::identity());
        world
            .try_insert_sprite_renderer2d(
                id.clone(),
                crate::components::SpriteRenderer2D::default(),
            )
            .unwrap();
        let mut context = LogicContext::new(
            7,
            1.0 / 60.0,
            crate::logic_executor::RulePhase::FrameUpdate,
            crate::world_api::WorldWriteApi::new(&mut world),
        );
        let result = invoke_rule(
            &api,
            ProjectRuntimeOpaqueHandle {
                value: 41,
                generation: 3,
            },
            "rule.fade",
            &mut context,
        );
        assert_eq!(result.status, LogicStatus::Failed);
        assert_eq!(result.errors[0].code, "world.component.unsupported_field");
        assert_eq!(
            result.writes.len(),
            1,
            "a failure does not undo or erase the preceding write"
        );
        let location = result.failure_location.as_ref().unwrap();
        assert_eq!(location.entity_id, id);
        assert_eq!(
            location.component_type,
            ComponentTypeId::sprite_renderer2d()
        );
        assert_eq!(location.field_path.as_deref(), Some("unsupportedColor"));
        let mut trace = crate::runtime_trace::RuntimeTrace::new();
        trace.record_logic_result(7, "Update", &result);
        let failure = trace
            .gameplay_records
            .iter()
            .find(|r| r.result == "failed")
            .unwrap();
        assert_eq!(failure.entity_id.as_ref(), Some(&id));
        assert_eq!(failure.field_path.as_deref(), Some("unsupportedColor"));
    }

    unsafe extern "C" fn panicked_rule(
        _module: ProjectRuntimeOpaqueHandle,
        _session: ProjectRuntimeOpaqueHandle,
        _context: *const ProjectRuntimeCallContext,
        _request: ProjectRuntimeByteSlice,
        _output: *mut ProjectRuntimeByteBuffer,
    ) -> ProjectRuntimeAbiStatus {
        ProjectRuntimeAbiStatus::PANICKED
    }

    #[test]
    fn native_callback_failure_keeps_abi_status_diagnostic() {
        let mut api = fake_api();
        api.invoke_rule = Some(panicked_rule);
        let mut world = World::new();
        let mut context = LogicContext::new(
            1,
            1.0 / 60.0,
            crate::logic_executor::RulePhase::FrameUpdate,
            crate::world_api::WorldWriteApi::new(&mut world),
        );
        let result = invoke_rule(
            &api,
            ProjectRuntimeOpaqueHandle {
                value: 41,
                generation: 3,
            },
            "rule.panics",
            &mut context,
        );
        assert_eq!(result.status, LogicStatus::Failed);
        assert_eq!(result.errors[0].code, NATIVE_CALL_FAILED);
        assert!(result.errors[0]
            .message
            .contains(&format!("status {}", ProjectRuntimeAbiStatus::PANICKED.0)));
    }
    const AOT_DIGEST: &str = "sha256:test-runtime-v1";
    const ENTITY_ID: &str = "entity-native-adapter";
    static DESTROY_COUNT: AtomicU64 = AtomicU64::new(0);
    static ACTION_COUNT: AtomicU64 = AtomicU64::new(0);
    static FIXED_COUNT: AtomicU64 = AtomicU64::new(0);
    static UI_VALUE_PRODUCTION_COUNT: AtomicU64 = AtomicU64::new(0);
    static FORCE_TERMINAL: AtomicBool = AtomicBool::new(false);
    static RULE_WIRE_VALID: AtomicBool = AtomicBool::new(false);

    fn write<T: serde::Serialize>(
        output: *mut ProjectRuntimeByteBuffer,
        value: T,
    ) -> ProjectRuntimeAbiStatus {
        ffi_boundary(output, || Ok(value))
    }

    unsafe extern "C" fn descriptor(
        _module: ProjectRuntimeOpaqueHandle,
        _session: ProjectRuntimeOpaqueHandle,
        _context: *const ProjectRuntimeCallContext,
        _request: ProjectRuntimeByteSlice,
        output: *mut ProjectRuntimeByteBuffer,
    ) -> ProjectRuntimeAbiStatus {
        write(
            output,
            SdkModuleDescriptor {
                module_id: MODULE_ID.to_string(),
                interface_version: PROJECT_RUNTIME_MODULE_INTERFACE_VERSION.to_string(),
                aot_content_digest: AOT_DIGEST.to_string(),
                ui_state_producer_id: "test.native.ui".to_string(),
                rules: vec![ProjectRuntimeRuleDescriptor {
                    rule_id: "project.test.native_rule".to_string(),
                    artifact_id: "rule-artifact:project.test.native_rule:hash".to_string(),
                }],
            },
        )
    }

    unsafe extern "C" fn create_session(
        module: ProjectRuntimeOpaqueHandle,
        _session: ProjectRuntimeOpaqueHandle,
        _context: *const ProjectRuntimeCallContext,
        request: ProjectRuntimeByteSlice,
        output: *mut ProjectRuntimeByteBuffer,
    ) -> ProjectRuntimeAbiStatus {
        // SAFETY: the host provides request storage for this call.
        let Ok(request) = (unsafe { read_input::<ProjectRuntimeSessionCreateRequest>(request) })
        else {
            return ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
        };
        if request.module_id != MODULE_ID {
            return ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
        }
        write(
            output,
            ProjectRuntimeSessionCreateResponse {
                handle_value: if module.value == 42 { 42 } else { 41 },
                handle_generation: 3,
            },
        )
    }

    unsafe extern "C" fn destroy_session_call(
        _module: ProjectRuntimeOpaqueHandle,
        session: ProjectRuntimeOpaqueHandle,
        _context: *const ProjectRuntimeCallContext,
        _request: ProjectRuntimeByteSlice,
        output: *mut ProjectRuntimeByteBuffer,
    ) -> ProjectRuntimeAbiStatus {
        if !matches!(session.value, 41 | 42) || session.generation != 3 {
            return ProjectRuntimeAbiStatus::INVALID_HANDLE;
        }
        DESTROY_COUNT.fetch_add(1, Ordering::SeqCst);
        write(output, serde_json::Value::Null)
    }

    unsafe extern "C" fn session_id(
        _module: ProjectRuntimeOpaqueHandle,
        session: ProjectRuntimeOpaqueHandle,
        _context: *const ProjectRuntimeCallContext,
        _request: ProjectRuntimeByteSlice,
        output: *mut ProjectRuntimeByteBuffer,
    ) -> ProjectRuntimeAbiStatus {
        if !matches!(session.value, 41 | 42) || session.generation != 3 {
            return ProjectRuntimeAbiStatus::INVALID_HANDLE;
        }
        write(output, "test.native.session")
    }

    fn require_context(
        context: *const ProjectRuntimeCallContext,
    ) -> Result<&'static ProjectRuntimeCallContext, ProjectRuntimeAbiStatus> {
        if context.is_null() {
            return Err(ProjectRuntimeAbiStatus::INVALID_ARGUMENT);
        }
        // SAFETY: fake callbacks borrow the host-owned context only for the callback.
        let context = unsafe { &*context };
        if context.struct_size < PROJECT_RUNTIME_CALL_CONTEXT_STRUCT_SIZE
            || context.host_api.is_null()
        {
            return Err(ProjectRuntimeAbiStatus::INVALID_ARGUMENT);
        }
        Ok(context)
    }

    fn query_world(
        context: &ProjectRuntimeCallContext,
    ) -> Result<Vec<String>, ProjectRuntimeAbiStatus> {
        // SAFETY: require_context validated the host API pointer for this call.
        let host_api = unsafe { &*context.host_api };
        let call = host_api
            .world_query
            .ok_or(ProjectRuntimeAbiStatus::UNSUPPORTED)?;
        let response: ProjectRuntimeWorldQueryResponse = call_host_json(
            call,
            context.host_context,
            &ProjectRuntimeWorldQueryRequest {
                all: vec![ComponentTypeId::transform().to_string()],
                none: Vec::new(),
            },
        )
        .map_err(|error| error.status)?;
        Ok(response.entity_ids)
    }

    unsafe extern "C" fn invoke_rule_call(
        module: ProjectRuntimeOpaqueHandle,
        session: ProjectRuntimeOpaqueHandle,
        context: *const ProjectRuntimeCallContext,
        request: ProjectRuntimeByteSlice,
        output: *mut ProjectRuntimeByteBuffer,
    ) -> ProjectRuntimeAbiStatus {
        let expected = if module.value == 42 { 42 } else { 41 };
        if session.value != expected || session.generation != 3 {
            return ProjectRuntimeAbiStatus::INVALID_HANDLE;
        }
        let Ok(context) = require_context(context) else {
            return ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
        };
        // SAFETY: the host provides request storage for this call.
        let Ok(request) = (unsafe { read_input::<ProjectRuntimeRuleRequest>(request) }) else {
            return ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
        };
        let Ok(entity_ids) = query_world(context) else {
            return ProjectRuntimeAbiStatus::FAILED;
        };
        UI_VALUE_PRODUCTION_COUNT.fetch_add(1, Ordering::SeqCst);
        RULE_WIRE_VALID.store(
            request.input_actions.iter().any(|action| {
                action.action_id == "action.move" && action.axis2 == Some([0.25, -0.5])
            }) && request.input_actions.iter().any(|action| {
                action.action_id == "action.fire" && action.phase.as_deref() == Some("pressed")
            }) && request.input_actions.iter().any(|action| {
                action.action_id == "action.pointer"
                    && action.phase.as_deref() == Some("pointer")
                    && action.axis2 == Some([880.0, 622.0])
            }) && request.collision_pairs.iter().any(|pair| {
                pair.entity_a == "collision-a"
                    && pair.entity_b == "collision-b"
                    && pair.is_sensor_pair
            }),
            Ordering::SeqCst,
        );
        let mut mutations = entity_ids
            .into_iter()
            .map(|entity_id| ProjectRuntimeDeferredMutation::WriteTransform {
                entity_id,
                transform: ProjectRuntimeTransform {
                    position: [request.frame.frame_index as f32, 2.0, 0.0],
                    rotation: [0.0; 3],
                    scale: [1.0; 3],
                },
            })
            .collect::<Vec<_>>();
        mutations.extend([
            ProjectRuntimeDeferredMutation::InstantiatePrefab {
                prefab_id: "prefab-native".to_string(),
                position: None,
            },
            ProjectRuntimeDeferredMutation::DespawnEntity {
                entity_id: "entity-native-despawn".to_string(),
            },
        ]);
        write(
            output,
            ProjectRuntimeRuleOutput {
                status: ProjectRuntimeStatus::Applied,
                mutations,
                diagnostics: Vec::new(),
            },
        )
    }

    unsafe extern "C" fn actions_call(
        _module: ProjectRuntimeOpaqueHandle,
        _session: ProjectRuntimeOpaqueHandle,
        context: *const ProjectRuntimeCallContext,
        request: ProjectRuntimeByteSlice,
        output: *mut ProjectRuntimeByteBuffer,
    ) -> ProjectRuntimeAbiStatus {
        if require_context(context).is_err() {
            return ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
        }
        // SAFETY: the host provides request storage for this call.
        let Ok(request) = (unsafe { read_input::<ProjectRuntimeAuiActionRequest>(request) }) else {
            return ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
        };
        ACTION_COUNT.fetch_add(1, Ordering::SeqCst);
        write(
            output,
            SdkSessionOutput {
                status: ProjectRuntimeStatus::Applied,
                handled_action_count: request.actions.len() as u64,
                unhandled_action_count: 0,
                rejected_action_count: 0,
                mutations: vec![ProjectRuntimeDeferredMutation::WriteTransform {
                    entity_id: ENTITY_ID.to_string(),
                    transform: ProjectRuntimeTransform {
                        position: [4.0, 5.0, 0.0],
                        rotation: [0.0; 3],
                        scale: [1.0; 3],
                    },
                }],
                diagnostics: Vec::new(),
            },
        )
    }

    unsafe extern "C" fn fixed_update_call(
        _module: ProjectRuntimeOpaqueHandle,
        _session: ProjectRuntimeOpaqueHandle,
        context: *const ProjectRuntimeCallContext,
        _request: ProjectRuntimeByteSlice,
        output: *mut ProjectRuntimeByteBuffer,
    ) -> ProjectRuntimeAbiStatus {
        if require_context(context).is_err() {
            return ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
        }
        FIXED_COUNT.fetch_add(1, Ordering::SeqCst);
        if FORCE_TERMINAL.load(Ordering::SeqCst) {
            return ProjectRuntimeAbiStatus::TERMINAL_FAULT;
        }
        write(output, SdkSessionOutput::no_op())
    }

    unsafe extern "C" fn ui_state_call(
        _module: ProjectRuntimeOpaqueHandle,
        session: ProjectRuntimeOpaqueHandle,
        context: *const ProjectRuntimeCallContext,
        request: ProjectRuntimeByteSlice,
        output: *mut ProjectRuntimeByteBuffer,
    ) -> ProjectRuntimeAbiStatus {
        if session.is_null() {
            return ProjectRuntimeAbiStatus::INVALID_HANDLE;
        }
        let Ok(context) = require_context(context) else {
            return ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
        };
        let Ok(request) = read_input::<ProjectRuntimeUiStateResolveRequest>(request) else {
            return ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
        };
        let identity = ProjectRuntimeUiStateIdentity {
            producer_epoch: session.value,
            visible_revision: 1,
            binding_set_digest: request.binding_set.digest().to_string(),
        };
        if request.previous_identity.as_ref() == Some(&identity) {
            return write(
                output,
                ProjectRuntimeUiStateResolveOutput::Reuse { identity },
            );
        }
        let Ok(entity_ids) = query_world(context) else {
            return ProjectRuntimeAbiStatus::FAILED;
        };
        write(
            output,
            ProjectRuntimeUiStateResolveOutput::Replace {
                identity,
                producer_id: "test.native.ui".to_string(),
                values: BTreeMap::from([(
                    "test.entity_count".to_string(),
                    ProjectRuntimeValue::Integer(entity_ids.len() as i64),
                )]),
            },
        )
    }

    unsafe extern "C" fn observe_call(
        _module: ProjectRuntimeOpaqueHandle,
        _session: ProjectRuntimeOpaqueHandle,
        context: *const ProjectRuntimeCallContext,
        _request: ProjectRuntimeByteSlice,
        output: *mut ProjectRuntimeByteBuffer,
    ) -> ProjectRuntimeAbiStatus {
        let Ok(context) = require_context(context) else {
            return ProjectRuntimeAbiStatus::INVALID_ARGUMENT;
        };
        let Ok(entity_ids) = query_world(context) else {
            return ProjectRuntimeAbiStatus::FAILED;
        };
        write(
            output,
            SdkObservationOutput {
                values: BTreeMap::from([(
                    "test.count".to_string(),
                    ProjectRuntimeValue::Integer(entity_ids.len() as i64),
                )]),
            },
        )
    }

    fn fake_api() -> ProjectRuntimeApi {
        let all: Option<ProjectRuntimeModuleCall> = Some(descriptor);
        ProjectRuntimeApi {
            struct_size: PROJECT_RUNTIME_API_STRUCT_SIZE,
            abi_major: PROJECT_RUNTIME_ABI_MAJOR,
            abi_minor: PROJECT_RUNTIME_ABI_MINOR,
            reserved: 0,
            capabilities: PROJECT_RUNTIME_CAP_RULES
                | PROJECT_RUNTIME_CAP_SESSIONS
                | PROJECT_RUNTIME_CAP_AUI_ACTIONS
                | PROJECT_RUNTIME_CAP_FIXED_UPDATE
                | PROJECT_RUNTIME_CAP_UI_STATE
                | PROJECT_RUNTIME_CAP_OBSERVATIONS
                | PROJECT_RUNTIME_CAP_WORLD_READ
                | PROJECT_RUNTIME_CAP_DEFERRED_MUTATIONS,
            module_context: ProjectRuntimeOpaqueHandle {
                value: 1,
                generation: 1,
            },
            contract_digest: project_runtime_contract_digest(),
            descriptor: all,
            create_session: Some(create_session),
            destroy_session: Some(destroy_session_call),
            session_id: Some(session_id),
            invoke_rule: Some(invoke_rule_call),
            handle_aui_actions: Some(actions_call),
            fixed_update: Some(fixed_update_call),
            resolve_ui_state: Some(ui_state_call),
            observe: Some(observe_call),
        }
    }

    fn package(aot_digest: &str) -> crate::runtime_package::RuntimePackage {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let package_dir = std::env::temp_dir()
            .join(format!("project-runtime-native-adapter-{stamp}"))
            .join("runtime-package");
        let mut input = RuntimePackageBuildInput::new(RuntimeProjectInfo::new(
            "project-test",
            "Test Project",
            "0.1.0",
            RuntimeProjectModuleRef::new(
                MODULE_ID,
                PROJECT_RUNTIME_MODULE_INTERFACE_VERSION,
                aot_digest,
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
        input.rule_manifest = Some(serde_json::from_value(serde_json::json!({
            "schemaVersion":"runtime-rule-manifest.v1", "mode":"rust-aot",
            "rules":[{"ruleId":"project.test.native_rule","phase":"Update","enabled":true,
                "executor":"rustAot","artifactId":"rule-artifact:project.test.native_rule:hash","irHash":"hash","irSource":"fixture"}],
            "modules":[{"artifactId":"rule-artifact:project.test.native_rule:hash","moduleKind":"staticRegistry","path":"fixture"}]
        })).unwrap());
        input.input_mappings.push(RuntimePackageSourceJson {
            id: input_none.asset_id.clone(),
            document: serde_json::to_value(input_none).unwrap(),
        });
        let report = RuntimePackageBuilder::build(
            &RuntimePackageBuildRequest::dev_desktop(&package_dir, "scene-main"),
            &input,
        );
        assert_eq!(report.status, RuntimePackageBuildStatus::Success);
        load_runtime_package(&package_dir).value.unwrap()
    }

    fn world() -> World {
        let mut world = World::new();
        world
            .try_spawn_with_components(
                EntityId::from(ENTITY_ID),
                "Native Adapter Entity",
                "test",
                true,
                Hierarchy {
                    parent_id: None,
                    sibling_order: 0,
                },
                Some(Transform::identity()),
                None,
            )
            .unwrap();
        world
    }

    #[test]
    fn project_runtime_native_adapter_fake_table_roundtrip() {
        DESTROY_COUNT.store(0, Ordering::SeqCst);
        ACTION_COUNT.store(0, Ordering::SeqCst);
        FIXED_COUNT.store(0, Ordering::SeqCst);
        UI_VALUE_PRODUCTION_COUNT.store(0, Ordering::SeqCst);
        FORCE_TERMINAL.store(false, Ordering::SeqCst);
        RULE_WIRE_VALID.store(false, Ordering::SeqCst);

        let adapter = LoadedProjectRuntimeModuleAdapter::new(fake_api()).expect("valid fake API");
        assert_eq!(adapter.descriptor().module_id, MODULE_ID);

        let wrong = LinkedProjectRuntimeSet::singleton(Arc::new(adapter.clone())).unwrap();
        let mismatch = ProjectRuntimeBootstrap::bind(&package("sha256:wrong"), &wrong)
            .err()
            .expect("descriptor mismatch must fail");
        assert_eq!(mismatch.code, "project_runtime.aot_digest_mismatch");

        let linked = LinkedProjectRuntimeSet::singleton(Arc::new(adapter.clone())).unwrap();
        let bound =
            ProjectRuntimeBootstrap::bind(&package(AOT_DIGEST), &linked).expect("exact bind");
        assert_eq!(bound.receipt().registered_rule_count, 1);
        assert_eq!(bound.receipt().producer_id, "test.native.ui");
        assert_eq!(bound.receipt().session_id, "test.native.session");

        let mut parts = bound.into_parts();
        let package = package(AOT_DIGEST);
        let mut world = world();
        let time = TimeContext::from_delta(2, 1.0 / 60.0, true);

        let actions = ActionSnapshot::with_actions(
            2,
            vec![
                InputActionState::axis2("action.move", 0.25, -0.5),
                InputActionState::pointer("action.pointer", 880.0, 622.0),
                InputActionState::button("action.fire", ActionPhase::Pressed),
            ],
        );
        let collisions = [CollisionPair {
            entity_a: EntityId::from("collision-a"),
            entity_b: EntityId::from("collision-b"),
            shape_a: Shape2D::Circle { radius: 1.0 },
            shape_b: Shape2D::Circle { radius: 1.0 },
            is_sensor_pair: true,
        }];
        let mut logic_context = LogicContext::with_time_context(
            2,
            time,
            crate::logic_executor::RulePhase::FixedUpdate,
            crate::world_api::WorldWriteApi::new(&mut world),
        )
        .with_action_snapshot(Some(&actions))
        .with_collision_pairs(&collisions);
        let rule = invoke_rule(
            &fake_api(),
            ProjectRuntimeOpaqueHandle {
                value: 41,
                generation: 3,
            },
            "project.test.native_rule",
            &mut logic_context,
        );
        assert_eq!(rule.status, crate::logic_executor::LogicStatus::Applied);
        assert_eq!(rule.writes.len(), 1);
        assert!(RULE_WIRE_VALID.load(Ordering::SeqCst));
        assert_eq!(logic_context.take_commands().len(), 2);
        drop(logic_context);
        assert_eq!(
            world
                .transform(&EntityId::from(ENTITY_ID))
                .unwrap()
                .local_position
                .x,
            2.0
        );

        let action = AuiAction {
            action_id: "test.action".to_string(),
            node_id: "button".to_string(),
            event: AuiActionEvent::Click,
            payload: None,
        };
        let action_output = parts.project_runtime_session.handle_aui_actions(
            ProjectRuntimeSessionContext {
                frame_index: 3,
                time,
                world: WorldReadApi::new(&world),
            },
            ProjectAuiActionBatch::new(&[action]),
        );
        assert_eq!(action_output.handled_action_count, 1);
        let prepared = action_output
            .prepare_mutations(&world)
            .expect("deferred mutation preflight");
        let crate::project_runtime_session::ProjectRuntimeMutationPreparation::Prepared(prepared) =
            prepared
        else {
            panic!("expected prepared native mutations")
        };
        prepared
            .commit(&mut world)
            .expect("deferred mutation commit");
        assert_eq!(
            world
                .transform(&EntityId::from(ENTITY_ID))
                .unwrap()
                .local_position
                .x,
            4.0
        );

        let mut ui_cache =
            crate::aui::ProjectUiStateSnapshotCache::new(["test.entity_count".to_string()]);
        let ui = ui_cache
            .resolve(
                parts.ui_state_producer.as_mut(),
                4,
                &package,
                &world,
                crate::aui::ProjectUiStateReportMode::Summary,
            )
            .expect("initial UI resolve");
        let crate::aui::ProjectUiStateSnapshotCacheResult::Replace(ui) = ui else {
            panic!("initial UI resolve must replace")
        };
        assert_eq!(
            ui.snapshot.values.get("test.entity_count"),
            Some(&AuiBindingValue::Number(1.0))
        );
        assert_eq!(
            ui_cache
                .resolve(
                    parts.ui_state_producer.as_mut(),
                    5,
                    &package,
                    &world,
                    crate::aui::ProjectUiStateReportMode::Summary,
                )
                .expect("clean UI resolve"),
            crate::aui::ProjectUiStateSnapshotCacheResult::Reuse
        );
        assert_eq!(UI_VALUE_PRODUCTION_COUNT.load(Ordering::SeqCst), 1);

        let observation = parts
            .project_runtime_session
            .observe(ProjectRuntimeObservationContext {
                frame_index: 4,
                time,
                world: WorldReadApi::new(&world),
                contract: &CookedProjectObservationContract {
                    schema_version: "test.v1".to_string(),
                    contract_id: "test".to_string(),
                    contract_digest: "sha256:test".to_string(),
                    observations: Vec::new(),
                },
                report_level: ProjectRuntimeSessionReportLevel::Summary,
            });
        assert_eq!(observation.len(), 1);

        let fixed = parts
            .project_runtime_session
            .fixed_update(ProjectRuntimeSessionContext {
                frame_index: 5,
                time,
                world: WorldReadApi::new(&world),
            });
        assert_eq!(fixed.status, ProjectRuntimeSessionStatus::NoOp);
        FORCE_TERMINAL.store(true, Ordering::SeqCst);
        let terminal = parts
            .project_runtime_session
            .fixed_update(ProjectRuntimeSessionContext {
                frame_index: 6,
                time,
                world: WorldReadApi::new(&world),
            });
        assert_eq!(terminal.status, ProjectRuntimeSessionStatus::Faulted);
        let fixed_calls = FIXED_COUNT.load(Ordering::SeqCst);
        let reentry = parts
            .project_runtime_session
            .fixed_update(ProjectRuntimeSessionContext {
                frame_index: 7,
                time,
                world: WorldReadApi::new(&world),
            });
        assert_eq!(reentry.status, ProjectRuntimeSessionStatus::Faulted);
        assert_eq!(FIXED_COUNT.load(Ordering::SeqCst), fixed_calls);

        let mut second_api = fake_api();
        second_api.module_context = ProjectRuntimeOpaqueHandle {
            value: 42,
            generation: 3,
        };
        let second_linked = linked_project_runtime_set_from_api(second_api).unwrap();
        let second = ProjectRuntimeBootstrap::bind(&package, &second_linked)
            .unwrap()
            .into_parts();
        let mut trace = crate::runtime_trace::RuntimeTrace::default();
        for runner in [&second.project_logic, &parts.project_logic] {
            let results =
                runner.run_frame_update_with_input(8, &mut world, &mut trace, Some(&actions));
            assert_eq!(results.len(), 1);
            assert_eq!(
                results[0].status,
                crate::logic_executor::LogicStatus::Applied
            );
        }
        drop(second);
        assert_eq!(DESTROY_COUNT.load(Ordering::SeqCst), 1);
        drop(parts.project_runtime_session);
        drop(parts.ui_state_producer);
        assert_eq!(DESTROY_COUNT.load(Ordering::SeqCst), 2);
        let expired = parts.project_logic.run_frame_update_with_input(
            9,
            &mut world,
            &mut trace,
            Some(&actions),
        );
        assert_eq!(
            expired[0].status,
            crate::logic_executor::LogicStatus::Failed
        );
        assert_eq!(ACTION_COUNT.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn project_runtime_native_adapter_rejects_contract_digest_mismatch() {
        let mut api = fake_api();
        api.contract_digest[0] ^= 0xff;
        let error = LoadedProjectRuntimeModuleAdapter::new(api)
            .err()
            .expect("digest mismatch must fail closed");
        assert_eq!(error.stage, "validate_api");
    }

    #[test]
    fn project_runtime_native_adapter_accepts_absent_optional_handlers_and_rejects_mismatch() {
        let mut api = fake_api();
        api.capabilities = project_runtime_abi::PROJECT_RUNTIME_CAP_SESSIONS;
        api.invoke_rule = None;
        api.handle_aui_actions = None;
        api.fixed_update = None;
        api.resolve_ui_state = None;
        api.observe = None;
        validate_api(&api).expect("sessions-only API with absent optional callbacks");

        api.capabilities |= project_runtime_abi::PROJECT_RUNTIME_CAP_FIXED_UPDATE;
        let error = validate_api(&api).expect_err("declared capability requires its callback");
        assert_eq!(error.stage, "validate_api");
        assert!(error.message.contains("fixed_update"));
    }
}
