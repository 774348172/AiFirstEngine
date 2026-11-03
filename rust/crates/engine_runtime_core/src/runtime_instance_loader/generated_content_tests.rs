use super::*;
use crate::archetype::ComponentValue;
use crate::components::Hierarchy;
use crate::runtime_asset::{RuntimeAssetIndex, RuntimePackageMountTable};
use crate::spatial_contract::Transform3D;

fn setup() -> (RuntimeInstanceLoader, World) {
    let loader = RuntimeInstanceLoader::new(RuntimeAssetLoader::new(
        "unused",
        RuntimeAssetIndex::new(
            vec![serde_json::from_value(serde_json::json!({
                "assetGuid":"material-guid", "assetId":"surface-material", "type":"material",
                "cookedAssetId":"surface-material", "bundleId":"fixture", "loaderKind":"material"
            }))
            .unwrap()],
            vec![],
        ),
        RuntimePackageMountTable::new(vec![]),
    ));
    let mut world = World::new();
    spawn(&mut world, "surface");
    (loader, world)
}
fn spawn(world: &mut World, id: &str) {
    world.commit_prepared_entity(
        id.into(),
        id.into(),
        "spatial".into(),
        true,
        Hierarchy {
            parent_id: None,
            sibling_order: 0,
        },
        vec![ComponentValue::Transform3D(
            Transform3D::identity().canonicalize().unwrap(),
        )],
    );
}
fn replace(world: &World, id: u64) -> GeneratedContentRequest {
    let geometry = GeometryBuilder3D::new(&[[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]])
        .triangles_u16(
            &[0, 1, 2],
            vec![GeometrySection3D {
                first_index: 0,
                index_count: 3,
                material_slot: "surface".into(),
                material: project_runtime_sdk::ProjectRuntimeSpatialAssetRef {
                    id: "surface-material".into(),
                    asset_type: "material".into(),
                    guid: Some("material-guid".into()),
                    sub_asset: None,
                },
            }],
        )
        .unwrap();
    GeneratedContentRequest {
        request_id: id,
        target: ProjectRuntimeCellEntity {
            entity_id: "surface".into(),
            generation: world.entity_generation(&"surface".into()).unwrap(),
        },
        expected_applied_revision: 0,
        operation: GeneratedOperation3D::Replace {
            geometry,
            collision: CollisionUpdate3D::Keep,
        },
    }
}
fn receipt(loader: &RuntimeInstanceLoader, ticket: ContentTicket) -> ContentReceipt {
    loader.generated_content_receipt(ticket).unwrap()
}

#[test]
fn generated_content_376_admission_profile_asset_limit_rejects_without_superseding() {
    let (mut loader, world) = setup();
    let request = replace(&world, 1);
    let charge = request.payload_bytes();
    let a = loader.accept_generated_content(request, &world).unwrap();
    let before = receipt(&loader, a);
    let budget = loader.asset_loader.streaming_budget();
    loader
        .asset_loader
        .set_streaming_budget(crate::runtime_asset_loader::StreamingBudget {
            max_asset_bytes: charge - 1,
            ..budget
        })
        .unwrap();
    let failure = loader
        .accept_generated_content(replace(&world, 2), &world)
        .unwrap_err();
    assert_eq!(failure.code, ContentErrorCode::BudgetExceeded);
    assert_eq!(failure.field_path, "request.assetBytes");
    assert_eq!(receipt(&loader, a), before);
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        charge
    );
    loader.close_generated_content();
}

#[test]
fn generated_content_376_original_inflight_limit_rejects_peak_without_superseding() {
    let (mut loader, world) = setup();
    let request = replace(&world, 1);
    let charge = request.payload_bytes();
    let a = loader.accept_generated_content(request, &world).unwrap();
    let budget = loader.asset_loader.streaming_budget();
    loader
        .asset_loader
        .set_streaming_budget(crate::runtime_asset_loader::StreamingBudget {
            max_asset_bytes: charge,
            max_in_flight_bytes: charge * 2 - 1,
            ..budget
        })
        .unwrap();
    let before = receipt(&loader, a);
    let failure = loader
        .accept_generated_content(replace(&world, 2), &world)
        .unwrap_err();
    assert_eq!(failure.code, ContentErrorCode::BudgetExceeded);
    assert_eq!(failure.field_path, "cpu.inFlightBytes");
    assert_eq!(receipt(&loader, a), before);
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        charge
    );
    loader.close_generated_content();
}

#[test]
fn generated_content_376_actual_worker_keeps_owned_bytes_and_charge_until_join() {
    use std::sync::mpsc;
    use std::time::Duration;
    let (mut loader, world) = setup();
    let request = replace(&world, 1);
    let charge = request.payload_bytes();
    let ticket = loader.accept_generated_content(request, &world).unwrap();
    let work = loader.begin_generated_content(ticket, &world).unwrap();
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        entered_tx.send(work.request.payload_bytes()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(work.request.payload_bytes(), charge);
        drop(work);
    });
    assert_eq!(
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        charge
    );
    loader.cancel_generated_content(ticket).unwrap();
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        charge
    );
    assert!(loader
        .validate_generated_content_commit(ticket, &world)
        .is_err());
    release_tx.send(()).unwrap();
    worker.join().unwrap();
    assert!(!loader.generated_content_worker_finished(ticket).unwrap());
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        0
    );
}

#[test]
fn generated_content_376_conflicting_material_guid_id_rejects_without_altering_pending() {
    let (mut loader, world) = setup();
    let a = loader
        .accept_generated_content(replace(&world, 1), &world)
        .unwrap();
    let before = receipt(&loader, a);
    let mut bad = replace(&world, 2);
    if let GeneratedOperation3D::Replace { geometry, .. } = &mut bad.operation {
        geometry.sections[0].material.id = "different-id-same-guid".into();
    }
    assert_eq!(
        loader
            .accept_generated_content(bad, &world)
            .unwrap_err()
            .field_path,
        "assetRef.identity"
    );
    assert_eq!(receipt(&loader, a), before);
    loader.close_generated_content();
    assert!(loader.generated_content.bindings.is_empty());
}

#[test]
fn generated_content_376_query_distinguishes_target_state_ticket_and_structured_expiry() {
    let (mut loader, world) = setup();
    let request = replace(&world, 1);
    let target = request.target.clone();
    let ticket = loader.accept_generated_content(request, &world).unwrap();
    let ContentQueryValue::Target { state } = loader
        .query_generated_content(ContentQuery::Target { target }, &world)
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(state.applied_revision, 0);
    assert_eq!(state.pending_revision, Some(1));
    let ContentQueryValue::Receipt { receipt: r } = loader
        .query_generated_content(ContentQuery::Request { request_id: 1 }, &world)
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(r.ticket, ticket);
    assert_eq!(
        loader
            .query_generated_content(
                ContentQuery::Ticket {
                    ticket: ContentTicket {
                        session_generation: ticket.session_generation + 1,
                        ..ticket
                    }
                },
                &world
            )
            .unwrap_err()
            .code,
        ContentErrorCode::StaleSession
    );
    assert_eq!(
        loader
            .query_generated_content(ContentQuery::Request { request_id: 2 }, &world)
            .unwrap_err()
            .code,
        ContentErrorCode::NotFound
    );
    assert!(
        serde_json::to_vec(&r).unwrap().len() < 1024,
        "query must not contain geometry bytes"
    );
}

