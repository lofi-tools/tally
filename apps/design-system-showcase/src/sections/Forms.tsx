import { For } from 'solid-js'
import { Card, Field, Input, Select, Textarea, createListCollection } from '@tally/design-system'
import { css } from 'styled-system/css'
import { Section } from './Section'

const filingStandards = createListCollection({
  items: [
    { label: 'FRS 105 — Micro-entity', value: 'frs105' },
    { label: 'FRS 102 — Small', value: 'frs102' },
    { label: 'FRS 101 — Reduced disclosure', value: 'frs101' },
    { label: 'IFRS', value: 'ifrs' },
  ],
})

const years = createListCollection({
  items: [
    { label: 'FY2023/24', value: 'fy2023-24' },
    { label: 'FY2024/25', value: 'fy2024-25' },
    { label: 'FY2025/26', value: 'fy2025-26' },
    { label: 'FY2026/27', value: 'fy2026-27' },
  ],
})

const companies = createListCollection({
  items: [{ label: 'Private limited company', value: 'ltd' }],
})

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
            <Card.Description>Label, hint, required and error states via <code class={css({ fontFamily: 'mono', fontSize: 'xs', bg: 'bg.subtle', px: '1', py: '0.5', borderRadius: 'sm' })}>Field</code>.</Card.Description>
          </Card.Header>
          <Card.Body class={css({ display: 'flex', flexDirection: 'column', gap: '5', maxW: 'md' })}>
            <Field.Root>
              <Field.Label>Email</Field.Label>
              <Input type="email" placeholder="you@company.co.uk" />
              <Field.HelperText>We'll never share your email.</Field.HelperText>
            </Field.Root>
            <Field.Root required>
              <Field.Label>
                Company name
                <Field.RequiredIndicator />
              </Field.Label>
              <Input placeholder="Example Ltd." />
            </Field.Root>
            <Field.Root invalid>
              <Field.Label>Unique Taxpayer Reference</Field.Label>
              <Input placeholder="1234567890" />
              <Field.ErrorText>Enter the 10-digit UTR shown on your tax return.</Field.ErrorText>
            </Field.Root>
            <Field.Root disabled>
              <Field.Label>Disabled</Field.Label>
              <Input placeholder="This field is locked" />
            </Field.Root>
            <Field.Root>
              <Field.Label>Notes</Field.Label>
              <Textarea placeholder="Anything else to add?" />
              <Field.HelperText>Optional — shown to your accountant.</Field.HelperText>
            </Field.Root>
          </Card.Body>
        </Card.Root>

        <Card.Root>
          <Card.Header>
            <Card.Title>Select</Card.Title>
            <Card.Description>Single-select with keyboard navigation and same-width dropdown.</Card.Description>
          </Card.Header>
          <Card.Body class={css({ display: 'flex', flexDirection: 'column', gap: '5', maxW: 'md' })}>
            <Select.Root collection={filingStandards}>
              <Select.Label>Filing standard</Select.Label>
              <Select.Control>
                <Select.Trigger>
                  <Select.ValueText placeholder="Choose a standard…" />
                  <Select.Indicator />
                </Select.Trigger>
              </Select.Control>
              <Select.Positioner>
                <Select.Content>
                  <For each={filingStandards.items}>
                    {(item) => (
                      <Select.Item item={item}>
                        <Select.ItemText>{item.label}</Select.ItemText>
                        <Select.ItemIndicator />
                      </Select.Item>
                    )}
                  </For>
                </Select.Content>
              </Select.Positioner>
              <Select.HiddenSelect />
            </Select.Root>

            <Select.Root collection={years} defaultValue={['fy2024-25']}>
              <Select.Label>Financial year</Select.Label>
              <Select.Control>
                <Select.Trigger>
                  <Select.ValueText placeholder="Pick a year…" />
                  <Select.Indicator />
                </Select.Trigger>
              </Select.Control>
              <Select.Positioner>
                <Select.Content>
                  <For each={years.items}>
                    {(item) => (
                      <Select.Item item={item}>
                        <Select.ItemText>{item.label}</Select.ItemText>
                        <Select.ItemIndicator />
                      </Select.Item>
                    )}
                  </For>
                </Select.Content>
              </Select.Positioner>
              <Select.HiddenSelect />
            </Select.Root>

            <Select.Root collection={companies} defaultValue={['ltd']} disabled>
              <Select.Label>Company type</Select.Label>
              <Select.Control>
                <Select.Trigger>
                  <Select.ValueText />
                  <Select.Indicator />
                </Select.Trigger>
              </Select.Control>
              <Select.Positioner>
                <Select.Content>
                  <For each={companies.items}>
                    {(item) => (
                      <Select.Item item={item}>
                        <Select.ItemText>{item.label}</Select.ItemText>
                        <Select.ItemIndicator />
                      </Select.Item>
                    )}
                  </For>
                </Select.Content>
              </Select.Positioner>
              <Select.HiddenSelect />
            </Select.Root>
          </Card.Body>
        </Card.Root>
      </div>
    </Section>
  )
}
