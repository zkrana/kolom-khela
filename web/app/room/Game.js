'use client'
import { useCallback, useEffect, useRef, useState } from 'react'
import { api, getToken, setToken, wsUrl } from '../../lib/api'
import { drawPen } from '../../lib/pens'
import { Avatar } from '../../lib/ui'
import { isMuted, setMuted, sfx } from '../../lib/sound'

const COLORS = ['#2f6df6', '#e53935', '#1faa59', '#8e44ad'] // one colour per player: name tag and the lobby list
const SIZE = 800

// Desk looks. Each is painted once into an offscreen canvas and then just copied every frame.
export const DESKS = {
  bench: { label: 'School bench', swatch: 'linear-gradient(135deg,#e0a867,#b4773c)' },
  felt: { label: 'Green felt', swatch: 'linear-gradient(135deg,#2aa371,#146b47)' },
  ocean: { label: 'Ocean', swatch: 'linear-gradient(135deg,#3b9be0,#1c4f93)' },
  classic: { label: 'Classic', swatch: 'linear-gradient(135deg,#efebe0,#d8d2c2)' },
}
const deskCache = new Map()

function rng(seed) {
  let a = seed
  return () => { a |= 0; a = (a + 0x6d2b79f5) | 0; let t = Math.imul(a ^ (a >>> 15), 1 | a); t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t; return ((t ^ (t >>> 14)) >>> 0) / 4294967296 }
}

function paintDesk(key) {
  const cv = document.createElement('canvas')
  cv.width = cv.height = SIZE
  const c = cv.getContext('2d')
  const r = rng(7)
  if (key === 'bench') {
    // planks of warm honey wood with grain
    for (let y = 0, i = 0; y < SIZE; y += 100, i++) {
      const l = 0.5 + r() * 0.12
      c.fillStyle = `hsl(${28 + r() * 6},${52 + r() * 8}%,${44 + l * 14}%)`
      c.fillRect(0, y, SIZE, 100)
      for (let k = 0; k < 14; k++) {
        c.strokeStyle = `rgba(90,50,15,${0.05 + r() * 0.09})`
        c.lineWidth = 1 + r() * 2
        const gy = y + 6 + r() * 88
        c.beginPath(); c.moveTo(0, gy)
        c.bezierCurveTo(SIZE * 0.3, gy + (r() - 0.5) * 14, SIZE * 0.65, gy + (r() - 0.5) * 14, SIZE, gy + (r() - 0.5) * 8)
        c.stroke()
      }
      c.fillStyle = 'rgba(60,30,8,.45)'
      c.fillRect(0, y, SIZE, 2.5)
      c.fillStyle = 'rgba(255,230,190,.18)'
      c.fillRect(0, y + 2.5, SIZE, 1.5)
      c.fillStyle = 'rgba(60,30,8,.35)' // plank joins
      c.fillRect(150 + r() * 500, y, 2.5, 100)
    }
  } else if (key === 'felt') {
    const g = c.createRadialGradient(SIZE / 2, SIZE / 2, 80, SIZE / 2, SIZE / 2, SIZE * 0.75)
    g.addColorStop(0, '#2ba873'); g.addColorStop(1, '#10603f')
    c.fillStyle = g; c.fillRect(0, 0, SIZE, SIZE)
    for (let i = 0; i < 9000; i++) { c.fillStyle = r() > 0.5 ? 'rgba(255,255,255,.05)' : 'rgba(0,0,0,.07)'; c.fillRect(r() * SIZE, r() * SIZE, 2, 2) }
    c.strokeStyle = 'rgba(255,255,255,.22)'; c.lineWidth = 3
    c.beginPath(); c.arc(SIZE / 2, SIZE / 2, 120, 0, 7); c.stroke()
  } else if (key === 'ocean') {
    const g = c.createLinearGradient(0, 0, SIZE, SIZE)
    g.addColorStop(0, '#46a6ea'); g.addColorStop(1, '#1b4a8f')
    c.fillStyle = g; c.fillRect(0, 0, SIZE, SIZE)
    c.strokeStyle = 'rgba(255,255,255,.07)'; c.lineWidth = 14
    for (let d = -SIZE; d < SIZE * 2; d += 60) { c.beginPath(); c.moveTo(d, 0); c.lineTo(d - SIZE, SIZE); c.stroke() }
  } else {
    c.fillStyle = '#e8e4da'; c.fillRect(0, 0, SIZE, SIZE)
    c.fillStyle = 'rgba(120,110,95,.18)'
    for (let y = 14; y < SIZE; y += 22) for (let x = ((y / 22) % 2) * 11 + 6; x < SIZE; x += 22) c.fillRect(x, y, 2, 2)
  }
  // soft light in the middle, darker towards the corners
  const v = c.createRadialGradient(SIZE / 2, SIZE / 2, SIZE * 0.25, SIZE / 2, SIZE / 2, SIZE * 0.78)
  v.addColorStop(0, 'rgba(255,255,255,.10)'); v.addColorStop(1, 'rgba(0,0,0,.28)')
  c.fillStyle = v; c.fillRect(0, 0, SIZE, SIZE)
  // the drop-off at every edge
  for (const [x, y, w, h, gx, gy, gw, gh] of [[0, 0, SIZE, 26, 0, 0, 0, 26], [0, SIZE - 26, SIZE, 26, 0, SIZE, 0, -26], [0, 0, 26, SIZE, 0, 0, 26, 0], [SIZE - 26, 0, 26, SIZE, SIZE, 0, -26, 0]]) {
    const lg = c.createLinearGradient(gx, gy, gx + gw, gy + gh)
    lg.addColorStop(0, 'rgba(25,14,5,.6)'); lg.addColorStop(1, 'rgba(25,14,5,0)')
    c.fillStyle = lg; c.fillRect(x, y, w, h)
  }
  return cv
}

