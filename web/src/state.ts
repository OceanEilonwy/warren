// App state and the actions that change it. The request (objective, inputs, constraints) is the state;
// every change re-plans. Solves are "latest wins": one runs at a time and only the newest waiting request
// is sent next, so dragging a slider never queues up a backlog (important for the WASM build, where a
// solve takes seconds and cannot be cancelled).
import { createSignal } from "solid-js";
import { api, backend } from "./api/client";
import { ApiError, type Constraint, type Meta, type Op, type PresetInfo, type SolveView, type WireRequest } from "./api/types";
import { fromNum } from "./format";

export const [meta, setMeta] = createSignal<Meta | null>(null);
export const [presets, setPresets] = createSignal<PresetInfo[]>([]);
const [request, setRequestSignal] = createSignal<WireRequest | null>(null);
export { request };
/** The newest request. Solid 2 batches signal writes until the next flush, so actions that write and then
 * read in the same tick use this instead of `request()`. */
let latest: WireRequest | null = null;
function setRequest(r: WireRequest) {
  latest = r;
  setRequestSignal(r);
}
export const [solution, setSolution] = createSignal<SolveView | null>(null);
/** Cost line per constraint key; `stale` while a new one is being worked out. */
export const [costs, setCosts] = createSignal<Record<string, { text: string; stale: boolean }>>({});
/** Whole-plan work in flight (solves, constraint and objective changes). */
export const [busy, setBusy] = createSignal(0);
/** How far the running solve has got (0 to 1), or null when unknown or idle. */
export const [progress, setProgress] = createSignal<number | null>(null);
/** Cost lines in flight. */
export const [costing, setCosting] = createSignal(0);
export const [error, setError] = createSignal<{ message: string; retry: () => void } | null>(null);
/** A message for the top of the sidebar ("Loaded preset …" and anything ignored). */
export const [note, setNote] = createSignal<string | null>(null);
/** Something went wrong before the app could start (e.g. the WASM build without cross-origin isolation). */
export const [fatal, setFatal] = createSignal<string | null>(null);

const message = (e: unknown) => (e instanceof ApiError || e instanceof Error ? e.message : String(e));

/** Run whole-plan work, counting it for the progress bar and reporting failures with a Retry. */
async function work<T>(f: () => Promise<T>, retry: () => void): Promise<T | undefined> {
  setBusy((n) => n + 1);
  inFlight++;
  void watchProgress();
  try {
    const out = await f();
    setError(null);
    return out;
  } catch (e) {
    setError({ message: message(e), retry });
    return undefined;
  } finally {
    inFlight--;
    setBusy((n) => n - 1);
  }
}

/** Whole-plan work in flight (a plain count: signal writes are batched). */
let inFlight = 0;
let watching = false;
const pause = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** While whole-plan work runs, ask the backend how far its solve has got a few times a second. */
async function watchProgress() {
  if (watching) return;
  watching = true;
  try {
    const b = await backend();
    await pause(150);
    while (inFlight > 0) {
      const f = await b.progress();
      if (inFlight > 0) setProgress(f);
      await pause(250);
    }
  } finally {
    setProgress(null);
    watching = false;
  }
}

// ---------------- solving ----------------

let solving = false;
let waiting: WireRequest | null = null;
let timer: ReturnType<typeof setTimeout> | undefined;
/** Bumped whenever a new solve starts, so cost lines for an older request are dropped. */
let generation = 0;

function enqueue(r: WireRequest) {
  waiting = r;
  void pump();
}

async function pump() {
  if (solving || !waiting) return;
  const r = waiting;
  waiting = null;
  solving = true;
  generation++;
  const s = await work(() => api.solve(r), () => enqueue(r));
  solving = false;
  if (s && !waiting) {
    setSolution(s);
    void refreshCosts(s.request, generation);
  }
  void pump();
}

/** Re-plan now, or after `delay` ms of quiet (slider drags). */
export function replan(delay = 0) {
  clearTimeout(timer);
  const go = () => {
    if (latest) enqueue(latest);
  };
  if (delay > 0) timer = setTimeout(go, delay);
  else go();
}

