import { fileURLToPath } from 'node:url'
import { defineConfig } from 'vite'
import solid from 'vite-plugin-solid'

// The design-system package's source imports `styled-system/*` (Panda's
// generated API). The app owns the Panda codegen (see panda.config.ts), so
// those imports must resolve to *this* app's `styled-system` directory.
const styledSystem = fileURLToPath(new URL('./styled-system', import.meta.url))

export default defineConfig({
  plugins: [solid()],
  resolve: {
    alias: {
      'styled-system': styledSystem,
    },
  },
  server: {
    // Dev-only: forward the API to the Rust backend (spec §4). Start it with
    // `nix develop -c dev-db` + `nix develop -c api`.
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:8080',
        changeOrigin: true,
      },
    },
  },
})
