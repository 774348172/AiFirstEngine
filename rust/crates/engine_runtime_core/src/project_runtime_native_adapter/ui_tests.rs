use super::*;
use crate::aui::*;
use crate::project_runtime_session::{
    execute_project_runtime_session_stage_with_media, ProjectRuntimeSessionReportLevel,
    ProjectRuntimeSessionStage,
};
use crate::runtime_package::RuntimeAuiDocumentRegistry;
use crate::world::World;
use project_runtime_sdk::aui::*;
use project_runtime_sdk::aui_extension::{
    UiExtensionChildInput, UiExtensionConstraints, UiExtensionInput,
};
use project_runtime_sdk::aui_host::HostUiAccess;
use project_runtime_sdk::aui_wire::UiWriteBuffer;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

static UI_EVENT_TEST_LOCK: Mutex<()> = Mutex::new(());
static UI_EVENT_CALLS: AtomicUsize = AtomicUsize::new(0);
static UI_TOKEN_INSTANCE: AtomicU64 = AtomicU64::new(0);
static UI_TOKEN_ID: AtomicU64 = AtomicU64::new(0);
static UI_EXTENSION_DESCRIBE_CALLS: AtomicUsize = AtomicUsize::new(0);
static UI_EXTENSION_MEASURE_CALLS: AtomicUsize = AtomicUsize::new(0);
static UI_EXTENSION_LAYOUT_CALLS: AtomicUsize = AtomicUsize::new(0);
static UI_EXTENSION_PAINT_CALLS: AtomicUsize = AtomicUsize::new(0);
static UI_EXTENSION_INVALID_PAINT: AtomicBool = AtomicBool::new(false);

fn documents(binding: bool) -> RuntimeAuiDocumentRegistry {
    let mut registry = RuntimeAuiDocumentRegistry::empty("ui-test");
    for id in ["a", "b"] {
        let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
        root.children.push("label".into());
        let mut text = AuiNode::new(
            "label",
            AuiNodeKind::Text,
            AuiRect::fixed_position(0., 0., 100., 20.),
        );
        text.parent = Some("root".into());
        text.text = Some("人".into());
        if binding && id == "b" {
            text.binding_refs.push(AuiBindingRef::new(
                "label",
                AuiBindingTarget::TextText,
                "game.label",
                None,
            ));
        }
        let document = AuiDocument::new(
            id,
            vec![AuiCanvas::screen_overlay("main", 800., 600., "root")],
            vec![root, text],
        );
        registry.documents_by_id.insert(id.into(), document);
    }
    registry
}

fn custom_documents() -> RuntimeAuiDocumentRegistry {
    let mut registry = RuntimeAuiDocumentRegistry::empty("ui-extension-test");
    let mut node = AuiNode::new(
        "circle",
        AuiNodeKind::Custom,
        AuiRect::fixed_position(0.0, 0.0, 50.0, 50.0),
    );
    node.custom = Some(crate::aui_extension::AuiCustomNodeSpec {
        type_id: "sample.circle".into(),
        schema_version: 1,
        properties: BTreeMap::from([(
            "progress".into(),
            project_runtime_sdk::aui_extension::UiExtensionValue::Float(0.5),
        )]),
    });
    node.children.push("child".into());
    let mut child = AuiNode::new(
        "child",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(0.0, 0.0, 10.0, 10.0),
    );
    child.parent = Some("circle".into());
    registry.documents_by_id.insert(
        "custom".into(),
        AuiDocument::new(
            "custom",
            vec![AuiCanvas::screen_overlay("main", 100.0, 100.0, "circle")],
            vec![node, child],
        ),
    );
    registry
}

