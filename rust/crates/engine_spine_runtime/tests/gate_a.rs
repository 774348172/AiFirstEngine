use engine_spine_runtime::{
    Asset, Error, EventCursor, EventIndex, Instances, OncePlan, live_native_allocations,
    seconds_to_ticks,
};

const JSON: &str = include_str!("fixtures/probe.json");
const ATLAS: &str = include_str!("fixtures/probe.atlas");

fn asset() -> Asset {
    // SAFETY: project-authored fixture was reviewed against the pinned parser;
    // all bones/slots/skins/attachments/timeline lengths reference valid data.
    unsafe { Asset::from_validated_json(JSON, ATLAS) }.expect("official parser accepts fixture")
}
fn near(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= 0.0001,
        "actual={actual}, expected={expected}"
    );
}

#[test]
fn spine374_a_load_vertices_and_instances_are_independent() {
    let asset = asset();
    assert_eq!(asset.atlas_pages(), 1);
    let (mut a, mut b) = (asset.instance().unwrap(), asset.instance().unwrap());
    a.play(0, "motion", None, 0.0, false, false).unwrap();
    b.play(0, "motion", None, 0.0, false, false).unwrap();
    a.step(0.5).unwrap();
    b.step(0.25).unwrap();
    near(a.bone("lower").unwrap().x, 5.0);
    near(b.bone("lower").unwrap().x, 2.5);
    near(a.bone("upper").unwrap().x, 2.0);
    let vertices = a.vertices("cloth").unwrap();
    for (actual, expected) in vertices.iter().zip([3.0, 10.0, 4.0, 10.0, 2.0, 12.0]) {
        near(*actual, expected);
    }
    assert_eq!(vertices.len(), 6);
    near(b.vertices("cloth").unwrap()[0], 1.5);
}

#[test]
fn spine374_a_reject_version_and_preflight_without_mutation() {
    // SAFETY: only the version is changed; official parser rejects it before
    // reading skeleton contents. The rest remains a reviewed valid fixture.
    let result = unsafe { Asset::from_validated_json(&JSON.replace("4.2.00", "4.1.00"), ATLAS) };
    assert!(matches!(result, Err(Error::Native(message)) if message.contains("does not match")));
    let asset = asset();
    let mut instance = asset.instance().unwrap();
    instance.play(0, "motion", None, 0.0, false, false).unwrap();
    instance.step(0.25).unwrap();
    assert_eq!(
        instance.play(0, "missing", None, 0.0, false, false),
        Err(Error::UnknownName)
    );
    assert_eq!(instance.step(f32::NAN), Err(Error::InvalidInput));
    assert_eq!(
        instance.play(8, "motion", None, 0.0, false, false),
        Err(Error::InvalidInput)
    );
    assert_eq!(
        instance.play(1, "motion", None, 0.0, false, true),
        Err(Error::Unsupported)
    );
    near(instance.clocks(0).unwrap().0, 0.25);
    near(instance.bone("lower").unwrap().x, 2.5);
    assert!(matches!(instance.bone("missing"), Err(Error::UnknownName)));
    assert!(matches!(asset.mask("motion", 0), Err(Error::Unsupported)));
    let mask = asset.mask("motion", 1 << 2).unwrap();
    assert_eq!(
        instance.play(1, "attack", Some(&mask), 0.0, false, false),
        Err(Error::InvalidInput)
    );
}

