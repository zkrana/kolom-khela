// Tiny synthesized sound effects (no files to download). Starts only after the first tap, as browsers require.
let ctx = null
let muted = false
try { muted = localStorage.getItem('kolom_mute') === '1' } catch {}

export const isMuted = () => muted
export function setMuted(v) {
  muted = v
  try { localStorage.setItem('kolom_mute', v ? '1' : '0') } catch {}
}

function ac() {
  if (muted) return null
  try {
    if (!ctx) ctx = new (window.AudioContext || window.webkitAudioContext)()
    if (ctx.state === 'suspended') ctx.resume()
    return ctx
  } catch { return null }
}

function tone(freq, to, dur, type = 'sine', vol = 0.2, delay = 0) {
  const a = ac()
  if (!a) return
  const t = a.currentTime + delay
  const o = a.createOscillator()
  const g = a.createGain()
  o.type = type
  o.frequency.setValueAtTime(freq, t)
  o.frequency.exponentialRampToValueAtTime(Math.max(30, to), t + dur)
  g.gain.setValueAtTime(vol, t)
  g.gain.exponentialRampToValueAtTime(0.0001, t + dur)
  o.connect(g).connect(a.destination)
  o.start(t)
  o.stop(t + dur + 0.02)
}

function noise(dur, vol = 0.15, cutoff = 2500) {
  const a = ac()
  if (!a) return
  const n = Math.floor(a.sampleRate * dur)
  const buf = a.createBuffer(1, n, a.sampleRate)
  const d = buf.getChannelData(0)
  for (let i = 0; i < n; i++) d[i] = (Math.random() * 2 - 1) * (1 - i / n)
  const src = a.createBufferSource()
  src.buffer = buf
  const f = a.createBiquadFilter()
  f.type = 'highpass'
  f.frequency.value = cutoff
  const g = a.createGain()
  g.gain.value = vol
  src.connect(f).connect(g).connect(a.destination)
  src.start()
}

export const sfx = {
  flick: (p = 0.5) => noise(0.12, 0.1 + p * 0.2, 1800),
  hit: (s = 0.5) => { tone(220, 70, 0.11, 'triangle', 0.12 + s * 0.35); noise(0.05, 0.05 + s * 0.12, 3500) },
  fall: () => tone(520, 90, 0.5, 'sawtooth', 0.12),
  win: () => [523, 659, 784, 1047].forEach((f, i) => tone(f, f, 0.22, 'triangle', 0.22, i * 0.12)),
  lose: () => [392, 330, 262].forEach((f, i) => tone(f, f * 0.97, 0.3, 'sine', 0.2, i * 0.16)),
  turn: () => tone(740, 740, 0.12, 'sine', 0.12),
}
