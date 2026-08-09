import { defineConfig } from '@pandacss/dev'
import { theme } from './src/theme'

export default defineConfig({
  preflight: true,
  include: ['./src/**/*.{ts,tsx}'],
  exclude: [],
  outdir: 'styled-system',
  theme,
})
