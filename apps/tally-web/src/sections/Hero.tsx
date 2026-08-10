import { For } from 'solid-js'
import { Badge, Button, Card } from '@tally/design-system'
import { cx, css } from 'styled-system/css'
import { ArrowRight, Layers, Palette, Zap } from '../components/icons'

const stack = [
  {
    name: 'SolidJS',
    desc: 'Fine-grained, reactive UI runtime',
    icon: Zap,
    class: css({ bg: 'colorPalette.subtle.bg', color: 'colorPalette.subtle.fg' }),
  },
  {
    name: 'Panda CSS',
    desc: 'Type-safe, token-driven styling',
    icon: Palette,
    class: css({ bg: 'green.subtle.bg', color: 'green.subtle.fg' }),
  },
  {
    name: 'Ark UI',
    desc: 'Headless, accessible primitives',
    icon: Layers,
    class: css({ bg: 'blue.subtle.bg', color: 'blue.subtle.fg' }),
  },
] as const

function scrollTo(id: string) {
  document.getElementById(id)?.scrollIntoView({ behavior: 'smooth' })
}

export function Hero() {
  return (
    <section class={css({ pt: '20', pb: '20', md: { pt: '28', pb: '24' } })}>
      <Badge variant="subtle" class={css({ gap: '1.5' })}>
        <span class={css({ w: '1.5', h: '1.5', borderRadius: 'full', bg: 'colorPalette.solid.bg', display: 'inline-block' })} />
        SolidJS + Panda CSS + Ark UI
      </Badge>
      <h1 class={css({ textStyle: '6xl', fontWeight: 'extrabold', letterSpacing: '-0.03em', mt: '5', maxW: '46rem' })}>
        One theme, every screen —{' '}
        <span class={css({ bgGradient: 'to-b', gradientFrom: 'brown.11', gradientTo: 'brown.9', bgClip: 'text', color: 'transparent' })}>
          the Tally design system
        </span>
      </h1>
      <p class={css({ textStyle: 'lg', color: 'fg.muted', mt: '4', maxW: '36rem' })}>
        A token-driven foundation for the Tally web app — Park UI on SolidJS. Configure the
        palette once in the theme, and every component — buttons, forms, dialogs, menus —
        follows along. Dark by default, light when you want it.
      </p>
      <div class={css({ display: 'flex', gap: '3', mt: '8', flexWrap: 'wrap' })}>
        <Button size="lg" onClick={() => scrollTo('components')}>
          Explore components <ArrowRight />
        </Button>
        <Button size="lg" variant="outline" onClick={() => scrollTo('tokens')}>
          View design tokens
        </Button>
      </div>
      <div class={css({ display: 'grid', gap: '4', mt: '14', sm: { gridTemplateColumns: 'repeat(3, 1fr)' } })}>
        <For each={stack}>
          {(item) => (
            <Card.Root>
              <Card.Body class={css({ display: 'flex', alignItems: 'center', gap: '3', p: '5' })}>
                <span class={css({ h: '10', w: '10', borderRadius: 'lg', display: 'grid', placeItems: 'center', flexShrink: '0' })}>
                  <span class={cx(css({ display: 'grid', placeItems: 'center', h: '8', w: '8', borderRadius: 'md' }), item.class)}>
                    <item.icon />
                  </span>
                </span>
                <span>
                  <span class={css({ display: 'block', fontSize: 'sm', fontWeight: '600', color: 'fg.default' })}>{item.name}</span>
                  <span class={css({ display: 'block', fontSize: 'xs', color: 'fg.muted', mt: '0.5' })}>{item.desc}</span>
                </span>
              </Card.Body>
            </Card.Root>
          )}
        </For>
      </div>
    </section>
  )
}
