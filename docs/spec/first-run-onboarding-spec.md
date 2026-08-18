# First-run onboarding & empty state — spec

Status: **Draft for review** · App: `apps/tally-web` (SolidJS + Panda + `@tally/design-system`) · Data: mock, no backend yet.

> Scope note: this spec is the result of an interview with the product owner.
> It replaces the naive "blocked full-screen 'add your company'" reading of the
> original request with a **demo-first, no-login** model (see §3 and §4).

---

## 1. Problem

A brand-new user who opens Tally has nothing to work with. The naive response is a
blocked full-screen wizard ("add a company or you can't proceed"). The product
owner's actual intent is different: **the product should demo itself before the
user commits to anything**, then gently funnel them from *exploration* → *their own
company* → *saved account* → *real data*.

Two concrete problems to solve:

1. **Cold start** — zero user companies and no onboarding guidance is a dead screen
   that teaches nothing.
2. **Demo/real confusion** — if demo data ships, users must never mistake it for
   their own, and it must retire once real data exists.

## 2. Goals & non-goals

### Goals
- The app is fully usable **without login** (no auth gate).
- A **demo company with pre-seeded data** is present from the first visit so
  every feature is explorable.
- A **strong, persistent hint** drives the user to: add their own company → save
  progress (create account, any time) → add data sources.
- Adding a company goes through a **simulated Companies House search** (real
  endpoint forwards to Companies House later; results are mocked for now), with
  the user supplying only the fields a search can't infer.
- A **simulated account** flow ("create account") flips the app into a "saved"
  state.
- **localStorage is the mock database**: companies, data sources and settings
  persist across refreshes.

### Non-goals (this iteration)
- Real auth / real Companies House API / real Open Banking (mock toasts only).
- GnuCash **file upload** UI (see §7.3 — guidance copy only for now, unless
  explicitly scoped in).
- The blocked full-screen onboarding screen: it is reduced to a **safety net**
  (§4.4) because the demo company is always present.
- Multi-step "guided setup wizard" for banks/team — the funnel stops at "company
  added", then uses in-app nudges (§6).

## 3. How other apps handle this (patterns & practice)

Research notes compiled from established onboarding/empty-state practice
(Xero, QuickBooks, FreeAgent, Wave, Stripe Dashboard, Linear, Notion, Figma,
Slack; NNGroup empty-state & progressive-disclosure guidance). Live web
fetching was unavailable during this interview; treat these as well-known
patterns to validate, not fresh citations.

- **Xero / FreeAgent** — classic blocked **full-screen setup wizards** on first
  login: business details → bank → contacts/products, with a progress indicator.
  Extremely effective at completion, but high friction; visitors can't explore
  before committing.
- **QuickBooks Online** — *guided setup checklist* plus a **dashboard-first**
  approach; ships an optional **"sample company" file** so users can explore
  without entering real data.
