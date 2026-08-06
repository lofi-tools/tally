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
| `--accounts-made-up-to <date>` | date at which the accounts are made (`YYYY-MM-DD`); the return period is deduced as the 12 months ending on it (wins over the config's `accounts.accounts_made_up_to`) |

### Config file

A JSON file with the same shape as
`libs/ixbrl/example_data/example2/input_config.json` (use it as a template): a nested
`company` block (identity + descriptive profile) and an `accounts` sub-object
(period + report metadata).  This is also the config the tests and the
`tally-example2` script run against.  `libs/ixbrl/example_data/example2/minimal_config.json`
is a minimal template — no identity, no period and a blank profile, with
only the required report metadata — which the live enrichment test resolves
against the environment and the Companies House API.

| Key | Type | Default |
|-----|------|---------|
| `company.name` | string (optional) | resolved from Companies House when an API key is configured |
| `company.tax_reference` | string (required) | `COMPANY_UNIQUE_TAXPAYER_REF` environment variable wins when set; it cannot be resolved from Companies House, so one of the two must be present |
| `company.company_number` | string (optional) | `COMPANY_NUMBER` environment variable wins when set |
| `company.directors`, `company.contact_name`, `company.address_lines`, `company.email`, `company.accountant_name`, `company.auditor_name`, ... | optional | none — an omitted (or blank) field parses to `None`, serialises back as omitted, and the reports render empty values for it; when an API key is set, the fields Companies House holds (registered-office address, SIC codes, jurisdiction, current directors) are filled from the profile instead |
| `company.logo_b64` | string (optional) | none — the logo is only embedded on the title page when present |
| `accounts.period.start` | date (optional) | none — the two dates must be given together; otherwise the period is deduced from `accounts.accounts_made_up_to` or the Companies House next period |
| `accounts.period.end` | date (optional) | none — see `accounts.period.start` |
| `accounts.accounts_made_up_to` | date (optional) | the `--accounts-made-up-to` flag wins; the return period is the 12 months ending on it |
| `accounts.fy1_year` / `fy2_year` | int (optional) | 2019 / 2020 |
| `accounts.fy1_rate` / `fy2_rate` | number (optional) | 19 / 19 (percent) |
| `accounts.report_date` | date (required) | the publication date — cannot be inferred |
| `accounts.authorised_date` | date (required) | the authorisation date — cannot be inferred |
| `accounts.signed_by` | string (optional) | defaults to the first director |
| `accounts.average_employees` | object (required) | average monthly employees by calendar year — cannot be inferred (`{}` for none) |
| `accounts.signature_b64` | string (required) | base64 image of the director's signature, embedded on the statement of financial position (`""` for none) |
| `accounts.incorporation_date` | date (optional) | filled from the Companies House profile when absent |
| `accounts.accounting_standards_dimension`, `accounts.accounts_type_dimension`, `accounts.accounts_status_dimension` | string (optional) | `uk-bus:Micro-entities` / `uk-bus:AbridgedAccounts` / `uk-bus:AuditExempt-NoAccountantsReport` — the values fixed for this report |

The company-identity fields, the return period and the financial-year
parameters are all optional in the config file (see
[Resolving the company identity](#resolving-the-company-identity) for how the
missing ones are filled in and what must always be present).  The company
profile fields (`company.*`) are optional too: an omitted field parses to
`None`, serialises back as omitted, and the reports render empty values for
it — copy the example config (or start from `minimal_config.json`) and fill
in what the report should show.  With a Companies House API key, the fields
Companies House holds (registered-office address, SIC codes, jurisdiction,
current directors) are enriched from the profile instead of rendering
blank.  The company logo (`company.logo_b64`) is one such optional asset.  The accounts' report metadata that *cannot* be
inferred — the publication and authorisation dates, the employee counts and
the signature — is required in the `accounts` sub-object; the signatory
defaults to the first director.
the rest (the period, the incorporation date) is optional, and the accounts
taxonomy dimensions default to the values fixed for this report.

### Defaults baked into the message

From the libraries' `Default` implementations (no config hook yet):

| Field | Default |
|-------|---------|
| GovTalk envelope | class `HMRC-CT-CT600`, `GatewayTest` 1, username `CTUser100` / password `password`, vendor `1234`, software `ct600` 1.0.0 |
| Principal contact | `Ms Sarah McAcre`, sarah@example.org |
| Financial years / rates | FY1 2019, FY2 2020, both 19% (`AccountsMeta` defaults) |
| Declaration (boxes 975 / 985) | contact name / `Director`; box 980 (date) = today |

### Resolving the company identity

The `company` block's *identity* fields are all optional (the profile fields
are required), but the resolved identity must be complete before the return
can be built:

- the **company name** is filled in from the company's Companies House
  profile when an API key is configured (`COMPANIES_HOUSE_API_KEY`, or
  `COMPANIES_HOUSE_SANDBOX_API_KEY` for the sandbox) — the lookup is
  cache-first and never happens for a config that already names the company;
  the **registration date** is filled in too, but only when the config
  carries no identity details at all (so the accounting periods are not
  skewed by partial inputs);
- the **company number** comes from the `COMPANY_NUMBER` environment
  variable (which wins) or the config's `company.company_number`;
- the **Corporation Tax reference (UTR)** cannot be resolved from Companies
  House: it comes from the `COMPANY_UNIQUE_TAXPAYER_REF` environment variable
  (which wins) or the config's `company.tax_reference`, so one of the two
  must always be present;
- the **return period** lives in the config's `accounts` sub-object and is
  optional: an explicit `accounts.period` (both dates) wins; otherwise the
  date at which the accounts are made — the `--accounts-made-up-to` flag
  (winning) or the config's `accounts.accounts_made_up_to` — gives the 12
  months ending on it; otherwise the period defaults to the company's
  **next accounting period to file**, resolved from the Companies House
  profile (`CompaniesHouseClient::next_accounting_period`), which needs an
  API key and company number.

If the resolved identity is incomplete, the command fails on the first
missing field, explaining that problem and how to resolve it, e.g.:

```text
error: cannot resolve the config from 'config.json': company.name is missing — no Companies House API key is set, so it cannot be resolved from Companies House (set COMPANIES_HOUSE_API_KEY or COMPANIES_HOUSE_SANDBOX_API_KEY), or add company.name to the config file
```

### Environment variables

Producing the CT600 needs the Corporation Tax reference, which comes from
`COMPANY_UNIQUE_TAXPAYER_REF` (winning) or the config's `company.tax_reference`.  The
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

A config file with the optional `company.*` profile fields (directors,
contacts, accountant/auditor, ...) and the required `accounts.*` report
metadata (dates, employees, signature; the signatory defaults to the first
director) — copy
`libs/ixbrl/example_data/example2/input_config.json`, or start from
`libs/ixbrl/example_data/example2/minimal_config.json` (no identity, no
period, blank profile: with a Companies House API key, `COMPANY_NUMBER` and
`COMPANY_UNIQUE_TAXPAYER_REF`, the identity and period are resolved at
runtime) — plus a Corporation Tax reference —
either a `COMPANY_UNIQUE_TAXPAYER_REF` environment variable or the config's
`company.tax_reference` — and either a company number or a `COMPANY_NUMBER`
environment variable.  The company name and return period
are only needed when no Companies House API key is configured: with a key,
the name and the next accounting period to file are resolved from the
company's profile at runtime (the period can also be given explicitly or
deduced from `--accounts-made-up-to`).  The company logo is optional.  The
envelope credentials / contact details default to the reference tool's
values (see above).

## Building and running

```bash
# Build the binary (target/debug/tally)
cargo build -p tally-cli

# Produce the example CT600 message
cargo run -p tally-cli -- ct600 \
  --config-path libs/ixbrl/example_data/example2/input_config.json \
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
