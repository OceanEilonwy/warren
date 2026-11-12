// The results panel: warnings, the plan, headline figures, the cage layout and the weekly charts.
import { For, Show } from "solid-js";
import type { CageKind, CageRole, SolveView, VarMeta } from "../api/types";
import { fmtNum, num, trim1, valueText } from "../format";
import { meta, request } from "../state";
import { Chart } from "./Chart";

const ROLE_LABEL: Record<CageRole, string> = {
  doe_litter: "Doe + litter",
  doe: "Doe",
  buck: "Buck",
  replacement: "Replacement",
  growers: "Growers",
  empty: "Empty",
};
const ROLE_VAR: Record<CageRole, string> = {
  doe_litter: "--s1",
  buck: "--s2",
  doe: "--s3",
  replacement: "--s4",
  growers: "--s5",
  empty: "--empty",
};
const LEGEND: CageRole[] = ["doe_litter", "buck", "doe", "replacement", "growers", "empty"];
const KIND_LABEL: Record<CageKind, string> = { doe: "doe cage", buck: "buck cage", grow: "grower cage" };

interface StatDef {
  key: string;
  icon: string;
  label: string;
  unit: string;
  dp: number;
  scale?: number;
  money?: boolean;
}

const THEMES: [string, StatDef[]][] = [
  [
    "Meat",
    [
      { key: "meat_kg_per_yr", icon: "🥩", label: "Dressed meat", unit: "kg/yr", dp: 0 },
      { key: "fryers_per_yr", icon: "🍖", label: "Rabbits butchered", unit: "/yr", dp: 0 },
      { key: "butcher_age_wk", icon: "📅", label: "Age at butchering", unit: "wk", dp: 1 },
      { key: "forced_per_yr", icon: "⚠️", label: "Butchered early (no room)", unit: "/yr", dp: 1 },
    ],
  ],
  [
    "Food",
    [
      { key: "pellet_kg_per_yr", icon: "🌾", label: "Pellets", unit: "kg/yr", dp: 0 },
      { key: "hay_kg_per_yr", icon: "🌿", label: "Hay", unit: "kg/yr", dp: 0 },
      { key: "feed_cost_per_yr", icon: "💵", label: "Feed cost", unit: "/yr", dp: 0, money: true },
      { key: "cost_per_kg", icon: "⚖️", label: "Cost per kg of meat", unit: "/kg", dp: 2, money: true },
    ],
  ],
  [
    "Herd",
    [
      { key: "peak_head", icon: "🐇", label: "Peak rabbits on hand", unit: "", dp: 0 },
      { key: "cage_use", icon: "🏠", label: "Cages in use", unit: "%", dp: 0, scale: 100 },
    ],
  ],
];

function Notes(props: { s: SolveView }) {
  return (
    <div id="lever-diff">
      <Show when={props.s.violations.length > 0}>
        <div class="warn">
          <strong>! These constraints can't all be met</strong> (closest plan shown):
          <ul>
            <For each={props.s.violations}>{(v) => <li>{v}</li>}</For>
          </ul>
        </div>
      </Show>
      <Show when={props.s.forced_share > 0.03}>
        <div class="warn">
          <strong>! This plan doesn't fit:</strong> {(props.s.forced_share * 100).toFixed(0)}% of rabbits are butchered early because the cages are
          full. The figures assume you do that; consider the "when room is needed" rule, fewer does or a younger butchering age.
        </div>
      </Show>
    </div>
  );
}

