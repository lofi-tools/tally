// Verifies the built Astro site: dist/index.html must contain the showcase
// island (client-only, hydrated on load) plus the page's own head content and
// bundled assets. Because the island is `client:only`, the section markup is
// NOT in the static HTML — a render error surfaces at hydration, so verify
// those paths in a real browser.
//
// Usage: pnpm --filter @tally/design-system-showcase smoke  (builds first)
import { existsSync, readFileSync, readdirSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { join } from 'node:path'

const dist = fileURLToPath(new URL('../dist', import.meta.url))
const htmlPath = join(dist, 'index.html')
if (!existsSync(htmlPath)) {
  console.error('SMOKE FAIL: no dist/index.html — run via `pnpm smoke` (builds first)')
  process.exit(1)
}

const html = readFileSync(htmlPath, 'utf8')

// The island is client-only: `client="only"` + the renderer name in `opts`.
const islandCheck =
  html.includes('<astro-island') &&
  /client="only"/.test(html) &&
  /component-export="Showcase"/.test(html) &&
  /solid-js/.test(html)

// The Outfit font is bundled into a CSS asset (its @font-face rules live in
// the css, not the html) with woff2 files alongside.
const cssAssets = existsSync(join(dist, '_astro'))
  ? readdirSync(join(dist, '_astro')).filter((f) => f.endsWith('.css'))
  : []
const cssText = cssAssets.map((f) => readFileSync(join(dist, '_astro', f), 'utf8')).join('\n')
const fontCheck = cssText.includes('Outfit Variable') && cssAssets.length > 0
const woffCheck = existsSync(join(dist, '_astro'))
  ? readdirSync(join(dist, '_astro')).some((f) => f.endsWith('.woff2'))
  : false

const checks = [
  ['island markup', html.includes('<astro-island')],
  ['island is the client-only Solid showcase', islandCheck],
  ['page title', /<title>[\s\S]*Tally[\s\S]*<\/title>/.test(html)],
  ['color-mode pre-paint script', html.includes('tally-color-mode')],
  ['styles bundled', cssAssets.length > 0],
  ['Outfit font bundled', fontCheck && woffCheck],
]

const fails = checks.filter(([, ok]) => !ok)
for (const [name, ok] of checks) console.log(`${ok ? '  ✓' : '  ✗'} ${name}`)
if (fails.length) {
  console.error(`SMOKE FAIL: ${fails.length} check(s) failed`)
  process.exit(1)
}
console.log('SMOKE OK: showcase page built with a hydrated Solid island')
