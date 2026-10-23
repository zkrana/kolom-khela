'use client'
import { drawPen } from '../../lib/pens'

export const SIZE = 800
export const COLORS = ['#2f6df6', '#e53935', '#1faa59', '#8e44ad'] // one per seat: name tag, lives, lobby list

// Desk looks. Each is painted once into an offscreen canvas and then just copied every frame.
export const DESKS = {
  bench: { label: 'স্কুলের বেঞ্চ', swatch: 'linear-gradient(135deg,#a9743f,#5e3a1c)' },
  slate: { label: 'স্লেট', swatch: 'linear-gradient(135deg,#3d4454,#1b1f2a)' },
  felt: { label: 'সবুজ কাপড়', swatch: 'linear-gradient(135deg,#1f8a5f,#0a4a30)' },
  ocean: { label: 'সাগর', swatch: 'linear-gradient(135deg,#2f7fc4,#10335f)' },
  classic: { label: 'খাতার পাতা', swatch: 'linear-gradient(135deg,#f3eee2,#ddd5c2)' },
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
    // Varnished school wood. The planks run along the bench, away from the player, so their seams
    // never line up with a kolom lying across the desk, and the grain stays fine enough to read as
    // wood rather than as stripes.
    const base = c.createLinearGradient(0, 0, SIZE, SIZE)
    base.addColorStop(0, '#5b3b1f'); base.addColorStop(0.5, '#482d16'); base.addColorStop(1, '#351f0e')
    c.fillStyle = base; c.fillRect(0, 0, SIZE, SIZE)
    for (let x = -24; x < SIZE; x += 196) {
      c.fillStyle = `hsla(${23 + r() * 6},${38 + r() * 8}%,${21 + r() * 4}%,.8)`
      c.fillRect(x, 0, 196, SIZE)
      for (let k = 0; k < 46; k++) {
        const light = r() > 0.5
        c.strokeStyle = light ? `rgba(224,172,112,${0.03 + r() * 0.05})` : `rgba(24,12,3,${0.04 + r() * 0.08})`
        c.lineWidth = 0.6 + r() * 2
        const gx = x + 4 + r() * 188
        c.beginPath(); c.moveTo(gx, -10)
        c.bezierCurveTo(gx + (r() - 0.5) * 16, SIZE * 0.3, gx + (r() - 0.5) * 16, SIZE * 0.6, gx + (r() - 0.5) * 10, SIZE + 10)
        c.stroke()
      }
      c.fillStyle = 'rgba(14,7,2,.45)'; c.fillRect(x, 0, 3, SIZE)         // the shadow in the join
      c.fillStyle = 'rgba(255,206,150,.08)'; c.fillRect(x + 3, 0, 1.5, SIZE) // and the light on the edge beside it
    }
    // A broad sheen across the varnish, so the desk reads as a polished surface and not a flat fill.
    const sheen = c.createLinearGradient(0, SIZE * 0.1, SIZE, SIZE * 0.9)
    sheen.addColorStop(0, 'rgba(255,226,180,0)')
    sheen.addColorStop(0.42, 'rgba(255,226,180,.11)')
    sheen.addColorStop(0.6, 'rgba(255,226,180,.04)')
    sheen.addColorStop(1, 'rgba(255,226,180,0)')
    c.fillStyle = sheen; c.fillRect(0, 0, SIZE, SIZE)
  } else if (key === 'slate') {
    // Cool dark stone: every pen colour reads at a glance on it.
    const g = c.createLinearGradient(0, 0, SIZE, SIZE)
    g.addColorStop(0, '#39414f'); g.addColorStop(0.55, '#262c38'); g.addColorStop(1, '#171c25')
    c.fillStyle = g; c.fillRect(0, 0, SIZE, SIZE)
    for (let i = 0; i < 2600; i++) {
      c.fillStyle = r() > 0.5 ? 'rgba(255,255,255,.035)' : 'rgba(0,0,0,.07)'
      const s = 1 + r() * 2.5
      c.fillRect(r() * SIZE, r() * SIZE, s, s)
    }
    c.strokeStyle = 'rgba(255,255,255,.05)'; c.lineWidth = 2
    for (let i = 0; i < 26; i++) { // faint veins
      const x = r() * SIZE, y = r() * SIZE
      c.beginPath(); c.moveTo(x, y)
      c.bezierCurveTo(x + (r() - 0.5) * 300, y + (r() - 0.5) * 200, x + (r() - 0.5) * 300, y + (r() - 0.5) * 300, x + (r() - 0.5) * 420, y + (r() - 0.5) * 420)
      c.stroke()
    }
  } else if (key === 'felt') {
    const g = c.createRadialGradient(SIZE / 2, SIZE * 0.42, 60, SIZE / 2, SIZE / 2, SIZE * 0.8)
    g.addColorStop(0, '#1f9a67'); g.addColorStop(0.6, '#13724a'); g.addColorStop(1, '#07402a')
    c.fillStyle = g; c.fillRect(0, 0, SIZE, SIZE)
    for (let i = 0; i < 14000; i++) { // the nap of the cloth
      c.fillStyle = r() > 0.5 ? 'rgba(255,255,255,.035)' : 'rgba(0,0,0,.06)'
      c.fillRect(r() * SIZE, r() * SIZE, 2, 2)
    }
    c.strokeStyle = 'rgba(255,255,255,.14)'; c.lineWidth = 3
    c.beginPath(); c.arc(SIZE / 2, SIZE / 2, 130, 0, 7); c.stroke()
    c.strokeStyle = 'rgba(255,255,255,.07)'; c.lineWidth = 2
    c.beginPath(); c.arc(SIZE / 2, SIZE / 2, 150, 0, 7); c.stroke()
  } else if (key === 'ocean') {
    const g = c.createLinearGradient(0, 0, SIZE * 0.5, SIZE)
    g.addColorStop(0, '#3a94d8'); g.addColorStop(0.5, '#1f5fa0'); g.addColorStop(1, '#0d2c52')
    c.fillStyle = g; c.fillRect(0, 0, SIZE, SIZE)
    c.lineWidth = 3 // light on the water
    for (let i = 0; i < 70; i++) {
      c.strokeStyle = `rgba(214,240,255,${0.03 + r() * 0.07})`
      const y = r() * SIZE
      c.beginPath(); c.moveTo(-20, y)
      for (let x = -20; x < SIZE + 20; x += 40) c.quadraticCurveTo(x + 20, y + (r() - 0.5) * 16, x + 40, y)
      c.stroke()
    }
  } else {
    // A page out of an exercise book.
    c.fillStyle = '#f5f0e3'; c.fillRect(0, 0, SIZE, SIZE)
    for (let i = 0; i < 5000; i++) { c.fillStyle = 'rgba(150,135,105,.05)'; c.fillRect(r() * SIZE, r() * SIZE, 2, 2) }
    c.strokeStyle = 'rgba(90,130,170,.3)'; c.lineWidth = 1.5
    for (let y = 60; y < SIZE; y += 54) { c.beginPath(); c.moveTo(0, y); c.lineTo(SIZE, y); c.stroke() }
    c.strokeStyle = 'rgba(210,110,110,.42)'; c.lineWidth = 2
    c.beginPath(); c.moveTo(96, 0); c.lineTo(96, SIZE); c.stroke() // the margin line
  }
  // Light in the middle, shade at the corners: the desk looks lit from above rather than printed.
  const v = c.createRadialGradient(SIZE / 2, SIZE * 0.44, SIZE * 0.2, SIZE / 2, SIZE / 2, SIZE * 0.8)
  v.addColorStop(0, 'rgba(255,248,230,.13)')
  v.addColorStop(0.55, 'rgba(0,0,0,.05)')
  v.addColorStop(1, 'rgba(0,0,0,.42)')
  c.fillStyle = v; c.fillRect(0, 0, SIZE, SIZE)
  // The drop-off at every edge, so it reads as a surface a kolom can roll off.
  for (const [x, y, w, h, gx, gy, gw, gh] of [[0, 0, SIZE, 34, 0, 0, 0, 34], [0, SIZE - 34, SIZE, 34, 0, SIZE, 0, -34], [0, 0, 34, SIZE, 0, 0, 34, 0], [SIZE - 34, 0, 34, SIZE, SIZE, 0, -34, 0]]) {
    const lg = c.createLinearGradient(gx, gy, gx + gw, gy + gh)
    lg.addColorStop(0, 'rgba(12,6,2,.62)')
    lg.addColorStop(0.5, 'rgba(12,6,2,.16)')
    lg.addColorStop(1, 'rgba(12,6,2,0)')
    c.fillStyle = lg; c.fillRect(x, y, w, h)
  }
  return cv
}

