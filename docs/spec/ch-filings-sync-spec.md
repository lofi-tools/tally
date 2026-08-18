# CH filings sync + balance-sheet backfill spec

Status: draft (interviewed; no code changed yet)
Scope: `apps/tally-api` (primary), `libs/ct600` (small extension), `apps/tally-web` (Filings view wiring)
Related: `docs/spec/api-backend-spec.md`, `docs/spec/web-api-wiring-spec.md` (§10 Filings)

## Summary

When a company is added to the API, its full Companies House filing history should
be fetched **asynchronously** (never blocking `POST /companies`), persisted, and the
most recent filed accounts document should be downloaded and parsed so the app can
auto-fill the balance sheet. The API currently has **no background-task system at
all** (no `tokio::spawn`, no job queue, no scheduler), so this feature also stands
up the durable job infrastructure that later work (report generation, bank sync)
will reuse.

The financial model is: two historical sources — **balance sheets** (from CH's filed
accounts, one snapshot per filed period, with the raw + parsed document stored) and
**transactions** (the app's own ledger uploads — CH exposes no transaction API).
The current financial status is **computed on the fly**: the latest filed CH balance
sheet as the opening anchor (CH wins) plus the ledger transactions dated after that
balance-sheet date.

## Decisions (from interview, 2026-08)

1. **Durable execution: DB job table + tokio worker.** No external framework — a
   `jobs` table in Postgres (status, attempts, last_error, next_retry_at) plus a
   tokio worker; startup re-claims unfinished jobs so work survives restarts. This is
   the honest weight for a single fetch task (Temporal / Restate / Inngest / queues
   were researched — nothing Rust-native or Postgres-native fits this scale).
2. **Progress/errors persisted**: full job table (pending/running/done/failed,
   attempt count, last_error), not just a per-company flag.
3. **Structured concurrency**: the worker uses `tokio::task::JoinSet` +
   `CancellationToken` (bounded fan-out, clean shutdown); jobs are claimed via
   `SELECT … FOR UPDATE SKIP LOCKED`.
4. **Store all filing history** — accounts, confirmation statements, everything —
   and **fetch all pages** (follow CH pagination until `items.len() == total_count`
   or the pages run out).
5. **Triggers**: enqueue on `POST /companies` (only when a company number is present
   and a CH key is set) and on a new dedicated endpoint
   `POST /companies/{id}/filings/refresh`. No lazy refetch on `GET`.
6. **Failure: fail fast.** One attempt; the error is persisted on the job; the
   manual Refresh action re-runs it. (The job table keeps `attempts` /
   `next_retry_at` so a backoff policy can be turned on later without a migration.)
7. **Auto-fill balance sheet: full pipeline now.** Download the most recent filed
   accounts document from CH (new document-content endpoint in the ct600 lib), unzip
   the iXBRL when needed, parse via `Frs105Accounts::from_ixbrl()`, and persist a
   balance-sheet history row.
8. **Data model**: new `filings`, `balance_sheets`, `jobs` tables; `transactions`
   stays the existing ledger model (the only transaction source).
9. **Current status computed on the fly** — no materialized "current" row: Accounts
   view / report derive it from (latest `balance_sheets` row) + (ledger transactions
   dated after its period end).
10. **Web: full Filings view from the API, as a two-pane layout** — a left
    sub-nav lists the company's financial periods (descending); the right pane
    shows the selected period's filings (confirmed by CH/HMRC and pending ones
    not sent yet). Periods are derived server-side from the CH filing history +
    the registration-date schedule (the period resolution already built in
    `period.rs`).

## Current state (as of writing)

### API — `apps/tally-api`

- **No async infrastructure.** `tokio = full` is available; nothing is spawned.
  `main.rs` is: env → connect DB → migrations → serve.
- **Migrations exist** (`src/migrations.rs`, embedded SQL in `migrations/`,
  `_migrations_history` tracking) — new tables slot straight in as `0002_*.sql`.
