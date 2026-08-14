use crate::native_workspace_window_host::*;

#[derive(Default)]
struct ProxyFactory {
    fail_create: bool,
    fail_move: bool,
    fail_destroy: bool,
    creates: usize,
    moves: usize,
    destroys: usize,
}

impl NativeDragProxyFactory for ProxyFactory {
    fn create(&mut self) -> Result<NativeDragProxyKey, NativeDragProxyStage> {
        if self.fail_create {
            return Err(NativeDragProxyStage::Create);
        }
        self.creates += 1;
        Ok(NativeDragProxyKey(41))
    }

    fn move_to(
        &mut self,
        _key: NativeDragProxyKey,
        _screen_x: i32,
        _screen_y: i32,
    ) -> Result<(), NativeDragProxyStage> {
        if self.fail_move {
            return Err(NativeDragProxyStage::Move);
        }
        self.moves += 1;
        Ok(())
    }

    fn destroy(&mut self, _key: NativeDragProxyKey) -> Result<(), NativeDragProxyStage> {
        if self.fail_destroy {
            return Err(NativeDragProxyStage::Destroy);
        }
        self.destroys += 1;
        Ok(())
    }
}

#[test]
fn workspace_native_drag_owns_one_transient_proxy_and_cleans_it() {
    let mut host = NativeWorkspaceWindowHost::default();
    let mut factory = ProxyFactory::default();
    assert!(host
        .reconcile_drag_proxy(Some((500, 320)), &mut factory)
        .is_empty());
    assert_eq!(factory.creates, 1);
    assert!(host
        .reconcile_drag_proxy(Some((520, 330)), &mut factory)
        .is_empty());
    assert_eq!(factory.creates, 1);
    assert_eq!(factory.moves, 2);
    assert!(host.reconcile_drag_proxy(None, &mut factory).is_empty());
    assert_eq!(factory.destroys, 1);
    assert!(!host.drag_proxy_is_live());
}

#[test]
fn workspace_native_drag_proxy_failures_fail_closed() {
    for stage in [
        NativeDragProxyStage::Create,
        NativeDragProxyStage::Move,
        NativeDragProxyStage::Destroy,
    ] {
        let mut host = NativeWorkspaceWindowHost::default();
        let mut factory = ProxyFactory {
            fail_create: stage == NativeDragProxyStage::Create,
            fail_move: stage == NativeDragProxyStage::Move,
            fail_destroy: stage == NativeDragProxyStage::Destroy,
            ..ProxyFactory::default()
        };
        let diagnostics = host.reconcile_drag_proxy(Some((1, 2)), &mut factory);
        if stage == NativeDragProxyStage::Destroy {
            assert!(diagnostics.is_empty());
            let diagnostics = host.reconcile_drag_proxy(None, &mut factory);
            assert_eq!(diagnostics[0].stage, stage);
        } else {
            assert_eq!(diagnostics[0].stage, stage);
        }
        assert!(!host.drag_proxy_is_live());
    }
}
