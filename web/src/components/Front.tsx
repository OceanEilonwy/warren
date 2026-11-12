// The front screen: three questions (space, what matters, a headcount cap) beside the herd they give.
import { createSignal, For, Show } from "solid-js";
import type { SolveView } from "../api/types";
import { fmtNum, num, render, trim1 } from "../format";
import { busy, go, meta, removeConstraint, request, setLimit, setObjective, solution } from "../state";
import { ControlRow, CostLine } from "./Controls";
import { NumberBox, Range } from "./Slider";

const OBJ_LABEL: Record<string, string> = { max_meat: "Most meat", max_qol: "Most room" };

function SpaceCard() {
  const m = meta()!;
  const [open, setOpen] = createSignal(false);
  const p = () => request()!.params;
  // a little plan of the space, to scale, with the access line when both sides are reachable
  const box = () => {
    const w = num(p().width_m);
    const d = num(p().depth_m);
    const s = Math.min(96 / w, 64 / d);
    return { w: Math.max(w * s, 16), d: Math.max(d * s, 12) };
  };
  return (
    <section class="card step">
      <div class="step-head">
        <h2>
          <span class="n">1</span> Your space
        </h2>
        <button type="button" class="soft" aria-expanded={open() ? "true" : "false"} onClick={() => void setOpen(!open())}>
          {open() ? "Done" : "Change"}
        </button>
      </div>
      <Show
        when={open()}
        fallback={
          <div class="space-sum">
            <div class="space-box" aria-hidden="true" style={{ width: `${box().w}px`, height: `${box().d}px` }}>
              <Show when={p().access_both_sides}>
                <i />
              </Show>
            </div>
            <p>
              <strong>
                {trim2(num(p().width_m))} × {trim2(num(p().depth_m))} m
              </strong>
              , {trim2(num(p().height_m))} m tall
              <br />
              <span class="muted">{p().access_both_sides ? "reach in from both sides" : "reach in from one side"}</span>
            </p>
          </div>
        }
      >
        <For each={m.space.ctls}>{(c) => <ControlRow c={c} />}</For>
      </Show>
    </section>
  );
}

const trim2 = (v: number) => v.toFixed(2).replace(/\.?0+$/, "");

function MattersCard() {
  const m = meta()!;
  const qol = m.vars.find((v) => v.key === "qol")!;
  const range = qol.kind.type === "range" ? qol.kind : { min: 1, max: 3, step: 0.05 };
  const obj = () => request()!.objective;
  const qolLimit = () => request()!.constraints.find((c) => c.key === "qol");
  const floor = () => qolLimit()?.value ?? range.min;
  const limitOf = (k: string) => request()!.constraints.find((c) => c.key === k);
  const spec = {
    id: "room",
    min: range.min,
    max: Math.min(range.max, 2),
    step: range.step,
    get value() {
      return floor();
    },
    fmt: qol.fmt,
    onChange: (v: number, live: boolean) => (v <= range.min + 1e-9 ? removeConstraint("qol") : setLimit("qol", "min", v, live)),
  };
  return (
    <section class="card step">
      <h2>
        <span class="n">2</span> What matters more?
      </h2>
      <div class="pills wide" role="radiogroup" aria-label="What matters more">
        <For each={m.objectives}>
          {(o) => (
            <label class="pill">
              <input type="radio" name="objective" checked={obj() === o.key} onChange={() => void setObjective(o.key)} />
              <span>{OBJ_LABEL[o.key] ?? o.label}</span>
            </label>
          )}
        </For>
      </div>
      <Show
        when={obj() === "max_meat"}
        fallback={
          <p class="hint">
            Warren gives the rabbits as much room as it can while still getting
            <Show when={limitOf("meat")} fallback=" what you ask for">
              {(c) => <> at least {fmtNum(c().value, 0)} kg of meat a year</>}
            </Show>
            <Show when={limitOf("peak_head")}>{(c) => <> and a herd of {fmtNum(c().value, 0)} or more</>}</Show>. Change those under{" "}
            <a href="#more" onClick={(e) => (e.preventDefault(), go("more"))}>
              Anything else
            </a>
            .
          </p>
        }
      >
        <div class="field">
          <div class="field-top">
            <label for="room">Room for every rabbit</label>
            <span class="val">{render("qol", floor())}</span>
          </div>
          <Range {...spec} />
          <div class="scale-ends">
            <span>Code minimum</span>
            <span>Double</span>
          </div>
        </div>
        <Show when={qolLimit()} fallback={<p class="hint">Every cage is the Code minimum. Slide right to give the rabbits more room.</p>}>
          <CostLine k="qol" />
        </Show>
      </Show>
    </section>
  );
}

