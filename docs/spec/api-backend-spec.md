# Tally Backend API — spec

Status: **spec** (no code yet)
Scope: new Rust API service in `apps/tally-api`, exposing the useful methods of
`libs/ixbrl` and `libs/ct600` over HTTP, with a stateful Postgres store (toasty).

This document records the decisions gathered from the interview. It is the
contract for implementation.

---

## 1. Goals

- A stateful HTTP API in Rust (axum) that exposes the compute capabilities of
  `libs/ixbrl` and `libs/ct600`:
  - **ixbrl**: GnuCash parsing, ledger views, FRS 105 accounts (balance sheet)
    iXBRL, FRS 105 corporation-tax computations + iXBRL.
  - **ct600**: CT600 GovTalk message generation, Companies House search /
    lookup / enrichment.
- Real user accounts with full auth (register / login / bearer sessions).
- Postgres as the database, **toasty** as the ORM/db library.
- Ledger uploads (`.gnucash`) stored to disk + parsed rows in Postgres.
- tally-web integration is a **follow-up task**; the API's resources are still
  designed so the web can consume them (companies, ledgers, reports).

## 2. Non-goals (explicitly out of scope for this task)

- **HMRC submission** — the API generates CT600 messages but does not run
  `HmrcCorpTaxClient`'s submit/poll/delete lifecycle (matches the CLI today).
  The lib's client remains available for a later task.
- **Wiring tally-web to the API** — separate follow-up. The web keeps its
  localStorage demo-company model until then.
- **Standalone P&L report** — the libs produce the FRS 105 statement of
  financial position (balance sheet) and corp-tax profit figures, but no
  standalone P&L document. The API exposes exactly what the libs produce; a
  P&L is a lib-side roadmap item.
- **Payroll** — no payroll logic exists in the libs; nothing to expose.
- **Email verification / refresh tokens / password reset** — auth is "full"
  for this codebase's stage but deliberately without those ceremonies.

## 3. Stack & versions

| Concern            | Choice                                                          |
|--------------------|-----------------------------------------------------------------|
| HTTP framework     | axum 0.8 (latest)                                               |
| Async runtime      | tokio (workspace dep, `full`)                                   |
| ORM / db library   | **toasty** — latest (0.9.x as of writing; "latest is fine", no pinning) |
| Database           | PostgreSQL 16 (docker-compose in repo)                          |
| Auth               | argon2 password hashing; opaque bearer tokens (hashed at rest)  |
| Errors             | **snafu** `AppError` (`#[derive(Debug, Snafu)]`, `#[snafu(display)]` + context selectors) + `IntoResponse` → JSON envelope `{ "error": { code, message, details } }`; full contract in §11 |
| Logging/tracing    | tracing + tracing-subscriber (env filter), tower-http trace     |
| CORS               | tower-http cors, allow `http://localhost:5173` (dev web)        |
| Testing            | offline unit tests + `pg-tests` cargo feature (see §12)         |

Workspace deps already present and reused: `serde`, `serde_json`,
`serde_json_lenient`, `chrono` (serde), `anyhow`, `snafu`,
`uuid`, `futures`, `rucash`, `ixbrl` (path), `ct600` (path, `default-features
= false` like tally-cli).

New deps (in `apps/tally-api/Cargo.toml`, workspace-shared where sensible):
`axum`, `toasty`, `tower`, `tower-http` (cors, trace, catch-panic),
`tracing`, `tracing-subscriber`, `argon2`, `rand`, `sha2`, `mime`,
`tokio-util` (as needed for multipart streaming), `tempfile` (dev).

## 4. Workspace layout