unsafe extern "C" fn ui_extension(
    _: ProjectRuntimeOpaqueHandle,
    _: ProjectRuntimeOpaqueHandle,
    context: *const ProjectRuntimeCallContext,
    request: ProjectRuntimeByteSlice,
    output: *mut ProjectRuntimeByteBuffer,
) -> ProjectRuntimeAbiStatus {
    assert!(
        context.is_null(),
        "extension callback must not receive mutable host context"
    );
    ffi_boundary(output, || {
        use project_runtime_sdk::aui_extension::*;
        let batch: UiExtensionRequestBatch = unsafe { read_input(request) }?;
        let responses = batch
            .requests
            .into_iter()
            .map(|request| {
                let output = match request.input {
                    UiExtensionInput::Describe => {
                        UI_EXTENSION_DESCRIBE_CALLS.fetch_add(1, Ordering::SeqCst);
                        UiExtensionOutput::Describe {
                            descriptor: UiExtensionTypeDescriptor {
                                type_id: "sample.circle".into(),
                                schema_version: 1,
                                callback_id: 91,
                                properties: vec![UiExtensionPropertyDescriptor {
                                    name: "progress".into(),
                                    property_type: UiExtensionPropertyType::Float,
                                    invalidates: vec![UiExtensionInvalidation::Paint],
                                }],
                                measure: true,
                                layout: true,
                                paint: true,
                            },
                        }
                    }
                    UiExtensionInput::Measure { .. } => {
                        UI_EXTENSION_MEASURE_CALLS.fetch_add(1, Ordering::SeqCst);
                        UiExtensionOutput::Measure {
                            output: UiExtensionMeasureOutput {
                                size: [50.0, 50.0],
                                first_baseline: None,
                            },
                        }
                    }
                    UiExtensionInput::Layout { size, children } => {
                        UI_EXTENSION_LAYOUT_CALLS.fetch_add(1, Ordering::SeqCst);
                        UiExtensionOutput::Layout {
                            children: children
                                .into_iter()
                                .map(|child| UiExtensionChildRect {
                                    handle: child.handle,
                                    rect: [
                                        size[0] - child.measured_size[0],
                                        size[1] - child.measured_size[1],
                                        child.measured_size[0],
                                        child.measured_size[1],
                                    ],
                                })
                                .collect(),
                        }
                    }
                    UiExtensionInput::Paint { .. } => {
                        UI_EXTENSION_PAINT_CALLS.fetch_add(1, Ordering::SeqCst);
                        UiExtensionOutput::Paint {
                            output: UiExtensionPaintOutput {
                                vertices: vec![
                                    UiExtensionMeshVertex {
                                        position: [25.0, 25.0],
                                        uv: [0.5, 0.5],
                                        color: [1.0, 0.0, 0.0, 1.0],
                                    },
                                    UiExtensionMeshVertex {
                                        position: [25.0, 0.0],
                                        uv: [0.5, 0.0],
                                        color: [1.0, 0.0, 0.0, 1.0],
                                    },
                                    UiExtensionMeshVertex {
                                        position: [50.0, 25.0],
                                        uv: [1.0, 0.5],
                                        color: [1.0, 0.0, 0.0, 1.0],
                                    },
                                ],
                                indices: if UI_EXTENSION_INVALID_PAINT.load(Ordering::SeqCst) {
                                    vec![0, 1, 999]
                                } else {
                                    vec![0, 1, 2]
                                },
                                asset_id: None,
                                primitives: vec![],
                            },
                        }
                    }
                };
                UiExtensionResponse {
                    request_id: request.request_id,
                    output,
                }
            })
            .collect();
        Ok(UiExtensionResponseBatch {
            version: UI_EXTENSION_VERSION,
            responses,
        })
    })
}

unsafe extern "C" fn invalidate_paint_update(
    _: ProjectRuntimeOpaqueHandle,
    _: ProjectRuntimeOpaqueHandle,
    context: *const ProjectRuntimeCallContext,
    _: ProjectRuntimeByteSlice,
    output: *mut ProjectRuntimeByteBuffer,
) -> ProjectRuntimeAbiStatus {
    ffi_boundary(output, || {
        let context = unsafe { &*context };
        let api = unsafe { &*context.host_api };
        let access = HostUiAccess::new_with_invalidate(
            api.ui_query,
            api.ui_stage,
            api.ui_subscribe,
            api.ui_cancel,
            api.ui_structure,
            api.ui_invalidate,
            context.host_context,
        );
        let mut result = SdkSessionOutput::no_op();
        let mut ui = UiWriteBuffer::new(&access);
        let node = ui
            .node("custom", "circle")
            .map_err(|_| ProjectRuntimeAbiStatus::FAILED)?;
        ui.set_custom_property(
            node,
            "progress",
            project_runtime_sdk::aui_extension::UiExtensionValue::Float(
                if context.frame_index == 2 { 0.8 } else { 0.75 },
            ),
        )
        .map_err(|_| ProjectRuntimeAbiStatus::FAILED)?;
        ui.flush().map_err(|_| ProjectRuntimeAbiStatus::FAILED)?;
        result.status = ProjectRuntimeStatus::Applied;
        Ok(result)
    })
}

unsafe extern "C" fn update(
    _: ProjectRuntimeOpaqueHandle,
    _: ProjectRuntimeOpaqueHandle,
    context: *const ProjectRuntimeCallContext,
    _: ProjectRuntimeByteSlice,
    output: *mut ProjectRuntimeByteBuffer,
) -> ProjectRuntimeAbiStatus {
    ffi_boundary(output, || {
        // SAFETY: invoked by the native adapter with its live call-scoped context and table.
        let context = unsafe { &*context };
        let api = unsafe { &*context.host_api };
        let access = HostUiAccess::new(
            api.ui_query,
            api.ui_stage,
            api.ui_subscribe,
            api.ui_cancel,
            api.ui_structure,
            context.host_context,
        );
        let mut ui = UiWriteBuffer::new(&access);
        for id in ["a", "b"] {
            let text = ui.text(id, "label").unwrap();
            if id == "b" && context.frame_index == 12 {
                text.node()
                    .take_property_ownership(&mut ui, UiProperty::Text)
                    .unwrap();
            }
            if id == "b" && context.frame_index == 13 {
                text.node()
                    .release_property_to_binding(&mut ui, UiProperty::Text)
                    .unwrap();
                continue;
            }
            if context.frame_index == 11 {
                assert_eq!(text.text(&ui).unwrap().len(), 128 * 1024);
            }
            text.set_text(
                &mut ui,
                if context.frame_index == 10 {
                    "\0".repeat(128 * 1024)
                } else {
                    "骑兵".into()
                },
            )
            .unwrap();
            assert_eq!(
                text.text(&ui).unwrap().len(),
                if context.frame_index == 10 {
                    128 * 1024
                } else {
                    6
                }
            );
        }
        let batch = ui.finish().unwrap();
        let bytes = batch.encode().unwrap();
        let result = access.stage(batch);
        if context.frame_index == 7 {
            assert_eq!(result.unwrap_err().kind, UiErrorKind::BindingOwned);
        } else {
            result.unwrap();
        }
        if context.frame_index == 9 {
            assert!(access
                .stage(UiUpdateBatch::decode(&bytes).unwrap())
                .is_err());
        }
        if context.frame_index == 3 {
            return Err(ProjectRuntimeAbiStatus::FAILED);
        }
        if context.frame_index == 6 {
            return Err(ProjectRuntimeAbiStatus::BUFFER_TOO_SMALL);
        }
        if context.frame_index == 8 {
            panic!("project failed after UI staging");
        }
        let mut result = SdkSessionOutput::no_op();
        result.status = if context.frame_index == 2 {
            ProjectRuntimeStatus::NoOp
        } else {
            ProjectRuntimeStatus::Applied
        };
        if context.frame_index == 5 {
            result.rejected_action_count = 1;
        }
        if context.frame_index == 4 {
            result
                .mutations
                .push(ProjectRuntimeDeferredMutation::WriteTransform {
                    entity_id: "missing".into(),
                    transform: ProjectRuntimeTransform {
                        position: [0.; 3],
                        rotation: [0.; 3],
                        scale: [1.; 3],
                    },
                });
        }
        Ok(result)
    })
}

