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

// ---------- fiscal year ----------
// The current FY window (FY2025/26 — the demo book's period). The chart of
// accounts can show balances for the current FY or all-time.
export const CURRENT_FY_LABEL = 'FY2025/26'
export const FY_START = '2025-04-01'
export const FY_END = '2026-03-31'

/** True when a transaction falls inside the current fiscal year. */
export function inCurrentFy(t: { date: string }): boolean {
  return t.date >= FY_START && t.date <= FY_END
}

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
// never persisted and carries the full seeded dataset (see demoCompanyData).
export const DEMO_COMPANY_ID = 'demo'

export const demoCompany: Company = {
  id: DEMO_COMPANY_ID,
  name: 'Demo Co Ltd',
  companyNumber: '00000000',
  utr: '—',
  sic: '—',
  address: '—',
  standard: 'FRS 105',
}

// ---------- transactions ----------
// One row per split of the basic-1 GnuCash book (see the chart of accounts
// below). Amounts follow the app sign convention (income and expenses
// positive, liabilities and equity negative); dates are shifted into
// FY2025/26 so the demo looks current.
export interface Transaction {
  id: string
  date: string // ISO
  description: string
  account: string
  source: 'Starling' | 'Barclays' | 'Manual'
  amount: number // GBP; positive = income / expense (debit-normal), negative = liability / equity / contra
  status: 'cleared' | 'pending' | 'matched'
}

