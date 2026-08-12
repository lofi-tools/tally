# @tally/design-system-showcase

The **design-system showcase**: a single page that displays every component
and token in the [`@tally/design-system`](../../packages/design-system/README.md)
package. It also serves as the pattern the future landing/docs site can follow
(Astro + a Solid island reusing the design system).

Stack: **Astro** + **SolidJS** (one `client:only="solid-js"` island) +
**Panda CSS** + **Ark UI** + **Park UI** (stock, via `@tally/design-system`).

## Run it

```bash
pnpm install                                   # from the repo root (pnpm workspace)
pnpm --filter @tally/design-system-showcase dev   # http://localhost:4321
```

(Note: the flake's `nix develop -c dev`/`-c web` scripts run the Tally app
stack, not this showcase — use the `pnpm --filter` command above.)

`predev`/`prebuild` run Panda codegen automatically (the generated
`styled-system/`, `.astro/` and `dist/` are gitignored). For typechecking:

```bash
pnpm --filter @tally/design-system-showcase typecheck
```

## How it's wired

- `astro.config.mjs` adds the `@astrojs/solid-js` integration (the showcase
  is one Solid island, `client:only` — no SSR; see the note below), aliases
  `styled-system/*` to this app's generated directory, keeps the source-first
  `@tally/design-system` out of Astro's SSR externalization
  (`vite.ssr.noExternal`), and wires the Panda PostCSS plugin.
- `panda.config.ts` applies the `parkUI` preset exported by
  `@tally/design-system` (brown accent, sand gray, Outfit, all recipes) and
  scans both the app and the package sources, so components are styled by the
  app's own Panda codegen.
- `src/pages/index.astro` loads the Outfit font and the generated styles,
  applies the persisted color mode before first paint, and mounts the island.
- `src/Showcase.tsx` is the page shell — sticky header (with dark-mode
  switch), showcase sections, footer.

## Structure

| Path | Contents |
|------|----------|
| `src/pages/index.astro` | The single Astro page; owns `<head>` (font, color-mode script, title) and mounts the island |
| `src/Showcase.tsx` | The island: sticky header (with dark-mode switch), sections, footer |
| `src/sections/` | Showcase sections: Hero, Tokens, Buttons & badges, Forms, Overlays |
| `src/components/icons.tsx` | Inline SVG icon set used by the showcase |
| `scripts/smoke.mjs` | Post-build check that the SSR'd page contains the showcase |

## Notes

The old jsdom interaction QA (`qa-pass.mjs`) did not survive the move to
Astro: the page is a hydrated island, so jsdom can't drive the interactive
paths (dialog, menu, tabs, dark toggle). Verify those in a real browser.

**Why client-only (no SSR):** `solid-js/web`'s server entry stubs the
client-only APIs that `lucide-solid`'s `Icon` imports (`spread`/`insert`/
`template`), which makes dev-mode server rendering throw. The showcase is a
client-side app, so SSR buys nothing here — `client:only="solid-js"` skips it
and hydrates on load. A future SSR'd landing page should keep this in mind
(render icons via inline SVG, as `src/components/icons.tsx` already does).
