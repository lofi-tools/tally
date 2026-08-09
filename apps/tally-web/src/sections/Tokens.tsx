import { For } from 'solid-js'
import { Card } from '@tally/design-system'
import { tokens } from '@tally/design-system/theme'
import { css } from 'styled-system/css'
import { Section } from './Section'

const kebab = (s: string) => s.replace(/([a-z0-9])([A-Z])/g, '$1-$2').toLowerCase()
/** CSS variable for a semantic color token, e.g. `var(--colors-surface-muted)`. */
const varOf = (name: string) => `var(--colors-${kebab(name)})`

const core = [
  ['bg', 'Page background'],
  ['surface', 'Cards & sheets'],
  ['surfaceMuted', 'Hover / secondary surfaces'],
  ['fg', 'Primary text'],
  ['fgMuted', 'Secondary text'],
  ['fgSubtle', 'Hint / placeholder text'],
  ['border', 'Default borders'],
  ['borderStrong', 'Strong borders'],
  ['accent', 'Primary action'],
  ['accentMuted', 'Accent soft fill'],
] as const

const feedback = [
  ['info', 'Info'],
  ['success', 'Success'],
  ['warning', 'Warning'],
  ['danger', 'Danger'],
] as const

function Swatch(props: { name: string; label: string }) {
  return (
    <div class={css({ display: 'flex', alignItems: 'center', gap: '3' })}>
      <span
        class={css({ h: '10', w: '10', borderRadius: 'lg', border: '1px solid', borderColor: 'border', flexShrink: '0' })}
        style={{ background: varOf(props.name) }}
      />
      <span class={css({ minW: '0' })}>
        <span class={css({ display: 'block', fontSize: 'xs', fontWeight: '600', color: 'fg', fontFamily: 'mono' })}>
          {props.name}
        </span>
        <span class={css({ display: 'block', fontSize: 'xs', color: 'fgMuted' })}>{props.label}</span>
      </span>
    </div>
  )
}

