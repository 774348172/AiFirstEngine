//! Serialized opaque I/O inside the existing instance owner. No DLL callback or gameplay data.
use super::*;
use engine_content::atomic_file_replace::project_storage::{
    CommitState, ProjectStorage, StorageCommit, StorageError, StorageErrorKind,
};
use project_runtime_abi::{ProjectRuntimeAbiStatus, ProjectRuntimeByteBuffer};
use project_runtime_sdk::generated_content::{
    ContentError, ContentErrorCode, ContentStatus, ContentTicket, GENERATED_RECEIPT_LIMIT,
};
use project_runtime_sdk::project_storage::*;

// Bound the metadata for any u64 ticket/length, including the binary envelope header.
const MAX_PAYLOAD: usize = project_runtime_sdk::spatial_bulk::MAX_SESSION_BYTES - 256;

#[derive(Debug)]
pub(super) struct StorageState {
    generation: u64,
    accepting: bool,
    last_request: u64,
    owner: Option<ProjectStorage>,
    worker: Option<Worker>,
    records: BTreeMap<u64, Record>,
    released: Vec<usize>,
    #[cfg(test)]
    gate: Option<TestGate>,
}
impl Default for StorageState {
    fn default() -> Self {
        Self {
            generation: generated_content::next_content_session_generation(),
            accepting: false,
            last_request: 0,
            owner: None,
            worker: None,
            records: BTreeMap::new(),
            released: Vec::new(),
            #[cfg(test)]
            gate: None,
        }
    }
}
#[derive(Debug)]
struct Record {
    receipt: StorageReceipt,
    request: Option<ProjectStorageRequest>,
    payload: Option<Vec<u8>>,
    charge: Option<usize>,
    active: bool,
    read: bool,
    abandoned: bool,
}
#[derive(Debug)]
struct Completion {
    commit: StorageCommit,
    bytes: Option<Vec<u8>>,
    count: u64,
}
#[derive(Debug)]
struct Worker {
    ticket: ContentTicket,
    thread: Option<std::thread::JoinHandle<(ProjectStorage, Completion)>>,
}
impl Drop for Worker {
    fn drop(&mut self) {
        if let Some(thread) = self.thread.take() {
            drop(thread.join());
        }
    }
}
#[cfg(test)]
#[derive(Debug)]
struct TestGate {
    entered: std::sync::mpsc::SyncSender<()>,
    release: std::sync::mpsc::Receiver<()>,
}

