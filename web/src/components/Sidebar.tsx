// The sidebar: presets, objective, the space, constraints (each with what it costs) and assumptions.
import { For, Show } from "solid-js";
import type { Constraint, CtlMeta, Op, VarMeta } from "../api/types";
import { num } from "../format";
import {
  addConstraint,
  costs,
  exportPreset,
  importPreset,
  loadPreset,
  meta,
  note,
  presets,
  removeConstraint,
  request,
  restoreDefaults,
  setConstraint,
  setConstraintOp,
  setObjective,
  setParam,
} from "../state";
import { NumberBox, Range } from "./Slider";

const OP_LABEL: Record<Op, string> = { min: "at least", eq: "exactly", max: "at most" };

function PresetBar() {
  let file!: HTMLInputElement;
  return (
    <div class="presets">
      <select
        id="preset-select"
        aria-label="Load a preset"
        onChange={(e) => {
          const id = e.currentTarget.value;
          e.currentTarget.value = "";
          void loadPreset(id);
        }}
      >
        <option value="">Load a preset…</option>
        <For each={presets()}>{(p) => <option value={p.id} title={p.description}>{p.name}</option>}</For>
      </select>
      <div class="preset-actions">
        <button type="button" data-tip="Download the current setup as a text file you can keep, share or edit" onClick={() => void exportPreset()}>
          Export
        </button>
        <button type="button" data-tip="Load a preset file saved with Export" onClick={() => file.click()}>
          Import…
        </button>
        <input
          ref={file}
          id="preset-file"
          type="file"
          accept=".txt,text/plain"
          hidden
          onChange={async (e) => {
            const f = e.currentTarget.files?.[0];
            e.currentTarget.value = "";
            if (f) await importPreset(await f.text());
          }}
        />
      </div>
      <Show when={note()}>
        {(n) => (
          <div class="preset-note" role="status">
            <For each={n().split("\n")}>{(line, i) => <>{i() > 0 && <br />}{line}</>}</For>
          </div>
        )}
      </Show>
    </div>
  );
}

/** A fixed input. Sliders show their value; dropdowns and choices already show theirs. */
function ControlRow(props: { c: CtlMeta }) {
  const value = () => num(request()?.params[props.c.key]);
  const id = () => `in-${props.c.key}`;
  const kind = props.c.kind;
  if (kind.type === "range") {
    const spec = {
      get id() {
        return id();
      },
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
      <div class="ctl" data-key={props.c.key}>
        <div class="ctl-top">
          <label for={id()}>{props.c.label}</label>
          <NumberBox {...spec} />
        </div>
        <Range {...spec} />
        <Show when={props.c.help}>
          <p class="help">{props.c.help}</p>
        </Show>
      </div>
    );
  }
  if (kind.type === "bool") {
    return (
      <div class="ctl" data-key={props.c.key}>
        <div class="ctl-top">
          <label for={id()} data-tip={props.c.help}>
            {props.c.label}
          </label>
          <select id={id()} value={value() > 0.5 ? "true" : "false"} onChange={(e) => setParam(props.c.key, e.currentTarget.value === "true" ? 1 : 0)}>
            <option value="true">yes</option>
            <option value="false">no</option>
          </select>
        </div>
      </div>
    );
  }
  return (
    <div class="ctl" data-key={props.c.key}>
      <div class="ctl-top">
        <label data-tip={props.c.help}>{props.c.label}</label>
      </div>
      <div class="seg" role="radiogroup">
        <For each={kind.options}>
          {(o) => (
            <label>
              <input type="radio" name={props.c.key} checked={Math.abs(o.value - value()) < 1e-9} onChange={() => setParam(props.c.key, o.value)} />
              <span>{o.label}</span>
            </label>
          )}
        </For>
      </div>
    </div>
  );
}

function CostLine(props: { k: string }) {
  const c = () => costs()[props.k];
  return (
    <p class={{ cost: true, stale: c()?.stale ?? true }} id={`cost-${props.k}`}>
      {c()?.text ?? "waiting for the plan…"}
    </p>
  );
}

