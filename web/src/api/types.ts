// Shapes of the planner API (see crates/core/src/api/mod.rs). Both backends — HTTP and WASM — return these.

export type Fmt =
  | "metres" | "cm" | "weeks_days" | "days" | "pct" | "pct_points" | "money" | "money_per_kg"
  | "int" | "years" | "g_per_day" | "qol" | "cap_or_none" | "plain" | "kg";

export type Kind =
  | { type: "range"; min: number; max: number; step: number }
  | { type: "bool" }
  | { type: "choice"; options: { value: number; label: string }[] };

export interface CtlMeta { key: string; label: string; kind: Kind; fmt: Fmt; help: string }
export interface GroupMeta { title: string; open: boolean; note: string; ctls: CtlMeta[] }
export type Op = "min" | "eq" | "max";
export interface VarMeta { key: string; label: string; short: string; group: string; kind: Kind; fmt: Fmt; help: string; ops: Op[] }
export interface OutMeta { key: string; label: string; short: string; fmt: Fmt; unit: string; min: number; max: number; step: number }
export interface ObjectiveMeta { key: string; label: string; blurb: string; constraints_note: string }

export type Params = Record<string, number | boolean | string>;

export interface Constraint { key: string; op: Op; value: number }
export interface WireRequest { objective: string; params: Params; constraints: Constraint[] }

export interface Meta {
  space: GroupMeta;
  assumptions: GroupMeta[];
  vars: VarMeta[];
  var_groups: string[];
  outs: OutMeta[];
  objectives: ObjectiveMeta[];
  defaults: WireRequest;
}

export interface PresetInfo { id: string; name: string; description: string }
export interface Loaded { request: WireRequest; name: string | null; description: string | null; warnings: string[]; note: string }

export interface Stat { mean: number; p10: number; p90: number }
export interface Series { mean: number[]; p10: number[]; p90: number[] }
export type CageKind = "doe" | "buck" | "grow";
export type CageRole = "empty" | "doe" | "doe_litter" | "buck" | "replacement" | "growers";
export interface Cage { kind: CageKind; tier: number; row: number; x: number; w: number; d: number; area: number }
export interface Layout {
  max_tiers: number; tiers: number; rows: number; row_depth: number; unused_depth: number;
  doe_area: number; buck_area: number; cages: Cage[]; n_doe: number; n_buck: number; n_grow: number;
  issues: string[]; feasible: boolean;
}

export interface SolveView {
  request: WireRequest;
  params: Params;
  objective_value: number;
  feasible: boolean;
  fits: boolean;
  forced_share: number;
  violations: string[];
  checks: { ok: boolean; text: string }[];
  floor_space_unlimited: boolean;
  metrics: Record<string, Stat>;
  layout: Layout;
  snapshot: { role: CageRole; count: number }[];
  series: { head: Series; growers: Series; cages: Series; meat: Series; cost: Series };
  eff_wean: number;
  eff_does: number;
  runs: number;
  warm_weeks: number;
}

/** A failed call, as both backends report it. */
export class ApiError extends Error {
  constructor(public status: number, message: string) {
    super(message);
  }
}
