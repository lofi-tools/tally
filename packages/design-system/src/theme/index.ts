/**
 * Tally design-system theme.
 *
 * Everything visual in the system is derived from these tokens. The theme is
 * plain data, so a consuming app can spread it into its own `panda.config.ts`
 * (see `apps/tally-web/panda.config.ts`).
 *
 * Raw tokens  -> brand (pastel teal-green) + neutral scales, fonts
 * Semantic    -> roles (bg, fg, border, accent, success…) with `_dark`
 *                variants; dark mode is class-based (`.dark` on <html>)
 * Recipes     -> reusable component variants (button, badge, kbd)
 *
 * Brand: dark-first, keyboard-first dev-tool aesthetic (Linear / Raycast /
 * Cursor / Framer lineage). One pastel teal-green accent, hairline borders,
 * layered shadows reserved for floating surfaces. See the repo-root DESIGN.md.
 */

const scale = (values: Record<string, string>) =>
  Object.fromEntries(Object.entries(values).map(([name, value]) => [name, { value }]))

/**
 * Teal-green — a pastel mint-teal (hue ~164°) that still reads vibrant
 * against near-black. 400 is the dark-mode accent, 700 the light-mode accent
 * (white text on it clears AA ~5.5:1), 300 the dark hover brightening.
 */
const brand = scale({
  '50': '#effaf6',
  '100': '#d9f3ea',
  '200': '#b5e8d7',
  '300': '#8adbc1',
  '400': '#5fcdb0',
  '500': '#42b493',
  '600': '#339278',
  '700': '#287563',
  '800': '#215d4f',
  '900': '#1b4940',
  '950': '#0e2b25',
})

/**
 * Neutral — cool greys with a whisper of the teal's green cast. Near-black at
 * the dark end; the page canvas is darker than 950 (see `bg`).
 */
const neutral = scale({
  '50': '#f6f7f3',
  '100': '#ecefe8',
  '200': '#daded3',
  '300': '#c0c6b8',
  '400': '#9aa192',
  '500': '#757d6e',
  '600': '#575e52',
  '700': '#41463d',
  '800': '#2c302a',
  '900': '#1d201b',
  '950': '#121410',
})

export const tokens = {
  colors: { brand, neutral },
  fonts: {
    sans: {
      value:
        "'Inter Variable', 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif",
    },
    mono: {
      value:
        "ui-monospace, SFMono-Regular, 'SF Mono', Menlo, Consolas, 'Liberation Mono', monospace",
    },
  },
}

/**
 * Semantic tokens map roles (not raw values) to the theme. Every color that
 * means something — page background, text, borders, accents, feedback — is a
 * semantic token with a light (`base`) and dark (`_dark`) value.
 *
 * Dark is the identity (the DESIGN.md brand): the `_dark` values are the
 * crafted ones. Light mode is a courtesy skin.
 */
