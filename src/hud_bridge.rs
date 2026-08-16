//! Bridges HUD state between Bevy and the React-rendered web overlay.
//!
//! On the web build, UI pinned to Bevy's own fixed 900x650 canvas space
//! stops being pinned to the *screen's* corners the moment the frontend's
//! cover-fit layout crops that canvas to cover an arbitrary aspect ratio
//! (see `web/src/App.css`) — a corner element can end up outside the
//! visible region entirely. React's DOM overlay doesn't have that problem
//! (it's positioned relative to the real viewport), so the mute/credits
//! buttons and the live stat readout (remaining/health/fuel/score) are
//! rendered there instead, fed by this bridge:
//! - Rust -> JS: `push_hud_stats` calls a JS global with the current
//!   values whenever they change.
//! - JS -> Rust: `toggle_mute`/`request_credits_open` (wasm-bindgen
//!   exports) stash a request that `poll_hud_requests` applies to the
//!   normal Bevy resources once per frame — the same pattern
//!   `virtual_input.rs` uses for the on-screen wheel.
use bevy::prelude::*;

/// Live values pushed out to the React HUD overlay. `visible` gates the
/// stat readout only (remaining/health/fuel/score) — Flight is the only
/// state they're meaningful in; `muted` (for the mute button's own icon)
/// stays meaningful everywhere.
#[derive(Resource, Default, Clone, Copy, PartialEq)]
pub struct HudStats {
    pub visible: bool,
    pub remaining: u32,
    pub health: f32,
    pub fuel: f32,
    pub score: u32,
    pub muted: bool,
}

/// Pending JS -> Rust requests, applied once per frame by `hud.rs`'s
/// systems in place of reading a Bevy `Interaction` the way the old
/// on-canvas buttons did.
#[derive(Resource, Default)]
pub struct HudRequests {
    pub toggle_mute: bool,
    pub open_credits: bool,
}

pub struct HudBridgePlugin;

impl Plugin for HudBridgePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HudStats>()
            .init_resource::<HudRequests>()
            .add_systems(PreUpdate, poll_hud_requests)
            .add_systems(Update, push_hud_stats.run_if(resource_changed::<HudStats>));
    }
}

fn poll_hud_requests(mut requests: ResMut<HudRequests>) {
    let (toggle_mute, open_credits) = bridge::poll();
    if toggle_mute {
        requests.toggle_mute = true;
    }
    if open_credits {
        requests.open_credits = true;
    }
}

fn push_hud_stats(stats: Res<HudStats>) {
    bridge::push(
        stats.visible,
        stats.remaining,
        stats.health,
        stats.fuel,
        stats.score,
        stats.muted,
    );
}

#[cfg(target_arch = "wasm32")]
mod bridge {
    use std::sync::{Mutex, OnceLock};
    use wasm_bindgen::prelude::*;

    #[derive(Default)]
    struct State {
        toggle_mute: bool,
        open_credits: bool,
    }

    fn state() -> &'static Mutex<State> {
        static STATE: OnceLock<Mutex<State>> = OnceLock::new();
        STATE.get_or_init(|| Mutex::new(State::default()))
    }

    /// Called from JS when the React-rendered mute button is pressed.
    #[wasm_bindgen]
    pub fn toggle_mute() {
        state().lock().unwrap().toggle_mute = true;
    }

    /// Called from JS when the React-rendered credits button is pressed.
    /// There's no matching `close_credits` export: the panel itself is
    /// still Bevy-rendered (it's centered content, not corner-pinned, so
    /// cover-fit cropping isn't a problem for it), and its own close
    /// button plus Escape already close it from inside the game.
    #[wasm_bindgen]
    pub fn request_credits_open() {
        state().lock().unwrap().open_credits = true;
    }

    pub(super) fn poll() -> (bool, bool) {
        let mut s = state().lock().unwrap();
        (std::mem::take(&mut s.toggle_mute), std::mem::take(&mut s.open_credits))
    }

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = window, js_name = __zerlakHudUpdate)]
        fn zerlak_hud_update(
            visible: bool,
            remaining: u32,
            health: f32,
            fuel: f32,
            score: u32,
            muted: bool,
        );
    }

    pub(super) fn push(visible: bool, remaining: u32, health: f32, fuel: f32, score: u32, muted: bool) {
        zerlak_hud_update(visible, remaining, health, fuel, score, muted);
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod bridge {
    pub(super) fn poll() -> (bool, bool) {
        (false, false)
    }

    pub(super) fn push(
        _visible: bool,
        _remaining: u32,
        _health: f32,
        _fuel: f32,
        _score: u32,
        _muted: bool,
    ) {
    }
}
