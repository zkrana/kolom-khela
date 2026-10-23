'use client'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { api } from '../../lib/api'
import { Avatar, Nav } from '../../lib/ui'
import { useMe } from '../../lib/useMe'

const when = (s) => (s ? new Date(s * 1000).toLocaleString() : '')

export default function Admin() {
  const { me, state } = useMe()
  const [ok, setOk] = useState(null)
  const [stats, setStats] = useState(null)
  const [users, setUsers] = useState([])
  const [rooms, setRooms] = useState([])
  const [games, setGames] = useState([])
  const [tab, setTab] = useState('overview')
  const [q, setQ] = useState('')
  const [error, setError] = useState('')

  const load = useCallback(async () => {
    try {
      const [s, u, r, g] = await Promise.all([api('/api/admin/stats'), api('/api/admin/users'), api('/api/admin/rooms'), api('/api/admin/games')])
      setStats(s); setUsers(u); setRooms(r); setGames(g); setOk(true)
    } catch (e) {
      if (e.status === 401 || e.status === 403) setOk(false)
      else setError(e.message)
    }
  }, [])

  useEffect(() => {
    if (state !== 'in') return
    load()
    const iv = setInterval(load, 5000)
    return () => clearInterval(iv)
  }, [state, load])

  const act = async (fn) => {
    setError('')
    try { await fn(); await load() } catch (e) { setError(e.message) }
  }
  const patch = (id, body) => act(() => api(`/api/admin/users/${id}`, { method: 'PATCH', body }))

  const shown = useMemo(() => {
    const t = q.trim().toLowerCase()
    return t ? users.filter((u) => u.name.toLowerCase().includes(t) || (u.guest ? u.contact || '' : u.email).toLowerCase().includes(t)) : users
  }, [users, q])

  if (!me || ok === null) return <div className="wrap muted">লোড হচ্ছে...</div>
  if (ok === false) return <div className="wrap"><Nav me={me} active="admin" /><div className="card"><h2>শুধু অ্যাডমিনদের জন্য</h2><p className="muted">এই পাতা শুধু অ্যাডমিন অ্যাকাউন্টের জন্য।</p><a className="btn" href="/">খেলায় ফিরুন</a></div></div>

  const TABS = [['overview', 'সারসংক্ষেপ'], ['players', `খেলোয়াড় (${users.length})`], ['rooms', `চলতি রুম (${rooms.length})`], ['games', 'খেলা']]

  return (
    <div className="wrap">
      <Nav me={me} active="admin" />
      <div className="row between" style={{ marginBottom: 12 }}>
        <h1 style={{ margin: 0 }}>অ্যাডমিন</h1>
        <div className="tabs" style={{ margin: 0 }}>
          {TABS.map(([k, l]) => <button key={k} type="button" className={tab === k ? 'on' : ''} onClick={() => setTab(k)} style={{ flex: 'none', padding: '8px 14px' }}>{l}</button>)}
        </div>
      </div>
      {error && <p className="err" role="alert">{error}</p>}

      {tab === 'overview' && (
        <>
          <div className="stats" style={{ gridTemplateColumns: 'repeat(auto-fit, minmax(130px, 1fr))', marginTop: 0 }}>
            <div className="stat"><b>{stats.users}</b>খেলোয়াড়</div>
            <div className="stat"><b>{stats.liveRooms}</b>চলতি রুম</div>
            <div className="stat"><b>{stats.games}</b>খেলা হয়েছে</div>
            <div className="stat"><b>{stats.blocked}</b>ব্লক করা</div>
          </div>
          <div className="grid2" style={{ marginTop: 16 }}>
            <div className="card">
              <h2>চলতি রুম</h2>
              <Rooms rooms={rooms} onClose={(code) => act(() => api(`/api/admin/rooms/${code}`, { method: 'DELETE' }))} />
            </div>
            <div className="card">
              <h2>সাম্প্রতিক খেলা</h2>
              <Games games={games.slice(0, 8)} />
            </div>
          </div>
        </>
      )}

      {tab === 'players' && (
        <div className="card" style={{ overflowX: 'auto' }}>
          <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="নাম বা ইমেইল দিয়ে খুঁজুন" aria-label="খেলোয়াড় খুঁজুন" style={{ maxWidth: 340, marginBottom: 12 }} />
          <table>
            <thead><tr><th>খেলোয়াড়</th><th>ভূমিকা</th><th>কয়েন</th><th>যোগ দিয়েছেন</th><th>কাজ</th></tr></thead>
            <tbody>{shown.map((u) => (
              <tr key={u.id}>
                <td>
                  <span className="row" style={{ gap: 10, flexWrap: 'nowrap' }}>
                    <Avatar id={u.id} name={u.name} at={u.avatarAt} size={36} />
                    <span><b>{u.name}</b>{u.blocked ? <span className="pill" style={{ marginLeft: 6, background: 'var(--red)', color: '#fff' }}>ব্লক</span> : null}{u.guest ? <span className="pill" style={{ marginLeft: 6 }}>গেস্ট</span> : null}<br /><span className="muted">{u.guest ? u.contact : u.email}</span></span>
                  </span>
                </td>
                <td>{u.role}</td>
                <td>{u.coins}</td>
                <td className="muted">{when(u.created_at)}</td>
                <td>
                  <div className="row" style={{ gap: 6 }}>
                    <button className="ghost small" onClick={() => patch(u.id, { blocked: !u.blocked })}>{u.blocked ? 'আনব্লক' : 'ব্লক'}</button>
                    <button className="ghost small" onClick={() => patch(u.id, { role: u.role === 'admin' ? 'player' : 'admin' })}>{u.role === 'admin' ? 'খেলোয়াড় করুন' : 'অ্যাডমিন করুন'}</button>
                    <button className="ghost small" onClick={() => { const v = prompt(`${u.name}-কে কত কয়েন দেবেন? (কেটে নিতে মাইনাস সংখ্যা দিন)`, '50'); const n = parseInt(v, 10); if (Number.isFinite(n)) patch(u.id, { add_coins: n }) }}>কয়েন</button>
                    {u.avatarAt && <button className="ghost small" onClick={() => confirm(`${u.name}-এর ছবি সরাবেন?`) && act(() => api(`/api/admin/users/${u.id}/avatar`, { method: 'DELETE' }))}>ছবি সরান</button>}
                    <button className="danger small" onClick={() => confirm(`${u.name}-কে ডিলিট করবেন? এটা আর ফেরানো যাবে না।`) && act(() => api(`/api/admin/users/${u.id}`, { method: 'DELETE' }))}>ডিলিট</button>
                  </div>
                </td>
              </tr>
            ))}</tbody>
          </table>
          {shown.length === 0 && <p className="muted">কোনো খেলোয়াড় মেলেনি।</p>}
        </div>
      )}

      {tab === 'rooms' && <div className="card"><Rooms rooms={rooms} onClose={(code) => act(() => api(`/api/admin/rooms/${code}`, { method: 'DELETE' }))} /></div>}
      {tab === 'games' && <div className="card" style={{ overflowX: 'auto' }}><Games games={games} /></div>}
    </div>
  )
}

