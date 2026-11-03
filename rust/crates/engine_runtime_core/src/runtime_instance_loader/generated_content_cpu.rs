//! One bounded CPU preparation lane owned by the existing runtime instance loader.
//! No project callbacks, World, native handles, filesystem work or public Job API.
use super::*;
use std::thread::JoinHandle;

#[cfg(test)]
#[derive(Debug)]
pub(super) struct TestGate {
    pub entered: std::sync::mpsc::SyncSender<()>,
    pub release: std::sync::mpsc::Receiver<()>,
    pub panic: bool,
}

#[derive(Debug)]
pub(super) struct Candidate {
    pub geometry: Option<GeneratedGeometry>,
    pub collision: Option<CollisionUpdate3D>,
    pub release: bool,
    pub cpu_bytes: usize,
}

#[derive(Debug)]
pub(super) struct BackingCharge {
    pub(super) bytes: usize,
    lifetime: std::sync::Weak<Vec<u8>>,
}

impl GeneratedContentState {
    // The existing asset loader owns the reservation. These weak references only identify
    // when its actual vertex/index allocation can retire, including native/frame readers.
    pub(super) fn reap_cpu_backings(&mut self, assets: &mut RuntimeAssetLoader) {
        self.cpu_backings.retain(|_, backing| {
            let held = backing.lifetime.strong_count() != 0;
            if !held {
                assets.release_prepared_cpu(backing.bytes);
            }
            held
        });
    }

    pub(super) fn pending_cpu_bytes(&self) -> usize {
        let resident: std::collections::BTreeSet<_> = self
            .bindings
            .values()
            .filter_map(|binding| binding.canonical.as_ref())
            .flat_map(|geometry| geometry.cpu_backings().map(|(identity, _, _)| identity))
            .collect();
        self.records
            .values()
            .map(|record| record.cpu_charge)
            .sum::<usize>()
            + self
                .cpu_backings
                .iter()
                .filter(|(identity, _)| !resident.contains(identity))
                .map(|(_, backing)| backing.bytes)
                .sum::<usize>()
    }

