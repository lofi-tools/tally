// Typed client for `tally-api` (docs/spec/web-api-wiring-spec.md §14).
//
// No UI here. Views branch on `ApiError.code` (stable snake_case), never on
// `message` text. The fetch wrapper is the single place that attaches the
// bearer token and intercepts 401s (§14.3).

import { handleExpired } from './session'

// Vite dev proxy forwards `/api/*` to 127.0.0.1:8080 (see vite.config.ts).
const API_BASE = '/api/v1'

// ---------------------------------------------------------------------------
// Token storage (§14.3)
// ---------------------------------------------------------------------------

export const TOKEN_KEY = 'tally.token.v1'

export const getToken = (): string | null => localStorage.getItem(TOKEN_KEY)

export const setToken = (t: string | null): void => {
  if (t === null) localStorage.removeItem(TOKEN_KEY)
  else localStorage.setItem(TOKEN_KEY, t)
}

// ---------------------------------------------------------------------------
// Errors (§14.3 / §14.5)
// ---------------------------------------------------------------------------

/** A parsed API envelope: `{ error: { code, message, details? } }`. */
export class ApiError extends Error {
  constructor(
    readonly code: string,
    readonly message: string,
    readonly details: Record<string, unknown> | undefined,
    readonly status: number,
  ) {
    super(message)
    this.name = 'ApiError'
  }
}

/** fetch threw, or a response body wasn't parseable JSON. */
export class NetworkError extends Error {
  constructor() {
    super('network error')
    this.name = 'NetworkError'
  }
}

// ---------------------------------------------------------------------------
// Fetch wrapper (§14.3)
// ---------------------------------------------------------------------------

export interface ApiOptions {
  method?: 'GET' | 'POST' | 'PATCH' | 'DELETE'
  /** JSON body (sets content-type application/json). */
  json?: unknown
  /** Multipart body (browser sets the boundary; no content-type set). */
  form?: FormData
  /** Default true-when-token; `false` never sends the token (CH search). */
  auth?: boolean
  /** Extra request headers (e.g. `X-Guest-Id` for the guest bootstrap). */
  headers?: Record<string, string>
  /** Resolve with the raw Response (report blobs). */
  raw?: boolean
  signal?: AbortSignal
}

export async function api<T = unknown>(path: string, opts: ApiOptions = {}): Promise<T> {
  const headers: Record<string, string> = { Accept: 'application/json' }
  if (opts.auth !== false) {
    const token = getToken()
    if (token) headers.Authorization = `Bearer ${token}`
  }
  if (opts.headers) Object.assign(headers, opts.headers)
  let body: BodyInit | undefined
  if (opts.json !== undefined) {
    headers['Content-Type'] = 'application/json'
    body = JSON.stringify(opts.json)
  } else if (opts.form) {
    body = opts.form
  }

  let resp: Response
  try {
    resp = await fetch(`${API_BASE}${path}`, {
      method: opts.method ?? 'GET',
      headers,
      body,
      signal: opts.signal,
    })
  } catch {
    throw new NetworkError()
  }

  if (resp.ok) {
    if (opts.raw) return resp as T
    if (resp.status === 204) return undefined as T
    try {
      return (await resp.json()) as T
    } catch {
      // A 2xx with a non-JSON body is a proxy/server fault from our side.
      throw new NetworkError()
    }
  }

  // Non-ok: parse the envelope, then handle 401s centrally.
  let payload: unknown
  try {
    payload = await resp.json()
  } catch {
    throw new NetworkError()
  }
  const error = (payload as { error?: { code?: unknown; message?: unknown; details?: unknown } })?.error
  const code = typeof error?.code === 'string' ? error.code : undefined
  const message = typeof error?.message === 'string' ? error.message : undefined
  if (code && message) {
    const err = new ApiError(
      code,
      message,
      typeof error?.details === 'object' && error.details !== null
        ? (error.details as Record<string, unknown>)
        : undefined,
      resp.status,
    )
    if (
      resp.status === 401 &&
      (code === 'auth_expired' || code === 'auth_invalid' || code === 'auth_missing')
    ) {
      handleExpired()
    }
    throw err
  }
  // A non-2xx that isn't the envelope: proxy/server fault.
  throw new NetworkError()
}

// ---------------------------------------------------------------------------
// Types (mirror the API's serialized shapes — models.rs / ledgers.rs / auth.rs)
// ---------------------------------------------------------------------------

// ---- auth ----
export interface AuthUser {
  id: string
  email: string
  display_name: string
  created_at: string
  /** True for guest workspaces (temp-user spec §4) — the UI treats it as guest mode. */
  is_temporary: boolean
  /** The client-generated anonymous identity; null for real users. */
  guest_id: string | null
}
export interface AuthResponse {
  token: string
  user: AuthUser
}