function HeadcountCard() {
  const m = meta()!;
  const v = m.vars.find((x) => x.key === "max_headcount")!;
  const range = v.kind.type === "range" ? v.kind : { min: 0, max: 300, step: 1 };
  const lo = Math.max(range.min, 5);
  const cap = () => request()!.constraints.find((c) => c.key === "max_headcount");
  const start = () => Math.max(lo, Math.round(solution()?.metrics.peak_head?.mean ?? 60));
  const spec = {
    id: "cap",
    min: lo,
    max: range.max,
    step: range.step,
    get value() {
      return cap()?.value ?? start();
    },
    fmt: "int" as const,
    onChange: (x: number, live: boolean) => setLimit("max_headcount", "max", x, live),
  };
  return (
    <section class="card step">
      <h2>
        <span class="n">3</span> Is there a most you can look after at once?
      </h2>
      <div class="pills" role="radiogroup" aria-label="Is there a most you can look after at once?">
        <label class="pill">
          <input type="radio" name="cap" checked={!cap()} onChange={() => removeConstraint("max_headcount")} />
          <span>No limit</span>
        </label>
        <label class="pill">
          <input type="radio" name="cap" checked={!!cap()} onChange={() => setLimit("max_headcount", "max", start())} />
          <span>Yes</span>
        </label>
      </div>
      <Show when={cap()}>
        <div class="cap-row">
          <label for="num-cap">No more than</label>
          <NumberBox {...spec} unit="rabbits" />
        </div>
        <Range {...spec} />
        <CostLine k="max_headcount" />
        <p class="hint">Counts every rabbit, kits included. Does are only mated when there will be room for the litter.</p>
      </Show>
    </section>
  );
}

function Rabbit(props: { kind: "doe" | "buck" | "young"; size: number }) {
  return (
    <svg class={`rabbit ${props.kind}`} width={props.size} height={(props.size * 30) / 26} viewBox="0 0 26 30" aria-hidden="true">
      <Show
        when={props.kind !== "young"}
        fallback={
          <>
            <ellipse cx="9" cy="9" rx="2.4" ry="6.5" />
            <ellipse cx="17" cy="9" rx="2.4" ry="6.5" />
            <ellipse cx="13" cy="21" rx="9" ry="8" />
          </>
        }
      >
        <ellipse class="body" cx="9" cy="8" rx="2.6" ry="7.5" transform="rotate(-10 9 8)" />
        <ellipse class="body" cx="17" cy="8" rx="2.6" ry="7.5" transform="rotate(10 17 8)" />
        <Show when={props.kind === "doe"}>
          <ellipse class="ear" cx="9.1" cy="8.5" rx="1" ry="5" transform="rotate(-10 9 8)" />
          <ellipse class="ear" cx="16.9" cy="8.5" rx="1" ry="5" transform="rotate(10 17 8)" />
        </Show>
        <ellipse class="body" cx="13" cy="21" rx="9" ry="8" />
        <circle class="eye" cx="10" cy="20" r="1" />
        <circle class="eye" cx="16" cy="20" r="1" />
      </Show>
    </svg>
  );
}

const MAX_GLYPHS = 120;

