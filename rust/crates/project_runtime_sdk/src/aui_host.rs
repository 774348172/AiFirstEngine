//! Call-scoped host transport used by generated Glue. No Rust ownership crosses the ABI.
use crate::aui::*;
use crate::aui_wire::{UiUpdateBatch, MAX_UI_UPDATE_WIRE_BYTES};
use project_runtime_abi::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub const MAX_UI_QUERY_BYTES: usize = 16 * 1024;
pub const MAX_UI_STRUCTURE_OPERATIONS: usize = 256;
pub const MAX_UI_STRUCTURE_WIRE_BYTES: usize = 1024 * 1024;
pub const UI_STRUCTURE_SCHEMA: &str = "aui-structure.v1;version:u32;operations<=256;operation:instantiate(template,parent,siblingIndex)|delete(target)|reparent(target,newParent,siblingIndex,keep_local|keep_screen)|set_sibling_index(target,index);result:templateInstanceId,root,localRefs;wire<=1048576;ordered;atomic";
pub const UI_STAGE_REPLY_BYTES: usize = 4096;
pub const UI_HOST_SCHEMA: &str = "ui-host.v6;query:resolve(document,node)|nodeType(handle)|read(handle,property)|localToScreen(handle,point)|screenToLocal(handle,point)|animation(play(description,owner,scope,clock,conflict,callbackId)|set_paused(handle,paused)|cancel(handle)|set_game_clock(paused,speed));animation:version1,steps<=256,depth<=32,session-slot-generation,instances<=4096,tracks<=4096,transactional;event:animation(handle,completed|cancelled|replaced|target_disposed|conflict|commit_conflict|error);coordinates:parent*local-affine,radians,normalized-pivot;inverse-error:NonInvertibleTransform;response:Result(handle|nodeType|value|point|animationReply,UiError);stage:aui-update.v7;structure:aui-structure.v1,operations<=256,wire<=1048576;subscribe:target,filter,scope,callbackId;cancel:token;invalidate:measure|paint;reply:Result(value,UiError);read-query<=16384;animation-query<=1048576;reply<=4194304;query-stage-reply-floor=4096;stateful-calls-once";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum UiQuery {
    Animation {
        request: crate::aui_animation::AnimationRequest,
    },
    Resolve {
        document: String,
        node: String,
    },
    NodeType {
        handle: UiNodeHandle,
    },
    Read {
        handle: UiNodeHandle,
        property: UiProperty,
    },
    LocalToScreen {
        handle: UiNodeHandle,
        point: [f32; 2],
    },
    ScreenToLocal {
        handle: UiNodeHandle,
        point: [f32; 2],
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UiQueryValue {
    Animation(crate::aui_animation::AnimationReply),
    Handle(UiNodeHandle),
    NodeType(UiNodeType),
    Value(UiValue),
    Point([f32; 2]),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiSubscribeRequest {
    pub target: UiNodeHandle,
    pub filter: UiEventFilter,
    pub scope: UiSubscriptionScope,
    pub callback_id: UiCallbackId,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiCancelRequest {
    pub token: UiSubscriptionToken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiInvalidateKind {
    Measure,
    Paint,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiInvalidateRequest {
    pub target: UiNodeHandle,
    pub kind: UiInvalidateKind,
}

pub fn unavailable() -> UiError {
    UiError {
        kind: UiErrorKind::Unavailable,
        handle: None,
        property: None,
    }
}

fn invalid() -> UiError {
    UiError {
        kind: UiErrorKind::InvalidValue,
        handle: None,
        property: None,
    }
}

/// Glue holds this adapter only while the host's context token is live.
pub struct HostUiAccess {
    query: Option<ProjectRuntimeHostCall>,
    stage: Option<ProjectRuntimeHostCall>,
    subscribe: Option<ProjectRuntimeHostCall>,
    cancel: Option<ProjectRuntimeHostCall>,
    structure: Option<ProjectRuntimeHostCall>,
    invalidate: Option<ProjectRuntimeHostCall>,
    context: ProjectRuntimeOpaqueHandle,
}

impl HostUiAccess {
    pub fn new(
        query: Option<ProjectRuntimeHostCall>,
        stage: Option<ProjectRuntimeHostCall>,
        subscribe: Option<ProjectRuntimeHostCall>,
        cancel: Option<ProjectRuntimeHostCall>,
        structure: Option<ProjectRuntimeHostCall>,
        context: ProjectRuntimeOpaqueHandle,
    ) -> Self {
        Self {
            query,
            stage,
            subscribe,
            cancel,
            structure,
            invalidate: None,
            context,
        }
    }

    pub fn new_with_invalidate(
        query: Option<ProjectRuntimeHostCall>,
        stage: Option<ProjectRuntimeHostCall>,
        subscribe: Option<ProjectRuntimeHostCall>,
        cancel: Option<ProjectRuntimeHostCall>,
        structure: Option<ProjectRuntimeHostCall>,
        invalidate: Option<ProjectRuntimeHostCall>,
        context: ProjectRuntimeOpaqueHandle,
    ) -> Self {
        Self {
            query,
            stage,
            subscribe,
            cancel,
            structure,
            invalidate,
            context,
        }
    }

    fn query(&self, request: UiQuery) -> Result<UiQueryValue, UiError> {
        let bytes = serde_json::to_vec(&request).map_err(|_| invalid())?;
        if bytes.len() > MAX_UI_QUERY_BYTES {
            return Err(invalid());
        }
        self.call(self.query.ok_or_else(unavailable)?, &bytes, true)?
    }

    /// Exactly one stateful call, even if a broken host reports insufficient output space.
    pub fn stage(&self, batch: UiUpdateBatch) -> Result<(), UiError> {
        if batch.updates().is_empty()
            && batch.custom_updates().is_empty()
            && batch.ownership_updates().is_empty()
        {
            return Ok(());
        }
        self.call(self.stage.ok_or_else(unavailable)?, &batch.encode()?, false)?
    }

    fn structure(&self, transaction: UiStructureTransaction) -> Result<UiStructureResult, UiError> {
        if transaction.version != UiStructureTransaction::VERSION
            || transaction.operations.len() > MAX_UI_STRUCTURE_OPERATIONS
        {
            return Err(invalid());
        }
        let request = serde_json::to_vec(&transaction).map_err(|_| invalid())?;
        if request.len() > MAX_UI_STRUCTURE_WIRE_BYTES {
            return Err(UiError {
                kind: UiErrorKind::CapacityExceeded,
                handle: None,
                property: None,
            });
        }
        let call = self.structure.ok_or_else(unavailable)?;
        let mut bytes = vec![0; MAX_UI_STRUCTURE_WIRE_BYTES];
        let mut output = ProjectRuntimeByteBuffer::from_slice(&mut bytes);
        // SAFETY: the context and both buffers are live for exactly this invocation.
        let status = unsafe {
            call(
                self.context,
                ProjectRuntimeByteSlice::from_slice(&request),
                &mut output,
            )
        };
        let written = usize::try_from(output.written).map_err(|_| invalid())?;
        if !status.is_ok() || written > bytes.len() {
            return Err(invalid());
        }
        serde_json::from_slice(&bytes[..written]).map_err(|_| invalid())?
    }

    fn call<T: DeserializeOwned>(
        &self,
        call: ProjectRuntimeHostCall,
        request: &[u8],
        retry_read: bool,
    ) -> Result<T, UiError> {
        let mut bytes = vec![0; UI_STAGE_REPLY_BYTES];
        for attempt in 0..2 {
            let mut output = ProjectRuntimeByteBuffer::from_slice(&mut bytes);
            // SAFETY: live caller-owned storage; callbacks borrow it only for this invocation.
            let status = unsafe {
                call(
                    self.context,
                    ProjectRuntimeByteSlice::from_slice(request),
                    &mut output,
                )
            };
            let written = usize::try_from(output.written).map_err(|_| invalid())?;
            if status == ProjectRuntimeAbiStatus::BUFFER_TOO_SMALL && retry_read && attempt == 0 {
                if written <= bytes.len() || written > MAX_UI_UPDATE_WIRE_BYTES {
                    return Err(invalid());
                }
                bytes.resize(written, 0);
                continue;
            }
            if !status.is_ok() || written > bytes.len() {
                return Err(invalid());
            }
            return serde_json::from_slice(&bytes[..written]).map_err(|_| invalid());
        }
        Err(invalid())
    }
}

impl UiAccess for HostUiAccess {
    fn resolve(&self, document: &str, node: &str) -> Result<UiNodeHandle, UiError> {
        match self.query(UiQuery::Resolve {
            document: document.into(),
            node: node.into(),
        })? {
            UiQueryValue::Handle(handle) => Ok(handle),
            _ => Err(invalid()),
        }
    }
}

impl crate::aui_animation::UiAnimationAccess for HostUiAccess {
    fn animation_request(
        &self,
        request: crate::aui_animation::AnimationRequest,
    ) -> Result<crate::aui_animation::AnimationReply, UiError> {
        if let crate::aui_animation::AnimationRequest::Play { request } = &request {
            request.animation.validate().map_err(|_| invalid())?;
        }
        let bytes = serde_json::to_vec(&UiQuery::Animation { request }).map_err(|_| invalid())?;
        if bytes.len() > MAX_UI_STRUCTURE_WIRE_BYTES {
            return Err(invalid());
        }
        // Stateful queries must never retry: the reserved handle belongs to this one call.
        match self.call::<Result<UiQueryValue, UiError>>(
            self.query.ok_or_else(unavailable)?,
            &bytes,
            false,
        )?? {
            UiQueryValue::Animation(reply) => Ok(reply),
            _ => Err(invalid()),
        }
    }
}

impl UiRead for HostUiAccess {
    fn node_type(&self, handle: UiNodeHandle) -> Result<UiNodeType, UiError> {
        match self.query(UiQuery::NodeType { handle })? {
            UiQueryValue::NodeType(kind) => Ok(kind),
            _ => Err(invalid()),
        }
    }
    fn read(&self, handle: UiNodeHandle, property: UiProperty) -> Result<UiValue, UiError> {
        match self.query(UiQuery::Read { handle, property })? {
            UiQueryValue::Value(value) => Ok(value),
            _ => Err(invalid()),
        }
    }
}

impl UiGeometryAccess for HostUiAccess {
    fn local_to_screen(&self, handle: UiNodeHandle, point: [f32; 2]) -> Result<[f32; 2], UiError> {
        match self.query(UiQuery::LocalToScreen { handle, point })? {
            UiQueryValue::Point(point) => Ok(point),
            _ => Err(invalid()),
        }
    }

    fn screen_to_local(&self, handle: UiNodeHandle, point: [f32; 2]) -> Result<[f32; 2], UiError> {
        match self.query(UiQuery::ScreenToLocal { handle, point })? {
            UiQueryValue::Point(point) => Ok(point),
            _ => Err(invalid()),
        }
    }
}

impl UiStructureAccess for HostUiAccess {
    fn transact_structure(
        &self,
        transaction: UiStructureTransaction,
    ) -> Result<UiStructureResult, UiError> {
        self.structure(transaction)
    }
}

impl crate::aui_wire::UiPropertyStage for HostUiAccess {
    fn stage_properties(&self, batch: UiUpdateBatch) -> Result<(), UiError> {
        self.stage(batch)
    }
}

impl UiEvents for HostUiAccess {
    fn subscribe(
        &self,
        target: UiNodeHandle,
        filter: UiEventFilter,
        scope: UiSubscriptionScope,
        callback_id: UiCallbackId,
    ) -> Result<UiSubscriptionToken, UiError> {
        if callback_id.0 == 0 {
            return Err(invalid());
        }
        self.call(
            self.subscribe.ok_or_else(unavailable)?,
            &serde_json::to_vec(&UiSubscribeRequest {
                target,
                filter,
                scope,
                callback_id,
            })
            .map_err(|_| invalid())?,
            false,
        )?
    }

    fn cancel(&self, token: UiSubscriptionToken) -> Result<bool, UiError> {
        self.call(
            self.cancel.ok_or_else(unavailable)?,
            &serde_json::to_vec(&UiCancelRequest { token }).map_err(|_| invalid())?,
            false,
        )?
    }
}

impl UiInvalidationAccess for HostUiAccess {
    fn request_measure(&self, target: UiNodeHandle) -> Result<(), UiError> {
        self.call(
            self.invalidate.ok_or_else(unavailable)?,
            &serde_json::to_vec(&UiInvalidateRequest {
                target,
                kind: UiInvalidateKind::Measure,
            })
            .map_err(|_| invalid())?,
            false,
        )?
    }

    fn request_repaint(&self, target: UiNodeHandle) -> Result<(), UiError> {
        self.call(
            self.invalidate.ok_or_else(unavailable)?,
            &serde_json::to_vec(&UiInvalidateRequest {
                target,
                kind: UiInvalidateKind::Paint,
            })
            .map_err(|_| invalid())?,
            false,
        )?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aui_wire::UiPropertyUpdate;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;
    static CALLS: AtomicUsize = AtomicUsize::new(0);
    static CALL_LOCK: Mutex<()> = Mutex::new(());

    unsafe extern "C" fn oversized_reply(
        _: ProjectRuntimeOpaqueHandle,
        _: ProjectRuntimeByteSlice,
        output: *mut ProjectRuntimeByteBuffer,
    ) -> ProjectRuntimeAbiStatus {
        CALLS.fetch_add(1, Ordering::SeqCst);
        // SAFETY: this fixture receives a live caller-owned descriptor.
        unsafe {
            (*output).written = (MAX_UI_UPDATE_WIRE_BYTES + 1) as u64;
        }
        ProjectRuntimeAbiStatus::BUFFER_TOO_SMALL
    }

    #[test]
    fn stateful_stage_is_not_retried_and_read_size_is_bounded() {
        let _lock = CALL_LOCK.lock().unwrap();
        CALLS.store(0, Ordering::SeqCst);
        let access = HostUiAccess::new(
            Some(oversized_reply),
            Some(oversized_reply),
            None,
            None,
            Some(oversized_reply),
            ProjectRuntimeOpaqueHandle::NULL,
        );
        let batch = UiUpdateBatch::new(vec![UiPropertyUpdate {
            handle: UiNodeHandle {
                instance: 1,
                slot: 0,
                generation: 1,
            },
            property: UiProperty::Text,
            value: UiValue::Text("人".into()),
            notify: true,
        }])
        .unwrap();
        assert!(access.stage(batch).is_err());
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
        assert!(access.resolve("a", "b").is_err());
        assert_eq!(CALLS.load(Ordering::SeqCst), 2);
        assert!(access
            .resolve(&"x".repeat(MAX_UI_QUERY_BYTES), "b")
            .is_err());
        assert_eq!(CALLS.load(Ordering::SeqCst), 2);
        assert!(access
            .transact_structure(UiStructureTransaction::new(Vec::new()))
            .is_err());
        assert_eq!(CALLS.load(Ordering::SeqCst), 3);
        use crate::aui_animation::{AnimationRequest, UiAnimationAccess};
        assert!(access
            .animation_request(AnimationRequest::SetGameClock {
                paused: true,
                speed: 1.
            })
            .is_err());
        assert_eq!(
            CALLS.load(Ordering::SeqCst),
            4,
            "stateful animation transport never retries"
        );
    }

    #[test]
    fn query_wire_rejects_unknown_fields() {
        assert!(serde_json::from_str::<UiQuery>(r#"{"read":{"handle":{"instance":1,"slot":0,"generation":1},"property":"text","unexpected":true}}"#).is_err());
        let error = unavailable();
        let bytes = serde_json::to_vec(&Result::<UiQueryValue, UiError>::Err(error)).unwrap();
        let decoded: Result<UiQueryValue, UiError> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded.unwrap_err(), error);
    }

    unsafe extern "C" fn animation_reply(
        _: ProjectRuntimeOpaqueHandle,
        request: ProjectRuntimeByteSlice,
        output: *mut ProjectRuntimeByteBuffer,
    ) -> ProjectRuntimeAbiStatus {
        // SAFETY: this test transport receives borrowed, valid ABI buffers.
        unsafe {
            crate::ffi_boundary(output, || {
                let query: UiQuery = crate::read_input(request)?;
                use crate::aui_animation::{AnimationHandle, AnimationReply, AnimationRequest};
                let reply: Result<UiQueryValue, UiError> = match query {
                    UiQuery::Animation {
                        request: AnimationRequest::Play { .. },
                    } => Ok(UiQueryValue::Animation(AnimationReply::Handle(
                        AnimationHandle {
                            session: 42,
                            slot: 3,
                            generation: 7,
                        },
                    ))),
                    UiQuery::Animation {
                        request: AnimationRequest::Cancel { .. },
                    } => Err(UiError {
                        kind: UiErrorKind::InvalidHandle,
                        handle: None,
                        property: None,
                    }),
                    UiQuery::Animation { .. } => Ok(UiQueryValue::Animation(AnimationReply::Done)),
                    _ => Err(invalid()),
                };
                Ok(reply)
            })
        }
    }

    #[test]
    fn animation_transport_consumes_the_host_result_envelope_and_typed_handle() {
        use crate::aui_animation::*;
        let access = HostUiAccess::new(
            Some(animation_reply),
            None,
            None,
            None,
            None,
            ProjectRuntimeOpaqueHandle::NULL,
        );
        let target = UiNodeHandle {
            instance: 1,
            slot: 0,
            generation: 1,
        };
        let reply = access
            .animation_request(AnimationRequest::Play {
                request: AnimationPlay {
                    owner: target,
                    scope: UiSubscriptionScope::Active,
                    clock: AnimationClock::Ui,
                    conflict: TrackConflictPolicy::Reject,
                    callback_id: None,
                    animation: Animation2D {
                        version: ANIMATION_2D_VERSION,
                        root: AnimationStep::Delay { duration_us: 10 },
                        repeat: Repeat::Once,
                        direction: PlaybackDirection::Forward,
                        completion: CompletionPolicy::Release,
                    },
                },
            })
            .unwrap();
        let handle = AnimationHandle {
            session: 42,
            slot: 3,
            generation: 7,
        };
        assert_eq!(reply, AnimationReply::Handle(handle));
        assert_eq!(
            access
                .animation_request(AnimationRequest::SetGameClock {
                    paused: true,
                    speed: 0.5
                })
                .unwrap(),
            AnimationReply::Done
        );
        assert_eq!(
            access
                .animation_request(AnimationRequest::Cancel { handle })
                .unwrap_err()
                .kind,
            UiErrorKind::InvalidHandle
        );
    }

    #[test]
    fn ownership_only_stage_reaches_host_so_animation_commit_observes_owner_release() {
        let _lock = CALL_LOCK.lock().unwrap();
        CALLS.store(0, Ordering::SeqCst);
        let access = HostUiAccess::new(
            None,
            Some(oversized_reply),
            None,
            None,
            None,
            ProjectRuntimeOpaqueHandle::NULL,
        );
        let batch = UiUpdateBatch::new_with_custom_and_ownership(
            Vec::new(),
            Vec::new(),
            vec![crate::aui_wire::UiOwnershipUpdate {
                handle: UiNodeHandle {
                    instance: 1,
                    slot: 0,
                    generation: 1,
                },
                property: UiProperty::Opacity,
                owner: UiPropertyOwner::Binding,
            }],
        )
        .unwrap();
        assert!(
            access.stage(batch).is_err(),
            "ownership-only transport must call the rejecting fixture host"
        );
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn structure_contract_rejects_operation_capacity_before_host_call() {
        let _lock = CALL_LOCK.lock().unwrap();
        CALLS.store(0, Ordering::SeqCst);
        let access = HostUiAccess::new(
            None,
            None,
            None,
            None,
            Some(oversized_reply),
            ProjectRuntimeOpaqueHandle::NULL,
        );
        let handle = UiNodeHandle {
            instance: 1,
            slot: 0,
            generation: 1,
        };
        let operations = (0..=MAX_UI_STRUCTURE_OPERATIONS)
            .map(|_| UiStructureOperation::Delete { target: handle })
            .collect();
        assert_eq!(
            access
                .transact_structure(UiStructureTransaction::new(operations))
                .unwrap_err()
                .kind,
            UiErrorKind::InvalidValue
        );
        assert_eq!(CALLS.load(Ordering::SeqCst), 0);
    }
}
