use super::*;
use crate::render_graph::color;

fn unique_plan(count: usize) -> RhiCommandPlan {
    let mut plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(
        &crate::rhi_command_plan::tests::retained_geometry_graph(count),
    );
    let vertices: Vec<f32> = vec![-1., -1., 0., 1., -1., 0., 0., 1., 0.];
    let geometry: GeometryBuffers = serde_json::from_value(serde_json::json!({
        "vertexBytes": bytemuck::cast_slice::<f32, u8>(&vertices),
        "indexBytes": [0,0,1,0,2,0], "vertexStride": 12,
        "vertexCount": 3, "indexCount": 3, "indexFormat": "uint16",
        "attributes": [{"semantic":"position","format":"float32x3","offset":0}]
    }))
    .unwrap();
    let geometry = Arc::new(geometry);
    let mut index = 0;
    for command in &mut plan.commands {
        if let RhiCommand::Draw {
            payload: RhiDrawPayload::Geometry { draw },
            ..
        } = command
        {
            index += 1;
            let draw = Arc::make_mut(draw);
            draw.geometry = geometry.clone();
            draw.world_transform[0] = 0.045f32.into();
            draw.world_transform[5] = 0.045f32.into();
            draw.world_transform[12] = (-1.3 + ((index - 1) % 24) as f32 * 0.11).into();
            draw.world_transform[13] = (-0.9 + ((index - 1) / 24) as f32 * 0.11).into();
            draw.world_transform[14] = (-2f32).into();
            draw.base_color = color([0.2 + (index % 4) as f32 * 0.2, 0.8, 0.3, 1.]);
            draw.geometry_binding = Arc::new(
                RenderBindingSet::single(
                    format!("visibility-mesh-{index}"),
                    RenderBindingKind::Mesh,
                    RenderResourceHandle {
                        kind: crate::render_resource::RenderResourceKind::MeshBuffer,
                        index,
                        generation: 1,
                    },
                    "nearestClamp".into(),
                    false,
                    "selection-test".into(),
                )
                .unwrap(),
            );
        }
    }
    assert_eq!(index as usize, count);
    plan
}

#[test]
#[ignore = "requires a real GPU"]
fn selection_rejects_malformed_binding_before_interpreting_native_handle() {
    let plan = unique_plan(4);
    let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
    backend
        .prepare_geometry_plan_for_extent(&plan, [640, 480], None)
        .unwrap();
    let mut invalid = plan.clone();
    let command = invalid
        .commands
        .iter_mut()
        .rfind(|command| geometry_draw(command).is_some())
        .unwrap();
    let RhiCommand::Draw {
        payload: RhiDrawPayload::Geometry { draw },
        ..
    } = command
    else {
        panic!()
    };
    Arc::make_mut(&mut Arc::make_mut(draw).geometry_binding)
        .slots
        .clear();
    let cached = backend.geometry.validated_plan.as_ref().unwrap();
    assert!(cached
        .moving_views
        .as_ref()
        .unwrap()
        .select(&backend, &invalid, cached, [640, 480], None)
        .is_none());
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
}

