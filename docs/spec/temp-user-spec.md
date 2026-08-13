# Temporary (guest) users — spec

Status: drafted from interviews — no code changes yet.
Scope: backend `apps/tally-api` + web `apps/tally-web`.

## 1. Problem

Today, adding a company while signed out writes **only** to localStorage
(`apps/tally-web/src/db.ts`). Nothing reaches the backend, so:

- no `companies` row is inserted (no CH enrichment, no ownership),
- no `fetch_filings` job is spawned (no filing history / balance sheet),
- the data lives only in the browser and only "migrates" when the user
  registers — via a client-side copy loop (`migrateCompanies`, §7.3) that
  re-creates each company through `POST /companies`.

`/companies/search` is already deliberately unprotected (§7.2), so the
search-as-you-type works pre-login — it is the **save** that has no home.

Goal: give every anonymous visitor a lightweight server-side identity — a
**temporary user** — so adds (and the jobs they spawn) work exactly like a
signed-in user's, and registering later *upgrades* that identity in place
instead of copying data around.

## 2. Terminology

| Term | Meaning |
|---|---|
| guest / temp user | A `users` row with `is_temporary = true`, a placeholder email, and no real password |
| guest id | A client-generated random UUID identifying one browser's anonymous workspace (`tally.guest.v1` in localStorage) |
| adoption / upgrade | Registering with email+password on the same `users` row, clearing the temp flag |
| outbox | Web-side localStorage queue of adds made while offline, replayed on reconnect |

## 3. Current state (verified facts)

- `User.email` is `TEXT NOT NULL` + unique index; `password_hash` is
  `TEXT NOT NULL` (migrations/0001_init.sql). A temp user therefore needs a
  placeholder email and a dummy hash to satisfy NOT NULL.
- Every non-search handler requires `AuthUser` (bearer token) —
  `auth.rs::FromRequestParts`. Sessions: opaque token, sha256 stored, 30-day
  TTL.
- `POST /companies` (`companies.rs::create`) requires auth, validates, CH-enriches
  when a number + key exist, then enqueues `fetch_filings` fire-and-forget
  when `company_number` + CH key are set.
- The web app is **local-first**: `db.ts` holds `companies` + `sources` in
  localStorage; the picker/views read from there; `App.tsx::addCompany`
  branches — signed-in → `POST /companies`, signed-out → localStorage only.
- Register currently triggers `migrateCompanies()` (§7.3): a client loop of
  `createCompany` per local company. `SignInDialog` owns that flow.
- `companies` table has **no** `created_at`/`updated_at` column at all.

## 4. Identity model (backend)

### 4.1 Schema — migration `0003_temp_users.sql`

On `users`:

```sql
ALTER TABLE "users" ADD COLUMN "is_temporary" BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE "users" ADD COLUMN "guest_id" TEXT;             -- unique, NULL for real users
CREATE UNIQUE INDEX "index_users_by_guest_id" ON "users" ("guest_id");
```

- Placeholder email for temp rows: `temp+<uuid>@local` (satisfies NOT NULL +
  unique; `valid_email` passes; cannot collide with a real email).
- Placeholder password: reuse the existing `DUMMY_HASH` constant (a valid
  argon2 hash of a random password) so login can never succeed for a temp
  user — and the timing-equalisation branch stays honest.

On `companies` (new columns; `standard` today exists only in the web mock
shape, `updated_at` is needed for the sweep):

```sql
ALTER TABLE "companies" ADD COLUMN "accounting_standard" TEXT NOT NULL DEFAULT 'FRS 105';
ALTER TABLE "companies" ADD COLUMN "updated_at" TEXT;
```

`updated_at` (RFC 3339) is written on create and on PATCH (and could be
bumped on any ownership-needing write later). It is the activity clock for
the sweep (§8).

### 4.2 Model updates (`models.rs`)

- `User`: `is_temporary: bool`, `guest_id: Option<String>` — both serialized
  (`is_temporary` especially: the web UI branches on it; `guest_id` is
  harmless to expose).
