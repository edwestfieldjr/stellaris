import { useEffect, useRef, useState } from 'react'
import './App.css'

export default function App() {
  const [status, setStatus] = useState('loading') // 'loading' | 'ready' | 'error'
  const started = useRef(false)

  useEffect(() => {
    // React 18 StrictMode double-invokes effects in dev; the wasm module
    // can only ever be started once.
    if (started.current) return
    started.current = true

    import('./wasm/solaris.js')
      .then((mod) => mod.default())
      .then(() => setStatus('ready'))
      .catch((err) => {
        console.error('Failed to start Stellaris:', err)
        setStatus('error')
      })
  }, [])

  return (
    <div className="stage">
      <div className="viewport">
        <canvas id="stellaris-canvas" />
        {status !== 'ready' && (
          <div className="overlay">
            {status === 'loading'
              ? 'Loading Stellaris…'
              : 'Failed to load. Please reload the page.'}
          </div>
        )}
      </div>
    </div>
  )
}
