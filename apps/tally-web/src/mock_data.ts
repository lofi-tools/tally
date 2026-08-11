// Mock data for the Tally web app.
//
// There is no backend yet — every value here is invented. When the Rust
// crates expose APIs, replace these exports with real fetches; the view
// components only consume the exported types, so swapping the data source
// stays contained to this file.

// ---------- formatting ----------
export const fmtMoney = (n: number) =>
  new Intl.NumberFormat('en-GB', { style: 'currency', currency: 'GBP' }).format(Math.abs(n))

export const fmtSignedMoney = (n: number) => `${n < 0 ? '−' : '+'}${fmtMoney(n)}`

export const fmtDate = (iso: string) =>
  new Date(iso).toLocaleDateString('en-GB', { day: 'numeric', month: 'short', year: 'numeric' })

export const fmtMonth = (iso: string) =>
  new Date(iso).toLocaleDateString('en-GB', { month: 'short', year: 'numeric' })

// ---------- companies ----------
export interface Company {
  id: string
  name: string
  companyNumber: string
  utr: string
  sic: string
  address: string
  standard: 'FRS 105' | 'FRS 102'
}

// The demo entity shown to users before they add their own company. It is
// never persisted and carries the full seeded dataset (see sampleCompanyData).
export const SAMPLE_COMPANY_ID = 'sample'

export const sampleCompany: Company = {
  id: SAMPLE_COMPANY_ID,
  name: 'Sample Co Ltd',
  companyNumber: '00000000',
  utr: '—',
  sic: '—',
  address: '—',
  standard: 'FRS 105',
}

// ---------- simulated Companies House search ----------
// The real search endpoint will forward to Companies House; for now results
// come from this local fixture so the onboarding flow can be exercised.
export interface CompanySearchResult {
  name: string
  companyNumber: string
  sic: string
  incorporationDate: string
  jurisdiction: string
  address: string
}

export const companySearchResults: CompanySearchResult[] = [
  {
    name: 'Northwind Trading Ltd',
    companyNumber: '12345678',
    sic: '47910 — Retail sale via mail order or internet',
    incorporationDate: '2018-03-14',
    jurisdiction: 'England and Wales',
    address: '12 Market Street, Manchester, M1 1AA',
  },
  {
    name: 'Ashford & Cole Ltd',
    companyNumber: '09651234',
    sic: '70229 — Management consultancy activities',
    incorporationDate: '2016-11-02',
    jurisdiction: 'England and Wales',
    address: '27 Queen Square, Bristol, BS1 4NH',
  },
  {
    name: 'Brightline Media Ltd',
    companyNumber: '11345678',
    sic: '73110 — Advertising agencies',
    incorporationDate: '2020-01-30',
    jurisdiction: 'England and Wales',
    address: '4 Clerkenwell Road, London, EC1M 5PS',
  },
  {
    name: 'Harbour & Finch Ltd',
    companyNumber: '13456789',
    sic: '56101 — Restaurants and mobile food service activities',
    incorporationDate: '2019-07-19',
    jurisdiction: 'Scotland',
    address: '8 Shore Road, Edinburgh, EH6 6QW',
  },
  {
    name: 'Meridian Trade Co Ltd',
    companyNumber: '14567890',
    sic: '46190 — Agents involved in the sale of a variety of goods',
    incorporationDate: '2021-04-08',
    jurisdiction: 'England and Wales',
    address: '55 Anchor Lane, Liverpool, L3 4DP',
  },
  {
    name: 'Cedar & Vine Studio Ltd',
    companyNumber: '15678901',
    sic: '71111 — Architectural activities',
    incorporationDate: '2017-09-25',
    jurisdiction: 'England and Wales',
    address: '102 Regent Court, Leeds, LS2 7QA',
  },
  {
    name: 'Ironwood Logistics Ltd',
    companyNumber: '16789012',
    sic: '49410 — Freight transport by road',
    incorporationDate: '2015-02-11',
    jurisdiction: 'England and Wales',
    address: '3 Gateway Business Park, Birmingham, B4 6TF',
  },
  {
    name: 'Penrose Healthcare Group Ltd',
    companyNumber: '17890123',
    sic: '86900 — Other human health activities',
    incorporationDate: '2022-06-21',
    jurisdiction: 'Wales',
    address: '18 Bryn Road, Cardiff, CF10 3DX',
  },
]

