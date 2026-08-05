# Current state and roadmap

This document describes what Tally currently does, what it does **not**
handle, and which mentioned-but-unimplemented features are on the roadmap.
See [`../README.md`](../README.md) for usage, and the per-crate READMEs
([`../apps/tally-cli/README.md`](../apps/tally-cli/README.md),
[`../libs/ct600/README.md`](../libs/ct600/README.md)) for details.

## What the software currently does

Tally is a set of Rust crates plus a small CLI that compute a UK limited
company's accounts and corporation-tax figures, and produce the filing
documents for Companies House and HMRC.

### The `tally` CLI (`apps/tally-cli`)

One subcommand exists: `tally ct600`, which turns a company config JSON file
plus a GnuCash ledger into the CT600 GovTalk message:

```bash
tally ct600 --config-path <config> --book <book> --out <dir>
# -> writes <dir>/ct600.xml
```

It produces the return; it does **not** submit it (see "What it doesn't
handle" below).

### The `ixbrl` crate (`libs/ixbrl`)

- **GnuCash parsing** — loads `.gnucash` books (XML or SQLite) with
  `GnucashBook::try_from_gnucash_file()`, builds the account tree, computes
  balances and net assets, and exposes the raw accounts, transactions and
  splits.
- **Company model** — `Company` identity, accounting-period derivation
  (first ARD, CT period 0/1 and subsequent years per the gov.uk rules), and
  financial-year / rate fields.
- **FRS 105 micro-entity accounts** — `Frs105Accounts` computes the
  statement of financial position (fixed/current assets, creditors, net
  assets, capital and reserves, current and previous period) and renders the
  full "Unaudited Micro-Entity Accounts" iXBRL document: title page, company
  information, balance sheet, statutory notes, approval/signature block. The
  document round-trips back into the struct via `from_ixbrl()`.
- **FRS 105 corporation tax** — `Frs105CorpTax` computes turnover, gross and
  net profit, tax expense, and the CT600 computation chain (annual
  investment allowance, adjusted trading profit, losses, gains, reliefs,
  donations, FY1/FY2 tax, marginal relief, losses of trades, etc.).
- **iXBRL formatting** — `ixbrl_fmt` helpers for building and parsing the
  tagged HTML documents.

### The `ct600` crate (`libs/ct600`)

- **Return builder** — `Ct600Return::from_inputs()` assembles the CT600
  GovTalk message: envelope, IR header, form figures (all CT600 boxes) and
  the attached iXBRL documents; `to_xml()` serialises it.
- **GovTalk messages** — submission request/acknowledgement/poll/error/
  response and delete request/response, plus `decode_govtalk_message()`.
- **HMRC Corporation Tax client** — `HmrcCorpTaxClient` runs the full
  Document Submission Protocol lifecycle (submit → poll → response →
  delete), computes and injects the IRmark (canonicalised body, SHA-1,
  base64), and supports External Test Service, live and Test-in-live
  configurations. Tested against an in-process GovTalk stub gateway.
- **Companies House client** — `companies_house` resolves/enriches a
  company (override → cached profile → live API), fetches and caches
  company profiles under `.cache/api_responses/`, and derives the CT600
  company header boxes from a profile.

### Tests

The test suites run fully offline with zero configuration: Companies House
is served from hardcoded fixtures and HMRC from an in-process stub gateway.
The FRS 105 accounts output matches the reference `ixbrl-reporter` fixture
byte for byte (modulo random element ids).

## What it doesn't handle

- **No submission from the CLI.** The `tally ct600` command writes
  `ct600.xml` only. The full HMRC submission lifecycle exists in the `ct600`
  library (`HmrcCorpTaxClient::submit_and_poll()`) but is not wired into any
  command.
- **No automatic data gathering.** The pipeline starts from an existing
  GnuCash ledger. There are no Open Banking connectors and no CSV/records
  importer for bank transactions (the `example3.csv` fixture has no code
  that reads it) — stage 1 of the pipeline described in the README is not
  present in the repository.
- **FRS 105 only.** The only reporting standard implemented is FRS 105
  (micro-entities). FRS 102, FRS 101, FRSSE and IFRS are not implemented.
- **No Companies House filing.** Accounts can be rendered to iXBRL, but the
  software does not submit accounts (or anything else) to Companies House.
- **No standalone P&L statement.** The accounts iXBRL document contains the
  micro-entity statement of financial position and notes; profit figures are
  computed for corporation-tax purposes, not rendered as a separate P&L
  page.
- **Hard-coded chart-of-accounts mapping.** The balance-sheet and tax
  computations reference specific account paths (e.g. `Assets:Capital
  Equipment`, `Accounts Receivable`, `VAT:Input`, `Income`, `Expenses`).
  Ledgers with a different account structure will not map correctly; there
  is no configurable mapping.



## Roadmap

Features that are not implemented yet:

1. **Open Banking connectors** — pull transactions from bank accounts via
   Open Banking (stage 1 of the pipeline). These previously lived in the
   removed `mk-accounts` app and are not in the repository.
2. **Loading extra transactions from local records** — importing
   transactions from local files (e.g. CSV) alongside the GnuCash book
   (stage 1 of the pipeline).
3. **Additional reporting standards** — FRS 102 (small company), FRS 101
   (reduced disclosure), FRSSE, and IFRS taxonomy mappings (stage 3; only
   FRS 105 is implemented).
4. **Submitting the CT600 from the CLI** — wire the existing
   `HmrcCorpTaxClient` submission lifecycle (ETS / live / Test-in-live) into
   `tally` so the produced return can be filed, using the `HMRC_CT_*`
   credentials.
5. **Config hooks for the baked-in defaults** — make the GovTalk envelope
   credentials, principal contact, financial years / rates and declaration
   boxes configurable instead of `Default`-baked.
