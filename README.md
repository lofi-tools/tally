# Accounting

Computes a UK limited company's accounts — balance sheet, profit & loss, corporation tax — and produces iXBRL and CT600 filings for Companies House and HMRC.

## How it works

The project works as a sequence of stages:

1. **Gather data from source systems.**
  <br/> Fetch company data from Companies House
  <br/> Pull transactions from bank accounts via Open Banking connectors and load extra transactions from local records (these connectors previously lived in the deprecated `mk-accounts` app)
  <br/> Alternatively, start from an existing `.gnucash` ledger and skip straight to the accounts structure.

2. **Build the accounts structure.**
   <br/> Classify raw transactions into a double-entry bookkeeping structure.
   <br/> Organise accounts into a tree, like GnuCash.
   <br/> Derive the balance sheet and profit & loss statement.

3. **Map to a reporting standard.**
   <br/> Map accounts onto a regulatory taxonomy (iXBRL tagging standard) for UK filing: for now, FRS 105 only (micro-entity). Planned: FRS 102 (small company), FRS 101 (reduced disclosure), FRSSE, IFRS.

4. **Handle the CT600 filing.**
   <br/> Wrap the iXBRL data in the format required by HMRC for online corporation tax submission: canonicalization, GovTalk envelope, IRMark.
   <br/> Include extra CT600 form values & computations.
   <br/> Handle the full submission lifecycle: submit, acknowledge, poll for success, handle errors / deletion.

## Project structure

| Crate | Stage | Purpose |
|-------|-------|---------|
| `apps/tally-cli` | 3–4 | CLI  for the libraries: `tally ct600` produces the CT600 message from a config file + GnuCash book (no submission yet) |
| `libs/ixbrl` | 1, 3 | GnuCash parser + iXBRL taxonomy definitions (FRS-105, FRS-102, etc.) + Companies House client / company resolution (`ixbrl::clients`) |
| `libs/ct600` | 4 | HMRC GovTalk XML message builder/parser for CT600 submission |

## Developer quickstart

1. Install Nix and direnv (see [`docs/installing-nix-and-direnv.md`](docs/installing-nix-and-direnv.md)).
2. `direnv allow` — loads the flake devShell (cargo, rustc, nextest, the reference Python tools, ...).
3. Produce the example CT600:

   ```bash
   cargo run -p tally-cli -- ct600 \
     --config-path libs/ixbrl/example_data/example2/input-company.json \
     --book libs/ixbrl/example_data/example2/input.gnucash \
     --out .cache/ct600
   # -> wrote .cache/ct600/ct600.xml
   ```

4. Run the tests (no configuration needed — see below):

   ```bash
   cargo test --workspace
   ```

## Running the tests without any configuration

The tests run fully offline out of the box:

- The Companies House endpoints serve from hardcoded fictional mock responses in
  `libs/ixbrl/src/clients/test_utils.rs`;
- The HMRC Corporation Tax client is tested against an in-process GovTalk stub gateway
  in `libs/ct600/src/test_utils.rs`;
- `libs/ct600`'s reference comparison skips gracefully when the cached reference
  message is absent.

So `cargo test -p ixbrl`, `cargo test -p ct600` and `cargo test --workspace` need no
API key, no network and no cached responses on a fresh checkout. `cargo test -p tally-cli`
currently has no tests.

## tally-cli configuration

### Command-line flags

All flags are required; there is no environment-variable fallback for them.

| Flag | Meaning |
|------|---------|
| `--config-path <config>` | JSON config: company identity + accounts metadata (see below) |
| `--book <book>` | GnuCash ledger (`input.gnucash`) |
| `--out <dir>` | output directory; the CT600 GovTalk message is written to `<dir>/ct600.xml` |

### Config file

A JSON file with the same shape as
`libs/ixbrl/example_data/example2/input-company.json` (use it as a template): a nested
`company` identity block plus the flat accounts-metadata fields.

| Key | Type | Meaning |
|-----|------|---------|
| `company.name` | string | company name |
| `company.tax_reference` | string | Corporation Tax reference (UTR) |
| `company.company_number` | string | Companies House registration number |
| `company.accounting_period_start` | date | start of the return period (`YYYY-MM-DD`) |
| `company.accounting_period_end` | date | end of the return period (`YYYY-MM-DD`) |
| `directors`, `sic_codes`, `address_lines`, `email`, ... | — | accounts metadata (all `AccountsMetadata` fields from `libs/ixbrl`) |

All fields are required — the parser applies no defaults, so copy the example config
and edit it.

### Defaults baked into the produced message

These come from the libraries' `Default` implementations and have no config hook yet:

| Field | Default |
|-------|---------|
| GovTalk envelope | class `HMRC-CT-CT600`, `GatewayTest` 1, username `CTUser100` / password `password`, vendor `1234`, software `ct600` 1.0.0 |
| Principal contact | `Ms Sarah McAcre`, sarah@example.org |
| Financial years / rates | FY1 2019, FY2 2020, both 19% (`Company::new` defaults) |
| Declaration (boxes 975 / 985) | contact name / `Director`; box 980 (date) = today |

### Environment variables (library-level)

Producing the CT600 needs none. The underlying libraries read a few variables for live
Companies House resolution and HMRC submission (future features):

| Variable | Used for |
|----------|----------|
| `COMPANY_NUMBER` | company resolution when no full company override is given |
| `COMPANIES_HOUSE_API_KEY` / `COMPANIES_HOUSE_API_KEY_TEST` | live / sandbox Companies House lookups |
| `CT600_CACHE_DIR` | company-profile cache directory (default `<repo>/.cache/api_responses`) |
| `HMRC_CT_*` | HMRC submission credentials (not used when producing the return) |

The Companies House client (`ixbrl::clients::CompaniesHouseClient`) caches fetched
company profiles under `.cache/api_responses/companies-house-<number>.json` (override
with `CT600_CACHE_DIR`) and serves from the cache when available.  Company resolution
follows a strict order: a full company override wins, otherwise the cached response for
the configured company number (`COMPANY_NUMBER`) is used, otherwise the profile is
fetched from the live API.  The CT600 `company_form_values` adapter (in `libs/ct600`)
fills the header boxes from the resolved profile.

## Links

- https://www.gov.uk/company-tax-returns
- https://www.gov.uk/government/collections/corporation-tax-online-support-for-software-developers
