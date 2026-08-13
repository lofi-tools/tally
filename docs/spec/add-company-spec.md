# Add-company simplification spec

Status: draft (interviewed; no code changed yet)
Scope: `apps/tally-web` + `apps/tally-api`
Related: `docs/spec/web-api-wiring-spec.md` (§7 add-company), `docs/spec/api-backend-spec.md` (§5, §6, §7)

## Summary

Onboarding a company currently asks for a UTR and a filing period. Neither belongs
at that moment:

- **UTR** is only consumed when a return is filed with HMRC — the corp-tax report and
  the CT600 carry it as the company's HMRC identity; the accounts report does not.
  Requiring it at add time blocks a fast onboarding, and it is only needed when the
  user actually prepares a filing.
- **Period** is derived data. The return period follows the company's accounting
  reference date (Companies House's next-accounts expectation when available, else
  the anniversary schedule from the registration date). The dialog's period inputs
  are dead weight — they are never sent to the API (the report request resolves the
  period server-side).

This change strips both from the add-company flow, moves the UTR requirement to
report-generation time (corp-tax + CT600), and makes the report period resolve
automatically from the company's registration data — with the optional explicit
override preserved for power users.

## Decisions (from interview, 2026-08)

1. **UTR field is removed from the add-company flow entirely** — no input, no
   submit-time validation, no "optional with note" variant.
2. **The Settings profile field is the home for UTR.** The Settings view already
   has a "Unique Taxpayer Reference" input; it stays, and becomes the only place a
   user enters it. (Settings persistence is currently a mock — see Out of scope.)
3. **Report generation gates on UTR.** The corp-tax and CT600 endpoints return a
   validation error when `tax_reference` is missing **or** invalid. The accounts
   report is unaffected (it carries no UTR).
4. **Period guess rule** (server-side, unchanged precedence, one branch fixed):
   explicit `period` → `made_up_to` → CH next-accounts (only when a CH key is set
   and the company has a number) → registration-date anniversary schedule. The
   registration-date branch must **no longer require a CH key** (today it errors
   `companies_house_key_missing` when no key is set — see Current state).
5. **The guessed period is not surfaced in the add flow.** It is computed silently
   at report time.
6. **The optional `period` / `made_up_to` override stays in the report request
   body.** The default (absent) resolves from registration data.
7. **The accounting-standard select (FRS 105 / FRS 102) stays in the add dialog.**
   Explicitly out of scope for this change.