unsafe extern "C" fn subscription_update(
    _: ProjectRuntimeOpaqueHandle,
    _: ProjectRuntimeOpaqueHandle,
    context: *const ProjectRuntimeCallContext,
    _: ProjectRuntimeByteSlice,
    output: *mut ProjectRuntimeByteBuffer,
) -> ProjectRuntimeAbiStatus {
    ffi_boundary(output, || {
        // SAFETY: invoked by the native adapter with its live call-scoped context and table.
        let context = unsafe { &*context };
        let api = unsafe { &*context.host_api };
        let access = HostUiAccess::new(
            api.ui_query,
            api.ui_stage,
            api.ui_subscribe,
            api.ui_cancel,
            api.ui_structure,
            context.host_context,
        );
        let mut ui = UiWriteBuffer::new(&access);
        let node = ui.node("a", "label").unwrap();
        if context.frame_index == 1 || context.frame_index == 6 {
            let token = ui
                .subscribe(
                    node,
                    UiEventFilter::All,
                    UiSubscriptionScope::Instance,
                    UiCallbackId::new(77).unwrap(),
                )
                .unwrap();
            UI_TOKEN_INSTANCE.store(token.instance, Ordering::SeqCst);
            UI_TOKEN_ID.store(token.id, Ordering::SeqCst);
        }
        if context.frame_index == 3 {
            assert!(ui
                .cancel(UiSubscriptionToken {
                    instance: UI_TOKEN_INSTANCE.load(Ordering::SeqCst),
                    id: UI_TOKEN_ID.load(Ordering::SeqCst),
                })
                .unwrap());
        }
        if context.frame_index == 4 {
            ui.text("a", "label")
                .unwrap()
                .set_text(&mut ui, "普通通知")
                .unwrap();
        }
        let batch = ui.finish().unwrap();
        access.stage(batch).unwrap();
        let mut result = SdkSessionOutput::no_op();
        result.status = if context.frame_index == 6 {
            ProjectRuntimeStatus::NoOp
        } else {
            ProjectRuntimeStatus::Applied
        };
        Ok(result)
    })
}

unsafe extern "C" fn ui_event(
    _: ProjectRuntimeOpaqueHandle,
    _: ProjectRuntimeOpaqueHandle,
    context: *const ProjectRuntimeCallContext,
    request: ProjectRuntimeByteSlice,
    output: *mut ProjectRuntimeByteBuffer,
) -> ProjectRuntimeAbiStatus {
    ffi_boundary(output, || {
        // SAFETY: the adapter owns both the bounded request and call context for this callback.
        let request: UiEventRequest = unsafe { read_input(request) }?;
        assert_eq!(request.callback_id, UiCallbackId(77));
        assert!(matches!(
            request.notification.kind,
            UiNotificationKind::Mount | UiNotificationKind::Enable
        ));
        UI_EVENT_CALLS.fetch_add(1, Ordering::SeqCst);
        let context = unsafe { &*context };
        let api = unsafe { &*context.host_api };
        let access = HostUiAccess::new(
            api.ui_query,
            api.ui_stage,
            api.ui_subscribe,
            api.ui_cancel,
            api.ui_structure,
            context.host_context,
        );
        let mut ui = UiWriteBuffer::new(&access);
        ui.text("a", "label")
            .unwrap()
            .set_text_without_notify(&mut ui, "无通知回填")
            .unwrap();
        access.stage(ui.finish().unwrap()).unwrap();
        let mut result = SdkSessionOutput::no_op();
        result.status = ProjectRuntimeStatus::Applied;
        Ok(result)
    })
}

