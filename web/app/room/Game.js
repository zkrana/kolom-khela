'use client'
import { useCallback, useEffect, useRef, useState } from 'react'
import { api, getToken, setToken, wsUrl } from '../../lib/api'
import { isMuted, setMuted, sfx } from '../../lib/sound'
import { COLORS, DESKS, SIZE, drawAim, drawBoard, drawBody, drawTurnHint } from './draw'
import { DifficultyPicker, EndOverlay, LEVELS, PlayerCard } from './panels'

const TURN_SECS = { easy: 35, medium: 25, hard: 18 }

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
    <div className="wrap" style={{ maxWidth: 460, minHeight: '100vh', display: 'grid', alignContent: 'center' }}>
      <div className="card hero">
        <h1 style={{ fontSize: 28 }}>{info && info.host ? `${info.host} আপনাকে ডেকেছেন` : 'আপনাকে খেলতে ডাকা হয়েছে'}</h1>
        <p className="muted"><b style={{ color: 'var(--gold)', letterSpacing: 2 }}>{code}</b> রুমের কলম খেলায় ঢুকুন। শুধু নাম আর ইমেইল দিলেই হবে, পাসওয়ার্ড লাগবে না।</p>
        {info === false && <p className="err">রুমটা হয়তো বন্ধ হয়ে গেছে, তবু চেষ্টা করে দেখতে পারেন।</p>}
        <form onSubmit={submit}>
          <label htmlFor="gname">আপনার নাম</label>
          <input id="gname" required minLength={2} maxLength={24} autoComplete="nickname" value={f.name} onChange={(e) => setF({ ...f, name: e.target.value })} />
          <label htmlFor="gemail">ইমেইল</label>
          <input id="gemail" type="email" required autoComplete="email" value={f.email} onChange={(e) => setF({ ...f, email: e.target.value })} />
          {error && <p className="err" role="alert">{error}</p>}
          <div style={{ marginTop: 16 }}><button disabled={busy} style={{ width: '100%', fontSize: 18, padding: 13 }}>{busy ? 'ঢুকছি...' : 'খেলায় ঢুকুন'}</button></div>
        </form>
        <p className="muted" style={{ marginTop: 14 }}>অ্যাকাউন্ট আছে? <a href="/" onClick={(e) => { e.preventDefault(); later() }}>লগইন করুন</a></p>
      </div>
    </div>
  )
}

function ShareBar({ url, host }) {
  const [copied, setCopied] = useState(false)
  const text = `${host ? `${host} আপনাকে` : 'আসুন,'} কলম খেলায় ডেকেছেন। ক্লাসের বেঞ্চের সেই টোকা দেওয়ার খেলা, এবার অনলাইনে!`
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
        <input id="invite-url" readOnly value={url} onFocus={(e) => e.target.select()} aria-label="ইনভাইট লিংক" style={{ fontSize: 13, padding: '9px 10px' }} />
        <button type="button" className="small" onClick={copy} style={{ whiteSpace: 'nowrap', padding: '9px 12px' }}>{copied ? 'কপি হয়েছে' : 'লিংক কপি'}</button>
      </div>
      <div className="row" style={{ gap: 6, marginTop: 8 }}>
        <button type="button" style={b('#1faa59')} onClick={() => open(`https://wa.me/?text=${enc(`${text} ${url}`)}`)}>WhatsApp</button>
        <button type="button" style={b('#1877f2')} onClick={() => open(`https://www.facebook.com/sharer/sharer.php?u=${enc(url)}&quote=${enc(text)}`)}>Facebook</button>
        <button type="button" style={b('#0a66c2')} onClick={() => open(`https://www.linkedin.com/sharing/share-offsite/?url=${enc(url)}`)}>LinkedIn</button>
        {native && <button type="button" className="ghost" style={{ flex: '1 1 46%', padding: '9px 8px', fontSize: 14 }} onClick={() => navigator.share({ title: 'কলম খেলা', text, url }).catch(() => {})}>আরও...</button>}
      </div>
    </div>
  )
}

