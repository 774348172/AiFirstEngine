//! Optional Vulkan display timings on the original Player surface. No new present path.
//! Raw handles remain local; reports use lifetime epochs and assigned image ordinals.
use ash::vk::{self, Handle};
use engine_runtime::render_thread::{GpuSubmissionToken, RenderFrameIdentity};
use engine_runtime::windowed_player::{NativePresentFrameObservation, NativePresentReport};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

const MAX_FRAMES: usize = crate::measurement::MAX_CAPTURE_FRAMES as usize;
const MAX_EVENT_BATCH: usize = 256;

fn present_report(
    policy: crate::measurement::MeasurementPolicy,
    has_timing: bool,
    frame_limit: u64,
) -> NativePresentReport {
    use crate::measurement::MeasurementLevel;
    // Frame indices start at one. A short filtered Trace must not reserve the
    // entire session, and a capability-only Summary never records frame rows.
    let frames = match policy.level {
        MeasurementLevel::Off => 0,
        MeasurementLevel::Summary if !has_timing => 0,
        MeasurementLevel::Summary => frame_limit,
        MeasurementLevel::Trace => policy
            .end_frame
            .unwrap_or(u64::MAX)
            .min(frame_limit.saturating_add(1))
            .saturating_sub(policy.first_frame.unwrap_or(1).max(1)),
    }
    .min(MAX_FRAMES as u64) as usize;
    NativePresentReport {
        status: if has_timing {
            "collecting"
        } else {
            "unavailable_display_timing"
        }
        .into(),
        source: "VK_GOOGLE_display_timing".into(),
        frames: Vec::with_capacity(frames),
        ..Default::default()
    }
}

/// A rejected render keeps the backend's last token. Never attach this present's
/// markers to that older submission (even when replaying the same frame index).
pub(crate) fn submission_for_present(
    previous: Option<GpuSubmissionToken>,
    current: Option<GpuSubmissionToken>,
    frame_index: u64,
    report: &engine_runtime::engine_rhi::RhiBackendReport,
) -> Result<GpuSubmissionToken, String> {
    if let Some(error) = report
        .diagnostics
        .iter()
        .find(|d| d.severity == engine_runtime::engine_rhi::RhiBackendDiagnosticSeverity::Error)
    {
        return Err(format!("{}: {}", error.code, error.message));
    }
    if report.frame_index != frame_index || report.failed_resource_count != 0 {
        return Err("gpu.backend_frame_rejected".into());
    }
    current
        .filter(|token| Some(*token) != previous && token.frame_index == frame_index)
        .ok_or_else(|| "gpu.current_submission_missing".into())
}

#[cfg(windows)]
mod windows_etw;

pub(crate) fn features(available: wgpu::Features, enabled: bool) -> wgpu::Features {
    if enabled {
        available & wgpu::Features::VULKAN_GOOGLE_DISPLAY_TIMING
    } else {
        wgpu::Features::empty()
    }
}

pub(crate) struct NativePresentObserver {
    policy: crate::measurement::MeasurementPolicy,
    #[cfg(windows)]
    windows_etw: Option<windows_etw::WindowsEtwCapture>,
    pending_present: Option<(usize, i64)>,
    timing: Option<ash::google::display_timing::Device>,
    swapchain: Option<vk::SwapchainKHR>,
    epoch: u64,
    images: BTreeMap<u64, u32>,
    by_present: BTreeMap<u32, usize>,
    capacity: usize,
    report: NativePresentReport,
}

