use engine_runtime::aui::*;
use engine_runtime::aui_instance::*;
use engine_runtime::aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION;
use project_runtime_sdk::aui::*;
use project_runtime_sdk::aui_wire::*;
use std::sync::Arc;

#[test]
fn scrolling_nonclipping_parent_keeps_onscreen_descendants_visible() {
    for clips_world in [false, true] {
        let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
        root.clip_policy = AuiClipPolicy::Rect;
        root.children.push("world".into());
        let mut world = AuiNode::new(
            "world",
            AuiNodeKind::Panel,
            AuiRect::fixed_position(0., 0., 960., 540.),
        );
        world.parent = Some("root".into());
        world.children.push("chunk".into());
        if clips_world {
            world.clip_policy = AuiClipPolicy::Rect;
        }
        let mut chunk = AuiNode::new(
            "chunk",
            AuiNodeKind::Panel,
            AuiRect::fixed_position(1024., 0., 512., 540.),
        );
        chunk.parent = Some("world".into());
        chunk.clip_policy = AuiClipPolicy::Rect;
        chunk.children.push("player".into());
        let mut player = AuiNode::new(
            "player",
            AuiNodeKind::Panel,
            AuiRect::fixed_position(40., 400., 24., 44.),
        );
        player.parent = Some("chunk".into());
        player.style = Some(AuiStyle::color("#ff0000"));
        let mut document = AuiDocument::new(
            "scrolling-world",
            vec![AuiCanvas::screen_overlay("main", 960., 540., "root")],
            vec![root, world, chunk, player],
        );
        document.schema_version = AUI_LAYOUT_SCHEMA_VERSION.into();
        let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
        let world = UiNodeRef::new(instance.node("world").unwrap());
        let mut runtime = AuiC2Runtime::default();
        // Cross the parent's entire width, return, and cross again with retained geometry.
        for (frame, offset, active) in [
            (1, -900., true),
            (2, -1024., true),
            (3, -900., true),
            (4, -1024., false),
            (5, -1024., true),
        ] {
            let mut edit = instance.edit();
            world.set_translation(&mut edit, [offset, 0.]).unwrap();
            world.set_active_without_notify(&mut edit, active).unwrap();
            edit.commit().unwrap();
            runtime.observe_retained(Some(&instance));
            let out =
                AuiRuntimePresenter::present_project_snapshot_with_fonts_for_presentation_cached(
                    &document,
                    ProjectUiStateSnapshotOutput::new(
                        "test",
                        AuiSnapshotSource::TestSnapshot,
                        ProjectUiStateSnapshot::new(frame),
                    ),
                    &engine_runtime::runtime_package::RuntimeAuiFontAtlasRegistry::empty("test"),
                    &engine_runtime::font_bundle::RuntimeFontBundleRegistry::default(),
                    None,
                    &mut runtime,
                );
            assert_eq!(out.report.status, AuiRuntimePresentStatus::Success);
            let expected = active && !clips_world;
            let visible = out
                .overlay
                .draw_items
                .iter()
                .any(|item| item.node_id == "player");
            assert_eq!(
                visible, expected,
                "offset={offset}, clips_world={clips_world}, active={active}"
            );
        }
    }
}

fn source(count: usize) -> Arc<AuiDocument> {
    let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
    let mut nodes = Vec::new();
    for i in 0..count {
        let id = format!("text-{i}");
        root.children.push(id.clone());
        let mut node = AuiNode::new(
            id,
            AuiNodeKind::Text,
            AuiRect::fixed_position(0., 0., 20., 20.),
        );
        node.parent = Some("root".into());
        node.text = Some("人".into());
        node.interactable = true;
        nodes.push(node);
    }
    nodes.insert(0, root);
    let mut document = AuiDocument::new(
        "retained-test",
        vec![AuiCanvas::screen_overlay("main", 800., 600., "root")],
        nodes,
    );
    document.schema_version = AUI_LAYOUT_SCHEMA_VERSION.into();
    Arc::new(document)
}

#[test]
fn progress_foreground_local_style_updates_fill_without_layout_invalidation() {
    let mut document = (*source(1)).clone();
    document.nodes[1].kind = AuiNodeKind::ProgressBar;
    document.nodes[1].text = None;
    document.nodes[1].progress_value = Some(0.5);
    document.nodes[1].style = Some(AuiStyle::color("#303030"));
    let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
    instance.drain_invalidations(&mut Vec::new());
    let node = UiNodeRef::new(instance.node("text-0").unwrap());
    let mut runtime = AuiC2Runtime::default();
    for (frame, foreground, expected) in [
        (1, [1., 0., 0., 1.], "#ff0000ff"),
        (2, [0., 1., 0., 0.5], "#00ff0080"),
    ] {
        let mut writes = UiWriteBuffer::new(&instance);
        node.set_local_style(
            &mut writes,
            UiLocalStyle {
                foreground: Some(foreground),
                ..Default::default()
            },
        )
        .unwrap();
        instance
            .apply_update_batch(&writes.finish().unwrap())
            .unwrap();
        let mut dirty = Vec::new();
        instance.drain_invalidations(&mut dirty);
        assert_eq!(dirty.len(), 1);
        assert!(dirty[0].invalidation.paint);
        assert!(!dirty[0].invalidation.measure && !dirty[0].invalidation.layout);
        runtime.observe_retained(Some(&instance));
        let out = AuiRuntimePresenter::present_project_snapshot_with_fonts_for_presentation_cached(
            &document,
            ProjectUiStateSnapshotOutput::new(
                "test",
                AuiSnapshotSource::TestSnapshot,
                ProjectUiStateSnapshot::new(frame),
            ),
            &engine_runtime::runtime_package::RuntimeAuiFontAtlasRegistry::empty("test"),
            &engine_runtime::font_bundle::RuntimeFontBundleRegistry::default(),
            None,
            &mut runtime,
        );
        assert_eq!(out.report.status, AuiRuntimePresentStatus::Success);
        let item = |id: &str| {
            out.overlay
                .draw_items
                .iter()
                .find(|item| item.node_id == id)
                .unwrap()
        };
        assert_eq!(item("text-0:fill").color.as_deref(), Some(expected));
        assert_eq!(item("text-0:track").color.as_deref(), Some("#303030"));
        assert_eq!(item("text-0:fill").rect.width, 10.);
    }
}

