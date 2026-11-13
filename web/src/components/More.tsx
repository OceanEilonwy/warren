// "Anything else to keep in mind?": every other constraint as a question, with what each one costs.
import { For, Show } from "solid-js";
import { fmtNum, outText, valueText } from "../format";
import { go, meta, request, solution } from "../state";
import { CostLine, LimitQuestion, OP_LABEL } from "./Controls";

/** Asked on the front screen instead. */
const ON_FRONT = new Set(["qol", "max_headcount"]);

/** The inputs asked about here, in the order they are shown (by group). */
export function moreVars() {
  const m = meta()!;
  return m.var_groups.flatMap((g) => m.vars.filter((v) => v.group === g && !ON_FRONT.has(v.key)));
}

const GROUP_TITLE: Record<string, string> = {
  Welfare: "Room for each kind of rabbit",
  Herd: "Your herd",
  "Cage design": "The cages",
  Timing: "Breeding and butchering",
};

function Changes() {
  const m = meta()!;
  const text = (key: string, op: keyof typeof OP_LABEL, value: number) => {
    const v = m.vars.find((x) => x.key === key);
    if (v) return `${v.short}: ${v.kind.type === "choice" ? "" : `${OP_LABEL[op]} `}${valueText(v.kind, v.fmt, value)}`;
    const o = m.outs.find((x) => x.key === key)!;
    return `${o.short}: ${OP_LABEL[op]} ${outText(o, value)}`;
  };
  return (
    <aside class="card changes">
      <h2>What your answers change</h2>
      <Show when={(request()?.constraints.length ?? 0) > 0} fallback={<p class="muted">Nothing yet: Warren is choosing everything.</p>}>
        <ul>
          <For each={request()!.constraints} keyed={(c) => c.key}>
            {(c) => (
              <li>
                <strong>{text(c().key, c().op, c().value)}</strong>
                <CostLine k={c().key} />
              </li>
            )}
          </For>
        </ul>
      </Show>
      <Show when={solution()}>
        {(s) => (
          <div class="now">
            <div class="muted">Your herd gives</div>
            <div class="big">{fmtNum(s().metrics.meat_kg_per_yr.mean, 0)} kg</div>
            <div class="muted">of meat a year</div>
          </div>
        )}
      </Show>
      <button type="button" class="primary" onClick={() => go("herd")}>
        Show my herd
      </button>
    </aside>
  );
}

export function More() {
  const m = meta()!;
  return (
    <div class="more">
      <div class="more-main">
        <div>
          <h1>Anything else to keep in mind?</h1>
          <p class="lede">Answer only what matters to you. Warren works out the rest, and says what each answer costs.</p>
        </div>
        <For each={m.var_groups}>
          {(g) => (
            <Show when={m.vars.some((v) => v.group === g && !ON_FRONT.has(v.key))}>
              <h2 class="group">{GROUP_TITLE[g] ?? g}</h2>
              <div class="qs">
                <For each={m.vars.filter((v) => v.group === g && !ON_FRONT.has(v.key))}>{(v) => <LimitQuestion v={v} />}</For>
              </div>
            </Show>
          )}
        </For>
        <h2 class="group">What you get out of it</h2>
        <div class="qs">
          <For each={m.outs}>{(o) => <LimitQuestion o={o} />}</For>
        </div>
      </div>
      <Changes />
    </div>
  );
}
