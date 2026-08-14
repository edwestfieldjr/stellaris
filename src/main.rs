mod flight;
mod galaxy_map;
mod game_over;
mod mouse;
mod state;
mod title;
mod warp;

use bevy::prelude::*;

use flight::FlightPlugin;
use galaxy_map::GalaxyMapPlugin;
use game_over::GameOverPlugin;
use state::{AppState, Campaign};
use title::TitlePlugin;
use warp::WarpPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Stellaris".to_string(),
                resolution: bevy::window::WindowResolution::new(900, 650),
                // On web, target the canvas the host page provides and let
                // it drive the resolution (the page keeps it letterboxed to
                // this same 900x650-ish aspect ratio via CSS, so none of
                // the game's hardcoded layout math needs to change).
                canvas: Some("#stellaris-canvas".to_string()),
                fit_canvas_to_parent: true,
                prevent_default_event_handling: true,
                ..default()
            }),
            ..default()
        }))
        .init_state::<AppState>()
        .init_resource::<Campaign>()
        .add_plugins((
            TitlePlugin,
            GalaxyMapPlugin,
            WarpPlugin,
            FlightPlugin,
            GameOverPlugin,
        ))
        .add_systems(Startup, spawn_camera)
        .run();
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
