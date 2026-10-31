import { useState, type FormEvent } from 'react'
import { ApiFailure, post } from '../api'

export default function Login({ onLogin }: { onLogin: () => void }) {
  const [password, setPassword] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  async function submit(e: FormEvent) {
    e.preventDefault()
    setBusy(true)
    setError(null)
    try {
      await post('/api/login', { password })
      onLogin()
    } catch (err) {
      setError(err instanceof ApiFailure ? err.message : 'Server nicht erreichbar')
    } finally {
      setBusy(false)
    }
  }

  return (
    <form className="card login stack" onSubmit={submit}>
      <h1 style={{ margin: 0, fontSize: '1.4rem' }}>🌱 AutoWeed</h1>
      <label className="field">
        <span>Passwort</span>
        <input
          type="password"
          autoComplete="current-password"
          autoFocus
          value={password}
          onChange={(e) => setPassword(e.target.value)}
        />
      </label>
      {error && <p className="error-list" style={{ paddingLeft: 0 }}>{error}</p>}
      <button className="primary" disabled={busy || !password}>
        Anmelden
      </button>
    </form>
  )
}
