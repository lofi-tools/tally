import { splitProps, type JSX } from 'solid-js'
import { cx } from 'styled-system/css'
import { kbd } from 'styled-system/recipes'

export type KbdProps = Omit<JSX.IntrinsicElements['kbd'], 'class'> & {
  class?: string
  children?: JSX.Element
}

/**
 * Keyboard-shortcut chip — the keyboard-first brand signature. Mono, small,
 * hairline-bordered; pair it with menu items, tooltips and empty-state hints.
 *
 *   <Menu.Item value="csv">
 *     <MenuItemText>Download CSV</MenuItemText>
 *     <Kbd>⌘⇧E</Kbd>
 *   </Menu.Item>
 */
export function Kbd(props: KbdProps) {
  const [local, rest] = splitProps(props, ['class', 'children'])
  return (
    <kbd class={cx(kbd(), local.class)} {...rest}>
      {local.children}
    </kbd>
  )
}