function ConstraintRow(props: { c: Constraint }) {
  const m = meta()!;
  const v: VarMeta | undefined = m.vars.find((x) => x.key === props.c.key);
  const o = m.outs.find((x) => x.key === props.c.key);
  const key = props.c.key;
  const label = v ? v.short : `Result: ${o?.short ?? key}`;
  const help = v?.help ?? "";
  const ops: Op[] = v ? v.ops : ["min", "eq", "max"];
  const id = `in-val_${key}`;
  const choice = v?.kind.type === "choice" ? v.kind : null;
  const range = v ? (v.kind.type === "range" ? v.kind : { min: 0, max: 1, step: 0.01 }) : { min: o!.min, max: o!.max, step: o!.step };
  const spec = {
    id,
    min: range.min,
    max: range.max,
    step: range.step,
    get value() {
      return props.c.value;
    },
    fmt: v ? v.fmt : "int",
    onChange: (x: number, live: boolean) => setConstraint(key, { value: x }, live),
  } as const;
  return (
    <div class="ctl cons" data-key={key}>
      <div class="cons-top">
        <label class="cons-label" data-tip={help} for={id}>
          {label}
        </label>
        <button class="x" type="button" data-tip="Remove this constraint" aria-label="Remove this constraint" onClick={() => removeConstraint(key)}>
          ×
        </button>
      </div>
      <Show
        when={choice}
        fallback={
          <div class="cons-ctl">
            <Show when={ops.length > 1} fallback={<span class="op-fixed">{OP_LABEL[ops[0]]}</span>}>
              <select class="op" aria-label="Constraint type" value={props.c.op} onChange={(e) => setConstraintOp(key, e.currentTarget.value as Op)}>
                <For each={ops}>{(op) => <option value={op}>{OP_LABEL[op]}</option>}</For>
              </select>
            </Show>
            <Range {...spec} />
            <NumberBox {...spec} unit={o ? o.unit : undefined} />
          </div>
        }
      >
        {(ch) => (
          <div class="seg" role="radiogroup">
            <For each={ch().options}>
              {(opt) => (
                <label>
                  <input type="radio" name={`val_${key}`} checked={Math.abs(opt.value - props.c.value) < 1e-9} onChange={() => setConstraint(key, { value: opt.value })} />
                  <span>{opt.label}</span>
                </label>
              )}
            </For>
          </div>
        )}
      </Show>
      <CostLine k={key} />
    </div>
  );
}

function AddConstraint() {
  const m = meta()!;
  const has = (k: string) => request()?.constraints.some((c) => c.key === k) ?? false;
  return (
    <select
      class="addcon"
      aria-label="Add a constraint"
      onChange={(e) => {
        const k = e.currentTarget.value;
        e.currentTarget.value = "";
        if (k) void addConstraint(k);
      }}
    >
      <option value="">+ Constraint</option>
      <For each={m.var_groups}>
        {(g) => (
          <Show when={m.vars.some((v) => v.group === g && !has(v.key))}>
            <optgroup label={g}>
              <For each={m.vars.filter((v) => v.group === g && !has(v.key))}>{(v) => <option value={v.key}>{v.short}</option>}</For>
            </optgroup>
          </Show>
        )}
      </For>
      <Show when={m.outs.some((o) => !has(o.key))}>
        <optgroup label="Results">
          <For each={m.outs.filter((o) => !has(o.key))}>{(o) => <option value={o.key}>{o.short}</option>}</For>
        </optgroup>
      </Show>
    </select>
  );
}

export function Sidebar() {
  const m = meta()!;
  const objective = () => m.objectives.find((o) => o.key === request()?.objective) ?? m.objectives[0];
  return (
    <aside>
      <PresetBar />
      <form id="form" onSubmit={(e) => e.preventDefault()}>
        <details open>
          <summary>Objective</summary>
          <div class="objs" role="radiogroup" aria-label="Objective">
            <For each={m.objectives}>
              {(o) => (
                <label class="obj">
                  <input type="radio" name="objective" value={o.key} checked={request()?.objective === o.key} onChange={() => void setObjective(o.key)} />
                  <span class="obj-body">
                    <strong>{o.label}</strong>
                    <small>{o.blurb}</small>
                  </span>
                </label>
              )}
            </For>
          </div>
        </details>
        <details open>
          <summary>{m.space.title}</summary>
          <For each={m.space.ctls}>{(c) => <ControlRow c={c} />}</For>
        </details>
        <details open>
          <summary>Constraints</summary>
          <p class="note">{objective().constraints_note}</p>
          <For each={request()?.constraints ?? []} keyed={(c) => c.key}>
            {(c) => <ConstraintRow c={c()} />}
          </For>
          <Show when={(request()?.constraints.length ?? 0) === 0}>
            <p class="note">No constraints: everything is free.</p>
          </Show>
          <AddConstraint />
        </details>
        <details>
          <summary>Assumptions</summary>
          <p class="note">Fixed inputs with typical NZ White and WA values pre-loaded. Change them if yours differ.</p>
          <For each={m.assumptions}>
            {(g) => (
              <details open={g.open}>
                <summary>{g.title}</summary>
                <Show when={g.note}>
                  <p class="note">{g.note}</p>
                </Show>
                <For each={g.ctls}>{(c) => <ControlRow c={c} />}</For>
              </details>
            )}
          </For>
        </details>
      </form>
      <div class="actions" style={{ "margin-top": "10px" }}>
        <button type="button" onClick={restoreDefaults}>
          Restore starting values
        </button>
      </div>
    </aside>
  );
}
