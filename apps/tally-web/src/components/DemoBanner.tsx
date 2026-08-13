import { Button } from "@tally/design-system";
import { FlaskConical, Plus } from "lucide-solid";
import { Show } from "solid-js";
import { css } from "styled-system/css";

/** Which demo-data state the banner is explaining (spec §5.2). */
export type DemoBannerVariant = "onboarding" | "viewing-demo" | "empty-data";

/**
 * Full-width announcement bar pinned above the whole app shell (sidebar and
 * main). Explains that the data on screen is the demo company and teaches
 * the two-step path to real data: a brown surface one step dimmer than the
 * solid accent (brown.solid.dim.bg), white icon/text, and a primary-brown
 * CTA with white text. Follows the selected company (§4 state machine) and
 * comes in three copy variants:
 *
 * - `onboarding` — no real companies yet: add one, then connect data.
 * - `viewing-demo` — real companies exist but the demo is selected.
 * - `empty-data` — a real company is selected but nothing is connected yet.
 */
export function DemoBanner(props: {
  variant: DemoBannerVariant;
  /** Name of the first real company, used by the `viewing-demo` action. */
  viewCompanyName?: string;
  onAddCompany: () => void;
  onViewCompany: () => void;
  onConnectBank: () => void;
}) {
  return (
    // Full-width strip above the whole app shell: a brown surface one step
    // dimmer than the solid accent, with white content. The value is fixed
    // across light/dark so the banner looks the same in both modes.
    <div class={css({ w: "full", flexShrink: "0", bg: "brown.solid.dim.bg", color: "white" })}>
      <div
        class={css({
          display: "flex",
          alignItems: "center",
          gap: "3",
          flexWrap: "wrap",
          px: { base: "4", md: "5" },
          py: "1",
        })}
      >
        <FlaskConical class={css({ w: "4", h: "4", color: "white", flexShrink: "0" })} />
        <span class={css({ textStyle: "sm" })}>
          <Show when={props.variant === "onboarding"}>
            <span class={css({ fontWeight: "600" })}>Demo data</span>
            <span class={css({ color: "white.a10" })}> · 1. Add your company → 2. Add a data source</span>
          </Show>
          <Show when={props.variant === "viewing-demo"}>
            <span class={css({ fontWeight: "600" })}>You're viewing demo data</span>
            <span class={css({ color: "white.a10" })}> · switch to your own company to see your numbers.</span>
          </Show>
          <Show when={props.variant === "empty-data"}>
            <span class={css({ fontWeight: "600" })}>Your data isn't here yet</span>
            <span class={css({ color: "white.a10" })}> · connect a bank or upload a ledger to pull in transactions.</span>
          </Show>
        </span>

        {/* CTAs sit left-aligned right after the copy. A fixed dark gray
            (the gray palette's solid flips to white in dark mode, so no
            variant token fits) keeps them distinct from the brown surface
            with readable white text in both modes. */}
        <Show when={props.variant === "onboarding"}>
          <Button size="xs" onClick={props.onAddCompany} class={css({ bg: "#2a2926", color: "white", _hover: { bg: "#34332f" } })}>
            <Plus class={css({ w: "3.5", h: "3.5" })} /> Add company
          </Button>
        </Show>
        <Show when={props.variant === "viewing-demo"}>
          <Button size="xs" onClick={props.onViewCompany} class={css({ bg: "#2a2926", color: "white", _hover: { bg: "#34332f" } })}>
            View {props.viewCompanyName}
          </Button>
        </Show>
        <Show when={props.variant === "empty-data"}>
          <Button size="xs" onClick={props.onConnectBank} class={css({ bg: "#2a2926", color: "white", _hover: { bg: "#34332f" } })}>
            Connect a bank
          </Button>
        </Show>
      </div>
    </div>
  );
}
