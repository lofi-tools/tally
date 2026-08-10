---
version: alpha
name: Tally Design System
description: >-
  Dark-first product design system for Tally — UK company accounts & CT600
  filing. A crafted,  keyboard-first interface language inspired by Linear,
  Raycast, Cursor and Framer: near-black surfaces, hairline borders, a single
  pastel teal-green accent, and dense, precise type.
colors:
  background: "#0D0F0B"
  surface: "#171A14"
  surfaceOverlay: "#22261E"
  text: "#F2F4EE"
  muted: "#A8AE9F"
  border: "#262B22"
  accent: "#5FCDB0"
  accentStrong: "#287563"
  success: "#34D399"
  warning: "#FACC15"
  danger: "#F87171"
typography:
  display:
    fontFamily: "Inter Variable, Inter, system-ui, sans-serif"
    fontSize: 48px
    fontWeight: 600
    lineHeight: 1.05
    letterSpacing: -0.035em
  body:
    fontFamily: "Inter Variable, Inter, system-ui, sans-serif"
    fontSize: 16px
    fontWeight: 400
    lineHeight: 1.6
    letterSpacing: -0.011em
  mono:
    fontFamily: "ui-monospace, SFMono-Regular, 'SF Mono', Menlo, Consolas, monospace"
    fontSize: 13px
    fontWeight: 400
rounded:
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
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.background}"
    rounded: "{rounded.lg}"
    padding: 8px 16px
  card:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.xl}"
    padding: 16px
  kbd:
    backgroundColor: "{colors.surfaceOverlay}"
    textColor: "{colors.muted}"
    rounded: "{rounded.md}"
    border: "1px solid {colors.border}"
---

## Overview

Tally is an accounting product for preparing and filing UK company accounts
and CT600 corporation-tax returns. It is a **tool first**: dense, fast,
keyboard-driven, and calm — the same design temperament as Linear, Raycast,
Cursor and Framer. The interface is a dark near-black cathedral with cool,
slightly warm-tinted greys and **one** pastel teal-green accent that is used
rarely and only where it means something: the primary action, the selected
state, focus.

Dark mode is the identity. Light mode exists as a courtesy, not a second
personality. Everything in the system is derived from design tokens in
`packages/design-system/src/theme`; nothing is hard-coded.

## Colors

Dark-first palette (all hex values above are the dark-mode values).

- `background` (`#0D0F0B`) — the near-black canvas. Never pure black; a
  whisper of warm green keeps it alive next to the teal-green.
- `surface` (`#171A14`) — cards, raised panels, inputs.
- `surfaceOverlay` (`#22261E`) — modals, menus, popovers, dropdowns,
  command-palette surfaces. One tier above `surface`, never the same.
- `text` (`#F2F4EE`) — primary copy, deliberately off-white.
- `muted` (`#A8AE9F`) — secondary copy, timestamps, metadata.
- `border` (`#262B22`) — hairline borders; nearly invisible by design.
- `accent` (`#5FCDB0`) — the pastel teal-green. A mint-teal (hue ~164°):
  soft and pastel, yet vibrant enough to lead the eye against near-black
  without going neon. Use on the primary CTA, the selected row/state, and
  focus.
- `accentStrong` (`#287563`) — the deeper teal used as the accent surface in
  light mode (white text on it clears AA contrast ~5.5:1).
- Feedback — `success`, `warning`, `danger` as above; use their soft
  container variants (`successSoft`, `dangerSoft`, …) for badges and fills.

**Light mode** keeps the same roles with inverted values (page `#F6F7F2`,
surface white, accent `accentStrong`). It is a secondary skin — never add a
second accent for it.

**Do:** treat the teal-green as a rarity — one CTA per surface, selected
states only. **Don't:** fill large backgrounds with full-saturation teal; it
is an accent, not a canvas. **Don't** add a second hue; the palette is
intentionally a single-accent system.

## Typography

Inter is the workhorse, set tight and precise. Display weight is **600**
(never 700+ — the brands we follow cap at 600), with tight tracking
(`-0.035em` on display, `-0.011em` on body). Hierarchy is enforced through
scale and weight, never color — only secondary copy shifts hue (to `muted`).

Mono (`ui-monospace`/SF Mono stack) appears in keyboard-shortcut chips, code
blocks, token names and numeric tables. Numerals in figures should use tabular
spacing where the layout is columnar.

