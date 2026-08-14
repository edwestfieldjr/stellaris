use bevy::prelude::*;
use rand::RngExt as _;
use std::f32::consts::TAU;

use crate::state::AppState;

const WARP_SECONDS: f32 = 0.65;
const STREAK_COUNT: usize = 140;

#[derive(Component)]
struct WarpUi;

#[derive(Component)]
struct WarpStreak {
    dir: Vec2,
}

#[derive(Resource)]
struct WarpTimer(Timer);

pub struct WarpPlugin;

impl Plugin for WarpPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Warp), setup)
            .add_systems(OnExit(AppState::Warp), teardown)
            .add_systems(
                Update,
                tick_warp.run_if(in_state(AppState::Warp)),
            );
    }
}

fn setup(mut commands: Commands) {
    commands.insert_resource(WarpTimer(Timer::from_seconds(
        WARP_SECONDS,
        TimerMode::Once,
    )));

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

    commands.spawn((
        Text::new("WARPING"),
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
    mut streaks: Query<(&mut Transform, &mut Sprite, &WarpStreak)>,
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

    if timer.0.is_finished() {
        next_state.set(AppState::Flight);
    }
}
