use bevy::audio::{
    AudioPlayer, AudioSink, AudioSinkPlayback, AudioSource, GlobalVolume, PlaybackSettings, Volume,
};
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
struct MusicTrack;

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
                    respawn_music_on_unlock,
                )
                    .chain(),
            );
    }
}

const MUSIC_VOLUME: f32 = 0.22;

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Ambient music bed: a single looping track spawned once at startup (not
    // tied to any AppState) so it plays continuously under the title,
    // galaxy map, flight, and warp screens alike instead of restarting or
    // cutting out on every transition. Sits well under the SFX volume.
    //
    // Doesn't rely on `GlobalVolume` for muting, unlike the SFX below:
    // bevy_audio only multiplies a sink's volume by `GlobalVolume` once, at
    // the moment the sink is *created* (see `play_queued_audio_system` in
    // bevy_audio) — it never revisits already-playing sinks when
    // `GlobalVolume` changes later. That's invisible for the SFX (each one
    // is freshly spawned, and so freshly volume-set, every time it plays)
    // but this sink is created exactly once at startup and then loops
    // forever, so it never sees a later mute toggle at all. `toggle_mute`
    // below mutes this specific sink directly instead.
    commands.spawn((
        AudioPlayer(asset_server.load::<AudioSource>("sounds/music_bed.wav")),
        PlaybackSettings::LOOP.with_volume(Volume::Linear(MUSIC_VOLUME)),
        MusicTrack,
    ));
}

fn toggle_mute(
    mut requests: ResMut<HudRequests>,
    mut muted: ResMut<Muted>,
    mut global_volume: ResMut<GlobalVolume>,
    mut hud_stats: ResMut<HudStats>,
    mut music_sink: Query<&mut AudioSink, With<MusicTrack>>,
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
    // GlobalVolume only affects sinks at the moment they're created (see
    // the comment in `setup`) — the music sink already exists by now, so
    // it needs to be muted directly.
    if let Ok(mut sink) = music_sink.single_mut() {
        if muted.0 {
            sink.mute();
        } else {
            sink.unmute();
        }
    }
    // Field write, not a full struct replace: keeps whatever
    // remaining/health/fuel/score `HudStats` already had.
    hud_stats.muted = muted.0;
}

/// The music track is spawned once at app startup, well before the player
/// has interacted with the page at all — its sink gets created against an
/// AudioContext that's still suspended (see the comment in `setup`). On
/// most browsers, later resuming that same context object is enough to
/// un-stick it, but iOS Safari/Chrome doesn't reliably honor that: a sink
/// created against a context that wasn't already running at creation time
/// can just stay silent even once the context itself reports `running`.
/// Despawning and respawning the track fresh, once there's finally been a
/// real gesture (`web/src/unlock-audio.js` calls through on the first
/// successful resume), creates a brand new sink against a context that's
/// actually running by the time it's built — the same path a freshly
/// spawned SFX sound already takes every time it plays, which is why SFX
/// never had this problem to begin with.
fn respawn_music_on_unlock(
    mut requests: ResMut<HudRequests>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    muted: Res<Muted>,
    existing: Query<Entity, With<MusicTrack>>,
) {
    if !std::mem::take(&mut requests.audio_unlocked) {
        return;
    }
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    let mut settings = PlaybackSettings::LOOP.with_volume(Volume::Linear(MUSIC_VOLUME));
    settings.muted = muted.0;
    commands.spawn((
        AudioPlayer(asset_server.load::<AudioSource>("sounds/music_bed.wav")),
        settings,
        MusicTrack,
    ));
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
