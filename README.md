# Tally

Computes a UK limited company's accounts — the balance sheet, the profit & loss and corporation-tax figures — and produces the iXBRL accounts and CT600 corporation-tax return for filing with Companies House and HMRC.

For a high-level overview of what the software currently does, what it does not
handle, and what is planned, see [`docs/current-state-and-roadmap.md`](docs/current-state-and-roadmap.md).

## Quickstart

Produce the CT600 corporation-tax message from a company config file and a GnuCash
book:

```bash
tally ct600 \
  --config-path libs/ixbrl/example_data/example2/input-company.json \
  --book libs/ixbrl/example_data/example2/input.gnucash \
  --out .cache/ct600
  # -> writes .cache/ct600/ct600.xml
```

This writes the CT600 GovTalk message to `.cache/ct600/ct600.xml`.  Use
`libs/ixbrl/example_data/example2/input-company.json` as a template for your own
company config, and see [tally-cli configuration](#tally-cli-configuration) for the
flags, config-file keys and defaults.

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

## Developer quickstart

1. Install Nix and direnv (see [`docs/installing-nix-and-direnv.md`](docs/installing-nix-and-direnv.md)).
2. `direnv allow` — loads the flake devShell (cargo, rustc, nextest, the reference Python tools, ...).
3. Produce the example CT600:

   ```bash
   cargo run -p tally-cli -- ct600 \
     --config-path libs/ixbrl/example_data/example2/input-company.json \
     --book libs/ixbrl/example_data/example2/input.gnucash \
     --out .cache/ct600
     # -> writes .cache/ct600/ct600.xml
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

### Project structure

| Crate | Stage | Purpose |
|-------|-------|---------|
| [`apps/tally-cli`](apps/tally-cli/README.md) | 3–4 | CLI  for the libraries: `tally ct600` produces the CT600 message from a config file + GnuCash book (no submission yet) |
| `libs/ixbrl` | 1, 3 | GnuCash parser + FRS-105 (micro-entity) iXBRL reports + Companies House client / company resolution (`ixbrl::clients`) |
| `libs/ct600` | 4 | HMRC GovTalk XML message builder/parser for CT600 submission |


## tally-cli configuration

All the flags, config-file keys and defaults baked into the message are
documented in the [tally-cli README](apps/tally-cli/README.md#configuration).
In short:

- every command-line flag is required and has no environment-variable fallback;
- the company-identity fields in the config are **optional** — the name is
  resolved from Companies House at runtime when an API key is configured (the
  registration date too, when the config carries no identity details at all),
  the company number falls back on `COMPANY_NUMBER`, and anything still
  missing fails with a clear error listing the missing fields;
- the Corporation Tax reference (UTR) comes from the `UNIQUE_TAXPAYER_REF`
  environment variable (winning) or the config's `company.tax_reference`; the
  return period cannot be resolved from Companies House and must always be in
  the config file;
- the flat accounts-metadata fields are all required (copy them from the
  example config below).

### Example configs

**With a Companies House API key** — the company block only needs the return
period; the name and registration number are resolved from the company's
profile at runtime (`COMPANY_NUMBER` provides the lookup number), and the UTR
comes from `UNIQUE_TAXPAYER_REF`:

```bash
export COMPANIES_HOUSE_API_KEY="your-api-key"   # or COMPANIES_HOUSE_API_KEY_TEST
export COMPANY_NUMBER=12345678
export UNIQUE_TAXPAYER_REF=8596148860
```

<!--```json
{
  "company": {
    "accounting_period_start": "2020-01-01",
    "accounting_period_end": "2020-12-31"
  },
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
  "...": "remaining required accounts-metadata fields — copy them from libs/ixbrl/example_data/example2/input-company.json"
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
    "accounting_period_start": "2020-01-01",
    "accounting_period_end": "2020-12-31"
  },
  "directors": ["A Bloggs"],
  "...": "remaining required accounts-metadata fields (as in the example above)"
}
```

The UTR can also come from `UNIQUE_TAXPAYER_REF`, which wins over
`company.tax_reference`.

## Links

- [docs/current-state-and-roadmap.md](docs/current-state-and-roadmap.md) — what the software does, what it doesn't handle, and the roadmap
- https://www.gov.uk/company-tax-returns
- https://www.gov.uk/government/collections/corporation-tax-online-support-for-software-developers
- [HMRC CT Inline XBRL Style Guide](https://assets.publishing.service.gov.uk/government/uploads/system/uploads/attachment_data/file/434588/xbrl-style-guide.pdf)
