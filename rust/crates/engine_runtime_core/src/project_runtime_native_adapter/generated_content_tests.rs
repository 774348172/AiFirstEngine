use super::*;
use crate::runtime_asset::{RuntimeAssetIndex, RuntimePackageMountTable};
use crate::runtime_asset_loader::RuntimeAssetLoader;
use crate::runtime_instance_loader::RuntimeInstanceLoader;
use crate::world::World;
use project_runtime_sdk::generated_content::*;

#[test]
fn generated_content_376_ingress_decodes_into_the_original_deferred_buffer() {
    let mut buffer = ProjectRuntimeMutationBuffer::new();
    append_deferred_mutation(
        &mut buffer,
        ProjectRuntimeDeferredMutation::GeneratedContent {
            request: GeneratedContentRequest {
                request_id: 1,
                target: project_runtime_sdk::ProjectRuntimeCellEntity {
                    entity_id: "surface".into(),
                    generation: 1,
                },
                expected_applied_revision: 0,
                operation: GeneratedOperation3D::Release,
            },
        },
    )
    .unwrap();
    append_deferred_mutation(
        &mut buffer,
        ProjectRuntimeDeferredMutation::GeneratedContentCancel {
            ticket: ContentTicket {
                session_generation: 1,
                request_id: 1,
            },
        },
    )
    .unwrap();
    assert_eq!(buffer.len(), 2);
    // Ordinary preparation has no runtime owner and must reject, never publish or lose intents.
    assert!(buffer.prepare(&World::new()).is_err());
}

#[test]
fn generated_content_376_compact_query_uses_real_ffi_host_and_cannot_retain_context() {
    let mut world = World::new();
    world.commit_prepared_entity(
        "surface".into(),
        "surface".into(),
        "spatial".into(),
        true,
        crate::components::Hierarchy {
            parent_id: None,
            sibling_order: 0,
        },
        vec![ComponentValue::Transform3D(
            crate::spatial_contract::Transform3D::identity()
                .canonicalize()
                .unwrap(),
        )],
    );
    let target = project_runtime_sdk::ProjectRuntimeCellEntity {
        entity_id: "surface".into(),
        generation: world.entity_generation(&"surface".into()).unwrap(),
    };
    let mut owner = RuntimeInstanceLoader::new(RuntimeAssetLoader::new(
        "unused",
        RuntimeAssetIndex::new(vec![], vec![]),
        RuntimePackageMountTable::new(vec![]),
    ));
    let ticket = owner
        .accept_generated_content(
            GeneratedContentRequest {
                request_id: 1,
                target: target.clone(),
                expected_applied_revision: 0,
                operation: GeneratedOperation3D::Release,
            },
            &world,
        )
        .unwrap();
    let request = ContentQueryRequest {
        generated_content: ContentQuery::Request { request_id: 1 },
    };
    let mut host = ReadOnlyWorldHost {
        world: WorldReadApi::with_runtime_cells(&world, Some(&owner)),
        spawn_receipts: None,
        animator_snapshots: None,
        ui: None,
    };
    let token = with_host_context(&mut host, |token| {
        let result: ContentQueryReply =
            project_runtime_sdk::call_host_json(host_world_query, token, &request).unwrap();
        let ContentQueryValue::Receipt { receipt } = result.unwrap() else {
            panic!()
        };
        assert_eq!(receipt.ticket, ticket);
        assert_eq!(receipt.status, ContentStatus::Accepted);
        assert_eq!(receipt.applied_revision, 0);
        assert_eq!(receipt.pending_revision, Some(1));
        assert!(serde_json::to_vec(&receipt).unwrap().len() < 1024);
        let result: ContentQueryReply = project_runtime_sdk::call_host_json(
            host_world_query,
            token,
            &ContentQueryRequest {
                generated_content: ContentQuery::Target {
                    target: target.clone(),
                },
            },
        )
        .unwrap();
        let ContentQueryValue::Target { state } = result.unwrap() else {
            panic!()
        };
        assert_eq!(state.target, target);
        assert_eq!(state.applied_revision, 0);
        assert_eq!(state.pending_revision, Some(1));
        token
    });
    let error = project_runtime_sdk::call_host_json::<_, ContentQueryReply>(
        host_world_query,
        token,
        &request,
    )
    .unwrap_err();
    assert_eq!(error.status, ProjectRuntimeAbiStatus::INVALID_HANDLE);
}