function drawBoard(ctx, key) {
  let cv = deskCache.get(key)
  if (!cv) { cv = paintDesk(key); deskCache.set(key, cv) }
  ctx.drawImage(cv, 0, 0)
}

function capsule(ctx, hl, r) {
  ctx.beginPath()
  ctx.moveTo(-hl, -r)
  ctx.lineTo(hl, -r)
  ctx.arc(hl, 0, r, -Math.PI / 2, Math.PI / 2)
  ctx.lineTo(-hl, r)
  ctx.arc(-hl, 0, r, Math.PI / 2, Math.PI * 1.5)
  ctx.closePath()
}

function drawBody(ctx, b, t, active, name, seat) {
  const s = b.scale ?? 1
  ctx.save()
  ctx.translate(b.x, b.y)
  ctx.scale(s, s)
  ctx.globalAlpha = b.alpha ?? 1
  ctx.rotate(b.ang || 0)
  ctx.save()
  ctx.translate(4, 7)
  ctx.fillStyle = 'rgba(0,0,0,.22)'
  capsule(ctx, b.hl, b.r)
  ctx.fill()
  ctx.restore()
  if (active) {
    ctx.strokeStyle = '#ff9f1c'
    ctx.lineWidth = 4
    ctx.globalAlpha = (b.alpha ?? 1) * (0.45 + 0.4 * Math.sin(t / 150))
    capsule(ctx, b.hl + 7, b.r + 7)
    ctx.stroke()
    ctx.globalAlpha = b.alpha ?? 1
  }
  drawPen(ctx, b.style, b.hl, b.r)
  ctx.restore()
  if (name && !b.scale) {
    ctx.save()
    ctx.font = 'bold 15px system-ui'
    ctx.textAlign = 'center'
    ctx.textBaseline = 'middle'
    const y = b.y - (Math.abs(Math.sin(b.ang || 0)) * (b.hl + b.r) + 24)
    const w = ctx.measureText(name).width + 16
    ctx.fillStyle = COLORS[seat % 4]
    ctx.beginPath(); ctx.roundRect(b.x - w / 2, y - 11, w, 22, 11); ctx.fill()
    ctx.fillStyle = seat % 4 === 3 ? '#fff' : '#fff'
    ctx.fillText(name, b.x, y + 1)
    ctx.restore()
  }
}