#[test]
fn presentation_properties_are_atomic_same_value_and_do_not_invalidate_layout() {
    let mut instance = mounted(2);
    let handle = instance.node("text-0").unwrap();
    let node = UiNodeRef::new(handle);
    let mut buffer = UiWriteBuffer::new(&instance);
    node.set_rotation(&mut buffer, 0.3).unwrap();
    node.set_scale(&mut buffer, [-2., 1.5]).unwrap();
    node.set_pivot(&mut buffer, [-0.5, 1.2]).unwrap();
    node.set_opacity(&mut buffer, 0.).unwrap();
    node.set_tint(&mut buffer, [0.5, 1., 0.2, 0.8]).unwrap();
    let encoded = buffer.finish().unwrap().encode().unwrap();
    let batch = UiUpdateBatch::decode(&encoded).unwrap();
    assert_eq!(instance.apply_update_batch(&batch).unwrap(), 5);
    let mut dirty = Vec::new();
    instance.drain_invalidations(&mut dirty);
    assert_eq!(dirty.len(), 1);
    assert!(dirty[0].invalidation.transform && dirty[0].invalidation.hit);
    assert!(!dirty[0].invalidation.measure && !dirty[0].invalidation.layout);
    assert_eq!(instance.apply_update_batch(&batch).unwrap(), 0);
    instance.drain_invalidations(&mut dirty);
    assert!(dirty.is_empty());
    let mut edit = instance.edit();
    node.set_rotation(&mut edit, 0.5).unwrap();
    assert_eq!(
        node.set_scale(&mut edit, [f32::NAN, 1.]).unwrap_err().kind,
        UiErrorKind::InvalidValue
    );
    assert!(edit.commit().is_err());
    assert_eq!(
        instance.read(handle, UiProperty::Rotation).unwrap(),
        UiValue::Number(0.3)
    );
    let mut buffer = UiWriteBuffer::new(&instance);
    node.set_translation(&mut buffer, [1., 2.]).unwrap();
    assert_eq!(
        node.set_opacity(&mut buffer, 1.01).unwrap_err().kind,
        UiErrorKind::InvalidValue
    );
    assert!(buffer.finish().is_err());
    assert_eq!(
        instance.read(handle, UiProperty::Translation).unwrap(),
        UiValue::Translation([0.; 2])
    );
}

#[test]
fn presentation_controller_override_preserves_other_bindings_and_release_restores_binding() {
    let mut document = (*source(2)).clone();
    for node in &mut document.nodes[1..] {
        node.kind = AuiNodeKind::Panel;
        node.text = None;
    }
    document.nodes[1].binding_refs.push(AuiBindingRef::new(
        "opacity",
        AuiBindingTarget::Opacity,
        "unit.opacity",
        None,
    ));
    let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
    let handle = instance.node("text-0").unwrap();
    let node = UiNodeRef::new(handle);
    let mut edit = instance.edit();
    assert_eq!(
        node.set_opacity(&mut edit, 0.2).unwrap_err().kind,
        UiErrorKind::BindingOwned
    );
    assert!(edit.commit().is_err());
    let mut runtime = AuiC2Runtime::default();
    let mut snapshot = ProjectUiStateSnapshot::new(1);
    snapshot
        .values
        .insert("unit.opacity".into(), AuiBindingValue::Number(0.6));
    let present = |frame: u64, runtime: &mut AuiC2Runtime| {
        let mut snapshot = snapshot.clone();
        snapshot.frame_index = frame;
        AuiRuntimePresenter::present_project_snapshot_with_fonts_for_presentation_cached(
            &document,
            ProjectUiStateSnapshotOutput::new("test", AuiSnapshotSource::TestSnapshot, snapshot),
            &engine_runtime::runtime_package::RuntimeAuiFontAtlasRegistry::empty("test"),
            &engine_runtime::font_bundle::RuntimeFontBundleRegistry::default(),
            None,
            runtime,
        )
    };
    let mut edit = instance.edit();
    node.set_scale(&mut edit, [2., 1.]).unwrap();
    edit.commit().unwrap();
    runtime.observe_retained(Some(&instance));
    let first = present(1, &mut runtime);
    let geometry = first
        .layout
        .computed_nodes
        .iter()
        .find(|n| n.node_id == "text-0")
        .unwrap()
        .presentation
        .unwrap();
    assert!(
        (geometry.modulation[3] - 0.6).abs() < 1e-6,
        "scale setter replaced opacity binding"
    );
    let mut edit = UiWriteBuffer::new(&instance);
    node.take_property_ownership(&mut edit, UiProperty::Opacity)
        .unwrap();
    node.set_opacity(&mut edit, 0.2).unwrap();
    let batch = edit.finish().unwrap();
    instance.apply_update_batch(&batch).unwrap();
    runtime.observe_retained(Some(&instance));
    let second = present(2, &mut runtime);
    assert!(
        (second.layout.computed_nodes[1]
            .presentation
            .unwrap()
            .modulation[3]
            - 0.2)
            .abs()
            < 1e-6
    );
    let mut edit = UiWriteBuffer::new(&instance);
    node.release_property_to_binding(&mut edit, UiProperty::Opacity)
        .unwrap();
    let batch = edit.finish().unwrap();
    instance.apply_update_batch(&batch).unwrap();
    runtime.observe_retained(Some(&instance));
    let third = present(3, &mut runtime);
    assert!(
        (third.layout.computed_nodes[1]
            .presentation
            .unwrap()
            .modulation[3]
            - 0.6)
            .abs()
            < 1e-6
    );
    assert_eq!(
        third.layout.computed_nodes[1].rect.width, 20.,
        "scale changed layout occupation"
    );
}

