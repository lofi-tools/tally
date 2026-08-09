import { splitProps, type JSX } from 'solid-js'
import { Switch, type SwitchCheckedChangeDetails } from '@ark-ui/solid/switch'
import { cx, sva } from 'styled-system/css'

const switchRecipe = sva({
  slots: ['root', 'label', 'control', 'thumb'],
  base: {
    root: {
      display: 'inline-flex',
      alignItems: 'center',
      gap: '2.5',
      cursor: 'pointer',
      _disabled: { cursor: 'not-allowed' },
    },
    label: { fontSize: 'sm', fontWeight: '500', color: 'fg', _disabled: { opacity: '0.6' } },
    control: {
      display: 'inline-flex',
      alignItems: 'center',
      flexShrink: '0',
      w: '10',
      h: '6',
      p: '0.5',
      borderRadius: 'full',
      bg: 'borderStrong',
      transition: 'background-color 200ms ease',
      _checked: { bg: 'accent' },
      _disabled: { opacity: '0.5' },
      _focusVisible: {
        outline: '2px solid',
        outlineColor: 'accent',
        outlineOffset: '2px',
      },
    },
    thumb: {
      bg: 'white',
      w: '5',
      h: '5',
      borderRadius: 'full',
      boxShadow: 'sm',
      transform: 'translateX(0)',
      transition: 'transform 200ms ease',
      _checked: { transform: 'translateX(16px)' },
    },
  },
})

export interface SwitchProps {
  class?: string
  label?: JSX.Element
  checked?: boolean
  defaultChecked?: boolean
  onCheckedChange?: (details: SwitchCheckedChangeDetails) => void
  disabled?: boolean
  name?: string
}

export function SwitchControl(props: SwitchProps) {
  const [local, rest] = splitProps(props, [
    'class',
    'label',
    'checked',
    'defaultChecked',
    'onCheckedChange',
    'disabled',
    'name',
  ])
  const styles = switchRecipe()
  return (
    <Switch.Root
      class={cx(styles.root, local.class)}
      checked={local.checked}
      defaultChecked={local.defaultChecked}
      onCheckedChange={local.onCheckedChange}
      disabled={local.disabled}
      name={local.name}
      {...rest}
    >
      <Switch.Control class={styles.control}>
        <Switch.Thumb class={styles.thumb} />
      </Switch.Control>
      {local.label && <Switch.Label class={styles.label}>{local.label}</Switch.Label>}
      <Switch.HiddenInput />
    </Switch.Root>
  )
}
