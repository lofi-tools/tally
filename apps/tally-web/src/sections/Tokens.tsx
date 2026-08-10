import { For } from 'solid-js'
import { Badge, Card } from '@tally/design-system'
import { tokens } from '@tally/design-system/theme'
import { css } from 'styled-system/css'
import { colorPaletteSeeds } from '../seeds'
import { Section } from './Section'

const kebab = (s: string) => s.replace(/([a-z0-9])([A-Z])/g, '$1-$2').replace(/\./g, '-').toLowerCase()
/** CSS variable for a semantic color token, e.g. `var(--colors-fg-muted)`. */
const varOf = (name: string) => `var(--colors-${kebab(name)})`

const core = [
  ['canvas', 'Page background'],
  ['fg.default', 'Primary text'],
  ['fg.muted', 'Secondary text'],
  ['fg.subtle', 'Hint / placeholder text'],
  ['border', 'Default borders'],
  ['bg.subtle', 'Soft fills & hovers'],
  ['error', 'Error / invalid'],
] as const

const accent = [
  ['brown.solid.bg', 'Solid buttons'],
  ['brown.solid.fg', 'On-solid text'],
  ['brown.subtle.bg', 'Soft accents'],
  ['brown.surface.bg', 'Surface accents'],
  ['brown.outline.border', 'Outlines'],
  ['brown.plain.fg', 'Plain text accent'],
] as const

const feedback = [
  ['green', 'Success'],
  ['blue', 'Info'],
  ['amber', 'Warning'],
  ['red', 'Danger'],
] as const

const typography = [
  { style: '7xl', sample: 'Display' },
  { style: '6xl', sample: 'Headline' },
  { style: '5xl', sample: 'Hero title' },
  { style: '4xl', sample: 'Section heading' },
  { style: '3xl', sample: 'Sub-heading' },
  { style: '2xl', sample: 'Card heading' },
  { style: 'xl', sample: 'Emphasised body' },
  { style: 'lg', sample: 'Large body' },
  { style: 'md', sample: 'Body' },
  { style: 'sm', sample: 'Small body' },
  { style: 'xs', sample: 'Caption' },
] as const

// cssgen seeds: the grid below applies `textStyle` from data (dynamic), which
// cssgen can't extract — precompute the utility class for every role instead.
const textStyleClasses = {
  '7xl': css({ textStyle: '7xl' }),
  '6xl': css({ textStyle: '6xl' }),
  '5xl': css({ textStyle: '5xl' }),
  '4xl': css({ textStyle: '4xl' }),
  '3xl': css({ textStyle: '3xl' }),
  '2xl': css({ textStyle: '2xl' }),
  xl: css({ textStyle: 'xl' }),
  lg: css({ textStyle: 'lg' }),
  md: css({ textStyle: 'md' }),
  sm: css({ textStyle: 'sm' }),
  xs: css({ textStyle: 'xs' }),
} as const

function Swatch(props: { name: string; label: string }) {
  return (
    <div class={css({ display: 'flex', alignItems: 'center', gap: '3' })}>
      <span
        class={css({ h: '10', w: '10', borderRadius: 'lg', border: '1px solid', borderColor: 'border', flexShrink: '0' })}
        style={{ background: varOf(props.name) }}
      />
      <span class={css({ minW: '0' })}>
        <span class={css({ display: 'block', fontSize: 'xs', fontWeight: '600', color: 'fg.default', fontFamily: 'mono' })}>
          {props.name}
        </span>
        <span class={css({ display: 'block', fontSize: 'xs', color: 'fg.muted' })}>{props.label}</span>
      </span>
    </div>
  )
}

