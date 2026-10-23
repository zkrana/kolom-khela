'use client'
import { useEffect, useRef, useState } from 'react'
import { Avatar } from '../../lib/ui'
import { PEN_NAMES } from '../../lib/pens'
import { sfx } from '../../lib/sound'
import { COLORS } from './draw'

export const LEVELS = [
  { key: 'easy', name: 'সহজ', icon: '🌱', lives: 3, coins: 5, blurb: 'ধীরে ছোটে, ৩টি প্রাণ, ৩৫ সেকেন্ড। নতুনদের জন্য।' },
  { key: 'medium', name: 'মাঝারি', icon: '⚔️', lives: 2, coins: 10, blurb: 'মাঝারি গতি, ২টি প্রাণ, ২৫ সেকেন্ড।' },
  { key: 'hard', name: 'কঠিন', icon: '🔥', lives: 1, coins: 20, blurb: 'জোরে ছোটে, ১টি প্রাণ, ১৮ সেকেন্ড। এক ভুলেই শেষ।' },
]

/** The host picks, everyone sees it change. */
export function DifficultyPicker({ value, canEdit, onPick }) {
  return (
    <div className="levels" role="radiogroup" aria-label="খেলার মাত্রা">
      {LEVELS.map((l) => {
        const on = value === l.key
        return (
          <button
            key={l.key}
            type="button"
            role="radio"
            aria-checked={on}
            disabled={!canEdit && !on}
            className={`level level--${l.key} ${on ? 'on' : ''}`}
            onClick={() => { if (canEdit && !on) { sfx.pick(); onPick(l.key) } }}
          >
            <span className="level__icon" aria-hidden="true">{l.icon}</span>
            <span className="level__name">{l.name}</span>
            <span className="level__meta">{'❤'.repeat(l.lives)} · {l.coins} কয়েন</span>
            {on && <span className="level__tick" aria-hidden="true">✓</span>}
          </button>
        )
      })}
    </div>
  )
}

/** Lives left, as filled and spent pips. */
export function Lives({ n, max, color }) {
  const total = Math.max(max || 0, n)
  if (!total) return null
  return (
    <span className="lives" title={`${n} প্রাণ বাকি`} aria-label={`${n} প্রাণ বাকি`}>
      {Array.from({ length: total }, (_, i) => (
        <i key={i} className={`life ${i < n ? 'life--on' : ''}`} style={i < n ? { background: color } : undefined} />
      ))}
    </span>
  )
}

/** One row in the players panel: who, their pen, lives, and whose turn it is. */
export function PlayerCard({ p, state, isMe, left }) {
  const myTurn = state.phase === 'playing' && state.turn === p.seat && p.alive
  const share = myTurn && left != null && state.turnSecs ? Math.max(0, Math.min(1, left / state.turnSecs)) : 0
  return (
    <div className={`player ${myTurn ? 'turn' : ''} ${!p.alive ? 'out' : ''}`}>
      <Avatar id={p.id} name={p.name} at={p.avatar} size={34} ring={COLORS[p.seat]} />
      <span className="player__who">
        <b className="player__name">{p.name}{isMe ? ' (আপনি)' : ''}{state.host === p.id ? ' ★' : ''}</b>
        <span className="player__sub">
          {PEN_NAMES[p.pen] || p.pen}
          {p.knocks > 0 && <span className="pill pill--knock" title="ফেলে দিয়েছে">⚡ {p.knocks}</span>}
        </span>
      </span>
      {!p.online && <span className="pill">নেই</span>}
      {p.alive ? <Lives n={p.lives} max={state.maxLives} color={COLORS[p.seat]} /> : <span className="pill pill--out">আউট</span>}
      {myTurn && left != null && (
        <span className={`timer ${left <= 5 ? 'timer--low' : ''}`} style={{ '--share': share }}>
          <span>{left}</span>
        </span>
      )}
    </div>
  )
}

/** A short burst of paper bits over the whole screen. Stops by itself; nothing to clean up but the frame. */
function Confetti({ run }) {
  const ref = useRef(null)
  useEffect(() => {
    if (!run) return
    const cv = ref.current
    if (!cv) return
    const ctx = cv.getContext('2d')
    const dpr = Math.min(window.devicePixelRatio || 1, 2)
    const resize = () => { cv.width = window.innerWidth * dpr; cv.height = window.innerHeight * dpr; ctx.setTransform(dpr, 0, 0, dpr, 0, 0) }
    resize()
    window.addEventListener('resize', resize)
    const W = () => cv.width / dpr
    const H = () => cv.height / dpr
    const colors = ['#ffc23d', '#ff9a3d', '#8b6cff', '#4fd18b', '#ff6b61', '#ffffff']
    const bits = Array.from({ length: 130 }, () => ({
      x: W() / 2 + (Math.random() - 0.5) * W() * 0.7,
      y: H() * 0.42 + (Math.random() - 0.5) * 80,
      vx: (Math.random() - 0.5) * 9,
      vy: -5 - Math.random() * 9,
      w: 6 + Math.random() * 7,
      h: 9 + Math.random() * 9,
      a: Math.random() * 7,
      va: (Math.random() - 0.5) * 0.3,
      c: colors[(Math.random() * colors.length) | 0],
    }))
    let raf
    let life = 0
    const loop = () => {
      life += 1
      ctx.clearRect(0, 0, W(), H())
      for (const b of bits) {
        b.vy += 0.26
        b.vx *= 0.995
        b.x += b.vx
        b.y += b.vy
        b.a += b.va
        ctx.save()
        ctx.translate(b.x, b.y)
        ctx.rotate(b.a)
        ctx.globalAlpha = Math.max(0, 1 - life / 230)
        ctx.fillStyle = b.c
        ctx.fillRect(-b.w / 2, -b.h / 2, b.w, b.h * (0.4 + 0.6 * Math.abs(Math.cos(b.a))))
        ctx.restore()
      }
      if (life < 230) raf = requestAnimationFrame(loop)
    }
    const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches
    if (!reduced) raf = requestAnimationFrame(loop)
    return () => { cancelAnimationFrame(raf); window.removeEventListener('resize', resize) }
  }, [run])
  if (!run) return null
  return <canvas ref={ref} className="confetti" aria-hidden="true" />
}

