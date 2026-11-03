use engine_spine_runtime::{
    EntrySample, Error,
    import::{ValidatedSource, validate},
    live_native_allocations,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
const ATLAS: &str = include_str!("fixtures/probe.atlas");
fn probe() -> Value {
    serde_json::from_str(include_str!("fixtures/probe.json")).unwrap()
}
fn checked(v: &Value) -> ValidatedSource {
    validate(
        &v.to_string(),
        ATLAS,
        &BTreeMap::from([("probe.png".into(), "texture".into())]),
    )
    .unwrap()
}
fn close(a: f32, b: f32) {
    assert!((a - b).abs() <= 1e-4_f32.max(2e-5 * b.abs()), "{a} != {b}");
}
fn compare(v: &Value, clip: &str, names: &[&str], bones: &[&str]) {
    let s = checked(v);
    let asset = s.load().unwrap();
    let mask = asset.bind_mask(&s.mask_plan(clip, names).unwrap()).unwrap();
    let mut full = asset.instance().unwrap();
    let mut filtered = asset.instance().unwrap();
    full.play(0, clip, None, 0.0, false, false).unwrap();
    filtered
        .play(0, clip, Some(&mask), 0.0, false, false)
        .unwrap();
    for dt in [0.0, 0.125, 0.375, 0.5] {
        full.step(dt).unwrap();
        filtered.step(dt).unwrap();
        for name in bones {
            let a = full.bone(name).unwrap();
            let b = filtered.bone(name).unwrap();
            close(a.x, b.x);
            close(a.y, b.y);
            close(a.rotation, b.rotation);
        }
        let a = full.geometry(1.0).unwrap();
        let b = filtered.geometry(1.0).unwrap();
        assert_eq!(a.indices, b.indices);
        assert_eq!(a.batches, b.batches);
        for (a, b) in a.vertices.iter().zip(&b.vertices) {
            for (a, b) in [
                (a.x, b.x),
                (a.y, b.y),
                (a.u, b.u),
                (a.v, b.v),
                (a.r, b.r),
                (a.g, b.g),
                (a.b, b.b),
                (a.a, b.a),
            ] {
                close(a, b);
            }
        }
    }
}
#[test]
fn spine374_c_real_full_body_filter_and_canonical_deform_ids() {
    let s = checked(&probe());
    let asset = s.load().unwrap();
    let mask = asset
        .bind_mask(&s.mask_plan("motion", &["upper"]).unwrap())
        .unwrap();
    assert_eq!(mask.timeline_count(), 3);
    let mut i = asset.instance().unwrap();
    i.play(0, "motion", Some(&mask), 0.0, false, false).unwrap();
    i.step(0.5).unwrap();
    close(i.bone("lower").unwrap().x, 0.0);
    close(i.bone("upper").unwrap().x, 2.0);
    close(i.vertices("cloth").unwrap()[0], 3.0);
    let mut different = probe();
    different["bones"][2]["y"] = 11.into();
    let other = checked(&different).load().unwrap();
    assert!(matches!(
        other.bind_mask(&s.mask_plan("motion", &["upper"]).unwrap()),
        Err(Error::WrongAsset)
    ));
    assert_eq!(
        s.mask_plan("motion", &[]).unwrap_err().code,
        "animator.mask.empty_selection"
    );
    assert_eq!(
        s.mask_plan("missing", &["upper"]).unwrap_err().code,
        "animator.binding.unknown_action"
    );
}
#[test]
fn spine374_c_256_bones_and_complete_continuous_timeline_clones() {
    let mut v = probe();
    v["animations"] = json!({"all":{"bones":{"upper":{}}}});
    for (kind, last) in [
        ("rotate", json!({"value":30})),
        ("translate", json!({"x":2,"y":3})),
        ("translatex", json!({"value":2})),
        ("translatey", json!({"value":3})),
        ("scale", json!({"x":2,"y":3})),
        ("scalex", json!({"value":2})),
        ("scaley", json!({"value":3})),
        ("shear", json!({"x":10,"y":15})),
        ("shearx", json!({"value":10})),
        ("sheary", json!({"value":15})),
    ] {
        let mut end = last;
        end["time"] = 1.into();
        v["animations"]["all"]["bones"]["upper"] = json!({kind:[{},end]});
        compare(&v, "all", &["upper"], &["upper"]);
    }
    v["bones"] = json!([{"name":"root"}]);
    for n in 1..256 {
        v["bones"].as_array_mut().unwrap().push(
            json!({"name":if n==255 {"upper".into()} else {format!("b{n}")},"parent":"root"}),
        );
    }
    v["animations"] = json!({"all":{"bones":{"upper":{"translate":[{}, {"time":1,"x":10}]}}}});
    let s = checked(&v);
    let a = s.load().unwrap();
    let m = a
        .bind_mask(&s.mask_plan("all", &["upper"]).unwrap())
        .unwrap();
    let mut i = a.instance().unwrap();
    i.play(0, "all", Some(&m), 0.0, false, false).unwrap();
    i.step(0.5).unwrap();
    close(i.bone("upper").unwrap().x, 5.0);
}
#[test]
fn spine374_c_slot_color_attachment_and_draw_order_clones() {
    for kind in ["rgba", "rgb", "alpha"] {
        let mut v = probe();
        let frames = match kind {
            "rgba" => json!([{"color":"ffffffff"},{"time":1,"color":"80402080"}]),
            "rgb" => json!([{"color":"ffffff"},{"time":1,"color":"804020"}]),
            _ => json!([{"value":1},{"time":1,"value":0.25}]),
        };
        v["animations"] = json!({"all":{"slots":{"cloth":{kind:frames}}}});
        compare(&v, "all", &["upper"], &["upper"]);
    }
    let mut v = probe();
    v["skins"][0]["attachments"]["cloth"]["other"] =
        json!({"type":"region","path":"triangle","width":4,"height":4});
    v["animations"] = json!({"all":{"slots":{"cloth":{"attachment":[{"name":"triangle"},{"time":0.5,"name":"other"}]}}}});
    compare(&v, "all", &["upper"], &["upper"]);
    v["slots"] = json!([{"name":"cloth","bone":"upper","attachment":"triangle"},{"name":"other","bone":"lower","attachment":"triangle"}]);
    v["skins"][0]["attachments"]["other"] = v["skins"][0]["attachments"]["cloth"].clone();
    v["animations"] =
        json!({"all":{"drawOrder":[{"offsets":[{"slot":"cloth","offset":1}]},{"time":1}]}});
    let s = checked(&v);
    assert_eq!(
        s.mask_plan("all", &["upper"]).unwrap_err().code,
        "animator.mask.dependency_conflict"
    );
    compare(&v, "all", &["upper", "lower"], &["upper", "lower"]);
}
#[test]
fn spine374_c_constraint_clones_and_dependency_rejection() {
    let mut v = probe();
    v["bones"] = json!([{"name":"root"},{"name":"upper","parent":"root","length":5},{"name":"target","parent":"root","x":6,"y":8}]);
    v["ik"] = json!([{"name":"reach","bones":["upper"],"target":"target"}]);
    v["animations"] = json!({"all":{"ik":{"reach":[{"mix":0},{"time":1,"mix":1}]}}});
    let s = checked(&v);
    assert_eq!(
        s.mask_plan("all", &["upper"]).unwrap_err().code,
        "animator.mask.dependency_conflict"
    );
    compare(&v, "all", &["upper", "target"], &["upper"]);
    v.as_object_mut().unwrap().remove("ik");
    v["transform"] = json!([{"name":"follow","bones":["upper"],"target":"target"}]);
    v["animations"] = json!({"all":{"transform":{"follow":[{"mixRotate":0,"mixX":0,"mixY":0},{"time":1,"mixRotate":1,"mixX":1,"mixY":1}]}}});
    let s = checked(&v);
    assert!(s.mask_plan("all", &["target"]).is_err());
    compare(&v, "all", &["upper", "target"], &["upper"]);
    let mut v = json!({"skeleton":{"spine":"4.2.00"},"bones":[{"name":"root"},{"name":"upper","parent":"root","length":2}],"slots":[{"name":"path","bone":"root","attachment":"line"},{"name":"cloth","bone":"upper","attachment":"triangle"}],"path":[{"name":"follow","bones":["upper"],"target":"path","positionMode":"fixed","spacingMode":"fixed","rotateMode":"chain"}],"skins":[{"name":"default","attachments":{"path":{"line":{"type":"path","closed":false,"constantSpeed":false,"vertexCount":6,"vertices":[-1,0,0,0,3.333333333,0,6.666666667,0,10,0,11,0],"lengths":[10,20]}},"cloth":{"triangle":{"type":"region","path":"triangle","width":2,"height":2}}}}]});
    for (kind, frames) in [
        ("position", json!([{"value":2},{"time":1,"value":4}])),
        ("spacing", json!([{"value":2},{"time":1,"value":4}])),
        (
            "mix",
            json!([{"mixRotate":0,"mixX":0,"mixY":0},{"time":1,"mixRotate":1,"mixX":1,"mixY":1}]),
        ),
    ] {
        v["animations"] = json!({"all":{"path":{"follow":{kind:frames}}}});
        let s = checked(&v);
        assert!(s.mask_plan("all", &["upper"]).is_err());
        compare(&v, "all", &["root", "upper"], &["upper"]);
    }
}
#[test]
fn spine374_c_parent_weighted_and_clipping_cross_selection_rejected() {
    let mut v = probe();
    v["animations"]["motion"]["bones"]["root"] = json!({"translate":[{}, {"time":1,"x":2}]});
    let s = checked(&v);
    assert!(s.mask_plan("motion", &["root"]).is_err());
    assert!(s.mask_plan("motion", &["upper"]).is_ok());
    let mut v = probe();
    v["skins"][0]["attachments"]["cloth"]["triangle"]["vertices"] = json!([
        2, 0, 0, 0, 0.5, 2, 0, 0, 0.5, 2, 0, 2, 0, 0.5, 2, 2, 0, 0.5, 2, 0, 0, 2, 0.5, 2, 0, 2, 0.5
    ]);
    let s = checked(&v);
    assert!(s.mask_plan("motion", &["upper"]).is_err());
    let mut v = probe();
    v["slots"]
        .as_array_mut()
        .unwrap()
        .insert(0, json!({"name":"clip","bone":"lower","attachment":"clip"}));
    v["skins"][0]["attachments"]["clip"] = json!({"clip":{"type":"clipping","end":"cloth","vertexCount":4,"vertices":[-1,-1,-1,1,1,1,1,-1]}});
    let s = checked(&v);
    assert!(s.mask_plan("motion", &["upper"]).is_err());
}
#[test]
fn spine374_c_animated_ancestor_of_excluded_constraint_target_is_a_conflict() {
    let v = json!({"skeleton":{"spine":"4.2.00"},"bones":[{"name":"root"},{"name":"driver","parent":"root"},{"name":"target","parent":"driver","x":5,"y":5},{"name":"a","parent":"root","length":5}],"ik":[{"name":"reach","bones":["a"],"target":"target"}],"animations":{"move":{"bones":{"driver":{"translate":[{}, {"time":1,"x":5}]}}}}});
    let s = checked(&v);
    assert_eq!(
        s.mask_plan("move", &["driver"]).unwrap_err().code,
        "animator.mask.dependency_conflict"
    );
}
#[test]
fn spine374_c_repeated_absolute_samples_do_not_accumulate_unkeyed_additive_properties() {
    let mut v = probe();
    v["animations"]["upper_add"] = json!({"bones":{"upper":{"translate":[{}, {"time":1,"x":8}]}}});
    let a = checked(&v).load().unwrap();
    let mut i = a.instance().unwrap();
    i.play(0, "base", None, 0.0, true, false).unwrap();
    i.tag_current(0, 1, 1.0).unwrap();
    i.play(1, "upper_add", None, 0.0, true, true).unwrap();
    i.tag_current(1, 2, 1.0).unwrap();
    let samples = [
        EntrySample {
            id: 1,
            phase: 0.5,
            mix: 0.0,
        },
        EntrySample {
            id: 2,
            phase: 0.5,
            mix: 0.0,
        },
    ];
    i.sample(&samples).unwrap();
    close(i.bone("upper").unwrap().x, 4.0);
    i.sample(&samples).unwrap();
    close(i.bone("upper").unwrap().x, 4.0);
}
#[test]
fn spine374_c_weighted_linked_mask_preserves_canonical_deform_and_excluded_slot_is_untouched() {
    let mut v = probe();
    v["skins"][0]["attachments"]["cloth"]["triangle"]["vertices"] =
        json!([1, 2, 0, 0, 1, 1, 2, 2, 0, 1, 1, 2, 0, 2, 1]);
    v["skins"][0]["attachments"]["cloth"]["linked"] =
        json!({"type":"linkedmesh","path":"triangle","parent":"triangle","timelines":true});
    v["slots"][0]["attachment"] = "linked".into();
    compare(&v, "motion", &["upper"], &["upper"]);
    let s = checked(&v);
    let a = s.load().unwrap();
    let mask = a
        .bind_mask(&s.mask_plan("motion", &["lower"]).unwrap())
        .unwrap();
    let mut i = a.instance().unwrap();
    i.play(0, "motion", Some(&mask), 0.0, false, false).unwrap();
    i.step(0.5).unwrap();
    close(i.bone("lower").unwrap().x, 5.0);
    close(i.bone("upper").unwrap().x, 0.0);
    close(i.vertices("cloth").unwrap()[0], 0.0);
    assert_eq!(mask.timeline_count(), 1);
}
#[test]
fn spine374_c_controlled_sampling_is_bounded_and_owned_clones_release() {
    let baseline = live_native_allocations();
    for _ in 0..30 {
        let s = checked(&probe());
        let a = s.load().unwrap();
        let m = a
            .bind_mask(&s.mask_plan("motion", &["upper"]).unwrap())
            .unwrap();
        let mut i = a.instance().unwrap();
        i.play(0, "motion", Some(&m), 0.0, true, false).unwrap();
        i.tag_current(0, 1, 1.0).unwrap();
        i.sample(&[EntrySample {
            id: 1,
            phase: 0.5,
            mix: 0.0,
        }])
        .unwrap();
        close(i.bone("upper").unwrap().x, 2.0);
        assert_eq!(i.live_entries().unwrap(), [1]);
        assert_eq!(i.sample(&[]), Err(Error::InvalidInput));
        i.clear_track(0).unwrap();
        close(i.bone("upper").unwrap().x, 0.0);
    }
    assert_eq!(live_native_allocations(), baseline);
}
#[test]
fn spine374_c_mask_property_hold_and_interrupted_mixing_match_official_filtered_reference() {
    let v = probe();
    let mut reference = v.clone();
    for (_, clip) in reference["animations"].as_object_mut().unwrap() {
        if let Some(bones) = clip["bones"].as_object_mut() {
            bones.remove("lower");
        }
    }
    let s = checked(&v);
    let a = s.load().unwrap();
    let r = checked(&reference).load().unwrap();
    let mut i = a.instance().unwrap();
    let mut expected = r.instance().unwrap();
    for clip in ["attack", "motion", "rest", "attack"] {
        let m = a
            .bind_mask(&s.mask_plan(clip, &["upper"]).unwrap())
            .unwrap();
        i.play(1, clip, Some(&m), 0.4, true, false).unwrap();
        expected.play(1, clip, None, 0.4, true, false).unwrap();
        for dt in [0.0, 0.05, 0.05] {
            i.step(dt).unwrap();
            expected.step(dt).unwrap();
            let actual = i.bone("upper").unwrap();
            let e = expected.bone("upper").unwrap();
            close(actual.x, e.x);
            close(actual.rotation, e.rotation);
            for (a, b) in i
                .vertices("cloth")
                .unwrap()
                .into_iter()
                .zip(expected.vertices("cloth").unwrap())
            {
                close(a, b);
            }
        }
    }
    for dt in [0.1, 0.2, 0.4] {
        i.step(dt).unwrap();
        expected.step(dt).unwrap();
        close(
            i.bone("upper").unwrap().x,
            expected.bone("upper").unwrap().x,
        );
    }
}