#[test]
fn generated_content_376_query_rejects_mixed_selectors_and_reports_missing_owner_semantically() {
    let world = World::new();
    let mut host = ReadOnlyWorldHost {
        world: WorldReadApi::new(&world),
        spawn_receipts: None,
        animator_snapshots: None,
        ui: None,
    };
    with_host_context(&mut host, |token| {
        let result: ContentQueryReply = project_runtime_sdk::call_host_json(
            host_world_query,
            token,
            &ContentQueryRequest {
                generated_content: ContentQuery::Request { request_id: 1 },
            },
        )
        .unwrap();
        assert_eq!(result.unwrap_err().code, ContentErrorCode::CapabilityGap);
        let error = project_runtime_sdk::call_host_json::<_, ContentQueryReply>(
            host_world_query,
            token,
            &serde_json::json!({"generatedContent":{"query":"request","requestId":1},"all":["Transform3D"]}),
        )
        .unwrap_err();
        assert_eq!(error.status, ProjectRuntimeAbiStatus::INVALID_ARGUMENT);
    });
}

#[test]
fn generated_content_376_admission_query_validates_before_missing_owner() {
    let world = World::new();
    let mut host = ReadOnlyWorldHost {
        world: WorldReadApi::new(&world),
        spawn_receipts: None,
        animator_snapshots: None,
        ui: None,
    };
    with_host_context(&mut host, |token| {
        for query in [
            ContentQuery::Request { request_id: 0 },
            ContentQuery::Ticket {
                ticket: ContentTicket {
                    session_generation: 0,
                    request_id: 1,
                },
            },
            ContentQuery::Target {
                target: project_runtime_sdk::ProjectRuntimeCellEntity {
                    entity_id: " ".into(),
                    generation: 1,
                },
            },
        ] {
            let result: ContentQueryReply = project_runtime_sdk::call_host_json(
                host_world_query,
                token,
                &ContentQueryRequest {
                    generated_content: query,
                },
            )
            .unwrap();
            let error = result.unwrap_err();
            assert_eq!(error.code, ContentErrorCode::InvalidArgument);
            assert_eq!(error.field_path, "generatedContent.query");
        }
    });
}

#[test]
fn generated_content_376_storage_ffi_queries_fail_closed_and_context_expires() {
    use project_runtime_sdk::project_storage::*;
    let world = World::new();
    let mut host = ReadOnlyWorldHost {
        world: WorldReadApi::new(&world),
        spawn_receipts: None,
        animator_snapshots: None,
        ui: None,
    };
    let ticket = ContentTicket {
        session_generation: 1,
        request_id: 2,
    };
    let token = with_host_context(&mut host, |token| {
        let result: StorageQueryReply = project_runtime_sdk::call_host_json(
            host_world_query,
            token,
            &StorageQueryRequest {
                project_storage: StorageQuery::Request { request_id: 2 },
            },
        )
        .unwrap();
        assert_eq!(result.unwrap_err().code, ContentErrorCode::CapabilityGap);
        let result = call_take_project_bytes(host_world_query, token, ticket).unwrap();
        let error = result.unwrap_err();
        assert_eq!(error.code, ContentErrorCode::CapabilityGap);
        assert_eq!(error.request_id, 2);
        let result: StorageQueryReply = project_runtime_sdk::call_host_json(
            host_world_query,
            token,
            &StorageQueryRequest {
                project_storage: StorageQuery::Request { request_id: 0 },
            },
        )
        .unwrap();
        assert_eq!(result.unwrap_err().code, ContentErrorCode::InvalidArgument);
        let error = project_runtime_sdk::call_host_json::<_, StorageQueryReply>(host_world_query, token, &serde_json::json!({"projectStorage":{"query":"request","requestId":2},"generatedContent":{"query":"request","requestId":1}})).unwrap_err();
        assert_eq!(error.status, ProjectRuntimeAbiStatus::INVALID_ARGUMENT);
        token
    });
    assert_eq!(
        call_take_project_bytes(host_world_query, token, ticket)
            .unwrap_err()
            .status,
        ProjectRuntimeAbiStatus::INVALID_HANDLE
    );
}

