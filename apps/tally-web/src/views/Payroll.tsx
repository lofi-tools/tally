import { For } from 'solid-js'
import { Avatar, Button, Card, Table, toaster } from '@tally/design-system'
import { CalendarDays, Play } from 'lucide-solid'
import { css } from 'styled-system/css'
import { employees, fmtDate, fmtMoney, payroll, payrollHistory, type Company } from '../mock_data'
import { numCell, PageHeader, StatCard, StatusBadge } from '../components/Shared'

export function PayrollView(props: { company: Company }) {
  return (
    <>
      <PageHeader
        title="Payroll"
        description={`Employee payroll for ${props.company.name}. RTI submissions land with the backend.`}
        actions={
          <Button
            onClick={() => toaster.create({ title: 'Run payroll (mock)', description: 'Submitting RTI to HMRC lands with the backend.', type: 'info' })}
          >
            <Play class={css({ w: '3.5', h: '3.5' })} /> Run payroll
          </Button>
        }
      />

      <div class={css({ display: 'grid', gap: '4', sm: { gridTemplateColumns: 'repeat(3, 1fr)' }, mb: '6' })}>
        <StatCard label="Next run" value={fmtDate(payroll.nextRun)} hint={`${payroll.frequency} · ${employees.length} employees`} />
        <StatCard label="Net pay per run" value={fmtMoney(payroll.netPerRun)} hint={`Gross ${fmtMoney(payroll.grossPerRun)}`} />
        <StatCard label="Employer NI (YTD)" value={fmtMoney(payroll.employerNi)} hint="Per run" />
      </div>

      <div class={css({ display: 'grid', gap: '6', lg: { gridTemplateColumns: 'repeat(2, 1fr)' } })}>
        {/* ---------- Employees ---------- */}
        <Card.Root>
          <div class={css({ px: '4', pt: '4', pb: '2', display: 'flex', alignItems: 'center', justifyContent: 'space-between' })}>
            <div class={css({ fontSize: 'sm', fontWeight: '600' })}>Employees</div>
            <div class={css({ fontSize: 'xs', color: 'fg.subtle' })}>{employees.length} active</div>
          </div>
          <Table.Root>
            <Table.Head>
              <Table.Row>
                <Table.Header>Name</Table.Header>
                <Table.Header>Role</Table.Header>
                <Table.Header textAlign="right">Amount</Table.Header>
                <Table.Header textAlign="right">YTD</Table.Header>
              </Table.Row>
            </Table.Head>
            <Table.Body>
              <For each={employees}>
                {(e) => (
                  <Table.Row>
                    <Table.Cell>
                      <span class={css({ display: 'inline-flex', alignItems: 'center', gap: '2.5' })}>
                        <Avatar.Root class={css({ h: '6', w: '6' })}>
                          <Avatar.Fallback name={e.name} class={css({ fontSize: 'xs' })} />
                        </Avatar.Root>
                        <span class={css({ fontWeight: '600' })}>{e.name}</span>
                      </span>
                    </Table.Cell>
                    <Table.Cell class={css({ color: 'fg.muted', fontSize: 'sm' })}>{e.role}</Table.Cell>
                    <Table.Cell textAlign="right" class={numCell}>
                      {fmtMoney(e.amount)}
                      <span class={css({ fontSize: 'xs', color: 'fg.subtle' })}>/{e.basis === 'annual' ? 'yr' : 'mo'}</span>
                    </Table.Cell>
                    <Table.Cell textAlign="right" class={numCell}>
                      {fmtMoney(e.ytd)}
                    </Table.Cell>
                  </Table.Row>
                )}
              </For>
            </Table.Body>
          </Table.Root>
        </Card.Root>

        {/* ---------- History ---------- */}
        <Card.Root>
          <div class={css({ px: '4', pt: '4', pb: '2', display: 'flex', alignItems: 'center', justifyContent: 'space-between' })}>
            <div class={css({ fontSize: 'sm', fontWeight: '600' })}>Run history</div>
            <div class={css({ fontSize: 'xs', color: 'fg.subtle' })}>Last 3 runs</div>
          </div>
          <Table.Root>
            <Table.Head>
              <Table.Row>
                <Table.Header>Period</Table.Header>
                <Table.Header textAlign="right">Gross</Table.Header>
                <Table.Header textAlign="right">PAYE</Table.Header>
                <Table.Header textAlign="right">NI</Table.Header>
                <Table.Header>Status</Table.Header>
              </Table.Row>
            </Table.Head>
            <Table.Body>
              <For each={payrollHistory}>
                {(r) => (
                  <Table.Row>
                    <Table.Cell>
                      <span class={css({ display: 'flex', alignItems: 'center', gap: '2' })}>
                        <CalendarDays class={css({ w: '3.5', h: '3.5', color: 'fg.subtle' })} />
                        {r.period}
                      </span>
                    </Table.Cell>
                    <Table.Cell textAlign="right" class={numCell}>
                      {fmtMoney(r.gross)}
                    </Table.Cell>
                    <Table.Cell textAlign="right" class={numCell}>
                      {fmtMoney(r.tax)}
                    </Table.Cell>
                    <Table.Cell textAlign="right" class={numCell}>
                      {fmtMoney(r.ni)}
                    </Table.Cell>
                    <Table.Cell>
                      <StatusBadge status={r.status} label="RTI filed" />
                    </Table.Cell>
                  </Table.Row>
                )}
              </For>
            </Table.Body>
          </Table.Root>
        </Card.Root>
      </div>
    </>
  )
}
