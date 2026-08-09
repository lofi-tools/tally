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
  globalCss: {
    html: { scrollBehavior: 'smooth' },
    ':root': { colorScheme: 'light' },
    '.dark': { colorScheme: 'dark' },
    '::selection': { bg: 'accentMuted', color: 'accent' },
  },
})
