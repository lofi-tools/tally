import { For } from 'solid-js'
import { Badge, Button, Card } from '@tally/design-system'
import { css } from 'styled-system/css'
import { Plus } from '../components/icons'
import { Section } from './Section'

const visuals = ['solid', 'subtle', 'outline', 'ghost'] as const
const tones = ['primary', 'danger', 'neutral'] as const
const sizes = ['xs', 'sm', 'md', 'lg'] as const
const badgeTones = ['primary', 'neutral', 'success', 'warning', 'danger', 'info'] as const
const badgeVariants = ['subtle', 'solid', 'outline'] as const

function Row(props: { label: string; children: import('solid-js').JSX.Element }) {
  return (
    <div class={css({ display: 'flex', flexDirection: 'column', gap: '2.5' })}>
      <span class={css({ fontSize: 'xs', fontWeight: '600', color: 'fgSubtle', textTransform: 'uppercase', letterSpacing: '0.06em' })}>
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
              <code class={css({ fontFamily: 'mono', fontSize: 'xs', bg: 'surfaceMuted', px: '1', py: '0.5', borderRadius: 'sm' })}>
                visual × tone × size
              </code>{' '}
              from the <code class={css({ fontFamily: 'mono', fontSize: 'xs', bg: 'surfaceMuted', px: '1', py: '0.5', borderRadius: 'sm' })}>button</code> recipe.
            </Card.Description>
          </Card.Header>
          <Card.Body class={css({ display: 'flex', flexDirection: 'column', gap: '6' })}>
            <Row label="Visual variants">
              <For each={visuals}>{(v) => <Button visual={v}>{v}</Button>}</For>
            </Row>
            <Row label="Tones">
              <For each={tones}>{(t) => <Button tone={t}>Action</Button>}</For>
            </Row>
            <Row label="Sizes">
              <For each={sizes}>{(s) => <Button size={s}>Button</Button>}</For>
            </Row>
            <Row label="With icon & states">
              <Button>
                <Plus /> New entry
              </Button>
              <Button visual="outline" disabled>
                Disabled
              </Button>
              <Button visual="ghost" disabled>
                Disabled
              </Button>
            </Row>
          </Card.Body>
        </Card.Root>

        <Card.Root>
          <Card.Header>
            <Card.Title>Badges</Card.Title>
            <Card.Description>
              Status chips from the <code class={css({ fontFamily: 'mono', fontSize: 'xs', bg: 'surfaceMuted', px: '1', py: '0.5', borderRadius: 'sm' })}>badge</code> recipe.
            </Card.Description>
          </Card.Header>
          <Card.Body class={css({ display: 'flex', flexDirection: 'column', gap: '6' })}>
            <Row label="Subtle (default)">
              <For each={badgeTones}>{(t) => <Badge tone={t}>{t}</Badge>}</For>
            </Row>
            <For each={badgeVariants.slice(1)}>
              {(v) => (
                <Row label={v}>
                  <For each={badgeTones}>{(t) => <Badge tone={t} variant={v}>{t}</Badge>}</For>
                </Row>
              )}
            </For>
          </Card.Body>
        </Card.Root>
      </div>
    </Section>
  )
}
