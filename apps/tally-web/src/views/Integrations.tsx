import { createSignal, For, Show } from 'solid-js'
import { Button, Card, Dialog, toaster } from '@tally/design-system'
import { FileUp, Landmark, Plus, X } from 'lucide-solid'
import { css } from 'styled-system/css'
import { button } from 'styled-system/recipes'
import { bankOptions, SAMPLE_COMPANY_ID, type Company, type DataSource } from '../mock_data'
import { DataSourceRows, EmptyState, PageHeader, SampleBadge, StatusBadge } from '../components/Shared'

export function IntegrationsView(props: {
  company: Company
  sources: DataSource[]
  onConnect: (bank: (typeof bankOptions)[number]) => void
}) {
  const [addOpen, setAddOpen] = createSignal(false)

  return (
    <>
      <PageHeader
        title="Integrations"
        description={`Data sources feeding ${props.company.name}'s books.`}
        badge={props.company.id === SAMPLE_COMPANY_ID ? <SampleBadge /> : undefined}
        actions={
          <Button onClick={() => setAddOpen(true)}>
            <Plus class={css({ w: '3.5', h: '3.5' })} /> Add bank account
          </Button>
        }
      />

      <Show
        when={props.sources.length > 0}
        fallback={
          <Card.Root>
            <EmptyState
              icon={<Landmark class={css({ w: '6', h: '6' })} />}
              title="No data sources yet"
              description="Connect a bank or upload a GnuCash ledger to pull in transactions."
              action={
                <>
                  <Button onClick={() => setAddOpen(true)}>
                    <Plus class={css({ w: '3.5', h: '3.5' })} /> Connect a bank
                  </Button>
                  <Button
                    variant="outline"
                    onClick={() =>
                      toaster.create({ title: 'Upload ledger (mock)', description: 'GnuCash / CSV import arrives with the backend.', type: 'info' })
                    }
                  >
                    <FileUp class={css({ w: '3.5', h: '3.5' })} /> Upload a GnuCash ledger
                  </Button>
                </>
              }
            />
          </Card.Root>
        }
      >
        <DataSourceRows
          sources={() => props.sources}
          onSync={(ds) =>
            toaster.create({ title: `Syncing ${ds.name}…`, description: 'A real Open Banking fetch lands with the backend.', type: 'info' })
          }
          footer={
            <div class={css({ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '3', flexWrap: 'wrap' })}>
              <span class={css({ textStyle: 'xs', color: 'fg.subtle' })}>
                {props.sources.filter((s) => s.status === 'connected').length} connected · CSV import and HMRC MTD arrive with the backend.
              </span>
            </div>
          }
        />
      </Show>

      {/* ---------- Add bank account ---------- */}
      <Dialog.Root open={addOpen()} onOpenChange={(d) => setAddOpen(d.open)}>
        <Dialog.Backdrop />
        <Dialog.Positioner>
          <Dialog.Content>
            <Dialog.CloseTrigger>
              <X />
            </Dialog.CloseTrigger>
            <Dialog.Header>
              <Dialog.Title>Add bank account</Dialog.Title>
              <Dialog.Description>
                Choose a bank to pull transactions from via Open Banking (mock — consent flow lands with the backend).
              </Dialog.Description>
            </Dialog.Header>
            <Dialog.Body>
              <div class={css({ display: 'flex', flexDirection: 'column' })}>
                <For each={bankOptions}>
                  {(bank) => {
                    const existing = props.sources.find((s) => s.id === bank.id)
                    return (
                      <div
                        class={css({
                          display: 'flex',
                          alignItems: 'center',
                          gap: '3',
                          px: '2',
                          py: '2.5',
                          borderRadius: 'md',
                          _hover: { bg: 'bg.subtle' },
                        })}
                      >
                        <span
                          class={css({
                            h: '8',
                            w: '8',
                            borderRadius: 'md',
                            bg: 'bg.subtle',
                            border: '1px solid {colors.border}',
                            display: 'grid',
                            placeItems: 'center',
                            color: 'fg.muted',
                            flexShrink: '0',
                          })}
                        >
                          <Landmark class={css({ w: '3.5', h: '3.5' })} />
                        </span>
                        <span class={css({ flex: '1', fontSize: 'sm', fontWeight: '600' })}>{bank.name}</span>
                        <Show
                          when={!existing}
                          fallback={<StatusBadge status={existing?.status ?? 'pending'} label={existing?.status === 'connected' ? 'Connected' : 'Pending'} />}
                        >
                          <Button size="xs" variant="outline" onClick={() => props.onConnect(bank)}>
                            Connect
                          </Button>
                        </Show>
                      </div>
                    )
                  }}
                </For>
              </div>
            </Dialog.Body>
            <Dialog.Footer>
              <Dialog.ActionTrigger class={button({ variant: 'outline' })}>Cancel</Dialog.ActionTrigger>
            </Dialog.Footer>
          </Dialog.Content>
        </Dialog.Positioner>
      </Dialog.Root>
    </>
  )
}
