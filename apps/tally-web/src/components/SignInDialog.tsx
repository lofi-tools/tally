// Sign-in / create-account dialog (docs/spec/web-api-wiring-spec.md §5.1,
// §7.3). Replaces the simulated SaveProgressDialog: real auth against the
// API, and registering migrates the user's local companies to the account.
//
// The migration loop is also exported so the sidebar's "Retry migration"
// action can re-run it for whatever failed last time.

import { createSignal, Show, type JSX } from 'solid-js'
import { Button, Dialog, Field, Input, Spinner, Tabs, toaster } from '@tally/design-system'
import { LogIn, UserRound, X } from 'lucide-solid'
import { css } from 'styled-system/css'
import { ApiError, createCompany, login, NetworkError, register, setToken } from '../api'
import { setSession } from '../session'
import type { Company } from '../mock_data'

type Mode = 'login' | 'register'
type Phase = 'form' | 'migrating'

export interface MigrationResult {
  migratedIds: string[]
  skipped: number
  failed: number
  total: number
}

/**
 * Migrate real local companies to the API (§7.3). Only user-added companies
 * are passed in — never the sample's demo data. Runs in name order; stops at
 * the first hard failure (network / 5xx / validation), keeping the local
 * copy so nothing is lost.
 */
export async function migrateCompanies(companies: Company[]): Promise<MigrationResult> {
  const sorted = [...companies].sort((a, b) => a.name.localeCompare(b.name))
  const result: MigrationResult = { migratedIds: [], skipped: 0, failed: 0, total: sorted.length }
  for (const c of sorted) {
    try {
      await createCompany({
        name: c.name,
        company_number: c.companyNumber,
        tax_reference: c.utr,
        sic_codes: c.sic && c.sic !== '—' ? [c.sic] : undefined,
        address_lines: c.address && c.address !== '—' ? [c.address] : undefined,
      })
      result.migratedIds.push(c.id)
    } catch (e) {
      if (e instanceof ApiError && e.code === 'duplicate_company') {
        result.skipped += 1
        result.migratedIds.push(c.id) // the API copy is authoritative
      } else {
        result.failed += 1
        break
      }
    }
  }
  return result
}

/** The §7.3 summary toast (dynamic counts). */
export function toastMigration(result: MigrationResult): void {
  if (result.failed > 0) {
    toaster.create({
      title: 'Account created — almost there',
      description: `${result.total - result.failed} of ${result.total} companies migrated; ${result.failed} couldn't be moved. They're still saved locally.`,
      type: 'warning',
    })
  } else if (result.skipped > 0) {
    toaster.create({
      title: 'Account created',
      description: `Migrated ${result.migratedIds.length} companies · ${result.skipped} already in your account.`,
      type: 'success',
    })
  } else if (result.total > 0) {
    toaster.create({
      title: 'Account created',
      description: `Migrated ${result.migratedIds.length} companies.`,
      type: 'success',
    })
  } else {
    toaster.create({ title: 'Account created', description: "You're all set.", type: 'success' })
  }
}

interface FieldErrors {
  name?: string
  email?: string
  password?: string
}

