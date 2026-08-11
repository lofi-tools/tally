import { createMemo, createSignal, For, Match, onCleanup, onMount, Show, Switch as SolidSwitch, type Component } from 'solid-js'
import {
  Avatar,
  Badge,
  Button,
  Dialog,
  Field,
  Input,
  Kbd,
  Select,
  Toaster,
  toaster,
} from '@tally/design-system'
import { createListCollection } from '@tally/design-system'
import { Building2, ChevronDown, FileCheck2, LayoutGrid, Plug, Plus, Settings, Users, X } from 'lucide-solid'
import { css } from 'styled-system/css'
import { button } from 'styled-system/recipes'
import { seedCompanies, type Company } from './mock_data'
import { AccountsView } from './views/Accounts'
import { FilingsView } from './views/Filings'
import { PayrollView } from './views/Payroll'
import { IntegrationsView } from './views/Integrations'
import { SettingsView } from './views/Settings'

type ViewKey = 'accounts' | 'filings' | 'payroll' | 'integrations' | 'settings'

interface NavItem {
  id: ViewKey
  label: string
  icon: Component
  key: string
}

const navTop: NavItem[] = [
  { id: 'accounts', label: 'Accounts', icon: LayoutGrid, key: '1' },
  { id: 'filings', label: 'Filings', icon: FileCheck2, key: '2' },
  { id: 'payroll', label: 'Payroll', icon: Users, key: '3' },
]

const navBottom: NavItem[] = [
  { id: 'integrations', label: 'Integrations', icon: Plug, key: '4' },
  { id: 'settings', label: 'Settings', icon: Settings, key: '5' },
]

function LogoMark() {
  return (
    <svg width="22" height="22" viewBox="0 0 32 32" fill="none" aria-hidden="true">
      <defs>
        <linearGradient id="tally-mark" x1="0" y1="0" x2="32" y2="32" gradientUnits="userSpaceOnUse">
          <stop stop-color="#dbb594" />
          <stop offset="0.55" stop-color="#ad7f58" />
          <stop offset="1" stop-color="#7c5f46" />
        </linearGradient>
      </defs>
      <rect x="1" y="1" width="30" height="30" rx="9" fill="url(#tally-mark)" />
      <path d="M9 11.5h14M9 16.5h10M9 21.5h6" stroke="white" stroke-width="2.4" stroke-linecap="round" />
    </svg>
  )
}

function NavButton(props: { item: NavItem; active: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={props.onClick}
      aria-current={props.active ? 'page' : undefined}
      class={css({
        position: 'relative',
        w: 'full',
        display: 'flex',
        alignItems: 'center',
        gap: '2.5',
        px: '2.5',
        py: '2',
        borderRadius: 'md',
        fontSize: 'sm',
        fontWeight: '500',
        color: props.active ? 'fg.default' : 'fg.muted',
        bg: props.active ? 'bg.subtle' : 'transparent',
        // Unselected rows sit a touch dimmer than the active one.
        opacity: props.active ? '1' : '0.82',
        _hover: { bg: 'bg.subtle', color: 'fg.default', opacity: '1' },
        transition: 'background-color 120ms ease, color 120ms ease, opacity 120ms ease',
        cursor: 'pointer',
        border: 'none',
        textAlign: 'left',
      })}
    >
      <Show when={props.active}>
        {/* Bronze active indicator: vertical bar with a barely-there glow. */}
        <span
          aria-hidden="true"
          class={css({
            position: 'absolute',
            left: '0',
            top: '50%',
            transform: 'translateY(-50%)',
            w: '0.5',
            h: '4',
            borderRadius: 'full',
            bg: 'brown.9',
            boxShadow: '0 0 8px {colors.brown.a6}',
            animationName: 'fade-in',
            animationDuration: 'fast',
          })}
        />
      </Show>
      <props.item.icon />
      <span class={css({ flex: '1' })}>{props.item.label}</span>
      <Kbd class={css({ fontSize: '10px', lineHeight: '1', py: '0.5', px: '1' })}>{props.item.key}</Kbd>
    </button>
  )
}

