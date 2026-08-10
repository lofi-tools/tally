import type { JSX } from 'solid-js'
import { Badge } from '@tally/design-system'
import { css } from 'styled-system/css'

export interface SectionProps {
  id?: string
  eyebrow: string
  title: string
  description: string
  children: JSX.Element
}

export function Section(props: SectionProps) {
  return (
    <section id={props.id} class={css({ py: '16', borderTop: '1px solid', borderColor: 'border' })}>
      <Badge variant="subtle">{props.eyebrow}</Badge>
      <h2 class={css({ textStyle: '4xl', fontWeight: 'extrabold', letterSpacing: '-0.02em', mt: '3' })}>
        {props.title}
      </h2>
      <p class={css({ textStyle: 'lg', color: 'fg.muted', mt: '2', maxW: '40rem' })}>
        {props.description}
      </p>
      <div class={css({ mt: '8' })}>{props.children}</div>
    </section>
  )
}
