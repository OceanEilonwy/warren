// One API, two transports: `POST /api/<method>` to the Rust server, or the same Rust code compiled to
// WebAssembly running in a Web Worker. Chosen at build time (`VITE_BACKEND=wasm` for the standalone build).
import { ApiError, type Loaded, type Meta, type PresetInfo, type SolveView, type WireRequest } from "./types";

export interface Backend {
  readonly kind: "http" | "wasm";
  call(method: string, body: unknown): Promise<unknown>;
  /** How far the running solve has got (0 to 1), or null if unknown. */
  progress(): Promise<number | null>;
}

function httpBackend(): Backend {
  return {
    kind: "http",
    async call(method, body) {
      let res: Response;
      try {
        res = await fetch(`${import.meta.env.BASE_URL}api/${method}`, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify(body ?? {}),
        });
      } catch {
        throw new ApiError(0, "Couldn't reach the server.");
      }
      const text = await res.text();
      if (!res.ok) {
        let message = text || res.statusText;
        try {
          message = JSON.parse(text).message ?? message;
        } catch {
          /* not JSON */
        }
        throw new ApiError(res.status, message);
      }
      return JSON.parse(text);
    },
    async progress() {
      try {
        return ((await this.call("progress", {})) as { done: number }).done;
      } catch {
        return null;
      }
    },
  };
}

let current: Promise<Backend> | null = null;

/** The backend for this build (the WASM one is only downloaded by the standalone build). */
export function backend(): Promise<Backend> {
  current ??= import.meta.env.VITE_BACKEND === "wasm" ? import("./wasm").then((m) => m.wasmBackend()) : Promise.resolve(httpBackend());
  return current;
}

async function call<T>(method: string, body: unknown = {}): Promise<T> {
  return (await (await backend()).call(method, body)) as T;
}

export const api = {
  meta: () => call<Meta>("meta"),
  presets: () => call<PresetInfo[]>("presets"),
  presetLoad: (id: string) => call<Loaded>("preset_load", { id }),
  presetParse: (text: string) => call<Loaded>("preset_parse", { text }),
  presetExport: (request: WireRequest, name?: string) => call<{ text: string }>("preset_export", { request, name }),
  solve: (request: WireRequest) => call<SolveView>("solve", { request }),
  cost: (request: WireRequest, key: string) => call<{ text: string }>("cost", { request, key }),
  addConstraint: (request: WireRequest, key: string) => call<{ request: WireRequest }>("add_constraint", { request, key }),
  setObjective: (request: WireRequest, objective: string) => call<{ request: WireRequest }>("set_objective", { request, objective }),
};