impl NativePresentObserver {
    pub(crate) fn new(
        device: &wgpu::Device,
        policy: crate::measurement::MeasurementPolicy,
        capacity: u64,
    ) -> Option<Self> {
        if !policy.enabled() {
            return None;
        }
        let timing = if device
            .features()
            .contains(wgpu::Features::VULKAN_GOOGLE_DISPLAY_TIMING)
        {
            // SAFETY: The guard keeps the live device/instance borrowed; this only loads
            // functions for the extension explicitly enabled on that same device.
            unsafe { device.as_hal::<wgpu::hal::api::Vulkan>() }.map(|hal| {
                ash::google::display_timing::Device::new(
                    hal.shared_instance().raw_instance(),
                    hal.raw_device(),
                )
            })
        } else {
            None
        };
        Some(Self {
            policy,
            #[cfg(windows)]
            windows_etw: (timing.is_none() && policy.native_events)
                .then(windows_etw::WindowsEtwCapture::new),
            pending_present: None,
            report: present_report(policy, timing.is_some(), capacity),
            timing,
            swapchain: None,
            epoch: 0,
            images: BTreeMap::new(),
            by_present: BTreeMap::new(),
            capacity: capacity.min(MAX_FRAMES as u64) as usize,
        })
    }

    fn error(&mut self, error: &str) {
        self.report.status = "incomplete_native_present".into();
        if self.report.errors.len() < 8 && !self.report.errors.iter().any(|e| e == error) {
            self.report.errors.push(error.into());
        }
    }

