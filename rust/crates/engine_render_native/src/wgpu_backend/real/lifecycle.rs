//! Queue completion and retirement owned by the original wgpu backend.
use super::*;
use crate::platform_time::Instant;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
static NEXT_DEVICE_EPOCH: AtomicU64 = AtomicU64::new(1);
// wgpu allows one lost callback per physical Device. Shared viewport backends must
// share its signal; replacing that callback per view would strand older owners.
type DeviceLossSignals =
    std::sync::Mutex<std::collections::HashMap<(u8, usize), std::sync::Weak<AtomicBool>>>;
static DEVICE_LOSS_SIGNALS: std::sync::OnceLock<DeviceLossSignals> = std::sync::OnceLock::new();
#[cfg(not(target_arch = "wasm32"))]
fn device_identity(device: &wgpu::Device) -> Option<(u8, usize)> {
    // Device Eq is only a core ID, which collides across different Instances.
    // SAFETY: we only take the address behind a live HAL guard. We neither use
    // native operations nor destroy/mutate the HAL resource. The lifecycle keeps
    // a Device clone alive until its map entry has been removed.
    unsafe {
        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "android"))]
        if let Some(guard) = device.as_hal::<wgpu::hal::api::Vulkan>() {
            return Some((1, (&*guard as *const _) as usize));
        }
        #[cfg(target_os = "windows")]
        if let Some(guard) = device.as_hal::<wgpu::hal::api::Dx12>() {
            return Some((2, (&*guard as *const _) as usize));
        }
        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "android"))]
        if let Some(guard) = device.as_hal::<wgpu::hal::api::Gles>() {
            return Some((3, (&*guard as *const _) as usize));
        }
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        if let Some(guard) = device.as_hal::<wgpu::hal::api::Metal>() {
            return Some((4, (&*guard as *const _) as usize));
        }
    }
    None
}
#[cfg(target_arch = "wasm32")]
fn device_identity(_device: &wgpu::Device) -> Option<(u8, usize)> {
    None
}

fn device_loss_signal(device: &wgpu::Device, identity: Option<(u8, usize)>) -> Arc<AtomicBool> {
    let Some(identity) = identity else {
        #[cfg(target_arch = "wasm32")]
        {
            // The browser player owns one backend per Device. WebGPU has no HAL
            // identity; its actual device.lost notification owns this signal.
            let signal = Arc::new(AtomicBool::new(false));
            let callback = Arc::downgrade(&signal);
            device.set_device_lost_callback(move |_, _| {
                if let Some(signal) = callback.upgrade() {
                    signal.store(true, Ordering::Release);
                }
            });
            return signal;
        }
        #[cfg(not(target_arch = "wasm32"))]
        return Arc::new(AtomicBool::new(true));
    };
    let mut signals = DEVICE_LOSS_SIGNALS
        .get_or_init(Default::default)
        .lock()
        .expect("device loss signals poisoned");
    if let Some(signal) = signals.get(&identity).and_then(std::sync::Weak::upgrade) {
        return signal;
    }
    signals.retain(|_, signal| signal.strong_count() > 0);
    let signal = Arc::new(AtomicBool::new(false));
    let callback = Arc::downgrade(&signal);
    device.set_device_lost_callback(move |_, _| {
        if let Some(signal) = callback.upgrade() {
            signal.store(true, Ordering::Release);
        }
    });
    signals.insert(identity, Arc::downgrade(&signal));
    signal
}
const MAX_IN_FLIGHT: usize = 16;
pub(super) const MAX_RETIRING_BYTES: u64 = 128 * 1024 * 1024;
pub(super) const MAX_RETIRING_OBJECTS: usize = 4096;

pub use crate::render_thread::{GpuSubmissionObservation, GpuSubmissionStatus, GpuSubmissionToken};
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuLifecycleStats {
    pub device_epoch: u64,
    pub submitted: u64,
    pub completed: u64,
    pub in_flight: usize,
    pub retiring_bytes: u64,
    pub retiring_objects: usize,
    pub abandoned: bool,
    pub duplicate_presents: u64,
    pub unmatched_observations: u64,
    pub omitted_observations: u64,
}
// Test-only delay of the real queue callback body, after wgpu has delivered it.
// This does not simulate an unfinished GPU submission or a driver TDR.
#[cfg(test)]
pub(crate) type DeferredCompletions =
    Arc<std::sync::Mutex<Vec<Box<dyn FnOnce() + Send + 'static>>>>;

