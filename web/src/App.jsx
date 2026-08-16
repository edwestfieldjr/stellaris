import { useEffect, useRef, useState } from 'react'
import './App.css'

// The game's native, fixed internal resolution — must match
// `WindowResolution::new(...)` in src/main.rs. `.canvas-wrap` in App.css is
// always exactly this size in CSS pixels; we scale it up/down with a
// transform to fill the screen instead of resizing it. See the comment in
// App.css for why that distinction is load-bearing on the web build.
const GAME_WIDTH = 900
const GAME_HEIGHT = 650

// The game's virtual-nudge/fire/mute/credits wasm exports don't exist
// until the module finishes loading (a multi-second, multi-MB download) —
// these no-op stand-ins mean the wheel and HUD buttons are safe to touch
// immediately, before that finishes, without every call needing an
// existence check.
window.__zerlakNudge ??= () => {}
window.__zerlakTriggerFire ??= () => {}
window.__zerlakToggleMute ??= () => {}
window.__zerlakRequestCreditsOpen ??= () => {}

// HUD stat push channel: Rust calls this directly (the opposite direction
// from the wheel/mute/credits calls above) once per changed frame with the
// latest remaining/health/fuel/score/muted. Defined at module scope, before
// the wasm module even starts loading, since the game can start pushing
// from the moment it boots — well before `status` reaches 'ready'.
const hudListeners = new Set()
window.__zerlakHudUpdate = (visible, remaining, health, fuel, score, muted) => {
  const stats = { visible, remaining, health, fuel, score, muted }
  for (const listener of hudListeners) listener(stats)
}

// Same idea as the HUD stat channel above, for whichever of Title/Galaxy
// Map/Game Over is current — see the comment on `ScreenTextOverlay`.
const screenTextListeners = new Set()
window.__zerlakScreenTextUpdate = (screen, fuel, level, banner, countdown, defeatReason) => {
  const text = { screen, fuel, level, banner, countdown, defeatReason }
  for (const listener of screenTextListeners) listener(text)
}

export default function App() {
  const [status, setStatus] = useState('loading') // 'loading' | 'ready' | 'error'
  const started = useRef(false)
  const gameAreaRef = useRef(null)
  const canvasWrapRef = useRef(null)

  useEffect(() => {
    // React 18 StrictMode double-invokes effects in dev; the wasm module
    // can only ever be started once.
    if (started.current) return
    started.current = true

    import('./wasm/solaris.js')
      .then((mod) =>
        mod.default().then(() => {
          // Hand the virtual wheel's live handlers off to the actual game
          // exports now that the wasm module is up, replacing the no-op
          // stand-ins set at module scope below.
          window.__zerlakNudge = mod.nudge_virtual_stick ?? window.__zerlakNudge
          window.__zerlakTriggerFire = mod.trigger_virtual_fire ?? window.__zerlakTriggerFire
          window.__zerlakToggleMute = mod.toggle_mute ?? window.__zerlakToggleMute
          window.__zerlakRequestCreditsOpen =
            mod.request_credits_open ?? window.__zerlakRequestCreditsOpen
        }),
      )
      .then(() => setStatus('ready'))
      .catch((err) => {
        console.error('Failed to start Zerlak Frontier:', err)
        setStatus('error')
      })
  }, [])

  // Fills whatever room `.game-area` actually has with the fixed-size canvas
  // wrapper, purely via a CSS transform (scale + centering translate) —
  // "cover" fit, not "contain": the wrapper is scaled up until it fills the
  // box on both axes, cropping whichever axis overflows, so the playfield
  // always fills the full window edge to edge with no letterbox bars.
  // Never changes the wrapper's (or the canvas's) actual box size — that
  // would trip winit's own resize detection and shrink the game's internal
  // resolution to match. See App.css.
  useEffect(() => {
    const gameArea = gameAreaRef.current
    const canvasWrap = canvasWrapRef.current
    if (!gameArea || !canvasWrap) return

    const applyScale = () => {
      const w = gameArea.clientWidth
      const h = gameArea.clientHeight
      if (w <= 0 || h <= 0) return
      const scale = Math.max(w / GAME_WIDTH, h / GAME_HEIGHT)
      const tx = (w - GAME_WIDTH * scale) / 2
      const ty = (h - GAME_HEIGHT * scale) / 2
      canvasWrap.style.transform = `translate(${tx}px, ${ty}px) scale(${scale})`
    }

    applyScale()
    const observer = new ResizeObserver(applyScale)
    observer.observe(gameArea)
    return () => observer.disconnect()
  }, [])

  return (
    <div className="stage">
      <div className="game-area" ref={gameAreaRef}>
        <div className="canvas-wrap" ref={canvasWrapRef}>
          <canvas id="game-canvas" />
        </div>
        {status !== 'ready' && (
          <div className="overlay">
            {status === 'loading'
              ? 'Loading Zerlak Frontier…'
              : 'Failed to load. Please reload the page.'}
          </div>
        )}
        <FullscreenButton />
        <HudOverlay />
        <ScreenTextOverlay />
      </div>
      {/* Only takes any layout space on tall-portrait screens (see the
          @media rule in App.css) — the game-area above always fills
          whatever's left, so the playfield never shows letterbox bars in
          either layout. */}
      <TouchPad />
    </div>
  )
}

