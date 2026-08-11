# @tally/design-system

The Tally design system: **stock Park UI** for SolidJS — the full theme
(tokens, colors, recipes) and the complete component catalog, vendored as
local source from the official registry (park-ui.com/registry), plus a
class-based dark-mode hook.

**Brand** (see the repo-root [`DESIGN.md`](../../DESIGN.md)): dark-first and
keyboard-first (Linear / Raycast / Cursor / Framer lineage). The brand is
expressed in exactly three choices on top of stock Park UI:

- **accent color — `brown`** — the default `colorPalette`, set on `<html>`
- **gray color — `sand`** — the warm neutral family (registered as `gray`)
- **font family — Outfit** — loaded by the app via `@fontsource-variable/outfit`

Everything visual in the system is derived from the vendored theme in
[`src/theme`](src/theme/index.ts) — change a token there and every component
re-themes, in light and dark mode (dark is the default identity; toggling
`.dark` on `<html>` flips every `_dark` semantic token).

## Layout

| Path | Contents |
|------|----------|
| `src/theme/` | The vendored Park UI theme: tokens (colors, radii, durations, shadows, z-index), conditions, global css, keyframes, animation styles, layer styles, text styles, all 56 recipes, and the brand colors (`brown`, `sand`, `green`, `blue`, `amber`, `red`) |
| `src/theme/index.ts` | `parkUI` — a Panda preset (via `definePreset`) assembling the theme with Outfit fonts, the standard breakpoints/radius scales, and **brown** as the default color palette |
| `src/components/ui/` | The complete Park UI Solid catalog (60+ components: Button, Badge, Card, Dialog, Menu, Select, Tabs, Tooltip, Switch, Field, Kbd, Toast, …), vendored from the registry |
| `src/hooks/color-mode.ts` | `createColorMode()` — class-based dark mode (toggles `.dark` on `<html>`), dark by default |

`@ark-ui/solid` primitives and `lucide-solid` icons power the components.

## How a consumer app uses it

The package is **source-first**: consumers compile its TSX directly, so there
is no build step. A consuming app's `panda.config.ts` applies the `parkUI`
preset and includes the package sources in its scan globs, so the components'
styles are generated as part of the app's own `styled-system`:

```ts
// apps/<app>/panda.config.ts
import { defineConfig } from '@pandacss/dev'
import { parkUI } from '@tally/design-system/theme'

export default defineConfig({
  include: ['./src/**/*.{ts,tsx}', '../../packages/design-system/src/**/*.{ts,tsx}'],
  outdir: 'styled-system',
  presets: [parkUI],
  jsxFramework: 'solid',
})
```

The app's bundler aliases `styled-system/*` to its own generated directory (see
`apps/design-system-showcase/astro.config.mjs`), and its `tsconfig.json` maps the
same paths.
Run `pnpm codegen` in the app before typechecking/building (the repo-root
`prepare` script does this automatically after `pnpm install`).

Ark UI helpers are re-exported from the package root so consumers don't need a
direct `@ark-ui/solid` dependency — e.g. `createListCollection` for
`Select.Root`/`Combobox.Root` `collection` props.

> **Dynamic values & cssgen** — Panda's cssgen only emits utilities for values
> it can read statically. If a consumer passes `colorPalette={…}` /
> `textStyle: …` from data, the classes won't be generated (silently
> unstyled). The app seeds them at module scope — see
> [`apps/design-system-showcase/src/seeds.ts`](../apps/design-system-showcase/src/seeds.ts).

See [`apps/design-system-showcase`](../apps/design-system-showcase/README.md)
for a working consumer (an Astro site with a Solid island), and
[`apps/tally-web`](../apps/tally-web/README.md) for the plain Vite app wiring.

## Development

The package keeps its own Panda config so it typechecks standalone:

```bash
pnpm --filter @tally/design-system codegen   # generate styled-system/ for typechecking
pnpm --filter @tally/design-system typecheck
```
