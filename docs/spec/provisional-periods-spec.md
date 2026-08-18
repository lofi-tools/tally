# Provisional financial periods (pre-fetch) — spec

Status: drafted from interviews (5 rounds) — no code changes yet.
Scope: backend `apps/tally-api` (`filings.rs` derivation) + web `apps/tally-web`
(Filings view). No migration needed (the `periods` shape lives in the response,
not the schema).
Related: `docs/spec/ch-filings-sync-spec.md` (§5 response shapes, period
derivation), `docs/spec/temp-user-spec.md`.

## 1. Problem

Right after a company is created, its filing history usually hasn't been
fetched yet (no CH key configured, no company number, the worker hasn't run, or
CH is slow/failing). Today `GET /companies/{id}/filings` then returns almost
nothing:

- with a registration date but nothing fetched → a single `ongoing` period;
- without a registration date and nothing fetched → `[]`, and the web Filings
  view shows its empty nav state ("No periods yet — they appear once Companies
  House has a record for this company.").

The user wants the financial-period **structure** visible immediately — the
full list of financial years the company has had since incorporation, deduced
from the registration-date ARD schedule (the same schedule `period.rs` /
`reports::company::Company::accounting_period_n` uses) — so the periods are
available for calculation/report work even before any fetch. Each unconfirmed
period is marked **provisional** and the UI signals that it still needs
fetching.

## 2. Terminology

| Term | Meaning |
|---|---|
| schedule period | One ARD-anniversary period from the registration-date schedule (`accounting_period_n`: n=0 incorporation→first year end, n=1 first-year-end→first ARD, n≥2 ARD anniversaries) |
| provisional | A period whose dates are **estimated from the registration date** — no Companies House data has confirmed them yet (structure-only: start/end + label) |
| enrichment | A successful fetch re-derives the periods from CH data, turning provisional rows into real ones (`filed` / `pending`) |
| invalidation | A fetched filing whose dates **overlap/conflict** with a provisional row (shortened period, changed ARD, late filing) — the CH-derived period replaces the estimate |
| coverage | Whether a period has been *seen* by a completed fetch: a period ending at/before the last successful fetch's completion time was checked against CH; one ending after it (or when no fetch ever completed) was not |

## 3. Current state (verified facts)

- `reports::company::Company::accounting_period_n(n)` walks the full schedule from
  the registration date (`period.rs`, `filings.rs` both use it). Pure — no
  fetch needed.
- `filings.rs::derive_periods(company, balance_sheets, filings)` today:
  - finds the **ongoing** period (schedule period containing today — needs a
    registration date);
  - collects **filed ends** (stored `balance_sheets` period ends + accounts
    filings' derived period ends);
  - when nothing is filed and there's no registration date → `[]`;
  - when nothing is filed but a registration date exists → `[ongoing]` only
    (deliberately *not* the whole history — a comment says so);
  - when history exists → walks the schedule **from the earliest filed end**
    (skipping pre-history periods) up to today: `filed` (confirmed) /
    `pending` (ended, no confirmed filing, between the most recent filed and
    ongoing — with `due` + `not-sent` Accounts/CT600 rows) / `ongoing`.
- `GET /filings` returns `{ periods, balance_sheets, status: FetchStatus }`
  where `FetchStatus.state ∈ none | pending | running | done | failed` and
  `fetched_at` is the last *done* job's `updated_at`.
- The web Filings view (`Filings.tsx`): `RealFilingsView` renders the two-pane
  shell; the sub-nav shows one row per period (green tick `filed` / yellow "!"
  `pending` / `Ongoing` badge); a `navEmpty` note when there are no periods;
  header Refresh icon; syncing + failed banners. `PeriodNavRow` and
  `PeriodDetail` are shared with the demo view.

## 4. Design — per-row provisional periods

Provisional is a **per-row** status, not a global mode. The backend always
walks the full schedule (incorporation → today) and labels each ended period
according to what CH data has confirmed. Background fetches then *enrich* the
list (provisional → real) or *invalidate* rows whose estimated dates conflict
with a real filing.

### 4.1 Period statuses (four, was three)

`Period.status` gains a fourth value; the legend becomes:

| status | Meaning | Shown |
|---|---|---|
| `filed` | confirmed accounts filing at CH (balance-sheet row or accounts filing matched by period end) | green tick |
| `pending` | ended, **covered by a completed fetch** (CH was asked and has no accounts filing for it) — genuinely unfiled, actionable | yellow "!" + *pending* |
| `ongoing` | the schedule period containing today | `Ongoing` badge |
| `provisional` | ended, **not covered** by any completed fetch (never fetched, or its end falls after the last successful fetch) — dates are an estimate from the registration date | per-row *Provisional* indicator |

### 4.2 Derivation rules (`derive_periods`)

Replace the current logic with:

1. **Walk the whole schedule** from incorporation (`n=0`) through today — not
   just from the earliest filed end. (Deliberate change: pre-history periods
   now appear; the "skip pre-history" behaviour is gone — see §7 edge cases.)
2. Status per period:
   - the period containing today → **`ongoing`** — always functional: `due`
     (end + 9m CH / + 12m HMRC) and the expected `not-sent` Accounts + CT600
     rows, exactly as today;
   - a period whose end matches a confirmed filing (balance sheet / accounts
     filing) → **`filed`**;
   - any other ended period → **`pending`** when its end is covered by the last
     completed fetch (i.e. `end <= fetched_at` of the latest *done* job, or a
     done job exists and the period predates it), else **`provisional`**.
3. `provisional` periods are **structure-only**: `start` + `end` + label, no
   `due`, no `filings` (empty array). `pending` periods keep today's content
   (`due` + `not-sent` rows). `filed`/`ongoing` unchanged.
4. A company with **no registration date** and nothing fetched keeps `[]` (the
   schedule needs an anchor) — the web empty state stays. A company with no
   registration date but filed history keeps the current behaviour (periods
   anchored by the filed ends).
5. Newest-first ordering and the start-edge computation (day after the next-older
   end) are unchanged.

**Coverage marker**: `derive_periods` needs the last successful fetch's
completion time. The `list` handler queries the most recent **`done`** job's
`updated_at` (its `fetched_at`) — *not* the latest job — and passes it in. A
company with no `done` job ever has `fetched_at = None` → every unconfirmed
ended period is `provisional`.

### 4.3 Enrichment and invalidation (no new machinery)

No new background task: the existing `fetch_filings` job, when it completes, is
what enriches. The derivation re-runs on every `GET /filings` (or a poll after
a refresh) with the new filings/balance sheets + the coverage anchor:

- **Enrichment** — a provisional row whose end **matches** a confirmed filing
  (balance-sheet `period_end`, or an accounts filing's period end parsed from
  the CH item's `description_values` — the typed `TypedFiling` parse in
  `ct600`, not the human-readable description) becomes `filed`; a
  covered-and-unfiled ended period (`end <= fetched_at`) becomes `pending`.
  Nothing more is needed — the status is re-derived per read, so a completed
  fetch flips the rows on the next load, and the web's mini-banner disappears.
- **Invalidation** — a confirmed filing whose dates conflict with an estimate
  (shortened first period, changed ARD, late filing) **drops** the estimate;
  the CH-derived end drives the list. Concrete rule (in the pure core
  `derive_periods_from`): the schedule is walked once as real `[start, end]`
  periods, and a non-filed, non-ongoing estimate is dropped when any confirmed
  filing's implied span `[filed_end − 12m + 1d, filed_end]` (the most CH's
  data gives for the filing's range) **overlaps** it. A filing ending exactly
  on the estimate's end enriches it to `filed` instead of dropping it.

  The 12-month span is deliberately bounded: a genuinely unfiled year is not
  overlapped by its neighbours' spans, so it survives as `pending` once covered
  (spec §7 — accurate: it was never filed). The heuristic's one known
  imprecision is a *shortened mid-life* filing (less than 12 months, e.g. an
  ARD change) whose implied span can over-reach backward and drop the estimate
  immediately before it; such a period reappears as `filed` as soon as it is
  itself filed.

### 4.4 Response shape

- `Period.status: 'filed' | 'pending' | 'ongoing' | 'provisional'` (new
  variant only — no field changes).
- `FetchStatus` unchanged; the web derives "missing periods" as
  `periods.filter(p => p.status === 'provisional').length > 0`.
- No migration: the periods shape is response-only.

## 5. Backend changes (`apps/tally-api/src/filings.rs`)

1. `derive_periods(company, balance_sheets, filings, fetched_at: Option<&str>)`
   — walk the full schedule; apply §4.2 status rules; structure-only
   `provisional` rows; §4.3 invalidation.
2. The derivation core is the pure, wasm-portable `derive_periods_from(reg
   date, filed ends, coverage)` (§10); `derive_periods` is the thin
   model-reading wrapper that also overlays the real CH filings onto
   non-provisional rows.
3. `list` handler queries the most recent `done` job's `updated_at` (the
   coverage anchor) — not the latest job, so a failed refresh doesn't revert
   covered rows (§7).
4. Unit + pg tests for the derivation and the invalidation rules (§8).

## 6. Web changes (`apps/tally-web/src/views/Filings.tsx`)

### 6.1 Sub-nav rows (`PeriodNavRow`)

- New `provisional` branch in the status indicator: a muted/dashed treatment
  with a small **Provisional** tag (distinct from the green tick / yellow "!"
  / `Ongoing` badge) — the row reads as an estimate.
- The `Period` type in `api.ts` gains `'provisional'` in its `status` union.

### 6.2 Mini-banner + fetch button inside the sub-nav column

- Rendered at the top of the **sub-nav column** (inside `<nav>`, above the
  period rows), visible while **any** period is `provisional`:
  - compact copy, e.g. "Some periods are estimated — filing history hasn't been
    fetched for them yet."
  - a small **Fetch / Refresh** button → `refreshFilings(companyId)` → the
    existing syncing state + poll (reuses `onRefresh` / `startPolling`).
- **Both disappear when no periods are missing** (no `provisional` rows) —
  including during/after a successful fetch (the derivation replaced the
  estimates).
- The header's existing Refresh icon button stays.

### 6.3 Detail pane for a selected provisional period

- Header: period label + range + a **Provisional** badge.
- A note (replacing the empty-state), e.g. "This period is estimated from the
  registration date — filing details appear once Companies House history is
  fetched."
- No `due` dates, no filings list, no Prepare/Preview or File actions
  (structure-only).
- The `ongoing` period in the unfetched state keeps its current functional
  pane (due dates + "To file" rows + actions).

### 6.4 Growing list over time

Because every load walks the schedule up to today, a year whose period end
passes *after* the last successful fetch appears as a new `provisional` row
automatically (no reload of CH needed) — this is the "with time passing, the
UI adds more unsynced financial years" behaviour. The mini-banner reappears
while such rows exist.

## 7. Edge cases

| Case | Behaviour |
|---|---|
| No registration date, nothing fetched | `[]` — existing empty nav state; no provisional rows → no mini-banner |
| No registration date, filed history | Periods anchored by filed ends (current behaviour); unconfirmed ended periods fall back to `provisional` (no schedule to date them) |
| Company with CH number + key | Job spawns at create; while `pending`/`running` the periods are provisional; on `done` the next read re-derives (enrich/invalidate) |
| No CH key / no number | No job ever spawns; state stays `none`; provisional list + mini-banner persist; the banner's Fetch button surfaces the existing `companies_house_key_missing` error path |
| Refresh fails after a prior success | Covered periods stay `filed`/`pending` (the per-row model — no global revert); only periods ending after the last success are provisional; the failed banner shows as today |
| Old company, many years | All periods since incorporation appear; the sub-nav scrolls; pre-history unfiled periods are `pending` once covered by a fetch (accurate — they were never filed) |
| Shortened first period | The real filing (e.g. first accounts running to the ARD) overlaps the n=0 estimate → the estimate is dropped, the filed end anchors (§4.3) |
| Changed ARD mid-life | Phantom schedule ends between the real filed ends are overlapped by the filings' spans → dropped; the real ends drive the list (§4.3) |
| Genuinely unfiled year | Neighbouring filings' 12-month spans do **not** overlap it → it survives and is `pending` once covered (accurate — never filed) |
| Filing ending exactly on an estimate | Enrichment, not invalidation: the row becomes `filed`; neighbours untouched |
| Period end passes while unfetched | New provisional row appears on the next load (§6.4) |
| Deletion / ownership | Unchanged (ownership-scoped handler) |

## 8. Testing / verification

**Chosen approach (interview): simplest — no-CH-key company, plus tests.**

Manual (dev stack, `COMPANIES_HOUSE_API_KEY` unset):

1. Add a company with a CH number (search → add). No job spawns (no key);
   `GET /filings` returns `status.state = 'none'`.
2. The Filings view shows the **full provisional list** (all years since
   incorporation): past rows with the per-row Provisional indicator, the
   current year `Ongoing` with due dates + To-file rows, and the mini-banner +
   Fetch button in the sub-nav.
3. Clicking Fetch → `companies_house_key_missing` → the existing failed-banner
   path.
4. Restart the API with the key set, Refresh → job runs → poll → periods
   re-derive to `filed`/`pending`/`ongoing`; provisional rows gone; mini-banner
   disappears.

Backend tests (`cargo test -p tally-api`):

- unit: `derive_periods_from` with a registration date and no history → all
  schedule periods (n=0..today), past = `provisional`, current = `ongoing`
  with due + not-sent rows; provisional rows have empty `filings` and no `due`;
- unit: with coverage after a period end and no filing → `pending`; with
  coverage before the period end → `provisional`;
- unit invalidation: shortened first period drops the n=0 estimate and anchors
  the real end; changed ARD drops the phantom mid-life estimates; a genuinely
  unfiled year survives as `pending`; a filing ending exactly on an estimate
  confirms it (`filed`) without disturbing its neighbours;
- pg integration, no-CH-number company (deterministic — no job can spawn):
  `GET /filings` returns exactly the schedule periods, `status.state = 'none'`,
  ended periods `provisional` structure-only, one functional `ongoing`, zero
  `pending`;
- pg integration, enrichment + invalidation round-trip: a done job + stored
  balance sheet at a changed-ARD end → that end is `filed`, the overlapping
  estimates are gone, covered ends are `pending`, uncovered ends stay
  `provisional`, the ongoing period stays functional;
- pg integration: a later failed refresh does not revert covered periods (the
  coverage anchor stays the last `done` job).

Web: `pnpm --filter @tally/web typecheck` + the `smoke`/`flow` jsdom scripts
(demo view untouched; the provisional path needs the API, so the jsdom check is
render-only for the real view's new status branch — add a canned `GET /filings`
fixture to `flow.mjs` if it should exercise the provisional render).

## 9. Out of scope

- Adding `due` dates / expected filings to provisional rows (structure-only by
  decision).
- A global "provisional mode" that replaces the whole list (per-row by
  decision).
- Automatic refetch when new years complete (manual Refresh / the create-time
  job only — the list just shows the estimate until then).
- Changes to `pending` semantics or the `fetch_status` shape.
- Any new DB columns or jobs (response-shape only).

## 10. Design note — wasm in the frontend (future)

Decision (interview): the backend derives the periods **for now** — it is the
single source of truth with one bug surface, and the schedule computation needs
no fetch. Note for later: the rust library (`libs/reports` company module) is
planned to be compiled to **wasm** and run the period derivation in the
frontend itself, removing the round-trip for this pure computation. The
derivation must therefore stay a **pure function of (registration date, filed
ends, coverage)** — no DB/state coupling — so the same code can run in both
places.

## 11. Decisions log (interviews)

1. Derivation lives in the **backend** `derive_periods`; wasm-in-frontend is a
   noted future switch (§10).
2. Provisional list = **all periods since incorporation** (every ARD-anniversary
   period up to and including the ongoing one).
3. Representation: **new per-period status `'provisional'`** (past periods are
   provisional, not pending); the current period stays `ongoing`.
4. Provisional applies to **any period not covered by a completed fetch**
   (never fetched, or ended after the last successful fetch) — per-row, not a
   global mode; a failed-after-success refresh does **not** revert confirmed
   rows.
5. Provisional periods are **structure-only** (start/end + label; no due, no
   filings); the `ongoing` period keeps its due dates + To-file rows even in
   the unfetched state.
6. `pending` is **kept** for covered-and-unfiled ended periods (actionable);
   `provisional` is only the unconfirmed estimate state.
7. UI: **per-row Provisional indicator** + a **mini-banner with a Fetch button
   inside the sub-nav column**; both disappear when no periods are missing.
8. Detail pane for a provisional period: header + badge + "estimated from the
   registration date" note; no actions.
9. Enrichment/invalidation happen via the existing fetch job re-running the
   derivation (§4.3).
10. Testing: **no-CH-key company** manual check + unit/pg tests (§8); no
    worker-disable flag.
