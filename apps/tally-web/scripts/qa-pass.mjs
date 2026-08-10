// Headless interaction QA: renders the built app in jsdom and drives the
// interactive code paths the smoke test skips — dark/light toggle, dialog,
// menu, tabs and tooltips. Catches runtime errors in open/close state machines.
//
// Usage: pnpm --filter @tally/web qa   (builds first)
import { readdirSync, existsSync } from 'node:fs'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { JSDOM } from 'jsdom'

const dom = new JSDOM('<!doctype html><html><body><div id="root"></div></body></html>', {
  url: 'http://localhost:5173/',
  pretendToBeVisual: true,
})

const { window } = dom
for (const key of [
  'window', 'document', 'navigator', 'HTMLElement', 'HTMLHeadElement',
  'HTMLInputElement', 'HTMLButtonElement', 'Element', 'Node', 'Event',
  'CustomEvent', 'MouseEvent', 'KeyboardEvent', 'PointerEvent',
  'getComputedStyle', 'requestAnimationFrame', 'cancelAnimationFrame',
  'MutationObserver', 'customElements', 'getSelection',
]) {
  try {
    globalThis[key] = window[key]
  } catch {
    Object.defineProperty(globalThis, key, { value: window[key], writable: true, configurable: true })
  }
}

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

// jsdom lacks these; Ark's focus management calls them.
Element.prototype.scrollIntoView ??= () => {}

const errors = []
window.addEventListener('error', (e) => errors.push(e.error ?? e))
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
const tick = () => sleep(60)

function press(btn) {
  btn.dispatchEvent(new window.PointerEvent('pointerdown', { bubbles: true, pointerType: 'mouse' }))
  btn.click()
}
function escapeKey() {
  document.body.dispatchEvent(
    new window.KeyboardEvent('keydown', { key: 'Escape', bubbles: true }),
  )
}
function byText(root, text) {
  return [...root.querySelectorAll('button, a, [role="tab"]')].find((el) => el.textContent.trim().includes(text))
}

const assetsDir = join(fileURLToPath(new URL('../dist/assets', import.meta.url)))
const bundle = readdirSync(assetsDir).find((f) => f.endsWith('.js') && f.startsWith('index-'))
if (!bundle) {
  console.error('QA FAIL: no built bundle in dist/assets — run via `pnpm qa` (builds first)')
  process.exit(1)
}

try {
  await import(join(assetsDir, bundle))
} catch (err) {
  errors.push(err)
}
await sleep(400)

const root = document.getElementById('root')
if (!root || root.childNodes.length === 0) {
  console.error('QA FAIL: #root is empty')
  process.exit(1)
}

const results = []
const check = (name, ok, detail = '') => {
  results.push([ok, name, detail])
  console.log(`${ok ? '  ✓' : '  ✗'} ${name}${detail ? ` — ${detail}` : ''}`)
}

// ---- Sections present -----------------------------------------------------
const bodyText = document.body.textContent
for (const [id, label] of [
  ['tokens', 'Tokens'],
  ['components', 'Components'],
  ['forms', 'Forms'],
  ['overlays', 'Overlays and controls'],
]) {
  const sec = document.getElementById(id)
  check(`section #${id} renders`, !!sec && sec.textContent.trim().length > 0)
}
check('hero renders', bodyText.includes('Tally') && /CT600|FRS 105|iXBRL|Companies House/.test(bodyText))

// ---- Dark / light toggle --------------------------------------------------
// The switch root is a <label> whose activation depends on the hidden input
// (Zag listens only to its `change` event). jsdom doesn't forward label clicks,
// so drive the input directly — real browsers do this natively via the label.
const headerSwitch = [...document.querySelectorAll('[data-scope="switch"][data-part="root"]')].find(
  (l) => l.textContent.includes('Dark'),
)
const isDark = () => document.documentElement.classList.contains('dark')
const before = isDark()
const headerInput = headerSwitch?.querySelector('input[type="checkbox"]')
if (headerSwitch && headerInput) {
  headerInput.click()
  await sleep(150)
  const flipped = isDark() !== before
  headerInput.click()
  await sleep(150)
  check('dark toggle flips .dark on <html>', flipped && isDark() === before)
  // Re-flip to dark (the identity) so the rest of the pass runs in dark mode.
  if (!isDark()) {
    headerInput.click()
    await sleep(150)
  }
  check('dark toggle control present', true)
} else {
  check('dark toggle control present', false)
}

