'use client'
import { useEffect, useState } from 'react'
import { api } from './api'

/** The server-wide live counter (/api/live), refreshed every few seconds while the tab is visible. */
export function useLive(every = 8000) {
  const [live, setLive] = useState(null)
  useEffect(() => {
    let dead = false
    const load = () => {
      if (document.hidden) return
      api('/api/live').then((d) => { if (!dead) setLive(d) }).catch(() => {})
    }
    load()
    const iv = setInterval(load, every)
    document.addEventListener('visibilitychange', load)
    return () => { dead = true; clearInterval(iv); document.removeEventListener('visibilitychange', load) }
  }, [every])
  return live
}

/**
 * "Who is playing right now", for the room's side panel and the home page. `room` (optional) adds this room's own
 * line: how many sit at the desk and how many are watching.
 */
export function LiveCard({ room, compact = false }) {
  const live = useLive()
  if (!live) return null
  return (
    <div className={`live ${compact ? 'live--compact' : ''}`} aria-live="polite">
      <div className="live__head">
        <span className="live__dot" aria-hidden="true" />
        <b>এখন লাইভ</b>
      </div>
      <div className="live__grid">
        <span className="live__stat"><b>{live.playing}</b>জন খেলছে</span>
        <span className="live__stat"><b>{live.online}</b>জন অনলাইনে</span>
        <span className="live__stat"><b>{live.guests}</b>জন গেস্ট</span>
        <span className="live__stat"><b>{live.matches}</b>টি খেলা চলছে</span>
      </div>
      <p className="live__foot">
        {live.rooms}টি রুম খোলা · ২৪ ঘণ্টায় {live.gamesDay}টি খেলা
        {room && <><br />এই রুমে {room.seated} জন খেলোয়াড়{room.bots ? ` (${room.bots}টি কম্পিউটার)` : ''}{room.watchers ? ` · ${room.watchers} জন দেখছে` : ''}</>}
      </p>
    </div>
  )
}
