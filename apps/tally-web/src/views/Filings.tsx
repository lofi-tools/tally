import { createEffect, createMemo, createSignal, For, onCleanup, Show, type JSX } from 'solid-js'
import { Badge, Button, Card, IconButton, Spinner, toaster } from '@tally/design-system'
import { ArrowUpRight, CheckCircle2, FileCheck2, RefreshCw, TriangleAlert } from 'lucide-solid'
import { css } from 'styled-system/css'
import { DEMO_COMPANY_ID, fmtDate, fmtMoney, type Company, type CompanyData } from '../mock_data'
import { EmptyState, StatusBadge } from '../components/Shared'
import { PageHeader } from '../components/layout'
import { session } from '../session'
import {
  ApiError,
  generateReportDocument,
  listFilings,
  listLedgers,
  refreshFilings,
  type BalanceSheet,
  type FilingsViewData,
  type Period,
  type PeriodFiling,
  type PreviousYearFigures,
} from '../api'

function daysTone(days: number) {
  if (days < 14) return 'red' as const
  if (days < 45) return 'amber' as const
  return 'green' as const
}

/** Days from today until an ISO date (negative = overdue). */
function daysLeft(iso: string): number {
  const target = new Date(`${iso}T00:00:00`).getTime()
  return Math.ceil((target - Date.now()) / 86_400_000)
}

/** `2025-04-01`/`2026-03-31` → `FY2025/26`. */
function fyLabel(p: { start: string; end: string }): string {
  const startYear = Number(p.start.slice(0, 4))
  const endYear = Number(p.end.slice(0, 4))
  return `FY${startYear}/${String(endYear).slice(2)}`
}

/** Map a filings/refresh failure to a banner message (same pattern as the
 *  AddCompanyDialog search error, §14.5). */
function filingErrorText(e: unknown): string {
  if (e instanceof ApiError) {
    switch (e.code) {
      case 'companies_house_key_missing':
        return "Companies House isn't configured — set COMPANIES_HOUSE_API_KEY and restart the API."
      case 'companies_house_rate_limited':
        return 'Companies House rate limit reached — try again shortly.'
      case 'companies_house_upstream':
        return 'Companies House is unavailable — try again.'
      default:
        return e.message // envelope messages are UI-safe by contract
    }
  }
  return "Can't reach the API — is it running?"
}

/** A readable description for a confirmed filing row. */
function filingTitle(f: PeriodFiling): string {
  if (f.description) return f.description
  switch (f.kind) {
    case 'accounts':
      return 'Accounts'
    case 'confirmation-statement':
      return 'Confirmation statement'
    case 'corporation-tax':
      return 'Corporation tax return'
    default:
      return f.form_type ?? 'Filing'
  }
}

/** Whole-pound figure, keeping the stored iXBRL sign (creditor lines negative). */
const fmtFigure = (n: number) => (n < 0 ? '−' : '') + fmtMoney(n)

/** The 11 stored figures in FRS 105 balance-sheet order (§6a); labels match
 *  the report's worksheet. */
function balanceSheetRows(f: PreviousYearFigures): { label: string; value: number }[] {
  return [
    { label: 'Fixed assets', value: f.fixed_assets },
    { label: 'Current assets', value: f.current_assets },
    { label: 'Prepayments and accrued income', value: f.prepayments_and_accrued_income },
    { label: 'Creditors: falling due within one year', value: f.creditors_within_1_year },
    { label: 'Net current assets', value: f.net_current_assets },
    { label: 'Total assets less liabilities', value: f.total_assets_less_liabilities },
    { label: 'Creditors: falling due after one year', value: f.creditors_after_1_year },
    { label: 'Provisions for liabilities', value: f.provisions_for_liabilities },
    { label: 'Accruals and deferred income', value: f.accruals_and_deferred_income },
    { label: 'Net assets', value: f.net_assets },
    { label: 'Capital and reserves', value: f.capital_and_reserves },
  ]
}

export function FilingsView(props: { company: Company; data: CompanyData }) {
  // Demo company keeps its seeded mock dataset (it is not a real CH
  // company); signed-in user companies are driven by the API; signed-out /
  // local companies keep today's empty behaviour (the mock view self-empties
  // on `emptyCompanyData`).
  const isDemo = () => props.company.id === DEMO_COMPANY_ID
  const signedIn = () => session().status === 'signed-in'
  return (
    <Show when={isDemo()} fallback={<Show when={signedIn()} fallback={<MockFilingsView company={props.company} data={props.data} />}>
      <RealFilingsView companyId={props.company.id} />
    </Show>}>
      <MockFilingsView company={props.company} data={props.data} />
    </Show>
  )
}