#[cfg(windows)]
#[test]
fn project_storage_376_real_ffi_owner_pins_short_result_and_claims_only_after_complete_copy() {
    use project_runtime_sdk::project_storage::*;
    let root = std::path::PathBuf::from(std::env::var_os("AIFE_376_TEMP_ROOT").unwrap())
        .join(format!("ffi-owner-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let world = World::new();
    let mut owner = RuntimeInstanceLoader::new(RuntimeAssetLoader::new(
        "unused",
        RuntimeAssetIndex::new(vec![], vec![]),
        RuntimePackageMountTable::new(vec![]),
    ));
    owner
        .bind_project_storage(&root, "project", "profile")
        .unwrap();
    let seed = owner
        .accept_project_storage(ProjectStorageRequest {
            request_id: 1,
            operation: StorageOperation::Write {
                key: "value".into(),
                bytes: vec![0xa5; 6000],
            },
        })
        .unwrap();
    let ticket = owner
        .accept_project_storage(ProjectStorageRequest {
            request_id: 2,
            operation: StorageOperation::Read {
                key: "value".into(),
            },
        })
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !owner
        .query_project_storage(StorageQuery::Ticket { ticket })
        .unwrap()
        .status
        .terminal()
    {
        owner.advance_project_storage();
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(
        owner
            .query_project_storage(StorageQuery::Ticket { ticket: seed })
            .unwrap()
            .commit_state,
        StorageCommitState::Committed
    );
    let held = owner.asset_loader().streaming_snapshot().reserved_bytes;
    let mut host = ReadOnlyWorldHost {
        world: WorldReadApi::with_runtime_cells(&world, Some(&owner)),
        spawn_receipts: None,
        animator_snapshots: None,
        ui: None,
    };
    let token = with_host_context(&mut host, |token| {
        let request = serde_json::to_vec(&StorageTakeRequest {
            project_storage_take: ticket,
        })
        .unwrap();
        let mut output = ProjectRuntimeByteBuffer {
            data: std::ptr::null_mut(),
            capacity: 0,
            written: 0,
        };
        for _ in 0..3 {
            // SAFETY: request and exclusive zero-capacity output stay live through this call.
            assert_eq!(
                unsafe {
                    host_world_query(
                        token,
                        ProjectRuntimeByteSlice::from_slice(&request),
                        &mut output,
                    )
                },
                ProjectRuntimeAbiStatus::BUFFER_TOO_SMALL
            );
            let result: StorageQueryReply = project_runtime_sdk::call_host_json(
                host_world_query,
                token,
                &StorageQueryRequest {
                    project_storage: StorageQuery::Ticket { ticket },
                },
            )
            .unwrap();
            assert!(!result.unwrap().result_claimed);
        }
        assert_eq!(
            call_take_project_bytes(host_world_query, token, ticket)
                .unwrap()
                .unwrap()
                .bytes,
            vec![0xa5; 6000]
        );
        let result: StorageQueryReply = project_runtime_sdk::call_host_json(
            host_world_query,
            token,
            &StorageQueryRequest {
                project_storage: StorageQuery::Ticket { ticket },
            },
        )
        .unwrap();
        assert!(result.unwrap().result_claimed);
        assert_eq!(
            call_take_project_bytes(host_world_query, token, ticket)
                .unwrap()
                .unwrap_err()
                .code,
            ContentErrorCode::AlreadyApplied
        );
        token
    });
    assert_eq!(
        call_take_project_bytes(host_world_query, token, ticket)
            .unwrap_err()
            .status,
        ProjectRuntimeAbiStatus::INVALID_HANDLE
    );
    assert!(held > 6000);
    owner.advance_project_storage();
    assert_eq!(owner.asset_loader().streaming_snapshot().reserved_bytes, 0);
    owner.close_project_storage();
}

#[test]
fn generated_content_376_binding_accepts_content_contracts_and_rejects_old_digest_early() {
    for feature in [
        project_runtime_abi::PROJECT_RUNTIME_CAP_GENERATED_CONTENT,
        project_runtime_abi::PROJECT_RUNTIME_CAP_PROJECT_STORAGE,
    ] {
        let mut api = super::tests::fake_api();
        api.capabilities |= feature;
        validate_api(&api).expect("content uses the existing deferred/world callbacks");
        api.capabilities &= !project_runtime_abi::PROJECT_RUNTIME_CAP_DEFERRED_MUTATIONS;
        assert!(validate_api(&api).unwrap_err().message.contains("deferred"));
    }
    let mut api = super::tests::fake_api();
    api.contract_digest[0] ^= 0xff;
    let error = validate_api(&api).unwrap_err();
    assert!(error.message.contains("RequiresRebuild"));
}

#[test]
fn project_storage_376_official_ingress_keeps_both_requests_until_runtime_admission() {
    use project_runtime_sdk::project_storage::*;
    let mut buffer = ProjectRuntimeMutationBuffer::new();
    append_deferred_mutation(
        &mut buffer,
        ProjectRuntimeDeferredMutation::ProjectStorage {
            request: ProjectStorageRequest {
                request_id: 1,
                operation: StorageOperation::Read {
                    key: "parameters".into(),
                },
            },
        },
    )
    .unwrap();
    append_deferred_mutation(
        &mut buffer,
        ProjectRuntimeDeferredMutation::ProjectStorageAbandon {
            ticket: ContentTicket {
                session_generation: 1,
                request_id: 1,
            },
        },
    )
    .unwrap();
    assert_eq!(buffer.len(), 2);
    assert!(
        buffer.prepare(&World::new()).is_err(),
        "a synchronous-only caller has no I/O owner"
    );
}
