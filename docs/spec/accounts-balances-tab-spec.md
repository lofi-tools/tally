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
`getCompanyData(companyId)` in `mock_data.ts` (sample company seeded,
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

GnuCash-style account tree, two levels deep, **all groups expanded by default**
(collapsible via a chevron on group rows).

**Columns:** Account name (indented by depth) + balance (right-aligned,
tabular numbers). No type/description columns.

**Sign & colour convention (app convention, not GnuCash natural signs):**

| Group        | Balance direction | Colour  |
| ------------ | ----------------- | ------- |
| Assets       | positive          | green   |
| Liabilities  | negative          | red     |
| Equity       | negative          | red     |
| Income       | positive          | green   |
| Expenses     | negative          | red     |

Zero balances render default/muted (no colour). Signed amounts formatted with
`fmtSignedMoney`.

**Group rows:** bold, show a rolled-up total of their children, have a chevron
that toggles expansion. **Leaf rows:** normal weight, indented; clicking a leaf
opens the right-side panel (§6).

**Data model** (new, in `mock_data.ts`):

- `AccountNode { name, group, parent? }` — a flat or nested structure describing
  the chart of accounts. Existing transaction accounts map into it:
  - **Income** → `Sales`
  - **Expenses** → `Rent`, `Utilities`, `Software`, `Telecom`, `Insurance`,
    `Office`, `Payroll`, `Tax`
  - **Assets** → `Starling` (current), `Barclays` (current)
  - **Liabilities** → `Loan` (e.g. "Loan — Lloyds")
  - **Equity** → `Share capital`
- **Balances are derived from the transactions list**, not static: each
  transaction's `amount` is summed into its `account`'s balance; group totals
  are the sum of children. The tree and the transactions table therefore always
  agree.
- To make Assets/Liabilities/Equity non-zero, **add a small set of mock
  transactions** to the sample data (dated early, e.g. incorporation-era, so
  they don't disturb the "most recent" list):
  - Opening bank balance → account `Starling`, amount `+45 000`
  - Share capital injection → account `Share capital`, amount `-15 000`
  - Loan drawdown → account `Loan`, amount `-20 000`
  (Signs follow the app convention above.)

### 5.3 Recent transactions

- The **5 most recent** transactions by date (descending), as a **compact
  list**: date, description, signed + coloured amount. No source/status
  columns.
- A trailing **"All transactions →"** affordance. Clicking it switches to the
  **Transactions** sub-tab **and resets** the transactions tab's search box and
  account filter (so the full unfiltered list is visible).

## 6. Right-side panel (account drill-down)

Clicking a **leaf account** in the tree opens a **half-width panel from the
right** showing that account's transactions. It is a **read-only** view — no
footer actions, closing is the only exit.

### 6.1 Component & placement

- Use the design-system `Drawer` (`packages/design-system/…/ui/drawer.tsx` — an
  Ark UI `Dialog` variant; no Drawer is used in the app yet, this is the
  first consumer).
- `placement="end"` (right side — already the recipe default) with
  `size="lg"` (`maxW: 32rem` ≈ half of the 60rem content column). The recipe
  sets `width: '100%'` on the content, so it renders full-width on small
  screens and caps at 32rem on desktop. `Root` already defaults to
  `lazyMount` + `unmountOnExit`.
- Controlled like the existing dialogs: `open` + `onOpenChange={(d) => …}`
  (match `AddCompanyDialog`'s `d.open` pattern).

### 6.2 Anatomy (top to bottom)

```
Drawer.Root open onOpenChange
├─ Drawer.Backdrop          black.a7 scrim, fade-in/out
├─ Drawer.Positioner        flex-end (right edge)
│  └─ Drawer.Content        slide-from-right, bg gray.surface.bg, shadow lg
│     ├─ Drawer.CloseTrigger   ✕  (recipe: absolute, top-end)
│     ├─ Drawer.Header
│     │  ├─ Drawer.Title          account name, textStyle xl, semibold
│     │  └─ Drawer.Description    balance sub-line, fg.muted, textStyle sm
│     └─ Drawer.Body           scrollable (recipe: overflow auto, flex 1)
│        └─ compact table: Date · Description · Source · Amount · Status
└─ (no Drawer.Footer — read-only)
```

- **Header title**: the leaf account's name (e.g. "Sales").
- **Header description**: the account's current balance, signed and coloured
  per the app convention (§5.2), e.g. `+£31,348.70` (green) for Sales,
  `−£1,850.00` (red) for Rent. Zero → muted, no colour.
- **Body**: the account's transactions as a table with **five columns** —
  Date, Description, Source, Amount, Status. The **Account column is dropped**
  (it is redundant in a per-account view). Amount and Status reuse the main
  table's formatting (`fmtSignedMoney` + colour, `StatusBadge`). Rows are not
  clickable.
- **Empty account** (leaf exists but zero transactions, e.g. an unused
  account): the body shows a compact `EmptyState` ("No transactions" /
  "This account has no activity yet.") — no table chrome.

### 6.3 Keyboard, focus & backdrop behaviour

Inherited from the Ark UI `Dialog` the drawer is built on (no custom wiring):

- **ESC** closes the drawer.
- **Backdrop click** closes the drawer.
- **Focus trap**: while open, focus cycles within the drawer content; the
  close button is reachable and `aria-label="Close"`-equivalent via the
  design system's `CloseTrigger`.
- **Focus return**: on close, focus returns to the tree row that opened the
  drawer (leaf rows are focusable buttons).
- **Scroll**: only the drawer body scrolls; the page behind stays put
  (`overscrollBehaviorY: none` on the positioner).

### 6.4 Reachability

- Only reachable from the Balances tree's leaf rows. Works for the sample
  company; for no-data companies the tree isn't rendered (§8), so the panel
  cannot be opened there.
- Opening a new account's panel while one is open simply replaces the
  content (single `openAccount` signal holding the account name); closing is
  always available via ✕ / ESC / backdrop.

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
- Keyboard: chevrons/rows are focusable buttons; the drawer is focus-trapped by
  the design-system `Drawer`.

## 10. Files touched (planned)

- `src/mock_data.ts` — chart-of-accounts data (`AccountNode` or equivalent),
  the three new mock transactions, a `getAccountBalance(account)`-style helper
  and a derived `accountOptions` list; existing `Transaction`/`CompanyData`
  types unchanged (extended, not broken).
- `src/views/Accounts.tsx` — tab reorder/rename, new Balances sections, moved
  chart + monthly table, derived filter, drawer wiring, "All transactions →"
  handler.
- `src/components/Shared.tsx` — only if a small shared piece (e.g. compact
  transaction row) is worth extracting; otherwise keep changes local.
- No changes to `db.ts`, `App.tsx`, or other views.

## 11. Out of scope / future

- Clicking a group row could one day expand-and-filter; for now it only
  expands/collapses.
- Real balances will come from the ledger backend (`POST …/ledgers` +
  account views); the derived-from-transactions approach is a mock stand-in and
  should be replaced by API data with the tree kept as the UI shape.
- Deep nesting (3+ levels), drag-to-reorder, account editing, and GnuCash
  natural-sign toggle are future work.
