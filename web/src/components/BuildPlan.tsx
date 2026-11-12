// The build plan: each tier and row as a strip with inside sizes, how the tiers stack, the cages and a
// parts list. Everything comes from the solved layout.
import { For, Show } from "solid-js";
import type { Cage, SolveView } from "../api/types";
import { num } from "../format";

const cm = (m: number) => Math.round(m * 100);
const m2 = (a: number) => `${a.toFixed(2)} m²`;
const KIND_NAME = { doe: "doe + litter", buck: "buck", grow: "growers" } as const;
/** WA Code areas, m². */
const YOUNG = 0.07;
const OLDER = 0.18;

interface Named extends Cage {
  id: string;
  i: number;
}

export function BuildPlan(props: { s: SolveView }) {
  const l = () => props.s.layout;
  const p = () => props.s.params;
  const W = () => num(p().width_m);
  const H = () => num(p().cage_height_cm);
  const over = () => num(p().tier_overhead_cm);
  const nest = () => num(p().nest_box_m2);
  const named = (): Named[] => {
    const n = { doe: 0, buck: 0, grow: 0 };
    const pre = { doe: "D", buck: "B", grow: "G" };
    // number bottom tier first, front row first, left to right
    const order = l()
      .cages.map((c, i) => ({ ...c, i }))
      .sort((a, b) => a.tier - b.tier || a.row - b.row || a.x - b.x);
    const ids = new Map<number, string>();
    for (const c of order) ids.set(c.i, `${pre[c.kind]}${++n[c.kind]}`);
    return l().cages.map((c, i) => ({ ...c, i, id: ids.get(i)! }));
  };
  const strips = () => {
    const out: { tier: number; row: number; cages: Named[] }[] = [];
    for (let t = l().tiers - 1; t >= 0; t--)
      for (const r of l().rows === 2 ? [1, 0] : [0]) out.push({ tier: t, row: r, cages: named().filter((c) => c.tier === t && c.row === r).sort((a, b) => a.x - b.x) });
    return out;
  };
  const tierName = (t: number) => (l().tiers === 1 ? "The tier" : t === 0 ? "Bottom tier" : t === l().tiers - 1 ? "Top tier" : `Tier ${t + 1}`);
  const side = (r: number) => (l().rows === 1 ? "One row" : r === 0 ? "Front row" : "Back row");
  const floorAt = (t: number) => Math.round(over() + t * (H() + over()));
  const frameH = () => l().tiers * (H() + over());
  const spare = () => Math.max(0, Math.round(num(p().height_m) * 100 - frameH()));

  // ---- parts ----
  const parts = () => {
    const cs = l().cages;
    const rd = l().row_depth;
    const h = H() / 100;
    const floor = cs.reduce((a, c) => a + c.area, 0);
    const kitten = cs.filter((c) => c.kind === "doe").reduce((a, c) => a + c.area, 0);
    const slots = new Map<string, number>();
    for (const c of cs) slots.set(`${c.tier}-${c.row}`, (slots.get(`${c.tier}-${c.row}`) ?? 0) + 1);
    const dividers = [...slots.values()].reduce((a, n) => a + Math.max(0, n - 1), 0);
    const depthAll = l().rows * rd;
    const perTier = 2 * (W() + depthAll) * h + (l().rows === 2 ? W() * h : 0) + W() * depthAll;
    const wallMesh = l().tiers * perTier + dividers * rd * h;
    const front = cs.filter((c) => c.row === 0).length;
    const nestW = nest() > 0 ? Math.sqrt((nest() * 4) / 3) : 0;
    return {
      floor,
      kitten,
      wallMesh,
      dividers,
      front,
      back: cs.length - front,
      doors: cs.length,
      trays: l().tiers,
      nests: l().n_doe,
      nestSize: nest() > 0 ? `${cm(nestW)} × ${cm(nest() / nestW)} cm` : "",
      boards: cs.length,
      depthAll,
    };
  };

  // ---- cage schedule: group cages of the same kind and size ----
  const schedule = () => {
    const groups = new Map<string, Named[]>();
    for (const c of named()) {
      const k = `${c.kind}|${cm(c.w)}`;
      groups.set(k, [...(groups.get(k) ?? []), c]);
    }
    return [...groups.values()]
      .sort((a, b) => "dbg".indexOf(a[0].kind[0]) - "dbg".indexOf(b[0].kind[0]) || b[0].w - a[0].w)
      .map((g) => {
        const c = g[0];
        const holds =
          c.kind === "grow" ? `${Math.floor(c.area / YOUNG + 1e-9)} to 12 wk, ${Math.floor(c.area / OLDER + 1e-9)} after` : KIND_NAME[c.kind];
        return { ids: g.map((x) => x.id).join(", "), size: `${cm(c.w)} × ${cm(l().row_depth)} × ${Math.round(H())}`, area: m2(c.area), holds };
      });
  };

  return (
    <div class="plan-sheet">
      <div class="plan-strips">
        <p class="hint" style={{ margin: "0 0 6px" }}>
          Inside sizes in cm, to scale, as you face the front. Rows meet back to back; each cage's door is on its row's side.
        </p>
        <For each={strips()}>
          {(s) => (
            <div class="strip">
              <div class="strip-head">
                <strong>{tierName(s.tier)}</strong> · {side(s.row)}
                <span class="muted"> · cage floor {floorAt(s.tier)} cm off the ground</span>
              </div>
              <div class="strip-dims" aria-hidden="true">
                <For each={s.cages}>
                  {(c) => (
                    <div style={{ flex: `${c.w} 1 0` }}>
                      <span>{cm(c.w)}</span>
                    </div>
                  )}
                </For>
              </div>
              <div class="strip-body">
                <For each={s.cages}>
                  {(c) => (
                    <div class={`cell ${c.kind}`} style={{ flex: `${c.w} 1 0` }}>
                      <div class="cell-top">
                        <strong>{c.id}</strong>
                        <span>{m2(c.area)}</span>
                      </div>
                      <div class="cell-use">{KIND_NAME[c.kind]}</div>
                      <Show when={c.kind === "doe" && nest() > 0}>
                        <span class={{ "nest-tag": true, back: s.row === 0 || l().rows === 1 }}>nest box</span>
                      </Show>
                    </div>
                  )}
                </For>
              </div>
            </div>
          )}
        </For>
        <div class="strip-foot muted">
          <span>← {cm(W())} cm wide →</span>
          <span>each row {cm(l().row_depth)} cm deep</span>
        </div>
      </div>

      <div class="plan-side">
        <section>
          <h3>How it stacks</h3>
          <div class="stack">
            <div class="stack-bar" aria-hidden="true">
              <Show when={spare() > 0}>
                <div class="spare" style={{ flex: `${spare()} 1 0` }} />
              </Show>
              <For each={Array.from({ length: l().tiers }, (_, i) => l().tiers - 1 - i)}>
                {() => (
                  <>
                    <div class="cage" style={{ flex: `${H()} 1 0` }} />
                    <div class="tray" style={{ flex: `${over()} 1 0` }} />
                  </>
                )}
              </For>
            </div>
            <div class="stack-labels">
              <Show when={spare() > 0}>
                <div style={{ flex: `${spare()} 1 0` }}>{spare()} cm spare</div>
              </Show>
              <For each={Array.from({ length: l().tiers }, (_, i) => l().tiers - 1 - i)}>
                {(t) => (
                  <>
                    <div style={{ flex: `${H()} 1 0` }}>
                      <strong>{tierName(t)}</strong>: {Math.round(H())} cm cages
                    </div>
                    <div style={{ flex: `${over()} 1 0` }}>{Math.round(over())} cm floor, tray, gap</div>
                  </>
                )}
              </For>
            </div>
          </div>
        </section>
        <section>
          <h3>What you'll need</h3>
          <table class="parts">
            <tbody>
              <tr>
                <td>Frame, {l().tiers} tier{l().tiers === 1 ? "" : "s"}</td>
                <td>
                  {cm(W())} × {cm(parts().depthAll)} × {Math.round(frameH())} cm
                </td>
              </tr>
              <tr>
                <td>Floor mesh, ≤ 19 × 19 mm, 2.5 mm wire</td>
                <td>{m2(parts().floor)}</td>
              </tr>
              <tr>
                <td>…of it 13 × 13 mm for kittens (doe cages)</td>
                <td>{m2(parts().kitten)}</td>
              </tr>
              <tr>
                <td>Wall and roof mesh</td>
                <td>about {Math.ceil(parts().wallMesh)} m²</td>
              </tr>
              <tr>
                <td>
                  Dividers, {cm(l().row_depth)} × {Math.round(H())} cm
                </td>
                <td>{parts().dividers}</td>
              </tr>
              <tr>
                <td>
                  Doors{l().rows === 2 ? `: ${parts().front} front, ${parts().back} back` : ""}
                </td>
                <td>{parts().doors}</td>
              </tr>
              <tr>
                <td>
                  Droppings trays, {cm(W())} × {cm(parts().depthAll)} cm
                </td>
                <td>{parts().trays}</td>
              </tr>
              <Show when={parts().nests > 0 && nest() > 0}>
                <tr>
                  <td>Nest boxes, about {parts().nestSize}</td>
                  <td>{parts().nests}</td>
                </tr>
              </Show>
              <tr>
                <td>Resting boards, ≥ 0.1 m², ≤ ⅓ of the floor</td>
                <td>{parts().boards}</td>
              </tr>
            </tbody>
          </table>
        </section>
        <section>
          <h3>Cages</h3>
          <table class="parts sched">
            <thead>
              <tr>
                <th>Cage</th>
                <th>Inside, cm</th>
                <th>Floor</th>
                <th>Holds</th>
              </tr>
            </thead>
            <tbody>
              <For each={schedule()}>
                {(r) => (
                  <tr>
                    <td>
                      <strong>{r.ids}</strong>
                    </td>
                    <td>{r.size}</td>
                    <td>{r.area}</td>
                    <td>{r.holds}</td>
                  </tr>
                )}
              </For>
            </tbody>
          </table>
          <Show when={nest() > 0 && l().n_doe > 0}>
            <p class="hint">
              Doe cages include {m2(nest())} for the nest box, leaving at least {m2(l().doe_floor)} of floor for the doe and litter.
            </p>
          </Show>
        </section>
        <button type="button" class="soft" onClick={() => window.print()}>
          Print the plan
        </button>
      </div>
    </div>
  );
}