// ---------------------------------------------------------------------------
// Real view — two-pane: periods sub-nav (left) + selected period's filings
// ---------------------------------------------------------------------------

function RealFilingsView(props: { companyId: string }) {
  const [data, setData] = createSignal<FilingsViewData | null>(null)
  const [error, setError] = createSignal<string | null>(null)
  const [syncing, setSyncing] = createSignal(false)
  const [selectedEnd, setSelectedEnd] = createSignal<string | null>(null)
  let pollTimer: ReturnType<typeof setInterval> | undefined

  const stopPolling = () => {
    if (pollTimer) {
      clearInterval(pollTimer)
      pollTimer = undefined
    }
  }

  /** Poll the filings view every ~2 s until the fetch leaves pending/running
   *  (timeout ~30 s). The table keeps its last-known rows meanwhile. */
  const startPolling = () => {
    stopPolling()
    setSyncing(true)
    let elapsed = 0
    pollTimer = setInterval(async () => {
      elapsed += 2000
      if (elapsed > 30_000) {
        stopPolling()
        setSyncing(false)
        return
      }
      try {
        const fresh = await listFilings(props.companyId)
        setData(fresh)
        if (fresh.status.state !== 'pending' && fresh.status.state !== 'running') {
          stopPolling()
          setSyncing(false)
        }
      } catch {
        // Keep the last-known rows; the poll just ends.
        stopPolling()
        setSyncing(false)
      }
    }, 2000)
  }

  const load = async () => {
    try {
      const d = await listFilings(props.companyId)
      setData(d)
      setError(null)
      if (d.status.state === 'pending' || d.status.state === 'running') startPolling()
      else setSyncing(false)
    } catch (e) {
      setError(filingErrorText(e))
      setSyncing(false)
    }
  }

  // Load on mount; reload + reset when the company switches (polling stops,
  // banners clear, selection falls back to the new company's ongoing period).
  createEffect((prev: string | undefined) => {
    const id = props.companyId
    if (prev !== id) {
      setSelectedEnd(null)
      setError(null)
      setSyncing(false)
      stopPolling()
      void load()
    }
    return id
  })
  onCleanup(stopPolling)

  const onRefresh = async () => {
    if (syncing()) return
    setError(null)
    try {
      await refreshFilings(props.companyId) // 202 → poll; 200 no-op → poll anyway
      startPolling()
    } catch (e) {
      setError(filingErrorText(e))
      setSyncing(false)
    }
  }

  const periods = () => data()?.periods ?? []

  const status = () => data()?.status
  const isSyncing = () =>
    syncing() || status()?.state === 'pending' || status()?.state === 'running'
  const isFailed = () => status()?.state === 'failed' || !!error()

  /** Generate a draft accounts report for a period (blob → new tab). */
  const onPreview = async (period: Period) => {
    try {
      const ledgers = await listLedgers(props.companyId)
      const latest = ledgers[0] // listLedgers is newest-first
      if (!latest) {
        toaster.create({
          title: 'No ledger yet',
          description: 'Upload a ledger to prepare a draft accounts report.',
          type: 'info',
        })
        return
      }
      const url = await generateReportDocument(props.companyId, 'accounts', {
        ledger_id: latest.id,
        period: { start: period.start, end: period.end },
      })
      window.open(url, '_blank')
      setTimeout(() => URL.revokeObjectURL(url), 60_000)
    } catch (e) {
      toaster.create({
        title: 'Could not prepare the draft',
        description: filingErrorText(e),
        type: 'error',
      })
    }
  }

  return (
    <FilingsTwoPane
      periods={periods()}
      balanceSheets={data()?.balance_sheets ?? []}
      selectedEnd={selectedEnd()}
      onSelectEnd={setSelectedEnd}
      onPreview={onPreview}
      navEmpty={
        data() && !isSyncing() && !isFailed()
          ? 'No periods yet — they appear once Companies House has a record for this company.'
          : null
      }
      detailEmptyTitle="No filings yet"
      detailEmptyDescription="They appear here once Companies House has a record for this company."
      header={
        <>
          <PageHeader
            title="Filings"
            description="Companies House and HMRC deadlines, per financial period."
            actions={
              <IconButton
                variant="outline"
                aria-label="Refresh filing history from Companies House"
                disabled={isSyncing()}
                onClick={() => void onRefresh()}
              >
                {isSyncing() ? <Spinner class={css({ w: '3.5', h: '3.5' })} /> : <RefreshCw class={css({ w: '3.5', h: '3.5' })} />}
              </IconButton>
            }
          />

          {/* Syncing banner */}
          <Show when={isSyncing()}>
            <div class={css({ display: 'flex', alignItems: 'center', gap: '2.5', mb: '4', px: '3', py: '2', borderRadius: 'md', bg: 'bg.subtle', fontSize: 'sm', color: 'fg.muted' })}>
              <Spinner class={css({ w: '3.5', h: '3.5' })} />
              <span>Syncing filing history from Companies House…</span>
            </div>
          </Show>

          {/* Failed banner (with Retry) */}
          <Show when={isFailed() && !isSyncing()}>
            <div class={css({ display: 'flex', alignItems: 'center', gap: '2.5', mb: '4', px: '3', py: '2', borderRadius: 'md', bg: 'red.subtle.bg', color: 'red.subtle.fg', fontSize: 'sm' })}>
              <TriangleAlert class={css({ w: '3.5', h: '3.5', flexShrink: '0' })} />
              <span class={css({ flex: '1' })}>
                Filing history unavailable — last sync failed: {error() ?? status()?.last_error ?? 'unknown error'}
              </span>
              <Button size="xs" variant="outline" onClick={() => void onRefresh()} disabled={isSyncing()}>
                Retry
              </Button>
            </div>
          </Show>
        </>
      }
    />
  )
}

