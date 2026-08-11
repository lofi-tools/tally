import { createMemo, createSignal, For, Match, onCleanup, onMount, Show, Switch as SolidSwitch, type Component } from 'solid-js'
import { Avatar, Badge, Button, Kbd, Select, Toaster, toaster } from '@tally/design-system'
import { createListCollection } from '@tally/design-system'
import { Building2, ChevronDown, FileCheck2, LayoutGrid, Plug, Plus, Settings, Users } from 'lucide-solid'
import { css } from 'styled-system/css'
import { bankOptions, getCompanyData, SAMPLE_COMPANY_ID, sampleCompany, type Company, type DataSource } from './mock_data'
import { loadDb, saveDb, type Db } from './db'
import { AccountsView } from './views/Accounts'
import { FilingsView } from './views/Filings'
import { PayrollView } from './views/Payroll'
import { IntegrationsView } from './views/Integrations'
import { SettingsView } from './views/Settings'
import { AddCompanyDialog, type NewCompanyInput } from './components/AddCompanyDialog'
import { SaveProgressDialog } from './components/SaveProgressDialog'
import { SampleBanner } from './components/SampleBanner'

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
  const [db, setDb] = createSignal<Db>(loadDb())
  const updateDb = (fn: (d: Db) => Db) => {
    const next = fn(db())
    setDb(next)
    saveDb(next)
  }

  const [view, setView] = createSignal<ViewKey>('accounts')
  const [companyId, setCompanyId] = createSignal<string>(SAMPLE_COMPANY_ID)
  const [addOpen, setAddOpen] = createSignal(false)
  const [accountOpen, setAccountOpen] = createSignal(false)

  const companies = () => db().companies
  const sources = () => db().sources
  const account = () => db().account

  // The sample company retires once ANY user company has a data source.
  const sampleRetired = createMemo(() => companies().some((c) => (sources()[c.id] ?? []).length > 0))
  const allCompanies = createMemo(() => (sampleRetired() ? companies() : [sampleCompany, ...companies()]))
  const currentCompany = createMemo(() => allCompanies().find((c) => c.id === companyId()) ?? allCompanies()[0])
  const hasRealCompany = () => companies().length > 0
  const bannerVisible = () => !hasRealCompany() && !db().bannerDismissed

  // Company picker items: sample (until retired) + user companies + "Add company".
  const pickerItems = createMemo(() => [
    ...(sampleRetired() ? [] : [{ label: sampleCompany.name, value: SAMPLE_COMPANY_ID, sample: true as const }]),
    ...companies().map((c) => ({ label: c.name, value: c.id })),
    { label: 'Add company…', value: '__add__' },
  ])
  const pickerCollection = createMemo(() => createListCollection({ items: pickerItems() }))

  const onCompanyChange = (d: { value: string[] }) => {
    const v = d.value[0]
    if (v === '__add__') {
      setAddOpen(true)
    } else if (v && allCompanies().some((c) => c.id === v)) {
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

  const addCompany = (input: NewCompanyInput) => {
    const company: Company = {
      id: input.name.toLowerCase().replace(/[^a-z0-9]+/g, '-') || 'company',
      name: input.name,
      companyNumber: input.companyNumber,
      utr: input.utr,
      sic: input.sic,
      address: input.address,
      standard: input.standard,
    }
    updateDb((d) => ({ ...d, companies: [...d.companies, company] }))
    setCompanyId(company.id)
    toaster.create({
      title: 'Company added',
      description: 'Connect a bank or upload your ledger to start.',
      type: 'success',
    })
  }

  const connectSource = (cid: string, bank: (typeof bankOptions)[number]) => {
    if (sources()[cid]?.some((s) => s.id === bank.id)) {
      toaster.create({ title: `${bank.name} is already connected`, type: 'info' })
      return
    }
    const src: DataSource = {
      id: bank.id,
      name: `${bank.name} Business`,
      kind: 'bank',
      institution: bank.name,
      status: 'connected',
      lastSync: 'just now',
      accountCount: 0,
    }
    updateDb((d) => ({ ...d, sources: { ...d.sources, [cid]: [...(d.sources[cid] ?? []), src] } }))
    // If connecting retires the sample while it is selected, move to the user's company.
    if (companyId() === SAMPLE_COMPANY_ID) setCompanyId(cid)
    toaster.create({
      title: `Connected ${bank.name}`,
      description: 'Mock — real Open Banking consent lands with the backend.',
      type: 'success',
    })
  }

  const saveAccount = (name: string, email: string) => {
    updateDb((d) => ({ ...d, account: { saved: true, name, email } }))
    toaster.create({ title: 'Progress saved', description: 'Mock — real auth lands with the backend.', type: 'success' })
  }

  const dismissBanner = () => updateDb((d) => ({ ...d, bannerDismissed: true }))

  // Safety net: zero companies of any kind (sample retired + none added).
  if (!currentCompany()) {
    return (
      <div class={css({ h: '100dvh', display: 'grid', placeItems: 'center', bg: 'canvas', color: 'fg.default', fontFamily: 'sans', px: '4' })}>
        <div class={css({ textAlign: 'center', maxW: '24rem' })}>
          <div class={css({ display: 'flex', justifyContent: 'center', mb: '4' })}>
            <LogoMark />
          </div>
          <h1 class={css({ textStyle: '2xl', fontWeight: '800', letterSpacing: '-0.02em' })}>Add your first company</h1>
          <p class={css({ textStyle: 'sm', color: 'fg.muted', mt: '2' })}>Tally needs a company before you can prepare accounts or file returns.</p>
          <div class={css({ mt: '5' })}>
            <Button onClick={() => setAddOpen(true)}>Add company</Button>
          </div>
        </div>
        <AddCompanyDialog
          open={addOpen()}
          onOpenChange={setAddOpen}
          existingNumbers={companies().map((c) => c.companyNumber)}
          onAdd={addCompany}
        />
        <Toaster />
      </div>
    )
  }

  // Reactive getter — Solid JSX only updates when expressions read signals
  // directly, so the current company must be read via a function in JSX.
  const cd = () => currentCompany()!

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
            value={[companyId()]}
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
                  <span class={css({ display: 'flex', alignItems: 'center', gap: '2', fontSize: 'sm', fontWeight: '600', minW: '0' })}>
                    <span class={css({ truncate: true })}>{cd().name}</span>
                    <Show when={cd().id === SAMPLE_COMPANY_ID}>
                      <Badge variant="outline" class={css({ flexShrink: '0', fontSize: '10px', px: '1.5', py: '0' })}>
                        Sample
                      </Badge>
                    </Show>
                  </span>
                  <span class={css({ display: 'block', fontSize: 'xs', color: 'fg.subtle', fontFamily: 'mono' })}>
                    {cd().companyNumber}
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
                        <Show
                          when={item.value === '__add__'}
                          fallback={
                            <span class={css({ display: 'inline-flex', alignItems: 'center', gap: '2' })}>
                              {item.label}
                              <Show when={'sample' in item && item.sample}>
                                <Badge variant="outline" class={css({ fontSize: '10px', px: '1.5', py: '0' })}>
                                  Sample
                                </Badge>
                              </Show>
                            </span>
                          }
                        >
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

        <div class={css({ borderTop: '1px solid {colors.border.subtle}', px: '3', py: '2.5', display: 'flex', flexDirection: 'column', gap: '2' })}>
          <Show when={hasRealCompany()}>
            <Show
              when={account()}
              fallback={
                <button
                  type="button"
                  onClick={() => setAccountOpen(true)}
                  class={css({
                    display: 'flex',
                    alignItems: 'center',
                    gap: '2',
                    w: 'full',
                    px: '2.5',
                    py: '1.5',
                    borderRadius: 'md',
                    fontSize: 'xs',
                    fontWeight: '500',
                    color: 'fg.muted',
                    bg: 'transparent',
                    border: 'none',
                    cursor: 'pointer',
                    textAlign: 'left',
                    _hover: { bg: 'bg.subtle', color: 'fg.default' },
                  })}
                >
                  Save your progress — <span class={css({ color: 'brown.11', fontWeight: '600' })}>create an account</span>
                </button>
              }
            >
              <span class={css({ px: '2.5', fontSize: 'xs', color: 'fg.subtle' })}>Saved · {account()!.name}</span>
            </Show>
          </Show>
          <div class={css({ display: 'flex', alignItems: 'center', gap: '2.5' })}>
            <Avatar.Root class={css({ h: '8', w: '8' })}>
              <Avatar.Fallback name="Sam Rivera" />
            </Avatar.Root>
            <span class={css({ minW: '0', flex: '1' })}>
              <span class={css({ display: 'block', fontSize: 'sm', fontWeight: '600', truncate: true })}>Sam Rivera</span>
              <span class={css({ display: 'block', fontSize: 'xs', color: 'fg.subtle' })}>Director</span>
            </span>
          </div>
        </div>
      </aside>

      {/* ---------- Main ---------- */}
      <main class={css({ flex: '1', minW: '0', overflowY: 'auto' })}>
        <div class={css({ maxW: '60rem', mx: 'auto', p: { base: '5', md: '8' } })}>
          <Show when={bannerVisible()}>
            <SampleBanner onAddCompany={() => setAddOpen(true)} onDismiss={dismissBanner} />
          </Show>
          <SolidSwitch>
            <Match when={view() === 'accounts'}>
              <AccountsView
                company={cd()}
                data={getCompanyData(cd().id)}
                sources={sources()[cd().id] ?? []}
                onGoToIntegrations={() => setView('integrations')}
              />
            </Match>
            <Match when={view() === 'filings'}>
              <FilingsView company={cd()} data={getCompanyData(cd().id)} />
            </Match>
            <Match when={view() === 'payroll'}>
              <PayrollView company={cd()} data={getCompanyData(cd().id)} />
            </Match>
            <Match when={view() === 'integrations'}>
              <IntegrationsView company={cd()} sources={sources()[cd().id] ?? []} onConnect={(bank) => connectSource(cd().id, bank)} />
            </Match>
            <Match when={view() === 'settings'}>
              <SettingsView company={cd()} />
            </Match>
          </SolidSwitch>
        </div>
      </main>

      {/* ---------- Add company (search) ---------- */}
      <AddCompanyDialog
        open={addOpen()}
        onOpenChange={setAddOpen}
        existingNumbers={companies().map((c) => c.companyNumber)}
        onAdd={addCompany}
      />

      {/* ---------- Save progress (simulated account) ---------- */}
      <SaveProgressDialog open={accountOpen()} onOpenChange={setAccountOpen} onSave={saveAccount} />

      <Toaster />
    </div>
  )
}
