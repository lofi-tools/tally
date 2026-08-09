import { splitProps, type JSX } from 'solid-js'
import { Tabs, type TabsValueChangeDetails } from '@ark-ui/solid/tabs'
import { cx, sva } from 'styled-system/css'

const tabsRecipe = sva({
  slots: ['root', 'list', 'trigger', 'indicator', 'content'],
  base: {
    root: { display: 'flex', flexDirection: 'column', gap: '4' },
    list: {
      display: 'inline-flex',
      alignItems: 'center',
      gap: '1',
      overflowX: 'auto',
      borderBottom: '1px solid',
      borderColor: 'border',
    },
    trigger: {
      display: 'inline-flex',
      alignItems: 'center',
      gap: '2',
      px: '3',
      py: '2',
      fontSize: 'sm',
      fontWeight: '500',
      color: 'fgMuted',
      cursor: 'pointer',
      whiteSpace: 'nowrap',
      marginBottom: '-1px',
      borderBottom: '2px solid transparent',
      transitionProperty: 'color, border-color',
      transitionDuration: '150ms',
      _hover: { color: 'fg' },
      _selected: { color: 'fg', fontWeight: '600', borderColor: 'accent' },
      _focusVisible: {
        outline: '2px solid',
        outlineColor: 'accent',
        outlineOffset: '2px',
        borderRadius: 'md',
      },
      _disabled: { opacity: '0.5', cursor: 'not-allowed' },
    },
    indicator: { height: '2px', bg: 'accent' },
    content: { textStyle: 'bodySm', color: 'fgMuted' },
  },
})

type SlotProps<E extends keyof JSX.IntrinsicElements> = {
  class?: string
} & Omit<JSX.IntrinsicElements[E], 'class'>

export interface TabsProps {
  class?: string
  value?: string
  defaultValue?: string
  onValueChange?: (details: TabsValueChangeDetails) => void
  children: JSX.Element
}

export function TabsRoot(props: TabsProps) {
  const [local, rest] = splitProps(props, ['class', 'value', 'defaultValue', 'onValueChange'])
  return (
    <Tabs.Root
      class={cx(tabsRecipe().root, local.class)}
      value={local.value}
      defaultValue={local.defaultValue}
      onValueChange={local.onValueChange}
      {...rest}
    >
      {props.children}
    </Tabs.Root>
  )
}

export function TabsList(props: SlotProps<'div'>) {
  const [local, rest] = splitProps(props, ['class'])
  return <Tabs.List class={cx(tabsRecipe().list, local.class)} {...rest} />
}

export interface TabsTriggerProps extends SlotProps<'button'> {
  value: string
}

export function TabsTrigger(props: TabsTriggerProps) {
  const [local, rest] = splitProps(props, ['class', 'value'])
  return <Tabs.Trigger value={local.value} class={cx(tabsRecipe().trigger, local.class)} {...rest} />
}

export function TabsIndicator(props: SlotProps<'div'>) {
  const [local, rest] = splitProps(props, ['class'])
  return <Tabs.Indicator class={cx(tabsRecipe().indicator, local.class)} {...rest} />
}

export interface TabsContentProps extends SlotProps<'div'> {
  value: string
}

export function TabsContent(props: TabsContentProps) {
  const [local, rest] = splitProps(props, ['class', 'value'])
  return <Tabs.Content value={local.value} class={cx(tabsRecipe().content, local.class)} {...rest} />
}

export const TabsParts = {
  Root: TabsRoot,
  List: TabsList,
  Trigger: TabsTrigger,
  Indicator: TabsIndicator,
  Content: TabsContent,
}
