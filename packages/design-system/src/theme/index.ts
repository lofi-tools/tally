/**
 * Tally design-system theme.
 *
 * Everything visual in the system is derived from these tokens. The theme is
 * plain data, so a consuming app can spread it into its own `panda.config.ts`
 * (see `apps/tally-web/panda.config.ts`).
 *
 * Raw tokens  -> brand + neutral color scales, fonts
 * Semantic    -> roles (bg, fg, border, accent, success…) with `_dark`
 *                variants; dark mode is class-based (`.dark` on <html>)
 * Recipes     -> reusable component variants (button, badge)
 */

const scale = (values: Record<string, string>) =>
  Object.fromEntries(Object.entries(values).map(([name, value]) => [name, { value }]))

const brand = scale({
  '50': '#eef2ff',
  '100': '#e0e7ff',
  '200': '#c7d2fe',
  '300': '#a5b4fc',
  '400': '#818cf8',
  '500': '#6366f1',
  '600': '#4f46e5',
  '700': '#4338ca',
  '800': '#3730a3',
  '900': '#312e81',
  '950': '#1e1b4b',
})

const neutral = scale({
  '50': '#f8fafc',
  '100': '#f1f5f9',
  '200': '#e2e8f0',
  '300': '#cbd5e1',
  '400': '#94a3b8',
  '500': '#64748b',
  '600': '#475569',
  '700': '#334155',
  '800': '#1e293b',
  '900': '#0f172a',
  '950': '#020617',
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
 * semantic token with a light (`base`) and dark (`_dark`) value. Dark mode is
 * class-based: adding `.dark` to `<html>` flips the whole palette.
 */
export const semanticTokens = {
  colors: {
    // Surfaces
    bg: { value: { base: '{colors.neutral.50}', _dark: '{colors.neutral.950}' } },
    surface: { value: { base: '{colors.white}', _dark: '{colors.neutral.900}' } },
    surfaceMuted: { value: { base: '{colors.neutral.100}', _dark: '{colors.neutral.800}' } },
    // Text
    fg: { value: { base: '{colors.neutral.900}', _dark: '{colors.neutral.50}' } },
    fgMuted: { value: { base: '{colors.neutral.600}', _dark: '{colors.neutral.400}' } },
    fgSubtle: { value: { base: '{colors.neutral.500}', _dark: '{colors.neutral.500}' } },
    // Borders
    border: { value: { base: '{colors.neutral.200}', _dark: '{colors.neutral.800}' } },
    borderStrong: { value: { base: '{colors.neutral.300}', _dark: '{colors.neutral.700}' } },
    // Brand / accent
    accent: { value: { base: '{colors.brand.600}', _dark: '{colors.brand.400}' } },
    accentFg: { value: { base: '{colors.white}', _dark: '{colors.neutral.950}' } },
    accentHover: { value: { base: '{colors.brand.700}', _dark: '{colors.brand.300}' } },
    accentMuted: { value: { base: '{colors.brand.50}', _dark: '{colors.brand.950}' } },
    // Feedback
    info: { value: { base: '#0284c7', _dark: '#38bdf8' } },
    infoFg: { value: { base: '{colors.white}', _dark: '#082f49' } },
    infoSoft: { value: { base: '#f0f9ff', _dark: '#082f49' } },
    success: { value: { base: '#059669', _dark: '#34d399' } },
    successFg: { value: { base: '{colors.white}', _dark: '#022c22' } },
    successSoft: { value: { base: '#ecfdf5', _dark: '#022c22' } },
    warning: { value: { base: '#d97706', _dark: '#fbbf24' } },
    warningFg: { value: { base: '{colors.white}', _dark: '#451a03' } },
    warningSoft: { value: { base: '#fffbeb', _dark: '#451a03' } },
    danger: { value: { base: '#dc2626', _dark: '#f87171' } },
    dangerFg: { value: { base: '{colors.white}', _dark: '#450a0a' } },
    dangerSoft: { value: { base: '#fef2f2', _dark: '#450a0a' } },
    // Overlays
    overlay: { value: { base: 'rgba(15, 23, 42, 0.5)', _dark: 'rgba(2, 6, 23, 0.72)' } },
  },
  shadows: {
    elevated: {
      value: { base: '{shadows.lg}', _dark: '0 10px 40px rgba(0, 0, 0, 0.5)' },
    },
  },
}

/** Typographic roles, used via `textStyle: 'display'` etc. */
export const textStyles = {
  display: {
    value: {
      fontFamily: 'sans',
      fontSize: '5xl',
      fontWeight: '700',
      lineHeight: '1.05',
      letterSpacing: '-0.03em',
    },
  },
  h1: {
    value: {
      fontFamily: 'sans',
      fontSize: '3xl',
      fontWeight: '700',
      lineHeight: '1.15',
      letterSpacing: '-0.02em',
    },
  },
  h2: {
    value: {
      fontFamily: 'sans',
      fontSize: '2xl',
      fontWeight: '650',
      lineHeight: '1.2',
      letterSpacing: '-0.02em',
    },
  },
  h3: {
    value: {
      fontFamily: 'sans',
      fontSize: 'xl',
      fontWeight: '650',
      lineHeight: '1.3',
      letterSpacing: '-0.01em',
    },
  },
  h4: {
    value: { fontFamily: 'sans', fontSize: 'md', fontWeight: '600', lineHeight: '1.4' },
  },
  body: { value: { fontFamily: 'sans', fontSize: 'md', lineHeight: '1.6' } },
  bodySm: { value: { fontFamily: 'sans', fontSize: 'sm', lineHeight: '1.6' } },
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
      _focusVisible: { outline: '2px solid', outlineColor: 'accent', outlineOffset: '2px' },
      _active: { transform: 'translateY(1px)' },
      _disabled: { opacity: '0.55', cursor: 'not-allowed', pointerEvents: 'none' },
    },
    variants: {
      visual: {
        solid: { bg: 'accent', color: 'accentFg', _hover: { bg: 'accentHover' } },
        subtle: { bg: 'accentMuted', color: 'accent', _hover: { bg: 'accentMuted' } },
        outline: {
          borderWidth: '1px',
          borderStyle: 'solid',
          borderColor: 'borderStrong',
          bg: 'transparent',
          color: 'fg',
          _hover: { bg: 'surfaceMuted' },
        },
        ghost: { bg: 'transparent', color: 'fg', _hover: { bg: 'surfaceMuted' } },
      },
      tone: { primary: {}, danger: {}, neutral: {} },
      size: {
        xs: { h: '7', px: '2.5', fontSize: 'xs', borderRadius: 'md' },
        sm: { h: '8', px: '3', fontSize: 'sm' },
        md: { h: '10', px: '4', fontSize: 'sm' },
        lg: { h: '11', px: '5', fontSize: 'md' },
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
