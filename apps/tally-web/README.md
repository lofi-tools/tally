# @tally/web

The **Tally web app** — the product UI for UK company accounts & CT600 filing.
Built with **SolidJS** + **Panda CSS** on the
[`@tally/design-system`](../../packages/design-system/README.md) package.

> **No backend yet.** All data lives in [`src/mock_data.ts`](src/mock_data.ts);
> views only consume the exported types, so swapping in real API calls stays
> contained to that file. The design-system showcase lives in
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
- **Mock data** (`src/mock_data.ts`) — companies, transactions, summaries,
  data sources, filings, payroll and preferences.

Icons come from `lucide-solid` (a direct dependency; the design system uses
the same version internally).

## Wiring

- `panda.config.ts` applies the `parkUI` preset exported by
  `@tally/design-system` (brown accent, sand gray, Outfit, all recipes) and
  scans both the app and the package sources.
- `vite.config.ts` aliases `styled-system/*` to this app's generated directory.
- `index.html` applies the persisted color mode before first paint;
  `createColorMode()` from the design system toggles it at runtime.