fn mounted(count: usize) -> AuiInstance {
    let mut instance = AuiInstance::mount(source(count)).unwrap();
    instance.drain_invalidations(&mut Vec::new());
    instance
}

#[test]
fn resource_and_local_style_use_mounted_identity_and_survive_theme_resolution() {
    let mut document = (*source(2)).clone();
    for node in &mut document.nodes[1..] {
        node.kind = AuiNodeKind::Image;
        node.text = None;
    }
    document.resource_identities = Arc::new(std::collections::BTreeMap::from([(
        "image".into(),
        ("guid-image".into(), "texture".into()),
    )]));
    let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
    let handle = instance.node("text-0").unwrap();
    let node = UiNodeRef::new(handle);
    let mut writes = UiWriteBuffer::new(&instance);
    node.set_resource(
        &mut writes,
        Some(UiResourceRef {
            asset_id: "image".into(),
            guid: "wrong".into(),
        }),
    )
    .unwrap();
    let batch = writes.finish().unwrap();
    assert_eq!(
        instance.apply_update_batch(&batch).unwrap_err().kind,
        UiErrorKind::InvalidValue
    );
    assert_eq!(
        instance.read(handle, UiProperty::Resource).unwrap(),
        UiValue::Resource(None)
    );
    let mut writes = UiWriteBuffer::new(&instance);
    node.set_resource(
        &mut writes,
        Some(UiResourceRef {
            asset_id: "image".into(),
            guid: "guid-image".into(),
        }),
    )
    .unwrap();
    node.set_local_style(
        &mut writes,
        UiLocalStyle {
            background: Some([1., 0., 0., 1.]),
            border: Some([0., 1., 0., 1.]),
            radius: Some(9.),
            ..Default::default()
        },
    )
    .unwrap();
    instance
        .apply_update_batch(&writes.finish().unwrap())
        .unwrap();
    let mut runtime = AuiC2Runtime::default();
    runtime.observe_retained(Some(&instance));
    let out = AuiRuntimePresenter::present_project_snapshot_with_fonts_for_presentation_cached(
        &document,
        ProjectUiStateSnapshotOutput::new(
            "test",
            AuiSnapshotSource::TestSnapshot,
            ProjectUiStateSnapshot::new(1),
        ),
        &engine_runtime::runtime_package::RuntimeAuiFontAtlasRegistry::empty("test"),
        &engine_runtime::font_bundle::RuntimeFontBundleRegistry::default(),
        None,
        &mut runtime,
    );
    let resolved = &out.resolved_document.nodes[1];
    assert_eq!(resolved.image.as_ref().unwrap().asset_id, "image");
    assert_eq!(resolved.resolved_style.as_ref().unwrap().paint.radius, 9.);
    assert_eq!(
        resolved
            .resolved_style
            .as_ref()
            .unwrap()
            .paint
            .border
            .color
            .as_deref(),
        Some("#00ff00ff")
    );
    document.nodes[1].binding_refs = vec![AuiBindingRef::new(
        "resource",
        AuiBindingTarget::Resource,
        "image",
        None,
    )];
    let invalid = ProjectUiStateSnapshot::new(1).with_value(
        "image",
        AuiBindingValue::Resource(UiResourceRef {
            asset_id: "image".into(),
            guid: "wrong".into(),
        }),
    );
    let (_, report) = AuiRuntimeResolver::resolve_bindings(&document, &invalid);
    assert_eq!(report.type_mismatch_count, 1);
    let valid = ProjectUiStateSnapshot::new(2).with_value(
        "image",
        AuiBindingValue::Resource(UiResourceRef {
            asset_id: "image".into(),
            guid: "guid-image".into(),
        }),
    );
    let (resolved, report) = AuiRuntimeResolver::resolve_bindings(&document, &valid);
    assert_eq!(report.resolved_count, 1);
    assert_eq!(resolved.nodes[1].image.as_ref().unwrap().asset_id, "image");
}