- **Companies House**: `ChApi` (concrete struct, `from_env()`) wraps the ct600 lib's
  client. `libs/ct600` already has `get_filing_history` → `FilingHistory`
  (`total_count`, `items: Vec<FilingHistoryItem>`; item = `date`, `category`,
  `form_type`, `description`, `links { self, document_metadata }`), plus
  `FilingHistory::accounts()` and a disk cache (`CT600_CACHE_DIR`). **No document
  content fetch** exists, and **pagination is not implemented** (first page only).
- **Models** (toasty): User, Session, Company, Ledger, Account, Transaction, Split.
  No Filing / BalanceSheet / Job.
- **Companies**: `create()` validates name, dedupes by number, enriches profile from
  CH when a number + key are set (`enrich_from_ch`), `create_row` inserts. `POST
  /companies/{id}/enrich` fills blank profile fields. Ownership helper `owned_company`
  is shared.
- **Periods**: `period.rs` resolves explicit `period` → `made_up_to` → CH
  next-accounts (key set) → registration-date schedule (no key needed).
- **Reports**: `Frs105Accounts::new(&book, &company, &profile, &meta)` computes the
  balance sheet (current + previous year) from the ledger book alone; `from_ixbrl()`
  round-trips a filed iXBRL document back into the struct. Four report endpoints;
  UTR gate on corp-tax/CT600 (from the add-company-simplification change).

### Web — `apps/tally-web`

- Filings view is **fully mock**: `previousFilings`, `nextFiling`, `financialYears`
  FY picker from `mock_data.ts`; Preview/File now are toasts. Spec §10 says the
  history table + FY picker "stay mocked (no filing-history endpoints)".
- Accounts view is mock-derived from the ledger; `api.ts` has no filings/balance-sheet
  client functions.

## Changes — API

### 1. Migration `0002_filings_jobs.sql` (committed, via `migrations.rs`)

- **`jobs`** — `id uuid pk`, `kind text` (`fetch_filings`), `company_id uuid fk`,
  `status text` (`pending|running|done|failed`), `attempts int default 0`,
  `last_error text null`, `next_retry_at timestamptz null`, `created_at`,
  `updated_at`. A **partial unique index** `(kind, company_id) WHERE status IN
  ('pending','running')` dedupes concurrent enqueues.
- **`filings`** — `id uuid pk`, `company_id uuid fk`, `ch_transaction_id text`
  (parsed from `links.self`), `category text`, `form_type text`, `description text`,
  `filed_on date`, `document_metadata_url text`, `raw jsonb` (the full original CH
  item), `fetched_at timestamptz`, `unique (company_id, ch_transaction_id)` (idempotent
  re-fetch upserts).
- **`balance_sheets`** — `id uuid pk`, `company_id uuid fk`, `period_end date`,
  `filed_on date`, `source_filing_id uuid fk → filings`, `figures jsonb` (the
  single filed period's line items in the exact `PreviousYearFigures` shape,
  §6a), `raw_document bytea`, `parsed_document text` (the iXBRL), `created_at`,
  `unique (company_id, period_end)`.

### 2. Models + registration

- New toasty models `Job`, `Filing`, `BalanceSheet` (in `src/models.rs`), registered
  in the `toasty::models!(…)` list in `main.rs` **and** `tests/common/mod.rs`
  (same pattern as the existing models).

### 3. `src/jobs.rs` — the durable worker (lifecycle sketch)

**Dependency**: add `tokio-util = { workspace = true, features = ["sync"] }` to
`tally-api` — the workspace has `tokio-util = "0.7"` with no features enabled, and
`CancellationToken` lives behind the non-default `sync` feature. (Fallback with
zero new deps: a `tokio::sync::watch`/`Notify` shutdown flag, but the token gives
child propagation for free.)

**Enqueue** (called from `create()` and the refresh endpoint) — the partial unique
index `(kind, company_id) WHERE status IN ('pending','running')` makes a duplicate
in-flight enqueue a no-op; a done/failed job is freely re-enqueued (that is what
Refresh does):

```rust
pub async fn enqueue(db: &mut toasty::Db, kind: &str, company_id: uuid::Uuid) -> Result<(), JobError> {
    toasty::sql::statement(
        r#"INSERT INTO "jobs" ("id", "kind", "company_id", "status", "attempts", "created_at", "updated_at")
           VALUES ($1, $2, $3, 'pending', 0, now(), now())
           ON CONFLICT DO NOTHING"#,
    )
    .bind(uuid::Uuid::new_v4())
    .bind(kind)
    .bind(company_id)
    .exec(db)
    .await?;
    Ok(())
}
```

