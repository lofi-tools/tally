// Panda cssgen seeds.
//
// cssgen only emits utilities for values it can read statically from source.
// The showcases pass `colorPalette={…}` from data (dynamic), so those
// `.color-palette_*` classes would never be generated. Evaluating each palette
// here, at module scope, forces cssgen to emit the rules — the components'
// recipe functions then apply the same classes at runtime.
import { css } from 'styled-system/css'

export const colorPaletteSeeds = {
  brown: css({ colorPalette: 'brown' }),
  gray: css({ colorPalette: 'gray' }),
  green: css({ colorPalette: 'green' }),
  blue: css({ colorPalette: 'blue' }),
  amber: css({ colorPalette: 'amber' }),
  red: css({ colorPalette: 'red' }),
} as const