#[test]
fn encoded_typed_updates_commit_to_retained_instance_only_when_accepted() {
    let mut instance = mounted(2);
    let handle = instance.node("text-0").unwrap();
    let text = UiTextRef::try_new(&instance, handle).unwrap();
    let mut buffer = UiWriteBuffer::new(&instance);
    text.set_text(&mut buffer, "马").unwrap();
    text.set_text(&mut buffer, "骑兵").unwrap();
    text.node()
        .set_translation(&mut buffer, [20., 40.])
        .unwrap();
    assert_eq!(text.text(&buffer).unwrap(), "骑兵");
    let bytes = buffer.finish().unwrap().encode().unwrap();
    assert_eq!(text.text(&instance).unwrap(), "人");
    let batch = UiUpdateBatch::decode(&bytes).unwrap();
    assert_eq!(instance.apply_update_batch(&batch).unwrap(), 2);
    assert_eq!(text.text(&instance).unwrap(), "骑兵");
    assert_eq!(
        instance.read(handle, UiProperty::Translation).unwrap(),
        UiValue::Translation([20., 40.])
    );
    let mut dirty = Vec::new();
    instance.drain_invalidations(&mut dirty);
    assert_eq!(dirty.len(), 1);
    assert!(
        dirty[0].invalidation.measure
            && dirty[0].invalidation.transform
            && dirty[0].invalidation.hit
    );
    assert_eq!(instance.apply_update_batch(&batch).unwrap(), 0);
    instance.drain_invalidations(&mut dirty);
    assert!(dirty.is_empty());
}

#[test]
fn encoded_stale_or_cross_instance_handle_rejects_entire_batch() {
    let mut instance = mounted(2);
    let other = mounted(2);
    let first = instance.node("text-0").unwrap();
    let second = instance.node("text-1").unwrap();
    for bad in [
        UiNodeHandle {
            generation: second.generation + 1,
            ..second
        },
        other.node("text-1").unwrap(),
    ] {
        let batch = UiUpdateBatch::new(vec![
            UiPropertyUpdate {
                handle: first,
                property: UiProperty::Text,
                value: UiValue::Text("骑".into()),
                notify: true,
            },
            UiPropertyUpdate {
                handle: bad,
                property: UiProperty::Text,
                value: UiValue::Text("兵".into()),
                notify: true,
            },
        ])
        .unwrap();
        let decoded = UiUpdateBatch::decode(&batch.encode().unwrap()).unwrap();
        assert_eq!(
            instance.apply_update_batch(&decoded).unwrap_err().kind,
            UiErrorKind::InvalidHandle
        );
        assert_eq!(
            instance.read(first, UiProperty::Text).unwrap(),
            UiValue::Text("人".into())
        );
        let mut dirty = Vec::new();
        instance.drain_invalidations(&mut dirty);
        assert!(dirty.is_empty());
    }
}

#[test]
fn encoded_binding_conflict_and_disposal_are_rechecked_at_commit() {
    let mut document = (*source(2)).clone();
    document.nodes[2].binding_refs.push(AuiBindingRef::new(
        "label",
        AuiBindingTarget::TextText,
        "game.label",
        None,
    ));
    let mut instance = AuiInstance::mount(Arc::new(document)).unwrap();
    instance.drain_invalidations(&mut Vec::new());
    let first = instance.node("text-0").unwrap();
    let second = instance.node("text-1").unwrap();
    let mut buffer = UiWriteBuffer::new(&instance);
    UiTextRef::try_new(&buffer, first)
        .unwrap()
        .set_text(&mut buffer, "骑")
        .unwrap();
    UiTextRef::try_new(&buffer, second)
        .unwrap()
        .set_text(&mut buffer, "兵")
        .unwrap();
    let batch = UiUpdateBatch::decode(&buffer.finish().unwrap().encode().unwrap()).unwrap();
    let error = instance.apply_update_batch(&batch).unwrap_err();
    assert_eq!(error.kind, UiErrorKind::BindingOwned);
    assert_eq!(error.handle, Some(second));
    assert_eq!(
        instance.read(first, UiProperty::Text).unwrap(),
        UiValue::Text("人".into())
    );
    let mut dirty = Vec::new();
    instance.drain_invalidations(&mut dirty);
    assert!(dirty.is_empty());
    instance.dispose();
    assert_eq!(
        instance.apply_update_batch(&batch).unwrap_err().kind,
        UiErrorKind::Disposed
    );
}

#[test]
fn typed_setter_reads_own_write_and_leaves_authoring_unchanged() {
    let document = source(2);
    let mut instance = AuiInstance::mount(Arc::clone(&document)).unwrap();
    let text = UiTextRef::try_new(&instance, instance.node("text-0").unwrap()).unwrap();
    let mut edit = instance.edit();
    text.set_text(&mut edit, "骑").unwrap();
    assert_eq!(text.text(&edit).unwrap(), "骑");
    assert_eq!(edit.pending_properties(), 1);
    assert_eq!(edit.commit().unwrap(), 1);
    assert_eq!(text.text(&instance).unwrap(), "骑");
    assert_eq!(document.nodes[1].text.as_deref(), Some("人"));
}

#[test]
fn same_value_and_return_to_original_have_no_work() {
    let mut instance = mounted(2);
    let text = UiTextRef::try_new(&instance, instance.node("text-0").unwrap()).unwrap();
    let mut edit = instance.edit();
    text.set_text(&mut edit, "人").unwrap();
    assert_eq!(edit.pending_properties(), 0);
    text.set_text(&mut edit, "马").unwrap();
    text.set_text(&mut edit, "人").unwrap();
    assert_eq!(edit.pending_properties(), 0);
    assert_eq!(edit.commit().unwrap(), 0);
    let mut dirty = Vec::new();
    instance.drain_invalidations(&mut dirty);
    assert!(dirty.is_empty());
}