function GuestJoin({ code, info, onDone }) {
  const [f, setF] = useState({ name: '', email: '' })
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const submit = async (e) => {
    e.preventDefault()
    setBusy(true); setError('')
    try {
      const d = await api('/api/auth/guest', { method: 'POST', body: f })
      setToken(d.token)
      onDone()
    } catch (err) { setError(err.message); setBusy(false) }
  }
  const later = () => { try { sessionStorage.setItem('kolom_next', `/room/?code=${code}`) } catch {}; window.location.href = '/' }
  return (
    <div className="wrap" style={{ maxWidth: 460, paddingTop: '8vh' }}>
      <div className="card hero">
        <h1 style={{ fontSize: 28 }}>{info && info.host ? `${info.host} invited you` : 'You are invited'}</h1>
        <p className="muted">Join the Kolom Khela game in room <b style={{ color: 'var(--gold)', letterSpacing: 2 }}>{code}</b>. Just a name and your email, no password needed.</p>
        {info === false && <p className="err">This room may have closed, but you can still try.</p>}
        <form onSubmit={submit}>
          <label htmlFor="gname">Your name</label>
          <input id="gname" required minLength={2} maxLength={24} autoComplete="nickname" value={f.name} onChange={(e) => setF({ ...f, name: e.target.value })} />
          <label htmlFor="gemail">Email</label>
          <input id="gemail" type="email" required autoComplete="email" value={f.email} onChange={(e) => setF({ ...f, email: e.target.value })} />
          {error && <p className="err" role="alert">{error}</p>}
          <div style={{ marginTop: 16 }}><button disabled={busy} style={{ width: '100%', fontSize: 18, padding: 13 }}>{busy ? 'Joining...' : 'Join the game'}</button></div>
        </form>
        <p className="muted" style={{ marginTop: 14 }}>Already have an account? <a href="/" onClick={(e) => { e.preventDefault(); later() }}>Log in</a></p>
      </div>
    </div>
  )
}

function ShareBar({ url, host }) {
  const [copied, setCopied] = useState(false)
  const text = `${host ? `${host} invited you` : 'Come play'} to Kolom Khela, the pen flicking game from school days!`
  const enc = encodeURIComponent
  const open = (href) => window.open(href, '_blank', 'noopener,noreferrer,width=640,height=560')
  const copy = async () => {
    try { await navigator.clipboard.writeText(url) } catch {
      const el = document.getElementById('invite-url'); el?.select(); try { document.execCommand('copy') } catch {}
    }
    setCopied(true); setTimeout(() => setCopied(false), 1600)
  }
  const native = typeof navigator !== 'undefined' && !!navigator.share
  const b = (bg) => ({ background: bg, color: '#fff', boxShadow: 'none', flex: '1 1 46%', padding: '9px 8px', fontSize: 14 })
  return (
    <div>
      <div className="row" style={{ flexWrap: 'nowrap', gap: 6 }}>
        <input id="invite-url" readOnly value={url} onFocus={(e) => e.target.select()} aria-label="Invite link" style={{ fontSize: 13, padding: '9px 10px' }} />
        <button type="button" className="small" onClick={copy} style={{ whiteSpace: 'nowrap', padding: '9px 12px' }}>{copied ? 'Copied' : 'Copy link'}</button>
      </div>
      <div className="row" style={{ gap: 6, marginTop: 8 }}>
        <button type="button" style={b('#1faa59')} onClick={() => open(`https://wa.me/?text=${enc(`${text} ${url}`)}`)}>WhatsApp</button>
        <button type="button" style={b('#1877f2')} onClick={() => open(`https://www.facebook.com/sharer/sharer.php?u=${enc(url)}&quote=${enc(text)}`)}>Facebook</button>
        <button type="button" style={b('#0a66c2')} onClick={() => open(`https://www.linkedin.com/sharing/share-offsite/?url=${enc(url)}`)}>LinkedIn</button>
        {native && <button type="button" className="ghost" style={{ flex: '1 1 46%', padding: '9px 8px', fontSize: 14 }} onClick={() => navigator.share({ title: 'Kolom Khela', text, url }).catch(() => {})}>More...</button>}
      </div>
    </div>
  )
}

