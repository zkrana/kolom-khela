'use client'
import { Fragment, useCallback, useEffect, useMemo, useState } from 'react'
import { api } from '../../lib/api'
import { PEN_NAMES } from '../../lib/pens'
import { Avatar, Nav } from '../../lib/ui'
import { useMe } from '../../lib/useMe'

const when = (s) => (s ? new Date(s * 1000).toLocaleString() : '')
const LEVEL = { easy: 'সহজ', medium: 'মাঝারি', hard: 'কঠিন' }
const PHASE = { lobby: 'লবিতে', playing: 'খেলা চলছে', over: 'শেষ' }
const END = { knockout: 'একজন টিকে রইল', turns: 'টোকার সীমা শেষ', walkaway: 'অন্যরা খেলা ছেড়ে দিল', '': 'পুরোনো খেলা' }
const CLOSE = { idle: 'কেউ ছিল না', admin: 'অ্যাডমিন বন্ধ করেছেন', restart: 'সার্ভার রিস্টার্ট', gone: 'বন্ধ' }
const dur = (s) => (s > 0 ? `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}` : '–')
const pct = (n, of) => (of ? Math.round((n / of) * 100) : 0)

export default function Admin() {
  const { me, state } = useMe()
  const [ok, setOk] = useState(null)
  const [stats, setStats] = useState(null)
  const [users, setUsers] = useState([])
  const [rooms, setRooms] = useState([])
  const [games, setGames] = useState([])
  const [history, setHistory] = useState([])
  const [tab, setTab] = useState('overview')
  const [q, setQ] = useState('')
  const [roomFilter, setRoomFilter] = useState(null) // a rooms_log id: the games tab shows only that room's matches
  const [error, setError] = useState('')

  const load = useCallback(async () => {
    try {
      const tz = new Date().getTimezoneOffset()
      const [s, u, r, g, h] = await Promise.all([
        api(`/api/admin/stats?tz=${tz}`), api('/api/admin/users'), api('/api/admin/rooms'), api('/api/admin/games'), api('/api/admin/room-history'),
      ])
      setStats(s); setUsers(u); setRooms(r); setGames(g); setHistory(h); setOk(true)
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
  const closeRoom = (code) => act(() => api(`/api/admin/rooms/${code}`, { method: 'DELETE' }))

  const shown = useMemo(() => {
    const t = q.trim().toLowerCase()
    return t ? users.filter((u) => u.name.toLowerCase().includes(t) || (u.guest ? u.contact || '' : u.email).toLowerCase().includes(t)) : users
  }, [users, q])

  if (!me || ok === null) return <div className="wrap muted">লোড হচ্ছে...</div>
  if (ok === false) return <div className="wrap"><Nav me={me} active="admin" /><div className="card"><h2>শুধু অ্যাডমিনদের জন্য</h2><p className="muted">এই পাতা শুধু অ্যাডমিন অ্যাকাউন্টের জন্য।</p><a className="btn" href="/">খেলায় ফিরুন</a></div></div>

  const TABS = [['overview', 'সারসংক্ষেপ'], ['players', `খেলোয়াড় (${users.length})`], ['rooms', `রুম (${rooms.length} চলছে)`], ['games', `খেলা (${stats.games})`]]
  const showRoomGames = (id) => { setRoomFilter(id); setTab('games') }

  return (
    <div className="wrap">
      <Nav me={me} active="admin" />
      <div className="row between" style={{ marginBottom: 12 }}>
        <h1 style={{ margin: 0 }}>অ্যাডমিন</h1>
        <div className="tabs" style={{ margin: 0, flexWrap: 'wrap' }}>
          {TABS.map(([k, l]) => <button key={k} type="button" className={tab === k ? 'on' : ''} onClick={() => setTab(k)} style={{ flex: 'none', padding: '8px 14px' }}>{l}</button>)}
        </div>
      </div>
      {error && <p className="err" role="alert">{error}</p>}

      {tab === 'overview' && <Overview stats={stats} rooms={rooms} games={games} onClose={closeRoom} onOpenGames={() => setTab('games')} />}

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
                    <span><b>{u.name}</b>{u.blocked ? <span className="pill" style={{ marginLeft: 6, background: 'var(--red)', color: '#fff' }}>ব্লক</span> : null}{u.guest ? <span className="tag tag--guest">গেস্ট</span> : null}<br /><span className="muted">{u.guest ? u.contact || 'ইমেইল ছাড়া' : u.email}</span></span>
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
                    {u.avatarAt ? <button className="ghost small" onClick={() => confirm(`${u.name}-এর ছবি সরাবেন?`) && act(() => api(`/api/admin/users/${u.id}/avatar`, { method: 'DELETE' }))}>ছবি সরান</button> : null}
                    <button className="danger small" onClick={() => confirm(`${u.name}-কে ডিলিট করবেন? এটা আর ফেরানো যাবে না।`) && act(() => api(`/api/admin/users/${u.id}`, { method: 'DELETE' }))}>ডিলিট</button>
                  </div>
                </td>
              </tr>
            ))}</tbody>
          </table>
          {shown.length === 0 && <p className="muted">কোনো খেলোয়াড় মেলেনি।</p>}
        </div>
      )}

      {tab === 'rooms' && (
        <>
          <div className="card">
            <h2>এখন খোলা রুম</h2>
            <LiveRooms rooms={rooms} onClose={closeRoom} />
          </div>
          <div className="card" style={{ marginTop: 16, overflowX: 'auto' }}>
            <h2>সব রুমের ইতিহাস</h2>
            <RoomHistory rows={history} onGames={showRoomGames} />
          </div>
        </>
      )}

      {tab === 'games' && (
        <div className="card" style={{ overflowX: 'auto' }}>
          <GamesTable games={games} history={history} roomFilter={roomFilter} setRoomFilter={setRoomFilter} />
        </div>
      )}
    </div>
  )
}

