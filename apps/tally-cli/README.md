# tally-cli

`tally` generates the CT600 Corporation Tax return — the XML message a UK limited
company files with HMRC — from a company config file and a GnuCash book, writing it
to `<out>/ct600.xml`.  It produces the return; it does not submit it.

```bash
tally ct600 --config-path <config> --book <book> --out <dir>
```

## Inputs → Outputs

| Input | Produces |
|---|---|
| `--config-path <config>` — JSON config: a nested `company` identity block plus the flat `AccountsMetadata` fields (same shape as `libs/ixbrl/example_data/example2/input-company.json`) | `ixbrl::company::Company` + `ixbrl::reports::uk_frs105_accounts::AccountsMetadata` |
| `--book <book>` — a GnuCash ledger (`input.gnucash`, XML or SQLite) | `ixbrl::GnucashBook` via `GnucashBook::try_from_gnucash_file()` |
| `Frs105Accounts` (computed from the book + config) | the attached accounts iXBRL document (`accounts.to_ixbrl()`) |
| `Frs105CorpTax` (computed from the book + config) | the attached computations iXBRL document + the CT600 form figures |
| `Frs105Accounts` + `Frs105CorpTax` | `Ct600Return::from_inputs()` → `to_xml()` = the CT600 GovTalk message, written to `<out>/ct600.xml` |

## How it works

The `ct600` subcommand is dispatched through a [`Command`](src/main.rs) enum:

1. `Command::parse_cmd_args()` reads the subcommand from `argv`;
2. `Ct600Args::parse_args()` reads the `--config-path`, `--book` and `--out`
   flags (all required);
3. the config JSON is parsed into the `Company` + `AccountsMetadata` inputs;
4. the company identity is resolved: with a Companies House API key the
   name (and registration date) are filled in from the company's profile at
   runtime, and anything still missing is reported with a clear error (see
   [Resolving the company identity](#resolving-the-company-identity));
5. the GnuCash book is loaded and the FRS 105 accounts and corporation-tax
   computation are built from it;
6. `Ct600Return::from_inputs()` assembles the return (envelope + IR header +
   form figures + attached iXBRL documents) and `to_xml()` serialises the
   GovTalk message, which is written to `<out>/ct600.xml`.

There is no submission step: producing the return never contacts HMRC.

## Configuration

### Flags

All flags are required; there is no environment-variable fallback.

| Flag | Meaning |
|------|---------|
| `--config-path <config>` | JSON config: company identity + accounts metadata |
| `--book <book>` | GnuCash ledger (`input.gnucash`) |
| `--out <dir>` | output directory; the CT600 message is written to `<dir>/ct600.xml` |

### Config file

A JSON file with the same shape as
`libs/ixbrl/example_data/example2/input-company.json` (use it as a template): a
nested `company` identity block plus the flat accounts-metadata fields.

| Key | Type | Meaning |
|-----|------|---------|
| `company.name` | string | company name (optional — resolved from Companies House when an API key is configured) |
| `company.tax_reference` | string | Corporation Tax reference (UTR) — fallback: the `UNIQUE_TAXPAYER_REF` environment variable wins when set; one of the two must be present, it cannot be resolved from Companies House |
| `company.company_number` | string | Companies House registration number (optional — falls back on the `COMPANY_NUMBER` environment variable) |
| `company.accounting_period_start` | date | start of the return period (`YYYY-MM-DD`) — always required, cannot be resolved from Companies House |
| `company.accounting_period_end` | date | end of the return period (`YYYY-MM-DD`) — always required, cannot be resolved from Companies House |
| `directors`, `sic_codes`, `address_lines`, `email`, ... | — | accounts metadata (all `AccountsMetadata` fields from `libs/ixbrl`) |

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
  `COMPANIES_HOUSE_API_KEY_TEST` for the sandbox) — the lookup is
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
- the **return period** cannot be resolved from Companies House, so it must
  always be in the config file.

If the resolved identity is still incomplete, the command fails with a
message listing every missing field and how to resolve it, e.g.:

```text
error: cannot resolve the company from config 'config.json': 2 fields are still missing
  - company.name: no Companies House API key is set, so it cannot be resolved from Companies House (set COMPANIES_HOUSE_API_KEY or COMPANIES_HOUSE_API_KEY_TEST)
  - company.tax_reference: the Corporation Tax reference (UTR) cannot be resolved from Companies House; set the UNIQUE_TAXPAYER_REF environment variable or add company.tax_reference to the config file
```

### Environment variables

Producing the CT600 needs the Corporation Tax reference, which comes from
`UNIQUE_TAXPAYER_REF` (winning) or the config's `company.tax_reference`.  The
underlying libraries also read a few variables for live Companies House
resolution and HMRC submission (future features): `COMPANY_NUMBER`,
`COMPANIES_HOUSE_API_KEY` / `COMPANIES_HOUSE_API_KEY_TEST`, `CT600_CACHE_DIR`,
and the `HMRC_CT_*` submission credentials.

The Companies House client (`ixbrl::clients::CompaniesHouseClient`) caches
fetched company profiles under
`.cache/api_responses/companies-house-<number>.json` (override with
`CT600_CACHE_DIR`) and serves from the cache when available.  Company
resolution follows a strict order: a full company override wins, otherwise
the cached response for the configured company number (`COMPANY_NUMBER`) is
used, otherwise the profile is fetched from the live API.

## Minimum configuration

A config file containing the return period and the accounts metadata, plus a
Corporation Tax reference — either a `UNIQUE_TAXPAYER_REF` environment
variable or the config's `company.tax_reference` — and either a company number
or a `COMPANY_NUMBER` environment variable.  The company name is only needed
when no Companies House API key is configured.  The envelope credentials /
contact details default to the reference tool's values (see above).

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

This crate currently has no tests of its own.  The pipeline it drives is
covered by the library suites, which run fully offline with zero configuration:

```bash
cargo test -p ixbrl
cargo test -p ct600
```