8. **There is no such thing as a "local company".** The search endpoint is
   deliberately unprotected, so every added company is a real company from the CH
   API. A signed-out company is simply one not yet attached to a user account or
   stored on the backend; it is attached and stored when the user creates an
   account (existing migration path). Consequently the registration date is always
   real data (the search result's `date_of_creation`) and must be captured so the
   period guess has an anchor even without a backend CH key.

## Current state (as of writing)

### UI — `apps/tally-web`

- `components/AddCompanyDialog.tsx` review step collects:
  - **UTR** — required; `submit()` toasts "UTR required" and aborts when empty.
  - **Accounting standard** — FRS 105 / FRS 102 select (kept, out of scope).
  - **Period start / Period end** — two date inputs; **never sent to the API**
    (`App.addCompany` POSTs only `{ name, company_number, tax_reference }`).
- `App.addCompany` stores a local `Company` (`mock_data.ts`:
  `{ id, name, companyNumber, utr, sic, address, standard }`) in localStorage
  (`db.ts`) when signed out; `createCompany` when signed in.
- `components/SignInDialog.tsx` migration sends `{ name, company_number,
  tax_reference: c.utr }` when attaching a signed-out company to a new account.
- `views/Settings.tsx` has the UTR input (mock save) and a "Filing preferences"
  section with a default-standard select.
- `views/Filings.tsx` is entirely mock (Preview / File now toast "lands with the
  backend"); no report endpoint is called from the UI yet.

### API — `apps/tally-api`

- `companies.rs` — `CompanyInput` is all-optional; `validate_create` requires only
  `name`. `create()` already stores `registration_date` when present in the body
  and enriches it from CH when a number + key are set. **No create-side change
  needed** for the web to send `registration_date`.
- `period.rs` — `resolve_period`: explicit `period` → `made_up_to` →
  `resolve_from_ch`. The CH branch **errors `companies_house_key_missing`** when no
  key is set (or `missing_company_number`), even though a pure registration-date
  guess is possible from stored data.
- `reports.rs` — four endpoints; `to_meta` requires only `report_date` /
  `authorised_date`. Corp-tax JSON includes `tax_reference`; the CT600 carries it
  in the return. **No UTR validation exists anywhere.**
- Tests: `tests/reports.rs` seeds `tax_reference: "1234567890"` and passes an
  explicit `period`; `report_without_period_needs_a_ch_key` currently asserts the
  `companies_house_key_missing` error (will flip — see Changes).

## Changes

### Web — `apps/tally-web`

1. **`components/AddCompanyDialog.tsx`**
   - Remove the UTR `Field.Root`, the required-UTR submit guard, and the
     "Not held by Companies House — used for the CT600" helper text.
   - Remove the Period start / Period end date inputs and their `initialForm`
     keys.
   - `NewCompanyInput` becomes:
     `{ name, companyNumber, sic, address, standard, registrationDate? }` where
     `registrationDate` is populated from the selected `SearchItem.date_of_creation`.
   - Keep the accounting-standard select unchanged.
   - Review-step description ("…add what a search can't tell us") still holds for
     the standard select; adjust wording if it references the removed fields.
2. **`App.tsx` (`addCompany`)**
   - Signed-in path: `createCompany({ name, company_number, tax_reference: '',
     registration_date: input.registrationDate ?? undefined })` — omit
     `tax_reference` if the API layer types allow, so the UTR stays blank until the
     user sets it in Settings; **send `registration_date`** so the stored row has an
     anchor for the period guess even when the backend has no CH key.
   - Local `Company` (mock shape): keep `utr` (default `''`) so Settings still
     binds and migration still sends it; **add `registrationDate`** (or a matching
     field) so the period guess works for signed-out companies too. `standard`
     unchanged.
3. **`mock_data.ts`** — extend the `Company` interface with the registration-date
   field (keep `utr`); demo company: `registrationDate` can stay unset.
4. **`components/SignInDialog.tsx`** — include `registration_date: c.registrationDate`
   in the migration payload when present.
5. **`views/Settings.tsx`** — unchanged (UTR input stays; persistence is mock).
6. **`views/Filings.tsx`** — unchanged (mock; the CT600 UTR gate surfaces when the
   UI starts calling the report endpoints).

### API — `apps/tally-api`

1. **`period.rs` — registration-date fallback without a CH key.**
   - `resolve_period` becomes: explicit `period` → `made_up_to` → CH next-accounts
     (only when `state.ch` is set **and** the company has a number) → registration-
     date schedule from the stored `registration_date` (the pure form of
     `next_accounting_period_from` with no profile).
   - Remove the `companies_house_key_missing` / `missing_company_number` errors
     from the report path. If neither an explicit period nor a stored
     `registration_date` exists, return a `validation_failed` error pointing at
     `registration_date` (tell the user to set it or pass an explicit period).
   - Keep `resolve_from_ch` only as the CH-enhancement branch (or fold it in);
     `period_from_request` and its unit tests are unchanged.
2. **`reports.rs` — UTR gate on corp-tax + CT600.**
   - New shared check applied by `corp_tax`, `corp_tax_json`, and `ct600` (not
     `accounts`): `tax_reference` must be present and exactly **10 ASCII digits**.
   - Errors (both 422 `validation_failed` envelopes, per §11.3):
     - missing → `FieldIssue { field: "tax_reference", reason: "required to
       generate this report" }`
     - malformed → `FieldIssue { field: "tax_reference", reason: "must be a
       10-digit number" }`
   - Validate before building the report; existing report tests already seed a
     valid 10-digit UTR, so they keep passing.
3. **`companies.rs`** — no change (name-only validation stays; `registration_date`
   already flows through `create_row`).
4. **Tests (`tests/reports.rs` etc.)**
   - Flip `report_without_period_needs_a_ch_key`: a report with no explicit period
     and no CH key now resolves from the seeded `registration_date` and succeeds.
   - Add: missing-UTR 422 on corp-tax and CT600; malformed-UTR 422 (e.g. 9 digits,
     or non-numeric); period resolved from registration date when no CH key is set
     and no period given.
   - Existing report tests keep their explicit `period` (override still honored)
     and their 10-digit `tax_reference`.

## Edge cases

- **Company created with a number but backend has no CH key**: `registration_date`
  sent by the web from the search result fills the gap (create-side); the period
  guess then works. If a company somehow has neither, the report request must carry
  an explicit `period`/`made_up_to` or it fails with the `registration_date`
  validation error.
- **Company created without a company number at all** (API/CLI path): CH branch is
  skipped; registration-date schedule still applies if a date is stored.
- **Signed-out add**: the local record carries `registrationDate` from the search
  result; reports are still mock in the UI, and when the company is attached on
  registration the date is sent to the backend with the rest of the profile.
- **Existing users** who already added a company with a UTR: nothing changes; the
  stored UTR keeps flowing into corp-tax/CT600. Existing signed-out records without
  `registrationDate` will have an empty guess anchor until re-added or set — note
  this is acceptable (reports are mock at this stage).

## Verification

- `pnpm --filter @tally/web typecheck` (and the smoke script if it is green on the
  tree — it is currently failing pre-existing in jsdom).
- `cargo test -p tally-api` — unit + pg suite (the two flipped/added tests above).
- Manual: add a company without a UTR → succeeds; Settings shows the empty UTR;
  corp-tax/CT600 without UTR → 422 `validation_failed`; with a 10-digit UTR →
  report generates; report with no period and no CH key → period comes from the
  stored registration date.

## Out of scope / follow-ups

- HMRC submission UI (Filings "File now") — still mock; the backend UTR gate will
  surface when the UI wires the report endpoints (`api.ts` already has
  `generateReportDocument`).
- Wiring the Settings "Filing preferences" standard select (or the UTR save) to the
  API.
- Surfacing the guessed period in the UI (explicitly rejected for now).