#[test]
fn generated_content_376_external_binding_change_invalidates_prepared_candidate() {
    use crate::spatial_contract::{MeshRenderer3D, SpatialAssetRef};
    let (mut loader, mut world) = setup();
    let a = loader
        .accept_generated_content(replace(&world, 1), &world)
        .unwrap();
    let work = loader.begin_generated_content(a, &world).unwrap();
    world.insert_component_value(
        "surface".into(),
        ComponentValue::MeshRenderer3D(MeshRenderer3D {
            schema_version: "mesh-renderer3d.v1".into(),
            mesh: SpatialAssetRef(RuntimeAssetRef {
                id: "external".into(),
                asset_type: "mesh".into(),
                guid: Some("external-guid".into()),
                sub_asset: None,
            }),
            materials: BTreeMap::new(),
            visible: true,
            layers: vec!["world".into()],
        }),
    );
    assert_eq!(
        loader
            .validate_generated_content_commit(a, &world)
            .unwrap_err()
            .field_path,
        "target.binding"
    );
    drop(work);
    loader.generated_content_worker_finished(a).unwrap();
    loader.cancel_generated_content(a).unwrap();
}

#[test]
fn generated_content_376_admission_moves_owned_input_without_claiming_ready_or_applied() {
    let (mut loader, world) = setup();
    let request = replace(&world, 1);
    let pointer = match &request.operation {
        GeneratedOperation3D::Replace { geometry, .. } => geometry.vertex_bytes.as_ptr(),
        _ => unreachable!(),
    };
    let charge = request.payload_bytes();
    let ticket = loader.accept_generated_content(request, &world).unwrap();
    assert_eq!(receipt(&loader, ticket).status, ContentStatus::Accepted);
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        charge
    );
    let work = loader.begin_generated_content(ticket, &world).unwrap();
    assert_eq!(work.ticket, ticket);
    assert_eq!(
        work.candidate_revision,
        receipt(&loader, ticket).candidate_revision
    );
    let GeneratedOperation3D::Replace { geometry, .. } = &work.request.operation else {
        panic!()
    };
    assert_eq!(geometry.vertex_bytes.as_ptr(), pointer);
    assert_eq!(receipt(&loader, ticket).status, ContentStatus::Preparing);
    assert_eq!(receipt(&loader, ticket).applied_revision, 0);
    assert!(loader.begin_generated_content(ticket, &world).is_err());
    drop(work);
    assert!(loader.generated_content_worker_finished(ticket).unwrap());
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        charge,
        "prepared candidate still needs its original reservation"
    );
    loader.cancel_generated_content(ticket).unwrap();
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        0
    );
}

#[test]
fn generated_content_376_rejected_candidate_does_not_supersede_previous_admission() {
    let (mut loader, world) = setup();
    let first = replace(&world, 1);
    let charge = first.payload_bytes();
    let ticket = loader.accept_generated_content(first, &world).unwrap();
    let work = loader.begin_generated_content(ticket, &world).unwrap();
    let old = receipt(&loader, ticket);
    let budget = loader.asset_loader.streaming_budget();
    loader
        .asset_loader
        .set_streaming_budget(crate::runtime_asset_loader::StreamingBudget {
            max_asset_bytes: charge,
            max_cpu_resident_bytes: charge * 2 - 1,
            ..budget
        })
        .unwrap();
    let error = loader
        .accept_generated_content(replace(&world, 2), &world)
        .unwrap_err();
    assert_eq!(error.code, ContentErrorCode::BudgetExceeded);
    assert_eq!(error.field_path, "cpu.preparedBytes");
    assert_eq!(receipt(&loader, ticket), old);
    assert!(loader
        .validate_generated_content_commit(ticket, &world)
        .is_ok());
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        charge
    );
    drop(work);
    loader.generated_content_worker_finished(ticket).unwrap();
    loader.cancel_generated_content(ticket).unwrap();
}

#[test]
fn generated_content_376_only_latest_admitted_candidate_can_pass_commit_guard_in_both_orders() {
    for older_finishes_first in [true, false] {
        let (mut loader, world) = setup();
        let a = loader
            .accept_generated_content(replace(&world, 1), &world)
            .unwrap();
        let work_a = loader.begin_generated_content(a, &world).unwrap();
        let b = loader
            .accept_generated_content(replace(&world, 2), &world)
            .unwrap();
        let work_b = loader.begin_generated_content(b, &world).unwrap();
        assert_eq!(receipt(&loader, a).status, ContentStatus::Superseded);
        assert_eq!(
            receipt(&loader, a).pending_revision,
            Some(work_b.candidate_revision)
        );
        assert_eq!(
            loader
                .validate_generated_content_commit(a, &world)
                .unwrap_err()
                .code,
            ContentErrorCode::RevisionConflict
        );
        assert!(loader.validate_generated_content_commit(b, &world).is_ok());
        let (first, second) = if older_finishes_first {
            (work_a, work_b)
        } else {
            (work_b, work_a)
        };
        let first_ticket = first.ticket;
        drop(first);
        loader
            .generated_content_worker_finished(first_ticket)
            .unwrap();
        assert!(loader.validate_generated_content_commit(a, &world).is_err());
        assert!(loader.validate_generated_content_commit(b, &world).is_ok());
        let second_ticket = second.ticket;
        drop(second);
        loader
            .generated_content_worker_finished(second_ticket)
            .unwrap();
        loader.cancel_generated_content(b).unwrap();
        assert_eq!(
            loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
            0
        );
    }
}

#[test]
fn generated_content_376_cancellation_and_session_close_pin_actual_worker_reservations() {
    let (mut loader, world) = setup();
    let a = loader
        .accept_generated_content(replace(&world, 1), &world)
        .unwrap();
    let work = loader.begin_generated_content(a, &world).unwrap();
    let charge = loader.asset_loader.streaming_snapshot().cpu_resident_bytes;
    loader.cancel_generated_content(a).unwrap();
    loader.cancel_generated_content(a).unwrap();
    assert_eq!(receipt(&loader, a).status, ContentStatus::Cancelled);
    assert_eq!(receipt(&loader, a).pending_revision, None);
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        charge
    );
    assert!(loader.validate_generated_content_commit(a, &world).is_err());
    loader.close_generated_content();
    assert_eq!(
        loader
            .accept_generated_content(replace(&world, 2), &world)
            .unwrap_err()
            .code,
        ContentErrorCode::StaleSession
    );
    drop(work);
    assert!(!loader.generated_content_worker_finished(a).unwrap());
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        0
    );
}

#[test]
fn generated_content_376_live_target_is_rechecked_at_dispatch_and_commit_after_reincarnation() {
    for before_dispatch in [true, false] {
        let (mut loader, mut world) = setup();
        let ticket = loader
            .accept_generated_content(replace(&world, 1), &world)
            .unwrap();
        let work =
            (!before_dispatch).then(|| loader.begin_generated_content(ticket, &world).unwrap());
        world.try_despawn_entity(&"surface".into()).unwrap();
        spawn(&mut world, "surface");
        if before_dispatch {
            assert_eq!(
                loader
                    .begin_generated_content(ticket, &world)
                    .unwrap_err()
                    .code,
                ContentErrorCode::StaleTarget
            );
            assert_eq!(receipt(&loader, ticket).status, ContentStatus::Failed);
            assert_eq!(receipt(&loader, ticket).error.unwrap().field_path, "target");
        } else {
            assert_eq!(
                loader
                    .validate_generated_content_commit(ticket, &world)
                    .unwrap_err()
                    .code,
                ContentErrorCode::StaleTarget
            );
            loader.cancel_generated_content(ticket).unwrap();
            drop(work);
            loader.generated_content_worker_finished(ticket).unwrap();
        }
        assert_eq!(
            loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
            0
        );
    }
}

