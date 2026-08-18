# Tally

Computes a UK limited company's accounts — the balance sheet, the profit & loss and corporation-tax figures — and produces the iXBRL accounts and CT600 corporation-tax return for filing with Companies House and HMRC.

## What this does

1. **Gather data from source systems.**
  <br/> Fetch company data from Companies House
  <br/> Pull transactions from bank accounts + load extra transactions from local records 
  <br/> Or provide your existing `.gnucash` ledger and skip this step.

2. **Build the accounts structure.**
   <br/> Classify raw transactions into a double-entry bookkeeping structure.
   <br/> Organise accounts into a tree, like GnuCash.
   <br/> Derive the balance sheet, profit & loss & corporation-tax figures.

3. **Map to a reporting standard.**
   <br/> Map accounts onto a regulatory taxonomy (iXBRL tagging standard) for UK filing: for now, FRS 105 only (micro-entity). More planned.

4. **Handle the CT600 filing.**
   <br/> Wrap the iXBRL data in the format required by HMRC for online corporation tax submission: canonicalization, GovTalk envelope, IRMark.
   <br/> Include extra CT600 form values & computations.
   <br/> No submission to HRMC yet (planned)

See [roadmap](docs/current-state-and-roadmap.md) for planned features & progress.

## Quickstart

`tally` turns your accounting data into the CT600 Corporation Tax return ready to file with HMRC. It does **NOT** file the return. You provide:

1. **A config file** (`--config-path`) — the company's identity (name,
   registration number, tax reference) and the accounts metadata
   (directors, SIC codes, address, …).  Anything left out is filled in
   automatically from the company's Companies House profile when an API
   key is set.
2. **Your books** (`--book`) — the GnuCash ledger (`input.gnucash`);

```bash
tally ct600 \
  --config-path example_data/basic-1/input_config.jsonc \
  --book example_data/basic-1/input.gnucash \
  --out .cache/tally-cli
  # -> writes .cache/tally-cli/ct600-<company-number>.xml
```

