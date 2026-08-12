# Sample-data clarity — spec

Status: **Draft for review** · App: `apps/tally-web` (SolidJS + Panda + `@tally/design-system`) · Data: mock, no backend yet.

> Scope note: this spec is the result of a 4-round interview with the product
> owner. It refines the sample-state UX introduced by
> `docs/spec/first-run-onboarding-spec.md` (§4.2, §5.1): the sample banner is
> more prominent, its copy teaches the path to real data, its visibility now
> follows the *selected* company rather than raw company count, and the sample
> company itself never disappears.

---

## 1. Problem

Three gaps remain in the current sample-state UX:

1. **Too subtle.** The banner uses `brown.a2` (α ≈ 0.04–0.05 — nearly
   invisible on the dark canvas). Users can miss that everything on screen is
   invented demo content.
2. **Unclear how to get your own data in.** The copy ("Sample data — add your
   company to get started") names step 1 but not step 2 (connect a bank /
   upload a ledger). After adding a company, the app goes silent — the banner
   vanishes even though the new company shows *empty* screens.
3. **Wrong visibility rule.** The banner only shows when the user has **zero**
   companies. If a user owns companies and selects `Sample Co Ltd` in the
   picker, only a tiny `Sample` badge marks it — exactly the state where
   "this isn't your data" matters most.

## 2. Goals & non-goals

### Goals
- The sample banner is **noticeably more prominent** (stronger surface color,
  an icon) without becoming a full-saturation wall.
- The banner **teaches the two-step path** to real data: add a company, then
  connect a data source.
- The banner **follows the selected company**: it appears whenever sample data
  is on screen, not only when the user owns nothing.
- Once the user owns companies, the banner **stays until real data flows** —
  a softer variant covers the "my company is empty" state.
- The sample label is **present wherever sample data is visible**: banner +
  picker badge + a `Sample` chip in every view header.
- The sample company **remains available to explore forever** (badged), so
  demo data is never lost.

### Non-goals (this iteration)
- Real auth / Companies House / Open Banking (unchanged — mock).
- GnuCash file **upload** UI (guidance copy only, as before).
- A first-visit modal or wizard (persistent banner only, per onboarding spec §5.1).
- New design-system tokens — `brown.subtle.bg` already exists.
- Any change to the offline banner (amber solid) or the Devtools banner.

## 3. Interview decisions (chosen options)

| # | Question | Decision |
|---|---|---|
| R1 | Banner color | **`brown.subtle.bg` (`brown.a3`)** — one step up in the brand hue (~2.5× the current `a2` tint). Not `brown.solid.bg` (DESIGN.md bans full-saturation brown fills) and not amber (collides with the amber offline banner). |
| R1 | When is the banner visible? | **Whenever the sample is selected** — including when the user owns companies and picks the sample. |
| R1 | Labeling beyond the banner | **Add a `Sample` chip to each view header** (next to the page title), plus the existing picker badge. |
| R2 | Banner content | **Two-step mini-path**: "1. Add your company → 2. Connect a bank or upload a ledger" + the Add company button. |
| R2 | Empty own company | **Softer banner until data connects** — a distinct copy variant that stays until at least one data source is connected on any company. |
| R2 | Switch affordance | **"View your company" button in the banner** — a secondary action when the sample is selected and real companies exist. |
| R3 | Dismissal | **None — not dismissible** (override after implementation): no close control; the banner renders on every view while the applicable state holds. The earlier "persist until data connects" choice is dropped. |
| R3 | Sample after retirement | **Keep in the picker, badged `Sample`** — stays selectable forever (one-way retirement is dropped). |
| R3 | Iconography | **Small demo icon** at the left of the banner (before the copy). |
| R4 | Banner copy by state | **Adapt by state** — three variants (see §5), not one static string. |
| R4 | `Sample` chip color | **Brown (brand default)** — default `outline` badge, consistent with the current picker badge. |
| R4 | Keyboard shortcut | **None** — the existing 1–5 view nav stays the only shortcut map. |

