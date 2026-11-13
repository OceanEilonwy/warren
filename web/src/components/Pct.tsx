// How far the running solve has got, as " 42%", shown beside "Re-planning…" messages.
import { Show } from "solid-js";
import { progress } from "../state";

export function Pct() {
  return (
    <Show when={progress() !== null}>
      <span class="pct"> {Math.floor((progress() ?? 0) * 100)}%</span>
    </Show>
  );
}
