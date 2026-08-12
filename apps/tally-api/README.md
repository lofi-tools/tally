# tally-api

The Tally web API: a Rust (axum) service exposing the compute capabilities of
`libs/ixbrl` (GnuCash parsing, FRS 105 accounts / corporation-tax iXBRL) and
`libs/ct600` (CT600 GovTalk, Companies House search / enrichment) over HTTP,
with a stateful Postgres store (toasty).  Full contract: `docs/spec/api-backend-spec.md`.

## Quickstart

Prereqs: nix (dev shell), docker.

```sh
nix develop -c dev-db        # postgres:16 on :5432 (tally/tally/tally)
nix develop -c api           # cargo run -p tally-api → http://127.0.0.1:8080
```

Or run the whole stack (db + api + web app) at once in a zellij session:
`nix develop -c dev` (from the repo root).

```sh
curl -s localhost:8080/health                    # {"status":"ok"}
curl -s -X POST localhost:8080/api/v1/auth/register \
  -H 'content-type: application/json' \
  -d '{"display_name":"Ada","email":"ada@example.com","password":"hunter2hunter2"}'
```

## Environment

| Var | Default | Notes |
|---|---|---|
| `DATABASE_URL` | `postgres://tally:tally@localhost:5432/tally` | toasty/Postgres |
| `TALLY_API_ADDR` | `127.0.0.1:8080` | `PORT` also honored |
| `TALLY_API_UPLOAD_DIR` | `.cache/tally-api/uploads` | ledger files |
| `TALLY_API_MAX_UPLOAD_BYTES` | `52428800` (50 MB) | multipart size cap |
| `COMPANIES_HOUSE_API_KEY` | — | live CH (enables search/enrich) |
| `COMPANIES_HOUSE_SANDBOX_API_KEY` | — | sandbox CH |
| `CT600_CACHE_DIR` | — | CH response cache |

Without a CH key, CH-backed endpoints return `400 companies_house_key_missing`;
everything else works.

## Endpoints (all under `/api/v1`, bearer auth except `/health` + `auth/register`/`auth/login`)

- `POST /auth/register`, `POST /auth/login` → `{ token, user }`; `POST /auth/logout`; `GET /auth/me`
- `GET|POST /companies`, `GET|PATCH|DELETE /companies/:id`, `GET /companies/search?q=`, `POST /companies/:id/enrich`
- `POST /companies/:id/ledgers` (multipart `.gnucash`), `GET /companies/:id/ledgers`, `GET|DELETE /ledgers/:id`, `GET /ledgers/:id/accounts`, `GET /ledgers/:id/transactions`
- `POST /companies/:id/reports/accounts` (FRS 105 accounts iXBRL), `POST /companies/:id/reports/corp-tax`, `POST /companies/:id/reports/corp-tax.json`, `POST /companies/:id/reports/ct600` (GovTalk XML)

Errors are the `{ "error": { code, message, details? } }` envelope (§11 of the
spec); `5xx` responses echo `X-Request-Id` in `details.request_id`.

## Testing

```sh
# Full suite — `test-api` first ensures the compose Postgres is up, then
# runs everything:
nix develop -c test-api

# First-clone / DB-less (integration tests compile away):
cargo test -p tally-api --no-default-features
# (or: nix develop -c test-api-offline)
```

Without docker/Postgres the pg-gated tests print a notice and skip (pass)
rather than failing, so plain `cargo test -p tally-api` is safe either way.

## Layout

- `src/models.rs` — toasty models (users, sessions, companies, ledgers, accounts, transactions, splits)
- `src/auth.rs` — register/login/logout/me + the bearer `AuthUser` extractor (argon2, sha256 tokens)
- `src/companies.rs` — company CRUD + ownership helpers (+ CH enrichment)
- `src/ledgers.rs` — upload (stream to temp file, sha256, parse, persist) + JSON views
- `src/reports.rs` — the four report endpoints (rebuild the book from stored rows)
- `src/period.rs` — accounting-period resolution chain
- `src/companies_house.rs` — thin adapter over `ct600::CompaniesHouseClient` + search
- `src/error.rs` — the §11 error contract (snafu) + axum rejections → `AppError`
- `src/app.rs` — router, state, middleware (request-id, trace, catch-panic, CORS, body limit)

## Note on `vendor/toasty-driver-sqlite`

The workspace patches `toasty-driver-sqlite` to a resolution-only stub.  Cargo
resolves dev-dependencies of every graph package into the lockfile, and the
real sqlite driver pins a `rusqlite` (→ `libsqlite3-sys`) that conflicts with
`rucash`'s, and `libsqlite3-sys` can only appear once (`links`).  The stub is
never built — the API only uses the postgresql driver.