#[test]
fn generated_content_376_active_window_cannot_evict_and_terminal_window_expires_old_tickets() {
    let (mut loader, mut world) = setup();
    let budget = loader.asset_loader.streaming_budget();
    loader
        .asset_loader
        .set_streaming_budget(crate::runtime_asset_loader::StreamingBudget {
            max_operations: 128,
            ..budget
        })
        .unwrap();
    let mut tickets = Vec::new();
    for n in 1..=128 {
        let name = format!("surface-{n}");
        spawn(&mut world, &name);
        let mut request = replace(&world, n);
        request.target.entity_id = name.clone();
        request.target.generation = world.entity_generation(&name.into()).unwrap();
        tickets.push(loader.accept_generated_content(request, &world).unwrap());
    }
    assert_eq!(
        loader
            .accept_generated_content(replace(&world, 129), &world)
            .unwrap_err()
            .code,
        ContentErrorCode::BudgetExceeded
    );
    assert_eq!(receipt(&loader, tickets[0]).status, ContentStatus::Accepted);
    loader.cancel_generated_content(tickets[0]).unwrap();
    let next = loader
        .accept_generated_content(replace(&world, 129), &world)
        .unwrap();
    assert_eq!(
        loader
            .generated_content_receipt(tickets[0])
            .unwrap_err()
            .code,
        ContentErrorCode::ReceiptExpired
    );
    assert_eq!(loader.generated_content.records.len(), 128);
    assert_eq!(receipt(&loader, next).status, ContentStatus::Accepted);
    loader.close_generated_content();
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        0
    );
}

#[test]
fn generated_content_376_session_generation_and_monotone_requests_are_host_owned() {
    let (mut a, world) = setup();
    let (b, _) = setup();
    let ticket = a
        .accept_generated_content(replace(&world, 4), &world)
        .unwrap();
    assert_eq!(
        b.generated_content_receipt(ticket).unwrap_err().code,
        ContentErrorCode::StaleSession
    );
    assert_eq!(
        a.accept_generated_content(replace(&world, 4), &world)
            .unwrap_err()
            .code,
        ContentErrorCode::InvalidArgument
    );
    let mut wrong_base = replace(&world, 5);
    wrong_base.expected_applied_revision = 99;
    let e = a.accept_generated_content(wrong_base, &world).unwrap_err();
    assert_eq!(e.code, ContentErrorCode::RevisionConflict);
    assert_eq!((e.expected, e.actual), (Some(0), Some(99)));
    assert_eq!(receipt(&a, ticket).status, ContentStatus::Accepted);
}

#[test]
fn generated_content_376_patch_requires_exact_applied_layout_without_pending_candidate() {
    let (mut loader, world) = setup();
    let mut patch = replace(&world, 1);
    patch.operation = GeneratedOperation3D::UpdateAttributes {
        patch: AttributePatch3D::positions(0, &[[3., 0., 0.]]).unwrap(),
    };
    assert_eq!(
        loader
            .accept_generated_content(patch.clone(), &world)
            .unwrap_err()
            .code,
        ContentErrorCode::RevisionConflict
    );
    // State-machine fixture for an already committed binding. It is not a GPU readiness proof.
    loader.generated_content.bindings.insert(
        target_key(&patch.target),
        Binding {
            applied: 10,
            layout: Some(Layout {
                vertex_count: 3,
                vertex_stride: 12,
                attributes: vec![AttributeLayout3D {
                    semantic: Attribute3D::Position,
                    offset: 0,
                }],
            }),
            ..Binding::default()
        },
    );
    patch.expected_applied_revision = 10;
    let accepted = loader
        .accept_generated_content(patch.clone(), &world)
        .unwrap();
    patch.request_id = 2;
    assert_eq!(
        loader
            .accept_generated_content(patch.clone(), &world)
            .unwrap_err()
            .code,
        ContentErrorCode::PendingUpdate
    );
    loader.cancel_generated_content(accepted).unwrap();
    patch.operation = GeneratedOperation3D::UpdateAttributes {
        patch: AttributePatch3D::uv(0, &[[0., 0.]]).unwrap(),
    };
    assert_eq!(
        loader
            .accept_generated_content(patch, &world)
            .unwrap_err()
            .field_path,
        "patch.rangeOrAttribute"
    );
}

#[test]
fn generated_content_376_applied_cancellation_is_explicit_and_commit_rechecks_base() {
    let (mut loader, world) = setup();
    let ticket = loader
        .accept_generated_content(replace(&world, 1), &world)
        .unwrap();
    let work = loader.begin_generated_content(ticket, &world).unwrap();
    let key = target_key(&work.request.target);
    loader
        .generated_content
        .bindings
        .get_mut(&key)
        .unwrap()
        .applied = 99;
    assert_eq!(
        loader
            .validate_generated_content_commit(ticket, &world)
            .unwrap_err()
            .field_path,
        "expectedAppliedRevision"
    );
    // Exercise Applied -> cancel on state only, without constructing a fake backend receipt.
    loader
        .generated_content
        .records
        .get_mut(&ticket.request_id)
        .unwrap()
        .receipt
        .status = ContentStatus::Applied;
    assert_eq!(
        loader.cancel_generated_content(ticket).unwrap_err().code,
        ContentErrorCode::AlreadyApplied
    );
    assert_eq!(
        receipt(&loader, ticket).applied_revision,
        0,
        "no metadata publication occurred"
    );
}

#[test]
fn generated_content_376_mesh_patch_reserves_full_vertex_candidate_before_admission() {
    let (mut loader, world) = setup();
    let mut request = replace(&world, 1);
    request.operation = GeneratedOperation3D::UpdateAttributes {
        patch: AttributePatch3D::positions(0, &[[2., 0., 0.]]).unwrap(),
    };
    loader.generated_content.bindings.insert(
        target_key(&request.target),
        Binding {
            applied: 5,
            layout: Some(Layout {
                vertex_count: 3,
                vertex_stride: 12,
                attributes: vec![AttributeLayout3D {
                    semantic: Attribute3D::Position,
                    offset: 0,
                }],
            }),
            ..Binding::default()
        },
    );
    request.expected_applied_revision = 5;
    let budget = loader.asset_loader.streaming_budget();
    loader
        .asset_loader
        .set_streaming_budget(crate::runtime_asset_loader::StreamingBudget {
            max_in_flight_bytes: 12 + 36 - 1,
            max_asset_bytes: 12,
            ..budget
        })
        .unwrap();
    let failure = loader
        .accept_generated_content(request, &world)
        .unwrap_err();
    assert_eq!(failure.code, ContentErrorCode::BudgetExceeded);
    assert_eq!(failure.field_path, "cpu.inFlightBytes");
    let binding = loader.generated_content.bindings.values().next().unwrap();
    assert_eq!(binding.applied, 5);
    assert_eq!(binding.pending, None);
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        0
    );
}

fn finish_cpu(loader: &mut RuntimeInstanceLoader, world: &World) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while loader.generated_content.worker.is_some() {
        assert!(
            std::time::Instant::now() < deadline,
            "CPU worker failed to complete"
        );
        loader.advance_generated_content_cpu(world);
        std::thread::yield_now();
    }
}

