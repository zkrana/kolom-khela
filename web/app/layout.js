import './globals.css'

const SITE = process.env.NEXT_PUBLIC_SITE_URL || 'https://kolom-khela.onrender.com'
const TITLE = 'কলম খেলা: ক্লাসের বেঞ্চের সেই খেলা, এবার অনলাইনে'
const DESC = 'টোকা দিয়ে বন্ধুর কলম বেঞ্চ থেকে ফেলে দিন, কয়েন জিতুন আর ভালো কলম কিনুন। বন্ধুদের সাথে ফ্রি খেলুন।'

export const metadata = {
  metadataBase: new URL(SITE),
  title: 'কলম খেলা',
  description: DESC,
  openGraph: { title: TITLE, description: DESC, siteName: 'কলম খেলা', type: 'website', images: [{ url: '/og.png', width: 1200, height: 630, alt: 'বেঞ্চের ওপর কলম' }] },
  twitter: { card: 'summary_large_image', title: TITLE, description: DESC, images: ['/og.png'] },
}
export const viewport = { width: 'device-width', initialScale: 1, themeColor: '#2b1b12' }

export default function RootLayout({ children }) {
  return (
    <html lang="bn">
      <body>{children}</body>
    </html>
  )
}