// ---------------------------------------------------------------- overview

function Stat({ n, label, sub, live }) {
  return <div className={`adm-stat ${live ? 'adm-stat--live' : ''}`}><b>{n}</b>{label}{sub && <small>{sub}</small>}</div>
}

function Overview({ stats: s, rooms, games, onClose, onOpenGames }) {
  const levels = ['easy', 'medium', 'hard'].map((k) => [LEVEL[k], s.byLevel.find((r) => r[0] === k)?.[1] || 0])
  const ends = s.byEnd.filter((r) => r[0] !== '').map(([k, n]) => [END[k] || k, n])
  const sizes = s.bySize.map(([k, n]) => [`${k} জন`, n])
  return (
    <>
      <h2 className="adm-h" style={{ marginTop: 0 }}>এই মুহূর্তে</h2>
      <div className="adm-grid">
        <Stat live n={s.playing} label="জন খেলছে" />
        <Stat live n={s.online} label="জন অনলাইনে" />
        <Stat live n={s.guestsOnline} label="জন গেস্ট অনলাইনে" />
        <Stat live n={s.liveRooms} label="টি রুম খোলা" />
      </div>

      <h2 className="adm-h">মোট হিসাব</h2>
      <div className="adm-grid">
        <Stat n={s.users} label="অ্যাকাউন্ট" sub={`${s.guests} জন গেস্ট আলাদা`} />
        <Stat n={s.newUsersDay} label="নতুন খেলোয়াড়, ২৪ ঘণ্টায়" sub="গেস্টসহ" />
        <Stat n={s.games} label="খেলা হয়েছে" sub={`২৪ ঘণ্টায় ${s.gamesDay}টি`} />
        <Stat n={s.roomsOpened} label="রুম খোলা হয়েছে" />
        <Stat n={dur(s.avgSecs)} label="গড় খেলার সময়" sub="মিনিট:সেকেন্ড" />
        <Stat n={s.avgTurns} label="গড় টোকা প্রতি খেলা" />
        <Stat n={s.vsComputer} label="কম্পিউটারসহ খেলা" sub={`${pct(s.vsComputer, s.games)}%`} />
        <Stat n={s.withGuests} label="গেস্টসহ খেলা" sub={`${pct(s.withGuests, s.games)}%`} />
        <Stat n={s.coinsPaid} label="কয়েন দেওয়া হয়েছে" />
        <Stat n={s.blocked} label="ব্লক করা" />
      </div>

      <div className="grid2" style={{ marginTop: 16 }}>
        <div className="card">
          <h2>দিনে কতটি খেলা <span className="muted" style={{ fontSize: 13, fontWeight: 400 }}>· শেষ ১৪ দিন</span></h2>
          <DayChart days={s.days} />
          <details style={{ marginTop: 12 }}>
            <summary className="muted" style={{ cursor: 'pointer' }}>সংখ্যাগুলো টেবিলে দেখুন</summary>
            <table style={{ marginTop: 8 }}>
              <thead><tr><th>দিন</th><th>খেলা</th><th>নতুন খেলোয়াড়</th></tr></thead>
              <tbody>{[...s.days].reverse().map((d) => <tr key={d.day}><td>{new Date(d.day * 1000).toLocaleDateString()}</td><td>{d.games}</td><td>{d.users}</td></tr>)}</tbody>
            </table>
          </details>
        </div>
        <div className="card">
          <h2>কীভাবে খেলা হচ্ছে</h2>
          <Split title="মাত্রা" rows={levels} />
          <Split title="খেলোয়াড় সংখ্যা" rows={sizes} />
          <Split title="কীভাবে শেষ হলো" rows={ends} />
        </div>
      </div>

      <div className="grid2" style={{ marginTop: 16 }}>
        <div className="card">
          <h2>এখন খোলা রুম</h2>
          <LiveRooms rooms={rooms.slice(0, 4)} onClose={onClose} />
        </div>
        <div className="card" style={{ overflowX: 'auto' }}>
          <div className="row between"><h2 style={{ margin: 0 }}>সাম্প্রতিক খেলা</h2><button className="ghost small" onClick={onOpenGames}>সব খেলা</button></div>
          <table style={{ marginTop: 10 }}>
            <thead><tr><th>রুম</th><th>মাত্রা</th><th>বিজয়ী</th><th>সময়</th></tr></thead>
            <tbody>{games.slice(0, 8).map((g) => <tr key={g.id}><td>{g.code}</td><td>{LEVEL[g.difficulty] || g.difficulty}</td><td>{g.winner || '–'}</td><td className="muted">{dur(g.secs)}</td></tr>)}</tbody>
          </table>
          {games.length === 0 && <p className="muted">এখনো কোনো খেলা হয়নি।</p>}
        </div>
      </div>

      <div className="card" style={{ marginTop: 16, overflowX: 'auto' }}>
        <h2>কলম কতটা চলে <span className="muted" style={{ fontSize: 13, fontWeight: 400 }}>· কম্পিউটার বাদে</span></h2>
        {s.pens.length === 0 ? <p className="muted">এখনো তথ্য নেই।</p> : (
          <table>
            <thead><tr><th>কলম</th><th>খেলা</th><th>জয়</th><th>জয়ের হার</th></tr></thead>
            <tbody>{s.pens.map((p) => <tr key={p.pen}><td>{PEN_NAMES[p.pen] || p.pen}</td><td>{p.games}</td><td>{p.wins}</td><td>{pct(p.wins, p.games)}%</td></tr>)}</tbody>
          </table>
        )}
      </div>
    </>
  )
}

