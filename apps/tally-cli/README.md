# tally-cli

`tally` generates the CT600 Corporation Tax return — the XML message a UK limited
company files with HMRC — from a company config file and a GnuCash book, writing it
to `<out>/ct600.xml`.  It produces the return; it does not submit it.

```bash
tally ct600 --config-path <config> --book <book> --out <dir>
```

## How it works

`tally` turns your accounting data into the Corporation Tax return a UK
limited company files with HMRC. You provide:

1. **A config file** (`--config-path`) — the company's identity (name,
   registration number, tax reference) and the accounts metadata
   (directors, SIC codes, address, …).  Anything left out is filled in
   automatically from the company's Companies House profile when an API
   key is set — the name, the registration date, and the accounting period
   to file if you didn't specify one.  Anything still missing is reported
   with a clear error (see
   [Resolving the company identity](#resolving-the-company-identity));
2. **Your books** (`--book`) — the GnuCash ledger (`input.gnucash`);
3. **An output directory** (`--out`) — where the CT600 message is written
   (`<out>/ct600.xml`).

From those, `tally` reads the book, builds the accounts and the
corporation-tax calculation, and assembles everything into the CT600 XML
message — nothing else is needed.

That's all it does: it *produces* the return, it never submits it — filing
it with HMRC is a separate step.

## Configuration

### Flags

`--config-path`, `--book` and `--out` are required; `--accounts-made-up-to`
is optional.  There is no environment-variable fallback.

| Flag | Meaning |
|------|---------|
| `--config-path <config>` | JSON config: company identity + accounts metadata |
| `--book <book>` | GnuCash ledger (`input.gnucash`) |
| `--out <dir>` | output directory; the CT600 message is written to `<dir>/ct600.xml` |
| `--accounts-made-up-to <date>` | date at which the accounts are made (`YYYY-MM-DD`); the return period is deduced as the 12 months ending on it (wins over the config's `company.accounts_made_up_to`) |

### Config file

A JSON file with the same shape as
`libs/ixbrl/example_data/example2/input-company.json` (use it as a template): a
nested `company` identity block plus the flat accounts-metadata fields.

| Key | Type | Default |
|-----|------|---------|
| `company.name` | string (optional) | resolved from Companies House when an API key is configured |
| `company.tax_reference` | string (required) | `UNIQUE_TAXPAYER_REF` environment variable wins when set; it cannot be resolved from Companies House, so one of the two must be present |
| `company.company_number` | string (optional) | `COMPANY_NUMBER` environment variable |
| `company.accounting_period_start` | date (optional) | none — the two dates must be given together; otherwise the period is deduced from `accounts_made_up_to` or the Companies House next period |
| `company.accounting_period_end` | date (optional) | none — see `accounting_period_start` |
| `company.accounts_made_up_to` | date (optional) | the `--accounts-made-up-to` flag wins; the return period is the 12 months ending on it |
| `directors`, `sic_codes`, `address_lines`, `email`, ... | required | — |

The company-identity fields are all optional in the config file (see
[Resolving the company identity](#resolving-the-company-identity) for how the
missing ones are filled in and what must always be present).  The flat
accounts-metadata fields are all required — the parser applies no defaults, so
copy the example config and edit it.

### Defaults baked into the message

From the libraries' `Default` implementations (no config hook yet):

| Field | Default |
|-------|---------|
| GovTalk envelope | class `HMRC-CT-CT600`, `GatewayTest` 1, username `CTUser100` / password `password`, vendor `1234`, software `ct600` 1.0.0 |
| Principal contact | `Ms Sarah McAcre`, sarah@example.org |
| Financial years / rates | FY1 2019, FY2 2020, both 19% (`Company::new` defaults) |
| Declaration (boxes 975 / 985) | contact name / `Director`; box 980 (date) = today |

### Resolving the company identity

The `company` block's fields are all optional, but the resolved identity must
be complete before the return can be built:

- the **company name** is filled in from the company's Companies House
  profile when an API key is configured (`COMPANIES_HOUSE_API_KEY`, or
  `COMPANIES_HOUSE_SANDBOX_API_KEY` for the sandbox) — the lookup is
  cache-first and never happens for a config that already names the company;
  the **registration date** is filled in too, but only when the config
  carries no identity details at all (so the accounting periods are not
  skewed by partial inputs);
- the **company number** comes from `company.company_number`, falling back
  on the `COMPANY_NUMBER` environment variable;
- the **Corporation Tax reference (UTR)** cannot be resolved from Companies
  House: it comes from the `UNIQUE_TAXPAYER_REF` environment variable
  (which wins) or the config's `company.tax_reference`, so one of the two
  must always be present;
- the **return period** is optional: an explicit `accounting_period_start`
  + `accounting_period_end` in the config wins; otherwise the date at which
  the accounts are made — the `--accounts-made-up-to` flag (winning) or the
  config's `company.accounts_made_up_to` — gives the 12 months ending on it;
  otherwise the period defaults to the company's **next accounting period to
  file**, resolved from the Companies House profile
  (`CompaniesHouseClient::next_accounting_period`), which needs an API key
  and company number.

If the resolved identity is still incomplete, the command fails with a
message listing every missing field and how to resolve it, e.g.:

```text
error: cannot resolve the company from config 'config.json': 2 fields are still missing
  - company.name: no Companies House API key is set, so it cannot be resolved from Companies House (set COMPANIES_HOUSE_API_KEY or COMPANIES_HOUSE_SANDBOX_API_KEY)
  - company.tax_reference: the Corporation Tax reference (UTR) cannot be resolved from Companies House; set the UNIQUE_TAXPAYER_REF environment variable or add company.tax_reference to the config file
```

### Environment variables

Producing the CT600 needs the Corporation Tax reference, which comes from
`UNIQUE_TAXPAYER_REF` (winning) or the config's `company.tax_reference`.  The
underlying libraries also read a few variables for live Companies House
resolution and HMRC submission (future features): `COMPANY_NUMBER`,
`COMPANIES_HOUSE_API_KEY` / `COMPANIES_HOUSE_SANDBOX_API_KEY`, `CT600_CACHE_DIR`,
and the `HMRC_CT_*` submission credentials.

The Companies House client (`ct600::CompaniesHouseClient`) caches
fetched company profiles under
`.cache/api_responses/companies-house-<number>.json` (override with
`CT600_CACHE_DIR`) and serves from the cache when available.  Company
resolution follows a strict order: a full company override wins, otherwise
the cached response for the configured company number (`COMPANY_NUMBER`) is
used, otherwise the profile is fetched from the live API.

## Minimum configuration

A config file containing the accounts metadata, plus a Corporation Tax
reference — either a `UNIQUE_TAXPAYER_REF` environment variable or the
config's `company.tax_reference` — and either a company number or a
`COMPANY_NUMBER` environment variable.  The company name and return period
are only needed when no Companies House API key is configured: with a key,
the name and the next accounting period to file are resolved from the
company's profile at runtime (the period can also be given explicitly or
deduced from `--accounts-made-up-to`).  The envelope credentials / contact
details default to the reference tool's values (see above).

## Building and running

```bash
# Build the binary (target/debug/tally)
cargo build -p tally-cli

# Produce the example CT600 message
cargo run -p tally-cli -- ct600 \
  --config-path libs/ixbrl/example_data/example2/input-company.json \
  --book libs/ixbrl/example_data/example2/input.gnucash \
  --out .cache/ct600
```

## Tests

The crate's config tests (`cargo test -p tally-cli`) cover the resolution
pipeline: the config-file / environment / flag merge, the company-identity
resolution and the return-period fallbacks.  They run fully offline — the
Companies House lookups are served from scratch cache fixtures.  The pipeline
it drives is additionally covered by the library suites (`cargo test -p
ixbrl`, `cargo test -p ct600`), which also run offline with zero
configuration.
