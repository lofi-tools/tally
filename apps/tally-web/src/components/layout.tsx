import type { JSX } from 'solid-js'
import { Badge } from '@tally/design-system'
import { css } from 'styled-system/css'
import { DEMO_COMPANY_ID, type Company } from '../mock_data'

/**
 * View header: title (+ demo badge) and description on the left, action
 * buttons on the right. Pass `company` and the header labels the demo entity
 * with a <DemoBadge/> whenever it's on screen (spec §6.1).
 */
export function PageHeader(props: {
  title: string
  description?: string
  /** Current company — renders a demo badge when it's the demo entity. */
  company?: Pick<Company, 'id'>
  actions?: JSX.Element
}) {
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
        <div class={css({ display: 'flex', alignItems: 'center', gap: '2' })}>
          <h1 class={css({ textStyle: '2xl', fontWeight: '800', letterSpacing: '-0.02em' })}>{props.title}</h1>
          {props.company?.id === DEMO_COMPANY_ID && <DemoBadge />}
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
