# ct600

CT600 corporation-tax return builder and GovTalk messages.  The Companies House client and company resolution live in `ixbrl::clients`; this crate re-exports them and adds the CT600 adapters.

## Inputs → Outputs

| Input | Produces |
|---|---|
| `Frs105CorpTax` + `Frs105Accounts` (from the `ixbrl` crate) | `Ct600Return::from_inputs()` → `Ct600Return::to_xml()` = the CT600 GovTalk message (round-trips via `from_xml()`) |
| `Company` (name/number may be empty) | `ixbrl::clients::CompaniesHouseClient::resolve_company()` / `enrich_company()` → `Company` with the absent name/number filled (registration date too when no details at all; override → cache → live API) |
| `Frs105CorpTax` | `companies_house::CompaniesHouseFormValues::company_form_values()` → `CompanyFormValues` (CT600 company header boxes) |
| `GovTalkParams` | `govtalk::GovTalkSubmissionRequest` / `…Acknowledgement` / `…Poll` / `…Error` / `…Response` / `…DeleteRequest` / `…DeleteResponse`; `decode_govtalk_message()` parses responses |
| `CompanyProfile` | cached fetch via `ixbrl::clients::CompaniesHouseClient::get_company_profile_cached()` → `companies-house-{number}.json` |

## Minimum configuration

**None** to build the return. Envelope credentials and the principal contact
default to the reference tool's `config.json`: username `CTUser100` / password
`password`, vendor `1234`, software `ct600` 1.0.0, contact `Ms Sarah McAcre`.

Companies House resolution (client + config in `ixbrl::clients`, re-exported here) is opt-in:

- `COMPANY_NUMBER` — company registration number. Resolution order: a full
  company override, else the cached response for this number, else a live
  API fetch.
- `COMPANIES_HOUSE_API_KEY` (live) or `COMPANIES_HOUSE_API_KEY_TEST` (sandbox) —
  only needed for live lookups; without a key only cached profiles are served.
- `CT600_CACHE_DIR` — profile cache directory (default `<repo>/.cache/api_responses`).

## Tests

`cargo test -p ct600` runs offline with zero configuration:

- Companies House is served from hardcoded fixtures in
  `ixbrl::clients::test_utils`: company `12345678`, `EXAMPLE CORP LTD`,
  type `ltd`, created `2001-01-01`, and the Acme sample company `9876543`,
  `Acme Ltd`, created `2020-01-01` (the company paired with
  `TestData::sample_tax()`, period 2025). A cache entry shadows the fixture;
  the live API is only hit when an API key is set.
- The return is built from the ixbrl example2 gnucash fixture: `Example Biz
  Ltd.`, UTR `8596148860`, period `2020-01-01 → 2020-12-31`; envelope/contact
  values as above.
- The element-structure check compares against `.cache/py-ct600/ct600.xml`
  when present and skips otherwise.