## 4. Product model & banner state machine

Banner visibility is driven by three booleans:

- `sampleSelected` — the current company in the picker is `SAMPLE_COMPANY_ID`.
- `hasRealCompanies` — `db().companies.length > 0`.
- `anyDataConnected` — **any** user company has ≥1 data source
  (`companies().some((c) => (sources()[c.id] ?? []).length > 0)`).

| State | Conditions | Banner | Primary action |
|---|---|---|---|
| **A — onboarding** | sample selected · no real companies | Variant A (two-step) | `Add company` (opens search dialog) |
| **B — viewing sample** | sample selected · real companies exist | Variant B (switch) | `View <first real company>` (switches selection) |
| **C — empty own data** | real company selected · `anyDataConnected` false | Variant C (connect) | `Connect a bank` (switches to Integrations) |
| **D — data flowing** | `anyDataConnected` true | no banner | — |

Precedence: **A > B > C** when multiple conditions overlap (e.g. sample selected
while a real company exists is always B, never C). Once `anyDataConnected` is
true, no variant renders in any selection state.

The old `bannerVisible = !hasRealCompany() && !bannerDismissed()` rule, the
transient per-screen-switch dismissal and the dismissal flag are **replaced**
by this machine: the banner is not dismissible and simply follows the table
above.

## 5. Banner redesign (`SampleBanner.tsx`)

### 5.1 Visual treatment (all variants)
- **Fill:** `bg: 'brown.subtle.bg'` (i.e. `brown.a3`) — replaces `brown.a2`.
- **Strip:** full-width, bleeding to the horizontal edges of the main column
  (current behavior), `borderBottom: 1px solid {colors.border}`.
- **Icon:** a small demo glyph at the left of the copy — `FlaskConical` from
  `lucide-solid` (fall back to `FlaskRound` / `Sparkles` if the export is
  absent). `color: 'brown.plain.fg'` so it reads on the tint; `w/h 4`.
- **Copy:** `fg.muted` body with the emphasized phrase in `fg.default`
  weight 600 (as today).
- **Action:** `Button size="sm"` (primary brown, per DESIGN.md).
- **No dismiss control** — the banner is not dismissible; it stays up on every
  view and sub-tab while the applicable state holds (override of the ×).
- **Layout:** flex row — icon · copy (flex:1, minW:0) · action(s);
  `px {base:4, md:5}`, `py 2`. Compact height, no radius (it is a bleed strip,
  not a card).

### 5.2 Variant copy (minimal & direct — onboarding spec §5.2 voice)

**A — onboarding** (no real companies):
> `Sample data` (bold) · `1. Add your company → 2. Connect a bank or upload a ledger.`
> Actions: `Add company` (primary).

**B — viewing sample** (real companies exist, sample selected):
> `You're viewing sample data` (bold) · `switch to your own company to see your numbers.`
> Actions: `View <name of first real company>` (primary; switches `companyId`
> to that company).

**C — empty own data** (real company selected, no data anywhere):
> `Your data isn't here yet` (bold) · `connect a bank or upload a ledger to pull in transactions.`
> Actions: `Connect a bank` (primary; calls the existing
> `onGoToIntegrations` path — switches view to Integrations).

No variant is dismissible.

## 6. Sample identity everywhere

### 6.1 View-header chips (new)
- Add a `SampleBadge` component to `components/Shared.tsx`:
  `Badge variant="outline"` (brand-default brown), label `Sample`, `fontSize xs`.
- `PageHeader` gains an optional `badge?: JSX.Element` prop, rendered inline
  after the `<h1>` title (flex row, `gap 2`, `alignItems baseline/center`).
- Each of the five views (`Accounts`, `Filings`, `Payroll`, `Integrations`,
  `Settings`) passes
  `badge={props.company.id === SAMPLE_COMPANY_ID ? <SampleBadge /> : undefined}`
  — import `SAMPLE_COMPANY_ID` from `../mock_data`.
- Effect: whenever sample data is on screen, the page title carries the
  `Sample` label — a second marker alongside the (undismissable) banner.

