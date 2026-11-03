use super::*;
use crate::runtime_asset::{RuntimeAssetIndex, RuntimePackageMountTable};
use crate::runtime_asset_loader::StreamingBudget;
use std::sync::atomic::{AtomicU64, Ordering};

fn fixture() -> (RuntimeInstanceLoader, std::path::PathBuf) {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let root = std::path::PathBuf::from(
        std::env::var_os("AIFE_376_TEMP_ROOT").expect("isolated root required"),
    )
    .join(format!(
        "session-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let mut loader = RuntimeInstanceLoader::new(RuntimeAssetLoader::new(
        "unused",
        RuntimeAssetIndex::new(vec![], vec![]),
        RuntimePackageMountTable::new(vec![]),
    ));
    loader
        .bind_project_storage(&root, "project", "profile")
        .unwrap();
    (loader, root)
}
fn submit(loader: &mut RuntimeInstanceLoader, id: u64, op: StorageOperation) -> ContentTicket {
    loader
        .accept_project_storage(ProjectStorageRequest {
            request_id: id,
            operation: op,
        })
        .unwrap()
}
fn write(key: &str, bytes: &[u8]) -> StorageOperation {
    StorageOperation::Write {
        key: key.into(),
        bytes: bytes.to_vec(),
    }
}
fn read(key: &str) -> StorageOperation {
    StorageOperation::Read { key: key.into() }
}
fn receipt(loader: &RuntimeInstanceLoader, ticket: ContentTicket) -> StorageReceipt {
    loader
        .query_project_storage(StorageQuery::Ticket { ticket })
        .unwrap()
}
fn settle(loader: &mut RuntimeInstanceLoader, ticket: ContentTicket) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !receipt(loader, ticket).status.terminal() {
        loader.advance_project_storage();
        assert!(
            std::time::Instant::now() < deadline,
            "storage worker did not finish"
        );
        std::thread::yield_now();
    }
}
fn take(loader: &RuntimeInstanceLoader, ticket: ContentTicket) -> Vec<u8> {
    let mut output = ProjectRuntimeByteBuffer {
        data: std::ptr::null_mut(),
        capacity: 0,
        written: 0,
    };
    // SAFETY: the descriptor is exclusive, zero capacity, and alive for the call.
    assert_eq!(
        unsafe { loader.copy_project_storage_bytes(ticket, &mut output) },
        ProjectRuntimeAbiStatus::BUFFER_TOO_SMALL
    );
    let mut wire = vec![0; output.written as usize];
    output.data = wire.as_mut_ptr();
    output.capacity = wire.len() as u64;
    // SAFETY: wire has the exact complete writable size and does not alias the payload.
    assert!(unsafe { loader.copy_project_storage_bytes(ticket, &mut output) }.is_ok());
    decode_project_bytes_reply(&wire).unwrap().unwrap().bytes
}

#[test]
fn project_storage_376_ordered_same_key_read_write_delete_and_ticket_lifetime() {
    let (mut loader, _) = fixture();
    let a = submit(&mut loader, 1, write("save", b"abc"));
    let b = submit(&mut loader, 2, read("save"));
    let c = submit(
        &mut loader,
        3,
        StorageOperation::Delete { key: "save".into() },
    );
    let d = submit(&mut loader, 4, read("save"));
    settle(&mut loader, d);
    assert_eq!(
        receipt(&loader, a).commit_state,
        StorageCommitState::Committed
    );
    assert_eq!(
        receipt(&loader, c).commit_state,
        StorageCommitState::Committed
    );
    assert_eq!(
        receipt(&loader, d).error.unwrap().code,
        ContentErrorCode::NotFound
    );
    assert_eq!(take(&loader, b), b"abc");
    assert!(receipt(&loader, b).result_claimed);
    let foreign = ContentTicket {
        session_generation: b.session_generation + 1,
        ..b
    };
    assert_eq!(
        loader
            .query_project_storage(StorageQuery::Ticket { ticket: foreign })
            .unwrap_err()
            .code,
        ContentErrorCode::StaleSession
    );
    loader.advance_project_storage();
    assert_eq!(loader.asset_loader().streaming_snapshot().reserved_bytes, 0);
    assert_eq!(
        loader
            .asset_loader()
            .streaming_snapshot()
            .pending_operations,
        0
    );
}