function FullscreenButton() {
  const [isFull, setIsFull] = useState(() => !!document.fullscreenElement)

  useEffect(() => {
    const onChange = () => setIsFull(!!document.fullscreenElement)
    document.addEventListener('fullscreenchange', onChange)
    return () => document.removeEventListener('fullscreenchange', onChange)
  }, [])

  const toggle = () => {
    if (document.fullscreenElement) {
      document.exitFullscreen()
    } else {
      // .stage, not documentElement: fullscreening just the game area keeps
      // the rest of the page (none of which is visible anyway) out of it,
      // and matters more once browsers ship element-scoped fullscreen UI.
      document.querySelector('.stage')?.requestFullscreen().catch(() => {})
    }
  }

  return (
    <button
      type="button"
      className="fullscreen-btn"
      onClick={toggle}
      aria-label={isFull ? 'Exit fullscreen' : 'Enter fullscreen'}
    >
      <span className={isFull ? 'fs-icon fs-icon-exit' : 'fs-icon fs-icon-enter'} />
    </button>
  )
}

// Stat readout (top-left, Flight only) plus mute/credits buttons
// (top-right, always shown) — rendered in the DOM instead of by Bevy so
// they stay pinned to the real screen corners regardless of the canvas's
// cover-fit scale/crop (see the comment on `.canvas-wrap` in App.css).
function HudOverlay() {
  const [stats, setStats] = useState({
    visible: false,
    remaining: 0,
    health: 0,
    fuel: 0,
    score: 0,
    muted: false,
  })

  useEffect(() => {
    hudListeners.add(setStats)
    return () => hudListeners.delete(setStats)
  }, [])

  return (
    <>
      {stats.visible && (
        <div className="hud-stats">
          <div>Remaining: {stats.remaining}</div>
          <div className="hud-stats-health">Health: {Math.round(stats.health)}</div>
          <div>Fuel: {Math.round(stats.fuel)}</div>
          <div>Score: {stats.score}</div>
        </div>
      )}
      <div className="hud-buttons">
        <button
          type="button"
          className={stats.muted ? 'hud-mute-btn hud-mute-btn-off' : 'hud-mute-btn'}
          onClick={() => window.__zerlakToggleMute()}
          aria-label={stats.muted ? 'Unmute' : 'Mute'}
        >
          {stats.muted ? 'X' : ')))'}
        </button>
        <button
          type="button"
          className="hud-credits-btn"
          onClick={() => window.__zerlakRequestCreditsOpen()}
        >
          CREDITS
        </button>
      </div>
    </>
  )
}