function PlanCard(props: { s: SolveView }) {
  const m = meta()!;
  const p = () => props.s.params;
  const varMeta = (k: string) => m.vars.find((v) => v.key === k) as VarMeta;
  const t = (k: string) => {
    const v = varMeta(k);
    return valueText(v.kind, v.fmt, num(p()[k]));
  };
  const cons = () => request()?.constraints ?? [];
  const mark = (k: string) => {
    const own = cons().find((c) => c.key === k);
    const qolType = ["qol_doe", "qol_buck", "qol_young", "qol_old"].includes(k);
    if (own?.op === "eq") return <span class="pin" data-tip="Pinned by you">🔒</span>;
    if (own || (qolType && cons().some((c) => c.key === "qol"))) return <span class="lim" data-tip="Within a limit you set">↕</span>;
    return null;
  };
  const row = (label: string, k: string | null, text: string) => (
    <div class="pr">
      <span class="pl">
        {label}
        {k && mark(k)}
      </span>
      <span class="pv">{text}</span>
    </div>
  );
  const meanQol = () => ["qol_doe", "qol_buck", "qol_young", "qol_old"].reduce((a, k) => a + num(p()[k]), 0) / 4;
  const big = () =>
    request()?.objective === "max_qol"
      ? meanQol() <= 1 + 1e-9
        ? "Code minimum"
        : `+${((meanQol() - 1) * 100).toFixed(0)}% floor space`
      : `${fmtNum(props.s.objective_value, 0)} kg/yr`;
  const small = () =>
    request()?.objective === "max_qol"
      ? "floor space vs the Code minimum — the most that still meets your result limits"
      : "dressed meat — the most this space gives within your constraints";
  const l = () => props.s.layout;
  const butcher = () => {
    const wk = trim1(num(p().butcher_days) / 7);
    return p().butcher_rule === "room" ? `when room is needed, by ${wk} wk` : `at ${wk} wk`;
  };
  return (
    <section class="card plan">
      <div class="plan-head">
        <div>
          <div class="big">{big()}</div>
          <div class="small">{small()}</div>
        </div>
        <Show when={props.s.floor_space_unlimited}>
          <p class="warn">
            Nothing you've set limits floor space, only what fits. Add a result constraint (dressed meat, or rabbits on hand) from “+ Constraint” to
            trade room against it.
          </p>
        </Show>
      </div>
      <div class="plan-cols">
        <div class="pc">
          <h3>Herd</h3>
          {row("Breeding does", "does", t("does"))}
          {row("Bucks", "bucks", t("bucks"))}
          <Show when={num(p().max_headcount) > 0}>{row("Max rabbits", "max_headcount", String(p().max_headcount))}</Show>
        </div>
        <div class="pc">
          <h3>Cages</h3>
          {row("Tiers", "tiers", String(l().tiers))}
          {row("Doe / buck / grower", null, `${l().n_doe} / ${l().n_buck} / ${l().n_grow}`)}
          {row("Grower width", "grower_cage_w", t("grower_cage_w"))}
        </div>
        <div class="pc">
          <h3>Timing</h3>
          {row("Wean at", "wean_days", t("wean_days"))}
          {row("Re-mate after", "rebreed_days", t("rebreed_days"))}
          {row("Butcher", "butcher_rule", butcher())}
        </div>
        <div class="pc">
          <h3>Floor space</h3>
          {row("Doe + litter", "qol_doe", t("qol_doe"))}
          {row("Buck", "qol_buck", t("qol_buck"))}
          {row("Growers 5–12 wk", "qol_young", t("qol_young"))}
          {row("Growers 12 wk+", "qol_old", t("qol_old"))}
        </div>
      </div>
    </section>
  );
}

function StatsCard(props: { s: SolveView }) {
  return (
    <section class="card stats">
      <For each={THEMES}>
        {([title, items]) => (
          <div class="theme">
            <h3>{title}</h3>
            <div class="theme-items">
              <For each={items}>
                {(it) => {
                  const st = () => props.s.metrics[it.key];
                  const sc = it.scale ?? 1;
                  const cash = it.money ? "$" : "";
                  const tip = () =>
                    `${it.label}: ${cash}${fmtNum(st().mean * sc, it.dp)} ${it.unit}\n80% of runs fall between ${cash}${fmtNum(st().p10 * sc, it.dp)} and ${cash}${fmtNum(st().p90 * sc, it.dp)}`;
                  return (
                    <div class="stat" data-tip={tip()}>
                      <span class="ico" aria-hidden="true">
                        {it.icon}
                      </span>
                      <span class="sv">
                        {cash}
                        {fmtNum(st().mean * sc, it.dp)}
                        <span class="su">{it.unit}</span>
                      </span>
                      <span class="sl">{it.label}</span>
                    </div>
                  );
                }}
              </For>
            </div>
          </div>
        )}
      </For>
    </section>
  );
}

