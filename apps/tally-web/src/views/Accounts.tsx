import { createMemo, createSignal, For, Show } from 'solid-js'
import { Button, Card, Collapsible, Drawer, Input, Select, Tabs, Table, toaster } from '@tally/design-system'
import { createListCollection } from '@tally/design-system'
import { ArrowRight, ChevronDown, Download, Landmark, Plus, Search, X } from 'lucide-solid'
import { css, cx } from 'styled-system/css'
import {
  accountBalance,
  chartAccountNames,
  chartOfAccounts,
  fmtDate,
  fmtMoney,
  fmtSignedMoney,
  groupBalance,
  transactionsFor,
  type AccountGroup,
  type Company,
  type CompanyData,
  type DataSource,
} from '../mock_data'
import { DataSourceRows, EmptyState, numCell, PageHeader, StatCard, StatusBadge } from '../components/Shared'

// Filter dropdown derived from the chart of accounts (kept in sync with the tree).
const accountOptions = createListCollection({
  items: [
    { label: 'All accounts', value: 'all' },
    ...chartAccountNames().map((name) => ({ label: name, value: name })),
  ],
})

/** App-convention balance colour: positive green, negative red, zero muted. */
const balanceFg = (n: number) => (n > 0 ? 'green.plain.fg' : n < 0 ? 'red.plain.fg' : 'fg.muted')
/** Zero renders unsigned; everything else signed (e.g. +£45,000.00 / −£1,850.00). */
const balanceText = (n: number) => (n === 0 ? fmtMoney(n) : fmtSignedMoney(n))