export const semanticTokens = {
  colors: {
    // Surfaces — 4-tier elevation (base / raised / overlay / scrim). Each
    // dark tier lifts 5–8% in lightness; overlays get their own bg, not just
    // a shadow.
    bg: { value: { base: '{colors.neutral.50}', _dark: '#0d0f0b' } },
    surface: { value: { base: '{colors.white}', _dark: '#171a14' } },
    surfaceMuted: { value: { base: '{colors.neutral.100}', _dark: '#1e2119' } },
    surfaceOverlay: { value: { base: '{colors.white}', _dark: '#22261e' } },
    // Text
    fg: { value: { base: '{colors.neutral.900}', _dark: '#f2f4ee' } },
    fgMuted: { value: { base: '{colors.neutral.600}', _dark: '#a8ae9f' } },
    fgSubtle: { value: { base: '{colors.neutral.500}', _dark: '#7a8273' } },
    // Borders — hairline by default
    border: { value: { base: '{colors.neutral.200}', _dark: '#262b22' } },
    borderStrong: { value: { base: '{colors.neutral.300}', _dark: '#3a4134' } },
    // Brand / accent — one pastel teal-green, used sparingly. Light-mode
    // accent is a deeper teal (brand.700) so white accentFg clears AA.
    accent: { value: { base: '{colors.brand.700}', _dark: '{colors.brand.400}' } },
    accentFg: { value: { base: '{colors.white}', _dark: '{colors.brand.950}' } },
    accentHover: { value: { base: '{colors.brand.800}', _dark: '{colors.brand.300}' } },
    accentMuted: { value: { base: '{colors.brand.50}', _dark: '{colors.brand.950}' } },
    // Feedback
    info: { value: { base: '#0284c7', _dark: '#38bdf8' } },
    infoFg: { value: { base: '{colors.white}', _dark: '#082f49' } },
    infoSoft: { value: { base: '#f0f9ff', _dark: '#0d1d2b' } },
    success: { value: { base: '#0e7a4e', _dark: '#34d399' } },
    successFg: { value: { base: '{colors.white}', _dark: '#022c22' } },
    successSoft: { value: { base: '#e9f9f1', _dark: '#0b2419' } },
    warning: { value: { base: '#b7791f', _dark: '#facc15' } },
    warningFg: { value: { base: '{colors.white}', _dark: '#422006' } },
    warningSoft: { value: { base: '#fdf6e3', _dark: '#2c2008' } },
    danger: { value: { base: '#c2253b', _dark: '#f87171' } },
    dangerFg: { value: { base: '{colors.white}', _dark: '#2b0d10' } },
    dangerSoft: { value: { base: '#fdeef0', _dark: '#2b1314' } },
    // Scrim — translucent layer behind overlays
    overlay: { value: { base: 'rgba(20, 26, 12, 0.5)', _dark: 'rgba(4, 5, 3, 0.7)' } },
  },
  shadows: {
    /**
     * Raised — cards & sticky headers. Two-layer: ambient halo + contact
     * edge. Dark adds a 1px inset specular highlight (surface-elevation).
     */
    raised: {
      value: {
        base: '0 1px 2px rgba(20, 26, 12, 0.06), 0 8px 24px rgba(20, 26, 12, 0.08)',
        _dark: 'inset 0 1px 0 rgba(255, 255, 255, 0.06), 0 1px 2px rgba(0, 0, 0, 0.5), 0 8px 24px rgba(0, 0, 0, 0.4)',
      },
    },
    /**
     * Overlay — modals, popovers, menus, dropdowns. One tier above `raised`:
     * bigger contact edge, deeper halo, specular highlight in dark.
     */
    overlay: {
      value: {
        base: '0 2px 4px rgba(20, 26, 12, 0.08), 0 24px 48px rgba(20, 26, 12, 0.18)',
        _dark: 'inset 0 1px 0 rgba(255, 255, 255, 0.08), 0 2px 4px rgba(0, 0, 0, 0.55), 0 24px 48px rgba(0, 0, 0, 0.55)',
      },
    },
  },
}

/** Typographic roles, used via `textStyle: 'display'` etc. */
export const textStyles = {
  display: {
    value: {
      fontFamily: 'sans',
      fontSize: '5xl',
      fontWeight: '600',
      lineHeight: '1.05',
      letterSpacing: '-0.035em',
    },
  },
  h1: {
    value: {
      fontFamily: 'sans',
      fontSize: '3xl',
      fontWeight: '600',
      lineHeight: '1.1',
      letterSpacing: '-0.03em',
    },
  },
  h2: {
    value: {
      fontFamily: 'sans',
      fontSize: '2xl',
      fontWeight: '600',
      lineHeight: '1.2',
      letterSpacing: '-0.025em',
    },
  },
  h3: {
    value: {
      fontFamily: 'sans',
      fontSize: 'xl',
      fontWeight: '600',
      lineHeight: '1.3',
      letterSpacing: '-0.02em',
    },
  },
  h4: {
    value: { fontFamily: 'sans', fontSize: 'md', fontWeight: '600', lineHeight: '1.4' },
  },
  body: {
    value: { fontFamily: 'sans', fontSize: 'md', lineHeight: '1.6', letterSpacing: '-0.011em' },
  },
  bodySm: {
    value: { fontFamily: 'sans', fontSize: 'sm', lineHeight: '1.6', letterSpacing: '-0.011em' },
  },
  caption: {
    value: { fontFamily: 'sans', fontSize: 'xs', lineHeight: '1.5', letterSpacing: '0.02em' },
  },
  code: { value: { fontFamily: 'mono', fontSize: 'sm', lineHeight: '1.6' } },
} as const

