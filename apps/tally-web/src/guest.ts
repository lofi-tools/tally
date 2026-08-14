// The browser's stable anonymous identity (temp-user spec §7.1).
//
// A random UUID generated on first use and stored under `tally.guest.v1`
// (mirroring `tally.token.v1`). It is sent as the `X-Guest-Id` header to
// `POST /auth/guest` (and register, for adoption). Never deleted by the app:
// it survives sign-in/out and is the identity the backend maps a temporary
// user to.

export const GUEST_KEY = 'tally.guest.v1'

export const getGuestId = (): string | null => localStorage.getItem(GUEST_KEY)

/** The guest id, generating + persisting one on first use. */
export function ensureGuestId(): string {
  const existing = getGuestId()
  if (existing) return existing
  const id = crypto.randomUUID()
  try {
    localStorage.setItem(GUEST_KEY, id)
  } catch {
    // Storage unavailable — the session still works for this page load.
  }
  return id
}