fn session(binding: bool) -> LoadedProjectRuntimeSession {
    let mut api = super::tests::fake_api();
    api.fixed_update = Some(update);
    api.destroy_session = None;
    LoadedProjectRuntimeSession {
        lease: Arc::new(NativeProjectRuntimeSessionLease {
            api: Arc::new(api),
            _lifetime_guard: None,
            handle: ProjectRuntimeOpaqueHandle::NULL,
        }),
        session_id: "ui-test".into(),
        terminal_fault: false,
        call_buffer: vec![0; PROJECT_RUNTIME_DEFAULT_STATEFUL_OUTPUT_CAPACITY_BYTES],
        spawn_receipts: BTreeMap::new(),
        animator_snapshots: Default::default(),
        ui: AuiSession::mount(&documents(binding)).unwrap(),
        ui_extension_types: BTreeMap::new(),
        ui_extension_stats: Default::default(),
    }
}

fn event_session() -> LoadedProjectRuntimeSession {
    let mut api = super::tests::fake_api();
    api.fixed_update = Some(subscription_update);
    api.capabilities |= project_runtime_abi::PROJECT_RUNTIME_CAP_UI_EVENTS;
    api.ui_event = Some(ui_event);
    api.destroy_session = None;
    LoadedProjectRuntimeSession {
        lease: Arc::new(NativeProjectRuntimeSessionLease {
            api: Arc::new(api),
            _lifetime_guard: None,
            handle: ProjectRuntimeOpaqueHandle::NULL,
        }),
        session_id: "ui-events".into(),
        terminal_fault: false,
        call_buffer: vec![0; PROJECT_RUNTIME_DEFAULT_STATEFUL_OUTPUT_CAPACITY_BYTES],
        spawn_receipts: BTreeMap::new(),
        animator_snapshots: Default::default(),
        ui: AuiSession::mount(&documents(false)).unwrap(),
        ui_extension_types: BTreeMap::new(),
        ui_extension_stats: Default::default(),
    }
}

fn extension_session() -> LoadedProjectRuntimeSession {
    let mut api = super::tests::fake_api();
    api.capabilities |= project_runtime_abi::PROJECT_RUNTIME_CAP_UI_EXTENSIONS;
    api.ui_extension = Some(ui_extension);
    api.fixed_update = Some(invalidate_paint_update);
    api.destroy_session = None;
    LoadedProjectRuntimeSession {
        lease: Arc::new(NativeProjectRuntimeSessionLease {
            api: Arc::new(api),
            _lifetime_guard: None,
            handle: ProjectRuntimeOpaqueHandle::NULL,
        }),
        session_id: "ui-extension".into(),
        terminal_fault: false,
        call_buffer: vec![0; project_runtime_sdk::aui_extension::MAX_UI_EXTENSION_WIRE_BYTES],
        spawn_receipts: BTreeMap::new(),
        animator_snapshots: Default::default(),
        ui: AuiSession::default(),
        ui_extension_types: BTreeMap::new(),
        ui_extension_stats: Default::default(),
    }
}

fn tick(
    session: &mut dyn ProjectRuntimeSession,
    mode: u64,
) -> crate::project_runtime_session::ProjectRuntimeSessionStageReport {
    execute_project_runtime_session_stage_with_media(
        session,
        ProjectRuntimeSessionStage::FixedUpdate,
        mode,
        TimeContext::from_delta(mode, 1. / 60., true),
        &mut World::new(),
        &[],
        ProjectRuntimeSessionReportLevel::Off,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut Vec::new(),
        &mut None,
        None,
    )
}

fn text(session: &LoadedProjectRuntimeSession, document: &str) -> String {
    let handle = session.ui.resolve(document, "label").unwrap();
    UiTextRef::try_new(&session.ui, handle)
        .unwrap()
        .text(&session.ui)
        .unwrap()
}

#[test]
fn project_runtime_ui_commits_two_documents_only_after_host_acceptance() {
    let mut session = session(false);
    let world = World::new();
    let output = session.fixed_update(ProjectRuntimeSessionContext {
        frame_index: 1,
        time: TimeContext::from_delta(1, 1. / 60., true),
        world: WorldReadApi::new(&world),
    });
    assert_eq!(output.status, ProjectRuntimeSessionStatus::Applied);
    assert_eq!(text(&session, "a"), "人");
    session.finish_ui_updates(false);
    assert_eq!(text(&session, "b"), "人");
    let report = tick(&mut session, 1);
    assert!(!report.terminal_fault, "{report:?}");
    assert_eq!(text(&session, "a"), "骑兵");
    assert_eq!(text(&session, "b"), "骑兵");
}

#[test]
fn project_runtime_ui_stage_transfers_binding_ownership_without_a_new_host_slot() {
    let mut session = session(true);
    assert!(!tick(&mut session, 12).terminal_fault);
    assert_eq!(text(&session, "b"), "骑兵");

    assert!(!tick(&mut session, 1).terminal_fault);
    assert_eq!(text(&session, "b"), "骑兵");

    assert!(!tick(&mut session, 13).terminal_fault);
    assert!(tick(&mut session, 7).terminal_fault);
    assert_eq!(text(&session, "b"), "骑兵");
}

