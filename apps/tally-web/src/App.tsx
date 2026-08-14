import { createEffect, createMemo, createSignal, For, Match, onCleanup, onMount, Show, Switch as SolidSwitch, type Component } from 'solid-js'
import { Avatar, Badge, Button, Kbd, Select, Toaster, toaster } from '@tally/design-system'
import { createListCollection } from '@tally/design-system'
import { Building2, ChevronDown, FileCheck2, LayoutGrid, LogIn, LogOut, Plug, Plus, Settings, UserRound, Users, WifiOff } from 'lucide-solid'
import { css } from 'styled-system/css'
import { bankOptions, getCompanyData, DEMO_COMPANY_ID, demoCompany, type Company, type DataSource } from './mock_data'
import { loadDb, saveDb, type Db } from './db'
import { restoreSession, session, setSession, signOut } from './session'
import { bootstrapGuest, createCompany, getToken, listCompanies, NetworkError, setToken, type Company as ApiCompany } from './api'
import { ensureGuestId } from './guest'
import { enqueueAdd, flushOutbox, listOutbox } from './outbox'
import { AccountsView } from './views/Accounts'
import { FilingsView } from './views/Filings'
import { PayrollView } from './views/Payroll'
import { IntegrationsView } from './views/Integrations'
import { SettingsView } from './views/Settings'
import { AddCompanyDialog, type NewCompanyInput } from './components/AddCompanyDialog'
import { SignInDialog } from './components/SignInDialog'
import { DemoBanner, type DemoBannerVariant } from './components/DemoBanner'
import { DevtoolsBanner } from './components/DevtoolsBanner'

type ViewKey = 'accounts' | 'filings' | 'payroll' | 'integrations' | 'settings'

interface NavItem {
  id: ViewKey
  label: string
  icon: Component
  key: string
  /** Disabled nav item, shown with a "Soon" badge (payroll — no endpoints yet). */
  soon?: boolean
}

const navTop: NavItem[] = [
  { id: 'accounts', label: 'Accounts', icon: LayoutGrid, key: '1' },
  { id: 'filings', label: 'Filings', icon: FileCheck2, key: '2' },
  { id: 'payroll', label: 'Payroll', icon: Users, key: '3', soon: true },
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
  const disabled = () => !!props.item.soon
  return (
    <button
      type="button"
      onClick={props.onClick}
      disabled={disabled()}
      aria-current={props.active ? 'page' : undefined}
      title={disabled() ? `${props.item.label} is coming soon` : undefined}
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
        opacity: disabled() ? '0.45' : props.active ? '1' : '0.82',
        cursor: disabled() ? 'not-allowed' : 'pointer',
        _hover: disabled() ? {} : { bg: 'bg.subtle', color: 'fg.default', opacity: '1' },
        transition: 'background-color 120ms ease, color 120ms ease, opacity 120ms ease',
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
      <Show when={props.item.soon}>
        <Badge variant="outline" class={css({ flexShrink: '0', fontSize: '10px', px: '1.5', py: '0' })}>
          Soon
        </Badge>
      </Show>
      <Kbd class={css({ fontSize: '10px', lineHeight: '1', py: '0.5', px: '1' })}>{props.item.key}</Kbd>
    </button>
  )
}

/** "Sam Rivera" → "SR" for the avatar fallback. */
const initials = (name: string) =>
  name
    .split(/\s+/)
    .filter(Boolean)
    .map((p) => p[0]!)
    .slice(0, 2)
    .join('')
    .toUpperCase()

