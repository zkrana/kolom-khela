'use client'
import { drawPen } from '../../lib/pens'

export const SIZE = 800
export const COLORS = ['#2f6df6', '#e53935', '#1faa59', '#8e44ad'] // one per seat: name tag, lives, lobby list

// Desk looks. Each is painted once into an offscreen canvas and then just copied every frame.
export const DESKS = {
  bench: { label: 'স্কুলের বেঞ্চ', swatch: 'linear-gradient(135deg,#e0a867,#b4773c)' },
  felt: { label: 'সবুজ কাপড়', swatch: 'linear-gradient(135deg,#2aa371,#146b47)' },
  ocean: { label: 'সাগর', swatch: 'linear-gradient(135deg,#3b9be0,#1c4f93)' },
  classic: { label: 'সাদামাটা', swatch: 'linear-gradient(135deg,#efebe0,#d8d2c2)' },
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
    for (let y = 0; y < SIZE; y += 100) {
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
  ctx.fillStyle = 'rgba(0,0,0,.22)'
  capsule(ctx, b.hl, b.r)
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

const POWER_COLOR = (p) => (p > 0.8 ? '#ff4d3d' : p > 0.45 ? '#ffb020' : '#35c26b')

/** The aim shown while a finger is down: rubber band, power ring, predicted path and where it stops. */
export function drawAim(ctx, pen, aim, shot, t) {
  const p = predict(pen, aim, shot)
  const danger = p.ready && (p.stop.x < 0 || p.stop.x > SIZE || p.stop.y < 0 || p.stop.y > SIZE)
  const col = danger ? '#ff4d3d' : POWER_COLOR(p.power)

  if (p.ready) {
    // Dots at equal slices of time: they bunch up towards the end, which is the kolom slowing down.
    ctx.save()
    for (let i = 1; i <= 14; i++) {
      const f = 1 - Math.exp(-3 * (i / 14)) // position at that moment, as a share of the whole travel
      const x = pen.x + p.ux * p.travel * f
      const y = pen.y + p.uy * p.travel * f
      ctx.globalAlpha = 0.65 * (1 - i / 16)
      ctx.fillStyle = col
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
    ctx.strokeStyle = col
    capsule(ctx, pen.hl, pen.r)
    ctx.stroke()
    ctx.setLineDash([])
    ctx.restore()

    if (danger) {
      ctx.save()
      ctx.globalAlpha = 0.55 + 0.35 * Math.sin(t / 110)
      ctx.strokeStyle = '#ff4d3d'
      ctx.lineWidth = 9
      ctx.strokeRect(5, 5, SIZE - 10, SIZE - 10) // the desk edge lights up: this shot goes off
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
  ctx.fillStyle = '#fff'
  ctx.font = 'bold 15px system-ui'
  ctx.textAlign = 'center'
  ctx.textBaseline = 'middle'
  ctx.strokeStyle = 'rgba(0,0,0,.55)'
  ctx.lineWidth = 4
  ctx.strokeText(`${Math.round(p.power * 100)}%`, 0, -36)
  ctx.fillText(`${Math.round(p.power * 100)}%`, 0, -36)
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