#[test]
fn failed_segment_cannot_partially_commit_or_expose_staged_values() {
    let mut instance = mounted(1);
    let handle = instance.node("text-0").unwrap();
    let text = UiTextRef::try_new(&instance, handle).unwrap();
    let node = UiNodeRef::new(handle);
    let mut edit = instance.edit();
    text.set_text(&mut edit, "骑").unwrap();
    assert!(node.set_translation(&mut edit, [f32::NAN, 0.]).is_err());
    assert!(text.text(&edit).is_err());
    assert!(edit.commit().is_err());
    assert_eq!(text.text(&instance).unwrap(), "人");
    let mut dirty = Vec::new();
    instance.drain_invalidations(&mut dirty);
    assert!(dirty.is_empty());
}

#[test]
fn dropping_edit_aborts_and_wrong_instance_handle_is_rejected() {
    let mut a = mounted(1);
    let b = mounted(1);
    let text = UiTextRef::try_new(&a, a.node("text-0").unwrap()).unwrap();
    {
        let mut edit = a.edit();
        text.set_text(&mut edit, "骑").unwrap();
    }
    assert_eq!(text.text(&a).unwrap(), "人");
    assert!(text.text(&b).is_err());
    assert!(UiButtonRef::try_new(&a, text.node().handle()).is_err());
    a.dispose();
    assert!(text.text(&a).is_err());
    assert!(a.node("text-0").is_err());
}

#[test]
fn binding_owner_rejects_direct_setter_without_changing_other_properties() {
    let mut document = (*source(1)).clone();
    document.nodes[1].binding_refs.push(AuiBindingRef::new(
        "label",
        AuiBindingTarget::TextText,
        "game.label",
        None,
    ));
    let mut instance = AuiInstance::mount(Arc::new(document)).unwrap();
    let handle = instance.node("text-0").unwrap();
    let mut edit = instance.edit();
    UiNodeRef::new(handle)
        .set_interactable(&mut edit, false)
        .unwrap();
    assert!(UiTextRef::try_new(&edit, handle)
        .unwrap()
        .set_text(&mut edit, "骑")
        .is_err());
    assert!(edit.commit().is_err());
    assert_eq!(
        instance.read(handle, UiProperty::Interactable).unwrap(),
        UiValue::Bool(true)
    );
}

#[test]
fn binding_ownership_can_be_taken_and_released_without_last_writer_wins() {
    let mut document = (*source(2)).clone();
    document.nodes[1].binding_refs.push(AuiBindingRef::new(
        "label",
        AuiBindingTarget::TextText,
        "game.label",
        None,
    ));
    let mut instance = AuiInstance::mount(Arc::new(document)).unwrap();
    instance.drain_invalidations(&mut Vec::new());
    let bound = instance.node("text-0").unwrap();
    let unbound = instance.node("text-1").unwrap();

    let direct = UiUpdateBatch::new(vec![UiPropertyUpdate {
        handle: bound,
        property: UiProperty::Text,
        value: UiValue::Text("冲突".into()),
        notify: false,
    }])
    .unwrap();
    assert_eq!(
        instance.apply_update_batch(&direct).unwrap_err().kind,
        UiErrorKind::BindingOwned
    );

    let takeover = UiUpdateBatch::new_with_custom_and_ownership(
        vec![UiPropertyUpdate {
            handle: bound,
            property: UiProperty::Text,
            value: UiValue::Text("接管".into()),
            notify: false,
        }],
        Vec::new(),
        vec![UiOwnershipUpdate {
            handle: bound,
            property: UiProperty::Text,
            owner: UiPropertyOwner::Controller,
        }],
    )
    .unwrap();
    assert_eq!(instance.apply_update_batch(&takeover).unwrap(), 2);
    assert_eq!(
        instance.read(bound, UiProperty::Text).unwrap(),
        UiValue::Text("接管".into())
    );
    assert_eq!(instance.apply_update_batch(&takeover).unwrap(), 0);

    let follow_up = UiUpdateBatch::new(vec![UiPropertyUpdate {
        handle: bound,
        property: UiProperty::Text,
        value: UiValue::Text("控制器".into()),
        notify: false,
    }])
    .unwrap();
    assert_eq!(instance.apply_update_batch(&follow_up).unwrap(), 1);

    let release = UiUpdateBatch::new_with_custom_and_ownership(
        Vec::new(),
        Vec::new(),
        vec![UiOwnershipUpdate {
            handle: bound,
            property: UiProperty::Text,
            owner: UiPropertyOwner::Binding,
        }],
    )
    .unwrap();
    assert_eq!(instance.apply_update_batch(&release).unwrap(), 1);
    assert_eq!(instance.apply_update_batch(&release).unwrap(), 0);
    assert_eq!(
        instance.apply_update_batch(&follow_up).unwrap_err().kind,
        UiErrorKind::BindingOwned
    );

    let invalid = UiUpdateBatch::new_with_custom_and_ownership(
        vec![UiPropertyUpdate {
            handle: unbound,
            property: UiProperty::Text,
            value: UiValue::Text("非法".into()),
            notify: false,
        }],
        Vec::new(),
        vec![UiOwnershipUpdate {
            handle: unbound,
            property: UiProperty::Text,
            owner: UiPropertyOwner::Controller,
        }],
    )
    .unwrap();
    assert_eq!(
        instance.apply_update_batch(&invalid).unwrap_err().kind,
        UiErrorKind::BindingUnavailable
    );
    assert_eq!(
        instance.read(unbound, UiProperty::Text).unwrap(),
        UiValue::Text("人".into())
    );
}

