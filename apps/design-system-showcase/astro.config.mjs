import { fileURLToPath } from 'node:url'
import { defineConfig } from 'astro/config'
import solid from '@astrojs/solid-js'
import pandaPostcss from '@pandacss/dev/postcss'

// The design-system package's source imports `styled-system/*` (Panda's
// generated API). This app owns the Panda codegen (see panda.config.ts), so
// those imports must resolve to *this* app's `styled-system` directory — for
// both the client build and Astro's server-side render of the Solid island.
const styledSystem = fileURLToPath(new URL('./styled-system', import.meta.url))

export default defineConfig({
  integrations: [solid()],
  vite: {
    css: {
      postcss: {
        plugins: [pandaPostcss()],
      },
    },
    resolve: {
      alias: {
        'styled-system': styledSystem,
      },
    },
    ssr: {
      // The design system is source-first: its exports point at TS/TSX with
      // no dist build, so Astro must not externalize it during SSR — Vite
      // transforms it like any other source. (Inert today because the island
      // is `client:only` and never server-rendered; matters if it ever
      // switches back to `client:load`.)
      noExternal: ['@tally/design-system'],
    },
  },
})
