# @tally/web

The **Tally web app** — the product UI for UK company accounts & CT600 filing.
Built with **SolidJS** + **Panda CSS** on the
[`@tally/design-system`](../../packages/design-system/README.md) package.

> **No backend yet.** The app is sample-first: a pre-seeded `Sample Co Ltd`
> company lets every feature demo itself, and user data persists to
> `localStorage` via [`src/db.ts`](src/db.ts). Static fixtures live in
> [`src/mock_data.ts`](src/mock_data.ts); views only consume exported types,
> so swapping in real API calls stays contained to those two files. The
> design-system showcase lives in
> [`apps/design-system-showcase`](../design-system-showcase/README.md).

## Run it

```bash
pnpm install           # from the repo root (pnpm workspace)
pnpm dev:web           # http://localhost:5173
```

`predev`/`prebuild` run Panda codegen automatically (the generated
`styled-system/` and `dist/` are gitignored). For typechecking:

```bash
pnpm --filter @tally/web typecheck
```

## What's here

- **App shell** (`src/App.tsx`) — 240px sidebar: company picker (Select, with
  an "Add company" choice that opens a dialog), the Workspace nav group
  (Accounts, Filings, Payroll) and the bottom group (Integrations, Settings),
  plus the signed-in user row. Keyboard-first: `1`–`5` switch views.
  First-run onboarding is layered on top: a persistent "Sample data" banner,
  an add-company flow (simulated Companies House search), a "Save your
  progress" simulated-account dialog, and auto-selection of the user's
  company (the sample retires from the picker once any company has a
  connected data source).
- **Accounts** (`src/views/Accounts.tsx`) — Transactions (search + account
  filter), Summaries (YTD stats, a net-by-month bar chart and a table), and
  Data sources tabs.
- **Filings** (`src/views/Filings.tsx`) — next filing card (due date, progress,
  File now/Preview) and previous filings filtered by a financial-year picker.
- **Payroll** (`src/views/Payroll.tsx`) — next run, employees and run history.
- **Integrations** (`src/views/Integrations.tsx`) — connected data sources and
  an "Add bank account" dialog.
- **Settings** (`src/views/Settings.tsx`) — company profile, filing
  preferences, notifications and a danger zone.
- **Mock data** (`src/mock_data.ts`) — the sample company, Companies House
  search fixture, transactions, summaries, data sources, filings, payroll and
  preferences.
- **Local DB** (`src/db.ts`) — versioned `localStorage` persistence for
  companies, per-company data sources, the simulated account, and banner
  dismissal.
- **Onboarding components** (`src/components/`) — `AddCompanyDialog`
  (search → review → fill remaining fields), `SaveProgressDialog` (simulated
  account), `SampleBanner`.

## Onboarding flow (no login)

1. App opens on `Sample Co Ltd` with seeded data; a banner says "Sample data —
   add your company to get started". Closing it only hides it for the current
   screen — it returns when you switch screens.
2. Adding a company runs a **simulated Companies House search** (per
   [`docs/spec/first-run-onboarding-spec.md`](../../docs/spec/first-run-onboarding-spec.md))
   → pick a result → fill only fields a search can't infer (UTR, standard,
   period) → the new company is auto-selected.
3. The user's company starts empty; guided empty states point to "Connect a
   bank".
4. Once any company has ≥1 connected data source, the sample leaves the
   picker.
5. "Save your progress" (anytime) opens the simulated-account dialog; the
   user row then shows "Saved · Name".

Icons come from `lucide-solid` (a direct dependency; the design system uses
the same version internally).

## Wiring

- `panda.config.ts` applies the `parkUI` preset exported by
  `@tally/design-system` (brown accent, sand gray, Outfit, all recipes) and
  scans both the app and the package sources.
- `vite.config.ts` aliases `styled-system/*` to this app's generated directory.
- `index.html` applies the persisted color mode before first paint;
  `createColorMode()` from the design system toggles it at runtime.
