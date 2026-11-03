//! Ordered synthetic input shared by semantic ticks and native presentation frames.
use engine_input::{RawInputEvent, RuntimePointerButton};
use serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ReplayMouseButton {
    Left,
    Right,
    Middle,
}

impl ReplayMouseButton {
    fn runtime(self) -> RuntimePointerButton {
        match self {
            Self::Left => RuntimePointerButton::Primary,
            Self::Right => RuntimePointerButton::Secondary,
            Self::Middle => RuntimePointerButton::Middle,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum ReplayInputEvent {
    KeyDown { key: String },
    KeyUp { key: String },
    PointerMove { position: [i32; 2] },
    MouseDown { button: ReplayMouseButton },
    MouseUp { button: ReplayMouseButton },
    MouseWheel { delta: i32 },
    TextInput { text: String },
}

impl ReplayInputEvent {
    pub(crate) fn raw(&self, index: u64, window: &str) -> RawInputEvent {
        match self {
            Self::KeyDown { key } => RawInputEvent::keyboard_down(index, window, key),
            Self::KeyUp { key } => RawInputEvent::keyboard_up(index, window, key),
            Self::PointerMove { position: [x, y] } => {
                RawInputEvent::mouse_move(index, window, *x as f32, *y as f32)
            }
            Self::MouseDown { button } => {
                RawInputEvent::mouse_button_down(index, window, button.runtime())
            }
            Self::MouseUp { button } => {
                RawInputEvent::mouse_button_up(index, window, button.runtime())
            }
            Self::MouseWheel { delta } => RawInputEvent::mouse_wheel(index, window, *delta as f32),
            Self::TextInput { text } => RawInputEvent::text_input(index, window, text),
        }
    }
}

pub(crate) fn legacy_events(
    down: &[String],
    up: &[String],
    position: Option<[i32; 2]>,
) -> Vec<ReplayInputEvent> {
    down.iter()
        .map(|key| ReplayInputEvent::KeyDown { key: key.clone() })
        .chain(
            up.iter()
                .map(|key| ReplayInputEvent::KeyUp { key: key.clone() }),
        )
        .chain(position.map(|position| ReplayInputEvent::PointerMove { position }))
        .collect()
}

pub(crate) fn supported_key(key: &str) -> bool {
    crate::semantic_outcome::supported_key(key)
        || matches!(
            key,
            "Tab"
                | "ShiftLeft"
                | "ShiftRight"
                | "ControlLeft"
                | "ControlRight"
                | "AltLeft"
                | "AltRight"
        )
}

#[derive(Default)]
pub(crate) struct ReplayInputState {
    keys: BTreeSet<String>,
    buttons: BTreeSet<ReplayMouseButton>,
    positioned: bool,
}

impl ReplayInputState {
    pub(crate) fn validate(
        &mut self,
        events: &[ReplayInputEvent],
        path: &str,
    ) -> Result<(), (String, String)> {
        if events.is_empty() || events.len() > 32 {
            return Err((path.into(), "Expected 1-32 ordered events.".into()));
        }
        for (index, event) in events.iter().enumerate() {
            let path = format!("{path}/{index}");
            match event {
                ReplayInputEvent::KeyDown { key } | ReplayInputEvent::KeyUp { key } => {
                    if !supported_key(key) {
                        return Err((
                            format!("{path}/key"),
                            format!(
                                "Unsupported key {key:?}; use A-Z, 0-9, Space, Enter, Escape, ArrowUp/Down/Left/Right, Tab, ShiftLeft/Right, ControlLeft/Right or AltLeft/Right."
                            ),
                        ));
                    }
                    let changed = if matches!(event, ReplayInputEvent::KeyDown { .. }) {
                        self.keys.insert(key.clone())
                    } else {
                        self.keys.remove(key)
                    };
                    if !changed {
                        return Err((
                            format!("{path}/key"),
                            format!("Key {key:?}: repeated press or release without press."),
                        ));
                    }
                }
                ReplayInputEvent::PointerMove { .. } => self.positioned = true,
                ReplayInputEvent::MouseDown { button } => {
                    if !self.positioned {
                        return Err((
                            path,
                            "mouseDown requires an earlier explicit pointerMove.".into(),
                        ));
                    }
                    if !self.buttons.insert(*button) {
                        return Err((
                            format!("{path}/button"),
                            format!("Button {button:?} is already pressed."),
                        ));
                    }
                }
                ReplayInputEvent::MouseUp { button } => {
                    if !self.buttons.remove(button) {
                        return Err((
                            format!("{path}/button"),
                            format!("Button {button:?} is not pressed."),
                        ));
                    }
                }
                ReplayInputEvent::MouseWheel { delta } => {
                    if !self.positioned || *delta == 0 || delta.unsigned_abs() > 120 {
                        return Err((path, "mouseWheel requires pointerMove and a nonzero delta of at most 120 steps.".into()));
                    }
                }
                ReplayInputEvent::TextInput { text } => {
                    if text.is_empty() || text.len() > 4096 || text.chars().any(char::is_control) {
                        return Err((
                            format!("{path}/text"),
                            "textInput requires 1-4096 UTF-8 bytes without control characters."
                                .into(),
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn released(&self) -> bool {
        self.keys.is_empty() && self.buttons.is_empty()
    }
}

// Preserve presence information (including empty legacy arrays) and event indexes at
// the JSON boundary. The typed DTO remains the sole format and enum definition.
pub(crate) fn deserialize_points<'de, D, T>(deserializer: D, root: &str) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let points = Vec::<serde_json::Value>::deserialize(deserializer)?;
    points.into_iter().enumerate().map(|(i, value)| {
        let path = format!("/{root}/{i}");
        if let Some(events) = value.get("events") {
            for field in ["keyDown", "keyUp", "pointerPosition"] {
                if value.get(field).is_some() {
                    return Err(serde::de::Error::custom(format!("{path}/{field}: v2 events cannot mix with v1 fields, including empty values")));
                }
            }
            let events = events.as_array().ok_or_else(|| serde::de::Error::custom(format!("{path}/events: expected an array")))?;
            for (j, event) in events.iter().enumerate() {
                serde_json::from_value::<ReplayInputEvent>(event.clone()).map_err(|e| serde::de::Error::custom(format!("{path}/events/{j}: {e}")))?;
            }
        }
        serde_json::from_value(value).map_err(|e| serde::de::Error::custom(format!("{path}: {e}")))
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic_outcome::PlaytestScenario;
    use crate::{scripted_input_events, NativePlayerInputScript, NativePlayerWindowRunRequest};
    use serde_json::{json, Value};

    fn scenario(inputs: Value) -> Value {
        json!({"schemaVersion":"playtest-scenario.v2","scenarioId":"ordered",
            "initialSceneId":"scene-main","target":"windows-headless",
            "maxSimulationTicks":10,"maxPresentationFrames":20,"timeoutMs":1000,
            "inputs":inputs})
    }
    fn native(frames: Value) -> NativePlayerInputScript {
        serde_json::from_value(json!({"schemaVersion":"native-player-input-script.v2",
            "scriptId":"ordered", "frames":frames}))
        .unwrap()
    }

    #[test]
    fn ordered_wheel_and_unicode_text_share_raw_input_and_reject_invalid_values() {
        let events: Vec<ReplayInputEvent> = serde_json::from_value(json!([
            {"type":"pointerMove","position":[100,200]},
            {"type":"mouseWheel","delta":-2},
            {"type":"textInput","text":"设置 العربية"}
        ]))
        .unwrap();
        ReplayInputState::default()
            .validate(&events, "/inputs/0/events")
            .unwrap();
        let raw = events
            .iter()
            .enumerate()
            .map(|(i, event)| event.raw(i as u64, "main"))
            .collect::<Vec<_>>();
        assert_eq!(
            raw[1].event_kind,
            engine_input::RawInputEventKind::MouseWheel
        );
        assert_eq!(
            raw[2].event_kind,
            engine_input::RawInputEventKind::TextInput
        );
        for input in [
            json!([{"type":"mouseWheel","delta":1}]),
            json!([{"type":"pointerMove","position":[0,0]},{"type":"mouseWheel","delta":0}]),
            json!([{"type":"pointerMove","position":[0,0]},{"type":"mouseWheel","delta":-2147483648_i32}]),
            json!([{"type":"textInput","text":""}]),
            json!([{"type":"textInput","text":"\u{0000}"}]),
            json!([{"type":"textInput","text":"a".repeat(4097)}]),
        ] {
            let events: Vec<ReplayInputEvent> = serde_json::from_value(input).unwrap();
            assert!(ReplayInputState::default()
                .validate(&events, "/inputs/0/events")
                .is_err());
        }
    }

    #[test]
    fn ordered_input_rejects_bad_states_at_precise_event() {
        for (events, suffix) in [
            (json!([]), "/events"),
            (json!([{"type":"mouseDown","button":"Left"}]), "/events/0"),
            (
                json!([{"type":"mouseUp","button":"Right"}]),
                "/events/0/button",
            ),
            (json!([{"type":"keyUp","key":"D"}]), "/events/0/key"),
            (json!([{"type":"keyDown","key":"KeyD"}]), "/events/0/key"),
            (
                json!([{"type":"keyDown","key":"D"},{"type":"keyDown","key":"D"}]),
                "/events/1/key",
            ),
            (
                json!([{"type":"pointerMove","position":[0,0]}, {"type":"mouseDown","button":"Left"}, {"type":"mouseDown","button":"Left"}]),
                "/events/2/button",
            ),
        ] {
            let scenario: PlaytestScenario =
                serde_json::from_value(scenario(json!([{"simulationTick":1,"events":events}])))
                    .unwrap();
            let error = scenario.validate(None).unwrap_err();
            assert_eq!(error.field_path, format!("/inputs/0{suffix}"));
            let script = native(json!([{"frameIndex":1,"events":events}]));
            assert!(script
                .validate()
                .unwrap_err()
                .contains(&format!("/frames/0{suffix}")));
        }
    }

    #[test]
    fn ordered_input_strict_fields_versions_and_limits() {
        for event in [
            json!({"type":"mouseDown","button":"Extra"}),
            json!({"type":"pointerMove","position":[0.5,2]}),
            json!({"type":"pointerMove","position":[2147483648_i64,2]}),
            json!({"type":"keyDown","key":"D","extra":true}),
            json!({"type":"keyDown"}),
            json!({"type":"unknown"}),
        ] {
            let error = serde_json::from_value::<PlaytestScenario>(scenario(
                json!([{"simulationTick":1,"events":[event]}]),
            ))
            .unwrap_err();
            assert!(error.to_string().contains("/inputs/0/events/0"), "{error}");
        }
        for field in ["keyDown", "keyUp", "pointerPosition"] {
            let mut input = json!({"simulationTick":1,"events":[]});
            input[field] = json!([]);
            assert!(serde_json::from_value::<PlaytestScenario>(scenario(json!([input]))).is_err());
        }
        for inputs in [
            json!([{"simulationTick":1,"events":null}]),
            json!([{"simulationTick":1,"events":{},"unknown":true}]),
        ] {
            assert!(serde_json::from_value::<PlaytestScenario>(scenario(inputs)).is_err());
        }
        for inputs in [
            json!([{"simulationTick":1,"keyDown":[]}]),
            json!([{"simulationTick":1,"events":[{"type":"keyDown","key":"Tab"}]}]),
            json!([{"simulationTick":0,"events":[{"type":"pointerMove","position":[0,0]}]}]),
            json!([{"simulationTick":11,"events":[{"type":"pointerMove","position":[0,0]}]}]),
            json!([{"simulationTick":1,"events":vec![json!({"type":"pointerMove","position":[0,0]});33]}]),
            json!([{"simulationTick":1,"events":[{"type":"pointerMove","position":[0,0]}]}, {"simulationTick":1,"events":[{"type":"pointerMove","position":[0,0]}]}]),
        ] {
            let decoded: PlaytestScenario = serde_json::from_value(scenario(inputs)).unwrap();
            assert!(decoded.validate(None).is_err());
        }
        let valid =
            json!([{"simulationTick":1,"events":[{"type":"pointerMove","position":[-10,2000]}]}]);
        let mut decoded: PlaytestScenario = serde_json::from_value(scenario(valid)).unwrap();
        decoded.validate(None).unwrap();
        decoded.schema_version = "playtest-scenario.v1".into();
        assert!(decoded.validate(None).is_err());
        decoded.schema_version = "playtest-scenario.v99".into();
        assert!(decoded.validate(None).is_err());
        decoded.inputs.clear();
        decoded.schema_version = "playtest-scenario.v2".into();
        decoded.validate(None).unwrap();
    }

    #[test]
    fn ordered_input_raw_order_coordinates_and_legacy_order() {
        let script = native(json!([{"frameIndex":1,"events":[
            {"type":"pointerMove","position":[10,20]},
            {"type":"mouseDown","button":"Left"},
            {"type":"pointerMove","position":[40,50]},
            {"type":"mouseUp","button":"Left"}
        ]}]));
        script.validate().unwrap();
        let request =
            NativePlayerWindowRunRequest::headless_surface_gate("unused").with_input_script(script);
        let mut state = engine_input::InputDeviceState::new();
        state.apply_raw_events(&scripted_input_events(&request, 1));
        let frame = state.to_runtime_input_frame(1, "main-window");
        let pointers: Vec<_> = frame
            .events
            .iter()
            .filter_map(engine_input::RuntimeInputEvent::pointer_event)
            .collect();
        assert_eq!(pointers.len(), 4);
        assert_eq!(
            pointers[1].phase,
            engine_input::input_mapping::RuntimePointerPhase::Down
        );
        assert_eq!((pointers[1].x, pointers[1].y), (10.0, 20.0));
        assert_eq!(
            pointers[3].phase,
            engine_input::input_mapping::RuntimePointerPhase::Up
        );
        assert_eq!((pointers[3].x, pointers[3].y), (40.0, 50.0));
        assert_eq!(state.pressed_mouse_button_count(), 0);
        assert!(scripted_input_events(&request, 2).is_empty());
        let legacy: NativePlayerInputScript = serde_json::from_value(json!({"schemaVersion":"native-player-input-script.v1", "scriptId":"legacy", "frames":[{"frameIndex":1,"keyDown":["D"],"keyUp":["D"],"pointerPosition":[5,6]}]})).unwrap();
        legacy.validate().unwrap(); // Native v1 deliberately remains permissive.
        let events = scripted_input_events(
            &NativePlayerWindowRunRequest::headless_surface_gate("unused")
                .with_input_script(legacy),
            1,
        );
        assert_eq!(
            events.iter().map(|e| e.event_kind).collect::<Vec<_>>(),
            vec![
                engine_input::RawInputEventKind::KeyboardKeyDown,
                engine_input::RawInputEventKind::KeyboardKeyUp,
                engine_input::RawInputEventKind::MouseMove
            ]
        );
    }

    #[test]
    fn ordered_input_all_modifiers_buttons_and_cross_tick_transitions() {
        let mut events = vec![ReplayInputEvent::PointerMove { position: [1, 2] }];
        for key in [
            "Tab",
            "ShiftLeft",
            "ShiftRight",
            "ControlLeft",
            "ControlRight",
            "AltLeft",
            "AltRight",
        ] {
            events.push(ReplayInputEvent::KeyDown { key: key.into() });
            events.push(ReplayInputEvent::KeyUp { key: key.into() });
        }
        for button in [
            ReplayMouseButton::Left,
            ReplayMouseButton::Right,
            ReplayMouseButton::Middle,
        ] {
            events.push(ReplayInputEvent::MouseDown { button });
            events.push(ReplayInputEvent::MouseUp { button });
        }
        let mut state = ReplayInputState::default();
        state.validate(&events, "/events").unwrap();
        assert!(state.released());
        state
            .validate(&[ReplayInputEvent::KeyDown { key: "D".into() }], "/first")
            .unwrap();
        state
            .validate(
                &[
                    ReplayInputEvent::KeyUp { key: "D".into() },
                    ReplayInputEvent::KeyDown { key: "D".into() },
                ],
                "/second",
            )
            .unwrap();
        assert!(!state.released());
        state
            .validate(&[ReplayInputEvent::KeyUp { key: "D".into() }], "/third")
            .unwrap();
        assert!(state.released());
    }

    #[test]
    fn ordered_input_shared_mapping_holds_modifiers_and_mouse_until_release() {
        use engine_input::{
            InputActionDefinition, InputActionValueType, InputBindingDefinition,
            InputContextDefinition, InputDeviceState, InputMappingAsset,
        };
        let script = native(json!([
            {"frameIndex":1,"events":[{"type":"pointerMove","position":[100,200]}, {"type":"mouseDown","button":"Left"},{"type":"keyDown","key":"ShiftLeft"},{"type":"keyDown","key":"D"}]},
            {"frameIndex":3,"events":[{"type":"pointerMove","position":[300,400]},{"type":"mouseUp","button":"Left"},{"type":"keyUp","key":"D"},{"type":"keyUp","key":"ShiftLeft"}]}
        ]));
        script.validate().unwrap();
        let request =
            NativePlayerWindowRunRequest::headless_surface_gate("unused").with_input_script(script);
        let mapping = InputMappingAsset::new(
            "test",
            ["fire", "shift", "right"]
                .into_iter()
                .map(|id| InputActionDefinition::new(id, InputActionValueType::Button))
                .collect(),
            vec![InputContextDefinition::new("gameplay", 0)],
            vec![
                InputBindingDefinition::new("gameplay", "fire", "mouse/Left"),
                InputBindingDefinition::button("shift", "ShiftLeft"),
                InputBindingDefinition::button("right", "D"),
            ],
        );
        let mut state = InputDeviceState::new();
        for tick in 1..=4 {
            let (snapshot, _, summary) = crate::resolve_native_input_frame(
                &mut state,
                &mapping,
                "test",
                None,
                &scripted_input_events(&request, tick),
                tick,
                "main-window",
                "script",
                "test",
            );
            for action in ["fire", "shift", "right"] {
                assert_eq!(
                    snapshot.button_pressed(action),
                    tick < 3,
                    "{tick}: {action}"
                );
            }
            assert_eq!(
                state
                    .to_runtime_input_frame(tick, "main-window")
                    .modifiers
                    .contains(&"Shift".to_string()),
                tick < 3
            );
            assert_eq!(
                summary.raw_event_count,
                if tick == 1 || tick == 3 { 4 } else { 0 }
            );
        }
        assert_eq!(state.pressed_key_count(), 0);
        assert_eq!(state.pressed_mouse_button_count(), 0);
    }
}