/** Games per day as columns. One series, so no legend: the heading names it. Hover a column for its exact count. */
function DayChart({ days }) {
  const max = Math.max(1, ...days.map((d) => d.games))
  const top = max <= 4 ? max : Math.ceil(max / 5) * 5
  const label = (d) => { const t = new Date(d.day * 1000); return `${t.getDate()}/${t.getMonth() + 1}` }
  return (
    <div className="cols" role="img" aria-label={`শেষ ১৪ দিনে ${days.reduce((a, d) => a + d.games, 0)}টি খেলা`}>
      <div className="cols__y" aria-hidden="true"><span>{top}</span><span>{Math.round(top / 2)}</span><span>0</span></div>
      <div className="cols__plot">
        {days.map((d) => (
          <div key={d.day} className="col" tabIndex={0} aria-label={`${label(d)}: ${d.games}টি খেলা`}>
            <div className="col__bar" style={{ height: `${(d.games / top) * 100}%` }} />
            <span className="col__tip"><b>{d.games}টি খেলা</b> <span>· {label(d)} · {d.users} নতুন</span></span>
          </div>
        ))}
      </div>
      <div className="cols__x" aria-hidden="true">{days.map((d, i) => <span key={d.day}>{i % 2 === days.length % 2 ? '' : label(d)}</span>)}</div>
    </div>
  )
}