#[test]
fn single_node_change_in_large_instance_only_queues_that_node() {
    let mut instance = mounted(10_000);
    let handle = instance.node("text-9000").unwrap();
    let mut edit = instance.edit();
    let node = UiNodeRef::new(handle);
    node.set_translation(&mut edit, [10., 20.]).unwrap();
    node.set_translation(&mut edit, [30., 40.]).unwrap();
    node.set_interactable(&mut edit, false).unwrap();
    assert_eq!(edit.pending_properties(), 2);
    assert_eq!(edit.commit().unwrap(), 2);
    let mut dirty = Vec::new();
    instance.drain_invalidations(&mut dirty);
    assert_eq!(dirty.len(), 1);
    assert_eq!(dirty[0].handle, handle);
    assert!(dirty[0].invalidation.transform && dirty[0].invalidation.hit);
    assert!(!dirty[0].invalidation.measure && !dirty[0].invalidation.layout);
    instance.drain_invalidations(&mut dirty);
    assert!(dirty.is_empty());
}

#[test]
fn text_and_visibility_classify_layout_and_paint_differently() {
    let mut instance = mounted(1);
    let handle = instance.node("text-0").unwrap();
    let node = UiNodeRef::new(handle);
    let mut edit = instance.edit();
    node.set_visibility(&mut edit, UiVisibility::Hidden)
        .unwrap();
    edit.commit().unwrap();
    let mut dirty = Vec::new();
    instance.drain_invalidations(&mut dirty);
    assert!(!dirty[0].invalidation.layout);
    assert!(dirty[0].invalidation.paint && dirty[0].invalidation.hit);
    let mut edit = instance.edit();
    node.set_visibility(&mut edit, UiVisibility::Collapsed)
        .unwrap();
    edit.commit().unwrap();
    instance.drain_invalidations(&mut dirty);
    assert!(dirty[0].invalidation.layout);
    let mut edit = instance.edit();
    UiTextRef::try_new(&edit, handle)
        .unwrap()
        .set_text(&mut edit, "弓")
        .unwrap();
    edit.commit().unwrap();
    instance.drain_invalidations(&mut dirty);
    assert!(dirty[0].invalidation.measure && dirty[0].invalidation.paint);
}

#[test]
fn invalid_document_is_rejected_before_creating_an_instance() {
    let mut document = (*source(1)).clone();
    document.nodes[1].node_id = "root".into();
    assert!(AuiInstance::mount(Arc::new(document)).is_err());
}

#[test]
fn stale_generation_wrong_value_and_missing_id_are_rejected() {
    let mut instance = mounted(1);
    let handle = instance.node("text-0").unwrap();
    let stale = UiNodeHandle {
        generation: handle.generation + 1,
        ..handle
    };
    assert_eq!(
        instance.node_type(stale).unwrap_err().kind,
        UiErrorKind::InvalidHandle
    );
    assert_eq!(
        instance.node("missing").unwrap_err().kind,
        UiErrorKind::UnknownNode
    );
    let mut edit = instance.edit();
    let failure = edit
        .write(handle, UiProperty::Text, UiValue::Bool(false))
        .unwrap_err();
    assert_eq!(failure.kind, UiErrorKind::WrongValueType);
    assert_eq!(failure.handle, Some(handle));
    assert_eq!(failure.property, Some(UiProperty::Text));
    assert_eq!(edit.commit().unwrap_err(), failure);
}

#[test]
fn text_budget_failure_rolls_back_and_replacement_releases_budget() {
    let mut instance = mounted(2);
    let a = UiTextRef::try_new(&instance, instance.node("text-0").unwrap()).unwrap();
    let b = UiTextRef::try_new(&instance, instance.node("text-1").unwrap()).unwrap();
    let mut edit = instance.edit();
    a.set_text(&mut edit, "x".repeat(MAX_AUI_EDIT_TEXT_BYTES))
        .unwrap();
    assert_eq!(
        b.set_text(&mut edit, "马").unwrap_err().kind,
        UiErrorKind::CapacityExceeded
    );
    assert!(edit.commit().is_err());
    assert_eq!(a.text(&instance).unwrap(), "人");
    let mut edit = instance.edit();
    a.set_text(&mut edit, "x".repeat(MAX_AUI_EDIT_TEXT_BYTES))
        .unwrap();
    a.set_text(&mut edit, "人").unwrap();
    b.set_text(&mut edit, "马").unwrap();
    assert_eq!(edit.commit().unwrap(), 1);
}