export function App() {
  const [view, setView] = createSignal<ViewKey>('accounts')
  const [companies, setCompanies] = createSignal<Company[]>(seedCompanies)
  const [companyId, setCompanyId] = createSignal(seedCompanies[0].id)
  const [addOpen, setAddOpen] = createSignal(false)
  const [form, setForm] = createSignal({ name: '', number: '', utr: '' })

  const currentCompany = createMemo(
    () => companies().find((c) => c.id === companyId()) ?? companies()[0],
  )

  // Company picker items: the companies + a trailing "Add company" choice.
  const pickerItems = createMemo(() => [
    ...companies().map((c) => ({ label: c.name, value: c.id })),
    { label: 'Add company…', value: '__add__' },
  ])
  const pickerCollection = createMemo(() => createListCollection({ items: pickerItems() }))

  const onCompanyChange = (d: { value: string[] }) => {
    const v = d.value[0]
    if (v === '__add__') {
      setAddOpen(true)
    } else if (v && companies().some((c) => c.id === v)) {
      setCompanyId(v)
    }
  }

  // Keyboard-first nav: 1–5 switch views (unless typing in a field).
  onMount(() => {
    const map: Record<string, ViewKey> = {
      '1': 'accounts',
      '2': 'filings',
      '3': 'payroll',
      '4': 'integrations',
      '5': 'settings',
    }
    const handler = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null
      if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable)) return
      const next = map[e.key]
      if (next) setView(next)
    }
    window.addEventListener('keydown', handler)
    onCleanup(() => window.removeEventListener('keydown', handler))
  })

  const addCompany = () => {
    const name = form().name.trim()
    if (!name) {
      toaster.create({ title: 'Company name required', type: 'error' })
      return
    }
    const company: Company = {
      id: name.toLowerCase().replace(/[^a-z0-9]+/g, '-'),
      name,
      companyNumber: form().number.trim() || '—',
      utr: form().utr.trim() || '—',
      sic: '—',
      address: '—',
      standard: 'FRS 105',
    }
    setCompanies((cs) => [...cs, company])
    setCompanyId(company.id)
    setAddOpen(false)
    setForm({ name: '', number: '', utr: '' })
    toaster.create({ title: `Added ${name}`, description: 'Mock — persisted company data lands with the backend.', type: 'success' })
  }

  return (
    <div class={css({ h: '100dvh', display: 'flex', bg: 'canvas', color: 'fg.default', fontFamily: 'sans' })}>
      {/* ---------- Sidebar ---------- */}
      <aside
        class={css({
          w: '60',
          flexShrink: '0',
          borderRight: '1px solid {colors.border}',
          display: 'flex',
          flexDirection: 'column',
          minH: '0',
        })}
      >
        <div class={css({ px: '3', pt: '3', pb: '2.5', borderBottom: '1px solid {colors.border.subtle}' })}>
          <div class={css({ display: 'flex', alignItems: 'center', gap: '2', px: '1.5', pb: '3' })}>
            <LogoMark />
            <span class={css({ fontWeight: '800', fontSize: 'lg', letterSpacing: '-0.02em' })}>Tally</span>
            <Badge variant="outline" class={css({ ml: '1' })}>
              alpha
            </Badge>
          </div>

          {/* Company picker */}
          <Select.Root
            collection={pickerCollection()}
            value={[currentCompany().id]}
            onValueChange={onCompanyChange}
            positioning={{ sameWidth: true }}
          >
            <Select.Control class={css({ w: 'full' })}>
              <Select.Trigger
                class={css({
                  w: 'full',
                  justifyContent: 'flex-start',
                  gap: '2.5',
                  px: '2.5',
                  py: '2',
                  h: 'auto',
                  borderRadius: 'md',
                  border: '1px solid {colors.border}',
                  bg: 'bg.subtle',
                  color: 'fg.default',
                  _hover: { bg: 'bg.subtle', borderColor: 'gray.8' },
                  cursor: 'pointer',
                  textAlign: 'left',
                })}
              >
                <Building2 class={css({ w: '4', h: '4', color: 'fg.muted', flexShrink: '0' })} />
                <span class={css({ minW: '0', flex: '1' })}>
                  <span class={css({ display: 'block', fontSize: 'sm', fontWeight: '600', truncate: true })}>{currentCompany().name}</span>
                  <span class={css({ display: 'block', fontSize: 'xs', color: 'fg.subtle', fontFamily: 'mono' })}>
                    {currentCompany().companyNumber}
                  </span>
                </span>
                <ChevronDown class={css({ w: '3.5', h: '3.5', color: 'fg.muted', flexShrink: '0' })} />
              </Select.Trigger>
            </Select.Control>
            <Select.Positioner>
              <Select.Content class={css({ maxH: '72', overflowY: 'auto' })}>
                <For each={pickerItems()}>
                  {(item) => (
                    <Select.Item
                      item={item}
                      class={css(
                        item.value === '__add__' && {
                          borderTop: '1px solid {colors.border}',
                          borderRadius: '0',
                          mt: '0.5',
                          color: 'fg.muted',
                        },
                      )}
                    >
                      <Select.ItemText>
                        <Show when={item.value === '__add__'} fallback={item.label}>
                          <span class={css({ display: 'inline-flex', alignItems: 'center', gap: '2' })}>
                            <Plus class={css({ w: '3.5', h: '3.5' })} /> {item.label}
                          </span>
                        </Show>
                      </Select.ItemText>
                      <Show when={item.value !== '__add__'}>
                        <Select.ItemIndicator />
                      </Show>
                    </Select.Item>
                  )}
                </For>
              </Select.Content>
            </Select.Positioner>
            <Select.HiddenSelect />
          </Select.Root>
        </div>

        <nav class={css({ flex: '1', minH: '0', overflowY: 'auto', display: 'flex', flexDirection: 'column', p: '2', gap: '0.5' })}>
          <div class={css({ px: '2.5', pb: '1.5', pt: '1', fontSize: 'xs', fontWeight: '600', color: 'fg.subtle', textTransform: 'uppercase', letterSpacing: '0.06em' })}>
            Workspace
          </div>
          <For each={navTop}>
            {(item) => <NavButton item={item} active={view() === item.id} onClick={() => setView(item.id)} />}
          </For>
          <div class={css({ flex: '1' })} />
          <div class={css({ borderTop: '1px solid {colors.border.subtle}', pt: '2', mt: '2', display: 'flex', flexDirection: 'column', gap: '0.5' })}>
            <For each={navBottom}>
              {(item) => <NavButton item={item} active={view() === item.id} onClick={() => setView(item.id)} />}
            </For>
          </div>
        </nav>

        <div class={css({ borderTop: '1px solid {colors.border.subtle}', px: '3', py: '2.5', display: 'flex', alignItems: 'center', gap: '2.5' })}>
          <Avatar.Root class={css({ h: '8', w: '8' })}>
            <Avatar.Fallback name="Sam Rivera" />
          </Avatar.Root>
          <span class={css({ minW: '0', flex: '1' })}>
            <span class={css({ display: 'block', fontSize: 'sm', fontWeight: '600', truncate: true })}>Sam Rivera</span>
            <span class={css({ display: 'block', fontSize: 'xs', color: 'fg.subtle' })}>Director</span>
          </span>
        </div>
      </aside>

      {/* ---------- Main ---------- */}
      <main class={css({ flex: '1', minW: '0', overflowY: 'auto' })}>
        <div class={css({ maxW: '60rem', mx: 'auto', p: { base: '5', md: '8' } })}>
          <SolidSwitch>
            <Match when={view() === 'accounts'}>
              <AccountsView company={currentCompany()} />
            </Match>
            <Match when={view() === 'filings'}>
              <FilingsView company={currentCompany()} />
            </Match>
            <Match when={view() === 'payroll'}>
              <PayrollView company={currentCompany()} />
            </Match>
            <Match when={view() === 'integrations'}>
              <IntegrationsView company={currentCompany()} />
            </Match>
            <Match when={view() === 'settings'}>
              <SettingsView company={currentCompany()} />
            </Match>
          </SolidSwitch>
        </div>
      </main>

      {/* ---------- Add company ---------- */}
      <Dialog.Root open={addOpen()} onOpenChange={(d) => setAddOpen(d.open)}>
        <Dialog.Backdrop />
        <Dialog.Positioner>
          <Dialog.Content>
            <Dialog.CloseTrigger>
              <X />
            </Dialog.CloseTrigger>
            <Dialog.Header>
              <Dialog.Title>Add company</Dialog.Title>
              <Dialog.Description>
                The profile is usually pulled from Companies House — this is a mock form until the backend exists.
              </Dialog.Description>
            </Dialog.Header>
            <Dialog.Body>
              <div class={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
                <Field.Root required>
                  <Field.Label>
                    Company name <Field.RequiredIndicator />
                  </Field.Label>
                  <Input
                    placeholder="Example Ltd."
                    value={form().name}
                    onInput={(e) => setForm((f) => ({ ...f, name: e.currentTarget.value }))}
                  />
                </Field.Root>
                <div class={css({ display: 'grid', gap: '4', sm: { gridTemplateColumns: 'repeat(2, 1fr)' } })}>
                  <Field.Root>
                    <Field.Label>Companies House number</Field.Label>
                    <Input
                      placeholder="12345678"
                      class={css({ fontFamily: 'mono' })}
                      value={form().number}
                      onInput={(e) => setForm((f) => ({ ...f, number: e.currentTarget.value }))}
                    />
                  </Field.Root>
                  <Field.Root>
                    <Field.Label>UTR</Field.Label>
                    <Input
                      placeholder="10-digit tax reference"
                      class={css({ fontFamily: 'mono' })}
                      value={form().utr}
                      onInput={(e) => setForm((f) => ({ ...f, utr: e.currentTarget.value }))}
                    />
                  </Field.Root>
                </div>
              </div>
            </Dialog.Body>
            <Dialog.Footer>
              <Dialog.ActionTrigger class={button({ variant: 'outline' })}>Cancel</Dialog.ActionTrigger>
              <Button onClick={addCompany}>
                <Plus class={css({ w: '3.5', h: '3.5' })} /> Add company
              </Button>
            </Dialog.Footer>
          </Dialog.Content>
        </Dialog.Positioner>
      </Dialog.Root>

      <Toaster />
    </div>
  )
}
