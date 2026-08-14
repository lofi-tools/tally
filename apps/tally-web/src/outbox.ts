// Offline outbox for company adds (temp-user spec §7.5).
//
// When the API is unreachable, adds are queued here (`tally.outbox.v1`, an
// array of `NewCompanyInput`) and replayed in order once a session is live
// again (guest or, after adoption, the upgraded account). A successful add
// removes its entry; a hard failure (e.g. `duplicate_company`) keeps it and
// stops the replay so the user can sort it out.

import { toaster } from '@tally/design-system'
import { createCompany, NetworkError, type CompanyInput } from './api'
import type { NewCompanyInput } from './components/AddCompanyDialog'

export const OUTBOX_KEY = 'tally.outbox.v1'

/** The create-body shape for an add (the same fields the dialog collects). */
export function toCompanyInput(input: NewCompanyInput): CompanyInput {
  return {
    name: input.name,
    company_number: input.companyNumber,
    registration_date: input.registrationDate,
    accounting_standard: input.standard,
  }
}

export function listOutbox(): NewCompanyInput[] {
  try {
    const raw = localStorage.getItem(OUTBOX_KEY)
    const parsed = raw ? (JSON.parse(raw) as NewCompanyInput[]) : []
    return Array.isArray(parsed) ? parsed : []
  } catch {
    return []
  }
}

function saveOutbox(queue: NewCompanyInput[]): void {
  try {
    localStorage.setItem(OUTBOX_KEY, JSON.stringify(queue))
  } catch {
    // Storage unavailable — the queue lives for this page load only.
  }
}

/** Queue an add made while offline. */
export function enqueueAdd(input: NewCompanyInput): void {
  const queue = listOutbox()
  queue.push(input)
  saveOutbox(queue)
}

export interface FlushResult {
  synced: number
  failed: number
}

/**
 * Replay queued adds in order with the current auth. `onSynced` fires after
 * each successful add (so the UI can refresh the company list). Stops at the
 * first network failure (still offline — keep the rest) and at the first
 * hard failure (kept + toasted).
 */
export async function flushOutbox(onSynced: () => void): Promise<FlushResult> {
  const queue = listOutbox()
  if (queue.length === 0) return { synced: 0, failed: 0 }

  const result: FlushResult = { synced: 0, failed: 0 }
  let i = 0
  while (i < queue.length) {
    const input = queue[i]
    try {
      await createCompany(toCompanyInput(input))
      result.synced += 1
      queue.splice(i, 1) // success removes the entry
      saveOutbox(queue)
      onSynced()
    } catch (e) {
      if (e instanceof NetworkError) break // still offline — try again later
      result.failed += 1
      break // hard failure: keep the entry and stop (no toast storm)
    }
  }

  if (result.synced > 0) {
    toaster.create({
      title: result.failed > 0 ? 'Some companies synced' : 'Saved companies synced',
      description:
        result.failed > 0
          ? `${result.synced} synced · ${result.failed} still waiting.`
          : `${result.synced} company${result.synced === 1 ? '' : 'ies'} added to your workspace.`,
      type: result.failed > 0 ? 'warning' : 'success',
    })
  }
  return result
}
