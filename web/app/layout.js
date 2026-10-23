import './globals.css'

export const metadata = {
  title: 'Kolom Khela',
  description: 'The childhood pen-flicking game, live with friends. Knock the coins off the board, keep your kolom on it.',
}
export const viewport = { width: 'device-width', initialScale: 1, themeColor: '#2b1b12' }

export default function RootLayout({ children }) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  )
}
