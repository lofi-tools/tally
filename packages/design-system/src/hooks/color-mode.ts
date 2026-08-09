import { createEffect, createSignal, type Accessor } from 'solid-js'

export type ColorMode = 'light' | 'dark'

const STORAGE_KEY = 'tally-color-mode'

function initialMode(): ColorMode {
  try {
    const stored = localStorage.getItem(STORAGE_KEY)
    if (stored === 'light' || stored === 'dark') return stored
    if (window.matchMedia('(prefers-color-scheme: dark)').matches) return 'dark'
  } catch {
    /* storage unavailable — fall through */
  }
  return 'light'
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
 * The initial value is read from localStorage, falling back to the OS
 * preference — `index.html` applies the same logic before first paint.
 */
export function createColorMode(): ColorModeController {
  const [mode, setMode] = createSignal<ColorMode>(initialMode())

  createEffect(() => {
    document.documentElement.classList.toggle('dark', mode() === 'dark')
    try {
      localStorage.setItem(STORAGE_KEY, mode())
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
