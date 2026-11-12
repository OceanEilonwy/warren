// Hand-rolled SVG charts: weekly series with an optional 80% band, a shaded start-up period, and one
// hover column per week (tooltip with that week's numbers).
import { For, Show } from "solid-js";
import { fmtNum } from "../format";
import { niceStep } from "./chart-math";

export interface ChartSeries {
  name: string;
  /** 1, 2 or 3: picks the colour. */
  colour: 1 | 2 | 3;
  mean: number[];
  band?: [number[], number[]];
}

export interface ChartSpec {
  series: ChartSeries[];
  kind: "line" | "bar";
  dp: number;
  warmWeeks: number;
  capacity?: [number, string];
}

const W = 640;
const H = 220;
const ML = 44;
const MR = 12;
const MT = 12;
const MB = 28;

export function Chart(props: { spec: ChartSpec }) {
  const geo = () => {
    const s = props.spec;
    const n = s.series[0]?.mean.length ?? 0;
    let ymax = 0;
    for (const se of s.series) for (let i = 0; i < n; i++) ymax = Math.max(ymax, se.band ? se.band[1][i] : se.mean[i]);
    if (s.capacity) ymax = Math.max(ymax, s.capacity[0]);
    const step = niceStep(ymax / 4);
    ymax = Math.max(step, Math.ceil(ymax / step) * step);
    const x = (i: number) => ML + ((W - ML - MR) * (i + 0.5)) / n;
    const y = (v: number) => MT + (H - MT - MB) * (1 - v / ymax);
    const ticks: number[] = [];
    for (let v = 0; v <= ymax + 1e-9; v += step) ticks.push(v);
    const years = Math.floor(n / 52.14);
    const yearTicks = Array.from({ length: years + 1 }, (_, yr) => ({ yr, i: Math.min(Math.round(yr * 52.14), n - 1) }));
    return { n, x, y, ticks, yearTicks };
  };
  const line = (vals: number[]) => {
    const { x, y } = geo();
    return vals.map((v, i) => `${i === 0 ? "M" : "L"}${x(i).toFixed(1)},${y(v).toFixed(1)}`).join("");
  };
  const band = ([lo, hi]: [number[], number[]]) => {
    const { x, y, n } = geo();
    let d = `M${x(0).toFixed(1)},${y(hi[0]).toFixed(1)}`;
    for (let i = 1; i < n; i++) d += `L${x(i).toFixed(1)},${y(hi[i]).toFixed(1)}`;
    for (let i = n - 1; i >= 0; i--) d += `L${x(i).toFixed(1)},${y(lo[i]).toFixed(1)}`;
    return `${d}Z`;
  };
  const tip = (i: number) => {
    let t = `Week ${i + 1}`;
    for (const s of props.spec.series) {
      t += `\n${s.name}: ${fmtNum(s.mean[i], props.spec.dp)}`;
      if (s.band) t += ` (${fmtNum(s.band[0][i], props.spec.dp)}–${fmtNum(s.band[1][i], props.spec.dp)})`;
    }
    return t;
  };
  return (
    <Show when={geo().n > 0}>
      <svg viewBox={`0 0 ${W} ${H}`} role="img">
        <rect x={ML} y={MT} width={(geo().x(props.spec.warmWeeks) - geo().x(0)).toFixed(1)} height={H - MT - MB} class="warm" />
        <text x={ML + 4} y={MT + 12} class="ax">
          start-up
        </text>
        <For each={geo().ticks}>
          {(v) => (
            <>
              <line x1={ML} x2={W - MR} y1={geo().y(v).toFixed(1)} y2={geo().y(v).toFixed(1)} class="grid" />
              <text x={ML - 6} y={(geo().y(v) + 4).toFixed(1)} text-anchor="end" class="ax">
                {fmtNum(v, 0)}
              </text>
            </>
          )}
        </For>
        <For each={geo().yearTicks}>
          {(t) => (
            <text x={geo().x(t.i).toFixed(1)} y={H - 8} text-anchor="middle" class="ax">
              {t.yr === 0 ? "week 0" : `yr ${t.yr}`}
            </text>
          )}
        </For>
        <Show when={props.spec.capacity}>
          {(c) => (
            <>
              <line x1={ML} x2={W - MR} y1={geo().y(c()[0]).toFixed(1)} y2={geo().y(c()[0]).toFixed(1)} class="cap" />
              <text x={W - MR} y={(geo().y(c()[0]) - 4).toFixed(1)} text-anchor="end" class="ax">
                {c()[1]}
              </text>
            </>
          )}
        </Show>
        <For each={props.spec.series}>
          {(s) =>
            props.spec.kind === "bar" ? (
              <For each={s.mean}>
                {(v, i) => {
                  const bw = () => Math.max((W - ML - MR) / geo().n - 1, 1);
                  return (
                    <rect
                      x={(geo().x(i()) - bw() / 2).toFixed(2)}
                      y={geo().y(v).toFixed(2)}
                      width={bw().toFixed(2)}
                      height={Math.max(geo().y(0) - geo().y(v), 0).toFixed(2)}
                      class="bar"
                      rx={Math.min(bw() / 2, 2).toFixed(1)}
                    />
                  );
                }}
              </For>
            ) : (
              <>
                <Show when={s.band}>{(b) => <path d={band(b())} class={`band${s.colour}`} />}</Show>
                <path d={line(s.mean)} fill="none" stroke-width="2" stroke-linejoin="round" class={`line${s.colour}`} />
              </>
            )
          }
        </For>
        <For each={Array.from({ length: geo().n }, (_, i) => i)}>
          {(i) => {
            const cw = () => (W - ML - MR) / geo().n;
            return <rect x={(geo().x(i) - cw() / 2).toFixed(2)} y={MT} width={cw().toFixed(2)} height={H - MT - MB} class="hit" data-tip={tip(i)} />;
          }}
        </For>
      </svg>
    </Show>
  );
}
