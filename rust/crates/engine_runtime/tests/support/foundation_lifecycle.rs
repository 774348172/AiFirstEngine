//! Original foundation lifecycle: ordinary session demands, real GPU retirement,
//! and a ten-minute same-state tail. Not a Release or complete heap qualification.
use engine_runtime::{
    engine_host_loop::EngineHostLoop,
    game_view_presentation::{GameViewExtent, GameViewScalePolicy, GameViewTargetSpec},
    project_runtime_session::ProjectRuntimeSessionReportLevel,
    resource_memory::{memory_totals, MemoryTotals, ResourceMemorySample},
    runtime_instance_loader::RuntimeInstanceLoader,
    wgpu_backend::real::{GeometryUploadBudget, RealWgpuBackend},
    windowed_player::{sample_player_process_memory, WindowedPlayerProcessMemorySample},
    world::World,
};
use project_runtime_sdk::{ProjectRuntimeCellIntent as Intent, ProjectRuntimeCellStatus as Status};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const MIB: u64 = 1024 * 1024;
const TAIL: Duration = Duration::from_secs(600);

struct Lifecycle {
    fixture: super::streamed_cells::Fixture,
    host: EngineHostLoop,
    control: Arc<Mutex<super::project_cell_session::Control>>,
    world: World,
    loader: RuntimeInstanceLoader,
    backend: RealWgpuBackend,
    frame: u64,
    started: Instant,
    evidence: Option<PathBuf>,
    journal: Option<File>,
}

impl Lifecycle {
    fn new() -> Self {
        let fixture = super::streamed_cells::build_dual_32mib_cells();
        let loader = RuntimeInstanceLoader::from_package(&fixture.package);
        let mut world = super::world(0);
        super::commit(&mut world, vec![("camera", super::move_to(0.))]);
        let (mut host, control) = super::project_cell_session::host();
        host.set_project_runtime_session_report_level(ProjectRuntimeSessionReportLevel::Off);
        host.set_game_view_target(GameViewTargetSpec {
            extent: GameViewExtent::new(128, 128),
            scale_policy: GameViewScalePolicy::Contain,
        });
        assert!(host.set_external_rhi_enabled(true));
        let mut backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
        backend
            .set_geometry_upload_budget(GeometryUploadBudget {
                resident_bytes: 2 * 1024 * MIB,
                staging_bytes: 128 * MIB as usize,
                bytes_per_frame: 16 * MIB as usize,
                max_tickets: 128,
            })
            .unwrap();
        let evidence = std::env::var_os("S1_LIFECYCLE_EVIDENCE_DIR").map(PathBuf::from);
        let journal = evidence.as_ref().map(|dir| {
            std::fs::create_dir_all(dir).unwrap();
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(dir.join("checkpoints.jsonl"))
                .unwrap()
        });
        Self {
            fixture,
            host,
            control,
            world,
            loader,
            backend,
            frame: 0,
            started: Instant::now(),
            evidence,
            journal,
        }
    }

    fn record(&mut self, value: &serde_json::Value) {
        if let Some(file) = &mut self.journal {
            serde_json::to_writer(&mut *file, value).unwrap();
            writeln!(file).unwrap();
            file.flush().unwrap();
        }
    }