/** Mock Companies House search: case-insensitive name / company-number match. */
export function searchCompanies(query: string): CompanySearchResult[] {
  const q = query.trim().toLowerCase()
  if (!q) return []
  return companySearchResults.filter(
    (r) => r.name.toLowerCase().includes(q) || r.companyNumber.includes(q),
  )
}

// ---------- transactions ----------
export interface Transaction {
  id: string
  date: string // ISO
  description: string
  account: string
  source: 'Starling' | 'Barclays' | 'Manual'
  amount: number // GBP; positive = income, negative = expense
  status: 'cleared' | 'pending' | 'matched'
}

export const transactions: Transaction[] = [
  { id: 't01', date: '2026-08-07', description: 'Sale — invoice #1042 (BrightBox Ltd)', account: 'Sales', source: 'Starling', amount: 4280.0, status: 'cleared' },
  { id: 't02', date: '2026-08-05', description: 'Stripe payout — July online sales', account: 'Sales', source: 'Starling', amount: 9310.2, status: 'cleared' },
  { id: 't03', date: '2026-08-04', description: 'Rent — Unit 3, Market Street', account: 'Rent', source: 'Barclays', amount: -1850.0, status: 'cleared' },
  { id: 't04', date: '2026-08-01', description: 'HMRC — CT payment (FY2024/25)', account: 'Tax', source: 'Barclays', amount: -2874.55, status: 'cleared' },
  { id: 't05', date: '2026-07-31', description: 'Salaries — July', account: 'Payroll', source: 'Barclays', amount: -12840.0, status: 'cleared' },
  { id: 't06', date: '2026-07-29', description: 'Octopus Energy — July electricity', account: 'Utilities', source: 'Starling', amount: -412.18, status: 'pending' },
  { id: 't07', date: '2026-07-27', description: 'Sale — invoice #1041 (Cobble & Pine)', account: 'Sales', source: 'Starling', amount: 3150.0, status: 'cleared' },
  { id: 't08', date: '2026-07-24', description: 'AWS — hosting, July', account: 'Software', source: 'Barclays', amount: -96.42, status: 'matched' },
  { id: 't09', date: '2026-07-20', description: 'BT Business — line rental', account: 'Telecom', source: 'Barclays', amount: -74.99, status: 'cleared' },
  { id: 't10', date: '2026-07-15', description: 'VAT — payment to HMRC (Q2)', account: 'Tax', source: 'Barclays', amount: -2210.0, status: 'cleared' },
  { id: 't11', date: '2026-07-12', description: 'Sale — invoice #1040 (Halewood Studio)', account: 'Sales', source: 'Starling', amount: 2048.5, status: 'cleared' },
  { id: 't12', date: '2026-07-08', description: 'Google Workspace — 6 seats', account: 'Software', source: 'Starling', amount: -54.0, status: 'matched' },
  { id: 't13', date: '2026-07-02', description: 'Insurance — annual policy premium', account: 'Insurance', source: 'Barclays', amount: -1285.0, status: 'cleared' },
  { id: 't14', date: '2026-06-30', description: 'Salaries — June', account: 'Payroll', source: 'Barclays', amount: -12840.0, status: 'cleared' },
  { id: 't15', date: '2026-06-27', description: 'Sale — invoice #1039 (BrightBox Ltd)', account: 'Sales', source: 'Starling', amount: 3560.0, status: 'cleared' },
  { id: 't16', date: '2026-06-18', description: 'Office supplies — Printworks', account: 'Office', source: 'Starling', amount: -238.4, status: 'cleared' },
  // Incorporation-era entries: they give Assets / Liabilities / Equity derived
  // balances in the chart of accounts without disturbing the "recent" lists.
  { id: 't17', date: '2024-04-02', description: 'Loan drawdown — Lloyds', account: 'Loan', source: 'Barclays', amount: -20000, status: 'cleared' },
  { id: 't18', date: '2024-04-01', description: 'Share capital — incorporation', account: 'Share capital', source: 'Manual', amount: -15000, status: 'cleared' },
  { id: 't19', date: '2024-04-01', description: 'Opening balance — bank transfer', account: 'Starling', source: 'Starling', amount: 45000, status: 'cleared' },
]

