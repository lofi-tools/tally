import { Button, Card, Collapsible, createListCollection, IconButton, Input, Select, Table, Tabs, toaster } from "@tally/design-system";
import { ArrowRight, ChevronDown, Download, Landmark, Plus, Search, X } from "lucide-solid";
import { createEffect, createMemo, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { css, cx } from "styled-system/css";
import { DataSourceRows, EmptyState, numCell, PageHeader, SampleBadge, StatCard, StatusBadge } from "../components/Shared";
import {
  accountBalance,
  accountBreadcrumb,
  accountLabel,
  accountPathOf,
  chartAccountNames,
  chartOfAccounts,
  fmtDate,
  fmtMoney,
  fmtSignedMoney,
  groupBalance,
  SAMPLE_COMPANY_ID,
  transactionsFor,
  type AccountNode,
  type Company,
  type CompanyData,
  type DataSource,
} from "../mock_data";

// Filter dropdown derived from the chart of accounts (kept in sync with the tree).
const accountOptions = createListCollection({
  items: [{ label: "All accounts", value: "all" }, ...chartAccountNames().map((name) => ({ label: name, value: name }))],
});

/** App-convention balance colour: positive green, negative red, zero muted. */
const balanceFg = (n: number) => (n > 0 ? "green.plain.fg" : n < 0 ? "red.plain.fg" : "fg.muted");
/** Zero renders unsigned; everything else signed (e.g. +£45,000.00 / −£1,850.00). */
const balanceText = (n: number) => (n === 0 ? fmtMoney(n) : fmtSignedMoney(n));

/**
 * Zero-balance accounts are hidden from the tree: a leaf when its balance is
 * £0.00; a group when it has no non-zero descendants (a group whose children
 * net to zero, e.g. R&D relief vs spend, stays visible because its children
 * are meaningful). Threshold matches what fmtMoney displays as £0.00.
 */
const isVisibleAccount = (node: AccountNode): boolean =>
  node.children.length === 0 ? Math.abs(accountBalance(accountPathOf(node))) >= 0.005 : node.children.some(isVisibleAccount);

export function AccountsView(props: { company: Company; data: CompanyData; sources: DataSource[]; onGoToIntegrations: () => void }) {
  const [tab, setTab] = createSignal("balances");
  const [query, setQuery] = createSignal("");
  const [account, setAccount] = createSignal("all");
  // Leaf account whose register is shown in the inline side panel; null = none.
  const [selected, setSelected] = createSignal<string | null>(null);

  // Drawer conventions: Escape closes the register; on small screens the
  // register stacks below the chart, so bring it into view when it opens.
  onMount(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setSelected(null);
    };
    document.addEventListener("keydown", onKey);
    onCleanup(() => document.removeEventListener("keydown", onKey));
  });
  let registerRef: HTMLDivElement | undefined;
  createEffect(() => {
    if (selected() && registerRef && window.matchMedia("(max-width: 63.9rem)").matches) {
      registerRef.scrollIntoView({ behavior: "smooth", block: "nearest" });
    }
  });

  const filtered = createMemo(() => {
    const q = query().trim().toLowerCase();
    return props.data.transactions.filter(
      (t) =>
        (account() === "all" || t.account === account()) &&
        (q === "" || t.description.toLowerCase().includes(q) || t.account.toLowerCase().includes(q) || t.source.toLowerCase().includes(q)),
    );
  });

  const recent = createMemo(() => [...props.data.transactions].sort((a, b) => b.date.localeCompare(a.date)).slice(0, 5));

  /**
   * Register of the selected account: transactions oldest → newest with the
   * running balance after each one, so the user can follow where amounts come
   * from. The final running total equals the account's current balance.
   */
  const register = createMemo(() => {
    const path = selected();
    if (!path) return [];
    const rows = transactionsFor(path).sort((a, b) => a.date.localeCompare(b.date) || a.id.localeCompare(b.id));
    let running = 0;
    return rows.map((t) => {
      running += t.amount;
      return { t, running };
    });
  });
  const registerBalance = () => (selected() ? accountBalance(selected()!) : 0);

  const yearIncome = props.data.summaries.reduce((s, m) => s + m.income, 0);
  const yearExpenses = props.data.summaries.reduce((s, m) => s + m.expenses, 0);
  const maxNet = Math.max(...props.data.summaries.map((m) => Math.abs(m.income - m.expenses)));

  const hasData = () => props.data.transactions.length > 0 || props.data.summaries.length > 0;

  // "All transactions →": jump to the full list with every filter cleared.
  const onAllTransactions = () => {
    setQuery("");
    setAccount("all");
    setTab("transactions");
  };

  return (
    <>
      <PageHeader
        title="Accounts"
        description={`Transactions and summaries for ${props.company.name}.`}
        badge={props.company.id === SAMPLE_COMPANY_ID ? <SampleBadge /> : undefined}
        actions={
          <>
            <Button
              variant="outline"
              onClick={() => toaster.create({ title: "Export (mock)", description: "CSV / MTD export lands with the backend.", type: "info" })}
            >
              <Download class={css({ w: "3.5", h: "3.5" })} /> Export
            </Button>
            <Button onClick={() => toaster.create({ title: "Add transaction (mock)", description: "Manual entry needs the backend.", type: "info" })}>
              <Plus class={css({ w: "3.5", h: "3.5" })} /> Add transaction
            </Button>
          </>
        }
      />

      <Tabs.Root value={tab()} onValueChange={(d) => setTab(d.value)}>
        <Tabs.List>
          <Tabs.Trigger value="balances">Balances</Tabs.Trigger>
          <Tabs.Trigger value="transactions">Transactions</Tabs.Trigger>
          <Tabs.Trigger value="sources">Data sources</Tabs.Trigger>
          <Tabs.Indicator />
        </Tabs.List>

        {/* ---------- Balances ---------- */}
        <Tabs.Content value="balances">
          <Show
            when={hasData()}
            fallback={
              <Card.Root>
                <EmptyState
                  icon={<Landmark class={css({ w: "6", h: "6" })} />}
                  title="No data yet"
                  description="Connect a bank or upload a ledger to populate your books."
                  action={<Button onClick={props.onGoToIntegrations}>Connect a bank</Button>}
                />
              </Card.Root>
            }
          >
            {/* Income / expenses / net */}
            <div class={css({ display: "grid", gap: "4", sm: { gridTemplateColumns: "repeat(3, 1fr)" }, mb: "6" })}>
              <StatCard label="Income (YTD)" value={fmtMoney(yearIncome)} hint="FY2025/26" tone="good" />
              <StatCard label="Expenses (YTD)" value={fmtMoney(yearExpenses)} hint="FY2025/26" tone="bad" />
              <StatCard label="Net (YTD)" value={fmtMoney(yearIncome - yearExpenses)} hint="Before corporation tax" />
            </div>

            {/* Chart of accounts + account register: an in-flow drawer. The
                register column collapses to a 0-width track until an account
                is selected, then unrolls from the right (ease-in-out). Grid
                stretch (default align) keeps both cards the same height. */}
            <div
              class={cx(
                css({ display: "grid", mb: "6" }),
                // Closed: no gap (chart spans full width); open: 50/50 split.
                // The register track animates concrete lengths (0px ↔ 28rem) —
                // the drawer width is pre-computed — so the interpolation is
                // always smooth. Animating fr tracks (0fr ↔ 1fr) is not
                // spec-interpolable and browsers fall back to a discrete flip
                // halfway through the transition (the visible "pause").
                selected()
                  ? css({ gap: "4", lg: { gridTemplateColumns: "minmax(0, 1fr) minmax(0, 28rem)", transition: "grid-template-columns 300ms ease-in-out, gap 300ms ease-in-out" } })
                  : css({ lg: { gridTemplateColumns: "minmax(0, 1fr) minmax(0, 0px)", transition: "grid-template-columns 300ms ease-in-out, gap 300ms ease-in-out" } }),
              )}
            >
              {/* Chart of accounts tree */}
              <Card.Root class={css({ minW: "0" })}>
                <div class={css({ px: "4", pt: "4", pb: "1" })}>
                  <div class={css({ fontSize: "sm", fontWeight: "600" })}>Chart of accounts</div>
                </div>
                <div class={css({ px: "2", pb: "2" })}>
                  <For each={chartOfAccounts.filter(isVisibleAccount)}>
                    {(node) => <AccountTreeGroup node={node} depth={0} selected={selected()} onSelect={setSelected} />}
                  </For>
                </div>
              </Card.Root>

              {/* Account register — in-flow drawer, hidden by default, unrolls
                  from the right when an account is selected. The card only
                  mounts once an account is selected, so there is never an
                  empty placeholder to flash while the track animates open or
                  closed. It is pinned to the pre-computed drawer width
                  (28rem at lg+, full width when stacked below lg) so its
                  content lays out exactly once at final size — the opening
                  animation only moves the clip window, never re-styles the
                  register. Fixed table layout guarantees the rows fit: the
                  description column absorbs the slack and truncates instead
                  of pushing the Balance column off the edge. */}
              <div
                ref={registerRef}
                class={cx(css({ minW: "0", overflow: "hidden" }), selected() ? css({ display: "block" }) : css({ display: "none", lg: { display: "block" } }))}
              >
                <Show when={selected()}>
                  <Card.Root class={css({ w: { base: "full", lg: "28rem" }, h: "full" })}>
                    <div class={css({ px: "4", pt: "4", pb: "1" })}>
                      <div class={css({ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "3" })}>
                        <div class={css({ display: "flex", alignItems: "baseline", gap: "3", minW: "0" })}>
                          <div class={css({ fontSize: "sm", fontWeight: "600", truncate: true })}>{accountLabel(selected()!)}</div>
                          <span class={cx(numCell, css({ fontSize: "sm", color: balanceFg(registerBalance()) }))}>{balanceText(registerBalance())}</span>
                        </div>
                        <IconButton size="sm" variant="plain" aria-label="Close register" onClick={() => setSelected(null)}>
                          <X class={css({ w: "3.5", h: "3.5" })} />
                        </IconButton>
                      </div>
                      <div class={css({ fontSize: "xs", color: "fg.subtle", mt: "0.5" })}>{accountBreadcrumb(selected()!)}</div>
                    </div>
                    <div class={css({ px: "2", pb: "2" })}>
                      <Show when={register().length > 0} fallback={<EmptyState title="No transactions" description="This account has no activity yet." />}>
                        <Table.Root class={css({ tableLayout: "fixed" })}>
                          <Table.Head>
                            <Table.Row>
                              <Table.Header class={css({ w: "7rem" })}>Date</Table.Header>
                              <Table.Header>Description</Table.Header>
                              <Table.Header textAlign="right" class={css({ w: "6.5rem" })}>
                                Amount
                              </Table.Header>
                              <Table.Header textAlign="right" class={css({ w: "7.5rem" })}>
                                Balance
                              </Table.Header>
                            </Table.Row>
                          </Table.Head>
                          <Table.Body>
                            <For each={register()}>
                              {({ t, running }) => (
                                <Table.Row>
                                  <Table.Cell class={cx(numCell, css({ w: "7rem", px: "2" }))}>{fmtDate(t.date)}</Table.Cell>
                                  <Table.Cell class={css({ minW: "0" })}>
                                    <span class={css({ display: "block", truncate: true })}>{t.description}</span>
                                    <span class={css({ display: "block", fontSize: "xs", color: "fg.subtle", mt: "0.5", truncate: true })}>{t.source}</span>
                                  </Table.Cell>
                                  <Table.Cell
                                    textAlign="right"
                                    class={cx(
                                      numCell,
                                      css({ w: "6.5rem", px: "2", ...(t.amount > 0 ? { color: "green.plain.fg" } : { color: "fg.default" }) }),
                                    )}
                                  >
                                    {fmtSignedMoney(t.amount)}
                                  </Table.Cell>
                                  <Table.Cell
                                    textAlign="right"
                                    class={cx(numCell, css({ w: "7.5rem", px: "2", color: balanceFg(running) }))}
                                    title={`Balance after this transaction`}
                                  >
                                    {balanceText(running)}
                                  </Table.Cell>
                                </Table.Row>
                              )}
                            </For>
                          </Table.Body>
                        </Table.Root>
                      </Show>
                    </div>
                  </Card.Root>
                </Show>
              </div>
            </div>

            {/* Recent transactions */}
            <Card.Root>
              <div class={css({ px: "4", pt: "4", pb: "1" })}>
                <div class={css({ fontSize: "sm", fontWeight: "600" })}>Recent transactions</div>
              </div>
              <For each={recent()}>
                {(t, i) => (
                  <div
                    class={css({
                      display: "flex",
                      alignItems: "center",
                      gap: "3",
                      px: "4",
                      py: "2.5",
                      borderTop: i() === 0 ? "none" : "1px solid {colors.border}",
                      _hover: { bg: "bg.subtle" },
                      transition: "background-color 120ms ease",
                    })}
                  >
                    <span class={css({ fontSize: "xs", color: "fg.muted", w: "7rem", flexShrink: "0" })}>{fmtDate(t.date)}</span>
                    <span class={css({ flex: "1", minW: "0", fontSize: "sm", truncate: true })}>{t.description}</span>
                    <span class={cx(numCell, css({ color: balanceFg(t.amount) }))}>{fmtSignedMoney(t.amount)}</span>
                  </div>
                )}
              </For>
              <button
                type="button"
                onClick={onAllTransactions}
                class={css({
                  w: "full",
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  gap: "1.5",
                  px: "4",
                  py: "2.5",
                  border: "none",
                  borderTop: "1px solid {colors.border}",
                  fontSize: "sm",
                  fontWeight: "500",
                  color: "brown.11",
                  bg: "transparent",
                  cursor: "pointer",
                  _hover: { bg: "bg.subtle" },
                  transition: "background-color 120ms ease",
                })}
              >
                All transactions <ArrowRight class={css({ w: "3.5", h: "3.5" })} />
              </button>
            </Card.Root>
          </Show>
        </Tabs.Content>

        {/* ---------- Transactions ---------- */}
        <Tabs.Content value="transactions">
          <Show
            when={props.data.transactions.length > 0}
            fallback={
              <Card.Root>
                <EmptyState
                  icon={<Landmark class={css({ w: "6", h: "6" })} />}
                  title="No transactions yet"
                  description="Connect a bank or upload a ledger to populate your books."
                  action={<Button onClick={props.onGoToIntegrations}>Connect a bank</Button>}
                />
              </Card.Root>
            }
          >
            <Show when={props.data.summaries.length > 0}>
              {/* Net by month */}
              <Card.Root class={css({ mb: "6" })}>
                <div class={css({ px: "4", pt: "4", pb: "6" })}>
                  <div class={css({ fontSize: "sm", fontWeight: "600" })}>Net by month</div>
                  <div class={css({ display: "flex", gap: "3", alignItems: "flex-end", h: "32", mt: "6" })}>
                    <For each={props.data.summaries}>
                      {(m) => {
                        const net = m.income - m.expenses;
                        const h = `${Math.max(6, (Math.abs(net) / maxNet) * 100)}%`;
                        return (
                          <div
                            class={css({
                              flex: "1",
                              display: "flex",
                              flexDirection: "column",
                              gap: "1.5",
                              alignItems: "center",
                              justifyContent: "flex-end",
                              h: "full",
                            })}
                          >
                            <span class={cx(numCell, css({ fontSize: "xs", color: "fg.muted" }))}>{fmtMoney(net)}</span>
                            <div
                              class={css({
                                w: "full",
                                h: "16",
                                bg: "bg.subtle",
                                borderRadius: "sm",
                                display: "flex",
                                alignItems: "flex-end",
                                overflow: "hidden",
                              })}
                            >
                              <div
                                style={{ height: h }}
                                class={css({
                                  w: "full",
                                  borderRadius: "sm",
                                  bg: net >= 0 ? "green.solid.bg" : "red.solid.bg",
                                  transition: "height 200ms ease",
                                })}
                              />
                            </div>
                            <span class={css({ fontSize: "xs", color: "fg.subtle" })}>{m.month}</span>
                          </div>
                        );
                      }}
                    </For>
                  </div>
                </div>
              </Card.Root>

              {/* Monthly summary table */}
              <Card.Root class={css({ mb: "6" })}>
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
                        const net = m.income - m.expenses;
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
                            <Table.Cell textAlign="right" class={cx(numCell, css({ color: net >= 0 ? "green.plain.fg" : "red.plain.fg" }))}>
                              {fmtSignedMoney(net)}
                            </Table.Cell>
                          </Table.Row>
                        );
                      }}
                    </For>
                  </Table.Body>
                </Table.Root>
              </Card.Root>
            </Show>

            <div class={css({ display: "flex", gap: "3", mb: "4", flexWrap: "wrap" })}>
              <div class={css({ position: "relative", flex: "1", minW: "16rem", maxW: "26rem" })}>
                <Search
                  class={css({
                    position: "absolute",
                    left: "3",
                    top: "50%",
                    transform: "translateY(-50%)",
                    w: "3.5",
                    h: "3.5",
                    color: "fg.subtle",
                    pointerEvents: "none",
                  })}
                />
                <Input placeholder="Search transactions…" value={query()} onInput={(e) => setQuery(e.currentTarget.value)} class={css({ pl: "9" })} />
              </div>
              {/* Controlled: 'All transactions →' on Balances resets this via setAccount. */}
              <Select.Root collection={accountOptions} value={[account()]} onValueChange={(d) => setAccount(d.value[0])}>
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
              <Show
                when={filtered().length > 0}
                fallback={<EmptyState title="No transactions match" description="Try clearing the search or picking a different account." />}
              >
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
                          <Table.Cell class={css({ maxW: "md", truncate: true })}>{t.description}</Table.Cell>
                          <Table.Cell class={css({ color: "fg.muted", fontSize: "sm", maxW: "xs", truncate: true })} title={t.account}>
                            {t.account}
                          </Table.Cell>
                          <Table.Cell class={css({ color: "fg.muted", fontSize: "sm" })}>{t.source}</Table.Cell>
                          <Table.Cell textAlign="right" class={cx(numCell, css(t.amount > 0 ? { color: "green.plain.fg" } : { color: "fg.default" }))}>
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

        {/* ---------- Data sources ---------- */}
        <Tabs.Content value="sources">
          <Show
            when={props.sources.length > 0}
            fallback={
              <Card.Root>
                <EmptyState
                  icon={<Landmark class={css({ w: "6", h: "6" })} />}
                  title="No data sources yet"
                  description="Connect a bank or upload a ledger to pull transactions into your books."
                  action={<Button onClick={props.onGoToIntegrations}>Connect a bank</Button>}
                />
              </Card.Root>
            }
          >
            <DataSourceRows
              sources={() => props.sources}
              onSync={(ds) => toaster.create({ title: `Syncing ${ds.name}…`, description: "A real Open Banking fetch lands with the backend.", type: "info" })}
            />
          </Show>
          <p class={css({ textStyle: "xs", color: "fg.subtle", mt: "3" })}>
            Transactions are pulled from your connected data sources. CSV import and Open Banking arrive with the backend.
          </p>
        </Tabs.Content>
      </Tabs.Root>
    </>
  );
}

