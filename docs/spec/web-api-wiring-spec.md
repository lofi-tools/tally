# tally-web → tally-api wiring spec

Date: 2026-08-11 · App: `apps/tally-web` · API: `apps/tally-api` · Status: spec (no code yet)

## 1. Overview

Wire the Tally web app to the `tally-api` backend (axum + Postgres + toasty)
that already exists. The app keeps its current **local-first** behaviour —
usable without an account, everything in localStorage — and gains a real
**signed-in** mode where the API is the source of truth. Registering copies the
local data up to the user's account, then the app switches to the API
permanently.

Scope for this task: **wire the views the API already covers** — auth,
companies (with real Companies House search), and accounts-from-ledger
(upload, tree, transactions). Payroll's nav item is disabled with a "soon"
badge; Settings, the Filings history list, and the mocked bank connections
stay as they are until their endpoints exist.

## 2. Answer to "do we have all the endpoints we need?"

**No — not quite.** Coverage:

| Frontend need | API endpoint(s) | Status |
|---|---|---|
| Register / login / logout / me | `POST /auth/register`, `POST /auth/login`, `POST /auth/logout`, `GET /auth/me` | ✅ complete |
| Companies CRUD | `GET/POST /companies`, `GET/PATCH/DELETE /companies/{id}` | ✅ complete |
| Companies House search | `GET /companies/search?q=` | ✅ **needs one change: make it unprotected** (§7.2) |
| CH enrichment | `POST /companies/{id}/enrich` | ✅ complete |
| Ledger upload | `POST /companies/{id}/ledgers` (multipart `.gnucash`) | ✅ complete |
| Ledger list / get / delete | `GET /companies/{id}/ledgers`, `GET/DELETE /ledgers/{id}` | ✅ complete |
| Accounts tree | `GET /ledgers/{id}/accounts` | ✅ complete (hierarchy via `parent_guid`) |
| Transactions | `GET /ledgers/{id}/transactions?limit=&offset=` | ✅ complete, but **shape differs** (§8.2) |
| Monthly summaries / YTD cards | — | ⚠️ none — derived **client-side** (§8.4) |
| Bank / Open Banking sources | — | ❌ none — banks stay mocked; ledger upload is the real source (§9) |
| Filing history | — | ❌ none — history stays mocked (§10) |
| Report generation | `POST /companies/{id}/reports/accounts` (+ `corp-tax`, `corp-tax.json`, `ct600`) | ✅ complete — wired for Filings "Preview" (§10) |
| Payroll | — | ❌ explicitly an API non-goal — nav disabled (§11) |
| Settings preferences | — | ❌ none — stays mocked (§12) |

The API contract (endpoints, bodies, error envelope) is `docs/spec/api-backend-spec.md`; this spec only adds the web-facing decisions.

## 3. Goals / non-goals

**Goals**

