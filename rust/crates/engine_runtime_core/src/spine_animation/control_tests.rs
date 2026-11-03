use super::*;
use engine_spine_runtime::import::validate;
use serde_json::{Value, json};
use std::{collections::BTreeMap, rc::Rc};
const ATLAS: &str = include_str!("../../../engine_spine_runtime/tests/fixtures/probe.atlas");
fn source() -> Value {
    serde_json::from_str(include_str!(
        "../../../engine_spine_runtime/tests/fixtures/probe.json"
    ))
    .unwrap()
}
fn checked(v: &Value) -> ValidatedSource {
    validate(
        &v.to_string(),
        ATLAS,
        &BTreeMap::from([("probe.png".into(), "texture".into())]),
    )
    .unwrap()
}
fn action(s: &ValidatedSource, name: &str, layer: u8) -> Action {
    Action {
        clip: s
            .animation_data
            .iter()
            .position(|c| c.name == name)
            .unwrap() as u16,
        layer,
        looping: true,
        speed_permille: 1000,
        mix_in_ticks: 0,
        fade_out_ticks: None,
        mask: None,
    }
}
fn definition(actions: Vec<Action>) -> Definition {
    Definition {
        tick_rate: 100,
        layers: vec![
            Layer {
                additive: false,
                weight_permille: 1000
            };
            2
        ],
        actions,
        masks: vec![],
        parameters: vec![],
        rules: vec![],
        default_action: 0,
        transitions: vec![],
        sockets: vec![],
        pixels_per_unit: 100.0,
        source_asset_id: "actor".into(),
    }
}
fn controller(s: &ValidatedSource, d: Definition) -> Controller {
    Rc::new(ControlTable::compile(s, d).unwrap())
        .instance()
        .unwrap()
}
fn close(a: f32, b: f32) {
    assert!((a - b).abs() <= 1e-4_f32.max(2e-5 * b.abs()), "{a} != {b}");
}