```
apps/tally-api/
  Cargo.toml          # package tally-api, edition 2024
  README.md           # quickstart: compose up → env → run → test
  src/
    main.rs           # env config, tracing init, bind, serve
    app.rs            # Router assembly + state (Arc<AppState>)
    error.rs          # AppError → IntoResponse
    auth.rs           # register/login/logout/me handlers + bearer middleware
    models.rs         # toasty #[model] definitions
    companies.rs      # company handlers + CH search/enrich
    ledgers.rs        # upload/parse/list/get/delete + ledger JSON views
    reports.rs        # accounts / corp-tax / ct600 generation handlers
    companies_house.rs# thin adapter over ct600::CompaniesHouseClient
    period.rs         # period resolution chain (reuse of CLI logic)
  tests/
    auth.rs, companies.rs, ledgers.rs, reports.rs   # pg-gated integration
docker-compose.yml    # postgres 16 (+ optional adminer), named volume, healthcheck
```

Crate is a workspace member via the existing `apps/*` glob (no `Cargo.toml`
exclusion changes needed; the JS apps are already excluded).

Binary name: `tally-api`. Default bind: `127.0.0.1:8080` (LTS owns 8081),
overridable via `TALLY_API_ADDR` / `PORT`. DB URL via `DATABASE_URL`
(default `postgres://tally:tally@localhost:5432/tally`).

## 5. Data model (toasty `#[model]`s)

All resources are owned by a `User`. Ownership is enforced on every route.

```
User        id, email (unique), password_hash, display_name, created_at
Session     id, user_id (BelongsTo<User>), token_hash (unique), created_at, expires_at
Company     id, user_id (BelongsTo<User>), + identity & config fields (see below)
Ledger      id, company_id (BelongsTo<Company>), name, file_path, file_sha256,
            uploaded_at, accounts_count, transactions_count, splits_count
Account     id, ledger_id (BelongsTo<Ledger>), guid, name, type, parent_guid, balance
Transaction id, ledger_id (BelongsTo<Ledger>), guid, post_datetime
Split       id, ledger_id (BelongsTo<Ledger>), tx_guid, account_guid, value
```

`Company` carries the union of what the CLI's config carries
(`CompanyConfig` + `AccountsConfig`, see `apps/tally-cli/src/config.rs` and
`libs/ixbrl/example_data/basic-1/input_config.jsonc`), so report generation
can rebuild the library inputs without a config file:

- identity: `name`, `tax_reference` (UTR), `company_number`, `registration_date`
- profile: directors, contacts, registered-office address, SIC codes,
  jurisdiction, accountant/auditor, dimensions, logo_b64 (all optional)
- accounts metadata: `fy1_year`, `fy2_year`, `associated_companies`,
  `report_date`, `authorised_date`, `incorporation_date`, `signed_by`,
  `average_employees` (map), taxonomy dimensions, `signature_b64`

`Account.balance` is a `rucash::Num`-compatible decimal (the ledger is
rebuilt for reports via `GnucashBook::from_raw_parts`, which recomputes
balances from splits; the stored balance is a convenience for the JSON views —
keep both in sync on ingest).

Schema generation: toasty's own mechanism (models → schema sync). Because
toasty is young, the implementation must nail down the exact codegen step
(`cargo toasty` CLI / build-time codegen) early and document it in the crate
README.

## 6. Endpoints (all under `/api/v1`)

Auth (bearer token in `Authorization: Bearer <token>` on every route below):

| Method | Path                          | Body / Notes                                        |
|--------|-------------------------------|-----------------------------------------------------|
| POST   | `/auth/register`              | `{ display_name, email, password }` → `{ token, user }` |
| POST   | `/auth/login`                 | `{ email, password }` → `{ token, user }`           |
| POST   | `/auth/logout`                | revokes the session token                           |
| GET    | `/auth/me`                    | current user                                        |

Companies:

| Method | Path                              | Notes                                            |
|--------|-----------------------------------|--------------------------------------------------|
| GET    | `/companies`                      | list current user's companies                    |
| POST   | `/companies`                      | create from config fields; enrich via CH when a number is given and a key is set |
| GET    | `/companies/:id`                  | ownership-scoped                                 |
| PATCH  | `/companies/:id`                  | partial update of config fields                  |
| DELETE | `/companies/:id`                  | cascades ledgers                                 |
| GET    | `/companies/search?q=`            | Companies House search (**requires a key**, else 400 with guidance) |
| POST   | `/companies/:id/enrich`           | fetch CH profile + officers, fill blank profile fields (config wins) |

