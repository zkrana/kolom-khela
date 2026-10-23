'use client'
import { useState } from 'react'
import { api, setToken } from './api'

const TINTS = ['#6c8cff', '#ff6b61', '#4fd18b', '#b77cff', '#ffb03d', '#2fc4d8']

// A player's picture, or a coloured circle with their initial when they have none.
export function Avatar({ id, name = '?', at, size = 36, ring }) {
  const [broken, setBroken] = useState(false)
  const style = { width: size, height: size, fontSize: size * 0.44, ...(ring ? { boxShadow: `0 0 0 3px ${ring}` } : {}) }
  if (id && at && !broken) {
    // eslint-disable-next-line @next/next/no-img-element
    return <img className="avatar" style={style} src={`${process.env.NEXT_PUBLIC_API || ''}/api/avatar/${id}?v=${at}`} alt="" onError={() => setBroken(true)} />
  }
  const tint = TINTS[(id || name.charCodeAt(0) || 0) % TINTS.length]
  return <span className="avatar" style={{ ...style, background: tint }} aria-hidden="true">{String(name).trim().slice(0, 1).toUpperCase()}</span>
}

export function Logo({ size = 26 }) {
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" aria-hidden="true">
      <g transform="rotate(-35 16 16)">
        <rect x="3" y="12" width="26" height="8" rx="4" fill="#4f7cff" />
        <rect x="17" y="11" width="11" height="10" rx="4" fill="#1f3fb8" />
        <rect x="14" y="12.5" width="9" height="2.5" rx="1.2" fill="#dfe4ec" />
      </g>
    </svg>
  )
}

export function Nav({ me, active, onLogout }) {
  const link = (href, label, key) => <a key={key} href={href} className={active === key ? 'on' : ''}>{label}</a>
  return (
    <nav className="nav" aria-label="Main">
      <a className="brand" href="/"><Logo /> Kolom Khela</a>
      <div className="navlinks">
        {link('/', 'Play', 'play')}
        {link('/shop/', 'Pen shop', 'shop')}
        {link('/profile/', 'Profile', 'profile')}
        {me?.user?.role === 'admin' && link('/admin/', 'Admin', 'admin')}
      </div>
      {me && (
        <>
          <a className="coinchip" href="/shop/" title="Your coins"><span className="coin" />{me.user.coins}</a>
          <a href="/profile/" style={{ textDecoration: 'none' }} title={me.user.name}><Avatar id={me.user.id} name={me.user.name} at={me.user.avatarAt} size={36} /></a>
          <button className="ghost small" onClick={async () => { try { await api('/api/auth/logout', { method: 'POST' }) } catch {} setToken(''); onLogout ? onLogout() : (window.location.href = '/') }}>Log out</button>
        </>
      )}
    </nav>
  )
}
