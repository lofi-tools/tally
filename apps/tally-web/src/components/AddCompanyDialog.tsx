import { createSignal, For, Show } from 'solid-js'
import { Button, Dialog, Field, Input, Select, Spinner, toaster } from '@tally/design-system'
import { createListCollection } from '@tally/design-system'
import { ArrowLeft, Search, X } from 'lucide-solid'
import { css } from 'styled-system/css'
import { button } from 'styled-system/recipes'
import { ApiError, NetworkError, searchCompanies, type SearchItem } from '../api'

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

/** Debounce so typing doesn't fire a Companies House request per keystroke. */
const SEARCH_DEBOUNCE_MS = 300

/** Map a search failure to an inline message (web-api-wiring-spec §14.5). */
function searchErrorText(e: unknown): string {
  if (e instanceof ApiError) {
    switch (e.code) {
      case 'companies_house_key_missing':
        return "Companies House isn't configured — set COMPANIES_HOUSE_API_KEY and restart the API."
      case 'company_not_found':
        return 'No company found with that name or number.'
      case 'companies_house_rate_limited':
        return 'Companies House rate limit reached — try again shortly.'
      case 'companies_house_upstream':
        return 'Companies House is unavailable — try again.'
      default:
        return e.message // envelope messages are UI-safe by contract
    }
  }
  return "Can't reach the API — is it running?"
}