**Claim** — `SELECT … FOR UPDATE SKIP LOCKED` inside a transaction: the lock is
held only for the claim (flip to `running` + commit), never for the job's
execution, so a crash mid-job leaves a `running` row that the next boot re-claims.
Rows are read as `toasty::stmt::Value::Uuid` / `::String` (the same raw-SQL
pattern as `migrations.rs`):

```rust
async fn claim_jobs(db: &mut toasty::Db, limit: usize) -> Result<Vec<ClaimedJob>, JobError> {
    let mut tx = db.transaction().await?;
    let rows = toasty::sql::query(
        r#"SELECT "id", "kind", "company_id" FROM "jobs"
           WHERE "status" = 'pending'
           ORDER BY "created_at" ASC
           LIMIT $1
           FOR UPDATE SKIP LOCKED"#,
    )
    .bind(limit as i64)
    .column_types([toasty::stmt::Type::String, toasty::stmt::Type::String, toasty::stmt::Type::String])
    .exec(&mut tx)
    .await?;

    let jobs = /* map rows: Value::Record → (Uuid id, String kind, Uuid company_id) */;
    for job in &jobs {
        toasty::sql::statement(
            r#"UPDATE "jobs" SET "status" = 'running', "updated_at" = now() WHERE "id" = $1"#,
        )
        .bind(job.id)
        .exec(&mut tx)
        .await?;
    }
    tx.commit().await?;
    Ok(jobs)
}
```

**Worker loop** — one task spawned from `main.rs`; structured concurrency via a
bounded `JoinSet` (each child gets a derived `CancellationToken`), and three
`select!` arms: shutdown, idle poll, and "a slot freed". Startup first re-claims
stale `running` rows (crash recovery — the durability guarantee):

```rust
const CONCURRENCY: usize = 4;                    // bounded fan-out
const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);

pub async fn run_worker(state: Arc<AppState>, shutdown: CancellationToken) {
    let mut db = state.db.clone();

    // Startup re-claim: 'running' rows are orphans from a previous process
    // (their lease died with it). Reset them to 'pending' so this boot retries.
    toasty::sql::statement(
        r#"UPDATE "jobs" SET "status" = 'pending', "updated_at" = now()
           WHERE "status" = 'running'"#,
    )
    .exec(&mut db)
    .await
    .ok(); // best-effort: a DB fault here shouldn't take down startup

    let mut workers = JoinSet::new();
    loop {
        tokio::select! {
            _ = shutdown.cancelled() => break,
            _ = tokio::time::sleep(POLL_INTERVAL) => {},
            _ = workers.join_next(), if !workers.is_empty() => {}, // slot freed
        }
        let free = CONCURRENCY.saturating_sub(workers.len());
        if free == 0 { continue; }
        for job in claim_jobs(&mut db, free).await {
            workers.spawn(run_job(state.clone(), job, shutdown.child()));
        }
    }

    // Graceful drain: children already hold cancelled tokens; await them to
    // empty (each exits at a safe point between steps).
    while workers.join_next().await.is_some() {}
}
```

**Run one job** — dispatch on `kind`, persist outcome, and observe the token
between steps (every CH call is wrapped in `select! { _ = token.cancelled() =>
Err(Cancelled), res = call => res }`):

```rust
async fn run_job(state: Arc<AppState>, job: ClaimedJob, token: CancellationToken) {
    let result = match job.kind.as_str() {
        "fetch_filings" => filings::fetch_and_store(&state, job.company_id, &token).await,
        other => Err(JobError::UnknownKind(other.to_string())),
    };
    let mut db = state.db.clone();
    let (status, err) = match result {
        Ok(()) => ("done", None),
        Err(e) => ("failed", Some(e.to_string())),
    };
    let _ = toasty::sql::statement(
        r#"UPDATE "jobs" SET "status" = $1, "last_error" = $2,
                "attempts" = "attempts" + 1, "updated_at" = now()
           WHERE "id" = $3"#,
    )
    .bind(status)
    .bind(err.as_deref())
    .bind(job.id)
    .exec(&mut db)
    .await;
}
```

