use super::*;

impl AuiInteractionSystem {
    pub fn ime_platform_request(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        paragraphs: &HashMap<String, Arc<crate::aui_paragraph::ParagraphLayout>>,
        state: &AuiInteractionState,
    ) -> AuiImePlatformRequest {
        let Some(input) = state.input_field.as_ref() else {
            return AuiImePlatformRequest::default();
        };
        if state.focus.focused_node.as_deref() != Some(input.node_id.as_str())
            || !matches!(
                &state.input_mode,
                AuiInputMode::TextEditing { node_id } if node_id == &input.node_id
            )
            || !document.nodes.iter().any(|node| {
                node.node_id == input.node_id
                    && node.kind == AuiNodeKind::InputField
                    && !node.read_only
            })
        {
            return AuiImePlatformRequest::default();
        }
        let Some(computed) = layout
            .computed_nodes
            .iter()
            .find(|computed| computed.node_id == input.node_id && computed.effective_visible)
        else {
            return AuiImePlatformRequest::default();
        };
        let Some(paragraph) = paragraphs.get(&input.node_id) else {
            return AuiImePlatformRequest::default();
        };
        let local = paragraph.caret_rect(input.caret_index as u32).or_else(|| {
            paragraph
                .lines
                .last()
                .and_then(|line| paragraph.caret_rect(line.source_end))
        });
        let Some(local) = local else {
            return AuiImePlatformRequest::default();
        };
        AuiImePlatformRequest {
            enabled: true,
            node_id: Some(input.node_id.clone()),
            canvas_id: Some(computed.canvas_id.clone()),
            cursor_rect: Some(AuiComputedRect {
                x: computed.rect.x + local.x,
                y: computed.rect.y + local.y,
                width: local.width.max(1.0),
                height: local.height.max(1.0),
            }),
        }
    }

    pub(super) fn topmost_modal_root(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        state: &AuiInteractionState,
    ) -> Option<String> {
        document
            .canvases
            .iter()
            .filter(|canvas| {
                canvas.mode == AuiCanvasMode::ScreenOverlay
                    && canvas.composition_stage == AuiCompositionStage::Modal
                    && Self::canvas_effective_visible(state, canvas)
                    && Self::node_effectively_visible(layout, canvas.root_node.as_str())
            })
            .max_by_key(|canvas| (canvas.layer, canvas.sorting_order))
            .map(|canvas| canvas.root_node.clone())
    }

    pub(super) fn node_effectively_visible(layout: &AuiLayoutResult, node_id: &str) -> bool {
        layout
            .computed_nodes
            .iter()
            .find(|computed| computed.node_id == node_id)
            .is_some_and(|computed| computed.effective_visible)
    }

    pub(super) fn reconcile_effectively_hidden_state(
        layout: &AuiLayoutResult,
        state: &mut AuiInteractionState,
        result: &mut AuiInteractionResult,
    ) {
        let hidden = |node_id: &str| !Self::node_effectively_visible(layout, node_id);

        if state.focus.focused_node.as_deref().is_some_and(hidden) {
            state.focus.focused_node = None;
            state.focus.focus_reason = AuiFocusReason::Cleared;
            result.focus_change_count += 1;
            result.visibility_reconciliation_count += 1;
        }
        if state.focus.focus_scope_root.as_deref().is_some_and(hidden) {
            state.focus.focus_scope_root = None;
            result.visibility_reconciliation_count += 1;
        }
        if state
            .input_field
            .as_ref()
            .is_some_and(|input| hidden(input.node_id.as_str()))
        {
            if state
                .input_field
                .as_ref()
                .and_then(|input| input.composition.as_ref())
                .is_some()
            {
                result.ime_cancel_count += 1;
            }
            state.input_field = None;
            state.input_mode = AuiInputMode::Navigation;
            result.visibility_reconciliation_count += 1;
        }
        if matches!(
            &state.input_mode,
            AuiInputMode::ModalBlocking { modal_root } if hidden(modal_root)
        ) {
            state.input_mode = AuiInputMode::Navigation;
            result.visibility_reconciliation_count += 1;
        }
        if state.active_modal_root.as_deref().is_some_and(hidden) {
            state.active_modal_root = None;
            result.visibility_reconciliation_count += 1;
        }
    }

    pub(super) fn node_in_subtree(
        nodes_by_id: &AuiNodeLookup<'_>,
        root_node: &str,
        node_id: &str,
    ) -> bool {
        if root_node == node_id {
            return true;
        }
        let mut current = node_id;
        let mut seen = HashSet::new();
        while seen.insert(current) {
            let Some(node) = nodes_by_id.get(current) else {
                return false;
            };
            let Some(parent) = node.parent.as_deref() else {
                return false;
            };
            if parent == root_node {
                return true;
            }
            current = parent;
        }
        false
    }

    pub(super) fn reconcile_session_identity(
        state: &mut AuiInteractionState,
        result: &mut AuiInteractionResult,
        session_id: Option<&str>,
    ) {
        let Some(session_id) = session_id else {
            return;
        };
        if state.interaction_session_id.as_deref() == Some(session_id) {
            return;
        }
        result.control_reconciliation_count += state.clear_control_transients();
        result.control_reconciliation_count +=
            state.toggle_values.len() + state.slider_values.len();
        state.toggle_values.clear();
        state.slider_values.clear();
        state.interaction_session_id = Some(session_id.to_string());
    }

