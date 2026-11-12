// Formatting must match the Rust side (crates/core/src/api/meta.rs `Fmt::render`, report.rs `fmt_num`).
import assert from "node:assert/strict";
import { test } from "node:test";
import { decimals, fmtNum, fromNum, num, outText, render, scale, valueText } from "./format.ts";
import { niceStep } from "./components/chart-math.ts";

test("numbers group thousands and never show -0", () => {
  assert.equal(fmtNum(1234567.891, 1), "1,234,567.9");
  assert.equal(fmtNum(12, 0), "12");
  assert.equal(fmtNum(-1234.6, 0), "-1,235");
  assert.equal(fmtNum(-0.0001, 1), "0.0");
});

test("values render like the Rust formatter", () => {
  assert.equal(render("weeks_days", 77), "11 wk (77 d)");
  assert.equal(render("weeks_days", 80), "11.4 wk (80 d)");
  assert.equal(render("pct", 0.15), "15%");
  assert.equal(render("qol", 1), "minimum");
  assert.equal(render("qol", 1.5), "+50%");
  assert.equal(render("cap_or_none", 0), "no cap");
  assert.equal(render("money_per_kg", 1.5), "$1.50/kg");
  assert.equal(render("sq_metres", 0.12), "0.12 m²");
  assert.equal(render("plain", 8), "8");
  assert.equal(render("plain", 8.5), "8.5");
  assert.equal(valueText({ type: "bool" }, "plain", 1), "yes");
  assert.equal(valueText({ type: "choice", options: [{ value: 0, label: "At set age" }, { value: 1, label: "When room is needed" }] }, "plain", 1), "When room is needed");
  assert.equal(outText({ key: "meat", label: "", short: "", fmt: "int", unit: "kg/yr", min: 0, max: 1, step: 1 }, 250), "250 kg/yr");
});

test("percentages are typed as percentages, with the right decimals", () => {
  assert.equal(scale("pct"), 100);
  assert.equal(scale("metres"), 1);
  assert.equal(decimals(0.05), 2);
  assert.equal(decimals(1), 0);
  assert.equal(decimals(0.01 * 100), 0);
});

test("booleans and enums round-trip through the 0/1 UI values", () => {
  assert.equal(num(true), 1);
  assert.equal(num("room"), 1);
  assert.equal(num("own"), 0);
  assert.equal(fromNum("access_both_sides", 0), false);
  assert.equal(fromNum("replacement", 1), "buy");
  assert.equal(fromNum("butcher_rule", 0), "age");
  assert.equal(fromNum("width_m", 2.5), 2.5);
});

test("chart axis steps are round numbers", () => {
  assert.equal(niceStep(0), 1);
  assert.equal(niceStep(3), 5);
  assert.equal(niceStep(70), 100);
  assert.equal(niceStep(0.3), 0.5);
});
