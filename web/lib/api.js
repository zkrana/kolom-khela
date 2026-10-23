const BASE = process.env.NEXT_PUBLIC_API || ''
const KEY = 'kolom_token'

export const getToken = () => {
  try { return localStorage.getItem(KEY) || '' } catch { return '' }
}
export const setToken = (t) => {
  try { t ? localStorage.setItem(KEY, t) : localStorage.removeItem(KEY) } catch {}
}

export async function api(path, { method = 'GET', body } = {}) {
  const headers = { 'Content-Type': 'application/json' }
  const t = getToken()
  if (t) headers.Authorization = `Bearer ${t}`
  const res = await fetch(`${BASE}${path}`, { method, headers, body: body ? JSON.stringify(body) : undefined })
  const data = await res.json().catch(() => ({}))
  if (!res.ok) {
    const e = new Error(data.error || 'Something went wrong')
    e.status = res.status
    throw e
  }
  return data
}

export function wsUrl(code) {
  const base = BASE || window.location.origin
  return `${base.replace(/^http/, 'ws')}/ws/${encodeURIComponent(code)}?token=${encodeURIComponent(getToken())}`
}
