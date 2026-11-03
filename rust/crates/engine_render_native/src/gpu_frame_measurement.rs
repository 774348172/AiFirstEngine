//! Bounded opt-in capture on the renderer queue. No mapping or waits during frames.
//! Timestamp resolution/header copies follow the measured commands; one readback at finish.
use crate::particle_gpu::ParticleGpuEffect;
use crate::windowed_player::{WindowedPlayerGpuFrameSample, WindowedPlayerGpuPerformance};
use std::collections::BTreeMap;

const QUERIES: u32 = 512;
const EMITTERS: usize = 256;
const QUERY_BYTES: u64 = QUERIES as u64 * 8;
const STRIDE: u64 = QUERY_BYTES + EMITTERS as u64 * 64;
pub const MAX_MEASUREMENT_FRAMES: u64 = 131_072;
const BUFFER_BUDGET: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy)]
struct SampleLayout {
    base: u64,
    headers: u64,
    end: u64,
}
impl SampleLayout {
    fn new(base: u64, queries: u32, emitters: usize, budget: u64) -> Result<Self, String> {
        if queries == 0 || queries > QUERIES || emitters > EMITTERS {
            return Err("gpu_measurement.invalid_sample_layout".into());
        }
        let headers = base + (u64::from(queries) * 8).div_ceil(256) * 256;
        let end = headers + (emitters as u64 * 64).div_ceil(256) * 256;
        if end > budget {
            return Err("gpu_measurement.buffer_budget_exceeded".into());
        }
        Ok(Self { base, headers, end })
    }
}

pub fn features(available: wgpu::Features, enabled: bool) -> wgpu::Features {
    let required =
        wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
    if enabled && available.contains(required) {
        required
    } else {
        wgpu::Features::empty()
    }
}

pub struct GpuFrameMeasurement {
    report: WindowedPlayerGpuPerformance,
    query: Option<wgpu::QuerySet>,
    results: Option<wgpu::Buffer>,
    warmup: u64,
    requested: u64,
    frame: u64,
    active: bool,
    next_query: u32,
    // One backing store for all retained ranges, indexed by immutable frame spans.
    // The census must not allocate/visit one record per historical frame.
    ranges: Vec<(u32, u32, u8)>,
    sample_ranges: Vec<std::ops::Range<usize>>,
    current_ranges: Vec<(u32, u32, u8)>,
    current: WindowedPlayerGpuFrameSample,
    layouts: Vec<SampleLayout>,
    used_bytes: u64,
}

