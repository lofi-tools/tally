import { Button, IconButton } from '@tally/design-system'
import { Plus, X } from 'lucide-solid'
import { css } from 'styled-system/css'

/**
 * Shown until the user has a real company of their own: explains that the
 * data on screen is the sample company and funnels them into adding theirs.
 */
export function SampleBanner(props: { onAddCompany: () => void; onDismiss: () => void }) {
  return (
    // Full-width strip: background and content both span the main column,
    // with compact padding. Warm tint draws the eye without shouting.
    <div class={css({ w: 'full', bg: 'brown.a2', borderBottom: '1px solid {colors.border}' })}>
      <div
        class={css({
          display: 'flex',
          alignItems: 'center',
          gap: '3',
          px: { base: '4', md: '5' },
          py: '2',
        })}
      >
        <span class={css({ flex: '1', minW: '0', textStyle: 'sm', color: 'fg.muted' })}>
          Sample data — <span class={css({ color: 'fg.default', fontWeight: '600' })}>add your company</span> to get started.
        </span>
        <Button size="sm" onClick={props.onAddCompany}>
          <Plus class={css({ w: '3.5', h: '3.5' })} /> Add company
        </Button>
        <IconButton size="sm" variant="plain" aria-label="Dismiss" onClick={props.onDismiss}>
          <X class={css({ w: '3.5', h: '3.5' })} />
        </IconButton>
      </div>
    </div>
  )
}