/** The two-pane shell shared by the API view and the demo view: a full-height
 *  periods sub-nav column (flush against the main sidebar — zero gap/margin,
 *  only the shared borderRight divider) + the main-right pane holding the
 *  heading/banners and the selected period's detail. */
function FilingsTwoPane(props: {
  periods: Period[]
  balanceSheets: BalanceSheet[]
  selectedEnd: string | null
  onSelectEnd: (end: string) => void
  onPreview: (period: Period) => void
  /** Shown in the empty nav column; null = show nothing (e.g. still loading). */
  navEmpty: string | null
  /** Rendered at the top of the main-right pane (heading, banners, actions). */
  header: JSX.Element
  detailEmptyTitle: string
  detailEmptyDescription: string
}) {
  const selected = createMemo(() => {
    const all = props.periods
    const byEnd = all.find((p) => p.end === props.selectedEnd)
    if (byEnd) return byEnd
    const ongoing = all.find((p) => p.status === 'ongoing')
    return ongoing ?? all[0] ?? null
  })
  /** The stored balance sheet for the selected period (matched by period end). */
  const selectedSheet = createMemo(
    () => props.balanceSheets.find((b) => b.period_end === selected()?.end) ?? null,
  )

  return (
    <div class={css({ h: 'full', display: 'flex', alignItems: 'stretch', minH: '0' })}>
      {/* Left — periods sub-nav: its own column, not merged into the main
          sidebar's sub-menus. */}
      <nav
        aria-label="Financial periods"
        class={css({
          w: '56',
          flexShrink: '0',
          borderRight: '1px solid {colors.border}',
          overflowY: 'auto',
          p: '2',
          display: 'flex',
          flexDirection: 'column',
          gap: '0.5',
          bg: 'bg.canvas',
        })}
      >
        <Show when={props.periods.length > 0} fallback={props.navEmpty ? <p class={css({ px: '2.5', py: '2', fontSize: 'sm', color: 'fg.subtle' })}>{props.navEmpty}</p> : null}>
          <For each={props.periods}>
            {(p) => (
              <PeriodNavRow
                period={p}
                selected={selected()?.end === p.end}
                onClick={() => props.onSelectEnd(p.end)}
              />
            )}
          </For>
        </Show>
      </nav>

      {/* Right — heading + banners + the selected period's filings, with the
          same padding as the other views (maxW 60rem, mx auto, p 5/8). */}
      <div class={css({ flex: '1', minW: '0', overflowY: 'auto' })}>
        <div class={css({ maxW: '60rem', mx: 'auto', p: { base: '5', md: '8' } })}>
          {props.header}
          <Show
            when={selected()}
            fallback={
              <Card.Root>
                <EmptyState title={props.detailEmptyTitle} description={props.detailEmptyDescription} />
              </Card.Root>
            }
          >
            {(period) => <PeriodDetail period={period()} balanceSheet={selectedSheet()} onPreview={props.onPreview} />}
          </Show>
        </div>
      </div>
    </div>
  )
}