async function refreshCosts(r: WireRequest, gen: number) {
  const keys = r.constraints.map((c) => c.key);
  setCosts((old) => Object.fromEntries(keys.map((k) => [k, { text: old[k]?.text ?? "working out what this costs…", stale: true }])));
  const one = async (key: string) => {
    if (gen !== generation) return;
    setCosting((n) => n + 1);
    try {
      const { text } = await api.cost(r, key);
      if (gen === generation) setCosts((c) => ({ ...c, [key]: { text, stale: false } }));
    } catch {
      /* a cost line can simply stay as it was */
    } finally {
      setCosting((n) => n - 1);
    }
  };
  // The server works on several at once; the WASM worker runs one call at a time, so ask in turn
  // (and stop as soon as the request changes).
  if ((await backend()).kind === "http") await Promise.all(keys.map(one));
  else for (const k of keys) await one(k);
}

// ---------------- screens ----------------

/** Which screen is showing; kept in the URL hash so the back button works. */
export type View = "herd" | "more" | "workings";
const viewFromHash = (): View => {
  const h = location.hash.slice(1);
  return h === "more" || h === "workings" ? h : "herd";
};
export const [view, setViewSignal] = createSignal<View>(viewFromHash());
window.addEventListener("hashchange", () => setViewSignal(viewFromHash()));
window.addEventListener("popstate", () => setViewSignal(viewFromHash()));
export function go(v: View) {
  if (v === viewFromHash()) return;
  if (v === "herd") history.pushState(null, "", location.pathname + location.search);
  else location.hash = v;
  setViewSignal(v);
  window.scrollTo(0, 0);
}

// ---------------- start-up ----------------

export async function start() {
  try {
    const [m, p] = await Promise.all([api.meta(), api.presets()]);
    setMeta(m);
    setPresets(p);
    setRequest(structuredClone(m.defaults));
    replan();
  } catch (e) {
    setFatal(message(e));
  }
}

// ---------------- edits ----------------

function update(f: (r: WireRequest) => WireRequest) {
  if (latest) setRequest(f(latest));
}

/** A fixed input (space or assumption). Sliders re-plan after a pause; other inputs at once. */
export function setParam(key: string, v: number, slider = false) {
  update((r) => ({ ...r, params: { ...r.params, [key]: fromNum(key, v) } }));
  replan(slider ? 250 : 0);
}

export function setConstraint(key: string, patch: Partial<Pick<Constraint, "op" | "value">>, slider = false) {
  update((r) => ({ ...r, constraints: r.constraints.map((c) => (c.key === key ? { ...c, ...patch } : c)) }));
  replan(slider ? 250 : 0);
}

export function setConstraintOp(key: string, op: Op) {
  setConstraint(key, { op });
}

/** Set a constraint directly (adding it if missing), without asking the API to anchor it. */
export function setLimit(key: string, op: Op, value: number, slider = false) {
  update((r) => {
    const has = r.constraints.some((c) => c.key === key);
    const constraints = has ? r.constraints.map((c) => (c.key === key ? { ...c, op, value } : c)) : [...r.constraints, { key, op, value }];
    return { ...r, constraints };
  });
  replan(slider ? 250 : 0);
}

export function removeConstraint(key: string) {
  update((r) => ({ ...r, constraints: r.constraints.filter((c) => c.key !== key) }));
  replan();
}

export async function addConstraint(key: string) {
  const r = latest;
  if (!r) return;
  const out = await work(() => api.addConstraint(r, key), () => void addConstraint(key));
  if (out) {
    setRequest(out.request);
    replan();
  }
}

export async function setObjective(objective: string) {
  const r = latest;
  if (!r) return;
  const out = await work(() => api.setObjective(r, objective), () => void setObjective(objective));
  if (out) {
    setRequest(out.request);
    replan();
  }
}

export function restoreDefaults() {
  const m = meta();
  if (!m) return;
  setNote(null);
  setRequest(structuredClone(m.defaults));
  replan();
}

// ---------------- presets ----------------

export async function loadPreset(id: string) {
  if (!id) return;
  const out = await work(() => api.presetLoad(id), () => void loadPreset(id));
  if (out) {
    setNote(out.note);
    setRequest(out.request);
    replan();
  }
}

export async function importPreset(text: string) {
  try {
    const out = await api.presetParse(text);
    setNote(out.note);
    setRequest(out.request);
    replan();
  } catch (e) {
    setNote(message(e));
  }
}

/** Download the current setup as a preset text file. */
export async function exportPreset() {
  const r = latest;
  if (!r) return;
  const out = await work(() => api.presetExport(r), () => void exportPreset());
  if (!out) return;
  const url = URL.createObjectURL(new Blob([out.text], { type: "text/plain;charset=utf-8" }));
  const a = document.createElement("a");
  a.href = url;
  a.download = "rabbit-plan.txt";
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