function ScaleCard(props: {
  title: string
  scale: Record<string, any>
  description?: string
}) {
  const entries = () =>
    Object.entries(props.scale).filter(([k]) => /^\d+$/.test(k))
  return (
    <Card.Root>
      <Card.Header>
        <Card.Title>{props.title}</Card.Title>
        <Card.Description>{props.description ?? 'Raw palette, defined once in the theme.'}</Card.Description>
      </Card.Header>
      <Card.Body>
        <div class={css({ display: 'grid', gridTemplateColumns: 'repeat(6, 1fr)', gap: '2', sm: { gridTemplateColumns: 'repeat(12, 1fr)' } })}>
          <For each={entries()}>
            {([shade, t]) => (
              <div class={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
                <span
                  class={css({ h: '12', borderRadius: 'lg', border: '1px solid', borderColor: 'border', display: 'block' })}
                  style={{ background: t.value._dark }}
                />
                <span class={css({ fontSize: '10px', color: 'fg.subtle', textAlign: 'center', fontFamily: 'mono' })}>
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

export function Tokens() {
  return (
    <Section
      id="tokens"
      eyebrow="Design tokens"
      title="Park UI, themed once"
      description="Semantic roles with dark-mode variants, raw palettes (brown accent, sand neutral), and Outfit text styles. Change a value in the theme and every component re-themes."
    >
      <div class={css({ display: 'grid', gap: '6', lg: { gridTemplateColumns: 'repeat(2, 1fr)' } })}>
        <Card.Root class={css({ lg: { gridColumn: 'span 2' } })}>
          <Card.Header>
            <Card.Title>Semantic colors</Card.Title>
            <Card.Description>
              Roles, not raw values — each maps to light and dark palettes via the{' '}
              <code class={css({ fontFamily: 'mono', fontSize: 'xs', bg: 'bg.subtle', px: '1', py: '0.5', borderRadius: 'sm' })}>
                _dark
              </code>{' '}
              condition. Toggle dark mode in the header.
            </Card.Description>
          </Card.Header>
          <Card.Body class={css({ display: 'grid', gap: '3', gridTemplateColumns: '1fr', sm: { gridTemplateColumns: 'repeat(2, 1fr)' } })}>
            <For each={core}>{(item) => <Swatch name={item[0]} label={item[1]} />}</For>
          </Card.Body>
        </Card.Root>

        <Card.Root>
          <Card.Header>
            <Card.Title>Accent — brown</Card.Title>
            <Card.Description>
              The default <code class={css({ fontFamily: 'mono', fontSize: 'xs', bg: 'bg.subtle', px: '1', py: '0.5', borderRadius: 'sm' })}>colorPalette</code>, set on <code class={css({ fontFamily: 'mono', fontSize: 'xs', bg: 'bg.subtle', px: '1', py: '0.5', borderRadius: 'sm' })}>{'<html>'}</code>. Every recipe resolves <code class={css({ fontFamily: 'mono', fontSize: 'xs', bg: 'bg.subtle', px: '1', py: '0.5', borderRadius: 'sm' })}>colorPalette.*</code> to it.
            </Card.Description>
          </Card.Header>
          <Card.Body class={css({ display: 'grid', gap: '3', gridTemplateColumns: '1fr', sm: { gridTemplateColumns: 'repeat(2, 1fr)' } })}>
            <For each={accent}>{(item) => <Swatch name={item[0]} label={item[1]} />}</For>
          </Card.Body>
        </Card.Root>

        <Card.Root>
          <Card.Header>
            <Card.Title>Feedback palettes</Card.Title>
            <Card.Description>Status chips and alerts — <code class={css({ fontFamily: 'mono', fontSize: 'xs', bg: 'bg.subtle', px: '1', py: '0.5', borderRadius: 'sm' })}>Badge colorPalette</code> swaps them in.</Card.Description>
          </Card.Header>
          <Card.Body class={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
            <For each={feedback}>
              {(item) => (
                <div class={css({ display: 'flex', alignItems: 'center', gap: '3' })}>
                  <span
                    class={css({ h: '6', w: '6', borderRadius: 'md', border: '1px solid', borderColor: 'border', flexShrink: '0' })}
                    style={{ background: varOf(`${item[0]}.solid.bg`) }}
                  />
                  <span class={css({ fontSize: 'xs', fontWeight: '600', color: 'fg.default', fontFamily: 'mono', w: '24' })}>{item[0]}</span>
                  <Badge colorPalette={item[0]} class={colorPaletteSeeds[item[0]]}>{item[1]}</Badge>
                </div>
              )}
            </For>
          </Card.Body>
        </Card.Root>

        <ScaleCard title="Brown — accent" scale={tokens.colors.brown} description="The brand accent, 1–12 (dark values shown)." />
        <ScaleCard title="Sand — gray" scale={tokens.colors.sand} description="The neutral family behind every surface and text role." />

        <Card.Root class={css({ lg: { gridColumn: 'span 2' } })}>
          <Card.Header>
            <Card.Title>Typography</Card.Title>
            <Card.Description>
              Outfit text styles from the theme — <code class={css({ fontFamily: 'mono', fontSize: 'xs', bg: 'bg.subtle', px: '1', py: '0.5', borderRadius: 'sm' })}>textStyle</code> roles.
            </Card.Description>
          </Card.Header>
          <Card.Body>
            <div class={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
              <For each={typography}>
                {(t) => (
                  <div class={css({ display: 'flex', alignItems: 'baseline', gap: '4', justifyContent: 'space-between' })}>
                    <span class={css({ fontSize: 'xs', color: 'fg.subtle', fontFamily: 'mono', flexShrink: '0', w: '16' })}>{t.style}</span>
                    <span class={textStyleClasses[t.style]}>{t.sample}</span>
                  </div>
                )}
              </For>
              <div class={css({ display: 'flex', alignItems: 'baseline', gap: '4', justifyContent: 'space-between' })}>
                <span class={css({ fontSize: 'xs', color: 'fg.subtle', fontFamily: 'mono', flexShrink: '0', w: '16' })}>label</span>
                <span class={css({ textStyle: 'label' })}>Field labels and menu items</span>
              </div>
              <div class={css({ display: 'flex', alignItems: 'center', gap: '4', justifyContent: 'space-between' })}>
                <span class={css({ fontSize: 'xs', color: 'fg.subtle', fontFamily: 'mono', flexShrink: '0', w: '16' })}>code</span>
                <code class={css({ textStyle: 'sm', fontFamily: 'mono', bg: 'bg.subtle', px: '3', py: '1.5', borderRadius: 'lg', color: 'fg.default' })}>
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
