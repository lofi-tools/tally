# ct600

CT600 corporation-tax return builder, GovTalk messages, the HMRC Corporation Tax online client, and the Companies House client with its company-resolution chain (`companies_house`) and the CT600 adapters over it.

## Inputs → Outputs

| Input | Produces |
|---|---|
| `Frs105CorpTax` + `Frs105Accounts` (from the `ixbrl` crate) | `Ct600Return::from_inputs()` → `Ct600Return::to_xml()` = the CT600 GovTalk message (round-trips via `from_xml()`) |
| `Company` (name/number may be empty) | `companies_house::CompaniesHouseClient::resolve_company()` / `enrich_company()` → `Company` with the absent name/number filled (registration date too when no details at all; override → cache → live API) |
| `Frs105CorpTax` | `companies_house::CompaniesHouseFormValues::company_form_values()` → `CompanyFormValues` (CT600 company header boxes) |
| `GovTalkParams` | `govtalk::GovTalkSubmissionRequest` / `…Acknowledgement` / `…Poll` / `…Error` / `…Response` / `…DeleteRequest` / `…DeleteResponse`; `decode_govtalk_message()` parses responses |
| `CompanyProfile` | cached fetch via `companies_house::CompaniesHouseClient::get_company_profile_cached()` → `companies-house-{number}.json` |
| Filing id (from a `FilingHistoryItem`'s `links.self`) | `companies_house::CompaniesHouseClient::download_filing(company_number, filing_id)` → the raw document bytes, cache-first in the `filings_downloads/` subdirectory as `{number}-{period_end}-{filing_id}` |
| `FilingHistoryItem` / `FilingHistory` | typed parse via `companies_house::TypedFiling::from(&item)` / `FilingHistory::typed()` → the kind-specific structs: `AccountsFiling` (period), `ConfirmationStatementFiling` (`made_on`), `ArdChangeFiling` (`new_ard_date`), `OfficerChangeFiling` (officer + action + date), `IncorporationFiling`, `OtherFiling`; the code classifier `FormType::from_code` is the single source of truth for the code table |
| `Ct600Return` + `HmrcCorpTaxConfig` | `clients::HmrcCorpTaxClient::submit_and_poll()` → the full Document Submission Protocol lifecycle: submit → acknowledge → poll → response → delete |

## Filing with HMRC

`clients::HmrcCorpTaxClient` submits a [`Ct600Return`](crate::Ct600Return)
through the HMRC *Transaction Engine* (the Corporation Tax online service),
using the Document Submission Protocol: the GovTalk message is POSTed to the
submission endpoint, the acknowledgement carries a correlation ID and poll
interval, the client polls the response endpoint until a final response
(success or error) arrives, then sends the delete request.

Endpoints (from the official “How to use the test service” guidance and the
Transaction Engine DSP):

- **External Test Service (ETS)**: `https://test-transaction-engine.tax.service.gov.uk`
- **Live**: `https://transaction-engine.tax.service.gov.uk`

A **Test-in-live** submission uses the live endpoints with the message class
`HMRC-CT-CT600-TIL` (full validation, no registration).

Config constructors: `HmrcCorpTaxConfig::test_from_env()` (ETS),
`::live_from_env()` (live), `::test_in_live_from_env()` (Test-in-live), with
`with_*` overrides.  The config embeds the `companies_house::Config` (company
resolution / Companies House / cache), so one config drives the whole
pipeline.

Filing environment variables (`HmrcCorpTaxConfig`):

- `HMRC_CT_USERNAME` / `HMRC_CT_PASSWORD` — the gateway credentials issued by
  the Software Developers Support Team (SDST);
- `HMRC_CT_VENDOR_ID` — the 4-digit vendor ID (`ChannelRouting` `URI`);
- `HMRC_CT_SUBMISSION_URL` / `HMRC_CT_POLL_URL` — endpoint overrides;
- `HMRC_CT_CLASS` — message class override (`HMRC-CT-CT600-TIL` for TIL);
- `HMRC_CT_GATEWAY_TEST` — `1`/`true` for the test services;
- `HMRC_CT_SOFTWARE` / `HMRC_CT_SOFTWARE_VERSION`;
- `HMRC_CT_POLL_TIMEOUT` / `HMRC_CT_POLL_INTERVAL` (seconds).

The submission message is built from the `Ct600Return` with the client's
credentials, and the IRmark is computed from the message body (C14N + SHA-1,
base64) and injected before sending.

## Minimum configuration

**None** to build the return. Envelope credentials and the principal contact
default to the reference tool's `config.json`: username `CTUser100` / password
`password`, vendor `1234`, software `ct600` 1.0.0, contact `Ms Sarah McAcre`.

Companies House resolution (client + config in `companies_house`) is opt-in:

- `COMPANY_NUMBER` — company registration number. Resolution order: a full
  company override, else the cached response for this number, else a live
  API fetch.
- `COMPANIES_HOUSE_API_KEY` (live) or `COMPANIES_HOUSE_SANDBOX_API_KEY` (sandbox) —
  only needed for live lookups; without a key only cached profiles are served.
- `CT600_CACHE_DIR` — optional profile cache directory; no disk cache when unset.

## Tests

`cargo test -p ct600` runs the full suite, including the live Companies
House integration tests (`live_tests`, part of the default-enabled
`cached_live_tests` feature — they use the repository's
`.cache/api_responses`, so only the first, cold run needs a key +
`COMPANY_NUMBER` and hits the API; repeat runs are served from disk).
Enable `always_live_tests` to force a fresh network run every time.  Run
fully offline with `cargo test -p ct600 --no-default-features` (the live
tests are then reported as ignored):

- Companies House is served from hardcoded fixtures in
  `companies_house::test_utils`: company `12345678`, `EXAMPLE CORP LTD`,
  type `ltd`, created `2001-01-01`, and the Acme sample company `9876543`,
  `Acme Ltd`, created `2020-01-01` (the company paired with
  `TestData::sample_tax()`, period 2025). A cache entry shadows the fixture;
  the live API is only hit when an API key is set.
- The return is built from the ixbrl basic-1 gnucash fixture: `Example Biz
  Ltd.`, UTR `8596148860`, period `2020-01-01 → 2020-12-31`; envelope/contact
  values as above.
- The element-structure check compares against `.cache/py-ct600/ct600.xml`
  when present and skips otherwise.
- The HMRC client is tested against an in-process GovTalk stub gateway:
  submission → acknowledgement → poll → response → delete, plus the message
  build (envelope overrides + IRmark injection), config resolution, and
  gateway-error surfacing.
