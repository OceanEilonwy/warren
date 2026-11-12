// A slider and its number box, kept in step: dragging updates the box at once, typing moves the slider.
import { createSignal, Show } from "solid-js";
import type { Fmt } from "../api/types";
import { affixes, decimals, scale, trim1 } from "../format";

export interface RangeSpec {
  id: string;
  min: number;
  max: number;
  step: number;
  value: number;
  fmt: Fmt;
  /** `live` is true while dragging or typing (re-plan after a pause). */
  onChange: (v: number, live: boolean) => void;
}

export function Range(props: RangeSpec) {
  const fill = () => `${((props.value - props.min) / (props.max - props.min)) * 100}%`;
  return (
    <input
      type="range"
      id={props.id}
      min={props.min}
      max={props.max}
      step={props.step}
      value={props.value}
      style={{ "--fill": fill() }}
      onInput={(e) => props.onChange(Number(e.currentTarget.value), true)}
    />
  );
}

export function NumberBox(props: RangeSpec & { unit?: string }) {
  const sc = () => scale(props.fmt);
  const dec = () => decimals(props.step * sc());
  const show = (v: number) => (v * sc()).toFixed(dec());
  const [editing, setEditing] = createSignal<string | null>(null);
  const aff = () => affixes(props.fmt);
  const suffix = () => props.unit ?? aff()[1];
  const width = () => Math.max(show(props.min).length, show(props.max).length, 2);
  const commit = (raw: string) => {
    const v = parseFloat(raw);
    if (!Number.isFinite(v)) return;
    const stored = v / sc();
    const stepped = Math.round((stored - props.min) / props.step) * props.step + props.min;
    props.onChange(Math.min(props.max, Math.max(props.min, Number(stepped.toFixed(6)))), true);
  };
  return (
    <span class="numwrap">
      <Show when={aff()[0]}>
        <span class="pre">{aff()[0]}</span>
      </Show>
      <input
        class="numbox"
        type="number"
        id={`num-${props.id}`}
        min={show(props.min)}
        max={show(props.max)}
        step={(props.step * sc()).toFixed(dec())}
        value={editing() ?? show(props.value)}
        aria-label="Type a value"
        style={{ "--n": String(width()) }}
        onFocus={(e) => {
          setEditing(e.currentTarget.value);
          e.currentTarget.select();
        }}
        onInput={(e) => {
          setEditing(e.currentTarget.value);
          commit(e.currentTarget.value);
        }}
        onBlur={() => setEditing(null)}
      />
      <Show when={suffix()}>
        <span class="unit">{suffix()}</span>
      </Show>
      <Show when={props.fmt === "weeks_days"}>
        <span class="unit"> = {trim1(props.value / 7)} wk</span>
      </Show>
    </span>
  );
}