function Rooms({ rooms, onClose }) {
  if (rooms.length === 0) return <p className="muted">কোনো রুম খোলা নেই।</p>
  return (
    <table>
      <thead><tr><th>কোড</th><th>হোস্ট</th><th>খেলোয়াড়</th><th>অবস্থা</th><th /></tr></thead>
      <tbody>{rooms.map((r) => (
        <tr key={r.code}><td><b>{r.code}</b></td><td>{r.host}</td><td>{r.players}</td><td>{r.phase}</td>
          <td><button className="danger small" onClick={() => confirm(`${r.code} রুম বন্ধ করবেন? খেলোয়াড়দের সংযোগ কেটে যাবে।`) && onClose(r.code)}>বন্ধ করুন</button></td></tr>
      ))}</tbody>
    </table>
  )
}

function Games({ games }) {
  if (games.length === 0) return <p className="muted">এখনো কোনো খেলা হয়নি।</p>
  return (
    <table>
      <thead><tr><th>রুম</th><th>বিজয়ী</th><th>লুজার</th><th>শেষ হয়েছে</th></tr></thead>
      <tbody>{games.map((g) => <tr key={g.id}><td>{g.code}</td><td>{g.winner || '-'}</td><td>{g.looser || '-'}</td><td className="muted">{when(g.finishedAt)}</td></tr>)}</tbody>
    </table>
  )
}
