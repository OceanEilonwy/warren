// Types for the WASM bindings (crates/wasm), used when `src/wasm-pkg/` hasn't been generated, so the
// HTTP-only build and its typecheck don't need the nightly WASM toolchain. Keep in step with
// crates/wasm/src/lib.rs; the generated `rabbitsim_wasm.d.ts` takes precedence when it exists.

/** Run one API method. Errors come back as `{"status": n, "message": "..."}` in the exception. */
export function call(method: string, body: string): string;
export function initThreadPool(num_threads: number): Promise<unknown>;
export default function init(module_or_path?: unknown): Promise<unknown>;
