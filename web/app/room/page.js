'use client'
import { Suspense } from 'react'
import { useSearchParams } from 'next/navigation'
import Game from './Game'

function Inner() {
  const q = useSearchParams()
  const code = (q.get('code') || '').toUpperCase()
  return code ? <Game code={code} vsBot={q.get('bot') === '1'} /> : <div className="wrap">No room code. <a href="/">Back</a></div>
}

export default function RoomPage() {
  return <Suspense fallback={<div className="wrap muted">Loading...</div>}><Inner /></Suspense>
}
