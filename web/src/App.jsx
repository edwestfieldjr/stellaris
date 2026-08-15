import { useEffect, useRef, useState } from 'react'
import './App.css'

// The game's native, fixed internal resolution — must match
// `WindowResolution::new(...)` in src/main.rs. `.canvas-wrap` in App.css is
// always exactly this size in CSS pixels; we scale it down with a transform
// to fit the screen instead of resizing it. See the comment in App.css for
// why that distinction is load-bearing on the web build.
const GAME_WIDTH = 900
const GAME_HEIGHT = 650

export default function App() {
  const [status, setStatus] = useState('loading') // 'loading' | 'ready' | 'error'
  const started = useRef(false)
  const viewportRef = useRef(null)
  const canvasWrapRef = useRef(null)

  useEffect(() => {
    // React 18 StrictMode double-invokes effects in dev; the wasm module
    // can only ever be started once.
    if (started.current) return
    started.current = true

    import('./wasm/solaris.js')
      .then((mod) => mod.default())
      .then(() => setStatus('ready'))
      .catch((err) => {
        console.error('Failed to start Zerlak Frontier:', err)
        setStatus('error')
      })
  }, [])

  // Fits the fixed-size canvas wrapper to whatever room `.viewport` actually
  // has, purely via a CSS transform. Never changes the wrapper's (or the
  // canvas's) actual box size — that would trip winit's own resize
  // detection and shrink the game's internal resolution to match. See
  // App.css.
  useEffect(() => {
    const viewport = viewportRef.current
    const canvasWrap = canvasWrapRef.current
    if (!viewport || !canvasWrap) return

    const applyScale = () => {
      const scale = viewport.clientWidth / GAME_WIDTH
      canvasWrap.style.transform = `scale(${scale})`
    }

    applyScale()
    const observer = new ResizeObserver(applyScale)
    observer.observe(viewport)
    return () => observer.disconnect()
  }, [])

  return (
    <div className="stage">
      <div className="viewport" ref={viewportRef}>
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
      </div>
    </div>
  )
}