#[test]
fn spine374_a_mask_owns_curves_deform_and_correct_property_ids() {
    let asset = asset();
    let mask = asset.mask("motion", 1 << 2).unwrap();
    assert_eq!(mask.timeline_count(), 3);
    assert!(!mask.contains_source_property("motion", 0).unwrap());
    for index in 1..=3 {
        assert!(
            mask.contains_source_property("motion", index).unwrap(),
            "timeline {index}"
        );
    }
    assert!(!mask.contains_source_property("motion", 4).unwrap());
    let mut instance = asset.instance().unwrap();
    instance.play(0, "base", None, 0.0, false, false).unwrap();
    instance
        .play(1, "motion", Some(&mask), 0.0, false, false)
        .unwrap();
    drop(mask); // native state still refers to cloned timelines.
    drop(asset); // instance retains canonical data and referenced attachments.
    instance.step(0.5).unwrap();
    near(instance.bone("lower").unwrap().x, 5.0);
    near(instance.bone("upper").unwrap().x, 2.0);
    near(instance.vertices("cloth").unwrap()[0], 3.0);
}

#[test]
fn spine374_a_mask_hold_chain_and_empty_fade_use_official_state() {
    let asset = asset();
    let attack = asset.mask("attack", 1 << 2).unwrap();
    let motion = asset.mask("motion", 1 << 2).unwrap();
    let mut instance = asset.instance().unwrap();
    instance.play(0, "base", None, 0.0, false, false).unwrap();
    instance
        .play(1, "attack", Some(&attack), 0.0, false, false)
        .unwrap();
    instance.step(0.1).unwrap();
    near(instance.bone("upper").unwrap().x, 8.0);
    for (name, mask) in [
        ("motion", &motion),
        ("attack", &attack),
        ("motion", &motion),
    ] {
        instance
            .play(1, name, Some(mask), 0.4, false, false)
            .unwrap();
        instance.step(0.1).unwrap();
        assert!(instance.bone("upper").unwrap().x.is_finite());
    }
    near(instance.bone("lower").unwrap().x, 4.0);
    instance.fade_out(1, 0.2).unwrap();
    instance.step(0.1).unwrap();
    let halfway = instance.bone("upper").unwrap().x;
    assert!(halfway > 0.0 && halfway < 8.0, "half fade {halfway}");
    instance.step(0.2).unwrap();
    near(instance.bone("lower").unwrap().x, 7.0);
    near(instance.bone("upper").unwrap().x, 0.0);
    near(instance.vertices("cloth").unwrap()[0], 0.0);
}

#[test]
fn spine374_a_generations_and_cross_asset_masks_are_rejected() {
    let a = asset();
    let b = asset();
    let mask = a.mask("motion", 1 << 2).unwrap();
    let mut instance = b.instance().unwrap();
    assert_eq!(
        instance.play(0, "motion", Some(&mask), 0.0, false, false),
        Err(Error::WrongAsset)
    );
    let mut registry = Instances::default();
    let first = registry.insert(instance);
    registry.remove(first).unwrap();
    let second = registry.insert(b.instance().unwrap());
    let mut other_registry = Instances::default();
    other_registry.insert(b.instance().unwrap());
    assert!(matches!(
        other_registry.get_mut(second),
        Err(Error::StaleHandle)
    ));
    assert_ne!(first, second);
    assert!(matches!(registry.get_mut(first), Err(Error::StaleHandle)));
    registry
        .get_mut(second)
        .unwrap()
        .play(0, "base", None, 0.0, true, false)
        .unwrap();
}