export function drawBoard(ctx, key) {
  let cv = deskCache.get(key)
  if (!cv) { cv = paintDesk(key); deskCache.set(key, cv) }
  ctx.drawImage(cv, 0, 0)
}

export function capsule(ctx, hl, r) {
  ctx.beginPath()
  ctx.moveTo(-hl, -r)
  ctx.lineTo(hl, -r)
  ctx.arc(hl, 0, r, -Math.PI / 2, Math.PI / 2)
  ctx.lineTo(-hl, r)
  ctx.arc(-hl, 0, r, Math.PI / 2, Math.PI * 1.5)
  ctx.closePath()
}

/** One kolom, with its name tag. `active` draws the whose-turn glow, `spawn` the coming-back flash. */
export function drawBody(ctx, b, t, { active = false, name = null, seat = 0, spawn = 0 } = {}) {
  const s = b.scale ?? 1
  ctx.save()
  ctx.translate(b.x, b.y)
  ctx.scale(s, s)
  ctx.globalAlpha = b.alpha ?? 1
  ctx.rotate(b.ang || 0)
  ctx.save()
  ctx.translate(4, 7)
  ctx.fillStyle = 'rgba(0,0,0,.3)'
  capsule(ctx, b.hl, b.r)
  ctx.fill()
  ctx.restore()
  // The owner's colour as a halo under the pen: two players holding the same model are still told apart.
  ctx.save()
  ctx.shadowColor = COLORS[seat % 4]
  ctx.shadowBlur = 22
  ctx.globalAlpha = (b.alpha ?? 1) * 0.9
  ctx.fillStyle = COLORS[seat % 4]
  capsule(ctx, b.hl, b.r * 0.9)
  ctx.fill()
  ctx.fill()
  ctx.restore()
  if (active) {
    // Two rings breathing out of phase, so whose turn it is reads at a glance.
    for (const [off, speed, alpha] of [[7, 150, 0.85], [15, 150, 0.35]]) {
      ctx.strokeStyle = '#ff9f1c'
      ctx.lineWidth = 4
      ctx.globalAlpha = (b.alpha ?? 1) * alpha * (0.4 + 0.45 * Math.sin(t / speed))
      capsule(ctx, b.hl + off, b.r + off)
      ctx.stroke()
    }
    ctx.globalAlpha = b.alpha ?? 1
  }
  if (spawn > 0) {
    ctx.strokeStyle = '#6ee7a8'
    ctx.lineWidth = 5
    ctx.globalAlpha = spawn
    capsule(ctx, b.hl + 10 + (1 - spawn) * 40, b.r + 10 + (1 - spawn) * 40)
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
    ctx.fillStyle = '#fff'
    ctx.fillText(name, b.x, y + 1)
    ctx.restore()
  }
}

