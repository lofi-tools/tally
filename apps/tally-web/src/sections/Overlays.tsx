import type { JSX } from 'solid-js'
import {
  Badge,
  Button,
  Card,
  Dialog,
  Kbd,
  Menu,
  Switch,
  Tabs,
  Tooltip,
  type ColorModeController,
} from '@tally/design-system'
import { button } from 'styled-system/recipes'
import { css } from 'styled-system/css'
import { Moon, Sun, X } from '../components/icons'
import { Section } from './Section'

function DemoCard(props: { title: string; description: string; children: JSX.Element }) {
  return (
    <Card.Root>
      <Card.Header>
        <Card.Title>{props.title}</Card.Title>
        <Card.Description>{props.description}</Card.Description>
      </Card.Header>
      <Card.Body>{props.children}</Card.Body>
    </Card.Root>
  )
}

export function Overlays(props: { colorMode: ColorModeController }) {
  return (
    <Section
      id="overlays"
      eyebrow="Feedback & navigation"
      title="Overlays and controls"
      description="Floating UI, focus management and a11y come from Ark UI — the design system just makes it look like Tally."
    >
      <div class={css({ display: 'grid', gap: '6', lg: { gridTemplateColumns: 'repeat(2, 1fr)' } })}>
        <DemoCard title="Dialog" description="Modal with a focus trap, ESC to close and a soft scale-in.">
          <Dialog.Root lazyMount unmountOnExit>
            <Dialog.Trigger class={button({ visual: 'solid' })}>Open dialog</Dialog.Trigger>
            <Dialog.Backdrop />
            <Dialog.Positioner>
              <Dialog.Content>
                <Dialog.CloseTrigger>
                  <X />
                </Dialog.CloseTrigger>
                <Dialog.Title>Submit your accounts</Dialog.Title>
                <Dialog.Description>
                  Your FRS 105 micro-entity accounts are ready to file with Companies House.
                  Review the balance sheet and the CT600 before you confirm.
                </Dialog.Description>
                <div class={css({ display: 'flex', justifyContent: 'flex-end', gap: '2', mt: '2' })}>
                  <Dialog.Trigger class={button({ visual: 'ghost' })}>Cancel</Dialog.Trigger>
                  <Button>Confirm & file</Button>
                </div>
              </Dialog.Content>
            </Dialog.Positioner>
          </Dialog.Root>
        </DemoCard>

        <DemoCard title="Menu" description="A contextual menu with groups, separators and keyboard support.">
          <Menu.Root>
            <Menu.Trigger class={button({ visual: 'outline' })}>Actions</Menu.Trigger>
            <Menu.Positioner>
              <Menu.Content>
                <Menu.Item value="csv">
                  <Menu.ItemText>Download CSV</Menu.ItemText>
                  <Kbd>⌘⇧E</Kbd>
                </Menu.Item>
                <Menu.Item value="ixbrl">
                  <Menu.ItemText>Download iXBRL</Menu.ItemText>
                  <Kbd>⌘⇧I</Kbd>
                </Menu.Item>
                <Menu.Separator />
                <Menu.Item value="edit">
                  <Menu.ItemText>Edit profile</Menu.ItemText>
                  <Kbd>⌘,</Kbd>
                </Menu.Item>
                <Menu.Item value="delete" disabled>
                  <Menu.ItemText>Delete company</Menu.ItemText>
                </Menu.Item>
              </Menu.Content>
            </Menu.Positioner>
          </Menu.Root>
        </DemoCard>

        <DemoCard title="Tooltip" description="Hover a control for more context — appears with a fade.">
          <div class={css({ display: 'flex', flexWrap: 'wrap', gap: '2.5' })}>
            <Tooltip.Root openDelay={0}>
              <Tooltip.Trigger class={button({ visual: 'outline' })}>Hover me</Tooltip.Trigger>
              <Tooltip.Positioner>
                <Tooltip.Content>Filing goes through HMRC's XML gateway</Tooltip.Content>
                <Tooltip.Arrow />
              </Tooltip.Positioner>
            </Tooltip.Root>
            <Tooltip.Root openDelay={0}>
              <Tooltip.Trigger class={button({ visual: 'ghost' })}>And me</Tooltip.Trigger>
              <Tooltip.Positioner>
                <Tooltip.Content>Accounts are due 9 months after the period end</Tooltip.Content>
                <Tooltip.Arrow />
              </Tooltip.Positioner>
            </Tooltip.Root>
          </div>
        </DemoCard>

        <DemoCard title="Tabs" description="Animated indicator, roving focus and lazy panels.">
          <Tabs.Root defaultValue="accounts">
            <Tabs.List>
              <Tabs.Trigger value="accounts">Accounts</Tabs.Trigger>
              <Tabs.Trigger value="filing">Filing</Tabs.Trigger>
              <Tabs.Trigger value="settings">Settings</Tabs.Trigger>
              <Tabs.Indicator />
            </Tabs.List>
            <Tabs.Content value="accounts">
              Your micro-entity accounts for the period — balance sheet, P&amp;L and notes — are ready to review.
            </Tabs.Content>
            <Tabs.Content value="filing">
              The CT600 return was generated and validated against the HMRC taxonomies.
            </Tabs.Content>
            <Tabs.Content value="settings">
              Company profile, directors and filing preferences live here.
            </Tabs.Content>
          </Tabs.Root>
        </DemoCard>

        <DemoCard title="Switch & color mode" description="Class-based dark mode: toggling adds .dark to <html>, and the semantic tokens follow.">
          <div class={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
            <Switch
              checked={props.colorMode.mode() === 'dark'}
              onCheckedChange={(d) => props.colorMode.set(d.checked ? 'dark' : 'light')}
              label={
                <span class={css({ display: 'inline-flex', alignItems: 'center', gap: '1.5' })}>
                  {props.colorMode.mode() === 'dark' ? <Moon /> : <Sun />}
                  Dark mode
                </span>
              }
            />
            <Switch defaultChecked label="Email notifications" />
            <Switch disabled label="Archived company" />
          </div>
        </DemoCard>

        <DemoCard title="Everything together" description="A filing status card using badges, buttons and switches.">
          <div class={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
            <div class={css({ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '3', flexWrap: 'wrap' })}>
              <div>
                <div class={css({ fontSize: 'sm', fontWeight: '600', color: 'fg' })}>CT600 — Example Ltd.</div>
                <div class={css({ fontSize: 'xs', color: 'fgMuted', mt: '0.5' })}>Period ended 31 Mar 2026</div>
              </div>
              <Badge tone="success" variant="solid">
                Validated
              </Badge>
            </div>
            <div class={css({ display: 'flex', gap: '2', flexWrap: 'wrap' })}>
              <Button size="sm">File now</Button>
              <Button size="sm" visual="outline">
                Preview
              </Button>
              <Button size="sm" visual="ghost">
                Discard
              </Button>
            </div>
          </div>
        </DemoCard>
      </div>
    </Section>
  )
}