- **Wave** — dashboard-first with task-list empty states ("connect a bank", "add
  income"); light guidance, no hard gate.
- **Stripe Dashboard** — no forced wizard. Dashboard renders with guided
  **empty states** ("Add your first…") and a persistent **setup checklist**;
  each surface teaches its own next action.
- **Linear / Notion** — account-scoped **workspace creation** at signup, then a
  *dismissible* quickstart checklist; Notion offers sample/template workspaces.
- **Figma / Slack / Notion** — the **"explore with sample data"** pattern: demo
  content present by default, clearly labeled, so the product demos itself; real
  setup is one action away.
- **NNGroup empty-state guidance** — explain *why* the surface is empty, show the
  *first action* to take, keep it to **one primary CTA**, and use progressive
  disclosure instead of dumping every setup step at once.

**Conclusion for Tally:** the owner's model (demo-first, no gate) is closest to
**QuickBooks' sample company + Stripe's guided empty states + the Figma/Slack
sample-workspace pattern** — with one hard rule borrowed from all demo-data
products: *demo data must be unmistakably labeled and must retire as soon as
real data arrives*.

## 4. Product model & user states

### 4.1 No-login, demo-first
- The app opens without auth. On first visit it shows the **demo company**
  (`Demo Co Ltd`) with the existing seeded dataset (transactions,
  monthly summaries, data sources, filings, payroll) so every view has content.
- **Zero user companies on startup.** The current `seedCompanies` seed is
  removed; the demo company is a distinct, always-present entity, not part of
  the user's company list.

### 4.2 The funnel (the "strong hint")
The user is guided through three steps, in order, at their own pace:

1. **Add your own company** — persistent banner + picker badge (see §5.1).
2. **Save your progress — create an account** — available *any time* after a
   company exists; simulated flow (§7.1).
3. **Add data sources** — in-app nudges once a company exists (§6).

### 4.3 Demo removal rule
- The demo company **stays in the picker**, clearly badged, as long as no user
  company has any connected data source.
- Once **any** user company has **≥1 connected data source** (bank, GnuCash/CSV,
  or manual — any kind), the demo company is removed from the picker.
- If the user's only connected sources are later disconnected/removed and no
  source remains, the demo does **not** return (one-way retirement, simplest
  rule; revisit if needed).

### 4.4 The blocked screen is a safety net
- The original "whole screen dedicated to adding a company, menus inaccessible"
  requirement now applies **only** to the hypothetical state where **zero
  companies of any kind exist** (e.g. demo removed by a future power-user
  action, or fresh data wipe). It should be implemented as a minimal fallback
  ("Add your first company"), not the primary experience.
- Because the demo is always present, this state should not occur in practice.

### 4.5 Company auto-selection
- After the user adds their **first** company, it is **auto-selected** and the
  view switches to that company's Accounts. Subsequent adds keep the current
  selection unless the added company is chosen.

## 5. Demo identity & first-run chrome

### 5.1 Demo badge + banner (chosen option: "Tag + banner")
- **Picker badge:** the demo company shows a small `Demo` tag in the company
  picker (and in the picker's selected-company summary).
- **Dismissible banner:** a banner at the top of the main content area:
  `Demo data — add your company to get started.` with a primary action
  "Add company" and a dismiss (×) button. The banner stays until **a real
  company exists** (dismissal persists in localStorage; re-appears if the
  dismissal is reset with the data).
- **No first-visit modal** (chosen option: "Persistent banner only").

### 5.2 Voice
- **Minimal & direct** copy everywhere (§8 for drafts). No brand-y warmth, no
  marketing claims — tool-like, one step per message.

## 6. In-app guidance after a company exists ("Both, lightly")

- **Guided empty states** as the default treatment on data-less surfaces:
  - **Integrations** empty state: two actions — "Connect a bank" and "Upload a
    GnuCash ledger".
  - **Accounts** empty transactions state: points to Integrations ("Connect a
    bank or upload a ledger to populate your books").
- **Toast nudge** after adding a company: `Connect a bank or upload your ledger
  to start.` (informational toast; dismissed normally).
- No persistent checklist in this iteration.

## 7. Add-company flow (simulated Companies House search)

### 7.1 Trigger points
- Banner "Add company" action · company-picker "Add company…" item · (keyboard:
  see §9.3).

### 7.2 Flow
1. **Search screen** — a text input ("Search Companies House") with a search
   button (Enter submits). Simulated results come from a local fixture list
   (reuse `bankOptions`-style mock pattern; ~6–8 plausible UK company names with
   company numbers, SIC, incorporation date, jurisdiction, registered address).
   The mock notes: *"the real search endpoint will forward to Companies House."*
2. **Pick a result** → a **review screen** prefilled with everything the search
   can infer (see 7.3), with the non-inferable fields empty/editable.
3. **Fill the gaps** → submit → company created, auto-selected, toast nudge
   (§6), demo still present until a data source connects (§4.3).
4. Search returning nothing → inline empty state ("No company found — check the
   name or number").

### 7.3 Fields: inferable vs not (grounded in `example_data/*/input_config.jsonc`)
Inferable from a Companies House search (prefilled, still editable):

| Field | Config key |
|---|---|
| Company name | `company.name` |
| Company number | `company.company_number` |
| Registered office (lines, county, location, postcode) | `company.address_lines` / `county` / `location` / `postcode` |
| SIC codes | `company.sic_codes` |
| Jurisdiction | `company.jurisdiction` |
| Incorporation date | `accounts.incorporation_date` |

Must be supplied by the user (not inferable):

| Field | Config key | Notes |
|---|---|---|
| UTR (tax reference) | `company.tax_reference` | HMRC-held; required |
| Accounting standard | `accounts.accounting_standards_dimension` | FRS 105 / FRS 102 picker |
| Accounting period (start/end) | `accounts.period` | prefill from ARD when known; otherwise a date range |
| Report / authorised dates | `accounts.report_date` / `authorised_date` | derive or leave default |
| Optional profile data | `contact_*`, `email`, `phone_*`, `vat_registration`, directors, accountant | collapsible "More details" section |

- Empty optional fields render empty in the accounts output (per the config
  model), so **only UTR + standard + period are required**.
- Future: the real CH profile API can fill more (directors, contact) — keep the
  field list aligned to the config schema so the swap is seamless.

## 8. Simulated account ("save your progress")

- After a company exists, a **persistent, quiet affordance** appears (e.g. a
  footer chip in the sidebar: "Save your progress" + kbd hint, or a link in the
  banner once dismissed): *"Create an account to save your progress."*
- **Simulated flow:** a dialog with name + email (+ optional "already have an
  account?" stub). Submitting flips the app into an **account-saved state**
  (badge/toast: "Progress saved — auth lands with the backend"), persists the
  flag to localStorage, and the affordance becomes a "Signed in (mock)" chip.
- No real auth, no validation beyond required fields.

## 9. Persistence (localStorage as the mock database)

Chosen option: **"Everything local."** Treat localStorage as the mock DB:

- **Companies** added by the user (id, name, number, UTR, standard, period,
  profile fields).
- **Data sources** added via "Add bank account" (current `extra()` state moves
  into storage).
- **Settings** (existing `preferences` + any UI prefs).
- **Account flag** (from §8) and **banner dismissal**.
- The **demo company is never persisted** — always re-created in memory.
- Migration/schema: versioned key (e.g. `tally.db.v1`) with a light
  read/validate/reset on load; clear on "reset demo data" if one is added.

## 10. Copy drafts (minimal & direct)

- Banner: `Demo data — add your company to get started.` · action: `Add company`
- Search title: `Add company` · helper: `Search Companies House` · empty:
  `No company found. Check the name or number.`
- Review gaps: `Add what the search can't tell us.` · required notes:
  `UTR, accounting standard and period are needed to file.`
- Post-add toast: `Company added — connect a bank or upload your ledger to start.`
- Account affordance: `Create an account to save your progress.` · dialog title:
  `Save your progress` · success toast: `Progress saved (mock — real auth lands
  with the backend).`
- Integrations empty state: `Connect a bank or upload a GnuCash ledger to pull
  in transactions.`

## 11. Design constraints (per DESIGN.md)

- Dark-first; `canvas` `#111110`; hairline `border`; one brown accent
  (`brown.9`) for the single primary CTA; white text on accent.
- Banner: raised surface (`gray.surface.bg`), hairline border, `l3` radius;
  dismiss × as `IconButton`; content left, action right; stays out of the
  density (compact height).
- Demo badge: `Badge` variant `subtle` with `colorPalette="amber"` or gray —
  pick one; keep the same label everywhere (`Demo`).
- Empty states: icon + one/two actions, per `EmptyState` component; mono `kbd`
  chips for shortcuts.
- Keyboard-first: search auto-focuses; Enter submits; Esc closes dialogs; the
  "Add company" action reachable via a documented shortcut (see §11 note) —
  consistent with existing 1–5 nav shortcuts.
- Motion: 100–180 ms, transforms/opacity only (Park animation styles).

## 12. Edge cases

- Search yields the demo company's name → fine (user can re-add; dedupe only
  by exact company number within user companies — see next).
- **Duplicate company number** among user companies → warn + block (toast:
  "Already added").
- User adds a company, **doesn't create an account**, refreshes → with "everything
  local", the company persists anyway; the account affordance remains. (Accept:
  the account is a save-progress *story*, not a hard gate.)
- Banner dismissed but no company added → stays dismissed until data reset.
- All data sources removed after demo retirement → demo stays retired (§4.3).
- Deleting companies: **out of scope** (no delete UI yet). The safety-net screen
  (§4.4) covers the hypothetical zero-everything state.

## 13. Acceptance criteria

1. Fresh load (no localStorage): app shows the demo company, all views populated,
   `Demo` badge in the picker, dismissible banner present.
2. Banner "Add company" opens the search flow; simulated results render; picking
   one prefills inferable fields; UTR + standard + period required; submit adds
   the company and **auto-selects** it.
3. Post-add toast appears; banner hides once a real company exists; demo still
   in picker (badged).
4. Connecting any data source on any user company removes the demo from the
   picker.
5. "Create an account" dialog → account-saved state persists across refresh;
   companies/data sources/settings all persist across refresh.
6. Zero-company state (data cleared) shows the safety-net add-company screen.
7. Typecheck, build and jsdom smoke pass.

## 14. Out of scope / future

- Real Companies House + Open Banking + auth.
- GnuCash/CSV **file upload** (only guidance copy now) — candidate next step.
- Company deletion & edit.
- Team/accountant invitation.
- A persistent setup checklist.

## 15. Open questions

- Exact name/identity of the demo company (`Demo Co Ltd`) — resolved.
- Should the "Save your progress" affordance live in the sidebar footer or in
  the banner?
- Does the safety-net screen need to be built now, or just designed?
- Should the search fixture include the demo company itself (yes/no)?