fn error(code: ContentErrorCode, request: u64, field: &str) -> ContentError {
    ContentError {
        code,
        request_id: request,
        field_path: field.into(),
        expected: None,
        actual: None,
        next_action: match code {
            ContentErrorCode::CommitUnknown => {
                "Do not replay automatically. Read the key and reconcile in project code."
            }
            ContentErrorCode::BudgetExceeded => {
                "Claim or abandon retained reads, or wait for outstanding I/O."
            }
            ContentErrorCode::StorageBusy => {
                "Wait for the current trusted namespace owner to close."
            }
            _ => "Query the request receipt or use a trusted host storage binding.",
        }
        .into(),
    }
}
fn storage_error(source: &StorageError, request: u64) -> ContentError {
    let code = match source.kind {
        StorageErrorKind::Busy => ContentErrorCode::StorageBusy,
        StorageErrorKind::InvalidKey => ContentErrorCode::InvalidArgument,
        StorageErrorKind::TooLarge => ContentErrorCode::BudgetExceeded,
        StorageErrorKind::NotFound => ContentErrorCode::NotFound,
        StorageErrorKind::CapabilityGap => ContentErrorCode::CapabilityGap,
        StorageErrorKind::Io => ContentErrorCode::CapabilityGap,
    };
    let mut result = error(code, request, "storage.io");
    result.actual = source.source.raw_os_error().map(|code| code as u64);
    result
}
impl StorageState {
    fn record(&self, ticket: ContentTicket) -> Result<&Record, ContentError> {
        if ticket.session_generation != self.generation {
            return Err(error(
                ContentErrorCode::StaleSession,
                ticket.request_id,
                "storage.ticket.sessionGeneration",
            ));
        }
        self.records.get(&ticket.request_id).ok_or_else(|| {
            error(
                if ticket.request_id != 0 && ticket.request_id <= self.last_request {
                    ContentErrorCode::ReceiptExpired
                } else {
                    ContentErrorCode::NotFound
                },
                ticket.request_id,
                "storage.ticket.requestId",
            )
        })
    }
    fn release(&mut self, id: u64) {
        let record = self.records.get_mut(&id).unwrap();
        if let Some(charge) = record.charge.take() {
            self.released.push(charge);
        }
    }
    fn drain_released(&mut self, assets: &mut RuntimeAssetLoader) {
        for charge in self.released.drain(..) {
            assets.release_external_io(charge);
        }
    }
    fn finish(&mut self) {
        let Some(mut worker) = self.worker.take() else {
            return;
        };
        let id = worker.ticket.request_id;
        let record = self.records.get_mut(&id).unwrap();
        record.active = false;
        match worker.thread.take().unwrap().join() {
            Ok((owner, completion)) => {
                self.owner = Some(owner);
                record.receipt.commit_state = match completion.commit.state {
                    CommitState::NotCommitted => StorageCommitState::NotCommitted,
                    CommitState::Committed => StorageCommitState::Committed,
                    CommitState::Unknown => StorageCommitState::Unknown,
                };
                record.receipt.byte_count = completion.count;
                record.receipt.error = if completion.commit.state == CommitState::Unknown {
                    Some(error(ContentErrorCode::CommitUnknown, id, "storage.commit"))
                } else {
                    completion
                        .commit
                        .error
                        .as_ref()
                        .map(|e| storage_error(e, id))
                };
                record.receipt.status = if completion.commit.state == CommitState::Unknown
                    || (completion.commit.state == CommitState::NotCommitted
                        && completion.commit.error.is_some())
                {
                    ContentStatus::Failed
                } else {
                    ContentStatus::Applied
                };
                if record.abandoned {
                    record.receipt.status = ContentStatus::Cancelled;
                    record.receipt.result_claimed = true;
                } else {
                    record.payload = completion.bytes;
                }
            }
            Err(_) => {
                self.accepting = false;
                record.receipt.status = ContentStatus::Failed;
                record.receipt.commit_state = if record.read {
                    StorageCommitState::NotCommitted
                } else {
                    StorageCommitState::Unknown
                };
                record.receipt.error = Some(error(
                    if record.read {
                        ContentErrorCode::CapabilityGap
                    } else {
                        ContentErrorCode::CommitUnknown
                    },
                    id,
                    "storage.workerPanicked",
                ));
            }
        }
        if record.payload.is_none() {
            self.release(id);
        }
    }
    fn advance(&mut self, assets: &mut RuntimeAssetLoader) {
        self.drain_released(assets);
        if self
            .worker
            .as_ref()
            .is_some_and(|w| w.thread.as_ref().unwrap().is_finished())
        {
            self.finish();
            self.drain_released(assets);
        }
        if !self.accepting {
            let queued: Vec<_> = self
                .records
                .iter_mut()
                .filter_map(|(id, r)| {
                    r.request.take().map(|_| {
                        r.receipt.status = ContentStatus::Failed;
                        r.receipt.error = Some(error(
                            ContentErrorCode::CapabilityGap,
                            *id,
                            "storage.workerStopped",
                        ));
                        *id
                    })
                })
                .collect();
            for id in queued {
                self.release(id);
            }
            self.drain_released(assets);
            return;
        }
        if self.worker.is_some() {
            return;
        }
        let Some(id) = self
            .records
            .iter()
            .find(|(_, r)| r.request.is_some())
            .map(|(id, _)| *id)
        else {
            return;
        };
        let record = self.records.get_mut(&id).unwrap();
        let request = record.request.take().unwrap();
        let ticket = record.receipt.ticket;
        let Some(mut owner) = self.owner.take() else {
            return;
        };
        #[cfg(test)]
        let gate = self.gate.take();
        let thread = std::thread::Builder::new()
            .name("runtime-project-storage".into())
            .spawn(move || {
                #[cfg(test)]
                if let Some(gate) = gate {
                    gate.entered.send(()).unwrap();
                    gate.release
                        .recv_timeout(std::time::Duration::from_secs(15))
                        .unwrap();
                }
                let completion = match request.operation {
                    StorageOperation::Read { key } => match owner.read(&key, MAX_PAYLOAD) {
                        Ok(bytes) => Completion {
                            count: bytes.len() as u64,
                            bytes: Some(bytes),
                            commit: StorageCommit {
                                state: CommitState::NotCommitted,
                                error: None,
                            },
                        },
                        Err(error) => Completion {
                            count: 0,
                            bytes: None,
                            commit: StorageCommit {
                                state: CommitState::NotCommitted,
                                error: Some(error),
                            },
                        },
                    },
                    StorageOperation::Write { key, bytes } => Completion {
                        count: bytes.len() as u64,
                        bytes: None,
                        commit: owner.write(&key, &bytes),
                    },
                    StorageOperation::Delete { key } => Completion {
                        count: 0,
                        bytes: None,
                        commit: owner.delete(&key),
                    },
                };
                (owner, completion)
            });
        match thread {
            Ok(thread) => {
                record.active = true;
                record.receipt.status = ContentStatus::Preparing;
                self.worker = Some(Worker {
                    ticket,
                    thread: Some(thread),
                });
            }
            Err(source) => {
                self.accepting = false;
                record.receipt.status = ContentStatus::Failed;
                let mut e = error(ContentErrorCode::CapabilityGap, id, "storage.workerSpawn");
                e.actual = source.raw_os_error().map(|code| code as u64);
                record.receipt.error = Some(e);
                self.release(id);
                self.drain_released(assets);
            }
        }
    }
}