#[test]
#[ignore = "requires a real GPU"]
fn large_selection_reads_use_bounded_workers_and_preserve_native_order_and_rejection() {
    let plan = unique_plan(2048);
    let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
    backend
        .prepare_geometry_plan_for_extent(&plan, [640, 480], None)
        .unwrap();
    let mut selected = plan.clone();
    let positions: Vec<_> = plan
        .commands
        .iter()
        .enumerate()
        .filter_map(|(i, c)| geometry_draw(c).map(|_| i))
        .collect();
    selected.commands.remove(positions[150]);
    let cached = backend.geometry.validated_plan.as_ref().unwrap();
    moving_views::READ_THREADS.with(|threads| threads.borrow_mut().clear());
    let selection = cached
        .moving_views
        .as_ref()
        .unwrap()
        .select(&backend, &selected, cached, [640, 480], None)
        .unwrap();
    assert_eq!(selection.sources_len_for_test(), 2047);
    let workers = moving_views::READ_THREADS.with(|threads| threads.borrow().len());
    assert!(
        (2..=4).contains(&workers),
        "large read-only selection remained serial: {workers} worker(s)"
    );
    drop(selection);
    let actual = backend
        .render_plan_to_rgba_bytes(&selected, 640, 480)
        .unwrap();
    assert!(actual.chunks_exact(4).any(|pixel| pixel[1] > 20));
    let cached = backend.geometry.validated_plan.as_ref().unwrap();
    assert_eq!(cached.native_templates.len(), selected.commands.len());
    for (command, native) in selected.commands.iter().zip(cached.native_templates.iter()) {
        assert_eq!(
            geometry_draw(command).map(|draw| draw.geometry_handle().raw()),
            native.as_ref().map(|template| template.mesh),
            "partition merge changed the aligned native command order"
        );
    }
    let mut reference = RealWgpuBackend::new_offscreen(640, 480).unwrap();
    assert_eq!(
        actual,
        reference
            .render_plan_to_rgba_bytes(&selected, 640, 480)
            .unwrap(),
        "parallel selection changed native command order or ranges"
    );
    reference
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();

    let mut moved = selected.clone();
    let mut view = None;
    for command in &mut moved.commands {
        match command {
            RhiCommand::BeginView { view: current, .. } => {
                let value = Arc::make_mut(current);
                value.version += 1;
                value.view_matrix[12] = 0.125f32.into();
                view = Some(current.clone());
            }
            RhiCommand::Draw {
                payload: RhiDrawPayload::Geometry { draw },
                ..
            } => {
                Arc::make_mut(draw).view = view.clone().unwrap();
            }
            _ => {}
        }
    }
    let cached = backend.geometry.validated_plan.as_ref().unwrap();
    moving_views::READ_THREADS.with(|threads| threads.borrow_mut().clear());
    assert!(cached.moving_views.as_ref().unwrap().matches(
        &backend,
        &moved,
        cached,
        &cached.native_templates,
        [640, 480],
        None
    ));
    let workers = moving_views::READ_THREADS.with(|threads| threads.borrow().len());
    assert!(
        (2..=4).contains(&workers),
        "stable moving reads remained serial: {workers}"
    );

    // Absent resident sources in different partitions still conflict with one
    // another. Every partition alone would admit its new, well-formed source.
    let mut conflict = selected.clone();
    let positions: Vec<_> = conflict
        .commands
        .iter()
        .enumerate()
        .filter_map(|(i, c)| geometry_draw(c).map(|_| i))
        .collect();
    for position in [positions[10], *positions.last().unwrap()] {
        let RhiCommand::Draw {
            payload: RhiDrawPayload::Geometry { draw },
            ..
        } = &mut conflict.commands[position]
        else {
            panic!()
        };
        let draw = Arc::make_mut(draw);
        let binding = Arc::make_mut(&mut draw.geometry_binding);
        let BindingResource::Geometry(handle) = &mut binding.slots[0].resource else {
            panic!()
        };
        *handle = GeometryHandle::try_from(RenderResourceHandle {
            index: 3000,
            ..handle.raw()
        })
        .unwrap();
    }
    let cached = backend.geometry.validated_plan.as_ref().unwrap();
    let shared = cached
        .moving_views
        .as_ref()
        .unwrap()
        .select(&backend, &conflict, cached, [640, 480], None)
        .unwrap();
    assert_eq!(
        shared.sources_len_for_test(),
        2046,
        "equal absent sources in different partitions were not deduplicated"
    );
    drop(shared);
    let RhiCommand::Draw {
        payload: RhiDrawPayload::Geometry { draw },
        ..
    } = &mut conflict.commands[*positions.last().unwrap()]
    else {
        panic!()
    };
    let draw = Arc::make_mut(draw);
    let mut wire = serde_json::to_value(draw.geometry.as_ref()).unwrap();
    wire["vertexBytes"][0] = serde_json::json!(1);
    draw.geometry = Arc::new(serde_json::from_value(wire).unwrap());
    assert!(
        cached
            .moving_views
            .as_ref()
            .unwrap()
            .select(&backend, &conflict, cached, [640, 480], None)
            .is_none(),
        "cross-partition absent sources lost their identity conflict"
    );
    assert_eq!(
        backend.prepare_geometry_plan_for_extent(&conflict, [640, 480], None),
        Err("geometry.resource_identity_conflict".into())
    );
    // Repair the selection, then put a malformed draw in the last partition.
    backend
        .prepare_geometry_plan_for_extent(&selected, [640, 480], None)
        .unwrap();
    let mut invalid = selected.clone();
    let RhiCommand::Draw {
        payload: RhiDrawPayload::Geometry { draw },
        ..
    } = &mut invalid.commands[*positions.last().unwrap()]
    else {
        panic!()
    };
    Arc::make_mut(draw).world_transform[0] = f32::NAN.into();
    let cached = backend.geometry.validated_plan.as_ref().unwrap();
    assert!(cached
        .moving_views
        .as_ref()
        .unwrap()
        .select(&backend, &invalid, cached, [640, 480], None)
        .is_none());
    assert!(backend
        .prepare_geometry_plan_for_extent(&invalid, [640, 480], None)
        .is_err());
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
}

