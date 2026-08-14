// Sign-in / create-account dialog (docs/spec/web-api-wiring-spec.md §5.1,
// temp-user spec §7.6).
//
// Register is "create account + adoption": when the browser has a guest id
// (`tally.guest.v1`), it is sent as `X-Guest-Id` so the backend upgrades the
// guest workspace in place (companies / jobs / filings keep their ids — no
// copy loop). A stale guest id (the workspace was already adopted, or never
// existed) falls back to a plain register automatically.

import { createSignal, Show, type JSX } from 'solid-js'
import { Button, Dialog, Field, Input, Tabs, toaster } from '@tally/design-system'
import { LogIn, UserRound, X } from 'lucide-solid'
import { css } from 'styled-system/css'
import { ApiError, login, NetworkError, register, setToken, type AuthResponse } from '../api'
import { getGuestId } from '../guest'
import { setSession } from '../session'

type Mode = 'login' | 'register'

interface FieldErrors {
  name?: string
  email?: string
  password?: string
}

export function SignInDialog(props: {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** Default tab when the dialog opens — 'register' when opened from guest mode. */
  defaultMode?: 'login' | 'register'
}) {
  const [mode, setMode] = createSignal<Mode>('login')
  const [name, setName] = createSignal('')
  const [email, setEmail] = createSignal('')
  const [password, setPassword] = createSignal('')
  const [fieldErrors, setFieldErrors] = createSignal<FieldErrors>({})
  const [formError, setFormError] = createSignal<string | undefined>(undefined)
  const [submitting, setSubmitting] = createSignal(false)

  const reset = (nextMode?: Mode) => {
    setMode(nextMode ?? 'login')
    setName('')
    setEmail('')
    setPassword('')
    setFieldErrors({})
    setFormError(undefined)
    setSubmitting(false)
  }

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

  /** Register, adopting the browser's guest workspace when it has one. */
  const registerWithAdoption = async (): Promise<AuthResponse> => {
    const body = {
      display_name: name().trim(),
      email: email().trim(),
      password: password(),
    }
    const guestId = getGuestId()
    if (!guestId) return register(body)
    try {
      return await register(body, guestId)
    } catch (e) {
      // A stale guest id (workspace adopted earlier, or never created) has
      // no temp user to upgrade — fall back to a plain register.
      if (e instanceof ApiError && (e.code === 'guest_not_found' || e.code === 'guest_already_adopted')) {
        return register(body)
      }
      throw e
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
          : await registerWithAdoption()
      setToken(resp.token)
      setSession({ status: 'signed-in', user: resp.user })
      toaster.create({
        title: mode() === 'register' ? 'Account created' : `Welcome back, ${resp.user.display_name}`,
        type: 'success',
      })
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
        // Default the tab from the caller (register when opened from guest mode).
        if (d.open) reset(props.defaultMode ?? 'login')
        else reset()
      }}
    >
      <Dialog.Backdrop />
      <Dialog.Positioner>
        <Dialog.Content>
          <Dialog.CloseTrigger>
            <X />
          </Dialog.CloseTrigger>
          <Dialog.Header>
            <Dialog.Title>Sign in to Tally</Dialog.Title>
            <Dialog.Description>
              {mode() === 'login'
                ? 'Sign in to load your companies and books.'
                : 'Create an account — your workspace comes with it.'}
            </Dialog.Description>
          </Dialog.Header>

          <Dialog.Body>
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
          </Dialog.Body>

          <Dialog.Footer>
            <Dialog.ActionTrigger class={css({ color: 'fg.muted' })}>
              Not now
            </Dialog.ActionTrigger>
            <Button
              disabled={submitting()}
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
