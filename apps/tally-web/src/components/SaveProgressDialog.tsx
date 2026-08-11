import { createSignal } from 'solid-js'
import { Button, Dialog, Field, Input } from '@tally/design-system'
import { UserRound, X } from 'lucide-solid'
import { css } from 'styled-system/css'

export function SaveProgressDialog(props: {
  open: boolean
  onOpenChange: (open: boolean) => void
  onSave: (name: string, email: string) => void
}) {
  const [name, setName] = createSignal('')
  const [email, setEmail] = createSignal('')

  const reset = () => {
    setName('')
    setEmail('')
  }

  return (
    <Dialog.Root
      open={props.open}
      onOpenChange={(d) => {
        props.onOpenChange(d.open)
        if (!d.open) reset()
      }}
    >
      <Dialog.Backdrop />
      <Dialog.Positioner>
        <Dialog.Content>
          <Dialog.CloseTrigger>
            <X />
          </Dialog.CloseTrigger>
          <Dialog.Header>
            <Dialog.Title>Save your progress</Dialog.Title>
            <Dialog.Description>
              Create an account to keep your companies and books. Real authentication lands with the backend — this is a mock.
            </Dialog.Description>
          </Dialog.Header>
          <Dialog.Body>
            <div class={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
              <Field.Root required>
                <Field.Label>
                  Name <Field.RequiredIndicator />
                </Field.Label>
                <Input placeholder="Sam Rivera" value={name()} onInput={(e) => setName(e.currentTarget.value)} />
              </Field.Root>
              <Field.Root required>
                <Field.Label>
                  Email <Field.RequiredIndicator />
                </Field.Label>
                <Input type="email" placeholder="you@company.co.uk" value={email()} onInput={(e) => setEmail(e.currentTarget.value)} />
              </Field.Root>
            </div>
          </Dialog.Body>
          <Dialog.Footer>
            <Dialog.ActionTrigger class={css({ color: 'fg.muted' })}>Not now</Dialog.ActionTrigger>
            <Button
              onClick={() => {
                if (!name().trim() || !email().trim()) return
                props.onSave(name().trim(), email().trim())
                props.onOpenChange(false)
              }}
            >
              <UserRound class={css({ w: '3.5', h: '3.5' })} /> Create account
            </Button>
          </Dialog.Footer>
        </Dialog.Content>
      </Dialog.Positioner>
    </Dialog.Root>
  )
}