    fn step(&mut self) -> Option<Vec<u8>> {
        self.frame += 1;
        self.backend.pump_geometry_uploads(self.frame).unwrap();
        let output = super::project_cell_session::tick(
            &mut self.host,
            &mut self.world,
            &mut self.loader,
            &self.fixture.package,
            1,
            Some(&mut self.backend),
        );
        // This existing fixture records gameplay observations for other tests.
        // Retaining one entry per tail frame would measure the test's own log.
        self.control.lock().unwrap().observed_counts.clear();
        assert!(
            !output
                .project_runtime_session_report
                .as_ref()
                .is_some_and(|r| r.terminal_fault),
            "{output:?}"
        );
        let frame = output
            .render_thread_frame
            .expect("actual Host render frame");
        let plan = &frame.renderer_output.rhi_command_plan;
        let pixels = self
            .backend
            .prepare_surface_resources(plan, [128, 128], None)
            .unwrap()
            .then(|| {
                self.backend
                    .render_plan_to_rgba_bytes(plan, 128, 128)
                    .unwrap()
            });
        if let Some(token) = self.backend.last_gpu_submission() {
            self.backend
                .wait_gpu_submission(token, Duration::from_secs(5))
                .unwrap();
        }
        let gpu = self.backend.poll_gpu_completion().unwrap();
        assert!(!gpu.abandoned);
        let upload = self.backend.geometry_upload_snapshot();
        assert!(upload.frame_uploaded_bytes <= 16 * MIB as usize);
        assert!(upload.staging_bytes <= 128 * MIB as usize);
        assert!(upload.tickets <= 128);
        assert!(self.backend.geometry_residency().texture_bytes <= 64 * MIB);
        for receipt in self
            .loader
            .project_cell_snapshot(&self.world)
            .receipts
            .values()
        {
            assert_ne!(receipt.status, Status::Failed, "{receipt:?}");
        }
        pixels
    }