Fail-fast policy: attempt 1 runs the task; any error lands in `last_error` and the
job is `failed` (no automatic retry today — `attempts`/`next_retry_at` stay
written for the future backoff policy).

**main.rs wiring** — start the worker alongside the server; one ctrl-c cancels
both (axum drains HTTP in flight, the worker drains its `JoinSet`):

```rust
use tokio_util::sync::CancellationToken;

let shutdown = CancellationToken::new();
let worker = tokio::spawn(jobs::run_worker(state.clone(), shutdown.child()));

axum::serve(listener, router(state))
    .with_graceful_shutdown(async move {
        let _ = tokio::signal::ctrl_c().await;
        shutdown.cancel();
    })
    .await
    .context("serve")?;
let _ = worker.await; // already draining via the token
```

### 4. `src/filings.rs` — the fetch task

1. **Fetch all pages** of `get_filing_history` (needs the lib extension below);
   upsert every item into `filings`.
2. Pick the **most recent accounts filing with a document link** (`category ==
   "accounts"`, `links.document_metadata` present).
3. **Download the document**: CH document API
   (`https://document-api.companieshouse.gov.uk`, same basic-auth key — a separate
   client/base from the main API). Fetch metadata → content URL → bytes.
4. **Unzip when needed** (CH serves accounts as zipped iXBRL or a single `.html`);
   if the result is PDF or otherwise unparseable, keep the raw bytes, mark the
   balance-sheet row as unparseable, and finish the job as done-with-partial (see
   Edge cases).
5. **Parse** via `Frs105Accounts::from_ixbrl(...)`; take the parsed document's
   **current-period column** (index `[0]`) and build a `PreviousYearFigures`
   (the shape of the `figures` column, §6a) as the historical balance sheet
   for that period end.
6. **Upsert** the `balance_sheets` row (figures, raw + parsed doc, source filing id).
7. Mark the job `done` (or `failed` with the error, per fail-fast).

### 5. Endpoints

- `GET /companies/{id}/filings` (owned) → `{ periods: Period[], balance_sheets:
  BalanceSheet[], status: FetchStatus }` — the web Filings view's data source
  (shapes detailed in Changes — web). `status` is the latest fetch job's state for
  the company: `{ state: 'none'|'pending'|'running'|'done'|'failed',
  fetched_at: string | null, last_error: string | null }` — so the UI can show
  syncing/error without a second call.

  **Period derivation (server-side)** — the company's financial periods, ordered
  newest first, from the §6 chain (explicit override → made_up_to → CH
  next-accounts → registration-date ARD schedule) plus the filed history:
  `Period = { start, end, status: 'filed'|'pending'|'ongoing', due: { hmrc, ch } |
  null, filings: PeriodFiling[] }` where:
  - **`ongoing`** — the period containing today (`accounting_period_containing`
    from the same chain used by `period.rs`); carries the `due` deadlines
    (`hmrc` = end + 12 months, `ch` = end + 9 months, via
    `ct600::next_accounting_period_from` when no CH profile, else the profile's
    next-accounts dates).
  - **`filed`** — a period whose end has a confirmed accounts filing (a
    `balance_sheets` row, or an accounts `filings` row whose derived period end
    matches the schedule); green tick in the UI.
  - **`pending`** — an ended period between the most recent `filed` period and
    `ongoing` with no confirmed accounts filing; yellow "!" in the UI.
  - `PeriodFiling = { kind: 'accounts'|'confirmation-statement'|'corporation-tax'
    |'other', state: 'confirmed'|'not-sent', filed_on?: string, form_type?: string,
    description?: string, document_metadata_url?: string }`. Confirmed items come
    from the CH history (matched to the period: accounts by period end, others by
    filing date); the outstanding expected items (`accounts`, `corporation-tax`)
    are added as `state: 'not-sent'` when the period has no confirmed match — the
    right pane renders both.
