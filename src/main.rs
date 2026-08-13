mod flight;
mod galaxy_map;
mod mouse;
mod state;

use bevy::prelude::*;

use flight::FlightPlugin;
use galaxy_map::GalaxyMapPlugin;
use state::{AppState, Campaign};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Solaris".to_string(),
                resolution: bevy::window::WindowResolution::new(900, 650),
                ..default()
            }),
            ..default()
        }))
        .init_state::<AppState>()
        .init_resource::<Campaign>()
        .add_plugins((GalaxyMapPlugin, FlightPlugin))
        .add_systems(Startup, spawn_camera)
        .run();
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