/** Animations used by overlay components (open/close transitions). */
export const keyframes = {
  fadeIn: { from: { opacity: '0' }, to: { opacity: '1' } },
  fadeOut: { from: { opacity: '1' }, to: { opacity: '0' } },
  scaleIn: {
    from: { opacity: '0', transform: 'translateY(4px) scale(0.98)' },
    to: { opacity: '1', transform: 'translateY(0) scale(1)' },
  },
  scaleOut: {
    from: { opacity: '1', transform: 'scale(1)' },
    to: { opacity: '0', transform: 'scale(0.98)' },
  },
}

// ---------------------------------------------------------------------------
// Recipes — reusable component variants, generated into `styled-system/recipes`
// ---------------------------------------------------------------------------

export const recipes = {
  button: {
    className: 'button',
    base: {
      display: 'inline-flex',
      alignItems: 'center',
      justifyContent: 'center',
      gap: '2',
      whiteSpace: 'nowrap',
      userSelect: 'none',
      fontFamily: 'sans',
      fontWeight: '600',
      lineHeight: '1.2',
      borderRadius: 'lg',
      cursor: 'pointer',
      transitionProperty: 'background-color, border-color, color, box-shadow, transform',
      transitionDuration: '150ms',
      transitionTimingFunction: 'ease',
      // Crisp focus — no glow ring (Linear rule)
      _focusVisible: {
        outline: '2px solid',
        outlineColor: 'accent',
        outlineOffset: '1px',
      },
      _active: { transform: 'translateY(1px)' },
      _disabled: { opacity: '0.55', cursor: 'not-allowed', pointerEvents: 'none' },
    },
    variants: {
      visual: {
        solid: { bg: 'accent', color: 'accentFg', _hover: { bg: 'accentHover' } },
        subtle: { bg: 'accentMuted', color: 'accent', _hover: { bg: 'accentMuted', color: 'accentHover' } },
        outline: {
          borderWidth: '1px',
          borderStyle: 'solid',
          borderColor: 'borderStrong',
          bg: 'transparent',
          color: 'fg',
          _hover: { bg: 'surfaceMuted', borderColor: 'borderStrong' },
        },
        ghost: { bg: 'transparent', color: 'fg', _hover: { bg: 'surfaceMuted' } },
      },
      tone: { primary: {}, danger: {}, neutral: {} },
      size: {
        xs: { h: '7', px: '2.5', fontSize: 'xs', borderRadius: 'md' },
        sm: { h: '8', px: '3', fontSize: 'sm' },
        md: { h: '9', px: '4', fontSize: 'sm' },
        lg: { h: '10', px: '5', fontSize: 'md' },
      },
    },
    defaultVariants: { visual: 'solid', tone: 'primary', size: 'md' },
    compoundVariants: [
      // danger tone
      { visual: 'solid', tone: 'danger', css: { bg: 'danger', color: 'dangerFg' } },
      { visual: 'subtle', tone: 'danger', css: { bg: 'dangerSoft', color: 'danger' } },
      { visual: 'outline', tone: 'danger', css: { borderColor: 'danger', color: 'danger' } },
      { visual: 'ghost', tone: 'danger', css: { color: 'danger' } },
      // neutral tone
      { visual: 'solid', tone: 'neutral', css: { bg: 'fg', color: 'bg' } },
      { visual: 'subtle', tone: 'neutral', css: { bg: 'surfaceMuted', color: 'fg' } },
      { visual: 'outline', tone: 'neutral', css: { borderColor: 'borderStrong', color: 'fg' } },
      { visual: 'ghost', tone: 'neutral', css: { color: 'fgMuted', _hover: { color: 'fg' } } },
    ],
  },
  badge: {
    className: 'badge',
    base: {
      display: 'inline-flex',
      alignItems: 'center',
      gap: '1',
      fontFamily: 'sans',
      fontWeight: '600',
      fontSize: 'xs',
      lineHeight: '1.4',
      borderRadius: 'full',
      px: '2',
      py: '0.5',
      whiteSpace: 'nowrap',
    },
    variants: {
      tone: {
        primary: {},
        neutral: {},
        success: {},
        warning: {},
        danger: {},
        info: {},
      },
      variant: { subtle: {}, solid: {}, outline: {} },
    },
    defaultVariants: { tone: 'neutral', variant: 'subtle' },
  },
  /**
   * Keyboard-shortcut chip — the keyboard-first brand signature. Mono, small,
   * hairline-bordered with a crisp bottom edge (key-cap style).
   */
  kbd: {
    className: 'kbd',
    base: {
      display: 'inline-flex',
      alignItems: 'center',
      justifyContent: 'center',
      fontFamily: 'mono',
      fontSize: 'xs',
      fontWeight: '500',
      lineHeight: '1.4',
      minW: '5',
      h: '5',
      px: '1.5',
      borderRadius: 'md',
      color: 'fgMuted',
      bg: 'surfaceOverlay',
      borderWidth: '1px',
      borderStyle: 'solid',
      borderColor: 'border',
      // Bottom edge only — a key, not a floating card
      boxShadow: '0 1px 0 rgba(20, 26, 12, 0.12)',
      _dark: { boxShadow: '0 1px 0 rgba(0, 0, 0, 0.55)' },
      whiteSpace: 'nowrap',
    },
  },
}

