// Versioned localStorage "database" for the mock app. There is no backend
// yet, so everything the user creates persists here instead. Swap this module
// for real API calls later — the rest of the app only reads/writes via
// loadDb/saveDb.
import { dataSources, SAMPLE_COMPANY_ID, type Company, type DataSource } from './mock_data'

export interface Db {
  version: 1
  /** User-added companies (never includes the sample). */
  companies: Company[]
  /** Data sources per company id; the sample company is pre-seeded. */
  sources: Record<string, DataSource[]>
}

export const DB_KEY = 'tally.db.v1'

function defaults(): Db {
  return {
    version: 1,
    companies: [],
    sources: { [SAMPLE_COMPANY_ID]: [...dataSources] },
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
    // Storage unavailable (private mode etc.) — the mock app still works in memory.
  }
}
