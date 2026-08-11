import { createSignal, For, Show } from 'solid-js'
import { Button, Dialog, toaster } from '@tally/design-system'
import { Landmark, Plus, X } from 'lucide-solid'
import { css } from 'styled-system/css'
import { button } from 'styled-system/recipes'
import { bankOptions, dataSources, type Company, type DataSource } from '../mock_data'
import { DataSourceRows, PageHeader, StatusBadge } from '../components/Shared'

export function IntegrationsView(props: { company: Company }) {
  const [addOpen, setAddOpen] = createSignal(false)
  const [extra, setExtra] = createSignal<DataSource[]>([])

  const sources = () => [...dataSources, ...extra()]

  const connect = (bank: (typeof bankOptions)[number]) => {
    if (sources().some((s) => s.id === bank.id)) {
      toaster.create({ title: `${bank.name} is already connected`, type: 'info' })
      return
    }
    setExtra((xs) => [
      ...xs,
      { id: bank.id, name: `${bank.name} Business`, kind: 'bank', institution: bank.name, status: 'pending', lastSync: '—', accountCount: 0 },
    ])
    toaster.create({
      title: `Connection started with ${bank.name}`,
      description: 'Open Banking consent flow lands with the backend.',
      type: 'success',
    })
    setAddOpen(false)
  }

  return (
    <>
      <PageHeader
        title="Integrations"
        description={`Data sources feeding ${props.company.name}'s books.`}
        actions={
          <Button onClick={() => setAddOpen(true)}>
            <Plus class={css({ w: '3.5', h: '3.5' })} /> Add bank account
          </Button>
        }
      />

      <DataSourceRows
        sources={sources}
        onSync={(ds) =>
          toaster.create({ title: `Syncing ${ds.name}…`, description: 'A real Open Banking fetch lands with the backend.', type: 'info' })
        }
        footer={
          <div class={css({ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '3', flexWrap: 'wrap' })}>
            <span class={css({ textStyle: 'xs', color: 'fg.subtle' })}>
              {sources().filter((s) => s.status === 'connected').length} connected · CSV import and HMRC MTD arrive with the backend.
            </span>
          </div>
        }
      />

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
                    const existing = sources().find((s) => s.id === bank.id)
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
                          fallback={
                            <StatusBadge
                              status={existing?.status ?? 'pending'}
                              label={existing?.status === 'connected' ? 'Connected' : 'Pending'}
                            />
                          }
                        >
                          <Button size="xs" variant="outline" onClick={() => connect(bank)}>
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
