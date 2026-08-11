import { defineConfig } from '@pandacss/dev'
import { parkUI } from '../../packages/design-system/src/theme'

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
  // The vendored Park UI components use `styled()` / `createStyleContext`
  // from `styled-system/jsx` — those must be generated for Solid.
  jsxFramework: 'solid',
  presets: [parkUI],
  // Components apply recipe variants dynamically (`<Button variant="…">`),
  // so cssgen can't see them statically. Pre-generate every recipe variant.
  staticCss: {
    recipes: '*',
  },
  globalCss: {
    html: { scrollBehavior: 'smooth' },
    // Dark is the identity; light mode is opt-in via the `.dark` class toggle
    ':root': { colorScheme: 'dark' },
    'html:not(.dark)': { colorScheme: 'light' },
    // Selection already follows the palette via the theme's global css.
  },
})
