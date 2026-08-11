# @tally/web

The **Tally web app** — currently a blank starter. The
[`@tally/design-system`](../../packages/design-system/README.md) package is
fully wired up; the showcase that used to live here moved to
[`apps/design-system-showcase`](../design-system-showcase/README.md).

Stack: **SolidJS** + **Panda CSS** + **Ark UI** + **Park UI** (stock, via
`@tally/design-system`), built with Vite.

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

## What's wired

- `panda.config.ts` applies the `parkUI` preset exported by
  `@tally/design-system` (brown accent, sand gray, Outfit, all recipes) and
  scans both the app and the package sources.
- `vite.config.ts` aliases `styled-system/*` to this app's generated directory.
- `index.html` applies the persisted color mode before first paint;
  `createColorMode()` from the design system toggles it at runtime.

Build the real UI in `src/App.tsx` — the design-system showcase
(`apps/design-system-showcase`) is the reference for how every component looks.
