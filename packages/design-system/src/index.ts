// Theme — Park UI preset (brown accent, sand gray, Outfit) as plain data,
// consumable by any Panda config.
export { parkUI, colorPalettes, tokens, presets } from './theme'

// Hooks
export { createColorMode } from './hooks/color-mode'
export type { ColorMode, ColorModeController } from './hooks/color-mode'

// Ark UI collection helper (Select/Combobox/DatePicker/etc.) — re-exported
// so consumers don't need a direct @ark-ui/solid dependency.
export { createListCollection } from '@ark-ui/solid'
export type { CollectionItem, ListCollection } from '@ark-ui/solid'

// Components — the full Park UI Solid catalog (vendored, MIT), re-exported.
export * from './components/ui'