### 6.2 Picker (unchanged + never retires)
- The sample stays in the picker **forever**, badged `Sample` (outline, brown)
  in both the trigger summary and the item row.
- The `sampleRetired` memo and one-way retirement rule in `App.tsx` are
  **removed**: `allCompanies` always includes the sample first.
- "Add company…" remains the bottom item.

### 6.3 Safety-net screen (unchanged)
- The zero-companies-of-any-kind screen (onboarding spec §4.4) keeps its own
  hero. With the sample permanent it is unreachable in practice — still kept
  as a fallback.

## 7. Persistence

The banner is **not dismissible** — no close control, no persistence flag.
Nothing is stored in `Db` for it: visibility depends only on the §4 machine
inputs (`sampleSelected`, `hasRealCompanies`, `anyDataConnected`). It renders
on every view and sub-tab while a variant applies, and disappears only once
any user company has a connected data source.

## 8. Edge cases

- **View switching / sub-tabs** → the banner stays put: it renders above the
  view content (outside the tab switcher) and is never hidden by navigation.
- **Two real companies, one with data** → selecting the empty one shows no
  banner (rule: `anyDataConnected` silences all). The empty-state CTAs inside
  Accounts/Integrations still guide. Accepted.
- **Connecting data** → the banner disappears in every selection state — the
  only thing that hides it.
- **Offline banner** (amber solid) stacks above the sample banner when both
  are visible — unchanged, and visually distinct from the brown tint.
- **Light mode** → `brown.a3` light value (`#a04b0018`, α ≈ 0.09) is a warm
  beige tint on `sand.1`; same copy colors hold. No extra work.
- **Migrated companies** (SignInDialog §7.3) → local companies removed after
  migration; `hasRealCompanies` still true while signed-in; banner behavior
  unchanged.
- **Devtools Reset** → wipes `tally.db.v1` and the token; a fresh load returns
  to variant A.

## 9. Implementation notes

- `components/SampleBanner.tsx` — becomes one component with a
  `variant: 'onboarding' | 'viewing-sample' | 'empty-data'` prop plus
  `onAddCompany` / `onViewCompany` / `onConnectBank` / `onDismiss`. Copy,
  icon and action labels switch on the variant.
- `App.tsx` — replace `bannerVisible` / `bannerDismissed` / `sampleRetired`
  with the §4 machine; add `anyDataConnected` memo; wire `onViewCompany`
  (`setCompanyId(companies()[0].id)`) and `onConnectBank`
  (`switchView('integrations')`). No dismissal state is kept.
- `db.ts` — unchanged (the `bannerDismissed` field from the draft was dropped).
- `components/Shared.tsx` — `SampleBadge` + `PageHeader` `badge` prop.
- The five view files — pass the badge conditionally (one line each).
- `mock_data.ts` — unchanged (`SAMPLE_COMPANY_ID` already exported).

## 10. Acceptance criteria

1. Fresh load (cleared localStorage): banner shows **variant A** — `brown.a3`
   fill, flask icon, two-step copy, `Add company` button.
2. Selecting the sample while owning companies shows **variant B**; `View
   <company>` switches the selection to the real company and the banner
   updates accordingly.
3. A real company with no data shows **variant C**; `Connect a bank` lands on
   the Integrations view.
4. Connecting any data source removes the banner in every selection state.
5. The banner has no dismiss control; it renders on every view and sub-tab
   while the applicable state holds, and disappears only when data connects.
6. Every view header shows the `Sample` chip whenever sample data is on
   screen; the picker keeps its `Sample` badge.
7. The sample remains selectable in the picker after data connects.
8. `pnpm --filter @tally/web typecheck` and `build` pass; the amber offline
   banner is untouched.

## 11. Out of scope / future

- First-visit explainer dialog or setup checklist.
- Distinguishing "sample" vs "demo" branding (fixed on `Sample`).
- Making the view-header chips dismissible or clickable.
- Per-variant visibility rules (a single global state machine chosen).