#[test]
#[ignore = "requires a real GPU"]
fn selection_scans_current_mesh_registry_once_for_sources_and_native_ranges() {
    let plan = unique_plan(384);
    let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
    backend
        .prepare_geometry_plan_for_extent(&plan, [640, 480], None)
        .unwrap();
    let mut changed = plan.clone();
    let removed = changed
        .commands
        .iter()
        .position(|c| geometry_draw(c).is_some())
        .unwrap();
    changed.commands.remove(removed);
    let cached = backend.geometry.validated_plan.as_ref().unwrap();
    residency::REGISTRY_BUILD_VISITS.with(|n| n.set(0));
    let selected = cached
        .moving_views
        .as_ref()
        .unwrap()
        .select(&backend, &changed, cached, [640, 480], None)
        .unwrap();
    assert_eq!(selected.sources_len_for_test(), 383);
    assert_eq!(
        residency::REGISTRY_BUILD_VISITS.with(|n| n.get()),
        384,
        "source admission and native checks rebuilt the same current registry"
    );
    drop(selected);
    residency::REGISTRY_BUILD_VISITS.with(|n| n.set(0));
    residency::SOURCE_CLOSURE_INSERTIONS.with(|n| n.set(0));
    moving_views::TARGET_COPIES.with(|n| n.set(0));
    native_template::SELECTION_TEMPLATE_REVISITS.with(|n| n.set(0));
    backend
        .prepare_geometry_plan_for_extent(&changed, [640, 480], None)
        .unwrap();
    assert!(
        residency::REGISTRY_BUILD_VISITS.with(|n| n.get()) <= 768,
        "selection installation rebuilt native lookup tables: {} visits",
        residency::REGISTRY_BUILD_VISITS.with(|n| n.get())
    );
    assert_eq!(
        native_template::SELECTION_TEMPLATE_REVISITS.with(|n| n.get()),
        0,
        "selection installation revisited already admitted native templates"
    );
    assert_eq!(
        (
            residency::SOURCE_CLOSURE_INSERTIONS.with(|n| n.get()),
            moving_views::TARGET_COPIES.with(|n| n.get())
        ),
        (0, 0),
        "selection publication rebuilt stable source nodes and repeated attachments"
    );
}

