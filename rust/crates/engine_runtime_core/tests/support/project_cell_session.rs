//! Real Host/FrameLoop consumer fixture. Commands enter through session deferred mutation.
use engine_runtime::{
    engine_host_loop::*, frame_loop::RuntimeFrameContext, project_runtime_session::*,
    runtime_instance_loader::*, world::World,
};
use project_runtime_sdk::{ProjectRuntimeCellIntent as Intent, ProjectRuntimeCellStatus as Status};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct Control {
    pub next: Vec<Intent>,
    pub rejected: bool,
    pub observed_counts: Vec<usize>,
}
struct Session(Arc<Mutex<Control>>);
impl ProjectRuntimeSession for Session {
    fn session_id(&self) -> &str {
        "project-cell-consumer"
    }
    fn handle_aui_actions(
        &mut self,
        _: ProjectRuntimeSessionContext<'_>,
        _: ProjectAuiActionBatch<'_>,
    ) -> ProjectRuntimeSessionOutput {
        ProjectRuntimeSessionOutput::no_op()
    }
    fn fixed_update(
        &mut self,
        ctx: ProjectRuntimeSessionContext<'_>,
    ) -> ProjectRuntimeSessionOutput {
        let mut control = self.0.lock().unwrap();
        control.observed_counts.push(
            ctx.world
                .query(&engine_runtime::query::QuerySpec::all([]))
                .len(),
        );
        if control.next.is_empty() {
            return ProjectRuntimeSessionOutput::no_op();
        }
        let mut buffer = ProjectRuntimeMutationBuffer::new();
        for intent in std::mem::take(&mut control.next) {
            buffer.cell_command(intent);
        }
        let mut output = ProjectRuntimeSessionOutput::applied(buffer);
        if control.rejected {
            output.status = ProjectRuntimeSessionStatus::Rejected;
        }
        output
    }
}
pub fn host() -> (EngineHostLoop, Arc<Mutex<Control>>) {
    let control = Arc::new(Mutex::new(Control::default()));
    (
        EngineHostLoop::with_project_runtime_session(
            "main",
            Default::default(),
            Box::new(Session(control.clone())),
        ),
        control,
    )
}
pub fn load(key: u64, name: &str, rendered: bool) -> Intent {
    Intent::Load {
        request_id: key,
        placement_id: format!("placement-{name}"),
        prefab: project_runtime_sdk::ProjectRuntimeSpatialAssetRef {
            id: format!("cell-{name}"),
            asset_type: "prefab".into(),
            guid: Some(format!("cell-{name}")),
            sub_asset: None,
        },
        parent: None,
        render_required: rendered,
    }
}
pub fn tick(
    host: &mut EngineHostLoop,
    world: &mut World,
    loader: &mut RuntimeInstanceLoader,
    package: &engine_runtime::runtime_package::RuntimePackage,
    ticks: usize,
    #[cfg(feature = "real-wgpu")] backend: Option<
        &mut engine_runtime::wgpu_backend::real::RealWgpuBackend,
    >,
) -> EngineFrameOutput {
    host.tick_with_runtime_context(
        EngineFrameInput::new(EngineHostMode::ExportedGame).with_fixed_step_count(ticks),
        world,
        RuntimeFrameContext {
            package,
            instance_loader: loader,
            cell_resources: None,
            #[cfg(feature = "real-wgpu")]
            cell_backend: backend,
        },
    )
}
pub fn wait_active(
    host: &mut EngineHostLoop,
    world: &mut World,
    loader: &mut RuntimeInstanceLoader,
    package: &engine_runtime::runtime_package::RuntimePackage,
    key: u64,
) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        assert!(
            std::time::Instant::now() < deadline,
            "cell did not activate: {:?}",
            loader.project_cell_snapshot(world)
        );
        let output = tick(
            host,
            world,
            loader,
            package,
            1,
            #[cfg(feature = "real-wgpu")]
            None,
        );
        assert!(
            !output
                .project_runtime_session_report
                .as_ref()
                .is_some_and(|r| r.terminal_fault),
            "{output:?}"
        );
        if loader.project_cell_snapshot(world).receipts[&key].status == Status::Active {
            break;
        }
        std::thread::yield_now();
    }
}
