import { Card, Input, Select, Textarea } from '@tally/design-system'
import { css } from 'styled-system/css'
import { Section } from './Section'

const filingStandards = [
  { label: 'FRS 105 — Micro-entity', value: 'frs105' },
  { label: 'FRS 102 — Small', value: 'frs102' },
  { label: 'FRS 101 — Reduced disclosure', value: 'frs101' },
  { label: 'IFRS', value: 'ifrs' },
]

const years = [
  { label: 'FY2023/24', value: 'fy2023-24' },
  { label: 'FY2024/25', value: 'fy2024-25' },
  { label: 'FY2025/26', value: 'fy2025-26' },
  { label: 'FY2026/27', value: 'fy2026-27' },
]

export function Forms() {
  return (
    <Section
      id="forms"
      eyebrow="Forms"
      title="Inputs & selection"
      description="Form controls built on Ark UI's headless field machinery — accessible labels, helper text and error states out of the box."
    >
      <div class={css({ display: 'grid', gap: '6', lg: { gridTemplateColumns: 'repeat(2, 1fr)' } })}>
        <Card.Root>
          <Card.Header>
            <Card.Title>Text inputs</Card.Title>
            <Card.Description>Label, hint, error and disabled states.</Card.Description>
          </Card.Header>
          <Card.Body class={css({ display: 'flex', flexDirection: 'column', gap: '5', maxW: 'md' })}>
            <Input label="Email" type="email" placeholder="you@company.co.uk" hint="We'll never share your email." />
            <Input label="Company name" required placeholder="Example Ltd." />
            <Input label="Unique Taxpayer Reference" placeholder="1234567890" error="Enter the 10-digit UTR shown on your tax return." invalid />
            <Input label="Disabled" disabled placeholder="This field is locked" />
            <Textarea label="Notes" placeholder="Anything else to add?" hint="Optional — shown to your accountant." />
          </Card.Body>
        </Card.Root>

        <Card.Root>
          <Card.Header>
            <Card.Title>Select</Card.Title>
            <Card.Description>Single-select with keyboard navigation and same-width dropdown.</Card.Description>
          </Card.Header>
          <Card.Body class={css({ display: 'flex', flexDirection: 'column', gap: '5', maxW: 'md' })}>
            <Select label="Filing standard" placeholder="Choose a standard…" items={filingStandards} />
            <Select label="Financial year" defaultValue={['fy2024-25']} items={years} />
            <Select label="Company type" disabled defaultValue={['ltd']} items={[{ label: 'Private limited company', value: 'ltd' }]} />
          </Card.Body>
        </Card.Root>
      </div>
    </Section>
  )
}
