import { For } from 'solid-js'
import { Badge, Button, Card } from '@tally/design-system'
import { css } from 'styled-system/css'
import { colorPaletteSeeds } from '../seeds'
import { Plus } from '../components/icons'
import { Section } from './Section'

const visuals = ['solid', 'surface', 'subtle', 'outline', 'plain'] as const
const sizes = ['2xs', 'xs', 'sm', 'md', 'lg'] as const
const palettes = ['brown', 'gray', 'green', 'blue', 'amber', 'red'] as const
const badgeVariants = ['subtle', 'surface', 'solid', 'outline'] as const

function Row(props: { label: string; children: import('solid-js').JSX.Element }) {
  return (
    <div class={css({ display: 'flex', flexDirection: 'column', gap: '2.5' })}>
      <span class={css({ fontSize: 'xs', fontWeight: '600', color: 'fg.subtle', textTransform: 'uppercase', letterSpacing: '0.06em' })}>
        {props.label}
      </span>
      <div class={css({ display: 'flex', flexWrap: 'wrap', gap: '2.5', alignItems: 'center' })}>{props.children}</div>
    </div>
  )
}

export function Buttons() {
  return (
    <Section
      id="components"
      eyebrow="Components"
      title="Buttons & badges"
      description="Variant recipes live in the theme. Components pick them up — new sizes or tones are theme changes, not component rewrites."
    >
      <div class={css({ display: 'grid', gap: '6', lg: { gridTemplateColumns: 'repeat(2, 1fr)' } })}>
        <Card.Root>
          <Card.Header>
            <Card.Title>Buttons</Card.Title>
            <Card.Description>
              <code class={css({ fontFamily: 'mono', fontSize: 'xs', bg: 'bg.subtle', px: '1', py: '0.5', borderRadius: 'sm' })}>
                variant × size
              </code>{' '}
              from the <code class={css({ fontFamily: 'mono', fontSize: 'xs', bg: 'bg.subtle', px: '1', py: '0.5', borderRadius: 'sm' })}>button</code> recipe — solid buttons resolve to the brown accent with white text.
            </Card.Description>
          </Card.Header>
          <Card.Body class={css({ display: 'flex', flexDirection: 'column', gap: '6' })}>
            <Row label="Variants">
              <For each={visuals}>{(v) => <Button variant={v}>{v}</Button>}</For>
            </Row>
            <Row label="Sizes">
              <For each={sizes}>{(s) => <Button size={s}>Button</Button>}</For>
            </Row>
            <Row label="Color palettes">
              <For each={palettes}>{(p) => <Button colorPalette={p} variant="subtle" class={colorPaletteSeeds[p]}>{p}</Button>}</For>
            </Row>
            <Row label="With icon & states">
              <Button>
                <Plus /> New entry
              </Button>
              <Button variant="outline" disabled>
                Disabled
              </Button>
              <Button variant="plain" disabled>
                Disabled
              </Button>
              <Button loading loadingText="Filing…">
                File accounts
              </Button>
            </Row>
          </Card.Body>
        </Card.Root>

        <Card.Root>
          <Card.Header>
            <Card.Title>Badges</Card.Title>
            <Card.Description>
              Status chips from the <code class={css({ fontFamily: 'mono', fontSize: 'xs', bg: 'bg.subtle', px: '1', py: '0.5', borderRadius: 'sm' })}>badge</code> recipe — <code class={css({ fontFamily: 'mono', fontSize: 'xs', bg: 'bg.subtle', px: '1', py: '0.5', borderRadius: 'sm' })}>colorPalette</code> swaps the accent.
            </Card.Description>
          </Card.Header>
          <Card.Body class={css({ display: 'flex', flexDirection: 'column', gap: '6' })}>
            <For each={badgeVariants}>
              {(v) => (
                <Row label={v}>
                  <For each={palettes}>{(p) => <Badge colorPalette={p} variant={v} class={colorPaletteSeeds[p]}>{p}</Badge>}</For>
                </Row>
              )}
            </For>
          </Card.Body>
        </Card.Root>
      </div>
    </Section>
  )
}