- `Company`: `accounting_standard: String`, `updated_at: Option<String>`.

## 5. Auth flow

### 5.1 `POST /auth/guest` — bootstrap a guest session (new)

Request: header `X-Guest-Id: <uuid>` (no body needed).
Response: `{ token, user }` (same shape as login/register).

Behaviour (idempotent):

1. Look up `User` by `guest_id`.
2. If found **and** `is_temporary` → issue a fresh session (like login) and
   return it. (Covers token expiry / re-visits: the same browser keeps its
   workspace.)
3. If found but **not** temporary (shouldn't happen — see §5.2 clears the
   id) → 400 `guest_already_adopted`.
4. If not found → create the temp user (`is_temporary=true`, placeholder
   email, `DUMMY_HASH`, `display_name="Guest"`, `created_at=now`), issue a
   session, return it.

Validation: missing/blank `X-Guest-Id` → 400 `guest_id_required`.

### 5.2 `POST /auth/register` — upgrade in place when a guest id is sent

Same endpoint, new optional behaviour: if the `X-Guest-Id` header is
present, register **upgrades** the mapped temp user instead of creating a
fresh user:

1. Find the temp user by `guest_id` (404 `guest_not_found` if absent).
2. Run today's field validation (display_name / email / password).
3. Run today's `email_taken` check (`User::filter_by_email`) — **reject with
   `email_taken` when the email belongs to any real account**. Merging into
   existing accounts is explicitly out of scope (§11): the temp workspace
   stays under the guest id; the user can register with a different email.
4. Update **the same row**: `email`, `password_hash`, `display_name`,
   `is_temporary = false`, `guest_id = NULL`.
