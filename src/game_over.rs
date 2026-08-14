use bevy::input::touch::Touches;
use bevy::prelude::*;

use crate::state::{AppState, Campaign, DefeatReason};

#[derive(Component)]
struct GameOverUi;

pub struct GameOverPlugin;

impl Plugin for GameOverPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::GameOver), setup)
            .add_systems(OnExit(AppState::GameOver), teardown)
            .add_systems(
                Update,
                continue_input.run_if(in_state(AppState::GameOver)),
            );
    }
}

fn setup(mut commands: Commands, campaign: Res<Campaign>) {
    let reason = match campaign.defeat_reason {
        Some(DefeatReason::OutOfFuel) => "Your ship drifted, out of fuel, deep in Zylon space.",
        Some(DefeatReason::Destroyed) => "Your ship was destroyed by Zylon fire.",
        None => "Mission ended.",
    };

    commands.spawn((
        Text::new("MISSION FAILED"),
        TextFont {
            font_size: bevy::text::FontSize::Px(56.0),
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.25, 0.25)),
        TextLayout::default().with_justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(180.0),
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            ..default()
        },
        GameOverUi,
    ));

    commands.spawn((
        Text::new(reason),
        TextFont {
            font_size: bevy::text::FontSize::Px(18.0),
            ..default()
        },
        TextColor(Color::srgb(0.8, 0.8, 0.8)),
        TextLayout::default().with_justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(260.0),
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            ..default()
        },
        GameOverUi,
    ));

    commands.spawn((
        Text::new(format!("Reached level {}", campaign.level)),
        TextFont {
            font_size: bevy::text::FontSize::Px(18.0),
            ..default()
        },
        TextColor(Color::srgb(0.7, 0.7, 0.75)),
        TextLayout::default().with_justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(292.0),
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            ..default()
        },
        GameOverUi,
    ));

    commands.spawn((
        Text::new("Enter / Click / Tap: return to title"),
        TextFont {
            font_size: bevy::text::FontSize::Px(16.0),
            ..default()
        },
        TextColor(Color::srgb(0.6, 0.6, 0.65)),
        TextLayout::default().with_justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(60.0),
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            ..default()
        },
        GameOverUi,
    ));
}

fn teardown(mut commands: Commands, query: Query<Entity, With<GameOverUi>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}

fn continue_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if keys.just_pressed(KeyCode::Enter)
        || mouse.just_pressed(MouseButton::Left)
        || touches.any_just_pressed()
    {
        next_state.set(AppState::Title);
    }
}