#[test]
fn spine374_c_integer_tick_long_clock_pause_and_idempotent_restart() {
    let s = checked(&source());
    let mut c = controller(&s, definition(vec![action(&s, "base", 0)]));
    c.advance(0, &[]).unwrap();
    c.advance(25, &[]).unwrap();
    close(c.bone("lower").unwrap().x, 2.5);
    let id = c.state.layers[0][0].id;
    c.advance(25, &[Intent::Play(0)]).unwrap();
    assert_eq!(c.state.layers[0][0].id, id);
    c.advance(30, &[Intent::SetPaused(true)]).unwrap();
    close(c.bone("lower").unwrap().x, 2.5);
    c.advance(1_000_000_030, &[]).unwrap();
    close(c.bone("lower").unwrap().x, 2.5);
    c.advance(1_000_000_031, &[Intent::SetPaused(false)])
        .unwrap();
    c.advance(1_000_000_056, &[]).unwrap();
    close(c.bone("lower").unwrap().x, 5.0);
    c.advance(1_000_000_056, &[Intent::Restart(0)]).unwrap();
    close(c.bone("lower").unwrap().x, 0.0);
    c.advance(8_000_000_081, &[]).unwrap();
    close(c.bone("lower").unwrap().x, 2.5);
    let clocks = c.backend.clocks(0).unwrap();
    assert!(clocks.0 <= 1.0 && clocks.1 <= 1.0);
}
#[test]
fn spine374_c_real_mask_overlay_clear_immediately_restores_base() {
    let s = checked(&source());
    let a = action(&s, "base", 0);
    let mut b = action(&s, "attack", 1);
    b.mask = Some(0);
    let mut d = definition(vec![a, b]);
    d.masks = vec![vec!["upper".into()]];
    let mut c = controller(&s, d);
    c.advance(0, &[]).unwrap();
    c.advance(50, &[Intent::Play(1)]).unwrap();
    close(c.bone("lower").unwrap().x, 5.0);
    close(c.bone("upper").unwrap().x, 8.0);
    c.advance(50, &[Intent::ClearLayer(1)]).unwrap();
    close(c.bone("upper").unwrap().x, 0.0);
    close(c.bone("lower").unwrap().x, 5.0);
}
#[test]
fn spine374_c_crossfade_and_three_interruptions_capacity_is_atomic() {
    let s = checked(&source());
    let mut acts = vec![
        action(&s, "base", 0),
        action(&s, "attack", 0),
        action(&s, "motion", 0),
        action(&s, "rest", 0),
    ];
    for a in &mut acts {
        a.mix_in_ticks = 100;
    }
    let mut c = controller(&s, definition(acts));
    c.advance(0, &[]).unwrap();
    c.advance(10, &[Intent::Play(1)]).unwrap();
    c.advance(60, &[]).unwrap();
    close(c.bone("lower").unwrap().x, 53.0);
    c.advance(61, &[Intent::Play(2)]).unwrap();
    c.advance(62, &[Intent::Play(3)]).unwrap();
    assert_eq!(c.live_entry_count(0), 4);
    let before = c.pose().clone();
    let logical = c.logical_tick();
    assert_eq!(
        c.advance(63, &[Intent::SetPaused(true), Intent::Restart(0)])
            .unwrap_err(),
        CHAIN
    );
    assert_eq!(c.pose().revision, before.revision);
    assert_eq!(c.logical_tick(), logical);
    assert!(!c.state.paused);
    c.advance(162, &[]).unwrap();
    close(c.bone("lower").unwrap().x, 0.0);
    c.advance(163, &[]).unwrap();
    assert!(c.live_entry_count(0) < 4);
}
#[test]
fn spine374_c_once_speed_ceil_empty_fade_and_interruption_cancels_plan() {
    let s = checked(&source());
    let mut once = action(&s, "attack", 1);
    once.looping = false;
    once.speed_permille = 3000;
    once.fade_out_ticks = Some(10);
    let mut c = controller(&s, definition(vec![action(&s, "base", 0), once]));
    c.advance(0, &[Intent::Play(1)]).unwrap();
    c.advance(33, &[]).unwrap();
    close(c.bone("upper").unwrap().x, 8.0);
    c.advance(34, &[]).unwrap();
    close(c.bone("upper").unwrap().x, 8.0);
    c.advance(39, &[]).unwrap();
    close(c.bone("upper").unwrap().x, 4.0);
    c.advance(44, &[]).unwrap();
    close(c.bone("upper").unwrap().x, 0.0);
    assert_eq!(c.live_entry_count(1), 0);
    c.advance(45, &[Intent::Play(1)]).unwrap();
    c.advance(60, &[Intent::Restart(1)]).unwrap();
    c.advance(79, &[]).unwrap();
    close(c.bone("upper").unwrap().x, 8.0);
    assert!(c.state.layers[1].last().unwrap().action.is_some());
}
#[test]
fn spine374_c_events_zero_multi_loop_payload_and_pause() {
    let mut v = source();
    v["animations"]["motion"]["events"][1]["int"] = 7.into();
    v["animations"]["motion"]["events"][1]["string"] = "payload".into();
    let s = checked(&v);
    let mut c = controller(&s, definition(vec![action(&s, "motion", 0)]));
    assert_eq!(c.advance(0, &[]).unwrap().events.len(), 1);
    assert!(c.advance(0, &[]).unwrap().events.is_empty());
    let e = c.advance(225, &[]).unwrap().events.clone();
    assert_eq!(e.len(), 7);
    assert_eq!(
        e.iter()
            .map(|e| (e.loop_index, e.source_ordinal))
            .collect::<Vec<_>>(),
        [(0, 1), (1, 0), (0, 2), (1, 1), (2, 0), (1, 2), (2, 1)]
    );
    assert_eq!(e[0].payload.int_value, 7);
    assert_eq!(e[0].payload.string_value, "payload");
    c.advance(226, &[Intent::SetPaused(true)]).unwrap();
    assert!(c.advance(900, &[]).unwrap().events.is_empty());
}
#[test]
fn spine374_c_event_budget_and_unknown_batch_leave_commit_and_cursor_unchanged() {
    let s = checked(&source());
    let mut c = controller(&s, definition(vec![action(&s, "motion", 0)]));
    c.advance(0, &[]).unwrap();
    let rev = c.pose().revision;
    assert_eq!(c.advance(2500, &[]).unwrap_err(), EVENT);
    assert_eq!(c.pose().revision, rev);
    assert_eq!(c.logical_tick(), 0);
    assert_eq!(
        c.advance(25, &[Intent::SetPaused(true), Intent::Play(99)])
            .unwrap_err(),
        ACTION
    );
    assert!(!c.state.paused);
    let e = c.advance(25, &[]).unwrap().events.clone();
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].payload.name, "beat");
}
#[test]
fn spine374_c_weight_zero_outgoing_clear_and_same_clip_multiple_tracks() {
    let s = checked(&source());
    let mut a = action(&s, "motion", 1);
    a.mix_in_ticks = 10;
    let mut d = definition(vec![action(&s, "motion", 0), a]);
    d.layers[1].weight_permille = 0;
    let mut c = controller(&s, d);
    c.advance(0, &[Intent::Play(1)]).unwrap();
    assert_eq!(c.advance(25, &[]).unwrap().events.len(), 1);
    c.advance(26, &[Intent::ClearLayer(1)]).unwrap();
    let mut d = definition(vec![
        action(&s, "motion", 0),
        action(&s, "motion", 1),
        action(&s, "attack", 0),
    ]);
    d.actions[2].mix_in_ticks = 10;
    let mut c = controller(&s, d);
    assert_eq!(c.advance(0, &[Intent::Play(1)]).unwrap().events.len(), 2);
    c.advance(10, &[Intent::Play(2)]).unwrap();
    let e = c.advance(25, &[]).unwrap().events.clone();
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].track, 1);
}
#[test]
fn spine374_c_compact_bool_trigger_rules_and_resume() {
    let s = checked(&source());
    let mut d = definition(vec![action(&s, "base", 0), action(&s, "attack", 0)]);
    d.parameters = vec![Parameter::Bool(false), Parameter::Trigger];
    d.rules = vec![Rule {
        all: vec![(0, true), (1, true)],
        action: 1,
    }];
    let mut c = controller(&s, d);
    c.advance(0, &[]).unwrap();
    c.advance(1, &[Intent::SetBool(0, true), Intent::SetTrigger(1)])
        .unwrap();
    close(c.bone("lower").unwrap().x, 100.0);
    assert!(!c.state.values[1]);
    c.advance(2, &[Intent::Play(1)]).unwrap();
    c.advance(3, &[]).unwrap();
    close(c.bone("lower").unwrap().x, 100.0);
    c.advance(4, &[Intent::Resume]).unwrap();
    close(c.bone("lower").unwrap().x, 0.0);
}
#[test]
fn spine374_c_additive_admission_and_official_translation() {
    let s = checked(&source());
    let mut d = definition(vec![action(&s, "base", 0), action(&s, "base", 1)]);
    d.layers[1].additive = true;
    let mut c = controller(&s, d);
    c.advance(0, &[Intent::Play(1)]).unwrap();
    c.advance(50, &[]).unwrap();
    close(c.bone("lower").unwrap().x, 10.0);
    let mut d = definition(vec![action(&s, "base", 0), action(&s, "motion", 1)]);
    d.layers[1].additive = true;
    assert_eq!(
        ControlTable::compile(&s, d).err(),
        Some(ControlError("animator.mix.unsupported_additive"))
    );
}

