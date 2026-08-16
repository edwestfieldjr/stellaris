use bevy::prelude::*;
use rand::RngExt as _;
use std::f32::consts::TAU;

use crate::hud_bridge::ScreenText;
use crate::state::{AppState, WarpTarget};

const WARP_SECONDS: f32 = 0.9;
const STREAK_COUNT: usize = 140;

// The targeting-computer HUD frame: three bars that slide up from off the
// bottom of the screen into a resting frame, hold there, then slide back
// down and out, at a constant half-transparent alpha throughout.
const BAR_START_Y: f32 = -420.0;
const BAR_SLIDE_IN_END: f32 = 0.35;
const BAR_HOLD_END: f32 = 0.7;
const BAR_ALPHA: f32 = 0.5;
const BAR_COLOR: Color = Color::srgba(0.3, 1.0, 0.6, BAR_ALPHA);

#[derive(Component)]
struct WarpUi;

#[derive(Component)]
struct WarpStreak {
    dir: Vec2,
}

/// One bar of the targeting-computer frame; `rest_y` is where it settles
/// once it's fully slid up from `BAR_START_Y`.
#[derive(Component)]
struct TargetingBar {
    rest_y: f32,
}

#[derive(Resource)]
struct WarpTimer(Timer);

pub struct WarpPlugin;

impl Plugin for WarpPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Warp), setup)
            .add_systems(OnExit(AppState::Warp), teardown)
            .add_systems(Update, tick_warp.run_if(in_state(AppState::Warp)));
    }
}

fn setup(mut commands: Commands, warp_target: Res<WarpTarget>, mut screen_text: ResMut<ScreenText>) {
    commands.insert_resource(WarpTimer(Timer::from_seconds(
        WARP_SECONDS,
        TimerMode::Once,
    )));

    // Warp is the only path between GalaxyMap and Flight in either
    // direction — clear whichever screen's text was showing (GalaxyMap's
    // fuel/level/banner/countdown) so it doesn't linger behind the warp
    // animation and into Flight, which doesn't push its own ScreenText.
    *screen_text = ScreenText::default();

    let mut rng = rand::rng();
    for _ in 0..STREAK_COUNT {
        let theta = rng.random_range(0.0..TAU);
        let dir = Vec2::new(theta.cos(), theta.sin());
        commands.spawn((
            Sprite {
                color: Color::srgba(0.7, 0.85, 1.0, 0.0),
                custom_size: Some(Vec2::new(2.0, 2.0)),
                ..default()
            },
            Transform::from_translation(Vec3::ZERO).with_rotation(Quat::from_rotation_z(theta)),
            WarpStreak { dir },
            WarpUi,
        ));
    }

    // Targeting-computer HUD frame: three bars of decreasing width,
    // stacked, all rising together from below the screen.
    for (size, rest_y) in [
        (Vec2::new(360.0, 4.0), -70.0),
        (Vec2::new(520.0, 3.0), 0.0),
        (Vec2::new(300.0, 4.0), 70.0),
    ] {
        commands.spawn((
            Sprite {
                color: BAR_COLOR,
                custom_size: Some(size),
                ..default()
            },
            Transform::from_xyz(0.0, BAR_START_Y, 5.0),
            TargetingBar { rest_y },
            WarpUi,
        ));
    }

    let label = match warp_target.0 {
        AppState::GalaxyMap => "RETURNING",
        _ => "WARPING",
    };
    commands.spawn((
        Text::new(label),
        TextFont {
            font_size: bevy::text::FontSize::Px(36.0),
            ..default()
        },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.85)),
        TextLayout::default().with_justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(0.0),
            bottom: Val::Px(0.0),
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            align_content: AlignContent::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        WarpUi,
    ));
}

fn teardown(mut commands: Commands, query: Query<Entity, With<WarpUi>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<WarpTimer>();
}

fn tick_warp(
    time: Res<Time>,
    mut timer: ResMut<WarpTimer>,
    warp_target: Res<WarpTarget>,
    mut streaks: Query<(&mut Transform, &mut Sprite, &WarpStreak)>,
    mut bars: Query<(&mut Transform, &TargetingBar), Without<WarpStreak>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    timer.0.tick(time.delta());
    // Eased 0..1 so streaks accelerate outward like a hyperspace jump.
    let t = timer.0.fraction();
    let eased = t * t;
    for (mut transform, mut sprite, streak) in &mut streaks {
        let dist = eased * 700.0;
        let len = 4.0 + eased * 260.0;
        transform.translation = (streak.dir * dist).extend(0.0);
        sprite.custom_size = Some(Vec2::new(len, 2.0 + eased * 2.0));
        sprite.color = Color::srgba(0.7, 0.85, 1.0, (t * 3.0).min(1.0) * (1.0 - eased * 0.3));
    }

    // Slide up, hold, slide back down — a targeting computer booting up
    // and retracting again.
    let bar_t = if t < BAR_SLIDE_IN_END {
        t / BAR_SLIDE_IN_END
    } else if t < BAR_HOLD_END {
        1.0
    } else {
        1.0 - (t - BAR_HOLD_END) / (1.0 - BAR_HOLD_END)
    }
    .clamp(0.0, 1.0);
    for (mut transform, bar) in &mut bars {
        transform.translation.y = BAR_START_Y + (bar.rest_y - BAR_START_Y) * bar_t;
    }

    if timer.0.is_finished() {
        next_state.set(warp_target.0);
    }
}
