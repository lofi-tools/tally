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

Produce the CT600 Corporation Tax return a UK limited company files with HMRC

```bash
tally ct600 \
  --config-path libs/ixbrl/example_data/example2/input_config.json \
  --book libs/ixbrl/example_data/example2/input.gnucash \
  --out .cache/ct600
  # -> writes .cache/ct600/ct600.xml
```

`tally` turns your accounting data into the Corporation Tax return a UK
limited company files with HMRC. You provide:

1. **A config file** (`--config-path`) — the company's identity (name,
   registration number, tax reference) and the accounts metadata
   (directors, SIC codes, address, …).  Anything left out is filled in
   automatically from the company's Companies House profile when an API
   key is set.
2. **Your books** (`--book`) — the GnuCash ledger (`input.gnucash`);

See [tally-cli configuration](#tally-cli-configuration) for the
flags, config, and defaults.


## Developer quickstart

1. Install Nix and direnv (see [`docs/installing-nix-and-direnv.md`](docs/installing-nix-and-direnv.md)).
2. `direnv allow` — loads the flake devShell (cargo, rustc, nextest, the reference Python tools, ...).
3. Produce the example CT600:

   ```bash
   cargo run -p tally-cli -- ct600 \
     --config-path libs/ixbrl/example_data/example2/input_config.json \
     --book libs/ixbrl/example_data/example2/input.gnucash \
     --out .cache/ct600
     # -> writes .cache/ct600/ct600.xml
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
- **The books** — example gnucash files from `libs/ixbrl/example_data/`
- **HMRC** — the Corporation Tax client tests run against an in-process
  GovTalk stub gateway (submit → acknowledge → poll → response → delete),
  so no HMRC credentials are ever needed.

### Testing with a Companies House API key

The tests that hit the live Companies House API live in
`libs/ct600/src/companies_house.rs` (the `live_tests` module) and in
`apps/tally-cli/src/config.rs` (a live enrichment test: it resolves a
minimum config from the API and checks the cache is served on the second
run).  Both are part of the **default-enabled** `api_tests` feature — plain
`cargo test -p ct600` / `cargo test -p tally-cli` run them:

```bash
export COMPANIES_HOUSE_API_KEY="your-api-key"             # or COMPANIES_HOUSE_SANDBOX_API_KEY
export COMPANY_NUMBER="00000006"                          # a company that exists in the API you chose
export UNIQUE_TAXPAYER_REF="8596148860"                   # needed by tally-cli's enrichment test
cargo test -p ct600
cargo test -p tally-cli
```

They only fetch data on first run, and cache it for the next runs.  (The
tally-cli test additionally needs `UNIQUE_TAXPAYER_REF`: the Corporation
Tax reference is never resolved from Companies House.)

### Project structure

| Crate | Stage | Purpose |
|-------|-------|---------|
| [`apps/tally-cli`](apps/tally-cli/README.md) | 3–4 | CLI  for the libraries: `tally ct600` produces the CT600 message from a config file + GnuCash book (no submission yet) |
| `libs/ixbrl` | 1, 3 | GnuCash parser + FRS-105 (micro-entity) iXBRL reports |
| `libs/ct600` | 4 | HMRC GovTalk XML message builder/parser for CT600 submission + Companies House client / company resolution (`companies_house`) |


## tally-cli configuration

All the flags, config-file keys and defaults baked into the message are
documented in the [tally-cli README](apps/tally-cli/README.md#configuration).
In short:

- `--config-path`, `--book` and `--out` are required; `--accounts-made-up-to`
  is optional — the flags have no environment-variable fallback;
- the company-identity fields in the config are **optional**, each with its
  own default: the name is resolved from Companies House at runtime when an
  API key is configured (the registration date too, when the config carries
  no identity details at all), the company number comes from `COMPANY_NUMBER`
  (winning) or `company.company_number`, and the first still-missing field
  fails with a clear error explaining how to resolve it;
- the Corporation Tax reference (UTR) cannot be resolved from Companies
  House: it comes from the `UNIQUE_TAXPAYER_REF` environment variable
  (winning) or the config's `company.tax_reference`, so one of the two must
  be present;
- the return period lives in the config's `accounts` sub-object and is
  optional too: an explicit `accounts.period` (both dates) or an
  `accounts.accounts_made_up_to` date in the config (or the
  `--accounts-made-up-to` flag) gives the period; otherwise it defaults to
  the company's next accounting period to file, resolved from the Companies
  House profile;
- the company's descriptive profile fields (`company.*`: directors,
  contacts, accountant/auditor, SIC codes, ...) and the accounts' report
  metadata (`accounts.*`: report title, dates, employee counts, ...) are
  all required and have no defaults (copy them from the example config
  below); the company logo (`company.logo_b64`) is the one optional asset.

### Example configs

**With a Companies House API key** — the company block only needs the
required profile fields; the identity can be left out: the name, registration
number and the next accounting period to file (the return period) are
resolved from the company's profile at runtime (`COMPANY_NUMBER` provides
the lookup number), and the UTR comes from `UNIQUE_TAXPAYER_REF`:

```bash
export COMPANIES_HOUSE_API_KEY="your-api-key"   # or COMPANIES_HOUSE_SANDBOX_API_KEY
export COMPANY_NUMBER=12345678
export UNIQUE_TAXPAYER_REF=8596148860
```

<!--```json
{
  "company": {
    "directors": ["A Bloggs"],
    "contact_name": "Corporate Enquiries",
    "address_lines": ["123 Leadbarton Street"],
    "county": "Minchingshire",
    "location": "Threapminchington",
    "postcode": "QQ99 9ZZ",
    "email": "corporate@example.org",
    "website_url": "https://example.org/corporate",
    "sic_codes": ["62020"],
    "activities": "Computer security consultancy",
    "jurisdiction": "England and Wales",
    "...": "remaining required company.* profile fields — copy them from libs/ixbrl/example_data/example2/input_config.json"
  },
  "accounts": {
    "period": {
      "start": "2020-01-01",
      "end": "2020-12-31"
    },
    "...": "remaining required accounts.* report fields — copy them from libs/ixbrl/example_data/example2/input_config.json"
  }
}
```-->

**Without an API key** — the company block must be complete, because nothing
can be resolved at runtime:

```json
{
  "company": {
    "name": "Example Biz Ltd.",
    "tax_reference": "8596148860",
    "company_number": "12345678",
    "directors": ["A Bloggs"],
    "...": "remaining required company.* profile fields (as in the example above)"
  },
  "accounts": {
    "period": {
      "start": "2020-01-01",
      "end": "2020-12-31"
    },
    "...": "remaining required accounts.* report fields (as in the example above)"
  }
}
```

The UTR can also come from `UNIQUE_TAXPAYER_REF`, which wins over
`company.tax_reference`.

## Docs & Links

- [docs/current-state-and-roadmap.md](docs/current-state-and-roadmap.md) — what the software does, what it doesn't handle, and the roadmap

### HMRC & gov.uk
- https://www.gov.uk/company-tax-returns
- https://www.gov.uk/government/collections/corporation-tax-online-support-for-software-developers
- [HMRC CT Inline XBRL Style Guide](https://assets.publishing.service.gov.uk/government/uploads/system/uploads/attachment_data/file/434588/xbrl-style-guide.pdf)
- [Local Test Service](https://www.gov.uk/government/publications/local-test-service-and-lts-update-manager)