impl RuntimeInstanceLoader {
    /// A trusted embedding host explicitly supplies a stable identity/profile and isolated base.
    /// This is not exposed to a project module, and does not enable the ABI capability.
    pub fn bind_project_storage(
        &mut self,
        base: &std::path::Path,
        identity: &str,
        profile: &str,
    ) -> Result<(), ContentError> {
        let state = self
            .project_storage
            .get_mut()
            .unwrap_or_else(|p| p.into_inner());
        if state.owner.is_some() || state.worker.is_some() || !state.records.is_empty() {
            return Err(error(ContentErrorCode::StorageBusy, 0, "storage.binding"));
        }
        let owner =
            ProjectStorage::open(base, identity, profile).map_err(|e| storage_error(&e, 0))?;
        state.owner = Some(owner);
        state.accepting = true;
        Ok(())
    }
    pub(crate) fn accept_project_storage(
        &mut self,
        request: ProjectStorageRequest,
    ) -> Result<ContentTicket, ContentError> {
        request.validate()?;
        let state = self
            .project_storage
            .get_mut()
            .unwrap_or_else(|p| p.into_inner());
        state.drain_released(&mut self.asset_loader);
        let id = request.request_id;
        if !state.accepting {
            return Err(error(ContentErrorCode::CapabilityGap, id, "storage.owner"));
        }
        if id <= state.last_request {
            return Err(error(
                ContentErrorCode::InvalidArgument,
                id,
                "storage.requestId",
            ));
        }
        let eviction = if state.records.len() >= GENERATED_RECEIPT_LIMIT {
            Some(
                state
                    .records
                    .iter()
                    .find(|(_, r)| {
                        r.receipt.status.terminal()
                            && !r.active
                            && r.charge.is_none()
                            && r.payload.is_none()
                    })
                    .map(|(id, _)| *id)
                    .ok_or_else(|| {
                        error(ContentErrorCode::BudgetExceeded, id, "storage.receipts")
                    })?,
            )
        } else {
            None
        };
        let (key, bytes) = match &request.operation {
            StorageOperation::Read { key } => (key, MAX_PAYLOAD),
            StorageOperation::Write { key, bytes } => {
                if bytes.len() > MAX_PAYLOAD {
                    let mut error =
                        error(ContentErrorCode::BudgetExceeded, id, "storage.payloadBytes");
                    error.expected = Some(MAX_PAYLOAD as u64);
                    error.actual = Some(bytes.len() as u64);
                    return Err(error);
                }
                (key, bytes.capacity())
            }
            StorageOperation::Delete { key } => (key, 0),
        };
        let charge = bytes
            .checked_add(key.capacity())
            .and_then(|n| n.checked_add(256))
            .ok_or_else(|| error(ContentErrorCode::BudgetExceeded, id, "storage.bytes"))?;
        let budget = self.asset_loader.streaming_budget();
        if charge > budget.max_asset_bytes {
            let mut error = error(ContentErrorCode::BudgetExceeded, id, "storage.assetBytes");
            error.expected = Some(budget.max_asset_bytes as u64);
            error.actual = Some(charge as u64);
            return Err(error);
        }
        self.asset_loader.reserve_external_io(charge).map_err(|_| {
            error(
                ContentErrorCode::BudgetExceeded,
                id,
                "storage.inFlightBytes",
            )
        })?;
        if let Some(id) = eviction {
            state.records.remove(&id);
        }
        let ticket = ContentTicket {
            session_generation: state.generation,
            request_id: id,
        };
        let read = matches!(request.operation, StorageOperation::Read { .. });
        state.records.insert(
            id,
            Record {
                receipt: StorageReceipt {
                    ticket,
                    status: ContentStatus::Accepted,
                    commit_state: StorageCommitState::NotCommitted,
                    byte_count: 0,
                    result_claimed: false,
                    error: None,
                },
                request: Some(request),
                payload: None,
                charge: Some(charge),
                active: false,
                read,
                abandoned: false,
            },
        );
        state.last_request = id;
        Ok(ticket)
    }
    pub(crate) fn query_project_storage(&self, query: StorageQuery) -> StorageQueryReply {
        query.validate()?;
        let state = self
            .project_storage
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let ticket = match query {
            StorageQuery::Request { request_id } => ContentTicket {
                session_generation: state.generation,
                request_id,
            },
            StorageQuery::Ticket { ticket } => ticket,
        };
        if state.owner.is_none() && state.worker.is_none() && state.records.is_empty() {
            return Err(error(
                ContentErrorCode::CapabilityGap,
                ticket.request_id,
                "storage.owner",
            ));
        }
        Ok(state.record(ticket)?.receipt.clone())
    }
    /// # Safety
    /// The synchronous ABI caller owns a valid exclusive output descriptor/backing.
    pub(crate) unsafe fn copy_project_storage_bytes(
        &self,
        ticket: ContentTicket,
        output: *mut ProjectRuntimeByteBuffer,
    ) -> ProjectRuntimeAbiStatus {
        let mut state = self
            .project_storage
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let result = state.record(ticket).and_then(|r| {
            r.payload.as_deref().ok_or_else(|| {
                error(
                    if r.receipt.result_claimed {
                        ContentErrorCode::AlreadyApplied
                    } else if r.receipt.status.terminal() {
                        r.receipt
                            .error
                            .as_ref()
                            .map_or(ContentErrorCode::NotFound, |e| e.code)
                    } else {
                        ContentErrorCode::PendingUpdate
                    },
                    ticket.request_id,
                    "storage.result",
                )
            })
        });
        let has_payload = result.is_ok();
        // SAFETY: the caller guarantees the complete writable output; state pins the input.
        let reply = result.map(|bytes| (ticket, bytes));
        let status = unsafe {
            write_project_bytes_reply(
                output,
                reply.as_ref().map(|(ticket, bytes)| (*ticket, *bytes)),
            )
        };
        if status.is_ok() && has_payload {
            let record = state.records.get_mut(&ticket.request_id).unwrap();
            record.payload = None;
            record.receipt.result_claimed = true;
            state.release(ticket.request_id);
        }
        status
    }
    pub(crate) fn abandon_project_storage(
        &mut self,
        ticket: ContentTicket,
    ) -> Result<(), ContentError> {
        let state = self
            .project_storage
            .get_mut()
            .unwrap_or_else(|p| p.into_inner());
        let record = state.record(ticket)?;
        if !record.read {
            return Err(error(
                ContentErrorCode::InvalidArgument,
                ticket.request_id,
                "storage.result",
            ));
        }
        let record = state.records.get_mut(&ticket.request_id).unwrap();
        record.abandoned = true;
        record.payload = None;
        record.receipt.result_claimed = true;
        if record.active {
            return Ok(());
        } // Keep the reservation until real I/O returns.
        if record.request.take().is_some() {
            record.receipt.status = ContentStatus::Cancelled;
        }
        state.release(ticket.request_id);
        state.drain_released(&mut self.asset_loader);
        Ok(())
    }
    pub(crate) fn advance_project_storage(&mut self) {
        self.project_storage
            .get_mut()
            .unwrap_or_else(|p| p.into_inner())
            .advance(&mut self.asset_loader);
    }
    pub fn close_project_storage(&mut self) {
        let state = self
            .project_storage
            .get_mut()
            .unwrap_or_else(|p| p.into_inner());
        state.accepting = false;
        // Never release the lease or charge while the worker can still commit.
        state.finish();
        let ids: Vec<_> = state.records.keys().copied().collect();
        for id in ids {
            let record = state.records.get_mut(&id).unwrap();
            if record.request.take().is_some() {
                record.receipt.status = ContentStatus::Cancelled;
            }
            record.payload = None;
            record.receipt.result_claimed = record.read;
            state.release(id);
        }
        state.drain_released(&mut self.asset_loader);
        state.owner = None;
    }
}

#[cfg(all(test, windows))]
#[path = "project_storage_tests.rs"]
mod tests;

impl RuntimeInstanceLoader {
    pub(super) fn observe_project_storage_usage(
        &self,
        sample: &mut crate::resource_memory::ResourceMemorySample,
    ) {
        let state = self.project_storage.lock().expect("project storage state");
        let charged: usize = state
            .records
            .values()
            .filter_map(|r| r.charge)
            .chain(state.released.iter().copied())
            .sum();
        for (name, value) in [
            ("receipts", state.records.len()),
            ("workerActive", usize::from(state.worker.is_some())),
            ("cpuChargedBytes", charged),
            (
                "unclaimedPayloadBytes",
                state
                    .records
                    .values()
                    .filter_map(|r| r.payload.as_ref())
                    .map(Vec::len)
                    .sum(),
            ),
        ] {
            sample
                .capacities
                .insert(format!("projectStorage.{name}"), value as u64);
        }
        sample.observed_scopes.insert("projectStorage.fixedUsageCounters;reservationAndLogicalPayloadBytesNotHeapCensus;requestIdentityViaStorageReceipt".into());
    }
}