#[test]
#[ignore = "requires a real GPU"]
fn visibility_change_rerecords_only_affected_native_ranges_and_preserves_pixels() {
    let plan = unique_plan(384);
    let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
    let original = backend.render_plan_to_rgba_bytes(&plan, 640, 480).unwrap();
    assert!(
        original.chunks_exact(4).any(|pixel| pixel[1] > 20),
        "fixture did not draw real triangles"
    );
    let mut changed = plan.clone();
    let removed = changed
        .commands
        .iter()
        .enumerate()
        .filter(|(_, c)| geometry_draw(c).is_some())
        .nth(150)
        .unwrap()
        .0;
    changed.commands.remove(removed);
    NATIVE_DRAW_RECORDINGS.with(|n| n.set(0));
    encoding::SURVIVAL_MESH_SEARCHES.with(|n| n.set(0));
    encoding::RECIPE_CAPTURE_ROWS.with(|n| n.set(0));
    let uploaded_indices = backend.retained_geometry_stats().index_uploaded_bytes;
    let full_validation = backend.retained_geometry_stats().validation_draw_visits;
    let actual = backend
        .render_plan_to_rgba_bytes(&changed, 640, 480)
        .unwrap();
    let recorded = NATIVE_DRAW_RECORDINGS.with(|n| n.get());
    assert!(
        encoding::RECIPE_CAPTURE_ROWS.with(|n| n.get()) <= 128,
        "one visibility change rebuilt recipes for {} unchanged draws",
        encoding::RECIPE_CAPTURE_ROWS.with(|n| n.get())
    );
    assert_eq!(
        backend.retained_geometry_stats().validation_draw_visits,
        full_validation,
        "visibility-only selection restarted complete native preparation"
    );
    assert_eq!(
        encoding::SURVIVAL_MESH_SEARCHES.with(|n| n.get()),
        0,
        "selection preparation rebuilt per-range sparse resource lookups"
    );
    let mut cold = RealWgpuBackend::new_offscreen(640, 480).unwrap();
    let expected = cold.render_plan_to_rgba_bytes(&changed, 640, 480).unwrap();
    assert_eq!(actual, expected, "selection reused incorrect native ranges");
    assert_ne!(actual, original, "hidden triangle did not change the image");
    assert_eq!(
        backend.retained_geometry_stats().index_uploaded_bytes,
        uploaded_indices,
        "removing one visible row rewrote the stable native selection"
    );
    assert_eq!(
        backend.geometry.meshes.len(),
        383,
        "invisible unpinned mesh was not retired"
    );
    eprintln!("384 unique meshes, one invisible: actual recorded draws {recorded}");
    assert!(
        recorded <= 128,
        "one visibility change rerecorded {recorded} stable draws"
    );
    NATIVE_DRAW_RECORDINGS.with(|n| n.set(0));
    let actual = backend.render_plan_to_rgba_bytes(&plan, 640, 480).unwrap();
    let recorded = NATIVE_DRAW_RECORDINGS.with(|n| n.get());
    assert_eq!(
        backend.retained_geometry_stats().validation_draw_visits,
        full_validation,
        "mesh reentry restarted complete native preparation"
    );
    let mut cold = RealWgpuBackend::new_offscreen(640, 480).unwrap();
    assert_eq!(
        actual,
        cold.render_plan_to_rgba_bytes(&plan, 640, 480).unwrap()
    );
    assert!(
        recorded <= 128,
        "mesh reentry rerecorded {recorded} stable draws"
    );
    let mut reversed = plan.clone();
    let positions: Vec<_> = reversed
        .commands
        .iter()
        .enumerate()
        .filter(|(_, c)| geometry_draw(c).is_some())
        .map(|(i, _)| i)
        .collect();
    for pair in 0..positions.len() / 2 {
        reversed
            .commands
            .swap(positions[pair], positions[positions.len() - 1 - pair]);
    }
    let mut duplicate = plan.clone();
    let command = duplicate.commands[positions[10]].clone();
    duplicate.commands.insert(positions[11], command);
    assert_eq!(
        backend
            .render_plan_to_rgba_bytes(&duplicate, 640, 480)
            .unwrap_err(),
        "geometry.duplicate_view_instance"
    );
    let mut small = plan.clone();
    for &position in positions[254..].iter().rev() {
        small.commands.remove(position);
    }
    for (next, sparse) in [(&reversed, true), (&small, false), (&plan, true)] {
        let actual = backend.render_plan_to_rgba_bytes(next, 640, 480).unwrap();
        let mut reference = RealWgpuBackend::new_offscreen(640, 480).unwrap();
        assert_eq!(
            actual,
            reference.render_plan_to_rgba_bytes(next, 640, 480).unwrap(),
            "order/layout transition reused stale native offsets"
        );
        assert_eq!(
            backend
                .geometry
                .parameters
                .as_ref()
                .unwrap()
                .selection
                .is_some(),
            sparse
        );
        reference
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }
    for cycle in 0..10 {
        let mut selection = plan.clone();
        for (ordinal, &position) in positions.iter().enumerate().rev() {
            if ordinal % 7 == cycle % 7 {
                selection.commands.remove(position);
            }
        }
        backend
            .render_plan_to_rgba_bytes(&selection, 640, 480)
            .unwrap();
        assert!(
            backend
                .geometry
                .validated_plan
                .as_ref()
                .unwrap()
                .bundles
                .len()
                <= 6,
            "bounded instance slots fragmented recordings at visibility cycle {cycle}"
        );
    }
    assert_eq!(
        backend.render_plan_to_rgba_bytes(&plan, 640, 480).unwrap(),
        original
    );
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
}

