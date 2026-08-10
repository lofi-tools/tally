import { splitProps, type JSX } from 'solid-js'
import { Field } from '@ark-ui/solid/field'
import { cx, css, sva } from 'styled-system/css'

const fieldRecipe = sva({
  slots: ['root', 'label', 'control', 'helper', 'error'],
  base: {
    root: { display: 'flex', flexDirection: 'column', gap: '1.5' },
    label: { textStyle: 'caption', fontWeight: '600', color: 'fg' },
    control: {
      width: 'full',
      h: '10',
      px: '3',
      fontSize: 'sm',
      color: 'fg',
      bg: 'surface',
      border: '1px solid',
      borderColor: 'border',
      borderRadius: 'lg',
      transitionProperty: 'border-color, box-shadow',
      transitionDuration: '150ms',
      _placeholder: { color: 'fgSubtle' },
      _hover: { borderColor: 'borderStrong' },
      // Crisp focus — border brightens to the accent, no glow ring (Linear rule)
      _focus: {
        outline: 'none',
        borderColor: 'accent',
        boxShadow: '0 0 0 1px',
        boxShadowColor: 'accent',
      },
      _invalid: { borderColor: 'danger' },
      _disabled: { opacity: '0.6', cursor: 'not-allowed', bg: 'surfaceMuted' },
    },
    helper: { textStyle: 'caption', color: 'fgMuted' },
    error: { textStyle: 'caption', color: 'danger' },
  },
})

type FieldControlProps = {
  class?: string
  label?: JSX.Element
  hint?: JSX.Element
  error?: JSX.Element
  invalid?: boolean
}

export type InputProps = FieldControlProps & Omit<JSX.IntrinsicElements['input'], 'class'>

/** Text input with optional label, helper text and error state (Ark `Field`). */
export function Input(props: InputProps) {
  const [local, rest] = splitProps(props, ['class', 'label', 'hint', 'error', 'invalid'])
  const styles = fieldRecipe()
  return (
    <Field.Root
      class={cx(styles.root, local.class)}
      invalid={local.invalid}
      required={rest.required}
      disabled={rest.disabled}
    >
      {local.label && <Field.Label class={styles.label}>{local.label}</Field.Label>}
      <Field.Input class={styles.control} {...rest} />
      {local.error ? (
        <Field.ErrorText class={styles.error}>{local.error}</Field.ErrorText>
      ) : (
        local.hint && <Field.HelperText class={styles.helper}>{local.hint}</Field.HelperText>
      )}
    </Field.Root>
  )
}

export type TextareaProps = FieldControlProps & Omit<JSX.IntrinsicElements['textarea'], 'class'>

/** Multi-line text area sharing the input's field styling. */
export function Textarea(props: TextareaProps) {
  const [local, rest] = splitProps(props, ['class', 'label', 'hint', 'error', 'invalid'])
  const styles = fieldRecipe()
  return (
    <Field.Root
      class={cx(styles.root, local.class)}
      invalid={local.invalid}
      required={rest.required}
      disabled={rest.disabled}
    >
      {local.label && <Field.Label class={styles.label}>{local.label}</Field.Label>}
      <Field.Textarea
        class={cx(styles.control, css({ h: 'auto', minH: '24', py: '2.5', resize: 'vertical', lineHeight: '1.5' }))}
        {...rest}
      />
      {local.error ? (
        <Field.ErrorText class={styles.error}>{local.error}</Field.ErrorText>
      ) : (
        local.hint && <Field.HelperText class={styles.helper}>{local.hint}</Field.HelperText>
      )}
    </Field.Root>
  )
}
