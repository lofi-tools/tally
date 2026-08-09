import { For } from 'solid-js'
import { Badge, Button, Card } from '@tally/design-system'
import { cx, css } from 'styled-system/css'
import { ArrowRight, Layers, Palette, Zap } from '../components/icons'

const stack = [
  {
    name: 'SolidJS',
    desc: 'Fine-grained, reactive UI runtime',
    icon: Zap,
    class: css({ bg: 'accentMuted', color: 'accent' }),
  },
  {
    name: 'Panda CSS',
    desc: 'Type-safe, token-driven styling',
    icon: Palette,
    class: css({ bg: 'successSoft', color: 'success' }),
  },
  {
    name: 'Ark UI',
    desc: 'Headless, accessible primitives',
    icon: Layers,
    class: css({ bg: 'infoSoft', color: 'info' }),
  },
] as const

function scrollTo(id: string) {
  document.getElementById(id)?.scrollIntoView({ behavior: 'smooth' })
}

export function Hero() {
  return (
    <section class={css({ pt: '20', pb: '20', md: { pt: '28', pb: '24' } })}>
      <Badge tone="primary" variant="subtle" class={css({ gap: '1.5' })}>
        <span class={css({ w: '1.5', h: '1.5', borderRadius: 'full', bg: 'accent', display: 'inline-block' })} />
        SolidJS + Panda CSS + Ark UI
      </Badge>
      <h1 class={css({ textStyle: 'display', mt: '5', maxW: '46rem' })}>
        One theme, every screen — <span class={css({ color: 'accent' })}>the Tally design system</span>
      </h1>
      <p class={css({ textStyle: 'body', color: 'fgMuted', mt: '4', maxW: '36rem' })}>
        A token-driven foundation for the Tally web app. Configure the palette once in the
        theme, and every component — buttons, forms, dialogs, menus — follows along, in light
        and dark mode.
      </p>
      <div class={css({ display: 'flex', gap: '3', mt: '8', flexWrap: 'wrap' })}>
        <Button size="lg" onClick={() => scrollTo('components')}>
          Explore components <ArrowRight />
        </Button>
        <Button size="lg" visual="outline" onClick={() => scrollTo('tokens')}>
          View design tokens
        </Button>
      </div>
      <div class={css({ display: 'grid', gap: '4', mt: '14', sm: { gridTemplateColumns: 'repeat(3, 1fr)' } })}>
        <For each={stack}>
          {(item) => (
            <Card.Root class={css({ _hover: { boxShadow: 'elevated' }, transition: 'box-shadow 200ms ease' })}>
              <Card.Body class={css({ display: 'flex', alignItems: 'center', gap: '3', p: '5' })}>
                <span class={css({ h: '10', w: '10', borderRadius: 'lg', display: 'grid', placeItems: 'center', flexShrink: '0' })}>
                  <span class={cx(css({ display: 'grid', placeItems: 'center', h: '8', w: '8', borderRadius: 'md' }), item.class)}>
                    <item.icon />
                  </span>
                </span>
                <span>
                  <span class={css({ display: 'block', fontSize: 'sm', fontWeight: '600', color: 'fg' })}>{item.name}</span>
                  <span class={css({ display: 'block', fontSize: 'xs', color: 'fgMuted', mt: '0.5' })}>{item.desc}</span>
                </span>
              </Card.Body>
            </Card.Root>
          )}
        </For>
      </div>
    </section>
  )
}