export function AccountsView(props: {
  company: Company
  data: CompanyData
  sources: DataSource[]
  onGoToIntegrations: () => void
}) {
  const [tab, setTab] = createSignal('balances')
  const [query, setQuery] = createSignal('')
  const [account, setAccount] = createSignal('all')
  // Leaf account currently shown in the right-side drawer; null = closed.
  const [openAccount, setOpenAccount] = createSignal<string | null>(null)

  const filtered = createMemo(() => {
    const q = query().trim().toLowerCase()
    return props.data.transactions.filter(
      (t) =>
        (account() === 'all' || t.account === account()) &&
        (q === '' ||
          t.description.toLowerCase().includes(q) ||
          t.account.toLowerCase().includes(q) ||
          t.source.toLowerCase().includes(q)),
    )
  })

  const recent = createMemo(() =>
    [...props.data.transactions].sort((a, b) => b.date.localeCompare(a.date)).slice(0, 5),
  )

  const acctTxs = createMemo(() => (openAccount() ? transactionsFor(openAccount()!) : []))
  const acctBalance = () => (openAccount() ? accountBalance(openAccount()!) : 0)

  const yearIncome = props.data.summaries.reduce((s, m) => s + m.income, 0)
  const yearExpenses = props.data.summaries.reduce((s, m) => s + m.expenses, 0)
  const maxNet = Math.max(...props.data.summaries.map((m) => Math.abs(m.income - m.expenses)))

  const hasData = () => props.data.transactions.length > 0 || props.data.summaries.length > 0

  // "All transactions →": jump to the full list with every filter cleared.
  const onAllTransactions = () => {
    setQuery('')
    setAccount('all')
    setTab('transactions')
  }

  return (
    <>
      <PageHeader
        title="Accounts"
        description={`Transactions and summaries for ${props.company.name}.`}
        actions={
          <>
            <Button
              variant="outline"
              onClick={() => toaster.create({ title: 'Export (mock)', description: 'CSV / MTD export lands with the backend.', type: 'info' })}
            >
              <Download class={css({ w: '3.5', h: '3.5' })} /> Export
            </Button>
            <Button onClick={() => toaster.create({ title: 'Add transaction (mock)', description: 'Manual entry needs the backend.', type: 'info' })}>
              <Plus class={css({ w: '3.5', h: '3.5' })} /> Add transaction
            </Button>
          </>
        }
      />

      <Tabs.Root value={tab()} onValueChange={(d) => setTab(d.value)}>
        <Tabs.List>
          <Tabs.Trigger value="balances">Balances</Tabs.Trigger>
          <Tabs.Trigger value="transactions">Transactions</Tabs.Trigger>
          <Tabs.Trigger value="sources">Data sources</Tabs.Trigger>
          <Tabs.Indicator />
        </Tabs.List>

        {/* ---------- Balances ---------- */}
        <Tabs.Content value="balances">
          <Show
            when={hasData()}
            fallback={
              <Card.Root>
                <EmptyState
                  icon={<Landmark class={css({ w: '6', h: '6' })} />}
                  title="No data yet"
                  description="Connect a bank or upload a ledger to populate your books."
                  action={<Button onClick={props.onGoToIntegrations}>Connect a bank</Button>}
                />
              </Card.Root>
            }
          >
            {/* Income / expenses / net */}
            <div class={css({ display: 'grid', gap: '4', sm: { gridTemplateColumns: 'repeat(3, 1fr)' }, mb: '6' })}>
              <StatCard label="Income (YTD)" value={fmtMoney(yearIncome)} hint="12 months to August" tone="good" />
              <StatCard label="Expenses (YTD)" value={fmtMoney(yearExpenses)} hint="12 months to August" tone="bad" />
              <StatCard label="Net (YTD)" value={fmtMoney(yearIncome - yearExpenses)} hint="Before corporation tax" />
            </div>

            {/* Chart of accounts tree */}
            <Card.Root class={css({ mb: '6' })}>
              <div class={css({ px: '4', pt: '4', pb: '1' })}>
                <div class={css({ fontSize: 'sm', fontWeight: '600' })}>Chart of accounts</div>
              </div>
              <div class={css({ px: '2', pb: '2' })}>
                <For each={chartOfAccounts}>
                  {(group) => <AccountTreeGroup group={group} onOpenAccount={setOpenAccount} />}
                </For>
              </div>
            </Card.Root>

            {/* Recent transactions */}
            <Card.Root>
              <div class={css({ px: '4', pt: '4', pb: '1' })}>
                <div class={css({ fontSize: 'sm', fontWeight: '600' })}>Recent transactions</div>
              </div>
              <For each={recent()}>
                {(t, i) => (
                  <div
                    class={css({
                      display: 'flex',
                      alignItems: 'center',
                      gap: '3',
                      px: '4',
                      py: '2.5',
                      borderTop: i() === 0 ? 'none' : '1px solid {colors.border}',
                      _hover: { bg: 'bg.subtle' },
                      transition: 'background-color 120ms ease',
                    })}
                  >
                    <span class={css({ fontSize: 'xs', color: 'fg.muted', w: '7rem', flexShrink: '0' })}>{fmtDate(t.date)}</span>
                    <span class={css({ flex: '1', minW: '0', fontSize: 'sm', truncate: true })}>{t.description}</span>
                    <span class={cx(numCell, css({ color: balanceFg(t.amount) }))}>{fmtSignedMoney(t.amount)}</span>
                  </div>
                )}
              </For>
              <button
                type="button"
                onClick={onAllTransactions}
                class={css({
                  w: 'full',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  gap: '1.5',
                  px: '4',
                  py: '2.5',
                  border: 'none',
                  borderTop: '1px solid {colors.border}',
                  fontSize: 'sm',
                  fontWeight: '500',
                  color: 'brown.11',
                  bg: 'transparent',
                  cursor: 'pointer',
                  _hover: { bg: 'bg.subtle' },
                  transition: 'background-color 120ms ease',
                })}
              >
                All transactions <ArrowRight class={css({ w: '3.5', h: '3.5' })} />
              </button>
            </Card.Root>
          </Show>
        </Tabs.Content>

        {/* ---------- Transactions ---------- */}
        <Tabs.Content value="transactions">
          <Show
            when={props.data.transactions.length > 0}
            fallback={
              <Card.Root>
                <EmptyState
                  icon={<Landmark class={css({ w: '6', h: '6' })} />}
                  title="No transactions yet"
                  description="Connect a bank or upload a ledger to populate your books."
                  action={<Button onClick={props.onGoToIntegrations}>Connect a bank</Button>}
                />
              </Card.Root>
            }
          >
            <Show when={props.data.summaries.length > 0}>
              {/* Net by month */}
              <Card.Root class={css({ mb: '6' })}>
                <div class={css({ px: '4', pt: '4', pb: '6' })}>
                  <div class={css({ fontSize: 'sm', fontWeight: '600' })}>Net by month</div>
                  <div class={css({ display: 'flex', gap: '3', alignItems: 'flex-end', h: '32', mt: '6' })}>
                    <For each={props.data.summaries}>
                      {(m) => {
                        const net = m.income - m.expenses
                        const h = `${Math.max(6, (Math.abs(net) / maxNet) * 100)}%`
                        return (
                          <div
                            class={css({
                              flex: '1',
                              display: 'flex',
                              flexDirection: 'column',
                              gap: '1.5',
                              alignItems: 'center',
                              justifyContent: 'flex-end',
                              h: 'full',
                            })}
                          >
                            <span class={cx(numCell, css({ fontSize: 'xs', color: 'fg.muted' }))}>{fmtMoney(net)}</span>
                            <div class={css({ w: 'full', h: '16', bg: 'bg.subtle', borderRadius: 'sm', display: 'flex', alignItems: 'flex-end', overflow: 'hidden' })}>
                              <div
                                style={{ height: h }}
                                class={css({
                                  w: 'full',
                                  borderRadius: 'sm',
                                  bg: net >= 0 ? 'green.solid.bg' : 'red.solid.bg',
                                  transition: 'height 200ms ease',
                                })}
                              />
                            </div>
                            <span class={css({ fontSize: 'xs', color: 'fg.subtle' })}>{m.month}</span>
                          </div>
                        )
                      }}
                    </For>
                  </div>
                </div>
              </Card.Root>

              {/* Monthly summary table */}
              <Card.Root class={css({ mb: '6' })}>
                <Table.Root>
                  <Table.Head>
                    <Table.Row>
                      <Table.Header>Month</Table.Header>
                      <Table.Header textAlign="right">Income</Table.Header>
                      <Table.Header textAlign="right">Expenses</Table.Header>
                      <Table.Header textAlign="right">VAT</Table.Header>
                      <Table.Header textAlign="right">Net</Table.Header>
                    </Table.Row>
                  </Table.Head>
                  <Table.Body>
                    <For each={props.data.summaries}>
                      {(m) => {
                        const net = m.income - m.expenses
                        return (
                          <Table.Row>
                            <Table.Cell>{m.month}</Table.Cell>
                            <Table.Cell textAlign="right" class={numCell}>
                              {fmtMoney(m.income)}
                            </Table.Cell>
                            <Table.Cell textAlign="right" class={numCell}>
                              {fmtMoney(m.expenses)}
                            </Table.Cell>
                            <Table.Cell textAlign="right" class={numCell}>
                              {fmtMoney(m.vat)}
                            </Table.Cell>
                            <Table.Cell textAlign="right" class={cx(numCell, css({ color: net >= 0 ? 'green.plain.fg' : 'red.plain.fg' }))}>
                              {fmtSignedMoney(net)}
                            </Table.Cell>
                          </Table.Row>
                        )
                      }}
                    </For>
                  </Table.Body>
                </Table.Root>
              </Card.Root>
            </Show>

            <div class={css({ display: 'flex', gap: '3', mb: '4', flexWrap: 'wrap' })}>
              <div class={css({ position: 'relative', flex: '1', minW: '16rem', maxW: '26rem' })}>
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
                  placeholder="Search transactions…"
                  value={query()}
                  onInput={(e) => setQuery(e.currentTarget.value)}
                  class={css({ pl: '9' })}
                />
              </div>
              {/* Controlled: 'All transactions →' on Balances resets this via setAccount. */}
              <Select.Root collection={accountOptions} value={[account()]} onValueChange={(d) => setAccount(d.value[0])}>
                <Select.Control>
                  <Select.Trigger>
                    <Select.ValueText />
                    <Select.Indicator />
                  </Select.Trigger>
                </Select.Control>
                <Select.Positioner>
                  <Select.Content>
                    <For each={accountOptions.items}>
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

            <Card.Root>
              <Show when={filtered().length > 0} fallback={<EmptyState title="No transactions match" description="Try clearing the search or picking a different account." />}>
                <Table.Root>
                  <Table.Head>
                    <Table.Row>
                      <Table.Header>Date</Table.Header>
                      <Table.Header>Description</Table.Header>
                      <Table.Header>Account</Table.Header>
                      <Table.Header>Source</Table.Header>
                      <Table.Header textAlign="right">Amount</Table.Header>
                      <Table.Header>Status</Table.Header>
                    </Table.Row>
                  </Table.Head>
                  <Table.Body>
                    <For each={filtered()}>
                      {(t) => (
                        <Table.Row>
                          <Table.Cell class={numCell}>{fmtDate(t.date)}</Table.Cell>
                          <Table.Cell class={css({ maxW: 'md', truncate: true })}>{t.description}</Table.Cell>
                          <Table.Cell class={css({ color: 'fg.muted', fontSize: 'sm' })}>{t.account}</Table.Cell>
                          <Table.Cell class={css({ color: 'fg.muted', fontSize: 'sm' })}>{t.source}</Table.Cell>
                          <Table.Cell
                            textAlign="right"
                            class={cx(numCell, css(t.amount > 0 ? { color: 'green.plain.fg' } : { color: 'fg.default' }))}
                          >
                            {fmtSignedMoney(t.amount)}
                          </Table.Cell>
                          <Table.Cell>
                            <StatusBadge status={t.status} />
                          </Table.Cell>
                        </Table.Row>
                      )}
                    </For>
                  </Table.Body>
                </Table.Root>
              </Show>
            </Card.Root>
          </Show>
        </Tabs.Content>

        {/* ---------- Data sources ---------- */}
        <Tabs.Content value="sources">
          <Show
            when={props.sources.length > 0}
            fallback={
              <Card.Root>
                <EmptyState
                  icon={<Landmark class={css({ w: '6', h: '6' })} />}
                  title="No data sources yet"
                  description="Connect a bank or upload a ledger to pull transactions into your books."
                  action={<Button onClick={props.onGoToIntegrations}>Connect a bank</Button>}
                />
              </Card.Root>
            }
          >
            <DataSourceRows
              sources={() => props.sources}
              onSync={(ds) =>
                toaster.create({ title: `Syncing ${ds.name}…`, description: 'A real Open Banking fetch lands with the backend.', type: 'info' })
              }
            />
          </Show>
          <p class={css({ textStyle: 'xs', color: 'fg.subtle', mt: '3' })}>
            Transactions are pulled from your connected data sources. CSV import and Open Banking arrive with the backend.
          </p>
        </Tabs.Content>
      </Tabs.Root>

      {/* ---------- Account drill-down drawer (right side) ---------- */}
      <Drawer.Root open={openAccount() !== null} onOpenChange={(d) => !d.open && setOpenAccount(null)} placement="end" size="lg">
        <Drawer.Backdrop />
        <Drawer.Positioner>
          <Drawer.Content>
            <Drawer.CloseTrigger aria-label="Close">
              <X class={css({ w: '4', h: '4' })} />
            </Drawer.CloseTrigger>
            <Drawer.Header>
              <Drawer.Title>{openAccount()}</Drawer.Title>
              <Drawer.Description class={css({ display: 'flex', alignItems: 'center', gap: '2' })}>
                Balance
                <span class={cx(numCell, css({ color: balanceFg(acctBalance()) }))}>{balanceText(acctBalance())}</span>
              </Drawer.Description>
            </Drawer.Header>
            <Drawer.Body>
              <Show
                when={acctTxs().length > 0}
                fallback={<EmptyState title="No transactions" description="This account has no activity yet." />}
              >
                <Table.Root>
                  <Table.Head>
                    <Table.Row>
                      <Table.Header>Date</Table.Header>
                      <Table.Header>Description</Table.Header>
                      <Table.Header>Source</Table.Header>
                      <Table.Header textAlign="right">Amount</Table.Header>
                      <Table.Header>Status</Table.Header>
                    </Table.Row>
                  </Table.Head>
                  <Table.Body>
                    <For each={acctTxs()}>
                      {(t) => (
                        <Table.Row>
                          <Table.Cell class={numCell}>{fmtDate(t.date)}</Table.Cell>
                          <Table.Cell class={css({ maxW: 'md', truncate: true })}>{t.description}</Table.Cell>
                          <Table.Cell class={css({ color: 'fg.muted', fontSize: 'sm' })}>{t.source}</Table.Cell>
                          <Table.Cell
                            textAlign="right"
                            class={cx(numCell, css(t.amount > 0 ? { color: 'green.plain.fg' } : { color: 'fg.default' }))}
                          >
                            {fmtSignedMoney(t.amount)}
                          </Table.Cell>
                          <Table.Cell>
                            <StatusBadge status={t.status} />
                          </Table.Cell>
                        </Table.Row>
                      )}
                    </For>
                  </Table.Body>
                </Table.Root>
              </Show>
            </Drawer.Body>
          </Drawer.Content>
        </Drawer.Positioner>
      </Drawer.Root>
    </>
  )
}

