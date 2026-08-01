# Accounting

Computes a UK limited company's accounts — balance sheet, profit & loss, corporation tax — and produces iXBRL and CT600 filings for Companies House and HMRC.

## How it works

The project works as a sequence of stages:

1. **Gather data from source systems.**
  <br/> Fetch company data from Companies House
  <br/> Pull transactions from bank accounts via Open Banking connectors (Starling, Nordigen, Plaid, TrueLayer, Yapily).
  <br/> Fetch exchange rates for multi-currency transactions.
  <br/> Load extra transactions from local records (energy bills, reimbursable costs, ...).

2. **Build the accounts structure.**
   <br/> Classify raw transactions into a double-entry bookkeeping structure.
   <br/> Organise accounts into a tree, like GnuCash.
   <br/> Derive the balance sheet and profit & loss statement.

3. **Map to a reporting standard.**
   <br/> Map accounts onto a regulatory taxonomy (iXBRL tagging standard) for UK filing: for now, FRS 105 only (micro-entity). Planned: FRS 102 (small company), FRS 101 (reduced disclosure), FRSSE, IFRS.
   <br/> If you already have your accounts in a `.gnucash` file, start at this step directly.

4. **Handle the CT600 filing.**
   <br/> Wrap the iXBRL data in the format required by HMRC for online corporation tax submission: canonicalization, GovTalk envelope, IRMark.
   <br/> Include extra CT600 form values & computations.
   <br/> Handle the full submission lifecycle: backup data to submit, submit, acknowledge, poll for success, handle errors / deletion.

## Project structure

| Crate | Stage | Purpose |
|-------|-------|---------|
| [DEPRECATED] `apps/mk-accounts` | 1–2 | Fetches transactions, builds the accounts structure, computes P&L and balance sheet |
| `libs/ixbrl` | 3 | GnuCash parser + iXBRL taxonomy definitions (FRS-105, FRS-102, etc.) |
| `libs/ct600` | 4 | HMRC GovTalk XML message builder/parser for CT600 submission |

## Developer quickstart

Setup using `direnv allow` (needs Nix and nix-direnv), then:
- Run unit tests: `utest`
- Compute accounts: `run`

## Useful commands

```bash
# Run all tests
cargo test

# Run tests for a specific crate
cargo test -p ixbrl
cargo test -p ct600

# Check compilation
cargo check --workspace
```

The ct600 tests run fully offline out of the box: the Companies House
endpoints serve from hardcoded fictional mock responses in
`libs/ct600/src/test_utils.rs`, so `cargo test -p ct600` needs no API key,
no network and no cached responses on a fresh checkout.

## Links

- https://www.gov.uk/company-tax-returns
- https://www.gov.uk/government/collections/corporation-tax-online-support-for-software-developers
