import { useCallback, useEffect, useState } from 'react'
import { api, post, setUnauthorizedHandler } from './api'
import type { SessionInfo } from './generated/SessionInfo'
import Dashboard from './pages/Dashboard'
import History from './pages/History'
import Login from './pages/Login'
import Settings from './pages/Settings'

const pages = {
  uebersicht: { title: 'Übersicht', component: Dashboard },
  verlauf: { title: 'Verlauf', component: History },
  einstellungen: { title: 'Einstellungen', component: Settings },
} as const

type PageKey = keyof typeof pages

function currentPage(): PageKey {
  const key = window.location.hash.replace('#', '')
  return key in pages ? (key as PageKey) : 'uebersicht'
}

function App() {
  const [authenticated, setAuthenticated] = useState<boolean | null>(null)
  const [page, setPage] = useState<PageKey>(currentPage)

  useEffect(() => {
    setUnauthorizedHandler(() => setAuthenticated(false))
    api<SessionInfo>('/api/session')
      .then((s) => setAuthenticated(s.authenticated))
      .catch(() => setAuthenticated(false))
    const onHash = () => setPage(currentPage())
    window.addEventListener('hashchange', onHash)
    return () => window.removeEventListener('hashchange', onHash)
  }, [])

  const logout = useCallback(async () => {
    await post('/api/logout', {}).catch(() => {})
    setAuthenticated(false)
  }, [])

  if (authenticated === null) return null
  if (!authenticated) return <Login onLogin={() => setAuthenticated(true)} />

  const Page = pages[page].component
  return (
    <>
      <header className="top">
        <h1>🌱 AutoWeed</h1>
        <nav>
          {(Object.keys(pages) as PageKey[]).map((key) => (
            <a key={key} href={`#${key}`} className={key === page ? 'active' : ''}>
              {pages[key].title}
            </a>
          ))}
        </nav>
        <button onClick={logout}>Abmelden</button>
      </header>
      <main>
        <Page />
      </main>
    </>
  )
}

export default App
