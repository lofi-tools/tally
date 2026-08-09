// Renders the built app bundle in jsdom to catch runtime errors that
// typecheck/build cannot (e.g. Panda's recipe compound-variant assertion).
// It exercises the initial render only — interactive open-state code paths
// (dialog, menu, select popovers) are not covered.
//
// Usage: pnpm --filter @tally/web smoke   (builds first)
import { readdirSync } from 'node:fs'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { JSDOM } from 'jsdom'

const dom = new JSDOM('<!doctype html><html><body><div id="root"></div></body></html>', {
  url: 'http://localhost:5173/',
  pretendToBeVisual: true,
})

const { window } = dom
for (const key of [
  'window',
  'document',
  'navigator',
  'HTMLElement',
  'HTMLInputElement',
  'HTMLButtonElement',
  'Element',
  'Node',
  'Event',
  'CustomEvent',
  'MouseEvent',
  'KeyboardEvent',
  'getComputedStyle',
  'requestAnimationFrame',
  'cancelAnimationFrame',
  'MutationObserver',
  'customElements',
]) {
  try {
    globalThis[key] = window[key]
  } catch {
    // Some node globals (e.g. navigator) are getter-only; replace them.
    Object.defineProperty(globalThis, key, { value: window[key], writable: true, configurable: true })
  }
}

// jsdom lacks a few browser APIs the app uses; stub them (real browsers have these).
window.matchMedia = () => ({
  matches: false,
  addEventListener() {},
  removeEventListener() {},
  addListener() {},
  removeListener() {},
})
globalThis.matchMedia = window.matchMedia

for (const Ctor of ['ResizeObserver', 'IntersectionObserver']) {
  const stub = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
  window[Ctor] = stub
  globalThis[Ctor] = stub
}

// Capture both errors thrown while the bundle evaluates/renders and any
// window-level errors reported afterwards (e.g. from mount effects).
const errors = []
window.addEventListener('error', (e) => errors.push(e.error ?? e))

const assetsDir = join(fileURLToPath(new URL('../dist/assets', import.meta.url)))
const bundle = readdirSync(assetsDir).find((f) => f.endsWith('.js') && f.startsWith('index-'))
if (!bundle) {
  console.error('SMOKE FAIL: no built bundle in dist/assets — the `smoke` script builds first; run it via pnpm')
  process.exit(1)
}

try {
  await import(join(assetsDir, bundle))
} catch (err) {
  errors.push(err)
}

await new Promise((r) => setTimeout(r, 300))

const root = document.getElementById('root')
const children = root ? root.childNodes.length : 0

if (errors.length > 0) {
  console.error('SMOKE FAIL: app threw during render:\n', errors[0])
  process.exit(1)
}
if (!root || children === 0) {
  console.error('SMOKE FAIL: #root is empty — app rendered nothing')
  process.exit(1)
}
console.log(`SMOKE OK: app rendered (${children} root children)`)
