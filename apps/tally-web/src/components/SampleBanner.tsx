import { Button, IconButton } from '@tally/design-system'
import { Plus, X } from 'lucide-solid'
import { css } from 'styled-system/css'

/**
 * Shown until the user has a real company of their own: explains that the
 * data on screen is the sample company and funnels them into adding theirs.
 */
export function SampleBanner(props: { onAddCompany: () => void; onDismiss: () => void }) {
  return (
    <div
      class={css({
        display: 'flex',
        alignItems: 'center',
        gap: '3',
        mb: '5',
        px: '4',
        py: '2.5',
        borderRadius: 'md',
        border: '1px solid {colors.border}',
        bg: 'gray.surface.bg',
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
  )
}
