// Static export: the Rust server serves the built files from web/out, so the page loads as plain files
// (no Node server, no cold start) and everything can live on one free host.
export default {
  output: 'export',
  trailingSlash: true,
  images: { unoptimized: true },
  reactStrictMode: true,
}
