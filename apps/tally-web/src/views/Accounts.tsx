import { createMemo, createSignal, For, Show } from 'solid-js'
import { Button, Card, Input, Select, Tabs, Table, toaster } from '@tally/design-system'
import { createListCollection } from '@tally/design-system'
import { Download, Landmark, Plus, Search } from 'lucide-solid'
import { css, cx } from 'styled-system/css'
import { fmtDate, fmtMoney, fmtSignedMoney, type Company, type CompanyData, type DataSource } from '../mock_data'
import { DataSourceRows, EmptyState, numCell, PageHeader, StatCard, StatusBadge } from '../components/Shared'

const accountOptions = createListCollection({
  items: [
    { label: 'All accounts', value: 'all' },
    { label: 'Sales', value: 'Sales' },
    { label: 'Rent', value: 'Rent' },
    { label: 'Payroll', value: 'Payroll' },
    { label: 'Tax', value: 'Tax' },
    { label: 'Utilities', value: 'Utilities' },
    { label: 'Software', value: 'Software' },
    { label: 'Telecom', value: 'Telecom' },
    { label: 'Insurance', value: 'Insurance' },
    { label: 'Office', value: 'Office' },
  ],
})

export function AccountsView(props: {
  company: Company
  data: CompanyData
  sources: DataSource[]
  onGoToIntegrations: () => void
}) {
  const [query, setQuery] = createSignal('')
  const [account, setAccount] = createSignal('all')

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

  const yearIncome = props.data.summaries.reduce((s, m) => s + m.income, 0)
  const yearExpenses = props.data.summaries.reduce((s, m) => s + m.expenses, 0)
  const maxNet = Math.max(...props.data.summaries.map((m) => Math.abs(m.income - m.expenses)))

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

      <Tabs.Root defaultValue="transactions">
        <Tabs.List>
          <Tabs.Trigger value="transactions">Transactions</Tabs.Trigger>
          <Tabs.Trigger value="summaries">Summaries</Tabs.Trigger>
          <Tabs.Trigger value="sources">Data sources</Tabs.Trigger>
          <Tabs.Indicator />
        </Tabs.List>

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
              <Select.Root collection={accountOptions} defaultValue={['all']} onValueChange={(d) => setAccount(d.value[0])}>
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

        {/* ---------- Summaries ---------- */}
        <Tabs.Content value="summaries">
          <Show
            when={props.data.summaries.length > 0}
            fallback={
              <Card.Root>
                <EmptyState
                  title="No summaries yet"
                  description="Summaries build up once transactions start flowing."
                  action={<Button onClick={props.onGoToIntegrations}>Connect a bank</Button>}
                />
              </Card.Root>
            }
          >
            <div class={css({ display: 'grid', gap: '4', sm: { gridTemplateColumns: 'repeat(3, 1fr)' }, mb: '6' })}>
              <StatCard label="Income (YTD)" value={fmtMoney(yearIncome)} hint="12 months to August" tone="good" />
              <StatCard label="Expenses (YTD)" value={fmtMoney(yearExpenses)} hint="12 months to August" tone="bad" />
              <StatCard label="Net (YTD)" value={fmtMoney(yearIncome - yearExpenses)} hint="Before corporation tax" />
            </div>

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

            <Card.Root>
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
    </>
  )
}