See the [example config file](example_data/basic-1/input_config.jsonc)
And the detailed [tally-cli config options](#tally-cli-configuration) for 
flags, config, and defaults.


## Developer quickstart

1. Install Nix and direnv (see [`docs/installing-nix-and-direnv.md`](docs/installing-nix-and-direnv.md)).
2. `direnv allow` — loads the flake devShell (cargo, rustc, nextest, the reference Python tools, ...).
3. Produce the example CT600:

   ```bash
   cargo run -p tally-cli -- ct600 \
     --config-path example_data/basic-1/input_config.jsonc \
     --book example_data/basic-1/input.gnucash \
     --out .cache/tally-cli
     # -> writes .cache/tally-cli/ct600-<company-number>.xml
   ```

4. Run the tests (with stub data, no remote APIs, no config needed):

   ```bash
   cargo test --workspace --no-default-features
   ```

## Testing

There are two ways to run the tests:

- **offline** — runs on the repo's fictional test data; no API key, network or config needed:  
  `cargo test --workspace --no-default-features` 
- **full** —  also runs integration tests. It needs env vars `COMPANIES_HOUSE_API_KEY` (or
  `COMPANIES_HOUSE_SANDBOX_API_KEY`) and `COMPANY_NUMBER` (see below):  
  `cargo test --workspace`

### Without API or configs

All the offline tests run on fictional data, hardcoded in the repo — so a
fresh checkout needs no API key and no network.  The mock data, by source:

- **Companies House company data**
- **The books** — example gnucash files from `example_data/`
- **HMRC** — the Corporation Tax client tests run against an in-process
  GovTalk stub gateway (submit → acknowledge → poll → response → delete),
  so no HMRC credentials are ever needed.

### Testing with a Companies House API key

The tests that hit the live Companies House API live in
`libs/ct600/src/companies_house.rs` (the `live_tests` module) and in
`apps/tally-cli/src/config.rs` (a live enrichment test: it resolves a
minimum config from the API and checks the cache is served on the second
run).  ct600's live tests are part of the **default-enabled**
`cached_live_tests` feature (they use the repository's
`.cache/api_responses`, so only the first, cold run hits the API —
`always_live_tests` forces a fresh run)
and tally-cli's enrichment test is part of its default-enabled `api_tests`
feature — plain `cargo test -p ct600` / `cargo test -p tally-cli` run them:

```bash
export COMPANIES_HOUSE_API_KEY="your-api-key"             # or COMPANIES_HOUSE_SANDBOX_API_KEY
export COMPANY_NUMBER="00000006"                          # a company that exists in the API you chose
export COMPANY_UNIQUE_TAXPAYER_REF="8596148860"                   # needed by tally-cli's enrichment test
cargo test -p ct600
cargo test -p tally-cli
```

They only fetch data on first run, and cache it for the next runs.  (The
tally-cli test additionally needs `COMPANY_UNIQUE_TAXPAYER_REF`: the Corporation
Tax reference is never resolved from Companies House.)

### Project structure

| Crate | Stage | Purpose |
|-------|-------|---------|
| [`apps/tally-cli`](apps/tally-cli/README.md) | 3–4 | CLI  for the libraries: `tally ct600` produces the CT600 message from a config file + GnuCash book (no submission yet) |
| `libs/ixbrl` | 1, 3 | GnuCash parser + FRS-105 (micro-entity) iXBRL reports |
| `libs/ct600` | 4 | HMRC GovTalk XML message builder/parser for CT600 submission + Companies House client / company resolution (`companies_house`) |

### JavaScript workspace (web app + design system)

The JS side of the repo is a pnpm workspace (`apps/*`, `packages/*`). It
currently contains the **design system**, an **Astro showcase** for it, and the
**blank web app** the real Tally UI will be built in — the app does not do
what `tally-cli` does yet.

| Package | Purpose |
|---------|---------|
| [`apps/design-system-showcase`](apps/design-system-showcase/README.md) | Astro site (one Solid island) displaying every component and token in the design system; the pattern for the future landing/docs site |
| [`apps/tally-web`](apps/tally-web/README.md) | The Tally web app (SolidJS + Vite) — a blank starter, design system already wired |
| `packages/design-system` | Token-driven design system (`@tally/design-system`): theme + components, shared by both apps (and, later, landing/docs sites) |

```bash
pnpm install     # from the repo root
pnpm dev         # run the design-system showcase (http://localhost:4321)
pnpm dev:web     # run the blank Tally web app (http://localhost:5173)
pnpm typecheck   # typecheck the whole JS workspace
```

Inside the Nix dev shell, one command runs the whole stack — Postgres, the
Rust API and the web app — with one zellij tab per process so you can follow
the logs (detach with the default zellij keybind; quitting the session stops
Postgres, whose data persists in the named volume — note that starting `dev`
also stops any dev database that's already running):

```bash
nix develop -c dev
```

For just the web app (Vite + Solid on :5173, when the API is already
running):

```bash
nix develop -c web
```

To wipe the dev database (container, data volume and uploaded ledgers) back
to a clean slate — stop a running `dev` session first:

```bash
nix develop -c reset
```

The design system is built on **design tokens** (brand/neutral scales, fonts,
semantic color roles with dark-mode variants, recipes) configured in
`packages/design-system/src/theme` — components are wired to the tokens, so
re-theming is a token change, not a component rewrite.

Note: the Cargo workspace globs `apps/*` as crates, so each JS-only app under
`apps/` needs a matching entry in the `exclude` list in the root
[`Cargo.toml`](Cargo.toml) (both JS apps are excluded so far).

**About Astro:** the design-system showcase is an Astro site (a Solid island
on `client:load`, server-rendered). The landing page and docs subdomain can be
added later the same way — Astro pages that reuse `@tally/design-system` (see
`apps/design-system-showcase/README.md`).


## `tally-cli` configuration

See [tally-cli README](apps/tally-cli/README.md#configuration) for detailed config options.

- **CLI flags** — `--config-path`, `--book` required; `--out` optional
  (defaults to the tally repo's `.cache/tally-cli` when run from the
  checkout, else `~/.cache/tally-cli`); `--accounts-made-up-to` optional.
- **Company identity** (`company.*`) — optional. Name, registration date and
  number come from Companies House when an API key is set (`COMPANY_NUMBER`
  wins for the number).
- **Tax reference (UTR)**
  `COMPANY_UNIQUE_TAXPAYER_REF` (env, wins) or `company.tax_reference`; one is required.
- **Return period** (`accounts.period`) — optional. `accounts.period`,
  `accounts.accounts_made_up_to` / `--accounts-made-up-to`, else the
  company's next accounting period from Companies House.
- **Company Profile** (`company.*`) — optional, can be auto-filled from Companies House API (needs an API key in env vars)
- **Accounts metadata** (`accounts.*`) — required: `report_date`,
  `authorised_date`, `signature_b64`. Other fields optional.

### Example configs

**With a Companies House API key** — the company block only needs the
required profile fields; the identity can be left out: the name, registration
number and the next accounting period to file (the return period) are
resolved from the company's profile at runtime (`COMPANY_NUMBER` provides
the lookup number), and the UTR comes from `COMPANY_UNIQUE_TAXPAYER_REF`:

```bash
export COMPANIES_HOUSE_API_KEY="your-api-key"   # or COMPANIES_HOUSE_SANDBOX_API_KEY
export COMPANY_NUMBER=12345678
export COMPANY_UNIQUE_TAXPAYER_REF=8596148860
```

Most company fields can be left out (resolved from calling Companies House API).
You only need to fill in the required accounts metadata (the fields that
cannot be inferred — the dates and signature;
the period can be left out and is resolved from Companies House):

```json
{
  "accounts": {
    "period": {
      "start": "2020-01-01",
      "end": "2020-12-31"
    },
    "report_date": "2021-03-01",
    "authorised_date": "2021-02-01",
    "signed_by": "B Smith",
    "average_employees": { "2020": 2, "2019": 1 },
    "signature_b64": "",      "...": "remaining accounts.* fields — copy them from example_data/basic-1/input_config.jsonc"
  }
}
```

**Without an API key** — the company block must be complete, because nothing
can be resolved at runtime:

```json
{
  "company": {
    "name": "Example Biz Ltd.",
    "tax_reference": "8596148860",
    "company_number": "12345678",
    "directors": ["A Bloggs"],      "...": "remaining company.* profile fields (optional — as in the example above)"
  },
  "accounts": {
    "period": {
      "start": "2020-01-01",
      "end": "2020-12-31"
    },
    "report_date": "2021-03-01",
    "authorised_date": "2021-02-01",
    "signed_by": "B Smith",
    "average_employees": { "2020": 2, "2019": 1 },
    "signature_b64": "",      "...": "remaining accounts.* fields — as in the example above"
  }
}
```

The UTR can also come from `COMPANY_UNIQUE_TAXPAYER_REF`, which wins over
`company.tax_reference`.

## Docs & Links

- [docs/current-state-and-roadmap.md](docs/current-state-and-roadmap.md) — what the software does, what it doesn't handle, and the roadmap

### HMRC & gov.uk
- https://www.gov.uk/company-tax-returns
- https://www.gov.uk/government/collections/corporation-tax-online-support-for-software-developers
- [HMRC CT Inline XBRL Style Guide](https://assets.publishing.service.gov.uk/government/uploads/system/uploads/attachment_data/file/434588/xbrl-style-guide.pdf)
- [Local Test Service](https://www.gov.uk/government/publications/local-test-service-and-lts-update-manager)
- [Corporation tax iXBRL (2018)](https://assets.publishing.service.gov.uk/media/5d84bef7e5274a27c2c6d5aa/CT_Online_XBRL_Technical_Pack_2.0.pdf#2#1)
- [Corp taxation manual](https://www.gov.uk/hmrc-internal-manuals/company-taxation-manual/ctm01000)
- [Corp Tax calculation](https://www.gov.uk/hmrc-internal-manuals/company-taxation-manual/ctm03955)
