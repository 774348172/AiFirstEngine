use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PointerPosition {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Axis2 {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Axis1 {
    pub value: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionPhase {
    Pressed,
    Released,
    Held,
    Changed,
}

impl ActionPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pressed => "pressed",
            Self::Released => "released",
            Self::Held => "held",
            Self::Changed => "changed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ActionValue {
    Button { phase: ActionPhase },
    Axis1 { value: Axis1 },
    Axis2 { value: Axis2 },
    Pointer { position: PointerPosition },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputActionState {
    pub action_id: String,
    pub value: ActionValue,
}

impl InputActionState {
    pub fn button(action_id: impl Into<String>, phase: ActionPhase) -> Self {
        Self {
            action_id: action_id.into(),
            value: ActionValue::Button { phase },
        }
    }

    pub fn axis2(action_id: impl Into<String>, x: f32, y: f32) -> Self {
        Self {
            action_id: action_id.into(),
            value: ActionValue::Axis2 {
                value: Axis2 { x, y },
            },
        }
    }

    pub fn axis1(action_id: impl Into<String>, value: f32) -> Self {
        Self {
            action_id: action_id.into(),
            value: ActionValue::Axis1 {
                value: Axis1 { value },
            },
        }
    }

    pub fn pointer(action_id: impl Into<String>, x: f32, y: f32) -> Self {
        Self {
            action_id: action_id.into(),
            value: ActionValue::Pointer {
                position: PointerPosition { x, y },
            },
        }
    }

    pub fn is_pressed(&self) -> bool {
        matches!(
            self.value,
            ActionValue::Button {
                phase: ActionPhase::Pressed | ActionPhase::Held
            }
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ActionSnapshot {
    pub frame_id: u64,
    pub actions: Vec<InputActionState>,
}

impl ActionSnapshot {
    pub fn new(frame_id: u64) -> Self {
        Self {
            frame_id,
            actions: Vec::new(),
        }
    }

    pub fn with_actions(frame_id: u64, actions: Vec<InputActionState>) -> Self {
        Self { frame_id, actions }
    }

    pub fn action_ids(&self) -> Vec<String> {
        self.actions
            .iter()
            .map(|action| action.action_id.clone())
            .collect()
    }

    pub fn action_count(&self) -> usize {
        self.actions.len()
    }

    pub fn button_pressed(&self, action_id: &str) -> bool {
        self.actions
            .iter()
            .any(|action| action.action_id == action_id && action.is_pressed())
    }

    pub fn axis2(&self, action_id: &str) -> Option<Axis2> {
        self.actions.iter().find_map(|action| {
            if action.action_id == action_id {
                if let ActionValue::Axis2 { value } = action.value {
                    return Some(value);
                }
            }
            None
        })
    }

    pub fn axis1(&self, action_id: &str) -> Option<Axis1> {
        self.actions.iter().find_map(|action| {
            if action.action_id == action_id {
                if let ActionValue::Axis1 { value } = action.value {
                    return Some(value);
                }
            }
            None
        })
    }

    pub fn pointer(&self, action_id: &str) -> Option<PointerPosition> {
        self.actions.iter().find_map(|action| {
            if action.action_id == action_id {
                if let ActionValue::Pointer { position } = action.value {
                    return Some(position);
                }
            }
            None
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputTraceSummary {
    pub frame_id: u64,
    pub viewport_id: Option<String>,
    pub viewport_kind: Option<String>,
    pub route_kind: Option<String>,
    pub route_reason: Option<String>,
    pub action_count: usize,
    pub action_ids: Vec<String>,
}

impl InputTraceSummary {
    pub fn from_snapshot(snapshot: Option<&ActionSnapshot>) -> Self {
        let Some(snapshot) = snapshot else {
            return Self {
                frame_id: 0,
                viewport_id: None,
                viewport_kind: None,
                route_kind: None,
                route_reason: None,
                action_count: 0,
                action_ids: Vec::new(),
            };
        };
        Self {
            frame_id: snapshot.frame_id,
            viewport_id: None,
            viewport_kind: None,
            route_kind: None,
            route_reason: None,
            action_count: snapshot.action_count(),
            action_ids: snapshot.action_ids(),
        }
    }

    pub fn with_route(
        mut self,
        viewport_id: Option<String>,
        viewport_kind: Option<String>,
        route_kind: Option<String>,
        route_reason: Option<String>,
    ) -> Self {
        self.viewport_id = viewport_id;
        self.viewport_kind = viewport_kind;
        self.route_kind = route_kind;
        self.route_reason = route_reason;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_snapshot_reports_button_axis_and_pointer() {
        let snapshot = ActionSnapshot::with_actions(
            7,
            vec![
                InputActionState::button("action.fire", ActionPhase::Pressed),
                InputActionState::axis1("action.scroll", -1.0),
                InputActionState::axis2("action.move", 1.0, -1.0),
                InputActionState::pointer("action.pointer", 32.0, 64.0),
            ],
        );

        assert!(snapshot.button_pressed("action.fire"));
        assert_eq!(snapshot.axis1("action.scroll"), Some(Axis1 { value: -1.0 }));
        assert_eq!(
            snapshot.axis2("action.move"),
            Some(Axis2 { x: 1.0, y: -1.0 })
        );
        assert_eq!(
            snapshot.pointer("action.pointer"),
            Some(PointerPosition { x: 32.0, y: 64.0 })
        );
        assert_eq!(
            snapshot.action_ids(),
            vec![
                "action.fire",
                "action.scroll",
                "action.move",
                "action.pointer"
            ]
        );
    }

    #[test]
    fn input_trace_summary_can_attach_route_context() {
        let snapshot = ActionSnapshot::with_actions(
            3,
            vec![InputActionState::button(
                "action.fire",
                ActionPhase::Pressed,
            )],
        );

        let summary = InputTraceSummary::from_snapshot(Some(&snapshot)).with_route(
            Some("viewport-game".to_string()),
            Some("Game".to_string()),
            Some("RuntimeInputFrame".to_string()),
            Some("game_view_focused".to_string()),
        );

        assert_eq!(summary.frame_id, 3);
        assert_eq!(summary.action_count, 1);
        assert_eq!(summary.action_ids, vec!["action.fire"]);
        assert_eq!(summary.route_reason.as_deref(), Some("game_view_focused"));
    }
}
