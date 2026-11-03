use super::*;

#[test]
fn spine374_f_frame_loop_reload_failure_preserves_old_and_success_resets_state() {
    let first = Fixture::new("f-reload-original");
    first.authored();
    let mut source: Value = serde_json::from_str(JSON).unwrap();
    let mut alternate = source["skins"][0].clone();
    alternate["name"] = json!("alternate");
    alternate["attachments"]["cloth"]["triangle"]["vertices"][0] = json!(20);
    source["skins"].as_array_mut().unwrap().push(alternate);
    first.write("Assets/hero.skeleton.json", &source);
    let mut r = Runtime::new(&first);
    let mut s = Session::new();
    s.fixed.push_back(vec![Intent::SetSkin {
        name: "alternate".into(),
    }]);
    r.tick(&mut s, false, 20).unwrap();
    s.fixed.push_back(vec![Intent::SetPaused { paused: true }]);
    r.tick(&mut s, false, 1).unwrap();
    // Resume the old source and leave an event pending at the replacement boundary.
    s.fixed.push_back(vec![Intent::SetPaused { paused: false }]);
    r.tick(&mut s, false, 11).unwrap();
    s.fixed.push_back(vec![Intent::Restart {
        name: "idle".into(),
    }]);
    r.tick(&mut s, false, 1).unwrap();
    let before = r
        .frame_loop
        .spine_module()
        .unwrap()
        .snapshot("robot")
        .unwrap()
        .clone();
    let geometry = r
        .frame_loop
        .spine_module()
        .unwrap()
        .geometry("robot")
        .unwrap();
    let old_digest = r.frame_loop.spine_module().unwrap().registry_digest.clone();
    let old_package = r.package.clone();

    let next = Fixture::new("f-reload-next");
    let mut author = next.authored();
    author["default"] = json!("run");
    next.write("Animations/hero.animator-description-2d.json", &author);
    let input = next.prepare().unwrap();
    let root = next.package(&input);
    r.package = load_runtime_package(&root).value.unwrap();
    let asset = &r.package.spine_animator_registry.as_ref().unwrap().assets[0];
    let payload = root.join(&asset.payload_path);
    let offline = payload.with_extension("offline");
    fs::rename(&payload, &offline).unwrap();
    let rejected = r.tick(&mut s, true, 0).unwrap_err();
    assert!(rejected.report.terminal_fault);
    assert_eq!(
        r.frame_loop.spine_module().unwrap().registry_digest,
        old_digest
    );
    assert_eq!(
        r.frame_loop.spine_module().unwrap().snapshot("robot"),
        Some(&before)
    );
    assert_eq!(
        r.frame_loop
            .spine_module()
            .unwrap()
            .geometry("robot")
            .unwrap(),
        geometry
    );
    // The failed attempt cannot consume the old module's pending event.
    r.package = old_package;
    r.tick(&mut s, true, 0).unwrap();
    assert!(!s.seen.last().unwrap().1.events.is_empty());

    fs::rename(&offline, &payload).unwrap();
    r.package = load_runtime_package(&root).value.unwrap();
    r.tick(&mut s, true, 0).unwrap();
    let module = r.frame_loop.spine_module().unwrap();
    assert_ne!(module.registry_digest, old_digest);
    assert_eq!(module.tick_index(), None);
    assert_eq!(module.snapshot("robot").unwrap().pose_revision, 0);
    let expected = SpineModule::load(&r.package, &r.world)
        .unwrap()
        .geometry("robot")
        .unwrap();
    assert_eq!(
        module.geometry("robot").unwrap(),
        expected,
        "reload kept old skin/deform"
    );
    assert_ne!(module.geometry("robot").unwrap(), geometry);
    assert!(s.seen.last().unwrap().1.events.is_empty());
    r.tick(&mut s, false, 1).unwrap();
    assert_eq!(
        r.frame_loop
            .spine_module()
            .unwrap()
            .snapshot("robot")
            .unwrap()
            .pose_revision,
        1
    );
}
