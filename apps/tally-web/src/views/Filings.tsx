import { createMemo, createSignal, For, Show } from 'solid-js'
import { Badge, Button, Card, Select, Table, toaster } from '@tally/design-system'
import { createListCollection } from '@tally/design-system'
import { ArrowUpRight, CalendarDays, Download, FileCheck2 } from 'lucide-solid'
import { css } from 'styled-system/css'
import { financialYears, fmtDate, DEMO_COMPANY_ID, type Company, type CompanyData } from '../mock_data'
import { EmptyState, numCell, PageHeader, DemoBadge, StatusBadge } from '../components/Shared'

const fyOptions = createListCollection({
  items: financialYears.map((fy) => ({ label: fy, value: fy })),
})

function daysTone(days: number) {
  if (days < 14) return 'red' as const
  if (days < 45) return 'amber' as const
  return 'green' as const
}

export function FilingsView(props: { company: Company; data: CompanyData }) {
  const [fy, setFy] = createSignal('FY2025/26')

  const next = () => props.data.nextFiling
  const previous = createMemo(() => props.data.previousFilings.filter((f) => f.period === fy()))

  return (
    <>
      <PageHeader
        title="Filings"
        description={`Companies House and HMRC deadlines for ${props.company.name}.`}
        badge={props.company.id === DEMO_COMPANY_ID ? <DemoBadge /> : undefined}
        actions={
          <Select.Root collection={fyOptions} defaultValue={['FY2025/26']} onValueChange={(d) => setFy(d.value[0])}>
            <Select.Control>
              <Select.Trigger>
                <Select.ValueText />
                <Select.Indicator />
              </Select.Trigger>
            </Select.Control>
            <Select.Positioner>
              <Select.Content>
                <For each={fyOptions.items}>
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
        }
      />

      {/* ---------- Next filing ---------- */}
      <Show
        when={next()}
        fallback={
          <Card.Root class={css({ mb: '6' })}>
            <EmptyState
              title="No filings yet"
              description="Prepare your first accounts once transactions start flowing into the books."
            />
          </Card.Root>
        }
      >
        {(nf) => (
          <Card.Root class={css({ mb: '6' })}>
            <div class={css({ display: 'flex', gap: '6', flexWrap: 'wrap', alignItems: 'flex-start', justifyContent: 'space-between', p: '5' })}>
              <div class={css({ minW: '0', flex: '1' })}>
                <div class={css({ display: 'flex', alignItems: 'center', gap: '2.5', mb: '2' })}>
                  <Badge variant="subtle" class={css({ gap: '1.5' })}>
                    <CalendarDays class={css({ w: '3', h: '3' })} /> Next filing
                  </Badge>
                  <StatusBadge status="due" tone={daysTone(nf().daysLeft)} label={`Due in ${nf().daysLeft} days`} />
                </div>
                <div class={css({ textStyle: 'lg', fontWeight: '700' })}>{nf().type}</div>
                <div class={css({ textStyle: 'sm', color: 'fg.muted', mt: '1' })}>
                  Period {fmtDate(nf().start)} – {fmtDate(nf().end)} · {nf().period} · due{' '}
                  <span class={css({ color: 'fg.default', fontWeight: '600' })}>{fmtDate(nf().due)}</span>
                </div>

                <div class={css({ mt: '4', maxW: '28rem' })}>
                  <div class={css({ display: 'flex', justifyContent: 'space-between', fontSize: 'xs', color: 'fg.muted', mb: '1.5' })}>
                    <span>Accounts prepared</span>
                    <span class={css({ fontVariantNumeric: 'tabular-nums' })}>{nf().progress}%</span>
                  </div>
                  <div class={css({ h: '1.5', borderRadius: 'full', bg: 'bg.subtle', overflow: 'hidden' })}>
                    <div
                      class={css({ h: 'full', borderRadius: 'full', bg: 'brown.solid.bg', transition: 'width 200ms ease' })}
                      style={{ width: `${nf().progress}%` }}
                    />
                  </div>
                </div>
              </div>

              <div class={css({ display: 'flex', gap: '2', flexDirection: 'column', alignItems: 'stretch', sm: { flexDirection: 'row', alignItems: 'center' } })}>
                <Button
                  variant="outline"
                  onClick={() => toaster.create({ title: 'Preview draft (mock)', description: 'iXBRL rendering lands with the backend.', type: 'info' })}
                >
                  <FileCheck2 class={css({ w: '3.5', h: '3.5' })} /> Preview
                </Button>
                <Button
                  onClick={() => toaster.create({ title: 'File now (mock)', description: 'Submitting to Companies House lands with the backend.', type: 'info' })}
                >
                  <ArrowUpRight class={css({ w: '3.5', h: '3.5' })} /> File now
                </Button>
              </div>
            </div>
          </Card.Root>
        )}
      </Show>

      {/* ---------- Previous filings ---------- */}
      <div class={css({ fontSize: 'sm', fontWeight: '600', mb: '3' })}>Previous filings — {fy()}</div>
      <Card.Root>
        <Show
          when={previous().length > 0}
          fallback={
            <EmptyState
              title={`Nothing filed for ${fy()} yet`}
              description={`Accounts for ${fy()} have not been filed. Prepare a draft from the next-filing card.`}
            />
          }
        >
          <Table.Root>
            <Table.Head>
              <Table.Row>
                <Table.Header>Period</Table.Header>
                <Table.Header>Return</Table.Header>
                <Table.Header>Filed</Table.Header>
                <Table.Header>Status</Table.Header>
                <Table.Header textAlign="right">Actions</Table.Header>
              </Table.Row>
            </Table.Head>
            <Table.Body>
              <For each={previous()}>
                {(f) => (
                  <Table.Row>
                    <Table.Cell class={numCell}>{f.period}</Table.Cell>
                    <Table.Cell>{f.type}</Table.Cell>
                    <Table.Cell class={numCell}>{fmtDate(f.filed)}</Table.Cell>
                    <Table.Cell>
                      <StatusBadge status={f.status} />
                    </Table.Cell>
                    <Table.Cell textAlign="right">
                      <Button
                        size="2xs"
                        variant="plain"
                        onClick={() => toaster.create({ title: `Download ${f.period} (mock)`, description: 'Stored returns land with the backend.', type: 'info' })}
                      >
                        <Download class={css({ w: '3', h: '3' })} /> Download
                      </Button>
                    </Table.Cell>
                  </Table.Row>
                )}
              </For>
            </Table.Body>
          </Table.Root>
        </Show>
      </Card.Root>
    </>
  )
}