impl GpuFrameMeasurement {
    pub(crate) fn observe_memory<'a>(
        &'a self,
        gpu: &mut crate::wgpu_backend::real::memory::GpuMemoryCensus<'a>,
        sample: &mut crate::resource_memory::ResourceMemorySample,
    ) {
        sample.cpu_vec(&self.ranges, "instrumentation", &["instrumentation"]);
        sample.cpu_vec(&self.sample_ranges, "instrumentation", &["instrumentation"]);
        sample.cpu_vec(
            &self.current_ranges,
            "instrumentation",
            &["instrumentation"],
        );
        sample.cpu_vec(&self.layouts, "instrumentation", &["instrumentation"]);
        sample.cpu_vec(
            &self.report.samples,
            "instrumentation",
            &["instrumentation"],
        );
        sample.partial(
            "cpu.owner.instrumentation",
            "measurement.histories.capacity",
        );
        if let Some(buffer) = &self.results {
            gpu.buffer(buffer, "instrumentation", &["instrumentation"]);
        }
        sample.partial(
            "gpu.owner.instrumentation",
            "measurement.results.allocatedBufferSize",
        );
        sample.partial(
            "gpu.role.instrumentation",
            "measurement.results.allocatedBufferSize",
        );
    }

    pub(crate) fn tag_submission(
        &mut self,
        token: crate::render_thread::GpuSubmissionToken,
        identity: Option<std::sync::Arc<crate::render_thread::RenderFrameIdentity>>,
    ) {
        if let Some(sample) = self
            .report
            .samples
            .last_mut()
            .filter(|s| s.frame_index == self.frame && s.submission.is_none())
        {
            sample.submission = Some(token);
            sample.frame_index = token.frame_index;
            sample.frame_identity = identity;
        }
    }
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        info: &wgpu::AdapterInfo,
        warmup: u64,
        requested: u64,
    ) -> Result<Self, String> {
        if requested == 0 || requested > MAX_MEASUREMENT_FRAMES {
            return Err("gpu_measurement.sample_limit_1_131072".into());
        }
        let buffer_bytes = (STRIDE * requested)
            .min(BUFFER_BUDGET)
            .min(device.limits().max_buffer_size);
        let supported = !features(device.features(), true).is_empty();
        let query = supported.then(|| {
            device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("explicit-frame-timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count: QUERIES,
            })
        });
        let results = supported.then(|| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("explicit-frame-results"),
                size: buffer_bytes,
                usage: wgpu::BufferUsages::QUERY_RESOLVE
                    | wgpu::BufferUsages::COPY_SRC
                    | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });
        Ok(Self {
            report: WindowedPlayerGpuPerformance {
                native_present: None,
                submissions: Vec::new(),
                status: if supported {
                    "collecting"
                } else {
                    "unsupported_timestamp"
                }
                .into(),
                adapter: info.name.clone(),
                driver: format!("{} {}", info.driver, info.driver_info),
                backend: format!("{:?}", info.backend),
                timestamp_period_ns: queue.get_timestamp_period(),
                measurement_buffer_bytes: if supported { buffer_bytes } else { 0 },
                measurement_used_bytes: 0,
                submission_observations_omitted: 0,
                frame: None,
                samples: Vec::new(),
            },
            query,
            results,
            warmup,
            requested,
            frame: 0,
            active: false,
            next_query: 0,
            ranges: Vec::new(),
            sample_ranges: Vec::new(),
            current_ranges: Vec::new(),
            current: Default::default(),
            layouts: Vec::new(),
            used_bytes: 0,
        })
    }
    pub fn begin(&mut self, encoder: &mut wgpu::CommandEncoder) {
        self.frame += 1;
        self.active = self.query.is_some()
            && self.frame > self.warmup
            && self.frame - self.warmup <= self.requested;
        self.next_query = 0;
        self.current_ranges.clear();
        self.current = WindowedPlayerGpuFrameSample {
            frame_index: self.frame,
            ..Default::default()
        };
        if self.active {
            encoder.write_timestamp(self.query.as_ref().unwrap(), 0);
            self.next_query = 1;
        }
    }
    pub(crate) fn active(&self) -> bool {
        self.active
    }
    pub(crate) fn spine_draws(&mut self, draws: u64, instances: u64) {
        if self.active {
            self.current.spine_draw_count += draws;
            self.current.spine_instances += instances;
        }
    }
    pub(crate) fn spine_mixed_source_batch(&mut self) {
        if self.active {
            self.current.spine_mixed_source_batches += 1;
        }
    }
    pub fn mark(&mut self, encoder: &mut wgpu::CommandEncoder) -> Result<Option<u32>, String> {
        if !self.active {
            return Ok(None);
        }
        if self.next_query >= QUERIES {
            return Err("gpu_measurement.query_limit_512".into());
        }
        let index = self.next_query;
        encoder.write_timestamp(self.query.as_ref().unwrap(), index);
        self.next_query += 1;
        Ok(Some(index))
    }
    pub fn end_range(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        start: Option<u32>,
        kind: u8,
    ) -> Result<(), String> {
        if let Some(start) = start {
            if let Some(end) = self.mark(encoder)? {
                self.current_ranges.push((start, end, kind));
            }
        }
        Ok(())
    }
    pub fn dispatches(&mut self, count: u64) {
        if self.active {
            self.current.dispatch_count += count;
        }
    }
    pub fn draw(&mut self) {
        if self.active {
            self.current.particle_draw_count += 1;
        }
    }
    pub fn end(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        effects: &BTreeMap<u64, ParticleGpuEffect>,
    ) -> Result<(), String> {
        if !self.active {
            return Ok(());
        }
        self.end_range(encoder, Some(0), 0)?;
        let count: usize = effects.values().map(|e| e.emitters.len()).sum();
        if count > EMITTERS {
            return Err("gpu_measurement.emitter_limit_256".into());
        }
        let layout = SampleLayout::new(
            self.used_bytes,
            self.next_query,
            count,
            self.report.measurement_buffer_bytes,
        )?;
        let base = layout.base;
        let results = self.results.as_ref().unwrap();
        encoder.resolve_query_set(
            self.query.as_ref().unwrap(),
            0..self.next_query,
            results,
            base,
        );
        for (i, emitter) in effects.values().flat_map(|e| &e.emitters).enumerate() {
            let offset = layout.headers + i as u64 * 64;
            encoder.copy_buffer_to_buffer(&emitter.scratch, 0, results, offset, 48);
            if let Some(render) = &emitter.render {
                encoder.copy_buffer_to_buffer(&render.indirect, 0, results, offset + 48, 16);
            }
        }
        self.current.effect_count = effects.len();
        self.current.emitter_count = count;
        self.current.particle_buffer_bytes =
            effects.values().map(ParticleGpuEffect::buffer_bytes).sum();
        self.report.samples.push(self.current.clone());
        self.layouts.push(layout);
        self.used_bytes = layout.end;
        self.report.measurement_used_bytes = self.used_bytes;
        self.retain_current_ranges();
        self.active = false;
        Ok(())
    }
    fn retain_current_ranges(&mut self) {
        let start = self.ranges.len();
        self.ranges.extend_from_slice(&self.current_ranges);
        self.sample_ranges.push(start..self.ranges.len());
        self.current_ranges.clear();
    }
    #[cfg(test)]
    fn ranges_for_frame(&self, frame: usize) -> &[(u32, u32, u8)] {
        &self.ranges[self.sample_ranges[frame].clone()]
    }
    pub fn finish(
        mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> WindowedPlayerGpuPerformance {
        if self.query.is_none() {
            return self.report;
        }
        match self.read(device, queue) {
            Ok(()) => {
                self.report.frame = summarize_gpu_frames(&self.report.samples);
                self.report.status = if self.report.samples.len() as u64 == self.requested {
                    "complete"
                } else {
                    "incomplete_samples"
                }
                .into()
            }
            Err(error) => {
                self.report.status = error;
                self.report.samples.clear();
            }
        }
        self.report
    }
    fn read(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> Result<(), String> {
        if self.report.samples.is_empty() {
            return Ok(());
        }
        let size = self.used_bytes;
        let target = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("explicit-frame-readback"),
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(self.results.as_ref().unwrap(), 0, &target, 0, size);
        queue.submit(Some(encoder.finish()));
        let (tx, rx) = std::sync::mpsc::channel();
        target.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        // Poll without blocking the device indefinitely; this is outside timed frames.
        let deadline = crate::platform_time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            device
                .poll(wgpu::PollType::Poll)
                .map_err(|e| format!("gpu_measurement.poll:{e}"))?;
            match rx.try_recv() {
                Ok(r) => {
                    r.map_err(|e| format!("gpu_measurement.map:{e}"))?;
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Empty)
                    if crate::platform_time::Instant::now() < deadline =>
                {
                    std::thread::sleep(std::time::Duration::from_millis(1))
                }
                _ => return Err("gpu_measurement.readback_timeout".into()),
            }
        }
        let data = target.slice(..).get_mapped_range();
        for (i, sample) in self.report.samples.iter_mut().enumerate() {
            let layout = self.layouts[i];
            let bytes = &data[layout.base as usize..layout.end as usize];
            for &(start, end, kind) in &self.ranges[self.sample_ranges[i].clone()] {
                let stamp = |n: u32| {
                    u64::from_le_bytes(
                        bytes[n as usize * 8..n as usize * 8 + 8]
                            .try_into()
                            .unwrap(),
                    )
                };
                let ticks = stamp(end)
                    .checked_sub(stamp(start))
                    .ok_or("gpu_measurement.nonmonotonic_timestamp")?;
                let ms = ticks as f64 * f64::from(self.report.timestamp_period_ns) / 1_000_000.0;
                match kind {
                    0 => sample.frame_ms = ms,
                    1 => sample.particle_simulation_ms += ms,
                    2 => sample.particle_prepare_ms += ms,
                    3 => sample.particle_draw_ms += ms,
                    4 => sample.spine_draw_ms += ms,
                    _ => unreachable!(),
                }
            }
            for n in 0..sample.emitter_count {
                let offset = (layout.headers - layout.base) as usize + n * 64;
                let word = |n: usize| {
                    u64::from(u32::from_le_bytes(
                        bytes[offset + n..offset + n + 4].try_into().unwrap(),
                    ))
                };
                sample.live += word(0);
                sample.spawned_last_substep += word(12);
                sample.dropped_last_substep += word(16);
                sample.collisions_last_substep += word(20);
                sample.invalid_last_substep += word(28);
                sample.indirect_instances += word(52);
            }
        }
        drop(data);
        target.unmap();
        Ok(())
    }
}

