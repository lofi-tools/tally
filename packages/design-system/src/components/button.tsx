import { splitProps, type JSX } from 'solid-js'
import { cx } from 'styled-system/css'
import { button, type ButtonVariantProps } from 'styled-system/recipes'

export type ButtonProps = ButtonVariantProps &
  Omit<JSX.IntrinsicElements['button'], 'class' | 'children'> & {
    class?: string
    children?: JSX.Element
  }

/**
 * Themed button. Variants come from the `button` recipe in the theme:
 * `visual` (solid | subtle | outline | ghost), `tone` (primary | danger |
 * neutral) and `size` (xs | sm | md | lg).
 */
export function Button(props: ButtonProps) {
  // Only variant keys may be passed to the recipe function — extra keys such
  // as `class`/`children` (JSX children are objects) trip Panda's compound-
  // variant assertion. `undefined` values are fine (defaults kick in).
  const [local, rest] = splitProps(props, ['visual', 'tone', 'size', 'class', 'children'])
  return (
    <button class={cx(button({ visual: local.visual, tone: local.tone, size: local.size }), local.class)} {...rest}>
      {local.children}
    </button>
  )
}
