// Building blocks shared by the screens: a fixed input (space or assumption), the cost line under a
// constraint, and a constraint asked as a question ("Let Warren pick" or set it yourself).
import { For, Show } from "solid-js";
import type { CtlMeta, Op, OutMeta, VarMeta } from "../api/types";
import { num } from "../format";
import { addConstraint, costs, removeConstraint, request, setConstraint, setConstraintOp, setParam } from "../state";
import { NumberBox, Range } from "./Slider";

export const OP_LABEL: Record<Op, string> = { min: "at least", eq: "exactly", max: "at most" };

/** A fixed input: slider + number box, a yes/no, or a choice. */
export function ControlRow(props: { c: CtlMeta }) {
  const value = () => num(request()?.params[props.c.key]);
  const id = `in-${props.c.key}`;
  const kind = props.c.kind;
  if (kind.type === "range") {
    const spec = {
      id,
      min: kind.min,
      max: kind.max,
      step: kind.step,
      get value() {
        return value();
      },
      fmt: props.c.fmt,
      onChange: (v: number, live: boolean) => setParam(props.c.key, v, live),
    };
    return (
      <div class="field" data-key={props.c.key}>
        <div class="field-top">
          <label for={id}>{props.c.label}</label>
          <NumberBox {...spec} />
        </div>
        <Range {...spec} />
        <Show when={props.c.help}>
          <p class="hint">{props.c.help}</p>
        </Show>
      </div>
    );
  }
  if (kind.type === "bool") {
    return (
      <div class="field" data-key={props.c.key}>
        <label class="check">
          <input type="checkbox" checked={value() > 0.5} onChange={(e) => setParam(props.c.key, e.currentTarget.checked ? 1 : 0)} />
          <span>{props.c.label}</span>
        </label>
        <Show when={props.c.help}>
          <p class="hint">{props.c.help}</p>
        </Show>
      </div>
    );
  }
  return (
    <div class="field" data-key={props.c.key}>
      <span class="field-label">{props.c.label}</span>
      <Pills
        name={props.c.key}
        options={kind.options.map((o) => ({ value: o.value, label: o.label }))}
        selected={value()}
        onPick={(v) => setParam(props.c.key, v)}
      />
      <Show when={props.c.help}>
        <p class="hint">{props.c.help}</p>
      </Show>
    </div>
  );
}

/** Radio buttons drawn as pills. */
export function Pills(props: { name: string; options: { value: number; label: string }[]; selected: number; onPick: (v: number) => void; label?: string }) {
  return (
    <div class="pills" role="radiogroup" aria-label={props.label}>
      <For each={props.options}>
        {(o) => (
          <label class="pill">
            <input type="radio" name={props.name} checked={Math.abs(o.value - props.selected) < 1e-9} onChange={() => props.onPick(o.value)} />
            <span>{o.label}</span>
          </label>
        )}
      </For>
    </div>
  );
}

/** What a constraint costs, worked out by re-planning without it. */
export function CostLine(props: { k: string }) {
  const c = () => costs()[props.k];
  const free = () => /^not limiting/i.test(c()?.text ?? "");
  return (
    <p class={{ cost: true, stale: c()?.stale ?? true, free: free() }} aria-live="polite">
      {c()?.text ?? "Working out what this costs…"}
    </p>
  );
}

/** Friendly questions for each constraint the solver would otherwise choose. */
export const QUESTIONS: Record<string, string> = {
  qol: "More room in every cage?",
  qol_doe: "More room for does and their litters?",
  qol_buck: "More room for the buck?",
  qol_young: "More room for young growers (5–12 weeks)?",
  qol_old: "More room for older growers (12 weeks and up)?",
  max_headcount: "Is there a most you can look after at once?",
  tiers: "How many tiers of cages?",
  grower_cage_w: "How wide should grower cages be?",
  does: "How many breeding does?",
  bucks: "How many bucks?",
  wean_days: "How long do kits stay with mum?",
  rebreed_days: "How soon after a litter can a doe be mated again?",
  butcher_days: "How old, at the latest, when butchered?",
  butcher_rule: "When do the youngsters go?",
  meat: "How much meat do you need?",
  fryers: "How many rabbits for the table each year?",
  peak_head: "How big a herd do you want on hand?",
};

/** One constraint as a question card. Off = "Let Warren pick"; on = your value, with what it costs. */
export function LimitQuestion(props: { v?: VarMeta; o?: OutMeta }) {
  const key = props.v?.key ?? props.o!.key;
  const c = () => request()?.constraints.find((x) => x.key === key);
  const ops: Op[] = props.v ? props.v.ops : ["min", "eq", "max"];
  const choice = props.v?.kind.type === "choice" ? props.v.kind : null;
  const range = props.v ? (props.v.kind.type === "range" ? props.v.kind : { min: 0, max: 1, step: 0.01 }) : { min: props.o!.min, max: props.o!.max, step: props.o!.step };
  const id = `q-${key}`;
  return (
    <section class={{ q: true, on: !!c() }} data-key={key}>
      <h3 id={`${id}-h`}>{QUESTIONS[key] ?? props.v?.label ?? props.o?.label}</h3>
      <div class="pills" role="radiogroup" aria-labelledby={`${id}-h`}>
        <label class="pill">
          <input type="radio" name={`${id}-on`} checked={!c()} onChange={() => removeConstraint(key)} />
          <span>{props.o ? "Whatever fits" : "Let Warren pick"}</span>
        </label>
        <label class="pill">
          <input type="radio" name={`${id}-on`} checked={!!c()} onChange={() => void addConstraint(key)} />
          <span>I'll say</span>
        </label>
      </div>
      <Show when={c()}>
        {(cc) => {
          const spec = {
            id,
            min: range.min,
            max: range.max,
            step: range.step,
            get value() {
              return cc().value;
            },
            fmt: props.v ? props.v.fmt : props.o!.fmt,
            onChange: (x: number, live: boolean) => setConstraint(key, { value: x }, live),
          } as const;
          return (
            <>
              <Show
                when={choice}
                fallback={
                  <div class="q-set">
                    <Show when={ops.length > 1} fallback={<span class="q-op">{OP_LABEL[ops[0]]}</span>}>
                      <select class="q-op" aria-label="At least, exactly or at most" value={cc().op} onChange={(e) => setConstraintOp(key, e.currentTarget.value as Op)}>
                        <For each={ops}>{(op) => <option value={op}>{OP_LABEL[op]}</option>}</For>
                      </select>
                    </Show>
                    <NumberBox {...spec} unit={props.o?.unit} />
                    <Range {...spec} />
                  </div>
                }
              >
                {(ch) => <Pills name={`${id}-v`} options={ch().options} selected={cc().value} onPick={(x) => setConstraint(key, { value: x })} />}
              </Show>
              <CostLine k={key} />
            </>
          );
        }}
      </Show>
      <Show when={props.v?.help}>
        <p class="hint">{props.v!.help}</p>
      </Show>
    </section>
  );
}
