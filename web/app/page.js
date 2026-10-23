'use client'
import { useCallback, useEffect, useState } from 'react'
import { api, setToken } from '../lib/api'
import { Avatar, Logo, Nav } from '../lib/ui'
import { PEN_NAMES } from '../lib/pens'
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
    <div className="wrap" style={{ display: 'grid', gap: 18, maxWidth: 900, minHeight: '100vh', alignContent: 'center' }}>
      <div className="grid2" style={{ alignItems: 'center' }}>
        <div>
          <div className="row" style={{ marginBottom: 10 }}><Logo size={40} /><h1 style={{ margin: 0, fontSize: 38 }}>কলম খেলা</h1></div>
          <p style={{ fontSize: 18, margin: '0 0 12px' }}>ক্লাসের বেঞ্চের সেই কলম খেলা, এবার অনলাইনে বন্ধুদের সাথে।</p>
          <p className="muted">টোকা দিয়ে অন্যের কলম বেঞ্চ থেকে ফেলুন, কয়েন জিতুন, ভালো কলম জমান। একদম ফ্রি। শুধু একটা ইমেইল, নাম আর পাসওয়ার্ড লাগবে।</p>
        </div>
        <div className="card">
          <div className="tabs">
            <button type="button" className={mode === 'login' ? 'on' : ''} onClick={() => setMode('login')}>লগইন</button>
            <button type="button" className={mode === 'register' ? 'on' : ''} onClick={() => setMode('register')}>সাইন আপ</button>
          </div>
          <form onSubmit={submit}>
            <label htmlFor="email">ইমেইল</label>
            <input id="email" type="email" required autoComplete="email" value={f.email} onChange={set('email')} />
            {mode === 'register' && (
              <>
                <label htmlFor="name">নাম (অন্য খেলোয়াড়রা দেখবে)</label>
                <input id="name" required minLength={2} maxLength={24} autoComplete="nickname" value={f.name} onChange={set('name')} />
              </>
            )}
            <label htmlFor="pw">পাসওয়ার্ড</label>
            <input id="pw" type="password" required minLength={6} autoComplete={mode === 'login' ? 'current-password' : 'new-password'} value={f.password} onChange={set('password')} />
            {error && <p className="err" role="alert">{error}</p>}
            <div style={{ marginTop: 16 }}>
              <button disabled={busy} style={{ width: '100%' }}>{busy ? 'একটু অপেক্ষা করুন...' : mode === 'login' ? 'লগইন' : 'অ্যাকাউন্ট খুলুন'}</button>
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
          আপনি গেস্ট হিসেবে খেলছেন। কয়েন আর কলম না হারাতে এবং আবার লগইন করতে <a href="/profile/">অ্যাকাউন্ট সেভ করুন</a>।
        </div>
      )}
      <div className="grid2">
        <div className="card hero">
          <div className="row" style={{ marginBottom: 6 }}>
            <Avatar id={me.user.id} name={me.user.name} at={me.user.avatarAt} size={64} />
            <div>
              <h2 style={{ margin: 0 }}>হ্যালো {me.user.name}</h2>
              <div className="muted">চলতি কলম: <b style={{ color: 'var(--text)' }}>{PEN_NAMES[me.user.pen] || me.user.pen}</b></div>
            </div>
          </div>
          <div className="stats">
            <div className="stat"><b>{s.played}</b>খেলেছেন</div>
            <div className="stat"><b>{s.wins}</b>জিতেছেন</div>
            <div className="stat"><b>{rate}%</b>জেতার হার</div>
            <div className="stat"><b>{me.user.coins}</b>কয়েন</div>
          </div>
          <div style={{ marginTop: 18 }}>
            <button onClick={create} disabled={busy} style={{ width: '100%', fontSize: 18, padding: 14 }}>নতুন খেলা শুরু করুন</button>
          </div>
          <p className="muted" style={{ margin: '14px 0 6px' }}>অথবা বন্ধুর কোড দিয়ে ঢুকুন:</p>
          <form className="row" onSubmit={join} style={{ flexWrap: 'nowrap' }}>
            <input value={code} onChange={(e) => setCode(e.target.value)} placeholder="ABCDE" maxLength={5} style={{ textTransform: 'uppercase', letterSpacing: 4, textAlign: 'center' }} aria-label="রুম কোড" />
            <button>ঢুকুন</button>
          </form>
          {error && <p className="err" role="alert">{error}</p>}
        </div>
        <div className="card">
          <h2>কীভাবে খেলবেন</h2>
          <ol className="muted" style={{ paddingLeft: 18, margin: 0, lineHeight: 1.7 }}>
            <li>২ থেকে ৪ জন, একটা বেঞ্চ, প্রত্যেকের একটা করে কলম।</li>
            <li>নিজের পালায় বেঞ্চের যেকোনো জায়গায় চেপে পেছনে টেনে ছেড়ে দিন। যেদিকে টেনেছেন কলম সেদিকে ছুটবে। কলমের মাথার কাছে চাপলে সেটা ঘুরবে।</li>
            <li>অন্যের কলমে ধাক্কা দিন। যে কলম বেঞ্চ থেকে পড়ে যাবে সে আউট।</li>
            <li>সবার আগে যার কলম পড়বে সে <b>লুজার</b>। শেষ পর্যন্ত বেঞ্চে যার কলম থাকবে সে জিতবে <b>১০ কয়েন</b>।</li>
            <li>কয়েন দিয়ে কলমের দোকান থেকে ভারী আর লম্বা কলম কিনুন।</li>
          </ol>
        </div>
      </div>
      <div className="card" style={{ marginTop: 16 }}>
        <h2>সেরা খেলোয়াড়</h2>
        {board.length === 0 ? <p className="muted">এখনো কোনো খেলা শেষ হয়নি। আপনিই প্রথম হোন।</p> : (
          <table>
            <thead><tr><th>#</th><th>খেলোয়াড়</th><th>জয়</th><th>খেলা</th></tr></thead>
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

  if (state === 'loading' || redirected) return <div className="wrap muted">লোড হচ্ছে...</div>
  if (state === 'out') return <AuthForm onDone={after} />
  return <Lobby me={me} reload={reload} />
}