#[test]
fn project_runtime_ui_extension_mount_is_batched_and_repaint_does_not_remeasure() {
    let _guard = UI_EVENT_TEST_LOCK.lock().unwrap();
    UI_EXTENSION_DESCRIBE_CALLS.store(0, Ordering::SeqCst);
    UI_EXTENSION_MEASURE_CALLS.store(0, Ordering::SeqCst);
    UI_EXTENSION_LAYOUT_CALLS.store(0, Ordering::SeqCst);
    UI_EXTENSION_PAINT_CALLS.store(0, Ordering::SeqCst);
    UI_EXTENSION_INVALID_PAINT.store(false, Ordering::SeqCst);
    let mut session = extension_session();
    session.mount_ui_documents(&custom_documents()).unwrap();
    assert_eq!(UI_EXTENSION_DESCRIBE_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(UI_EXTENSION_MEASURE_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(UI_EXTENSION_LAYOUT_CALLS.load(Ordering::SeqCst), 0);
    assert_eq!(UI_EXTENSION_PAINT_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(session.ui_extension_stats.calls, 3);
    assert_eq!(session.ui_extension_stats.measure_nodes, 1);
    assert_eq!(session.ui_extension_stats.paint_nodes, 1);
    assert!(session.ui_extension_stats.request_bytes > 0);
    assert!(session.ui_extension_stats.response_bytes > 0);
    let output = session
        .ui
        .document("custom")
        .unwrap()
        .custom_runtime_outputs()
        .next()
        .unwrap()
        .1;
    assert_eq!(output.paint.as_ref().unwrap().indices, vec![0, 1, 2]);

    session.finish_ui_updates(true);
    assert_eq!(UI_EXTENSION_MEASURE_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(UI_EXTENSION_PAINT_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(
        session.ui_extension_stats.calls, 3,
        "clean UI completion must not cross the DLL"
    );
    let report = tick(&mut session, 1);
    assert!(!report.terminal_fault, "{report:?}");
    assert_eq!(UI_EXTENSION_MEASURE_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(UI_EXTENSION_PAINT_CALLS.load(Ordering::SeqCst), 2);
    assert_eq!(session.ui_extension_stats.calls, 4);

    UI_EXTENSION_INVALID_PAINT.store(true, Ordering::SeqCst);
    assert!(!tick(&mut session, 2).terminal_fault);
    let output = session
        .ui
        .document("custom")
        .unwrap()
        .custom_runtime_outputs()
        .next()
        .unwrap()
        .1;
    assert!(
        output.paint.is_none(),
        "last-good paint must not survive a property identity change"
    );
    UI_EXTENSION_INVALID_PAINT.store(false, Ordering::SeqCst);
}

#[test]
fn project_runtime_ui_extension_rejects_missing_or_incompatible_type_at_mount() {
    let _guard = UI_EVENT_TEST_LOCK.lock().unwrap();
    let mut documents = custom_documents();
    documents.documents_by_id.get_mut("custom").unwrap().nodes[0]
        .custom
        .as_mut()
        .unwrap()
        .type_id = "sample.missing".into();
    let mut session = extension_session();
    let error = session.mount_ui_documents(&documents).unwrap_err();
    assert!(error.message.contains("circle"));
    assert!(error.message.contains("sample.missing"));

    let mut documents = custom_documents();
    documents.documents_by_id.get_mut("custom").unwrap().nodes[0]
        .custom
        .as_mut()
        .unwrap()
        .properties
        .insert(
            "progress".into(),
            project_runtime_sdk::aui_extension::UiExtensionValue::Integer(1),
        );
    let mut session = extension_session();
    let error = session.mount_ui_documents(&documents).unwrap_err();
    assert!(error.message.contains("circle"));
    assert!(error.message.contains("properties"));
}

#[test]
fn project_runtime_ui_extension_uses_presented_container_and_child_sizes() {
    let _guard = UI_EVENT_TEST_LOCK.lock().unwrap();
    UI_EXTENSION_LAYOUT_CALLS.store(0, Ordering::SeqCst);
    UI_EXTENSION_INVALID_PAINT.store(false, Ordering::SeqCst);
    let mut session = extension_session();
    session.mount_ui_documents(&custom_documents()).unwrap();
    assert_eq!(
        UI_EXTENSION_LAYOUT_CALLS.load(Ordering::SeqCst),
        0,
        "custom layout must wait for solver-provided child measurements"
    );
    assert!(
        session
            .ui
            .document("custom")
            .unwrap()
            .custom_runtime_outputs()
            .next()
            .unwrap()
            .1
            .child_rects
            .is_empty(),
        "mount must not publish child rects computed from fabricated zero measurements"
    );
    let frame = crate::aui_extension::AuiCustomFrame {
        document_id: "custom".into(),
        resource_identity: "package-a".into(),
        nodes: vec![crate::aui_extension::AuiCustomFrameNode {
            node_id: "circle".into(),
            constraints: Some(UiExtensionConstraints {
                known_width: None,
                known_height: None,
                available_width: Some(80.0),
                available_height: Some(70.0),
            }),
            size: [80.0, 70.0],
            children: vec![("child".into(), [12.0, 13.0])],
        }],
    };
    let first = session
        .refresh_ui_extensions_for_presented_frame(&frame, true)
        .unwrap();
    assert!(first.changed);
    let second = session
        .refresh_ui_extensions_for_presented_frame(&frame, true)
        .unwrap();
    assert!(!second.changed);
    assert_eq!(UI_EXTENSION_LAYOUT_CALLS.load(Ordering::SeqCst), 1);
    let runtime = session
        .ui
        .document("custom")
        .unwrap()
        .custom_runtime_outputs()
        .next()
        .unwrap()
        .1;
    assert_eq!(runtime.child_rects[0].rect, [68.0, 57.0, 12.0, 13.0]);
    assert_eq!(
        runtime
            .layout_identity
            .as_ref()
            .map(|identity| &identity.input),
        Some(&UiExtensionInput::Layout {
            size: [80.0, 70.0],
            children: vec![UiExtensionChildInput {
                handle: runtime.child_rects[0].handle,
                measured_size: [12.0, 13.0],
            }],
        })
    );
    let retained_rect = runtime.child_rects[0].rect;

    let mut changed_frame = frame;
    changed_frame.nodes[0].size = [90.0, 90.0];
    let limited = session
        .refresh_ui_extensions_for_presented_frame(&changed_frame, false)
        .unwrap();
    assert!(limited.stabilization_limit_exceeded);
    let after_limit = session
        .ui
        .document("custom")
        .unwrap()
        .custom_runtime_outputs()
        .next()
        .unwrap()
        .1;
    assert_eq!(after_limit.child_rects[0].rect, retained_rect);
}

#[test]
fn project_runtime_ui_extension_identity_refresh_is_revision_neutral_and_stale_resources_clear() {
    let _guard = UI_EVENT_TEST_LOCK.lock().unwrap();
    UI_EXTENSION_MEASURE_CALLS.store(0, Ordering::SeqCst);
    UI_EXTENSION_LAYOUT_CALLS.store(0, Ordering::SeqCst);
    UI_EXTENSION_PAINT_CALLS.store(0, Ordering::SeqCst);
    UI_EXTENSION_INVALID_PAINT.store(false, Ordering::SeqCst);
    let mut documents = custom_documents();
    let document = documents.documents_by_id.get_mut("custom").unwrap();
    document.nodes[0].children.clear();
    document.nodes.truncate(1);
    let mut session = extension_session();
    session.mount_ui_documents(&documents).unwrap();
    let initial_key = session.ui.document("custom").unwrap().content_key();
    let frame = crate::aui_extension::AuiCustomFrame {
        document_id: "custom".into(),
        resource_identity: "package-a".into(),
        nodes: vec![crate::aui_extension::AuiCustomFrameNode {
            node_id: "circle".into(),
            constraints: Some(UiExtensionConstraints {
                known_width: Some(50.0),
                known_height: Some(50.0),
                available_width: None,
                available_height: None,
            }),
            size: [50.0, 50.0],
            children: vec![],
        }],
    };

    let refreshed = session
        .refresh_ui_extensions_for_presented_frame(&frame, true)
        .unwrap();
    assert!(
        !refreshed.changed,
        "equal payloads only refresh their identities"
    );
    assert_eq!(
        session.ui.document("custom").unwrap().content_key(),
        initial_key
    );
    let calls_after_identity_refresh = (
        UI_EXTENSION_MEASURE_CALLS.load(Ordering::SeqCst),
        UI_EXTENSION_LAYOUT_CALLS.load(Ordering::SeqCst),
        UI_EXTENSION_PAINT_CALLS.load(Ordering::SeqCst),
    );
    let clean = session
        .refresh_ui_extensions_for_presented_frame(&frame, true)
        .unwrap();
    assert!(!clean.changed);
    assert_eq!(
        (
            UI_EXTENSION_MEASURE_CALLS.load(Ordering::SeqCst),
            UI_EXTENSION_LAYOUT_CALLS.load(Ordering::SeqCst),
            UI_EXTENSION_PAINT_CALLS.load(Ordering::SeqCst),
        ),
        calls_after_identity_refresh,
        "an identical presented frame must perform zero extension callbacks"
    );

    UI_EXTENSION_INVALID_PAINT.store(true, Ordering::SeqCst);
    let mut changed_resource = frame;
    changed_resource.resource_identity = "package-b".into();
    assert!(session
        .refresh_ui_extensions_for_presented_frame(&changed_resource, true)
        .is_err());
    let runtime = session
        .ui
        .document("custom")
        .unwrap()
        .custom_runtime_outputs()
        .next()
        .unwrap()
        .1;
    assert!(
        runtime.paint.is_none(),
        "paint from a different resource identity must not survive a failed refresh"
    );
    UI_EXTENSION_INVALID_PAINT.store(false, Ordering::SeqCst);
}

#[test]
fn project_runtime_ui_failures_never_publish_staged_properties() {
    for mode in 2..=9 {
        let mut session = session(mode == 7);
        let report = tick(&mut session, mode);
        assert_eq!(text(&session, "a"), "人", "mode {mode}: {report:?}");
        assert_eq!(text(&session, "b"), "人", "mode {mode}: {report:?}");
        assert!(session.ui.pending_is_empty_for_test());
    }
}

#[test]
fn project_runtime_ui_large_batch_uses_separate_channel_and_sessions_are_isolated() {
    let mut first = session(false);
    let second = session(false);
    let old = first.ui.resolve("a", "label").unwrap();
    assert!(second.ui.node_type(old).is_err());
    assert!(!tick(&mut first, 10).terminal_fault);
    assert_eq!(text(&first, "a").len(), 128 * 1024);
    assert_eq!(text(&first, "b").len(), 128 * 1024);
    assert_eq!(text(&second, "a"), "人");
    assert!(!tick(&mut first, 11).terminal_fault);
    assert_eq!(text(&first, "a"), "骑兵");
    first.mount_ui_documents(&documents(false)).unwrap();
    assert!(first.ui.node_type(old).is_err());
}

#[test]
fn project_runtime_ui_stage_sizes_before_mutating_and_context_expires() {
    let world = World::new();
    let mut ui = AuiSession::mount(&documents(false)).unwrap();
    let mut host = ReadOnlyWorldHost {
        world: WorldReadApi::new(&world),
        spawn_receipts: None,
        animator_snapshots: None,
        ui: Some(&mut ui),
    };
    let token = with_host_context(&mut host, |token| {
        let access = HostUiAccess::new(
            HOST_API.ui_query,
            HOST_API.ui_stage,
            HOST_API.ui_subscribe,
            HOST_API.ui_cancel,
            HOST_API.ui_structure,
            token,
        );
        let mut buffer = UiWriteBuffer::new(&access);
        buffer
            .text("a", "label")
            .unwrap()
            .set_text(&mut buffer, "骑")
            .unwrap();
        buffer
            .node("a", "label")
            .unwrap()
            .set_active(&mut buffer, false)
            .unwrap();
        let bytes = buffer.finish().unwrap().encode().unwrap();
        let mut storage = [0; 1];
        let mut output = ProjectRuntimeByteBuffer::from_slice(&mut storage);
        // SAFETY: all pointers refer to live, correctly sized caller-owned storage.
        let result = unsafe {
            host_ui_stage(
                token,
                ProjectRuntimeByteSlice::from_slice(&bytes),
                &mut output,
            )
        };
        assert_eq!(result, ProjectRuntimeAbiStatus::BUFFER_TOO_SMALL);
        assert_eq!(output.written, UI_STAGE_REPLY_BYTES as u64);
        access
            .stage(UiUpdateBatch::decode(&bytes).unwrap())
            .unwrap(); // first actual stage
        token
    });
    ui.finish(true);
    let label = ui.resolve("a", "label").unwrap();
    assert_eq!(
        ui.read(label, UiProperty::Active).unwrap(),
        UiValue::Bool(false)
    );
    let access = HostUiAccess::new(
        HOST_API.ui_query,
        HOST_API.ui_stage,
        HOST_API.ui_subscribe,
        HOST_API.ui_cancel,
        HOST_API.ui_structure,
        token,
    );
    assert!(access.resolve("a", "label").is_err());
    let mut host = ReadOnlyWorldHost {
        world: WorldReadApi::new(&world),
        spawn_receipts: None,
        animator_snapshots: None,
        ui: None,
    };
    with_host_context(&mut host, |token| {
        let access = HostUiAccess::new(
            HOST_API.ui_query,
            HOST_API.ui_stage,
            HOST_API.ui_subscribe,
            HOST_API.ui_cancel,
            HOST_API.ui_structure,
            token,
        );
        assert_eq!(
            access.resolve("a", "label").unwrap_err().kind,
            UiErrorKind::Unavailable
        );
    });
}

#[test]
fn aui_animation_query_sizes_before_reserving_and_expires_with_host_context() {
    use project_runtime_sdk::aui_animation::*;
    let world = World::new();
    let mut ui = AuiSession::mount(&documents(false)).unwrap();
    let target = ui.resolve("a", "label").unwrap();
    ui.begin();
    let mut host = ReadOnlyWorldHost {
        world: WorldReadApi::new(&world),
        spawn_receipts: None,
        animator_snapshots: None,
        ui: Some(&mut ui),
    };
    let (token, handle) = with_host_context(&mut host, |token| {
        let access = HostUiAccess::new(
            HOST_API.ui_query,
            HOST_API.ui_stage,
            HOST_API.ui_subscribe,
            HOST_API.ui_cancel,
            HOST_API.ui_structure,
            token,
        );
        let request = AnimationRequest::Play {
            request: AnimationPlay {
                owner: target,
                scope: UiSubscriptionScope::Active,
                clock: AnimationClock::Ui,
                conflict: TrackConflictPolicy::Reject,
                callback_id: None,
                animation: Animation2D {
                    version: ANIMATION_2D_VERSION,
                    root: AnimationStep::Delay { duration_us: 100 },
                    repeat: Repeat::Once,
                    direction: PlaybackDirection::Forward,
                    completion: CompletionPolicy::Hold,
                },
            },
        };
        let bytes = serde_json::to_vec(&UiQuery::Animation {
            request: request.clone(),
        })
        .unwrap();
        let mut storage = [0; 1];
        let mut output = ProjectRuntimeByteBuffer::from_slice(&mut storage);
        // SAFETY: live bounded request and caller-owned output storage.
        assert_eq!(
            unsafe {
                host_ui_query(
                    token,
                    ProjectRuntimeByteSlice::from_slice(&bytes),
                    &mut output,
                )
            },
            ProjectRuntimeAbiStatus::BUFFER_TOO_SMALL
        );
        assert_eq!(output.written, UI_STAGE_REPLY_BYTES as u64);
        let AnimationReply::Handle(handle) = access.animation_request(request).unwrap() else {
            panic!("play must return a handle")
        };
        assert_eq!(
            handle.slot, 0,
            "short response cannot reserve the first playback slot"
        );
        (token, handle)
    });
    ui.finish(true);
    let access = HostUiAccess::new(
        HOST_API.ui_query,
        HOST_API.ui_stage,
        HOST_API.ui_subscribe,
        HOST_API.ui_cancel,
        HOST_API.ui_structure,
        token,
    );
    assert!(access
        .animation_request(AnimationRequest::Cancel { handle })
        .is_err());
    ui.advance_animations(1, 100).unwrap();
}

#[test]
fn project_runtime_ui_old_abi_minor_is_rejected() {
    let mut api = super::tests::fake_api();
    api.abi_minor -= 1;
    assert!(LoadedProjectRuntimeModuleAdapter::new(api).is_err());
}

#[test]
fn project_runtime_structure_host_call_returns_draft_refs_and_commits_once() {
    let world = World::new();
    let mut ui = AuiSession::mount(&documents(false)).unwrap();
    let parent = ui.resolve("a", "root").unwrap();
    ui.begin();
    let instance = {
        let mut host = ReadOnlyWorldHost {
            world: WorldReadApi::new(&world),
            spawn_receipts: None,
            animator_snapshots: None,
            ui: Some(&mut ui),
        };
        with_host_context(&mut host, |token| {
            let access = HostUiAccess::new(
                HOST_API.ui_query,
                HOST_API.ui_stage,
                HOST_API.ui_subscribe,
                HOST_API.ui_cancel,
                HOST_API.ui_structure,
                token,
            );
            let mut context = UiWriteBuffer::new(&access);
            UiNodeRef::new(parent)
                .set_translation(&mut context, [1.0, 2.0])
                .unwrap();
            let instance = context
                .instantiate("b", UiNodeRef::new(parent), Some(0))
                .unwrap();
            let label = instance.node("label").unwrap().handle();
            let text = UiTextRef::try_new(&context, label).unwrap();
            text.set_text(&mut context, "骑兵").unwrap();
            access.stage(context.finish().unwrap()).unwrap();
            instance
        })
    };
    ui.finish(true);
    assert_eq!(
        ui.node_type(instance.node("label").unwrap().handle())
            .unwrap(),
        UiNodeType::Text
    );
    assert_eq!(
        ui.read(instance.node("label").unwrap().handle(), UiProperty::Text)
            .unwrap(),
        UiValue::Text("骑兵".into())
    );
}

#[test]
fn project_runtime_ui_callbacks_route_by_id_and_cancel_before_future_changed() {
    let _guard = UI_EVENT_TEST_LOCK.lock().unwrap();
    UI_EVENT_CALLS.store(0, Ordering::SeqCst);
    let mut session = event_session();
    assert!(!tick(&mut session, 1).terminal_fault);
    assert_eq!(session.ui.subscription_count_for_test(), 1);
    assert_eq!(UI_EVENT_CALLS.load(Ordering::SeqCst), 0);

    assert!(!tick(&mut session, 2).terminal_fault);
    assert_eq!(UI_EVENT_CALLS.load(Ordering::SeqCst), 2);
    assert_eq!(text(&session, "a"), "无通知回填");

    assert!(!tick(&mut session, 3).terminal_fault);
    assert_eq!(session.ui.subscription_count_for_test(), 0);
    assert!(!tick(&mut session, 4).terminal_fault);
    assert!(!tick(&mut session, 5).terminal_fault);
    assert_eq!(UI_EVENT_CALLS.load(Ordering::SeqCst), 2);
    assert_eq!(text(&session, "a"), "普通通知");
    session.ui.shutdown();
    assert_eq!(session.ui.subscription_count_for_test(), 0);
}

#[test]
fn project_runtime_ui_event_output_survives_absent_optional_stage_callback() {
    let _guard = UI_EVENT_TEST_LOCK.lock().unwrap();
    UI_EVENT_CALLS.store(0, Ordering::SeqCst);
    let mut session = event_session();
    assert!(!tick(&mut session, 1).terminal_fault);

    let world = World::new();
    let output = session.invoke_session(
        None,
        ProjectRuntimeSessionContext {
            frame_index: 2,
            time: TimeContext::from_delta(2, 1. / 60., true),
            world: WorldReadApi::new(&world),
        },
        &(),
    );

    assert_eq!(output.status, ProjectRuntimeSessionStatus::Applied);
    assert_eq!(UI_EVENT_CALLS.load(Ordering::SeqCst), 2);
    assert_eq!(text(&session, "a"), "无通知回填");
}

#[test]
fn rejected_project_callback_does_not_publish_subscription() {
    let _guard = UI_EVENT_TEST_LOCK.lock().unwrap();
    let mut session = event_session();
    assert!(!tick(&mut session, 6).terminal_fault);
    assert_eq!(session.ui.subscription_count_for_test(), 0);
}
