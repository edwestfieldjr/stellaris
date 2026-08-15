use bevy::prelude::*;

/// Top-level game mode.
#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum AppState {
    #[default]
    Title,
    GalaxyMap,
    /// Brief transition animation played on the way into a Zerlak sector, so
    /// the jump from map to combat reads as one continuous flight.
    Warp,
    Flight,
    GameOver,
}

/// Persistent player/campaign data that survives transitions between
/// states, and across a full run (Title resets it).
#[derive(Resource)]
pub struct Campaign {
    pub fuel: f32,
    pub health: f32,
    pub sector: (i32, i32),
    /// Increments each time a galaxy is fully cleared; drives difficulty
    /// (grid size, Zerlak density, enemy speed) of the next one.
    pub level: u32,
    /// Set when a run ends, so the GameOver screen can say why.
    pub defeat_reason: Option<DefeatReason>,
}

#[derive(Clone, Copy, Debug)]
pub enum DefeatReason {
    OutOfFuel,
    Destroyed,
}

impl Default for Campaign {
    fn default() -> Self {
        Self {
            fuel: 100.0,
            health: 100.0,
            sector: (0, 0),
            level: 1,
            defeat_reason: None,
        }
    }
}