function ScaleCard(props: { title: string; scale: Record<string, { value: string }> }) {
  return (
    <Card.Root>
      <Card.Header>
        <Card.Title>{props.title}</Card.Title>
        <Card.Description>Raw palette, defined once in the theme.</Card.Description>
      </Card.Header>
      <Card.Body>
        <div class={css({ display: 'grid', gridTemplateColumns: 'repeat(5, 1fr)', gap: '2', sm: { gridTemplateColumns: 'repeat(10, 1fr)' } })}>
          <For each={Object.entries(props.scale)}>
            {([shade, t]) => (
              <div class={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
                <span
                  class={css({ h: '12', borderRadius: 'lg', border: '1px solid', borderColor: 'border', display: 'block' })}
                  style={{ background: t.value }}
                />
                <span class={css({ fontSize: '10px', color: 'fgSubtle', textAlign: 'center', fontFamily: 'mono' })}>
                  {shade}
                </span>
              </div>
            )}
          </For>
        </div>
      </Card.Body>
    </Card.Root>
  )
}

const typography = [
  { style: 'display', sample: 'Display — the annual accounts' },
  { style: 'h1', sample: 'Heading 1' },
  { style: 'h2', sample: 'Heading 2' },
  { style: 'h3', sample: 'Heading 3' },
  { style: 'h4', sample: 'Heading 4' },
] as const

export function Tokens() {
  return (
    <Section
      id="tokens"
      eyebrow="Design tokens"
      title="Configure the whole theme from a few tokens"
      description="Raw scales (brand, neutral, fonts) plus semantic roles with dark-mode variants. Change a value here and every component re-themes."
    >
      <div class={css({ display: 'grid', gap: '6', lg: { gridTemplateColumns: 'repeat(2, 1fr)' } })}>
        <Card.Root class={css({ lg: { gridColumn: 'span 2' } })}>
          <Card.Header>
            <Card.Title>Semantic colors</Card.Title>
            <Card.Description>
              Roles, not raw values — each maps to light and dark palettes via the{' '}
              <code class={css({ fontFamily: 'mono', fontSize: 'xs', bg: 'surfaceMuted', px: '1', py: '0.5', borderRadius: 'sm' })}>
                _dark
              </code>{' '}
              condition. Toggle dark mode in the header.
            </Card.Description>
          </Card.Header>
          <Card.Body class={css({ display: 'grid', gap: '3', gridTemplateColumns: '1fr', sm: { gridTemplateColumns: 'repeat(2, 1fr)' } })}>
            <For each={core}>{(item) => <Swatch name={item[0]} label={item[1]} />}</For>
          </Card.Body>
          <Card.Footer class={css({ borderTop: '1px solid', borderColor: 'border', pt: '4', mt: '0' })}>
            <div class={css({ display: 'grid', gap: '3', gridTemplateColumns: 'repeat(2, 1fr)', w: 'full', sm: { gridTemplateColumns: 'repeat(4, 1fr)' } })}>
              <For each={feedback}>
                {(item) => (
                  <div class={css({ display: 'flex', alignItems: 'center', gap: '2' })}>
                    <span
                      class={css({ h: '6', w: '6', borderRadius: 'md', border: '1px solid', borderColor: 'border', flexShrink: '0' })}
                      style={{ background: varOf(item[0]) }}
                    />
                    <span class={css({ fontSize: 'xs', fontWeight: '600', color: 'fg', fontFamily: 'mono' })}>{item[0]}</span>
                  </div>
                )}
              </For>
            </div>
          </Card.Footer>
        </Card.Root>

        <ScaleCard title="Brand" scale={tokens.colors.brand} />
        <ScaleCard title="Neutral" scale={tokens.colors.neutral} />

        <Card.Root class={css({ lg: { gridColumn: 'span 2' } })}>
          <Card.Header>
            <Card.Title>Typography</Card.Title>
            <Card.Description>Text styles from the theme — <code class={css({ fontFamily: 'mono', fontSize: 'xs', bg: 'surfaceMuted', px: '1', py: '0.5', borderRadius: 'sm' })}>textStyle</code> roles.</Card.Description>
          </Card.Header>
          <Card.Body>
            <div class={css({ display: 'flex', flexDirection: 'column', gap: '5' })}>
              <For each={typography}>
                {(t) => (
                  <div class={css({ display: 'flex', alignItems: 'baseline', gap: '4', justifyContent: 'space-between' })}>
                    <span class={css({ fontSize: 'xs', color: 'fgSubtle', fontFamily: 'mono', flexShrink: '0', w: '16' })}>{t.style}</span>
                    <span class={css({ textStyle: t.style })}>{t.sample}</span>
                  </div>
                )}
              </For>
              <div class={css({ display: 'flex', alignItems: 'baseline', gap: '4', justifyContent: 'space-between' })}>
                <span class={css({ fontSize: 'xs', color: 'fgSubtle', fontFamily: 'mono', flexShrink: '0', w: '16' })}>body</span>
                <p class={css({ textStyle: 'body', color: 'fgMuted' })}>
                  Body copy carries the day-to-day text of the product — ledgers, filing status and everything in between.
                </p>
              </div>
              <div class={css({ display: 'flex', alignItems: 'baseline', gap: '4', justifyContent: 'space-between' })}>
                <span class={css({ fontSize: 'xs', color: 'fgSubtle', fontFamily: 'mono', flexShrink: '0', w: '16' })}>caption</span>
                <span class={css({ textStyle: 'caption', color: 'fgMuted' })}>Captions annotate fields and footnotes.</span>
              </div>
              <div class={css({ display: 'flex', alignItems: 'baseline', gap: '4', justifyContent: 'space-between' })}>
                <span class={css({ fontSize: 'xs', color: 'fgSubtle', fontFamily: 'mono', flexShrink: '0', w: '16' })}>code</span>
                <code class={css({ textStyle: 'code', bg: 'surfaceMuted', px: '3', py: '1.5', borderRadius: 'lg', color: 'fg' })}>
                  tally ct600 --book input.gnucash
                </code>
              </div>
            </div>
          </Card.Body>
        </Card.Root>
      </div>
    </Section>
  )
}