struct Pending {
    token: GpuSubmissionToken,
    leases: Arc<std::sync::Mutex<Option<Vec<Arc<()>>>>>,
}
struct Retired {
    serial: u64,
    bytes: u64,
    _resource: Box<dyn super::memory::RetiredGpuResource>,
}
pub(super) struct GpuLifecycle {
    _device: wgpu::Device,
    identity: Option<(u8, usize)>,
    epoch: u64,
    serial: u64,
    completed: Arc<AtomicU64>,
    lost: Arc<AtomicBool>,
    abandoned: bool,
    #[cfg(test)]
    deferred_completions: Option<DeferredCompletions>,
    pending: VecDeque<Pending>,
    retired: Vec<Retired>,
    last: Option<GpuSubmissionToken>,
    capture: bool,
    observation_capacity: usize,
    observations: VecDeque<GpuSubmissionObservation>,
    duplicates: u64,
    unmatched: u64,
    omitted: u64,
}
impl GpuLifecycle {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        let identity = device_identity(device);
        Self {
            _device: device.clone(),
            identity,
            epoch: NEXT_DEVICE_EPOCH.fetch_add(1, Ordering::Relaxed),
            serial: 0,
            completed: Arc::new(AtomicU64::new(0)),
            lost: device_loss_signal(device, identity),
            abandoned: false,
            #[cfg(test)]
            deferred_completions: None,
            pending: VecDeque::new(),
            retired: Vec::new(),
            last: None,
            capture: false,
            observation_capacity: 2048,
            observations: VecDeque::new(),
            duplicates: 0,
            unmatched: 0,
            omitted: 0,
        }
    }
    pub(super) fn observe_memory<'a>(
        &'a self,
        gpu: &mut super::memory::GpuMemoryCensus<'a>,
        sample: &mut crate::resource_memory::ResourceMemorySample,
    ) {
        if !gpu.is_backing_only() {
            sample.cpu_vec(&self.retired, "renderer", &["retiring"]);
            sample.cpu_deque(&self.pending, "queues", &["queuePayload"]);
            sample.cpu_deque(&self.observations, "instrumentation", &["instrumentation"]);
            for pending in &self.pending {
                let leases = pending
                    .leases
                    .lock()
                    .expect("GPU submission leases poisoned");
                if let Some(values) = leases.as_ref() {
                    sample.cpu_vec(values, "queues", &["queuePayload"]);
                }
            }
        }
        for retired in &self.retired {
            retired._resource.observe(gpu, sample);
        }
        if !gpu.is_backing_only() {
            sample.partial("gpu.role.retiring", "lifecycle.retainedNativeBacking");
        }
    }
    fn completed(&self) -> u64 {
        self.completed.load(Ordering::Acquire)
    }
    fn collect(&mut self) {
        let completed = self.completed();
        self.pending.retain(|p| p.token.serial > completed);
        self.retired.retain(|r| r.serial > completed);
    }
    fn stats(&self) -> GpuLifecycleStats {
        GpuLifecycleStats {
            device_epoch: self.epoch,
            submitted: self.serial,
            completed: self.completed(),
            in_flight: self.pending.len(),
            retiring_bytes: self.retired.iter().map(|r| r.bytes).sum(),
            retiring_objects: self.retired.len(),
            abandoned: self.abandoned,
            duplicate_presents: self.duplicates,
            unmatched_observations: self.unmatched,
            omitted_observations: self.omitted,
        }
    }
}
impl Drop for GpuLifecycle {
    fn drop(&mut self) {
        if let (Some(signals), Some(identity)) = (DEVICE_LOSS_SIGNALS.get(), self.identity) {
            let mut signals = signals.lock().expect("device loss signals poisoned");
            if Arc::strong_count(&self.lost) == 1 {
                signals.remove(&identity);
            }
        }
    }
}
impl RealWgpuBackend {
    pub(super) fn validate_graph_capabilities(&self, plan: &RhiCommandPlan) -> Result<(), String> {
        if self.reuse_geometry_capabilities(plan) {
            return Ok(());
        }
        if let Some(schedule) = &plan.graph_schedule {
            if schedule
                .resource_shapes
                .values()
                .any(|s| s.mip_levels != 1 || s.layers != 1 || s.samples != 1)
            {
                return Err("graph.unsupported_backend_subresources".into());
            }
            if schedule
                .resources
                .iter()
                .any(|r| r.format == crate::render_graph::RenderResourceFormat::Depth24Plus)
            {
                return Err("graph.unsupported_backend_depth_format".into());
            }
        }
        for command in &plan.commands {
            if let RhiCommand::BeginView {
                target,
                view,
                depth_target,
            } = command
            {
                if !view.is_shadow_map() {
                    continue;
                }
                let schedule = plan
                    .graph_schedule
                    .as_ref()
                    .ok_or("shadow.schedule_missing")?;
                let resource = schedule
                    .resources
                    .iter()
                    .find(|r| &r.resource_id == target)
                    .ok_or("shadow.resource_missing")?;
                if depth_target.as_ref() != Some(target)
                    || resource.format != crate::render_graph::RenderResourceFormat::Depth32Float
                    || !resource
                        .usage
                        .contains(&crate::render_graph::RenderResourceUsage::DepthTarget)
                    || !resource
                        .usage
                        .contains(&crate::render_graph::RenderResourceUsage::Sampled)
                    || [resource.size.width, resource.size.height] != view.target_extent
                {
                    return Err("shadow.attachment_descriptor".into());
                }
            }
        }
        self.geometry_color_layout(plan)?;
        self.record_geometry_capabilities(plan);
        Ok(())
    }
    pub fn gpu_lifecycle_stats(&self) -> GpuLifecycleStats {
        let mut stats = self.lifecycle.stats();
        stats.retiring_bytes = stats
            .retiring_bytes
            .saturating_add(self.retained_ui.stats.retiring_bytes as u64);
        stats.retiring_objects = stats
            .retiring_objects
            .saturating_add(self.retained_ui.retiring_count());
        stats
    }
    pub fn last_gpu_submission(&self) -> Option<GpuSubmissionToken> {
        self.lifecycle.last
    }
    #[cfg(test)]
    pub(crate) fn defer_gpu_completion_callbacks_for_test(&mut self) -> DeferredCompletions {
        let deferred = DeferredCompletions::default();
        self.lifecycle.deferred_completions = Some(deferred.clone());
        deferred
    }
    pub fn set_submission_capture_enabled(&mut self, enabled: bool) {
        self.lifecycle.capture = enabled;
        if enabled {
            // Commit the bounded observation history before sampling starts.
            // Configuring a disabled capture keeps its original lazy allocation.
            self.lifecycle.observations.reserve_exact(
                self.lifecycle
                    .observation_capacity
                    .saturating_sub(self.lifecycle.observations.len()),
            );
        } else {
            self.lifecycle.observations.clear();
        }
    }
    pub(super) fn set_submission_capture_capacity(
        &mut self,
        capacity: usize,
    ) -> Result<(), String> {
        if capacity == 0 || capacity > 262_144 {
            return Err("gpu.observation_capacity_limit_1_262144".into());
        }
        self.lifecycle.observation_capacity = capacity;
        while self.lifecycle.observations.len() > capacity {
            self.lifecycle.observations.pop_front();
            self.lifecycle.omitted += 1;
        }
        if self.lifecycle.capture {
            self.lifecycle
                .observations
                .reserve_exact(capacity.saturating_sub(self.lifecycle.observations.len()));
        }
        Ok(())
    }
    /// Returned values are immutable snapshots, never references into a live report.
    pub fn submission_observations(&self) -> Vec<GpuSubmissionObservation> {
        self.lifecycle
            .observations
            .iter()
            .cloned()
            .map(|mut o| {
                o.status = if o.token.serial <= self.lifecycle.completed() {
                    GpuSubmissionStatus::Completed
                } else if self.lifecycle.abandoned {
                    GpuSubmissionStatus::Abandoned
                } else {
                    GpuSubmissionStatus::Submitted
                };
                o
            })
            .collect()
    }
    pub fn submission_status(&self, token: GpuSubmissionToken) -> Option<GpuSubmissionStatus> {
        if token.device_epoch != self.lifecycle.epoch
            || token.serial == 0
            || token.serial > self.lifecycle.serial
        {
            return None;
        }
        Some(if token.serial <= self.lifecycle.completed() {
            GpuSubmissionStatus::Completed
        } else if self.lifecycle.abandoned {
            GpuSubmissionStatus::Abandoned
        } else {
            GpuSubmissionStatus::Submitted
        })
    }
    pub fn record_surface_present(&mut self, token: GpuSubmissionToken) -> bool {
        if token.device_epoch != self.lifecycle.epoch || self.lifecycle.abandoned {
            self.lifecycle.unmatched += 1;
            return false;
        }
        if let Some(o) = self
            .lifecycle
            .observations
            .iter_mut()
            .find(|o| o.token == token)
        {
            o.surface_present_calls += 1;
            if o.surface_present_calls > 1 {
                self.lifecycle.duplicates += 1;
            }
            true
        } else {
            if self.lifecycle.capture {
                self.lifecycle.unmatched += 1;
            }
            false
        }
    }
    pub fn record_submission_capture(&mut self, token: GpuSubmissionToken) -> bool {
        if token.device_epoch != self.lifecycle.epoch || self.lifecycle.abandoned {
            self.lifecycle.unmatched += 1;
            return false;
        }
        if let Some(o) = self
            .lifecycle
            .observations
            .iter_mut()
            .find(|o| o.token == token)
        {
            o.capture_count += 1;
            true
        } else {
            if self.lifecycle.capture {
                self.lifecycle.unmatched += 1;
            }
            false
        }
    }
    pub fn poll_gpu_completion(&mut self) -> Result<GpuLifecycleStats, String> {
        if self.lifecycle.abandoned {
            return Ok(self.gpu_lifecycle_stats());
        }
        if let Err(e) = self.device.poll(wgpu::PollType::Poll) {
            self.abandon_device_epoch();
            return Err(format!("gpu.poll:{e}"));
        }
        if self.lifecycle.lost.load(Ordering::Acquire) {
            self.abandon_device_epoch();
            return Err("gpu.device_lost".into());
        }
        self.lifecycle.collect();
        self.geometry
            .collect_visual_uploads(self.lifecycle.completed());
        self.retained_ui.collect_completed();
        self.collect_geometry_pages();
        Ok(self.gpu_lifecycle_stats())
    }
    pub fn wait_gpu_submission(
        &mut self,
        token: GpuSubmissionToken,
        timeout: Duration,
    ) -> Result<GpuSubmissionStatus, String> {
        if self.submission_status(token).is_none() {
            return Err("gpu.stale_submission".into());
        }
        let start = Instant::now();
        loop {
            self.poll_gpu_completion()?;
            match self.submission_status(token) {
                Some(GpuSubmissionStatus::Submitted) => (),
                Some(s) => return Ok(s),
                None => return Err("gpu.stale_submission".into()),
            }
            if start.elapsed() >= timeout {
                return Err("gpu.completion_timeout".into());
            }
            std::thread::yield_now();
        }
    }
    /// Explicit loss/exit strategy: destroy the backend's device before releasing old allocations.
    /// This never advances the completion counter and late callbacks have only the old atomic.
    pub fn abandon_device_epoch(&mut self) {
        if self.lifecycle.abandoned {
            return;
        }
        self.lifecycle.abandoned = true;
        self.lifecycle.lost.store(true, Ordering::Release);
        self.device.destroy();
        for p in &self.lifecycle.pending {
            p.leases.lock().expect("submission leases poisoned").take();
        }
        self.lifecycle.pending.clear();
        self.lifecycle.retired.clear();
        self.release_geometry_after_device_destroy();
        self.textures.clear();
        self.pending_draw_buffers.clear();
        self.output_textures.clear();
        self.readback_buffers.clear();
        self.font_bundle_identities.clear();
        self.particle_meshes.clear();
        self.particle_effects.clear();
    }
    pub fn shutdown_gpu(&mut self, timeout: Duration) -> Result<GpuLifecycleStats, String> {
        if self.lifecycle.abandoned {
            return Ok(self.gpu_lifecycle_stats());
        }
        let started = Instant::now();
        // Failed preparation may have queued buffer-init writes without a draw
        // submission. Flush through the original ledger, retaining its owners
        // until this actual completion point.
        if !self.pending_draw_buffers.is_empty()
            || self.retained_ui.has_unsubmitted_work()
            || self.geometry.has_queued_visual_uploads()
        {
            // The final flush must also respect the original 16-submission cap.
            if let Some(token) = self.lifecycle.last {
                if let Err(error) =
                    self.wait_gpu_submission(token, timeout.saturating_sub(started.elapsed()))
                {
                    self.abandon_device_epoch();
                    return Err(error);
                }
            }
            self.admit_gpu_frame()?;
            let encoder = self.device.create_command_encoder(&Default::default());
            self.submit_resource_maintenance(encoder);
        }
        if let Some(token) = self.lifecycle.last {
            if let Err(e) =
                self.wait_gpu_submission(token, timeout.saturating_sub(started.elapsed()))
            {
                self.abandon_device_epoch();
                return Err(e);
            }
        }
        self.pending_draw_buffers.clear();
        self.retained_ui.collect(&self.device)?;
        self.lifecycle.collect();
        Ok(self.gpu_lifecycle_stats())
    }
    pub(super) fn admit_gpu_frame(&mut self) -> Result<(), String> {
        self.poll_gpu_completion()?;
        if self.lifecycle.abandoned {
            return Err("gpu.device_epoch_abandoned".into());
        }
        if self.lifecycle.pending.len() >= MAX_IN_FLIGHT
            || self.gpu_lifecycle_stats().retiring_bytes >= MAX_RETIRING_BYTES
        {
            return Err("gpu.in_flight_backpressure".into());
        }
        Ok(())
    }
    pub(super) fn retire_gpu<T: super::memory::RetiredGpuResource + 'static>(
        &mut self,
        resource: T,
        bytes: u64,
    ) {
        if self.lifecycle.serial > self.lifecycle.completed() {
            self.lifecycle.retired.push(Retired {
                serial: self.lifecycle.serial,
                bytes,
                _resource: Box::new(resource),
            });
        }
    }
    pub(super) fn admit_gpu_retirement(&self, bytes: u64, objects: usize) -> Result<(), String> {
        let draw = self.pending_draw_retirement();
        let output = self.pending_output_retirement();
        let pending = (
            draw.0.saturating_add(output.0),
            draw.1.saturating_add(output.1),
        );
        if self.lifecycle.serial > self.lifecycle.completed() || pending.1 != 0 {
            if self
                .lifecycle
                .stats()
                .retiring_bytes
                .saturating_add(self.retained_ui.stats.retiring_bytes as u64)
                .saturating_add(pending.0)
                .saturating_add(bytes)
                > MAX_RETIRING_BYTES
                || self
                    .lifecycle
                    .retired
                    .len()
                    .saturating_add(pending.1)
                    .saturating_add(self.retained_ui.retiring_count())
                    .saturating_add(objects)
                    > MAX_RETIRING_OBJECTS
            {
                return Err("gpu.retirement_backpressure".into());
            }
        }
        Ok(())
    }
    pub(super) fn submit_gpu_frame(
        &mut self,
        encoder: wgpu::CommandEncoder,
        plan: &RhiCommandPlan,
        encoded_cpu_ns: u64,
    ) {
        let frame_leases = collect_submission_leases(plan);
        let leases = Arc::new(std::sync::Mutex::new(Some(frame_leases)));
        self.lifecycle.serial += 1;
        let token = GpuSubmissionToken {
            device_epoch: self.lifecycle.epoch,
            serial: self.lifecycle.serial,
            frame_index: plan.frame_index,
            content_version: plan
                .packet_content_version
                .unwrap_or(self.retained_geometry_stats().installed_content_version),
        };
        let start = Instant::now();
        self.queue.submit(Some(encoder.finish()));
        let submit_cpu_ns = start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        let completed = self.lifecycle.completed.clone();
        let lost = Arc::downgrade(&self.lifecycle.lost);
        let serial = token.serial;
        let callback_leases = leases.clone();
        #[cfg(test)]
        let deferred = self.lifecycle.deferred_completions.clone();
        let completion = move || {
            callback_leases
                .lock()
                .expect("submission leases poisoned")
                .take();
            if lost
                .upgrade()
                .is_some_and(|lost| !lost.load(Ordering::Acquire))
            {
                completed.fetch_max(serial, Ordering::Release);
            }
        };
        self.queue.on_submitted_work_done(move || {
            #[cfg(test)]
            if let Some(deferred) = deferred {
                deferred.lock().unwrap().push(Box::new(completion));
                return;
            }
            completion();
        });
        if let Some(m) = &mut self.measurement {
            m.tag_submission(token, plan.frame_identity.clone());
        }
        self.lifecycle.pending.push_back(Pending { token, leases });
        self.lifecycle.last = Some(token);
        self.geometry.submitted_visual_uploads(token);
        if self.lifecycle.capture {
            if self.lifecycle.observations.len() == self.lifecycle.observation_capacity {
                self.lifecycle.observations.pop_front();
                self.lifecycle.omitted += 1;
            }
            self.lifecycle
                .observations
                .push_back(GpuSubmissionObservation {
                    frame_identity: plan.frame_identity.clone(),
                    offscreen_capture: self.capturing,
                    token,
                    encoded_cpu_ns,
                    submit_cpu_ns,
                    status: GpuSubmissionStatus::Submitted,
                    surface_present_calls: 0,
                    capture_count: 0,
                });
        }
        self.retained_ui.submitted(&self.queue);
    }
}

