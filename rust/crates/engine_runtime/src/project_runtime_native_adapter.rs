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
use crate::logic_executor::{ExecutorKind, LogicContext, LogicResult};
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
use std::sync::Arc;

const NATIVE_CALL_FAILED: &str = "project_runtime.native_module_call_failed";
const NATIVE_TERMINAL_FAULT: &str = "project_runtime.native_module_terminal_fault";

#[derive(Clone)]
pub struct LoadedProjectRuntimeModuleAdapter {
    api: Arc<ProjectRuntimeApi>,
    descriptor: ProjectRuntimeModuleDescriptor,
    rules: Vec<project_runtime_sdk::ProjectRuntimeRuleDescriptor>,
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
        Ok(Self {
            descriptor: ProjectRuntimeModuleDescriptor {
                module_id: descriptor.module_id,
                interface_version: descriptor.interface_version,
                aot_content_digest: descriptor.aot_content_digest,
            },
            rules: descriptor.rules,
            producer_id: descriptor.ui_state_producer_id,
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

impl ProjectRuntimeModule for LoadedProjectRuntimeModuleAdapter {
    fn descriptor(&self) -> &ProjectRuntimeModuleDescriptor {
        &self.descriptor
    }

    fn install(
        &self,
        registration: &mut ProjectRuntimeRegistration,
    ) -> Result<(), ProjectRuntimeError> {
        for rule in &self.rules {
            let api = Arc::clone(&self.api);
            let lifetime_guard = self._lifetime_guard.clone();
            let rule_id = rule.rule_id.clone();
            let callback_rule_id = rule_id.clone();
            registration.register_rust_aot_rule(
                rule_id,
                rule.artifact_id.clone(),
                move |context| {
                    let _keep_library_loaded = &lifetime_guard;
                    invoke_rule(&api, &callback_rule_id, context)
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
                        engine_input::ActionValue::Pointer { .. } => (None, None, None),
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
                ProjectRuntimeOpaqueHandle::NULL,
                Some(&call_context),
                &ProjectRuntimeRuleRequest {
                    rule_id: rule_id.to_string(),
                    frame: frame_request(frame_index, time),
                    input_actions,
                    collision_pairs,
                },
            )
        });
    let Ok(response) = response else {
        return LogicResult::failed(
            rule_id,
            ExecutorKind::RustAot,
            NATIVE_CALL_FAILED,
            "native rule callback failed",
        );
    };
    if response.status == ProjectRuntimeStatus::Faulted {
        return LogicResult::failed(
            rule_id,
            ExecutorKind::RustAot,
            NATIVE_TERMINAL_FAULT,
            response.diagnostics.join("; "),
        );
    }
    if response.status == ProjectRuntimeStatus::Rejected {
        return LogicResult::failed(
            rule_id,
            ExecutorKind::RustAot,
            NATIVE_CALL_FAILED,
            response.diagnostics.join("; "),
        );
    }
    if matches!(
        response.status,
        ProjectRuntimeStatus::NoOp | ProjectRuntimeStatus::Unhandled
    ) {
        return LogicResult::skipped(rule_id, ExecutorKind::RustAot);
    }
    let mut result = LogicResult::applied(rule_id, ExecutorKind::RustAot);
    for mutation in response.mutations {
        match apply_rule_mutation(host.context, mutation) {
            Ok(Some(write)) => result.writes.push(write),
            Ok(None) => {}
            Err(message) => {
                return LogicResult::failed(
                    rule_id,
                    ExecutorKind::RustAot,
                    NATIVE_CALL_FAILED,
                    message,
                )
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
        append_deferred_mutation(&mut mutations, mutation).map_err(|message| {
            project_runtime_sdk::ProjectRuntimeSdkError {
                status: ProjectRuntimeAbiStatus::FAILED,
                message,
            }
        })?;
    }
    Ok(ProjectRuntimeSessionOutput {
        status: session_status(output.status),
        handled_action_count: usize::try_from(output.handled_action_count).map_err(|_| {
            project_runtime_sdk::ProjectRuntimeSdkError {
                status: ProjectRuntimeAbiStatus::FAILED,
                message: "handled action count exceeds host range".to_string(),
            }
        })?,
        unhandled_action_count: usize::try_from(output.unhandled_action_count).map_err(|_| {
            project_runtime_sdk::ProjectRuntimeSdkError {
                status: ProjectRuntimeAbiStatus::FAILED,
                message: "unhandled action count exceeds host range".to_string(),
            }
        })?,
        rejected_action_count: usize::try_from(output.rejected_action_count).map_err(|_| {
            project_runtime_sdk::ProjectRuntimeSdkError {
                status: ProjectRuntimeAbiStatus::FAILED,
                message: "rejected action count exceeds host range".to_string(),
            }
        })?,
        mutations,
        diagnostics: if output.diagnostics.is_empty() {
            Vec::new()
        } else {
            vec![NATIVE_CALL_FAILED]
        },
    })
}

fn append_deferred_mutation(
    output: &mut ProjectRuntimeMutationBuffer,
    mutation: ProjectRuntimeDeferredMutation,
) -> Result<(), String> {
    match mutation {
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
        ProjectRuntimeDeferredMutation::InstantiatePrefab { prefab_id } => {
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

fn apply_rule_mutation(
    context: &mut LogicContext<'_>,
    mutation: ProjectRuntimeDeferredMutation,
) -> Result<Option<crate::logic_executor::LogicWrite>, String> {
    match mutation {
        ProjectRuntimeDeferredMutation::WriteTransform {
            entity_id,
            transform,
        } => context
            .write_component(
                EntityId::from(entity_id),
                ComponentTypeId::transform(),
                ComponentValue::Transform(engine_transform(transform)),
            )
            .map(Some)
            .map_err(|error| error.message),
        ProjectRuntimeDeferredMutation::WriteComponentField {
            entity_id,
            component_type,
            field_path,
            value,
        } => context
            .write_component_field(
                EntityId::from(entity_id),
                ComponentTypeId::from(component_type),
                &FieldPath::parse(field_path).map_err(|error| error.code.to_string())?,
                runtime_value(value)?,
            )
            .map(Some)
            .map_err(|error| error.message),
        ProjectRuntimeDeferredMutation::ReplaceDynamicComponent { .. } => {
            Err("replace_dynamic_component is session-deferred only".to_string())
        }
        ProjectRuntimeDeferredMutation::InstantiatePrefab { prefab_id } => {
            context.request_instantiate_prefab(
                crate::runtime_package::RuntimeAssetRef {
                    id: prefab_id,
                    asset_type: "prefab".to_string(),
                    guid: None,
                    sub_asset: None,
                },
                None,
                None,
            );
            Ok(None)
        }
        ProjectRuntimeDeferredMutation::DespawnEntity { entity_id } => {
            context.request_despawn_entity(EntityId::from(entity_id));
            Ok(None)
        }
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
    let required_capabilities = project_runtime_abi::PROJECT_RUNTIME_CAP_RULES
        | project_runtime_abi::PROJECT_RUNTIME_CAP_SESSIONS
        | project_runtime_abi::PROJECT_RUNTIME_CAP_AUI_ACTIONS
        | project_runtime_abi::PROJECT_RUNTIME_CAP_FIXED_UPDATE
        | project_runtime_abi::PROJECT_RUNTIME_CAP_UI_STATE
        | project_runtime_abi::PROJECT_RUNTIME_CAP_OBSERVATIONS
        | project_runtime_abi::PROJECT_RUNTIME_CAP_WORLD_READ
        | project_runtime_abi::PROJECT_RUNTIME_CAP_DEFERRED_MUTATIONS;
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
        ("invoke_rule", api.invoke_rule),
        ("handle_aui_actions", api.handle_aui_actions),
        ("fixed_update", api.fixed_update),
        ("resolve_ui_state", api.resolve_ui_state),
        ("observe", api.observe),
    ] {
        required_call(call, name)?;
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
        _module: ProjectRuntimeOpaqueHandle,
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
                handle_value: 41,
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
        if session.value != 41 || session.generation != 3 {
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
        if session.value != 41 || session.generation != 3 {
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
        _module: ProjectRuntimeOpaqueHandle,
        _session: ProjectRuntimeOpaqueHandle,
        context: *const ProjectRuntimeCallContext,
        request: ProjectRuntimeByteSlice,
        output: *mut ProjectRuntimeByteBuffer,
    ) -> ProjectRuntimeAbiStatus {
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
            "0.0.2",
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
        let rule = invoke_rule(&fake_api(), "project.test.native_rule", &mut logic_context);
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

        assert_eq!(DESTROY_COUNT.load(Ordering::SeqCst), 0);
        drop(parts);
        assert_eq!(DESTROY_COUNT.load(Ordering::SeqCst), 1);
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
}
