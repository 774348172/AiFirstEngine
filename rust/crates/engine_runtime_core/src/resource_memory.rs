//! Opt-in allocation observations. IDs are local to one frame/device capture.
//! Bytes describe requested backing storage/capacity, excluding allocator/driver
//! overhead. Coverage must say so; tracked bytes are never a complete heap claim.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MemoryDomain {
    Cpu,
    Gpu,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AllocationSample {
    pub allocation_id: String,
    pub domain: MemoryDomain,
    pub owner: String,
    pub bytes: u64,
    /// Tags may overlap (e.g. resident + retiring); allocation bytes count once.
    pub roles: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MemoryCoverage {
    Complete,
    Partial,
    Unavailable,
}

/// Driver budget for this process on the physical device used by the Player.
/// This is budget headroom, not physical free VRAM or engine-owned allocations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GpuMemoryBudgetSample {
    pub source: String,
    pub adapter_uuid: String,
    pub local_heaps: Vec<GpuLocalHeapBudget>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GpuLocalHeapBudget {
    pub heap_index: u32,
    pub heap_size: u64,
    pub budget: u64,
    pub usage: u64,
}
impl GpuMemoryBudgetSample {
    pub fn available_bytes(&self) -> Option<u64> {
        if self.source != "VK_EXT_memory_budget"
            || self.adapter_uuid.len() != 32
            || !self.adapter_uuid.bytes().all(|c| c.is_ascii_hexdigit())
            || self.adapter_uuid.bytes().all(|c| c == b'0')
            || self.local_heaps.is_empty()
            || self.local_heaps.len() > 16
        {
            return None;
        }
        let mut seen = BTreeSet::new();
        self.local_heaps.iter().try_fold(0u64, |sum, heap| {
            if heap.heap_index >= 16
                || !seen.insert(heap.heap_index)
                || heap.heap_size == 0
                || heap.budget == 0
                || heap.budget > heap.heap_size
            {
                return None;
            }
            sum.checked_add(heap.budget.saturating_sub(heap.usage))
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceMemorySample {
    pub frame_index: u64,
    /// The existing Extract identity; never reconstructed from the latest World.
    pub frame_identity: Option<std::sync::Arc<crate::render_thread::RenderFrameIdentity>>,
    pub device_epoch: u64,
    pub capture_started_ns: u64,
    pub capture_finished_ns: u64,
    pub allocations: Vec<AllocationSample>,
    /// Keys are domain.owner.name or domain.role.name. Missing is unknown.
    pub coverage: BTreeMap<String, MemoryCoverage>,
    /// Narrow storage scopes actually visited, never a claim of all-owner coverage.
    pub observed_scopes: BTreeSet<String>,
    pub process_private_bytes: Option<u64>,
    pub system_available_bytes: Option<u64>,
    /// Driver-reported availability on the matching adapter; never VRAM - tracked.
    pub gpu_available_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_memory_budget: Option<GpuMemoryBudgetSample>,
    pub capacities: BTreeMap<String, u64>,
}
impl ResourceMemorySample {
    pub fn new(frame_index: u64, device_epoch: u64, started_ns: u64) -> Self {
        Self {
            frame_index,
            frame_identity: None,
            device_epoch,
            capture_started_ns: started_ns,
            capture_finished_ns: started_ns,
            allocations: Vec::new(),
            coverage: BTreeMap::new(),
            observed_scopes: BTreeSet::new(),
            process_private_bytes: None,
            system_available_bytes: None,
            gpu_available_bytes: None,
            gpu_memory_budget: None,
            capacities: BTreeMap::new(),
        }
    }
    /// Borrowed identity only: no ownership is retained and no pointers are dereferenced.
    /// Simultaneously live CPU backing stores have distinct addresses; cross-frame
    /// address reuse is allowed because the containing capture is the identity scope.
    #[doc(hidden)]
    pub fn cpu_storage(&mut self, address: usize, bytes: usize, owner: &str, roles: &[&str]) {
        if bytes == 0 {
            return;
        }
        self.allocations.push(AllocationSample {
            allocation_id: format!("cpu-storage:{address:x}"),
            domain: MemoryDomain::Cpu,
            owner: owner.into(),
            bytes: bytes as u64,
            roles: roles.iter().map(|r| (*r).into()).collect(),
        });
    }
    /// Requested data storage only. Arc control blocks/allocator overhead are
    /// not observable through this API and are excluded explicitly.
    #[doc(hidden)]
    pub fn cpu_arc<T: ?Sized>(&mut self, value: &std::sync::Arc<T>, owner: &str, roles: &[&str]) {
        self.cpu_storage(
            std::sync::Arc::as_ptr(value) as *const () as usize,
            std::mem::size_of_val(value.as_ref()),
            owner,
            roles,
        );
    }
    #[doc(hidden)]
    pub fn cpu_vec<T>(&mut self, value: &Vec<T>, owner: &str, roles: &[&str]) {
        self.cpu_storage(
            value.as_ptr() as usize,
            value.capacity() * std::mem::size_of::<T>(),
            owner,
            roles,
        );
    }
    /// VecDeque exclusively owns its backing. Its object identity denotes that
    /// allocation for this borrowed capture; std does not expose the allocation
    /// base for an unwrapped deque. Clones own distinct stores, aliases dedup.
    #[doc(hidden)]
    pub fn cpu_deque<T>(
        &mut self,
        value: &std::collections::VecDeque<T>,
        owner: &str,
        roles: &[&str],
    ) {
        let bytes = value.capacity() * std::mem::size_of::<T>();
        if bytes == 0 {
            return;
        }
        self.allocations.push(AllocationSample {
            allocation_id: format!("cpu-owned-deque:{:x}", value as *const _ as usize),
            domain: MemoryDomain::Cpu,
            owner: owner.into(),
            bytes: bytes as u64,
            roles: roles.iter().map(|r| (*r).into()).collect(),
        });
    }
    #[doc(hidden)]
    pub fn cpu_string(&mut self, value: &String, owner: &str, roles: &[&str]) {
        self.cpu_storage(value.as_ptr() as usize, value.capacity(), owner, roles);
    }
    #[doc(hidden)]
    pub fn partial(&mut self, key: &str, scope: &str) {
        self.coverage.insert(key.into(), MemoryCoverage::Partial);
        self.observed_scopes.insert(scope.into());
    }
}

/// Native system reserve. This is neither process working set nor GPU availability.
pub fn system_available_bytes() -> Option<u64> {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct MemoryStatus {
            length: u32,
            load: u32,
            total_phys: u64,
            avail_phys: u64,
            total_page: u64,
            avail_page: u64,
            total_virtual: u64,
            avail_virtual: u64,
            avail_extended: u64,
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GlobalMemoryStatusEx(status: *mut MemoryStatus) -> i32;
        }
        let mut status = MemoryStatus {
            length: std::mem::size_of::<MemoryStatus>() as u32,
            load: 0,
            total_phys: 0,
            avail_phys: 0,
            total_page: 0,
            avail_page: 0,
            total_virtual: 0,
            avail_virtual: 0,
            avail_extended: 0,
        };
        // SAFETY: native MEMORYSTATUSEX layout, initialized size and writable lifetime.
        if unsafe { GlobalMemoryStatusEx(&mut status) } == 0 {
            None
        } else {
            Some(status.avail_phys)
        }
    }
    #[cfg(not(windows))]
    {
        None
    }
}

pub const MEMORY_OWNERS: &[&str] = &[
    "world",
    "animation",
    "physics",
    "navigation",
    "assets",
    "queues",
    "snapshots",
    "staging",
    "renderer",
    "instrumentation",
    "other",
];
pub const MEMORY_ROLES: &[&str] = &[
    "resident",
    "decodedCache",
    "staging",
    "retiring",
    "decodeWork",
    "queuePayload",
    "snapshot",
    "instrumentation",
    "other",
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn memory_cpu_capacity_and_shared_storage_keep_physical_identity() {
        let mut storage = Vec::<u8>::with_capacity(4096);
        storage.push(1);
        let shared = std::sync::Arc::new(storage);
        let alias = shared.clone();
        let mut s = ResourceMemorySample::new(1, 1, 0);
        s.cpu_vec(&shared, "assets", &["decodedCache"]);
        s.cpu_vec(&alias, "assets", &["snapshot"]);
        assert_eq!(
            s.allocations[0].allocation_id,
            s.allocations[1].allocation_id
        );
        assert_eq!(s.allocations[0].bytes, 4096);
        let detached = shared.as_ref().clone();
        s.cpu_vec(&detached, "assets", &["decodedCache"]);
        assert_ne!(
            s.allocations[0].allocation_id,
            s.allocations[2].allocation_id
        );
        let empty: Vec<u8> = Vec::new();
        s.cpu_vec(&empty, "other", &["other"]);
        assert_eq!(s.allocations.len(), 3);
    }
    #[test]
    fn memory_arc_data_and_wrapped_deque_aliases_count_once_with_roles() {
        let value = std::sync::Arc::new([7u64; 4]);
        let alias = value.clone();
        let mut queue = std::collections::VecDeque::with_capacity(16);
        queue.extend(0u64..16);
        queue.pop_front();
        queue.push_back(16);
        assert!(!queue.as_slices().1.is_empty());
        let detached = queue.clone();
        let mut s = ResourceMemorySample::new(1, 1, 0);
        s.cpu_arc(&value, "snapshots", &["snapshot"]);
        s.cpu_arc(&alias, "snapshots", &["retiring"]);
        s.cpu_deque(&queue, "queues", &["queuePayload"]);
        s.cpu_deque(&queue, "queues", &["retiring"]);
        s.cpu_deque(&detached, "queues", &["queuePayload"]);
        let totals = memory_totals(&s.allocations).unwrap();
        assert_eq!(totals.unique_allocations, 3);
        assert_eq!(
            totals.cpu_allocated,
            (32 + (queue.capacity() + detached.capacity()) * 8) as u64
        );
        assert_eq!(
            totals.by_role["cpu.retiring"],
            (32 + queue.capacity() * 8) as u64
        );
        assert_eq!(
            std::sync::Arc::strong_count(&value),
            2,
            "observation must not retain shared owners"
        );
    }
    #[test]
    fn memory_windows_system_reserve_comes_from_platform() {
        #[cfg(windows)]
        assert!(system_available_bytes().is_some_and(|v| v > 0));
        #[cfg(not(windows))]
        assert_eq!(system_available_bytes(), None);
    }
}

#[cfg(test)]
mod driver_budget_tests {
    use super::*;
    #[test]
    fn memory_reduction_preserves_alias_union_domains_zero_and_failures() {
        let allocation = |id: &str, domain, owner: &str, bytes, roles: &[&str]| AllocationSample {
            allocation_id: id.into(),
            domain,
            owner: owner.into(),
            bytes,
            roles: roles.iter().map(|r| (*r).into()).collect(),
        };
        let mut records: Vec<_> = (0..4096)
            .map(|i| {
                allocation(
                    &format!("retained:{i}"),
                    MemoryDomain::Cpu,
                    "assets",
                    32,
                    &["decodedCache"],
                )
            })
            .collect();
        records.push(allocation(
            "retained:0",
            MemoryDomain::Cpu,
            "assets",
            32,
            &["snapshot", "retiring"],
        ));
        records.push(allocation(
            "retained:0",
            MemoryDomain::Gpu,
            "renderer",
            64,
            &["resident"],
        ));
        records.push(allocation(
            "zero",
            MemoryDomain::Cpu,
            "queues",
            0,
            &["queuePayload"],
        ));
        let totals = memory_totals(&records).unwrap();
        assert_eq!(totals.unique_allocations, 4098);
        assert_eq!(totals.cpu_allocated, 4096 * 32);
        assert_eq!(totals.gpu_allocated, 64);
        assert_eq!(totals.by_owner["cpu.queues"], 0);
        assert_eq!(totals.by_role["cpu.queuePayload"], 0);
        assert_eq!(totals.by_role["cpu.decodedCache"], 4096 * 32);
        assert_eq!(totals.by_role["cpu.snapshot"], 32);
        assert_eq!(totals.by_role["cpu.retiring"], 32);
        for changed in [
            allocation("retained:0", MemoryDomain::Cpu, "assets", 33, &["snapshot"]),
            allocation("retained:0", MemoryDomain::Cpu, "world", 32, &["snapshot"]),
        ] {
            records.push(changed);
            assert_eq!(
                memory_totals(&records).unwrap_err().code,
                "measurement.allocation_conflict"
            );
            records.pop();
        }
        for bad in [
            allocation(" ", MemoryDomain::Cpu, "assets", 0, &["resident"]),
            allocation("bad", MemoryDomain::Cpu, "unknown", 0, &["resident"]),
            allocation("bad", MemoryDomain::Cpu, "assets", 0, &[]),
            allocation("bad", MemoryDomain::Cpu, "assets", 0, &["unknown"]),
        ] {
            assert_eq!(
                memory_totals(&[bad]).unwrap_err().code,
                "measurement.allocation_identity"
            );
        }
        assert_eq!(
            memory_totals(&[
                allocation("a", MemoryDomain::Cpu, "assets", u64::MAX, &["resident"]),
                allocation("b", MemoryDomain::Cpu, "assets", 1, &["resident"])
            ])
            .unwrap_err()
            .code,
            "measurement.memory_overflow"
        );
    }

    fn budget() -> GpuMemoryBudgetSample {
        GpuMemoryBudgetSample {
            source: "VK_EXT_memory_budget".into(),
            adapter_uuid: "ab".repeat(16),
            local_heaps: vec![
                GpuLocalHeapBudget {
                    heap_index: 0,
                    heap_size: 1024,
                    budget: 800,
                    usage: 300,
                },
                GpuLocalHeapBudget {
                    heap_index: 1,
                    heap_size: 512,
                    budget: 400,
                    usage: 100,
                },
            ],
        }
    }
    #[test]
    fn driver_budget_sums_distinct_local_heaps_and_exhaustion_is_known_zero() {
        let mut b = budget();
        assert_eq!(b.available_bytes(), Some(800));
        b.local_heaps[0].usage = 900;
        assert_eq!(b.available_bytes(), Some(300));
        b.local_heaps[1].usage = 500;
        assert_eq!(b.available_bytes(), Some(0));
    }
    #[test]
    fn driver_budget_rejects_unknown_provenance_duplicate_heap_and_invalid_budget() {
        let mut b = budget();
        b.source = "estimated".into();
        assert_eq!(b.available_bytes(), None);
        let mut b = budget();
        b.adapter_uuid = "0".repeat(32);
        assert_eq!(b.available_bytes(), None);
        let mut b = budget();
        b.local_heaps[1].heap_index = 0;
        assert_eq!(b.available_bytes(), None);
        for value in [0, 1025] {
            let mut b = budget();
            b.local_heaps[0].budget = value;
            assert_eq!(b.available_bytes(), None);
        }
        let mut b = budget();
        b.local_heaps.clear();
        assert_eq!(b.available_bytes(), None);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryObservationError {
    pub code: &'static str,
    pub message: String,
}
impl MemoryObservationError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryTotals {
    pub cpu_allocated: u64,
    pub gpu_allocated: u64,
    pub by_owner: BTreeMap<String, u64>,
    pub by_role: BTreeMap<String, u64>,
    pub unique_allocations: usize,
}
/// One instantaneous snapshot, never a sum of asynchronously sampled category peaks.
pub fn memory_totals(samples: &[AllocationSample]) -> Result<MemoryTotals, MemoryObservationError> {
    // Borrow immutable census records. Deep-cloning every identity and role here
    // makes retained loader metadata cause hundreds of thousands of allocations
    // per frame in long streaming diagnostics.
    let mut unique: std::collections::HashMap<(bool, &str), (&AllocationSample, usize, u16)> =
        std::collections::HashMap::with_capacity(samples.len());
    for sample in samples {
        let owner = MEMORY_OWNERS.iter().position(|v| *v == sample.owner);
        let mut roles = 0u16;
        for role in &sample.roles {
            let Some(index) = MEMORY_ROLES.iter().position(|v| *v == role) else {
                return Err(MemoryObservationError::new(
                    "measurement.allocation_identity",
                    "explicit allocation/owner/roles required",
                ));
            };
            roles |= 1 << index;
        }
        if sample.allocation_id.trim().is_empty() || owner.is_none() || roles == 0 {
            return Err(MemoryObservationError::new(
                "measurement.allocation_identity",
                "explicit allocation/owner/roles required",
            ));
        }
        let key = (
            sample.domain == MemoryDomain::Gpu,
            sample.allocation_id.as_str(),
        );
        if let Some((prior, _, prior_roles)) = unique.get_mut(&key) {
            if prior.bytes != sample.bytes || prior.owner != sample.owner {
                return Err(MemoryObservationError::new(
                    "measurement.allocation_conflict",
                    sample.allocation_id.clone(),
                ));
            }
            *prior_roles |= roles;
        } else {
            unique.insert(key, (sample, owner.unwrap(), roles));
        }
    }
    let mut result = MemoryTotals {
        cpu_allocated: 0,
        gpu_allocated: 0,
        by_owner: BTreeMap::new(),
        by_role: BTreeMap::new(),
        unique_allocations: unique.len(),
    };
    let mut owners = [[None::<u64>; MEMORY_OWNERS.len()]; 2];
    let mut roles = [[None::<u64>; MEMORY_ROLES.len()]; 2];
    let add = |target: &mut u64, bytes: u64| -> Result<(), MemoryObservationError> {
        *target = target.checked_add(bytes).ok_or_else(|| {
            MemoryObservationError::new("measurement.memory_overflow", "allocation sum overflows")
        })?;
        Ok(())
    };
    for &(sample, owner, mask) in unique.values() {
        let domain = match sample.domain {
            MemoryDomain::Cpu => {
                add(&mut result.cpu_allocated, sample.bytes)?;
                0
            }
            MemoryDomain::Gpu => {
                add(&mut result.gpu_allocated, sample.bytes)?;
                1
            }
        };
        add(owners[domain][owner].get_or_insert(0), sample.bytes)?;
        for (index, value) in roles[domain].iter_mut().enumerate() {
            if mask & (1 << index) != 0 {
                add(value.get_or_insert(0), sample.bytes)?;
            }
        }
    }
    for (domain, prefix) in ["cpu", "gpu"].iter().enumerate() {
        for (index, bytes) in owners[domain].iter().enumerate() {
            if let Some(bytes) = bytes {
                result
                    .by_owner
                    .insert(format!("{prefix}.{}", MEMORY_OWNERS[index]), *bytes);
            }
        }
        for (index, bytes) in roles[domain].iter().enumerate() {
            if let Some(bytes) = bytes {
                result
                    .by_role
                    .insert(format!("{prefix}.{}", MEMORY_ROLES[index]), *bytes);
            }
        }
    }
    Ok(result)
}

#[path = "resource_memory/run.rs"]
mod run;
pub use run::*;
