import { Badge, Button, Card } from '@tally/design-system'
import { css } from 'styled-system/css'

// Blank starter for the Tally web app.
//
// The design system is fully wired up (panda.config.ts + the styled-system
// alias in vite.config.ts) — replace this placeholder with the real UI.

export function App() {
  return (
    <div
      class={css({
        minH: '100dvh',
        bg: 'canvas',
        color: 'fg.default',
        fontFamily: 'sans',
        display: 'grid',
        placeItems: 'center',
        p: '4',
      })}
    >
      <Card.Root class={css({ w: 'full', maxW: 'sm' })}>
        <Card.Header>
          <Badge variant="subtle">Tally</Badge>
          <Card.Title>UK company accounts &amp; CT600 filing</Card.Title>
          <Card.Description>
            The real app starts here — the design system is already wired up.
          </Card.Description>
        </Card.Header>
        <Card.Body>
          <p class={css({ fontSize: 'sm', color: 'fg.muted' })}>
            Build the accounts workspace, filing flow, and settings on top of
            <code class={css({ fontFamily: 'mono', fontSize: 'xs' })}> @tally/design-system</code>.
          </p>
        </Card.Body>
        <Card.Footer>
          <Button disabled>Coming soon</Button>
        </Card.Footer>
      </Card.Root>
    </div>
  )
}