#[test]
fn generated_content_376_mesh_cpu_lane_moves_backing_and_retains_preparing_until_native_commit() {
    let (mut loader, world) = setup();
    let request = replace(&world, 1);
    let bytes = request.payload_bytes();
    let pointer = match &request.operation {
        GeneratedOperation3D::Replace { geometry, .. } => geometry.vertex_bytes.as_ptr(),
        _ => unreachable!(),
    };
    let ticket = loader.accept_generated_content(request, &world).unwrap();
    loader.advance_generated_content_cpu(&world);
    finish_cpu(&mut loader, &world);
    let record = &loader.generated_content.records[&ticket.request_id];
    assert_eq!(record.receipt.status, ContentStatus::Preparing);
    assert_eq!(record.receipt.applied_revision, 0);
    assert_eq!(record.receipt.commit_epoch, None);
    let candidate = record.candidate.as_ref().unwrap();
    assert_eq!(
        candidate.geometry.as_ref().unwrap().vertex_pointer(),
        pointer
    );
    assert!(matches!(candidate.collision, Some(CollisionUpdate3D::Keep)));
    assert!(!candidate.release);
    assert_eq!(
        record.cpu_charge, 0,
        "geometry reservation transferred to its actual backing lifetime"
    );
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        bytes
    );
    assert!(world.mesh_renderer3d(&"surface".into()).is_none());
    loader.close_generated_content();
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        0
    );
}

#[test]
fn generated_content_376_mesh_actual_cpu_lane_cancel_panic_and_close_reclaim_only_after_join() {
    for panic in [false, true] {
        let (mut loader, world) = setup();
        let request = replace(&world, 1);
        let bytes = request.payload_bytes();
        let ticket = loader.accept_generated_content(request, &world).unwrap();
        let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(1);
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
        loader.generated_content.worker_gate = Some(cpu::TestGate {
            entered: entered_tx,
            release: release_rx,
            panic,
        });
        loader.advance_generated_content_cpu(&world);
        entered_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        if !panic {
            loader.cancel_generated_content(ticket).unwrap();
            assert_eq!(receipt(&loader, ticket).status, ContentStatus::Cancelled);
        }
        assert_eq!(
            loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
            bytes
        );
        release_tx.send(()).unwrap();
        finish_cpu(&mut loader, &world);
        let receipt = receipt(&loader, ticket);
        if panic {
            assert_eq!(receipt.status, ContentStatus::Failed);
            assert_eq!(receipt.error.unwrap().field_path, "cpu.workerPanicked");
        } else {
            assert_eq!(receipt.status, ContentStatus::Cancelled);
        }
        assert!(loader.generated_content.records[&ticket.request_id]
            .candidate
            .is_none());
        assert_eq!(
            loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
            0
        );
        loader.close_generated_content();
        assert!(loader.generated_content.worker.is_none());
    }
    let (mut loader, world) = setup();
    let ticket = loader
        .accept_generated_content(replace(&world, 1), &world)
        .unwrap();
    loader.advance_generated_content_cpu(&world);
    loader.close_generated_content();
    assert_eq!(receipt(&loader, ticket).status, ContentStatus::Cancelled);
    assert!(loader.generated_content.worker.is_none());
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        0
    );
}

#[test]
fn generated_content_376_mesh_cpu_lane_discards_late_result_for_destroyed_target() {
    let (mut loader, mut world) = setup();
    let ticket = loader
        .accept_generated_content(replace(&world, 1), &world)
        .unwrap();
    loader.advance_generated_content_cpu(&world);
    world.try_despawn_entity(&"surface".into()).unwrap();
    spawn(&mut world, "surface");
    finish_cpu(&mut loader, &world);
    let receipt = receipt(&loader, ticket);
    assert_eq!(receipt.status, ContentStatus::Failed);
    assert_eq!(receipt.error.unwrap().code, ContentErrorCode::StaleTarget);
    assert!(loader.generated_content.records[&ticket.request_id]
        .candidate
        .is_none());
    assert!(world.mesh_renderer3d(&"surface".into()).is_none());
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        0
    );
}

#[test]
fn generated_content_376_mesh_cpu_consumer_pins_actual_backing_after_cancel_until_last_reader_drops(
) {
    let (mut loader, world) = setup();
    let request = replace(&world, 1);
    let bytes = request.payload_bytes();
    let ticket = loader.accept_generated_content(request, &world).unwrap();
    loader.advance_generated_content_cpu(&world);
    finish_cpu(&mut loader, &world);
    let consumer = loader.generated_content.records[&ticket.request_id]
        .candidate
        .as_ref()
        .unwrap()
        .geometry
        .clone()
        .unwrap();
    loader.cancel_generated_content(ticket).unwrap();
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        bytes,
        "cancellation must not release a backing still held by an upload/frame reader"
    );
    drop(consumer);
    loader.advance_generated_content_cpu(&world);
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        0
    );
}

#[test]
fn generated_content_376_mesh_cpu_capacity_and_retained_backing_handles_apply_backpressure() {
    let (mut loader, world) = setup();
    let budget = loader.asset_loader.streaming_budget();
    loader
        .asset_loader
        .set_streaming_budget(crate::runtime_asset_loader::StreamingBudget {
            max_handles: 2,
            ..budget
        })
        .unwrap();
    let mut request = replace(&world, 1);
    let GeneratedOperation3D::Replace { geometry, .. } = &mut request.operation else {
        unreachable!()
    };
    geometry.vertex_bytes.reserve_exact(1024);
    let bytes = cpu::request_backing_bytes(&request);
    assert!(bytes > request.payload_bytes());
    let ticket = loader.accept_generated_content(request, &world).unwrap();
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        bytes
    );
    loader.advance_generated_content_cpu(&world);
    finish_cpu(&mut loader, &world);
    let reader = loader.generated_content.records[&ticket.request_id]
        .candidate
        .as_ref()
        .unwrap()
        .geometry
        .clone()
        .unwrap();
    loader.cancel_generated_content(ticket).unwrap();
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        bytes
    );
    let failure = loader
        .accept_generated_content(replace(&world, 2), &world)
        .unwrap_err();
    assert_eq!(failure.field_path, "cpu.backingHandles");
    assert_eq!(receipt(&loader, ticket).status, ContentStatus::Cancelled);
    drop(reader);
    // Admission itself reaps the retired backing; no special caller-driven drain is needed.
    let next = loader
        .accept_generated_content(replace(&world, 2), &world)
        .unwrap();
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        42
    );
    loader.cancel_generated_content(next).unwrap();
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        0
    );
}

#[test]
fn generated_content_376_mesh_expired_terminal_receipt_does_not_uncharge_its_live_reader() {
    let (mut loader, world) = setup();
    let first = loader
        .accept_generated_content(replace(&world, 1), &world)
        .unwrap();
    loader.advance_generated_content_cpu(&world);
    finish_cpu(&mut loader, &world);
    let reader = loader.generated_content.records[&1]
        .candidate
        .as_ref()
        .unwrap()
        .geometry
        .clone()
        .unwrap();
    loader.cancel_generated_content(first).unwrap();
    for id in 2..=130 {
        let mut request = replace(&world, id);
        request.operation = GeneratedOperation3D::Release;
        let ticket = loader.accept_generated_content(request, &world).unwrap();
        loader.cancel_generated_content(ticket).unwrap();
    }
    assert_eq!(
        loader.generated_content_receipt(first).unwrap_err().code,
        ContentErrorCode::ReceiptExpired
    );
    assert!(loader.generated_content.records.len() <= GENERATED_RECEIPT_LIMIT);
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        42
    );
    drop(reader);
    loader.advance_generated_content_cpu(&world);
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        0
    );
}