function LayoutCard(props: { s: SolveView }) {
  const l = () => props.s.layout;
  const p = () => props.s.params;
  const geo = () => {
    const width = num(p().width_m);
    const depth = num(p().depth_m);
    const tw = 220;
    const gap = 28;
    const scale = Math.min(tw / width, 150 / depth);
    const pw = width * scale;
    const pd = depth * scale;
    return { scale, pw, pd, gap, w: Math.max(l().tiers, 1) * (pw + gap) - gap + 20, h: pd + 56 };
  };
  const snap = () => (props.s.snapshot.length === l().cages.length ? props.s.snapshot : null);
  const areaRange = (kind: CageKind) => {
    const a = l()
      .cages.filter((c) => c.kind === kind)
      .map((c) => c.area);
    if (a.length === 0) return "–";
    const lo = Math.min(...a);
    const hi = Math.max(...a);
    return hi - lo < 0.005 ? `${lo.toFixed(2)} m²` : `${lo.toFixed(2)}–${hi.toFixed(2)} m²`;
  };
  const tiers = () => Array.from({ length: l().tiers }, (_, t) => t);
  return (
    <section class="card">
      <h2>Cage layout</h2>
      <div class="layout-body">
        <div class="layout-plan">
          <div class="layout-svg">
            <Show when={l().tiers > 0} fallback={<p>No cages fit.</p>}>
              <svg viewBox={`0 0 ${geo().w.toFixed(0)} ${geo().h.toFixed(0)}`} width={geo().w.toFixed(0)} height={geo().h.toFixed(0)} role="img" aria-label="Plan view of each cage tier">
                <For each={tiers()}>
                  {(t) => {
                    const x0 = () => 10 + t * (geo().pw + geo().gap);
                    const y0 = 22;
                    const suffix = t === 0 ? " (bottom)" : t === l().tiers - 1 ? " (top)" : "";
                    return (
                      <>
                        <text x={x0().toFixed(1)} y="14" class="t-lab">
                          Tier {t + 1}
                          {suffix}
                        </text>
                        <rect x={x0().toFixed(1)} y={y0} width={geo().pw.toFixed(1)} height={geo().pd.toFixed(1)} class="space" />
                        <For each={l().cages.map((c, i) => ({ c, i })).filter(({ c }) => c.tier === t)}>
                          {({ c, i }) => {
                            const ch = () => l().row_depth * geo().scale;
                            const cw = () => c.w * geo().scale;
                            const x = () => x0() + c.x * geo().scale;
                            const y = () => (l().rows === 2 && c.row === 1 ? y0 : y0 + geo().pd - ch());
                            const role = () => snap()?.[i].role ?? "empty";
                            const count = () => snap()?.[i].count ?? 0;
                            const label = () =>
                              role() === "growers" ? `Growers ×${count()}` : role() === "doe_litter" ? `Doe + ${count() - 1}` : ROLE_LABEL[role()];
                            return (
                              <g data-tip={`${label()} — ${KIND_LABEL[c.kind]}, ${c.w.toFixed(2)} × ${c.d.toFixed(2)} m = ${c.area.toFixed(2)} m²`}>
                                <rect
                                  x={(x() + 1).toFixed(1)}
                                  y={(y() + 1).toFixed(1)}
                                  width={Math.max(cw() - 2, 0).toFixed(1)}
                                  height={Math.max(ch() - 2, 0).toFixed(1)}
                                  rx="4"
                                  class="cage"
                                  style={{ fill: `var(${ROLE_VAR[role()]})` }}
                                />
                                <Show when={cw() > 46}>
                                  <text x={(x() + cw() / 2).toFixed(1)} y={(y() + ch() / 2 + 1).toFixed(1)} class="c-lab" text-anchor="middle">
                                    {label()}
                                  </text>
                                  <text x={(x() + cw() / 2).toFixed(1)} y={(y() + ch() / 2 + 12).toFixed(1)} class="t-dim" text-anchor="middle">
                                    {c.area.toFixed(2)} m²
                                  </text>
                                </Show>
                              </g>
                            );
                          }}
                        </For>
                        <text x={(x0() + geo().pw / 2).toFixed(1)} y={(y0 + geo().pd + 16).toFixed(1)} class="t-dim" text-anchor="middle">
                          {l().rows === 2 ? "access front & back" : "access front ▾"}
                        </text>
                      </>
                    );
                  }}
                </For>
              </svg>
            </Show>
          </div>
          <div class="legend">
            <For each={LEGEND}>
              {(r) => (
                <span class="lg">
                  <i style={{ background: `var(${ROLE_VAR[r]})` }} />
                  {ROLE_LABEL[r]}
                </span>
              )}
            </For>
          </div>
        </div>
        <div class="layout-tables">
          <table class="facts-table">
            <thead>
              <tr>
                <th>Cages</th>
                <th class="num">No.</th>
                <th class="num">Each</th>
              </tr>
            </thead>
            <tbody>
              <tr>
                <td>Doe + litter</td>
                <td class="num">{l().n_doe}</td>
                <td class="num">{areaRange("doe")}</td>
              </tr>
              <tr>
                <td>Buck</td>
                <td class="num">{l().n_buck}</td>
                <td class="num">{areaRange("buck")}</td>
              </tr>
              <tr>
                <td>Grower</td>
                <td class="num">{l().n_grow}</td>
                <td class="num">{areaRange("grow")}</td>
              </tr>
            </tbody>
            <tfoot>
              <tr>
                <td>Total</td>
                <td class="num">{l().cages.length}</td>
                <td class="num" />
              </tr>
            </tfoot>
          </table>
          <table class="facts-table">
            <thead>
              <tr>
                <th colspan="2">Space</th>
              </tr>
            </thead>
            <tbody>
              <tr>
                <td>Tiers</td>
                <td class="num">
                  {l().tiers} of {l().max_tiers} that fit in {num(p().height_m).toFixed(2)} m
                </td>
              </tr>
              <tr>
                <td>Rows</td>
                <td class="num">
                  {l().rows}
                  {l().rows === 2 ? " (front and back)" : ""}
                </td>
              </tr>
              <tr>
                <td>Cage depth</td>
                <td class="num">{l().row_depth.toFixed(2)} m</td>
              </tr>
              <tr>
                <td>Cage height</td>
                <td class="num">{String(p().cage_height_cm)} cm</td>
              </tr>
              <Show when={l().unused_depth > 0.01}>
                <tr>
                  <td>Out of reach</td>
                  <td class="num">{l().unused_depth.toFixed(2)} m of depth, unused</td>
                </tr>
              </Show>
            </tbody>
          </table>
        </div>
      </div>
    </section>
  );
}