#[test]
fn index_upload_ranges_cover_exact_changed_rows_and_do_not_clear_unselected_tail() {
    let old = [[0, 1, 2], [1, 1, 2], [2, 1, 2], [3, 1, 2]];
    assert!(parameters::index_updates(&old, &old[..2]).is_empty());
    let mut next = old.to_vec();
    next[1][1] = 3;
    next[3][2] = 4;
    next.push([4, 1, 2]);
    assert_eq!(parameters::index_updates(&old, &next), vec![1..2, 3..5]);
    assert_eq!(parameters::index_updates(&[], &old), vec![0..4]);
}

#[test]
#[ignore = "requires a real GPU"]
fn late_same_count_selection_does_not_check_the_stable_prefix_twice() {
    let plan = unique_plan(384);
    let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
    backend.render_plan_to_rgba_bytes(&plan, 640, 480).unwrap();
    let mut next = plan.clone();
    let command = next
        .commands
        .iter_mut()
        .rev()
        .find(|c| geometry_draw(c).is_some())
        .unwrap();
    let RhiCommand::Draw {
        payload: RhiDrawPayload::Geometry { draw },
        ..
    } = command
    else {
        panic!()
    };
    let draw = Arc::make_mut(draw);
    let binding = Arc::make_mut(&mut draw.geometry_binding);
    let BindingResource::Geometry(handle) = &mut binding.slots[0].resource else {
        panic!()
    };
    *handle = GeometryHandle::try_from(RenderResourceHandle {
        index: 385,
        ..handle.raw()
    })
    .unwrap();
    native_template::CURRENT_CHECKS.with(|n| n.set(0));
    let actual = backend.render_plan_to_rgba_bytes(&next, 640, 480).unwrap();
    let checks = native_template::CURRENT_CHECKS.with(|n| n.get());
    assert!(
        checks <= 512,
        "late selection change checked stable native prefix twice: {checks}"
    );
    let mut reference = RealWgpuBackend::new_offscreen(640, 480).unwrap();
    assert_eq!(
        actual,
        reference
            .render_plan_to_rgba_bytes(&next, 640, 480)
            .unwrap()
    );
    assert_eq!(backend.geometry.meshes.len(), 384);
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
}

