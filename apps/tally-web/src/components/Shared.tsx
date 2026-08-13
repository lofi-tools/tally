import { For, type JSX } from 'solid-js'
import { Badge, Card, IconButton } from '@tally/design-system'
import { FileSpreadsheet, Landmark, PenLine, RefreshCw } from 'lucide-solid'
import { css } from 'styled-system/css'
import { type DataSource } from '../mock_data'

/**
 * View header: title (+ optional badge) and description on the left, action
 * buttons on the right. `badge` renders inline after the title — views pass
 * a <DemoBadge/> whenever the demo company is on screen.
 */
export function PageHeader(props: { title: string; description?: string; badge?: JSX.Element; actions?: JSX.Element }) {
  return (
    <div
      class={css({
        display: 'flex',
        alignItems: 'flex-start',
        justifyContent: 'space-between',
        gap: '4',
        flexWrap: 'wrap',
        mb: '6',
      })}
    >
      <div class={css({ minW: '0' })}>
        <div class={css({ display: 'flex', alignItems: 'baseline', gap: '2' })}>
          <h1 class={css({ textStyle: '2xl', fontWeight: '800', letterSpacing: '-0.02em' })}>{props.title}</h1>
          {props.badge}
        </div>
        {props.description && (
          <p class={css({ textStyle: 'sm', color: 'fg.muted', mt: '1', maxW: '40rem' })}>{props.description}</p>
        )}
      </div>
      {props.actions && <div class={css({ display: 'flex', gap: '2', alignItems: 'center', flexWrap: 'wrap' })}>{props.actions}</div>}
    </div>
  )
}

/** "Demo" marker — labels the demo company wherever its data is visible (spec §6.1). */
export function DemoBadge() {
  return (
    <Badge variant="outline" class={css({ fontSize: 'xs', flexShrink: '0' })}>
      Demo
    </Badge>
  )
}

/** Small stat card for dashboard-style headers. */
export function StatCard(props: { label: string; value: string; hint?: string; tone?: 'default' | 'good' | 'bad' }) {
  return (
    <Card.Root class={css({ p: '4' })}>
      <div
        class={css({
          fontSize: 'xs',
          fontWeight: '600',
          color: 'fg.muted',
          textTransform: 'uppercase',
          letterSpacing: '0.06em',
        })}
      >
        {props.label}
      </div>
      <div
        class={css({
          textStyle: '2xl',
          fontWeight: '700',
          mt: '1.5',
          fontVariantNumeric: 'tabular-nums',
          color: props.tone === 'good' ? 'green.plain.fg' : props.tone === 'bad' ? 'red.plain.fg' : 'fg.default',
        })}
      >
        {props.value}
      </div>
      {props.hint && <div class={css({ fontSize: 'xs', color: 'fg.subtle', mt: '1' })}>{props.hint}</div>}
    </Card.Root>
  )
}

const STATUS_TONES: Record<string, 'green' | 'amber' | 'red' | 'blue' | 'gray'> = {
  cleared: 'green',
  connected: 'green',
  validated: 'green',
  paid: 'green',
  matched: 'blue',
  filed: 'blue',
  pending: 'amber',
  'needs-auth': 'amber',
  due: 'amber',
  scheduled: 'blue',
  draft: 'gray',
  overdue: 'red',
  failed: 'red',
}

/** Badge whose tone follows a known status word (cleared, connected, pending…). */
export function StatusBadge(props: { status: string; label?: string; tone?: 'green' | 'amber' | 'red' | 'blue' | 'gray' }) {
  const tone = () => props.tone ?? STATUS_TONES[props.status] ?? 'gray'
  const label = () =>
    props.label ??
    props.status.replace(/-/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase())
  return (
    <Badge colorPalette={tone()} variant="subtle" class={css({ fontSize: 'xs' })}>
      {label()}
    </Badge>
  )
}

const kindIcon = {
  bank: Landmark,
  csv: FileSpreadsheet,
  manual: PenLine,
}

const kindLabel: Record<DataSource['kind'], string> = {
  bank: 'Bank',
  csv: 'CSV import',
  manual: 'Manual',
}

/**
 * Rows of connected/pending data sources, used by both the Accounts view
 * ("Data sources" tab) and the Integrations view. `onSync` adds a refresh
 * button per row; `footer` renders under the list.
 */
export function DataSourceRows(props: {
  sources: () => DataSource[]
  onSync?: (ds: DataSource) => void
  footer?: JSX.Element
}) {
  return (
    <Card.Root>
      <For each={props.sources()}>
        {(ds, i) => {
          const Icon = kindIcon[ds.kind]
          return (
            <div
              class={css({
                display: 'flex',
                alignItems: 'center',
                gap: '3',
                px: '4',
                py: '3.5',
                borderTop: i() === 0 ? 'none' : '1px solid {colors.border}',
                _hover: { bg: 'bg.subtle' },
                transition: 'background-color 120ms ease',
              })}
            >
              <span
                class={css({
                  h: '9',
                  w: '9',
                  borderRadius: 'md',
                  bg: 'bg.subtle',
                  border: '1px solid {colors.border}',
                  display: 'grid',
                  placeItems: 'center',
                  color: 'fg.muted',
                  flexShrink: '0',
                })}
              >
                <Icon class={css({ w: '4', h: '4' })} />
              </span>
              <span class={css({ minW: '0', flex: '1' })}>
                <span class={css({ display: 'block', fontSize: 'sm', fontWeight: '600', color: 'fg.default', truncate: true })}>
                  {ds.name}
                </span>
                <span class={css({ display: 'block', fontSize: 'xs', color: 'fg.muted', mt: '0.5' })}>
                  {kindLabel[ds.kind]}
                  {ds.accountCount > 0 ? ` · ${ds.accountCount} account${ds.accountCount > 1 ? 's' : ''}` : ''} · synced {ds.lastSync}
                </span>
              </span>
              <StatusBadge status={ds.status} />
              {props.onSync && (
                <IconButton
                  size="sm"
                  variant="subtle"
                  aria-label={`Sync ${ds.name}`}
                  onClick={() => props.onSync?.(ds)}
                  class={css({ color: 'fg.muted', _hover: { color: 'fg.default' } })}
                >
                  <RefreshCw class={css({ w: '3.5', h: '3.5' })} />
                </IconButton>
              )}
            </div>
          )
        }}
      </For>
      {props.footer && (
        <div class={css({ px: '4', py: '3', borderTop: '1px solid {colors.border}' })}>{props.footer}</div>
      )}
    </Card.Root>
  )
}

/** Compact numeric table cell (monospace + tabular figures). */
export const numCell = css({ fontFamily: 'mono', fontSize: 'sm', fontVariantNumeric: 'tabular-nums' })

/** Empty-state placeholder for empty lists and first-run guidance. */
export function EmptyState(props: {
  title: string
  description: string
  icon?: JSX.Element
  action?: JSX.Element
}) {
  return (
    <div class={css({ py: '12', display: 'flex', flexDirection: 'column', alignItems: 'center', textAlign: 'center', gap: '1' })}>
      {props.icon && <span class={css({ color: 'fg.subtle', mb: '2' })}>{props.icon}</span>}
      <span class={css({ textStyle: 'md', fontWeight: '600', color: 'fg.default' })}>{props.title}</span>
      <span class={css({ textStyle: 'sm', color: 'fg.muted', maxW: '24rem' })}>{props.description}</span>
      {props.action && <div class={css({ mt: '4', display: 'flex', gap: '2', flexWrap: 'wrap', justifyContent: 'center' })}>{props.action}</div>}
    </div>
  )
}
