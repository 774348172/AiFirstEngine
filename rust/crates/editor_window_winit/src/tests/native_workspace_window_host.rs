use crate::native_workspace_window_host::*;
use editor_ui_renderer::{
    DockNode, PanelId, WorkspaceWindowId, WorkspaceWindowPlacement, WorkspaceWindowPlan,
    WorkspaceWindowPlanEntry,
};
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct FakeFactory {
    next_key: u64,
    create_counts: BTreeMap<String, usize>,
    fail: Option<(String, NativeWorkspaceWindowCreateStage)>,
}

impl NativeWorkspaceWindowFactory for FakeFactory {
    fn create(
        &mut self,
        window_id: &WorkspaceWindowId,
    ) -> Result<NativeWorkspaceWindowState, NativeWorkspaceWindowCreateStage> {
        *self
            .create_counts
            .entry(window_id.as_str().to_string())
            .or_default() += 1;
        if self
            .fail
            .as_ref()
            .is_some_and(|(id, _)| id == window_id.as_str())
        {
            return Err(self.fail.as_ref().expect("checked").1);
        }
        self.next_key += 1;
        Ok(NativeWorkspaceWindowState {
            workspace_window_id: window_id.clone(),
            native_key: NativeWorkspaceWindowKey(self.next_key),
            renderer_owner: self.next_key * 10,
            surface_owner: self.next_key * 100,
            metrics: NativeWorkspaceWindowMetrics {
                physical_width: 800,
                physical_height: 600,
                scale_factor: 1.0,
                focused: false,
            },
        })
    }
}

pub(super) fn plan(ids: &[&str]) -> WorkspaceWindowPlan {
    WorkspaceWindowPlan {
        windows: ids
            .iter()
            .map(|id| WorkspaceWindowPlanEntry {
                window_id: WorkspaceWindowId::new(*id).expect("valid window id"),
                root: DockNode::Stack {
                    node_id: editor_ui_renderer::LayoutNodeId::new(format!("stack-{id}"))
                        .expect("valid node id"),
                    tabs: vec![PanelId::new("Inspector").expect("valid panel id")],
                    active_panel_id: PanelId::new("Inspector").expect("valid panel id"),
                },
                placement: WorkspaceWindowPlacement::default(),
            })
            .collect(),
    }
}

#[test]
fn native_workspace_window_host_reconciles_idempotently_and_removes_only_owned_floating() {
    let mut host = NativeWorkspaceWindowHost::default();
    let mut factory = FakeFactory::default();
    assert!(host
        .reconcile(&plan(&["main", "floating-a", "floating-b"]), &mut factory)
        .is_empty());
    assert_eq!(host.windows().len(), 3);
    assert!(host
        .reconcile(&plan(&["main", "floating-a", "floating-b"]), &mut factory)
        .is_empty());
    assert!(factory.create_counts.values().all(|count| *count == 1));

    let floating_b_key = host.windows()["floating-b"].native_key;
    host.reconcile(&plan(&["main", "floating-b"]), &mut factory);
    assert_eq!(host.windows().len(), 2);
    assert_eq!(
        host.workspace_window_id(floating_b_key)
            .map(WorkspaceWindowId::as_str),
        Some("floating-b")
    );
}

#[test]
fn native_workspace_window_host_commits_only_surface_ready_windows() {
    let mut host = NativeWorkspaceWindowHost::default();
    let mut factory = FakeFactory {
        fail: Some((
            "floating-a".to_string(),
            NativeWorkspaceWindowCreateStage::Surface,
        )),
        ..FakeFactory::default()
    };
    let diagnostics = host.reconcile(&plan(&["main", "floating-a"]), &mut factory);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].code,
        "native_workspace_surface_create_failed"
    );
    assert!(host.windows().contains_key("main"));
    assert!(!host.windows().contains_key("floating-a"));

    let mut window_failure_factory = FakeFactory {
        fail: Some((
            "floating-b".to_string(),
            NativeWorkspaceWindowCreateStage::Window,
        )),
        ..FakeFactory::default()
    };
    let diagnostics = host.reconcile(
        &plan(&["main", "floating-a", "floating-b"]),
        &mut window_failure_factory,
    );
    assert_eq!(diagnostics[0].code, "native_workspace_window_create_failed");
    assert!(!host.windows().contains_key("floating-b"));
}
