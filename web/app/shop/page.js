'use client'
import { useCallback, useEffect, useRef, useState } from 'react'
import { api } from '../../lib/api'
import { drawPen } from '../../lib/pens'
import { Nav } from '../../lib/ui'
import { useMe } from '../../lib/useMe'

function Preview({ id, hl, r }) {
  const ref = useRef(null)
  useEffect(() => {
    const cv = ref.current
    const ctx = cv.getContext('2d')
    ctx.setTransform(2, 0, 0, 2, 0, 0)
    const g = ctx.createLinearGradient(0, 0, 300, 64)
    g.addColorStop(0, '#d9a066'); g.addColorStop(1, '#b97f45')
    ctx.fillStyle = g
    ctx.fillRect(0, 0, 300, 64)
    ctx.translate(150, 32)
    ctx.rotate(-0.12)
    drawPen(ctx, id, hl, r)
  }, [id, hl, r])
  return <canvas ref={ref} width={600} height={128} style={{ width: '100%', height: 70, borderRadius: 12, display: 'block' }} aria-label={`${id} pen`} />
}

const Bar = ({ label, v, max }) => (
  <div style={{ marginBottom: 6 }}>
    <div className="muted" style={{ fontSize: 12 }}>{label}</div>
    <div style={{ background: 'rgba(10,6,24,.5)', borderRadius: 6, height: 8 }}><div style={{ width: `${Math.min(100, (v / max) * 100)}%`, background: 'linear-gradient(90deg, var(--gold), var(--gold2))', height: 8, borderRadius: 6 }} /></div>
  </div>
)

export default function Shop() {
  const { me, state, reload } = useMe()
  const [data, setData] = useState(null)
  const [error, setError] = useState('')
  const [busy, setBusy] = useState('')

  const load = useCallback(async () => {
    try { setData(await api('/api/shop')) } catch (e) { setError(e.message) }
    reload()
  }, [reload])
  useEffect(() => { if (state === 'in') load() }, [state]) // eslint-disable-line react-hooks/exhaustive-deps

  const act = async (path, id) => {
    setBusy(id); setError('')
    try { await api(path, { method: 'POST', body: { id } }); await load() } catch (e) { setError(e.message) } finally { setBusy('') }
  }

  if (!me || !data) return <div className="wrap muted">{error || 'Loading...'}</div>
  return (
    <div className="wrap">
      <Nav me={me} active="shop" />
      <h1>Pen shop</h1>
      <p className="muted">Win a game to earn {data.reward} coins. Heavier pens hit harder and are harder to knock off the desk. Longer pens reach further.</p>
      {error && <p className="err" role="alert">{error}</p>}
      <div className="cards" style={{ marginTop: 14 }}>
        {data.pens.map((p) => (
          <div key={p.id} className="card" style={data.selected === p.id ? { borderColor: 'var(--gold)' } : undefined}>
            <Preview id={p.id} hl={p.hl} r={p.r} />
            <div className="row between" style={{ marginTop: 10 }}>
              <h3 style={{ margin: 0 }}>{p.name}</h3>
              {p.owned && <span className="pill">owned</span>}
            </div>
            <p className="muted" style={{ margin: '4px 0 10px' }}>{p.desc}</p>
            <Bar label="Weight" v={p.m} max={4.5} />
            <Bar label="Reach" v={p.hl} max={80} />
            <div style={{ marginTop: 12 }}>
              {data.selected === p.id ? <button disabled style={{ width: '100%' }}>In use</button>
                : p.owned ? <button className="ghost" disabled={busy === p.id} onClick={() => act('/api/shop/select', p.id)} style={{ width: '100%' }}>Use this pen</button>
                : <button disabled={busy === p.id || data.coins < p.price} onClick={() => act('/api/shop/buy', p.id)} style={{ width: '100%' }}>{data.coins < p.price ? `${p.price} coins (need ${p.price - data.coins} more)` : `Buy for ${p.price} coins`}</button>}
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