- `POST /companies/{id}/filings/refresh` (owned) → enqueues `fetch_filings`; returns
  `202` + the job id (or `200` no-op when one is already pending). Errors
  `companies_house_key_missing` when no key is configured.
- `create()` hook: after `create_row`, when `company_number` is non-empty and
  `state.ch` is set, enqueue the job (fire-and-forget — never awaited in the request
  path).

### 6. Current-status computation + report integration

- A shared function computes the on-the-fly current balance sheet: latest
  `balance_sheets` row (opening anchor) + the effect of the company's ledger
  transactions with `post_datetime > balance_sheet.period_end`, mapped to
  balance-sheet lines with the existing account-type logic
  (`is_balance_sheet_type` in `ledgers.rs` / `account_path` in the reports lib).
- **Report integration**: when a `balance_sheets` row exists, the accounts (and
  CT600) handlers apply its figures to the comparative/previous-year column via
  `Frs105Accounts::with_previous_year(...)` — CH wins when present, ledger-derived
  previous year stays as the fallback (§6a).

### 6a. `balance_sheets.figures` shape + the previous-year override

**JSON shape** — a flat object of the filed accounts' balance-sheet line items,
keyed exactly by the `Frs105Accounts` field names, values in **whole pounds** (as
filed — iXBRL renders at `decimals = 0`) with the iXBRL sign convention (creditor
lines negative). It deserialises directly into the `PreviousYearFigures` struct:

```json
{
  "fixed_assets": 10500.0,
  "current_assets": 38920.0,
  "prepayments_and_accrued_income": 0.0,
  "creditors_within_1_year": -4310.0,
  "net_current_assets": 34610.0,
  "total_assets_less_liabilities": 45110.0,
  "creditors_after_1_year": 0.0,
  "provisions_for_liabilities": 0.0,
  "accruals_and_deferred_income": 0.0,
  "net_assets": 45110.0,
  "capital_and_reserves": 45110.0
}
```

All 11 keys are always present (the four always-zero lines in the generated report
— prepayments, creditors-after-1-year, provisions, accruals — are still stored
explicitly so a future full-accounts parse round-trips honestly). The `BalanceSheet`
model maps the column as `toasty::Json<PreviousYearFigures>`.

The report's comparative column is then injected with a consuming builder on
`Frs105Accounts` (see Changes — `libs/reports`); `new()` is unchanged, so the CLI and
the ct600 test helpers compile untouched and only the tally-api handlers opt in.

### 7. Tests

- **jobs**: enqueue dedupe (unique pending index), claim/run/fail state transitions,
  startup re-claim of `running` jobs (offline unit tests, following the existing
  pattern).
- **filings persistence**: upsert idempotency on refetch; balance-sheet upsert
  keyed on `(company_id, period_end)`.
- **Document parse**: `from_ixbrl` on a committed fixture iXBRL document (libs/reports
  already round-trips; add a fixture if none is committed).
- **pg integration**: `POST /companies/{id}/filings/refresh` without a CH key →
  `companies_house_key_missing`; with a stubbed CH source → filings + balance sheet
  persisted. **Testability note**: `ChApi` is a concrete struct, and the pg harness
  builds `AppState { ch: None }`. To test the happy path without live CH, either
  (a) give `ChApi` a test constructor backed by a canned `reqwest`/mock server, or
  (b) abstract the fetch behind a small trait so tests inject a stub. Flag for the
  implementer — the harness change is small either way.

## Changes — `libs/ct600`

- `get_filing_history`: add pagination (page parameter and/or follow the CH links
  until exhausted) so the task can fetch all pages.
- **New document fetch**: document-metadata + document-content calls against the
  document API host, same API key; return the bytes (+ content type) for the
  downloading step.
- Keep the existing disk-cache behaviour (`CT600_CACHE_DIR`).

## Changes — `libs/reports`

- New `PreviousYearFigures` struct (in `libs/reports/src/reports/uk_frs105_accounts.rs`)
  mirroring the twelve `Frs105Accounts` balance-sheet fields as `f64` (single
  period), deriving `Debug, Clone, Default, PartialEq, Serialize, Deserialize`:

