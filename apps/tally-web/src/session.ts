// Session store (docs/spec/web-api-wiring-spec.md §14.2).
//
// A single module-scope Solid signal (created inside createRoot, so it's
// usable from anywhere) holding the auth state machine:
//
//   restoring → signed-in | local | offline
//   local     → signed-in (login/register)
//   signed-in → local (sign out / expired)
//   signed-in | restoring → offline (network fault; token kept)
//
// `api()` imports `handleExpired` (runtime-only cycle — both modules only
// call each other's exports inside function bodies, which is safe in ESM).

import { createRoot, createSignal, type Setter } from 'solid-js'
import { toaster } from '@tally/design-system'
import { ApiError, getToken, logout, me, NetworkError, setToken, type AuthUser } from './api'

export type Session =
  | { status: 'restoring' } // token present, /auth/me pending
  | { status: 'local' } // no valid session (default mode)
  | { status: 'signed-in'; user: AuthUser } // API mode
  | { status: 'offline' } // API unreachable (token kept)

export const [session, setSession]: [
  () => Session,
  Setter<Session>,
] = createRoot(() => createSignal<Session>({ status: 'local' }))

/** Clear the token and drop to local mode (used by the sign-out button). */
export async function signOut(): Promise<void> {
  try {
    await logout()
  } catch {
    // Offline or already expired — still clear locally.
  }
  setToken(null)
  setSession({ status: 'local' })
}

/**
 * A runtime 401: clear the token and drop to local mode. Only tells the
 * user when a session was actually live (or restoring) — so parallel
 * requests against one expired token toast once, and a manual sign-out
 * with an already-expired session doesn't toast "Session expired" at all.
 */
export function handleExpired(): void {
  const hadSession = session().status === 'signed-in' || session().status === 'restoring'
  setToken(null)
  setSession({ status: 'local' })
  if (hadSession) {
    toaster.create({
      title: 'Session expired',
      description: 'Sign in again to keep using the API.',
      type: 'warning',
    })
  }
}

/** A network fault: keep the token, mark the session offline (retry later). */
export function markOffline(): void {
  setSession((s) => (s.status === 'signed-in' || s.status === 'restoring' ? { status: 'offline' } : s))
}

/**
 * App boot: restore a stored token via `GET /auth/me` (§5.2).
 * - 200 → signed-in
 * - 401 (auth_invalid/auth_expired) → handleExpired (cleared + toast)
 * - network failure → offline (not silent local fallback)
 */
export async function restoreSession(): Promise<void> {
  if (!getToken()) {
    setSession({ status: 'local' })
    return
  }
  setSession({ status: 'restoring' })
  try {
    const user = await me()
    setSession({ status: 'signed-in', user })
  } catch (e) {
    if (e instanceof NetworkError) {
      markOffline()
    } else if (e instanceof ApiError && e.status === 401) {
      // The wrapper already ran handleExpired() (clear + toast).
    } else {
      // Unknown fault: don't strand the user — fall back to local.
      setSession({ status: 'local' })
    }
  }
}
