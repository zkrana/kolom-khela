'use client'
import { useCallback, useEffect, useState } from 'react'
import { api, getToken, setToken } from './api'

// Loads the signed-in player. With `required`, a visitor without a session is sent to the login page.
export function useMe({ required = true } = {}) {
  const [me, setMe] = useState(null)
  const [state, setState] = useState('loading') // loading | in | out
  const reload = useCallback(async () => {
    if (!getToken()) { setState('out'); return }
    try { setMe(await api('/api/me')); setState('in') } catch (e) {
      if (e.status === 401) setToken('')
      setState('out')
    }
  }, [])
  useEffect(() => { reload() }, [reload])
  useEffect(() => { if (required && state === 'out') window.location.href = '/' }, [required, state])
  return { me, state, reload }
}
