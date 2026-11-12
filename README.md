# Warren: NZ White meat rabbit planner

Plans a small meat-rabbit herd that fits your space and the WA Code of Practice: a day-by-day stochastic
simulation in Rust, a solver that chooses the herd, cages and timings, and a Solid.js UI.

**Try it:** https://warren.hollingworth.xyz (the standalone build on GitHub Pages, deployed by CI from `main`).

It runs two ways, from the same Rust code:

- **Server:** the Rust planner is a JSON API (axum) that also serves the UI. Solves use every CPU core.
- **Standalone:** the Rust planner is compiled to WebAssembly and runs entirely in the browser, on Web
  Workers with rayon threads (`wasm-bindgen-rayon`). Any static host works; no server needed.

## Quick start

```sh
# server
(cd web && npm ci && npm run build)      # UI → web/dist
cargo run --release                      # http://127.0.0.1:3000  (PORT / BIND to change)

# standalone (needs the WASM toolchain below)
cd web && npm run build:standalone       # → web/dist-standalone, a static site
npm run preview:standalone               # http://localhost:4173
```

Development: `cargo run --release` in one terminal and `cd web && npm run dev` in another (Vite proxies
`/api` to port 3000). `npm run dev:standalone` runs the UI on the WASM build instead.

### WASM toolchain

```sh
rustup toolchain install nightly --component rust-src --target wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.130 --locked     # must match crates/wasm/Cargo.toml
```

`scripts/build-wasm.sh` (run by `npm run wasm`) rebuilds `std` with atomics and shared memory, so rayon can
use threads, and writes the bindings to `web/src/wasm-pkg/`. Set `WASM_TOOLCHAIN` to pick a toolchain (CI pins
`nightly-2026-11-01`).

**Hosting the standalone build.** Threads need `SharedArrayBuffer`, so the page must be cross-origin
isolated: serve it with `Cross-Origin-Opener-Policy: same-origin` and
`Cross-Origin-Embedder-Policy: require-corp`. On hosts that can't set headers (e.g. GitHub Pages) the
bundled `coi-serviceworker.js` adds them: the first visit reloads once. Either way it needs HTTPS (or
localhost).

## Tests and lint

```sh
cargo test --release                     # engine, solver, API and HTTP tests
cargo fmt --all --check && cargo clippy --release --all-targets -- -D warnings
cd web && npm run typecheck && npm test  # TypeScript and formatting tests
```

CI runs all of these, plus clippy on the WASM crate and the standalone build; pushes to `main` that pass
everything deploy the standalone build to GitHub Pages.

## Layout

```
crates/core/     rabbitsim: the engine and the transport-free API
  src/engine/    params, layout (cage sizes), sim (daily run), summary (runs in parallel), constraints, solve, rng, bio
  src/api/       mod (method dispatch, solution cache), meta (input table, request checking),
                 presets (text format, from a folder or built in), report (Code checks, costs, violations)
crates/server/   axum: POST /api/<method>, plus the built UI from web/dist
crates/wasm/     wasm-bindgen: call(method, body) and initThreadPool(n)
web/             Solid 2 + Vite + TypeScript
  src/api/       types, client (HTTP or WASM), wasm (worker bridge), worker (owns the WASM instance)
  src/state.ts   the request is the state; "latest wins" solving; cost lines
  src/components App (shell), Front (space, what matters, headcount cap, the herd), More (other
                 constraints as questions), Workings (Code checks, plan detail, assumptions, examples),
                 Controls, Results (detail cards), Chart, Slider
presets/         shipped presets (*.txt), read live by the server and built into the WASM
scripts/         build-wasm.sh
```

## The API

Both front ends speak the same API: `method` + JSON body → JSON reply (HTTP: `POST /api/<method>`;
WASM: `call(method, body)`). Errors are `{"status", "message"}` (HTTP status 400 or 404).

| method           | body                         | reply                                                        |
|------------------|------------------------------|--------------------------------------------------------------|
| `meta`           | `{}`                         | inputs, constraint pool, objectives, starting request        |
| `presets`        | `{}`                         | `[{id, name, description}]`                                  |
| `preset_load`    | `{id}`                       | `{request, name, description, warnings, note}`               |
| `preset_parse`   | `{text}`                     | same as `preset_load`                                        |
| `preset_export`  | `{request, name?}`           | `{text}`                                                     |
| `solve`          | `{request}`                  | the plan: params, metrics, layout, snapshot, weekly series, Code checks, violations |
| `cost`           | `{request, key}`             | `{text}`: what that constraint costs                         |
| `add_constraint` | `{request, key}`             | `{request}` with the constraint anchored on the current plan |
| `set_objective`  | `{request, objective}`       | `{request}` (switching to max floor space suggests minimums) |

A request is `{objective, params, constraints: [{key, op: "min"|"eq"|"max", value}]}`. Anything missing
takes its default and everything is clamped to the slider ranges; replies carry the checked request back.

## Model of the UI

Three screens (the URL hash keeps the back button working):

- **Your herd** (front): your space; what matters more (most meat, with a "room for every rabbit" slider
  that sets a floor-space minimum, or most room); and "Is there a most you can look after at once?" (a
  headcount cap). Beside them, the herd drawn as rabbits with meat a year, rabbits a year and cost per kg.
- **Anything else to keep in mind?** Every other optimisable input (`engine::Var`) and result
  (`engine::Out`) as a question: "Let Warren pick" or your own *exactly* / *at least* / *at most* value.
  Each answer shows what it costs in the objective (the solver re-plans without it).
- **The workings:** Code of Practice checks, the full plan, every figure, the cage layout, weekly charts,
  the assumptions (fixed inputs: prices, biology, housing, simulation), example presets, save/open, and how
  the model works.

Nothing is constrained to start with.

## Presets

Plain-text `key = value` files in `presets/` (the server reads them on every request, so dropping a file in
adds a preset; override the folder with `PRESETS_DIR`). The standalone build has them built in. The same
format is what **Export** downloads and **Import…** reads.

```text
name = Backyard starter                 # shown in the dropdown (else made from the file name)
description = One line of detail
objective = max_meat                    # or max_qol
width_m = 2                             # any space / assumption input, by its key (see Export for the full list)
constraint.qol = min 1                  # constraint.<key> = <min|eq|max> <value>   (>=, =, <= also work)
constraint.meat = min 250
```

Unknown keys, bad values and out-of-range numbers are reported ("Ignored: line 12 …") and the rest of the file
is still applied. Preset ids are limited to letters, digits, `-` and `_`; files to 64 KB.