fn summarize_gpu_frames(
    samples: &[WindowedPlayerGpuFrameSample],
) -> Option<crate::windowed_player::WindowedPlayerFramePhasePerformanceSummary> {
    let mut values = samples
        .iter()
        .map(|sample| sample.frame_ms)
        .collect::<Vec<_>>();
    values.sort_by(f64::total_cmp);
    if values.is_empty() {
        return None;
    }
    let percentile = |fraction: f64| {
        let rank = ((values.len() as f64 * fraction).ceil() as usize)
            .saturating_sub(1)
            .min(values.len() - 1);
        values[rank]
    };
    Some(
        crate::windowed_player::WindowedPlayerFramePhasePerformanceSummary {
            observed_sample_frames: values.len() as u64,
            mean_ms: values.iter().sum::<f64>() / values.len() as f64,
            p50_ms: percentile(0.50),
            p95_ms: percentile(0.95),
            p99_ms: percentile(0.99),
            max_ms: *values.last().unwrap(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_history_census_visits_backing_not_every_historical_sample() {
        let mut history = GpuFrameMeasurement {
            report: Default::default(),
            query: None,
            results: None,
            warmup: 0,
            requested: 45000,
            frame: 0,
            active: false,
            next_query: 0,
            ranges: Vec::new(),
            sample_ranges: Vec::new(),
            current_ranges: Vec::new(),
            current: Default::default(),
            layouts: Vec::new(),
            used_bytes: 0,
        };
        for frame in 0..32768 {
            for kind in 0..=frame % 4 {
                history.current_ranges.push((frame, frame + 1, kind as u8));
            }
            history.retain_current_ranges();
        }
        for frame in 0..32768 {
            let ranges = history.ranges_for_frame(frame);
            assert_eq!(ranges.len(), frame % 4 + 1);
            assert_eq!(ranges[0], (frame as u32, frame as u32 + 1, 0));
        }
        let mut sample = crate::resource_memory::ResourceMemorySample::new(1, 1, 0);
        let mut gpu = crate::wgpu_backend::real::memory::GpuMemoryCensus::default();
        let start = crate::platform_time::Instant::now();
        history.observe_memory(&mut gpu, &mut sample);
        let totals = crate::resource_memory::memory_totals(&sample.allocations).unwrap();
        eprintln!(
            "history=32768 backingVisits={} censusAndReduceUs={} cpuBytes={}",
            sample.allocations.len(),
            start.elapsed().as_micros(),
            totals.cpu_allocated
        );
        assert!(
            sample.allocations.len() <= 6,
            "census cost must depend on live backing stores, not historical frame count"
        );
        assert_eq!(
            totals.cpu_allocated,
            (history.ranges.capacity() * std::mem::size_of::<(u32, u32, u8)>()
                + history.sample_ranges.capacity() * std::mem::size_of::<std::ops::Range<usize>>()
                + history.current_ranges.capacity() * std::mem::size_of::<(u32, u32, u8)>())
                as u64
        );
    }
    #[test]
    #[ignore = "requires real Vulkan timestamp support"]
    fn flat_range_history_preserves_real_gpu_readback_and_sample_identity() {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        let required = features(adapter.features(), true);
        assert!(!required.is_empty(), "real timestamp support required");
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_features: required,
            ..Default::default()
        }))
        .unwrap();
        let mut capture =
            GpuFrameMeasurement::new(&device, &queue, &adapter.get_info(), 0, 8).unwrap();
        for frame in 0..8 {
            let mut encoder = device.create_command_encoder(&Default::default());
            capture.begin(&mut encoder);
            for kind in 1..=frame % 3 + 1 {
                let start = capture.mark(&mut encoder).unwrap();
                capture.end_range(&mut encoder, start, kind).unwrap();
            }
            capture.end(&mut encoder, &BTreeMap::new()).unwrap();
            queue.submit(Some(encoder.finish()));
        }
        let report = capture.finish(&device, &queue);
        assert_eq!(report.status, "complete");
        assert_eq!(report.samples.len(), 8);
        for (i, sample) in report.samples.iter().enumerate() {
            assert_eq!(sample.frame_index, i as u64 + 1);
            for ms in [
                sample.frame_ms,
                sample.particle_simulation_ms,
                sample.particle_prepare_ms,
                sample.particle_draw_ms,
            ] {
                assert!(ms.is_finite() && ms >= 0.0);
            }
            if i % 3 == 0 {
                assert_eq!(sample.particle_prepare_ms, 0.0);
                assert_eq!(sample.particle_draw_ms, 0.0);
            }
        }
    }

    #[test]
    fn packed_measurement_accepts_long_geometry_windows_and_rejects_budget_exhaustion() {
        let mut used = 0;
        // Whole-frame + normal render ranges, with no particle emitter headers.
        for _ in 0..72_000 {
            let layout = SampleLayout::new(used, 8, 0, BUFFER_BUDGET).unwrap();
            assert_eq!(layout.base % 256, 0);
            used = layout.end;
        }
        assert_eq!(used, 72_000 * 256);
        assert!(used < BUFFER_BUDGET);
        let full = SampleLayout::new(0, QUERIES, EMITTERS, STRIDE).unwrap();
        assert_eq!(full.headers, QUERY_BYTES);
        assert_eq!(full.end, STRIDE);
        assert_eq!(
            SampleLayout::new(full.end, 2, 0, STRIDE).unwrap_err(),
            "gpu_measurement.buffer_budget_exceeded"
        );
        assert!(SampleLayout::new(0, QUERIES + 1, 0, BUFFER_BUDGET).is_err());
    }

    #[test]
    fn gpu_frame_summary_reports_required_percentiles_and_maximum() {
        let samples = [1.0, 4.0, 2.0, 3.0]
            .into_iter()
            .map(|frame_ms| WindowedPlayerGpuFrameSample {
                frame_ms,
                ..Default::default()
            })
            .collect::<Vec<_>>();
        let summary = summarize_gpu_frames(&samples).unwrap();
        assert_eq!(summary.observed_sample_frames, 4);
        assert_eq!(summary.mean_ms, 2.5);
        assert_eq!(summary.p50_ms, 2.0);
        assert_eq!(summary.p95_ms, 4.0);
        assert_eq!(summary.p99_ms, 4.0);
        assert_eq!(summary.max_ms, 4.0);
    }
}