function Charts(props: { s: SolveView }) {
  const s = () => props.s.series;
  return (
    <div class="charts">
      <section class="card">
        <h2>Rabbits on hand</h2>
        <p class="sub">Average across runs; shaded band = 80% of runs. Annual figures exclude the shaded start-up period.</p>
        <div class="ch-legend">
          <i style={{ background: "var(--s1)" }} />
          All rabbits (incl. unweaned kits)
          <i style={{ background: "var(--s2)" }} />
          Growers
        </div>
        <div class="chart">
          <Chart
            spec={{
              kind: "line",
              dp: 0,
              warmWeeks: props.s.warm_weeks,
              series: [
                { name: "All rabbits", colour: 1, mean: s().head.mean, band: [s().head.p10, s().head.p90] },
                { name: "Growers", colour: 2, mean: s().growers.mean },
              ],
            }}
          />
        </div>
      </section>
      <div class="grid2">
        <section class="card">
          <h2>Dressed meat per week (kg)</h2>
          <p class="sub">Average across runs.</p>
          <div class="chart">
            <Chart spec={{ kind: "bar", dp: 1, warmWeeks: props.s.warm_weeks, series: [{ name: "Dressed meat (kg)", colour: 1, mean: s().meat.mean }] }} />
          </div>
        </section>
        <section class="card">
          <h2>Cages in use</h2>
          <p class="sub">Average across runs; band = 80% of runs.</p>
          <div class="chart">
            <Chart
              spec={{
                kind: "line",
                dp: 1,
                warmWeeks: props.s.warm_weeks,
                capacity: [props.s.layout.cages.length, `${props.s.layout.cages.length} cages available`],
                series: [{ name: "Cages in use", colour: 3, mean: s().cages.mean, band: [s().cages.p10, s().cages.p90] }],
              }}
            />
          </div>
        </section>
      </div>
    </div>
  );
}

export function Results(props: { s: SolveView }) {
  const empty = () => !props.s.layout.feasible || props.s.eff_does === 0;
  return (
    <>
      <Notes s={props.s} />
      <Show when={empty()}>
        <div id="infeasible-msg">
          <strong>No production possible with this cage layout.</strong> {props.s.layout.issues.join(" ")}
        </div>
      </Show>
      <Show when={!empty()}>
        <PlanCard s={props.s} />
        <StatsCard s={props.s} />
      </Show>
      <LayoutCard s={props.s} />
      <Show when={!empty()}>
        <Charts s={props.s} />
      </Show>
    </>
  );
}
