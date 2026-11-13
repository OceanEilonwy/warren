/// <reference lib="webworker" />
// Owns the WebAssembly planner. Starts a rayon thread pool (one worker per core) once, then answers
// `{id, method, body}` messages with `{id, ok}` or `{id, err}`. Once ready it sends `{progress: {buffer,
// at}}`: the module's shared memory and where the running solve's progress counter sits in it, so the
// page can read it while this worker is busy.
import init, { call, initThreadPool, progress_ptr } from "rabbitsim-wasm";

const ready = (async () => {
  const wasm = await init();
  await initThreadPool(Math.max(1, navigator.hardwareConcurrency || 4));
  self.postMessage({ progress: { buffer: wasm.memory.buffer, at: progress_ptr() } });
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