// ---- Dialog ---------------------------------------------------------------
const openDialog = byText(document.body, 'Open dialog')
if (openDialog) {
  press(openDialog)
  await tick()
  const dialog = document.body.querySelector('[data-scope="dialog"][data-part="content"]')
  check('dialog opens', !!dialog)
  check('dialog is modal (aria-modal)', dialog?.getAttribute('aria-modal') === 'true')
  check('dialog title present', document.body.textContent.includes('Submit your accounts'))
  if (dialog) {
    const cancel = byText(dialog, 'Cancel')
    if (cancel) {
      cancel.click()
      await tick()
      check('dialog closes via Cancel', !document.body.querySelector('[data-scope="dialog"][data-part="content"]'))
    } else {
      escapeKey()
      await tick()
      check('dialog closes via Escape', !document.body.querySelector('[data-scope="dialog"][data-part="content"]'))
    }
  }
} else {
  check('dialog trigger present', false)
}

// ---- Menu -----------------------------------------------------------------
const openMenu = byText(document.body, 'Actions')
if (openMenu) {
  press(openMenu)
  await tick()
  const menu = document.body.querySelector('[data-scope="menu"][data-part="content"]')
  check('menu opens', !!menu)
  check('menu has items', !!menu && menu.querySelectorAll('[role="menuitem"]').length >= 3)
  check('menu has disabled item', !!menu && !!menu.querySelector('[data-disabled]'))
  escapeKey()
  await tick()
  check('menu closes via Escape', !document.body.querySelector('[data-scope="menu"][data-part="content"]'))
} else {
  check('menu trigger present', false)
}

// ---- Tabs -----------------------------------------------------------------
// Ark emits no `data-value` on content parts; match by id suffix
// (`tabs:…:content-<value>`) and by the active marker (`data-selected`, no `hidden`).
const contentById = (value) => document.querySelector(`[data-scope="tabs"][data-part="content"][id$="content-${value}"]`)
const visibleContent = () =>
  [...document.querySelectorAll('[data-scope="tabs"][data-part="content"]')].find((c) => !c.hasAttribute('hidden'))
const tabsList = document.querySelector('[data-scope="tabs"][data-part="list"]')
if (tabsList) {
  const filing = [...tabsList.querySelectorAll('[data-part="trigger"]')].find((t) => t.textContent.trim() === 'Filing')
  check('tabs default to Accounts', /micro-entity accounts/.test(visibleContent()?.textContent ?? ''))
  if (filing) {
    filing.click()
    await sleep(150)
    const visible = visibleContent()
    const accountsEl = contentById('accounts')
    check(
      'tab switch shows Filing content',
      /CT600/.test(visible?.textContent ?? '') && accountsEl?.hasAttribute('hidden'),
      `visible=${visible?.textContent?.trim().slice(0, 40) ?? '(none)'} accountsHidden=${accountsEl?.hasAttribute('hidden') ?? '(none)'}`,
    )
    check('active tab is aria-selected', filing.getAttribute('aria-selected') === 'true')
  }
} else {
  check('tabs present', false)
}

// ---- Tooltip --------------------------------------------------------------
const tooltipTrigger = byText(document.body, 'Hover me')
if (tooltipTrigger) {
  tooltipTrigger.dispatchEvent(
    new window.PointerEvent('pointerover', { bubbles: true, pointerType: 'mouse' }),
  )
  tooltipTrigger.focus()
  await sleep(150)
  const tooltip = document.body.querySelector('[data-scope="tooltip"][data-part="content"]')
  check('tooltip opens on hover/focus', !!tooltip)
  check('tooltip content correct', !!tooltip && /HMRC/.test(tooltip.textContent))
} else {
  check('tooltip trigger present', false)
}

// ---- Footer ---------------------------------------------------------------
check('footer renders', bodyText.includes('Tally — UK company accounts'))

// ---- Aggregate ------------------------------------------------------------
const fails = results.filter(([ok]) => !ok)
if (errors.length) {
  console.error('\nQA FAIL: runtime errors during pass:\n', errors.map((e) => (e?.stack ?? e).split('\n').slice(0, 3).join('\n')).join('\n---\n'))
  process.exit(1)
}
if (fails.length) {
  console.error(`\nQA FAIL: ${fails.length} check(s) failed`)
  process.exit(1)
}
console.log(`\nQA OK: all ${results.length} interaction checks passed`)
