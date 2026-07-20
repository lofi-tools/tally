# Accounting

Computes a UK limited company's accounts — balance sheet, profit & loss, corporation tax — and produces iXBRL and CT600 filings for Companies House and HMRC.

## How it works

The project works as a sequence of stages:

1. **Gather data from source systems.**
   <br/> Pull transactions from bank accounts via Open Banking connectors (Starling, Nordigen, Plaid, TrueLayer, Yapily).
   <br/> Fetch exchange rates for multi-currency transactions.
   <br/> Load expenses from local records (energy bills, reimbursable costs).

2. **Build the accounts structure.**
   <br/> Classify raw transactions into a double-entry bookkeeping structure.
   <br/> Organise accounts into a tree, like in GnuCash.
   <br/> Derive the balance sheet and profit & loss statement.

3. **Map to a reporting standard.**
   <br/> Map accounts onto a regulatory taxonomy for UK filing: FRS 105 (micro-entity), FRS 102 (small company), FRS 101 (reduced disclosure), FRSSE, IFRS.
   <br/> If you already have your accounts in a `.gnucash` file, start at this step directly.

4. **Produce the CT600 filing.**
   <br/> Wrap the accounts data in the format required by HMRC for online corporation tax submission.
   <br/> Handle the full request/response lifecycle: submit, acknowledge, poll, error, delete.

## Project structure

| Crate | Stage | Purpose |
|-------|-------|---------|
| `apps/mk-accounts` | 1–2 | Fetches transactions, builds the accounts structure, computes P&L and balance sheet |
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

## Links

- https://www.gov.uk/company-tax-returns
- https://www.gov.uk/government/collections/corporation-tax-online-support-for-software-developers
