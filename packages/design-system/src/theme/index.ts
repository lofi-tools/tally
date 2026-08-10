/**
 * Tally design-system theme — built on **Park UI** (vendored, MIT).
 *
 * Park UI distributes its entire system as local source (open code): theme
 * tokens, colors, recipes and components all live in this package, copied
 * from the official registry (park-ui.com/registry). Because everything is
 * local and generated from the same snapshot, components and recipes can
 * never drift apart.
 *
 * Brand choices (see DESIGN.md):
 *   - accent color: **brown**  — the default `colorPalette`, set on `<html>`
 *   - gray color:   **sand**   — warm neutral family (registered as `gray`)
 *   - font family:  **Outfit** — the `sans` stack
 *   - radius:       **sm**     — l1 xs / l2 sm / l3 md
 *
 * Feedback palettes (green / blue / amber / red) are also vendored so status
 * chips and alerts can use stock Park UI colors.
 *
 * Dark mode is class-based: toggling `.dark` on `<html>` flips every
 * `_dark` semantic token. `createColorMode` in this package does exactly
 * that; dark is the Tally identity.
 */
import { definePreset } from '@pandacss/dev'
import { animationStyles } from './animation-styles'
import { conditions } from './conditions'
import { globalCss } from './global-css'
import { keyframes } from './keyframes'
import { layerStyles } from './layer-styles'
import { recipes, slotRecipes } from './recipes'
import { textStyles } from './text-styles'
import { colors } from './tokens/colors'
import { durations } from './tokens/durations'
import { shadows } from './tokens/shadows'
import { zIndex } from './tokens/z-index'
import { amber } from './colors/amber'
import { blue } from './colors/blue'
import { brown } from './colors/brown'
import { green } from './colors/green'
import { red } from './colors/red'
import { sand } from './colors/sand'

/** Raw color palettes, exported for the Tokens showcase. */
export const colorPalettes = { brown, sand, green, blue, amber, red }

/** Brand font — Outfit, loaded via `@fontsource-variable/outfit` in the app. */
const outfit =
  "'Outfit Variable', 'Outfit', ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, 'Noto Sans', sans-serif"

/**
 * The full Park UI preset for Tally: default Panda preset + vendored Park UI
 * theme, with **brown** as the default color palette (accent) and **sand**
 * as gray.
 *
 * Uses `@pandacss/preset-panda` (NOT `preset-base`) — the base preset ships
 * no token scales (spacing, sizes, fontSizes, …), so every recipe size/font
 * reference would emit unresolved strings like `sizes.10`. The standard
 * scales are required for the recipes to resolve.
 */
export const parkUI = definePreset({
  name: '@tally/park-ui',
  presets: ['@pandacss/preset-panda'],
  globalCss: {
    ...globalCss,
    extend: {
      ...globalCss.extend,
      // Brown is the accent — make it the default color palette so recipes
      // that reference `colorPalette.*` resolve to brown without props.
      html: { colorPalette: 'brown' },
    },
  },
  conditions,
  theme: {
    extend: {
      animationStyles,
      recipes,
      slotRecipes,
      keyframes,
      layerStyles,
      textStyles,
      // preset-base ships no breakpoints — the standard Panda scale (used by
      // `sm:`/`md:`/`lg:` responsive conditions in app styles).
      breakpoints: {
        sm: '640px',
        md: '768px',
        lg: '1024px',
        xl: '1280px',
        '2xl': '1536px',
      },
      tokens: {
        colors,
        durations,
        zIndex,
        // preset-base ships no radius scale — the Park recipes resolve
        // `l1`/`l2`/`l3` (semantic) to the standard raw radii below.
        radii: {
          xs: { value: '0.125rem' },
          sm: { value: '0.25rem' },
          md: { value: '0.375rem' },
          lg: { value: '0.5rem' },
          xl: { value: '0.75rem' },
          '2xl': { value: '1rem' },
          '3xl': { value: '1.5rem' },
          full: { value: '9999px' },
        },
        fonts: {
          sans: { value: outfit },
          mono: {
            value:
              "ui-monospace, SFMono-Regular, 'SF Mono', Menlo, Consolas, 'Liberation Mono', monospace",
          },
          code: {
            value:
              "ui-monospace, SFMono-Regular, 'SF Mono', Menlo, Consolas, 'Liberation Mono', monospace",
          },
        },
      },
      semanticTokens: {
        colors: {
          // accent palettes + gray (sand)
          brown,
          gray: sand,
          green,
          blue,
          amber,
          red,
          // aliases
          fg: {
            default: { value: { _light: '{colors.gray.12}', _dark: '{colors.gray.12}' } },
            muted: { value: { _light: '{colors.gray.11}', _dark: '{colors.gray.11}' } },
            subtle: { value: { _light: '{colors.gray.10}', _dark: '{colors.gray.10}' } },
          },
          canvas: { value: { _light: '{colors.gray.1}', _dark: '{colors.gray.1}' } },
          border: { value: { _light: '{colors.gray.4}', _dark: '{colors.gray.4}' } },
          error: { value: { _light: '{colors.red.9}', _dark: '{colors.red.9}' } },
          bg: {
            subtle: { value: { _light: '{colors.gray.2}', _dark: '{colors.gray.3}' } },
          },
        },
        shadows,
        radii: {
          l1: { value: '{radii.xs}' },
          l2: { value: '{radii.sm}' },
          l3: { value: '{radii.md}' },
        },
      },
    },
  },
})

/** The Park UI preset list a consumer config spreads in. */
export const presets = [parkUI]

/** Raw palette token objects (for showcases). */
export const tokens = {
  colors: {
    brown: brown,
    sand: sand,
    green: green,
    blue: blue,
    amber: amber,
    red: red,
  },
}