/** `FY2024/25` → `[2024-04-01, 2025-03-31]` (or `[null, null]` if unparsable). */
function fyRange(label: string): [string | null, string | null] {
  const m = /^FY(\d{4})\/(\d{2})$/.exec(label)
  if (!m) return [null, null]
  const endYear = Number(m[2]) + 2000
  return [`${endYear - 1}-04-01`, `${endYear}-03-31`]
}

/** Add months to an ISO date, clamping the day to the target month. */
function isoAddMonths(iso: string, months: number): string {
  const [y, m, d] = iso.slice(0, 10).split('-').map(Number)
  const total = y * 12 + (m - 1) + months
  const ty = Math.floor(total / 12)
  const tm = (total % 12) + 1
  const dim = new Date(Date.UTC(ty, tm, 0)).getUTCDate()
  return `${ty}-${String(tm).padStart(2, '0')}-${String(Math.min(d, dim)).padStart(2, '0')}`
}

/** One row of the periods sub-nav: FY label + range, trailing status
 *  indicator (green tick / yellow "!" pending / Ongoing badge). */
function PeriodNavRow(props: { period: Period; selected: boolean; onClick: () => void }) {
  const p = () => props.period
  return (
    <button
      type="button"
      onClick={props.onClick}
      aria-current={props.selected ? 'true' : undefined}
      class={css({
        position: 'relative',
        w: 'full',
        display: 'flex',
        alignItems: 'center',
        gap: '2',
        px: '2.5',
        py: '2',
        borderRadius: 'md',
        bg: props.selected ? 'bg.subtle' : 'transparent',
        border: 'none',
        cursor: 'pointer',
        textAlign: 'left',
        color: props.selected ? 'fg.default' : 'fg.muted',
        opacity: props.selected ? '1' : '0.82',
        _hover: { bg: 'bg.subtle', color: 'fg.default', opacity: '1' },
        transition: 'background-color 120ms ease, color 120ms ease, opacity 120ms ease',
      })}
    >
      <Show when={props.selected}>
        <span
          aria-hidden="true"
          class={css({
            position: 'absolute',
            left: '0',
            top: '50%',
            transform: 'translateY(-50%)',
            w: '0.5',
            h: '4',
            borderRadius: 'full',
            bg: 'brown.9',
          })}
        />
      </Show>
      <span class={css({ flex: '1', minW: '0' })}>
        <span class={css({ display: 'block', fontSize: 'sm', fontWeight: '600' })}>{fyLabel(p())}</span>
        <span class={css({ display: 'block', fontSize: 'xs', color: 'fg.subtle' })}>
          {fmtDate(p().start)} – {fmtDate(p().end)}
        </span>
      </span>
      <Show
        when={p().status === 'filed'}
        fallback={
          <Show
            when={p().status === 'pending'}
            fallback={
              <Badge variant="outline" class={css({ flexShrink: '0', fontSize: '10px', px: '1.5', py: '0' })}>
                Ongoing
              </Badge>
            }
          >
            <span class={css({ display: 'inline-flex', alignItems: 'center', gap: '1', flexShrink: '0', color: 'amber.subtle.fg' })}>
              <TriangleAlert class={css({ w: '3.5', h: '3.5' })} />
              <span class={css({ fontSize: 'xs', fontWeight: '600' })}>pending</span>
            </span>
          </Show>
        }
      >
        <CheckCircle2 class={css({ w: '4', h: '4', color: 'green.plain.fg', flexShrink: '0' })} />
      </Show>
    </button>
  )
}

/** The right pane: the selected period's header + its filings (confirmed by
 *  CH/HMRC and pending, not-sent yet). */