/**
 * What a flick would do, worked out with the same numbers the server uses (sent in state.shot), so the preview
 * is the real shot and not a guess:
 *   speed  = power x shot.speed          the kolom's starting speed
 *   travel = speed / shot.damp           how far it coasts with nothing in the way
 *   spin   = the same cross product the server applies, divided by shot.spinDamp for the total turn
 */
export function predict(pen, aim, shot) {
  const vx = aim.sx - aim.x
  const vy = aim.sy - aim.y
  const len = Math.hypot(vx, vy)
  const power = Math.min(len / 220, 1)
  if (!shot || len < 1) return { power, len, ready: false }
  const ux = vx / len
  const uy = vy / len
  const speed = power * shot.speed
  const travel = speed / shot.damp
  const c = Math.cos(pen.ang)
  const s = Math.sin(pen.ang)
  // Where the finger sits on the kolom, in desk coordinates relative to its middle.
  const rx = aim.lx * c - aim.ly * s
  const ry = aim.lx * s + aim.ly * c
  const half = pen.hl + pen.r
  const maxSpin = shot.maxSpin ?? 10
  const spin = Math.max(-maxSpin, Math.min(maxSpin, (speed * (rx * uy - ry * ux) * 3) / (half * half)))
  return {
    ready: true,
    power,
    len,
    ux,
    uy,
    speed,
    travel,
    spin,
    turn: spin / (shot.spinDamp ?? 3.5),
    grab: { x: pen.x + rx, y: pen.y + ry },
    stop: { x: pen.x + ux * travel, y: pen.y + uy * travel },
  }
}