// One submission borrows its immutable plan. The original completion owner
// retains the resulting exact lease closure until the real queue callback.
fn collect_submission_leases(plan: &RhiCommandPlan) -> Vec<Arc<()>> {
    let mut frame_leases: Vec<Arc<()>> = Vec::new();
    let mut selected_leases = std::collections::HashSet::new();
    for c in &plan.commands {
        if let Some(payload) = c.geometry_payload() {
            if let RhiDrawPayload::GeometryBatch { batch } = payload {
                for entry in &batch.visible {
                    #[cfg(test)]
                    LEASE_COLLECTION_WORK.with(|n| {
                        let mut v = n.get();
                        v[0] += 1;
                        n.set(v);
                    });
                    let lease =
                        batch.instances[entry.instance_index as usize].native_resource_lease();
                    // The last retained Arc already keeps this exact lease
                    // alive. All rows are still visited; only the redundant
                    // hash-set query is skipped. Non-consecutive identities
                    // continue through the original exact membership check.
                    if frame_leases
                        .last()
                        .is_none_or(|last| !Arc::ptr_eq(last, lease))
                        && {
                            #[cfg(test)]
                            LEASE_COLLECTION_WORK.with(|n| {
                                let mut v = n.get();
                                v[1] += 1;
                                n.set(v);
                            });
                            selected_leases.insert(Arc::as_ptr(lease))
                        }
                    {
                        frame_leases.push(lease.clone());
                    }
                }
            }
            let draw = payload.geometry_draw().unwrap();
            if frame_leases
                .last()
                .is_none_or(|last| !Arc::ptr_eq(last, draw.native_resource_lease()))
            {
                frame_leases.push(draw.native_resource_lease().clone());
            }
        }
    }
    frame_leases
}
#[cfg(test)]
thread_local! { static LEASE_COLLECTION_WORK: std::cell::Cell<[usize; 2]> = const { std::cell::Cell::new([0; 2]) }; }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render_contract::MaterialBlend;

    #[test]
    #[ignore = "real GPU device required; bounded capture capacity, no performance claim"]
    fn spine374_f_capture_history_reserves_before_sampling_and_preserves_existing_tokens() {
        let mut backend = RealWgpuBackend::new_offscreen(8, 8).unwrap();
        assert_eq!(backend.lifecycle.observations.capacity(), 0);
        backend.set_submission_capture_capacity(4216).unwrap();
        assert_eq!(
            backend.lifecycle.observations.capacity(),
            0,
            "disabled capture allocated"
        );
        backend.set_submission_capture_enabled(true);
        let capacity = backend.lifecycle.observations.capacity();
        assert!(
            capacity >= 4216,
            "late capture history growth at 4097: {capacity}"
        );
        for serial in 1..=4201 {
            backend
                .lifecycle
                .observations
                .push_back(GpuSubmissionObservation {
                    frame_identity: None,
                    offscreen_capture: false,
                    token: GpuSubmissionToken {
                        device_epoch: backend.lifecycle.epoch,
                        serial,
                        frame_index: serial,
                        content_version: 17,
                    },
                    encoded_cpu_ns: 0,
                    submit_cpu_ns: 0,
                    status: GpuSubmissionStatus::Submitted,
                    surface_present_calls: 0,
                    capture_count: 0,
                });
            assert_eq!(backend.lifecycle.observations.capacity(), capacity);
        }
        assert!(backend.set_submission_capture_capacity(0).is_err());
        assert!(backend.set_submission_capture_capacity(262_145).is_err());
        assert_eq!(backend.lifecycle.observation_capacity, 4216);
        backend.set_submission_capture_enabled(true);
        assert_eq!(backend.lifecycle.observations.len(), 4201);
        assert_eq!(backend.lifecycle.observations.capacity(), capacity);
        backend.set_submission_capture_capacity(6000).unwrap();
        assert!(
            backend.lifecycle.observations.capacity() >= 6000,
            "active resize must reserve"
        );
        assert_eq!(
            backend.lifecycle.observations.front().unwrap().token.serial,
            1
        );
        assert_eq!(
            backend.lifecycle.observations.back().unwrap().token.serial,
            4201
        );
        backend.set_submission_capture_capacity(16).unwrap();
        assert_eq!(backend.lifecycle.observations.len(), 16);
        assert_eq!(backend.lifecycle.omitted, 4185);
        assert_eq!(
            backend.lifecycle.observations.front().unwrap().token.serial,
            4186
        );
        assert_eq!(
            backend
                .lifecycle
                .observations
                .back()
                .unwrap()
                .token
                .content_version,
            17
        );
        backend.set_submission_capture_enabled(false);
        assert!(backend.lifecycle.observations.is_empty());
        assert_eq!(backend.lifecycle.omitted, 4185);
        eprintln!(
            "GPU submission observation bytes={}, initial capacity={capacity}",
            std::mem::size_of::<GpuSubmissionObservation>()
        );
    }

    fn shared_submission_plan(count: usize) -> RhiCommandPlan {
        use crate::render_geometry::{batch_test_frame, batch_test_graph, GeometryVisibleInstance};
        let frame = batch_test_frame(true, MaterialBlend::Opaque, 1, 3);
        let mut plan =
            crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&batch_test_graph(&frame));
        let first = plan
            .commands
            .iter()
            .find_map(RhiCommand::geometry_payload)
            .unwrap()
            .geometry_draw()
            .unwrap()
            .native_instance()
            .clone();
        let table = Arc::new(
            (0..count)
                .map(|index| {
                    if index == 0 {
                        first.clone()
                    } else {
                        let mut instance = first.as_ref().clone();
                        instance.instance_id = index as u64;
                        Arc::new(instance)
                    }
                })
                .collect::<Vec<_>>(),
        );
        for command in &mut plan.commands {
            if let RhiCommand::Draw {
                payload: RhiDrawPayload::GeometryBatch { batch },
                ..
            } = command
            {
                let batch = Arc::make_mut(batch);
                Arc::make_mut(&mut batch.draw).instance = table[0].clone();
                batch.instances = table.clone();
                batch.visible = (0..count)
                    .map(|index| GeometryVisibleInstance {
                        instance_index: index as u32,
                        camera_depth: batch.draw.camera_depth,
                    })
                    .collect::<Vec<_>>()
                    .into();
                batch.validate().unwrap();
            }
        }
        plan
    }

    #[test]
    fn submission_shared_lease_queries_actual_set_once_without_skipping_rows() {
        let mut actual = Vec::new();
        for count in [4, 32, 256] {
            let plan = shared_submission_plan(count);
            LEASE_COLLECTION_WORK.with(|n| n.set([0; 2]));
            let leases = collect_submission_leases(&plan);
            let work = LEASE_COLLECTION_WORK.with(|n| n.get());
            assert_eq!(leases.len(), 1);
            assert_eq!(work[0], count * 3, "selected rows were skipped");
            actual.push((count, work[1]));
        }
        eprintln!("actual submission lease set insertions: {actual:?}");
        assert!(actual.iter().all(|(_, calls)| *calls == 1), "{actual:?}");
    }

    #[test]
    fn submission_lease_closure_keeps_non_head_cow_and_flat_but_not_hidden_rows() {
        let mut plan = shared_submission_plan(4);
        let old = plan.clone();
        let selected = Arc::new(());
        let hidden = Arc::new(());
        let selected_weak = Arc::downgrade(&selected);
        let hidden_weak = Arc::downgrade(&hidden);
        let mut flat = None;
        for command in &mut plan.commands {
            if let RhiCommand::Draw {
                payload: RhiDrawPayload::GeometryBatch { batch },
                ..
            } = command
            {
                let batch = Arc::make_mut(batch);
                let table = Arc::make_mut(&mut batch.instances);
                Arc::make_mut(&mut table[3]).resource_lease = selected.clone();
                let mut unselected = table[3].as_ref().clone();
                unselected.instance_id = 99;
                unselected.resource_lease = hidden.clone();
                table.push(Arc::new(unselected));
                let mut draw = batch.draw.as_ref().clone();
                draw.instance = table[3].clone();
                flat = Some(RhiCommand::Draw {
                    target: draw.target.clone(),
                    draw_kind: crate::rhi_command_plan::RhiDrawKind::Geometry,
                    vertex_count: draw.index_count,
                    payload: RhiDrawPayload::Geometry {
                        draw: Arc::new(draw),
                    },
                });
                batch.validate().unwrap();
            }
        }
        plan.commands.push(flat.unwrap());
        // Independent old-order closure: inspect all selected rows and heads.
        let mut expected = std::collections::BTreeSet::new();
        for command in &plan.commands {
            if let Some(payload) = command.geometry_payload() {
                if let RhiDrawPayload::GeometryBatch { batch } = payload {
                    for entry in &batch.visible {
                        expected.insert(Arc::as_ptr(
                            batch.instances[entry.instance_index as usize].native_resource_lease(),
                        ) as usize);
                    }
                }
                expected.insert(Arc::as_ptr(
                    payload.geometry_draw().unwrap().native_resource_lease(),
                ) as usize);
            }
        }
        let retained = collect_submission_leases(&plan);
        assert_eq!(
            retained
                .iter()
                .map(|lease| Arc::as_ptr(lease) as usize)
                .collect::<std::collections::BTreeSet<_>>(),
            expected
        );
        assert!(collect_submission_leases(&old)
            .iter()
            .all(|lease| !Arc::ptr_eq(lease, &selected)));
        drop((plan, old, selected, hidden));
        assert!(
            selected_weak.upgrade().is_some(),
            "non-head lease missing from submitted closure"
        );
        assert!(hidden_weak.upgrade().is_none(), "unselected row was pinned");
        drop(retained);
        assert!(selected_weak.upgrade().is_none());
    }

    #[test]
    #[ignore = "real GPU required; controlled Device::destroy, not driver TDR"]
    fn graph_lifetime_real_loss_callback_and_late_completion_do_not_touch_rebuilt_epoch() {
        let mut old = RealWgpuBackend::new_offscreen(640, 480).unwrap();
        let mut shared = RealWgpuBackend::from_shared_device_queue(
            &old.device,
            &old.queue,
            wgpu::TextureFormat::Rgba8Unorm,
            640,
            480,
            "f05-loss-consumer",
        );
        assert!(Arc::ptr_eq(&old.lifecycle.lost, &shared.lifecycle.lost));
        fn visible_plan() -> RhiCommandPlan {
            let mut graph = crate::rhi_command_plan::tests::retained_geometry_graph(1);
            let vertices = [-0.7f32, -0.7, 0., 0.7, -0.7, 0., 0., 0.7, 0.]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect::<Vec<_>>();
            for command in graph.passes.iter_mut().flat_map(|pass| &mut pass.commands) {
                if let crate::render_graph::RenderPassCommand::DrawGeometry { draw } = command {
                    let draw = Arc::make_mut(draw);
                    let mut wire = serde_json::to_value(draw.geometry.as_ref()).unwrap();
                    wire["vertexBytes"] = serde_json::json!(vertices);
                    draw.geometry = Arc::new(serde_json::from_value(wire).unwrap());
                    draw.world_transform[14] = (-2f32).into();
                    draw.base_color = [1f32, 0., 0., 1.].map(Into::into);
                }
            }
            crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph)
        }
        let plan = visible_plan();
        let reference = old.render_plan_to_rgba_bytes(&plan, 640, 480).unwrap();
        assert!(reference
            .chunks_exact(4)
            .any(|pixel| pixel != &reference[..4]));
        old.shutdown_gpu(Duration::from_secs(5)).unwrap();
        let completed_before = old.gpu_lifecycle_stats().completed;
        let lease = Arc::downgrade(
            plan.commands
                .iter()
                .find_map(RhiCommand::geometry_payload)
                .unwrap()
                .geometry_draw()
                .unwrap()
                .native_resource_lease(),
        );
        let deferred: DeferredCompletions = Default::default();
        old.lifecycle.deferred_completions = Some(deferred.clone());
        let output = old.create_output_texture(640, 480).unwrap();
        old.render_to_view(&plan, &output.create_view(&Default::default()), None)
            .unwrap();
        let token = old.last_gpu_submission().unwrap();
        assert!(token.serial > completed_before);
        let start = Instant::now();
        while deferred.lock().unwrap().is_empty() {
            old.device.poll(wgpu::PollType::Poll).unwrap();
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "real queue callback not delivered"
            );
            std::thread::yield_now();
        }
        // Actual work has completed; only delivery to our original owner is held.
        assert_eq!(old.gpu_lifecycle_stats().completed, completed_before);
        assert_eq!(
            old.submission_status(token),
            Some(GpuSubmissionStatus::Submitted)
        );
        drop(plan);
        assert!(lease.upgrade().is_some());

        // Trigger wgpu's real registered lost callback without calling our
        // abandon method or directly setting the signal/completion atomics.
        // A real package worker is paused after read/hash and before decode.
        // Keep this operation alive across device loss and new-device recovery.
        let (mut loading, loading_gate, loading_root) =
            crate::runtime_asset_loader::RuntimeAssetLoader::gated_streaming_fixture(2);
        eprintln!("F05 recovery loading fixture: {}", loading_root.display());
        let loading_ref = crate::runtime_package::RuntimeAssetRef {
            id: "sound".into(),
            asset_type: "audio".into(),
            guid: None,
            sub_asset: None,
        };
        let cancelled = loading.request_async(&loading_ref).unwrap();
        let loading_started = Instant::now();
        while !loading_gate.entered.load(Ordering::Acquire) {
            assert!(loading_started.elapsed() < Duration::from_secs(3));
            std::thread::yield_now();
        }
        let old_operation = loading_gate.operation.load(Ordering::Acquire);
        assert!(old_operation > 0);
        assert_ne!(
            *loading_gate.thread.lock().unwrap(),
            Some(std::thread::current().id())
        );
        assert!(loading.streaming_snapshot().reserved_bytes > 0);
        assert_eq!(loading.decoded_cache_len(), 0);
        assert!(!old.lifecycle.lost.load(Ordering::Acquire));
        old.device.destroy();
        let start = Instant::now();
        while !old.lifecycle.lost.load(Ordering::Acquire) {
            let _ = old.device.poll(wgpu::PollType::Poll);
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "real lost callback not delivered"
            );
            std::thread::yield_now();
        }
        assert!(!old.gpu_lifecycle_stats().abandoned);
        assert!(old.poll_gpu_completion().is_err());
        assert!(shared.poll_gpu_completion().is_err());
        assert!(old.gpu_lifecycle_stats().abandoned);
        assert!(shared.gpu_lifecycle_stats().abandoned);
        assert_eq!(
            old.submission_status(token),
            Some(GpuSubmissionStatus::Abandoned)
        );
        assert_eq!(old.gpu_lifecycle_stats().completed, completed_before);
        assert_eq!(old.gpu_lifecycle_stats().in_flight, 0);
        assert_eq!(old.gpu_lifecycle_stats().retiring_objects, 0);
        assert_eq!(old.geometry_residency().physical_mesh_bytes, 0);
        assert!(
            lease.upgrade().is_none(),
            "abandoned native lease still retained"
        );

        // Cancel during recovery, then immediately admit the same source in a
        // new generation. Do not poll old completions until new pixels exist.
        loading.release(&cancelled).unwrap();
        let replacement = loading.request_async(&loading_ref).unwrap();
        assert_ne!(replacement.generation, cancelled.generation);
        assert_eq!(loading.streaming_snapshot().pending_operations, 2);
        assert_eq!(
            loading.get_handle_state(&cancelled),
            Some(crate::runtime_asset::RuntimeAssetLoadState::Released)
        );
        let mut rebuilt = RealWgpuBackend::new_offscreen(640, 480).unwrap();
        let rebuilt_plan = visible_plan();
        assert_ne!(
            token.device_epoch,
            rebuilt.gpu_lifecycle_stats().device_epoch
        );
        assert!(rebuilt.submission_status(token).is_none());
        assert!(!Arc::ptr_eq(&old.lifecycle.lost, &rebuilt.lifecycle.lost));
        let recovered = rebuilt
            .render_plan_to_rgba_bytes(&rebuilt_plan, 640, 480)
            .unwrap();
        assert!(
            recovered == reference,
            "rebuilt device did not recover actual geometry pixels"
        );
        let wait_replacement = Instant::now();
        while loading_gate.operation.load(Ordering::Acquire) == old_operation {
            assert!(wait_replacement.elapsed() < Duration::from_secs(3));
            std::thread::yield_now();
        }
        assert_eq!(
            loading.get_handle_state(&replacement),
            Some(crate::runtime_asset::RuntimeAssetLoadState::Loading)
        );
        assert!(loading.get_audio_clip(&cancelled).is_none());
        loading_gate.blocked.store(false, Ordering::Release);
        while loading.streaming_snapshot().pending_operations != 0
            || loading.streaming_snapshot().pending_tickets != 0
        {
            loading.poll_async();
            assert!(wait_replacement.elapsed() < Duration::from_secs(3));
            std::thread::yield_now();
        }
        assert_eq!(
            loading.get_handle_state(&cancelled),
            Some(crate::runtime_asset::RuntimeAssetLoadState::Released)
        );
        assert!(loading.get_audio_clip(&cancelled).is_none());
        assert_eq!(
            loading.get_handle_state(&replacement),
            Some(crate::runtime_asset::RuntimeAssetLoadState::Ready)
        );
        assert!(loading.get_audio_clip(&replacement).is_some());
        assert_eq!(loading.decoded_cache_len(), 1);
        let mut stale = replacement.clone();
        stale.generation = cancelled.generation;
        assert!(loading.get_handle_state(&stale).is_none());
        assert!(loading.release(&stale).is_err());
        loading.release(&replacement).unwrap();
        assert!(loading.shutdown_streaming(Duration::from_secs(2)));
        assert_eq!(loading.streaming_snapshot().reserved_bytes, 0);
        assert_eq!(loading.streaming_snapshot().cpu_resident_bytes, 0);
        assert_eq!(loading.decoded_cache_len(), 0);
        drop(loading);
        std::fs::remove_dir_all(&loading_root).unwrap();
        assert!(!loading_root.exists());
        rebuilt.shutdown_gpu(Duration::from_secs(5)).unwrap();
        let rebuilt_before = rebuilt.gpu_lifecycle_stats();
        let old_before = old.gpu_lifecycle_stats();
        let callbacks = std::mem::take(&mut *deferred.lock().unwrap());
        assert!(!callbacks.is_empty());
        for callback in callbacks.into_iter().rev() {
            callback();
        }
        assert_eq!(
            old.gpu_lifecycle_stats(),
            old_before,
            "late callback forged old completion"
        );
        assert_eq!(
            rebuilt.gpu_lifecycle_stats(),
            rebuilt_before,
            "late callback touched rebuilt epoch"
        );
        assert!(!rebuilt.lifecycle.lost.load(Ordering::Acquire));
        assert!(
            rebuilt
                .render_plan_to_rgba_bytes(&rebuilt_plan, 640, 480)
                .unwrap()
                == reference
        );
        rebuilt.shutdown_gpu(Duration::from_secs(5)).unwrap();
        assert_eq!(rebuilt.gpu_lifecycle_stats().in_flight, 0);
        assert_eq!(rebuilt.gpu_lifecycle_stats().retiring_objects, 0);
        eprintln!("F05 controlled real loss, shared abandonment, delayed queue callback, new-device geometry recovery passed");
    }

    #[test]
    #[ignore = "real GPU required"]
    fn graph_lifetime_shared_device_loss_reaches_all_backend_owners() {
        let mut source = RealWgpuBackend::new_offscreen(16, 16).unwrap();
        let mut shared = RealWgpuBackend::from_shared_device_queue(
            &source.device,
            &source.queue,
            wgpu::TextureFormat::Rgba8Unorm,
            16,
            16,
            "loss-consumer",
        );
        assert!(Arc::ptr_eq(&source.lifecycle.lost, &shared.lifecycle.lost));
        assert_ne!(source.lifecycle.epoch, shared.lifecycle.epoch);
        source.abandon_device_epoch();
        assert!(shared.poll_gpu_completion().is_err());
        assert!(shared.gpu_lifecycle_stats().abandoned);
        assert_eq!(shared.gpu_lifecycle_stats().completed, 0);
        assert!(shared.admit_gpu_frame().is_err());
        let rebuilt = RealWgpuBackend::new_offscreen(16, 16).unwrap();
        assert!(!rebuilt.gpu_lifecycle_stats().abandoned);
        assert!(!Arc::ptr_eq(
            &shared.lifecycle.lost,
            &rebuilt.lifecycle.lost
        ));
    }
    #[test]
    #[ignore = "real GPU required"]
    fn graph_lifetime_retirement_checks_the_incoming_allocation_before_mutation() {
        let mut backend = RealWgpuBackend::new_offscreen(16, 16).unwrap();
        // Deterministic owner-boundary budget condition, independent of GPU speed.
        backend.lifecycle.serial = 1;
        assert!(backend
            .admit_gpu_retirement(MAX_RETIRING_BYTES + 1, 1)
            .is_err());
        assert!(backend
            .admit_gpu_retirement(0, MAX_RETIRING_OBJECTS + 1)
            .is_err());
        assert!(backend
            .admit_gpu_retirement(MAX_RETIRING_BYTES, MAX_RETIRING_OBJECTS)
            .is_ok());
        assert_eq!(backend.gpu_lifecycle_stats().retiring_objects, 0);
        backend.lifecycle.serial = 0;
    }
    #[test]
    #[ignore = "S2-A real texture owner, deterministic completion boundary"]
    fn s2a_texture_replacement_keeps_old_storage_until_its_completion() {
        let mut backend = RealWgpuBackend::new_offscreen(16, 16).unwrap();
        let handle = RenderResourceHandle {
            kind: crate::render_resource::RenderResourceKind::Texture,
            index: 217,
            generation: 1,
        };
        backend
            .register_rgba8_texture(handle, 1, 1, &[255, 0, 0, 255], "nearestClamp")
            .unwrap();
        let old = Arc::downgrade(&backend.textures[&handle]._texture);
        // Same deterministic owner seam as the retirement admission test above.
        backend.lifecycle.serial = 1;
        backend
            .register_rgba8_texture(handle, 1, 1, &[0, 255, 0, 255], "nearestClamp")
            .unwrap();
        assert!(
            old.upgrade().is_some(),
            "replacement destroyed old storage before completion"
        );
        assert_eq!(backend.gpu_lifecycle_stats().retiring_bytes, 4);
        backend.lifecycle.completed.store(1, Ordering::Release);
        backend.lifecycle.collect();
        assert!(old.upgrade().is_none());
        assert_eq!(backend.gpu_lifecycle_stats().retiring_bytes, 0);
        backend.lifecycle.serial = 0;
    }
}
