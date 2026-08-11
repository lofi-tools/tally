import { createSignal, For } from 'solid-js'
import { Button, Card, Dialog, Field, Input, Select, Switch, toaster } from '@tally/design-system'
import { createListCollection } from '@tally/design-system'
import { AlertTriangle, Save, X } from 'lucide-solid'
import { css } from 'styled-system/css'
import { button } from 'styled-system/recipes'
import { preferences, type Company } from '../mock_data'
import { PageHeader } from '../components/Shared'

const standardOptions = createListCollection({
  items: [
    { label: 'FRS 105 — Micro-entity', value: 'FRS 105' },
    { label: 'FRS 102 — Small', value: 'FRS 102' },
  ],
})

const leadOptions = createListCollection({
  items: [
    { label: '7 days before due', value: '7' },
    { label: '14 days before due', value: '14' },
    { label: '30 days before due', value: '30' },
  ],
})

function Section(props: { title: string; description?: string; children: import('solid-js').JSX.Element; danger?: boolean }) {
  return (
    <Card.Root
      class={css({
        mb: '6',
        _last: { mb: '0' },
        ...(props.danger ? { borderColor: 'red.outline.border' } : {}),
      })}
    >
      <div class={css({ px: '5', pt: '4', pb: '1' })}>
        <div class={css({ fontSize: 'sm', fontWeight: '600', color: props.danger ? 'red.plain.fg' : 'fg.default' })}>{props.title}</div>
        {props.description && <div class={css({ textStyle: 'xs', color: 'fg.muted', mt: '0.5' })}>{props.description}</div>}
      </div>
      <div class={css({ px: '5', py: '4' })}>{props.children}</div>
    </Card.Root>
  )
}

