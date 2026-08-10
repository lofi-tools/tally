import { splitProps, type JSX } from 'solid-js'
import { cx, sva } from 'styled-system/css'

const card = sva({
  slots: ['root', 'header', 'title', 'description', 'body', 'footer'],
  base: {
    root: {
      display: 'flex',
      flexDirection: 'column',
      bg: 'surface',
      border: '1px solid',
      borderColor: 'border',
      borderRadius: 'xl',
      overflow: 'hidden',
      // Flat by default (Linear rule: no shadow on non-floating elements);
      // hover lifts the border, not a shadow.
      transitionProperty: 'border-color, background-color',
      transitionDuration: '200ms',
      _hover: { borderColor: 'borderStrong' },
    },
    header: { display: 'flex', flexDirection: 'column', gap: '1', p: '6', pb: '0' },
    title: { textStyle: 'h3', color: 'fg' },
    description: { textStyle: 'bodySm', color: 'fgMuted' },
    body: { p: '6', flex: '1' },
    footer: { display: 'flex', alignItems: 'center', gap: '2', p: '6', pt: '0', mt: 'auto' },
  },
})

type SlotProps<E extends keyof JSX.IntrinsicElements> = {
  class?: string
} & Omit<JSX.IntrinsicElements[E], 'class'>

export function CardRoot(props: SlotProps<'div'>) {
  const [local, rest] = splitProps(props, ['class'])
  return <div class={cx(card().root, local.class)} {...rest} />
}

export function CardHeader(props: SlotProps<'div'>) {
  const [local, rest] = splitProps(props, ['class'])
  return <div class={cx(card().header, local.class)} {...rest} />
}

export function CardTitle(props: SlotProps<'h3'>) {
  const [local, rest] = splitProps(props, ['class'])
  return <h3 class={cx(card().title, local.class)} {...rest} />
}

export function CardDescription(props: SlotProps<'p'>) {
  const [local, rest] = splitProps(props, ['class'])
  return <p class={cx(card().description, local.class)} {...rest} />
}

export function CardBody(props: SlotProps<'div'>) {
  const [local, rest] = splitProps(props, ['class'])
  return <div class={cx(card().body, local.class)} {...rest} />
}

export function CardFooter(props: SlotProps<'div'>) {
  const [local, rest] = splitProps(props, ['class'])
  return <div class={cx(card().footer, local.class)} {...rest} />
}

export const Card = {
  Root: CardRoot,
  Header: CardHeader,
  Title: CardTitle,
  Description: CardDescription,
  Body: CardBody,
  Footer: CardFooter,
}
