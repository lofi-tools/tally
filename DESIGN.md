---
version: alpha
name: Tally Design System
description: >-
  Dark-first product design system for Tally — UK company accounts & CT600
  filing. Built on stock Park UI (SolidJS): the brown accent palette, the sand
  gray family and the Outfit typeface, tuned as a dense, keyboard-first tool
  in the lineage of Linear, Raycast, Cursor and Framer.
colors:
  canvas: "#111110"
  surface: "#111110"
  accent: "#AD7F58"
  accentHover: "#B88C67"
  onAccent: "#FFFFFF"
  text: "#EEEEEC"
  muted: "#B5B3AD"
  border: "#2A2A28"
  success: "#6A9F75"
  info: "#5E9EA3"
  warning: "#B8933C"
  danger: "#B54E4E"
typography:
  fontFamily: "Outfit Variable, Outfit, ui-sans-serif, system-ui, sans-serif"
  display:
    fontSize: 60px
    fontWeight: 800
    lineHeight: 1
    letterSpacing: -0.03em
  body:
    fontSize: 16px
    fontWeight: 400
    lineHeight: 1.5
  mono:
    fontFamily: "ui-monospace, SFMono-Regular, 'SF Mono', Menlo, Consolas, monospace"
    fontSize: 13px
rounded:
  xs: 2px
  sm: 4px
  md: 6px
  lg: 8px
  xl: 12px
  full: 9999px
spacing:
  unit: 4
  xs: 4px
  sm: 8px
  md: 12px
  lg: 16px
  xl: 24px
  2xl: 32px
  3xl: 48px
  4xl: 64px
components:
  button:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.onAccent}"
    rounded: "{rounded.sm}"
    height: 40px
  card:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.md}"
    border: "1px solid {colors.border}"
---

## Overview

