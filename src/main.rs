mod flight;
mod galaxy_map;
mod game_over;
mod hud;
mod hud_bridge;
mod mouse;
mod state;
mod title;
mod virtual_input;
mod warp;

use bevy::prelude::*;

use flight::FlightPlugin;
use galaxy_map::GalaxyMapPlugin;
use game_over::GameOverPlugin;
use hud::PersistentUiPlugin;
use hud_bridge::HudBridgePlugin;
use state::{AppState, Campaign, WarpTarget};
use title::TitlePlugin;
use virtual_input::VirtualInputPlugin;
use warp::WarpPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            // Bevy's asset server normally probes for a `<file>.meta`
            // sidecar next to every asset (per-asset loader settings). On
            // the web build there's no such file for anything we ship, and
            // static hosts like GitHub Pages (and the local dev/preview
            // server) don't return a real 404 for it — they fall back to
            // serving `index.html` with a 200. Bevy then tries to parse
            // that HTML as the meta file's RON format, fails, and the
            // asset load itself never completes (silently: no sound, no
            // font). Telling it to never check for meta files at all
            // avoids the request entirely.
            meta_check: bevy::asset::AssetMetaCheck::Never,
            ..default()
        }).set(WindowPlugin {
            primary_window: Some(Window {
                title: "Zerlak Frontier".to_string(),
                resolution: bevy::window::WindowResolution::new(900, 650),
                // On web, target the canvas the host page provides. Note
                // fit_canvas_to_parent is deliberately NOT set: that would
                // shrink Bevy's actual render resolution to match a small
                // phone viewport, and every hardcoded pixel offset in the
                // game's UI is calibrated for a 900x650 canvas — at a
                // smaller internal resolution it would all read as
                // comically oversized. Instead Bevy always renders at a
                // fixed 900x650 and the page's CSS scales that canvas
                // element visually to fit the screen (see web/src/App.css),
                // the same way a pixel-art game scales up cleanly.
                canvas: Some("#game-canvas".to_string()),
                prevent_default_event_handling: true,
                ..default()
            }),
            ..default()
        }))
        .init_state::<AppState>()
        .init_resource::<Campaign>()
        .init_resource::<WarpTarget>()
        .add_plugins((
            TitlePlugin,
            GalaxyMapPlugin,
            WarpPlugin,
            FlightPlugin,
            GameOverPlugin,
            PersistentUiPlugin,
            VirtualInputPlugin,
            HudBridgePlugin,
        ))
        .add_systems(Startup, spawn_camera)
        .run();
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
