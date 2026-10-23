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
    return t ? users.filter((u) => u.name.toLowerCase().includes(t) || u.email.toLowerCase().includes(t)) : users
  }, [users, q])

  if (!me || ok === null) return <div className="wrap muted">Loading...</div>
  if (ok === false) return <div className="wrap"><Nav me={me} active="admin" /><div className="card"><h2>Admins only</h2><p className="muted">This page is for the admin account.</p><a className="btn" href="/">Back to the game</a></div></div>

  const TABS = [['overview', 'Overview'], ['players', `Players (${users.length})`], ['rooms', `Live rooms (${rooms.length})`], ['games', 'Games']]

  return (
    <div className="wrap">
      <Nav me={me} active="admin" />
      <div className="row between" style={{ marginBottom: 12 }}>
        <h1 style={{ margin: 0 }}>Admin</h1>
        <div className="tabs" style={{ margin: 0 }}>
          {TABS.map(([k, l]) => <button key={k} type="button" className={tab === k ? 'on' : ''} onClick={() => setTab(k)} style={{ flex: 'none', padding: '8px 14px' }}>{l}</button>)}
        </div>
      </div>
      {error && <p className="err" role="alert">{error}</p>}

      {tab === 'overview' && (
        <>
          <div className="stats" style={{ gridTemplateColumns: 'repeat(auto-fit, minmax(130px, 1fr))', marginTop: 0 }}>
            <div className="stat"><b>{stats.users}</b>players</div>
            <div className="stat"><b>{stats.liveRooms}</b>live rooms</div>
            <div className="stat"><b>{stats.games}</b>games played</div>
            <div className="stat"><b>{stats.blocked}</b>blocked</div>
          </div>
          <div className="grid2" style={{ marginTop: 16 }}>
            <div className="card">
              <h2>Live rooms</h2>
              <Rooms rooms={rooms} onClose={(code) => act(() => api(`/api/admin/rooms/${code}`, { method: 'DELETE' }))} />
            </div>
            <div className="card">
              <h2>Recent games</h2>
              <Games games={games.slice(0, 8)} />
            </div>
          </div>
        </>
      )}

      {tab === 'players' && (
        <div className="card" style={{ overflowX: 'auto' }}>
          <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="Search by name or email" aria-label="Search players" style={{ maxWidth: 340, marginBottom: 12 }} />
          <table>
            <thead><tr><th>Player</th><th>Role</th><th>Coins</th><th>Joined</th><th>Actions</th></tr></thead>
            <tbody>{shown.map((u) => (
              <tr key={u.id}>
                <td>
                  <span className="row" style={{ gap: 10, flexWrap: 'nowrap' }}>
                    <Avatar id={u.id} name={u.name} at={u.avatarAt} size={36} />
                    <span><b>{u.name}</b>{u.blocked ? <span className="pill" style={{ marginLeft: 6, background: 'var(--red)', color: '#fff' }}>blocked</span> : null}<br /><span className="muted">{u.email}</span></span>
                  </span>
                </td>
                <td>{u.role}</td>
                <td>{u.coins}</td>
                <td className="muted">{when(u.created_at)}</td>
                <td>
                  <div className="row" style={{ gap: 6 }}>
                    <button className="ghost small" onClick={() => patch(u.id, { blocked: !u.blocked })}>{u.blocked ? 'Unblock' : 'Block'}</button>
                    <button className="ghost small" onClick={() => patch(u.id, { role: u.role === 'admin' ? 'player' : 'admin' })}>{u.role === 'admin' ? 'Make player' : 'Make admin'}</button>
                    <button className="ghost small" onClick={() => { const v = prompt(`Add how many coins to ${u.name}? (use a minus number to take away)`, '50'); const n = parseInt(v, 10); if (Number.isFinite(n)) patch(u.id, { add_coins: n }) }}>Coins</button>
                    {u.avatarAt && <button className="ghost small" onClick={() => confirm(`Remove ${u.name}'s picture?`) && act(() => api(`/api/admin/users/${u.id}/avatar`, { method: 'DELETE' }))}>Remove pic</button>}
                    <button className="danger small" onClick={() => confirm(`Delete ${u.name}? This cannot be undone.`) && act(() => api(`/api/admin/users/${u.id}`, { method: 'DELETE' }))}>Delete</button>
                  </div>
                </td>
              </tr>
            ))}</tbody>
          </table>
          {shown.length === 0 && <p className="muted">No players match.</p>}
        </div>
      )}

      {tab === 'rooms' && <div className="card"><Rooms rooms={rooms} onClose={(code) => act(() => api(`/api/admin/rooms/${code}`, { method: 'DELETE' }))} /></div>}
      {tab === 'games' && <div className="card" style={{ overflowX: 'auto' }}><Games games={games} /></div>}
    </div>
  )
}

function Rooms({ rooms, onClose }) {
  if (rooms.length === 0) return <p className="muted">No rooms open.</p>
  return (
    <table>
      <thead><tr><th>Code</th><th>Host</th><th>Players</th><th>State</th><th /></tr></thead>
      <tbody>{rooms.map((r) => (
        <tr key={r.code}><td><b>{r.code}</b></td><td>{r.host}</td><td>{r.players}</td><td>{r.phase}</td>
          <td><button className="danger small" onClick={() => confirm(`Close room ${r.code}? Players are disconnected.`) && onClose(r.code)}>Close</button></td></tr>
      ))}</tbody>
    </table>
  )
}

function Games({ games }) {
  if (games.length === 0) return <p className="muted">No games yet.</p>
  return (
    <table>
      <thead><tr><th>Room</th><th>Winner</th><th>Looser</th><th>Finished</th></tr></thead>
      <tbody>{games.map((g) => <tr key={g.id}><td>{g.code}</td><td>{g.winner || '-'}</td><td>{g.looser || '-'}</td><td className="muted">{when(g.finishedAt)}</td></tr>)}</tbody>
    </table>
  )
}
