// How each pen looks. Drawn along the x axis: the cap end is at -(hl+r), the writing tip at +(hl+r).
// Positions are fractions (0..1) of the full length from the cap end.

export const LOOKS = {
  // Every pen is drawn with its cap on, so it reads as a pen and not a pencil.
  econo: { barrel: '#cfe3ff', glass: true, ink: '#1f4fd1', cap: '#1f4fd1', capLen: 0.34, back: '#1f4fd1', backLen: 0.07, clip: '#cfd4db', rings: [{ at: 0.575, w: 0.02, color: '#e9eef9' }] },
  matador: { barrel: '#f3f3ef', cap: '#c62828', capLen: 0.34, back: '#c62828', backLen: 0.07, clip: '#d9dde3', rings: [{ at: 0.2, w: 0.02, color: '#c62828' }, { at: 0.575, w: 0.02, color: '#f3f3ef' }] },
  gel: { barrel: '#d9f4e6', glass: true, ink: '#1c9a5e', cap: '#1c9a5e', capLen: 0.32, back: '#157a49', backLen: 0.08, grip: { from: 0.4, to: 0.6, color: '#157a49', dots: true }, clip: '#e5e9ee', rings: [] },
  marker: { barrel: '#e9ff3d', cap: '#2b2b2b', capLen: 0.38, back: '#2b2b2b', backLen: 0.06, clip: '#9aa1ab', rings: [{ at: 0.3, w: 0.03, color: '#2f9e44' }, { at: 0.55, w: 0.025, color: '#2f9e44' }] },
  steel: { barrel: '#17171b', cap: '#17171b', capLen: 0.34, back: '#dfe4ec', backLen: 0.05, clip: '#d9a441', capRing: '#dfe4ec', rings: [{ at: 0.28, w: 0.025, color: '#dfe4ec' }] },
}

function hex(c) {
  const n = parseInt(c.slice(1), 16)
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255]
}
function shade(c, k) {
  const [r, g, b] = hex(c)
  const f = (v) => Math.max(0, Math.min(255, Math.round(k >= 0 ? v + (255 - v) * k : v * (1 + k))))
  return `rgb(${f(r)},${f(g)},${f(b)})`
}

// A cylinder look: dark edge, light body, soft highlight, dark edge.
function cyl(ctx, r, base) {
  const g = ctx.createLinearGradient(0, -r, 0, r)
  g.addColorStop(0, shade(base, -0.38))
  g.addColorStop(0.22, shade(base, 0.12))
  g.addColorStop(0.38, shade(base, 0.45))
  g.addColorStop(0.6, base)
  g.addColorStop(1, shade(base, -0.42))
  return g
}

export function drawPen(ctx, style, hl, r) {
  const look = LOOKS[style] || LOOKS.econo
  const L = (hl + r) * 2
  const x0 = -(hl + r)
  const at = (f) => x0 + L * f
  const rr = r * 0.95 // barrel
  const rc = rr * 1.14 // the cap is a little fatter than the barrel
  const capStart = at(1 - look.capLen)
  const rect = (x, w, rad, h) => { ctx.beginPath(); ctx.roundRect(x, -h, w, h * 2, rad); }

  // barrel (back plug included)
  rect(x0, capStart - x0 + 6, [rr * 0.7, 2, 2, rr * 0.7], rr)
  ctx.fillStyle = cyl(ctx, rr, look.barrel)
  ctx.fill()
  ctx.save()
  ctx.clip()
  if (look.glass && look.ink) {
    ctx.fillStyle = look.ink
    ctx.globalAlpha = 0.85
    ctx.fillRect(at(look.backLen), -rr * 0.2, capStart - at(look.backLen), rr * 0.4)
    ctx.globalAlpha = 1
  }
  if (look.grip) {
    ctx.fillStyle = cyl(ctx, rr, look.grip.color)
    ctx.fillRect(at(look.grip.from), -rr, L * (look.grip.to - look.grip.from), rr * 2)
    if (look.grip.dots) {
      ctx.fillStyle = 'rgba(0,0,0,.25)'
      for (let f = look.grip.from + 0.015; f < look.grip.to - 0.01; f += 0.03) {
        ctx.beginPath(); ctx.arc(at(f), -rr * 0.35, rr * 0.12, 0, 7); ctx.fill()
        ctx.beginPath(); ctx.arc(at(f + 0.015), rr * 0.35, rr * 0.12, 0, 7); ctx.fill()
      }
    }
  }
  ctx.fillStyle = cyl(ctx, rr, look.back)
  ctx.fillRect(x0 - 2, -rr, L * look.backLen + 2, rr * 2)
  for (const ring of look.rings || []) {
    ctx.fillStyle = cyl(ctx, rr, ring.color)
    ctx.fillRect(at(ring.at), -rr, L * ring.w, rr * 2)
  }
  ctx.restore()
  rect(x0, capStart - x0 + 6, [rr * 0.7, 2, 2, rr * 0.7], rr)
  ctx.strokeStyle = 'rgba(20,20,30,.55)'
  ctx.lineWidth = 1.5
  ctx.stroke()

  // the cap: slightly fatter, rounded at the writing end, with a clip along its side
  rect(capStart, L * look.capLen, [2, rc * 0.8, rc * 0.8, 2], rc)
  ctx.fillStyle = cyl(ctx, rc, look.cap)
  ctx.fill()
  ctx.strokeStyle = 'rgba(20,20,30,.6)'
  ctx.stroke()
  if (look.capRing) {
    ctx.fillStyle = cyl(ctx, rc, look.capRing)
    ctx.fillRect(capStart, -rc, L * 0.02, rc * 2)
    ctx.fillRect(at(1) - L * 0.045, -rc, L * 0.02, rc * 2)
  } else {
    ctx.fillStyle = 'rgba(255,255,255,.28)'
    ctx.fillRect(capStart, -rc, L * 0.018, rc * 2)
  }
  // clip: starts on the cap and runs back over the barrel
  const cx0 = capStart - L * 0.07
  const cx1 = at(1) - L * 0.06
  ctx.fillStyle = cyl(ctx, rc * 0.35, look.clip)
  ctx.beginPath(); ctx.roundRect(cx0, -rc * 0.78, cx1 - cx0, rc * 0.34, rc * 0.17); ctx.fill()
  ctx.strokeStyle = 'rgba(20,20,30,.5)'
  ctx.lineWidth = 1
  ctx.stroke()
  ctx.fillStyle = look.clip
  ctx.beginPath(); ctx.arc(cx0 + rc * 0.17, -rc * 0.61, rc * 0.2, 0, 7); ctx.fill()
}