export function SettingsView(props: { company: Company }) {
  const [removeOpen, setRemoveOpen] = createSignal(false)
  const [profile, setProfile] = createSignal({
    name: props.company.name,
    number: props.company.companyNumber,
    utr: props.company.utr,
    sic: props.company.sic,
    address: props.company.address,
  })
  const setField = (key: keyof ReturnType<typeof profile>) => (e: { currentTarget: { value: string } }) =>
    setProfile((p) => ({ ...p, [key]: e.currentTarget.value }))

  return (
    <>
      <PageHeader title="Settings" description={`Workspace preferences and ${props.company.name}'s profile.`} />

      <div class={css({ maxW: '42rem' })}>
        <Section title="Company profile" description="This data is pulled from Companies House and cached locally.">
          <div class={css({ display: 'grid', gap: '4', md: { gridTemplateColumns: 'repeat(2, 1fr)' } })}>
            <Field.Root>
              <Field.Label>Company name</Field.Label>
              <Input value={profile().name} onInput={setField('name')} />
            </Field.Root>
            <Field.Root>
              <Field.Label>Companies House number</Field.Label>
              <Input value={profile().number} onInput={setField('number')} class={css({ fontFamily: 'mono' })} />
            </Field.Root>
            <Field.Root>
              <Field.Label>Unique Taxpayer Reference</Field.Label>
              <Input value={profile().utr} onInput={setField('utr')} class={css({ fontFamily: 'mono' })} />
              <Field.HelperText>10 digits — used for the CT600.</Field.HelperText>
            </Field.Root>
            <Field.Root>
              <Field.Label>SIC code</Field.Label>
              <Input value={profile().sic} onInput={setField('sic')} />
            </Field.Root>
            <Field.Root class={css({ md: { gridColumn: 'span 2' } })}>
              <Field.Label>Registered address</Field.Label>
              <Input value={profile().address} onInput={setField('address')} />
            </Field.Root>
          </div>
          <div class={css({ display: 'flex', justifyContent: 'flex-end', mt: '4' })}>
            <Button
              onClick={() =>
                toaster.create({ title: 'Profile saved (mock)', description: 'Persisting to the backend lands with the API.', type: 'success' })
              }
            >
              <Save class={css({ w: '3.5', h: '3.5' })} /> Save changes
            </Button>
          </div>
        </Section>

        <Section title="Filing preferences" description="Defaults applied when preparing a new set of accounts.">
          <div class={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
            <div class={css({ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '4' })}>
              <div>
                <div class={css({ fontSize: 'sm', fontWeight: '600' })}>Reporting standard</div>
                <div class={css({ textStyle: 'xs', color: 'fg.muted' })}>Used for new accounts periods.</div>
              </div>
              <Select.Root collection={standardOptions} defaultValue={[preferences.defaultStandard]}>
                <Select.Control>
                  <Select.Trigger>
                    <Select.ValueText />
                    <Select.Indicator />
                  </Select.Trigger>
                </Select.Control>
                <Select.Positioner>
                  <Select.Content>
                    <For each={standardOptions.items}>
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
            </div>

            <div class={css({ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '4' })}>
              <div>
                <div class={css({ fontSize: 'sm', fontWeight: '600' })}>Reminder lead time</div>
                <div class={css({ textStyle: 'xs', color: 'fg.muted' })}>When to nudge you before a filing is due.</div>
              </div>
              <Select.Root collection={leadOptions} defaultValue={[String(preferences.reminderLeadDays)]}>
                <Select.Control>
                  <Select.Trigger>
                    <Select.ValueText />
                    <Select.Indicator />
                  </Select.Trigger>
                </Select.Control>
                <Select.Positioner>
                  <Select.Content>
                    <For each={leadOptions.items}>
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
            </div>

            <div class={css({ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '4' })}>
              <div>
                <div class={css({ fontSize: 'sm', fontWeight: '600' })}>Auto-fetch from Companies House</div>
                <div class={css({ textStyle: 'xs', color: 'fg.muted' })}>Keep the profile in sync using the API key.</div>
              </div>
              <Switch.Root defaultChecked={preferences.autoFetchCompaniesHouse}>
                <Switch.Control />
                <Switch.HiddenInput />
              </Switch.Root>
            </div>
          </div>
        </Section>

        <Section title="Notifications" description="Where Tally can reach you about deadlines and runs.">
          <div class={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
            <div class={css({ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '4' })}>
              <div>
                <div class={css({ fontSize: 'sm', fontWeight: '600' })}>Email reminders</div>
                <div class={css({ textStyle: 'xs', color: 'fg.muted' })}>Filing deadlines and sync failures.</div>
              </div>
              <Switch.Root defaultChecked={preferences.emailReminders}>
                <Switch.Control />
                <Switch.HiddenInput />
              </Switch.Root>
            </div>
            <div class={css({ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '4' })}>
              <div>
                <div class={css({ fontSize: 'sm', fontWeight: '600' })}>Product updates</div>
                <div class={css({ textStyle: 'xs', color: 'fg.muted' })}>New filing standards and features.</div>
              </div>
              <Switch.Root defaultChecked={preferences.productUpdates}>
                <Switch.Control />
                <Switch.HiddenInput />
              </Switch.Root>
            </div>
          </div>
        </Section>

        <Section title="Danger zone" description="Irreversible actions for this company." danger>
          <div class={css({ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '4', flexWrap: 'wrap' })}>
            <div>
              <div class={css({ fontSize: 'sm', fontWeight: '600' })}>Remove {props.company.name}</div>
              <div class={css({ textStyle: 'xs', color: 'fg.muted' })}>Deletes the cached profile and book locally.</div>
            </div>
            <Button variant="outline" colorPalette="red" onClick={() => setRemoveOpen(true)}>
              <AlertTriangle class={css({ w: '3.5', h: '3.5' })} /> Remove company
            </Button>
          </div>
        </Section>
      </div>

      {/* ---------- Confirm remove ---------- */}
      <Dialog.Root open={removeOpen()} onOpenChange={(d) => setRemoveOpen(d.open)}>
        <Dialog.Backdrop />
        <Dialog.Positioner>
          <Dialog.Content>
            <Dialog.CloseTrigger>
              <X />
            </Dialog.CloseTrigger>
            <Dialog.Header>
              <Dialog.Title>Remove {props.company.name}?</Dialog.Title>
              <Dialog.Description>
                This is a mock action — real deletion will wipe the locally cached profile and transactions.
              </Dialog.Description>
            </Dialog.Header>
            <Dialog.Footer>
              <Dialog.ActionTrigger class={button({ variant: 'outline' })}>Cancel</Dialog.ActionTrigger>
              <Button
                colorPalette="red"
                onClick={() => {
                  setRemoveOpen(false)
                  toaster.create({ title: `Removed ${props.company.name} (mock)`, type: 'success' })
                }}
              >
                Remove company
              </Button>
            </Dialog.Footer>
          </Dialog.Content>
        </Dialog.Positioner>
      </Dialog.Root>
    </>
  )
}
