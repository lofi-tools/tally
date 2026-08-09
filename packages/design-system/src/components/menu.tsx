import { splitProps, type JSX } from 'solid-js'
import { Menu, type MenuItemProps as ArkMenuItemProps } from '@ark-ui/solid/menu'
import { cx, sva } from 'styled-system/css'

const menuRecipe = sva({
  slots: ['positioner', 'content', 'item', 'itemText', 'itemGroupLabel', 'separator'],
  base: {
    positioner: { zIndex: 'dropdown' },
    content: {
      bg: 'surface',
      border: '1px solid',
      borderColor: 'border',
      borderRadius: 'lg',
      boxShadow: 'elevated',
      p: '1',
      minW: '48',
      _open: { animation: 'scaleIn 150ms ease-out' },
      _closed: { animation: 'scaleOut 120ms ease-in' },
    },
    item: {
      display: 'flex',
      alignItems: 'center',
      gap: '2',
      px: '2.5',
      py: '1.5',
      borderRadius: 'md',
      fontSize: 'sm',
      color: 'fg',
      cursor: 'pointer',
      _highlighted: { bg: 'accentMuted', color: 'accent' },
      _disabled: { opacity: '0.5', cursor: 'not-allowed' },
    },
    itemText: {},
    itemGroupLabel: { px: '2.5', py: '1', fontSize: 'xs', fontWeight: '600', color: 'fgSubtle' },
    separator: { height: '1px', bg: 'border', my: '1' },
  },
})

type SlotProps<E extends keyof JSX.IntrinsicElements> = {
  class?: string
} & Omit<JSX.IntrinsicElements[E], 'class'>

export interface MenuProps {
  lazyMount?: boolean
  unmountOnExit?: boolean
  children: JSX.Element
}

export function MenuRoot(props: MenuProps) {
  return (
    <Menu.Root lazyMount={props.lazyMount} unmountOnExit={props.unmountOnExit}>
      {props.children}
    </Menu.Root>
  )
}

export function MenuTrigger(props: SlotProps<'button'>) {
  const [local, rest] = splitProps(props, ['class'])
  return <Menu.Trigger class={local.class} {...rest} />
}

export function MenuPositioner(props: SlotProps<'div'>) {
  const [local, rest] = splitProps(props, ['class'])
  return <Menu.Positioner class={cx(menuRecipe().positioner, local.class)} {...rest} />
}

export function MenuContent(props: SlotProps<'div'>) {
  const [local, rest] = splitProps(props, ['class'])
  return <Menu.Content class={cx(menuRecipe().content, local.class)} {...rest} />
}

export interface MenuItemProps extends Omit<ArkMenuItemProps, 'class'> {
  class?: string
}

export function MenuItem(props: MenuItemProps) {
  const [local, rest] = splitProps(props, ['class'])
  return <Menu.Item class={cx(menuRecipe().item, local.class)} {...rest} />
}

export function MenuItemText(props: SlotProps<'div'>) {
  const [local, rest] = splitProps(props, ['class'])
  return <Menu.ItemText class={cx(menuRecipe().itemText, local.class)} {...rest} />
}

export function MenuItemGroupLabel(props: SlotProps<'div'>) {
  const [local, rest] = splitProps(props, ['class'])
  return <Menu.ItemGroupLabel class={cx(menuRecipe().itemGroupLabel, local.class)} {...rest} />
}

export function MenuSeparator(props: SlotProps<'hr'>) {
  const [local, rest] = splitProps(props, ['class'])
  return <Menu.Separator class={cx(menuRecipe().separator, local.class)} {...rest} />
}

export const MenuParts = {
  Root: MenuRoot,
  Trigger: MenuTrigger,
  Positioner: MenuPositioner,
  Content: MenuContent,
  Item: MenuItem,
  ItemText: MenuItemText,
  ItemGroupLabel: MenuItemGroupLabel,
  Separator: MenuSeparator,
}
