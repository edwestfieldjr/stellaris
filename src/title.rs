use bevy::input::touch::Touches;
use bevy::prelude::*;

use crate::galaxy_map::GalaxyGrid;
use crate::hud::credits_closed;
use crate::state::{AppState, Campaign};

#[derive(Component)]
struct TitleUi;

pub struct TitlePlugin;

impl Plugin for TitlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Title), setup)
            .add_systems(OnExit(AppState::Title), teardown)
            .add_systems(
                Update,
                (start_input, quit_input)
                    .run_if(in_state(AppState::Title).and_then(credits_closed)),
            );
    }
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let title_font: Handle<Font> = asset_server.load("fonts/Audiowide-Regular.ttf");

    commands.spawn((
        Text::new("ZERLAK FRONTIER"),
        TextFont {
            font: bevy::text::FontSource::Handle(title_font),
            font_size: bevy::text::FontSize::Px(44.0),
            ..default()
        },
        TextColor(Color::srgb(0.45, 0.8, 1.0)),
        TextLayout::default().with_justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(140.0),
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            ..default()
        },
        TitleUi,
    ));

    commands.spawn((
        Text::new("A Zerlak incursion threatens the frontier."),
        TextFont {
            font_size: bevy::text::FontSize::Px(18.0),
            ..default()
        },
        TextColor(Color::srgb(0.75, 0.75, 0.8)),
        TextLayout::default().with_justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(220.0),
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            ..default()
        },
        TitleUi,
    ));

    commands.spawn((
        Text::new(
            "GALAXY MAP - Arrows/mouse: pick a sector   Enter/Space/click: warp in\n\
             Red = Zerlak (fight it)   Blue = Friendly (refuel)   decide fast, or one gets picked for you\n\
             \n\
             FLIGHT - Arrows/mouse: aim   Space/click: fire\n\
             Dodge a charging enemy laser by moving your crosshair clear before it fires\n\
             \n\
             Fuel or health hits zero and the mission ends. Esc always backs out a screen.",
        ),
        TextFont {
            font_size: bevy::text::FontSize::Px(15.0),
            ..default()
        },
        TextColor(Color::srgb(0.68, 0.7, 0.75)),
        TextLayout::default().with_justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(300.0),
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            ..default()
        },
        TitleUi,
    ));

    commands.spawn((
        Text::new("Enter / Click / Tap: launch     Esc: quit"),
        TextFont {
            font_size: bevy::text::FontSize::Px(18.0),
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
        TitleUi,
    ));
}

fn teardown(mut commands: Commands, query: Query<Entity, With<TitleUi>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}

fn start_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if keys.just_pressed(KeyCode::Enter)
        || mouse.just_pressed(MouseButton::Left)
        || touches.any_just_pressed()
    {
        commands.insert_resource(Campaign::default());
        commands.insert_resource(GalaxyGrid::generate(1));
        next_state.set(AppState::GalaxyMap);
    }
}

fn quit_input(keys: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}
