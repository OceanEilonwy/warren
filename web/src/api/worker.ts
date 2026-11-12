/// <reference lib="webworker" />
// Owns the WebAssembly planner. Starts a rayon thread pool (one worker per core) once, then answers
// `{id, method, body}` messages with `{id, ok}` or `{id, err}`.
import init, { call, initThreadPool } from "../wasm-pkg/rabbitsim_wasm.js";

const ready = (async () => {
  await init();
  await initThreadPool(Math.max(1, navigator.hardwareConcurrency || 4));
})();

self.addEventListener("message", async (e: MessageEvent) => {
  const { id, method, body } = e.data ?? {};
  if (typeof id !== "number") return; // rayon's own worker messages
  try {
    await ready;
    self.postMessage({ id, ok: call(method, body) });
  } catch (err) {
    self.postMessage({ id, err: typeof err === "string" ? err : String((err as Error)?.message ?? err) });
  }
});