function PeriodDetail(props: {
  period: Period
  /** The CH-sourced balance sheet for this period, when one has been parsed. */
  balanceSheet?: BalanceSheet | null
  onPreview: (period: Period) => void
}) {
  const p = () => props.period
  const confirmed = createMemo(() => p().filings.filter((f) => f.state === 'confirmed'))
  const notSent = createMemo(() => p().filings.filter((f) => f.state === 'not-sent'))

  return (
    <>
      {/* Header: label + status badge + deadlines (pending/ongoing). */}
      <div class={css({ mb: '4' })}>
        <div class={css({ display: 'flex', alignItems: 'center', gap: '2.5', flexWrap: 'wrap' })}>
          <h2 class={css({ textStyle: 'lg', fontWeight: '700' })}>{fyLabel(p())}</h2>
          <Show when={p().status === 'filed'} fallback={
            <StatusBadge status={p().status === 'ongoing' ? 'ongoing' : 'pending'} tone={p().status === 'ongoing' ? 'blue' : 'amber'} />
          }>
            <StatusBadge status="filed" tone="green" label="Filed" />
          </Show>
        </div>
        <div class={css({ fontSize: 'sm', color: 'fg.muted', mt: '1' })}>
          Period {fmtDate(p().start)} – {fmtDate(p().end)}
        </div>
        <Show when={p().status === 'pending' || p().status === 'ongoing'}>
          <Show when={p().due}>
            {(due) => {
              const chTone = daysTone(daysLeft(due().ch))
              const hmrcTone = daysTone(daysLeft(due().hmrc))
              const toneColor = (tone: 'red' | 'amber' | 'green') =>
                tone === 'red' ? 'red.plain.fg' : tone === 'amber' ? 'amber.subtle.fg' : 'fg.default'
              return (
                <div class={css({ display: 'flex', gap: '4', flexWrap: 'wrap', mt: '2', fontSize: 'sm' })}>
                  <span>
                    Accounts due{' '}
                    <span class={css({ fontWeight: '600', color: toneColor(chTone) })}>{fmtDate(due().ch)}</span>
                  </span>
                  <span>
                    CT600 due{' '}
                    <span class={css({ fontWeight: '600', color: toneColor(hmrcTone) })}>{fmtDate(due().hmrc)}</span>
                  </span>
                </div>
              )
            }}
          </Show>
        </Show>
      </div>

      {/* Previous-year figures from the stored (CH-sourced) balance sheet. */}
      <Show when={props.balanceSheet}>
        {(bs) => (
          <Card.Root class={css({ mb: '4' })}>
            <div class={css({ px: '4', pt: '3.5', pb: '1' })}>
              <div class={css({ fontSize: 'sm', fontWeight: '600' })}>Previous year figures</div>
              <div class={css({ fontSize: 'xs', color: 'fg.subtle', mt: '0.5' })}>
                From the balance sheet filed at Companies House · year ended {fmtDate(bs().period_end)}
                <Show when={bs().filed_on}> · filed {fmtDate(bs().filed_on!)}</Show>
              </div>
            </div>
            <For each={balanceSheetRows(bs().figures)}>
              {(row) => (
                <div class={css({ display: 'flex', alignItems: 'center', gap: '3', px: '4', py: '1.5', borderTop: '1px solid {colors.border}' })}>
                  <span class={css({ flex: '1', minW: '0', fontSize: 'sm', color: 'fg.muted' })}>{row.label}</span>
                  <span class={css({ fontSize: 'sm', fontWeight: '600', fontVariantNumeric: 'tabular-nums' })}>{fmtFigure(row.value)}</span>
                </div>
              )}
            </For>
          </Card.Root>
        )}
      </Show>

      {/* Confirmed filings (filed at CH / HMRC). */}
      <Show when={confirmed().length > 0}>
        <Card.Root class={css({ mb: '4' })}>
          <div class={css({ px: '4', pt: '3.5', pb: '1', fontSize: 'sm', fontWeight: '600' })}>Filed</div>
          <For each={confirmed()}>
            {(f, i) => (
              <div class={css({
                display: 'flex',
                alignItems: 'center',
                gap: '3',
                px: '4',
                py: '3',
                borderTop: i() === 0 ? 'none' : '1px solid {colors.border}',
              })}>
                <CheckCircle2 class={css({ w: '4', h: '4', color: 'green.plain.fg', flexShrink: '0' })} />
                <span class={css({ flex: '1', minW: '0' })}>
                  <span class={css({ display: 'block', fontSize: 'sm', fontWeight: '500', truncate: true })}>
                    {filingTitle(f)}
                  </span>
                  <Show when={f.form_type}>
                    <span class={css({ display: 'block', fontSize: 'xs', color: 'fg.subtle', fontFamily: 'mono' })}>
                      {f.form_type}
                    </span>
                  </Show>
                </span>
                <Show when={f.filed_on}>
                  <span class={css({ fontSize: 'sm', color: 'fg.muted' })}>{fmtDate(f.filed_on!)}</span>
                </Show>
                <StatusBadge status="confirmed" tone="green" label="Filed at CH" />
              </div>
            )}
          </For>
        </Card.Root>
      </Show>

      {/* Pending filings, not sent yet (expected accounts + CT600). */}
      <Show when={notSent().length > 0}>
        <Card.Root>
          <div class={css({ px: '4', pt: '3.5', pb: '1', fontSize: 'sm', fontWeight: '600' })}>To file</div>
          <For each={notSent()}>
            {(f, i) => (
              <div class={css({
                display: 'flex',
                alignItems: 'center',
                gap: '3',
                px: '4',
                py: '3',
                borderTop: i() === 0 ? 'none' : '1px solid {colors.border}',
              })}>
                <TriangleAlert class={css({ w: '4', h: '4', color: 'amber.subtle.fg', flexShrink: '0' })} />
                <span class={css({ flex: '1', minW: '0' })}>
                  <span class={css({ display: 'block', fontSize: 'sm', fontWeight: '500' })}>
                    {f.kind === 'accounts' ? 'Accounts (FRS 105)' : f.kind === 'corporation-tax' ? 'CT600' : 'Filing'}
                  </span>
                </span>
                <StatusBadge status="pending" tone="amber" />
                <Button
                  size="2xs"
                  variant="outline"
                  onClick={() => void props.onPreview(p())}
                >
                  <FileCheck2 class={css({ w: '3', h: '3' })} /> Prepare / Preview
                </Button>
                <Button
                  size="2xs"
                  onClick={() => toaster.create({ title: 'File now (mock)', description: 'Submitting to Companies House lands with the backend.', type: 'info' })}
                >
                  <ArrowUpRight class={css({ w: '3', h: '3' })} /> File
                </Button>
              </div>
            )}
          </For>
        </Card.Root>
      </Show>

      <Show when={confirmed().length === 0 && notSent().length === 0}>
        <Card.Root>
          <EmptyState title="Nothing for this period" description="No filings are recorded or expected for this period." />
        </Card.Root>
      </Show>
    </>
  )
}

