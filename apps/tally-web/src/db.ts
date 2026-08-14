// Versioned localStorage store for the pieces that still live client-side.
//
// User-added companies are API-backed end-to-end now (temp-user spec §7.4):
// they live on the backend (owned by the signed-in user or a guest
// workspace) and are fetched via `GET /companies`, so the local DB no longer
// holds company rows. What stays here: per-company data-source connections
// (still a mock until Open Banking lands) — the demo company is pre-seeded.
import { dataSources, DEMO_COMPANY_ID, type DataSource } from './mock_data'

export interface Db {
  version: 1
  /** Data sources per company id (API company ids); the demo is pre-seeded. */
  sources: Record<string, DataSource[]>
}

export const DB_KEY = 'tally.db.v1'

function defaults(): Db {
  return {
    version: 1,
    sources: { [DEMO_COMPANY_ID]: [...dataSources] },
  }
}

export function loadDb(): Db {
  try {
    const raw = localStorage.getItem(DB_KEY)
    if (!raw) return defaults()
    const parsed = JSON.parse(raw) as Partial<Db>
    return {
      ...defaults(),
      ...parsed,
      sources: { ...defaults().sources, ...(parsed.sources ?? {}) },
    }
  } catch {
    return defaults()
  }
}

export function saveDb(db: Db): void {
  try {
    localStorage.setItem(DB_KEY, JSON.stringify(db))
  } catch {
    // Storage unavailable (private mode etc.) — the app still works in memory.
  }
}