```rust
/// Previous-period balance-sheet figures — the comparative column of the
/// statement of financial position when the ledger doesn't cover the
/// previous period (e.g. sourced from the company's last filed accounts at
/// Companies House). Mirrors the `Frs105Accounts` balance-sheet fields;
/// values in whole pounds, iXBRL sign convention (creditors negative).
pub struct PreviousYearFigures {
    pub fixed_assets: f64,
    pub called_up_share_capital_not_paid: f64,
    pub current_assets: f64,
    pub prepayments_and_accrued_income: f64,
    pub creditors_within_1_year: f64,
    pub net_current_assets: f64,
    pub total_assets_less_liabilities: f64,
    pub creditors_after_1_year: f64,
    pub provisions_for_liabilities: f64,
    pub accruals_and_deferred_income: f64,
    pub net_assets: f64,
    pub capital_and_reserves: f64,
}
```

- New consuming builder on `Frs105Accounts` that replaces only the previous-period
  column (index `[1]` of each array); the current column stays ledger-computed.
  `new()` is untouched, so all existing callers (`tally-cli`, `tally-api`, ct600
  tests) compile unchanged:

```rust
impl Frs105Accounts {
    /// Replace the previous-period (comparative) column with externally
    /// sourced figures (e.g. the last filed accounts at Companies House).
    /// The current-period column is left as computed from the ledger.
    pub fn with_previous_year(mut self, prev: PreviousYearFigures) -> Self {
        self.fixed_assets[1] = prev.fixed_assets;
        self.called_up_share_capital_not_paid[1] = prev.called_up_share_capital_not_paid;
        self.current_assets[1] = prev.current_assets;
        self.prepayments_and_accrued_income[1] = prev.prepayments_and_accrued_income;
        self.creditors_within_1_year[1] = prev.creditors_within_1_year;
        self.net_current_assets[1] = prev.net_current_assets;
        self.total_assets_less_liabilities[1] = prev.total_assets_less_liabilities;
        self.creditors_after_1_year[1] = prev.creditors_after_1_year;
        self.provisions_for_liabilities[1] = prev.provisions_for_liabilities;
        self.accruals_and_deferred_income[1] = prev.accruals_and_deferred_income;
        self.net_assets[1] = prev.net_assets;
        self.capital_and_reserves[1] = prev.capital_and_reserves;
        self
    }
}
```

- **tally-api usage** (accounts + CT600 handlers):

```rust
let accounts = Frs105Accounts::new(&inputs.book, &inputs.company, &inputs.profile, &inputs.meta);
let accounts = match previous_year_figures(&inputs.company, &mut db).await? {
    Some(prev) => accounts.with_previous_year(prev),
    None => accounts, // no CH balance sheet on file → ledger-derived comparatives
};
```

- **tally-cli**: unchanged for now (it runs from a local config + ledger; the
  override is additive and can be adopted later).
- **Unit test**: parse a committed filed-accounts fixture via `from_ixbrl`, apply
  `with_previous_year`, assert the previous column matches and the current column
  is untouched (the existing round-trip fixture pattern).

## Changes — web (`apps/tally-web`)

### `api.ts`

- Types mirroring the API: `Period` (`{ start, end, status: 'filed'|'pending'|
  'ongoing', due: { hmrc, ch } | null, filings: PeriodFiling[] }`), `PeriodFiling`
  (`{ kind, state: 'confirmed'|'not-sent', filed_on?, form_type?, description?,
  document_metadata_url? }`), `BalanceSheet` (period_end, filed_on,
  `figures: PreviousYearFigures`-shaped interface — the same 11 keys),
  `FetchStatus { state, fetched_at, last_error }`.
- `listFilings(companyId) → { periods, balance_sheets, status }` and
  `refreshFilings(companyId) → { job_id }` (or the no-op 200).

### Filings view — two-pane layout

The demo company (`DEMO_COMPANY_ID`) keeps its seeded mock dataset untouched (it is
not a real CH company); signed-out/local companies keep today's empty behaviour.
Real wiring applies to signed-in user companies, driven by `listFilings(companyId)`
fetched on view mount. The mock next-filing card + FY picker + previous-filings
list are **replaced** by:

