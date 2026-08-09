import { For } from 'solid-js'
import { Badge, Switch, createColorMode, type ColorModeController } from '@tally/design-system'
import { css } from 'styled-system/css'
import { LogoMark, Moon, Sun } from './components/icons'
import { Hero } from './sections/Hero'
import { Tokens } from './sections/Tokens'
import { Buttons } from './sections/Buttons'
import { Forms } from './sections/Forms'
import { Overlays } from './sections/Overlays'

const nav = [
  ['Tokens', 'tokens'],
  ['Components', 'components'],
  ['Forms', 'forms'],
  ['Overlays', 'overlays'],
] as const

const navLink = css({
  fontSize: 'sm',
  color: 'fgMuted',
  px: '2',
  py: '1.5',
  borderRadius: 'md',
  textDecoration: 'none',
  transition: 'color 150ms ease, background-color 150ms ease',
  _hover: { color: 'fg', bg: 'surfaceMuted' },
})

function Header(props: { colorMode: ColorModeController }) {
  return (
    <header
      class={css({
        position: 'sticky',
        top: '0',
        zIndex: 'sticky',
        borderBottom: '1px solid',
        borderColor: 'border',
        bg: 'bg/85',
        backdropFilter: 'auto',
        backdropBlur: '8px',
      })}
    >
      <div
        class={css({
          maxW: '72rem',
          mx: 'auto',
          px: { base: '5', md: '8' },
          h: '16',
          display: 'flex',
          alignItems: 'center',
          gap: '3',
        })}
      >
        <a
          href="#top"
          class={css({
            display: 'inline-flex',
            alignItems: 'center',
            gap: '2.5',
            fontWeight: '700',
            fontSize: 'lg',
            color: 'fg',
            textDecoration: 'none',
          })}
        >
          <LogoMark />
          Tally
        </a>
        <Badge tone="primary" variant="outline" class={css({ display: { base: 'none', sm: 'inline-flex' } })}>
          design system
        </Badge>
        <nav class={css({ display: 'none', alignItems: 'center', gap: '1', md: { display: 'flex' } })}>
          <For each={nav}>
            {([label, id]) => (
              <a href={`#${id}`} class={navLink}>
                {label}
              </a>
            )}
          </For>
        </nav>
        <div class={css({ ml: 'auto' })}>
          <Switch
            checked={props.colorMode.mode() === 'dark'}
            onCheckedChange={(d) => props.colorMode.set(d.checked ? 'dark' : 'light')}
            label={
              <span class={css({ display: 'inline-flex', alignItems: 'center', gap: '1.5' })}>
                {props.colorMode.mode() === 'dark' ? <Moon /> : <Sun />}
                Dark
              </span>
            }
          />
        </div>
      </div>
    </header>
  )
}

export function App() {
  const colorMode = createColorMode()
  return (
    <div id="top" class={css({ minH: '100dvh', display: 'flex', flexDirection: 'column', bg: 'bg', color: 'fg', fontFamily: 'sans' })}>
      <Header colorMode={colorMode} />
      <main class={css({ flex: '1', w: 'full', maxW: '72rem', mx: 'auto', px: { base: '5', md: '8' } })}>
        <Hero />
        <Tokens />
        <Buttons />
        <Forms />
        <Overlays colorMode={colorMode} />
      </main>
      <footer class={css({ borderTop: '1px solid', borderColor: 'border', mt: '8', py: '8' })}>
        <div
          class={css({
            maxW: '72rem',
            mx: 'auto',
            px: { base: '5', md: '8' },
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            gap: '3',
            flexWrap: 'wrap',
          })}
        >
          <span class={css({ fontSize: 'xs', color: 'fgMuted' })}>Tally — UK company accounts &amp; CT600 filing</span>
          <span class={css({ fontSize: 'xs', color: 'fgSubtle' })}>SolidJS · Panda CSS · Ark UI</span>
        </div>
      </footer>
    </div>
  )
}