export function AddCompanyDialog(props: {
  open: boolean
  onOpenChange: (open: boolean) => void
  existingNumbers: string[]
  onAdd: (input: NewCompanyInput) => Promise<void>
}) {
  const [step, setStep] = createSignal<'search' | 'review'>('search')
  const [query, setQuery] = createSignal('')
  const [results, setResults] = createSignal<SearchItem[]>([])
  const [searched, setSearched] = createSignal(false)
  const [searching, setSearching] = createSignal(false)
  const [searchError, setSearchError] = createSignal<string | undefined>(undefined)
  const [sel, setSel] = createSignal<SearchItem | null>(null)
  const [form, setForm] = createSignal(initialForm())
  const [submitting, setSubmitting] = createSignal(false)

  // Debounce timer + a request sequence number, so an out-of-order response
  // (slow earlier query beating a fast later one) is discarded.
  let debounceTimer: number | undefined
  let searchSeq = 0

  const setField =
    (key: keyof ReturnType<typeof form>) => (e: { currentTarget: { value: string } }) =>
      setForm((f) => ({ ...f, [key]: e.currentTarget.value }))

  const reset = () => {
    window.clearTimeout(debounceTimer)
    searchSeq += 1 // invalidate any in-flight search
    setStep('search')
    setQuery('')
    setResults([])
    setSearched(false)
    setSearching(false)
    setSearchError(undefined)
    setSel(null)
    setForm(initialForm())
    setSubmitting(false)
  }

  const runSearch = async (raw: string) => {
    const q = raw.trim()
    if (!q) {
      searchSeq += 1
      setResults([])
      setSearched(false)
      setSearching(false)
      setSearchError(undefined)
      return
    }
    const mySeq = ++searchSeq
    setSearching(true)
    setSearchError(undefined)
    try {
      const items = await searchCompanies(q)
      if (mySeq !== searchSeq) return // stale — a newer query owns the list
      setResults(items)
      setSearched(true)
    } catch (e) {
      if (mySeq !== searchSeq) return
      setResults([])
      setSearched(true)
      setSearchError(searchErrorText(e))
    } finally {
      if (mySeq === searchSeq) setSearching(false)
    }
  }

  const onInput = (e: { currentTarget: { value: string } }) => {
    const value = e.currentTarget.value
    setQuery(value)
    window.clearTimeout(debounceTimer)
    // Search as you type: refresh the matches shortly after the last key.
    debounceTimer = window.setTimeout(() => void runSearch(value), SEARCH_DEBOUNCE_MS)
  }

  const pick = (r: SearchItem) => {
    setSel(r)
    setStep('review')
  }

  const submit = async () => {
    const s = sel()
    if (!s) return
    if (!form().utr.trim()) {
      toaster.create({ title: 'UTR required', description: 'The CT600 needs a 10-digit tax reference.', type: 'error' })
      return
    }
    if (props.existingNumbers.includes(s.company_number)) {
      toaster.create({ title: 'Company already added', description: `${s.company_name} is already in your workspace.`, type: 'error' })
      return
    }
    setSubmitting(true)
    try {
      await props.onAdd({
        name: s.company_name,
        companyNumber: s.company_number,
        utr: form().utr.trim(),
        sic: '',
        address: s.address_snippet ?? '',
        standard: form().standard,
        periodStart: form().periodStart,
        periodEnd: form().periodEnd,
      })
      props.onOpenChange(false)
    } catch (e) {
      if (e instanceof ApiError && e.code === 'duplicate_company') {
        toaster.create({ title: 'Company already added', description: `${s.company_name} is already in your workspace.`, type: 'error' })
      } else if (e instanceof NetworkError) {
        toaster.create({ title: "Can't reach the API", description: 'Is the API running?', type: 'error' })
      } else if (e instanceof ApiError) {
        toaster.create({ title: 'Could not add company', description: e.message, type: 'error' })
      } else {
        toaster.create({ title: 'Could not add company', description: 'Something went wrong. Please try again.', type: 'error' })
      }
    } finally {
      setSubmitting(false)
    }
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
                ? 'Search Companies House to find your company.'
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
                        <div class={css({ fontSize: 'sm', fontWeight: '700' })}>{s().company_name}</div>
                        <div class={css({ textStyle: 'xs', color: 'fg.muted', mt: '0.5', fontFamily: 'mono' })}>
                          {s().company_number}
                          {s().date_of_creation ? ` · incorporated ${s().date_of_creation}` : ''}
                        </div>
                        <Show when={s().address_snippet}>
                          <div class={css({ textStyle: 'xs', color: 'fg.subtle', mt: '1.5' })}>{s().address_snippet}</div>
                        </Show>
                        <Show when={s().company_type || s().company_status}>
                          <div class={css({ textStyle: 'xs', color: 'fg.subtle' })}>
                            {[s().company_type, s().company_status].filter(Boolean).join(' · ')}
                          </div>
                        </Show>
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
                      onInput={onInput}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') {
                          window.clearTimeout(debounceTimer)
                          void runSearch(query())
                        }
                      }}
                    />
                  </div>
                  <Button
                    onClick={() => {
                      window.clearTimeout(debounceTimer)
                      void runSearch(query())
                    }}
                    disabled={searching() || !query().trim()}
                  >
                    Search
                  </Button>
                </div>

                <Show when={searchError()}>
                  <div
                    class={css({
                      textStyle: 'sm',
                      color: 'red.plain.fg',
                      bg: 'bg.subtle',
                      border: '1px solid {colors.red.a5}',
                      px: '3',
                      py: '2',
                      borderRadius: 'md',
                    })}
                  >
                    {searchError()}
                  </div>
                </Show>

                <Show when={searching()}>
                  <div class={css({ py: '6', display: 'flex', justifyContent: 'center' })}>
                    <Spinner size="sm" class={css({ color: 'brown.11' })} />
                  </div>
                </Show>

                <Show when={searched() && !searching() && !searchError()}>
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
                            <span class={css({ display: 'block', fontSize: 'sm', fontWeight: '600', color: 'fg.default' })}>{r.company_name}</span>
                            <span class={css({ display: 'block', textStyle: 'xs', color: 'fg.muted', mt: '0.5' })}>
                              <span class={css({ fontFamily: 'mono' })}>{r.company_number}</span>
                              {r.company_status ? ` · ${r.company_status}` : ''}
                            </span>
                            <Show when={r.address_snippet}>
                              <span class={css({ display: 'block', textStyle: 'xs', color: 'fg.subtle', mt: '0.5', truncate: true })}>
                                {r.address_snippet}
                              </span>
                            </Show>
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
              <Button variant="plain" disabled={submitting()} onClick={() => setStep('search')}>
                <ArrowLeft class={css({ w: '3.5', h: '3.5' })} /> Back to search
              </Button>
              <Button onClick={() => void submit()} loading={submitting()} disabled={submitting()}>
                Add company
              </Button>
            </Show>
          </Dialog.Footer>
        </Dialog.Content>
      </Dialog.Positioner>
    </Dialog.Root>
  )
}
