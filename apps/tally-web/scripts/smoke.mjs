// Renders the built app bundle in jsdom to catch runtime errors that
// typecheck/build cannot (e.g. Panda recipe assertions, bad component usage).
// It exercises the initial render only — interactive paths (dialogs, selects)
// are best verified in a real browser.
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
  'HTMLHeadElement',
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

// jsdom lacks localStorage; the app (api.ts token, db.ts) reads it on boot.
// Fresh per run, so scripts start from the first-run onboarding state.
const lsStore = new Map()
const localStorageStub = {
  getItem: (k) => (lsStore.has(k) ? lsStore.get(k) : null),
  setItem: (k, v) => { lsStore.set(k, String(v)) },
  removeItem: (k) => { lsStore.delete(k) },
  clear: () => lsStore.clear(),
  key: (i) => [...lsStore.keys()][i] ?? null,
  get length() { return lsStore.size },
}
globalThis.localStorage = localStorageStub
try {
  window.localStorage = localStorageStub
} catch {
  // jsdom's window.localStorage is getter-only; replace it via defineProperty.
  Object.defineProperty(window, 'localStorage', { value: localStorageStub, writable: true, configurable: true })
}

const errors = []
window.addEventListener('error', (e) => errors.push(e.error ?? e))

const assetsDir = join(fileURLToPath(new URL('../dist/assets', import.meta.url)))
const bundle = readdirSync(assetsDir).find((f) => f.endsWith('.js') && f.startsWith('index-'))
if (!bundle) {
  console.error('SMOKE FAIL: no built bundle in dist/assets — run via `pnpm smoke` (builds first)')
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

// First-run onboarding state: fresh localStorage -> demo company selected,
// demo banner visible, demo dataset rendered.
const text = root.textContent
const checks = [
  ['demo company selected', text.includes('Demo Co Ltd')],
  ['demo banner', text.includes('Demo data') && text.includes('Add your company')],
  ['demo dataset (transactions)', text.includes('Shareholders initial stock purchase')],
  ['workspace nav', text.includes('Accounts') && text.includes('Filings') && text.includes('Payroll')],
]
const failed = checks.filter(([, ok]) => !ok)
if (failed.length > 0) {
  console.error('SMOKE FAIL — onboarding state missing:', failed.map(([name]) => name).join(', '))
  process.exit(1)
}
console.log(`SMOKE OK: app rendered (${children} root children) — onboarding state verified`)
