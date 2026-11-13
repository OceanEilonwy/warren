// The WASM backend: every call goes to one Web Worker that owns the WebAssembly instance (and, through
// wasm-bindgen-rayon, its own pool of thread workers). Calls run one at a time, in order.
import { ApiError } from "./types";
import type { Backend } from "./client";

type Reply = { id: number; ok?: string; err?: string; progress?: { buffer: SharedArrayBuffer; at: number } };

export function wasmBackend(): Backend {
  if (!globalThis.crossOriginIsolated) {
    throw new ApiError(0, "This page needs cross-origin isolation (COOP/COEP headers) to run the planner in your browser.");
  }
  const worker = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });
  const pending = new Map<number, { resolve: (v: unknown) => void; reject: (e: unknown) => void }>();
  let next = 1;
  let counter: Uint32Array | null = null;
  worker.addEventListener("message", (e: MessageEvent<Reply>) => {
    if (e.data.progress) {
      counter = new Uint32Array(e.data.progress.buffer, e.data.progress.at, 1);
      return;
    }
    const p = pending.get(e.data.id);
    if (!p) return;
    pending.delete(e.data.id);
    if (e.data.ok !== undefined) {
      p.resolve(JSON.parse(e.data.ok));
    } else {
      let status = 500;
      let message = e.data.err ?? "The planner failed.";
      try {
        const j = JSON.parse(message);
        status = j.status ?? status;
        message = j.message ?? message;
      } catch {
        /* a panic or JS error, not an API error */
      }
      p.reject(new ApiError(status, message));
    }
  });
  worker.addEventListener("error", (e) => {
    for (const p of pending.values()) p.reject(new ApiError(500, e.message || "The planner worker crashed."));
    pending.clear();
  });
  return {
    kind: "wasm",
    call(method, body) {
      const id = next++;
      return new Promise((resolve, reject) => {
        pending.set(id, { resolve, reject });
        worker.postMessage({ id, method, body: JSON.stringify(body ?? {}) });
      });
    },
    async progress() {
      return counter ? Atomics.load(counter, 0) / 10000 : null;
    },
  };
}