    fn demand(&mut self, intents: Vec<Intent>, key: u64, status: Status, cells: usize) -> Vec<u8> {
        self.control.lock().unwrap().next = intents;
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            assert!(
                Instant::now() < deadline,
                "cell {key} never reached {status:?}: {:?}",
                self.loader.project_cell_snapshot(&self.world)
            );
            let pixels = self.step();
            let snapshot = self.loader.project_cell_snapshot(&self.world);
            let ready = snapshot
                .receipts
                .get(&key)
                .is_some_and(|r| r.status == status);
            let stream = self.loader.asset_loader().streaming_snapshot();
            let upload = self.backend.geometry_upload_snapshot();
            let gpu = self.backend.gpu_lifecycle_stats();
            if ready
                && self.loader.cell_lifetime_counts() == (cells, cells)
                && stream.pending_operations == 0
                && stream.reserved_bytes == 0
                && upload.staging_bytes == 0
                && upload.tickets == cells
                && gpu.in_flight == 0
                && gpu.retiring_bytes == 0
            {
                if let Some(pixels) = pixels {
                    assert_eq!(snapshot.entities.len(), cells * 2);
                    assert_eq!(self.world.entity_count(), 1 + cells * 2);
                    assert_eq!(self.backend.geometry_residency().textures, cells * 2);
                    assert_eq!(
                        self.backend.geometry_residency().texture_bytes,
                        cells as u64 * 32 * MIB
                    );
                    return pixels;
                }
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn pixels(&mut self, pixels: &[u8], red: bool, blue: bool, label: &str) {
        let count = |channel: usize| {
            pixels
                .chunks_exact(4)
                .filter(|p| p[channel] > 120 && p[1] < 8 && p[2 - channel] < 8)
                .count()
        };
        let (r, b) = (count(0), count(2));
        assert_eq!(r > 100, red, "{label}: {r} red pixels");
        assert_eq!(b > 100, blue, "{label}: {b} blue pixels");
        assert!(red || r == 0, "unloaded red cell still visible");
        assert!(blue || b == 0, "unloaded blue cell still visible");
        // Both 16 MiB textures must produce pixels, including the darker sRGB
        // texels on the lower authored root/detail. The two halves separate
        // those entities independently of renderer residency counters.
        let halves: Vec<_> = pixels
            .chunks_exact(128 * 64 * 4)
            .map(|half| {
                let visible = |channel: usize| {
                    half.chunks_exact(4)
                        .filter(|p| p[channel] > 40 && p[1] < 8 && p[2 - channel] < 8)
                        .count()
                };
                (visible(0), visible(2))
            })
            .collect();
        for &(red_pixels, blue_pixels) in &halves {
            assert_eq!(red_pixels > 100, red, "{label}: root/detail red pixels");
            assert_eq!(blue_pixels > 100, blue, "{label}: root/detail blue pixels");
        }
        if let Some(dir) = &self.evidence {
            let file = File::create_new(dir.join(format!("{label}.png"))).unwrap();
            let mut encoder = png::Encoder::new(file, 128, 128);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(pixels)
                .unwrap();
        }
        self.record(
            &serde_json::json!({"kind":"pixels","label":label,"frame":self.frame,
            "redPixels":r,"bluePixels":b,"rootDetailHalves":halves}),
        );
    }

    fn capture(
        &mut self,
        label: &str,
        full: bool,
    ) -> (MemoryTotals, WindowedPlayerProcessMemorySample) {
        let gpu = self.backend.gpu_lifecycle_stats();
        assert_eq!(gpu.in_flight, 0);
        assert_eq!(gpu.retiring_bytes, 0);
        let mut sample = ResourceMemorySample::new(self.frame, gpu.device_epoch, 0);
        self.world.observe_memory(&mut sample);
        self.loader.observe_memory(&mut sample);
        self.host.observe_project_memory(&mut sample);
        self.backend.observe_memory(&mut sample);
        assert!(sample.capacities["loader.diagnostics.retained"] <= 1024);
        let totals = memory_totals(&sample.allocations).unwrap();
        let process = sample_player_process_memory().expect("Windows process memory");
        let snapshot = self.loader.project_cell_snapshot(&self.world);
        let stream = self.loader.asset_loader().streaming_snapshot();
        self.record(&serde_json::json!({"kind":"memory","label":label,"frame":self.frame,
            "elapsedSeconds":self.started.elapsed().as_secs_f64(),"totals":totals,
            "process":process,"coverage":sample.coverage,"capacities":sample.capacities,
            "receipts":snapshot.receipts.len(),"activeLiveCells":self.loader.cell_lifetime_counts(),
            "entities":self.world.entity_count(),"gpu":gpu,"residency":self.backend.geometry_residency(),
            "pendingOperations":stream.pending_operations,"pendingTickets":stream.pending_tickets,
            "cpuResidentBytes":stream.cpu_resident_bytes,"reservedBytes":stream.reserved_bytes}));
        assert!(process.private_commit_bytes <= 6 * 1024 * MIB);
        assert!(totals.gpu_allocated <= 4 * 1024 * MIB);
        assert!(snapshot.receipts.len() <= 128);
        if full {
            if let Some(dir) = &self.evidence {
                serde_json::to_writer(
                    File::create_new(dir.join(format!("{label}-census.json"))).unwrap(),
                    &sample,
                )
                .unwrap();
            }
        }
        (totals, process)
    }
}

#[test]
#[ignore = "real GPU; original 20 resource cycles followed by a 600-second tail"]
fn foundation_twenty_resource_cycles_then_ten_minute_retired_baseline() {
    let mut test = Lifecycle::new();
    let mut a = 1;
    let pixels = test.demand(
        vec![super::project_cell_session::load(a, "a", true)],
        a,
        Status::Active,
        1,
    );
    test.pixels(&pixels, true, false, "initial-a");
    test.capture("initial-a", true);
    for round in 1..=20 {
        let old_a = test.loader.project_cell_snapshot(&test.world).entities;
        let b = a + 1;
        let pixels = test.demand(
            vec![super::project_cell_session::load(b, "b", true)],
            b,
            Status::Active,
            2,
        );
        test.pixels(&pixels, true, true, &format!("round-{round:02}-both"));
        let pixels = test.demand(
            vec![Intent::Release { request_id: a }],
            a,
            Status::Released,
            1,
        );
        test.pixels(
            &pixels,
            false,
            true,
            &format!("round-{round:02}-a-released"),
        );
        for (id, generation) in old_a.values() {
            assert_ne!(test.world.entity_generation(id), Some(*generation));
        }
        a = b + 1;
        let pixels = test.demand(
            vec![super::project_cell_session::load(a, "a", true)],
            a,
            Status::Active,
            2,
        );
        test.pixels(&pixels, true, true, &format!("round-{round:02}-a-reloaded"));
        let old_b = test
            .loader
            .project_cell_snapshot(&test.world)
            .entities
            .into_iter()
            .filter(|((placement, _), _)| placement == "placement-b")
            .collect::<Vec<_>>();
        let pixels = test.demand(
            vec![Intent::Release { request_id: b }],
            b,
            Status::Released,
            1,
        );
        test.pixels(&pixels, true, false, &format!("round-{round:02}-baseline"));
        for (_, (id, generation)) in &old_b {
            assert_ne!(test.world.entity_generation(id), Some(*generation));
        }
        test.capture(&format!("round-{round:02}"), true);
        eprintln!("lifecycle round {round}/20 retired, one active 32 MiB cell");
    }
    let (baseline, process) = test.capture("tail-start", true);
    let tail_started = Instant::now();
    let mut next_sample = Duration::from_secs(1);
    let mut samples = 0;
    while tail_started.elapsed() < TAIL {
        let iteration = Instant::now();
        test.step();
        if tail_started.elapsed() >= next_sample {
            samples += 1;
            let (totals, current) = test.capture(&format!("tail-{samples:03}"), false);
            assert!(
                totals.cpu_allocated <= baseline.cpu_allocated,
                "same-state tracked CPU grew: {baseline:?} -> {totals:?}"
            );
            assert!(
                totals.gpu_allocated <= baseline.gpu_allocated,
                "same-state tracked GPU grew: {baseline:?} -> {totals:?}"
            );
            let private_allowance = (process.private_commit_bytes / 20).max(64 * MIB);
            let rss_allowance = (process.working_set_bytes / 20).max(64 * MIB);
            assert!(
                current.private_commit_bytes <= process.private_commit_bytes + private_allowance,
                "tail private growth requires investigation: {process:?} -> {current:?}"
            );
            assert!(
                current.working_set_bytes <= process.working_set_bytes + rss_allowance,
                "tail working-set growth requires investigation: {process:?} -> {current:?}"
            );
            assert_eq!(test.loader.cell_lifetime_counts(), (1, 1));
            assert_eq!(test.world.entity_count(), 3);
            assert_eq!(test.backend.geometry_residency().texture_bytes, 32 * MIB);
            let stream = test.loader.asset_loader().streaming_snapshot();
            assert_eq!(stream.pending_operations, 0);
            assert_eq!(stream.reserved_bytes, 0);
            if samples % 60 == 0 {
                eprintln!(
                    "lifecycle tail {}/600 seconds",
                    tail_started.elapsed().as_secs()
                );
            }
            next_sample += Duration::from_secs(1);
        }
        // Modest pacing for lifecycle observation, never a performance claim.
        std::thread::sleep(Duration::from_millis(33).saturating_sub(iteration.elapsed()));
    }
    let tail_seconds = tail_started.elapsed().as_secs_f64();
    assert!(tail_seconds >= 600.);
    test.capture("tail-end", true);
    let pixels = test.demand(vec![], a, Status::Active, 1);
    test.pixels(&pixels, true, false, "tail-end-a");
    let pixels = test.demand(
        vec![Intent::Release { request_id: a }],
        a,
        Status::Released,
        0,
    );
    test.pixels(&pixels, false, false, "all-unloaded");
    assert!(test
        .loader
        .asset_loader_mut()
        .shutdown_streaming(Duration::from_secs(5)));
    test.backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
    assert_eq!(
        test.loader
            .asset_loader()
            .streaming_snapshot()
            .cpu_resident_bytes,
        0
    );
    assert_eq!(test.backend.geometry_residency().meshes, 0);
    assert_eq!(test.backend.geometry_residency().textures, 0);
    assert_eq!(test.backend.geometry_upload_snapshot().tickets, 0);
    test.capture("final-drain", true);
    test.record(&serde_json::json!({"kind":"completed","resourceCycles":20,
        "tailSeconds":tail_seconds,"tailSamples":samples,"finalLiveCells":0,
        "formalPerformanceQualified":false,"completeMemoryCoverage":false,"actualOsDisplayQualified":false}));
}

#[path = "foundation_faults_scaled.rs"]
mod faults_scaled;