#[test]
fn spine374_c_finite_backend_failure_isolates_and_preserves_last_pose_and_events() {
    let mut v = source();
    v["bones"] = json!([{"name":"root"},{"name":"a","parent":"root"},{"name":"b","parent":"a"},{"name":"c","parent":"b"},{"name":"upper","parent":"c"}]);
    v["animations"] =
        json!({"overflow":{"bones":{},"events":[{"name":"zero"},{"time":1,"name":"end"}]}});
    for name in ["root", "a", "b", "c", "upper"] {
        v["animations"]["overflow"]["bones"][name] =
            json!({"scale":[{"x":1,"y":1},{"time":1,"x":1000000000.0,"y":1000000000.0}]});
    }
    let s = checked(&v);
    let mut a = action(&s, "overflow", 0);
    a.looping = false;
    let mut d = definition(vec![a]);
    d.sockets = vec![Socket {
        bone: 4,
        offset: [0.1, 0.2, 0.0],
    }];
    let mut c = controller(&s, d);
    c.advance(0, &[]).unwrap();
    let before = c.pose().clone();
    assert_eq!(c.advance(100, &[]).unwrap_err(), BACKEND);
    assert!(c.isolated);
    assert_eq!(c.pose().revision, before.revision);
    assert_eq!(c.pose().tick, before.tick);
    assert_eq!(c.pose().events.len(), 1);
    assert_eq!(c.pose().events[0].payload.name, "zero");
    assert_eq!(c.advance(101, &[]).unwrap_err(), BACKEND);
    close(c.bone("upper").unwrap().x, before.bones[4].x);
    close(c.socket(0).unwrap().x, before.sockets[0].x);
    close(c.socket(0).unwrap().y, before.sockets[0].y);
}
#[test]
fn spine374_c_once_end_event_zero_speed_and_base_return() {
    let s = checked(&source());
    let mut once = action(&s, "motion", 1);
    once.looping = false;
    once.fade_out_ticks = Some(10);
    let mut c = controller(&s, definition(vec![action(&s, "base", 0), once]));
    c.advance(0, &[Intent::Play(1)]).unwrap();
    let events = c.advance(100, &[]).unwrap().events.clone();
    assert_eq!(
        events
            .iter()
            .map(|e| e.payload.name.as_str())
            .collect::<Vec<_>>(),
        ["beat", "end"]
    );
    c.advance(110, &[]).unwrap();
    c.advance(111, &[]).unwrap();
    assert_eq!(c.live_entry_count(1), 0);
    let mut once = once;
    once.speed_permille = 0;
    let mut c = controller(&s, definition(vec![action(&s, "base", 0), once]));
    c.advance(0, &[Intent::Play(1)]).unwrap();
    c.advance(10000, &[]).unwrap();
    assert_eq!(c.live_entry_count(1), 1);
    close(c.bone("upper").unwrap().x, 0.0);
    let mut once = action(&s, "attack", 0);
    once.looping = false;
    let mut c = controller(&s, definition(vec![action(&s, "base", 0), once]));
    c.advance(0, &[Intent::Play(1)]).unwrap();
    c.advance(100, &[]).unwrap();
    close(c.bone("lower").unwrap().x, 0.0);
}
#[test]
fn spine374_c_action_pair_mix_override_and_default_once_does_not_restart() {
    let s = checked(&source());
    let mut d = definition(vec![action(&s, "base", 0), action(&s, "attack", 0)]);
    d.transitions = vec![Transition {
        from: 0,
        to: 1,
        mix_ticks: 20,
    }];
    let mut c = controller(&s, d);
    c.advance(0, &[]).unwrap();
    c.advance(10, &[Intent::Play(1)]).unwrap();
    c.advance(20, &[]).unwrap();
    close(c.bone("lower").unwrap().x, 51.0);
    let mut once = action(&s, "attack", 0);
    once.looping = false;
    once.fade_out_ticks = Some(10);
    let mut c = controller(&s, definition(vec![once]));
    c.advance(0, &[]).unwrap();
    c.advance(100, &[]).unwrap();
    c.advance(110, &[]).unwrap();
    c.advance(120, &[]).unwrap();
    assert_eq!(c.live_entry_count(0), 0);
}
#[test]
fn spine374_c_socket_units_nonuniform_scale_rotation_and_invalid_binding() {
    let mut v = source();
    v["bones"][2]["rotation"] = 90.into();
    v["bones"][2]["scaleX"] = 2.into();
    v["bones"][2]["scaleY"] = 3.into();
    let s = checked(&v);
    let mut d = definition(vec![action(&s, "base", 0)]);
    d.sockets = vec![Socket {
        bone: 2,
        offset: [0.1, 0.2, 90.0],
    }];
    let mut c = controller(&s, d);
    c.advance(0, &[]).unwrap();
    let p = c.socket(0).unwrap();
    close(p.x, -0.6);
    close(p.y, 0.3);
    assert!((p.rotation.abs() - 180.0).abs() < 0.001);
    let rev = c.pose().revision;
    assert_eq!(
        c.advance(1, &[Intent::ClearLayer(99)]).unwrap_err(),
        INVALID
    );
    assert_eq!(c.pose().revision, rev);
    close(c.socket(0).unwrap().x, -0.6);
    let mut d = definition(vec![action(&s, "base", 0)]);
    d.sockets = vec![Socket {
        bone: 99,
        offset: [0.0; 3],
    }];
    assert_eq!(
        ControlTable::compile(&s, d).err(),
        Some(ControlError("animator.binding.unknown_socket"))
    );
}
#[test]
fn spine374_c_cross_layer_budget_preflight_and_event_identity_order() {
    let s = checked(&source());
    let mut c = controller(
        &s,
        definition(vec![action(&s, "motion", 0), action(&s, "motion", 1)]),
    );
    let first = c.advance(0, &[Intent::Play(1)]).unwrap().events.clone();
    assert_eq!(
        first[0].source_event_id,
        engine_spine_runtime::import::source_event_id("actor", "motion", 0)
    );
    assert_eq!(first[0].source_event_id, first[1].source_event_id);
    assert_ne!(first[0].play_id, first[1].play_id);
    assert_eq!(c.advance(1200, &[]).unwrap_err(), EVENT);
    assert_eq!(c.logical_tick(), 0);
    let e = c.advance(25, &[]).unwrap().events.clone();
    assert_eq!(e.len(), 2);
    assert_eq!(e.iter().map(|e| e.track).collect::<Vec<_>>(), [0, 1]);
    assert_eq!(e[0].crossing_us, 250000);
}
#[test]
fn spine374_c_zero_duration_pose_and_table_limits() {
    let mut v = source();
    v["animations"] = json!({"pose":{"bones":{"upper":{"translate":[{"x":8}]}}}});
    let s = checked(&v);
    let mut c = controller(&s, definition(vec![action(&s, "pose", 0)]));
    c.advance(0, &[]).unwrap();
    c.advance(10000, &[]).unwrap();
    close(c.bone("upper").unwrap().x, 8.0);
    let mut d = definition(vec![action(&s, "pose", 0)]);
    d.layers = vec![
        Layer {
            additive: false,
            weight_permille: 1000
        };
        5
    ];
    assert_eq!(
        ControlTable::compile(&s, d).err(),
        Some(ControlError("animator.mix.layer_capacity"))
    );
    let mut d = definition(vec![action(&s, "pose", 0)]);
    d.actions[0].speed_permille = 4001;
    assert_eq!(ControlTable::compile(&s, d).err(), Some(INVALID));
    let mut d = definition(vec![action(&s, "pose", 0)]);
    d.actions[0].mix_in_ticks = 6001;
    assert_eq!(ControlTable::compile(&s, d).err(), Some(INVALID));
}
#[test]
fn spine374_c_rules_resolve_per_layer_and_pause_defers_trigger_consumption() {
    let s = checked(&source());
    let mut d = definition(vec![action(&s, "base", 0), action(&s, "attack", 1)]);
    d.parameters = vec![Parameter::Bool(false), Parameter::Trigger];
    d.rules = vec![Rule {
        all: vec![(0, true)],
        action: 1,
    }];
    let mut c = controller(&s, d);
    c.advance(0, &[Intent::SetBool(0, true)]).unwrap();
    c.advance(50, &[]).unwrap();
    close(c.bone("lower").unwrap().x, 100.0);
    assert!(!c.state.layers[0].is_empty());
    c.advance(51, &[Intent::SetBool(0, false)]).unwrap();
    close(c.bone("lower").unwrap().x, 5.1);
    assert!(c.state.layers[1].is_empty());
    c.advance(52, &[Intent::Play(1)]).unwrap();
    c.advance(53, &[Intent::Resume]).unwrap();
    assert!(c.state.layers[1].is_empty());
    let mut d = definition(vec![action(&s, "base", 0), action(&s, "attack", 0)]);
    d.parameters = vec![Parameter::Trigger];
    d.rules = vec![Rule {
        all: vec![(0, true)],
        action: 1,
    }];
    let mut c = controller(&s, d);
    c.advance(0, &[]).unwrap();
    c.advance(1, &[Intent::SetPaused(true), Intent::SetTrigger(0)])
        .unwrap();
    assert!(c.state.values[0]);
    close(c.bone("lower").unwrap().x, 0.0);
    c.advance(100, &[Intent::SetPaused(false)]).unwrap();
    assert!(!c.state.values[0]);
    close(c.bone("lower").unwrap().x, 100.0);
}
#[test]
fn spine374_c_additive_rotation_uses_official_setup_reference() {
    let mut v = source();
    v["animations"] =
        json!({"turn":{"bones":{"upper":{"rotate":[{"value":10},{"time":1,"value":30}]}}}});
    v["bones"][2]["rotation"] = 15.into();
    let s = checked(&v);
    let mut d = definition(vec![action(&s, "turn", 0), action(&s, "turn", 1)]);
    d.layers[1].additive = true;
    let mut c = controller(&s, d);
    c.advance(0, &[Intent::Play(1)]).unwrap();
    c.advance(50, &[]).unwrap();
    assert!((c.bone("upper").unwrap().rotation - 55.0).abs() < 0.001);
}
#[test]
fn spine374_c_once_plan_reserves_empty_entry_before_accepting_play() {
    let s = checked(&source());
    let mut acts = vec![
        action(&s, "base", 0),
        action(&s, "motion", 1),
        action(&s, "attack", 1),
        action(&s, "rest", 1),
        action(&s, "attack", 1),
    ];
    for a in &mut acts {
        a.mix_in_ticks = 1000;
    }
    acts[4].looping = false;
    acts[4].speed_permille = 4000;
    acts[4].fade_out_ticks = Some(10);
    let mut c = controller(&s, definition(acts));
    c.advance(0, &[Intent::Play(1)]).unwrap();
    c.advance(1, &[Intent::Play(2)]).unwrap();
    c.advance(2, &[Intent::Play(3)]).unwrap();
    let rev = c.pose().revision;
    assert_eq!(c.advance(3, &[Intent::Play(4)]).unwrap_err(), CHAIN);
    assert_eq!(c.pose().revision, rev);
    c.advance(3, &[Intent::ClearLayer(1), Intent::Play(1)])
        .unwrap();
    c.advance(4, &[Intent::Play(2)]).unwrap();
    c.advance(5, &[Intent::Play(4)]).unwrap();
    c.advance(30, &[]).unwrap();
    assert!(c.live_entry_count(1) <= 4);
    c.advance(40, &[]).unwrap();
    assert_eq!(c.live_entry_count(1), 0);
}
#[test]
fn spine374_c_interrupt_uses_current_tick_mix_weight_not_previous_sample() {
    let s = checked(&source());
    let mut acts = vec![
        action(&s, "base", 0),
        action(&s, "attack", 0),
        action(&s, "motion", 0),
        action(&s, "rest", 0),
    ];
    for a in &mut acts {
        a.mix_in_ticks = 100;
    }
    let mut c = controller(&s, definition(acts));
    let mut reference = c.table.asset.instance().unwrap();
    c.advance(0, &[]).unwrap();
    reference.play(0, "base", None, 0.0, true, false).unwrap();
    reference.step(0.0).unwrap();
    let mut last = 0;
    for (tick, action, name) in [(10, 1, "attack"), (50, 2, "motion"), (75, 3, "rest")] {
        reference.step((tick - last) as f32 / 100.0).unwrap();
        reference.play(0, name, None, 1.0, true, false).unwrap();
        reference.step(0.0).unwrap();
        c.advance(tick, &[Intent::Play(action)]).unwrap();
        close(
            c.bone("lower").unwrap().x,
            reference.bone("lower").unwrap().x,
        );
        close(
            c.bone("upper").unwrap().x,
            reference.bone("upper").unwrap().x,
        );
        last = tick;
    }
}
#[test]
fn spine374_c_single_loop_callback_count_same_time_source_order_and_payload() {
    let mut v = source();
    v["animations"]["motion"]["events"] = json!([{"name":"zero"},{"time":0.25,"name":"beat","int":7,"float":0.5,"string":"first","volume":0.75,"balance":-0.5},{"time":0.25,"name":"beat","int":9,"string":"second"},{"time":1,"name":"end"}]);
    let s = checked(&v);
    let mut c = controller(&s, definition(vec![action(&s, "motion", 0)]));
    let count = c.advance(0, &[]).unwrap().events.len();
    assert_eq!(count, c.backend.callback_event_count() as usize);
    let e = c.advance(25, &[]).unwrap().events.clone();
    assert_eq!(e.len(), 2);
    assert_eq!(e.len(), c.backend.callback_event_count() as usize);
    assert_eq!(
        e.iter().map(|e| e.source_ordinal).collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(e[0].payload.int_value, 7);
    assert_eq!(e[1].payload.int_value, 9);
    assert_eq!(e[0].payload.string_value, "first");
    close(e[0].payload.float_value, 0.5);
    c.advance(100, &[]).unwrap();
    assert_eq!(
        c.pose().events.len(),
        c.backend.callback_event_count() as usize
    );
}
