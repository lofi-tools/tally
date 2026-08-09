import { createMemo, For, splitProps, type JSX } from 'solid-js'
import { Select, createListCollection, type SelectValueChangeDetails } from '@ark-ui/solid/select'
import { cx, sva } from 'styled-system/css'

const selectRecipe = sva({
  slots: ['root', 'label', 'trigger', 'value', 'indicator', 'content', 'item', 'itemText'],
  base: {
    root: { display: 'flex', flexDirection: 'column', gap: '1.5' },
    label: { textStyle: 'caption', fontWeight: '600', color: 'fg' },
    trigger: {
      width: 'full',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'space-between',
      gap: '2',
      h: '10',
      px: '3',
      fontSize: 'sm',
      color: 'fg',
      bg: 'surface',
      textAlign: 'left',
      cursor: 'pointer',
      border: '1px solid',
      borderColor: 'border',
      borderRadius: 'lg',
      transitionProperty: 'border-color, box-shadow',
      transitionDuration: '150ms',
      _hover: { borderColor: 'borderStrong' },
      _focusVisible: {
        outline: 'none',
        borderColor: 'accent',
        boxShadow: '0 0 0 3px',
        boxShadowColor: 'accentMuted',
      },
      _disabled: { opacity: '0.6', cursor: 'not-allowed', bg: 'surfaceMuted' },
    },
    value: { flex: '1', _placeholder: { color: 'fgSubtle' } },
    indicator: {
      color: 'fgSubtle',
      flexShrink: '0',
      display: 'inline-flex',
      transition: 'transform 150ms ease',
      _open: { transform: 'rotate(180deg)' },
    },
    content: {
      bg: 'surface',
      border: '1px solid',
      borderColor: 'border',
      borderRadius: 'lg',
      boxShadow: 'elevated',
      p: '1',
      zIndex: 'dropdown',
      maxH: '64',
      overflowY: 'auto',
      minW: 'max-content',
      _open: { animation: 'scaleIn 150ms ease-out' },
      _closed: { animation: 'scaleOut 120ms ease-in' },
    },
    item: {
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'space-between',
      gap: '2',
      px: '2.5',
      py: '1.5',
      borderRadius: 'md',
      fontSize: 'sm',
      color: 'fg',
      cursor: 'pointer',
      _highlighted: { bg: 'accentMuted', color: 'accent' },
      _selected: { color: 'accent', fontWeight: '600' },
      _disabled: { opacity: '0.5', cursor: 'not-allowed' },
    },
    itemText: {},
  },
})

export interface SelectOption {
  label: string
  value: string
  disabled?: boolean
}

export interface SelectProps {
  class?: string
  label?: JSX.Element
  items: SelectOption[]
  placeholder?: string
  defaultValue?: string[]
  value?: string[]
  onValueChange?: (details: SelectValueChangeDetails) => void
  disabled?: boolean
  invalid?: boolean
  name?: string
}

const chevron = (
  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <path d="m6 9 6 6 6-6" />
  </svg>
)

/** Single-select control built on Ark UI's `Select`. */
export function SelectControl(props: SelectProps) {
  const [local] = splitProps(props, [
    'class',
    'label',
    'items',
    'placeholder',
    'defaultValue',
    'value',
    'onValueChange',
    'disabled',
    'invalid',
    'name',
  ])
  const collection = createMemo(() => createListCollection({ items: local.items }))
  const styles = selectRecipe()
  return (
    <Select.Root
      class={cx(styles.root, local.class)}
      collection={collection()}
      defaultValue={local.defaultValue}
      value={local.value}
      onValueChange={local.onValueChange}
      disabled={local.disabled}
      invalid={local.invalid}
      name={local.name}
      positioning={{ sameWidth: true }}
    >
      {local.label && <Select.Label class={styles.label}>{local.label}</Select.Label>}
      <Select.Trigger class={styles.trigger}>
        <Select.ValueText class={styles.value} placeholder={local.placeholder} />
        <Select.Indicator class={styles.indicator}>{chevron}</Select.Indicator>
      </Select.Trigger>
      <Select.Positioner>
        <Select.Content class={styles.content}>
          <For each={local.items}>
            {(item) => (
              <Select.Item item={item} class={styles.item}>
                <Select.ItemText class={styles.itemText}>{item.label}</Select.ItemText>
              </Select.Item>
            )}
          </For>
        </Select.Content>
      </Select.Positioner>
      <Select.HiddenSelect />
    </Select.Root>
  )
}
