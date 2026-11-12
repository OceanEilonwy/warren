import solid from "@solidjs/vite-plugin";
import { defineConfig, type Plugin } from "vite";

// The standalone build runs the Rust planner as WebAssembly with threads, which needs SharedArrayBuffer,
// which needs a cross-origin isolated page.
const isolation = {
  "Cross-Origin-Opener-Policy": "same-origin",
  "Cross-Origin-Embedder-Policy": "require-corp",
};

/** For static hosts that can't send those headers: a service worker that adds them (coi-serviceworker). */
function coiServiceWorker(): Plugin {
  return {
    name: "warren-coi-serviceworker",
    transformIndexHtml: () => [{ tag: "script", attrs: { src: "./coi-serviceworker.js" }, injectTo: "head-prepend" }],
  };
}

export default defineConfig(({ mode }) => {
  const standalone = mode === "standalone";
  return {
    plugins: [solid(), ...(standalone ? [coiServiceWorker()] : [])],
    base: standalone ? "./" : "/",
    publicDir: standalone ? "public-standalone" : false,
    build: { outDir: standalone ? "dist-standalone" : "dist", target: "es2022", emptyOutDir: true },
    worker: { format: "es" },
    optimizeDeps: { exclude: ["./src/wasm-pkg"] },
    server: {
      headers: standalone ? isolation : {},
      proxy: standalone ? undefined : { "/api": "http://127.0.0.1:3000" },
    },
    preview: { headers: standalone ? isolation : {} },
  };
});