    pub(super) fn reconcile_control_values(
        nodes_by_id: &AuiNodeLookup<'_>,
        state: &mut AuiInteractionState,
        result: &mut AuiInteractionResult,
    ) {
        state.toggle_values.retain(|node_id, _| {
            nodes_by_id.get(node_id.as_str()).is_some_and(|node| {
                node.kind == AuiNodeKind::Toggle && node.toggle_checked.is_some()
            })
        });
        state.slider_values.retain(|node_id, _| {
            nodes_by_id
                .get(node_id.as_str())
                .is_some_and(|node| node.kind == AuiNodeKind::Slider && node.slider_value.is_some())
        });

        for node in nodes_by_id.control_values() {
            if node.kind == AuiNodeKind::Toggle {
                if let Some(source_value) = node.toggle_checked {
                    match state.toggle_values.entry(node.node_id.clone()) {
                        std::collections::btree_map::Entry::Vacant(entry) => {
                            entry.insert(AuiToggleValueState {
                                source_value,
                                value: source_value,
                            });
                        }
                        std::collections::btree_map::Entry::Occupied(mut entry)
                            if entry.get().source_value != source_value =>
                        {
                            entry.insert(AuiToggleValueState {
                                source_value,
                                value: source_value,
                            });
                            result.control_reconciliation_count += 1;
                        }
                        _ => {}
                    }
                }
            } else if node.kind == AuiNodeKind::Slider {
                if let Some(source_value) = node.slider_value {
                    let source_value = normalize_slider_value(node, source_value);
                    match state.slider_values.entry(node.node_id.clone()) {
                        std::collections::btree_map::Entry::Vacant(entry) => {
                            entry.insert(AuiSliderValueState {
                                source_value,
                                value: source_value,
                            });
                        }
                        std::collections::btree_map::Entry::Occupied(mut entry)
                            if entry.get().source_value.to_bits() != source_value.to_bits() =>
                        {
                            entry.insert(AuiSliderValueState {
                                source_value,
                                value: source_value,
                            });
                            result.control_reconciliation_count += 1;
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    pub(super) fn append_toggle_value_changes_for_activation(
        nodes_by_id: &AuiNodeLookup<'_>,
        state: &mut AuiInteractionState,
        commands: &mut Vec<AuiCommand>,
        base_command_count: usize,
    ) {
        let toggle_nodes = commands
            .iter()
            .filter(|command| {
                matches!(
                    command.command_kind,
                    AuiCommandKind::Click | AuiCommandKind::Submit
                )
            })
            .filter_map(|command| {
                nodes_by_id
                    .get(command.source_node.as_str())
                    .filter(|node| node.kind == AuiNodeKind::Toggle)
                    .map(|node| node.node_id.clone())
            })
            .collect::<Vec<_>>();
        for node_id in toggle_nodes {
            let Some(value_state) = state.toggle_values.get_mut(&node_id) else {
                continue;
            };
            value_state.value = !value_state.value;
            let payload = AuiControlValuePayload::new(
                node_id.clone(),
                AuiControlValue::Bool(value_state.value),
            )
            .to_payload_string();
            commands.push(
                AuiCommand::new(
                    base_command_count + commands.len(),
                    node_id,
                    AuiCommandKind::ValueChanged,
                )
                .with_payload(payload),
            );
        }
    }

    pub(super) fn append_slider_value_change(
        node: &AuiNode,
        state: &mut AuiInteractionState,
        commands: &mut Vec<AuiCommand>,
        base_command_count: usize,
        requested_value: f32,
    ) -> bool {
        let value = normalize_slider_value(node, requested_value);
        let Some(value_state) = state.slider_values.get_mut(&node.node_id) else {
            return false;
        };
        if value_state.value.to_bits() == value.to_bits() {
            return true;
        }
        value_state.value = value;
        let payload =
            AuiControlValuePayload::new(node.node_id.clone(), AuiControlValue::Number(value))
                .to_payload_string();
        commands.push(
            AuiCommand::new(
                base_command_count + commands.len(),
                node.node_id.clone(),
                AuiCommandKind::ValueChanged,
            )
            .with_payload(payload),
        );
        true
    }

    pub(super) fn append_slider_pointer_value_change(
        nodes_by_id: &AuiNodeLookup<'_>,
        layout: &AuiLayoutResult,
        state: &mut AuiInteractionState,
        commands: &mut Vec<AuiCommand>,
        base_command_count: usize,
        node_id: Option<&str>,
        pointer: AuiPointer,
    ) -> bool {
        let Some(node) = node_id
            .and_then(|node_id| nodes_by_id.get(node_id))
            .filter(|node| node.kind == AuiNodeKind::Slider)
        else {
            return false;
        };
        let Some(computed) = layout
            .computed_nodes
            .iter()
            .find(|n| n.node_id == node.node_id)
        else {
            return false;
        };
        let rect = &computed.rect;
        if rect.width <= f32::EPSILON {
            return false;
        }
        let (min, max, _) = slider_range(node);
        let Some(local) = computed.presented_point_to_local(pointer) else {
            return false;
        };
        let ratio = (local[0] / rect.width).clamp(0.0, 1.0);
        Self::append_slider_value_change(
            node,
            state,
            commands,
            base_command_count,
            min + ratio * (max - min),
        )
    }

    pub(super) fn append_slider_key_value_change(
        nodes_by_id: &AuiNodeLookup<'_>,
        state: &mut AuiInteractionState,
        commands: &mut Vec<AuiCommand>,
        base_command_count: usize,
        node_id: Option<&str>,
        key: &str,
    ) -> bool {
        let direction = if key.eq_ignore_ascii_case("ArrowLeft") || key.eq_ignore_ascii_case("Left")
        {
            -1.0
        } else if key.eq_ignore_ascii_case("ArrowRight") || key.eq_ignore_ascii_case("Right") {
            1.0
        } else {
            return false;
        };
        let Some(node) = node_id
            .and_then(|node_id| nodes_by_id.get(node_id))
            .filter(|node| node.kind == AuiNodeKind::Slider)
        else {
            return false;
        };
        let Some(current) = state
            .slider_values
            .get(&node.node_id)
            .map(|value| value.value)
        else {
            return false;
        };
        let (_, _, step) = slider_range(node);
        Self::append_slider_value_change(
            node,
            state,
            commands,
            base_command_count,
            current + direction * step,
        )
    }

    pub(super) fn control_node_status(
        nodes_by_id: &AuiNodeLookup<'_>,
        layout: &AuiLayoutResult,
        node_id: &str,
        scope_root: Option<&str>,
        require_interactable: bool,
    ) -> (bool, bool) {
        let hidden = !Self::node_effectively_visible(layout, node_id);
        let valid = nodes_by_id.get(node_id).is_some_and(|node| {
            !hidden
                && (!require_interactable || node.interactable)
                && scope_root
                    .map(|root| Self::node_in_subtree(nodes_by_id, root, node_id))
                    .unwrap_or(true)
        });
        (valid, hidden)
    }

    pub(super) fn reconcile_control_state(
        nodes_by_id: &AuiNodeLookup<'_>,
        layout: &AuiLayoutResult,
        state: &mut AuiInteractionState,
        result: &mut AuiInteractionResult,
        active_modal_root: Option<&str>,
    ) {
        let active_screen_root = state
            .screen_stack
            .active_stack
            .last()
            .map(|entry| entry.root_node_id.as_str());
        let scope_root = active_modal_root.or(active_screen_root);

        if let Some(node_id) = state.hovered_node.as_deref() {
            let (valid, hidden) =
                Self::control_node_status(nodes_by_id, layout, node_id, scope_root, true);
            if !valid {
                state.hovered_node = None;
                result.control_reconciliation_count += 1;
                result.visibility_reconciliation_count += usize::from(hidden);
            }
        }
        if let Some(node_id) = state
            .primary_press
            .as_ref()
            .map(|press| press.node_id.as_str())
        {
            let (valid, hidden) =
                Self::control_node_status(nodes_by_id, layout, node_id, scope_root, true);
            if !valid {
                state.primary_press = None;
                result.control_reconciliation_count += 1;
                result.visibility_reconciliation_count += usize::from(hidden);
            }
        }
        if let Some(node_id) = state
            .active_drag
            .as_ref()
            .map(|drag| drag.source_node.as_str())
        {
            let (valid, hidden) =
                Self::control_node_status(nodes_by_id, layout, node_id, scope_root, true);
            if !valid {
                state.active_drag = None;
                result.control_reconciliation_count += 1;
                result.visibility_reconciliation_count += usize::from(hidden);
            }
        }
        if let Some(node_id) = state
            .active_scroll_capture
            .as_ref()
            .map(|capture| capture.node_id.as_str())
        {
            let (valid, hidden) =
                Self::control_node_status(nodes_by_id, layout, node_id, scope_root, false);
            if !valid {
                state.active_scroll_capture = None;
                result.control_reconciliation_count += 1;
                result.visibility_reconciliation_count += usize::from(hidden);
            }
        }
    }

    pub(super) fn control_snapshot(
        frame_id: u64,
        state: &AuiInteractionState,
    ) -> AuiControlInteractionSnapshot {
        let primary_press = state.primary_press.as_ref();
        AuiControlInteractionSnapshot {
            frame_id,
            session_id: state.interaction_session_id.clone(),
            hovered_node: state.hovered_node.clone(),
            pressed_node: primary_press.map(|press| press.node_id.clone()),
            pressed_inside: primary_press.is_some_and(|press| press.inside),
            pointer_id: primary_press.map(|press| press.pointer_id),
            pointer_device_kind: primary_press.map(|press| press.device_kind),
            pointer_hover_capable: primary_press.is_some_and(|press| press.hover_capable),
            focused_node: state.focus.focused_node.clone(),
            focus_visible: state.focus.focused_node.is_some()
                && state.focus.focus_reason != AuiFocusReason::Pointer,
            active_modal_root: state.active_modal_root.clone(),
            active_screen_id: state
                .screen_stack
                .active_stack
                .last()
                .map(|entry| entry.screen_id.clone()),
        }
    }

    pub(super) fn find_scroll_containers(
        nodes_by_id: &AuiNodeLookup<'_>,
        hit_node: Option<&str>,
    ) -> Vec<String> {
        let Some(mut current) = hit_node else {
            return Vec::new();
        };
        let mut seen = HashSet::new();
        let mut containers = Vec::new();
        while seen.insert(current) {
            let Some(node) = nodes_by_id.get(current) else {
                break;
            };
            if matches!(node.kind, AuiNodeKind::ScrollView | AuiNodeKind::List) {
                containers.push(node.node_id.clone());
            }
            let Some(parent) = node.parent.as_deref() else {
                break;
            };
            current = parent;
        }
        containers
    }

    pub(super) fn find_scroll_container(
        nodes_by_id: &AuiNodeLookup<'_>,
        hit_node: Option<&str>,
    ) -> Option<String> {
        Self::find_scroll_containers(nodes_by_id, hit_node)
            .into_iter()
            .next()
    }

    pub(super) fn computed_rect_for<'a>(
        layout: &'a AuiLayoutResult,
        node_id: &str,
    ) -> Option<&'a AuiComputedRect> {
        layout
            .computed_nodes
            .iter()
            .find(|computed| computed.node_id == node_id)
            .map(|computed| &computed.rect)
    }

    pub(super) fn estimate_scroll_max_offset_y(
        nodes_by_id: &AuiNodeLookup<'_>,
        layout: &AuiLayoutResult,
        scroll_node: &str,
    ) -> f32 {
        let Some(viewport) = Self::computed_rect_for(layout, scroll_node) else {
            return 0.0;
        };
        let mut max_bottom = viewport.y + viewport.height;
        for computed in &layout.computed_nodes {
            if computed.node_id == scroll_node {
                continue;
            }
            if Self::node_in_subtree(nodes_by_id, scroll_node, computed.node_id.as_str()) {
                max_bottom = max_bottom.max(computed.rect.y + computed.rect.height);
            }
        }
        (max_bottom - (viewport.y + viewport.height)).max(0.0)
    }

    pub(super) fn update_scroll_offset(
        nodes_by_id: &AuiNodeLookup<'_>,
        layout: &AuiLayoutResult,
        state: &mut AuiInteractionState,
        scroll_node: &str,
        delta_y: f32,
    ) -> Option<AuiScrollPayload> {
        Self::apply_scroll_delta(nodes_by_id, layout, state, scroll_node, delta_y)
            .map(|(payload, _)| payload)
    }

    pub(super) fn apply_scroll_delta(
        nodes_by_id: &AuiNodeLookup<'_>,
        layout: &AuiLayoutResult,
        state: &mut AuiInteractionState,
        scroll_node: &str,
        delta_y: f32,
    ) -> Option<(AuiScrollPayload, f32)> {
        let max_offset_y = Self::estimate_scroll_max_offset_y(nodes_by_id, layout, scroll_node);
        let scroll_state = state
            .scroll_offsets
            .entry(scroll_node.to_string())
            .or_insert_with(|| AuiScrollState::new(scroll_node, max_offset_y));
        scroll_state.max_offset_y = max_offset_y;
        let applied_delta_y = scroll_state.apply_delta(delta_y);
        if applied_delta_y.abs() <= f32::EPSILON {
            return None;
        }
        Some((
            AuiScrollPayload::new(
                scroll_node,
                scroll_state.offset_y,
                scroll_state.max_offset_y,
                applied_delta_y,
                "runtime_input",
            ),
            applied_delta_y,
        ))
    }

    pub(super) fn apply_nested_scroll_delta(
        nodes_by_id: &AuiNodeLookup<'_>,
        layout: &AuiLayoutResult,
        state: &mut AuiInteractionState,
        scroll_nodes: &[String],
        delta_y: f32,
    ) -> Vec<(String, AuiScrollPayload)> {
        let mut remaining = delta_y;
        let mut applied = Vec::new();
        for scroll_node in scroll_nodes {
            if remaining.abs() <= f32::EPSILON {
                break;
            }
            if let Some((payload, applied_delta)) =
                Self::apply_scroll_delta(nodes_by_id, layout, state, scroll_node, remaining)
            {
                remaining -= applied_delta;
                applied.push((scroll_node.clone(), payload));
            }
        }
        applied
    }

    pub(super) fn focusable_nodes_in_scope(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        nodes_by_id: &AuiNodeLookup<'_>,
        scope_root: &str,
    ) -> Vec<String> {
        layout
            .computed_nodes
            .iter()
            .filter(|computed| computed.effective_visible)
            .filter_map(|computed| {
                let node = nodes_by_id.get(computed.node_id.as_str())?;
                let focusable = node.focusable.unwrap_or(node.interactable);
                if focusable
                    && computed.effective_visible
                    && Self::node_in_subtree(nodes_by_id, scope_root, node.node_id.as_str())
                    && document
                        .nodes
                        .iter()
                        .any(|candidate| candidate.node_id == node.node_id)
                {
                    Some(node.node_id.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    pub(super) fn focus_scope_root(
        document: &AuiDocument,
        nodes_by_id: &AuiNodeLookup<'_>,
        state: &AuiInteractionState,
    ) -> Option<String> {
        if let Some(scope_root) = &state.focus.focus_scope_root {
            return Some(scope_root.clone());
        }
        if let Some(focused_node) = &state.focus.focused_node {
            if let Some(canvas) = document.canvases.iter().find(|canvas| {
                Self::node_in_subtree(nodes_by_id, canvas.root_node.as_str(), focused_node)
            }) {
                return Some(canvas.root_node.clone());
            }
        }
        document
            .canvases
            .iter()
            .filter(|canvas| Self::canvas_effective_visible(state, canvas))
            .max_by_key(|canvas| (canvas.layer, canvas.sorting_order))
            .map(|canvas| canvas.root_node.clone())
    }

    pub(super) fn apply_default_focus_if_needed(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        nodes_by_id: &AuiNodeLookup<'_>,
        state: &mut AuiInteractionState,
    ) -> bool {
        if state.focus.focused_node.is_some() {
            return false;
        }
        let Some(canvas) = document
            .canvases
            .iter()
            .filter(|canvas| Self::canvas_effective_visible(state, canvas))
            .filter(|canvas| canvas.default_focus_node_id.is_some())
            .max_by_key(|canvas| (canvas.layer, canvas.sorting_order))
        else {
            return false;
        };
        let Some(default_focus) = canvas.default_focus_node_id.as_deref() else {
            return false;
        };
        let focusable = Self::focusable_nodes_in_scope(
            document,
            layout,
            nodes_by_id,
            canvas.root_node.as_str(),
        );
        if !focusable.iter().any(|node| node == default_focus) {
            return false;
        }
        state.focus.focused_node = Some(default_focus.to_string());
        state.focus.focus_scope_root = Some(canvas.root_node.clone());
        state.focus.focus_reason = AuiFocusReason::Keyboard;
        state.screen_stack.default_focus_applied_count += 1;
        true
    }

    pub(super) fn navigation_direction_for_key(
        key: &str,
        reverse: bool,
    ) -> Option<AuiNavigationDirection> {
        if key.eq_ignore_ascii_case("Tab") {
            return Some(if reverse {
                AuiNavigationDirection::Previous
            } else {
                AuiNavigationDirection::Next
            });
        }
        if key.eq_ignore_ascii_case("ArrowUp") || key.eq_ignore_ascii_case("Up") {
            return Some(AuiNavigationDirection::Up);
        }
        if key.eq_ignore_ascii_case("ArrowDown") || key.eq_ignore_ascii_case("Down") {
            return Some(AuiNavigationDirection::Down);
        }
        if key.eq_ignore_ascii_case("ArrowLeft") || key.eq_ignore_ascii_case("Left") {
            return Some(AuiNavigationDirection::Left);
        }
        if key.eq_ignore_ascii_case("ArrowRight") || key.eq_ignore_ascii_case("Right") {
            return Some(AuiNavigationDirection::Right);
        }
        None
    }

    pub(super) fn explicit_navigation_target<'a>(
        node: &'a AuiNode,
        direction: AuiNavigationDirection,
    ) -> Option<&'a str> {
        match direction {
            AuiNavigationDirection::Next => node.navigation.next.as_deref(),
            AuiNavigationDirection::Previous => node.navigation.previous.as_deref(),
            AuiNavigationDirection::Up => node.navigation.up.as_deref(),
            AuiNavigationDirection::Down => node.navigation.down.as_deref(),
            AuiNavigationDirection::Left => node.navigation.left.as_deref(),
            AuiNavigationDirection::Right => node.navigation.right.as_deref(),
        }
    }

    pub(super) fn navigation_mode_allows(
        mode: AuiNavigationMode,
        direction: AuiNavigationDirection,
    ) -> bool {
        match mode {
            AuiNavigationMode::None => matches!(
                direction,
                AuiNavigationDirection::Next | AuiNavigationDirection::Previous
            ),
            AuiNavigationMode::Auto | AuiNavigationMode::Explicit => true,
            AuiNavigationMode::Vertical => matches!(
                direction,
                AuiNavigationDirection::Next
                    | AuiNavigationDirection::Previous
                    | AuiNavigationDirection::Up
                    | AuiNavigationDirection::Down
            ),
            AuiNavigationMode::Horizontal => matches!(
                direction,
                AuiNavigationDirection::Next
                    | AuiNavigationDirection::Previous
                    | AuiNavigationDirection::Left
                    | AuiNavigationDirection::Right
            ),
        }
    }

    pub(super) fn computed_node_for<'a>(
        layout: &'a AuiLayoutResult,
        node_id: &str,
    ) -> Option<&'a AuiComputedNode> {
        layout
            .computed_nodes
            .iter()
            .find(|computed| computed.node_id == node_id)
    }

    pub(super) fn choose_directional_focus_candidate(
        layout: &AuiLayoutResult,
        focusable: &[String],
        current: &str,
        direction: AuiNavigationDirection,
    ) -> Option<String> {
        let current_computed = Self::computed_node_for(layout, current)?;
        let current_center_x = current_computed.rect.x + current_computed.rect.width * 0.5;
        let current_center_y = current_computed.rect.y + current_computed.rect.height * 0.5;
        focusable
            .iter()
            .filter(|candidate| candidate.as_str() != current)
            .filter_map(|candidate| {
                let computed = Self::computed_node_for(layout, candidate)?;
                let center_x = computed.rect.x + computed.rect.width * 0.5;
                let center_y = computed.rect.y + computed.rect.height * 0.5;
                let epsilon = 0.001;
                let (primary, secondary) = match direction {
                    AuiNavigationDirection::Down if center_y > current_center_y + epsilon => (
                        center_y - current_center_y,
                        (center_x - current_center_x).abs(),
                    ),
                    AuiNavigationDirection::Up if center_y < current_center_y - epsilon => (
                        current_center_y - center_y,
                        (center_x - current_center_x).abs(),
                    ),
                    AuiNavigationDirection::Right if center_x > current_center_x + epsilon => (
                        center_x - current_center_x,
                        (center_y - current_center_y).abs(),
                    ),
                    AuiNavigationDirection::Left if center_x < current_center_x - epsilon => (
                        current_center_x - center_x,
                        (center_y - current_center_y).abs(),
                    ),
                    _ => return None,
                };
                Some((candidate.clone(), primary, secondary, computed.tree_order))
            })
            .min_by(|a, b| {
                a.1.partial_cmp(&b.1)
                    .unwrap_or(Ordering::Equal)
                    .then_with(|| a.2.partial_cmp(&b.2).unwrap_or(Ordering::Equal))
                    .then_with(|| a.3.cmp(&b.3))
            })
            .map(|(node_id, _, _, _)| node_id)
    }

    pub(super) fn move_focus(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        nodes_by_id: &AuiNodeLookup<'_>,
        state: &mut AuiInteractionState,
        direction: AuiNavigationDirection,
    ) -> Option<(Option<String>, String)> {
        let scope_root = Self::focus_scope_root(document, nodes_by_id, state)?;
        let focusable = Self::focusable_nodes_in_scope(document, layout, nodes_by_id, &scope_root);
        if focusable.is_empty() {
            return None;
        }
        let previous = state.focus.focused_node.clone();
        let current_index = previous
            .as_ref()
            .and_then(|node| focusable.iter().position(|candidate| candidate == node));
        let explicit_target = previous.as_ref().and_then(|node_id| {
            let node = nodes_by_id.get(node_id.as_str())?;
            let target = Self::explicit_navigation_target(node, direction)?;
            focusable
                .iter()
                .any(|candidate| candidate == target)
                .then(|| target.to_string())
        });
        let next = if let Some(target) = explicit_target {
            target
        } else {
            if let Some(current) = previous.as_ref() {
                if let Some(node) = nodes_by_id.get(current.as_str()) {
                    if !Self::navigation_mode_allows(node.navigation.mode, direction) {
                        return None;
                    }
                }
            }
            match direction {
                AuiNavigationDirection::Next | AuiNavigationDirection::Previous => {
                    let reverse = direction == AuiNavigationDirection::Previous;
                    let next_index = match (current_index, reverse) {
                        (Some(index), false) => (index + 1) % focusable.len(),
                        (Some(0), true) => focusable.len() - 1,
                        (Some(index), true) => index - 1,
                        (None, false) => 0,
                        (None, true) => focusable.len() - 1,
                    };
                    focusable[next_index].clone()
                }
                AuiNavigationDirection::Up
                | AuiNavigationDirection::Down
                | AuiNavigationDirection::Left
                | AuiNavigationDirection::Right => {
                    if let Some(current) = previous.as_ref() {
                        Self::choose_directional_focus_candidate(
                            layout, &focusable, current, direction,
                        )?
                    } else {
                        focusable[0].clone()
                    }
                }
            }
        };
        if previous.as_deref() == Some(next.as_str()) {
            return None;
        }
        state.focus.focused_node = Some(next.clone());
        state.focus.focus_reason = AuiFocusReason::Keyboard;
        Some((previous, next))
    }

    pub(super) fn scroll_container_for_node(
        nodes_by_id: &AuiNodeLookup<'_>,
        node_id: &str,
    ) -> Option<String> {
        let mut current = node_id;
        let mut seen = HashSet::new();
        while seen.insert(current) {
            let node = nodes_by_id.get(current)?;
            if matches!(node.kind, AuiNodeKind::ScrollView | AuiNodeKind::List) {
                return Some(node.node_id.clone());
            }
            current = node.parent.as_deref()?;
        }
        None
    }

    pub(super) fn ensure_focused_node_visible(
        nodes_by_id: &AuiNodeLookup<'_>,
        layout: &AuiLayoutResult,
        state: &mut AuiInteractionState,
        focused_node: &str,
    ) -> Option<AuiScrollPayload> {
        let scroll_node = Self::scroll_container_for_node(nodes_by_id, focused_node)?;
        if scroll_node == focused_node {
            return None;
        }
        let viewport = *Self::computed_rect_for(layout, &scroll_node)?;
        let focused_rect = Self::computed_rect_for(layout, focused_node)?;
        let delta_y = if focused_rect.y < viewport.y {
            focused_rect.y - viewport.y
        } else if focused_rect.y + focused_rect.height > viewport.y + viewport.height {
            focused_rect.y + focused_rect.height - (viewport.y + viewport.height)
        } else {
            return None;
        };
        Self::update_scroll_offset(nodes_by_id, layout, state, &scroll_node, delta_y)
    }

    pub(super) fn scrollbar_thumb_at_pointer<'a>(
        layout: &'a AuiLayoutResult,
        pointer: AuiPointer,
    ) -> Option<&'a AuiScrollbarMetrics> {
        layout.scrollbar_metrics.iter().rev().find(|metrics| {
            metrics.visible
                && layout
                    .computed_nodes
                    .iter()
                    .find(|n| n.node_id == metrics.scroll_node_id)
                    .is_some_and(|node| {
                        node.effective_visible
                            && node.effective_clips.iter().all(|c| c.contains(pointer))
                            && node.presented_point_to_local(pointer).is_some_and(|p| {
                                metrics
                                    .thumb_rect
                                    .contains(p[0] + node.rect.x, p[1] + node.rect.y)
                            })
                    })
        })
    }

    pub fn canvas_effective_visible(state: &AuiInteractionState, canvas: &AuiCanvas) -> bool {
        state
            .canvas_visibility_overrides
            .get(canvas.canvas_id.as_str())
            .copied()
            .unwrap_or(canvas.visible)
    }

    pub fn push_screen(
        document: &AuiDocument,
        state: &mut AuiInteractionState,
        screen_id: &str,
    ) -> Option<AuiScreenStackEntry> {
        let canvas = document.canvases.iter().find(|canvas| {
            canvas.screen_id.as_deref() == Some(screen_id) || canvas.canvas_id == screen_id
        })?;
        state.queue_control_reconciliation();
        let previous_focus_node_id = state.focus.focused_node.clone();
        let default_focus = canvas
            .default_focus_node_id
            .clone()
            .or_else(|| Some(canvas.root_node.clone()));
        state
            .canvas_visibility_overrides
            .insert(canvas.canvas_id.clone(), true);
        state.focus.focused_node = default_focus.clone();
        state.focus.focus_scope_root = Some(canvas.root_node.clone());
        state.focus.focus_reason = AuiFocusReason::Keyboard;
        let default_focus_applied = default_focus.is_some();
        let entry = AuiScreenStackEntry {
            screen_id: canvas
                .screen_id
                .clone()
                .unwrap_or_else(|| canvas.canvas_id.clone()),
            document_path: None,
            canvas_id: canvas.canvas_id.clone(),
            root_node_id: canvas.root_node.clone(),
            default_focus_node_id: default_focus,
            previous_focus_node_id,
            modal: canvas.composition_stage == AuiCompositionStage::Modal,
            can_cancel: canvas.cancel_action_id.is_some()
                || canvas.composition_stage == AuiCompositionStage::Modal,
        };
        state.screen_stack.push_count += 1;
        state.screen_stack.default_focus_applied_count += usize::from(default_focus_applied);
        state.screen_stack.active_stack.push(entry.clone());
        Some(entry)
    }

    pub fn pop_screen(state: &mut AuiInteractionState) -> Option<AuiScreenStackEntry> {
        let entry = state.screen_stack.active_stack.pop()?;
        state.queue_control_reconciliation();
        state
            .canvas_visibility_overrides
            .insert(entry.canvas_id.clone(), false);
        state.screen_stack.last_popped_screen_id = Some(entry.screen_id.clone());
        state.focus.focused_node = entry.previous_focus_node_id.clone();
        state.focus.focus_scope_root = state
            .screen_stack
            .active_stack
            .last()
            .map(|entry| entry.root_node_id.clone());
        state.focus.focus_reason = if state.focus.focused_node.is_some() {
            AuiFocusReason::Keyboard
        } else {
            AuiFocusReason::Cleared
        };
        state.screen_stack.focus_restore_count += 1;
        Some(entry)
    }

    pub fn hit_test(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        x: f32,
        y: f32,
    ) -> AuiHitTestResult {
        Self::hit_test_with_presentation(document, layout, x, y, None)
    }

    pub fn hit_test_target_space(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        x: f32,
        y: f32,
        presentation: &ResolvedGameViewPresentation,
    ) -> AuiHitTestResult {
        Self::hit_test_with_presentation(document, layout, x, y, Some(presentation))
    }

    pub(super) fn hit_test_with_presentation(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        x: f32,
        y: f32,
        presentation: Option<&ResolvedGameViewPresentation>,
    ) -> AuiHitTestResult {
        let nodes_by_id = AuiNodeLookup::build(document);
        Self::hit_test_indexed(&nodes_by_id, layout, x, y, presentation)
    }

    // The input batch already owns this lookup table. Each query reads current
    // geometry without rebuilding document indices or retaining stale node data.
    pub(super) fn hit_test_indexed(
        nodes_by_id: &AuiNodeLookup<'_>,
        layout: &AuiLayoutResult,
        x: f32,
        y: f32,
        presentation: Option<&ResolvedGameViewPresentation>,
    ) -> AuiHitTestResult {
        let pointer = AuiPointer::new(x, y);
        let candidates = layout.computed_nodes.iter().rev().filter_map(|computed| {
            let candidate_pointer = match presentation {
                Some(presentation) => {
                    let Ok(point) = presentation
                        .target_to_reference(computed.canvas_id.as_str(), GameViewPoint::new(x, y))
                    else {
                        return None;
                    };
                    AuiPointer::new(point.x, point.y)
                }
                None => pointer,
            };
            Some((computed, candidate_pointer))
        });
        Self::hit_test_candidates(nodes_by_id, pointer, candidates)
    }

    pub(super) fn hit_test_spatial(
        nodes_by_id: &AuiNodeLookup<'_>,
        layout: &AuiLayoutResult,
        x: f32,
        y: f32,
        presentation: Option<&ResolvedGameViewPresentation>,
        index: &mut AuiInteractionIndexRuntime,
    ) -> AuiHitTestResult {
        let pointer = AuiPointer::new(x, y);
        let candidates =
            index
                .candidates(x, y, presentation)
                .iter()
                .filter_map(|(slot, candidate_pointer)| {
                    layout
                        .computed_nodes
                        .get(*slot)
                        .map(|computed| (computed, *candidate_pointer))
                });
        Self::hit_test_candidates(nodes_by_id, pointer, candidates)
    }

    pub(super) fn hit_test_candidates<'a>(
        nodes_by_id: &AuiNodeLookup<'_>,
        pointer: AuiPointer,
        candidates: impl Iterator<Item = (&'a AuiComputedNode, AuiPointer)>,
    ) -> AuiHitTestResult {
        let mut blocked_by_non_interactable = None;
        let mut clip_rejected_count = 0;

        for (computed, candidate_pointer) in candidates {
            if computed.clipped_by_node.is_some() {
                match computed.effective_clip_rect {
                    Some(clip_rect)
                        if !clip_rect.contains(candidate_pointer.x, candidate_pointer.y) =>
                    {
                        clip_rejected_count += 1;
                        continue;
                    }
                    None => {
                        clip_rejected_count += 1;
                        continue;
                    }
                    _ => {}
                }
            }
            if !computed
                .effective_clips
                .iter()
                .all(|clip| clip.contains(candidate_pointer))
            {
                clip_rejected_count += 1;
                continue;
            }
            if !computed.effective_visible || !computed.contains_presented_point(candidate_pointer)
            {
                continue;
            }
            let Some(node) = nodes_by_id.get(computed.node_id.as_str()) else {
                continue;
            };
            if node.interactable {
                return AuiHitTestResult {
                    pointer: candidate_pointer,
                    hit_node: Some(node.node_id.clone()),
                    consumed: node.consume_input,
                    reason: AuiHitTestReason::HitInteractable,
                    clip_rejected_count,
                };
            }
            blocked_by_non_interactable
                .get_or_insert_with(|| (node.node_id.clone(), candidate_pointer));
        }

        if let Some((node_id, candidate_pointer)) = blocked_by_non_interactable {
            return AuiHitTestResult {
                pointer: candidate_pointer,
                hit_node: Some(node_id),
                consumed: false,
                reason: AuiHitTestReason::HitNonInteractable,
                clip_rejected_count,
            };
        }

        let mut outside = AuiHitTestResult::outside(pointer);
        outside.clip_rejected_count = clip_rejected_count;
        outside
    }

    pub(super) fn is_submit_key(key: &str) -> bool {
        key.eq_ignore_ascii_case("Enter")
            || key.eq_ignore_ascii_case("Return")
            || key.eq_ignore_ascii_case("Space")
            || key == " "
    }

    pub(super) fn is_cancel_key(key: &str) -> bool {
        key.eq_ignore_ascii_case("Escape") || key.eq_ignore_ascii_case("Esc")
    }

    pub(super) fn text_edit_command_for_key(
        key: &str,
        shift: bool,
        control: bool,
    ) -> Option<AuiTextEditCommand> {
        if control && key.eq_ignore_ascii_case("A") {
            return Some(AuiTextEditCommand::SelectAll);
        }
        if key.eq_ignore_ascii_case("Backspace") {
            return Some(AuiTextEditCommand::Backspace);
        }
        if key.eq_ignore_ascii_case("Delete") {
            return Some(AuiTextEditCommand::Delete);
        }
        if key.eq_ignore_ascii_case("Home") {
            return Some(AuiTextEditCommand::MoveCaretHome);
        }
        if key.eq_ignore_ascii_case("End") {
            return Some(AuiTextEditCommand::MoveCaretEnd);
        }
        if key.eq_ignore_ascii_case("ArrowLeft") || key.eq_ignore_ascii_case("Left") {
            return Some(if shift {
                AuiTextEditCommand::SelectLeft
            } else {
                AuiTextEditCommand::MoveCaretLeft
            });
        }
        if key.eq_ignore_ascii_case("ArrowRight") || key.eq_ignore_ascii_case("Right") {
            return Some(if shift {
                AuiTextEditCommand::SelectRight
            } else {
                AuiTextEditCommand::MoveCaretRight
            });
        }
        None
    }

    pub(super) fn gamepad_direction_for_event(
        event: &RuntimeInputEvent,
    ) -> Option<AuiNavigationDirection> {
        match event {
            RuntimeInputEvent::GamepadButtonDown { button, .. }
            | RuntimeInputEvent::GamepadButtonHeld { button, .. } => {
                match button.to_ascii_lowercase().as_str() {
                    "dpadup" | "up" => Some(AuiNavigationDirection::Up),
                    "dpaddown" | "down" => Some(AuiNavigationDirection::Down),
                    "dpadleft" | "left" => Some(AuiNavigationDirection::Left),
                    "dpadright" | "right" => Some(AuiNavigationDirection::Right),
                    _ => None,
                }
            }
            RuntimeInputEvent::GamepadAxis2d { x, y, .. } => {
                if y.abs() >= x.abs() && y.abs() > 0.5 {
                    Some(if *y > 0.0 {
                        AuiNavigationDirection::Down
                    } else {
                        AuiNavigationDirection::Up
                    })
                } else if x.abs() > 0.5 {
                    Some(if *x > 0.0 {
                        AuiNavigationDirection::Right
                    } else {
                        AuiNavigationDirection::Left
                    })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub(super) fn gamepad_submit(event: &RuntimeInputEvent) -> bool {
        matches!(
            event,
            RuntimeInputEvent::GamepadButtonDown { button, .. }
                if matches!(button.to_ascii_lowercase().as_str(), "south" | "a" | "submit")
        )
    }

    pub(super) fn gamepad_cancel(event: &RuntimeInputEvent) -> bool {
        matches!(
            event,
            RuntimeInputEvent::GamepadButtonDown { button, .. }
                if matches!(button.to_ascii_lowercase().as_str(), "east" | "b" | "cancel")
        )
    }

    pub(super) fn start_text_editing(
        nodes_by_id: &AuiNodeLookup<'_>,
        state: &mut AuiInteractionState,
        node_id: &str,
    ) -> bool {
        let Some(node) = nodes_by_id.get(node_id) else {
            return false;
        };
        if node.kind != AuiNodeKind::InputField || node.read_only {
            return false;
        }
        let already_editing = matches!(
            &state.input_mode,
            AuiInputMode::TextEditing { node_id: active } if active == node_id
        );
        if !already_editing {
            state.input_field = Some(AuiInputFieldState::start(node));
            state.input_mode = AuiInputMode::TextEditing {
                node_id: node_id.to_string(),
            };
        }
        true
    }

    pub(super) fn text_payload(state: &AuiInputFieldState) -> String {
        serde_json::json!({
            "node_id": state.node_id,
            "draft_text": state.draft_text,
            "caret_index": state.caret_index,
            "selection_anchor": state.selection_anchor,
            "selection_focus": state.selection_focus,
        })
        .to_string()
    }

    pub fn process(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        input_frame: &RuntimeInputFrame,
    ) -> AuiInteractionResult {
        let mut state = AuiInteractionState::default();
        Self::process_with_state(
            document,
            layout,
            input_frame,
            &mut state,
            AuiInteractionConfig::default(),
        )
    }

    pub fn process_with_state(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        input_frame: &RuntimeInputFrame,
        state: &mut AuiInteractionState,
        config: AuiInteractionConfig,
    ) -> AuiInteractionResult {
        Self::process_with_state_in_space(document, layout, input_frame, state, config, None, None)
    }

    pub fn process_session_with_state(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        input_frame: &RuntimeInputFrame,
        state: &mut AuiInteractionState,
        config: AuiInteractionConfig,
        session_id: &str,
    ) -> AuiInteractionResult {
        Self::process_with_state_in_space(
            document,
            layout,
            input_frame,
            state,
            config,
            None,
            Some(session_id),
        )
    }

    pub fn process_target_space_with_state(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        input_frame: &RuntimeInputFrame,
        state: &mut AuiInteractionState,
        config: AuiInteractionConfig,
        presentation: &ResolvedGameViewPresentation,
    ) -> AuiInteractionResult {
        Self::process_with_state_in_space(
            document,
            layout,
            input_frame,
            state,
            config,
            Some(presentation),
            None,
        )
    }

    pub fn process_target_space_session_with_state(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        input_frame: &RuntimeInputFrame,
        state: &mut AuiInteractionState,
        config: AuiInteractionConfig,
        presentation: &ResolvedGameViewPresentation,
        session_id: &str,
    ) -> AuiInteractionResult {
        Self::process_with_state_in_space(
            document,
            layout,
            input_frame,
            state,
            config,
            Some(presentation),
            Some(session_id),
        )
    }

    pub(super) fn process_with_state_in_space(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        input_frame: &RuntimeInputFrame,
        state: &mut AuiInteractionState,
        config: AuiInteractionConfig,
        presentation: Option<&ResolvedGameViewPresentation>,
        session_id: Option<&str>,
    ) -> AuiInteractionResult {
        let mut result = AuiInteractionResult::default();
        result.input_mode_before = state.input_mode.label();
        result.ime_platform_coverage = AUI_IME_PLATFORM_COVERAGE.to_string();
        result.focusable_derived_from_interactable = document
            .nodes
            .iter()
            .any(|node| node.focusable.is_none() && node.interactable);
        state.interaction_index.sync(document, layout);
        let nodes_by_id = state.interaction_index.node_lookup(document);
        result.control_reconciliation_count += state.drain_control_reconciliation_count();
        Self::reconcile_session_identity(state, &mut result, session_id);
        Self::reconcile_control_values(&nodes_by_id, state, &mut result);
        Self::reconcile_effectively_hidden_state(layout, state, &mut result);
        let active_modal_root = Self::topmost_modal_root(document, layout, state);
        Self::reconcile_control_state(
            &nodes_by_id,
            layout,
            state,
            &mut result,
            active_modal_root.as_deref(),
        );
        state.active_modal_root = active_modal_root.clone();
        result.active_modal_root = active_modal_root.clone();
        match active_modal_root.as_ref() {
            Some(root) if state.focus.focus_scope_root.as_deref() != Some(root.as_str()) => {
                state.focus.focus_scope_root = Some(root.clone());
                state.focus.focus_reason = AuiFocusReason::ModalOpen;
                result.focus_change_count += 1;
            }
            None if state.focus.focus_scope_root.is_some() => {
                state.focus.focus_scope_root = None;
                state.focus.focus_reason = AuiFocusReason::Cleared;
                result.focus_change_count += 1;
            }
            _ => {}
        }
        if Self::apply_default_focus_if_needed(document, layout, &nodes_by_id, state) {
            result.default_focus_applied_count += 1;
            result.focus_change_count += 1;
        }

        for (event_index, event) in input_frame.events.iter().enumerate() {
            let text_event_kind = match event {
                RuntimeInputEvent::TextInput { .. } => Some(AuiInteractionEventKind::TextInput),
                RuntimeInputEvent::ImePreedit { .. } => Some(AuiInteractionEventKind::ImePreedit),
                RuntimeInputEvent::ImeCommit { .. } => Some(AuiInteractionEventKind::ImeCommit),
                RuntimeInputEvent::ImeCancel => Some(AuiInteractionEventKind::ImeCancel),
                _ => None,
            };
            if let Some(event_kind) = text_event_kind {
                let mut event_commands = Vec::new();
                let mut consumed = false;
                let mut source_node = None;
                if let Some(input_field) = state.input_field.as_mut() {
                    source_node = Some(input_field.node_id.clone());
                    if let Some(node) = nodes_by_id.get(input_field.node_id.as_str()) {
                        consumed = true;
                        result.normalized_ui_intent_count += 1;
                        match event {
                            RuntimeInputEvent::TextInput { text } => {
                                if !node.read_only && input_field.insert_text(text, node.max_length)
                                {
                                    result.text_changed_count += 1;
                                    event_commands.push(
                                        AuiCommand::new(
                                            result.commands.len() + event_commands.len(),
                                            input_field.node_id.clone(),
                                            AuiCommandKind::TextChanged,
                                        )
                                        .with_payload(Self::text_payload(input_field)),
                                    );
                                }
                            }
                            RuntimeInputEvent::ImePreedit {
                                text,
                                cursor_start,
                                cursor_end,
                            } => {
                                input_field.set_composition(
                                    text.clone(),
                                    *cursor_start,
                                    *cursor_end,
                                );
                                result.ime_preedit_count += 1;
                            }
                            RuntimeInputEvent::ImeCommit { text } => {
                                result.ime_commit_count += 1;
                                if !node.read_only
                                    && input_field.commit_composition(text, node.max_length)
                                {
                                    result.text_changed_count += 1;
                                    event_commands.push(
                                        AuiCommand::new(
                                            result.commands.len() + event_commands.len(),
                                            input_field.node_id.clone(),
                                            AuiCommandKind::TextChanged,
                                        )
                                        .with_payload(Self::text_payload(input_field)),
                                    );
                                }
                            }
                            RuntimeInputEvent::ImeCancel => {
                                input_field.cancel_composition();
                                result.ime_cancel_count += 1;
                            }
                            _ => {}
                        }
                    }
                }
                let command_count = event_commands.len();
                let action_count = AuiActionMapper::map(document, &event_commands).len();
                if consumed {
                    record_consumed_event(&mut result, event_index, event);
                }
                result.commands.extend(event_commands);
                result.traces.push(AuiInteractionTrace {
                    frame: input_frame.frame_id,
                    event_index,
                    event_kind,
                    pointer: AuiPointer::default(),
                    hit_node: source_node,
                    captured_node: None,
                    drop_target: None,
                    consumed,
                    reason: AuiHitTestReason::OutsideUi,
                    command_count,
                    action_count,
                });
                continue;
            }

            if let Some(delta) = wheel_event_delta(event) {
                let pointer = input_frame
                    .pointer_position
                    .map(|position| AuiPointer::new(position.x, position.y))
                    .unwrap_or_default();
                let hit = Self::hit_test_spatial(
                    &nodes_by_id,
                    layout,
                    pointer.x,
                    pointer.y,
                    presentation,
                    &mut state.interaction_index,
                );
                let pointer = hit.pointer;
                result.hit_test_clip_rejected_count += hit.clip_rejected_count;
                let mut event_commands = Vec::new();
                let mut consumed = false;
                let modal_inside = active_modal_root.as_deref().is_some_and(|root| {
                    hit.hit_node
                        .as_deref()
                        .is_some_and(|node| Self::node_in_subtree(&nodes_by_id, root, node))
                });
                let scroll_nodes =
                    Self::find_scroll_containers(&nodes_by_id, hit.hit_node.as_deref());
                if !scroll_nodes.is_empty() {
                    let delta_y = -delta * config.wheel_scroll_px_per_delta;
                    let applied = Self::apply_nested_scroll_delta(
                        &nodes_by_id,
                        layout,
                        state,
                        &scroll_nodes,
                        delta_y,
                    );
                    result.scroll_offset_change_count += applied.len();
                    for (scroll_node, payload) in applied {
                        event_commands.push(
                            AuiCommand::new(
                                result.commands.len() + event_commands.len(),
                                scroll_node,
                                AuiCommandKind::Scroll,
                            )
                            .with_payload(payload.to_payload_string()),
                        );
                    }
                    consumed = !event_commands.is_empty();
                } else if active_modal_root.is_some()
                    && config.modal_blocks_wheel_outside
                    && !modal_inside
                {
                    consumed = true;
                }
                let command_count = event_commands.len();
                let action_count = AuiActionMapper::map(document, &event_commands).len();
                if consumed {
                    record_consumed_event(&mut result, event_index, event);
                }
                result.commands.extend(event_commands);
                result.traces.push(AuiInteractionTrace {
                    frame: input_frame.frame_id,
                    event_index,
                    event_kind: AuiInteractionEventKind::MouseWheel,
                    pointer,
                    hit_node: hit.hit_node,
                    captured_node: None,
                    drop_target: None,
                    consumed,
                    reason: hit.reason,
                    command_count,
                    action_count,
                });
                continue;
            }

            if let Some(key_info) = key_event_info(event) {
                let mut event_commands = Vec::new();
                let modal_keyboard_consumed =
                    active_modal_root.is_some() && config.modal_blocks_keyboard;
                let mut consumed = false;
                let source_node = state.focus.focused_node.clone();
                let trace_source_node = source_node.clone().or_else(|| active_modal_root.clone());
                if key_info.kind == AuiInteractionEventKind::KeyDown {
                    let shift = input_frame
                        .modifiers
                        .iter()
                        .any(|modifier| modifier.eq_ignore_ascii_case("Shift"));
                    let control = input_frame
                        .modifiers
                        .iter()
                        .any(|modifier| modifier.eq_ignore_ascii_case("Control"));
                    let mut clear_text_editing = false;
                    if let Some(input_field) = state.input_field.as_mut() {
                        if Self::is_submit_key(&key_info.key) {
                            result.normalized_ui_intent_count += 1;
                            result.keyboard_intent_count += 1;
                            result.text_submitted_count += 1;
                            event_commands.push(
                                AuiCommand::new(
                                    result.commands.len() + event_commands.len(),
                                    input_field.node_id.clone(),
                                    AuiCommandKind::TextSubmitted,
                                )
                                .with_payload(Self::text_payload(input_field)),
                            );
                            clear_text_editing = true;
                            consumed = true;
                        } else if Self::is_cancel_key(&key_info.key) {
                            result.normalized_ui_intent_count += 1;
                            result.keyboard_intent_count += 1;
                            result.cancel_count += 1;
                            result.text_cancelled_count += 1;
                            event_commands.push(
                                AuiCommand::new(
                                    result.commands.len() + event_commands.len(),
                                    input_field.node_id.clone(),
                                    AuiCommandKind::TextCancelled,
                                )
                                .with_payload(input_field.original_text.clone()),
                            );
                            clear_text_editing = true;
                            consumed = true;
                        } else if let Some(command) =
                            Self::text_edit_command_for_key(&key_info.key, shift, control)
                        {
                            result.normalized_ui_intent_count += 1;
                            result.keyboard_intent_count += 1;
                            let mut changed = false;
                            let mut moved = false;
                            match command {
                                AuiTextEditCommand::Backspace => {
                                    changed = input_field.backspace();
                                }
                                AuiTextEditCommand::Delete => {
                                    changed = input_field.delete();
                                }
                                AuiTextEditCommand::MoveCaretLeft
                                | AuiTextEditCommand::MoveCaretRight
                                | AuiTextEditCommand::MoveCaretHome
                                | AuiTextEditCommand::MoveCaretEnd
                                | AuiTextEditCommand::SelectLeft
                                | AuiTextEditCommand::SelectRight
                                | AuiTextEditCommand::SelectAll => {
                                    moved = input_field.move_caret(
                                        command,
                                        matches!(
                                            command,
                                            AuiTextEditCommand::SelectLeft
                                                | AuiTextEditCommand::SelectRight
                                        ),
                                    );
                                }
                            }
                            if changed {
                                result.text_changed_count += 1;
                                event_commands.push(
                                    AuiCommand::new(
                                        result.commands.len() + event_commands.len(),
                                        input_field.node_id.clone(),
                                        AuiCommandKind::TextChanged,
                                    )
                                    .with_payload(Self::text_payload(input_field)),
                                );
                            }
                            if moved {
                                result.caret_move_count += 1;
                                if input_field.selection_anchor != input_field.selection_focus {
                                    result.selection_change_count += 1;
                                }
                            }
                            consumed = true;
                        }
                    } else if Self::is_submit_key(&key_info.key) {
                        result.normalized_ui_intent_count += 1;
                        result.keyboard_intent_count += 1;
                        if let Some(source_node) = source_node.clone().filter(|node_id| {
                            Self::control_node_status(
                                &nodes_by_id,
                                layout,
                                node_id,
                                active_modal_root.as_deref(),
                                true,
                            )
                            .0
                        }) {
                            if nodes_by_id
                                .get(source_node.as_str())
                                .is_some_and(|node| node.kind == AuiNodeKind::InputField)
                                && Self::start_text_editing(&nodes_by_id, state, &source_node)
                            {
                                result.text_edit_session_count += 1;
                            } else {
                                event_commands.push(AuiCommand::new(
                                    result.commands.len() + event_commands.len(),
                                    source_node,
                                    AuiCommandKind::Submit,
                                ));
                                result.submit_count += 1;
                            }
                            consumed = true;
                        }
                    } else if Self::is_cancel_key(&key_info.key) {
                        result.normalized_ui_intent_count += 1;
                        result.keyboard_intent_count += 1;
                        result.cancel_count += 1;
                        if state.screen_stack.active_stack.last().is_some() {
                            if let Some(popped) = Self::pop_screen(state) {
                                result.screen_stack_pop_count += 1;
                                result.control_reconciliation_count +=
                                    state.drain_control_reconciliation_count();
                                result.focus_restore_count = state.screen_stack.focus_restore_count;
                                event_commands.push(AuiCommand::new(
                                    result.commands.len() + event_commands.len(),
                                    popped.root_node_id,
                                    AuiCommandKind::Cancel,
                                ));
                            }
                            consumed = true;
                        } else if active_modal_root.is_some() {
                            if let Some(source_node) =
                                source_node.clone().or_else(|| active_modal_root.clone())
                            {
                                event_commands.push(AuiCommand::new(
                                    result.commands.len() + event_commands.len(),
                                    source_node,
                                    AuiCommandKind::Cancel,
                                ));
                            }
                            consumed = true;
                        }
                    }

                    if !consumed
                        && Self::append_slider_key_value_change(
                            &nodes_by_id,
                            state,
                            &mut event_commands,
                            result.commands.len(),
                            source_node.as_deref(),
                            &key_info.key,
                        )
                    {
                        result.normalized_ui_intent_count += 1;
                        result.keyboard_intent_count += 1;
                        consumed = true;
                    }

                    if !consumed {
                        if let Some(direction) =
                            Self::navigation_direction_for_key(&key_info.key, shift)
                        {
                            result.normalized_ui_intent_count += 1;
                            result.keyboard_intent_count += 1;
                            if let Some((previous, next)) =
                                Self::move_focus(document, layout, &nodes_by_id, state, direction)
                            {
                                if let Some(previous) = previous {
                                    event_commands.push(AuiCommand::new(
                                        result.commands.len() + event_commands.len(),
                                        previous,
                                        AuiCommandKind::Blur,
                                    ));
                                }
                                event_commands.push(AuiCommand::new(
                                    result.commands.len() + event_commands.len(),
                                    next,
                                    AuiCommandKind::Focus,
                                ));
                                result.focus_change_count += 1;
                                result.keyboard_navigation_event_count += 1;
                                let focused_after = state.focus.focused_node.clone();
                                if let Some(payload) = Self::ensure_focused_node_visible(
                                    &nodes_by_id,
                                    layout,
                                    state,
                                    focused_after.as_deref().unwrap_or_default(),
                                ) {
                                    result.focus_visible_scroll_count += 1;
                                    result.scroll_offset_change_count += 1;
                                    event_commands.push(
                                        AuiCommand::new(
                                            result.commands.len() + event_commands.len(),
                                            payload.node_id.clone(),
                                            AuiCommandKind::Scroll,
                                        )
                                        .with_payload(payload.to_payload_string()),
                                    );
                                }
                            }
                            consumed = true;
                        }
                    }

                    if clear_text_editing {
                        state.input_field = None;
                        state.input_mode = AuiInputMode::Navigation;
                    }
                    Self::append_toggle_value_changes_for_activation(
                        &nodes_by_id,
                        state,
                        &mut event_commands,
                        result.commands.len(),
                    );
                }
                if !consumed && modal_keyboard_consumed {
                    consumed = true;
                }
                let command_count = event_commands.len();
                let action_count = AuiActionMapper::map(document, &event_commands).len();
                if consumed {
                    record_consumed_event(&mut result, event_index, event);
                }
                result.commands.extend(event_commands);
                result.traces.push(AuiInteractionTrace {
                    frame: input_frame.frame_id,
                    event_index,
                    event_kind: key_info.kind,
                    pointer: AuiPointer::default(),
                    hit_node: trace_source_node,
                    captured_node: None,
                    drop_target: None,
                    consumed,
                    reason: AuiHitTestReason::OutsideUi,
                    command_count,
                    action_count,
                });
                continue;
            }

            let gamepad_event_kind = match event {
                RuntimeInputEvent::GamepadButtonDown { .. } => {
                    Some(AuiInteractionEventKind::GamepadButtonDown)
                }
                RuntimeInputEvent::GamepadButtonUp { .. } => {
                    Some(AuiInteractionEventKind::GamepadButtonUp)
                }
                RuntimeInputEvent::GamepadButtonHeld { .. } => {
                    Some(AuiInteractionEventKind::GamepadButtonHeld)
                }
                RuntimeInputEvent::GamepadAxis2d { .. } => {
                    Some(AuiInteractionEventKind::GamepadAxis2d)
                }
                _ => None,
            };
            if let Some(event_kind) = gamepad_event_kind {
                let mut event_commands = Vec::new();
                let mut consumed = false;
                let source_node = state.focus.focused_node.clone();
                let trace_source_node = source_node.clone().or_else(|| active_modal_root.clone());
                let mut clear_text_editing = false;
                if Self::gamepad_submit(event) {
                    result.normalized_ui_intent_count += 1;
                    result.gamepad_intent_count += 1;
                    if let Some(input_field) = state.input_field.as_mut() {
                        result.text_submitted_count += 1;
                        event_commands.push(
                            AuiCommand::new(
                                result.commands.len() + event_commands.len(),
                                input_field.node_id.clone(),
                                AuiCommandKind::TextSubmitted,
                            )
                            .with_payload(Self::text_payload(input_field)),
                        );
                        clear_text_editing = true;
                    } else if let Some(source_node) = source_node.clone().filter(|node_id| {
                        Self::control_node_status(
                            &nodes_by_id,
                            layout,
                            node_id,
                            active_modal_root.as_deref(),
                            true,
                        )
                        .0
                    }) {
                        if nodes_by_id
                            .get(source_node.as_str())
                            .is_some_and(|node| node.kind == AuiNodeKind::InputField)
                            && Self::start_text_editing(&nodes_by_id, state, &source_node)
                        {
                            result.text_edit_session_count += 1;
                        } else {
                            result.submit_count += 1;
                            event_commands.push(AuiCommand::new(
                                result.commands.len() + event_commands.len(),
                                source_node,
                                AuiCommandKind::Submit,
                            ));
                        }
                    }
                    consumed = true;
                } else if Self::gamepad_cancel(event) {
                    result.normalized_ui_intent_count += 1;
                    result.gamepad_intent_count += 1;
                    result.cancel_count += 1;
                    if let Some(input_field) = state.input_field.as_ref() {
                        result.text_cancelled_count += 1;
                        event_commands.push(
                            AuiCommand::new(
                                result.commands.len() + event_commands.len(),
                                input_field.node_id.clone(),
                                AuiCommandKind::TextCancelled,
                            )
                            .with_payload(input_field.original_text.clone()),
                        );
                        clear_text_editing = true;
                    } else if let Some(popped) = Self::pop_screen(state) {
                        result.screen_stack_pop_count += 1;
                        result.control_reconciliation_count +=
                            state.drain_control_reconciliation_count();
                        event_commands.push(AuiCommand::new(
                            result.commands.len() + event_commands.len(),
                            popped.root_node_id,
                            AuiCommandKind::Cancel,
                        ));
                    } else if let Some(source_node) =
                        source_node.clone().or_else(|| active_modal_root.clone())
                    {
                        event_commands.push(AuiCommand::new(
                            result.commands.len() + event_commands.len(),
                            source_node,
                            AuiCommandKind::Cancel,
                        ));
                    }
                    consumed = true;
                } else if let Some(direction) = Self::gamepad_direction_for_event(event) {
                    result.normalized_ui_intent_count += 1;
                    result.gamepad_intent_count += 1;
                    if let Some((previous, next)) =
                        Self::move_focus(document, layout, &nodes_by_id, state, direction)
                    {
                        if let Some(previous) = previous {
                            event_commands.push(AuiCommand::new(
                                result.commands.len() + event_commands.len(),
                                previous,
                                AuiCommandKind::Blur,
                            ));
                        }
                        event_commands.push(AuiCommand::new(
                            result.commands.len() + event_commands.len(),
                            next,
                            AuiCommandKind::Focus,
                        ));
                        result.focus_change_count += 1;
                        result.keyboard_navigation_event_count += 1;
                    }
                    consumed = true;
                }
                if !consumed && active_modal_root.is_some() && config.modal_blocks_keyboard {
                    consumed = true;
                }
                if clear_text_editing {
                    state.input_field = None;
                    state.input_mode = AuiInputMode::Navigation;
                }
                Self::append_toggle_value_changes_for_activation(
                    &nodes_by_id,
                    state,
                    &mut event_commands,
                    result.commands.len(),
                );
                let command_count = event_commands.len();
                let action_count = AuiActionMapper::map(document, &event_commands).len();
                if consumed {
                    record_consumed_event(&mut result, event_index, event);
                }
                result.commands.extend(event_commands);
                result.traces.push(AuiInteractionTrace {
                    frame: input_frame.frame_id,
                    event_index,
                    event_kind,
                    pointer: AuiPointer::default(),
                    hit_node: trace_source_node,
                    captured_node: None,
                    drop_target: None,
                    consumed,
                    reason: AuiHitTestReason::OutsideUi,
                    command_count,
                    action_count,
                });
                continue;
            }

            let Some(event_info) = pointer_event_info(event) else {
                continue;
            };
            let event_kind = event_info.kind;
            let target_pointer = event_info.pointer;
            let hit = Self::hit_test_spatial(
                &nodes_by_id,
                layout,
                target_pointer.x,
                target_pointer.y,
                presentation,
                &mut state.interaction_index,
            );
            let pointer = hit.pointer;
            result.hit_test_clip_rejected_count += hit.clip_rejected_count;
            let mut event_commands = Vec::new();
            let mut consumed = hit.consumed;
            let mut captured_node = state.active_drag_source().map(ToOwned::to_owned);
            let mut drop_target = None;
            let modal_inside = active_modal_root.as_deref().is_some_and(|root| {
                hit.hit_node
                    .as_deref()
                    .is_some_and(|node| Self::node_in_subtree(&nodes_by_id, root, node))
            });
            let pointer_blocked_by_modal =
                active_modal_root.is_some() && config.modal_blocks_pointer_outside && !modal_inside;
            if active_modal_root.is_some() && modal_inside {
                consumed = true;
            }
            let scroll_node = Self::find_scroll_container(&nodes_by_id, hit.hit_node.as_deref());
            let eligible_hit_node = (!pointer_blocked_by_modal
                && hit.reason == AuiHitTestReason::HitInteractable)
                .then(|| hit.hit_node.clone())
                .flatten();

            if event_info.hover_capable
                && matches!(
                    event_kind,
                    AuiInteractionEventKind::PointerDown
                        | AuiInteractionEventKind::PointerMove
                        | AuiInteractionEventKind::PointerUp
                )
            {
                state.hovered_node = eligible_hit_node.clone();
                if event_kind == AuiInteractionEventKind::PointerMove
                    && state.focus.focused_node.is_some()
                {
                    state.focus.focus_reason = AuiFocusReason::Pointer;
                }
            } else if !event_info.hover_capable
                || matches!(
                    event_kind,
                    AuiInteractionEventKind::PointerCancel | AuiInteractionEventKind::PointerLeave
                )
            {
                state.hovered_node = None;
            }

            if let Some(press) = state.primary_press.as_mut() {
                if event_info.matches_primary_press(press)
                    && event_kind == AuiInteractionEventKind::PointerMove
                {
                    press.inside = eligible_hit_node.as_deref() == Some(press.node_id.as_str());
                }
            }

            if event_kind == AuiInteractionEventKind::PointerCancel {
                let matching_press = state
                    .primary_press
                    .as_ref()
                    .is_some_and(|capture| event_info.matches_primary_press(capture));
                let matching_drag = state
                    .active_drag
                    .as_ref()
                    .is_some_and(|capture| event_info.matches_drag(capture));
                let matching_scroll = state
                    .active_scroll_capture
                    .as_ref()
                    .is_some_and(|capture| event_info.matches_scroll(capture));
                if matching_press || matching_drag || matching_scroll {
                    consumed = true;
                    result.pointer_cancel_count += 1;
                    if matching_press {
                        if let Some(press) = state.primary_press.take() {
                            captured_node = Some(press.node_id.clone());
                            event_commands.push(AuiCommand::new(
                                result.commands.len() + event_commands.len(),
                                press.node_id,
                                AuiCommandKind::PointerCancel,
                            ));
                        }
                    }
                    if matching_drag {
                        if let Some(active_drag) = state.active_drag.take() {
                            captured_node = Some(active_drag.source_node.clone());
                            if active_drag.started {
                                let payload = AuiDragDropPayload::new(
                                    active_drag.source_node.clone(),
                                    None,
                                    active_drag.start_pointer,
                                    pointer,
                                    "cancel",
                                )
                                .to_payload_string();
                                event_commands.push(
                                    AuiCommand::new(
                                        result.commands.len() + event_commands.len(),
                                        active_drag.source_node,
                                        AuiCommandKind::DragCancel,
                                    )
                                    .with_payload(payload),
                                );
                            }
                        }
                    }
                    if matching_scroll {
                        if let Some(capture) = state.active_scroll_capture.take() {
                            captured_node = Some(capture.captured_node_id);
                        }
                    }
                }
            } else if event_kind == AuiInteractionEventKind::PointerLeave {
                if let Some(press) = state.primary_press.as_mut() {
                    if event_info.matches_primary_press(press) {
                        press.inside = false;
                        captured_node = Some(press.node_id.clone());
                        consumed = true;
                    }
                }
                if state
                    .active_drag
                    .as_ref()
                    .is_some_and(|capture| event_info.matches_drag(capture))
                    || state
                        .active_scroll_capture
                        .as_ref()
                        .is_some_and(|capture| event_info.matches_scroll(capture))
                {
                    consumed = true;
                }
            } else if state
                .active_scroll_capture
                .as_ref()
                .is_some_and(|capture| event_info.matches_scroll(capture))
            {
                consumed = true;
                let mut release_scroll = false;
                let mut cancel_press_for_scroll = false;
                let mut complete_press_after_scroll = false;
                let mut pending_scroll_delta: Option<(String, f32)> = None;
                if let Some(capture) = state.active_scroll_capture.as_mut() {
                    captured_node = Some(capture.captured_node_id.clone());
                    match event_kind {
                        AuiInteractionEventKind::PointerMove => {
                            if !capture.started
                                && pointer_distance(capture.start_pointer, pointer)
                                    >= config.drag_scroll_threshold_px
                            {
                                capture.started = true;
                                cancel_press_for_scroll = true;
                            }
                            if capture.started {
                                let pointer_delta_y = layout
                                    .computed_nodes
                                    .iter()
                                    .find(|n| n.node_id == capture.node_id)
                                    .and_then(|node| {
                                        Some(
                                            node.presented_point_to_local(pointer)?[1]
                                                - node.presented_point_to_local(
                                                    capture.last_pointer,
                                                )?[1],
                                        )
                                    })
                                    .unwrap_or(0.);
                                let delta_y = capture
                                    .scroll_delta_per_pointer_delta_y
                                    .map(|ratio| pointer_delta_y * ratio)
                                    .unwrap_or_else(|| -pointer_delta_y);
                                capture.last_pointer = pointer;
                                pending_scroll_delta = Some((capture.node_id.clone(), delta_y));
                            }
                        }
                        AuiInteractionEventKind::PointerUp => {
                            complete_press_after_scroll = !capture.started;
                            release_scroll = true;
                        }
                        _ => {}
                    }
                }
                if cancel_press_for_scroll {
                    if state
                        .primary_press
                        .as_ref()
                        .is_some_and(|press| event_info.matches_primary_press(press))
                    {
                        if let Some(press) = state.primary_press.take() {
                            event_commands.push(AuiCommand::new(
                                result.commands.len() + event_commands.len(),
                                press.node_id,
                                AuiCommandKind::PointerCancel,
                            ));
                        }
                    }
                    if state
                        .active_drag
                        .as_ref()
                        .is_some_and(|drag| event_info.matches_drag(drag))
                    {
                        state.active_drag = None;
                    }
                }
                if let Some((scroll_node, delta_y)) = pending_scroll_delta {
                    if let Some(payload) = Self::update_scroll_offset(
                        &nodes_by_id,
                        layout,
                        state,
                        &scroll_node,
                        delta_y,
                    ) {
                        result.scroll_offset_change_count += 1;
                        event_commands.push(
                            AuiCommand::new(
                                result.commands.len() + event_commands.len(),
                                scroll_node,
                                AuiCommandKind::Scroll,
                            )
                            .with_payload(payload.to_payload_string()),
                        );
                    }
                }
                if complete_press_after_scroll
                    && state
                        .primary_press
                        .as_ref()
                        .is_some_and(|press| event_info.matches_primary_press(press))
                {
                    let mut press = state.primary_press.take().expect("matching primary press");
                    press.inside = eligible_hit_node.as_deref() == Some(press.node_id.as_str());
                    let owner = press.node_id;
                    captured_node = Some(owner.clone());
                    event_commands.push(AuiCommand::new(
                        result.commands.len() + event_commands.len(),
                        owner.clone(),
                        AuiCommandKind::PointerUp,
                    ));
                    if press.inside {
                        event_commands.push(AuiCommand::new(
                            result.commands.len() + event_commands.len(),
                            owner,
                            AuiCommandKind::Click,
                        ));
                    }
                    if state
                        .active_drag
                        .as_ref()
                        .is_some_and(|drag| event_info.matches_drag(drag))
                    {
                        state.active_drag = None;
                    }
                }
                if release_scroll {
                    state.active_scroll_capture = None;
                }
            } else if event_kind == AuiInteractionEventKind::PointerDown
                && event_info.is_primary_pointer()
                && scroll_node.is_some()
                && Self::scrollbar_thumb_at_pointer(layout, pointer).is_some()
            {
                let scrollbar_thumb = Self::scrollbar_thumb_at_pointer(layout, pointer);
                let scroll_node = scrollbar_thumb
                    .map(|metrics| metrics.scroll_node_id.clone())
                    .or_else(|| scroll_node.clone())
                    .expect("scroll node");
                let (captured_node_id, started, scroll_delta_per_pointer_delta_y) =
                    if let Some(metrics) = scrollbar_thumb {
                        let track_travel =
                            (metrics.track_rect.height - metrics.thumb_rect.height).max(0.0);
                        let ratio = if track_travel <= f32::EPSILON {
                            0.0
                        } else {
                            metrics.max_offset_y / track_travel
                        };
                        (metrics.thumb_node_id(), true, Some(ratio))
                    } else {
                        (scroll_node.clone(), false, None)
                    };
                state.active_scroll_capture = Some(AuiActiveScrollCapture {
                    node_id: scroll_node.clone(),
                    captured_node_id: captured_node_id.clone(),
                    start_pointer: pointer,
                    last_pointer: pointer,
                    started,
                    scroll_delta_per_pointer_delta_y,
                    pointer_id: event_info.pointer_id,
                    device_kind: event_info.device_kind,
                });
                captured_node = Some(captured_node_id);
                consumed = true;
            } else if state
                .active_drag
                .as_ref()
                .is_some_and(|capture| event_info.matches_drag(capture))
            {
                consumed = true;
                let mut release_drag = false;
                let mut click_source: Option<String> = None;
                if let Some(active_drag) = state.active_drag.as_mut() {
                    captured_node = Some(active_drag.source_node.clone());
                    active_drag.current_pointer = pointer;
                    match event_kind {
                        AuiInteractionEventKind::PointerMove => {
                            if !active_drag.started
                                && pointer_distance(active_drag.start_pointer, pointer)
                                    >= config.drag_threshold_px
                            {
                                active_drag.started = true;
                                let payload = AuiDragDropPayload::new(
                                    active_drag.source_node.clone(),
                                    None,
                                    active_drag.start_pointer,
                                    pointer,
                                    "start",
                                )
                                .to_payload_string();
                                event_commands.push(
                                    AuiCommand::new(
                                        result.commands.len() + event_commands.len(),
                                        active_drag.source_node.clone(),
                                        AuiCommandKind::DragStart,
                                    )
                                    .with_payload(payload),
                                );
                            }
                            if active_drag.started {
                                let payload = AuiDragDropPayload::new(
                                    active_drag.source_node.clone(),
                                    None,
                                    active_drag.start_pointer,
                                    pointer,
                                    "move",
                                )
                                .to_payload_string();
                                event_commands.push(
                                    AuiCommand::new(
                                        result.commands.len() + event_commands.len(),
                                        active_drag.source_node.clone(),
                                        AuiCommandKind::DragMove,
                                    )
                                    .with_payload(payload),
                                );
                            }
                        }
                        AuiInteractionEventKind::PointerUp => {
                            let source_node = active_drag.source_node.clone();
                            event_commands.push(AuiCommand::new(
                                result.commands.len() + event_commands.len(),
                                source_node.clone(),
                                AuiCommandKind::PointerUp,
                            ));
                            if active_drag.started {
                                drop_target = hit.hit_node.clone().filter(|node_id| {
                                    nodes_by_id
                                        .get(node_id.as_str())
                                        .map(|node| node.drop_target)
                                        .unwrap_or(false)
                                });
                                let phase = if drop_target.is_some() {
                                    "drop"
                                } else {
                                    "cancel"
                                };
                                let payload = AuiDragDropPayload::new(
                                    source_node.clone(),
                                    drop_target.clone(),
                                    active_drag.start_pointer,
                                    pointer,
                                    phase,
                                )
                                .to_payload_string();
                                event_commands.push(
                                    AuiCommand::new(
                                        result.commands.len() + event_commands.len(),
                                        source_node,
                                        if drop_target.is_some() {
                                            AuiCommandKind::Drop
                                        } else {
                                            AuiCommandKind::DragCancel
                                        },
                                    )
                                    .with_payload(payload),
                                );
                            } else if hit.hit_node.as_deref() == Some(source_node.as_str()) {
                                click_source = Some(source_node);
                            }
                            release_drag = true;
                        }
                        AuiInteractionEventKind::PointerDown => {}
                        _ => {}
                    }
                }
                if let Some(source_node) = click_source {
                    event_commands.push(AuiCommand::new(
                        result.commands.len() + event_commands.len(),
                        source_node,
                        AuiCommandKind::Click,
                    ));
                }
                if release_drag {
                    state.active_drag = None;
                    state.primary_press = None;
                }
            } else if state
                .primary_press
                .as_ref()
                .is_some_and(|capture| event_info.matches_primary_press(capture))
                && matches!(
                    event_kind,
                    AuiInteractionEventKind::PointerMove | AuiInteractionEventKind::PointerUp
                )
            {
                consumed = true;
                let press = state.primary_press.as_mut().expect("primary press");
                press.inside = eligible_hit_node.as_deref() == Some(press.node_id.as_str());
                let owner = press.node_id.clone();
                captured_node = Some(owner.clone());
                let command_kind = if event_kind == AuiInteractionEventKind::PointerUp {
                    AuiCommandKind::PointerUp
                } else {
                    AuiCommandKind::PointerMove
                };
                event_commands.push(AuiCommand::new(
                    result.commands.len() + event_commands.len(),
                    owner.clone(),
                    command_kind,
                ));
                if event_kind == AuiInteractionEventKind::PointerUp {
                    if press.inside {
                        event_commands.push(AuiCommand::new(
                            result.commands.len() + event_commands.len(),
                            owner,
                            AuiCommandKind::Click,
                        ));
                    }
                    state.primary_press = None;
                }
            } else if pointer_blocked_by_modal {
                consumed = true;
            } else if let Some(hit_node) = hit.hit_node.clone() {
                if hit.consumed {
                    let command_kind = match event_kind {
                        AuiInteractionEventKind::PointerDown => AuiCommandKind::PointerDown,
                        AuiInteractionEventKind::PointerUp => AuiCommandKind::PointerUp,
                        AuiInteractionEventKind::PointerMove => AuiCommandKind::PointerMove,
                        _ => AuiCommandKind::PointerMove,
                    };
                    event_commands.push(AuiCommand::new(
                        result.commands.len() + event_commands.len(),
                        hit_node.clone(),
                        command_kind,
                    ));

                    if event_kind == AuiInteractionEventKind::PointerMove {
                        event_commands.push(AuiCommand::new(
                            result.commands.len() + event_commands.len(),
                            hit_node.clone(),
                            AuiCommandKind::Hover,
                        ));
                    }

                    if event_kind == AuiInteractionEventKind::PointerDown {
                        if event_info.is_primary_pointer() {
                            state.primary_press = Some(AuiPrimaryPressCapture {
                                node_id: hit_node.clone(),
                                pointer_id: event_info.pointer_id,
                                device_kind: event_info.device_kind,
                                hover_capable: event_info.hover_capable,
                                inside: true,
                            });
                            captured_node = Some(hit_node.clone());
                        }
                        if state.focus.focused_node.as_deref() != Some(hit_node.as_str()) {
                            if let Some(previous) =
                                state.focus.focused_node.replace(hit_node.clone())
                            {
                                event_commands.push(AuiCommand::new(
                                    result.commands.len() + event_commands.len(),
                                    previous,
                                    AuiCommandKind::Blur,
                                ));
                            }
                            state.focus.focus_reason = AuiFocusReason::Pointer;
                            result.focus_change_count += 1;
                            event_commands.push(AuiCommand::new(
                                result.commands.len() + event_commands.len(),
                                hit_node.clone(),
                                AuiCommandKind::Focus,
                            ));
                        }
                        if nodes_by_id
                            .get(hit_node.as_str())
                            .is_some_and(|node| node.kind == AuiNodeKind::InputField)
                            && Self::start_text_editing(&nodes_by_id, state, &hit_node)
                        {
                            result.text_edit_session_count += 1;
                        }
                        if event_info.is_primary_pointer()
                            && nodes_by_id
                                .get(hit_node.as_str())
                                .map(|node| node.draggable)
                                .unwrap_or(false)
                        {
                            state.active_drag = Some(AuiActiveDrag {
                                source_node: hit_node.clone(),
                                start_pointer: pointer,
                                current_pointer: pointer,
                                started: false,
                                pointer_id: event_info.pointer_id,
                                device_kind: event_info.device_kind,
                            });
                            captured_node = Some(hit_node.clone());
                        }
                    }
                }
            }

            if event_kind == AuiInteractionEventKind::PointerDown
                && event_info.is_primary_pointer()
                && !pointer_blocked_by_modal
                && state.active_scroll_capture.is_none()
            {
                if let Some(scroll_node) = scroll_node.as_ref() {
                    state.active_scroll_capture = Some(AuiActiveScrollCapture {
                        node_id: scroll_node.clone(),
                        captured_node_id: scroll_node.clone(),
                        start_pointer: pointer,
                        last_pointer: pointer,
                        started: false,
                        scroll_delta_per_pointer_delta_y: None,
                        pointer_id: event_info.pointer_id,
                        device_kind: event_info.device_kind,
                    });
                    if captured_node.is_none() {
                        captured_node = Some(scroll_node.clone());
                    }
                    consumed = true;
                }
            }

            if matches!(
                event_kind,
                AuiInteractionEventKind::PointerDown
                    | AuiInteractionEventKind::PointerMove
                    | AuiInteractionEventKind::PointerUp
            ) {
                Self::append_slider_pointer_value_change(
                    &nodes_by_id,
                    layout,
                    state,
                    &mut event_commands,
                    result.commands.len(),
                    captured_node.as_deref(),
                    pointer,
                );
            }
            Self::append_toggle_value_changes_for_activation(
                &nodes_by_id,
                state,
                &mut event_commands,
                result.commands.len(),
            );

            let command_count = event_commands.len();
            let action_count = AuiActionMapper::map(document, &event_commands).len();
            if consumed {
                record_consumed_event(&mut result, event_index, event);
            }
            result.commands.extend(event_commands);
            result.traces.push(AuiInteractionTrace {
                frame: input_frame.frame_id,
                event_index,
                event_kind,
                pointer,
                hit_node: hit.hit_node,
                captured_node,
                drop_target,
                consumed,
                reason: hit.reason,
                command_count,
                action_count,
            });
        }

        result.actions = AuiActionMapper::map(document, &result.commands);
        result.input_mode_after = state.input_mode.label();
        result.active_screen_id = state
            .screen_stack
            .active_stack
            .last()
            .map(|entry| entry.screen_id.clone());
        result.screen_stack_push_count = state.screen_stack.push_count;
        result.default_focus_applied_count = state.screen_stack.default_focus_applied_count;
        result.focus_restore_count = state.screen_stack.focus_restore_count;
        result.action_prompt_reported = state.focus.focused_node.is_some();
        result.control_snapshot = Self::control_snapshot(input_frame.frame_id, state);
        result
    }
}
