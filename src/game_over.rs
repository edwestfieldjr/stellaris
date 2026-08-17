use bevy::input::touch::Touches;
use bevy::prelude::*;

use crate::galaxy_map::GalaxyGrid;
use crate::hud::credits_closed;
use crate::hud_bridge::{HudRequests, ScreenKind, ScreenText};
use crate::state::{AppState, Campaign, DefeatReason};

pub struct GameOverPlugin;

impl Plugin for GameOverPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::GameOver), enter)
            .add_systems(
                Update,
                (continue_input, restart_input)
                    .run_if(in_state(AppState::GameOver).and_then(credits_closed)),
            );
    }
}

/// No Bevy-rendered UI on this screen either (see the comment in
/// `title.rs`) — "MISSION FAILED" and the continue prompt are static, so
/// React hardcodes them; only the actual defeat reason and level reached
/// vary, so those are the only two values pushed through.
fn enter(campaign: Res<Campaign>, mut screen_text: ResMut<ScreenText>) {
    let reason = match campaign.defeat_reason {
        Some(DefeatReason::OutOfFuel) => "Your ship drifted, out of fuel, deep in Zerlak space.",
        Some(DefeatReason::Destroyed) => "Your ship was destroyed by Zerlak fire.",
        None => "Mission ended.",
    };
    *screen_text = ScreenText {
        screen: ScreenKind::GameOver,
        level: campaign.level,
        defeat_reason: reason.to_string(),
        ..default()
    };
}

fn continue_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if keys.just_pressed(KeyCode::Enter)
        || mouse.just_pressed(MouseButton::Left)
        || touches.any_just_pressed()
    {
        next_state.set(AppState::Title);
    }
}

/// React's "PLAY AGAIN" button — starts a brand new run directly, same
/// setup as the title screen's own launch (`title.rs::start_input`), just
/// skipping back through the title screen itself.
fn restart_input(
    mut requests: ResMut<HudRequests>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if !std::mem::take(&mut requests.restart_game) {
        return;
    }
    commands.insert_resource(Campaign::default());
    commands.insert_resource(GalaxyGrid::generate(1));
    next_state.set(AppState::GalaxyMap);
}
