# Accounts tab: Balances sub-tab spec

Date: 2026-08-11 · App: `apps/tally-web` · Status: spec (no code yet)

## 1. Overview

Restructure the Accounts view's sub-tabs. The current **Summaries** sub-tab becomes
the first sub-tab, renamed **Balances**, and gains two new sections: a
GnuCash-style chart-of-accounts tree and a compact "recent transactions" list.
The income/expenses/net cards stay. The old bar chart and monthly table move to
the **Transactions** sub-tab.

Final sub-tab order: **Balances** (default) → **Transactions** → **Data sources**
(unchanged).

## 2. Goals / non-goals

**Goals**

- Give the accounts landing view an at-a-glance financial picture: P&L headline
  numbers, the full book of accounts as a collapsible tree, and recent activity
  with a one-click path to the full list.
- Keep everything mock-driven (no backend); all new data lives in
  `mock_data.ts`.
- Keep the app's existing sign/colour convention consistent across the tree,
  cards, and tables.

**Non-goals**

- No real chart-of-accounts editing, no account CRUD, no ledger backend wiring.
- The **Data sources** sub-tab is untouched.
- No changes to the sidebar, header, or other views.

## 3. Current state (reference)

`views/Accounts.tsx` today:

- `Tabs.Root defaultValue="transactions"` with three triggers:
  `Transactions` (default), `Summaries`, `Data sources`.
- **Transactions** tab: search input + account filter dropdown (hard-coded
  `accountOptions`) + full table (Date, Description, Account, Source, Amount,
  Status).
- **Summaries** tab: three YTD `StatCard`s (Income, Expenses, Net), a "Net by
  month" bar chart, and a monthly table (Month / Income / Expenses / VAT / Net).
- **Data sources** tab: `DataSourceRows` + footer note.

Shared pieces: `PageHeader`, `StatCard`, `EmptyState`, `StatusBadge`,
`DataSourceRows`, `numCell` in `components/Shared.tsx`. Data comes from
`getCompanyData(companyId)` in `mock_data.ts` (demo company seeded,
user companies return `emptyCompanyData`).

## 4. New sub-tab structure

```
Tabs.Root defaultValue="balances"
  Balances      ← renamed + first + default
  Transactions  ← receives the chart + monthly table, keeps search/table
  Data sources  ← unchanged
```

## 5. Balances sub-tab (new)

Three sections, top to bottom:

### 5.1 Income / expenses / net cards

