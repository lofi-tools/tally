import { splitProps, type JSX } from 'solid-js'
import { Dialog, type DialogOpenChangeDetails } from '@ark-ui/solid/dialog'
import { cx, css, sva } from 'styled-system/css'

const dialogRecipe = sva({
  slots: ['backdrop', 'positioner', 'content', 'title', 'description'],
  base: {
    backdrop: {
      bg: 'overlay',
      _open: { animation: 'fadeIn 150ms ease-out' },
      _closed: { animation: 'fadeOut 120ms ease-in' },
    },
    positioner: {
      position: 'fixed',
      inset: '0',
      zIndex: 'modal',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      p: '4',
    },
    content: {
      position: 'relative',
      display: 'flex',
      flexDirection: 'column',
      gap: '4',
      w: 'full',
      maxW: 'md',
      p: '6',
      bg: 'surfaceOverlay',
      border: '1px solid',
      borderColor: 'border',
      borderRadius: 'xl',
      boxShadow: 'overlay',
      _open: { animation: 'scaleIn 180ms ease-out' },
      _closed: { animation: 'scaleOut 120ms ease-in' },
    },
    title: { textStyle: 'h2', color: 'fg', pr: '8' },
    description: { textStyle: 'bodySm', color: 'fgMuted' },
  },
})

const closeTrigger = css({
  position: 'absolute',
  top: '3',
  right: '3',
  display: 'inline-flex',
  alignItems: 'center',
  justifyContent: 'center',
  h: '8',
  w: '8',
  borderRadius: 'md',
  color: 'fgMuted',
  cursor: 'pointer',
  transition: 'background-color 150ms ease, color 150ms ease',
  _hover: { bg: 'surfaceMuted', color: 'fg' },
  _focusVisible: { outline: '2px solid', outlineColor: 'accent', outlineOffset: '2px' },
})

type SlotProps<E extends keyof JSX.IntrinsicElements> = {
  class?: string
} & Omit<JSX.IntrinsicElements[E], 'class'>

export interface DialogProps {
  open?: boolean
  onOpenChange?: (details: DialogOpenChangeDetails) => void
  lazyMount?: boolean
  unmountOnExit?: boolean
  modal?: boolean
  children: JSX.Element
}

export function DialogTrigger(props: SlotProps<'button'>) {
  const [local, rest] = splitProps(props, ['class'])
  return <Dialog.Trigger class={local.class} {...rest} />
}

export function DialogRoot(props: DialogProps) {
  return (
    <Dialog.Root
      open={props.open}
      onOpenChange={props.onOpenChange}
      lazyMount={props.lazyMount}
      unmountOnExit={props.unmountOnExit}
      modal={props.modal}
    >
      {props.children}
    </Dialog.Root>
  )
}

export function DialogBackdrop(props: SlotProps<'div'>) {
  const [local, rest] = splitProps(props, ['class'])
  return <Dialog.Backdrop class={cx(dialogRecipe().backdrop, local.class)} {...rest} />
}

export function DialogPositioner(props: SlotProps<'div'>) {
  const [local, rest] = splitProps(props, ['class'])
  return <Dialog.Positioner class={cx(dialogRecipe().positioner, local.class)} {...rest} />
}

export function DialogContent(props: SlotProps<'div'>) {
  const [local, rest] = splitProps(props, ['class'])
  return <Dialog.Content class={cx(dialogRecipe().content, local.class)} {...rest} />
}

export function DialogTitle(props: SlotProps<'h2'>) {
  const [local, rest] = splitProps(props, ['class'])
  return <Dialog.Title class={cx(dialogRecipe().title, local.class)} {...rest} />
}

export function DialogDescription(props: SlotProps<'div'>) {
  const [local, rest] = splitProps(props, ['class'])
  return <Dialog.Description class={cx(dialogRecipe().description, local.class)} {...rest} />
}

export function DialogCloseTrigger(props: SlotProps<'button'>) {
  const [local, rest] = splitProps(props, ['class'])
  return <Dialog.CloseTrigger class={cx(closeTrigger, local.class)} {...rest} />
}

/** Styled dialog parts; the trigger stays unstyled so callers can use any button. */
export const DialogParts = {
  Root: DialogRoot,
  Trigger: DialogTrigger,
  Backdrop: DialogBackdrop,
  Positioner: DialogPositioner,
  Content: DialogContent,
  Title: DialogTitle,
  Description: DialogDescription,
  CloseTrigger: DialogCloseTrigger,
}
