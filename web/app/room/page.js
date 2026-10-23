'use client'
import { Suspense } from 'react'
import { useSearchParams } from 'next/navigation'
import Game from './Game'

function Inner() {
  const code = (useSearchParams().get('code') || '').toUpperCase()
  return code ? <Game code={code} /> : <div className="wrap">No room code. <a href="/">Back</a></div>
}

export default function RoomPage() {
  return <Suspense fallback={<div className="wrap muted">Loading...</div>}><Inner /></Suspense>
}
