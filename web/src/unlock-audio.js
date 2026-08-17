// The game's audio (Rust, via rodio/cpal) opens its Web Audio AudioContext
// at app startup, before the player has interacted with the page at all.
// Browsers only let an AudioContext actually start producing sound once
// there has been a real user gesture — otherwise it's created (and stays)
// "suspended" forever, with no error anywhere: playback calls all silently
// succeed and produce no sound. iOS Safari in particular does NOT
// auto-resume a suspended context on later interaction; it only resumes one
// that's `.resume()`d from directly inside a genuine gesture handler. Since
// the Rust side's own resume() attempt happens before any gesture exists,
// it doesn't count.
//
// Fix: intercept every AudioContext the page creates (imported before the
// wasm module, so this runs before the game creates its own), and resume
// all of them on the player's first tap/click/key press anywhere on the
// page. This is the same shim other wasm/web game engines (Unity WebGL,
// Godot, etc.) ship for the same reason.
const audioContexts = []

for (const name of ['AudioContext', 'webkitAudioContext']) {
  const Native = window[name]
  if (!Native) continue
  window[name] = new Proxy(Native, {
    construct(target, args) {
      const ctx = new target(...args)
      audioContexts.push(ctx)
      return ctx
    },
  })
}

// Tells Rust (see hud_bridge.rs) to despawn and respawn the music track once
// there's been a real, *confirmed* unlock — its sink was created at app
// startup against a context that was still suspended, and on iOS resuming
// that same context object later doesn't reliably un-stick it. Fires only
// once, and only after a resume() call actually resolves (not just on any
// gesture), so the fresh sink is built against a context that's truly
// running by then, the same way a freshly spawned SFX sound already is
// every time it plays.
let notified = false
function notifyUnlocked() {
  if (notified) return
  notified = true
  window.__zerlakNotifyAudioUnlocked?.()
}

function unlockAudio() {
  for (const ctx of audioContexts) {
    // Not just 'suspended': iOS Safari/Chrome also has an 'interrupted'
    // state (the OS audio session got preempted — another app, a phone
    // call, the silent switch, or sometimes just how the context starts
    // out) that a plain `state === 'suspended'` check silently misses,
    // leaving resume() never even attempted. Try on anything that isn't
    // already the one state that means "actually playing".
    if (ctx.state !== 'running') {
      ctx.resume().then(notifyUnlocked).catch(() => {})
    } else {
      notifyUnlocked()
    }
  }
}

// Capture phase, not bubble: winit's own touch handling on the game canvas
// calls stopPropagation (to block the browser's default gesture handling —
// pull-to-refresh, pinch-zoom, and so on), which would otherwise silently
// swallow every tap before it ever reached a bubble-phase listener on
// `window`. Capture-phase listeners run on the way *down* to the target,
// before the canvas's own handler gets a chance to stop anything — this is
// almost certainly why muting/unmuting a real desktop click worked while a
// tap on the actual game canvas on a phone never unlocked audio at all.
for (const evt of ['pointerdown', 'touchstart', 'touchend', 'mousedown', 'keydown']) {
  window.addEventListener(evt, unlockAudio, { passive: true, capture: true })
}