5. Issue a fresh session token (old guest sessions are deleted — revoke the
   temp user's prior `Session` rows, then create one). Return `{ token,
   user }`.

No `X-Guest-Id` header → today's plain register, unchanged.

> Rationale (interview): in-place upgrade means companies / jobs / filings /
> balance sheets / ledgers never change ids — nothing is re-pointed, the
> worker's rows keep working, and the just-issued session is the one the app
> keeps.

### 5.3 `POST /auth/login` — unchanged

Temp users cannot log in (dummy hash; placeholder email isn't guessable).
Plain login does **not** adopt a guest workspace (interview: only register
adopts it). A logged-in user's guest workspace remains tied to its guest id
and reappears when they sign out — the web app never deletes `tally.guest.v1`.

### 5.4 `GET /auth/me` — unchanged

Returns the user including `is_temporary`. The web app uses it to render the
guest affordances (§9.5).

### 5.5 Auth extractor — unchanged

Once a guest has a session token (§5.1), every handler works through the
existing `AuthUser` bearer path. **One auth path** for all users (interview
decision). No handler needs to know about guest ids.

## 6. Company endpoints

### 6.1 `POST /companies` — accept `accounting_standard`

`CompanyInput` gains `accounting_standard?: 'FRS 105' | 'FRS 102'` (validated,
default `'FRS 105'`). `create` and `patch` persist it (config wins as usual).
The web `AddCompanyDialog` already collects `standard` via
`NewCompanyInput.standard` — wire it through for **both** signed-in and guest
adds (interview decision).

### 6.2 Jobs — identical semantics for temp users

A guest's company with a CH number + configured key enqueues `fetch_filings`
exactly like a signed-in user's (interview decision). The worker fetches the
full history + balance sheet; those rows survive the in-place upgrade
unchanged because company ids never change.

### 6.3 Everything else — unchanged

`list / get / patch / delete / enrich / filings / ledgers / reports` already
scope by `user.id`; a temp user's `user_id` is just a different user. The
`delete` cascade (jobs / filings / balance sheets / ledgers + files) applies
to temp-owned rows as-is — the sweep (§8) reuses that cascade.

## 7. Web changes (`apps/tally-web`)

### 7.1 Guest id storage

- New key `tally.guest.v1` (mirrors `tally.token.v1`), a random UUID
  generated on first use (`crypto.randomUUID()`).
- New `guest.ts` module: `getGuestId()`, `ensureGuestId()` (generate +
  persist), plus the `X-Guest-Id` header helper.
- Never deleted by the app (survives sign-in/out; it is the browser's stable
  anonymous identity).

### 7.2 Bootstrap: guest session is created lazily

On the first ownership-needing action — the user opens the Add-company
dialog, or (if they went straight at it) submits an add — while there is no
valid session token:

1. `ensureGuestId()`
2. `POST /auth/guest` with `X-Guest-Id` → store the returned token via
   `setToken`, set session to `signed-in` with `user.is_temporary`.
3. Proceed with the add.

Not on app boot (interview decision: no junk rows for browsers that only
browse the demo).

### 7.3 Session model (`session.ts`)

- `restoreSession()` is unchanged in shape: a stored token resolves via
  `/auth/me`; a temp-user token resolves the same way and the user object
  carries `is_temporary: true`.
- The UI treats `signed-in && user.is_temporary` as **guest mode**.

### 7.4 Guest mode becomes API-backed end-to-end

`App.tsx::addCompany` no longer branches to localStorage:

- Guest mode (signed out with a temp session): `POST /companies` with
  `accounting_standard` — the company is created server-side, the backfill
  job spawns, and the response row is added to the picker.
- The localStorage `Company` rows (camelCase mock shape) are **replaced by
  real API `Company` rows** (snake_case) for user-added companies. The
  picker/views normalise via a small adapter (API `Company` → the internal
  display shape) so the existing views keep working.
- The **demo company stays client-side only** — a mock, never a DB row,
  always first in the picker with the Demo badge (interview decision).
- `db.ts` keeps the demo `sources` seeding; the user-company part of the DB
  is dropped (or kept only as a display cache — see §12, open question).

### 7.5 Offline outbox

While the API is unreachable, adds are queued in a localStorage outbox
(`tally.outbox.v1`, an array of `NewCompanyInput`), the UI shows the existing
offline banner, and the add surfaces "saved, will sync when you're back
online". On reconnect (successful `restoreSession`/`/auth/guest`, or app boot
with connectivity), the outbox replays in order with the current auth (guest
or, after adoption, the upgraded account); success removes the entry, a hard
failure keeps it and toasts. (Interview decision: localStorage outbox, not a
server-side one.)

### 7.6 Registration dialog (`SignInDialog`)

- Register mode becomes **"Create account" adoption**: on submit, send
  `X-Guest-Id` (when the browser has one) with `POST /auth/register`. On
  success, session is `signed-in` (temp flag cleared) and the workspace is
  already server-side — **no migration phase**.
- `email_taken` surfaces inline as today.
- The **`migrateCompanies` / §7.3 copy loop is removed** (interview decision:
  replaced by guest adoption). The sidebar "Retry migration" button and
  `toastMigration` go away. Legacy localStorage rows from older builds are
  not migrated — see §12 (transition note / open question).

### 7.7 Guest affordances in the UI

While `signed-in && user.is_temporary` (interview decision: a visible guest
state, not today's local-mode UI):

- The sidebar auth footer shows a **"Save your work — create account"**
  primary affordance (name/email/password) instead of the plain "Sign in"
  button; a subtle "Guest workspace · stored on this browser" caption.
- The sign-in dialog defaults to the register/create-account tab when opened
  from the guest state.
- Demo banner logic (§4 of first-run-onboarding-spec) is unchanged — the
  demo company still drives it.

### 7.8 `api.ts`

- `api()` unchanged (token-based auth path).
- New: `bootstrapGuest(guestId)` → `POST /auth/guest` with the header.
- `register()` gains an optional `guestId` header parameter.
- `Company`/`CompanyInput` types gain `accounting_standard`.
- `createCompany` callers pass `accounting_standard`.

## 8. Sweep of abandoned temp users

- **Home**: once at API startup (interview decision — no timer), right after
  migrations apply in `main.rs` (before the worker spawns). Cheap: a couple
  of queries, runs in milliseconds.
- **TTL**: 90 days since last activity.
- **Activity**: `MAX(updated_at)` across the temp user's owned rows —
  `companies.updated_at` (new), `jobs.updated_at`, `filings.fetched_at`,
  `balance_sheets.created_at`, `ledgers.uploaded_at` (interview decision:
  reuse owned-row timestamps, no `last_active_at` column).
- **Action**: hard delete — the temp user, its sessions, and its companies
  with the existing cascade (jobs, filings, balance sheets, ledgers + stored
  ledger files). Reuses the `delete` handler's cascade (§6.3).
- Real users are never touched (only `is_temporary = true`).

## 9. Edge cases

| Case | Behaviour |
|---|---|
| Guest adds 2+ companies | Same guest id → same temp user → both companies owned by it |
| Token expires / browser re-visits | `POST /auth/guest` re-issues a session for the existing temp user (idempotent) |
| Register with an email that exists | `email_taken`; temp workspace stays under the guest id (no merge) |
| Register with a fresh email | In-place upgrade; all owned rows keep their ids; jobs/filings intact |
| Login with an existing account | Adopts nothing; guest workspace remains under the guest id, reappears after sign-out |
| Two browsers on one machine | Two guest ids → two independent temp workspaces |
| Guest deletes their only company | Works via the normal delete cascade; the temp user row remains until the sweep (or register) |
| Offline add | Outbox queues it; replayed on reconnect |
| API down at first add | `/auth/guest` fails → outbox path (guest id already persisted) |
| CH key absent | No backfill job — same as today; Refresh once configured |
| Guest registers, then opens another browser | New guest workspace — adoption is per-browser by design |

## 10. Verification plan

Backend (`cargo test -p tally-api`, real Postgres):

1. `POST /auth/guest` twice with the same id → same user id, fresh tokens.
2. Guest `POST /companies` (CH number + key) → company row + `fetch_filings`
   job enqueued; worker completes; balance sheet parsed.
3. Register with the guest header + fresh email → same company id, flag
   cleared, guest sessions revoked, `/auth/me` returns the real user.
4. Register with the guest header + taken email → `email_taken`, workspace
   intact.
5. Register without the header → today's plain register.
6. Login as the temp user's placeholder email → `invalid_credentials`.
7. Sweep: seed a temp user with old `updated_at` → startup deletes it + owned
   rows; a recent one survives; real users untouched.
8. `accounting_standard` round-trips through create/PATCH.

Web:

- `pnpm --filter @tally/web typecheck`; `smoke` + `flow` jsdom scripts.
- Manual (dev stack): signed out → add a company → company + filings appear
  from the API; kill the API → offline add queues; restart → replays;
  register → the company is still there with no copy loop.

## 11. Out of scope

- Merging a guest workspace into an **existing** account (register with a
  taken email rejects; login adopts nothing).
- Server-side offline queue / durable outbox (web localStorage outbox only).
- Periodic (timer-based) sweep — startup-only for now.
- A real `last_active_at` column (owned-row timestamps suffice per interview).
- CH-quota shielding for guest workspaces (jobs run identically).## 12. Open questions / transition notes

1. **Legacy localStorage companies** — **RESOLVED (dev build)**: this is a dev
   version with no real users yet, so no legacy-migration handling is
   implemented. Older builds' localStorage rows simply become irrelevant; the
   developer resets browser state. The §7.3 copy loop is still removed.
2. **`db.ts` user-company part**: keep as a display cache or delete entirely
   (demo sources remain). Implementation can decide once the adapter (§7.4)
   lands.
3. **Sweep threshold**: 90 days is a default; pick a number, or make it an
   env var (`TALLY_GUEST_TTL_DAYS`) — confirm preference.
4. **`display_name` for guests**: "Guest" is the placeholder — a nicety could
   let the add-company dialog collect a name for the temp user. Not
   required.