    pub(crate) fn before_present(
        &mut self,
        surface: &wgpu::Surface<'_>,
        texture: &wgpu::Texture,
        token: GpuSubmissionToken,
        identity: Option<Arc<RenderFrameIdentity>>,
    ) {
        self.pending_present = None;
        if self.policy.level == crate::measurement::MeasurementLevel::Trace
            && !self.policy.trace_frame(token.frame_index)
        {
            return;
        }
        if self.policy.level == crate::measurement::MeasurementLevel::Summary
            && self.timing.is_none()
        {
            // Capability-only summary: no HAL handle reads, per-frame allocation,
            // QPC boundaries or ETW lifecycle when native timings are unavailable.
            self.report.unobserved_frames += 1;
            return;
        }
        let started = Instant::now();
        if self.report.frames.len() >= self.capacity {
            self.error("native_present.frame_capacity_exceeded");
            return;
        }
        // SAFETY: Live guards are used only for reading this surface's swapchain and
        // acquired texture, and setting the supported next-present metadata. The
        // original wgpu surface.present still owns submission/presentation.
        let native_surface = unsafe { surface.as_hal::<wgpu::hal::api::Vulkan>() };
        let native_texture = unsafe { texture.as_hal::<wgpu::hal::api::Vulkan>() };
        let Some((native_surface, native_texture)) = native_surface.zip(native_texture) else {
            self.error("native_present.native_surface_unavailable");
            return;
        };
        let Some(swapchain) = native_surface.raw_swapchain() else {
            self.error("native_present.swapchain_unconfigured");
            return;
        };
        if self.swapchain != Some(swapchain) {
            self.swapchain = Some(swapchain);
            self.epoch += 1;
            self.images.clear();
        }
        // SAFETY: The acquired texture's handle is copied while its guard is live.
        let raw_image = unsafe { native_texture.raw_handle() }.as_raw();
        if self.images.len() >= 16 && !self.images.contains_key(&raw_image) {
            self.error("native_present.image_capacity_exceeded");
            return;
        }
        let next_image = self.images.len() as u32;
        let image_index = *self.images.entry(raw_image).or_insert(next_image);
        let present_id = self.report.frames.len() as u32 + 1;
        if self.timing.is_some() {
            native_surface
                .set_next_present_time(vk::PresentTimeGOOGLE::default().present_id(present_id));
            self.by_present.insert(present_id, self.report.frames.len());
        }
        self.report.frames.push(NativePresentFrameObservation {
            application_present_qpc: None,
            application_present_thread_id: None,
            submission: token,
            frame_identity: self
                .policy
                .trace_frame(token.frame_index)
                .then_some(identity)
                .flatten(),
            application_present_index: u64::from(present_id),
            present_id: self.timing.as_ref().map(|_| present_id),
            swapchain_epoch: self.epoch,
            image_index,
            actual_present_time_ns: None,
            earliest_present_time_ns: None,
            present_margin_ns: None,
        });
        self.report.sampling_cpu_ns +=
            started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        #[cfg(windows)]
        if self.windows_etw.is_some() {
            if let Some(start) = windows_etw::qpc() {
                let index = self.report.frames.len() - 1;
                // SAFETY: GetCurrentThreadId has no arguments or ownership side effects.
                self.report.frames[index].application_present_thread_id =
                    Some(unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() });
                self.pending_present = Some((index, start));
            }
        }
    }

    /// Called immediately after the original surface.present. These are application
    /// call boundaries only; no ETW event is inferred from their time interval.
    pub(crate) fn after_present(&mut self) {
        #[cfg(windows)]
        if let Some((index, start)) = self.pending_present.take() {
            self.report.frames[index].application_present_qpc = windows_etw::qpc()
                .filter(|end| *end >= start)
                .map(|end| [start, end]);
        }
    }

    fn consume(&mut self, epoch: u64, event: vk::PastPresentationTimingGOOGLE) {
        let Some(&index) = self.by_present.get(&event.present_id) else {
            self.report.unmatched_events += 1;
            return;
        };
        let frame = &mut self.report.frames[index];
        if frame.swapchain_epoch != epoch || event.actual_present_time == 0 {
            self.report.unmatched_events += 1;
            return;
        }
        if frame.actual_present_time_ns.is_some() {
            self.report.duplicate_events += 1;
            return;
        }
        frame.actual_present_time_ns = Some(event.actual_present_time);
        frame.earliest_present_time_ns = Some(event.earliest_present_time);
        frame.present_margin_ns = Some(event.present_margin);
    }

    pub(crate) fn collect(&mut self) {
        let Some((timing, swapchain)) = self.timing.as_ref().zip(self.swapchain) else {
            return;
        };
        let started = Instant::now();
        let mut events = [vk::PastPresentationTimingGOOGLE::default(); MAX_EVENT_BATCH];
        let mut count = events.len() as u32;
        // SAFETY: The extension/device/swapchain belong to the live Player. The count
        // is capped to the fixed writable array, so even INCOMPLETE cannot overrun it.
        let result = unsafe {
            (timing.fp().get_past_presentation_timing_google)(
                timing.device(),
                swapchain,
                &mut count,
                events.as_mut_ptr(),
            )
        };
        if result != vk::Result::SUCCESS && result != vk::Result::INCOMPLETE {
            self.error("native_present.query_failed");
        } else {
            for event in events
                .into_iter()
                .take((count as usize).min(MAX_EVENT_BATCH))
            {
                self.consume(self.epoch, event);
            }
            if result == vk::Result::INCOMPLETE {
                self.error("native_present.event_batch_exceeded");
            }
        }
        self.report.sampling_cpu_ns +=
            started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
    }

    /// Query the old swapchain before resize destroys it. A new lifetime never
    /// inherits old events even if the driver reuses the raw handle value.
    pub(crate) fn reconfigure(&mut self) {
        self.collect();
        self.swapchain = None;
        self.images.clear();
    }

    pub(crate) fn abandon_device(&mut self) {
        self.error("native_present.device_abandoned");
        self.timing = None;
        self.swapchain = None;
    }

    pub(crate) fn finish(mut self) -> NativePresentReport {
        self.collect();
        #[cfg(windows)]
        if let Some(capture) = self.windows_etw.take() {
            windows_etw::attach_to_report(capture.finish(), &mut self.report);
        }
        if self.policy.level == crate::measurement::MeasurementLevel::Trace || self.timing.is_some()
        {
            self.report.unobserved_frames = self
                .report
                .frames
                .iter()
                .filter(|f| f.actual_present_time_ns.is_none())
                .count() as u64;
        }
        if self.timing.is_some() && self.report.errors.is_empty() {
            self.report.status =
                if self.report.unobserved_frames == 0 && !self.report.frames.is_empty() {
                    "observed"
                } else {
                    "partial_display_events"
                }
                .into();
        }
        if self.policy.level == crate::measurement::MeasurementLevel::Summary {
            self.report.frames.clear();
        }
        self.report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spine374_f_present_history_reserves_only_eligible_frames_before_sampling() {
        use crate::measurement::{MeasurementLevel, MeasurementPolicy};
        let trace = MeasurementPolicy {
            level: MeasurementLevel::Trace,
            ..Default::default()
        };
        let mut report = present_report(trace, false, 4200);
        assert_eq!(
            report.frames.capacity(),
            4200,
            "late history growth at 4097"
        );
        let pointer = report.frames.as_ptr();
        for frame_index in 1..=4200 {
            report.frames.push(NativePresentFrameObservation {
                application_present_qpc: None,
                application_present_thread_id: None,
                submission: GpuSubmissionToken {
                    device_epoch: 7,
                    serial: frame_index,
                    frame_index,
                    content_version: 9,
                },
                frame_identity: None,
                application_present_index: frame_index,
                present_id: None,
                swapchain_epoch: 1,
                image_index: (frame_index % 3) as u32,
                actual_present_time_ns: None,
                earliest_present_time_ns: None,
                present_margin_ns: None,
            });
            assert_eq!(report.frames.as_ptr(), pointer);
        }
        assert_eq!(report.frames[4096].submission.frame_index, 4097);
        assert_eq!(report.frames.last().unwrap().submission.frame_index, 4200);
        for timing in [false, true] {
            assert_eq!(
                present_report(Default::default(), timing, 4200)
                    .frames
                    .capacity(),
                0
            );
        }
        let summary = MeasurementPolicy {
            level: MeasurementLevel::Summary,
            ..Default::default()
        };
        assert_eq!(present_report(summary, false, 4200).frames.capacity(), 0);
        assert_eq!(present_report(summary, true, 4200).frames.capacity(), 4200);
        let filtered = MeasurementPolicy {
            first_frame: Some(4090),
            end_frame: Some(4110),
            ..trace
        };
        assert_eq!(present_report(filtered, false, 4200).frames.capacity(), 20);
        assert_eq!(present_report(filtered, false, 4095).frames.capacity(), 6);
        assert_eq!(present_report(filtered, false, 4000).frames.capacity(), 0);
        assert_eq!(present_report(trace, false, 0).frames.capacity(), 0);
        assert_eq!(
            present_report(trace, false, u64::MAX).frames.capacity(),
            MAX_FRAMES
        );
        let late = MeasurementPolicy {
            first_frame: Some(300_000),
            end_frame: Some(300_010),
            ..trace
        };
        assert_eq!(present_report(late, false, 300_020).frames.capacity(), 10);
        eprintln!(
            "native present observation bytes={}, reserved4200={}",
            std::mem::size_of::<NativePresentFrameObservation>(),
            4200 * std::mem::size_of::<NativePresentFrameObservation>()
        );
    }
    #[test]
    fn rejected_or_replayed_present_cannot_borrow_the_previous_submission() {
        let mut report =
            engine_runtime::engine_rhi::RhiBackendReport::unavailable("test", 7, "unused");
        report.diagnostics.clear();
        let previous = GpuSubmissionToken {
            device_epoch: 1,
            serial: 3,
            frame_index: 7,
            content_version: 2,
        };
        assert!(submission_for_present(Some(previous), Some(previous), 8, &report).is_err());
        assert!(submission_for_present(Some(previous), Some(previous), 7, &report).is_err());
        assert!(submission_for_present(None, Some(previous), 8, &report).is_err());
        assert!(submission_for_present(Some(previous), None, 8, &report).is_err());
        let fresh = GpuSubmissionToken {
            serial: 4,
            ..previous
        };
        assert_eq!(
            submission_for_present(Some(previous), Some(fresh), 7, &report),
            Ok(fresh)
        );
        report
            .diagnostics
            .push(engine_runtime::engine_rhi::RhiBackendDiagnostic::error(
                "wgpu.render_surface_failed",
                "environment.resident_capacity",
            ));
        assert!(
            submission_for_present(Some(previous), Some(fresh), 7, &report).is_err(),
            "backend error must stop Present even if a token exists"
        );
        report.diagnostics.clear();
        report.failed_resource_count = 1;
        assert!(submission_for_present(Some(previous), Some(fresh), 7, &report).is_err());
        report.failed_resource_count = 0;
        report.frame_index = 8;
        assert!(submission_for_present(Some(previous), Some(fresh), 7, &report).is_err());
    }
    #[test]
    fn windows_capture_capability_is_separate_from_display_events() {
        let report = serde_json::to_value(NativePresentReport::default()).unwrap();
        assert_eq!(report.get("windowsEtw"), Some(&serde_json::Value::Null));
        let mut legacy = report;
        legacy.as_object_mut().unwrap().remove("windowsEtw");
        let decoded: NativePresentReport = serde_json::from_value(legacy).unwrap();
        assert!(decoded.windows_etw.is_none());
        let old = engine_runtime::windowed_player::WindowsEtwCaptureReport::default();
        let mut old = serde_json::to_value(old).unwrap();
        old.as_object_mut().unwrap().remove("decoded");
        old.as_object_mut().unwrap().remove("correlated");
        let old: engine_runtime::windowed_player::WindowsEtwCaptureReport =
            serde_json::from_value(old).unwrap();
        assert!(old.decoded.is_none());
        assert!(old.correlated.is_none());
    }
    #[test]
    fn native_present_epoch_unknown_duplicate_and_absent_are_distinct() {
        let token = GpuSubmissionToken {
            device_epoch: 1,
            serial: 1,
            frame_index: 1,
            content_version: 1,
        };
        let mut owner = NativePresentObserver {
            policy: crate::measurement::MeasurementPolicy {
                level: crate::measurement::MeasurementLevel::Trace,
                ..Default::default()
            },
            #[cfg(windows)]
            windows_etw: None,
            pending_present: None,
            timing: None,
            swapchain: None,
            epoch: 2,
            images: BTreeMap::new(),
            capacity: 2,
            by_present: [(1, 0)].into(),
            report: NativePresentReport {
                frames: vec![NativePresentFrameObservation {
                    application_present_qpc: None,
                    application_present_thread_id: None,
                    submission: token,
                    frame_identity: None,
                    application_present_index: 1,
                    present_id: Some(1),
                    swapchain_epoch: 2,
                    image_index: 0,
                    actual_present_time_ns: None,
                    earliest_present_time_ns: None,
                    present_margin_ns: None,
                }],
                ..Default::default()
            },
        };
        let event = vk::PastPresentationTimingGOOGLE {
            present_id: 1,
            actual_present_time: 100,
            ..Default::default()
        };
        #[cfg(windows)]
        {
            owner.pending_present = Some((0, windows_etw::qpc().unwrap()));
            owner.after_present();
            let boundary = owner.report.frames[0].application_present_qpc.unwrap();
            assert!(boundary[1] >= boundary[0]);
            assert!(owner.report.frames[0].actual_present_time_ns.is_none());
            owner.after_present();
            assert_eq!(
                owner.report.frames[0].application_present_qpc,
                Some(boundary)
            );
        }
        owner.consume(1, event);
        assert_eq!(owner.report.unmatched_events, 1);
        assert!(owner.report.frames[0].actual_present_time_ns.is_none());
        owner.consume(
            2,
            vk::PastPresentationTimingGOOGLE {
                present_id: 99,
                ..event
            },
        );
        owner.consume(2, event);
        owner.consume(2, event);
        assert_eq!(owner.report.unmatched_events, 2);
        assert_eq!(owner.report.duplicate_events, 1);
        assert_eq!(owner.report.frames[0].actual_present_time_ns, Some(100));
        let report = owner.finish();
        assert_eq!(report.unobserved_frames, 0);
        assert!(report.discarded_frames.is_none() && report.repeated_displays.is_none());
        assert!(features(wgpu::Features::VULKAN_GOOGLE_DISPLAY_TIMING, false).is_empty());
    }
}