// Title/Galaxy Map/Game Over screen text — rendered in the DOM instead of
// by Bevy for the same reason as `HudOverlay`: pinned to `.game-area` (the
// real viewport box), not the fixed 900x650 canvas space cover-fit can
// crop. Title's heading and the galaxy map's instructions line are static
// enough that they're just hardcoded here (matching what used to be
// spawned in title.rs/galaxy_map.rs) instead of round-tripping unchanging
// strings through the bridge every frame; only the handful of values that
// actually vary (fuel, level, banner, countdown, defeat reason) come from
// Rust via `window.__zerlakScreenTextUpdate`.
function ScreenTextOverlay() {
  const [text, setText] = useState({
    screen: '',
    fuel: 0,
    level: 0,
    banner: '',
    countdown: 0,
    defeatReason: '',
  })

  useEffect(() => {
    screenTextListeners.add(setText)
    return () => screenTextListeners.delete(setText)
  }, [])

  if (text.screen === 'title') {
    return (
      <div className="screen-text-center">
        <div className="title-heading">ZERLAK FRONTIER</div>
        <div className="title-subtitle">A Zerlak incursion threatens the frontier.</div>
        <div className="title-instructions">
          GALAXY MAP - Arrows/mouse: pick a sector&nbsp;&nbsp;&nbsp;Enter/Space/click: warp in
          <br />
          Red = Zerlak (fight it)&nbsp;&nbsp;&nbsp;Blue = Friendly (refuel)&nbsp;&nbsp;&nbsp;decide fast, or one
          gets picked for you
          <br />
          <br />
          FLIGHT - Arrows/mouse: aim&nbsp;&nbsp;&nbsp;Space/click: fire
          <br />
          Dodge a charging enemy laser by moving your crosshair clear before it fires
          <br />
          <br />
          Fuel or health hits zero and the mission ends. Esc always backs out a screen.
        </div>
        <div className="title-prompt">Enter / Click / Tap: launch&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;Esc: quit</div>
      </div>
    )
  }

  if (text.screen === 'galaxy_map') {
    return (
      <>
        <div className="galaxy-stats">
          <div>Fuel: {Math.round(text.fuel)}</div>
          <div className="galaxy-level">Level {text.level}</div>
        </div>
        <div className="galaxy-banner-area">
          {text.banner && <div className="galaxy-banner">{text.banner}</div>}
          <div className="galaxy-countdown">Zerlak lock in {text.countdown.toFixed(2)}s</div>
        </div>
        <div className="galaxy-instructions">
          Arrows/mouse: select&nbsp;&nbsp;&nbsp;&nbsp;Enter/Space/click: warp&nbsp;&nbsp;&nbsp;&nbsp;Esc: quit
          to title&nbsp;&nbsp;&nbsp;&nbsp;(Zerlak = red, Friendly = blue)
        </div>
      </>
    )
  }

  if (text.screen === 'game_over') {
    return (
      <div className="screen-text-center">
        <div className="gameover-heading">MISSION FAILED</div>
        <div className="gameover-reason">{text.defeatReason}</div>
        <div className="gameover-level">Reached level {text.level}</div>
        <div className="gameover-prompt">Enter / Click / Tap: return to title</div>
      </div>
    )
  }

  return null
}

// The entire lower-third strip (tall-portrait layout only) is one
// trackpad, not just the drawn wheel graphic inside it — trackpad-style,
// not a joystick: dragging moves the crosshair by the drag's own motion,
// the same way dragging a mouse or a laptop trackpad moves a cursor, not
// by snapping to a held direction from center. Lift and drag again to
// keep going, exactly like a trackpad stroke. Every tap anywhere in the
// strip fires (once per touch-down), same as tapping the playfield
// directly does outside this layout — the whole area exists so aiming and
// firing don't fight over the same point under your finger, not to gate
// firing behind a specific sub-region. The drawn wheel (styled after the
// old iPod click wheel) is purely decorative, indicating "drag here" —
// `pointer-events: none`, it never intercepts anything itself.
function TouchPad() {
  const lastPos = useRef(null)

  const onPointerDown = (e) => {
    e.currentTarget.setPointerCapture(e.pointerId)
    lastPos.current = { x: e.clientX, y: e.clientY }
    window.__zerlakTriggerFire()
  }
  const onPointerMove = (e) => {
    if (!lastPos.current) return
    const dx = e.clientX - lastPos.current.x
    const dy = e.clientY - lastPos.current.y
    lastPos.current = { x: e.clientX, y: e.clientY }
    // Flip Y: screen-down is game-up.
    window.__zerlakNudge(dx, -dy)
  }
  const onPointerEnd = () => {
    lastPos.current = null
  }

  return (
    <div
      className="control-cluster"
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerEnd}
      onPointerCancel={onPointerEnd}
    >
      <div className="ipod-wheel">
        <span className="ipod-wheel-arrow ipod-wheel-arrow-up" />
        <span className="ipod-wheel-arrow ipod-wheel-arrow-down" />
        <span className="ipod-wheel-arrow ipod-wheel-arrow-left" />
        <span className="ipod-wheel-arrow ipod-wheel-arrow-right" />
        <div className="ipod-fire" />
      </div>
    </div>
  )
}
