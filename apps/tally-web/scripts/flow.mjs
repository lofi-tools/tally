// End-to-end onboarding test (`pnpm flow`) — mounts the built bundle in jsdom
// and drives the flows: add company via search -> demo banner resolves ->
// connect bank -> sign-in dialog opens.
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

// The add-company dialog searches real Companies House through the backend
// (GET /api/v1/companies/search), and guest adds are API-backed end-to-end
// (POST /auth/guest + POST/GET /companies — temp-user spec §7.4). There's
// no server in jsdom, so serve canned, stateful responses for those; any
// other request behaves offline.
const northwindSearch = [{
  company_number: '01234567',
  company_name: 'Northwind Trading Ltd',
  company_status: 'active',
  date_of_creation: '2020-04-01',
  address_snippet: '1 Northwind Way, London',
  company_type: 'ltd',
  description: 'Trading company',
}]
const guestUser = {
  id: '00000000-0000-0000-0000-0000000000aa',
  email: 'temp+jsdom@local',
  display_name: 'Guest',
  created_at: new Date().toISOString(),
  is_temporary: true,
  guest_id: 'jsdom-guest',
}
const createdCompanies = []
// Canned filings for the newly-added company: no fetch has ever completed,
// so the backend's provisional-period derivation returns the full schedule
// — ended years as `provisional` (structure-only), the current year as
// `ongoing` (provisional-periods spec §4.2).
const filingsFixture = {
  periods: [
    {
      start: '2026-04-01', end: '2027-03-31', status: 'ongoing',
      due: { hmrc: '2028-03-31', ch: '2027-12-31' },
      filings: [
        { kind: 'accounts', state: 'not-sent' },
        { kind: 'corporation-tax', state: 'not-sent' },
      ],
    },
    { start: '2025-04-01', end: '2026-03-31', status: 'provisional', due: null, filings: [] },
    { start: '2024-04-01', end: '2025-03-31', status: 'provisional', due: null, filings: [] },
    { start: '2023-04-01', end: '2024-03-31', status: 'provisional', due: null, filings: [] },
  ],
  balance_sheets: [],
  status: { state: 'none', fetched_at: null, last_error: null },
}
const json = (body, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } })
globalThis.fetch = async (input, init) => {
  const url = String(input)
  const method = (init && init.method) || 'GET'
  if (url.startsWith('/api/v1/companies/search?')) return json(northwindSearch)
  if (url === '/api/v1/auth/guest' && method === 'POST') {
    return json({ token: 'guest-token-' + createdCompanies.length, user: guestUser })
  }
  if (url === '/api/v1/companies' && method === 'POST') {
    const body = JSON.parse(init.body)
    const company = {
      id: `c-${createdCompanies.length + 1}`,
      user_id: guestUser.id,
      name: body.name,
      company_number: body.company_number,
      tax_reference: '',
      registration_date: body.registration_date ?? null,
      sic_codes: [],
      address_lines: [],
      accounting_standard: body.accounting_standard ?? 'FRS 105',
      updated_at: new Date().toISOString(),
    }
    createdCompanies.push(company)
    return json(company)
  }
  if (url === '/api/v1/companies' && method === 'GET') return json(createdCompanies)
  if (url.includes('/filings') && method === 'GET') return json(filingsFixture)
  throw new TypeError('offline in jsdom: ' + url)
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
if (!text().includes('Demo data')) fail('banner missing at start')

// 0. The demo banner is persistent (not dismissible): it shows on every
// screen while the onboarding/demo state holds (DemoBanner, spec §5.2).
click(findByText('button', 'Filings'))
await sleep(50)
if (!text().includes('Demo data')) fail('banner missing on the Filings screen')
click(findByText('button', 'Accounts'))
await sleep(50)
if (!text().includes('Demo data')) fail('banner missing after switching back')

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

// 3. Submit (the dialog footer button is the LAST 'Add company'). UTR is no
// longer collected at add time — it's entered in Settings when filing.
const submitBtn = [...document.querySelectorAll('button')].filter((b) => b.textContent.trim() === 'Add company').at(-1)
if (!submitBtn) fail('dialog submit button missing')
click(submitBtn)
await sleep(100)
if (errors.length) console.error('RUNTIME ERRORS after submit:', errors.map((e) => String(e && e.message ? e.message : e).slice(0, 200)).join(' | '))

// 4. Assertions after add
if (text().includes('Demo data')) fail('banner still visible after adding a company')
if (!text().includes('Northwind Trading Ltd')) fail('new company not in UI')
if (!text().includes('No transactions yet')) {
  console.error('--- root text after add (first 700 chars) ---')
  console.error(text().slice(0, 700))
  fail('empty accounts state missing for user company')
}
// 4b. Filings for the guest company: no fetch has completed, so the sub-nav
// shows the full provisional period list — dashed rows with a per-row
// indicator + the mini-banner with a Fetch button — and a provisional
// detail pane is structure-only (provisional-periods spec §6.1–§6.3).
click(findByText('button', 'Filings'))
await sleep(120)
if (!text().includes('Some periods are estimated')) fail('provisional mini-banner missing in the filings sub-nav')
if (!findByText('nav button', 'provisional')) fail('per-row provisional indicator missing')
if (!findByText('nav button', 'Fetch missing filings')) fail('sub-nav Fetch button missing')
// Select the newest provisional period → its detail is the estimated note
// with no due dates or file actions.
const provRow = [...document.querySelectorAll('nav button')].find((b) => b.textContent.includes('provisional'))
if (!provRow) fail('no provisional period row to select')
click(provRow)
await sleep(80)
if (!text().includes('Estimated period')) fail('provisional detail note missing')
if (text().includes('Prepare / Preview') || text().includes('File now')) fail('provisional period must have no actions')
click(findByText('button', 'Accounts'))
await sleep(80)

// 5. Connect a bank -> the demo banner resolves. Both the accounts
// empty-state and the banner carry a 'Connect a bank' CTA, so scope to
// <main> to click the view's own button (banner CTA only navigates).
click(findByText('main button', 'Connect a bank'))
await sleep(80)
// Integrations empty-state 'Connect a bank' opens the add-bank dialog
click(findByText('main button', 'Connect a bank'))
await sleep(80)
const starlingConnect = [...document.querySelectorAll('button')].find((b) => b.textContent.trim() === 'Connect' && b.parentElement.textContent.includes('Starling'))
if (!starlingConnect) fail('Starling row Connect button missing')
click(starlingConnect)
await sleep(150)
// Once any company has a data source the demo banner resolves entirely
// (spec §4). The demo company itself stays in the picker, badged (§6.2).
if (text().includes('Demo data')) fail('demo banner still visible after connecting a source')
const pickerText = document.querySelector('aside').textContent
if (!pickerText.includes('Demo Co Ltd')) fail('demo should stay in the picker (badged) per spec §6.2')

// 6. The add ran as a guest workspace (no session at submit time), so the
// sidebar shows the guest affordance — "Save your work — create account"
// (temp-user spec §7.7). Verify it opens the (register-defaulted) dialog.
const saveBtn = findByText('button', 'Save your work — create account')
if (!saveBtn) fail('guest create-account affordance missing')
click(saveBtn)
await sleep(80)
if (!text().includes('Sign in to Tally')) fail('sign-in dialog did not open')

console.log('INTERACT OK: search→guest-add→provisional-filings→connect→banner-resolve→create-account flow verified')
process.exit(0)