// ---------- chart of accounts ----------
// GnuCash-style top-level groups with their leaf accounts. Leaf names MUST
// match `Transaction.account`, because balances are derived from the
// transactions list (tree and table therefore always agree).
export interface AccountLeaf {
  name: string
}

export interface AccountGroup {
  name: string
  accounts: AccountLeaf[]
}

export const chartOfAccounts: AccountGroup[] = [
  { name: 'Assets', accounts: [{ name: 'Starling' }, { name: 'Barclays' }] },
  { name: 'Liabilities', accounts: [{ name: 'Loan' }] },
  { name: 'Equity', accounts: [{ name: 'Share capital' }] },
  { name: 'Income', accounts: [{ name: 'Sales' }] },
  {
    name: 'Expenses',
    accounts: [
      { name: 'Rent' },
      { name: 'Utilities' },
      { name: 'Software' },
      { name: 'Telecom' },
      { name: 'Insurance' },
      { name: 'Office' },
      { name: 'Payroll' },
      { name: 'Tax' },
    ],
  },
]

export function transactionsFor(account: string): Transaction[] {
  return transactions.filter((t) => t.account === account)
}

/** Balance of one account: the sum of its transactions (app sign convention). */
export function accountBalance(account: string): number {
  return transactionsFor(account).reduce((s, t) => s + t.amount, 0)
}

/** Rolled-up total of a top-level group's leaf accounts. */
export function groupBalance(group: AccountGroup): number {
  return group.accounts.reduce((s, a) => s + accountBalance(a.name), 0)
}

/** Leaf account names in chart order (drives the Transactions filter dropdown). */
export function chartAccountNames(): string[] {
  return chartOfAccounts.flatMap((g) => g.accounts.map((a) => a.name))
}

// ---------- summaries ----------
export interface MonthSummary {
  month: string
  income: number
  expenses: number
  vat: number
}

export const summaries: MonthSummary[] = [
  { month: 'Mar 2026', income: 36140, expenses: 20890, vat: 3210 },
  { month: 'Apr 2026', income: 38240, expenses: 21300, vat: 3480 },
  { month: 'May 2026', income: 41720, expenses: 22450, vat: 3790 },
  { month: 'Jun 2026', income: 35480, expenses: 20180, vat: 3225 },
  { month: 'Jul 2026', income: 38950, expenses: 23200, vat: 3540 },
  { month: 'Aug 2026', income: 33610, expenses: 19840, vat: 3055 },
]

// ---------- data sources ----------
export interface DataSource {
  id: string
  name: string
  kind: 'bank' | 'csv' | 'manual'
  institution?: string
  status: 'connected' | 'needs-auth' | 'pending'
  lastSync: string
  accountCount: number
}

export const dataSources: DataSource[] = [
  { id: 'starling', name: 'Starling Business', kind: 'bank', institution: 'Starling', status: 'connected', lastSync: '2 min ago', accountCount: 2 },
  { id: 'barclays', name: 'Barclays Business', kind: 'bank', institution: 'Barclays', status: 'needs-auth', lastSync: '3 weeks ago', accountCount: 1 },
  { id: 'ledger', name: 'Manual ledger entries', kind: 'manual', status: 'connected', lastSync: 'today', accountCount: 1 },
]

export const bankOptions = [
  { id: 'starling', name: 'Starling' },
  { id: 'barclays', name: 'Barclays' },
  { id: 'monzo', name: 'Monzo' },
  { id: 'hsbc', name: 'HSBC' },
  { id: 'natwest', name: 'NatWest' },
]

// ---------- filings ----------
export const financialYears = ['FY2025/26', 'FY2024/25', 'FY2023/24', 'FY2022/23']

