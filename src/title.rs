use bevy::input::touch::Touches;
use bevy::prelude::*;

use crate::galaxy_map::GalaxyGrid;
use crate::hud::credits_closed;
use crate::hud_bridge::{ScreenKind, ScreenText};
use crate::state::{AppState, Campaign};

pub struct TitlePlugin;

impl Plugin for TitlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Title), enter)
            .add_systems(
                Update,
                (start_input, quit_input)
                    .run_if(in_state(AppState::Title).and_then(credits_closed)),
            );
    }
}

/// No Bevy-rendered UI on this screen at all — its text is entirely
/// static, so the React overlay just hardcodes it (matching what used to
/// be spawned here) and shows it whenever `ScreenText.screen` says
/// `Title`, rather than round-tripping unchanging strings through the
/// bridge every frame. See `hud_bridge.rs` for why this moved off Bevy's
/// own UI in the first place.
fn enter(mut screen_text: ResMut<ScreenText>) {
    *screen_text = ScreenText {
        screen: ScreenKind::Title,
        ..default()
    };
}

fn start_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if keys.just_pressed(KeyCode::Enter)
        || mouse.just_pressed(MouseButton::Left)
        || touches.any_just_pressed()
    {
        commands.insert_resource(Campaign::default());
        commands.insert_resource(GalaxyGrid::generate(1));
        next_state.set(AppState::GalaxyMap);
    }
}

fn quit_input(keys: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}
