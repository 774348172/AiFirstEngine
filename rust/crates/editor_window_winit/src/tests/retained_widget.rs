use editor_input::{EditorInputEvent, EditorInputRouter, PointerButton};
use editor_ui_model::UiCommandPayload;
use editor_ui_renderer::{
    extract_widget_tree, layout_widget_tree, pick_widget, reconcile_widget_tree,
    EditorCommandBinding, EditorWidgetAction, EditorWidgetDeclaration, HitTarget, UiColor, UiPoint,
    WidgetId, WidgetPaint, WidgetRole,
};

#[test]
fn retained_widget_synthetic_pipeline_routes_existing_command() {
    let mut root =
        EditorWidgetDeclaration::new(WidgetId::semantic("root").unwrap(), WidgetRole::Root);
    root.style.clip = true;
    let mut play = EditorWidgetDeclaration::new(
        WidgetId::semantic("toolbar/play").unwrap(),
        WidgetRole::Button,
    );
    play.style.width = Some(100.0);
    play.style.height = Some(30.0);
    play.paint.push(WidgetPaint::Rect {
        color: UiColor::rgba(20, 30, 40, 255),
        corner_radius: 0.0,
    });
    play.binding = Some(EditorCommandBinding {
        action: EditorWidgetAction::Activate,
        command_id: "tick_one_frame".into(),
        target: HitTarget::ToolbarCommand {
            command_id: "tick_one_frame".into(),
        },
        reason_disabled: None,
    });
    root.children.push(play);

    let (mut tree, first_report) = reconcile_widget_tree(None, &root).unwrap();
    layout_widget_tree(
        &mut tree,
        320.0,
        200.0,
        &mut |_: &WidgetId, _: Option<f32>| (0.0, 0.0),
    )
    .unwrap();
    let before_pick = tree.clone();
    let pick = pick_widget(&tree, UiPoint { x: 10.0, y: 10.0 }, None).expect("retained pick");
    assert_eq!(pick.target.as_str(), "toolbar/play");
    assert_eq!(
        tree, before_pick,
        "pointer queries must not rebuild or mutate the tree"
    );

    let extracted = extract_widget_tree(&tree, 1, 0, 320.0, 200.0);
    let mut router = EditorInputRouter::new();
    let routed = router.route(
        EditorInputEvent::PointerDown {
            x: 10.0,
            y: 10.0,
            button: PointerButton::Primary,
        },
        &extracted.draw_list,
    );
    assert_eq!(
        routed.command.expect("command binding").payload,
        UiCommandPayload::TickOneFrame
    );
    assert_eq!(first_report.created, 2);
    assert_eq!(extracted.extracted_widget_count, 2);
}

#[test]
fn retained_widget_removed_node_is_reported_for_focus_cleanup() {
    let mut first =
        EditorWidgetDeclaration::new(WidgetId::semantic("root").unwrap(), WidgetRole::Root);
    first.children.push(EditorWidgetDeclaration::new(
        WidgetId::semantic("field").unwrap(),
        WidgetRole::TextInput,
    ));
    let (tree, _) = reconcile_widget_tree(None, &first).unwrap();
    let second =
        EditorWidgetDeclaration::new(WidgetId::semantic("root").unwrap(), WidgetRole::Root);
    let (_, report) = reconcile_widget_tree(Some(&tree), &second).unwrap();
    assert_eq!(report.removed, vec![WidgetId::semantic("field").unwrap()]);
}