Ledgers:

| Method | Path                                | Notes                                          |
|--------|-------------------------------------|------------------------------------------------|
| POST   | `/companies/:id/ledgers`            | multipart `.gnucash` upload → disk + parse + rows |
| GET    | `/companies/:id/ledgers`            | list ledgers for the company                   |
| GET    | `/ledgers/:id`                      | metadata (counts, sha256, name)                |
| DELETE | `/ledgers/:id`                      | remove rows + file                             |
| GET    | `/ledgers/:id/accounts`             | JSON account tree with balances (like `Display` for `GnucashBook`) |
| GET    | `/ledgers/:id/transactions`         | JSON transactions with their splits (pagination: `?limit=&offset=`) |

Reports (raw documents, per "JSON + raw XML endpoints"):

| Method | Path                                        | Returns                                            |
|--------|---------------------------------------------|----------------------------------------------------|
| POST   | `/companies/:id/reports/accounts`           | FRS 105 accounts iXBRL → `text/html` (`Frs105Accounts::to_ixbrl`) |
| POST   | `/companies/:id/reports/corp-tax`           | FRS 105 corp tax iXBRL → `text/html` (`Frs105CorpTax::to_ixbrl`) |
| POST   | `/companies/:id/reports/corp-tax.json`      | corp-tax figures as JSON (the `Frs105CorpTax` getters) |
| POST   | `/companies/:id/reports/ct600`              | CT600 GovTalk XML → `application/xml` (`Ct600Return::from_inputs().to_xml()`) |

Report request bodies: `{ ledger_id, period?: {start,end}, made_up_to?: date,
declaration?: {...}, ...overrides }`. Period resolution reuses the CLI chain
(§7). Raw docs are streamed/generated per request from the stored ledger rows
(via `GnucashBook::from_raw_parts`) — nothing document-shaped is cached in the
DB.

Health: `GET /health` (no auth) → `{ status: "ok" }`.

## 7. Period resolution (reuse of the CLI chain)

Exactly the chain from `tally-cli`'s `ConfigBuilder::resolve_period`, moved
into `period.rs`:

1. explicit `period` in the request wins;
2. else `made_up_to` date → the 12 months ending on it;
3. else the company's **next accounting period** from Companies House
   (`next_accounting_period_from` on the stored profile; requires a key,
   else 400 explaining how to set one or pass an explicit period).

## 8. External services

- **Companies House**: **requires a configured key** (`COMPANIES_HOUSE_API_KEY`
  for live, `COMPANIES_HOUSE_SANDBOX_API_KEY` for sandbox). Without one,
  search/enrich/period-from-CH endpoints return a 400 with a clear message —
  no fixture fallback in the API. Cache-first via `CT600_CACHE_DIR`
  (`get_company_profile_cached`), exactly as the CLI does.
- **HMRC**: out of scope (generation only). The `HmrcCorpTaxClient` config
  (`HmrcCorpTaxConfig::from_env`) is not wired into the API in this task.

## 9. Uploads

- `POST /companies/:id/ledgers` accepts `multipart/form-data` with a
  `file` field (`.gnucash` only; the parser itself already detects
  XML-vs-SQLite magic in `GnucashBook::try_from_gnucash_file`).
- Bytes stream to disk under `<repo>/.cache/tally-api/uploads/<uuid>.gnucash`
  (gitignored alongside the other `.cache` content); the DB row keeps the
  path, a sha256, and the parsed counts.
- Ingest: parse the file once, write `Account`/`Transaction`/`Split` rows,
  and rebuild via `GnucashBook::from_raw_parts` for any downstream use.
- Reject oversized files (configurable limit, e.g. 50 MB default) and
  non-`.gnucash` names with 422/413.

## 10. Auth details

- `POST /auth/register` and `/auth/login`: verify with argon2; issue an
  opaque 32-byte random token; store only its sha256 in `Session`; return the
  plaintext token once.