/** Counts up to `to`, one coin sound per step, and leaves the number on screen. */
function CoinCounter({ to, delay = 0 }) {
  const [n, setN] = useState(0)
  useEffect(() => {
    if (!to) return
    let raf
    let step = 0
    const start = performance.now() + delay
    const per = Math.max(60, Math.min(170, 1100 / to))
    const tick = (t) => {
      if (t >= start + step * per && step < to) {
        step += 1
        setN(step)
        sfx.coin(Math.min(step, 10))
      }
      if (step < to) raf = requestAnimationFrame(tick)
    }
    raf = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(raf)
  }, [to, delay])
  if (!to) return null
  return (
    <b className={`coinwin ${n >= to ? 'coinwin--done' : ''}`}>
      <span className="coin" aria-hidden="true" />+{n}
    </b>
  )
}

/** The end of a match: places revealed from last to first, then the winner's coins land one by one. */
export function EndOverlay({ state, me, isHost, onRematch }) {
  const results = state.results || []
  const mine = results.find((r) => r.id === me?.id)
  const iWon = mine?.rank === 1
  const rows = results // winner first, as a results list should read
  const revealMs = 420
  const total = rows.length * revealMs + 500
  const [done, setDone] = useState(false)
  useEffect(() => {
    const id = setTimeout(() => setDone(true), total)
    return () => clearTimeout(id)
  }, [total])

  return (
    <div className="overlay" role="dialog" aria-modal="true" aria-label="খেলা শেষ">
      <Confetti run={iWon} />
      <div className="card endcard">
        <div className={`endhead ${iWon ? 'endhead--win' : ''}`}>
          <span className="endhead__icon" aria-hidden="true">{iWon ? '🏆' : mine?.looser ? '💔' : '🎯'}</span>
          <h2>{iWon ? 'আপনি জিতেছেন!' : mine?.looser ? 'আপনিই লুজার' : 'খেলা শেষ'}</h2>
          <p className="muted">{LEVELS.find((l) => l.key === state.difficulty)?.name || ''} মাত্রা · {results.length} জন</p>
        </div>

        <ol className="results">
          {rows.map((r, i) => (
            <li
              key={r.id}
              className={`result ${r.rank === 1 ? 'result--win' : ''} ${r.looser ? 'result--loser' : ''} ${me && r.id === me.id ? 'result--me' : ''}`}
              /* Places appear from the bottom up, so the winner's row is the last to land. */
              style={{ animationDelay: `${(rows.length - 1 - i) * revealMs}ms` }}
            >
              <span className="result__rank">{r.rank === 1 ? '🥇' : r.rank === 2 ? '🥈' : r.rank === 3 ? '🥉' : r.rank}</span>
              <Avatar id={r.id} name={r.name} at={r.avatar} size={36} />
              <span className="result__who">
                <b>{r.name}{me && r.id === me.id ? ' (আপনি)' : ''}</b>
                <span className="result__stats">
                  {r.knocks > 0 && <span title="ফেলে দিয়েছে">⚡ {r.knocks}</span>}
                  {r.falls > 0 && <span title="নিজে পড়েছে">💥 {r.falls}</span>}
                  {r.lives > 0 && <span title="প্রাণ বাকি">❤ {r.lives}</span>}
                </span>
              </span>
              {r.looser && <span className="pill pill--out">লুজার</span>}
              {r.reward > 0 ? <CoinCounter to={r.reward} delay={total - revealMs} /> : null}
            </li>
          ))}
        </ol>

        {done && (
          <div className="endfoot">
            {mine && mine.rank !== 1 && (
              <p className="muted">
                {mine.looser ? 'সবার আগে আপনার কলম পড়েছে। আবার চেষ্টা করুন!' : 'জিততে পারলে কয়েন পেতেন। আরেকবার?'}
              </p>
            )}
            {iWon && <p className="muted">কয়েন দিয়ে <a href="/shop/">দোকান</a> থেকে ভারী কলম কিনুন।</p>}
            <div className="row" style={{ marginTop: 10 }}>
              {isHost ? (
                <button onClick={onRematch} style={{ flex: 1 }}>আবার খেলুন</button>
              ) : (
                <span className="muted" style={{ flex: 1 }}>হোস্ট আবার শুরু করতে পারেন।</span>
              )}
              <a className="btn ghost" href="/">লবি</a>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