#[test]
fn project_storage_376_short_buffer_pins_payload_and_shared_io_cpu_budget_until_claim_or_abandon() {
    let (mut loader, _) = fixture();
    let a = submit(&mut loader, 1, write("value", b"payload"));
    settle(&mut loader, a);
    let budget = loader.asset_loader().streaming_budget();
    loader
        .asset_loader_mut()
        .set_streaming_budget(StreamingBudget {
            max_operations: 2,
            ..budget
        })
        .unwrap();
    let b = submit(&mut loader, 2, read("value"));
    let c = submit(&mut loader, 3, read("value"));
    settle(&mut loader, c);
    let held = loader.asset_loader().streaming_snapshot();
    assert_eq!(held.pending_operations, 2);
    assert!(held.reserved_bytes >= 2 * MAX_PAYLOAD);
    let mut sample = crate::resource_memory::ResourceMemorySample::new(1, 1, 0);
    loader.observe_memory(&mut sample);
    assert_eq!(
        sample.capacities["projectStorage.cpuChargedBytes"],
        held.reserved_bytes as u64
    );
    assert_eq!(
        sample.capacities["projectStorage.unclaimedPayloadBytes"],
        14
    );
    assert_eq!(sample.capacities["projectStorage.receipts"], 3);
    assert_eq!(
        loader
            .asset_loader_mut()
            .set_streaming_budget(budget)
            .unwrap_err(),
        crate::runtime_asset_loader::StreamingError::Busy
    );
    assert_eq!(
        loader
            .asset_loader_mut()
            .reserve_prepared_cpu(budget.max_cpu_resident_bytes)
            .unwrap_err(),
        crate::runtime_asset_loader::StreamingError::BudgetExceeded
    );
    let refused = loader
        .accept_project_storage(ProjectStorageRequest {
            request_id: 4,
            operation: write("other", b"no"),
        })
        .unwrap_err();
    assert_eq!(refused.code, ContentErrorCode::BudgetExceeded);
    for _ in 0..3 {
        let mut one = [0u8; 1];
        let mut output = ProjectRuntimeByteBuffer {
            data: one.as_mut_ptr(),
            capacity: 1,
            written: 0,
        };
        // SAFETY: the one-byte exclusive descriptor cannot hold a complete envelope.
        assert_eq!(
            unsafe { loader.copy_project_storage_bytes(b, &mut output) },
            ProjectRuntimeAbiStatus::BUFFER_TOO_SMALL
        );
        assert!(output.written > 7);
        assert!(!receipt(&loader, b).result_claimed);
        assert_eq!(loader.asset_loader().streaming_snapshot(), held);
    }
    assert_eq!(take(&loader, b), b"payload");
    let mut claimed = crate::resource_memory::ResourceMemorySample::new(1, 1, 0);
    loader.observe_memory(&mut claimed);
    assert_eq!(
        claimed.capacities["projectStorage.unclaimedPayloadBytes"],
        7
    );
    // Release is conservative until the instance owner polls or next admits work.
    let d = submit(&mut loader, 4, write("other", b"yes"));
    settle(&mut loader, d);
    assert_eq!(
        loader
            .asset_loader()
            .streaming_snapshot()
            .pending_operations,
        1
    );
    loader.abandon_project_storage(c).unwrap();
    assert_eq!(loader.asset_loader().streaming_snapshot().reserved_bytes, 0);
    assert_eq!(
        loader
            .asset_loader()
            .streaming_snapshot()
            .pending_operations,
        0
    );
}

#[test]
fn project_storage_376_unclaimed_results_cannot_be_evicted_by_terminal_receipt_pressure() {
    let (mut loader, _) = fixture();
    let budget = loader.asset_loader().streaming_budget();
    loader
        .asset_loader_mut()
        .set_streaming_budget(StreamingBudget {
            max_operations: 256,
            max_in_flight_bytes: 2 * 1024 * 1024 * 1024,
            max_cpu_resident_bytes: 2 * 1024 * 1024 * 1024,
            ..budget
        })
        .unwrap();
    let seed = submit(&mut loader, 1, write("empty", b""));
    settle(&mut loader, seed);
    let mut tickets = Vec::new();
    for id in 2..=(GENERATED_RECEIPT_LIMIT as u64 + 1) {
        let ticket = submit(&mut loader, id, read("empty"));
        settle(&mut loader, ticket);
        tickets.push(ticket);
    }
    let next = GENERATED_RECEIPT_LIMIT as u64 + 2;
    let rejected = loader
        .accept_project_storage(ProjectStorageRequest {
            request_id: next,
            operation: write("other", b"x"),
        })
        .unwrap_err();
    assert_eq!(rejected.field_path, "storage.receipts");
    for ticket in &tickets {
        assert!(!receipt(&loader, *ticket).result_claimed);
    }
    assert_eq!(
        loader
            .project_storage
            .lock()
            .unwrap()
            .records
            .values()
            .filter_map(|record| record.payload.as_ref())
            .map(Vec::capacity)
            .sum::<usize>(),
        0,
        "128 empty results must not allocate 128 maximum-sized buffers"
    );
    assert_eq!(take(&loader, tickets[0]), b"");
    let accepted = submit(&mut loader, next, write("other", b"x"));
    settle(&mut loader, accepted);
    assert_eq!(
        loader
            .query_project_storage(StorageQuery::Ticket { ticket: tickets[0] })
            .unwrap_err()
            .code,
        ContentErrorCode::ReceiptExpired
    );
    assert!(!receipt(&loader, tickets[1]).result_claimed);
    loader.close_project_storage();
    assert_eq!(loader.asset_loader().streaming_snapshot().reserved_bytes, 0);
    assert_eq!(
        loader
            .asset_loader()
            .streaming_snapshot()
            .pending_operations,
        0
    );
}

