//! Ordinary project consumer: only the public game SDK is imported.
use project_game_sdk::aui::UiNodeHandle;
use project_game_sdk::aui_animation::{
    Animation2D, AnimationStep, CompletionPolicy, Easing, PlaybackDirection, PresentationClock,
    Repeat, TweenProperty, TweenTrack, TweenValue, ANIMATION_2D_VERSION,
};
use project_game_sdk::aui_draw2d::{Draw2D, FillRule, PathBuilder};
use project_game_sdk::aui_presentation::Transform2D;

#[test]
fn project_authors_graphics_and_presentation_without_runtime_internals() {
    let mut path = PathBuilder::new();
    path.move_to([0.0, 0.0])
        .unwrap()
        .line_to([20.0, 0.0])
        .unwrap()
        .quadratic_to([30.0, 10.0], [20.0, 20.0])
        .unwrap()
        .close()
        .unwrap();
    let mut draw = Draw2D::new();
    draw.fill(path.finish(), [0.5, 0.25, 0.1, 1.0], FillRule::NonZero)
        .unwrap();
    draw.finish().validate().unwrap();

    let parent = Transform2D {
        scale: [2.0; 2],
        pivot: [0.0; 2],
        ..Default::default()
    }
    .local_matrix([40.0, 60.0], [100.0; 2])
    .unwrap();
    let child = Transform2D::default()
        .local_matrix([10.0, 5.0], [20.0; 2])
        .unwrap();
    let screen = parent.compose(child).unwrap();
    assert_eq!(screen.transform_point([0.0; 2]).unwrap(), [60.0, 70.0]);
    assert_eq!(
        screen
            .try_inverse()
            .unwrap()
            .transform_point([60.0, 70.0])
            .unwrap(),
        [0.0; 2]
    );
}

#[test]
fn project_authors_yoyo_and_separate_pause_clocks() {
    let animation = Animation2D {
        version: ANIMATION_2D_VERSION,
        root: AnimationStep::Tween {
            track: TweenTrack {
                target: UiNodeHandle {
                    instance: 1,
                    slot: 2,
                    generation: 1,
                },
                property: TweenProperty::Scale,
                from: TweenValue::Vec2([1.0; 2]),
                to: TweenValue::Vec2([1.1; 2]),
                duration_us: 100_000,
                delay_us: 0,
                easing: Easing::EaseOutQuad,
            },
        },
        repeat: Repeat::Once,
        direction: PlaybackDirection::Yoyo,
        completion: CompletionPolicy::Release,
    };
    assert_eq!(animation.validate().unwrap().total_us, Some(200_000));
    let mut ui = PresentationClock::default();
    let mut game = PresentationClock::default();
    game.set_paused(true);
    assert_eq!(ui.advance(1, 16_667).unwrap().delta_us, 16_667);
    assert_eq!(game.advance(1, 16_667).unwrap().delta_us, 0);
}