    fn retain_candidate_backings(
        &mut self,
        ticket: ContentTicket,
        candidate: &Candidate,
    ) -> Result<(), ContentError> {
        let backings: Vec<_> = candidate
            .geometry
            .as_ref()
            .into_iter()
            .flat_map(GeneratedGeometry::cpu_backings)
            .filter(|(identity, _, _)| !self.cpu_backings.contains_key(identity))
            .collect();
        let bytes: usize = backings.iter().map(|(_, bytes, _)| bytes).sum();
        let record = self.records.get_mut(&ticket.request_id).unwrap();
        if bytes > record.cpu_charge {
            return Err(error(
                ContentErrorCode::BudgetExceeded,
                ticket.request_id,
                "cpu.candidateBytes",
            ));
        }
        // Transfer, never release/re-reserve: a patch's shared index was already charged.
        record.cpu_charge -= bytes;
        for (identity, bytes, lifetime) in backings {
            self.cpu_backings
                .insert(identity, BackingCharge { bytes, lifetime });
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(super) struct Worker {
    pub ticket: ContentTicket,
    thread: Option<JoinHandle<Result<Candidate, ContentError>>>,
}

impl Worker {
    fn spawn(
        work: GeneratedContentWork,
        base: Option<GeneratedGeometry>,
        #[cfg(test)] gate: Option<TestGate>,
    ) -> Result<Self, ContentError> {
        let ticket = work.ticket;
        let thread = std::thread::Builder::new()
            .name("runtime-generated-geometry".into())
            .spawn(move || {
                #[cfg(test)]
                if let Some(gate) = gate {
                    gate.entered.send(()).unwrap();
                    gate.release
                        .recv_timeout(std::time::Duration::from_secs(5))
                        .unwrap();
                    assert!(!gate.panic, "injected generated CPU worker panic");
                }
                prepare(work, base)
            })
            .map_err(|_| {
                error(
                    ContentErrorCode::CapabilityGap,
                    ticket.request_id,
                    "cpu.workerSpawn",
                )
            })?;
        Ok(Self {
            ticket,
            thread: Some(thread),
        })
    }

    fn finished(&self) -> bool {
        self.thread.as_ref().is_some_and(JoinHandle::is_finished)
    }

    pub(super) fn join(mut self) -> Result<Candidate, ContentError> {
        self.thread
            .take()
            .expect("one worker join")
            .join()
            .unwrap_or_else(|_| {
                Err(error(
                    ContentErrorCode::CapabilityGap,
                    self.ticket.request_id,
                    "cpu.workerPanicked",
                ))
            })
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        // Bounded CPU-only work must finish before its instance owner is destroyed.
        if let Some(thread) = self.thread.take() {
            drop(thread.join());
        }
    }
}

pub(super) fn collision_bytes(collision: &CollisionUpdate3D) -> usize {
    match collision {
        CollisionUpdate3D::Replace { body } => match &body.shape {
            StaticShape3D::Convex { vertex_bytes, .. } => vertex_bytes.capacity(),
            StaticShape3D::StaticMesh {
                vertex_bytes,
                triangle_bytes,
                ..
            } => vertex_bytes
                .capacity()
                .saturating_add(triangle_bytes.capacity()),
            _ => 0,
        },
        _ => 0,
    }
}

pub(super) fn collision_decode_bytes(request: &GeneratedContentRequest) -> usize {
    match &request.operation {
        GeneratedOperation3D::Replace { collision, .. }
        | GeneratedOperation3D::ReplaceCollision { collision } => collision_bytes(collision),
        _ => 0,
    }
}

pub(super) fn request_backing_bytes(request: &GeneratedContentRequest) -> usize {
    match &request.operation {
        GeneratedOperation3D::Replace {
            geometry,
            collision,
        } => geometry
            .vertex_bytes
            .capacity()
            .saturating_add(geometry.index_bytes.capacity())
            .saturating_add(collision_bytes(collision)),
        GeneratedOperation3D::UpdateAttributes { patch } => patch.bytes.capacity(),
        GeneratedOperation3D::ReplaceCollision { collision } => collision_bytes(collision),
        GeneratedOperation3D::Release => 0,
    }
}

fn prepare(
    work: GeneratedContentWork,
    base: Option<GeneratedGeometry>,
) -> Result<Candidate, ContentError> {
    let input_bytes = request_backing_bytes(&work.request) + collision_decode_bytes(&work.request);
    let result = match work.request.operation {
        GeneratedOperation3D::Replace {
            geometry,
            collision,
        } => GeneratedGeometry::prepare(geometry).map(|geometry| Candidate {
            geometry: Some(geometry),
            collision: Some(collision),
            release: false,
            cpu_bytes: input_bytes,
        }),
        GeneratedOperation3D::UpdateAttributes { patch } => base
            .ok_or_else(|| {
                error(
                    ContentErrorCode::RevisionConflict,
                    work.ticket.request_id,
                    "patch.canonicalBase",
                )
            })
            .and_then(|base| base.patch(patch))
            .map(|geometry| {
                let cpu_bytes = geometry.owned_vertex_bytes();
                Candidate {
                    geometry: Some(geometry),
                    collision: None,
                    release: false,
                    cpu_bytes,
                }
            }),
        GeneratedOperation3D::ReplaceCollision { collision } => Ok(Candidate {
            geometry: None,
            collision: Some(collision),
            release: false,
            cpu_bytes: input_bytes,
        }),
        GeneratedOperation3D::Release => Ok(Candidate {
            geometry: None,
            collision: None,
            release: true,
            cpu_bytes: 0,
        }),
    };
    result.map_err(|mut e| {
        e.request_id = work.ticket.request_id;
        e
    })
}

impl RuntimeInstanceLoader {
    /// CPU completion is only a candidate. Original GPU/native owners must still prepare,
    /// prove readiness and publish at the existing safe point; never infer Applied here.
    pub(crate) fn advance_generated_content_cpu(&mut self, world: &World) {
        self.generated_content
            .reap_cpu_backings(&mut self.asset_loader);
        if self.generated_content.check_world(world, 0).is_err() {
            self.close_generated_content();
            return;
        }
        let stale: Vec<_> = self
            .generated_content
            .records
            .values()
            .filter(|r| !r.receipt.status.terminal() && !live(&r.receipt.target, world))
            .map(|r| r.receipt.ticket)
            .collect();
        for ticket in stale {
            let failure = error(ContentErrorCode::StaleTarget, ticket.request_id, "target");
            self.generated_content
                .fail(ticket, failure, &mut self.asset_loader)
                .expect("current ticket");
        }
        if self
            .generated_content
            .worker
            .as_ref()
            .is_some_and(Worker::finished)
        {
            let worker = self.generated_content.worker.take().unwrap();
            let ticket = worker.ticket;
            let candidate = worker.join();
            let guard = self.validate_generated_content_commit(ticket, world);
            match (candidate, guard) {
                (Ok(candidate), Ok(())) => {
                    let record = self
                        .generated_content
                        .records
                        .get_mut(&ticket.request_id)
                        .unwrap();
                    if candidate.cpu_bytes <= record.cpu_charge {
                        // Input attachments are gone after join. Only actual candidate backing remains.
                        self.asset_loader
                            .release_prepared_cpu(record.cpu_charge - candidate.cpu_bytes);
                        record.cpu_charge = candidate.cpu_bytes;
                        if let Err(failure) = self
                            .generated_content
                            .retain_candidate_backings(ticket, &candidate)
                        {
                            drop(candidate);
                            self.generated_content
                                .fail(ticket, failure, &mut self.asset_loader)
                                .unwrap();
                        } else {
                            self.generated_content
                                .records
                                .get_mut(&ticket.request_id)
                                .unwrap()
                                .candidate = Some(candidate);
                        }
                    } else {
                        drop(candidate);
                        let failure = error(
                            ContentErrorCode::BudgetExceeded,
                            ticket.request_id,
                            "cpu.candidateBytes",
                        );
                        self.generated_content
                            .fail(ticket, failure, &mut self.asset_loader)
                            .unwrap();
                    }
                }
                (candidate, guard) => {
                    let failure = match candidate {
                        Err(failure) => failure,
                        Ok(candidate) => {
                            drop(candidate);
                            guard.unwrap_err()
                        }
                    };
                    self.generated_content
                        .fail(ticket, failure, &mut self.asset_loader)
                        .unwrap();
                }
            }
            self.generated_content_worker_finished(ticket).unwrap();
        }
        if self.generated_content.worker.is_some() || !self.generated_content.accepting {
            return;
        }
        if let Some(ticket) = self
            .generated_content
            .records
            .values()
            .find(|r| r.receipt.status == ContentStatus::Accepted)
            .map(|r| r.receipt.ticket)
        {
            let base = self
                .generated_content
                .records
                .get(&ticket.request_id)
                .and_then(|r| {
                    self.generated_content
                        .bindings
                        .get(&target_key(&r.receipt.target))
                })
                .and_then(|binding| binding.canonical.clone());
            if let Ok(work) = self.begin_generated_content(ticket, world) {
                match Worker::spawn(
                    work,
                    base,
                    #[cfg(test)]
                    self.generated_content.worker_gate.take(),
                ) {
                    Ok(worker) => self.generated_content.worker = Some(worker),
                    Err(failure) => {
                        self.generated_content
                            .fail(ticket, failure, &mut self.asset_loader)
                            .unwrap();
                        self.generated_content_worker_finished(ticket).unwrap();
                    }
                }
            }
        }
    }
}