/** One collapsible top-level group in the chart of accounts tree. */
function AccountTreeGroup(props: { group: AccountGroup; onOpenAccount: (name: string) => void }) {
  const [open, setOpen] = createSignal(true)
  const total = () => groupBalance(props.group)
  return (
    <Collapsible.Root open={open()} onOpenChange={(d) => setOpen(d.open)}>
      <Collapsible.Trigger
        class={css({
          w: 'full',
          display: 'flex',
          alignItems: 'center',
          gap: '2',
          px: '2',
          py: '2',
          borderRadius: 'md',
          bg: 'transparent',
          border: 'none',
          cursor: 'pointer',
          textAlign: 'left',
          fontSize: 'sm',
          fontWeight: '600',
          color: 'fg.default',
          _hover: { bg: 'bg.subtle' },
          transition: 'background-color 120ms ease',
        })}
      >
        <ChevronDown
          class={css({
            w: '3.5',
            h: '3.5',
            flexShrink: '0',
            color: 'fg.muted',
            transition: 'transform 150ms ease',
            transform: open() ? 'rotate(0deg)' : 'rotate(-90deg)',
          })}
        />
        <span class={css({ flex: '1', minW: '0', truncate: true })}>{props.group.name}</span>
        <span class={cx(numCell, css({ color: balanceFg(total()) }))}>{balanceText(total())}</span>
      </Collapsible.Trigger>
      <Collapsible.Content>
        <For each={props.group.accounts}>
          {(a) => {
            const bal = accountBalance(a.name)
            return (
              <button
                type="button"
                onClick={() => props.onOpenAccount(a.name)}
                class={css({
                  w: 'full',
                  display: 'flex',
                  alignItems: 'center',
                  gap: '2',
                  pl: '9',
                  pr: '2',
                  py: '1.5',
                  borderRadius: 'md',
                  bg: 'transparent',
                  border: 'none',
                  cursor: 'pointer',
                  textAlign: 'left',
                  fontSize: 'sm',
                  color: 'fg.default',
                  _hover: { bg: 'bg.subtle' },
                  transition: 'background-color 120ms ease',
                })}
              >
                <span class={css({ flex: '1', minW: '0', truncate: true })}>{a.name}</span>
                <span class={cx(numCell, css({ color: balanceFg(bal) }))}>{balanceText(bal)}</span>
              </button>
            )
          }}
        </For>
      </Collapsible.Content>
    </Collapsible.Root>
  )
}