- Sessions expire (default 30 days) and are revoked on logout.
- Every non-auth handler runs through a bearer extractor that resolves the
  `User` and rejects unknown/expired tokens with 401.
- All company/ledger/report queries are filtered by `user_id` (ownership),
  so cross-user access is impossible even by guessing ids (UUIDs).

## 11. Error contract

Every non-2xx response is a JSON error envelope. Lib error text never
leaks verbatim — the libs' `Display` messages are for server logs only.

### 11.1 Envelope shape

```json
{
  "error": {
    "code": "ledger_not_found",
    "message": "No ledger with id 'abc' exists for this account.",
    "details": { "resource": "ledger", "id": "abc" }
  }
}
```

- `code` — stable snake_case machine identifier. Clients branch on this,
  never on message text. Codes are additive; a code's meaning never changes.
- `message` — human-readable and UI-safe (safe to show verbatim).
- `details` — optional free-form object; omitted when empty. Contents are
  per-variant in §11.2.
- Content-Type is always `application/json`, including on the report
  endpoints that return HTML/XML on success.
- Every response carries `X-Request-Id` (uuid, set by middleware and reused
  in the tracing span); 5xx `details.request_id` echoes it. No stack traces
  or internals reach the client.
- 401 responses additionally carry `WWW-Authenticate: Bearer`.

Status-code conventions: `400` malformed/unparseable request, `422`
well-formed but semantically invalid, `401` authentication, `404` missing
*or* foreign resources (never `403` — the existence of other users'
resources is never revealed), `409` conflicts, `413`/`415` upload
constraints, `429`/`502`/`404` upstream passthrough for Companies House,
`500` internal.

### 11.2 AppError variants

**All error enums in the API — `AppError` and any route-level helper enums —
derive snafu** (`#[derive(Debug, Snafu)]`), matching the libs' own error
types (`GnucashError`, `CompaniesHouseError`, `HmrcCorpTaxError`). Each
variant declares a `#[snafu(display("..."))]` message; wrapped lib errors
use snafu context selectors (`source`, `url`, `status`, …) so `From`
conversions are lossless and the message stays readable. `impl IntoResponse
for AppError` is the single place where status/code mapping lives.

| AppError variant | Source | HTTP | `code` | `details` |
|---|---|---|---|---|
| `AuthHeaderMissing` | bearer extractor | 401 | `auth_missing` | — |
| `AuthTokenInvalid` | bearer extractor | 401 | `auth_invalid` | — |
| `AuthTokenExpired` | bearer extractor | 401 | `auth_expired` | — |
| `EmailTaken` | register | 409 | `email_taken` | `{ email }` |
| `InvalidCredentials` | login | 401 | `invalid_credentials` | — |
| `InvalidJson` | `serde_json::Error` | 400 | `invalid_json` | `{ line, column }` |
| `Validation` | semantic checks | 422 | `validation_failed` | `{ fields: [{ field, reason }] }` |
| `UnsupportedFileType` | upload `file` field | 415 | `unsupported_file_type` | `{ expected, got }` |
| `FileTooLarge` | size limit | 413 | `file_too_large` | `{ limit_bytes }` |
| `Multipart` | multipart stream errors | 400 | `multipart_error` | — |
| `NotFound` | toasty query miss | 404 | `not_found` | `{ resource, id }` |
| `CompanyLedgerMismatch` | ledger not in this company | 422 | `ledger_not_in_company` | `{ company_id, ledger_id }` |
| `DuplicateCompany` | company number already added | 409 | `duplicate_company` | `{ company_number }` |
| `PeriodNotDetermined` | period chain exhausted | 422 | `period_not_determined` | `{ hint }` |
| `CompaniesHouseKeyMissing` | no key configured | 400 | `companies_house_key_missing` | `{ hint }` |
| `CompaniesHouseNotFound` | CH returned 404 | 404 | `company_not_found` | `{ company_number }` |
| `CompaniesHouseRateLimited` | CH returned 429 | 429 | `companies_house_rate_limited` | `{ retry_after? }` |
| `CompaniesHouseUpstream` | CH network / other status / decode | 502 | `companies_house_upstream` | `{ url, upstream_status? }` |
| `LedgerParse` | `GnucashError` | 422 | `ledger_parse_failed` | — |
| `Storage` | upload disk IO | 500 | `storage_error` | `{ request_id }` |
| `Db` | toasty errors | 500 | `internal` | `{ request_id }` |
| `Internal` | catch-all (`anyhow`) | 500 | `internal` | `{ request_id }` |