**Layout — a standalone sub-nav column, flush against the main nav.** The periods
sub-nav is **its own column**, never nested as sub-menus under the main sidebar's
"Filings" item — the main nav keeps only its top-level items (Accounts / Filings /
Payroll / Integrations / Settings). The Filings view renders full-bleed inside
`<main>` as `[ periods sub-nav | period detail ]`, and the sub-nav column sits
**directly against the main sidebar: zero gap, zero margin between the two
columns**. The sub-nav starts at the sidebar's right edge with only a shared
`borderRight: 1px solid {colors.border}` divider (the same treatment the sidebar
itself uses) — no padding or centering between them. Because the other views live
in a shared `maxW: 60rem; mx: auto; p: 5/8` container in `App.tsx`, `FilingsView`
is mounted outside that container (full-bleed within `<main>`); the detail pane
applies the page's own max-width/padding to its content, and the other views keep
the current container untouched.

**Left — periods sub-nav.** A vertical list of the company's financial periods
(`periods`, already newest-first), each row showing:

- the period label — `FYyyyy/yy` with the date range (`1 Apr 2024 – 31 Mar 2025`)
  as a sub-line;
- a trailing status indicator per `status`:
  - **`filed`** → a green tick (`CheckCircle2` in `green.plain.fg`) — the period's
    accounts are filed and confirmed;
  - **`pending`** → a yellow "!" (`TriangleAlert`/`AlertCircle` in amber) with the
    word **pending** — the period has ended but still needs filings;
  - **`ongoing`** → an **Ongoing** outline badge (no icon) — the current period
    containing today.

Row states mirror the app's nav convention: selected row highlighted
(`bg.subtle`, active indicator), hover state, `aria-current` on the selected one.

**Right — selected period's filings.** Clicking a period selects it (default: the
`ongoing` period, falling back to the most recent `filed`/`pending`). The pane
shows:

- a header with the period label, its status badge, and — for `pending`/
  `ongoing` — the `due` dates ("Accounts due <ch>", "CT600 due <hmrc>") with the
  existing `daysTone` urgency colouring (red < 14 days, amber < 45, green);
- the period's **confirmed filings** (CH/HMRC) — each row: `form_type`/`description`
  (e.g. `AA` → "Micro-entity accounts"), `filed_on`, and a green **Confirmed**
  badge (label "Filed at CH" for accounts/confirmation statements — CH has no
  "validated" status; that is the app's own submission status, which doesn't
  exist yet);
- the period's **pending filings, not sent yet** (`state: 'not-sent'`) — e.g.
  "Accounts (FRS 105)" and "CT600" rows with the yellow "!" **Pending** badge,
  and the actions: **Prepare/Preview** (→ `POST /companies/{id}/reports/accounts`
  with the latest ledger id, per web-api-wiring-spec §10 — fetch → blob →
  `window.open`) and **File** (stays a mock toast — HMRC submission is future; the
  backend already gates corp-tax/CT600 on UTR);
- when a period has neither confirmed nor outstanding filings, the existing
  `EmptyState`.

| Mock element | Becomes | Notes |
| --- | --- | --- |
| Next-filing card | Replaced by the `ongoing` period pane | Its period dates + due come from the period's `start`/`end`/`due`; type from the company standard (defaults to FRS 105); `daysLeft` computed client-side; no progress bar (no real signal) |
| FY picker | Replaced by the periods sub-nav | Periods list is server-derived (newest first); selection replaces the picker |
| Previous-filings list | Replaced by the selected period's confirmed filings | Confirmed CH items per period |
| Preview button | **Real** — per web-api-wiring-spec §10 | Lives on `not-sent` rows; `POST /reports/accounts` → blob → `window.open` (already specced, never wired) |
| File now button | **Stays mock** | HMRC submission is future; keep the "lands with the backend" toast |

### Refresh + sync/error states

- **Refresh control**: an icon button in the Filings header (above the periods
  sub-nav). Clicking posts `refreshFilings(companyId)`: `202` → enter the syncing
  state and poll `listFilings` every ~2 s (timeout ~30 s) until `status.state`
  leaves `pending`/`running`; a `200` no-op is a silent no-op. The button shows a
  `Spinner` and is disabled while syncing.