#[test]
fn project_storage_376_close_settles_real_worker_before_releasing_lease_and_cancels_queued_io() {
    let (mut loader, root) = fixture();
    let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(0);
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    loader.project_storage.get_mut().unwrap().gate = Some(TestGate {
        entered: entered_tx,
        release: release_rx,
    });
    let a = submit(&mut loader, 1, write("save", b"committed"));
    let b = submit(
        &mut loader,
        2,
        StorageOperation::Delete { key: "save".into() },
    );
    loader.advance_project_storage();
    entered_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    let joined = std::thread::spawn(move || {
        loader.close_project_storage();
        loader
    });
    assert_eq!(
        ProjectStorage::open(&root, "project", "profile")
            .unwrap_err()
            .kind,
        StorageErrorKind::Busy
    );
    release_tx.send(()).unwrap();
    let mut loader = joined.join().unwrap();
    assert_eq!(
        receipt(&loader, a).commit_state,
        StorageCommitState::Committed
    );
    assert_eq!(receipt(&loader, b).status, ContentStatus::Cancelled);
    assert_eq!(loader.asset_loader().streaming_snapshot().reserved_bytes, 0);
    let reloaded = ProjectStorage::open(&root, "project", "profile").unwrap();
    assert_eq!(reloaded.read("save", 100).unwrap(), b"committed");
    assert_eq!(
        loader
            .accept_project_storage(ProjectStorageRequest {
                request_id: 3,
                operation: read("save")
            })
            .unwrap_err()
            .code,
        ContentErrorCode::CapabilityGap
    );
}

#[test]
fn project_storage_376_failed_admission_preserves_fifo_and_never_allocates_maximum_payloads() {
    let (mut loader, _) = fixture();
    let budget = loader.asset_loader().streaming_budget();
    loader
        .asset_loader_mut()
        .set_streaming_budget(StreamingBudget {
            max_operations: 1,
            ..budget
        })
        .unwrap();
    let a = submit(&mut loader, 1, write("save", b"old"));
    assert_eq!(
        loader
            .accept_project_storage(ProjectStorageRequest {
                request_id: 2,
                operation: write("save", b"bad")
            })
            .unwrap_err()
            .code,
        ContentErrorCode::BudgetExceeded
    );
    settle(&mut loader, a);
    let b = submit(&mut loader, 2, read("save"));
    settle(&mut loader, b);
    assert_eq!(take(&loader, b), b"old");
    assert!(loader
        .project_storage
        .lock()
        .unwrap()
        .records
        .values()
        .all(|r| r.payload.is_none()));
}

#[test]
fn project_storage_376_worker_failure_is_unknown_and_never_replayed_automatically() {
    let (mut loader, root) = fixture();
    let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(0);
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    loader.project_storage.get_mut().unwrap().gate = Some(TestGate {
        entered: entered_tx,
        release: release_rx,
    });
    let a = submit(&mut loader, 1, write("save", b"candidate"));
    let b = submit(&mut loader, 2, write("other", b"queued"));
    loader.advance_project_storage();
    entered_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    drop(release_tx); // The actual worker panics; its progress cannot be recovered.
    settle(&mut loader, a);
    assert_eq!(
        receipt(&loader, a).commit_state,
        StorageCommitState::Unknown
    );
    assert_eq!(
        receipt(&loader, a).error.unwrap().code,
        ContentErrorCode::CommitUnknown
    );
    assert_eq!(receipt(&loader, b).status, ContentStatus::Failed);
    for _ in 0..3 {
        loader.advance_project_storage();
    }
    assert_eq!(loader.asset_loader().streaming_snapshot().reserved_bytes, 0);
    assert_eq!(
        loader
            .asset_loader()
            .streaming_snapshot()
            .pending_operations,
        0
    );
    let owner = ProjectStorage::open(&root, "project", "profile").unwrap();
    assert_eq!(
        owner.read("save", 100).unwrap_err().kind,
        StorageErrorKind::NotFound
    );
    assert_eq!(
        owner.read("other", 100).unwrap_err().kind,
        StorageErrorKind::NotFound
    );
}