export const nextFiling = {
  period: 'FY2025/26',
  type: 'Micro-entity accounts (FRS 105)',
  start: '2025-04-01',
  end: '2026-03-31',
  due: '2026-12-31',
  daysLeft: 142,
  progress: 65, // percent of the accounts prepared
}

export interface PreviousFiling {
  period: string
  type: string
  filed: string // ISO
  status: 'validated' | 'filed'
}

export const previousFilings: PreviousFiling[] = [
  { period: 'FY2024/25', type: 'Micro-entity accounts (FRS 105)', filed: '2025-12-12', status: 'validated' },
  { period: 'FY2023/24', type: 'Micro-entity accounts (FRS 105)', filed: '2024-12-04', status: 'validated' },
  { period: 'FY2022/23', type: 'Micro-entity accounts (FRS 105)', filed: '2023-12-06', status: 'filed' },
]

// ---------- payroll ----------
export interface Employee {
  id: string
  name: string
  role: string
  basis: 'annual' | 'monthly'
  amount: number
  ytd: number
  ni: number // employer NI (YTD)
}

export const employees: Employee[] = [
  { id: 'e1', name: 'Ava Sharma', role: 'Head of Product', basis: 'annual', amount: 72000, ytd: 42000, ni: 3120 },
  { id: 'e2', name: 'Ben Okafor', role: 'Engineer', basis: 'annual', amount: 60000, ytd: 35000, ni: 2480 },
  { id: 'e3', name: 'Clem Wright', role: 'Accountant', basis: 'annual', amount: 48000, ytd: 28000, ni: 1820 },
  { id: 'e4', name: 'Dara Oyelaran', role: 'Office Manager', basis: 'monthly', amount: 2400, ytd: 16800, ni: 840 },
]

export const payroll = {
  nextRun: '2026-08-31',
  frequency: 'Monthly',
  grossPerRun: 17400,
  netPerRun: 12840,
  employerNi: 1610,
}

export interface PayrollRun {
  period: string
  gross: number
  tax: number
  ni: number
  net: number
  paid: string // ISO
  status: 'paid' | 'scheduled'
}

export const payrollHistory: PayrollRun[] = [
  { period: 'Jul 2026', gross: 17400, tax: 2460, ni: 2100, net: 12840, paid: '2026-07-31', status: 'paid' },
  { period: 'Jun 2026', gross: 17400, tax: 2435, ni: 2095, net: 12870, paid: '2026-06-30', status: 'paid' },
  { period: 'May 2026', gross: 17400, tax: 2410, ni: 2090, net: 12900, paid: '2026-05-29', status: 'paid' },
]

// ---------- per-company datasets ----------
// Static content a company has (transactions, filings, payroll…). Only the
// sample company is pre-seeded; user companies start empty until a backend
// exists. Data sources live separately in the DB (see src/db.ts) because they
// are mutable per company.
export interface NextFiling {
  period: string
  type: string
  start: string
  end: string
  due: string
  daysLeft: number
  progress: number
}

export interface PayrollMeta {
  nextRun: string
  frequency: string
  grossPerRun: number
  netPerRun: number
  employerNi: number
}

export interface CompanyData {
  transactions: Transaction[]
  summaries: MonthSummary[]
  nextFiling: NextFiling | null
  previousFilings: PreviousFiling[]
  employees: Employee[]
  payroll: PayrollMeta | null
  payrollHistory: PayrollRun[]
}

export const sampleCompanyData: CompanyData = {
  transactions,
  summaries,
  nextFiling,
  previousFilings,
  employees,
  payroll,
  payrollHistory,
}

export const emptyCompanyData: CompanyData = {
  transactions: [],
  summaries: [],
  nextFiling: null,
  previousFilings: [],
  employees: [],
  payroll: null,
  payrollHistory: [],
}

export function getCompanyData(companyId: string): CompanyData {
  return companyId === SAMPLE_COMPANY_ID ? sampleCompanyData : emptyCompanyData
}

// ---------- preferences ----------
export const preferences = {
  defaultStandard: 'FRS 105',
  reminderLeadDays: 30,
  emailReminders: true,
  productUpdates: false,
  autoFetchCompaniesHouse: true,
}