#[test]
fn generated_content_376_mesh_patch_shared_index_is_charged_once_through_reader_retirement() {
    let (mut loader, world) = setup();
    let request = replace(&world, 1);
    let target = request.target.clone();
    let ticket = loader.accept_generated_content(request, &world).unwrap();
    loader.advance_generated_content_cpu(&world);
    finish_cpu(&mut loader, &world);
    let canonical = loader
        .generated_content
        .records
        .get_mut(&ticket.request_id)
        .unwrap()
        .candidate
        .take()
        .unwrap()
        .geometry
        .unwrap();
    loader.cancel_generated_content(ticket).unwrap();
    // A CPU-only applied-base fixture exercises allocation ownership, without claiming a
    // native receipt or modifying World. Production publication is still unavailable.
    loader.generated_content.bindings.insert(
        target_key(&target),
        Binding {
            applied: 5,
            canonical: Some(canonical.clone()),
            layout: Some(Layout {
                vertex_count: 3,
                vertex_stride: 12,
                attributes: vec![AttributeLayout3D {
                    semantic: Attribute3D::Position,
                    offset: 0,
                }],
            }),
            ..Binding::default()
        },
    );
    let make_patch = || GeneratedContentRequest {
        request_id: 2,
        target: target.clone(),
        expected_applied_revision: 5,
        operation: GeneratedOperation3D::UpdateAttributes {
            patch: AttributePatch3D::positions(0, &[[2., 0., 0.]]).unwrap(),
        },
    };
    let patch_charge = cpu::request_backing_bytes(&make_patch());
    let peak = 42 + patch_charge + 36;
    let budget = loader.asset_loader.streaming_budget();
    loader
        .asset_loader
        .set_streaming_budget(crate::runtime_asset_loader::StreamingBudget {
            max_cpu_resident_bytes: peak - 1,
            max_asset_bytes: 42,
            max_in_flight_bytes: patch_charge + 36,
            ..budget
        })
        .unwrap();
    let failure = loader
        .accept_generated_content(make_patch(), &world)
        .unwrap_err();
    assert_eq!(failure.field_path, "cpu.preparedBytes");
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        42
    );
    assert_eq!(
        loader.generated_content.bindings[&target_key(&target)].pending,
        None
    );
    loader
        .asset_loader
        .set_streaming_budget(crate::runtime_asset_loader::StreamingBudget {
            max_cpu_resident_bytes: peak,
            max_asset_bytes: 42,
            max_in_flight_bytes: patch_charge + 36,
            ..budget
        })
        .unwrap();
    let patch = loader
        .accept_generated_content(make_patch(), &world)
        .unwrap();
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        peak
    );
    loader.advance_generated_content_cpu(&world);
    finish_cpu(&mut loader, &world);
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        78
    );
    let reader = loader.generated_content.records[&patch.request_id]
        .candidate
        .as_ref()
        .unwrap()
        .geometry
        .clone()
        .unwrap();
    let old_backings = canonical.cpu_backings();
    let new_backings = reader.cpu_backings();
    assert_ne!(old_backings[0].0, new_backings[0].0);
    assert_eq!(old_backings[1].0, new_backings[1].0);
    assert_eq!(loader.generated_content.cpu_backings.len(), 3);
    loader.cancel_generated_content(patch).unwrap();
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        78
    );
    loader.close_generated_content();
    drop(canonical);
    loader.advance_generated_content_cpu(&world);
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        42
    );
    drop(reader);
    loader.advance_generated_content_cpu(&world);
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        0
    );
}

#[test]
fn generated_content_376_lifecycle_same_entity_generation_in_another_world_cannot_reuse_session() {
    let (mut loader, world) = setup();
    let ticket = loader
        .accept_generated_content(replace(&world, 1), &world)
        .unwrap();
    let mut another = World::new();
    spawn(&mut another, "surface");
    assert_eq!(
        world.entity_generation(&"surface".into()),
        another.entity_generation(&"surface".into())
    );
    assert_ne!(world.epoch(), another.epoch());
    assert_eq!(
        loader
            .query_generated_content(ContentQuery::Ticket { ticket }, &another)
            .unwrap_err()
            .code,
        ContentErrorCode::StaleSession
    );
    assert_eq!(
        loader
            .begin_generated_content(ticket, &another)
            .unwrap_err()
            .code,
        ContentErrorCode::StaleSession
    );
    loader.advance_generated_content_cpu(&another);
    assert_eq!(receipt(&loader, ticket).status, ContentStatus::Cancelled);
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        0
    );
}

struct FrameFixture {
    package: crate::runtime_package::RuntimePackage,
    root: std::path::PathBuf,
    parent: std::path::PathBuf,
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires exclusive real GPU; original upload source pins the actual CPU backing"]
fn generated_content_376_mesh_native_source_keeps_cpu_reservation_after_request_cancel() {
    use crate::render_geometry::generated::binding_tests::{binding, fixture, geometry};
    use crate::wgpu_backend::real::RealWgpuBackend;
    let (mut loader, world) = setup();
    let (mut manager, material, _) = fixture();
    let mut request = replace(&world, 1);
    let GeneratedOperation3D::Replace { geometry: data, .. } = &mut request.operation else {
        unreachable!()
    };
    *data = geometry(3, [0., 0., 1.], [1.; 4]);
    data.sections[0].material.id = "surface-material".into();
    let bytes = cpu::request_backing_bytes(&request);
    let ticket = loader.accept_generated_content(request, &world).unwrap();
    loader.advance_generated_content_cpu(&world);
    finish_cpu(&mut loader, &world);
    let generated = loader.generated_content.records[&ticket.request_id]
        .candidate
        .as_ref()
        .unwrap()
        .geometry
        .clone()
        .unwrap();
    let (binding, provenance) = binding(&mut manager, &generated, 1);
    let prepared = generated
        .bind(0, binding, provenance, &material, &manager)
        .unwrap();
    let mut backend = RealWgpuBackend::new_offscreen(16, 16).unwrap();
    let upload = backend
        .begin_geometry_upload(std::slice::from_ref(&prepared), &[])
        .unwrap();
    drop(prepared);
    drop(generated);
    loader.cancel_generated_content(ticket).unwrap();
    loader.advance_generated_content_cpu(&world);
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        bytes,
        "only the original backend upload source still holds the backing"
    );
    assert!(backend.geometry_upload_snapshot().pending_resources > 0);
    backend.release_geometry_upload(upload).unwrap();
    let drained = backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
    assert!(!drained.abandoned);
    assert_eq!(drained.submitted, drained.completed);
    assert_eq!(
        (
            drained.in_flight,
            drained.retiring_objects,
            drained.retiring_bytes
        ),
        (0, 0, 0)
    );
    loader.advance_generated_content_cpu(&world);
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        0
    );
    let pending = backend.geometry_upload_snapshot();
    assert_eq!(
        (
            pending.tickets,
            pending.pending_resources,
            pending.staging_bytes
        ),
        (0, 0, 0)
    );
    eprintln!("376 actual native upload source pinned {bytes} CPU bytes after cancellation and drained to zero.");
}