Unchanged from today: three YTD `StatCard`s — Income (YTD), Expenses (YTD),
Net (YTD) with the existing hints ("12 months to August", "Before corporation
tax").

### 5.2 Chart of accounts tree

The demo data is now the **real GnuCash book** from
`example_data/basic-1/input.gnucash` (a SQLite GnuCash file), ported
by a one-off generator script into `mock_data.ts`:

- The **whole tree is kept in memory** as a recursive `AccountNode { name,
  type, children }` structure, re-rooted under the app's five top-level groups
  (`Assets` / `Liabilities` / `Equity` / `Income` / `Expenses`) by account
  type. Redundant same-name wrappers (the book's own `Assets/Assets`) are
  spliced away; the two `Input`/`Output` VAT accounts are disambiguated by
  parent wrapper (`Assets/VAT/Input`, `Liabilities/VAT/Output/Sales`).
- **One row per split** of the book (95 splits, 27 accounts with activity),
  signed per the app convention (income and expenses positive — GnuCash's
  debit-normal direction — liabilities and equity negative) and with **dates
  shifted into FY2025/26** (Apr 2025 → Mar 2026) so the demo looks current.
- **Balances derive from the transactions list** (summed per full account
  path), so the tree, every account register and the YTD cards always agree.
  The Income card (£18,184.12) equals the `Income` group total and the
  Expenses card (£12,135.97) the `Expenses` group total by construction.

**Rendering:** recursive tree, collapsible via a chevron on group rows.
**Expand one level by default** — top-level groups start open so the first
sub-level is visible (as before); deeper levels start collapsed.

**Columns:** Account name (indented by depth) + balance (right-aligned,
tabular numbers). No type/description columns.

**Sign & colour convention (app convention, not GnuCash natural signs):**

| Group        | Balance direction | Colour  |
| ------------ | ----------------- | ------- |
| Assets       | positive          | green   |
| Liabilities  | negative          | red     |
| Equity       | negative          | red     |
| Income       | positive          | green   |
| Expenses     | positive          | green   |

Zero balances render default/muted (no colour). Signed amounts formatted with
`fmtSignedMoney`.

**Group rows:** bold, show a rolled-up total of their children, have a chevron
that toggles expansion. **Leaf rows:** normal weight, indented. Clicking a leaf
**selects** it (bronze indicator, §6.1) and shows its register in the side
panel (§6).

### 5.3 Recent transactions

- The **5 most recent** transactions by date (descending), as a **compact
  list**: date, description, signed + coloured amount. No source/status
  columns.
- A trailing **"All transactions →"** affordance. Clicking it switches to the
  **Transactions** sub-tab **and resets** the transactions tab's search box and
  account filter (so the full unfiltered list is visible).

## 6. Right-side register panel (account drill-down)

Clicking a **leaf account** in the tree **selects** it and shows its register
in an **inline panel to the right of the chart** — a plain `Card.Root` at the
**same elevation as the chart** (side-by-side grid, 50/50 on `lg+`, stacked on
small screens). No overlay: there is **no backdrop, no dimming, no elevation
above the chart**, and the panel lives inside the main content's padding box
like any other card. It is a **read-only** view.

### 6.1 Selection & placement

- A single `selected` signal holds the selected leaf account's full path
  (`null` = nothing selected).
- The **selected leaf row** shows a bronze indicator echoing the sidebar's
  active bar: a 2px `brown.9` vertical bar with a barely-there
  `0 0 6px {colors.brown.a6}` glow, plus a `brown.a3` row background.
- Layout: `display: grid; gap: 4; lg: { gridTemplateColumns: repeat(2,
  minmax(0, 1fr)) }; alignItems: start` — chart card left, register card
  right. Both cards scroll independently with the page (no sticky/fixed).

### 6.2 Anatomy (top to bottom)

```
Card.Root (register, same elevation as the chart card)
├─ header row:  account label (bold) … current balance (signed, coloured)
├─ sub-line:    account breadcrumb (path minus the top-level group)
└─ body:        register table: Date · Description · Amount · Balance
```

- **Header**: the leaf account's label (e.g. "Current Account") plus its
  current balance, signed and coloured per the app convention (§5.2), e.g.
  `+£19,694.36` (green) for Current Account, `+£12,135.97` (green) for
  Expenses (debit-normal).
  Below: the breadcrumb path ("Bank Accounts › Current Account").
- **Body — register table with four columns**:
  - Date (tabular), Description (truncating, with the Source as a muted
    sub-line), Amount (signed, coloured per row), **Balance**.
  - **Balance = running balance after each transaction**: rows are ordered
    oldest → newest and the running total accumulates each row's amount, so
    the user can follow where amounts come from. The final row's Balance
    equals the account's current balance. Zero → muted, no colour.
- **Nothing selected**: the body shows a compact `EmptyState` ("Select an
  account" / "Pick a row in the chart of accounts…").
- **Empty account** (leaf exists but zero transactions): "No transactions" /
  "This account has no activity yet."

### 6.3 Keyboard & accessibility

Plain inline content — no overlay to trap or dismiss:

- Leaf rows and group chevrons are focusable buttons; Tab moves through the
  tree and panel like any other page content.
- Selecting a different leaf simply replaces the register content; clicking
  the same leaf keeps it selected (no toggle-off).
- The selected row's bronze bar is `aria-hidden`; the row itself is a button
  with the account name as its accessible name.

### 6.4 Reachability

- Only reachable from the Balances tree's leaf rows. Works for the demo
  company; for no-data companies the tree isn't rendered (§8), so the panel
  cannot be opened there.

## 6.5 Plausibility of the demo data

- The balances in the chart are the sums of the transactions **by
  construction** (both derive from the same `transactions` list), so the tree
  never contradicts the register or the YTD cards.
- The previous invented data did not reconcile (Sales showed £22,348.70 of
  transactions against a £224,140 Income card). The ported basic-1 book is a
  real, self-consistent ledger — every split has its counterpart, and the
  group totals equal the YTD cards exactly.

## 7. Transactions sub-tab changes

- Receives the **"Net by month" bar chart** and the **monthly table** (Month /
  Income / Expenses / VAT / Net) from the old Summaries tab.
- Placement: **above** the existing search + transactions table (chart →
  monthly table → search/filter → table).
- The monthly sections render only when `summaries` data exists (same condition
  as today).
- The account filter dropdown is **derived from the chart of accounts leaf
  accounts** (plus "All accounts"), replacing the hard-coded `accountOptions`,
  so the two stay in sync.
- Everything else (search behaviour, table columns, status badges, export/add
  buttons in the header) is unchanged.

## 8. Empty state (user companies with no data)

When `CompanyData` is empty (no transactions, no summaries), the **whole
Balances tab** is replaced by **one combined empty state** (cards, tree, and
recent list are not rendered separately):

- Title: "No data yet"
- Description: connect a bank or upload a ledger to populate your books.
- Action: "Connect a bank" → `onGoToIntegrations` (existing prop).
- The Transactions tab keeps its existing empty state; Data sources is
  unchanged.

## 9. Interaction & polish details

- Tab switch via "All transactions →" also clears filters (§5.3) — implemented
  by lifting the search/filter signals' reset into the tab-change handler.
- Group chevrons rotate on expand; rows use the app's existing subtle hover
  treatment (matching `DataSourceRows` / table rows).
- Tree rows and the recent-transactions list use `numCell`/`fmtSignedMoney` for
  tabular, consistent numbers.
- Keyboard: chevrons/rows are focusable buttons; the inline register panel is
  plain page content (no focus trap needed — see §6.3).

## 10. Files touched (planned)

- `src/mock_data.ts` — the ported basic-1 book: recursive `AccountNode`
  tree, per-split `transactions` (FY2025/26 dates), derived `summaries`,
  and helpers (`accountPathOf`, `transactionsFor`, `accountBalance`,
  `groupBalance`, `chartAccountNames`). The `Transaction`/`CompanyData`
  interfaces are unchanged.
- `src/views/Accounts.tsx` — tab reorder/rename, new Balances sections, moved
  chart + monthly table, derived filter, inline register panel (§6) with
  running balance, bronze selected-row indicator, "All transactions →"
  handler.
- The generator script that produced the ported book is a throwaway in
  `/tmp` (not committed); the book itself is committed as data in
  `mock_data.ts`.
- No changes to `db.ts`, `App.tsx`, or other views.

## 11. Out of scope / future

- Clicking a group row could one day expand-and-filter; for now it only
  expands/collapses.
- Real balances will come from the ledger backend (`POST …/ledgers` +
  account views); the derived-from-transactions approach is a mock stand-in and
  should be replaced by API data with the tree kept as the UI shape.
- Drag-to-reorder, account editing, and a GnuCash natural-sign toggle are
  future work. A full-screen register (with search/filter per account) could
  replace the inline panel later.
- The old floating Drawer behaviour (backdrop, ESC-to-close) is intentionally
  **not** used here — the panel is inline per the product decision that
  selecting an account is a persistent, non-modal action.