/** Where a straight path from (x, y) along (ux, uy) leaves the desk, and by which edge. */
function edgeCrossing(x, y, ux, uy) {
  let best = { side: 'bottom', x, y, d: Infinity }
  const put = (side, d) => {
    if (d > 0 && d < best.d) best = { side, d, x: x + ux * d, y: y + uy * d }
  }
  if (ux > 0.0001) put('right', (SIZE - x) / ux)
  if (ux < -0.0001) put('left', -x / ux)
  if (uy > 0.0001) put('bottom', (SIZE - y) / uy)
  if (uy < -0.0001) put('top', -y / uy)
  return best
}

const POWER_COLOR = (p) => (p > 0.8 ? '#ff4d3d' : p > 0.45 ? '#ffb020' : '#35c26b')

/** The aim shown while a finger is down: rubber band, power ring, predicted path and where it stops. */
export function drawAim(ctx, pen, aim, shot, t) {
  const p = predict(pen, aim, shot)
  const danger = p.ready && (p.stop.x < 0 || p.stop.x > SIZE || p.stop.y < 0 || p.stop.y > SIZE)
  // Two different questions, so two different colours: the ring says how hard, the path says where it ends.
  // A hard but safe shot would otherwise be the same red as one that walks your own kolom off the desk.
  const col = danger ? '#ff4d3d' : POWER_COLOR(p.power)
  const path = danger ? '#ff4d3d' : '#6ee7a8'

  if (p.ready) {
    // Dots at equal slices of time: they bunch up towards the end, which is the kolom slowing down.
    ctx.save()
    for (let i = 1; i <= 14; i++) {
      const f = 1 - Math.exp(-3 * (i / 14)) // position at that moment, as a share of the whole travel
      const x = pen.x + p.ux * p.travel * f
      const y = pen.y + p.uy * p.travel * f
      ctx.globalAlpha = 0.65 * (1 - i / 16)
      ctx.fillStyle = path
      ctx.beginPath(); ctx.arc(x, y, 7 - i * 0.3, 0, 7); ctx.fill()
    }
    ctx.restore()

    // Where it comes to rest: the kolom's own outline, turned by the spin this flick gives it.
    ctx.save()
    ctx.translate(p.stop.x, p.stop.y)
    ctx.rotate(pen.ang + p.turn)
    ctx.globalAlpha = 0.5
    ctx.setLineDash([10, 7])
    ctx.lineWidth = 3
    ctx.strokeStyle = path
    capsule(ctx, pen.hl, pen.r)
    ctx.stroke()
    ctx.setLineDash([])
    ctx.restore()

    if (danger) {
      // Light up the edge it would leave by, and mark the spot where it goes over.
      const exit = edgeCrossing(pen.x, pen.y, p.ux, p.uy)
      ctx.save()
      ctx.globalAlpha = 0.5 + 0.3 * Math.sin(t / 110)
      const band = 70
      const [bx, by, bw, bh, g0, g1] = {
        bottom: [0, SIZE - band, SIZE, band, [0, SIZE], [0, SIZE - band]],
        top: [0, 0, SIZE, band, [0, 0], [0, band]],
        left: [0, 0, band, SIZE, [0, 0], [band, 0]],
        right: [SIZE - band, 0, band, SIZE, [SIZE, 0], [SIZE - band, 0]],
      }[exit.side]
      const lg = ctx.createLinearGradient(g0[0], g0[1], g1[0], g1[1])
      lg.addColorStop(0, 'rgba(255,60,45,.85)')
      lg.addColorStop(1, 'rgba(255,60,45,0)')
      ctx.fillStyle = lg
      ctx.fillRect(bx, by, bw, bh)
      ctx.globalAlpha = 0.95
      ctx.fillStyle = '#ff4d3d'
      ctx.strokeStyle = '#fff'
      ctx.lineWidth = 3
      ctx.beginPath(); ctx.arc(exit.x, exit.y, 17, 0, 7); ctx.fill(); ctx.stroke()
      ctx.fillStyle = '#fff'
      ctx.font = 'bold 22px system-ui'
      ctx.textAlign = 'center'
      ctx.textBaseline = 'middle'
      ctx.fillText('!', exit.x, exit.y + 1)
      ctx.restore()
    }

    // A curved arrow when the finger is off the middle, so the spin is visible before letting go.
    if (Math.abs(p.turn) > 0.35) {
      const way = Math.sign(p.turn)
      ctx.save()
      ctx.translate(p.grab.x, p.grab.y)
      ctx.strokeStyle = '#fff'
      ctx.lineWidth = 4
      ctx.globalAlpha = 0.9
      ctx.beginPath()
      ctx.arc(0, 0, 26, way > 0 ? -0.6 : 0.6, way > 0 ? 2.1 : -2.1, way < 0)
      ctx.stroke()
      const end = way > 0 ? 2.1 : -2.1
      ctx.translate(Math.cos(end) * 26, Math.sin(end) * 26)
      ctx.rotate(end + (way > 0 ? Math.PI / 2 : -Math.PI / 2))
      ctx.fillStyle = '#fff'
      ctx.beginPath(); ctx.moveTo(0, -9); ctx.lineTo(8, 5); ctx.lineTo(-8, 5); ctx.closePath(); ctx.fill()
      ctx.restore()
    }
  }

  // The rubber band from the kolom back to the finger.
  const g = p.grab || { x: pen.x, y: pen.y }
  ctx.save()
  ctx.strokeStyle = 'rgba(30,16,4,.45)'
  ctx.lineWidth = 5
  ctx.lineCap = 'round'
  ctx.beginPath(); ctx.moveTo(g.x, g.y); ctx.lineTo(aim.x, aim.y); ctx.stroke()
  ctx.fillStyle = 'rgba(30,16,4,.5)'
  ctx.beginPath(); ctx.arc(aim.x, aim.y, 7, 0, 7); ctx.fill()
  ctx.restore()

  // The power ring around the finger's grip on the kolom.
  ctx.save()
  ctx.translate(g.x, g.y)
  ctx.lineWidth = 7
  ctx.lineCap = 'round'
  ctx.strokeStyle = 'rgba(255,255,255,.35)'
  ctx.beginPath(); ctx.arc(0, 0, 20, 0, 7); ctx.stroke()
  ctx.strokeStyle = col
  ctx.beginPath(); ctx.arc(0, 0, 20, -Math.PI / 2, -Math.PI / 2 + Math.PI * 2 * p.power); ctx.stroke()
  ctx.fillStyle = col
  ctx.beginPath(); ctx.arc(0, 0, 9, 0, 7); ctx.fill()
  ctx.strokeStyle = '#fff'
  ctx.lineWidth = 3
  ctx.stroke()
  const label = `${Math.round(p.power * 100)}%`
  ctx.font = 'bold 16px system-ui'
  ctx.textAlign = 'center'
  ctx.textBaseline = 'middle'
  const lw = ctx.measureText(label).width + 20
  ctx.fillStyle = 'rgba(16,8,2,.82)'
  ctx.beginPath(); ctx.roundRect(-lw / 2, -58, lw, 26, 13); ctx.fill()
  ctx.fillStyle = col
  ctx.fillText(label, 0, -44)
  ctx.restore()
  return p
}

/** Before a finger goes down on your turn: a soft halo plus "press and pull" so a first-time player is not stuck. */
export function drawTurnHint(ctx, pen, t) {
  const pulse = (t % 2200) / 2200
  ctx.save()
  ctx.globalAlpha = 0.35 * (1 - pulse)
  ctx.strokeStyle = '#ff9f1c'
  ctx.lineWidth = 3
  capsule(ctx, pen.hl + 12 + pulse * 46, pen.r + 12 + pulse * 46)
  ctx.stroke()
  ctx.restore()
}
