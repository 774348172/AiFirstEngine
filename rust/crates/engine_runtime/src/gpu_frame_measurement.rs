//! Bounded opt-in capture on the renderer queue. No mapping or waits during frames.
//! Timestamp resolution/header copies follow the measured commands; one readback at finish.
use crate::particle_gpu::ParticleGpuEffect;
use crate::windowed_player::{WindowedPlayerGpuFrameSample, WindowedPlayerGpuPerformance};
use std::collections::BTreeMap;

const QUERIES: u32 = 512;
const EMITTERS: usize = 256;
const QUERY_BYTES: u64 = QUERIES as u64 * 8;
const STRIDE: u64 = QUERY_BYTES + EMITTERS as u64 * 64;

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
    ranges: Vec<Vec<(u32, u32, u8)>>,
    current_ranges: Vec<(u32, u32, u8)>,
    current: WindowedPlayerGpuFrameSample,
}

impl GpuFrameMeasurement {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        info: &wgpu::AdapterInfo,
        warmup: u64,
        requested: u64,
    ) -> Result<Self, String> {
        if requested == 0 || requested > 2048 {
            return Err("gpu_measurement.sample_limit_1_2048".into());
        }
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
                size: STRIDE * requested,
                usage: wgpu::BufferUsages::QUERY_RESOLVE
                    | wgpu::BufferUsages::COPY_SRC
                    | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });
        Ok(Self {
            report: WindowedPlayerGpuPerformance {
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
                measurement_buffer_bytes: if supported { STRIDE * requested } else { 0 },
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
            current_ranges: Vec::new(),
            current: Default::default(),
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
        let base = self.report.samples.len() as u64 * STRIDE;
        let results = self.results.as_ref().unwrap();
        encoder.resolve_query_set(
            self.query.as_ref().unwrap(),
            0..self.next_query,
            results,
            base,
        );
        for (i, emitter) in effects.values().flat_map(|e| &e.emitters).enumerate() {
            let offset = base + QUERY_BYTES + i as u64 * 64;
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
        self.ranges.push(std::mem::take(&mut self.current_ranges));
        self.active = false;
        Ok(())
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
        let size = self.report.samples.len() as u64 * STRIDE;
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
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
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
                    if std::time::Instant::now() < deadline =>
                {
                    std::thread::sleep(std::time::Duration::from_millis(1))
                }
                _ => return Err("gpu_measurement.readback_timeout".into()),
            }
        }
        let data = target.slice(..).get_mapped_range();
        for (i, sample) in self.report.samples.iter_mut().enumerate() {
            let bytes = &data[i * STRIDE as usize..(i + 1) * STRIDE as usize];
            for &(start, end, kind) in &self.ranges[i] {
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
                    _ => unreachable!(),
                }
            }
            for n in 0..sample.emitter_count {
                let offset = QUERY_BYTES as usize + n * 64;
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
