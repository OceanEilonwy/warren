// The hutch on the front page: a 3D view (default) with the rabbits hopping about, or the build plan.
import { createEffect, createSignal, onSettled, Show } from "solid-js";
import type { SolveView } from "../api/types";
import { num } from "../format";
import type { Hutch as HutchApi, HutchData } from "../hutch/scene";
import { BuildPlan } from "./BuildPlan";

/** Which day the 3D view shows: a typical day, or the busiest. */
export type HutchDay = "typical" | "busiest";

function snapOf(s: SolveView, day: HutchDay) {
  const snap = day === "busiest" ? s.peak_snapshot : s.snapshot;
  return snap?.length === s.layout.cages.length ? snap : null;
}

/** Rabbits in the hutch on that day. */
function headOn(s: SolveView, day: HutchDay): number | null {
  const snap = snapOf(s, day);
  return snap ? snap.reduce((n, c) => n + c.count, 0) : null;
}

/** The 3D scene's input, from a solved plan. */
export function hutchData(s: SolveView, day: HutchDay = "typical"): HutchData {
  const p = s.params;
  const l = s.layout;
  const snap = snapOf(s, day);
  return {
    width: num(p.width_m),
    rows: l.rows,
    rowDepth: l.row_depth,
    tiers: l.tiers,
    cageH: num(p.cage_height_cm) / 100,
    overhead: num(p.tier_overhead_cm) / 100,
    nestArea: num(p.nest_box_m2),
    doeArea: s.eff_wean > 35 ? 0.74 : 0.56,
    cages: l.cages.map((c, i) => {
      const sn = snap?.[i] ?? { role: "empty" as const, count: 0, older: 0 };
      const label =
        sn.role === "doe_litter"
          ? `Doe + ${sn.count - 1} kit${sn.count === 2 ? "" : "s"}`
          : sn.role === "doe"
            ? "Doe"
            : sn.role === "buck"
              ? "Buck"
              : sn.role === "replacement"
                ? "Young breeder"
                : sn.role === "growers"
                  ? `${sn.count} grower${sn.count === 1 ? "" : "s"}`
                  : "Empty";
      return { kind: c.kind, tier: c.tier, row: c.row, x: c.x, w: c.w, area: c.area, role: sn.role, count: sn.count, older: sn.older ?? 0, label };
    }),
  };
}

function Hutch3D(props: { s: SolveView; day: HutchDay; lift: boolean; onFail: (why: string) => void; api: (h: HutchApi | null) => void }) {
  let el!: HTMLDivElement;
  let hutch: HutchApi | null = null;
  const [loading, setLoading] = createSignal(true);
  onSettled(() => {
    let dead = false;
    void (async () => {
      try {
        const { createHutch } = await import("../hutch/scene");
        if (dead) return;
        hutch = await createHutch(el, hutchData(props.s, props.day));
        if (dead) return hutch.dispose();
        hutch.setLift(props.lift);
        props.api(hutch);
      } catch (e) {
        props.onFail(e instanceof Error ? e.message : String(e));
      } finally {
        setLoading(false);
      }
    })();
    return () => {
      dead = true;
      hutch?.dispose();
      hutch = null;
      props.api(null);
    };
  });
  createEffect(
    () => [props.s, props.day] as const,
    ([s, day]) => hutch?.update(hutchData(s, day)),
  );
  createEffect(
    () => props.lift,
    (on) => hutch?.setLift(on),
  );
  return (
    <div class="hutch-3d" ref={el} role="img" aria-label="3D view of the hutch with the rabbits inside. Drag to turn it.">
      <Show when={loading()}>
        <div class="hutch-loading">
          <div class="big-spinner" aria-hidden="true" />
          Building your hutch…
        </div>
      </Show>
    </div>
  );
}