#[test]
fn property_budget_failure_rejects_the_entire_segment() {
    let mut instance = mounted(MAX_AUI_EDIT_PROPERTIES + 1);
    let handles = (0..=MAX_AUI_EDIT_PROPERTIES)
        .map(|i| instance.node(&format!("text-{i}")).unwrap())
        .collect::<Vec<_>>();
    let mut edit = instance.edit();
    for &handle in &handles[..MAX_AUI_EDIT_PROPERTIES] {
        UiNodeRef::new(handle)
            .set_translation(&mut edit, [1., 0.])
            .unwrap();
    }
    assert_eq!(
        UiNodeRef::new(handles[MAX_AUI_EDIT_PROPERTIES])
            .set_translation(&mut edit, [1., 0.])
            .unwrap_err()
            .kind,
        UiErrorKind::CapacityExceeded
    );
    assert!(edit.commit().is_err());
    assert_eq!(
        instance.read(handles[0], UiProperty::Translation).unwrap(),
        UiValue::Translation([0., 0.])
    );
}

#[test]
fn button_reference_and_style_invalidation_use_the_same_access_contract() {
    let mut document = (*source(1)).clone();
    document.nodes[1].kind = AuiNodeKind::Button;
    let mut instance = AuiInstance::mount(Arc::new(document)).unwrap();
    let mut dirty = Vec::new();
    instance.drain_invalidations(&mut dirty);
    let button = UiButtonRef::try_new(&instance, instance.node("text-0").unwrap()).unwrap();
    let mut edit = instance.edit();
    button.set_interactable(&mut edit, false).unwrap();
    edit.commit().unwrap();
    instance.drain_invalidations(&mut dirty);
    assert_eq!(dirty.len(), 1);
    assert!(dirty[0].invalidation.style && dirty[0].invalidation.paint);
}

#[test]
fn legacy_import_keeps_source_schema_and_rejects_legacy_c2_mismatch() {
    let mut document = (*source(1)).clone();
    document.schema_version = AUI_DOCUMENT_SCHEMA_VERSION.into();
    let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
    assert_eq!(
        instance.source().schema_version,
        AUI_DOCUMENT_SCHEMA_VERSION
    );
    instance.dispose();
    assert_eq!(
        instance.edit().commit().unwrap_err().kind,
        UiErrorKind::Disposed
    );
    document.nodes[0].layout = Some(Default::default());
    assert!(AuiInstance::mount(Arc::new(document)).is_err());
}

fn dispatch_one(instance: &mut AuiInstance) -> (AuiNotification, Vec<u64>) {
    let event = instance
        .begin_dispatch()
        .expect("dispatch state")
        .expect("queued notification");
    let mut callbacks = Vec::new();
    while let Some(listener) = instance.next_dispatch_listener().expect("active dispatch") {
        callbacks.push(listener.callback_id);
    }
    instance.end_dispatch().expect("active dispatch");
    (event, callbacks)
}

#[test]
fn active_visibility_and_interactable_are_independent_and_inherit_active() {
    let mut instance = mounted(2);
    let root = instance.node("root").unwrap();
    let child = instance.node("text-0").unwrap();
    let sibling = instance.node("text-1").unwrap();

    let mut edit = instance.edit();
    UiNodeRef::new(child)
        .set_visibility(&mut edit, UiVisibility::Hidden)
        .unwrap();
    UiNodeRef::new(child)
        .set_interactable(&mut edit, false)
        .unwrap();
    UiNodeRef::new(root).set_active(&mut edit, false).unwrap();
    edit.commit().unwrap();

    assert_eq!(
        instance.read(root, UiProperty::Active).unwrap(),
        UiValue::Bool(false)
    );
    assert_eq!(
        instance.read(child, UiProperty::Visibility).unwrap(),
        UiValue::Visibility(UiVisibility::Hidden)
    );
    assert_eq!(
        instance.read(child, UiProperty::Interactable).unwrap(),
        UiValue::Bool(false)
    );
    assert!(!instance.is_effectively_active(child).unwrap());
    assert!(!instance.is_effectively_active(sibling).unwrap());

    let mut edit = instance.edit();
    UiNodeRef::new(root).set_active(&mut edit, true).unwrap();
    edit.commit().unwrap();
    assert!(instance.is_effectively_active(sibling).unwrap());
    assert!(instance.is_effectively_active(child).unwrap());
    assert_eq!(
        instance.read(child, UiProperty::Visibility).unwrap(),
        UiValue::Visibility(UiVisibility::Hidden),
        "reenabling must not rewrite visibility"
    );
}

#[test]
fn changed_and_lifecycle_notifications_obey_barriers_and_silent_writes() {
    let mut instance = mounted(1);
    let handle = instance.node("text-0").unwrap();
    instance
        .subscribe(
            handle,
            AuiEventFilter::All,
            AuiSubscriptionScope::Instance,
            17,
        )
        .unwrap();
    let (mount, callbacks) = dispatch_one(&mut instance);
    assert_eq!(mount.kind, AuiNotificationKind::Mount);
    assert_eq!(callbacks, vec![17]);
    let (enable, _) = dispatch_one(&mut instance);
    assert_eq!(enable.kind, AuiNotificationKind::Enable);

    let mut edit = instance.edit_without_notify(AuiChangeOrigin::Binding);
    UiTextRef::try_new(&edit, handle)
        .unwrap()
        .set_text(&mut edit, "静默")
        .unwrap();
    edit.commit().unwrap();
    assert!(instance.begin_dispatch().unwrap().is_none());

    let mut edit = instance.edit_from(AuiChangeOrigin::Programmatic);
    UiTextRef::try_new(&edit, handle)
        .unwrap()
        .set_text(&mut edit, "变更")
        .unwrap();
    UiNodeRef::new(handle).set_active(&mut edit, false).unwrap();
    edit.commit().unwrap();
    let (changed_text, _) = dispatch_one(&mut instance);
    assert_eq!(
        changed_text.kind,
        AuiNotificationKind::Changed {
            property: UiProperty::Text,
            old_value: UiValue::Text("静默".into()),
            new_value: UiValue::Text("变更".into()),
            origin: AuiChangeOrigin::Programmatic,
        }
    );
    let (changed_active, _) = dispatch_one(&mut instance);
    assert!(matches!(
        changed_active.kind,
        AuiNotificationKind::Changed {
            property: UiProperty::Active,
            old_value: UiValue::Bool(true),
            new_value: UiValue::Bool(false),
            ..
        }
    ));
    let (disable, _) = dispatch_one(&mut instance);
    assert_eq!(disable.kind, AuiNotificationKind::Disable);
    assert!(instance.begin_dispatch().unwrap().is_none());
}

