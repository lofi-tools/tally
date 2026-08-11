import { createEffect, createSignal, type Accessor } from 'solid-js'

export type ColorMode = 'light' | 'dark'

const STORAGE_KEY = 'tally-color-mode'

function initialMode(): ColorMode {
  try {
    // Guard for SSR (Astro server-renders islands): no window/localStorage.
    if (typeof localStorage !== 'undefined') {
      const stored = localStorage.getItem(STORAGE_KEY)
      if (stored === 'light' || stored === 'dark') return stored
    }
  } catch {
    /* storage unavailable — fall through */
  }
  // Dark is the identity; only an explicit stored choice opts into light.
  return 'dark'
}

export interface ColorModeController {
  /** Current mode ('light' | 'dark'), reactive. */
  mode: Accessor<ColorMode>
  set: (mode: ColorMode) => void
  toggle: () => void
}

/**
 * Class-based dark mode: toggles `.dark` on `<html>` (which Panda's `_dark`
 * semantic-token condition targets) and persists the choice to localStorage.
 * The initial value is read from localStorage, defaulting to **dark** (the
 * Tally brand identity — see DESIGN.md) — the app's `index.html`/Astro page
 * applies the same logic before first paint. SSR-safe: the storage read is
 * guarded and the effect only touches `document`/`localStorage` on the client
 * (Solid effects don't run during server render anyway).
 */
export function createColorMode(): ColorModeController {
  const [mode, setMode] = createSignal<ColorMode>(initialMode())

  createEffect(() => {
    if (typeof document !== 'undefined') {
      document.documentElement.classList.toggle('dark', mode() === 'dark')
    }
    try {
      if (typeof localStorage !== 'undefined') {
        localStorage.setItem(STORAGE_KEY, mode())
      }
    } catch {
      /* storage unavailable — the class still toggles */
    }
  })

  return {
    mode,
    set: (m) => setMode(m),
    toggle: () => setMode((m) => (m === 'dark' ? 'light' : 'dark')),
  }
}