**Do:** surface keyboard shortcuts everywhere — chips next to menu items,
tooltips, command palette, empty states. **Don't:** use heavy display weights;
600 is the max. **Don't** switch to a serif anywhere.

## Layout

Dense vertical rhythm — 4 and 8 px increments dominate, rarely above 64 px in
product surfaces. App layouts follow the three-column dev-tool pattern:
240 px sidebar, fluid main, optional 320 px detail rail. Marketing-style
surfaces cap at ~1100 px. Rows are compact (28–32 px) and hover-fill carries
the affordance — no separators between list rows.

## Elevation & Depth

The product is essentially flat with hairline borders. Elevation is reserved
for floating things — modals, popovers, menus, toasts — and even then the
shadow is dark, soft and **layered** (an ambient halo plus a sharp contact
edge), never a single big drop shadow. Raised surfaces in dark mode carry a
1 px inset top highlight (`rgba(255,255,255,0.06)`) that sells the lift
without light-mode gimmicks.

- base (page) → never elevated
- raised (cards, sticky headers) → hairline border, `raised` shadow token
- overlay (modals, menus, dropdowns) → `surfaceOverlay` bg, `overlay` shadow
- scrim → translucent dark (`rgba(4,5,3,0.7)`), no blur on the backdrop

A child surface's radius must be ≤ its parent's.

## Shapes

- Buttons/inputs/selects: 6–8 px radius (`lg`), 32 px default height.
- Cards/modals: 12 px radius (`xl`).
- Badges, avatars, keyboard chips: pill or near-pill.
- Focus: a crisp 1–2 px accent outline or border brightening — **no glow,
  no 3 px blurred ring** (Linear's rule).

## Components

- **Buttons**: solid = teal-green fill, near-black text, no shadow, color
  shift on hover; subtle = soft teal tint; outline/ghost = hairline border or
  bare.
- **Inputs**: 32 px, 8 px radius, hairline border that brightens to the teal
  on focus; no ring.
- **Tabs**: underline indicator that slides between triggers (animated),
  selected trigger keeps weight + accent.
- **Keyboard chips (`kbd`)**: monospace ~12 px, `surfaceOverlay` bg, hairline
  border, rounded — used beside menu items, tooltips and shortcuts.
- **Cards**: `surface` bg, hairline border, 12 px radius, flat in dark; hover
  lifts the border, not a shadow.
- **Dialog/Menu/Select overlays**: `surfaceOverlay` bg, hairline border,
  layered `overlay` shadow, 100–180 ms scale/fade in.

## Do's & Don'ts

**Do**

- Default to dark; use the warm near-black canvas everywhere.
- Keep rows dense and let hover-fill carry affordance.
- Show keyboard shortcuts as mono chips next to actions.
- Use the teal-green as the single brand color; make it rare.
- Animate briskly (100–180 ms) with smooth standard easing; transforms and
  opacity only.
- Keep focus rings visible for keyboard users — crisp, not glowy.

**Don't**

- Use drop shadows on non-floating elements.
- Add icons to list rows unless functionally required.
- Switch to warm *brown* greys — Tally's greys are cool with a green cast.
- Use the teal-green as a full-saturation background fill.
- Animate longer than 300 ms; Tally feels fast.
- Ever use the default indigo (`#6366f1`) or purple gradients — that is
  generic AI default, not this system.

## Responsive Behavior

Below ~960 px the detail rail collapses; below ~720 px the sidebar collapses
behind a toggle. Dense list rows stay compact on mobile — never reflow into
cards. Keyboard chips remain visible but may stack under the action label on
very narrow screens. Dark mode is the default at every breakpoint.

## Agent Prompt Guide

When designing "in the style of Tally":

1. Start dark: `#0D0F0B` canvas, `#F2F4EE` text, `#262B22` hairline borders.
2. Use the pastel teal-green (`#5FCDB0`) as the only accent — primary CTA
   and selected state.
3. Set type in Inter at 600 max for display, tight tracking, dense 4/8 px
   rhythm, 32 px controls, 6–8 px radii.
4. Put keyboard shortcuts in mono chips next to actions — keyboard-first.
5. Keep it flat: hairline borders, layered shadows only on floating surfaces,
   animation under 200 ms.

---

*Tokens defined in `packages/design-system/src/theme`. Design language
inspired by Linear, Raycast, Cursor and Framer public materials. Source
structure based on [VoltAgent/awesome-design-md](https://github.com/VoltAgent/awesome-design-md) (MIT).*