function HerdCard(props: { s: SolveView }) {
  const p = () => props.s.params;
  const does = () => num(p().does);
  const bucks = () => num(p().bucks);
  // rabbits on hand on an average day once the herd is running, less the breeders
  const young = () => {
    const h = props.s.series.head.mean.slice(props.s.warm_weeks);
    const avg = h.length ? h.reduce((a, b) => a + b, 0) / h.length : 0;
    return Math.max(0, Math.round(avg - does() - bucks()));
  };
  const met = (k: string) => props.s.metrics[k]?.mean ?? 0;
  const tip = (k: string, unit: string, dp: number, cash = "") =>
    `80% of simulated years: ${cash}${fmtNum(props.s.metrics[k].p10, dp)}–${cash}${fmtNum(props.s.metrics[k].p90, dp)} ${unit}`;
  const meanQol = () => ["qol_doe", "qol_buck", "qol_young", "qol_old"].reduce((a, k) => a + num(p()[k]), 0) / 4;
  const bad = () => props.s.checks.filter((c) => !c.ok).length;
  const butcher = () =>
    p().butcher_rule === "room"
      ? `take the oldest youngsters when the cages get full (all by ${trim1(num(p().butcher_days) / 7)} weeks)`
      : `butcher at ${trim1(num(p().butcher_days) / 7)} weeks`;
  const empty = () => !props.s.layout.feasible || props.s.eff_does === 0;
  return (
    <section class={{ card: true, herd: true, busy: busy() > 0 }}>
      <div>
        <h1>Here's your herd</h1>
        <p class="lede">
          On a typical day, in {props.s.layout.tiers} tier{props.s.layout.tiers === 1 ? "" : "s"} of cages.
        </p>
      </div>
      <Show when={props.s.violations.length > 0}>
        <div class="warn">
          <strong>Some of your answers can't all be met.</strong> This is the closest Warren found:
          <ul>
            <For each={props.s.violations}>{(v) => <li>{v}</li>}</For>
          </ul>
        </div>
      </Show>
      <Show when={props.s.forced_share > 0.03}>
        <div class="warn">
          <strong>It's a squeeze:</strong> {(props.s.forced_share * 100).toFixed(0)}% of youngsters would have to go early because the cages are full.
        </div>
      </Show>
      <Show
        when={!empty()}
        fallback={
          <div class="warn">
            <strong>No rabbits fit in this space.</strong> {props.s.layout.issues.join(" ")}
          </div>
        }
      >
        <div class="flock">
          <div class="grp">
            <div class="glyphs">
              <For each={Array.from({ length: Math.min(does(), 24) })}>{() => <Rabbit kind="doe" size={44} />}</For>
            </div>
            <div class="grp-l">
              {does()} doe{does() === 1 ? "" : "s"}
            </div>
          </div>
          <div class="grp">
            <div class="glyphs">
              <For each={Array.from({ length: Math.min(bucks(), 3) })}>{() => <Rabbit kind="buck" size={44} />}</For>
            </div>
            <div class="grp-l">
              {bucks()} buck{bucks() === 1 ? "" : "s"}
            </div>
          </div>
          <div class="grp grow">
            <div class="glyphs small">
              <For each={Array.from({ length: Math.min(young(), MAX_GLYPHS) })}>{() => <Rabbit kind="young" size={16} />}</For>
            </div>
            <div class="grp-l">About {young()} kits and growers</div>
          </div>
        </div>
        <div class="tiles">
          <div class="tile blue" data-tip={tip("meat_kg_per_yr", "kg", 0)}>
            <div class="tv">{fmtNum(met("meat_kg_per_yr"), 0)} kg</div>
            <div class="tl">of meat a year</div>
          </div>
          <div class="tile pink" data-tip={tip("fryers_per_yr", "rabbits", 0)}>
            <div class="tv">{fmtNum(met("fryers_per_yr"), 0)}</div>
            <div class="tl">rabbits for the table each year</div>
          </div>
          <div class="tile green" data-tip={tip("cost_per_kg", "per kg", 2, "$")}>
            <div class="tv">${fmtNum(met("cost_per_kg"), 2)}</div>
            <div class="tl">per kg, in feed</div>
          </div>
        </div>
        <p class="summary">
          Wean at {trim1(props.s.eff_wean / 7)} weeks, mate again {String(p().rebreed_days)} days after each litter, and {butcher()}.
          <Show when={meanQol() > 1 + 1e-9}> Cages average {render("qol", meanQol())} more floor space than the Code minimum.</Show>{" "}
          <Show
            when={bad() === 0}
            fallback={
              <a class="bad" href="#workings" onClick={(e) => (e.preventDefault(), go("workings"))}>
                {bad()} Code of Practice issue{bad() === 1 ? "" : "s"}: see the workings.
              </a>
            }
          >
            <span class="good">It all meets the WA Code of Practice.</span>
          </Show>
        </p>
      </Show>
      <div class="grass" aria-hidden="true" />
    </section>
  );
}

export function Front() {
  return (
    <div class="front">
      <div class="steps">
        <SpaceCard />
        <MattersCard />
        <HeadcountCard />
        <a
          class="card more-link"
          href="#more"
          onClick={(e) => {
            e.preventDefault();
            go("more");
          }}
        >
          <span>Anything else to keep in mind?</span>
          <span class="plus" aria-hidden="true">
            +
          </span>
          <Show when={(request()?.constraints.filter((c) => !["qol", "max_headcount"].includes(c.key)).length ?? 0) > 0}>
            <span class="count">{request()!.constraints.filter((c) => !["qol", "max_headcount"].includes(c.key)).length} set</span>
          </Show>
        </a>
      </div>
      <Show
        when={solution()}
        fallback={
          <section class="card herd pending">
            <div class="big-spinner" aria-hidden="true" />
            <p>Planning your herd…</p>
            <p class="muted">Running hundreds of randomised simulations to find the best plan.</p>
          </section>
        }
      >
        {(s) => <HerdCard s={s()} />}
      </Show>
    </div>
  );
}
