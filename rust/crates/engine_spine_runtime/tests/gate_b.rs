use engine_spine_runtime::{
    BlendMode, Error,
    import::{ValidatedSource, validate},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const PROBE: &str = include_str!("fixtures/probe.json");
const ATLAS: &str = include_str!("fixtures/probe.atlas");
fn refs() -> BTreeMap<String, String> {
    BTreeMap::from([("probe.png".into(), "texture-probe".into())])
}

#[test]
fn spine374_f_clipping_batches_preserve_scalar_geometry_bytes() {
    use sha2::{Digest, Sha256};
    let mut hashes = Vec::new();
    let mut geometry_us = Vec::new();
    for source in [
        include_str!("fixtures/recipe-regular.json"),
        include_str!("fixtures/recipe-clipping.json"),
    ] {
        let asset = validate(source, ATLAS, &refs()).unwrap().load().unwrap();
        let mut instance = asset.instance().unwrap();
        instance.play(0, "clip0", None, 0.0, true, false).unwrap();
        for (clip, delta) in [
            (None, 0.0),
            (None, 0.25),
            (None, 0.25),
            (Some("clip1"), 0.1),
            (Some("clip2"), 0.1),
            (Some("clip3"), 0.1),
        ] {
            if let Some(clip) = clip {
                instance.play(0, clip, None, 0.5, true, false).unwrap();
            }
            instance.step(delta).unwrap();
            let g = instance.geometry(100.0).unwrap();
            for _ in 0..8 {
                let started = std::time::Instant::now();
                let repeat = instance.geometry(100.0).unwrap();
                geometry_us.push(started.elapsed().as_micros());
                assert_eq!(repeat, g);
            }
            let mut h = Sha256::new();
            for count in [g.vertices.len(), g.indices.len(), g.batches.len()] {
                h.update((count as u64).to_le_bytes());
            }
            for v in &g.vertices {
                for x in [v.x, v.y, v.u, v.v, v.r, v.g, v.b, v.a] {
                    h.update(x.to_bits().to_le_bytes());
                }
            }
            for i in &g.indices {
                h.update(i.to_le_bytes());
            }
            for b in &g.batches {
                for x in [b.page, b.blend as u32, b.first_index, b.index_count] {
                    h.update(x.to_le_bytes());
                }
            }
            for x in g.bounds.unwrap() {
                h.update(x.to_bits().to_le_bytes());
            }
            hashes.push(format!("{:x}", h.finalize()));
        }
    }
    // Captured from the original one-triangle-per-call implementation.
    geometry_us.sort_unstable();
    eprintln!(
        "owner debug geometry only: samples={}, median={}us, p95={}us; not Release qualification",
        geometry_us.len(),
        geometry_us[geometry_us.len() / 2],
        geometry_us[(geometry_us.len() * 95).div_ceil(100) - 1]
    );
    assert_eq!(
        hashes,
        [
            "f72276b3a5bd562c9de399cb831b99917b118e09ccde5aa5483b8820c5c05c9a",
            "3f0630ca5a47bb1e51a46160f9a8f45c6446c29683b26d970947d5005593a0ea",
            "211d0bf5286b49ca3514c7b927112c63cd5e910b90e53f7838fd38b9f337a60a",
            "bfb2ff3469b5eddba2d3e8319a5a2d3fefde5c52a79af6b3f554686cdacf52f5",
            "e1903853a4c9ed98ae8c0d0386219565639ffb6ea9005606b789c1205b4792ad",
            "e960c990af2ff9df204f45e64f281950c57ce938e4a5c65796d0f14b43a41140",
            "6d83bda5bd40277ac14028a385c6868af7a4d0d29e2eecfbafc76da05dfb0f6d",
            "8f49502a197cd029533ffffc3217e7693230c850dd93a4330ca75c4aed3bb2c3",
            "1d66af37d4a793fa923bf85441a3383e9ab1d80d68d90a00f996d42c1e2acb68",
            "44c08545ea324ec692ffb08e1083784fd03d430514180c5c454d7bff4babab36",
            "972f427ab765213d09e7d26fb3fb891885baa75bdd46472520565dc94d511b24",
            "faab17dc15206eae2147445a58237b98347198d0171f94b904c5ad2907b7424e"
        ]
    );
}
fn checked(v: &Value) -> ValidatedSource {
    validate(&v.to_string(), ATLAS, &refs()).unwrap()
}
fn probe() -> Value {
    serde_json::from_str(PROBE).unwrap()
}
fn close(a: f32, b: f32) {
    assert!((a - b).abs() <= 1e-4_f32.max(2e-5 * b.abs()), "{a} != {b}");
}
fn rejected(v: &Value) {
    assert!(
        validate(&v.to_string(), ATLAS, &refs()).is_err(),
        "unexpected admission: {v}"
    );
}

#[test]
fn spine374_b_safe_load_and_local_geometry_golden() {
    let input = validate(PROBE, ATLAS, &refs()).unwrap();
    assert_eq!(input.pages[0].texture_asset_id, "texture-probe");
    assert!(!input.pages[0].pma);
    let asset = input.load().unwrap();
    let mut i = asset.instance().unwrap();
    i.play(0, "motion", None, 0.0, false, false).unwrap();
    i.step(0.5).unwrap();
    let g = i.geometry(100.0).unwrap();
    assert_eq!(g.indices, [0, 1, 2]);
    assert_eq!(g.batches.len(), 1);
    for (v, (x, y)) in g
        .vertices
        .iter()
        .zip([(0.03, 0.1), (0.04, 0.1), (0.02, 0.12)])
    {
        close(v.x, x);
        close(v.y, y);
    }
    for (a, b) in g.bounds.unwrap().into_iter().zip([0.02, 0.1, 0.04, 0.12]) {
        close(a, b);
    }
    assert_eq!(g.batches[0].blend, BlendMode::Normal);
    assert_eq!(i.geometry_with_capacity(100.0, 2, 3), Err(Error::Capacity));
    assert_eq!(
        i.geometry(100.0).unwrap(),
        g,
        "capacity rejection must preserve pose"
    );
    assert_eq!(i.geometry(f32::NAN), Err(Error::InvalidInput));
}

#[test]
fn spine374_f_small_geometry_output_does_not_reserve_the_frame_limit() {
    let source = checked(&probe());
    let asset = source.load().unwrap();
    let instance = asset.instance().unwrap();
    for _ in 0..3 {
        let g = instance.geometry(100.0).unwrap();
        assert_eq!(g.vertices.len(), 3);
        assert_eq!(g.indices.len(), 3);
        let reserved = g.vertices.capacity() * std::mem::size_of::<engine_spine_runtime::Vertex>()
            + g.indices.capacity() * std::mem::size_of::<u32>();
        assert!(
            reserved <= 16 * 1024,
            "three-vertex frame reserved {reserved} bytes"
        );
    }
}

#[test]
fn spine374_f_geometry_capacity_follows_skin_growth_without_caching_the_pose() {
    let mut value = probe();
    value["animations"] = json!({});
    let mut wide = value["skins"][0].clone();
    wide["name"] = json!("wide");
    let mesh = &mut wide["attachments"]["cloth"]["triangle"];
    mesh["vertices"] = json!(
        (0..300)
            .flat_map(|n| [n as f32, (n % 3) as f32])
            .collect::<Vec<_>>()
    );
    mesh["uvs"] = json!(
        (0..300)
            .flat_map(|n| [f32::from(n % 2 == 0), 0.0])
            .collect::<Vec<_>>()
    );
    mesh["triangles"] = json!((0..300).collect::<Vec<_>>());
    value["skins"].as_array_mut().unwrap().push(wide);
    let source = checked(&value);
    let asset = source.load().unwrap();
    let mut instance = asset.instance().unwrap();
    let original = instance.geometry(1.0).unwrap();
    instance.set_skin("wide").unwrap();
    let grown = instance.geometry(1.0).unwrap();
    assert_eq!(grown.vertices.len(), 300);
    assert_eq!(
        grown,
        instance.geometry_with_capacity(1.0, 32768, 98304).unwrap()
    );
    let warm = instance.geometry(1.0).unwrap();
    assert_eq!(warm, grown);
    assert!(warm.vertices.capacity() <= 512 && warm.indices.capacity() <= 512);
    assert_eq!(
        instance.geometry_with_capacity(1.0, 299, 300),
        Err(Error::Capacity)
    );
    assert_eq!(instance.geometry(1.0).unwrap(), grown);
    instance.set_skin("default").unwrap();
    assert_eq!(instance.geometry(1.0).unwrap(), original);
    let small = instance.geometry(1.0).unwrap();
    assert!(small.vertices.capacity() <= 64 && small.indices.capacity() <= 192);
    let independent = asset.instance().unwrap().geometry(1.0).unwrap();
    assert_eq!(independent, original);
    assert!(independent.vertices.capacity() <= 64);
}

#[test]
fn spine374_b_rejects_structure_before_native() {
    for path in ["4.1.00", "4.3.00", "4.20", "4.2.", "4.2.x"] {
        let mut v = probe();
        v["skeleton"]["spine"] = path.into();
        assert_eq!(
            validate(&v.to_string(), ATLAS, &refs()).err().unwrap().code,
            "spine.import.version_mismatch"
        );
    }
    for text in [
        r#"{"bones":[],"bones":[]}"#,
        r#"{"x":{"a":1,"a":2}}"#,
        r#"{"x":"\u0000"}"#,
        "[]",
        "null",
        "not-json",
    ] {
        assert!(validate(text, ATLAS, &refs()).is_err());
    }
    let mut cases = Vec::new();
    let mut v = probe();
    v["bones"][1]["parent"] = "missing".into();
    cases.push(v);
    let mut v = probe();
    v["slots"][0]["bone"] = "missing".into();
    cases.push(v);
    let mut v = probe();
    v["bones"][1]["name"] = "root".into();
    cases.push(v);
    let mut v = probe();
    v["skins"][0]["attachments"]["unknown"] = json!({});
    cases.push(v);
    let mut v = probe();
    v["bones"][0]["x"] = json!(1e30);
    cases.push(v);
    let mut v = probe();
    v["animations"]["motion"]["bones"]["upper"]["translate"][0]["curve"] = json!([0, 0, 1, 1]);
    cases.push(v);
    let mut v = probe();
    v["animations"]["motion"]["bones"]["upper"]["translate"] = json!([]);
    cases.push(v);
    let mut v = probe();
    v["animations"]["motion"]["bones"]["upper"]["typo"] = json!([{}]);
    cases.push(v);
    let mut v = probe();
    v["skeleton"]["typo"] = true.into();
    cases.push(v);
    let mut v = probe();
    v["slots"][0]["color"] = "fff".into();
    cases.push(v);
    for v in cases {
        rejected(&v);
    }
}

#[test]
fn spine374_b_rejects_unqualified_features_explicitly() {
    {
        let mut v = probe();
        v["physics"] = json!([]);
        assert_eq!(
            validate(&v.to_string(), ATLAS, &refs()).err().unwrap().code,
            "spine.import.unsupported_feature"
        );
    }
    let mut v = probe();
    v["skins"][0]["attachments"]["cloth"]["triangle"]["sequence"] = json!({});
    rejected(&v);
    let mut v = probe();
    v["slots"][0]["dark"] = "000000".into();
    rejected(&v);
}

#[test]
fn spine374_b_reference_and_vertex_negative_cases() {
    for (key, value) in [
        ("uvs", json!([0, 0, 1])),
        ("triangles", json!([0, 1, 3])),
        ("triangles", json!([0, 1])),
        ("vertices", json!([1, 99, 0, 0, 1])),
        ("vertices", json!([5, 0, 0, 0, 1])),
        ("vertices", json!([0, 0, 0])),
        ("path", json!("missing")),
        ("parent", json!("missing")),
    ] {
        let mut v = probe();
        v["skins"][0]["attachments"]["cloth"]["triangle"][key] = value;
        rejected(&v);
    }
    let mut v = probe();
    v["animations"]["motion"]["attachments"]["default"]["cloth"]["triangle"]["deform"][1]["offset"] =
        1.into();
    rejected(&v);
    let mut v = probe();
    v["animations"]["motion"]["attachments"]["default"]["cloth"]["triangle"]["deform"][1]["offset"] =
        (-1).into();
    rejected(&v);
    let mut v = probe();
    v["animations"]["motion"]["events"][0]["name"] = "missing".into();
    rejected(&v);
    let mut v = probe();
    v["animations"]["motion"]["drawOrder"] = json!([{"offsets":[{"slot":"cloth","offset":1}]}]);
    rejected(&v);
}

#[test]
fn spine374_b_atlas_pages_rotation_and_no_external_paths() {
    assert_eq!(
        validate(PROBE, ATLAS, &BTreeMap::new()).err().unwrap().code,
        "spine.import.atlas_page_missing"
    );
    for page in [
        "../probe.png",
        "/probe.png",
        "C:/probe.png",
        "a\\probe.png",
        "a/../probe.png",
    ] {
        assert!(validate(PROBE, &ATLAS.replace("probe.png", page), &refs()).is_err());
    }
    for (from, to) in [
        ("size: 2, 2", "size: 0, 2"),
        ("bounds: 0, 0, 2, 2", "bounds: 1, 0, 2, 2"),
        ("rotate: false", "rotate: 180"),
        ("pma: false", "pma: unknown"),
        ("offsets: 0, 0, 2, 2", "offsets: -1, 0, 2, 2"),
    ] {
        assert!(validate(PROBE, &ATLAS.replace(from, to), &refs()).is_err());
    }
    let atlas = format!(
        "{}\n{}",
        ATLAS.replace("rotate: false", "rotate: 90"),
        ATLAS
            .replace("probe.png", "second.png")
            .replace("triangle\n", "second\n")
            .replace("pma: false", "pma: true")
    );
    let bindings = BTreeMap::from([
        ("probe.png".into(), "t0".into()),
        ("second.png".into(), "t1".into()),
    ]);
    let mut v = probe();
    v["skins"][0]["attachments"]["cloth"]["alternate"] =
        json!({"type":"region","path":"second","width":2,"height":2});
    v["animations"]["switch"] = json!({"slots":{"cloth":{"attachment":[{"name":"alternate"}]}}});
    let input = validate(&v.to_string(), &atlas, &bindings).unwrap();
    assert_eq!(input.pages.len(), 2);
    assert!(input.pages[1].pma);
    let asset = input.load().unwrap();
    let mut instance = asset.instance().unwrap();
    let g = instance.geometry(1.0).unwrap();
    assert_eq!(g.batches[0].page, 0);
    assert_eq!(
        g.vertices.iter().map(|v| (v.u, v.v)).collect::<Vec<_>>(),
        [(0.0, 1.0), (0.0, 0.0), (1.0, 1.0)]
    );
    instance.play(0, "switch", None, 0.0, false, false).unwrap();
    instance.step(0.0).unwrap();
    assert_eq!(instance.geometry(1.0).unwrap().batches[0].page, 1);
}

#[test]
fn spine374_b_weighted_deform_and_linked_mesh_golden() {
    let mut v = probe();
    // Each vertex averages root and upper; upper has setup y=10. Weighted
    // deform has two xy offsets per vertex, unlike six unweighted coordinates.
    v["skins"][0]["attachments"]["cloth"]["triangle"]["vertices"] = json!([
        2, 0, 0, 0, 0.5, 2, 0, 0, 0.5, 2, 0, 2, 0, 0.5, 2, 2, 0, 0.5, 2, 0, 0, 2, 0.5, 2, 0, 2, 0.5
    ]);
    v["skins"][0]["attachments"]["cloth"]["linked"] =
        json!({"type":"linkedmesh","path":"triangle","parent":"triangle","timelines":true});
    v["animations"]["motion"]["attachments"]["default"]["cloth"]["triangle"]["deform"][1]["vertices"] =
        json!([2, 0, 2, 0]);
    v["animations"]["attack"]["attachments"]["default"]["cloth"]["triangle"]["deform"][1]["vertices"] =
        json!([4, 0, 4, 0]);
    v["slots"][0]["attachment"] = "linked".into();
    let source = checked(&v);
    let asset = source.load().unwrap();
    let mut instance = asset.instance().unwrap();
    instance.play(0, "motion", None, 0.0, false, false).unwrap();
    instance.step(0.5).unwrap();
    let g = instance.geometry(1.0).unwrap();
    for (v, (x, y)) in g.vertices.iter().zip([(2.0, 5.0), (3.0, 5.0), (1.0, 7.0)]) {
        close(v.x, x);
        close(v.y, y);
    }
    let mut bad = v.clone();
    bad["skins"][0]["attachments"]["cloth"]["linked"]["parent"] = "linked".into();
    rejected(&bad);
}

fn clipped() -> Value {
    json!({"skeleton":{"spine":"4.2.00"},"bones":[{"name":"root"}],"slots":[{"name":"clip","bone":"root","attachment":"clip"},{"name":"image","bone":"root","attachment":"image"}],"skins":[{"name":"default","attachments":{"clip":{"clip":{"type":"clipping","end":"image","vertexCount":4,"vertices":[-1,-1,-1,1,0,1,0,-1]}},"image":{"image":{"type":"region","path":"triangle","width":2,"height":2}}}}]})
}
#[test]
fn spine374_b_clipping_bounds_and_capacity_are_real_geometry() {
    let asset = checked(&clipped()).load().unwrap();
    let i = asset.instance().unwrap();
    let g = i.geometry(1.0).unwrap();
    assert!(!g.indices.is_empty());
    let b = g.bounds.unwrap();
    for (a, b) in b.into_iter().zip([-1.0, -1.0, 0.0, 1.0]) {
        close(a, b);
    }
    let area: f32 = g
        .indices
        .chunks_exact(3)
        .map(|t| {
            let a = g.vertices[t[0] as usize];
            let b = g.vertices[t[1] as usize];
            let c = g.vertices[t[2] as usize];
            ((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)).abs() * 0.5
        })
        .sum();
    close(area, 2.0);
    for v in &g.vertices {
        close(v.u, (v.x + 1.0) * 0.5);
        close(v.v, (1.0 - v.y) * 0.5);
    }
    assert_eq!(i.geometry_with_capacity(1.0, 1, 3), Err(Error::Capacity));
    let mut bad = clipped();
    bad["skins"][0]["attachments"]["clip"]["clip"]["vertexCount"] = 17.into();
    rejected(&bad);
    let mut bad = clipped();
    bad["slots"].as_array_mut().unwrap().insert(
        1,
        json!({"name":"nested","bone":"root","attachment":"clip"}),
    );
    bad["skins"][0]["attachments"]["nested"] = bad["skins"][0]["attachments"]["clip"].clone();
    rejected(&bad);
}

#[test]
fn spine374_b_draw_order_blends_and_skin_switch() {
    let mut v = probe();
    v["animations"] = json!({"swap":{"drawOrder":[{"offsets":[{"slot":"a","offset":2}]}]}});
    v["slots"] = json!([{"name":"a","bone":"upper","attachment":"image","blend":"normal"},{"name":"b","bone":"upper","attachment":"image","blend":"additive"},{"name":"c","bone":"upper","attachment":"image","blend":"multiply"},{"name":"d","bone":"upper","attachment":"image","blend":"screen"}]);
    let mut at = serde_json::Map::new();
    for slot in ["a", "b", "c", "d"] {
        at.insert(
            slot.into(),
            json!({"image":{"type":"region","path":"triangle","width":2,"height":2}}),
        );
    }
    let mut alt = at.clone();
    alt["a"]["image"]["width"] = 4.into();
    v["skins"] = json!([{"name":"default","attachments":at},{"name":"wide","attachments":alt}]);
    let asset = checked(&v).load().unwrap();
    let mut i = asset.instance().unwrap();
    let g = i.geometry(1.0).unwrap();
    assert_eq!(
        g.batches.iter().map(|b| b.blend).collect::<Vec<_>>(),
        [
            BlendMode::Normal,
            BlendMode::Additive,
            BlendMode::Multiply,
            BlendMode::Screen
        ]
    );
    i.play(0, "swap", None, 0.0, false, false).unwrap();
    i.step(0.0).unwrap();
    assert_eq!(
        i.geometry(1.0)
            .unwrap()
            .batches
            .iter()
            .map(|b| b.blend)
            .collect::<Vec<_>>(),
        [
            BlendMode::Additive,
            BlendMode::Multiply,
            BlendMode::Normal,
            BlendMode::Screen
        ]
    );
    i.set_skin("wide").unwrap();
    let wide = i.geometry(1.0).unwrap();
    close(wide.bounds.unwrap()[0], -2.0);
    assert_eq!(i.set_skin("missing"), Err(Error::UnknownName));
    assert_eq!(i.geometry(1.0).unwrap(), wide);
    i.set_skin("default").unwrap();
    close(i.geometry(1.0).unwrap().bounds.unwrap()[0], -1.0);
}

#[test]
fn spine374_b_frozen_recipes_pass_safe_admission_and_extract() {
    for source in [
        include_str!("fixtures/recipe-single.json"),
        include_str!("fixtures/recipe-regular.json"),
        include_str!("fixtures/recipe-clipping.json"),
    ] {
        let checked = validate(source, ATLAS, &refs()).unwrap();
        let asset = checked.load().unwrap();
        let mut i = asset.instance().unwrap();
        i.play(0, "clip0", None, 0.0, true, false).unwrap();
        i.step(0.5).unwrap();
        let g = i.geometry(100.0).unwrap();
        assert!(!g.indices.is_empty());
        assert!(g.vertices.len() <= 32768);
        assert!(g.indices.len() <= 98304);
    }
}

#[test]
fn spine374_b_descriptor_and_cook_dependency_identity() {
    use engine_spine_runtime::import::validate_declared;
    let spec = json!({"schemaVersion":"spine-skeleton-asset.v1","assetId":"actor","skeletonSource":"assets/actor.json","atlasSource":"assets/actor.atlas","pages":{"probe.png":{"assetId":"texture-probe"}},"format":"json","exportVersion":"4.2.00","pixelsPerUnit":100});
    let source = validate_declared(&spec.to_string(), PROBE, ATLAS).unwrap();
    let dependencies = BTreeMap::from([("texture-probe".into(), "a".repeat(64))]);
    let key = source
        .cook_key(&dependencies, "animator.spine.4.2.v1")
        .unwrap();
    let other = BTreeMap::from([("texture-probe".into(), "b".repeat(64))]);
    assert_ne!(
        key,
        source.cook_key(&other, "animator.spine.4.2.v1").unwrap()
    );
    assert!(
        source
            .cook_key(&BTreeMap::new(), "animator.spine.4.2.v1")
            .is_err()
    );
    assert!(source.cook_key(&dependencies, "wrong").is_err());
    assert_ne!(source.event_id("motion", 0), source.event_id("motion", 1));
    for (k, value) in [
        ("format", json!("binary")),
        ("exportVersion", json!("4.2.01")),
        ("skeletonSource", json!("../outside.json")),
        ("pixelsPerUnit", json!(0)),
        ("unknown", json!(true)),
    ] {
        let mut bad = spec.clone();
        bad[k] = value;
        assert!(validate_declared(&bad.to_string(), PROBE, ATLAS).is_err());
    }
    let mut scaled = spec;
    scaled["pixelsPerUnit"] = 200.into();
    assert_ne!(
        key,
        validate_declared(&scaled.to_string(), PROBE, ATLAS)
            .unwrap()
            .cook_key(&dependencies, "animator.spine.4.2.v1")
            .unwrap()
    );
}

#[test]
fn spine374_b_source_event_f64_quantization_payload_and_ordinal() {
    let mut v = probe();
    v["events"]["beat"] =
        json!({"int":7,"float":0.5,"string":"default","volume":0.75,"balance":-0.25});
    v["animations"]["motion"]["events"] = json!([{"name":"beat","time":3599.123456},{"name":"beat","time":3599.123456,"int":9,"string":"override"},{"name":"end","time":3600}]);
    let input = checked(&v);
    let clip = input
        .animation_data
        .iter()
        .find(|c| c.name == "motion")
        .unwrap();
    assert_eq!(clip.duration_us, 3_600_000_000);
    assert_eq!(clip.events[0].time_us, 3_599_123_456);
    assert_eq!(clip.events[1].time_us, 3_599_123_456);
    assert_eq!(clip.events[0].source_ordinal, 0);
    assert_eq!(clip.events[1].source_ordinal, 1);
    assert_eq!(clip.events[0].int_value, 7);
    assert_eq!(clip.events[1].int_value, 9);
    assert_eq!(clip.events[1].string_value, "override");
    close(clip.events[0].volume, 0.75);
    close(clip.events[0].balance, -0.25);
    let mut zero = probe();
    zero["animations"] = json!({"bad":{"events":[{"name":"beat"}]}});
    rejected(&zero);
}

#[test]
fn spine374_b_sparse_deform_expansion_is_bounded_before_native() {
    let mut v: Value = serde_json::from_str(include_str!("fixtures/recipe-clipping.json")).unwrap();
    let frames: Vec<_> = (0..40000).map(|i| json!({"time":i as f64/20.0})).collect();
    v["animations"]["clip0"]["attachments"]["default"]["mesh0"]["grid"]["deform"] =
        Value::Array(frames);
    assert!(
        validate(&v.to_string(), ATLAS, &refs())
            .err()
            .unwrap()
            .message
            .contains("expanded timeline")
    );
}

#[test]
fn spine374_b_deterministic_structural_mutations_never_enter_unchecked_parser() {
    let base = probe();
    let replacements = [
        Value::Null,
        json!(true),
        json!("bad"),
        json!({}),
        json!([]),
        json!(-1),
        json!(1e30),
    ];
    let paths = [
        "/skeleton",
        "/skeleton/spine",
        "/bones",
        "/bones/0",
        "/bones/1/parent",
        "/slots",
        "/slots/0/bone",
        "/skins",
        "/skins/0/attachments/cloth/triangle",
        "/skins/0/attachments/cloth/triangle/vertices",
        "/skins/0/attachments/cloth/triangle/uvs",
        "/skins/0/attachments/cloth/triangle/triangles",
        "/animations/motion/bones/upper/translate",
        "/animations/motion/bones/upper/translate/1",
        "/animations/motion/attachments/default/cloth/triangle/deform",
        "/animations/motion/events",
        "/animations/motion/events/1/name",
    ];
    let mut rejected_count = 0;
    for path in paths {
        for value in &replacements {
            let mut v = base.clone();
            *v.pointer_mut(path).unwrap() = value.clone();
            match validate(&v.to_string(), ATLAS, &refs()) {
                Err(_) => rejected_count += 1,
                Ok(checked) => {
                    let a = checked.load().unwrap();
                    let i = a.instance().unwrap();
                    i.geometry(100.0).unwrap();
                }
            }
        }
    }
    assert_eq!(rejected_count, paths.len() * replacements.len());
}

#[test]
fn spine374_b_one_and_two_bone_ik_and_animated_mix_golden() {
    let mut v = json!({"skeleton":{"spine":"4.2.00"},"bones":[{"name":"root"},{"name":"a","parent":"root","length":5},{"name":"b","parent":"a","x":5,"length":5},{"name":"target","parent":"root","x":6,"y":8}],"ik":[{"name":"reach","bones":["a","b"],"target":"target"}]});
    let asset = checked(&v).load().unwrap();
    let i = asset.instance().unwrap();
    let a = i.bone("a").unwrap();
    let b = i.bone("b").unwrap();
    close(a.rotation, 53.130104);
    close(b.x, 3.0);
    close(b.y, 4.0);
    close(b.rotation, 53.130104);
    v["ik"][0]["bones"] = json!(["a"]);
    v["animations"] = json!({"mix":{"ik":{"reach":[{"mix":0},{"time":1,"mix":1}]}}});
    let asset = checked(&v).load().unwrap();
    let mut i = asset.instance().unwrap();
    i.play(0, "mix", None, 0.0, false, false).unwrap();
    i.step(0.5).unwrap();
    close(i.bone("a").unwrap().rotation, 26.565052);
    let mut bad = v.clone();
    bad["ik"][0]["target"] = "missing".into();
    rejected(&bad);
    let mut bad = v.clone();
    bad["ik"][0]["bones"] = json!(["root"]);
    rejected(&bad);
    let mut bad = v.clone();
    bad["ik"][0]["bones"] = json!(["b", "a"]);
    rejected(&bad);
    let mut bad = v.clone();
    bad["animations"]["mix"]["ik"]["unknown"] = json!([{}]);
    rejected(&bad);
}

#[test]
fn spine374_b_transform_all_local_relative_combinations_golden() {
    for local in [false, true] {
        for relative in [false, true] {
            let v = json!({"skeleton":{"spine":"4.2.00"},"bones":[{"name":"root"},{"name":"a","parent":"root","x":2,"y":3},{"name":"target","parent":"root","x":10,"y":20}],"transform":[{"name":"follow","bones":["a"],"target":"target","local":local,"relative":relative,"x":1,"y":2}]});
            let asset = checked(&v).load().unwrap();
            let i = asset.instance().unwrap();
            let b = i.bone("a").unwrap();
            close(b.x, if relative { 13.0 } else { 11.0 });
            close(b.y, if relative { 25.0 } else { 22.0 });
        }
    }
    let mut v = probe();
    v["bones"][0]["rotation"] = 30.into();
    v["bones"][1]["rotation"] = 10.into();
    let asset = checked(&v).load().unwrap();
    close(
        asset.instance().unwrap().bone("lower").unwrap().rotation,
        40.0,
    );
}

#[test]
fn spine374_b_tint_once_and_page_mapping_in_cook_key() {
    let mut v = probe();
    v["slots"][0]["color"] = "80402080".into();
    v["skins"][0]["attachments"]["cloth"]["triangle"]["color"] = "4080ff80".into();
    let asset = checked(&v).load().unwrap();
    let i = asset.instance().unwrap();
    for vertex in i.geometry(1.0).unwrap().vertices {
        close(vertex.r, 128.0 * 64.0 / (255.0 * 255.0));
        close(vertex.g, 64.0 * 128.0 / (255.0 * 255.0));
        close(vertex.b, 32.0 / 255.0);
        close(vertex.a, 128.0 * 128.0 / (255.0 * 255.0));
    }
    let atlas = format!(
        "{ATLAS}\n{}",
        ATLAS
            .replace("probe.png", "second.png")
            .replace("triangle\n", "second\n")
    );
    let mut spec = json!({"schemaVersion":"spine-skeleton-asset.v1","assetId":"actor","skeletonSource":"assets/actor.json","atlasSource":"assets/actor.atlas","pages":{"probe.png":{"assetId":"t0"},"second.png":{"assetId":"t1"}},"format":"json","exportVersion":"4.2.00","pixelsPerUnit":100});
    let deps = BTreeMap::from([("t0".into(), "a".repeat(64)), ("t1".into(), "b".repeat(64))]);
    let key = engine_spine_runtime::import::validate_declared(&spec.to_string(), PROBE, &atlas)
        .unwrap()
        .cook_key(&deps, "animator.spine.4.2.v1")
        .unwrap();
    spec["pages"]["probe.png"]["assetId"] = "t1".into();
    spec["pages"]["second.png"]["assetId"] = "t0".into();
    let swapped = engine_spine_runtime::import::validate_declared(&spec.to_string(), PROBE, &atlas)
        .unwrap()
        .cook_key(&deps, "animator.spine.4.2.v1")
        .unwrap();
    assert_ne!(
        key, swapped,
        "swapping two atlas page bindings must invalidate Cook"
    );
}

#[test]
fn spine374_b_native_geometry_and_skin_lifecycle_balances() {
    use engine_spine_runtime::live_native_allocations;
    let baseline = live_native_allocations();
    for _ in 0..64 {
        let source = checked(&clipped());
        let a = source.load().unwrap();
        let mut i = a.instance().unwrap();
        for _ in 0..4 {
            i.set_skin("default").unwrap();
            let g = i.geometry(100.0).unwrap();
            assert!(!g.indices.is_empty());
            assert_eq!(i.geometry_with_capacity(1.0, 1, 1), Err(Error::Capacity));
        }
    }
    assert_eq!(
        live_native_allocations(),
        baseline,
        "native clipping/skin resources must release on success and rejected output"
    );
}

fn path_fixture(position: &str, spacing: &str, rotate: &str, constant: bool) -> Value {
    json!({"skeleton":{"spine":"4.2.00"},"bones":[{"name":"root"},{"name":"a","parent":"root","length":2},{"name":"b","parent":"root","length":2}],"slots":[{"name":"path","bone":"root","attachment":"line"},{"name":"image","bone":"a","attachment":"image"}],"path":[{"name":"follow","bones":["a","b"],"target":"path","positionMode":position,"spacingMode":spacing,"rotateMode":rotate,"position":if position=="fixed" {2.0}else{0.2},"spacing":match spacing {"length"=>1.0,"percent"=>0.3,_=>3.0}}],"skins":[{"name":"default","attachments":{"path":{"line":{"type":"path","closed":false,"constantSpeed":constant,"vertexCount":6,"vertices":[-1,0,0,0,3.333333333333333,0,6.666666666666667,0,10,0,11,0],"lengths":[10,20]}},"image":{"image":{"type":"region","path":"triangle","width":2,"height":2}}}}]})
}
#[test]
fn spine374_b_path_position_spacing_rotate_and_lengths_golden() {
    for position in ["fixed", "percent"] {
        for spacing in ["fixed", "percent", "length"] {
            for rotate in ["tangent", "chain", "chainScale"] {
                for constant in [false, true] {
                    let v = path_fixture(position, spacing, rotate, constant);
                    let asset = checked(&v).load().unwrap();
                    let i = asset.instance().unwrap();
                    let a = i.bone("a").unwrap();
                    let b = i.bone("b").unwrap();
                    close(a.x, 2.0);
                    close(a.y, 0.0);
                    close(a.rotation, 0.0);
                    // Chain mode places the next bone at the prior bone's
                    // actual tip (length 2); tangent/chainScale use spacing 3.
                    close(b.x, if rotate == "chain" { 4.0 } else { 5.0 });
                    close(b.y, 0.0);
                    let g = i.geometry(1.0).unwrap();
                    assert_eq!(
                        g.indices.len(),
                        6,
                        "path must not produce drawable triangles"
                    );
                    let bounds = g.bounds.unwrap();
                    close(bounds[0], if rotate == "chainScale" { 0.5 } else { 1.0 });
                    close(bounds[2], if rotate == "chainScale" { 3.5 } else { 3.0 });
                }
            }
        }
    }
    let mut v = path_fixture("fixed", "fixed", "chain", false);
    v["animations"] =
        json!({"move":{"path":{"follow":{"position":[{"value":2},{"time":1,"value":4}]}}}});
    let asset = checked(&v).load().unwrap();
    let mut i = asset.instance().unwrap();
    i.play(0, "move", None, 0.0, false, false).unwrap();
    i.step(0.5).unwrap();
    close(i.bone("a").unwrap().x, 3.0);
    let mut bad = v.clone();
    bad["skins"][0]["attachments"]["path"]["line"]["lengths"] = json!([10]);
    rejected(&bad);
    let mut bad = v.clone();
    bad["path"][0]["spacingMode"] = "unknown".into();
    rejected(&bad);
    let mut bad = v.clone();
    bad["animations"]["move"]["path"]["follow"]["sequence"] = json!([{}]);
    rejected(&bad);
}

#[test]
fn spine374_b_texture_cook_linear_pma_and_dimension_contract() {
    use engine_spine_runtime::import::Page;
    let mut page = Page {
        name: "page".into(),
        texture_asset_id: "texture".into(),
        pma: false,
        width: 2,
        height: 1,
    };
    let straight = page
        .linear_premultiplied_rgba8(2, 1, &[128, 128, 128, 128, 255, 70, 30, 0])
        .unwrap();
    assert_eq!(straight, [28, 28, 28, 128, 0, 0, 0, 0]);
    page.pma = true;
    assert!(
        page.linear_premultiplied_rgba8(2, 1, &[255, 0, 0, 128, 0, 0, 0, 0])
            .is_err()
    );
    let pma = page
        .linear_premultiplied_rgba8(2, 1, &[64, 64, 64, 128, 255, 70, 30, 0])
        .unwrap();
    for (a, b) in straight.iter().zip(pma) {
        assert!(i16::from(*a).abs_diff(i16::from(b)) <= 1);
    }
    assert!(
        page.linear_premultiplied_rgba8(1, 1, &[0, 0, 0, 0])
            .is_err()
    );
    assert!(
        page.linear_premultiplied_rgba8(2, 1, &[0, 0, 0, 0])
            .is_err()
    );
}

#[test]
fn spine374_b_transform_rotation_scale_shear_and_timeline_geometry_golden() {
    let mut v = probe();
    v["bones"] = json!([{"name":"root"},{"name":"upper","parent":"root","x":2,"y":3},{"name":"target","parent":"root","x":10,"y":20,"rotation":90,"scaleX":2,"scaleY":3}]);
    v["animations"] = json!({"blend":{"transform":{"follow":[{"mixRotate":0,"mixX":0,"mixY":0,"mixScaleX":0,"mixScaleY":0,"mixShearY":0},{"time":1,"mixRotate":1,"mixX":1,"mixY":1,"mixScaleX":1,"mixScaleY":1,"mixShearY":1}]}}});
    v["transform"] = json!([{"name":"follow","bones":["upper"],"target":"target"}]);
    let asset = checked(&v).load().unwrap();
    let mut i = asset.instance().unwrap();
    let g = i.geometry(1.0).unwrap();
    for (v, (x, y)) in g
        .vertices
        .iter()
        .zip([(10.0, 20.0), (10.0, 24.0), (4.0, 20.0)])
    {
        close(v.x, x);
        close(v.y, y);
    }
    i.play(0, "blend", None, 0.0, false, false).unwrap();
    i.step(0.5).unwrap();
    let g = i.geometry(1.0).unwrap();
    for (v, (x, y)) in
        g.vertices
            .iter()
            .zip([(6.0, 11.5), (8.12132, 13.62132), (3.171573, 14.328427)])
    {
        close(v.x, x);
        close(v.y, y);
    }
    v["animations"] = json!({});
    v["bones"][2]["rotation"] = 0.into();
    v["bones"][2]["scaleX"] = 1.into();
    v["bones"][2]["scaleY"] = 1.into();
    v["bones"][2]["shearY"] = 30.into();
    let asset = checked(&v).load().unwrap();
    let g = asset.instance().unwrap().geometry(1.0).unwrap();
    close(g.vertices[2].x, 9.0);
    close(g.vertices[2].y, 21.73205);
}
