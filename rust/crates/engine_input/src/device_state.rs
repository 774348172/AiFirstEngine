use crate::input_action::PointerPosition;
use crate::input_mapping::{
    RuntimeInputEvent, RuntimeInputFrame, RuntimePointerButton, RuntimePointerEvent,
    RuntimePointerPhase,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputDeviceKind {
    Keyboard,
    Mouse,
    Touch,
    Gamepad,
    Window,
}

impl InputDeviceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Keyboard => "keyboard",
            Self::Mouse => "mouse",
            Self::Touch => "touch",
            Self::Gamepad => "gamepad",
            Self::Window => "window",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RawInputEventKind {
    KeyboardKeyDown,
    KeyboardKeyUp,
    MouseMove,
    MouseButtonDown,
    MouseButtonUp,
    MouseWheel,
    MouseLeave,
    TouchStart,
    TouchMove,
    TouchEnd,
    TouchCancel,
    TextInput,
    ImePreedit,
    ImeCommit,
    ImeCancel,
    GamepadButtonDown,
    GamepadButtonUp,
    GamepadAxis2d,
    ModifiersChanged,
    FocusLost,
}

impl RawInputEventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::KeyboardKeyDown => "keyboard_key_down",
            Self::KeyboardKeyUp => "keyboard_key_up",
            Self::MouseMove => "mouse_move",
            Self::MouseButtonDown => "mouse_button_down",
            Self::MouseButtonUp => "mouse_button_up",
            Self::MouseWheel => "mouse_wheel",
            Self::MouseLeave => "mouse_leave",
            Self::TouchStart => "touch_start",
            Self::TouchMove => "touch_move",
            Self::TouchEnd => "touch_end",
            Self::TouchCancel => "touch_cancel",
            Self::TextInput => "text_input",
            Self::ImePreedit => "ime_preedit",
            Self::ImeCommit => "ime_commit",
            Self::ImeCancel => "ime_cancel",
            Self::GamepadButtonDown => "gamepad_button_down",
            Self::GamepadButtonUp => "gamepad_button_up",
            Self::GamepadAxis2d => "gamepad_axis2d",
            Self::ModifiersChanged => "modifiers_changed",
            Self::FocusLost => "focus_lost",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RawInputValue {
    None,
    Button {
        pressed: bool,
    },
    Axis1 {
        value: f32,
    },
    Axis2 {
        x: f32,
        y: f32,
    },
    Pointer {
        x: f32,
        y: f32,
    },
    Touch {
        touch_id: u64,
        x: f32,
        y: f32,
    },
    Text {
        text: String,
    },
    ImePreedit {
        text: String,
        cursor_start: usize,
        cursor_end: usize,
    },
    Modifiers {
        active: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawInputEvent {
    pub frame_id: u64,
    pub window_id: String,
    pub device_kind: InputDeviceKind,
    pub event_kind: RawInputEventKind,
    pub device_path: String,
    pub value: RawInputValue,
    pub is_repeat: bool,
}

impl RawInputEvent {
    pub fn keyboard_down(
        frame_id: u64,
        window_id: impl Into<String>,
        key: impl Into<String>,
    ) -> Self {
        let key = key.into();
        Self {
            frame_id,
            window_id: window_id.into(),
            device_kind: InputDeviceKind::Keyboard,
            event_kind: RawInputEventKind::KeyboardKeyDown,
            device_path: format!("keyboard/{key}"),
            value: RawInputValue::Button { pressed: true },
            is_repeat: false,
        }
    }

    pub fn keyboard_up(
        frame_id: u64,
        window_id: impl Into<String>,
        key: impl Into<String>,
    ) -> Self {
        let key = key.into();
        Self {
            frame_id,
            window_id: window_id.into(),
            device_kind: InputDeviceKind::Keyboard,
            event_kind: RawInputEventKind::KeyboardKeyUp,
            device_path: format!("keyboard/{key}"),
            value: RawInputValue::Button { pressed: false },
            is_repeat: false,
        }
    }

    pub fn mouse_move(frame_id: u64, window_id: impl Into<String>, x: f32, y: f32) -> Self {
        Self {
            frame_id,
            window_id: window_id.into(),
            device_kind: InputDeviceKind::Mouse,
            event_kind: RawInputEventKind::MouseMove,
            device_path: "mouse/Position".to_string(),
            value: RawInputValue::Pointer { x, y },
            is_repeat: false,
        }
    }

    pub fn mouse_button_down(
        frame_id: u64,
        window_id: impl Into<String>,
        button: RuntimePointerButton,
    ) -> Self {
        Self {
            frame_id,
            window_id: window_id.into(),
            device_kind: InputDeviceKind::Mouse,
            event_kind: RawInputEventKind::MouseButtonDown,
            device_path: format!("mouse/{}", button.as_device_button()),
            value: RawInputValue::Button { pressed: true },
            is_repeat: false,
        }
    }

    pub fn mouse_button_up(
        frame_id: u64,
        window_id: impl Into<String>,
        button: RuntimePointerButton,
    ) -> Self {
        Self {
            frame_id,
            window_id: window_id.into(),
            device_kind: InputDeviceKind::Mouse,
            event_kind: RawInputEventKind::MouseButtonUp,
            device_path: format!("mouse/{}", button.as_device_button()),
            value: RawInputValue::Button { pressed: false },
            is_repeat: false,
        }
    }

    pub fn mouse_wheel(frame_id: u64, window_id: impl Into<String>, delta: f32) -> Self {
        Self {
            frame_id,
            window_id: window_id.into(),
            device_kind: InputDeviceKind::Mouse,
            event_kind: RawInputEventKind::MouseWheel,
            device_path: "mouse/Wheel".to_string(),
            value: RawInputValue::Axis1 { value: delta },
            is_repeat: false,
        }
    }

    pub fn mouse_leave(frame_id: u64, window_id: impl Into<String>, x: f32, y: f32) -> Self {
        Self {
            frame_id,
            window_id: window_id.into(),
            device_kind: InputDeviceKind::Mouse,
            event_kind: RawInputEventKind::MouseLeave,
            device_path: "mouse/Position".to_string(),
            value: RawInputValue::Pointer { x, y },
            is_repeat: false,
        }
    }

    fn touch(
        frame_id: u64,
        window_id: impl Into<String>,
        event_kind: RawInputEventKind,
        touch_id: u64,
        x: f32,
        y: f32,
    ) -> Self {
        Self {
            frame_id,
            window_id: window_id.into(),
            device_kind: InputDeviceKind::Touch,
            event_kind,
            device_path: format!("touch/{touch_id}"),
            value: RawInputValue::Touch { touch_id, x, y },
            is_repeat: false,
        }
    }

    pub fn touch_start(
        frame_id: u64,
        window_id: impl Into<String>,
        touch_id: u64,
        x: f32,
        y: f32,
    ) -> Self {
        Self::touch(
            frame_id,
            window_id,
            RawInputEventKind::TouchStart,
            touch_id,
            x,
            y,
        )
    }

    pub fn touch_move(
        frame_id: u64,
        window_id: impl Into<String>,
        touch_id: u64,
        x: f32,
        y: f32,
    ) -> Self {
        Self::touch(
            frame_id,
            window_id,
            RawInputEventKind::TouchMove,
            touch_id,
            x,
            y,
        )
    }

    pub fn touch_end(
        frame_id: u64,
        window_id: impl Into<String>,
        touch_id: u64,
        x: f32,
        y: f32,
    ) -> Self {
        Self::touch(
            frame_id,
            window_id,
            RawInputEventKind::TouchEnd,
            touch_id,
            x,
            y,
        )
    }

    pub fn touch_cancel(
        frame_id: u64,
        window_id: impl Into<String>,
        touch_id: u64,
        x: f32,
        y: f32,
    ) -> Self {
        Self::touch(
            frame_id,
            window_id,
            RawInputEventKind::TouchCancel,
            touch_id,
            x,
            y,
        )
    }

    pub fn text_input(
        frame_id: u64,
        window_id: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            frame_id,
            window_id: window_id.into(),
            device_kind: InputDeviceKind::Keyboard,
            event_kind: RawInputEventKind::TextInput,
            device_path: "keyboard/TextInput".to_string(),
            value: RawInputValue::Text { text: text.into() },
            is_repeat: false,
        }
    }

    pub fn ime_preedit(
        frame_id: u64,
        window_id: impl Into<String>,
        text: impl Into<String>,
        cursor_start: usize,
        cursor_end: usize,
    ) -> Self {
        Self {
            frame_id,
            window_id: window_id.into(),
            device_kind: InputDeviceKind::Keyboard,
            event_kind: RawInputEventKind::ImePreedit,
            device_path: "keyboard/ImePreedit".to_string(),
            value: RawInputValue::ImePreedit {
                text: text.into(),
                cursor_start,
                cursor_end,
            },
            is_repeat: false,
        }
    }

    pub fn ime_commit(
        frame_id: u64,
        window_id: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            frame_id,
            window_id: window_id.into(),
            device_kind: InputDeviceKind::Keyboard,
            event_kind: RawInputEventKind::ImeCommit,
            device_path: "keyboard/ImeCommit".to_string(),
            value: RawInputValue::Text { text: text.into() },
            is_repeat: false,
        }
    }

    pub fn ime_cancel(frame_id: u64, window_id: impl Into<String>) -> Self {
        Self {
            frame_id,
            window_id: window_id.into(),
            device_kind: InputDeviceKind::Keyboard,
            event_kind: RawInputEventKind::ImeCancel,
            device_path: "keyboard/ImeCancel".to_string(),
            value: RawInputValue::None,
            is_repeat: false,
        }
    }

    pub fn gamepad_button_down(
        frame_id: u64,
        window_id: impl Into<String>,
        gamepad_id: u32,
        button: impl Into<String>,
    ) -> Self {
        let button = button.into();
        Self {
            frame_id,
            window_id: window_id.into(),
            device_kind: InputDeviceKind::Gamepad,
            event_kind: RawInputEventKind::GamepadButtonDown,
            device_path: format!("gamepad/{gamepad_id}/{button}"),
            value: RawInputValue::Button { pressed: true },
            is_repeat: false,
        }
    }

    pub fn gamepad_button_up(
        frame_id: u64,
        window_id: impl Into<String>,
        gamepad_id: u32,
        button: impl Into<String>,
    ) -> Self {
        let button = button.into();
        Self {
            frame_id,
            window_id: window_id.into(),
            device_kind: InputDeviceKind::Gamepad,
            event_kind: RawInputEventKind::GamepadButtonUp,
            device_path: format!("gamepad/{gamepad_id}/{button}"),
            value: RawInputValue::Button { pressed: false },
            is_repeat: false,
        }
    }

    pub fn gamepad_axis2d(
        frame_id: u64,
        window_id: impl Into<String>,
        gamepad_id: u32,
        axis: impl Into<String>,
        x: f32,
        y: f32,
    ) -> Self {
        let axis = axis.into();
        Self {
            frame_id,
            window_id: window_id.into(),
            device_kind: InputDeviceKind::Gamepad,
            event_kind: RawInputEventKind::GamepadAxis2d,
            device_path: format!("gamepad/{gamepad_id}/{axis}"),
            value: RawInputValue::Axis2 { x, y },
            is_repeat: false,
        }
    }

    pub fn modifiers_changed(
        frame_id: u64,
        window_id: impl Into<String>,
        active: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            frame_id,
            window_id: window_id.into(),
            device_kind: InputDeviceKind::Keyboard,
            event_kind: RawInputEventKind::ModifiersChanged,
            device_path: "keyboard/Modifiers".to_string(),
            value: RawInputValue::Modifiers {
                active: active.into_iter().map(Into::into).collect(),
            },
            is_repeat: false,
        }
    }

    pub fn focus_lost(frame_id: u64, window_id: impl Into<String>) -> Self {
        Self {
            frame_id,
            window_id: window_id.into(),
            device_kind: InputDeviceKind::Window,
            event_kind: RawInputEventKind::FocusLost,
            device_path: "window/Focus".to_string(),
            value: RawInputValue::Button { pressed: false },
            is_repeat: false,
        }
    }

    pub fn key_name(&self) -> Option<&str> {
        self.device_path.strip_prefix("keyboard/")
    }

    pub fn mouse_button(&self) -> Option<RuntimePointerButton> {
        match self.device_path.to_ascii_lowercase().as_str() {
            "mouse/left" => Some(RuntimePointerButton::Primary),
            "mouse/right" => Some(RuntimePointerButton::Secondary),
            "mouse/middle" => Some(RuntimePointerButton::Middle),
            _ => None,
        }
    }

    pub fn gamepad_path(&self) -> Option<(u32, &str)> {
        let path = self.device_path.strip_prefix("gamepad/")?;
        let (gamepad_id, name) = path.split_once('/')?;
        Some((gamepad_id.parse().ok()?, name))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputDeviceStateReport {
    pub focused: bool,
    pub pressed_key_count: usize,
    pub pressed_mouse_button_count: usize,
    pub pointer_position: Option<PointerPosition>,
    pub mouse_wheel_delta: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InputDeviceState {
    pressed_keys: BTreeSet<String>,
    pressed_mouse_buttons: BTreeSet<RuntimePointerButton>,
    pointer_position: Option<PointerPosition>,
    primary_touch: Option<(u64, PointerPosition)>,
    mouse_wheel_delta: f32,
    active_modifiers: BTreeSet<String>,
    pressed_gamepad_buttons: BTreeSet<(u32, String)>,
    focused: bool,
    frame_events: Vec<RuntimeInputEvent>,
}

impl Default for InputDeviceState {
    fn default() -> Self {
        Self {
            pressed_keys: BTreeSet::new(),
            pressed_mouse_buttons: BTreeSet::new(),
            pointer_position: None,
            primary_touch: None,
            mouse_wheel_delta: 0.0,
            active_modifiers: BTreeSet::new(),
            pressed_gamepad_buttons: BTreeSet::new(),
            focused: true,
            frame_events: Vec::new(),
        }
    }
}

impl InputDeviceState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin_frame(&mut self) {
        self.frame_events.clear();
        self.mouse_wheel_delta = 0.0;
    }

    pub fn apply_raw_events<'a>(&mut self, events: impl IntoIterator<Item = &'a RawInputEvent>) {
        for event in events {
            self.apply_raw_event(event);
        }
    }

    pub fn apply_raw_event(&mut self, event: &RawInputEvent) {
        match event.event_kind {
            RawInputEventKind::KeyboardKeyDown => {
                if let Some(key) = event.key_name() {
                    self.focused = true;
                    self.pressed_keys.insert(key.to_string());
                    if let Some(modifier) = canonical_modifier_name(key) {
                        self.active_modifiers.insert(modifier.to_string());
                    }
                    self.frame_events.push(RuntimeInputEvent::KeyDown {
                        key: key.to_string(),
                    });
                }
            }
            RawInputEventKind::KeyboardKeyUp => {
                if let Some(key) = event.key_name() {
                    self.pressed_keys.remove(key);
                    if let Some(modifier) = canonical_modifier_name(key) {
                        self.active_modifiers.remove(modifier);
                    }
                    self.frame_events.push(RuntimeInputEvent::KeyUp {
                        key: key.to_string(),
                    });
                }
            }
            RawInputEventKind::MouseMove => {
                if let RawInputValue::Pointer { x, y } = event.value {
                    self.pointer_position = Some(PointerPosition { x, y });
                    self.frame_events
                        .push(RuntimeInputEvent::PointerMove { x, y });
                }
            }
            RawInputEventKind::MouseButtonDown => {
                if let Some(button) = event.mouse_button() {
                    self.focused = true;
                    self.pressed_mouse_buttons.insert(button);
                    let position = self
                        .pointer_position
                        .unwrap_or(PointerPosition { x: 0.0, y: 0.0 });
                    self.frame_events.push(RuntimeInputEvent::PointerDown {
                        x: position.x,
                        y: position.y,
                        button,
                    });
                }
            }
            RawInputEventKind::MouseButtonUp => {
                if let Some(button) = event.mouse_button() {
                    self.pressed_mouse_buttons.remove(&button);
                    let position = self
                        .pointer_position
                        .unwrap_or(PointerPosition { x: 0.0, y: 0.0 });
                    self.frame_events.push(RuntimeInputEvent::PointerUp {
                        x: position.x,
                        y: position.y,
                        button,
                    });
                }
            }
            RawInputEventKind::MouseWheel => {
                if let RawInputValue::Axis1 { value } = event.value {
                    self.mouse_wheel_delta += value;
                    self.frame_events
                        .push(RuntimeInputEvent::MouseWheel { delta: value });
                }
            }
            RawInputEventKind::MouseLeave => {
                if let RawInputValue::Pointer { x, y } = event.value {
                    self.pointer_position = Some(PointerPosition { x, y });
                    self.frame_events.push(RuntimeInputEvent::Pointer {
                        pointer: RuntimePointerEvent::mouse(
                            RuntimePointerPhase::Leave,
                            0,
                            x,
                            y,
                            None,
                        ),
                    });
                }
            }
            RawInputEventKind::TouchStart => {
                if let RawInputValue::Touch { touch_id, x, y } = event.value {
                    if self.primary_touch.is_none() {
                        let position = PointerPosition { x, y };
                        self.primary_touch = Some((touch_id, position));
                        self.pointer_position = Some(position);
                        self.frame_events.push(RuntimeInputEvent::Pointer {
                            pointer: RuntimePointerEvent::touch(
                                RuntimePointerPhase::Down,
                                touch_id,
                                x,
                                y,
                            ),
                        });
                    }
                }
            }
            RawInputEventKind::TouchMove => {
                if let RawInputValue::Touch { touch_id, x, y } = event.value {
                    if self.primary_touch.map(|(owner, _)| owner) == Some(touch_id) {
                        let position = PointerPosition { x, y };
                        self.primary_touch = Some((touch_id, position));
                        self.pointer_position = Some(position);
                        self.frame_events.push(RuntimeInputEvent::Pointer {
                            pointer: RuntimePointerEvent::touch(
                                RuntimePointerPhase::Move,
                                touch_id,
                                x,
                                y,
                            ),
                        });
                    }
                }
            }
            RawInputEventKind::TouchEnd | RawInputEventKind::TouchCancel => {
                if let RawInputValue::Touch { touch_id, x, y } = event.value {
                    if self.primary_touch.map(|(owner, _)| owner) == Some(touch_id) {
                        let phase = if event.event_kind == RawInputEventKind::TouchEnd {
                            RuntimePointerPhase::Up
                        } else {
                            RuntimePointerPhase::Cancel
                        };
                        self.primary_touch = None;
                        self.pointer_position = Some(PointerPosition { x, y });
                        self.frame_events.push(RuntimeInputEvent::Pointer {
                            pointer: RuntimePointerEvent::touch(phase, touch_id, x, y),
                        });
                    }
                }
            }
            RawInputEventKind::TextInput => {
                if let RawInputValue::Text { text } = &event.value {
                    self.frame_events
                        .push(RuntimeInputEvent::TextInput { text: text.clone() });
                }
            }
            RawInputEventKind::ImePreedit => {
                if let RawInputValue::ImePreedit {
                    text,
                    cursor_start,
                    cursor_end,
                } = &event.value
                {
                    self.frame_events.push(RuntimeInputEvent::ImePreedit {
                        text: text.clone(),
                        cursor_start: *cursor_start,
                        cursor_end: *cursor_end,
                    });
                }
            }
            RawInputEventKind::ImeCommit => {
                if let RawInputValue::Text { text } = &event.value {
                    self.frame_events
                        .push(RuntimeInputEvent::ImeCommit { text: text.clone() });
                }
            }
            RawInputEventKind::ImeCancel => {
                self.frame_events.push(RuntimeInputEvent::ImeCancel);
            }
            RawInputEventKind::GamepadButtonDown => {
                if let Some((gamepad_id, button)) = event.gamepad_path() {
                    self.pressed_gamepad_buttons
                        .insert((gamepad_id, button.to_string()));
                    self.frame_events
                        .push(RuntimeInputEvent::GamepadButtonDown {
                            gamepad_id,
                            button: button.to_string(),
                        });
                }
            }
            RawInputEventKind::GamepadButtonUp => {
                if let Some((gamepad_id, button)) = event.gamepad_path() {
                    self.pressed_gamepad_buttons
                        .remove(&(gamepad_id, button.to_string()));
                    self.frame_events.push(RuntimeInputEvent::GamepadButtonUp {
                        gamepad_id,
                        button: button.to_string(),
                    });
                }
            }
            RawInputEventKind::GamepadAxis2d => {
                if let (Some((gamepad_id, axis)), RawInputValue::Axis2 { x, y }) =
                    (event.gamepad_path(), &event.value)
                {
                    self.frame_events.push(RuntimeInputEvent::GamepadAxis2d {
                        gamepad_id,
                        axis: axis.to_string(),
                        x: *x,
                        y: *y,
                    });
                }
            }
            RawInputEventKind::ModifiersChanged => {
                if let RawInputValue::Modifiers { active } = &event.value {
                    self.active_modifiers = active
                        .iter()
                        .filter_map(|modifier| canonical_modifier_name(modifier))
                        .map(ToOwned::to_owned)
                        .collect();
                }
            }
            RawInputEventKind::FocusLost => {
                self.focused = false;
                for key in std::mem::take(&mut self.pressed_keys) {
                    self.frame_events.push(RuntimeInputEvent::KeyUp { key });
                }
                self.active_modifiers.clear();
                for button in std::mem::take(&mut self.pressed_mouse_buttons) {
                    let position = self
                        .pointer_position
                        .unwrap_or(PointerPosition { x: 0.0, y: 0.0 });
                    self.frame_events.push(RuntimeInputEvent::Pointer {
                        pointer: RuntimePointerEvent::mouse(
                            RuntimePointerPhase::Cancel,
                            0,
                            position.x,
                            position.y,
                            Some(button),
                        ),
                    });
                }
                if let Some((touch_id, position)) = self.primary_touch.take() {
                    self.frame_events.push(RuntimeInputEvent::Pointer {
                        pointer: RuntimePointerEvent::touch(
                            RuntimePointerPhase::Cancel,
                            touch_id,
                            position.x,
                            position.y,
                        ),
                    });
                }
                for (gamepad_id, button) in std::mem::take(&mut self.pressed_gamepad_buttons) {
                    self.frame_events
                        .push(RuntimeInputEvent::GamepadButtonUp { gamepad_id, button });
                }
            }
        }
    }

    pub fn to_runtime_input_frame(
        &self,
        frame_id: u64,
        viewport_id: impl Into<String>,
    ) -> RuntimeInputFrame {
        let mut events = self.frame_events.clone();
        for key in &self.pressed_keys {
            events.push(RuntimeInputEvent::KeyHeld { key: key.clone() });
        }
        for (gamepad_id, button) in &self.pressed_gamepad_buttons {
            events.push(RuntimeInputEvent::GamepadButtonHeld {
                gamepad_id: *gamepad_id,
                button: button.clone(),
            });
        }
        for button in &self.pressed_mouse_buttons {
            let position = self
                .pointer_position
                .unwrap_or(PointerPosition { x: 0.0, y: 0.0 });
            events.push(RuntimeInputEvent::PointerHeld {
                x: position.x,
                y: position.y,
                button: *button,
            });
        }
        RuntimeInputFrame {
            frame_id,
            viewport_id: viewport_id.into(),
            events,
            modifiers: self.active_modifiers.iter().cloned().collect(),
            pointer_position: self.pointer_position,
        }
    }

    pub fn report(&self) -> InputDeviceStateReport {
        InputDeviceStateReport {
            focused: self.focused,
            pressed_key_count: self.pressed_keys.len(),
            pressed_mouse_button_count: self.pressed_mouse_buttons.len(),
            pointer_position: self.pointer_position,
            mouse_wheel_delta: self.mouse_wheel_delta,
        }
    }

    pub fn pressed_key_count(&self) -> usize {
        self.pressed_keys.len()
    }

    pub fn pressed_mouse_button_count(&self) -> usize {
        self.pressed_mouse_buttons.len()
    }
}

fn canonical_modifier_name(name: &str) -> Option<&'static str> {
    match name.to_ascii_lowercase().as_str() {
        "shift" | "shiftleft" | "shiftright" => Some("Shift"),
        "control" | "ctrl" | "controlleft" | "controlright" => Some("Control"),
        "alt" | "altleft" | "altright" => Some("Alt"),
        "super" | "logo" | "meta" | "windows" | "command" => Some("Logo"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_mapping::RuntimePointerDeviceKind;

    #[test]
    fn input_device_state_generates_pressed_and_held_key_events() {
        let mut state = InputDeviceState::new();
        state.apply_raw_event(&RawInputEvent::keyboard_down(1, "main-window", "D"));

        let first = state.to_runtime_input_frame(1, "main-window");
        assert!(first
            .events
            .iter()
            .any(|event| matches!(event, RuntimeInputEvent::KeyDown { key } if key == "D")));
        assert!(first
            .events
            .iter()
            .any(|event| matches!(event, RuntimeInputEvent::KeyHeld { key } if key == "D")));

        state.begin_frame();
        let second = state.to_runtime_input_frame(2, "main-window");
        assert_eq!(second.events.len(), 1);
        assert!(matches!(&second.events[0], RuntimeInputEvent::KeyHeld { key } if key == "D"));
    }

    #[test]
    fn input_device_state_releases_all_on_focus_lost() {
        let mut state = InputDeviceState::new();
        state.apply_raw_event(&RawInputEvent::keyboard_down(1, "main-window", "Space"));
        state.apply_raw_event(&RawInputEvent::mouse_button_down(
            1,
            "main-window",
            RuntimePointerButton::Primary,
        ));
        state.begin_frame();
        state.apply_raw_event(&RawInputEvent::focus_lost(2, "main-window"));

        let frame = state.to_runtime_input_frame(2, "main-window");
        assert_eq!(state.pressed_key_count(), 0);
        assert_eq!(state.pressed_mouse_button_count(), 0);
        assert!(frame
            .events
            .iter()
            .any(|event| matches!(event, RuntimeInputEvent::KeyUp { key } if key == "Space")));
        assert!(frame
            .events
            .iter()
            .filter_map(RuntimeInputEvent::pointer_event)
            .any(|pointer| pointer.phase == RuntimePointerPhase::Cancel
                && pointer.device_kind == RuntimePointerDeviceKind::Mouse
                && pointer.button == Some(RuntimePointerButton::Primary)));
    }

    #[test]
    fn runtime_pointer_mouse_events_are_hover_capable() {
        let mut state = InputDeviceState::new();
        state.apply_raw_event(&RawInputEvent::mouse_move(1, "main-window", 10.0, 20.0));
        state.apply_raw_event(&RawInputEvent::mouse_button_down(
            1,
            "main-window",
            RuntimePointerButton::Primary,
        ));

        let frame = state.to_runtime_input_frame(1, "main-window");
        let pointers = frame
            .events
            .iter()
            .filter_map(RuntimeInputEvent::pointer_event)
            .collect::<Vec<_>>();

        assert!(pointers.iter().all(|pointer| {
            pointer.device_kind == RuntimePointerDeviceKind::Mouse && pointer.hover_capable
        }));
        assert!(pointers
            .iter()
            .any(|pointer| pointer.phase == RuntimePointerPhase::Down));
    }

    #[test]
    fn runtime_pointer_touch_uses_one_primary_owner_and_cancel() {
        let mut state = InputDeviceState::new();
        state.apply_raw_event(&RawInputEvent::touch_start(
            1,
            "main-window",
            10,
            20.0,
            30.0,
        ));
        state.apply_raw_event(&RawInputEvent::touch_start(
            1,
            "main-window",
            11,
            80.0,
            90.0,
        ));
        state.apply_raw_event(&RawInputEvent::touch_move(1, "main-window", 11, 81.0, 91.0));
        state.apply_raw_event(&RawInputEvent::touch_cancel(
            1,
            "main-window",
            10,
            22.0,
            33.0,
        ));

        let frame = state.to_runtime_input_frame(1, "main-window");
        let pointers = frame
            .events
            .iter()
            .filter_map(RuntimeInputEvent::pointer_event)
            .collect::<Vec<_>>();

        assert_eq!(pointers.len(), 2);
        assert_eq!(pointers[0].pointer_id, 10);
        assert_eq!(pointers[0].phase, RuntimePointerPhase::Down);
        assert_eq!(pointers[1].pointer_id, 10);
        assert_eq!(pointers[1].phase, RuntimePointerPhase::Cancel);
        assert!(pointers.iter().all(|pointer| {
            pointer.device_kind == RuntimePointerDeviceKind::Touch && !pointer.hover_capable
        }));
    }

    #[test]
    fn runtime_pointer_touch_move_and_end_preserve_primary_identity() {
        let mut state = InputDeviceState::new();
        state.apply_raw_event(&RawInputEvent::touch_start(
            1,
            "main-window",
            12,
            20.0,
            30.0,
        ));
        state.apply_raw_event(&RawInputEvent::touch_move(1, "main-window", 12, 22.0, 33.0));
        state.apply_raw_event(&RawInputEvent::touch_end(1, "main-window", 12, 24.0, 36.0));

        let phases = state
            .to_runtime_input_frame(1, "main-window")
            .events
            .iter()
            .filter_map(RuntimeInputEvent::pointer_event)
            .map(|pointer| (pointer.pointer_id, pointer.phase, pointer.hover_capable))
            .collect::<Vec<_>>();
        assert_eq!(
            phases,
            vec![
                (12, RuntimePointerPhase::Down, false),
                (12, RuntimePointerPhase::Move, false),
                (12, RuntimePointerPhase::Up, false),
            ]
        );
    }

    #[test]
    fn runtime_pointer_mouse_leave_is_typed() {
        let mut state = InputDeviceState::new();
        state.apply_raw_event(&RawInputEvent::mouse_leave(1, "main-window", 100.0, 200.0));
        let pointer = state
            .to_runtime_input_frame(1, "main-window")
            .events
            .iter()
            .find_map(RuntimeInputEvent::pointer_event)
            .unwrap();
        assert_eq!(pointer.phase, RuntimePointerPhase::Leave);
        assert_eq!(pointer.device_kind, RuntimePointerDeviceKind::Mouse);
        assert!(pointer.hover_capable);
    }

    #[test]
    fn input_device_state_keeps_pointer_and_wheel() {
        let mut state = InputDeviceState::new();
        state.apply_raw_event(&RawInputEvent::mouse_move(1, "main-window", 10.0, 20.0));
        state.apply_raw_event(&RawInputEvent::mouse_wheel(1, "main-window", -2.0));

        let frame = state.to_runtime_input_frame(1, "main-window");
        let report = state.report();

        assert_eq!(
            frame.pointer_position,
            Some(PointerPosition { x: 10.0, y: 20.0 })
        );
        assert_eq!(report.mouse_wheel_delta, -2.0);
        assert!(frame.events.iter().any(
            |event| matches!(event, RuntimeInputEvent::MouseWheel { delta } if *delta == -2.0)
        ));
    }

    #[test]
    fn text_input_and_ime_events_enter_runtime_frame() {
        let mut state = InputDeviceState::new();
        state.apply_raw_event(&RawInputEvent::text_input(1, "main-window", "A"));
        state.apply_raw_event(&RawInputEvent::ime_preedit(1, "main-window", "ni", 0, 2));
        state.apply_raw_event(&RawInputEvent::ime_commit(1, "main-window", "nihao"));
        state.apply_raw_event(&RawInputEvent::ime_cancel(1, "main-window"));

        let frame = state.to_runtime_input_frame(1, "main-window");

        assert!(frame
            .events
            .iter()
            .any(|event| matches!(event, RuntimeInputEvent::TextInput { text } if text == "A")));
        assert!(frame.events.iter().any(|event| matches!(
            event,
            RuntimeInputEvent::ImePreedit {
                text,
                cursor_start: 0,
                cursor_end: 2,
            } if text == "ni"
        )));
        assert!(frame.events.iter().any(
            |event| matches!(event, RuntimeInputEvent::ImeCommit { text } if text == "nihao")
        ));
        assert!(frame
            .events
            .iter()
            .any(|event| matches!(event, RuntimeInputEvent::ImeCancel)));
    }

    #[test]
    fn modifiers_changed_populates_runtime_frame_modifiers() {
        let mut state = InputDeviceState::new();
        state.apply_raw_event(&RawInputEvent::modifiers_changed(
            1,
            "main-window",
            ["ShiftLeft", "ctrl", "Alt", "Command"],
        ));

        let frame = state.to_runtime_input_frame(1, "main-window");

        assert_eq!(
            frame.modifiers,
            vec![
                "Alt".to_string(),
                "Control".to_string(),
                "Logo".to_string(),
                "Shift".to_string()
            ]
        );
    }

    #[test]
    fn gamepad_events_enter_runtime_frame_and_clear_on_focus_lost() {
        let mut state = InputDeviceState::new();
        state.apply_raw_event(&RawInputEvent::gamepad_button_down(
            1,
            "main-window",
            0,
            "South",
        ));
        state.apply_raw_event(&RawInputEvent::gamepad_axis2d(
            1,
            "main-window",
            0,
            "LeftStick",
            0.25,
            -0.75,
        ));

        let first = state.to_runtime_input_frame(1, "main-window");
        assert!(first.events.iter().any(|event| matches!(
            event,
            RuntimeInputEvent::GamepadButtonDown {
                gamepad_id: 0,
                button,
            } if button == "South"
        )));
        assert!(first.events.iter().any(|event| matches!(
            event,
            RuntimeInputEvent::GamepadAxis2d {
                gamepad_id: 0,
                axis,
                x,
                y,
            } if axis == "LeftStick" && *x == 0.25 && *y == -0.75
        )));
        assert!(first.events.iter().any(|event| matches!(
            event,
            RuntimeInputEvent::GamepadButtonHeld {
                gamepad_id: 0,
                button,
            } if button == "South"
        )));

        state.begin_frame();
        state.apply_raw_event(&RawInputEvent::focus_lost(2, "main-window"));
        let second = state.to_runtime_input_frame(2, "main-window");
        assert!(second.events.iter().any(|event| matches!(
            event,
            RuntimeInputEvent::GamepadButtonUp {
                gamepad_id: 0,
                button,
            } if button == "South"
        )));
    }
}