const badgeTones = {
  primary: { subtle: ['accentMuted', 'accent'], solid: ['accent', 'accentFg'], outline: ['accent', 'accent'] },
  neutral: { subtle: ['surfaceMuted', 'fgMuted'], solid: ['fg', 'bg'], outline: ['borderStrong', 'fgMuted'] },
  success: { subtle: ['successSoft', 'success'], solid: ['success', 'successFg'], outline: ['success', 'success'] },
  warning: { subtle: ['warningSoft', 'warning'], solid: ['warning', 'warningFg'], outline: ['warning', 'warning'] },
  danger: { subtle: ['dangerSoft', 'danger'], solid: ['danger', 'dangerFg'], outline: ['danger', 'danger'] },
  info: { subtle: ['infoSoft', 'info'], solid: ['info', 'infoFg'], outline: ['info', 'info'] },
} as const

// Panda's recipe type derives the variant unions from the recipe above; inject
// the tone × variant combinations so each tone/variant pair has colors.
// (Typed loosely on purpose — panda resolves the tokens at codegen time.)
;(recipes.badge as any).compoundVariants = (
  Object.entries(badgeTones) as [string, (typeof badgeTones)[keyof typeof badgeTones]][]
).flatMap(([tone, styles]) => [
  { tone, variant: 'subtle', css: { bg: styles.subtle[0], color: styles.subtle[1] } },
  { tone, variant: 'solid', css: { bg: styles.solid[0], color: styles.solid[1] } },
  {
    tone,
    variant: 'outline',
    css: { borderWidth: '1px', borderStyle: 'solid', borderColor: styles.outline[0], color: styles.outline[1] },
  },
])

/**
 * The full theme object a Panda config spreads in:
 *
 *   theme: { extend: { tokens, semanticTokens, textStyles, keyframes, recipes } }
 */
export const theme = {
  extend: {
    tokens,
    semanticTokens,
    textStyles,
    keyframes,
    recipes,
  },
}