#[test]
fn generated_content_376_ingress_original_commit_preflight_and_nonterminal_stage_diagnostics() {
    use crate::gameplay_command::RuntimeCommandContext;
    use crate::project_runtime_session::*;
    let fixture = frame_fixture();
    let (mut loader, mut world) = setup();
    let original = loader
        .accept_generated_content(replace(&world, 1), &world)
        .unwrap();
    let make = |world: &World, valid: bool| {
        let mut buffer = ProjectRuntimeMutationBuffer::new();
        buffer.spatial_command(
            "surface".into(),
            world.entity_generation(&"surface".into()).unwrap() + u64::from(!valid),
            project_runtime_sdk::ProjectRuntimeSpatialIntent::SetLocalTransform {
                transform: project_runtime_sdk::ProjectRuntimeTransform3D {
                    translation: [2., 3., 4.],
                    ..project_runtime_sdk::ProjectRuntimeTransform3D::identity()
                },
            },
        );
        buffer.generated_content_request(replace(world, 2));
        buffer
    };
    let before = world.transform3d(&"surface".into()).cloned();
    let error = make(&world, false)
        .commit_with_runtime(
            &mut world,
            Some(RuntimeCommandContext {
                package: &fixture.package,
                instance_loader: &mut loader,
            }),
        )
        .unwrap_err();
    assert_eq!(
        (
            error.report.staged_count,
            error.report.committed_count,
            error.report.rejected_count
        ),
        (2, 0, 2)
    );
    assert!(
        error.report.generated_content_errors.is_empty(),
        "no async admission after synchronous preflight failure"
    );
    assert_eq!(world.transform3d(&"surface".into()).cloned(), before);
    assert_eq!(receipt(&loader, original).status, ContentStatus::Accepted);

    struct Session {
        output: Option<ProjectRuntimeMutationBuffer>,
    }
    impl ProjectRuntimeSession for Session {
        fn session_id(&self) -> &str {
            "376-ingress-consumer"
        }
        fn handle_aui_actions(
            &mut self,
            _: ProjectRuntimeSessionContext<'_>,
            _: ProjectAuiActionBatch<'_>,
        ) -> ProjectRuntimeSessionOutput {
            ProjectRuntimeSessionOutput::no_op()
        }
        fn fixed_update(
            &mut self,
            _: ProjectRuntimeSessionContext<'_>,
        ) -> ProjectRuntimeSessionOutput {
            self.output
                .take()
                .map(ProjectRuntimeSessionOutput::applied)
                .unwrap_or_else(ProjectRuntimeSessionOutput::no_op)
        }
    }
    let mut session = Session {
        output: Some(make(&world, true)),
    };
    let mut animator = vec![];
    let mut audio = vec![];
    let mut particles = vec![];
    let mut control = None;
    for tick in [1, 2] {
        let report = execute_project_runtime_session_stage_with_media(
            &mut session,
            ProjectRuntimeSessionStage::FixedUpdate,
            tick,
            crate::runtime_time::TimeContext::from_delta(tick, 1. / 60., true),
            &mut world,
            &[],
            ProjectRuntimeSessionReportLevel::Summary,
            &mut animator,
            &mut audio,
            &mut particles,
            &mut control,
            Some(RuntimeCommandContext {
                package: &fixture.package,
                instance_loader: &mut loader,
            }),
        );
        assert!(!report.terminal_fault);
        if tick == 1 {
            assert_eq!(
                (
                    report.staged_mutation_count,
                    report.committed_mutation_count,
                    report.rejected_mutation_count
                ),
                if cfg!(feature = "real-wgpu") {
                    (2, 2, 0)
                } else {
                    (2, 1, 1)
                }
            );
            if cfg!(feature = "real-wgpu") {
                assert!(report.diagnostics.is_empty());
            } else {
                assert!(report.diagnostics.iter().any(
                    |message| message.contains("capability.generatedContent.nativePublication")
                ));
            }
            assert_eq!(
                world.transform3d(&"surface".into()).unwrap().translation,
                [2., 3., 4.]
            );
        } else {
            assert_eq!(report.status, ProjectRuntimeSessionStatus::NoOp);
            assert_eq!(report.staged_mutation_count, 0);
        }
    }
    assert_eq!(
        receipt(&loader, original).status,
        if cfg!(feature = "real-wgpu") {
            ContentStatus::Superseded
        } else {
            ContentStatus::Accepted
        }
    );
    assert_eq!(
        loader.generated_content.last_request,
        if cfg!(feature = "real-wgpu") { 2 } else { 1 }
    );
    let current = if cfg!(feature = "real-wgpu") {
        let ContentQueryValue::Receipt { receipt } = loader
            .query_generated_content(ContentQuery::Request { request_id: 2 }, &world)
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(receipt.status, ContentStatus::Accepted);
        receipt.ticket
    } else {
        original
    };
    let mut cancel = ProjectRuntimeMutationBuffer::new();
    cancel.generated_content_cancel(current);
    let (report, _) = cancel
        .commit_with_runtime(
            &mut world,
            Some(RuntimeCommandContext {
                package: &fixture.package,
                instance_loader: &mut loader,
            }),
        )
        .unwrap();
    assert_eq!(
        (
            report.staged_count,
            report.committed_count,
            report.rejected_count
        ),
        (1, 1, 0)
    );
    assert_eq!(receipt(&loader, current).status, ContentStatus::Cancelled);
}
#[test]
fn generated_content_376_frame_fixture_cleans_its_publish_lock() {
    let fixture = frame_fixture();
    let root = fixture.root.clone();
    let lock = fixture.parent.join(format!(
        ".{}.publish.lock",
        root.file_name().unwrap().to_string_lossy()
    ));
    assert!(root.is_dir());
    assert!(lock.is_file());
    drop(fixture);
    assert!(!root.exists());
    assert!(
        !lock.exists(),
        "the completed test must remove its own publish lock"
    );
}