export const transactions: Transaction[] = [
  { id: 't01', date: "2025-04-01", description: "Shareholders initial stock purchase", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: 1250.0, status: 'cleared' },
  { id: 't02', date: "2025-04-01", description: "Shareholders initial stock purchase", account: "Equity/Shareholdings/Preference Shares", source: "Manual", amount: -1250.0, status: 'cleared' },
  { id: 't03', date: "2025-08-01", description: "Internet", account: "Expenses/VAT Purchases/Telecoms", source: "Manual", amount: 136.71, status: 'cleared' },
  { id: 't04', date: "2025-08-01", description: "Internet", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: -136.71, status: 'cleared' },
  { id: 't05', date: "2025-08-16", description: "Employee 1", account: "Expenses/Emoluments/Employees/Net Salaries", source: "Manual", amount: 1127.0, status: 'cleared' },
  { id: 't06', date: "2025-08-16", description: "Employee 1", account: "Expenses/Emoluments/Employer Pension Contribution", source: "Manual", amount: 100.0, status: 'cleared' },
  { id: 't07', date: "2025-08-16", description: "Employee 1", account: "Expenses/Emoluments/Employees/Income Tax", source: "Manual", amount: 120.0, status: 'cleared' },
  { id: 't08', date: "2025-08-16", description: "Employee 1", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: -1347.0, status: 'cleared' },
  { id: 't09', date: "2025-08-20", description: "Stock purchase", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: 3750.0, status: 'cleared' },
  { id: 't10', date: "2025-08-20", description: "Stock purchase", account: "Equity/Shareholdings/Ordinary Shares", source: "Manual", amount: -3750.0, status: 'cleared' },
  { id: 't11', date: "2025-08-20", description: "Bank charges", account: "Expenses/VAT Purchases/Bank Charges", source: "Manual", amount: 101.78, status: 'cleared' },
  { id: 't12', date: "2025-08-20", description: "Bank charges", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: -101.78, status: 'cleared' },
  { id: 't13', date: "2025-08-20", description: "Printer cartridges", account: "Expenses/VAT Purchases/Office", source: "Manual", amount: 438.21, status: 'cleared' },
  { id: 't14', date: "2025-08-20", description: "Printer cartridges", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: -438.21, status: 'cleared' },
  { id: 't15', date: "2025-08-20", description: "Travel/accom", account: "Expenses/VAT Purchases/Travel/Accom", source: "Manual", amount: 67.81, status: 'cleared' },
  { id: 't16', date: "2025-08-20", description: "Travel/accom", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: -67.81, status: 'cleared' },
  { id: 't17', date: "2025-08-20", description: "Sundries", account: "Expenses/VAT Purchases/Sundries", source: "Manual", amount: 36.61, status: 'cleared' },
  { id: 't18', date: "2025-08-20", description: "Sundries", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: -36.61, status: 'cleared' },
  { id: 't19', date: "2025-08-29", description: "Computer equipment", account: "Assets/Capital Equipment/Computer Equipment", source: "Manual", amount: 827.41, status: 'cleared' },
  { id: 't20', date: "2025-08-29", description: "Computer equipment", account: "Assets/VAT/Input", source: "Manual", amount: 165.48, status: 'cleared' },
  { id: 't21', date: "2025-08-29", description: "Computer equipment", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: -992.89, status: 'cleared' },
  { id: 't22', date: "2025-09-15", description: "Interest paid", account: "Expenses/Interest Paid", source: "Manual", amount: 42.0, status: 'cleared' },
  { id: 't23', date: "2025-09-15", description: "Interest paid", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: -42.0, status: 'cleared' },
  { id: 't24', date: "2025-09-25", description: "Interest received", account: "Income/Interest", source: "Manual", amount: 142.0, status: 'cleared' },
  { id: 't25', date: "2025-09-25", description: "Interest received", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: 142.0, status: 'cleared' },
  { id: 't26', date: "2025-09-30", description: "Contract 1b", account: "Assets/Owed To Us", source: "Manual", amount: 4094.4, status: 'cleared' },
  { id: 't27', date: "2025-09-30", description: "Contract 1b", account: "Income/Sales/UK", source: "Manual", amount: 3412.0, status: 'cleared' },
  { id: 't28', date: "2025-09-30", description: "Contract 1b", account: "Liabilities/VAT/Output/Sales", source: "Manual", amount: -682.4, status: 'cleared' },
  { id: 't29', date: "2025-10-18", description: "Contract 1b payment", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: 4094.4, status: 'cleared' },
  { id: 't30', date: "2025-10-18", description: "Contract 1b payment", account: "Assets/Owed To Us", source: "Manual", amount: -4094.4, status: 'cleared' },
  { id: 't31', date: "2025-10-20", description: "Contract 1c", account: "Assets/Owed To Us", source: "Manual", amount: 4094.4, status: 'cleared' },
  { id: 't32', date: "2025-10-20", description: "Contract 1c", account: "Income/Sales/UK", source: "Manual", amount: 3412.0, status: 'cleared' },
  { id: 't33', date: "2025-10-20", description: "Contract 1c", account: "Liabilities/VAT/Output/Sales", source: "Manual", amount: -682.4, status: 'cleared' },
  { id: 't34', date: "2025-11-13", description: "Deprec", account: "Expenses/Depreciation", source: "Manual", amount: 194.31, status: 'cleared' },
  { id: 't35', date: "2025-11-13", description: "Deprec", account: "Assets/Capital Equipment/Depreciation", source: "Manual", amount: -194.31, status: 'cleared' },
  { id: 't36', date: "2025-11-13", description: "Corp tax", account: "Equity/Corporation Tax/Corporation Tax", source: "Manual", amount: 1655.57, status: 'cleared' },
  { id: 't37', date: "2025-11-13", description: "Corp tax", account: "Liabilities/Owed Corporation Tax", source: "Manual", amount: -1655.57, status: 'cleared' },
  { id: 't38', date: "2025-11-18", description: "Contract 1 payment", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: 5054.4, status: 'cleared' },
  { id: 't39', date: "2025-11-18", description: "Contract 1 payment", account: "Assets/Owed To Us", source: "Manual", amount: -5054.4, status: 'cleared' },
  { id: 't40', date: "2025-11-27", description: "Internet", account: "Expenses/VAT Purchases/Telecoms", source: "Manual", amount: 958.0, status: 'cleared' },
  { id: 't41', date: "2025-11-27", description: "Internet", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: -958.0, status: 'cleared' },
  { id: 't42', date: "2025-11-27", description: "Pay accountant", account: "Expenses/VAT Purchases/Accountant", source: "Manual", amount: 1487.0, status: 'cleared' },
  { id: 't43', date: "2025-11-27", description: "Pay accountant", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: -1487.0, status: 'cleared' },
  { id: 't44', date: "2025-11-27", description: "Bank charges", account: "Expenses/VAT Purchases/Bank Charges", source: "Manual", amount: 482.76, status: 'cleared' },
  { id: 't45', date: "2025-11-27", description: "Bank charges", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: -482.76, status: 'cleared' },
  { id: 't46', date: "2025-11-27", description: "Printer cartridges", account: "Expenses/VAT Purchases/Office", source: "Manual", amount: 67.34, status: 'cleared' },
  { id: 't47', date: "2025-11-27", description: "Printer cartridges", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: -67.34, status: 'cleared' },
  { id: 't48', date: "2025-11-27", description: "Travel/accom", account: "Expenses/VAT Purchases/Travel/Accom", source: "Manual", amount: 622.0, status: 'cleared' },
  { id: 't49', date: "2025-11-27", description: "Travel/accom", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: -622.0, status: 'cleared' },
  { id: 't50', date: "2025-11-27", description: "Sundries", account: "Expenses/VAT Purchases/Sundries", source: "Manual", amount: 82.41, status: 'cleared' },
  { id: 't51', date: "2025-11-27", description: "Sundries", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: -82.41, status: 'cleared' },
  { id: 't52', date: "2025-11-27", description: "Subscriptions", account: "Expenses/VAT Purchases/Subscriptions", source: "Manual", amount: 242.0, status: 'cleared' },
  { id: 't53', date: "2025-11-27", description: "Subscriptions", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: -242.0, status: 'cleared' },
  { id: 't54', date: "2025-12-03", description: "Contract 2", account: "Assets/Owed To Us", source: "Manual", amount: 3744.0, status: 'cleared' },
  { id: 't55', date: "2025-12-03", description: "Contract 2", account: "Income/Sales/UK", source: "Manual", amount: 3120.0, status: 'cleared' },
  { id: 't56', date: "2025-12-03", description: "Contract 2", account: "Liabilities/VAT/Output/Sales", source: "Manual", amount: -624.0, status: 'cleared' },
  { id: 't57', date: "2025-12-04", description: "Employee 1", account: "Expenses/Emoluments/Employees/Net Salaries", source: "Manual", amount: 4477.31, status: 'cleared' },
  { id: 't58', date: "2025-12-04", description: "Employee 1", account: "Expenses/Emoluments/Employer Pension Contribution", source: "Manual", amount: 91.12, status: 'cleared' },
  { id: 't59', date: "2025-12-04", description: "Employee 1", account: "Expenses/Emoluments/Employees/Income Tax", source: "Manual", amount: 421.12, status: 'cleared' },
  { id: 't60', date: "2025-12-04", description: "Employee 1", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: -4989.55, status: 'cleared' },
  { id: 't61', date: "2025-12-09", description: "Computer equipment", account: "Assets/Capital Equipment/Computer Equipment", source: "Manual", amount: 591.12, status: 'cleared' },
  { id: 't62', date: "2025-12-09", description: "Computer equipment", account: "Assets/VAT/Input", source: "Manual", amount: 118.22, status: 'cleared' },
  { id: 't63', date: "2025-12-09", description: "Computer equipment", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: -709.34, status: 'cleared' },
  { id: 't64', date: "2025-12-13", description: "Buy-in", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: 3000.0, status: 'cleared' },
  { id: 't65', date: "2025-12-13", description: "Buy-in", account: "Equity/Shareholdings/Preference Shares", source: "Manual", amount: -3000.0, status: 'cleared' },
  { id: 't66', date: "2025-12-13", description: "Contract 1c payment", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: 4094.4, status: 'cleared' },
  { id: 't67', date: "2025-12-13", description: "Contract 1c payment", account: "Assets/Owed To Us", source: "Manual", amount: -4094.4, status: 'cleared' },
  { id: 't68', date: "2025-12-20", description: "Pay accountant", account: "Expenses/VAT Purchases/Accountant", source: "Manual", amount: 482.0, status: 'cleared' },
  { id: 't69', date: "2025-12-20", description: "Pay accountant", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: -482.0, status: 'cleared' },
  { id: 't70', date: "2026-01-02", description: "Pay Corp tax", account: "Liabilities/Owed Corporation Tax", source: "Manual", amount: 1655.57, status: 'cleared' },
  { id: 't71', date: "2026-01-02", description: "Pay Corp tax", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: -1655.57, status: 'cleared' },
  { id: 't72', date: "2026-01-11", description: "Interest paid", account: "Expenses/Interest Paid", source: "Manual", amount: 67.0, status: 'cleared' },
  { id: 't73', date: "2026-01-11", description: "Interest paid", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: -67.0, status: 'cleared' },
  { id: 't74', date: "2026-01-21", description: "Interest received", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: 98.0, status: 'cleared' },
  { id: 't75', date: "2026-01-21", description: "Interest received", account: "Income/Interest", source: "Manual", amount: 98.0, status: 'cleared' },
  { id: 't76', date: "2026-02-10", description: "Contract 2 payment", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: 3744.0, status: 'cleared' },
  { id: 't77', date: "2026-02-10", description: "Contract 2 payment", account: "Assets/Owed To Us", source: "Manual", amount: -3744.0, status: 'cleared' },
  { id: 't78', date: "2026-02-10", description: "Contract 2b", account: "Assets/Owed To Us", source: "Manual", amount: 4800.14, status: 'cleared' },
  { id: 't79', date: "2026-02-10", description: "Contract 2b", account: "Income/Sales/UK", source: "Manual", amount: 4000.12, status: 'cleared' },
  { id: 't80', date: "2026-02-10", description: "Contract 2b", account: "Liabilities/VAT/Output/Sales", source: "Manual", amount: -800.02, status: 'cleared' },
  { id: 't81', date: "2026-02-15", description: "Contract 2c", account: "Assets/Owed To Us", source: "Manual", amount: 4800.0, status: 'cleared' },
  { id: 't82', date: "2026-02-15", description: "Contract 2c", account: "Income/Sales/UK", source: "Manual", amount: 4000.0, status: 'cleared' },
  { id: 't83', date: "2026-02-15", description: "Contract 2c", account: "Liabilities/VAT/Output/Sales", source: "Manual", amount: -800.0, status: 'cleared' },
  { id: 't84', date: "2026-03-02", description: "Divident payout", account: "Equity/Dividends/Shareholder Dividends 1", source: "Manual", amount: 125.0, status: 'cleared' },
  { id: 't85', date: "2026-03-02", description: "Divident payout", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: -125.0, status: 'cleared' },
  { id: 't86', date: "2026-03-02", description: "Contract 2b payment", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: 4800.14, status: 'cleared' },
  { id: 't87', date: "2026-03-02", description: "Contract 2b payment", account: "Assets/Owed To Us", source: "Manual", amount: -4800.14, status: 'cleared' },
  { id: 't88', date: "2026-03-12", description: "Deprec", account: "Expenses/Depreciation", source: "Manual", amount: 291.48, status: 'cleared' },
  { id: 't89', date: "2026-03-12", description: "Deprec", account: "Assets/Capital Equipment/Depreciation", source: "Manual", amount: -291.48, status: 'cleared' },
  { id: 't90', date: "2026-03-12", description: "Corp tax", account: "Equity/Corporation Tax/Corporation Tax", source: "Manual", amount: 123.5, status: 'cleared' },
  { id: 't91', date: "2026-03-12", description: "Corp tax", account: "Liabilities/Owed Corporation Tax", source: "Manual", amount: -123.5, status: 'cleared' },
  { id: 't92', date: "2026-03-12", description: "R&D Enhanced Expenditure for project Iguana - £357.69 @ 130%", account: "Expenses/R&D Enhanced Expenditure/Expenditure/Project Iguana/Staffing Costs", source: "Manual", amount: 465.2, status: 'cleared' },
  { id: 't93', date: "2026-03-12", description: "R&D Enhanced Expenditure for project Iguana - £357.69 @ 130%", account: "Expenses/R&D Enhanced Expenditure/Relief Claimed", source: "Manual", amount: -465.2, status: 'cleared' },
  { id: 't94', date: "2026-03-31", description: "Contract 2c payment", account: "Assets/Bank Accounts/Current Account", source: "Starling", amount: 4800.0, status: 'cleared' },
  { id: 't95', date: "2026-03-31", description: "Contract 2c payment", account: "Assets/Owed To Us", source: "Manual", amount: -4800.0, status: 'cleared' },
]
// ---------- chart of accounts ----------
// The whole GnuCash book from libs/ixbrl/example_data/basic-1/input.gnucash,
// kept as a tree in memory and re-rooted under the app's five top-level groups
// by account type. Balances derive from the transactions list (keyed by full
// path), so the tree, the account registers and the YTD cards always agree.
export interface AccountNode {
  name: string
  type: string
  children: AccountNode[]
}

