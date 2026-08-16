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
      </div>
      {/* Only takes any layout space on tall-portrait screens (see the
          @media rule in App.css) — the game-area above always fills
          whatever's left, so the playfield never shows letterbox bars in
          either layout. */}
      <div className="control-cluster">
        <VirtualWheel />
      </div>
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

// A circular drag surface — trackpad-style, not a joystick: dragging moves
// the crosshair by the drag's motion, the same way dragging a mouse or a
// laptop trackpad moves a cursor, not by snapping to a held direction from
// center. Lift and drag again to keep going, exactly like a trackpad
// stroke. The center "click wheel" button is FIRE (one shot per press) —
// styled after the old iPod click wheel, reserved for tall-portrait screens
// where a one-tap crosshair doesn't leave enough room for both aiming and
// firing precisely.
function VirtualWheel() {
  const lastPos = useRef(null)

  const onPointerDown = (e) => {
    e.currentTarget.setPointerCapture(e.pointerId)
    lastPos.current = { x: e.clientX, y: e.clientY }
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
      className="ipod-wheel"
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerEnd}
      onPointerCancel={onPointerEnd}
    >
      <button
        type="button"
        className="ipod-fire"
        aria-label="Fire"
        onPointerDown={(e) => {
          e.stopPropagation()
          window.__zerlakTriggerFire()
        }}
      >
        FIRE
      </button>
    </div>
  )
}