#[test]
fn generated_content_376_report_levels_keep_large_pending_payload_out_of_reports() {
    use crate::aui::{AuiAction, AuiActionEvent};
    use crate::project_runtime_session::*;
    struct Session;
    impl ProjectRuntimeSession for Session {
        fn session_id(&self) -> &str {
            "376-report-work"
        }
        fn handle_aui_actions(
            &mut self,
            _: ProjectRuntimeSessionContext<'_>,
            _: ProjectAuiActionBatch<'_>,
        ) -> ProjectRuntimeSessionOutput {
            ProjectRuntimeSessionOutput::no_op()
        }
        fn fixed_update(
            &mut self,
            context: ProjectRuntimeSessionContext<'_>,
        ) -> ProjectRuntimeSessionOutput {
            let ContentQueryValue::Receipt { receipt } = context
                .world
                .query_generated_content(ContentQuery::Request { request_id: 1 })
                .unwrap()
            else {
                panic!()
            };
            assert_eq!(receipt.status, ContentStatus::Accepted);
            assert_eq!(receipt.pending_revision, Some(1));
            assert!(serde_json::to_vec(&receipt).unwrap().len() < 1024);
            ProjectRuntimeSessionOutput::no_op()
        }
    }
    let fixture = frame_fixture();
    let (mut loader, mut world) = setup();
    let mut request = replace(&world, 1);
    if let GeneratedOperation3D::Replace { geometry, .. } = &mut request.operation {
        let mut vertices = vec![[0., 0., 0.]; 100_000];
        vertices[1] = [1., 0., 0.];
        vertices[2] = [0., 1., 0.];
        *geometry = GeometryBuilder3D::new(&vertices)
            .triangles_u16(&[0, 1, 2], geometry.sections.clone())
            .unwrap();
    }
    let bytes = request.payload_bytes();
    assert!(bytes > 1_000_000);
    let ticket = loader.accept_generated_content(request, &world).unwrap();
    let resident = loader.asset_loader.streaming_snapshot().cpu_resident_bytes;
    let actions = vec![AuiAction {
        action_id: "test.action".into(),
        node_id: "test.node".into(),
        event: AuiActionEvent::Click,
        payload: Some("x".repeat(1_000_000)),
    }];
    for level in [
        ProjectRuntimeSessionReportLevel::Off,
        ProjectRuntimeSessionReportLevel::Summary,
        ProjectRuntimeSessionReportLevel::Trace,
    ] {
        report_work::PAYLOAD_BYTES_HASHED.with(|n| n.set(0));
        report_work::CONTENT_USAGE_CALLS.with(|n| n.set(0));
        let report = execute_project_runtime_session_stage_with_media(
            &mut Session,
            ProjectRuntimeSessionStage::FixedUpdate,
            1,
            crate::runtime_time::TimeContext::from_delta(1, 1. / 60., true),
            &mut world,
            &actions,
            level,
            &mut vec![],
            &mut vec![],
            &mut vec![],
            &mut None,
            Some(crate::gameplay_command::RuntimeCommandContext {
                package: &fixture.package,
                instance_loader: &mut loader,
            }),
        );
        assert!(!report.terminal_fault);
        assert_eq!(
            report.action_trace.len(),
            usize::from(level == ProjectRuntimeSessionReportLevel::Trace)
        );
        assert_eq!(
            report_work::PAYLOAD_BYTES_HASHED.with(|n| n.get()),
            if level == ProjectRuntimeSessionReportLevel::Trace {
                1_000_000
            } else {
                0
            }
        );
        if level != ProjectRuntimeSessionReportLevel::Trace {
            assert_eq!(report.action_trace.capacity(), 0);
        }
        assert!(
            serde_json::to_vec(&report).unwrap().len() < 2048,
            "no vertex or action payload JSON in bounded stage report"
        );
        assert_eq!(
            loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
            resident
        );
        assert_eq!(receipt(&loader, ticket).status, ContentStatus::Accepted);
        assert_eq!(
            report_work::CONTENT_USAGE_CALLS.with(|n| n.get()),
            0,
            "normal session stages never invoke the opt-in census"
        );
        let mut sample = crate::resource_memory::ResourceMemorySample::new(1, 1, 0);
        loader.observe_memory(&mut sample);
        assert_eq!(
            sample.capacities["generatedContent.cpuChargedBytes"],
            resident as u64
        );
        assert_eq!(sample.capacities["generatedContent.receipts"], 1);
        assert_eq!(sample.capacities["generatedContent.pendingTargets"], 1);
        assert_eq!(report_work::CONTENT_USAGE_CALLS.with(|n| n.get()), 1);
        assert!(
            serde_json::to_vec(&sample).unwrap().len() < 20_000,
            "opt-in counters never serialize the pending geometry bytes"
        );
    }
    loader.close_generated_content();
}

