import { defineConfig } from '@pandacss/dev'
import { theme } from '../../packages/design-system/src/theme'

export default defineConfig({
  preflight: true,
  // Scan both the app and the design-system package sources so their
  // `styled-system/*` imports resolve against this app's generated styles.
  include: [
    './src/**/*.{ts,tsx}',
    '../../packages/design-system/src/**/*.{ts,tsx}',
  ],
  exclude: [],
  outdir: 'styled-system',
  theme,
  // Recipe variants are applied dynamically through the design-system
  // components (`button({ size: local.size })` etc.), so cssgen can't see
  // them statically. Pre-generate every variant — otherwise xs/sm/lg buttons
  // lose padding/height and badge tone/variant classes never exist.
  staticCss: {
    recipes: {
      button: ['*'],
      badge: ['*'],
      kbd: ['*'],
    },
  },
  globalCss: {
    html: { scrollBehavior: 'smooth' },
    // Dark is the identity; light mode is opt-in via the `.dark` class toggle
    ':root': { colorScheme: 'dark' },
    'html:not(.dark)': { colorScheme: 'light' },
    '::selection': { bg: 'accent', color: 'accentFg' },
  },
})
