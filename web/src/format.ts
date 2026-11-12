// Number formatting, matching the Rust side (crates/core/src/api/meta.rs `Fmt`, report.rs `fmt_num`).
import type { Fmt, Kind, OutMeta } from "./api/types";

export function trim1(v: number): string {
  return v.toFixed(1).replace(/\.0$/, "");
}

/** Thousands separators and a fixed number of decimals; never "-0". */
export function fmtNum(v: number, dp: number): string {
  const s = Math.abs(v).toFixed(dp);
  const [int, frac] = s.split(".");
  const grouped = int.replace(/\B(?=(\d{3})+(?!\d))/g, ",");
  const out = frac !== undefined ? `${grouped}.${frac}` : grouped;
  return v < 0 && /[1-9]/.test(s) ? `-${out}` : out;
}

const AFFIXES: Partial<Record<Fmt, [string, string]>> = {
  metres: ["", "m"],
  cm: ["", "cm"],
  weeks_days: ["", "d"],
  days: ["", "d"],
  pct: ["", "%"],
  pct_points: ["", "%"],
  money: ["$", ""],
  money_per_kg: ["$", "/kg"],
  years: ["", "yr"],
  g_per_day: ["", "g/day"],
  qol: ["×", ""],
  cap_or_none: ["", "rabbits"],
  kg: ["", "kg"],
};

/** Text before and after the number in an entry box. */
export function affixes(f: Fmt): [string, string] {
  return AFFIXES[f] ?? ["", ""];
}

/** Multiplier from the stored value to the number typed in the box (fractions are typed as percentages). */
export function scale(f: Fmt): number {
  return f === "pct" ? 100 : 1;
}

const RENDER: Record<Fmt, (v: number) => string> = {
  metres: (v) => `${v.toFixed(2)} m`,
  cm: (v) => `${v.toFixed(0)} cm`,
  weeks_days: (v) => `${trim1(v / 7)} wk (${v.toFixed(0)} d)`,
  days: (v) => `${v.toFixed(0)} d`,
  pct: (v) => `${(v * 100).toFixed(0)}%`,
  pct_points: (v) => `${v.toFixed(0)}%`,
  money: (v) => `$${v.toFixed(0)}`,
  money_per_kg: (v) => `$${v.toFixed(2)}/kg`,
  int: (v) => v.toFixed(0),
  years: (v) => `${trim1(v)} yr`,
  g_per_day: (v) => `${v.toFixed(0)} g/day`,
  qol: (v) => (Math.abs(v - 1) < 1e-9 ? "minimum" : `+${((v - 1) * 100).toFixed(0)}%`),
  cap_or_none: (v) => (v === 0 ? "no cap" : v.toFixed(0)),
  plain: (v) => trim1(v),
  kg: (v) => `${v.toFixed(1)} kg`,
};

export function render(f: Fmt, v: number): string {
  return RENDER[f](v);
}

export function valueText(kind: Kind, f: Fmt, v: number): string {
  if (kind.type === "bool") return v > 0.5 ? "yes" : "no";
  if (kind.type === "choice") return kind.options.find((o) => Math.abs(o.value - v) < 1e-9)?.label ?? "";
  return render(f, v);
}

export function outText(u: OutMeta, v: number): string {
  const n = render(u.fmt, v);
  return u.unit ? `${n} ${u.unit}` : n;
}

/** Digits needed to show `step`. */
export function decimals(step: number): number {
  let d = 0;
  while (d < 4 && Math.abs(Math.round(step * 10 ** d) - step * 10 ** d) > 1e-6) d++;
  return d;
}

/** A Params field as a number (the boolean and the two enums are 0/1 in the UI). */
export function num(v: number | boolean | string | undefined): number {
  if (typeof v === "boolean") return v ? 1 : 0;
  if (v === "buy" || v === "room") return 1;
  if (v === "own" || v === "age") return 0;
  return typeof v === "number" ? v : Number(v ?? 0);
}

/** A UI number stored back into a Params field, keeping the field's type. */
export function fromNum(key: string, v: number): number | boolean | string {
  if (key === "access_both_sides") return v > 0.5;
  if (key === "replacement") return v > 0.5 ? "buy" : "own";
  if (key === "butcher_rule") return v > 0.5 ? "room" : "age";
  return v;
}