// ---------------------------------------------------------------------------
// Mock view — demo company (seeded dataset) / local companies (empty)
// ---------------------------------------------------------------------------

function MockFilingsView(props: { company: Company; data: CompanyData }) {
  const [selectedEnd, setSelectedEnd] = createSignal<string | null>(null)

  /** Derive a `Period[]` (newest first) from the seeded mock dataset so the
   *  demo / local companies render the same two-pane layout as the API view. */
  const periods = createMemo<Period[]>(() => {
    const d = props.data
    const list: Period[] = []
    const nf = d.nextFiling
    if (nf) {
      list.push({
        start: nf.start,
        end: nf.end,
        status: 'ongoing',
        due: { ch: nf.due, hmrc: isoAddMonths(nf.end, 12) },
        filings: [
          { kind: 'accounts', state: 'not-sent', description: nf.type },
          { kind: 'corporation-tax', state: 'not-sent' },
        ],
      })
    }
    for (const f of d.previousFilings) {
      const [start, end] = fyRange(f.period)
      if (!start || !end) continue
      list.push({
        start,
        end,
        status: 'filed',
        due: null,
        filings: [{ kind: 'accounts', state: 'confirmed', description: f.type, filed_on: f.filed }],
      })
    }
    return list
  })

  return (
    <FilingsTwoPane
      periods={periods()}
      balanceSheets={[]}
      selectedEnd={selectedEnd()}
      onSelectEnd={setSelectedEnd}
      onPreview={() =>
        toaster.create({ title: 'Preview draft (mock)', description: 'iXBRL rendering lands with the backend.', type: 'info' })
      }
      navEmpty="No periods yet."
      detailEmptyTitle="No filings yet"
      detailEmptyDescription="Prepare your first accounts once transactions start flowing into the books."
      header={
        <PageHeader
          title="Filings"
          description={`Companies House and HMRC deadlines for ${props.company.name}.`}
          company={props.company}
        />
      }
    />
  )
}
