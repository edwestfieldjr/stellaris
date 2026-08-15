mod flight;
mod galaxy_map;
mod game_over;
mod hud;
mod mouse;
mod state;
mod title;
mod warp;

use bevy::prelude::*;

use flight::FlightPlugin;
use galaxy_map::GalaxyMapPlugin;
use game_over::GameOverPlugin;
use hud::PersistentUiPlugin;
use state::{AppState, Campaign, WarpTarget};
use title::TitlePlugin;
use warp::WarpPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
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
        ))
        .add_systems(Startup, spawn_camera)
        .run();
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