- Two-mode app: local (no account, localStorage — today's behaviour) and
  signed-in (API-backed). Zero friction to start; real data when the user
  registers.
- Companies + accounts (balances, tree, transactions) are real, fed by
  uploaded GnuCash ledgers.
- Real auth: register/login with bearer tokens persisted across reloads;
  graceful handling of expired sessions.
- Registration migrates the user's local companies (+ any real ledger files)
  to the API, then switches permanently.

**Non-goals**

- No new payroll endpoints, no filing-history storage, no Open Banking.
- No in-browser WASM computation in this task (see §16 for the feasibility
  note).
- No reconciliation / dedup UX yet — merging multiple data sources shows raw
  combined data for now (future work, §18).
- No HMRC submission.

## 4. Architecture

```
browser (vite dev on :5173)                     Rust API (:8080)
┌────────────────────────────┐  /api/* (proxy)  ┌─────────────────────────┐
│ local mode (no token)      │ ───────────────► │ auth, companies,        │
│   localStorage companies   │                  │ ledgers, reports        │
│ signed-in mode (token)     │ ◄─────────────── │ Postgres (toasty)       │
│   API is source of truth   │                  └─────────────────────────┘
└────────────────────────────┘
```

- **Vite dev proxy** forwards `/api/*` to `http://127.0.0.1:8080` (dev only;
  the API's CORS is already permissive for `localhost:5173`).
- **Hard requirement in dev**: Postgres + the API must be running
  (`nix develop -c dev-db` then `nix develop -c api`). The README(s) and a
  `scripts/` note document this. API-backed views show a clear error state
  when the API is unreachable — no silent mock fallback for signed-in views.
- Pre-login, the app needs no API except **CH search** (§7.2), which is
  deliberately unprotected.

## 5. Auth & session

### 5.1 UI

- The sidebar's current "Save your progress — create an account" button
  becomes a **"Sign in"** button opening the **`SignInDialog`** (replaces
  `SaveProgressDialog`). The app stays fully usable without auth — no
  full-screen gate.
- `SignInDialog` anatomy (follows the existing dialog conventions):
  - **Title**: "Sign in to Tally" — with a small **segmented control** with
    two tabs: **Log in** / **Create account**.
  - **Description** (login): "Sign in to load your companies and books."
  - **Description** (register): "Create an account — your local data is moved
    to it automatically."
  - **Login form**: `Email`, `Password` → button **"Log in"**
    (`POST /auth/login`).
  - **Register form**: `Name`, `Email`, `Password` (helper text "At least 8
    characters") → button **"Create account"** (`POST /auth/register`),
    then the migration phase (§7.3) if local data exists.
  - Footer: **"Not now"** (secondary, closes the dialog).
- After sign-in the sidebar shows the signed-in user (from `/auth/me`) with a
  **"Sign out"** item in a small user menu (`POST /auth/logout`, clears the
  token, returns to local mode). The simulated `account` state and
  `SaveProgressDialog` are removed.
- Errors are inline per field from the mapping table (§14.5):
  `email_taken` under Email on register; `invalid_credentials` under the
  password on login.

### 5.2 Token & restore

- Token persisted in **localStorage** (`tally.token.v1`), returned once from
  register/login.
- On app load: if a token exists, call `GET /auth/me` to restore the session.
  - `200` → signed-in mode (§6).
  - `401` (`auth_invalid` / `auth_expired`) → clear the token, drop to local
    mode, show a toast ("Session expired — sign in again").
  - network failure → show the API-offline error state (not silent local
    fallback, per §4).

### 5.3 401 handling at runtime

- A single fetch wrapper intercepts `401` responses: clears the token,
  switches to local mode, and surfaces a "session expired" toast. Every
  request goes through this wrapper.

## 6. Mode switch (local → signed-in)

- **Permanent switch**: after a successful register (and migration, §7.3),
  the app uses the API for everything. The migrated localStorage data is
  cleared (the DB copy is now authoritative). Logout returns to an empty
  local workspace (sample company may reappear — see §13).
- Login to an **existing** account: switch to API mode directly (no
  migration — the account already has its data server-side).
- The sample company rules (§13) apply in both modes.

## 7. Companies

### 7.1 List / picker / CRUD

- Signed-in mode: the company picker and list come from `GET /companies`
  (sorted by name). "Add company" posts to `POST /companies`.
- The current AddCompanyDialog's create step sends the mapped fields:
  `name`, `company_number`, `tax_reference` (from UTR), `sic_codes: [sic]`,
  `address_lines: [address]`, and the reporting standard is kept locally for
  now (the API's accounts-metadata dimensions stay default until the
  accounts-prep flow is built).
- Settings' "Remove company" wires to `DELETE /companies/{id}` (cascades
  ledgers server-side).
- `PATCH /companies/{id}` backs the Settings company-profile save (only the
  mapped fields above).

### 7.2 CH search (the one API change)

- The app calls the backend's search endpoint **without a bearer token** —
  the backend holds the Companies House key and relays. Currently
  `GET /companies/search` requires `AuthUser`; **change it to unprotected**
  (same handler, drop the extractor; rate-limiting/abuse is accepted at this
  stage). Everything else (error mapping, `q` validation) is unchanged.
- The AddCompanyDialog search results come from the API (`company_not_found`/
  `companies_house_upstream`/`companies_house_rate_limited` → inline error
  message; `companies_house_key_missing` → "Companies House isn't configured"
  with guidance, user can still create the company manually). The local mock
  `companySearchResults` fixture is **removed**.
- `POST /companies/{id}/enrich` backs a "Fetch from Companies House" button
  on the AddCompanyDialog review step (fill blanks from CH).

### 7.3 Migration on register

Only **real** local data migrates (never the sample's demo data or mock bank
connections). If there is no local data, register lands directly in
signed-in mode with the "no data" toast below.

**Step order** (runs inside `SignInDialog` after a successful register,
replacing the form with a progress state):

```
register OK
  ├─ local data?  no  → setToken → session signed-in → toast "Account created —
  │                        you're all set."                          [no-data]
  └─ yes → phase MIGRATING (spinner + "Moving your data…")
           1. for each local company (sorted by name):
                POST /companies  → migrated++ / duplicate_company → skipped++
                 / hard failure → failed++, stop
           2. for each locally staged .gnucash file (none today):
                POST /companies/{id}/ledgers → migrated++ / failure → failed++, stop
  └─ setToken → session signed-in → clear migrated localStorage entries → toast
```

**Per-item outcomes & local-data handling:**

- `migrated` — POST succeeded; the local copy is redundant → removed from
  localStorage.
- `skipped` — `409 duplicate_company`: the company already exists in the
  account (or another account's) — counted, not an error; the local copy is
  removed (the API copy is authoritative).
- `failed` — anything else (`NetworkError`, 5xx, `validation_failed`): stop
  the loop, **keep the local data**, still enter signed-in mode (the account
  exists), and show the partial toast below.

**Summary toast copy** (dynamic counts):

| Outcome | Title | Description |
|---|---|---|
| All migrated | "Account created" | "Migrated 3 companies · 1 ledger." |
| Some skipped (conflicts) | "Account created" | "Migrated 2 companies · 1 already in your account." |
| Some failed | "Account created — almost there" | "2 of 3 companies migrated; 1 couldn't be moved. It's still saved locally." |
| No local data | "Account created" | "You're all set." |

**Retry**: when `failed > 0`, a **"Retry migration"** item appears in the
sidebar user menu; it re-runs §7.3 for the remaining local items (no
re-register needed — it uses the stored token) and updates the summary toast.

## 8. Accounts tab from API data

### 8.1 Which data feeds the tab

- **Merge all of the company's ledgers** (`GET /companies/{id}/ledgers` →
  fetch `accounts` + `transactions` for each). Dedup / reconciliation /
  linking of overlapping transactions is **future UX work** (§18); for now
  the combined rows are shown as-is, sorted by date.
- Uploaded ledgers appear in Integrations (§9) and are the only real source;
  mocked bank connections contribute no data.

### 8.2 Transaction rows — adapter shape (decision)

One row **per transaction**, showing **both accounts involved** (names only),
and the **amount transacted**:

| Column | Source |
|---|---|
| Date | `transaction.post_datetime` (RFC 3339 → day) |
| Description | `transaction.description` (empty → rendered as "From → To", e.g. "Starling → Rent") |
| From account | derived from the splits (§8.2.1) |
| To account | derived from the splits (§8.2.1) |
| Amount | §8.2.1 — magnitude of the moved value, coloured by the P&L type |
| Source | the ledger name |
| Status | derived: `cleared` (the API stores no status; reconciliation states are future work) |

**Inputs**: the transaction (`guid`, `post_datetime`, `description`, `splits[]`)
plus a `Map<guid, { name, type }>` built by flattening the ledger's accounts
view (the API change in §15 adds `guid` to each node). Splits whose `value`
is `"0"` are dropped before classification.

#### 8.2.1 From/To derivation & amount

**P&L account set**: GnuCash types `INCOME` and `EXPENSE` (everything else —
Asset/Bank/Cash/Receivable/Liability/Credit/Payable/Equity — is
"balance-sheet"). Split values are GnuCash-natural (§8.3).

1. **An `EXPENSE` split exists** → the money left the book into an expense:
   - `From` = the counterpart (first non-P&L split's account);
   - `To` = the expense account;
   - Amount = `|counterpart value|`, shown **negative/red**.
   - Multiple expense splits → `To` = the largest-|value| one; a tooltip
     lists the rest ("+1 more: Utilities £100.00").
2. **An `INCOME` split exists** (and no expense) → money came in from income:
   - `From` = the income account; `To` = the counterpart;
   - Amount = `|counterpart value|`, shown **positive/green**.
   - Multiple income splits → same "primary + tooltip" rule.
3. **No P&L split (balance-sheet only)** → a transfer:
   - `From` = the account of the **negative**-value split;
   - `To` = the account of the **positive**-value split (splits sum to zero,
     so there is one of each in the 2-split case);
   - Amount = `|value|`, shown **muted/unsigned**.
4. **≥3 splits with no P&L**: group by sign — `From` = the negative-side
   accounts, `To` = the positive-side accounts; primary = largest |value|,
   rest in the tooltip.
5. **Rare both-P&L / unresolvable** (mixed income+expense, all-same-sign):
   degrade — `From`/`To` = the two largest-|value| accounts in order,
   amount muted. Never crash; never invent an account name.

#### 8.2.2 Worked examples

| Splits (guid → account resolved) | Kind | From | To | Amount shown |
|---|---|---|---|---|
| Starling −1850.00 · Rent +1850.00 | expense | Starling | Rent | −£1,850.00 (red) |
| Sales −4280.00 · Starling +4280.00 | income | Sales | Starling | +£4,280.00 (green) |
| Starling −500.00 · Barclays +500.00 | transfer | Starling | Barclays | £500.00 (muted) |
| Share capital −15000.00 · Starling +15000.00 | transfer | Share capital | Starling | £15,000.00 (muted) |
| Starling +1200.00 · Sales −1000.00 · VAT payable −200.00 | income + VAT | Sales | Starling (tooltip: VAT payable −£200.00) | +£1,200.00 (green) |
| Starling −300.00 · Rent +200.00 · Utilities +100.00 | expense, split | Starling | Rent (tooltip: Utilities £100.00) | −£300.00 (red) |

- Pagination: fetch with a generous `limit` (e.g. 500) and a "load more"
  affordance; the merged view sorts client-side by date desc.
- Sign conventions in the examples are GnuCash-natural (negative split =
  money out of that account); the app *display* signs are applied in §8.3.

### 8.3 Sign normalisation (app convention)

API balances and split values are GnuCash-natural; the app shows **income
positive/green** and **expenses negative/red**. Normalise with a single
function, applied everywhere (tree node balances, rolled-up totals, YTD
cards, monthly summaries, and the row amount's sign):

```ts
// v = GnuCash-natural decimal; returns the app-display sign
appSign(type: GnuCashType, v: number): number {
  return (type === 'INCOME' || type === 'EXPENSE') ? -v : v
}
```

| GnuCash type | GnuCash balance | `appSign` | App colour |
|---|---|---|---|
| Asset / Bank / Cash / Receivable | positive | `v` | green (positive) |
| Liability / Credit / Payable | negative | `v` | red |
| Equity | negative | `v` | red |
| Income | negative | `-v` → positive | green |
| Expense | positive | `-v` → negative | red |

Zero → muted, no colour. The row **amount** uses the P&L-derived sign (§8.2.1)
plus the magnitude; the tree and summaries use `appSign` per account/type.

Worked checks against the Balances mock (which uses the same convention):

- Sales balance: GnuCash ≈ −22 348.70 → app shows **+£22,348.70 green**.
- Rent balance: GnuCash ≈ +1 850.00 → app shows **−£1,850.00 red**.
- Starling: +45 000.00 → **+£45,000.00 green**; Loan −20 000.00 →
  **−£20,000.00 red**; Share capital −15 000.00 → **−£15,000.00 red**.

### 8.4 Balances tab

- **Tree**: built from `GET /ledgers/{id}/accounts` merged across ledgers —
  real hierarchy from `parent_guid`, real balances (normalised per §8.3),
  rolled-up group totals computed from the tree. Render depth as the ledger
  has (not forced to 2 levels). Group rows stay collapsible; leaf click opens
  the existing right drawer with that account's transactions.
- **YTD cards + monthly summaries**: derived **client-side** from the merged
  transactions view (sum split values per month by account type, sign-
  normalised). No new endpoint.
- Empty states: no ledgers → the existing combined "No data yet — connect a
  bank / upload a ledger" state, now with a working **"Upload a GnuCash
  ledger"** action.

## 9. Integrations / data sources

- **Real**: "Upload a GnuCash ledger" — file input (`input[type=file]`,
  `.gnucash`) → `POST /companies/{id}/ledgers` (multipart, field `file`).
  Progress + success toast; errors mapped from the envelope (`file_too_large`
  413, `unsupported_file_type` 415, `ledger_parse_failed` 422, etc.).
- **Mock (kept)**: the bank-connection list (Starling, Barclays, Monzo…)
  stays exactly as today with its "lands with the backend" toasts — Open
  Banking is future work.
- Uploaded ledgers render as **data source rows** (new `ledger` kind in
  `DataSourceRows` with name = uploaded filename, synced = uploaded date,
  delete action → `DELETE /ledgers/{id}`).

## 10. Filings

- **Wire generation**: the "Preview" button on the next-filing card posts to
  `POST /companies/{id}/reports/accounts` with the latest ledger's id and
  opens the returned FRS 105 iXBRL (HTML) in a new tab. Because the bearer
  token can't travel in a plain `<a href>`, fetch → `Blob` → `URL.createObjectURL`
  → `window.open`. (Corp-tax JSON + CT600 XML buttons can follow the same
  pattern; wire the JSON one too since it's cheap.)
- **Stays mocked**: the previous-filings history table and the FY picker
  (no filing-history endpoints). "File now" keeps its mock toast.

## 11. Payroll

- Nav item **disabled** with a **"Soon" badge** (sidebar). The Payroll view
  is unreachable until payroll endpoints exist (an explicit API non-goal).

## 12. Settings

- Stays mocked as-is (profile save, preferences, notifications, remove
  company confirm). Remove-company *action* wires to the API (§7.1); the rest
  keeps its "lands with the backend" toasts.

## 13. Sample company

- Same rule as today: the localStorage sample shows until **real data
  exists** (signed-in: any company with a ledger; local mode: any local
  company with a source), then retires. In signed-in mode with zero migrated
  companies, the sample still shows.

## 14. API client layer (`src/api.ts` + `src/session.ts`)

### 14.1 Module layout

- `src/api.ts` — the fetch wrapper, the error classes, and one typed
  function per endpoint. No UI here.
- `src/session.ts` — the session store (Solid signal created inside
  `createRoot`, so it's usable at module scope): a tiny state machine plus
  `signOut()` / `handleExpired()` helpers that also clear the token.
- `src/components/Shared.tsx` — a small `LoadState` helper (loading skeleton
  / error card with retry) reused by the API-backed views.

### 14.2 Session store

```ts
export type Session =
  | { status: 'restoring' }                          // token present, /auth/me pending
  | { status: 'local' }                              // no valid session (default mode)
  | { status: 'signed-in'; user: AuthUser }          // API mode
  | { status: 'offline' }                            // API unreachable

export const [session, setSession]: Signal<Session>

export function signOut(): void          // setToken(null) + { status: 'local' }
export function handleExpired(): void    // setToken(null) + { status: 'local' } + toast "Session expired"
export function markOffline(): void      // { status: 'offline' } (kept token; retry on next call)
```

- App boot: token? → `{ status: 'restoring' }` + `me()` → signed-in, or 401 →
  local (handleExpired), or `NetworkError` → offline.
- Any runtime 401 (`auth_invalid`/`auth_expired`) funnels through
  `handleExpired()` (§14.3 step 6); the app reacts to `session()` and swaps
  the view data sources.

### 14.3 Fetch wrapper

```ts
const API_BASE = '/api'   // vite dev proxy → 127.0.0.1:8080 (no env var in dev)

const TOKEN_KEY = 'tally.token.v1'
export const getToken = (): string | null => localStorage.getItem(TOKEN_KEY)
export const setToken = (t: string | null): void   // set / remove TOKEN_KEY

export class ApiError extends Error {
  constructor(
    readonly code: string,              // envelope error.code (stable, snake_case)
    readonly message: string,           // envelope error.message (UI-safe)
    readonly details: Record<string, unknown> | undefined,
    readonly status: number,
  ) { super(message) }
}
export class NetworkError extends Error { }        // fetch threw or body wasn't JSON

interface ApiOptions {
  method?: 'GET' | 'POST' | 'PATCH' | 'DELETE'     // default GET
  json?: unknown                                    // JSON body (sets content-type)
  form?: FormData                                   // multipart (no content-type set)
  auth?: boolean                                    // default true-when-token; false = never send
  raw?: boolean                                     // resolve with the Response (report blobs)
  signal?: AbortSignal
}

export async function api<T = unknown>(path: string, opts: ApiOptions = {}): Promise<T>
```

Behaviour, in order:

1. URL = `API_BASE + path`; `Accept: application/json`.
2. `opts.auth !== false && getToken()` → `Authorization: Bearer <token>`.
3. Body: `json` → `JSON.stringify` + `application/json`; `form` → as-is
   (browser sets the boundary).
4. `fetch`; a throw → `throw new NetworkError()`.
5. `resp.ok`: `raw` → return the `Response`; `204` → return `undefined`;
   else parse JSON → `T`.
6. Non-ok: parse the envelope body (`{ error: { code, message, details } }`).
   - Parsed → `throw new ApiError(code, message, details, status)`.
   - `status === 401 && code ∈ { auth_expired, auth_invalid }` →
     `handleExpired()` first, then throw.
   - Body not parseable → `throw new NetworkError()` (5xx envelope should
     always parse; anything else is a proxy/server fault).

Callers never branch on `message` text — only on `code` (§14.5).

### 14.4 Typed endpoint functions

All bodies/returns mirror the API's serialized shapes (`apps/tally-api`
`models.rs` / `ledgers.rs` / `auth.rs` / `companies_house.rs`):

```ts
// ---- auth ----
export interface AuthUser { id: string; email: string; display_name: string; created_at: string }
export interface AuthResponse { token: string; user: AuthUser }
register(body: { display_name: string; email: string; password: string }): Promise<AuthResponse>
login(body: { email: string; password: string }): Promise<AuthResponse>
logout(): Promise<void>                                  // POST, 204
me(): Promise<AuthUser>

// ---- companies ----
export interface Company {                              // full serialized model (§5 of api-backend-spec)
  id: string; user_id: string; name: string; tax_reference: string;
  company_number: string; registration_date: string | null;
  directors: string[]; sic_codes: string[]; address_lines: string[];
  fy1_year: number; fy2_year: number; /* …all other profile fields… */
}
export interface CompanyInput { /* all-optional §5 union (name, tax_reference, company_number, sic_codes, address_lines, …) */ }
listCompanies(): Promise<Company[]>
createCompany(input: CompanyInput): Promise<Company>
getCompany(id: string): Promise<Company>
patchCompany(id: string, input: CompanyInput): Promise<Company>
deleteCompany(id: string): Promise<void>                 // 204
searchCompanies(q: string): Promise<SearchItem[]>        // NO auth (opts.auth=false)
enrichCompany(id: string): Promise<Company>

// ---- ledgers ----
export interface Ledger {
  id: string; company_id: string; name: string; file_sha256: string;
  uploaded_at: string; accounts_count: number; transactions_count: number; splits_count: number;
}
export interface AccountNode { name: string; type: string; balance: string; guid: string; children: AccountNode[] }
export interface AccountsView { accounts: AccountNode[]; net_assets: string }
export interface Split { account_guid: string; value: string }        // value = decimal string

export interface LedgerTransaction {
  guid: string; post_datetime: string; description: string;          // description: API change (§15)
  splits: Split[];
}
export interface TransactionsPage { items: LedgerTransaction[]; limit: number; offset: number }
listLedgers(companyId: string): Promise<Ledger[]>
uploadLedger(companyId: string, file: File): Promise<Ledger>        // FormData('file', file)
deleteLedger(id: string): Promise<void>                              // 204
ledgerAccounts(id: string): Promise<AccountsView>
ledgerTransactions(id: string, q?: { limit?: number; offset?: number }): Promise<TransactionsPage>

// ---- reports (raw documents) ----
export interface ReportRequest {
  ledger_id: string;
  period?: { start: string; end: string };
  made_up_to?: string;
  /* declaration + overrides as the build needs */
}
/** fetch → Blob → object URL, for opening iXBRL/XML in a new tab (§10). */
export async function generateReportDocument(companyId: string, kind: 'accounts' | 'corp-tax' | 'ct600', body: ReportRequest): Promise<string>
```

### 14.5 Error-code mapping table

`code` (from the envelope) → UI behaviour. Every known code is mapped here;
unknown codes fall through to a generic toast that shows the envelope
`message` (it is UI-safe by contract).

| code | HTTP | UI behaviour |
|---|---|---|
| `auth_missing` / `auth_invalid` / `auth_expired` | 401 | `handleExpired()` — clear session, toast "Session expired", drop to local mode |
| `email_taken` | 409 | inline under email field: "An account with this email already exists" |
| `invalid_credentials` | 401 | inline on login: "Invalid email or password" |
| `validation_failed` | 422 | map `details.fields[]` → per-field errors in dialogs; else summary toast |
| `duplicate_company` | 409 | AddCompany inline: "Already in your workspace"; migration: skip + count (§7.3) |
| `not_found` | 404 | if a just-deleted resource → silent refresh of the list; else toast |
| `ledger_not_in_company` | 422 | toast (invariant — client sent a foreign ledger id) |
| `unsupported_file_type` | 415 | upload inline: "Only .gnucash files are supported" |
| `file_too_large` | 413 | upload inline: "File too large (limit {details.limit_bytes})" |
| `ledger_parse_failed` | 422 | upload inline: "Couldn't read this ledger file — is it a valid GnuCash book?" |
| `companies_house_key_missing` | 400 | search inline: "Companies House isn't configured" + manual-add path (§7.2); enrich → toast |
| `company_not_found` | 404 | search inline: "No company found with that number" |
| `companies_house_rate_limited` | 429 | search inline: "Companies House rate limit — try again shortly" |
| `companies_house_upstream` | 502 | search inline: "Companies House is unavailable — try again" |
| `period_not_determined` | 422 | reports toast with `details.hint` |
| `storage_error` / `internal` | 500 | toast "Something went wrong" (log `details.request_id` to console for support) |
| `route_not_found` / `method_not_allowed` | 404/405 | toast (dev-facing — signals a client/API version mismatch) |
| *(fetch threw / body unparseable)* | — | `NetworkError` → offline state (§14.6) |

### 14.6 Loading / 401 / offline propagation to views

- **Loading**: each API-backed view loads via Solid `createResource` and
  renders through the shared `LoadState` helper:
  `loading → skeleton rows` (accounts tree / transaction rows keep their
  real geometry), `error → error card with Retry` (`refetch()`), `data →
  content`. Mutations (upload, save) use small per-button spinners.
- **401**: `api()` calls `handleExpired()` (14.3.6); `session()` flips to
  `local`; `App` swaps the view data props to local-mode sources. Views never
  handle 401 themselves.
- **Offline**: a `NetworkError` marks the session `offline` (token kept).
  `App` shows a persistent "API unreachable — is the API running?" banner
  (same slot as the sample banner); API-backed views show their error cards
  (with retry — a successful call flips back to `signed-in`).
- **Restoring**: on boot with a token, the app keeps rendering the local mode
  (or a light skeleton) until `me()` resolves; no flash of a login wall.

## 15. API changes required (summary)

| Change | Why |
|---|---|
| `GET /companies/search` — remove the `AuthUser` requirement | frontend search is pre-login; backend holds the CH key (§7.2) |
| `GET /ledgers/{id}/accounts` — include `guid` on each `AccountNodeOut` | resolve split `account_guid`s → account names for the From/To columns (§8.2); the node currently carries only name/type/balance/children |
| Store `description` on `Transaction` (model + ingest) and return it in `TransactionOut` | the app's Description column has no source today — the API drops GnuCash descriptions at ingest (§8.2) |

All three are additive and backwards-compatible. Everything else reuses
existing endpoints unchanged.

## 16. WASM feasibility note (asked during interview)

"Can the Rust libs run as WASM in the frontend?" — **partly, with one real
blocker**:

- **WASM-friendly**: the compute core — `quick-xml` (XML books + iXBRL
  generation), `rust_decimal`, `sha2`, `chrono` (needs the `wasmbind`
  feature), `serde`/`serde_json`; `reqwest` compiles for
  `wasm32-unknown-unknown` via the browser `fetch` (wasm-bindgen ecosystem),
  so even the Companies House client could run in-browser.
- **Blocker**: `rucash`'s **`sqlite` feature** pulls `rusqlite` →
  `libsqlite3-sys`, which compiles native C (the `cc` crate does not support
  `wasm32-unknown-unknown`). SQLite-format GnuCash books could not be parsed
  in-browser without extra work (e.g. `wasm32-wasi` + clang, or a
  wasm-bindgen SQLite shim). XML-format books would work.
- **Verdict**: keep computation server-side for this task (it already is).
  A WASM build of the ixbrl compute core (XML path) is a plausible future
  spike; SQLite books and the wasm-bindgen toolchain make it non-trivial.

## 17. Files touched (planned)

- `src/api.ts` (new) — fetch wrapper (§14.3), typed endpoint functions
  (§14.4), error mapping (§14.5), token storage helpers.
- `src/session.ts` (new) — session store state machine (§14.2).
- `src/components/Shared.tsx` — `LoadState` helper (skeleton / error card
  with retry, §14.6).
- `src/db.ts` — token key already in `api.ts`; migration/clear helpers.
- `src/App.tsx` — signed-in vs local mode, session restore (§5.2), sidebar
  auth UI + logout, disabled Payroll item + "Soon" badge, wiring `accounts`
  view data props to the API layer.
- `src/views/Accounts.tsx` — data adapter (§8.2–8.4), loading/error states,
  real empty-state upload action.
- `src/views/Integrations.tsx` — real ledger upload, ledger data-source rows.
- `src/components/AddCompanyDialog.tsx` — API search (§7.2), enrich button.
- `src/components/SignInDialog.tsx` (new) — login/register + migration
  phases (§5.1, §7.3); delete `SaveProgressDialog.tsx`.
- `src/views/Filings.tsx` — Preview → generate iXBRL (§10).
- `src/views/Settings.tsx` — save/remove wired to the API (§7.1, §12).
- `vite.config.ts` — `/api` proxy to `:8080`.
- `apps/tally-api/src/companies.rs` — drop `AuthUser` from `search`.
- `apps/tally-api/src/ledgers.rs` — include `guid` in `AccountNodeOut`;
  return `description` in `TransactionOut`.
- `apps/tally-api/src/models.rs` — add `description` to the `Transaction`
  model (+ ingest in `ledgers.rs`).
- READMEs — dev requirement (db + api) documented.

## 18. Out of scope / future

- **Reconciliation / dedup / transaction linking** across merged sources
  (a UX to match the same transaction from bank + ledger).
- Open Banking connections, CSV import, HMRC MTD.
- Filing-history endpoints and "File now" submission.
- Payroll endpoints.
- P&L standalone report (lib-side roadmap), then an API endpoint.
- WASM-in-browser spike (§16).
