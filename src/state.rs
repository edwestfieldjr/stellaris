use bevy::prelude::*;

/// Top-level game mode, mirroring the two views of the original Atari 2600
/// *Solaris*: the strategic galaxy map, and first-person sector combat.
#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum AppState {
    #[default]
    GalaxyMap,
    Flight,
}

/// Persistent player/campaign data that survives transitions between states.
#[derive(Resource)]
pub struct Campaign {
    pub fuel: f32,
    pub sector: (i32, i32),
}

impl Default for Campaign {
    fn default() -> Self {
        Self {
            fuel: 100.0,
            sector: (0, 0),
        }
    }
}