#[test]
fn spine374_a_source_index_zero_and_multiloop_match_real_track_clock() {
    let asset = asset();
    let index = asset.event_index("motion").unwrap();
    assert_eq!(index.duration_us, 1_000_000);
    assert_eq!(
        index.keys.iter().map(|k| k.time_us).collect::<Vec<_>>(),
        [0, 250_000, 1_000_000]
    );
    let mut cursor = EventCursor::default();
    let mut instance = asset.instance().unwrap();
    instance.play(0, "motion", None, 0.0, true, false).unwrap();
    instance.step(0.0).unwrap();
    assert_eq!(instance.callback_event_count(), 1);
    assert_eq!(cursor.preview(&index, 0, true, 64).unwrap().len(), 1);
    cursor.commit(0).unwrap();
    assert!(cursor.preview(&index, 0, true, 64).unwrap().is_empty());
    instance.step(0.5).unwrap();
    assert_eq!(instance.callback_event_count(), 1);
    assert_eq!(cursor.preview(&index, 500_000, true, 64).unwrap().len(), 1);
    cursor.commit(500_000).unwrap();
    instance.step(2.25).unwrap();
    let (absolute, local) = instance.clocks(0).unwrap();
    near(absolute, 2.75);
    near(local, 0.75);
    let events = cursor
        .preview(
            &index,
            (f64::from(absolute) * 1_000_000.0).round() as u64,
            true,
            64,
        )
        .unwrap();
    assert_eq!(
        events
            .iter()
            .map(|e| (e.absolute_us, e.loop_index, e.ordinal))
            .collect::<Vec<_>>(),
        [
            (1_000_000, 1, 0),
            (1_000_000, 0, 2),
            (1_250_000, 1, 1),
            (2_000_000, 2, 0),
            (2_000_000, 1, 2),
            (2_250_000, 2, 1)
        ]
    );
    assert!(
        instance.callback_event_count() < events.len() as u32,
        "official callback does not enumerate complete crossed loops"
    );
    cursor.commit(2_750_000).unwrap();
    assert!(
        cursor
            .preview(&index, 2_750_000, true, 64)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn spine374_a_event_budget_is_preflight_and_once_plan_is_tick_bound() {
    let index = EventIndex::new(1.0, [(0.0, 0), (0.25, 1), (1.0, 2)]).unwrap();
    let cursor = EventCursor::default();
    assert_eq!(
        cursor.preview(&index, 3_500_000, true, 3),
        Err(Error::Capacity)
    );
    assert_eq!(cursor.preview(&index, 0, true, 3).unwrap().len(), 1);
    assert_eq!(
        cursor.preview(&index, 5_000_000, false, 3).unwrap().len(),
        3
    );
    assert_eq!(seconds_to_ticks(0.08, 60).unwrap(), 5);
    assert_eq!(seconds_to_ticks(0.12, 60).unwrap(), 8);
    assert_eq!(seconds_to_ticks(0.0, 60).unwrap(), 0);
    assert_eq!(seconds_to_ticks(0.14, 100).unwrap(), 14);
    assert_eq!(seconds_to_ticks(0.14000001, 100).unwrap(), 15);
    assert_eq!(seconds_to_ticks(1e-20, 60).unwrap(), 1);
    let plan = OncePlan::new(10, 1.0, 0.08, 60).unwrap();
    assert_eq!((plan.fade_start_tick, plan.retire_tick), (70, 75));
    assert_eq!(
        seconds_to_ticks(f64::INFINITY, 60),
        Err(Error::InvalidInput)
    );
    let tiny = EventIndex::new(0.000001, [(0.0, 0)]).unwrap();
    assert_eq!(
        cursor.preview(&tiny, u64::MAX, true, 64),
        Err(Error::Capacity)
    );
}

#[test]
fn spine374_a_bezier_copy_preserves_official_samples_after_source_drop() {
    // A 2-channel translate curve contains eight control numbers, with constant
    // Y. The asymmetric X curve tests copied Bezier tables, not only endpoints.
    let curved = JSON.replace(
        "\"translate\":[{\"x\":0,\"y\":0},{\"time\":1,\"x\":4,\"y\":0}]",
        "\"translate\":[{\"x\":0,\"y\":0,\"curve\":[0.1,0,0.2,4,0.1,0,0.2,0]},{\"time\":1,\"x\":4,\"y\":0}]"
    );
    assert_ne!(curved, JSON);
    // SAFETY: reviewed fixture with eight valid cubic control values added.
    let a = unsafe { Asset::from_validated_json(&curved, ATLAS) }.unwrap();
    let m = a.mask("motion", 1 << 2).unwrap();
    let (mut original, mut masked) = (a.instance().unwrap(), a.instance().unwrap());
    original.play(0, "motion", None, 0.0, false, false).unwrap();
    masked
        .play(0, "motion", Some(&m), 0.0, false, false)
        .unwrap();
    drop(m);
    drop(a);
    for _ in 0..9 {
        original.step(0.1).unwrap();
        masked.step(0.1).unwrap();
        near(
            masked.bone("upper").unwrap().x,
            original.bone("upper").unwrap().x,
        );
        near(
            masked.vertices("cloth").unwrap()[0],
            original.vertices("cloth").unwrap()[0],
        );
        near(masked.bone("lower").unwrap().x, 0.0);
    }
}

#[test]
fn spine374_a_continuous_additive_and_compiled_once_fade_are_feasible() {
    let a = asset();
    let mut i = a.instance().unwrap();
    i.play(0, "base", None, 0.0, false, false).unwrap();
    i.play(1, "base", None, 0.0, false, true).unwrap();
    i.step(0.5).unwrap();
    near(i.bone("lower").unwrap().x, 10.0);
    let mask = a.mask("attack", 1 << 2).unwrap();
    let mut once = a.instance().unwrap();
    let plan = OncePlan::new(0, 1.0, 0.08, 60).unwrap();
    once.play(0, "base", None, 0.0, false, false).unwrap();
    once.play(1, "attack", Some(&mask), 0.0, false, false)
        .unwrap();
    for _ in 0..plan.fade_start_tick {
        once.step(1.0 / 60.0).unwrap();
    }
    near(once.bone("upper").unwrap().x, 8.0);
    once.fade_out(1, 5.0 / 60.0).unwrap();
    for _ in plan.fade_start_tick..=plan.retire_tick {
        once.step(1.0 / 60.0).unwrap();
    }
    near(once.bone("upper").unwrap().x, 0.0);
}

#[test]
fn spine374_a_frozen_recipe_inputs_load_in_the_official_runtime() {
    for json in [
        include_str!("fixtures/recipe-single.json"),
        include_str!("fixtures/recipe-regular.json"),
        include_str!("fixtures/recipe-clipping.json"),
    ] {
        // SAFETY: generated project-authored grids have bounded parent-first
        // bones, 2 influences/weighted vertex, valid indices and deform lengths.
        let a = unsafe { Asset::from_validated_json(json, ATLAS) }.unwrap();
        let m = a.mask("clip0", 1 << 2).unwrap();
        let (mut original, mut masked) = (a.instance().unwrap(), a.instance().unwrap());
        original.play(0, "clip0", None, 0.0, false, false).unwrap();
        masked
            .play(0, "clip0", Some(&m), 0.0, false, false)
            .unwrap();
        original.step(0.5).unwrap();
        masked.step(0.5).unwrap();
        let expected = original.vertices("mesh1").unwrap();
        let actual = masked.vertices("mesh1").unwrap();
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert!(actual.is_finite());
            near(*actual, expected);
        }
        assert_eq!(a.event_index("clip0").unwrap().keys.len(), 4);
    }
}

#[test]
fn spine374_a_repeated_mask_and_state_destruction_balance_official_allocator() {
    // Run this binary with --test-threads=1: process-wide allocation counters must
    // not race unrelated tests. Warm the official static empty animation first.
    {
        let a = asset();
        let mut i = a.instance().unwrap();
        i.fade_out(0, 0.0).unwrap();
    }
    let baseline = live_native_allocations();
    for _ in 0..256 {
        let a = asset();
        let m = a.mask("motion", 1 << 2).unwrap();
        let mut i = a.instance().unwrap();
        i.play(0, "motion", Some(&m), 0.0, false, false).unwrap();
        i.step(0.5).unwrap();
        i.play(0, "attack", None, 0.2, false, false).unwrap();
        i.step(0.1).unwrap();
    }
    assert_eq!(
        live_native_allocations(),
        baseline,
        "official allocator leaked timeline/state/data memory"
    );
}