Tally is an accounting product for preparing and filing UK company accounts
and CT600 corporation-tax returns. It is a **tool first**: dense, fast,
keyboard-driven, and calm — the same design temperament as Linear, Raycast,
Cursor and Framer. The interface is a dark sand-tinted canvas with **one**
brown accent (Park UI's `brown` palette) used rarely and only where it means
something: the primary action, the selected state, focus.

The design system is **stock Park UI**, vendored in full as local source (see
`packages/design-system/src/theme` and `.../components/ui`) from the official
registry (park-ui.com/registry) — theme tokens, colors, recipes and 60+
Solid components, all from one snapshot so they can never drift. The brand is
expressed in exactly three choices:

- **Accent color — `brown`** — the default `colorPalette`, set on `<html>`.
  Recipes resolve `colorPalette.*` to it, so solid buttons are brown
  (`brown.9` ≈ `#AD7F58`) with **white text**.
- **Gray color — `sand`** — a warm neutral family (Radix-derived), registered
  as the `gray` palette that backs every surface and text role.
- **Font family — Outfit** — a geometric grotesque, geometric enough for
  display but neutral enough for ledgers. Loaded via
  `@fontsource-variable/outfit`.

Dark mode is the identity (class-based: toggling `.dark` on `<html>` flips
every `_dark` semantic token). Light mode exists as a courtesy, not a second
personality. Everything is derived from design tokens; nothing is
hard-coded.

## Colors

All values below are the dark-mode values (the identity).

- `canvas` (`sand.1`, `#111110`) — page background. A whisper of warm sand
  keeps it alive against the brown.
- `gray.surface.bg` (`sand.1`) — cards and raised panels (same tier as the
  canvas; hairline borders carry the separation).
- `fg.default` (`sand.12`, `#EEEEEC`) — primary text.
- `fg.muted` (`sand.11`, `#B5B3AD`) — secondary text, timestamps, metadata.
- `fg.subtle` (`sand.10`, `#7C7B74`) — hint/placeholder text.
- `border` (`sand.4`, `#2A2A28`) — hairline borders.
- `brown.solid.bg` (`brown.9`, `#AD7F58`) — the accent fill: primary
  buttons, selected states, focus. **White text on it keeps AA.**
- `brown.subtle.bg` / `brown.surface.bg` / `brown.outline.border` —
  soft accents, surface accents and outlines (alpha-composited).
- `brown.plain.fg` (`brown.11`, `#DBB594`) — accent-toned **text** on dark
  fills: subtle buttons, highlighted list items, badge text.
- Feedback palettes — `green`, `blue`, `amber`, `red` — each with the same
  `solid` / `subtle` / `surface` / `outline` / `plain` sub-tokens; swap them
  in via `colorPalette="green"` etc. on badges and buttons.

**Do:** treat the brown as a rarity — one CTA per surface, selected states
only. **Don't:** fill large backgrounds with full-saturation brown; it is an
accent, not a canvas. **Don't** add a second hue; the palette is
intentionally a single-accent system.

## Typography

Outfit is the workhorse. Display weight is **800** with tight tracking
(`-0.03em`); body copy is 400. Hierarchy is enforced through scale and
weight — only secondary copy shifts hue (to `fg.muted`). Text styles
(`xs` → `7xl` plus `label`) come from the Park theme; every component uses
them, so type is consistent by construction.

Mono (`ui-monospace`/SF Mono stack) appears in keyboard-shortcut chips, code
blocks, token names and numeric tables. Numerals in figures should use tabular
spacing where the layout is columnar.

**Do:** surface keyboard shortcuts everywhere — chips next to menu items,
tooltips, command palette, empty states. **Don't:** switch to a serif
anywhere.

## Layout

Dense vertical rhythm — 4 and 8 px increments dominate, rarely above 64 px in
product surfaces. App layouts follow the three-column dev-tool pattern:
240 px sidebar, fluid main, optional 320 px detail rail. Rows are compact
(28–32 px) and hover-fill carries the affordance — no separators between list
rows.

## Elevation & Depth

The product is essentially flat with hairline borders. Elevation is reserved
for floating things — modals, popovers, menus, toasts — which use Park's
semantic `shadow` tokens (layered: an ambient halo plus a sharp contact edge),
never a single big drop shadow.

- base (page) → never elevated
- raised (cards, sticky headers) → hairline border only
- overlay (modals, menus, dropdowns) → `shadow.lg`–`shadow.xl`, slide/fade in
  via the vendored animation styles
- scrim → `black.a6`-style translucent dark, no blur on the backdrop

## Shapes

- Buttons: `l2` radius (`radii.sm`, 4 px), 40 px default height.
- Inputs/selects: `l2` (4 px).
- Cards: `l3` (`radii.md`, 6 px).
- Badges, avatars, keyboard chips: `l2`, near-pill.
- Focus: a crisp 1–2 px accent outline (`focusVisibleRing`) — no glow, no
  3 px blurred ring (Linear's rule).

## Components

Everything is recipe-driven and lives in the Park theme:
- **Buttons**: `solid` = brown fill with **white text**, hover brightens
  (`brown.10`); `surface` / `subtle` / `outline` / `plain` for the quieter
  tiers. Sizes `2xs`–`2xl`.
- **Inputs/Selects/Textareas**: `l2` radius, hairline border that brightens
  to the accent on focus (`focusVisibleRing: 'inside'`).
- **Tabs**: animated indicator that slides between triggers, roving focus.
- **Keyboard chips (`kbd`)**: monospace, sand-tinted bg, hairline border —
  used beside menu items and shortcuts.
- **Cards**: `gray.surface.bg`, hairline border, `l3` radius, flat in dark.
- **Dialog/Menu/Select overlays**: `gray.surface.bg`, hairline border,
  layered shadow, 100–180 ms slide/fade in.

## Do's & Don'ts

**Do**

- Default to dark; use the sand-tinted near-black canvas everywhere.
- Keep rows dense and let hover-fill carry affordance.
- Show keyboard shortcuts as mono chips next to actions.
- Use the brown as the single brand color; make it rare.
- Put white text on accent fills; use `brown.plain.fg` for accent-toned text
  on dark fills.
- Animate briskly (100–180 ms) with smooth standard easing; transforms and
  opacity only.
- Keep focus rings visible for keyboard users — crisp, not glowy.

**Don't**

- Use drop shadows on non-floating elements.
- Add icons to list rows unless functionally required.
- Mix hue families — everything derives from the sand/brown families.
- Use the brown as a full-saturation background fill.
- Animate longer than 300 ms; Tally feels fast.
- Reach for loud purple/blue gradients or generic AI-accent colors — stock
  Park palettes only.

## Responsive Behavior

Below ~960 px the detail rail collapses; below ~720 px the sidebar collapses
behind a toggle. Dense list rows stay compact on mobile — never reflow into
cards. Keyboard chips remain visible but may stack under the action label on
very narrow screens. Dark mode is the default at every breakpoint.

## Agent Prompt Guide

When designing "in the style of Tally":

1. Start dark: `canvas` (`#111110`), `fg.default` (`#EEEEEC`), hairline
   `border`; raise with `gray.surface.bg` cards.
2. Use the brown accent (`brown.9`, `#AD7F58`) as the only accent — primary
   CTA and selected state, with **white text on accent fills**; accent-toned
   text on dark fills uses `brown.11` (`#DBB594`).
3. Set type in Outfit — 800 display weight with tight tracking, dense 4/8 px
   rhythm, 40 px controls, 4 px button radius (`l2`).
4. Put keyboard shortcuts in mono chips next to actions — keyboard-first.
5. Keep it flat: hairline borders, layered shadows only on floating surfaces,
   animation under 200 ms.
6. Prefer stock Park UI components and recipes; express brand through the
   three knobs (accent, gray, font) rather than custom styles.

---

*Brand is stock Park UI (MIT, vendored from park-ui.com/registry): brown
accent palette, sand gray family, Outfit typeface. Tokens and components in
`packages/design-system/src/theme` and `packages/design-system/src/components/ui`.
Design language inspired by Linear, Raycast, Cursor and Framer public
materials. Source structure based on
[VoltAgent/awesome-design-md](https://github.com/VoltAgent/awesome-design-md) (MIT).*
