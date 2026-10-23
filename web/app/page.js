'use client'
import { useCallback, useEffect, useState } from 'react'
import { api, setToken } from '../lib/api'
import { Avatar, Logo, Nav } from '../lib/ui'
import { useMe } from '../lib/useMe'

function AuthForm({ onDone }) {
  const [mode, setMode] = useState('login')
  const [f, setF] = useState({ email: '', name: '', password: '' })
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const set = (k) => (e) => setF((x) => ({ ...x, [k]: e.target.value }))

  const submit = async (e) => {
    e.preventDefault()
    setBusy(true)
    setError('')
    try {
      const body = mode === 'login' ? { email: f.email, password: f.password } : f
      const d = await api(`/api/auth/${mode}`, { method: 'POST', body })
      setToken(d.token)
      onDone()
    } catch (err) {
      setError(err.message)
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="wrap" style={{ display: 'grid', gap: 18, maxWidth: 900, paddingTop: '6vh' }}>
      <div className="grid2" style={{ alignItems: 'center' }}>
        <div>
          <div className="row" style={{ marginBottom: 10 }}><Logo size={40} /><h1 style={{ margin: 0, fontSize: 38 }}>Kolom Khela</h1></div>
          <p style={{ fontSize: 18, margin: '0 0 12px' }}>The pen game from the school bench, now online with your friends.</p>
          <p className="muted">Flick your kolom, knock theirs off the desk, win coins, collect better pens. Free to play. All you need is an email, a name and a password.</p>
        </div>
        <div className="card">
          <div className="tabs">
            <button type="button" className={mode === 'login' ? 'on' : ''} onClick={() => setMode('login')}>Log in</button>
            <button type="button" className={mode === 'register' ? 'on' : ''} onClick={() => setMode('register')}>Sign up</button>
          </div>
          <form onSubmit={submit}>
            <label htmlFor="email">Email</label>
            <input id="email" type="email" required autoComplete="email" value={f.email} onChange={set('email')} />
            {mode === 'register' && (
              <>
                <label htmlFor="name">Name (shown to other players)</label>
                <input id="name" required minLength={2} maxLength={24} autoComplete="nickname" value={f.name} onChange={set('name')} />
              </>
            )}
            <label htmlFor="pw">Password</label>
            <input id="pw" type="password" required minLength={6} autoComplete={mode === 'login' ? 'current-password' : 'new-password'} value={f.password} onChange={set('password')} />
            {error && <p className="err" role="alert">{error}</p>}
            <div style={{ marginTop: 16 }}>
              <button disabled={busy} style={{ width: '100%' }}>{busy ? 'Please wait...' : mode === 'login' ? 'Log in' : 'Create account'}</button>
            </div>
          </form>
        </div>
      </div>
    </div>
  )
}

function Lobby({ me, reload }) {
  const [code, setCode] = useState('')
  const [board, setBoard] = useState([])
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)

  useEffect(() => { api('/api/leaderboard').then(setBoard).catch(() => {}) }, [])
  const go = (c) => { window.location.href = `/room/?code=${encodeURIComponent(c)}` }

  const create = async () => {
    setBusy(true)
    setError('')
    try { go((await api('/api/rooms', { method: 'POST' })).code) } catch (e) { setError(e.message); setBusy(false) }
  }
  const join = async (e) => {
    e.preventDefault()
    setError('')
    const c = code.trim().toUpperCase()
    try { await api(`/api/rooms/${c}`); go(c) } catch (e2) { setError(e2.message) }
  }

  const s = me.stats
  const rate = s.played ? Math.round((s.wins / s.played) * 100) : 0
  return (
    <div className="wrap">
      <Nav me={me} active="play" onLogout={reload} />
      {me.user.guest && (
        <div className="card" style={{ marginBottom: 16, borderColor: 'var(--gold)' }}>
          You are playing as a guest. <a href="/profile/">Save your account</a> so you keep your coins and pens and can log in again.
        </div>
      )}
      <div className="grid2">
        <div className="card hero">
          <div className="row" style={{ marginBottom: 6 }}>
            <Avatar id={me.user.id} name={me.user.name} at={me.user.avatarAt} size={64} />
            <div>
              <h2 style={{ margin: 0 }}>Hi {me.user.name}</h2>
              <div className="muted">Pen in use: <b style={{ color: 'var(--text)' }}>{me.user.pen}</b></div>
            </div>
          </div>
          <div className="stats">
            <div className="stat"><b>{s.played}</b>played</div>
            <div className="stat"><b>{s.wins}</b>won</div>
            <div className="stat"><b>{rate}%</b>win rate</div>
            <div className="stat"><b>{me.user.coins}</b>coins</div>
          </div>
          <div style={{ marginTop: 18 }}>
            <button onClick={create} disabled={busy} style={{ width: '100%', fontSize: 18, padding: 14 }}>Start a new game</button>
          </div>
          <p className="muted" style={{ margin: '14px 0 6px' }}>Or join a friend with their code:</p>
          <form className="row" onSubmit={join} style={{ flexWrap: 'nowrap' }}>
            <input value={code} onChange={(e) => setCode(e.target.value)} placeholder="ABCDE" maxLength={5} style={{ textTransform: 'uppercase', letterSpacing: 4, textAlign: 'center' }} aria-label="Room code" />
            <button>Join</button>
          </form>
          {error && <p className="err" role="alert">{error}</p>}
        </div>
        <div className="card">
          <h2>How to play</h2>
          <ol className="muted" style={{ paddingLeft: 18, margin: 0, lineHeight: 1.7 }}>
            <li>2 to 4 players, one desk, one kolom (pen) each.</li>
            <li>On your turn press anywhere on the desk, pull back and let go. The kolom flicks the way you pulled. Press near an end and it spins.</li>
            <li>Hit the other koloms. Any kolom that leaves the desk is out.</li>
            <li>The first kolom to fall is the <b>looser</b>. The last one on the desk wins <b>10 coins</b>.</li>
            <li>Spend coins in the pen shop on heavier, longer pens.</li>
          </ol>
        </div>
      </div>
      <div className="card" style={{ marginTop: 16 }}>
        <h2>Top players</h2>
        {board.length === 0 ? <p className="muted">No games finished yet. Be the first.</p> : (
          <table>
            <thead><tr><th>#</th><th>Player</th><th>Wins</th><th>Games</th></tr></thead>
            <tbody>{board.map((p, i) => (
              <tr key={p.id}>
                <td>{i + 1}</td>
                <td><span className="row" style={{ gap: 8, flexWrap: 'nowrap' }}><Avatar id={p.id} name={p.name} at={p.avatar} size={28} />{p.name}</span></td>
                <td>{p.wins}</td><td>{p.played}</td>
              </tr>
            ))}</tbody>
          </table>
        )}
      </div>
    </div>
  )
}

export default function Home() {
  const { me, state, reload } = useMe({ required: false })
  const [redirected, setRedirected] = useState(false)

  const after = useCallback(async () => {
    await reload()
    let next = ''
    try { next = sessionStorage.getItem('kolom_next') || ''; sessionStorage.removeItem('kolom_next') } catch {}
    if (next.startsWith('/room/')) { setRedirected(true); window.location.href = next }
  }, [reload])

  if (state === 'loading' || redirected) return <div className="wrap muted">Loading...</div>
  if (state === 'out') return <AuthForm onDone={after} />
  return <Lobby me={me} reload={reload} />
}
