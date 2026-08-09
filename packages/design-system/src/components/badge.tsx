import { splitProps, type JSX } from 'solid-js'
import { cx } from 'styled-system/css'
import { badge, type BadgeVariantProps } from 'styled-system/recipes'

export type BadgeProps = BadgeVariantProps &
  Omit<JSX.IntrinsicElements['span'], 'class' | 'children'> & {
    class?: string
    children?: JSX.Element
  }

/**
 * Small status/label chip. Variants come from the `badge` recipe:
 * `tone` (primary | neutral | success | warning | danger | info) and
 * `variant` (subtle | solid | outline).
 */
export function Badge(props: BadgeProps) {
  const [local, rest] = splitProps(props, ['tone', 'variant', 'class', 'children'])
  return (
    <span class={cx(badge({ tone: local.tone, variant: local.variant }), local.class)} {...rest}>
      {local.children}
    </span>
  )
}
