import { createSignal, onCleanup } from 'solid-js'
import { Button } from '@tally/design-system'
import { RotateCcw } from 'lucide-solid'
import { css } from 'styled-system/css'
import { DB_KEY } from '../db'
import { TOKEN_KEY } from '../api'
import { session } from '../session'

/**
 * Dev-only strip pinned to the bottom of the app.
 *
 * `import.meta.env.DEV` is a Vite compile-time constant: vite replaces it
 * with `true` during `vite dev` and `false` at build time, and dead-code
 * elimination then strips this branch — and its imports — out of the
 * production bundle entirely. Nothing here ships to prod.
 */
export function DevtoolsBanner() {
  if (!import.meta.env.DEV) return null

  // Two-step reset: the first click arms the button, the second executes.
  // Dev data is throwaway, but a stray click wiping companies is still annoying.
  const [armed, setArmed] = createSignal(false)
  let timer: ReturnType<typeof setTimeout> | undefined

  const reset = () => {
    if (!armed()) {
      setArmed(true)
      timer = setTimeout(() => setArmed(false), 4000)
      return
    }
    if (timer) clearTimeout(timer)
    // Wipe UI state (companies, data sources, session token) and land back
    // on the empty onboarding screen. Backend data is untouched — see the
    // `nix develop -c reset` hint.
    localStorage.removeItem(DB_KEY)
    localStorage.removeItem(TOKEN_KEY)
    location.reload()
  }
  onCleanup(() => {
    if (timer) clearTimeout(timer)
  })

  const s = session()

  return (
    <div
      class={css({
        display: 'flex',
        alignItems: 'center',
        gap: '3',
        flexShrink: '0',
        px: '3',
        py: '1.5',
        bg: 'gray.a3',
        borderTop: '1px solid {colors.border}',
        fontFamily: 'mono',
        fontSize: 'xs',
        color: 'fg.muted',
        userSelect: 'none',
      })}
    >
      <span class={css({ display: 'inline-flex', alignItems: 'center', gap: '1.5', fontWeight: '600', color: 'fg.default' })}>
        <span aria-hidden="true" class={css({ w: '1.5', h: '1.5', borderRadius: 'full', bg: 'amber.solid.bg' })} />
        dev
      </span>
      <span class={css({ color: 'fg.subtle' })}>mode: {import.meta.env.MODE}</span>
      <span class={css({ color: 'fg.subtle' })}>session: {s.status}</span>
      <span class={css({ flex: '1' })} />
      <span
        class={css({ color: 'fg.subtle', userSelect: 'text', _hover: { color: 'fg.muted' } })}
        title="UI data lives in localStorage; backend data lives in Postgres — wipe that with this command too"
      >
        db: nix develop -c reset
      </span>
      <Button
        size="xs"
        variant="outline"
        onClick={reset}
        title="Clears local companies, data sources and your session, then reloads"
        class={css({
          // Fixed width: the armed label swap must not nudge the strip.
          w: '8rem',
          justifyContent: 'center',
          ...(armed()
            ? {
                bg: 'amber.solid.bg',
                borderColor: 'amber.solid.bg',
                color: 'amber.solid.fg',
                _hover: { bg: 'amber.solid.bg', borderColor: 'amber.solid.bg' },
              }
            : {}),
        })}
      >
        <RotateCcw class={css({ w: '3', h: '3' })} />
        {armed() ? 'Confirm reset' : 'Reset'}
      </Button>
    </div>
  )
}
