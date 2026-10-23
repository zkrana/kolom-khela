'use client'
import { useRef, useState } from 'react'
import { api, getToken } from '../../lib/api'
import { Avatar, Nav } from '../../lib/ui'
import { useMe } from '../../lib/useMe'

const BASE = process.env.NEXT_PUBLIC_API || ''

// Crop to a square and shrink to 192px in the browser, so uploads stay tiny whatever the phone camera produced.
async function toSquareJpeg(file) {
  const bmp = await createImageBitmap(file)
  const side = Math.min(bmp.width, bmp.height)
  const cv = document.createElement('canvas')
  cv.width = cv.height = 192
  cv.getContext('2d').drawImage(bmp, (bmp.width - side) / 2, (bmp.height - side) / 2, side, side, 0, 0, 192, 192)
  return new Promise((res, rej) => cv.toBlob((b) => (b ? res(b) : rej(new Error('Could not read that picture'))), 'image/jpeg', 0.88))
}

export default function Profile() {
  const { me, reload } = useMe()
  const file = useRef(null)
  const [name, setName] = useState('')
  const [pw, setPw] = useState({ current: '', next: '' })
  const [msg, setMsg] = useState({ kind: '', text: '' })
  const [busy, setBusy] = useState(false)
  const [save, setSave] = useState({ email: '', password: '' })
  const flash = (kind, text) => setMsg({ kind, text })

  if (!me) return <div className="wrap muted">Loading...</div>
  const u = me.user
  const s = me.stats

  const upload = async (e) => {
    const f = e.target.files?.[0]
    e.target.value = ''
    if (!f) return
    setBusy(true); flash('', '')
    try {
      const blob = await toSquareJpeg(f)
      const res = await fetch(`${BASE}/api/me/avatar`, { method: 'POST', headers: { Authorization: `Bearer ${getToken()}`, 'Content-Type': 'image/jpeg' }, body: blob })
      const d = await res.json().catch(() => ({}))
      if (!res.ok) throw new Error(d.error || 'Upload failed')
      await reload(); flash('ok', 'Picture updated')
    } catch (err) { flash('err', err.message) } finally { setBusy(false) }
  }
  const removePic = async () => {
    setBusy(true)
    try { await api('/api/me/avatar', { method: 'DELETE' }); await reload(); flash('ok', 'Picture removed') } catch (err) { flash('err', err.message) } finally { setBusy(false) }
  }
  const saveName = async (e) => {
    e.preventDefault()
    try { await api('/api/me', { method: 'PATCH', body: { name } }); setName(''); await reload(); flash('ok', 'Name saved') } catch (err) { flash('err', err.message) }
  }
  const savePw = async (e) => {
    e.preventDefault()
    try { await api('/api/me/password', { method: 'POST', body: { current: pw.current, new: pw.next } }); setPw({ current: '', next: '' }); flash('ok', 'Password changed') } catch (err) { flash('err', err.message) }
  }

  const saveAccount = async (e) => {
    e.preventDefault()
    try { await api('/api/me/upgrade', { method: 'POST', body: save }); setSave({ email: '', password: '' }); await reload(); flash('ok', 'Account saved. You can log in with this email and password anywhere.') } catch (err) { flash('err', err.message) }
  }

  return (
    <div className="wrap">
      <Nav me={me} active="profile" />
      <div className="grid2">
        <div className="card hero">
          <div className="row" style={{ gap: 18 }}>
            <Avatar id={u.id} name={u.name} at={u.avatarAt} size={96} />
            <div>
              <h2 style={{ margin: 0 }}>{u.name}</h2>
              <div className="muted">{u.guest ? 'Guest player' : u.email}{u.role === 'admin' ? ' · admin' : ''}</div>
              <div className="row" style={{ marginTop: 12 }}>
                <input ref={file} type="file" accept="image/jpeg,image/png,image/webp" onChange={upload} hidden />
                <button className="small" onClick={() => file.current.click()} disabled={busy}>{u.avatarAt ? 'Change picture' : 'Add a picture'}</button>
                {u.avatarAt && <button className="ghost small" onClick={removePic} disabled={busy}>Remove</button>}
              </div>
              <p className="muted" style={{ margin: '8px 0 0' }}>JPG, PNG or WebP. It is cropped to a square.</p>
            </div>
          </div>
          <div className="stats">
            <div className="stat"><b>{s.played}</b>played</div>
            <div className="stat"><b>{s.wins}</b>won</div>
            <div className="stat"><b>{s.loser}</b>looser</div>
            <div className="stat"><b>{u.coins}</b>coins</div>
          </div>
          {msg.text && <p className={msg.kind === 'ok' ? 'ok' : 'err'} role="status">{msg.text}</p>}
        </div>
        <div className="card">
          {u.guest && (
            <div style={{ marginBottom: 22 }}>
              <h3>Save your account</h3>
              <p className="muted">You joined from an invite link, so this account has no password yet. Save it to keep your coins and pens and to log in from any device.</p>
              <form onSubmit={saveAccount}>
                <label htmlFor="se">Email</label>
                <input id="se" type="email" required autoComplete="email" value={save.email} onChange={(e) => setSave({ ...save, email: e.target.value })} />
                <label htmlFor="sp">Choose a password</label>
                <input id="sp" type="password" required minLength={6} autoComplete="new-password" value={save.password} onChange={(e) => setSave({ ...save, password: e.target.value })} />
                <div style={{ marginTop: 14 }}><button>Save my account</button></div>
              </form>
            </div>
          )}
          <h3>Display name</h3>
          <form onSubmit={saveName} className="row" style={{ flexWrap: 'nowrap' }}>
            <input value={name} onChange={(e) => setName(e.target.value)} placeholder={u.name} minLength={2} maxLength={24} aria-label="New name" />
            <button disabled={name.trim().length < 2}>Save</button>
          </form>
          {!u.guest && (
            <>
              <h3 style={{ marginTop: 22 }}>Change password</h3>
              <form onSubmit={savePw}>
                <label htmlFor="cur">Current password</label>
                <input id="cur" type="password" autoComplete="current-password" required value={pw.current} onChange={(e) => setPw({ ...pw, current: e.target.value })} />
                <label htmlFor="new">New password</label>
                <input id="new" type="password" autoComplete="new-password" required minLength={6} value={pw.next} onChange={(e) => setPw({ ...pw, next: e.target.value })} />
                <div style={{ marginTop: 14 }}><button>Change password</button></div>
              </form>
            </>
          )}
        </div>
      </div>
    </div>
  )
}
