// Page shell: progress bar and status, the header, and whichever screen is showing.
import { createEffect, Match, Show, Switch } from "solid-js";
import logo from "../logo.svg";
import { busy, costing, error, fatal, go, meta, progress, view } from "../state";
import { Front } from "./Front";
import { Pct } from "./Pct";
import { More } from "./More";
import { Workings } from "./Workings";

function Status() {
  // the stylesheet drives the bar and the pill from these classes on <html>
  createEffect(
    () => [busy() > 0, costing() > 0] as const,
    ([calc, cost]) => {
      document.documentElement.classList.toggle("calculating", calc);
      document.documentElement.classList.toggle("costing", cost);
    },
  );
  // a known fraction turns the sliding bar into a filling one
  createEffect(progress, (f) => {
    const html = document.documentElement;
    html.classList.toggle("known", f !== null);
    html.style.setProperty("--progress", `${Math.round((f ?? 0) * 100)}%`);
  });
  return (
    <>
      <div id="topbar" aria-hidden="true" />
      <div id="status" role="status" aria-live="polite">
        <span class="spinner" aria-hidden="true" />
        <span class="when-calc">
          Re-planning your herd…<Pct />
        </span>
        <span class="when-cost">Working out what each answer costs…</span>
      </div>
      <div id="status-error" role="alert" hidden={!error()}>
        <span>{error()?.message ?? ""} What's on screen may be out of date.</span>
        <button type="button" class="soft" onClick={() => error()?.retry()}>
          Retry
        </button>
      </div>
    </>
  );
}

function Header() {
  const link = (label: string, to: "herd" | "workings") => (
    <a
      class="nav"
      href={to === "herd" ? "#" : `#${to}`}
      onClick={(e) => {
        e.preventDefault();
        go(to);
      }}
    >
      {label}
    </a>
  );
  return (
    <header class="top">
      <a
        class="brand"
        href="#"
        onClick={(e) => {
          e.preventDefault();
          go("herd");
        }}
      >
        <img src={logo} alt="" width="40" height="40" />
        <span>Warren</span>
      </a>
      <Show when={meta()}>
        <Show when={view() === "herd"} fallback={link("← Back to your herd", "herd")}>
          {link("Show the workings", "workings")}
        </Show>
      </Show>
    </header>
  );
}

export function App() {
  return (
    <>
      <Status />
      <Header />
      <main class="page">
        <Show
          when={meta()}
          fallback={
            <section class="card pending">
              <Show
                when={fatal()}
                fallback={
                  <>
                    <div class="big-spinner" aria-hidden="true" />
                    <p>Starting the planner…</p>
                  </>
                }
              >
                <p>
                  <strong>The planner couldn't start.</strong>
                </p>
                <p class="muted">{fatal()}</p>
              </Show>
            </section>
          }
        >
          <Switch>
            <Match when={view() === "more"}>
              <More />
            </Match>
            <Match when={view() === "workings"}>
              <Workings />
            </Match>
            <Match when={true}>
              <Front />
            </Match>
          </Switch>
        </Show>
      </main>
    </>
  );
}
