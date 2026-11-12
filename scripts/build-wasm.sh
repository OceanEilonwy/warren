#!/usr/bin/env bash
# Build the planner's API as WebAssembly with rayon threads (wasm-bindgen-rayon), for the standalone
# browser build. Needs: a nightly toolchain with `rust-src` and the wasm32-unknown-unknown target, and
# `wasm-bindgen` (the CLI) at the same version as the `wasm-bindgen` crate in crates/wasm/Cargo.toml.
#
#   rustup toolchain install nightly --component rust-src --target wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version 0.2.130 --locked
set -euo pipefail
cd "$(dirname "$0")/.."
TOOLCHAIN="${WASM_TOOLCHAIN:-nightly}"
OUT="${WASM_OUT:-web/src/wasm-pkg}"
# Threads need shared memory: atomics + bulk memory, std rebuilt with them, the memory imported and shared
# (so every worker gets the same one), and the thread-local-storage hooks wasm-bindgen sets up per thread.
# 4 GiB is the wasm32 limit.
export RUSTFLAGS="-C target-feature=+atomics,+bulk-memory \
  -C link-arg=--shared-memory -C link-arg=--import-memory -C link-arg=--max-memory=4294967296 \
  -C link-arg=--export=__wasm_init_tls -C link-arg=--export=__tls_size -C link-arg=--export=__tls_align -C link-arg=--export=__tls_base"
cargo "+$TOOLCHAIN" build -p rabbitsim-wasm --target wasm32-unknown-unknown --release -Z build-std=panic_abort,std
rm -rf "$OUT"
wasm-bindgen target/wasm32-unknown-unknown/release/rabbitsim_wasm.wasm --out-dir "$OUT" --target web
# wasm-bindgen-rayon's worker helper imports the package directory ('../../..'), so give it an entry point
# (wasm-pack would write this file).
cat > "$OUT/package.json" <<'JSON'
{ "name": "rabbitsim-wasm", "type": "module", "main": "rabbitsim_wasm.js", "types": "rabbitsim_wasm.d.ts", "sideEffects": ["./snippets/*"] }
JSON
ls -l "$OUT"