- **On mount**: if `status.state` is `pending`/`running` (e.g. a fetch started at
  company creation), the view shows the syncing state and polls until settled —
  so a mid-flight backfill never looks like missing data.
- **Syncing** (`pending`/`running`): a subtle inline row/banner above the history
  — "Syncing filing history from Companies House…" with a small `Spinner`; the
  table keeps its last-known rows.
- **Failed** (`status.state == 'failed'`, `last_error` set): a warning banner —
  "Filing history unavailable — last sync failed: <mapped error>" with a
  **Retry** button (posts refresh again). The error text maps known envelope
  codes to friendly copy (reusing the `AddCompanyDialog.searchErrorText` pattern):
  `companies_house_key_missing` → "Companies House isn't configured — set
  COMPANIES_HOUSE_API_KEY and restart the API."; `companies_house_rate_limited` →
  "Companies House rate limit reached — try again shortly.";
  `companies_house_upstream` → "Companies House is unavailable — try again.";
  anything else → the envelope message (UI-safe by contract).
- **Refresh endpoint errors**: `companies_house_key_missing` (no key configured)
  surfaces as the same banner with a non-retryable hint; a `NetworkError` keeps
  the last-known rows and re-uses the app's existing offline affordance.
- **Empty** (`status.state == 'done'` and no periods at all): the existing
  `EmptyState` — "No filings yet — they appear here once Companies House has a
  record for this company."
- **Fetch state hygiene**: the syncing banner and failed banner clear as soon as
  a poll returns `done`; polling stops and the banners reset on company switch
  (cleanup via `onCleanup`).
- **Period selection**: survives the poll/refresh (selection is keyed by period
  `end`); a refresh that removes the selected period (e.g. a re-derivation)
  falls back to the `ongoing` period.

## Edge cases

- **No company number or no CH key at create time**: no job is enqueued; the
  refresh endpoint returns `companies_house_key_missing`; `GET /filings` reports the
  state (no periods until a successful fetch).
- **Company with no registration date and no filed history**: no periods can be
  derived (the schedule needs an anchor) — `periods` is empty; the view shows the
  EmptyState until a registration date or a filed period exists.
- **New company with no filings**: empty history, no balance sheet; the current
  status falls back to ledger-only (unchanged behaviour).
- **Document is a PDF** (some CH accounts filings): keep the raw document, mark the
  balance-sheet row unparseable; comparatives unavailable; the job still completes
  (partial success, not a hard failure).
- **Zipped iXBRL**: unzip to the `.html` before parsing.
- **Concurrent refresh clicks**: deduped by the partial unique pending index.
- **CH rate limiting / 5xx**: job fails fast with the error persisted; refresh
  re-runs later.
- **Restart mid-job**: startup marks `running` → `pending`; the rerun is idempotent
  (unique keys on filings/balance-sheets).
- **Company deleted**: cascade or orphan-check jobs/filings/balance-sheets (delete()
  already cascades ledgers; extend to the new tables).

## Out of scope / follow-ups

- Accounts view real-data wiring (the computed on-the-fly balance sheet) —
  follow-up to this change.
- Demo company / local-mode Filings data (kept mock/empty by design — see
  Changes — web).
- Automatic retry/backoff policy (schema supports it; fail-fast for now).
- App-owned filing status ("validated", submitted CT600 records) — CH history is
  filed-at-CH only; until it exists, the right pane's `not-sent` items are the
  expected accounts + CT600 for the period.
- Multi-company worker scaling / fan-out tuning.
- Re-fetch freshness policy beyond manual refresh.

## Verification

- `cargo test -p tally-api` — new unit tests (jobs) + pg tests (refresh without key,
  persistence with a stubbed CH source); existing 47 tests stay green.
- `cargo check -p tally-api --no-default-features` (offline build).
- `cargo test -p ct600` for the new pagination + document-fetch paths (offline
  unless `cached_live_tests` / `always_live_tests`).
- `pnpm --filter @tally/web typecheck` after the Filings view wiring.
- Manual: add a company with the key set → job runs → filings + balance sheet
  appear; refresh re-runs; kill the API mid-job → restart re-claims and finishes.
