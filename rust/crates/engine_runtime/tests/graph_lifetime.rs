use engine_runtime::render_graph::*;

fn clear(id: &str, target: &str) -> RenderPass {
    RenderPass {
        pass_id: id.into(),
        pass_name: id.into(),
        pass_kind: RenderPassKind::Clear,
        view_id: "main".into(),
        reads: vec![],
        writes: vec![target.into()],
        color_targets: vec![target.into()],
        depth_target: None,
        commands: vec![RenderPassCommand::Clear {
            target: target.into(),
            color: color([1., 0., 0., 1.]),
        }]
        .into(),
        debug_source: None,
    }
}
fn graph() -> RenderGraph {
    let mut g = RenderGraph::new("lifetime", 1);
    g.output_target = Some("out".into());
    g.resources
        .push(RenderResource::surface_backbuffer("out", 16, 16));
    g
}
#[test]
fn graph_lifetime_uninitialized_read_fails_before_submit() {
    let mut g = graph();
    let mut scratch = RenderResource::surface_backbuffer("scratch", 16, 16);
    scratch.resource_kind = RenderResourceKind::Texture;
    scratch.lifetime = RenderResourceLifetime::FrameLocal;
    g.resources.push(scratch);
    let mut p = clear("consumer", "out");
    p.reads.push("scratch".into());
    g.passes.push(p);
    assert!(g
        .validate()
        .iter()
        .any(|d| d.code == "graph.uninitialized_read"));
}
#[test]
fn graph_lifetime_explicit_topology_and_cycles() {
    let mut g = graph();
    g.passes = vec![clear("consumer", "out"), clear("producer", "out")];
    g.contracts.before.push((
        GraphPassId("producer".into()),
        GraphPassId("consumer".into()),
    ));
    assert_eq!(g.compile_schedule().unwrap().order, vec![1, 0]);
    g.contracts.before.push((
        GraphPassId("consumer".into()),
        GraphPassId("producer".into()),
    ));
    assert!(g
        .compile_schedule()
        .unwrap_err()
        .iter()
        .any(|e| e.code == "graph.cycle"));
    let p = engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(&g);
    assert!(p.has_errors());
    assert!(!p
        .commands
        .iter()
        .any(|c| matches!(c, engine_runtime::rhi_command_plan::RhiCommand::Submit)));
}
#[test]
fn graph_lifetime_subresources_feedback_and_attachment_shape() {
    let mut g = graph();
    g.passes.push(clear("write", "out"));
    g.contracts.accesses.insert(
        GraphPassId("write".into()),
        vec![GraphAccess {
            resource: GraphResourceId("out".into()),
            aspect: GraphAspect::Depth,
            mip: 0,
            layer: 0,
            kind: GraphAccessKind::Write,
        }],
    );
    assert!(g
        .compile_schedule()
        .unwrap_err()
        .iter()
        .any(|e| e.code == "graph.invalid_subresource"));
    let a = g
        .contracts
        .accesses
        .get_mut(&GraphPassId("write".into()))
        .unwrap();
    a[0].aspect = GraphAspect::Color;
    a[0].mip = 1;
    assert!(g.compile_schedule().is_err());
    let mut g = graph();
    g.passes.push(clear("write", "out"));
    g.contracts.accesses.insert(
        GraphPassId("write".into()),
        vec![GraphAccess {
            resource: GraphResourceId("out".into()),
            aspect: GraphAspect::Color,
            mip: 0,
            layer: 0,
            kind: GraphAccessKind::Read,
        }],
    );
    assert!(
        g.compile_schedule()
            .unwrap_err()
            .iter()
            .any(|e| e.code == "graph.undeclared_access"),
        "explicit metadata cannot disguise an attachment write as a read"
    );
    let a = g
        .contracts
        .accesses
        .get_mut(&GraphPassId("write".into()))
        .unwrap();
    a[0].kind = GraphAccessKind::Write;
    let mut read = a[0].clone();
    read.kind = GraphAccessKind::Read;
    a.push(read);
    assert!(g
        .compile_schedule()
        .unwrap_err()
        .iter()
        .any(|e| e.code == "graph.feedback"));
    g.contracts.accesses.clear();
    g.contracts.resources.insert(
        GraphResourceId("out".into()),
        GraphResourceShape {
            layers: u32::MAX,
            ..Default::default()
        },
    );
    assert!(g
        .compile_schedule()
        .unwrap_err()
        .iter()
        .any(|e| e.code == "graph.invalid_shape"));
    g.contracts.resources.clear();
    let mut second = RenderResource::surface_backbuffer("second", 32, 16);
    second.lifetime = RenderResourceLifetime::External;
    g.resources.push(second);
    g.passes[0].color_targets.push("second".into());
    assert!(g
        .compile_schedule()
        .unwrap_err()
        .iter()
        .any(|e| e.code == "graph.attachment_mismatch"));
}
#[test]
fn graph_lifetime_transients_alias_only_nonoverlap_and_compatible_descriptors() {
    let mut g = graph();
    for id in ["a", "b", "overlap"] {
        let mut r = RenderResource::surface_backbuffer(id, 16, 16);
        r.resource_kind = RenderResourceKind::Texture;
        r.lifetime = RenderResourceLifetime::FrameLocal;
        g.resources.push(r);
    }
    let a = clear("a-write", "a");
    let b = clear("b-write", "b");
    let mut use_a = clear("a-read", "out");
    use_a.reads.push("a".into());
    let overlap = clear("overlap-write", "overlap");
    let mut use_overlap = clear("overlap-read", "out");
    use_overlap.reads.push("overlap".into());
    g.passes = vec![a, overlap, use_a, b, use_overlap];
    let s = g.compile_schedule().unwrap();
    let slot = |id: &str| {
        s.lifetimes
            .iter()
            .find(|l| l.resource.0 == id)
            .unwrap()
            .transient_slot
    };
    assert_eq!(slot("a"), slot("b"));
    assert_ne!(slot("a"), slot("overlap"));
    g.resources
        .iter_mut()
        .find(|r| r.resource_id == "b")
        .unwrap()
        .size
        .width = 32;
    let s = g.compile_schedule().unwrap();
    let a = s.lifetimes.iter().find(|l| l.resource.0 == "a").unwrap();
    let b = s.lifetimes.iter().find(|l| l.resource.0 == "b").unwrap();
    assert_ne!(a.transient_slot, b.transient_slot);
}
#[test]
fn graph_lifetime_structure_cache_and_history_identity_are_independent_of_frame_commands() {
    let mut g = graph();
    g.passes.push(clear("producer", "out"));
    let mut cache = GraphScheduleCache::default();
    let old = cache.compile(&g).unwrap();
    g.frame_index = 99;
    if let RenderPassCommand::Clear { color: c, .. } = &mut g.passes[0].commands[0] {
        *c = color([0., 1., 0., 1.]);
    }
    assert!(std::sync::Arc::ptr_eq(&old, &cache.compile(&g).unwrap()));
    assert_eq!(cache.reuses, 1);
    g.resources[0].size.width = 32;
    assert!(!std::sync::Arc::ptr_eq(&old, &cache.compile(&g).unwrap()));
    g.resources[0].lifetime = RenderResourceLifetime::Persistent;
    let h = GraphHistoryIdentity {
        device_epoch: 1,
        origin_epoch: 1,
        camera_id: 1,
        resource_version: 1,
        extent: [32, 16],
    };
    g.contracts.resources.insert(
        GraphResourceId("out".into()),
        GraphResourceShape {
            history: Some(h),
            ..Default::default()
        },
    );
    let old = cache.compile(&g).unwrap();
    g.contracts
        .resources
        .get_mut(&GraphResourceId("out".into()))
        .unwrap()
        .history
        .as_mut()
        .unwrap()
        .origin_epoch += 1;
    assert!(!std::sync::Arc::ptr_eq(&old, &cache.compile(&g).unwrap()));
    g.contracts
        .resources
        .get_mut(&GraphResourceId("out".into()))
        .unwrap()
        .history
        .as_mut()
        .unwrap()
        .extent = [1, 1];
    assert!(cache.compile(&g).is_err());
}