export default function Game({ code }) {
  const canvas = useRef(null)
  const ws = useRef(null)
  const bodies = useRef(new Map())
  const ghosts = useRef([])
  const spawns = useRef(new Map()) // id -> when it came back, for the flash
  const aim = useRef(null)
  const st = useRef(null)
  const recvAt = useRef(0)
  const meRef = useRef(null)
  const lastTick = useRef(0)
  const [state, setState] = useState(null)
  const [me, setMe] = useState(null)
  const [logs, setLogs] = useState([])
  const [conn, setConn] = useState('connecting') // connecting | live | closed | gone
  const [, setTick] = useState(0)
  const [desk, setDesk] = useState('bench')
  const [auth, setAuth] = useState('checking') // checking | need | ok
  const [invite0, setInvite0] = useState(null)
  const [mute, setMute] = useState(false)
  const [shotInfo, setShotInfo] = useState(null) // live power readout under the desk while aiming
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

  const pushLog = useCallback((m) => setLogs((l) => [...l.slice(-8), { id: Date.now() + Math.random(), text: m }]), [])

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
        } else if (m.t === 'respawn') {
          // The kolom is put back by the next state; this only says "flash it when it lands".
          for (const id of m.ids) spawns.current.set(id, performance.now() + 120)
          sfx.respawn()
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
      const live = s?.phase === 'playing' && !s.simulating
      for (const b of bodies.current.values()) {
        const born = spawns.current.get(b.id)
        const spawn = born ? Math.max(0, 1 - (t - born) / 650) : 0
        if (born && spawn <= 0) spawns.current.delete(b.id)
        // While you aim, your own name tag would sit under the power reading, so it steps aside.
        const hideName = !!aim.current && b.id === mySeat
        drawBody(ctx, b, t, { active: live && b.id === s.turn, name: hideName ? null : s?.players[b.id]?.name, seat: b.id, spawn })
      }
      ghosts.current = ghosts.current.filter((g) => t - g.born < 600)
      for (const g of ghosts.current) {
        const p = (t - g.born) / 600
        drawBody(ctx, { ...g, scale: 1 - p * 0.7, alpha: 1 - p }, t, { seat: g.id })
      }
      const pen = bodies.current.get(mySeat)
      if (aim.current && pen) {
        const p = drawAim(ctx, pen, aim.current, s?.shot, t)
        const now = Math.round(p.power * 100)
        if (now !== lastTick.current) { lastTick.current = now; setShotInfo({ power: now, danger: p.ready && (p.stop.x < 0 || p.stop.x > SIZE || p.stop.y < 0 || p.stop.y > SIZE), spin: Math.abs(p.turn || 0) > 0.35 }) }
      } else if (live && pen && s.turn === mySeat) {
        drawTurnHint(ctx, pen, t)
      }
      raf = requestAnimationFrame(loop)
    }
    raf = requestAnimationFrame(loop)
    return () => cancelAnimationFrame(raf)
  }, [mySeat])

  // ---- aiming: press anywhere on the desk and pull back; the kolom goes the way you pulled from. The finger lands on
  // the point of the kolom nearest to where you pressed, so pressing near an end spins it.
  const pt = (e) => {
    const r = canvas.current.getBoundingClientRect()
    return { x: ((e.clientX - r.left) / r.width) * SIZE, y: ((e.clientY - r.top) / r.height) * SIZE }
  }
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
  const release = () => {
    const a = aim.current
    aim.current = null
    setShotInfo(null)
    const pen = bodies.current.get(mySeat)
    if (!a || !pen || !myTurn) return
    const vx = a.sx - a.x, vy = a.sy - a.y
    const len = Math.hypot(vx, vy)
    if (len < 24) return
    send({ t: 'shoot', dx: vx, dy: vy, power: Math.min(len / 220, 1), lx: a.lx, ly: a.ly })
  }

  // A countdown tick in the last seconds of your own turn.
  const left = state && state.left_ms ? Math.max(0, Math.ceil((state.left_ms - (performance.now() - recvAt.current)) / 1000)) : null
  useEffect(() => {
    if (!myTurn || left == null || left > 5 || left < 1) return
    sfx.tick()
  }, [myTurn, left])

  const invite = typeof window !== 'undefined' ? `${window.location.origin}/room/?code=${code}` : ''
  if (auth === 'checking') return <div className="wrap muted">লোড হচ্ছে...</div>
  if (auth === 'need') return <GuestJoin code={code} info={invite0} onDone={() => setAuth('checking')} />
  if (conn === 'gone') return <div className="wrap"><div className="card"><h2>এই রুম বন্ধ হয়ে গেছে</h2><p className="muted">লবি থেকে নতুন খেলা শুরু করুন।</p><a className="btn" href="/">লবিতে ফিরুন</a></div></div>

  const isHost = !!state && !!me && state.host === me.id
  const turnName = state?.players[state.turn]?.name
  const level = LEVELS.find((l) => l.key === state?.difficulty)
  const lobby = state?.phase === 'lobby'
  const seats = state?.players || []
  const enriched = state ? { ...state, turnSecs: TURN_SECS[state.difficulty] || 25 } : null

  // The one line under the desk that says what to do right now.
  let prompt = { text: 'সংযোগ হচ্ছে...', tone: '' }
  if (state) {
    if (lobby) prompt = { text: seats.length < 2 ? 'আরও একজন দরকার। ইনভাইট লিংক পাঠান।' : isHost ? 'সবাই এসে গেছে, খেলা শুরু করুন।' : 'হোস্টের শুরুর অপেক্ষায়...', tone: '' }
    else if (state.phase === 'over') prompt = { text: 'খেলা শেষ', tone: '' }
    else if (state.simulating) prompt = { text: 'কলম ছুটছে...', tone: 'go' }
    else if (myTurn) {
      prompt = shotInfo
        ? { text: shotInfo.danger ? `${shotInfo.power}% — এই শটে নিজের কলমই পড়ে যাবে!` : `${shotInfo.power}% শক্তি${shotInfo.spin ? ' · কলম ঘুরবে' : ''} — ছেড়ে দিন`, tone: shotInfo.danger ? 'bad' : 'go' }
        : { text: `আপনার পালা: বেঞ্চে চেপে পেছনে টেনে ছেড়ে দিন${left !== null ? ` · ${left} সে.` : ''}`, tone: 'you' }
    } else prompt = { text: `${turnName}-এর পালা${left !== null ? ` · ${left} সে.` : ''}`, tone: '' }
  }

  return (
    <div className="wrap">
      <div className="roombar">
        <a href="/" className="btn ghost small">&larr; লবি</a>
        <span className="roombar__code">রুম <b>{code}</b></span>
        {level && <span className={`pill pill--level pill--${level.key}`}>{level.icon} {level.name}</span>}
        {conn !== 'live' && <span className="pill pill--warn">আবার সংযোগ হচ্ছে...</span>}
      </div>

      <div className="game">
        <div className="boardwrap">
          <canvas
            ref={canvas}
            className={`board ${myTurn ? 'board--mine' : ''}`}
            onPointerDown={down}
            onPointerMove={move}
            onPointerUp={release}
            onPointerCancel={() => { aim.current = null; setShotInfo(null) }}
            aria-label="খেলার বেঞ্চ"
          />
          <p className={`prompt prompt--${prompt.tone}`}>{prompt.text}</p>
        </div>

        <div className="side">
          <div className="card">
            <div className="row between" style={{ marginBottom: 8 }}>
              <h3 style={{ margin: 0 }}>খেলোয়াড়</h3>
              {!lobby && state?.phase !== 'over' && <span className="muted" style={{ fontSize: 12 }}>⚡ ফেলা · ❤ প্রাণ</span>}
            </div>
            {seats.map((p) => <PlayerCard key={p.id} p={p} state={enriched} isMe={!!me && p.id === me.id} left={state.turn === p.seat ? left : null} />)}
            {lobby && Array.from({ length: Math.max(0, 2 - seats.length) }, (_, i) => (
              <div key={`empty${i}`} className="player player--empty"><span className="player__slot" aria-hidden="true">+</span><span className="muted">খালি চেয়ার</span></div>
            ))}

            {lobby && (
              <div className="lobbybox">
                <h3>মাত্রা বেছে নিন</h3>
                <DifficultyPicker value={state.difficulty} canEdit={isHost} onPick={(level) => send({ t: 'difficulty', level })} />
                <p className="muted levelblurb">{level?.blurb}{!isHost && ' হোস্ট বেছে নেবেন।'}</p>
                <h3 style={{ marginTop: 14 }}>বন্ধুদের ডাকুন</h3>
                <ShareBar url={invite} host={me?.name} />
                <div style={{ marginTop: 10 }}>
                  {isHost
                    ? <button onClick={() => { sfx.pick(); send({ t: 'start' }) }} disabled={seats.length < 2} className="bigcta">{seats.length < 2 ? 'দুজন লাগবে' : `খেলা শুরু করুন · ${level?.coins} কয়েন`}</button>
                    : <p className="muted">হোস্ট খেলা শুরু করবেন।</p>}
                </div>
              </div>
            )}
          </div>

          <div className="card">
            <h3>বেঞ্চ</h3>
            <div className="row" style={{ gap: 8 }}>
              <div className="swatches">
                {Object.entries(DESKS).map(([k, d]) => <button key={k} type="button" className={`swatch ${desk === k ? 'on' : ''}`} style={{ background: d.swatch }} title={d.label} aria-label={d.label} onClick={() => pickDesk(k)} />)}
              </div>
              <button type="button" className="ghost small" onClick={toggleMute} style={{ marginLeft: 'auto' }}>{mute ? '🔇 শব্দ বন্ধ' : '🔊 শব্দ চালু'}</button>
            </div>
            <h3 style={{ marginTop: 14 }}>ঘটনা</h3>
            <div className="log">
              {logs.length ? logs.map((l) => <div key={l.id} className="logline">{l.text}</div>) : <span className="muted">এখনো কিছু হয়নি।</span>}
            </div>
          </div>
        </div>
      </div>

      {state?.phase === 'over' && <EndOverlay state={state} me={me} isHost={isHost} onRematch={() => send({ t: 'rematch' })} />}
    </div>
  )
}
