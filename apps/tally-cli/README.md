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
4. the GnuCash book is loaded and the FRS 105 accounts and corporation-tax
   computation are built from it;
5. `Ct600Return::from_inputs()` assembles the return (envelope + IR header +
   form figures + attached iXBRL documents) and `to_xml()` serialises the
   GovTalk message, which is written to `<out>/ct600.xml`.

There is no submission step: producing the return never contacts HMRC or
Companies House.

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
| `company.name` | string | company name |
| `company.tax_reference` | string | Corporation Tax reference (UTR) |
| `company.company_number` | string | Companies House registration number |
| `company.accounting_period_start` | date | start of the return period (`YYYY-MM-DD`) |
| `company.accounting_period_end` | date | end of the return period (`YYYY-MM-DD`) |
| `directors`, `sic_codes`, `address_lines`, `email`, ... | — | accounts metadata (all `AccountsMetadata` fields from `libs/ixbrl`) |

All fields are required — the parser applies no defaults, so copy the example
config and edit it.

### Defaults baked into the message

From the libraries' `Default` implementations (no config hook yet):

| Field | Default |
|-------|---------|
| GovTalk envelope | class `HMRC-CT-CT600`, `GatewayTest` 1, username `CTUser100` / password `password`, vendor `1234`, software `ct600` 1.0.0 |
| Principal contact | `Ms Sarah McAcre`, sarah@example.org |
| Financial years / rates | FY1 2019, FY2 2020, both 19% (`Company::new` defaults) |
| Declaration (boxes 975 / 985) | contact name / `Director`; box 980 (date) = today |

### Environment variables

Producing the CT600 needs none.  The underlying libraries read a few variables
for live Companies House resolution and HMRC submission (future features):
`COMPANY_NUMBER`, `COMPANIES_HOUSE_API_KEY` / `COMPANIES_HOUSE_API_KEY_TEST`,
`CT600_CACHE_DIR`, and the `HMRC_CT_*` submission credentials.

## Minimum configuration

**None** to produce the return: the config file + book + output directory are
all CLI flags, and the envelope credentials / contact details default to the
reference tool's values (see above).

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
