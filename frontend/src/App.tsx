import { useEffect, useState } from 'react'
import type { Health } from './generated/Health'

function App() {
  const [health, setHealth] = useState<Health | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    fetch('/api/v1/health')
      .then((res) => {
        if (!res.ok) throw new Error(`HTTP ${res.status}`)
        return res.json() as Promise<Health>
      })
      .then(setHealth)
      .catch((err: Error) => setError(err.message))
  }, [])

  return (
    <main>
      <h1>AutoWeed</h1>
      {health && (
        <p>
          Verbunden mit <code>{health.service}</code> v{health.version}
        </p>
      )}
      {error && <p>Backend nicht erreichbar: {error}</p>}
    </main>
  )
}

export default App