// ---- companies ----
/** Full serialized Company model (models.rs — every profile field). */
export interface Company {
  id: string
  user_id: string
  name: string
  tax_reference: string
  company_number: string
  registration_date: string | null
  directors: string[]
  contact_name: string | null
  address_lines: string[]
  county: string | null
  location: string | null
  postcode: string | null
  email: string | null
  phone_country: string | null
  phone_area: string | null
  phone_number: string | null
  website_url: string | null
  website_description: string | null
  vat_registration: string | null
  sic_codes: string[]
  activities: string | null
  jurisdiction: string | null
  accountant_name: string | null
  accountant_business: string | null
  accountant_address: string | null
  auditor_name: string | null
  auditor_business: string | null
  auditor_address: string | null
  industry_sector_dimension: string | null
  legal_form_dimension: string | null
  country_dimension: string | null
  contact_country_dimension: string | null
  phone_type_dimension: string | null
  logo_b64: string | null
  accounting_standard: string
  updated_at: string | null
  fy1_year: number
  fy2_year: number
  associated_companies: number | null
  report_date: string | null
  authorised_date: string | null
  incorporation_date: string | null
  signed_by: string | null
  average_employees: Record<string, number> | null
  signature_b64: string | null
}

/** All-optional §5 union — the same shape serves create and PATCH. */
export interface CompanyInput {
  name?: string
  tax_reference?: string
  company_number?: string
  registration_date?: string
  directors?: string[]
  contact_name?: string
  address_lines?: string[]
  county?: string
  location?: string
  postcode?: string
  email?: string
  phone_country?: string
  phone_area?: string
  phone_number?: string
  website_url?: string
  website_description?: string
  vat_registration?: string
  sic_codes?: string[]
  activities?: string
  jurisdiction?: string
  accountant_name?: string
  accountant_business?: string
  accountant_address?: string
  auditor_name?: string
  auditor_business?: string
  auditor_address?: string
  industry_sector_dimension?: string
  legal_form_dimension?: string
  country_dimension?: string
  contact_country_dimension?: string
  phone_type_dimension?: string
  logo_b64?: string
  accounting_standard?: 'FRS 105' | 'FRS 102'
  fy1_year?: number
  fy2_year?: number
  associated_companies?: number
  report_date?: string
  authorised_date?: string
  incorporation_date?: string
  signed_by?: string
  average_employees?: Record<string, number>
  signature_b64?: string
}

/** A Companies House search result (companies_house.rs SearchItem). */
export interface SearchItem {
  company_number: string
  company_name: string
  company_status?: string | null
  date_of_creation?: string | null
  address_snippet?: string | null
  company_type?: string | null
  description?: string | null
}

// ---- ledgers ----
export interface Ledger {
  id: string
  company_id: string
  name: string
  file_path: string
  file_sha256: string
  uploaded_at: string
  accounts_count: number
  transactions_count: number
  splits_count: number
}

export interface AccountNode {
  guid: string
  name: string
  type: string
  balance: string // decimal string, GnuCash-natural sign
  children: AccountNode[]
}
export interface AccountsView {
  accounts: AccountNode[]
  net_assets: string
}

export interface Split {
  account_guid: string
  value: string // decimal string
}

export interface LedgerTransaction {
  guid: string
  post_datetime: string // RFC 3339
  description: string
  splits: Split[]
}
export interface TransactionsPage {
  items: LedgerTransaction[]
  limit: number
  offset: number
}

// ---- filings (ch-filings-sync-spec §5) ----
/** The 12 balance-sheet line items, in the `Frs105Accounts` field names. */
export interface PreviousYearFigures {
  fixed_assets: number
  called_up_share_capital_not_paid: number
  current_assets: number
  prepayments_and_accrued_income: number
  creditors_within_1_year: number
  net_current_assets: number
  total_assets_less_liabilities: number
  creditors_after_1_year: number
  provisions_for_liabilities: number
  accruals_and_deferred_income: number
  net_assets: number
  capital_and_reserves: number
}

export interface PeriodDue {
  /** CT600 deadline (HMRC): 12 months after the period end. */
  hmrc: string
  /** Accounts deadline (Companies House): 9 months after the period end. */
  ch: string
}

export interface PeriodFiling {
  kind: 'accounts' | 'confirmation-statement' | 'corporation-tax' | 'other'
  state: 'confirmed' | 'not-sent'
  filed_on?: string | null
  form_type?: string | null
  description?: string | null
  document_metadata_url?: string | null
}

export interface Period {
  start: string
  end: string
  status: 'filed' | 'pending' | 'ongoing' | 'provisional'
  due: PeriodDue | null
  filings: PeriodFiling[]
}

export interface BalanceSheet {
  period_end: string
  filed_on: string | null
  figures: PreviousYearFigures
}

export interface FetchStatus {
  state: 'none' | 'pending' | 'running' | 'done' | 'failed'
  fetched_at: string | null
  last_error: string | null
}

export interface FilingsViewData {
  periods: Period[]
  balance_sheets: BalanceSheet[]
  status: FetchStatus
}

