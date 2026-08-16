use bevy::audio::{AudioPlayer, AudioSource, GlobalVolume, PlaybackSettings, Volume};
use bevy::prelude::*;

use crate::hud_bridge::{HudRequests, HudStats};

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
struct CreditsPanel;

#[derive(Component)]
struct CreditsCloseButton;

/// Mute state and the credits panel — no on-canvas trigger buttons here
/// anymore (mute/credits buttons live in the React-rendered HUD overlay
/// instead, see `hud_bridge.rs`: Bevy's fixed 900x650 canvas space stops
/// being pinned to the *screen's* corners the moment the web frontend's
/// cover-fit layout crops it to cover an arbitrary aspect ratio). The
/// credits panel itself stays here — it's centered content, not
/// corner-pinned, so that cropping isn't a problem for it.
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

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Ambient music bed: a single looping track spawned once at startup (not
    // tied to any AppState) so it plays continuously under the title,
    // galaxy map, flight, and warp screens alike instead of restarting or
    // cutting out on every transition. Sits well under the SFX volume, and
    // rides the same `GlobalVolume` mute toggle as everything else below.
    commands.spawn((
        AudioPlayer(asset_server.load::<AudioSource>("sounds/music_bed.wav")),
        PlaybackSettings::LOOP.with_volume(Volume::Linear(0.22)),
    ));
}

fn toggle_mute(
    mut requests: ResMut<HudRequests>,
    mut muted: ResMut<Muted>,
    mut global_volume: ResMut<GlobalVolume>,
    mut hud_stats: ResMut<HudStats>,
) {
    if !std::mem::take(&mut requests.toggle_mute) {
        return;
    }
    muted.0 = !muted.0;
    global_volume.volume = if muted.0 {
        Volume::Linear(0.0)
    } else {
        Volume::Linear(1.0)
    };
    // Field write, not a full struct replace: keeps whatever
    // remaining/health/fuel/score `HudStats` already had.
    hud_stats.muted = muted.0;
}

fn open_credits(mut requests: ResMut<HudRequests>, mut credits_open: ResMut<CreditsOpen>) {
    if std::mem::take(&mut requests.open_credits) {
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
