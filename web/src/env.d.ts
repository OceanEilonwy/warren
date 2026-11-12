/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** `wasm` for the standalone build (planner runs in the browser); otherwise the HTTP API is used. */
  readonly VITE_BACKEND?: "wasm" | "http";
}