export function Hutch(props: { s: SolveView }) {
  let box!: HTMLDivElement;
  const [mode, setMode] = createSignal<"3d" | "plan">("3d");
  // full screen: the browser's own where it has one (not iPhone Safari), else the hutch fills the window
  const [full, setFull] = createSignal(false);
  const native = () => typeof document !== "undefined" && document.fullscreenEnabled && !!box?.requestFullscreen;
  const toggleFull = () => {
    if (full()) {
      if (document.fullscreenElement) void document.exitFullscreen();
      else setFull(false);
    } else if (native()) {
      box.requestFullscreen().catch(() => setFull(true));
    } else setFull(true);
  };
  onSettled(() => {
    const onChange = () => setFull(document.fullscreenElement === box);
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && full() && !document.fullscreenElement) setFull(false);
    };
    document.addEventListener("fullscreenchange", onChange);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("fullscreenchange", onChange);
      document.removeEventListener("keydown", onKey);
      if (document.fullscreenElement === box) void document.exitFullscreen();
    };
  });
  createEffect(
    () => full(),
    (on) => {
      document.documentElement.classList.toggle("hutch-full", on);
    },
  );
  const [lift, setLift] = createSignal(false);
  const [day, setDay] = createSignal<HutchDay>("typical");
  const dayLabel = (d: HutchDay) => {
    const n = headOn(props.s, d);
    return `${d === "typical" ? "Typical day" : "Busiest day"}${n === null ? "" : ` · ${n}`}`;
  };
  const [failed, setFailed] = createSignal<string | null>(null);
  let api: HutchApi | null = null;
  return (
    <div class={{ hutch: true, full: full() && !document.fullscreenElement }} ref={box}>
      <div class="hutch-bar">
        <div class="pills seg-pills" role="radiogroup" aria-label="View">
          <label class="pill">
            <input type="radio" name="hutch-view" checked={mode() === "3d"} onChange={() => setMode("3d")} disabled={!!failed()} />
            <span>3D</span>
          </label>
          <label class="pill">
            <input
              type="radio"
              name="hutch-view"
              checked={mode() === "plan"}
              onChange={() => {
                if (full()) toggleFull();
                setMode("plan");
              }}
            />
            <span>Plan</span>
          </label>
        </div>
        <Show when={mode() === "3d" && !failed()}>
          <div class="hutch-tools">
            <div class="pills seg-pills small" role="radiogroup" aria-label="Which day to show">
              <label class="pill" data-tip="The last day of a typical run: what the hutch usually looks like.">
                <input type="radio" name="hutch-day" checked={day() === "typical"} onChange={() => setDay("typical")} />
                <span>{dayLabel("typical")}</span>
              </label>
              <label class="pill" data-tip="The day the herd was biggest in a run with a typical peak: every cage the plan needs is in use.">
                <input type="radio" name="hutch-day" checked={day() === "busiest"} onChange={() => setDay("busiest")} />
                <span>{dayLabel("busiest")}</span>
              </label>
            </div>
            <label class="check">
              <input type="checkbox" checked={lift()} onChange={(e) => setLift(e.currentTarget.checked)} />
              <span>Lift the top tier to look in</span>
            </label>
            <button type="button" class="soft" onClick={() => api?.frontOn()}>
              Front on
            </button>
            <button type="button" class="soft icon-btn" onClick={toggleFull} aria-pressed={full() ? "true" : "false"} data-tip={full() ? "Back to the page (Esc)" : "Fill the screen with the 3D view"}>
              <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d={full() ? "M9 4v5H4M15 4v5h5M9 20v-5H4M15 20v-5h5" : "M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5"} />
              </svg>
              <span>{full() ? "Exit full screen" : "Full screen"}</span>
            </button>
          </div>
        </Show>
      </div>
      <Show
        when={mode() === "3d" && !failed()}
        fallback={
          <>
            <Show when={failed()}>
              <p class="hint">The 3D view couldn't start here ({failed()}), so here's the plan.</p>
            </Show>
            <BuildPlan s={props.s} />
          </>
        }
      >
        <Hutch3D
          s={props.s}
          day={day()}
          lift={lift()}
          onFail={(why) => {
            setFailed(why);
            setMode("plan");
          }}
          api={(h) => (api = h)}
        />
        <div class="hutch-legend">
          <span>
            <i class="dash" aria-hidden="true" /> Floor the WA Code requires: 0.07 m² per grower to 12 weeks, 0.18 m² after; a doe and litter share
            0.56–0.74 m²; a buck 0.56 m².
          </span>
          <span class="muted">Drag to turn · scroll or pinch to zoom</span>
        </div>
      </Show>
    </div>
  );
}