Lib-error mapping (all in `error.rs`):

- `ixbrl::GnucashError::{Io, Rucash}` → `LedgerParse` — a file that
  streamed in but is not a valid book is a 422 (client's input), not a 500.
- `ct600::CompaniesHouseError::{RequestFailed, HttpStatus, DecodeFailed}` →
  `CompaniesHouseUpstream`, except `HttpStatus { status: 404 }` →
  `CompaniesHouseNotFound` and `HttpStatus { status: 429 }` →
  `CompaniesHouseRateLimited`. `MissingCompanyNumber` → 422
  `missing_company_number` (a server/config condition surfaced as a clear
  client error).
- `ct600::Ct600Error::{XmlError, ConfigError, C14nError}` → `Internal`:
  CT600 document assembly consumes only our own stored models, so a build
  failure is a bug, not a client error.
- `std::env::VarError` from `CompaniesHouseClient::from_env` → surfaced per
  request as `CompaniesHouseKeyMissing`, so a running server without a key
  degrades to clear 400s rather than failing to boot.

Upload-path note: failure to write the streamed bytes under
`.cache/tally-api/uploads/` is `Storage` (500) — the client's file was
fine; the server failed to persist it.

### 11.3 Endpoint → possible codes

`auth*` below means `auth_missing` / `auth_invalid` / `auth_expired`
(every authenticated route can also return `internal`).

| Endpoint | Codes beyond `auth*` and `internal` |
|---|---|
| `POST /auth/register` | `invalid_json`, `validation_failed`, `email_taken` |
| `POST /auth/login` | `invalid_json`, `validation_failed`, `invalid_credentials` |
| `POST /auth/logout`, `GET /auth/me` | — |
| `GET /companies` | — |
| `POST /companies` | `invalid_json`, `validation_failed`, `duplicate_company`, and when a key is set: `companies_house_upstream`, `company_not_found`, `companies_house_rate_limited` |
| `GET /companies/:id` | `not_found` |
| `PATCH /companies/:id` | `not_found`, `invalid_json`, `validation_failed` |
| `DELETE /companies/:id` | `not_found`, `storage_error` (ledger files) |
| `GET /companies/search` | `validation_failed` (missing `q`), `companies_house_key_missing`, `companies_house_upstream`, `companies_house_rate_limited` |
| `POST /companies/:id/enrich` | `not_found`, `companies_house_key_missing`, `company_not_found`, `companies_house_upstream`, `companies_house_rate_limited` |
| `POST /companies/:id/ledgers` | `not_found`, `multipart_error`, `unsupported_file_type`, `file_too_large`, `validation_failed`, `ledger_parse_failed`, `storage_error` |
| `GET /companies/:id/ledgers` | `not_found` |
| `GET /ledgers/:id`, `DELETE /ledgers/:id` | `not_found`, `storage_error` (delete) |
| `GET /ledgers/:id/accounts` | `not_found` |
| `GET /ledgers/:id/transactions` | `not_found`, `validation_failed` (bad `limit`/`offset`) |
| `POST /companies/:id/reports/accounts` | `not_found`, `ledger_not_in_company`, `validation_failed` (bad period), `period_not_determined`, `companies_house_key_missing` (period-from-CH path) |
| `POST /companies/:id/reports/corp-tax` | same set as `reports/accounts` |
| `POST /companies/:id/reports/corp-tax.json` | same set as `reports/accounts` |
| `POST /companies/:id/reports/ct600` | the accounts set, plus `internal` for document-assembly bugs |
| `GET /health` | `internal` only (no auth) |

Framework-level responses (router fallback, not `AppError` variants, but
part of the contract): unmatched routes → `404 route_not_found`; matched
path with wrong method → `405 method_not_allowed`. Panics are caught by
catch-panic middleware and become `500 internal` with a `request_id`.

### 11.4 Testing the contract

- Unit (offline): a table-driven test asserting `(variant → status, code)`
  for every `AppError` variant, plus one example body per variant family.
- Integration (`pg-tests`): every endpoint test asserts both the status
  *and* the `code` for its error cases (wrong-owner ledger, duplicate
  company, key-less search, oversized upload, etc.).
- The integration suite asserts `X-Request-Id` is present on every response.

## 12. Testing

- **Unit tests** (offline, no DB): period resolution, error mapping, auth
  token logic, report-input assembly. These mirror the repo's fully-offline
  convention (fixtures from `libs/ixbrl/example_data/` and the ct600 test
  fixtures).
- **Integration tests** (need Postgres) are compiled behind a cargo feature
  **`pg-tests`**, **on by default**:
  - First-clone / DB-less test command documented everywhere:
    `cargo test -p tally-api --no-default-features`.
  - `nix develop -c test-api` (or plain `cargo test -p tally-api`) runs the
    full suite; the harness is self-sufficient (see DB lifecycle below).
  - An unreachable database is a **hard failure**, never a silent skip:
    if docker/Postgres isn't available the pg-gated tests panic with
    instructions (start the DB or disable `pg-tests`).
- DB lifecycle for tests: the tests use a **shared** database — the
  docker-compose `test-api-db` service (port 5433, `tally_test`, profiled
  so plain `docker compose up` never starts it).  The harness auto-starts
  the container (`docker compose up -d --wait test-api-db`), waits for
  readiness, and re-initialises the schema **once per run** under a
  Postgres advisory lock, but only when the committed migrations changed
  (a checksum marker).  Tests arrange their own data (unique emails/guest
  ids) so the shared DB has minimal effects between tests and across runs.

## 13. Dev environment (flake + docker)

- `docker-compose.yml` at the repo root: `postgres:16` for the dev DB
  (named volume, healthcheck, `tally`/`tally`/`tally` db) plus a profiled
  `test-api-db` service (port 5433, `tally_test` DB) for the integration
  tests — profiled services are never started by plain `docker compose up`;
  the tests start `test-api-db` themselves.  `adminer` is also profiled
  (`docker compose --profile tools up adminer`).
- Flake scripts:
  - `dev-db` — `docker compose up -d api-db` (+ wait for health)
  - `db-down` — `docker compose down`
  - `api` — `cargo run -p tally-api` (env from `.env` / defaults)
  - `test-api` — full suite; the harness owns the test DB lifecycle
    (auto-starts `test-api-db`, reinit once per run, fail-hard)
  - `test-api-offline` — `cargo test -p tally-api --no-default-features`
- `apps/tally-api/README.md` documents the quickstart and the first-clone
  test command.

## 14. Follow-up (explicitly deferred)

- **tally-web integration**: rewire real-data paths (company CRUD via CH
  search,  ledger upload as a data source, accounts/filings views from ledger
  JSON + report endpoints) to this API; the web's localStorage stays for the
  demo company. The API's resource design above anticipates exactly that.
- **HMRC submission** endpoints (`HmrcCorpTaxClient` lifecycle).
- **Standalone P&L** (lib-side work, then an API endpoint).

## 15. Open implementation questions (to resolve during build, low risk)

- Exact toasty codegen/schema-sync step for Postgres (CLI vs build-time) —
  pin down early, document in the crate README.
- `rucash::Num` serialization for JSON views — serialize as a decimal string
  or number (decision: string, to preserve precision, unless the web needs
  numbers).
- Multipart streaming vs buffering for large `.gnucash` files (SQLite books
  can be sizable) — decide against a size benchmark in the build.
