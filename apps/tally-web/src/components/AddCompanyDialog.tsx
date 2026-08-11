import { createSignal, For, Show } from 'solid-js'
import { Button, Dialog, Field, Input, Select, toaster } from '@tally/design-system'
import { createListCollection } from '@tally/design-system'
import { ArrowLeft, Search, X } from 'lucide-solid'
import { css } from 'styled-system/css'
import { button } from 'styled-system/recipes'
import { searchCompanies, type CompanySearchResult } from '../mock_data'

export interface NewCompanyInput {
  name: string
  companyNumber: string
  utr: string
  sic: string
  address: string
  standard: 'FRS 105' | 'FRS 102'
  periodStart: string
  periodEnd: string
}

const standardOptions = createListCollection({
  items: [
    { label: 'FRS 105 — Micro-entity', value: 'FRS 105' },
    { label: 'FRS 102 — Small', value: 'FRS 102' },
  ],
})

const initialForm = () => ({
  utr: '',
  standard: 'FRS 105' as 'FRS 105' | 'FRS 102',
  periodStart: '',
  periodEnd: '',
})

export function AddCompanyDialog(props: {
  open: boolean
  onOpenChange: (open: boolean) => void
  existingNumbers: string[]
  onAdd: (input: NewCompanyInput) => void
}) {
  const [step, setStep] = createSignal<'search' | 'review'>('search')
  const [query, setQuery] = createSignal('')
  const [results, setResults] = createSignal<CompanySearchResult[]>([])
  const [searched, setSearched] = createSignal(false)
  const [sel, setSel] = createSignal<CompanySearchResult | null>(null)
  const [form, setForm] = createSignal(initialForm())

  const setField =
    (key: keyof ReturnType<typeof form>) => (e: { currentTarget: { value: string } }) =>
      setForm((f) => ({ ...f, [key]: e.currentTarget.value }))

  const reset = () => {
    setStep('search')
    setQuery('')
    setResults([])
    setSearched(false)
    setSel(null)
    setForm(initialForm())
  }

  const doSearch = () => {
    setResults(searchCompanies(query()))
    setSearched(true)
  }

  const pick = (r: CompanySearchResult) => {
    setSel(r)
    setStep('review')
  }

  const submit = () => {
    const s = sel()
    if (!s) return
    if (!form().utr.trim()) {
      toaster.create({ title: 'UTR required', description: 'The CT600 needs a 10-digit tax reference.', type: 'error' })
      return
    }
    if (props.existingNumbers.includes(s.companyNumber)) {
      toaster.create({ title: 'Company already added', description: `${s.name} is already in your workspace.`, type: 'error' })
      return
    }
    props.onAdd({
      name: s.name,
      companyNumber: s.companyNumber,
      utr: form().utr.trim(),
      sic: s.sic,
      address: s.address,
      standard: form().standard,
      periodStart: form().periodStart,
      periodEnd: form().periodEnd,
    })
    props.onOpenChange(false)
  }

  return (
    <Dialog.Root
      open={props.open}
      onOpenChange={(d) => {
        props.onOpenChange(d.open)
        if (!d.open) reset()
      }}
    >
      <Dialog.Backdrop />
      <Dialog.Positioner>
        <Dialog.Content>
          <Dialog.CloseTrigger>
            <X />
          </Dialog.CloseTrigger>
          <Dialog.Header>
            <Dialog.Title>{step() === 'search' ? 'Add company' : 'Confirm company details'}</Dialog.Title>
            <Dialog.Description>
              {step() === 'search'
                ? 'Search Companies House to find your company (mocked — the real search forwards to Companies House).'
                : 'Check the search results and add what a search can’t tell us.'}
            </Dialog.Description>
          </Dialog.Header>
          <Dialog.Body>
            <Show
              when={step() === 'search'}
              fallback={
                <Show when={sel()} fallback={null}>
                  {(s) => (
                    <div class={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
                      <div
                        class={css({
                          p: '3.5',
                          borderRadius: 'md',
                          border: '1px solid {colors.border}',
                          bg: 'bg.subtle',
                        })}
                      >
                        <div class={css({ fontSize: 'sm', fontWeight: '700' })}>{s().name}</div>
                        <div class={css({ textStyle: 'xs', color: 'fg.muted', mt: '0.5', fontFamily: 'mono' })}>
                          {s().companyNumber} · incorporated {s().incorporationDate}
                        </div>
                        <div class={css({ textStyle: 'xs', color: 'fg.subtle', mt: '1.5' })}>{s().address}</div>
                        <div class={css({ textStyle: 'xs', color: 'fg.subtle' })}>{s().sic}</div>
                      </div>

                      <Field.Root required>
                        <Field.Label>
                          Unique Taxpayer Reference <Field.RequiredIndicator />
                        </Field.Label>
                        <Input
                          placeholder="10-digit tax reference"
                          class={css({ fontFamily: 'mono' })}
                          value={form().utr}
                          onInput={setField('utr')}
                        />
                        <Field.HelperText>Not held by Companies House — used for the CT600.</Field.HelperText>
                      </Field.Root>

                      <Field.Root>
                        <Field.Label>Accounting standard</Field.Label>
                        <Select.Root
                          collection={standardOptions}
                          value={[form().standard]}
                          onValueChange={(d) => setForm((f) => ({ ...f, standard: (d.value[0] as 'FRS 105' | 'FRS 102') ?? f.standard }))}
                        >
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
                      </Field.Root>

                      <div class={css({ display: 'grid', gap: '4', sm: { gridTemplateColumns: 'repeat(2, 1fr)' } })}>
                        <Field.Root>
                          <Field.Label>Period start</Field.Label>
                          <Input type="date" value={form().periodStart} onInput={setField('periodStart')} class={css({ fontFamily: 'mono' })} />
                        </Field.Root>
                        <Field.Root>
                          <Field.Label>Period end</Field.Label>
                          <Input type="date" value={form().periodEnd} onInput={setField('periodEnd')} class={css({ fontFamily: 'mono' })} />
                        </Field.Root>
                      </div>
                    </div>
                  )}
                </Show>
              }
            >
              <div class={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
                <div class={css({ display: 'flex', gap: '2' })}>
                  <div class={css({ position: 'relative', flex: '1' })}>
                    <Search
                      class={css({
                        position: 'absolute',
                        left: '3',
                        top: '50%',
                        transform: 'translateY(-50%)',
                        w: '3.5',
                        h: '3.5',
                        color: 'fg.subtle',
                        pointerEvents: 'none',
                      })}
                    />
                    <Input
                      placeholder="Company name or number"
                      class={css({ pl: '9' })}
                      value={query()}
                      onInput={(e) => setQuery(e.currentTarget.value)}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') doSearch()
                      }}
                    />
                  </div>
                  <Button onClick={doSearch}>Search</Button>
                </div>

                <Show when={searched()}>
                  <Show when={results().length > 0} fallback={<div class={css({ py: '8', textAlign: 'center', textStyle: 'sm', color: 'fg.muted' })}>No company found — check the name or number.</div>}>
                    <div class={css({ display: 'flex', flexDirection: 'column', maxH: '64', overflowY: 'auto' })}>
                      <For each={results()}>
                        {(r) => (
                          <button
                            type="button"
                            onClick={() => pick(r)}
                            class={css({
                              display: 'block',
                              w: 'full',
                              textAlign: 'left',
                              px: '3',
                              py: '2.5',
                              borderRadius: 'md',
                              border: 'none',
                              bg: 'transparent',
                              cursor: 'pointer',
                              _hover: { bg: 'bg.subtle' },
                              _focusVisible: { bg: 'bg.subtle', outline: 'none' },
                            })}
                          >
                            <span class={css({ display: 'block', fontSize: 'sm', fontWeight: '600', color: 'fg.default' })}>{r.name}</span>
                            <span class={css({ display: 'block', textStyle: 'xs', color: 'fg.muted', mt: '0.5' })}>
                              <span class={css({ fontFamily: 'mono' })}>{r.companyNumber}</span> · {r.jurisdiction}
                            </span>
                            <span class={css({ display: 'block', textStyle: 'xs', color: 'fg.subtle', mt: '0.5', truncate: true })}>
                              {r.address}
                            </span>
                          </button>
                        )}
                      </For>
                    </div>
                  </Show>
                </Show>
              </div>
            </Show>
          </Dialog.Body>
          <Dialog.Footer>
            <Show when={step() === 'review'} fallback={<Dialog.ActionTrigger class={button({ variant: 'outline' })}>Cancel</Dialog.ActionTrigger>}>
              <Button variant="plain" onClick={() => setStep('search')}>
                <ArrowLeft class={css({ w: '3.5', h: '3.5' })} /> Back to search
              </Button>
              <Button onClick={submit}>Add company</Button>
            </Show>
          </Dialog.Footer>
        </Dialog.Content>
      </Dialog.Positioner>
    </Dialog.Root>
  )
}