/**
 * One row of the recursive chart-of-accounts tree. Groups (nodes with
 * children) are collapsible and show a rolled-up total; leaves are buttons
 * that select the account for the register panel. Top-level groups start
 * expanded so the first sub-level is visible; deeper levels start collapsed.
 */
function AccountTreeGroup(props: { node: AccountNode; depth: number; selected: string | null; onSelect: (path: string | null) => void }) {
  const [open, setOpen] = createSignal(props.depth === 0);
  const path = () => accountPathOf(props.node);
  const isLeaf = () => props.node.children.length === 0;
  const isSelected = () => path() === props.selected;
  // Drawer convention: re-clicking the open account closes the register.
  const toggle = () => props.onSelect(isSelected() ? null : path());

  // Leaf row: selectable, shows a neutral background when selected.
  if (isLeaf()) {
    return (
      <button
        type="button"
        onClick={toggle}
        class={css({
          w: "full",

          display: "flex",
          alignItems: "center",
          gap: "2",
          py: "1.5",
          pr: "2",
          borderRadius: "md",
          bg: isSelected() ? "bg.subtle" : "transparent",
          border: "none",
          cursor: "pointer",
          textAlign: "left",
          fontSize: "sm",
          color: isSelected() ? "fg.default" : "fg.muted",
          _hover: { bg: "bg.subtle", color: "fg.default" },
          transition: "background-color 120ms ease, color 120ms ease",
        })}
      >
        <span class={css({ flex: "1", minW: "0", truncate: true })}>{props.node.name}</span>
        <span class={cx(numCell, css({ color: balanceFg(accountBalance(path())) }))}>{balanceText(accountBalance(path()))}</span>
      </button>
    );
  }

  // Group row: collapsible, rolled-up total.
  const total = () => groupBalance(props.node);
  return (
    <Collapsible.Root open={open()} onOpenChange={(d) => setOpen(d.open)}>
      <Collapsible.Trigger
        class={css({
          w: "full",
          display: "flex",
          alignItems: "center",
          gap: "2",
          py: "2",
          pr: "2",
          borderRadius: "md",
          bg: "transparent",
          border: "none",
          cursor: "pointer",
          textAlign: "left",
          fontSize: "sm",
          fontWeight: "600",
          color: "fg.default",
          _hover: { bg: "bg.subtle" },
          transition: "background-color 120ms ease",
        })}
      >
        <ChevronDown
          class={css({
            w: "3.5",
            h: "3.5",
            flexShrink: "0",
            color: "fg.muted",
            transition: "transform 150ms ease",
            transform: open() ? "rotate(0deg)" : "rotate(-90deg)",
          })}
        />
        <span class={css({ flex: "1", minW: "0", truncate: true })}>{props.node.name}</span>
        <span class={cx(numCell, css({ color: balanceFg(total()) }))}>{balanceText(total())}</span>
      </Collapsible.Trigger>
      {/* Static padding per nesting level: each Collapsible.Content adds one
          indent step, so the recursion itself produces the indentation. */}
      <Collapsible.Content class={css({ pl: "5" })}>
        <For each={props.node.children.filter(isVisibleAccount)}>
          {(child) => <AccountTreeGroup node={child} depth={props.depth + 1} selected={props.selected} onSelect={props.onSelect} />}
        </For>
      </Collapsible.Content>
    </Collapsible.Root>
  );
}
