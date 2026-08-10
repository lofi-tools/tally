# @tally/design-system

The Tally design system: a token-driven theme plus SolidJS components built on
[Ark UI](https://ark-ui.com) primitives, styled with
[Panda CSS](https://panda-css.com).

**Brand:** dark-first and keyboard-first (Linear / Raycast / Cursor / Framer
lineage) with a single pastel teal-green accent — see the repo-root [`DESIGN.md`](../../DESIGN.md).
Everything visual in the system is derived from the design tokens in
[`src/theme`](src/theme/index.ts) — change a token there and every component
re-themes, in light and dark mode (dark is the default identity).

## Layout

| Path | Contents |
|------|----------|
| `src/theme/index.ts` | Design tokens — raw scales (pastel teal-green brand, neutral, fonts), semantic roles with `_dark` variants, text styles, keyframes, and the `button` / `badge` / `kbd` recipes |
| `src/components/` | Components wired to the theme: `Button`, `Badge`, `Kbd`, `Card`, `Input`, `Textarea`, `Select`, `Dialog`, `Tabs`, `Switch`, `Menu`, `Tooltip` |
| `src/hooks/color-mode.ts` | `createColorMode()` — class-based dark mode (toggles `.dark` on `<html>`), dark by default |

## How a consumer app uses it

The package is **source-first**: consumers compile its TSX directly, so there is
no build step. A consuming app's `panda.config.ts` spreads the exported theme in
and includes the package sources in its scan globs, so the components' styles
are generated as part of the app's own `styled-system`:

```ts
// apps/<app>/panda.config.ts
import { defineConfig } from '@pandacss/dev'
import { theme } from '../../packages/design-system/src/theme'

export default defineConfig({
  include: ['./src/**/*.{ts,tsx}', '../../packages/design-system/src/**/*.{ts,tsx}'],
  outdir: 'styled-system',
  theme,
})
```

The app's bundler aliases `styled-system/*` to its own generated directory (see
`apps/tally-web/vite.config.ts`), and its `tsconfig.json` maps the same paths.
Run `pnpm codegen` in the app before typechecking/building.

See [`apps/tally-web`](../apps/tally-web/README.md) for a working consumer.

## Development

The package keeps its own Panda config so it typechecks standalone:

```bash
pnpm --filter @tally/design-system codegen   # generate styled-system/ for typechecking
pnpm --filter @tally/design-system typecheck
```