export function App() {
  const [db, setDb] = createSignal<Db>(loadDb())
  const updateDb = (fn: (d: Db) => Db) => {
    const next = fn(db())
    setDb(next)
    saveDb(next)
  }

  const [view, setView] = createSignal<ViewKey>('accounts')
  const [companyId, setCompanyId] = createSignal<string>(DEMO_COMPANY_ID)
  const [addOpen, setAddOpen] = createSignal(false)
  const [signInOpen, setSignInOpen] = createSignal(false)

  // User companies come from the API (owned by the signed-in user or the
  // guest workspace — temp-user spec §7.4); the local DB only holds sources.
  const [apiCompanies, setApiCompanies] = createSignal<ApiCompany[]>([])
  const sources = () => db().sources

  // Session: restore a stored token on boot (local mode until resolved — no
  // login-wall flash, §5.2 / §14.6).
  onMount(() => {
    void (async () => {
      await restoreSession()
      // Offline adds queued earlier need a workspace to sync into — if the
      // browser has no session but does have a pending outbox, bootstrap the
      // guest now (temp-user spec §7.5: "app boot with connectivity").
      if (!getToken() && listOutbox().length > 0) {
        try {
          const resp = await bootstrapGuest(ensureGuestId())
          setToken(resp.token)
          setSession({ status: 'signed-in', user: resp.user })
        } catch {
          // Still offline — the outbox waits.
        }
      }
    })()
  })

  const sessionUser = createMemo(() => {
    const s = session()
    return s.status === 'signed-in' ? s.user : null
  })

  /** Guest mode: signed-in with a temporary user (temp-user spec §7.3). */
  const isGuest = createMemo(() => {
    const s = session()
    return s.status === 'signed-in' && s.user.is_temporary
  })

  /** The API `Company` shape → the internal display shape the views use. */
  const toDisplayCompany = (api: ApiCompany): Company => ({
    id: api.id,
    name: api.name,
    companyNumber: api.company_number,
    utr: api.tax_reference || '', // set in Settings when filing (never at add time)
    sic: api.sic_codes?.[0] ?? '',
    address: api.address_lines?.[0] ?? '',
    standard: api.accounting_standard === 'FRS 102' ? 'FRS 102' : 'FRS 105',
    registrationDate: api.registration_date ?? undefined,
  })

  const companies = createMemo(() => apiCompanies().map(toDisplayCompany))

  const refreshCompanies = async (): Promise<void> => {
    try {
      setApiCompanies(await listCompanies())
    } catch {
      // Offline — keep whatever is already shown.
    }
  }

  // Session → company list: fetch on sign-in (login, guest bootstrap,
  // adoption, restore) and replay offline adds; clear on sign-out.
  createEffect(() => {
    const s = session()
    if (s.status === 'signed-in') {
      void refreshCompanies()
      void flushOutbox(() => void refreshCompanies())
    } else if (s.status === 'local') {
      setApiCompanies([])
    }
  })

  // The demo company never retires: it stays in the picker (badged) and is
  // always the first item, so demo data remains explorable (spec §6.2).
  const allCompanies = createMemo(() => [demoCompany, ...companies()])
  const currentCompany = createMemo(() => allCompanies().find((c) => c.id === companyId()) ?? allCompanies()[0])

  // Demo banner state machine (spec §4). Precedence A > B > C; once ANY
  // user company has a data source, no variant renders in any selection.
  const anyDataConnected = createMemo(() => companies().some((c) => (sources()[c.id] ?? []).length > 0))
  // Not dismissible: the banner renders on every view while the applicable
  // state holds, so it always "re-appears" when switching screens/tabs.
  const banner = createMemo<DemoBannerVariant | null>(() => {
    if (anyDataConnected()) return null
    // Key off what is actually displayed: currentCompany() falls back to the
    // demo when companyId dangles (e.g. the selected company was migrated).
    if (currentCompany()?.id === DEMO_COMPANY_ID) {
      return companies().length > 0 ? 'viewing-demo' : 'onboarding'
    }
    return 'empty-data'
  })

  const switchView = (v: ViewKey) => {
    if (v === 'payroll') return // disabled until payroll endpoints exist
    setView(v)
  }

  // Company picker items: demo (always first) + user companies + "Add company".
  const pickerItems = createMemo(() => [
    { label: demoCompany.name, value: DEMO_COMPANY_ID, demo: true as const },
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
      if (next) switchView(next)
    }
    window.addEventListener('keydown', handler)
    onCleanup(() => window.removeEventListener('keydown', handler))
  })

  /**
   * Ensure a session exists before an ownership-needing action. Lazy: only
   * bootstraps the guest workspace here, never at app boot (temp-user spec
   * §7.2). Throws `NetworkError` when the API is unreachable — the caller
   * routes to the outbox.
   */
  const ensureSession = async (): Promise<void> => {
    if (getToken()) return // signed-in or restoring — use the token as-is
    const resp = await bootstrapGuest(ensureGuestId())
    setToken(resp.token)
    setSession({ status: 'signed-in', user: resp.user })
  }

  const addCompany = async (input: NewCompanyInput): Promise<void> => {
    // Guest mode is API-backed end-to-end (temp-user spec §7.4): the company
    // is created server-side (CH enrichment + the filings backfill job spawn
    // exactly like a signed-in user's) — never a localStorage row.
    if (!getToken()) {
      try {
        await ensureSession()
      } catch (e) {
        if (e instanceof NetworkError) {
          // Offline: queue the add; it syncs when we're back online.
          enqueueAdd(input)
          toaster.create({
            title: 'Saved offline',
            description: "You're offline — the company will sync when you're back online.",
            type: 'info',
          })
          return
        }
        throw e
      }
    }
    try {
      const saved = await createCompany({
        name: input.name,
        company_number: input.companyNumber,
        registration_date: input.registrationDate,
        accounting_standard: input.standard,
      })
      // Optimistic picker update; the session effect's refetch confirms it.
      setApiCompanies((list) => [saved, ...list.filter((c) => c.id !== saved.id)])
      setCompanyId(saved.id)
      toaster.create({
        title: 'Company added',
        description: 'Connect a bank or upload your ledger to start.',
        type: 'success',
      })
    } catch (e) {
      if (e instanceof NetworkError) {
        enqueueAdd(input)
        toaster.create({
          title: 'Saved offline',
          description: "You're offline — the company will sync when you're back online.",
          type: 'info',
        })
        return
      }
      throw e // the dialog surfaces ApiError toasts
    }
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
    toaster.create({
      title: `Connected ${bank.name}`,
      description: 'Mock — real Open Banking consent lands with the backend.',
      type: 'success',
    })
  }

  // Safety net: zero companies of any kind (demo retired + none added).
  if (!currentCompany()) {
    return (
      <div class={css({ h: '100dvh', display: 'flex', flexDirection: 'column', bg: 'canvas', color: 'fg.default', fontFamily: 'sans' })}>
        <div class={css({ flex: '1', minH: '0', display: 'grid', placeItems: 'center', px: '4' })}>
          <div class={css({ textAlign: 'center', maxW: '24rem' })}>
            <div class={css({ display: 'flex', justifyContent: 'center', mb: '4' })}>
              <LogoMark />
            </div>
            <h1 class={css({ textStyle: '2xl', fontWeight: '800', letterSpacing: '-0.02em' })}>Add your first company</h1>
            <p class={css({ textStyle: 'sm', color: 'fg.muted', mt: '2' })}>Tally needs a company before you can prepare accounts or file returns.</p>
            <div class={css({ mt: '5' })}>
              <Button onClick={() => setAddOpen(true)}>Add company</Button>
            </div>
            <Show when={sessionUser()}>
              <Button variant="plain" size="sm" onClick={() => void signOut()} class={css({ mt: '3', color: 'fg.muted' })}>
                <LogOut class={css({ w: '3.5', h: '3.5' })} /> Sign out
              </Button>
            </Show>
          </div>
        </div>

        <DevtoolsBanner />

        <AddCompanyDialog
          open={addOpen()}
          onOpenChange={setAddOpen}
          existingNumbers={companies().map((c) => c.companyNumber)}
          onAdd={addCompany}
        />
        {/* Keep auth reachable here too — a signed-in user who removed every
            company still needs Sign out. */}
        <SignInDialog
          open={signInOpen()}
          onOpenChange={setSignInOpen}
          defaultMode={isGuest() ? 'register' : 'login'}
        />
        <Toaster />
      </div>
    )
  }

  // Reactive getter — Solid JSX only updates when expressions read signals
  // directly, so the current company must be read via a function in JSX.
  const cd = () => currentCompany()!

  return (
    <div class={css({ h: '100dvh', display: 'flex', flexDirection: 'column', bg: 'canvas', color: 'fg.default', fontFamily: 'sans' })}>
      {/* Full-width demo-data banner: spans the whole screen, above the
          sidebar + main row (a top-of-app announcement bar). */}
      <Show when={banner()}>
        {(variant) => (
          <DemoBanner
            variant={variant()}
            viewCompanyName={companies()[0]?.name}
            onAddCompany={() => setAddOpen(true)}
            onViewCompany={() => companies()[0] && setCompanyId(companies()[0].id)}
            onConnectBank={() => switchView('integrations')}
          />
        )}
      </Show>
      <div class={css({ flex: '1', minH: '0', display: 'flex' })}>
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
                    <span class={css({ truncate: true })}>{cd().name}</span>                      <Show when={cd().id === DEMO_COMPANY_ID}>
                        <Badge variant="solid" class={css({ flexShrink: '0', fontSize: '10px', px: '1.5', py: '0' })}>
                          Demo
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
                              <Show when={'demo' in item && item.demo}>
                                <Badge variant="solid" class={css({ fontSize: '10px', px: '1.5', py: '0' })}>
                                  Demo
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
            {(item) => <NavButton item={item} active={view() === item.id} onClick={() => switchView(item.id)} />}
          </For>
          <div class={css({ flex: '1' })} />
          <div class={css({ borderTop: '1px solid {colors.border.subtle}', pt: '2', mt: '2', display: 'flex', flexDirection: 'column', gap: '0.5' })}>
            <For each={navBottom}>
              {(item) => <NavButton item={item} active={view() === item.id} onClick={() => switchView(item.id)} />}
            </For>
          </div>
        </nav>

        {/* ---------- Auth footer (§5.1) ---------- */}
        <div class={css({ borderTop: '1px solid {colors.border.subtle}', px: '3', py: '2.5', display: 'flex', flexDirection: 'column', gap: '2' })}>
          <Show when={session().status === 'restoring'}>
            <span class={css({ px: '2.5', fontSize: 'xs', color: 'fg.subtle' })}>Checking your session…</span>
          </Show>

          <Show when={sessionUser()} fallback={<Show when={session().status === 'local' || session().status === 'offline'}>
            <Button variant="subtle" justifyContent="flex-start" onClick={() => setSignInOpen(true)} class={css({ w: 'full', gap: '2' })}>
              <LogIn class={css({ w: '4', h: '4' })} /> Sign in
            </Button>
          </Show>}>
            {(user) => (
              <Show
                when={!user().is_temporary}
                fallback={
                  // Guest workspace (temp-user spec §7.7): the primary action
                  // is adopting it into an account — no avatar, no migration.
                  <>
                    <Button
                      onClick={() => setSignInOpen(true)}
                      class={css({ w: 'full', justifyContent: 'flex-start', gap: '2' })}
                    >
                      <UserRound class={css({ w: '4', h: '4' })} /> Save your work — create account
                    </Button>
                    <span class={css({ px: '2.5', fontSize: 'xs', color: 'fg.subtle' })}>
                      Guest workspace · stored on this browser
                    </span>
                    <Button variant="plain" size="sm" onClick={() => void signOut()} class={css({ w: 'full', justifyContent: 'flex-start', fontSize: 'xs', color: 'fg.muted' })}>
                      <LogOut class={css({ w: '3.5', h: '3.5' })} /> Sign out
                    </Button>
                  </>
                }
              >
                <div class={css({ display: 'flex', alignItems: 'center', gap: '2.5' })}>
                  <Avatar.Root class={css({ h: '8', w: '8', flexShrink: '0' })}>
                    <Avatar.Fallback name={user().display_name}>{initials(user().display_name)}</Avatar.Fallback>
                  </Avatar.Root>
                  <span class={css({ minW: '0', flex: '1' })}>
                    <span class={css({ display: 'block', fontSize: 'sm', fontWeight: '600', truncate: true })}>{user().display_name}</span>
                    <span class={css({ display: 'block', fontSize: 'xs', color: 'fg.subtle', truncate: true })}>{user().email}</span>
                  </span>
                </div>
                <Button variant="plain" size="sm" onClick={() => void signOut()} class={css({ w: 'full', justifyContent: 'flex-start', fontSize: 'xs', color: 'fg.muted' })}>
                  <LogOut class={css({ w: '3.5', h: '3.5' })} /> Sign out
                </Button>
              </Show>
            )}
          </Show>
        </div>
      </aside>

      {/* ---------- Main ---------- */}
      <main class={css({ flex: '1', minW: '0', overflowY: 'auto' })}>
        {/* Full-width banner: bleeds to the edges of the main column. */}
        <Show when={session().status === 'offline'}>
          <div
            class={css({
              display: 'flex',
              alignItems: 'center',
              gap: '2.5',
              px: '4',
              py: '2',
              bg: 'amber.solid.bg',
              color: 'amber.solid.fg',
              fontSize: 'sm',
              fontWeight: '500',
            })}
          >
            <WifiOff class={css({ w: '4', h: '4', flexShrink: '0' })} />
            <span class={css({ flex: '1' })}>API unreachable — is the API running?</span>
            <Button
              size="xs"
              variant="outline"
              onClick={() => void restoreSession()}
              class={css({ bg: 'transparent', _hover: { bg: 'white.a8' } })}
            >
              Retry
            </Button>
          </div>
        </Show>
        {/* The Filings view is full-bleed (its own sub-nav column sits flush
            against the main sidebar); the other views share the centred
            max-width container. */}
        <Show when={view() === 'filings'}>
          <FilingsView company={cd()} data={getCompanyData(cd().id)} />
        </Show>
        <Show when={view() !== 'filings'}>
          <div class={css({ maxW: '60rem', mx: 'auto', p: { base: '5', md: '8' } })}>
            <SolidSwitch>
              <Match when={view() === 'accounts'}>
                <AccountsView
                  company={cd()}
                  data={getCompanyData(cd().id)}
                  sources={sources()[cd().id] ?? []}
                  onGoToIntegrations={() => switchView('integrations')}
                />
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
        </Show>
      </main>
      </div>

      <DevtoolsBanner />

      {/* ---------- Add company (search) ---------- */}
      <AddCompanyDialog
        open={addOpen()}
        onOpenChange={setAddOpen}
        existingNumbers={companies().map((c) => c.companyNumber)}
        onAdd={addCompany}
      />

      {/* ---------- Sign in / create account ---------- */}
      <SignInDialog
        open={signInOpen()}
        onOpenChange={setSignInOpen}
        defaultMode={isGuest() ? 'register' : 'login'}
      />

      <Toaster />
    </div>
  )
}