export const chartOfAccounts: AccountNode[] = [
  { name: "Assets", type: "ASSETS", children: [
    { name: "Bank Accounts", type: "BANK", children: [
      { name: "Current Account", type: "BANK", children: [] },
      { name: "Reserve Account", type: "BANK", children: [] },
    ] },
    { name: "Capital Equipment", type: "ASSET", children: [
      { name: "Computer Equipment", type: "ASSET", children: [] },
      { name: "EU Reverse VAT Purchase", type: "ASSET", children: [] },
      { name: "Depreciation", type: "ASSET", children: [] },
    ] },
    { name: "Other", type: "ASSET", children: [] },
    { name: "Owed To Us", type: "ASSET", children: [] },
    { name: "VAT Repayments Due", type: "ASSET", children: [] },
    { name: "Accounts Receivable", type: "RECEIVABLE", children: [] },
    { name: "Cash", type: "CASH", children: [] },
    { name: "VAT", type: "WRAPPER", children: [
      { name: "Input", type: "ASSET", children: [] },
    ] },
    { name: "Settlement", type: "WRAPPER", children: [
      { name: "Input", type: "ASSET", children: [] },
    ] },
  ] },
  { name: "Liabilities", type: "LIABILITIES", children: [
    { name: "Owed Corporation Tax", type: "LIABILITY", children: [] },
    { name: "Owed Fees", type: "LIABILITY", children: [] },
    { name: "Owed Tax/NI", type: "LIABILITY", children: [] },
    { name: "Other", type: "LIABILITY", children: [] },
    { name: "Credit Cards", type: "LIABILITY", children: [] },
    { name: "VAT", type: "LIABILITY", children: [
      { name: "Output", type: "LIABILITY", children: [
        { name: "EU", type: "LIABILITY", children: [] },
        { name: "Sales", type: "LIABILITY", children: [] },
      ] },
      { name: "Settlement", type: "LIABILITY", children: [
        { name: "Output", type: "LIABILITY", children: [] },
      ] },
    ] },
    { name: "Accounts Payable", type: "PAYABLE", children: [] },
  ] },
  { name: "Equity", type: "EQUITY", children: [
    { name: "Director's Loan", type: "EQUITY", children: [] },
    { name: "Dividends", type: "EQUITY", children: [
      { name: "Director's Dividends 1", type: "EQUITY", children: [] },
      { name: "Director's Dividends 2", type: "EQUITY", children: [] },
      { name: "Shareholder Dividends 1", type: "EQUITY", children: [] },
    ] },
    { name: "Opening Balances", type: "EQUITY", children: [] },
    { name: "Grants", type: "EQUITY", children: [] },
    { name: "Shareholdings", type: "EQUITY", children: [
      { name: "Ordinary Shares", type: "EQUITY", children: [] },
      { name: "Preference Shares", type: "EQUITY", children: [] },
    ] },
    { name: "Corporation Tax", type: "EQUITY", children: [
      { name: "Corporation Tax", type: "EQUITY", children: [] },
    ] },
  ] },
  { name: "Income", type: "INCOME", children: [
    { name: "Interest", type: "INCOME", children: [] },
    { name: "Misc", type: "INCOME", children: [] },
    { name: "Sales", type: "INCOME", children: [
      { name: "UK", type: "INCOME", children: [] },
      { name: "EU", type: "INCOME", children: [
        { name: "Goods", type: "INCOME", children: [] },
        { name: "Services", type: "INCOME", children: [] },
      ] },
      { name: "World", type: "INCOME", children: [] },
    ] },
  ] },
  { name: "Expenses", type: "EXPENSES", children: [
    { name: "Depreciation", type: "EXPENSE", children: [] },
    { name: "Emoluments", type: "EXPENSE", children: [
      { name: "Director's Fees", type: "EXPENSE", children: [] },
      { name: "Employer's NICs", type: "EXPENSE", children: [] },
      { name: "Employees", type: "EXPENSE", children: [
        { name: "Net Salaries", type: "EXPENSE", children: [] },
        { name: "Stakeholder Contributions", type: "EXPENSE", children: [] },
        { name: "NICs", type: "EXPENSE", children: [] },
        { name: "Income Tax", type: "EXPENSE", children: [] },
      ] },
      { name: "Employer Pension Contribution", type: "EXPENSE", children: [] },
    ] },
    { name: "Other non-VAT expenses", type: "EXPENSE", children: [] },
    { name: "VAT Purchases", type: "EXPENSE", children: [
      { name: "Accountant", type: "EXPENSE", children: [] },
      { name: "Bank Charges", type: "EXPENSE", children: [] },
      { name: "EU Reverse VAT", type: "EXPENSE", children: [] },
      { name: "Office", type: "EXPENSE", children: [] },
      { name: "Telecoms", type: "EXPENSE", children: [] },
      { name: "Software", type: "EXPENSE", children: [] },
      { name: "Subscriptions", type: "EXPENSE", children: [] },
      { name: "Sundries", type: "EXPENSE", children: [] },
      { name: "Travel/Accom", type: "EXPENSE", children: [] },
    ] },
    { name: "Interest Paid", type: "EXPENSE", children: [] },
    { name: "R&D Enhanced Expenditure", type: "EXPENSE", children: [
      { name: "Relief Claimed", type: "EXPENSE", children: [] },
      { name: "Expenditure", type: "EXPENSE", children: [
        { name: "Project Iguana", type: "EXPENSE", children: [
          { name: "Staffing Costs", type: "EXPENSE", children: [] },
          { name: "Software/Consumables", type: "EXPENSE", children: [] },
          { name: "External Workers", type: "EXPENSE", children: [] },
        ] },
      ] },
    ] },
  ] },
]

