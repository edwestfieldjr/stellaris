//! Bridges the web frontend's on-screen "iPod wheel" control cluster (a
//! React overlay, `web/src/App.jsx`) into the game's input systems.
//!
//! That overlay lives outside the `<canvas>` element entirely, so touches on
//! it never reach winit's `Touches` resource the way aiming/firing normally
//! does — there's no DOM event for Bevy to see. Instead the JS side calls a
//! couple of `#[wasm_bindgen]`-exported functions directly, which stash
//! their values in a small piece of global state; `poll_virtual_input` reads
//! that state once per frame into ordinary Bevy resources that `flight.rs`
//! consumes exactly like keyboard/mouse/touch input.
use bevy::prelude::*;

/// Accumulated drag movement from the on-screen wheel since the last poll,
/// in the same trackpad/mouse sense as a `MouseMotion` delta — not a
/// position, so `move_crosshair` adds it straight onto the crosshair rather
/// than treating it as a held direction. Reset to zero every poll.
#[derive(Resource, Default, Clone, Copy)]
pub struct VirtualNudge(pub Vec2);

/// Set when the on-screen FIRE button is pressed; consumed (reset to
/// `false`) by `shoot` the same frame it's handled, so it behaves as a
/// single-shot edge trigger like Space/click/tap rather than a held-down
/// repeat.
#[derive(Resource, Default)]
pub struct VirtualFirePending(pub bool);

pub struct VirtualInputPlugin;

impl Plugin for VirtualInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VirtualNudge>()
            .init_resource::<VirtualFirePending>()
            .add_systems(PreUpdate, poll_virtual_input);
    }
}

fn poll_virtual_input(mut nudge: ResMut<VirtualNudge>, mut fire: ResMut<VirtualFirePending>) {
    let (x, y, fired) = bridge::poll();
    nudge.0 = Vec2::new(x, y);
    if fired {
        fire.0 = true;
    }
}

#[cfg(target_arch = "wasm32")]
mod bridge {
    use std::sync::{Mutex, OnceLock};
    use wasm_bindgen::prelude::*;

    #[derive(Default)]
    struct State {
        nudge_x: f32,
        nudge_y: f32,
        fire_pending: bool,
    }

    fn state() -> &'static Mutex<State> {
        static STATE: OnceLock<Mutex<State>> = OnceLock::new();
        STATE.get_or_init(|| Mutex::new(State::default()))
    }

    /// Called from JS on every drag-move over the virtual wheel with the
    /// pointer's movement *since the last call* (CSS pixels, screen-space).
    /// Accumulates rather than overwrites: several pointermove events can
    /// land within one Bevy frame, and each one's motion should count.
    #[wasm_bindgen]
    pub fn nudge_virtual_stick(dx: f32, dy: f32) {
        let mut s = state().lock().unwrap();
        s.nudge_x += dx;
        s.nudge_y += dy;
    }

    /// Called from JS once per press of the wheel's center FIRE button.
    #[wasm_bindgen]
    pub fn trigger_virtual_fire() {
        state().lock().unwrap().fire_pending = true;
    }

    pub(super) fn poll() -> (f32, f32, bool) {
        let mut s = state().lock().unwrap();
        let nx = std::mem::take(&mut s.nudge_x);
        let ny = std::mem::take(&mut s.nudge_y);
        let fired = std::mem::take(&mut s.fire_pending);
        (nx, ny, fired)
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod bridge {
    pub(super) fn poll() -> (f32, f32, bool) {
        (0.0, 0.0, false)
    }
}
