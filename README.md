# NZ White meat rabbit planner (Rust)

axum (on hyper) + maud + htmx rewrite of the TypeScript prototype in `../src`.

```
cargo run --release        # http://127.0.0.1:3000  (PORT=8080 to change)
cargo test --release       # unit + engine + solver + HTTP tests
```

## Model of the UI

- **Objective:** maximise meat, or maximise floor space (QoL) while meeting result limits (it starts with a meat minimum).
- **Fixed inputs** (not optimised, sensible defaults pre-loaded): the space (always shown) and an
  "Assumptions" section (feed and stock prices, biology, housing details, simulation settings; costs are reported, not optimised).
- **Optimisable inputs** (`engine::Var`): floor space per cage type (+ an "all cages" shorthand), herd cap,
  tiers, grower cage width, does, bucks, weaning age, re-mating interval, butchering rule and age.
  Unconstrained ones are chosen by the solver.
- **Constraints:** "+ Constraint" adds any optimisable input or result (`engine::Out`) as *exactly*,
  *at least* or *at most*. Starting constraints: one QoL and the herd cap. Each constraint shows what it
  costs in the objective (the solver re-plans without it).

## Layout

- `src/engine/` — pure simulation: `params`, `layout` (mixed cage sizes), `sim` (daily stochastic run),
  `summary` (many runs in parallel with rayon), `constraints` (`Var`, `Out`, `Bound`, `Objective`, `Problem`),
  `solve` (parallel coordinate search; bisection on QoL), `rng`, `bio`.
- `src/web/` — `controls` (fixed-input table, constraint-pool metadata, form parsing), `views` (maud),
  `charts` (server-side SVG), `mod` (routes + solution cache).
- Stateless server: the form is the state (`objective`, one field per fixed input, `on_/op_/val_<key>` per
  constraint, `cons=1` marking that the constraint list is present).
- Routes: `/` page · `/results` (htmx: re-solve) · `/app?action=add|remove|defaults` (whole-app swaps) ·
  `/cost?drop=<key>` (cost chip) · static `/htmx.min.js`, `/style.css`.

## Presets

Plain-text `key = value` files in `server/presets/` (override the folder with `PRESETS_DIR`). They are read on
every request, so dropping a `.txt` file in adds it to the "Load a preset…" dropdown. The same format is what
**Export** downloads and **Import…** uploads.

```text
name = Backyard starter                 # shown in the dropdown (else made from the file name)
description = One line of detail
objective = max_meat                    # or max_qol
width_m = 2                             # any space / assumption input, by its key (see Export for the full list)
constraint.qol = min 1                  # constraint.<key> = <min|eq|max> <value>   (>=, =, <= also work)
constraint.meat = min 250
```

Unknown keys, bad values and out-of-range numbers are reported in the sidebar ("Ignored: line 12 …") and the
rest of the file is still applied. File names (the preset id) are limited to letters, digits, `-` and `_`;
uploads are limited to 64 KB.
