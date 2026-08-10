import { defineConfig } from '@pandacss/dev'
import { parkUI } from './src/theme'

export default defineConfig({
  preflight: true,
  include: ['./src/**/*.{ts,tsx}'],
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
})
