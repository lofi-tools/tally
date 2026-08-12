import { Button } from "@tally/design-system";
import { FlaskConical, Plus } from "lucide-solid";
import { Show } from "solid-js";
import { css } from "styled-system/css";

/** Which sample-data state the banner is explaining (spec §5.2). */
export type SampleBannerVariant = "onboarding" | "viewing-sample" | "empty-data";

/**
 * Full-width strip explaining that the data on screen is the demo company
 * and teaching the two-step path to real data. Follows the selected company
 * (§4 state machine) and comes in three copy variants:
 *
 * - `onboarding` — no real companies yet: add one, then connect data.
 * - `viewing-sample` — real companies exist but the sample is selected.
 * - `empty-data` — a real company is selected but nothing is connected yet.
 */
export function SampleBanner(props: {
  variant: SampleBannerVariant;
  /** Name of the first real company, used by the `viewing-sample` action. */
  viewCompanyName?: string;
  onAddCompany: () => void;
  onViewCompany: () => void;
  onConnectBank: () => void;
}) {
  return (
    // Full-width strip: background and content both span the main column,
    // with compact padding. brown.a3 is ~2.5× the old tint — noticeable but
    // still a brand-hue whisper, not a solid fill.
    <div class={css({ w: "full", bg: "brown.subtle.bg", borderBottom: "1px solid {colors.border}" })}>
      <div
        class={css({
          display: "flex",
          alignItems: "center",
          gap: "3",
          px: { base: "4", md: "5" },
          py: "2",
        })}
      >
        <FlaskConical class={css({ w: "4", h: "4", color: "brown.plain.fg", flexShrink: "0" })} />
        <span class={css({ flex: "1", minW: "0", textStyle: "sm", color: "fg.muted" })}>
          <Show when={props.variant === "onboarding"}>
            <span class={css({ color: "fg.default", fontWeight: "600" })}>Sample data</span> · 1. Add your company → 2. Add a data source
          </Show>
          <Show when={props.variant === "viewing-sample"}>
            <span class={css({ color: "fg.default", fontWeight: "600" })}>You're viewing sample data</span> · switch to your own company to see your numbers.
          </Show>
          <Show when={props.variant === "empty-data"}>
            <span class={css({ color: "fg.default", fontWeight: "600" })}>Your data isn't here yet</span> · connect a bank or upload a ledger to pull in
            transactions.
          </Show>
        </span>

        <Show when={props.variant === "onboarding"}>
          <Button size="sm" onClick={props.onAddCompany}>
            <Plus class={css({ w: "3.5", h: "3.5" })} /> Add company
          </Button>
        </Show>
        <Show when={props.variant === "viewing-sample"}>
          <Button size="sm" onClick={props.onViewCompany}>
            View {props.viewCompanyName}
          </Button>
        </Show>
        <Show when={props.variant === "empty-data"}>
          <Button size="sm" onClick={props.onConnectBank}>
            Connect a bank
          </Button>
        </Show>
      </div>
    </div>
  );
}
