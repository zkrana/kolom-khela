import './globals.css'

const SITE = process.env.NEXT_PUBLIC_SITE_URL || 'https://kolom-khela.onrender.com'
const TITLE = 'Kolom Khela: the school-bench pen game, online'
const DESC = "Flick your kolom, knock your friends' pens off the desk, win coins and collect better pens. Free to play with friends."

export const metadata = {
  metadataBase: new URL(SITE),
  title: 'Kolom Khela',
  description: DESC,
  openGraph: { title: TITLE, description: DESC, siteName: 'Kolom Khela', type: 'website', images: [{ url: '/og.png', width: 1200, height: 630, alt: 'Pens on a school bench' }] },
  twitter: { card: 'summary_large_image', title: TITLE, description: DESC, images: ['/og.png'] },
}
export const viewport = { width: 'device-width', initialScale: 1, themeColor: '#2b1b12' }

export default function RootLayout({ children }) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  )
}
