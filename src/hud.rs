use bevy::audio::{GlobalVolume, Volume};
use bevy::prelude::*;

const BTN_ON_COLOR: Color = Color::srgb(0.3, 1.0, 0.6);
const BTN_OFF_COLOR: Color = Color::srgb(0.4, 0.4, 0.45);

#[derive(Resource)]
struct Muted(bool);

impl Default for Muted {
    fn default() -> Self {
        Self(false)
    }
}

#[derive(Resource, Default)]
pub(crate) struct CreditsOpen(bool);

/// Run condition for gameplay input systems: `true` unless the credits
/// panel is open, so a click that closes it can't also register as a shot,
/// a sector pick, or any other in-game action underneath.
pub fn credits_closed(credits_open: Res<CreditsOpen>) -> bool {
    !credits_open.0
}

#[derive(Component)]
struct MuteButton;

#[derive(Component)]
struct MuteButtonLabel;

#[derive(Component)]
struct CreditsButton;

#[derive(Component)]
struct CreditsPanel;

#[derive(Component)]
struct CreditsCloseButton;

/// A speaker-mute toggle and a credits button, both spawned once at
/// startup (not tied to any `AppState`) so they stay put in the corner
/// across every screen instead of being despawned on each state's
/// teardown.
pub struct PersistentUiPlugin;

impl Plugin for PersistentUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Muted>()
            .init_resource::<CreditsOpen>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                    toggle_mute,
                    open_credits,
                    close_credits,
                    sync_credits_panel,
                    pause_while_credits_open,
                )
                    .chain(),
            );
    }
}

fn setup(mut commands: Commands) {
    // Mute toggle, top-right corner. The label is plain ASCII (not a
    // speaker emoji) since neither the embedded default font nor the web
    // build's font reliably has emoji glyphs — same class of bug the
    // em-dashes hit earlier.
    commands
        .spawn((
            Button,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                right: Val::Px(10.0),
                width: Val::Px(34.0),
                height: Val::Px(34.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(17.0)),
                ..default()
            },
            BackgroundColor(BTN_ON_COLOR),
            GlobalZIndex(100),
            MuteButton,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(")))"),
                TextFont {
                    font_size: bevy::text::FontSize::Px(13.0),
                    ..default()
                },
                TextColor(Color::BLACK),
                MuteButtonLabel,
            ));
        });

    // Credits button, stacked just below the mute toggle.
    commands
        .spawn((
            Button,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(52.0),
                right: Val::Px(10.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.15, 0.15, 0.2, 0.85)),
            GlobalZIndex(100),
            CreditsButton,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("CREDITS"),
                TextFont {
                    font_size: bevy::text::FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgb(0.8, 0.8, 0.85)),
            ));
        });
}

fn toggle_mute(
    interactions: Query<&Interaction, (Changed<Interaction>, With<MuteButton>)>,
    mut muted: ResMut<Muted>,
    mut global_volume: ResMut<GlobalVolume>,
    mut bg_query: Query<&mut BackgroundColor, With<MuteButton>>,
    mut label_query: Query<&mut Text, With<MuteButtonLabel>>,
) {
    if !interactions.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    muted.0 = !muted.0;
    global_volume.volume = if muted.0 {
        Volume::Linear(0.0)
    } else {
        Volume::Linear(1.0)
    };
    if let Ok(mut bg) = bg_query.single_mut() {
        bg.0 = if muted.0 { BTN_OFF_COLOR } else { BTN_ON_COLOR };
    }
    if let Ok(mut text) = label_query.single_mut() {
        **text = if muted.0 { "X".to_string() } else { ")))".to_string() };
    }
}

fn open_credits(
    interactions: Query<&Interaction, (Changed<Interaction>, With<CreditsButton>)>,
    mut credits_open: ResMut<CreditsOpen>,
) {
    if interactions.iter().any(|i| *i == Interaction::Pressed) {
        credits_open.0 = true;
    }
}

fn close_credits(
    keys: Res<ButtonInput<KeyCode>>,
    interactions: Query<&Interaction, (Changed<Interaction>, With<CreditsCloseButton>)>,
    mut credits_open: ResMut<CreditsOpen>,
) {
    if keys.just_pressed(KeyCode::Escape) || interactions.iter().any(|i| *i == Interaction::Pressed)
    {
        credits_open.0 = false;
    }
}

/// Spawns or despawns the credits panel to match `CreditsOpen`, only when
/// it actually changed this frame.
fn sync_credits_panel(
    mut commands: Commands,
    credits_open: Res<CreditsOpen>,
    panel_query: Query<Entity, With<CreditsPanel>>,
) {
    if !credits_open.is_changed() {
        return;
    }
    if credits_open.0 {
        if panel_query.is_empty() {
            spawn_credits_panel(&mut commands);
        }
    } else {
        for entity in &panel_query {
            commands.entity(entity).despawn();
        }
    }
}

/// Freezes simulation time while the credits panel is up (enemy movement,
/// warp animation, spawn/laser timers, ...) and resumes it on close. Input
/// systems are separately gated by `credits_closed` so nothing underneath
/// reacts to clicks meant for the panel.
fn pause_while_credits_open(credits_open: Res<CreditsOpen>, mut time: ResMut<Time<Virtual>>) {
    if !credits_open.is_changed() {
        return;
    }
    if credits_open.0 {
        time.pause();
    } else {
        time.unpause();
    }
}

fn spawn_credits_panel(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75)),
            GlobalZIndex(200),
            CreditsPanel,
        ))
        .with_children(|backdrop| {
            backdrop
                .spawn((
                    Node {
                        width: Val::Px(560.0),
                        padding: UiRect::all(Val::Px(28.0)),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(10.0),
                        border_radius: BorderRadius::all(Val::Px(8.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.09, 0.1, 0.13)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("CREDITS"),
                        TextFont {
                            font_size: bevy::text::FontSize::Px(28.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.45, 0.8, 1.0)),
                    ));
                    panel.spawn((
                        Text::new(
                            "Zerlak Frontier\n\
                             An unofficial, non-commercial fan tribute inspired by Solaris\n\
                             (Atari, 1986) by Douglas Neubauer. Not affiliated with Atari.\n\
                             \n\
                             Built in Rust with the Bevy game engine (bevyengine.org)\n\
                             Written with Claude Code (claude.com/claude-code)\n\
                             Title font: Audiowide by Brian J. Bonislawsky (OFL 1.1)\n\
                             \n\
                             Licensed under the PolyForm Noncommercial License 1.0.0\n\
                             github.com/edwestfieldjr/zerlak-frontier",
                        ),
                        TextFont {
                            font_size: bevy::text::FontSize::Px(15.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.78, 0.8, 0.85)),
                        TextLayout::default().with_justify(Justify::Center),
                    ));
                    panel
                        .spawn((
                            Button,
                            Node {
                                margin: UiRect::top(Val::Px(8.0)),
                                padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.3, 1.0, 0.6)),
                            CreditsCloseButton,
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Text::new("CLOSE"),
                                TextFont {
                                    font_size: bevy::text::FontSize::Px(16.0),
                                    ..default()
                                },
                                TextColor(Color::BLACK),
                            ));
                        });
                });
        });
}
