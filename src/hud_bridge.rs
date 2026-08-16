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

/// Which non-Flight screen (if any) the React overlay should render text
/// for. Everything on these screens was, like the Flight HUD `HudStats`
/// above, plain Bevy text pinned to the fixed 900x650 canvas space — with
/// the same cropping problem once cover-fit started covering an arbitrary
/// aspect ratio instead of letterboxing.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum ScreenKind {
    #[default]
    None,
    Title,
    GalaxyMap,
    GameOver,
}

impl ScreenKind {
    fn as_str(self) -> &'static str {
        match self {
            ScreenKind::None => "",
            ScreenKind::Title => "title",
            ScreenKind::GalaxyMap => "galaxy_map",
            ScreenKind::GameOver => "game_over",
        }
    }
}

/// Live values for whichever screen is current; fields not relevant to
/// that screen are left at their defaults and simply ignored on the React
/// side. The title screen's own copy and the galaxy map's instructions
/// line are static enough that React just hardcodes them (matching what
/// used to be in `title.rs`/`galaxy_map.rs`) rather than needing them
/// pushed through here too.
#[derive(Resource, Default, Clone, PartialEq)]
pub struct ScreenText {
    pub screen: ScreenKind,
    /// Galaxy map.
    pub fuel: f32,
    pub level: u32,
    /// Galaxy map: "LEVEL N - the Zerlak Empire regroups..." while it's
    /// showing, empty otherwise.
    pub banner: String,
    /// Galaxy map: seconds left before a sector is auto-picked.
    pub countdown: f32,
    /// Game over.
    pub defeat_reason: String,
}

pub struct HudBridgePlugin;

impl Plugin for HudBridgePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HudStats>()
            .init_resource::<HudRequests>()
            .init_resource::<ScreenText>()
            .add_systems(PreUpdate, poll_hud_requests)
            .add_systems(Update, push_hud_stats.run_if(resource_changed::<HudStats>))
            .add_systems(
                Update,
                push_screen_text.run_if(resource_changed::<ScreenText>),
            );
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

fn push_screen_text(text: Res<ScreenText>) {
    bridge::push_screen(
        text.screen.as_str(),
        text.fuel,
        text.level,
        &text.banner,
        text.countdown,
        &text.defeat_reason,
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

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = window, js_name = __zerlakScreenTextUpdate)]
        fn zerlak_screen_text_update(
            screen: &str,
            fuel: f32,
            level: u32,
            banner: &str,
            countdown: f32,
            defeat_reason: &str,
        );
    }

    pub(super) fn push_screen(
        screen: &str,
        fuel: f32,
        level: u32,
        banner: &str,
        countdown: f32,
        defeat_reason: &str,
    ) {
        zerlak_screen_text_update(screen, fuel, level, banner, countdown, defeat_reason);
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

    pub(super) fn push_screen(
        _screen: &str,
        _fuel: f32,
        _level: u32,
        _banner: &str,
        _countdown: f32,
        _defeat_reason: &str,
    ) {
    }
}
