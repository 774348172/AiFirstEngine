use crate::native_workspace_window_host::*;
use editor_ui_renderer::WorkspaceWindowId;

#[test]
fn workspace_multi_window_routes_each_native_window_to_its_owned_root() {
    let mut host = NativeWorkspaceWindowHost::default();
    let mut factory = super::native_workspace_window_host::FakeFactory::default();
    host.reconcile(
        &super::native_workspace_window_host::plan(&["main", "floating-a"]),
        &mut factory,
    );
    let main_key = host.windows()["main"].native_key;
    let floating_key = host.windows()["floating-a"].native_key;

    assert_eq!(
        host.route_event(main_key, NativeWorkspaceWindowEvent::CloseRequested),
        Some(NativeWorkspaceWindowAction::ShutdownEditor)
    );
    assert_eq!(
        host.route_event(floating_key, NativeWorkspaceWindowEvent::CloseRequested),
        Some(NativeWorkspaceWindowAction::CloseFloating(
            WorkspaceWindowId::new("floating-a").expect("valid id")
        ))
    );
    assert_eq!(
        host.route_event(
            floating_key,
            NativeWorkspaceWindowEvent::Resized {
                physical_width: 1200,
                physical_height: 900,
            },
        ),
        Some(NativeWorkspaceWindowAction::Resized {
            window_id: WorkspaceWindowId::new("floating-a").expect("valid id"),
            logical_width: 1200,
            logical_height: 900,
        })
    );
    assert_eq!(
        host.route_event(
            floating_key,
            NativeWorkspaceWindowEvent::ScaleFactorChanged { scale_factor: 1.5 },
        ),
        Some(NativeWorkspaceWindowAction::ScaleFactorChanged {
            window_id: WorkspaceWindowId::new("floating-a").expect("valid id"),
            scale_factor: 1.5,
            logical_width: 800,
            logical_height: 600,
        })
    );
    assert_eq!(host.windows()["main"].metrics.scale_factor, 1.0);
    assert_eq!(host.windows()["floating-a"].metrics.scale_factor, 1.5);
    assert_eq!(
        host.route_event(floating_key, NativeWorkspaceWindowEvent::Focused(true)),
        Some(NativeWorkspaceWindowAction::FocusChanged {
            window_id: WorkspaceWindowId::new("floating-a").expect("valid id"),
            focused: true,
        })
    );
    assert_eq!(
        host.route_event(floating_key, NativeWorkspaceWindowEvent::RedrawRequested),
        Some(NativeWorkspaceWindowAction::Redraw(
            WorkspaceWindowId::new("floating-a").expect("valid id")
        ))
    );
}