#[test]
#[ignore = "requires a real GPU"]
fn moving_and_selected_geometry_checks_current_output_capabilities_once() {
    let plan = unique_plan(384);
    let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
    backend.render_plan_to_rgba_bytes(&plan, 640, 480).unwrap();
    backend.validate_graph_capabilities(&plan).unwrap();
    let mut moved = plan.clone();
    let mut current_view = None;
    for command in &mut moved.commands {
        if let RhiCommand::BeginView { view, .. } = command {
            let value = Arc::make_mut(view);
            value.version += 1;
            value.view_matrix[12] = 0.125f32.into();
            current_view = Some(view.clone());
        } else if let RhiCommand::Draw {
            payload: RhiDrawPayload::Geometry { draw },
            ..
        } = command
        {
            Arc::make_mut(draw).view = current_view.clone().unwrap();
        }
    }
    let mut selected = moved.clone();
    let index = selected
        .commands
        .iter()
        .rposition(|c| geometry_draw(c).is_some())
        .unwrap();
    selected.commands.remove(index);
    for next in [&moved, &selected] {
        targets::OUTPUT_LAYOUT_CALLS.with(|n| n.set(0));
        assert!(backend
            .prepare_surface_resources(next, [640, 480], None)
            .unwrap());
        backend.validate_graph_capabilities(next).unwrap();
        assert_eq!(
            targets::OUTPUT_LAYOUT_CALLS.with(|n| n.get()),
            1,
            "current output contract was rechecked after successful native preparation"
        );
        let actual = backend.render_plan_to_rgba_bytes(next, 640, 480).unwrap();
        let mut cold = RealWgpuBackend::new_offscreen(640, 480).unwrap();
        assert_eq!(
            actual,
            cold.render_plan_to_rgba_bytes(next, 640, 480).unwrap()
        );
        cold.shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }
    let mut forged = selected.clone();
    let command = forged
        .commands
        .iter_mut()
        .find(|c| geometry_draw(c).is_some())
        .unwrap();
    if let RhiCommand::Draw {
        target,
        payload: RhiDrawPayload::Geometry { draw },
        ..
    } = command
    {
        *target = Arc::from("unregistered-secondary");
        Arc::make_mut(draw).target = target.clone();
    }
    assert!(backend.validate_graph_capabilities(&forged).is_err());
    let mut changed_schedule = selected.clone();
    let schedule = Arc::make_mut(changed_schedule.graph_schedule.as_mut().unwrap());
    schedule.resources[0].format = crate::render_graph::RenderResourceFormat::Depth24Plus;
    assert_eq!(
        backend.validate_graph_capabilities(&changed_schedule),
        Err("graph.unsupported_backend_depth_format".into())
    );
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
}

#[test]
#[ignore = "requires a real GPU"]
fn selection_publication_consumes_the_admitted_source_closure_without_a_second_merge() {
    let mut plan = unique_plan(384);
    moving_views::CAPTURE_DRAW_VISITS.with(|n| n.set(0));
    let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
    backend
        .prepare_geometry_plan_for_extent(&plan, [640, 480], None)
        .unwrap();
    assert_eq!(
        moving_views::CAPTURE_DRAW_VISITS.with(|n| n.get()),
        384,
        "full cold preparation must exercise the capture counter"
    );
    moving_views::CAPTURE_DRAW_VISITS.with(|n| n.set(0));
    let removed = plan
        .commands
        .iter()
        .position(|c| geometry_draw(c).is_some())
        .unwrap();
    plan.commands.remove(removed);
    let command = plan
        .commands
        .iter_mut()
        .find(|c| geometry_draw(c).is_some())
        .unwrap();
    let RhiCommand::Draw {
        payload: RhiDrawPayload::Geometry { draw },
        ..
    } = command
    else {
        panic!()
    };
    let detached = Arc::new(
        serde_json::from_value::<GeometryBuffers>(
            serde_json::to_value(draw.geometry.as_ref()).unwrap(),
        )
        .unwrap(),
    );
    assert_eq!(draw.geometry, detached);
    assert!(!Arc::ptr_eq(&draw.geometry, &detached));
    let handle = draw.geometry_handle().raw();
    Arc::make_mut(draw).geometry = detached.clone();
    residency::SOURCE_PUBLICATION_REVISITS.with(|n| n.set(0));
    backend
        .prepare_geometry_plan_for_extent(&plan, [640, 480], None)
        .unwrap();
    assert_eq!(
        residency::SOURCE_PUBLICATION_REVISITS.with(|n| n.get()),
        0,
        "successful selection merged the complete source closure again"
    );
    assert_eq!(
        moving_views::CAPTURE_DRAW_VISITS.with(|n| n.get()),
        0,
        "successful selection recaptured all admitted draws"
    );
    let cached = backend.geometry.validated_plan.as_ref().unwrap();
    assert_eq!(
        cached.meshes.as_ptr() as usize,
        residency::SOURCE_PUBLICATION_ALLOCATION.with(|p| p.get()),
        "publication replaced the admitted ordered source allocation"
    );
    let published = cached.meshes.iter().find(|(h, _)| *h == handle).unwrap();
    assert!(
        Arc::ptr_eq(&published.1, &detached),
        "publication retained a detached previous source witness"
    );
    let actual: Vec<_> = cached.meshes.iter().map(|(h, _)| *h).collect();
    let expected: Vec<_> = backend.geometry.meshes.keys().copied().collect();
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), 383);
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
}