// Path of every account ("Assets/Bank Accounts/Current Account") and the leaf
// paths in chart order — built once from the tree at module load.
const nodePaths = new Map<AccountNode, string>()
const leafPaths: string[] = []
const indexNode = (node: AccountNode, prefix: string): void => {
  const path = prefix ? `${prefix}/${node.name}` : node.name
  nodePaths.set(node, path)
  if (node.children.length === 0) leafPaths.push(path)
  for (const child of node.children) indexNode(child, path)
}
for (const group of chartOfAccounts) indexNode(group, '')

/** Absolute path of a tree node. */
export function accountPathOf(node: AccountNode): string {
  return nodePaths.get(node) ?? node.name
}

/** Leaf name of an account path, e.g. "Current Account". */
export function accountLabel(path: string): string {
  return path.split('/').pop() ?? path
}

/** Path minus the top-level group, e.g. "Bank Accounts › Current Account". */
export function accountBreadcrumb(path: string): string {
  return path.split('/').slice(1).join(' › ')
}

export function transactionsFor(account: string): Transaction[] {
  return transactions.filter((t) => t.account === account)
}

/** Balance of one account: the sum of its transactions (app sign convention). */
export function accountBalance(account: string): number {
  return transactionsFor(account).reduce((s, t) => s + t.amount, 0)
}

