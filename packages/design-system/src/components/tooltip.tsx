import { splitProps, type JSX } from 'solid-js'
import { Tooltip } from '@ark-ui/solid/tooltip'
import { cx, sva } from 'styled-system/css'

const tooltipRecipe = sva({
  slots: ['positioner', 'content', 'arrow', 'arrowTip'],
  base: {
    positioner: { zIndex: 'tooltip' },
    content: {
      bg: 'fg',
      color: 'bg',
      textStyle: 'caption',
      fontWeight: '500',
      borderRadius: 'md',
      px: '2.5',
      py: '1.5',
      boxShadow: 'overlay',
      _open: { animation: 'fadeIn 120ms ease-out' },
      _closed: { animation: 'fadeOut 100ms ease-in' },
    },
    arrow: { '--arrow-size': '8px' },
    arrowTip: { bg: 'fg', borderRadius: '2px' },
  },
})

type SlotProps<E extends keyof JSX.IntrinsicElements> = {
  class?: string
} & Omit<JSX.IntrinsicElements[E], 'class'>

export interface TooltipProps {
  openDelay?: number
  closeDelay?: number
  children: JSX.Element
}

export function TooltipRoot(props: TooltipProps) {
  return (
    <Tooltip.Root openDelay={props.openDelay} closeDelay={props.closeDelay}>
      {props.children}
    </Tooltip.Root>
  )
}

export function TooltipTrigger(props: SlotProps<'button'>) {
  const [local, rest] = splitProps(props, ['class'])
  return <Tooltip.Trigger class={local.class} {...rest} />
}

export function TooltipPositioner(props: SlotProps<'div'>) {
  const [local, rest] = splitProps(props, ['class'])
  return <Tooltip.Positioner class={cx(tooltipRecipe().positioner, local.class)} {...rest} />
}

export function TooltipContent(props: SlotProps<'div'>) {
  const [local, rest] = splitProps(props, ['class'])
  return <Tooltip.Content class={cx(tooltipRecipe().content, local.class)} {...rest} />
}

export function TooltipArrow(props: SlotProps<'div'>) {
  const [local, rest] = splitProps(props, ['class'])
  return (
    <Tooltip.Arrow class={cx(tooltipRecipe().arrow, local.class)} {...rest}>
      <Tooltip.ArrowTip class={tooltipRecipe().arrowTip} />
    </Tooltip.Arrow>
  )
}

export const TooltipParts = {
  Root: TooltipRoot,
  Trigger: TooltipTrigger,
  Positioner: TooltipPositioner,
  Content: TooltipContent,
  Arrow: TooltipArrow,
}
