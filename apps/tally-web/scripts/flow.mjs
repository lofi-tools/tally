// End-to-end onboarding test (`pnpm flow`) — mounts the built bundle in jsdom
// and drives the flows: add company via search -> banner hides -> connect bank
// -> sample retires -> save account.
import { readdirSync } from 'node:fs'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { JSDOM } from 'jsdom'

const dom = new JSDOM('<!doctype html><html class="dark"><body><div id="root"></div></body></html>', {
  url: 'http://localhost:5173/',
  pretendToBeVisual: true,
})
const { window } = dom
for (const key of [
  'window', 'document', 'navigator', 'HTMLElement', 'HTMLHeadElement', 'HTMLInputElement',
  'HTMLButtonElement', 'Element', 'Node', 'Event', 'CustomEvent', 'MouseEvent', 'KeyboardEvent',
  'getComputedStyle', 'requestAnimationFrame', 'cancelAnimationFrame', 'MutationObserver', 'customElements',
]) {
  try { globalThis[key] = window[key] } catch {
    Object.defineProperty(globalThis, key, { value: window[key], writable: true, configurable: true })
  }
}
window.matchMedia = () => ({ matches: false, addEventListener() {}, removeEventListener() {}, addListener() {}, removeListener() {} })
globalThis.matchMedia = window.matchMedia
for (const Ctor of ['ResizeObserver', 'IntersectionObserver']) {
  const stub = class { observe() {} unobserve() {} disconnect() {} }
  window[Ctor] = stub
  globalThis[Ctor] = stub
}
const errors = []
window.addEventListener('error', (e) => errors.push(e.error ?? e))

const assetsDir = join(fileURLToPath(new URL('../dist/assets', import.meta.url)))
const bundle = readdirSync(assetsDir).find((f) => f.endsWith('.js') && f.startsWith('index-'))
await import(join(assetsDir, bundle))
await new Promise((r) => setTimeout(r, 200))

const fail = (msg) => { console.error('INTERACT FAIL:', msg); process.exit(1) }
const text = () => document.getElementById('root').textContent
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
const findByText = (tag, t) => [...document.querySelectorAll(tag)].find((el) => el.textContent.trim().includes(t))
const click = (el) => {
  if (!el) {
    const btns = [...document.querySelectorAll('button')].map((b) => b.textContent.trim().slice(0, 30))
    console.error('CLICK MISSING — buttons on page:', JSON.stringify(btns))
    process.exit(1)
  }
  el.dispatchEvent(new window.MouseEvent('click', { bubbles: true, cancelable: true }))
}
const type = (el, value) => {
  el.value = value
  el.dispatchEvent(new window.Event('input', { bubbles: true }))
}

if (errors.length) fail('render threw: ' + errors[0])
if (!text().includes('Sample data')) fail('banner missing at start')

// 1. Open the add-company dialog from the banner
const addBtn = findByText('button', 'Add company')
if (!addBtn) fail('banner "Add company" button missing')
click(addBtn)
await sleep(50)
if (!text().includes('Search Companies House')) fail('search dialog did not open')

// 2. Search + pick a result
type(document.querySelector('input[placeholder="Company name or number"]'), 'Northwind')
click(findByText('button', 'Search'))
await sleep(50)
const result = findByText('button', 'Northwind Trading Ltd')
if (!result) fail('search results did not render')
click(result)
await sleep(50)
if (!text().includes('Confirm company details')) fail('review step did not open')

// 3. Fill UTR and submit (the dialog footer button is the LAST 'Add company')
const utrInput = document.querySelector('input[placeholder="10-digit tax reference"]')
if (!utrInput) fail('UTR input missing')
type(utrInput, '1234567890')
const submitBtn = [...document.querySelectorAll('button')].filter((b) => b.textContent.trim() === 'Add company').at(-1)
if (!submitBtn) fail('dialog submit button missing')
click(submitBtn)
await sleep(100)
if (errors.length) console.error('RUNTIME ERRORS after submit:', errors.map((e) => String(e && e.message ? e.message : e).slice(0, 200)).join(' | '))

// 4. Assertions after add
if (text().includes('Sample data')) fail('banner still visible after adding a company')
if (!text().includes('Northwind Trading Ltd')) fail('new company not in UI')
if (!text().includes('No transactions yet')) {
  console.error('--- root text after add (first 700 chars) ---')
  console.error(text().slice(0, 700))
  fail('empty accounts state missing for user company')
}
// 5. Connect a bank -> sample retires from picker
// (Accounts empty-state 'Connect a bank' navigates to Integrations)
click(findByText('button', 'Connect a bank'))
await sleep(80)
// Integrations empty-state 'Connect a bank' opens the add-bank dialog
click(findByText('button', 'Connect a bank'))
await sleep(80)
const starlingConnect = [...document.querySelectorAll('button')].find((b) => b.textContent.trim() === 'Connect' && b.parentElement.textContent.includes('Starling'))
if (!starlingConnect) fail('Starling row Connect button missing')
click(starlingConnect)
await sleep(150)
const pickerText = document.querySelector('aside').textContent
if (pickerText.includes('Sample Co Ltd')) fail('sample still in picker after connecting a source')

// 6. Save progress (simulated account)
const saveBtn = findByText('button', 'Save your progress')
if (!saveBtn) fail('save-progress affordance missing')
click(saveBtn)
await sleep(50)
const nameInput = [...document.querySelectorAll('input')].find((i) => i.placeholder === 'Sam Rivera')
if (!nameInput) fail('account dialog did not open')
type(nameInput, 'Sam')
const emailInput = [...document.querySelectorAll('input')].find((i) => i.placeholder === 'you@company.co.uk')
type(emailInput, 'sam@northwind.co.uk')
click(findByText('button', 'Create account'))
await sleep(100)
if (!text().includes('Saved · Sam')) fail('account-saved state missing')

console.log('INTERACT OK: search→add→connect→retire→account flow verified')
process.exit(0)