export default function Game({ code }) {
  const canvas = useRef(null)
  const ws = useRef(null)
  const bodies = useRef(new Map())
  const ghosts = useRef([])
  const aim = useRef(null)
  const st = useRef(null)
  const recvAt = useRef(0)
  const meRef = useRef(null)
  const [state, setState] = useState(null)
  const [me, setMe] = useState(null)
  const [logs, setLogs] = useState([])
  const [conn, setConn] = useState('connecting') // connecting | live | closed | gone
  const [, setTick] = useState(0)
  const [desk, setDesk] = useState('bench')
  const [auth, setAuth] = useState('checking') // checking | need | ok
  const [invite0, setInvite0] = useState(null)
  const [mute, setMute] = useState(false)
  const deskRef = useRef('bench')

  useEffect(() => {
    let d = 'bench'
    try { d = localStorage.getItem('kolom_desk') || 'bench' } catch {}
    if (!DESKS[d]) d = 'bench'
    deskRef.current = d
    setDesk(d)
    setMute(isMuted())
  }, [])
  const pickDesk = (d) => { deskRef.current = d; setDesk(d); try { localStorage.setItem('kolom_desk', d) } catch {} }
  const toggleMute = () => { setMuted(!mute); setMute(!mute) }

  const pushLog = useCallback((m) => setLogs((l) => [...l.slice(-7), m]), [])

  // ---- connection
  useEffect(() => {
    let dead = false
    let retry = 0
    let timer
    const connect = () => {
      const s = new WebSocket(wsUrl(code))
      ws.current = s
      s.onopen = () => { retry = 0; setConn('live') }
      s.onmessage = (ev) => {
        let m
        try { m = JSON.parse(ev.data) } catch { return }
        if (m.t === 'state') {
          const seen = new Set()
          for (const b of m.bodies) {
            seen.add(b.id)
            const cur = bodies.current.get(b.id)
            if (cur) { cur.tx = b.x; cur.ty = b.y; cur.tang = b.ang; cur.r = b.r; cur.hl = b.hl; cur.style = b.style }
            else bodies.current.set(b.id, { id: b.id, style: b.style, r: b.r, hl: b.hl, x: b.x, y: b.y, ang: b.ang, tx: b.x, ty: b.y, tang: b.ang })
          }
          for (const [id, b] of bodies.current) if (!seen.has(id)) { ghosts.current.push({ ...b, born: performance.now() }); bodies.current.delete(id); sfx.fall() }
          const prev = st.current
          const meId = meRef.current?.id
          if (prev && prev.phase !== 'over' && m.phase === 'over') {
            const mine = m.results.find((r) => r.id === meId)
            if (mine) (mine.rank === 1 ? sfx.win : sfx.lose)()
          }
          if (m.simulating && !(prev && prev.simulating)) sfx.flick(0.6)
          if (m.phase === 'playing' && !m.simulating && (!prev || prev.simulating || prev.turn !== m.turn) && m.players[m.turn]?.id === meId) sfx.turn()
          st.current = m
          recvAt.current = performance.now()
          setState(m)
        } else if (m.t === 'frame') {
          const seen = new Set()
          for (const [id, x, y, ang] of m.b) {
            seen.add(id)
            const cur = bodies.current.get(id)
            if (cur) { cur.tx = x; cur.ty = y; cur.tang = ang }
          }
          for (const [id, b] of bodies.current) if (!seen.has(id)) { ghosts.current.push({ ...b, born: performance.now() }); bodies.current.delete(id); sfx.fall() }
          if (m.h > 25) sfx.hit(Math.min(1, m.h / 900))
        } else if (m.t === 'log') pushLog(m.msg)
        else if (m.t === 'error') pushLog(m.msg)
        else if (m.t === 'closed') { setConn('gone'); dead = true; s.close() }
      }
      s.onclose = () => {
        if (dead) return
        setConn('closed')
        if (retry++ < 6) timer = setTimeout(connect, 1200 * retry)
        else setConn('gone')
      }
    }
    if (auth !== 'ok') return
    connect()
    const iv = setInterval(() => setTick((t) => t + 1), 250)
    return () => { dead = true; clearTimeout(timer); clearInterval(iv); ws.current?.close() }
  }, [code, pushLog, auth])

  // Is there a signed-in player? If not, the page asks for a name and email instead of sending them away.
  useEffect(() => {
    if (auth !== 'checking') return
    if (!getToken()) { setAuth('need'); return }
    api('/api/me').then((d) => { meRef.current = d.user; setMe(d.user); setAuth('ok') }).catch(() => { setToken(''); setAuth('need') })
  }, [auth])
  useEffect(() => { api(`/api/rooms/${code}`).then(setInvite0).catch(() => setInvite0(false)) }, [code])

  const send = (o) => { if (ws.current?.readyState === 1) ws.current.send(JSON.stringify(o)) }

  const mySeat = state && me ? state.players.findIndex((p) => p.id === me.id) : -1
  const myTurn = !!state && state.phase === 'playing' && !state.simulating && mySeat >= 0 && state.turn === mySeat && state.players[mySeat]?.alive

  // ---- drawing
  useEffect(() => {
    const cv = canvas.current
    if (!cv) return
    const ctx = cv.getContext('2d')
    let raf
    const loop = (t) => {
      const css = cv.clientWidth || 600
      const dpr = Math.min(window.devicePixelRatio || 1, 2)
      if (cv.width !== Math.round(css * dpr)) { cv.width = Math.round(css * dpr); cv.height = Math.round(css * dpr) }
      ctx.setTransform((css * dpr) / SIZE, 0, 0, (css * dpr) / SIZE, 0, 0)
      drawBoard(ctx, deskRef.current)
      const s = st.current
      for (const b of bodies.current.values()) {
        b.x += (b.tx - b.x) * 0.55
        b.y += (b.ty - b.y) * 0.55
        let da = (b.tang ?? b.ang) - b.ang
        da = Math.atan2(Math.sin(da), Math.cos(da)) // shortest way round
        b.ang += da * 0.55
      }
      for (const b of bodies.current.values()) drawBody(ctx, b, t, s?.phase === 'playing' && !s.simulating && b.id === s.turn, s?.players[b.id]?.name, b.id)
      ghosts.current = ghosts.current.filter((g) => t - g.born < 600)
      for (const g of ghosts.current) {
        const p = (t - g.born) / 600
        drawBody(ctx, { ...g, scale: 1 - p * 0.7, alpha: 1 - p }, t, false, null, g.id)
      }
      const a = aim.current
      const pen = a && bodies.current.get(mySeat)
      if (a && pen) {
        const c = Math.cos(pen.ang), sn = Math.sin(pen.ang)
        const gx = pen.x + a.lx * c - a.ly * sn // where the finger lands on the kolom
        const gy = pen.y + a.lx * sn + a.ly * c
        const vx = a.sx - a.x, vy = a.sy - a.y
        const len = Math.hypot(vx, vy)
        const power = Math.min(len / 220, 1)
        ctx.fillStyle = '#ff9f1c'
        ctx.strokeStyle = '#fff'
        ctx.lineWidth = 3
        ctx.beginPath(); ctx.arc(gx, gy, 9, 0, 7); ctx.fill(); ctx.stroke()
        if (len > 4) {
          const ux = vx / len, uy = vy / len
          ctx.strokeStyle = 'rgba(40,20,5,.4)'; ctx.lineWidth = 3; ctx.setLineDash([])
          ctx.beginPath(); ctx.moveTo(a.sx, a.sy); ctx.lineTo(a.x, a.y); ctx.stroke() // the pull
          ctx.fillStyle = 'rgba(40,20,5,.5)'
          ctx.beginPath(); ctx.arc(a.sx, a.sy, 6, 0, 7); ctx.fill()
          ctx.strokeStyle = power > 0.8 ? '#ff4d3d' : power > 0.45 ? '#ffb020' : '#35c26b'
          ctx.lineWidth = 6; ctx.setLineDash([14, 10])
          ctx.beginPath(); ctx.moveTo(gx, gy); ctx.lineTo(gx + ux * (90 + power * 330), gy + uy * (90 + power * 330)); ctx.stroke()
          ctx.setLineDash([])
        }
      }
      raf = requestAnimationFrame(loop)
    }
    raf = requestAnimationFrame(loop)
    return () => cancelAnimationFrame(raf)
  }, [mySeat])

  // ---- aiming (pull back from your kolom, let go to flick)
  const pt = (e) => {
    const r = canvas.current.getBoundingClientRect()
    return { x: ((e.clientX - r.left) / r.width) * SIZE, y: ((e.clientY - r.top) / r.height) * SIZE }
  }
  // Press anywhere on the desk and pull back; the kolom goes the way you pulled from. The finger lands on the point of
  // the kolom nearest to where you pressed, so pressing near an end spins it and pressing in line with the middle slides it.
  const down = (e) => {
    if (!myTurn) return
    const p = pt(e)
    const pen = bodies.current.get(mySeat)
    if (!pen) return
    const c = Math.cos(-pen.ang), sn = Math.sin(-pen.ang)
    const dx = p.x - pen.x, dy = p.y - pen.y
    const lx = Math.max(-pen.hl - pen.r, Math.min(pen.hl + pen.r, dx * c - dy * sn))
    const ly = Math.max(-pen.r, Math.min(pen.r, dx * sn + dy * c))
    canvas.current.setPointerCapture(e.pointerId)
    aim.current = { sx: p.x, sy: p.y, x: p.x, y: p.y, lx, ly }
  }
  const move = (e) => { if (aim.current) { const p = pt(e); aim.current = { ...aim.current, x: p.x, y: p.y } } }
  const up = () => {
    const a = aim.current
    aim.current = null
    const pen = bodies.current.get(mySeat)
    if (!a || !pen || !myTurn) return
    const vx = a.sx - a.x, vy = a.sy - a.y
    const len = Math.hypot(vx, vy)
    if (len < 24) return
    send({ t: 'shoot', dx: vx, dy: vy, power: Math.min(len / 220, 1), lx: a.lx, ly: a.ly })
  }

  const invite = typeof window !== 'undefined' ? `${window.location.origin}/room/?code=${code}` : ''
  if (auth === 'checking') return <div className="wrap muted">Loading...</div>
  if (auth === 'need') return <GuestJoin code={code} info={invite0} onDone={() => setAuth('checking')} />

  if (conn === 'gone') return <div className="wrap"><div className="card"><h2>This room has closed</h2><p className="muted">Start a new game from the lobby.</p><a className="btn" href="/">Back to lobby</a></div></div>

  const left = state && state.left_ms ? Math.max(0, Math.ceil((state.left_ms - (performance.now() - recvAt.current)) / 1000)) : null
  const isHost = !!state && !!me && state.host === me.id
  const turnName = state?.players[state.turn]?.name

  return (
    <div className="wrap">
      <div className="row" style={{ justifyContent: 'space-between', marginBottom: 10 }}>
        <a href="/" className="btn ghost small">&larr; Lobby</a>
        <span className="muted">Room <b style={{ color: 'var(--gold)', letterSpacing: 2 }}>{code}</b> {conn !== 'live' && <span className="pill">reconnecting...</span>}</span>
      </div>
      <div className="game">
        <div>
          <canvas ref={canvas} className="board" onPointerDown={down} onPointerMove={move} onPointerUp={up} onPointerCancel={() => { aim.current = null }} aria-label="Game board" />
          <p className="muted" style={{ textAlign: 'center', margin: '8px 0 0' }}>
            {!state ? 'Connecting...' : state.phase === 'lobby' ? 'Waiting for players. Share the invite link.' : state.phase === 'over' ? 'Game over' : state.simulating ? 'Kolom flying...' : myTurn ? `Your turn: press anywhere on the desk, pull back and let go${left !== null ? ` (${left}s)` : ''}` : `${turnName}'s turn${left !== null ? ` (${left}s)` : ''}`}
          </p>
        </div>
        <div className="card">
          <h3>Players</h3>
          {(state?.players || []).map((p) => (
            <div key={p.id} className={`player ${state.phase === 'playing' && state.turn === p.seat && p.alive ? 'turn' : ''} ${!p.alive ? 'out' : ''}`}>
              <Avatar id={p.id} name={p.name} at={p.avatar} size={32} ring={COLORS[p.seat]} />
              <span style={{ flex: 1, minWidth: 0, overflow: 'hidden', textOverflow: 'ellipsis' }}>{p.name}{me && p.id === me.id ? ' (you)' : ''}{state.host === p.id ? ' *' : ''}</span>
              {!p.online && <span className="pill">away</span>}
              {!p.alive && <span className="pill">fell</span>}
              <span className="muted" style={{ fontSize: 12 }}>{p.pen}</span>
            </div>
          ))}
          {state?.phase === 'lobby' && (
            <div style={{ marginTop: 12 }}>
              <h3 style={{ margin: '0 0 8px' }}>Invite friends</h3>
              <ShareBar url={invite} host={me?.name} />
              <div style={{ height: 10 }} />
              {isHost ? <button onClick={() => send({ t: 'start' })} disabled={state.players.length < 2} style={{ width: '100%' }}>{state.players.length < 2 ? 'Need 2 players' : 'Start game'}</button> : <p className="muted">The host starts the game.</p>}
            </div>
          )}
          <h3 style={{ marginTop: 16 }}>Desk</h3>
          <div className="row" style={{ gap: 8 }}>
            <div className="swatches">
              {Object.entries(DESKS).map(([k, d]) => <button key={k} type="button" className={`swatch ${desk === k ? 'on' : ''}`} style={{ background: d.swatch }} title={d.label} aria-label={d.label} onClick={() => pickDesk(k)} />)}
            </div>
            <button type="button" className="ghost small" onClick={toggleMute} style={{ marginLeft: 'auto' }}>{mute ? 'Sound off' : 'Sound on'}</button>
          </div>
          <h3 style={{ marginTop: 16 }}>Activity</h3>
          <div className="log">{logs.length ? logs.map((l, i) => <div key={i}>{l}</div>) : 'Nothing yet.'}</div>
        </div>
      </div>

      {state?.phase === 'over' && (
        <div className="overlay">
          <div className="card">
            <h2>Game over</h2>
            {state.results.map((r) => (
              <div key={r.id} className="player">
                <b style={{ width: 22 }}>{r.rank}</b>
                <Avatar id={r.id} name={r.name} size={32} />
                <span style={{ flex: 1 }}>{r.name}{me && r.id === me.id ? ' (you)' : ''}</span>
                {r.looser && <span className="pill" style={{ background: 'var(--red)', color: '#fff' }}>looser</span>}
                {r.rank === 1 && <span className="pill" style={{ background: 'var(--gold)', color: '#2a1a05' }}>winner</span>}
                {r.reward > 0 && <b style={{ color: 'var(--gold)' }}>+{r.reward} coins</b>}
              </div>
            ))}
            <div className="row" style={{ marginTop: 14 }}>
              {isHost && <button onClick={() => send({ t: 'rematch' })}>Play again</button>}
              <a className="btn ghost" style={{ background: 'transparent', color: 'var(--text)', border: '1px solid var(--line)' }} href="/">Lobby</a>
            </div>
            {!isHost && <p className="muted">The host can start a rematch.</p>}
          </div>
        </div>
      )}
    </div>
  )
}
