# @tally/web

The Tally web app — currently a **design-system showcase**: a single page that
displays every component and token in the
[`@tally/design-system`](../../packages/design-system/README.md) package. It
does **not** (yet) do what `tally-cli` does; that work starts from this
foundation.

Stack: **SolidJS** + **Panda CSS** + **Ark UI**, built with Vite.

## Run it

```bash
pnpm install            # from the repo root (pnpm workspace)
pnpm dev                # http://localhost:5173
# or, in the Nix dev shell (installs deps + starts the server):
nix develop -c dev
```

`predev`/`prebuild` run Panda codegen automatically (the generated
`styled-system/` and `dist/` are gitignored). For typechecking:

```bash
pnpm typecheck
```

## How the design system plugs in

- `panda.config.ts` spreads the theme exported by `@tally/design-system`
  (tokens, semantic colors, recipes) and scans both the app and the package
  sources, so components are styled by the app's own Panda codegen.
- `vite.config.ts` aliases `styled-system/*` to this app's generated directory
  (the design-system package's source imports `styled-system/*` too).
- `index.html` applies the persisted/OS color mode before first paint; the
  `Switch` in the header toggles it at runtime via `createColorMode()`
  (`@tally/design-system`), which flips Panda's class-based `_dark` condition.

## Structure

| Path | Contents |
|------|----------|
| `src/App.tsx` | Page shell — sticky header (with dark-mode switch), sections, footer |
| `src/sections/` | Showcase sections: Hero, Tokens, Buttons & badges, Forms, Overlays |
| `src/components/icons.tsx` | Inline SVG icon set used by the showcase |

## Roadmap

- Replace the showcase with the real Tally app (accounting, filing) as the
  backend crates expose APIs.
- Landing page + docs site: see the Astro question in the repo README — the
  design system package is designed to be shared with those, so they can be
  added later without rework.