function Split({ title, rows }) {
  const total = rows.reduce((a, r) => a + r[1], 0)
  return (
    <div style={{ marginBottom: 14 }}>
      <h3 style={{ margin: '0 0 8px', fontSize: 14 }}>{title}</h3>
      {total === 0 ? <p className="muted" style={{ margin: 0 }}>এখনো তথ্য নেই।</p> : (
        <div className="split">
          {rows.map(([k, n]) => (
            <div key={k} className="split__row">
              <span>{k}</span>
              <span className="split__track"><span className="split__fill" style={{ display: 'block', width: `${pct(n, total)}%` }} /></span>
              <span className="split__num">{n} · {pct(n, total)}%</span>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}

// ---------------------------------------------------------------- rooms

function LiveRooms({ rooms, onClose }) {
  if (rooms.length === 0) return <p className="muted">কোনো রুম খোলা নেই।</p>
  return (
    <div className="roomcards">
      {rooms.map((r) => (
        <div key={r.code} className="roomcard">
          <div className="row between" style={{ marginBottom: 6 }}>
            <b style={{ letterSpacing: 2 }}>{r.code}</b>
            <span className={`pill ${r.phase === 'playing' ? 'pill--easy' : ''}`}>{PHASE[r.phase] || r.phase}</span>
          </div>
          <div className="muted" style={{ fontSize: 13, marginBottom: 6 }}>
            হোস্ট {r.host} · {LEVEL[r.difficulty]} · খোলা {when(r.createdAt)}<br />
            {r.people} জন সংযুক্ত{r.watchers ? ` (${r.watchers} জন দেখছে)` : ''}{r.guests ? ` · ${r.guests} গেস্ট` : ''} · এই রুমে {r.games}টি খেলা
            {r.phase === 'playing' && <> · {r.turns}টি টোকা · শুরু {when(r.startedAt)}</>}
          </div>
          {r.seats.map((p, i) => (
            <div key={i} className="roomcard__seat">
              <span>{p.name}{p.bot && <span className="tag tag--bot">কম্পিউটার</span>}{p.guest && <span className="tag tag--guest">গেস্ট</span>}{!p.online && <span className="tag">নেই</span>}</span>
              <span className="muted">{PEN_NAMES[p.pen] || p.pen} · {p.alive ? `❤ ${p.lives}` : 'আউট'}{p.knocks ? ` · ⚡ ${p.knocks}` : ''}</span>
            </div>
          ))}
          {r.seats.length === 0 && <p className="muted" style={{ margin: 0, fontSize: 13 }}>এখনো কেউ বসেনি।</p>}
          <button className="danger small" style={{ marginTop: 8 }} onClick={() => confirm(`${r.code} রুম বন্ধ করবেন? খেলোয়াড়দের সংযোগ কেটে যাবে।`) && onClose(r.code)}>বন্ধ করুন</button>
        </div>
      ))}
    </div>
  )
}

function RoomHistory({ rows, onGames }) {
  if (rows.length === 0) return <p className="muted">এখনো কোনো রুম খোলা হয়নি।</p>
  return (
    <table>
      <thead><tr><th>কোড</th><th>হোস্ট</th><th>খোলা</th><th>চলেছে</th><th>সর্বোচ্চ লোক</th><th>খেলা</th><th>অবস্থা</th></tr></thead>
      <tbody>{rows.map((r) => (
        <tr key={r.id}>
          <td><b>{r.code}</b>{r.bots && <span className="tag tag--bot">কম্পিউটার</span>}</td>
          <td>{r.host}</td>
          <td className="muted">{when(r.createdAt)}</td>
          <td>{r.closedAt ? dur(r.closedAt - r.createdAt) : '–'}</td>
          <td>{r.peak}</td>
          <td>{r.games > 0 ? <button className="ghost small" onClick={() => onGames(r.id)}>{r.games}টি দেখুন</button> : 0}</td>
          <td>{r.closedAt ? <span className="muted">{CLOSE[r.closeReason] || r.closeReason}</span> : <span className="pill pill--easy">খোলা</span>}</td>
        </tr>
      ))}</tbody>
    </table>
  )
}

// ---------------------------------------------------------------- games

function GamesTable({ games, history, roomFilter, setRoomFilter }) {
  const [open, setOpen] = useState(null)
  const [level, setLevel] = useState('')
  const [text, setText] = useState('')
  const room = roomFilter ? history.find((h) => h.id === roomFilter) : null
  const list = games.filter((g) => {
    if (roomFilter && g.roomId !== roomFilter) return false
    if (level && g.difficulty !== level) return false
    const t = text.trim().toLowerCase()
    return !t || g.code.toLowerCase().includes(t) || g.players.some((p) => p.name.toLowerCase().includes(t))
  })
  return (
    <>
      <div className="filters">
        <input value={text} onChange={(e) => setText(e.target.value)} placeholder="রুম কোড বা খেলোয়াড়ের নাম" aria-label="খেলা খুঁজুন" />
        <select value={level} onChange={(e) => setLevel(e.target.value)} aria-label="মাত্রা">
          <option value="">সব মাত্রা</option>
          {Object.entries(LEVEL).map(([k, v]) => <option key={k} value={k}>{v}</option>)}
        </select>
        {room && <span className="pill pill--warn">রুম {room.code} <button type="button" className="ghost small" style={{ padding: '0 6px', marginLeft: 4 }} onClick={() => setRoomFilter(null)} aria-label="রুমের ফিল্টার সরান">✕</button></span>}
        <span className="muted" style={{ fontSize: 13 }}>{list.length}টি খেলা · সারিতে চাপলে পুরো হিসাব</span>
      </div>
      {list.length === 0 ? <p className="muted">কোনো খেলা মেলেনি।</p> : (
        <table>
          <thead><tr><th>শেষ হয়েছে</th><th>রুম</th><th>মাত্রা</th><th>জন</th><th>বিজয়ী</th><th>লুজার</th><th>টোকা</th><th>সময়</th></tr></thead>
          <tbody>{list.map((g) => (
            <Fragment key={g.id}>
              <tr className={`clickable ${open === g.id ? 'open' : ''}`} onClick={() => setOpen(open === g.id ? null : g.id)} tabIndex={0} onKeyDown={(e) => { if (e.key === 'Enter') setOpen(open === g.id ? null : g.id) }} aria-expanded={open === g.id}>
                <td className="muted">{when(g.finishedAt)}</td>
                <td><b>{g.code}</b></td>
                <td>{LEVEL[g.difficulty] || g.difficulty}</td>
                <td>{g.size}{g.bots ? <span className="tag tag--bot">{g.bots} 🤖</span> : null}{g.guests ? <span className="tag tag--guest">{g.guests} গেস্ট</span> : null}</td>
                <td>{g.winner || '–'}</td>
                <td>{g.looser || '–'}</td>
                <td>{g.turns || '–'}</td>
                <td>{dur(g.secs)}</td>
              </tr>
              {open === g.id && (
                <tr className="gdetail"><td colSpan={8}>
                  <div className="gdetail__meta">
                    <span>খেলা নং <b>#{g.id}</b></span>
                    <span>শুরু <b>{g.startedAt ? when(g.startedAt) : '–'}</b></span>
                    <span>শেষ <b>{when(g.finishedAt)}</b></span>
                    <span>সময় <b>{dur(g.secs)}</b></span>
                    <span>টোকা <b>{g.turns}</b></span>
                    <span>সময় পেরিয়ে বাদ <b>{g.skips}</b></span>
                    <span>শেষ হলো <b>{END[g.end] ?? g.end}</b></span>
                  </div>
                  <table>
                    <thead><tr><th>স্থান</th><th>খেলোয়াড়</th><th>কলম</th><th>⚡ ফেলেছে</th><th>💥 পড়েছে</th><th>❤ বাকি</th><th>কয়েন</th></tr></thead>
                    <tbody>{g.players.map((p) => (
                      <tr key={`${g.id}-${p.id}`}>
                        <td>{p.rank === 1 ? '🥇' : p.rank}{p.looser && <span className="pill pill--out" style={{ marginLeft: 6 }}>লুজার</span>}</td>
                        <td>{p.name}{p.bot && <span className="tag tag--bot">কম্পিউটার</span>}{p.guest && <span className="tag tag--guest">গেস্ট</span>}</td>
                        <td>{PEN_NAMES[p.pen] || p.pen || '–'}</td>
                        <td>{p.knocks}</td>
                        <td>{p.falls}</td>
                        <td>{p.lives}</td>
                        <td>{p.coins || 0}</td>
                      </tr>
                    ))}</tbody>
                  </table>
                </td></tr>
              )}
            </Fragment>
          ))}</tbody>
        </table>
      )}
    </>
  )
}