impl Drop for FrameFixture {
    fn drop(&mut self) {
        // An exact process/test-owned child. Keep it on any ownership mismatch.
        let plain = |path: &std::path::Path| {
            std::fs::symlink_metadata(path).is_ok_and(|metadata| {
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    metadata.file_attributes() & 0x400 == 0
                }
                #[cfg(not(windows))]
                {
                    !metadata.file_type().is_symlink()
                }
            })
        };
        let name = self.root.file_name().unwrap().to_string_lossy();
        if self.root.parent() != Some(self.parent.as_path())
            || !name.starts_with(&format!("376-frame-{}-", std::process::id()))
            || !self.parent.ancestors().all(plain)
        {
            return;
        }
        if plain(&self.root) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
        // RuntimePackageBuilder has returned and released its publication guard.
        let lock = self.parent.join(format!(".{name}.publish.lock"));
        if plain(&lock) && lock.is_file() {
            let _ = std::fs::remove_file(lock);
        }
    }
}
fn frame_fixture() -> FrameFixture {
    frame_fixture_with_assets(vec![])
}
fn frame_fixture_with_assets(
    assets: Vec<crate::runtime_package_builder::RuntimePackageSourceAsset>,
) -> FrameFixture {
    frame_fixture_with_content(assets, vec![])
}
fn frame_fixture_with_content(
    assets: Vec<crate::runtime_package_builder::RuntimePackageSourceAsset>,
    textures: Vec<crate::runtime_package_builder::RuntimePackageSourceTexture>,
) -> FrameFixture {
    use crate::runtime_package::*;
    use crate::runtime_package_builder::*;
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let parent = std::env::var_os("AIFE_376_TEMP_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&parent).unwrap();
    let parent = parent.canonicalize().unwrap();
    let root = parent.join(format!(
        "376-frame-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    assert!(!root.exists());
    let mut input = RuntimePackageBuildInput::new(RuntimeProjectInfo::explicit_empty(
        "376-fixture",
        "376 frame fixture",
        "1",
    ));
    input.assets = assets;
    input.texture_payloads = textures;
    input.scenes.push(
        serde_json::from_value(serde_json::json!({
            "schemaVersion":"runtime-scene.v1", "id":"main", "name":"Main", "gravity":0,
            "background":"#000000", "skyColor":"#000000", "entities":[]
        }))
        .unwrap(),
    );
    let mapping = crate::input_mapping::InputMappingAsset::explicit_empty("input.none");
    input.input_mappings.push(RuntimePackageSourceJson {
        id: mapping.asset_id.clone(),
        document: serde_json::to_value(mapping).unwrap(),
    });
    let built = RuntimePackageBuilder::build(
        &RuntimePackageBuildRequest::dev_desktop(&root, "main"),
        &input,
    );
    assert_eq!(
        built.status,
        RuntimePackageBuildStatus::Success,
        "{:?}",
        built.diagnostics
    );
    let package = load_runtime_package(&root).value.unwrap();
    FrameFixture {
        package,
        root,
        parent,
    }
}

#[cfg(feature = "real-wgpu")]
#[path = "generated_content_native_tests.rs"]
mod native_tests;

#[test]
fn generated_content_376_mesh_original_package_material_preparation_is_transactional() {
    use crate::render_asset_production::RuntimeRenderAssetProducer;
    use crate::render_contract::*;
    use crate::render_geometry::generated::binding_tests::{fixture, geometry};
    let (_, material, _) = fixture();
    let bytes = serde_json::to_vec(&RenderContract::new(material.asset)).unwrap();
    let mut source = crate::runtime_package_builder::RuntimePackageSourceAsset::new(
        "surface-material",
        "Surface",
        "material",
        "Assets/surface-material.json",
        "cooked/surface-material.json",
    );
    source.asset_guid = Some("material-guid".into());
    source.hash = Some(crate::canonical_digest::sha256_prefixed(&bytes));
    let fixture = frame_fixture_with_assets(vec![source.with_runtime_payload(bytes)]);
    let mut loader = RuntimeAssetLoader::new(
        &fixture.root,
        fixture.package.runtime_asset_index.clone(),
        fixture.package.runtime_asset_mount_table.clone(),
    );
    assert!(loader.mount_bundle("startup"));
    let mut manager = crate::render_resource::RenderResourceManager::new();
    let ticket = ContentTicket {
        session_generation: 376,
        request_id: 1,
    };
    let make = |normal| {
        let mut data = geometry(3, normal, [1.; 4]);
        data.sections[0].material.id = "surface-material".into();
        GeneratedGeometry::prepare(data).unwrap()
    };
    let old = make([0., 0., 1.]);
    let prepared = RuntimeRenderAssetProducer
        .prepare_generated_geometry(1, &old, ticket, 1, &mut loader, &mut manager)
        .unwrap();
    assert_eq!(prepared.geometry.len(), 1);
    for handle in prepared.resource_handles() {
        manager.validate_handle(handle).unwrap();
    }
    let before = serde_json::to_value(manager.report(2)).unwrap();
    let invalid = make([0.; 3]);
    let failure = RuntimeRenderAssetProducer
        .prepare_generated_geometry(2, &invalid, ticket, 2, &mut loader, &mut manager)
        .unwrap_err();
    assert_eq!(failure.field, "material.zeroNormal");
    assert_eq!(
        serde_json::to_value(manager.report(2)).unwrap(),
        before,
        "a rejected candidate must not publish staged mesh/material requests"
    );
    for handle in prepared.resource_handles() {
        manager.validate_handle(handle).unwrap();
    }
    let next = RuntimeRenderAssetProducer
        .prepare_generated_geometry(3, &old, ticket, 3, &mut loader, &mut manager)
        .unwrap();
    let by_kind = |prepared: &crate::render_asset_production::PreparedMeshRenderer, kind| {
        prepared
            .resource_handles()
            .filter(|h| h.kind == kind)
            .collect::<std::collections::BTreeSet<_>>()
    };
    assert_eq!(
        by_kind(
            &prepared,
            crate::render_resource::RenderResourceKind::MaterialParams
        ),
        by_kind(
            &next,
            crate::render_resource::RenderResourceKind::MaterialParams
        ),
        "the original manager reuses the package material"
    );
    assert_ne!(
        by_kind(
            &prepared,
            crate::render_resource::RenderResourceKind::MeshBuffer
        ),
        by_kind(
            &next,
            crate::render_resource::RenderResourceKind::MeshBuffer
        )
    );
    for handle in prepared.resource_handles() {
        manager.validate_handle(handle).unwrap();
    }
    assert_eq!(manager.report(3).pending_release_count, 0);
    let all: std::collections::BTreeSet<_> = prepared
        .resource_handles()
        .chain(next.resource_handles())
        .collect();
    drop(prepared);
    drop(next);
    for handle in all {
        manager
            .release_resource(crate::render_resource::RenderResourceReleaseRequest {
                handle,
                frame_index: 4,
            })
            .unwrap();
    }
    manager.end_frame(7);
    assert_eq!(manager.report(7).resident_bytes, 0);
    assert_eq!(manager.report(7).pending_release_count, 0);
    let mut cache = crate::render_geometry::ProductionGeometryCache::default();
    let a = ContentTicket {
        session_generation: 377,
        request_id: 1,
    };
    let b = ContentTicket {
        session_generation: 377,
        request_id: 2,
    };
    let first = cache
        .prepare_generated_candidate(8, &old, a, 1, &mut loader, &mut manager)
        .unwrap();
    let before = serde_json::to_value(manager.report(9)).unwrap();
    assert_eq!(
        cache
            .prepare_generated_candidate(9, &invalid, b, 2, &mut loader, &mut manager)
            .unwrap_err()
            .field_path,
        "material.zeroNormal"
    );
    assert_eq!(serde_json::to_value(manager.report(9)).unwrap(), before);
    let second = cache
        .prepare_generated_candidate(9, &old, b, 2, &mut loader, &mut manager)
        .unwrap();
    cache
        .release_generated_candidate(10, b, 2, &mut manager)
        .unwrap();
    for handle in first.resource_handles() {
        assert_eq!(
            manager.record(handle).unwrap().state,
            crate::render_resource::RenderResourceState::Resident
        );
    }
    cache
        .release_generated_candidate(10, a, 1, &mut manager)
        .unwrap();
    drop(first);
    drop(second);
    manager.end_frame(13);
    assert_eq!(manager.report(13).resident_bytes, 0);
    assert_eq!(manager.report(13).pending_release_count, 0);
}

#[test]
fn generated_content_376_lifecycle_original_frame_exit_and_fault_join_and_close_admission() {
    use crate::project_runtime_session::*;
    struct EndingSession {
        fault: bool,
    }
    impl ProjectRuntimeSession for EndingSession {
        fn session_id(&self) -> &str {
            "376-frame-ending"
        }
        fn handle_aui_actions(
            &mut self,
            _: ProjectRuntimeSessionContext<'_>,
            _: ProjectAuiActionBatch<'_>,
        ) -> ProjectRuntimeSessionOutput {
            ProjectRuntimeSessionOutput::no_op()
        }
        fn fixed_update(
            &mut self,
            _: ProjectRuntimeSessionContext<'_>,
        ) -> ProjectRuntimeSessionOutput {
            let mut output =
                ProjectRuntimeSessionOutput::applied(ProjectRuntimeMutationBuffer::new());
            if self.fault {
                output.status = ProjectRuntimeSessionStatus::Faulted;
            } else {
                output.host_control = Some(project_runtime_sdk::ProjectRuntimeHostControl::Exit);
            }
            output
        }
    }
    let fixture = frame_fixture();
    for (fault, steps) in [(false, 1), (true, 1), (false, 5)] {
        let (mut loader, mut world) = setup();
        let ticket = loader
            .accept_generated_content(replace(&world, 1), &world)
            .unwrap();
        let mut frames = crate::frame_loop::FrameLoop::new("main");
        let mut scene = crate::render_state::RenderSceneState::new();
        let mut extract = crate::render_extract::RenderExtractContext::new();
        let mut session = EndingSession { fault };
        let result = frames.tick_runtime_frame_with_project_session_delta_and_fixed_steps(
            &mut world,
            &mut scene,
            &mut extract,
            None,
            None,
            1. / 60.,
            Some(crate::frame_loop::RuntimeFrameContext::new(
                &fixture.package,
                &mut loader,
            )),
            crate::frame_loop::ProjectRuntimeFrameSession {
                session: &mut session,
                actions: &[],
                report_level: ProjectRuntimeSessionReportLevel::Off,
                observation_contract: None,
            },
            steps,
        );
        if fault || steps == 5 {
            assert!(result.is_err());
        } else {
            assert_eq!(
                result.unwrap().host_control,
                Some(project_runtime_sdk::ProjectRuntimeHostControl::Exit)
            );
        }
        assert_eq!(receipt(&loader, ticket).status, ContentStatus::Cancelled);
        assert!(loader.generated_content.worker.is_none());
        assert_eq!(
            loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
            0
        );
        assert_eq!(
            loader
                .accept_generated_content(replace(&world, 2), &world)
                .unwrap_err()
                .code,
            ContentErrorCode::StaleSession
        );
    }
}

#[test]
fn generated_content_376_headless_host_refuses_native_publication_without_world_change() {
    use crate::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
    let fixture = frame_fixture();
    let (mut loader, mut world) = setup();
    let ticket = loader
        .accept_generated_content(replace(&world, 1), &world)
        .unwrap();
    loader.advance_generated_content_cpu(&world);
    finish_cpu(&mut loader, &world);
    let mut host = EngineHostLoop::new("main");
    host.tick_with_runtime_context(
        EngineFrameInput::new(EngineHostMode::HeadlessServer).with_deferred_render_submission(),
        &mut world,
        crate::frame_loop::RuntimeFrameContext::new(&fixture.package, &mut loader),
    );
    let result = receipt(&loader, ticket);
    assert_eq!(result.status, ContentStatus::Failed);
    assert_eq!(result.error.unwrap().code, ContentErrorCode::CapabilityGap);
    assert_eq!(result.commit_epoch, None);
    assert!(world.mesh_renderer3d(&"surface".into()).is_none());
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        0
    );
    loader.close_generated_content();
}