// ---- reports ----
export interface ReportRequest {
  ledger_id: string
  period?: { start: string; end: string }
  made_up_to?: string
  declaration?: { name?: string; status?: string }
}

// ---------------------------------------------------------------------------
// Endpoint functions (§14.4)
// ---------------------------------------------------------------------------

// ---- auth ----
/**
 * Register (or, with `guestId`, adopt the guest workspace in place — temp-
 * user spec §5.2/§7.6). The `X-Guest-Id` header is only sent when a guest
 * id is passed.
 */
export const register = (
  body: { display_name: string; email: string; password: string },
  guestId?: string,
): Promise<AuthResponse> =>
  api<AuthResponse>('/auth/register', {
    method: 'POST',
    json: body,
    ...(guestId ? { headers: { 'X-Guest-Id': guestId } } : {}),
  })

export const login = (body: { email: string; password: string }): Promise<AuthResponse> =>
  api<AuthResponse>('/auth/login', { method: 'POST', json: body })

/** Bootstrap (or re-issue) a guest session for a client-generated id (§5.1). */
export const bootstrapGuest = (guestId: string): Promise<AuthResponse> =>
  api<AuthResponse>('/auth/guest', { method: 'POST', headers: { 'X-Guest-Id': guestId } })

export const logout = (): Promise<void> => api<void>('/auth/logout', { method: 'POST' })

export const me = (): Promise<AuthUser> => api<AuthUser>('/auth/me')

// ---- companies ----
export const listCompanies = (): Promise<Company[]> => api<Company[]>('/companies')

export const createCompany = (input: CompanyInput): Promise<Company> =>
  api<Company>('/companies', { method: 'POST', json: input })

export const getCompany = (id: string): Promise<Company> => api<Company>(`/companies/${id}`)

export const patchCompany = (id: string, input: CompanyInput): Promise<Company> =>
  api<Company>(`/companies/${id}`, { method: 'PATCH', json: input })

export const deleteCompany = (id: string): Promise<void> =>
  api<void>(`/companies/${id}`, { method: 'DELETE' })

/** Companies House search — deliberately unauthenticated (§7.2). */
export const searchCompanies = (q: string): Promise<SearchItem[]> =>
  api<SearchItem[]>(`/companies/search?q=${encodeURIComponent(q)}`, { auth: false })

export const enrichCompany = (id: string): Promise<Company> =>
  api<Company>(`/companies/${id}/enrich`, { method: 'POST' })

// ---- filings ----
/** The company's financial periods + balance sheets + fetch status (§5). */
export const listFilings = (companyId: string): Promise<FilingsViewData> =>
  api<FilingsViewData>(`/companies/${companyId}/filings`)

/**
 * Re-enqueue the filing-history backfill. Resolves with `{ job_id }` on
 * both 202 (new job) and 200 (a job is already pending/running).
 */
export const refreshFilings = (companyId: string): Promise<{ job_id: string | null }> =>
  api<{ job_id: string | null }>(`/companies/${companyId}/filings/refresh`, { method: 'POST' })

// ---- ledgers ----
export const listLedgers = (companyId: string): Promise<Ledger[]> =>
  api<Ledger[]>(`/companies/${companyId}/ledgers`)

export const uploadLedger = (companyId: string, file: File): Promise<Ledger> => {
  const form = new FormData()
  form.append('file', file)
  return api<Ledger>(`/companies/${companyId}/ledgers`, { method: 'POST', form })
}

export const deleteLedger = (id: string): Promise<void> =>
  api<void>(`/ledgers/${id}`, { method: 'DELETE' })

export const ledgerAccounts = (id: string): Promise<AccountsView> =>
  api<AccountsView>(`/ledgers/${id}/accounts`)

export const ledgerTransactions = (
  id: string,
  q?: { limit?: number; offset?: number },
): Promise<TransactionsPage> => {
  const params = new URLSearchParams()
  if (q?.limit !== undefined) params.set('limit', String(q.limit))
  if (q?.offset !== undefined) params.set('offset', String(q.offset))
  const qs = params.toString()
  return api<TransactionsPage>(`/ledgers/${id}/transactions${qs ? `?${qs}` : ''}`)
}

// ---- reports (raw documents) ----
/**
 * Generate a report document and return an object URL for it (fetch → blob,
 * because the bearer token can't ride a plain `<a href>`, §10). The caller
 * revokes the URL when done (`URL.revokeObjectURL`).
 */
export async function generateReportDocument(
  companyId: string,
  kind: 'accounts' | 'corp-tax' | 'ct600',
  body: ReportRequest,
): Promise<string> {
  const path =
    kind === 'corp-tax'
      ? `/companies/${companyId}/reports/corp-tax.json`
      : `/companies/${companyId}/reports/${kind}`
  const resp = await api<Response>(path, { method: 'POST', json: body, raw: true })
  const blob = await resp.blob()
  return URL.createObjectURL(blob)
}