#[test]
fn project_storage_376_abandon_inflight_read_keeps_charge_until_actual_worker_returns() {
    let (mut loader, _) = fixture();
    let seed = submit(&mut loader, 1, write("save", b"owned"));
    settle(&mut loader, seed);
    let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(0);
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    loader.project_storage.get_mut().unwrap().gate = Some(TestGate {
        entered: entered_tx,
        release: release_rx,
    });
    let ticket = submit(&mut loader, 2, read("save"));
    loader.advance_project_storage();
    entered_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    let held = loader.asset_loader().streaming_snapshot();
    loader.abandon_project_storage(ticket).unwrap();
    assert_eq!(loader.asset_loader().streaming_snapshot(), held);
    assert!(!receipt(&loader, ticket).status.terminal());
    release_tx.send(()).unwrap();
    settle(&mut loader, ticket);
    assert_eq!(receipt(&loader, ticket).status, ContentStatus::Cancelled);
    assert!(receipt(&loader, ticket).result_claimed);
    assert_eq!(loader.asset_loader().streaming_snapshot().reserved_bytes, 0);
    assert!(loader
        .project_storage
        .lock()
        .unwrap()
        .records
        .values()
        .all(|record| record.payload.is_none()));
}

#[test]
fn project_storage_376_byte_limits_backpressure_shared_io_and_cpu_without_allocating_read_payloads()
{
    let (mut loader, _) = fixture();
    let seed = submit(&mut loader, 1, write("value", b"v"));
    settle(&mut loader, seed);
    let budget = loader.asset_loader().streaming_budget();
    for (index, (io, cpu)) in [
        (budget.max_asset_bytes, budget.max_cpu_resident_bytes),
        (budget.max_in_flight_bytes, budget.max_asset_bytes),
    ]
    .into_iter()
    .enumerate()
    {
        loader
            .asset_loader_mut()
            .set_streaming_budget(StreamingBudget {
                max_operations: 8,
                max_in_flight_bytes: io,
                max_cpu_resident_bytes: cpu,
                ..budget
            })
            .unwrap();
        let id = 2 + index as u64;
        let held = submit(&mut loader, id, read("value"));
        assert_eq!(
            loader
                .accept_project_storage(ProjectStorageRequest {
                    request_id: id + 1,
                    operation: read("value")
                })
                .unwrap_err()
                .code,
            ContentErrorCode::BudgetExceeded
        );
        assert!(loader
            .project_storage
            .lock()
            .unwrap()
            .records
            .values()
            .all(|record| record.payload.is_none()));
        loader.abandon_project_storage(held).unwrap();
        assert_eq!(loader.asset_loader().streaming_snapshot().reserved_bytes, 0);
    }
}

#[test]
fn project_storage_376_maximum_payload_fits_complete_wire_and_larger_input_is_rejected() {
    let (mut loader, _) = fixture();
    let payload = vec![0x71; MAX_PAYLOAD];
    let seed = submit(&mut loader, 1, write("large", &payload));
    settle(&mut loader, seed);
    assert_eq!(
        loader
            .accept_project_storage(ProjectStorageRequest {
                request_id: 2,
                operation: write("large", &vec![0; MAX_PAYLOAD + 1])
            })
            .unwrap_err()
            .field_path,
        "storage.payloadBytes"
    );
    let read = submit(&mut loader, 2, read("large"));
    settle(&mut loader, read);
    let mut output = ProjectRuntimeByteBuffer {
        data: std::ptr::null_mut(),
        capacity: 0,
        written: 0,
    };
    // SAFETY: exclusive live zero-capacity descriptor, used only to determine complete size.
    assert_eq!(
        unsafe { loader.copy_project_storage_bytes(read, &mut output) },
        ProjectRuntimeAbiStatus::BUFFER_TOO_SMALL
    );
    assert!(output.written <= project_runtime_sdk::spatial_bulk::MAX_SESSION_BYTES as u64);
    assert_eq!(take(&loader, read), payload);
    loader.advance_project_storage();
    assert_eq!(loader.asset_loader().streaming_snapshot().reserved_bytes, 0);
}