/** Rolled-up total of a tree node — the sum of every descendant leaf. */
export function groupBalance(node: AccountNode): number {
  if (node.children.length === 0) return accountBalance(accountPathOf(node))
  return node.children.reduce((s, child) => s + groupBalance(child), 0)
}

/** Leaf account paths in chart order (drives the Transactions filter dropdown). */
export function chartAccountNames(): string[] {
  return leafPaths
}
// ---------- summaries ----------
export interface MonthSummary {
  month: string
  income: number
  expenses: number
  vat: number
}

export const summaries: MonthSummary[] = (() => {
  // Derived from the transactions so the YTD cards reconcile with the chart.
  // Income = sums on Income/ accounts; expenses = sums on Expenses/ accounts
  // (positive, so credits like the R&D relief reduce the spend);
  // VAT = postings on the VAT output / input accounts.
  const byMonth = new Map<string, { income: number; expenses: number; vat: number }>()
  for (const t of transactions) {
    const key = t.date.slice(0, 7)
    const m = byMonth.get(key) ?? { income: 0, expenses: 0, vat: 0 }
    if (t.account.startsWith('Income/')) m.income += t.amount
    if (t.account.startsWith('Expenses/')) m.expenses += t.amount
    if (t.account.includes('/VAT/') || t.account.includes('/Settlement/')) m.vat += t.amount
    byMonth.set(key, m)
  }
  return [...byMonth.entries()]
    .filter(([, v]) => v.income !== 0 || v.expenses !== 0 || v.vat !== 0)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([key, v]) => ({ month: fmtMonth(`${key}-01`), ...v }))
})()

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
// demo company is pre-seeded; user companies start empty until a backend
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

export const demoCompanyData: CompanyData = {
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
  return companyId === DEMO_COMPANY_ID ? demoCompanyData : emptyCompanyData
}

// ---------- preferences ----------
export const preferences = {
  defaultStandard: 'FRS 105',
  reminderLeadDays: 30,
  emailReminders: true,
  productUpdates: false,
  autoFetchCompaniesHouse: true,
}