#[test]
fn subscription_order_scope_cancellation_and_reentrancy_are_deterministic() {
    let mut instance = mounted(1);
    let handle = instance.node("text-0").unwrap();
    let first = instance
        .subscribe(
            handle,
            AuiEventFilter::Changed(None),
            AuiSubscriptionScope::Instance,
            1,
        )
        .unwrap();
    let second = instance
        .subscribe(
            handle,
            AuiEventFilter::Changed(None),
            AuiSubscriptionScope::Active,
            2,
        )
        .unwrap();

    let mut edit = instance.edit();
    UiTextRef::try_new(&edit, handle)
        .unwrap()
        .set_text(&mut edit, "一")
        .unwrap();
    edit.commit().unwrap();
    let event = instance.begin_dispatch().unwrap().unwrap();
    assert!(matches!(event.kind, AuiNotificationKind::Changed { .. }));
    assert_eq!(
        instance.begin_dispatch().unwrap_err(),
        AuiDispatchError::Reentrant
    );
    assert_eq!(
        instance
            .next_dispatch_listener()
            .unwrap()
            .unwrap()
            .callback_id,
        1
    );
    let late = instance
        .subscribe(
            handle,
            AuiEventFilter::Changed(None),
            AuiSubscriptionScope::Instance,
            3,
        )
        .unwrap();
    assert!(instance.cancel_subscription(second));
    assert!(!instance.cancel_subscription(second));
    assert!(instance.next_dispatch_listener().unwrap().is_none());
    instance.end_dispatch().unwrap();

    let mut edit = instance.edit();
    UiTextRef::try_new(&edit, handle)
        .unwrap()
        .set_text(&mut edit, "二")
        .unwrap();
    edit.commit().unwrap();
    let (_, callbacks) = dispatch_one(&mut instance);
    assert_eq!(callbacks, vec![1, 3]);
    assert!(instance.cancel_subscription(first));
    assert!(instance.cancel_subscription(late));
}

#[test]
fn dispose_emits_disable_then_dispose_once_and_releases_subscriptions() {
    let mut instance = mounted(1);
    let handle = instance.node("text-0").unwrap();
    let token = instance
        .subscribe(
            handle,
            AuiEventFilter::Lifecycle,
            AuiSubscriptionScope::Instance,
            9,
        )
        .unwrap();
    dispatch_one(&mut instance);
    dispatch_one(&mut instance);
    instance.dispose();
    instance.dispose();
    let (disable, callbacks) = dispatch_one(&mut instance);
    assert_eq!(disable.kind, AuiNotificationKind::Disable);
    assert_eq!(callbacks, vec![9]);
    let (dispose, callbacks) = dispatch_one(&mut instance);
    assert_eq!(dispose.kind, AuiNotificationKind::Dispose);
    assert_eq!(callbacks, vec![9]);
    assert!(instance.begin_dispatch().unwrap().is_none());
    assert!(!instance.cancel_subscription(token));
    assert_eq!(
        instance.read(handle, UiProperty::Active).unwrap_err().kind,
        UiErrorKind::Disposed
    );
}

#[test]
fn active_scope_pauses_while_inactive_and_resumes_without_reregistering() {
    let mut instance = mounted(1);
    let handle = instance.node("text-0").unwrap();
    instance
        .subscribe(
            handle,
            AuiEventFilter::Changed(Some(UiProperty::Text)),
            AuiSubscriptionScope::Active,
            5,
        )
        .unwrap();

    let mut edit = instance.edit();
    UiNodeRef::new(handle).set_active(&mut edit, false).unwrap();
    edit.commit().unwrap();
    let mut edit = instance.edit();
    UiTextRef::try_new(&edit, handle)
        .unwrap()
        .set_text(&mut edit, "停用期间")
        .unwrap();
    edit.commit().unwrap();

    let mut edit = instance.edit();
    UiNodeRef::new(handle).set_active(&mut edit, true).unwrap();
    edit.commit().unwrap();
    let mut edit = instance.edit();
    UiTextRef::try_new(&edit, handle)
        .unwrap()
        .set_text(&mut edit, "恢复")
        .unwrap();
    edit.commit().unwrap();
    let (_, callbacks) = dispatch_one(&mut instance);
    assert_eq!(
        callbacks,
        vec![5],
        "the inactive change must not be replayed after the scope resumes"
    );
    assert!(instance.begin_dispatch().unwrap().is_none());
}