export function SignInDialog(props: {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** Real local companies to migrate on register (never the sample). */
  localCompanies: () => Company[]
  onMigrationComplete: (migratedIds: string[]) => void
}) {
  const [mode, setMode] = createSignal<Mode>('login')
  const [phase, setPhase] = createSignal<Phase>('form')
  const [name, setName] = createSignal('')
  const [email, setEmail] = createSignal('')
  const [password, setPassword] = createSignal('')
  const [fieldErrors, setFieldErrors] = createSignal<FieldErrors>({})
  const [formError, setFormError] = createSignal<string | undefined>(undefined)
  const [submitting, setSubmitting] = createSignal(false)

  const reset = () => {
    setMode('login')
    setPhase('form')
    setName('')
    setEmail('')
    setPassword('')
    setFieldErrors({})
    setFormError(undefined)
    setSubmitting(false)
  }

  const migrating = () => phase() === 'migrating'

  const close = () => {
    props.onOpenChange(false)
    reset()
  }

  /** Map an API `validation_failed` details.fields[] onto the form. */
  const mapValidation = (details: Record<string, unknown> | undefined) => {
    const errs: FieldErrors = {}
    for (const f of (details?.fields as Array<{ field?: unknown; reason?: unknown }> | undefined) ?? []) {
      const field = typeof f.field === 'string' ? f.field : ''
      const reason = typeof f.reason === 'string' ? f.reason : 'invalid'
      if (field === 'display_name') errs.name = reason
      else if (field === 'email') errs.email = reason
      else if (field === 'password') errs.password = reason
    }
    setFieldErrors(errs)
  }

  /** Shared failure handling: surface the error where it belongs. */
  const surfaceError = (e: unknown): void => {
    if (e instanceof NetworkError) {
      setFormError("Can't reach the API — is it running?")
    } else if (e instanceof ApiError) {
      if (e.code === 'validation_failed') {
        mapValidation(e.details)
      } else if (e.code === 'email_taken') {
        setFieldErrors({ email: 'An account with this email already exists' })
      } else if (e.code === 'invalid_credentials') {
        setFieldErrors({ password: 'Invalid email or password' })
      } else {
        // Any other code: the envelope message is UI-safe by contract.
        setFormError(e.message)
      }
    } else {
      setFormError('Something went wrong. Please try again.')
    }
  }

  const submit = async (e: Event) => {
    e.preventDefault()
    if (submitting()) return
    setSubmitting(true)
    setFieldErrors({})
    setFormError(undefined)
    try {
      const resp =
        mode() === 'login'
          ? await login({ email: email().trim(), password: password() })
          : await register({
              display_name: name().trim(),
              email: email().trim(),
              password: password(),
            })
      setToken(resp.token)
      setSession({ status: 'signed-in', user: resp.user })

      if (mode() === 'register') {
        const locals = props.localCompanies()
        if (locals.length > 0) {
          // §7.3 migration phase — replace the form with a progress state.
          setPhase('migrating')
          setSubmitting(false)
          const result = await migrateCompanies(locals)
          props.onMigrationComplete(result.migratedIds)
          toastMigration(result)
        } else {
          toastMigration({ migratedIds: [], skipped: 0, failed: 0, total: 0 })
        }
      } else {
        toaster.create({ title: `Welcome back, ${resp.user.display_name}`, type: 'success' })
      }
      close()
    } catch (err) {
      setSubmitting(false)
      surfaceError(err)
    }
  }

  const field = (err: string | undefined): JSX.Element =>
    err ? <Field.ErrorText>{err}</Field.ErrorText> : <></>

  return (
    <Dialog.Root
      open={props.open}
      onOpenChange={(d) => {
        props.onOpenChange(d.open)
        if (!d.open) reset()
      }}
      closeOnInteractOutside={!migrating()}
      closeOnEscape={!migrating()}
    >
      <Dialog.Backdrop />
      <Dialog.Positioner>
        <Dialog.Content>
          <Show when={!migrating()}>
            <Dialog.CloseTrigger>
              <X />
            </Dialog.CloseTrigger>
          </Show>
          <Dialog.Header>
            <Dialog.Title>Sign in to Tally</Dialog.Title>
            <Dialog.Description>
              {mode() === 'login'
                ? 'Sign in to load your companies and books.'
                : 'Create an account — your local data is moved to it automatically.'}
            </Dialog.Description>
          </Dialog.Header>

          <Dialog.Body>
            <Show
              when={phase() === 'form'}
              fallback={
                <div class={css({ py: '10', display: 'flex', flexDirection: 'column', alignItems: 'center', gap: '3', textAlign: 'center' })}>
                  <Spinner size="sm" class={css({ color: 'brown.11' })} />
                  <div class={css({ textStyle: 'sm', fontWeight: '600' })}>Moving your data…</div>
                  <div class={css({ textStyle: 'xs', color: 'fg.muted', maxW: '20rem' })}>
                    Your companies are being copied to your new account.
                  </div>
                </div>
              }
            >
              <form onSubmit={submit}>
                {/* Segmented control: Log in / Create account */}
                <Tabs.Root value={mode()} onValueChange={(d) => setMode(d.value as Mode)} class={css({ mb: '5' })}>
                  <Tabs.List class={css({ bg: 'bg.subtle', p: '1', borderRadius: 'md', border: '1px solid {colors.border}' })}>
                    <Tabs.Trigger
                      value="login"
                      class={css({ flex: '1', borderRadius: 'sm', py: '1.5', _selected: { bg: 'bg.default', boxShadow: 'sm' } })}
                    >
                      Log in
                    </Tabs.Trigger>
                    <Tabs.Trigger
                      value="register"
                      class={css({ flex: '1', borderRadius: 'sm', py: '1.5', _selected: { bg: 'bg.default', boxShadow: 'sm' } })}
                    >
                      Create account
                    </Tabs.Trigger>
                    <Tabs.Indicator />
                  </Tabs.List>
                </Tabs.Root>

                <div class={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
                  <Show when={mode() === 'register'}>
                    <Field.Root invalid={!!fieldErrors().name} required>
                      <Field.Label>
                        Name <Field.RequiredIndicator />
                      </Field.Label>
                      <Input
                        placeholder="Sam Rivera"
                        autocomplete="name"
                        value={name()}
                        onInput={(e) => setName(e.currentTarget.value)}
                      />
                      {field(fieldErrors().name)}
                    </Field.Root>
                  </Show>

                  <Field.Root invalid={!!fieldErrors().email} required>
                    <Field.Label>
                      Email <Field.RequiredIndicator />
                    </Field.Label>
                    <Input
                      type="email"
                      placeholder="you@company.co.uk"
                      autocomplete="email"
                      value={email()}
                      onInput={(e) => setEmail(e.currentTarget.value)}
                    />
                    {field(fieldErrors().email)}
                  </Field.Root>

                  <Field.Root invalid={!!fieldErrors().password} required>
                    <Field.Label>
                      Password <Field.RequiredIndicator />
                    </Field.Label>
                    <Input
                      type="password"
                      placeholder="••••••••"
                      autocomplete={mode() === 'login' ? 'current-password' : 'new-password'}
                      value={password()}
                      onInput={(e) => setPassword(e.currentTarget.value)}
                    />
                    <Show when={mode() === 'register' && !fieldErrors().password}>
                      <Field.HelperText>At least 8 characters.</Field.HelperText>
                    </Show>
                    {field(fieldErrors().password)}
                  </Field.Root>

                  <Show when={formError()}>
                    <div class={css({ textStyle: 'sm', color: 'red.plain.fg', bg: 'bg.subtle', border: '1px solid {colors.red.a5}', px: '3', py: '2', borderRadius: 'md' })}>
                      {formError()}
                    </div>
                  </Show>
                </div>
              </form>
            </Show>
          </Dialog.Body>

          <Dialog.Footer>
            <Dialog.ActionTrigger disabled={migrating()} class={css({ color: 'fg.muted' })}>
              Not now
            </Dialog.ActionTrigger>
            <Button
              disabled={submitting() || migrating()}
              onClick={(e) => submit(e)}
              loading={submitting()}
            >
              {mode() === 'login' ? (
                <>
                  <LogIn class={css({ w: '3.5', h: '3.5' })} /> Log in
                </>
              ) : (
                <>
                  <UserRound class={css({ w: '3.5', h: '3.5' })} /> Create account
                </>
              )}
            </Button>
          </Dialog.Footer>
        </Dialog.Content>
      </Dialog.Positioner>
    </Dialog.Root>
  )
}
