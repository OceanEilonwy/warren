// Page shell: progress bar and status, header with the Code of Practice badge and Help, sidebar, results.
import { createEffect, For, Show } from "solid-js";
import logo from "../logo.svg";
import { busy, costing, error, fatal, meta, solution } from "../state";
import { hideTip } from "../tooltip";
import { Results } from "./Results";
import { Sidebar } from "./Sidebar";

const CODE_URL =
  "https://www.dpird.wa.gov.au/siteassets/documents/agriculture/livestock/animal-welfare-code-of-practice/rabbits-code-of-practice-for-keeping-rabbits-in-wa.pdf";

function Status() {
  // the stylesheet drives the bar and the pill from these classes on <html>
  createEffect(
    () => [busy() > 0, costing() > 0] as const,
    ([calc, cost]) => {
      document.documentElement.classList.toggle("calculating", calc);
      document.documentElement.classList.toggle("costing", cost);
    },
  );
  return (
    <>
      <div id="topbar" aria-hidden="true" />
      <div id="status" role="status" aria-live="polite">
        <span class="spinner" aria-hidden="true" />
        <span class="when-calc">Re-planning your herd…</span>
        <span class="when-cost">Working out what each constraint costs…</span>
      </div>
      <div id="status-error" role="alert" hidden={!error()}>
        <span>{error()?.message ?? ""} What's on screen may be out of date.</span>
        <button type="button" onClick={() => error()?.retry()}>
          Retry
        </button>
      </div>
    </>
  );
}

function CodeDialog(props: { ref: (d: HTMLDialogElement) => void }) {
  return (
    <dialog id="code-dialog" ref={props.ref}>
      <div class="dlg-head">
        <h2>Code of Practice checks</h2>
        <form method="dialog">
          <button>Close</button>
        </form>
      </div>
      <ul class="checks">
        <For each={solution()?.checks ?? []}>
          {(c) => (
            <li class={c.ok ? "ok" : "bad"}>
              <span aria-hidden="true">{c.ok ? "✓" : "!"}</span> <span class="sr">{c.ok ? "OK:" : "Problem:"}</span>
              {c.text}
            </li>
          )}
        </For>
      </ul>
      <p class="small">
        <strong>Not modelled, still required by the Code:</strong> mesh openings ≤ 19 × 19 mm (13 × 13 mm for kittens), wire ≥ 2.5 mm; a solid resting
        board ≥ 0.1 m² covering at most ⅓ of the floor; nest box in at least 2 days before kindling; daily inspection (leave litters under a week old
        undisturbed); 10–25 °C optimum; does taken to the buck for mating; humane slaughter by a competent person.
      </p>
    </dialog>
  );
}

function HelpDialog(props: { ref: (d: HTMLDialogElement) => void }) {
  return (
    <dialog id="help-dialog" class="wide" ref={props.ref}>
      <div class="dlg-head">
        <h2>Help</h2>
        <form method="dialog">
          <button>Close</button>
        </form>
      </div>
      <div class="help-body">
        <h3>What this does</h3>
        <p>
          It simulates a small breeding herd of NZ White meat rabbits day by day, hundreds of times with randomised litters, losses and conceptions, in
          the space you describe and under the <a href={CODE_URL}>WA Code of Practice for keeping rabbits</a> (Section 3 floor areas). Then it works out
          the best plan: how many does and bucks, how many tiers and what cage sizes, and when to wean, re-mate and butcher.
        </p>
        <h3>Using it</h3>
        <ol>
          <li>
            <strong>Choose an objective.</strong> Most meat, or the most floor space for the rabbits. Maximising floor space starts with a minimum for
            meat and for rabbits on hand (75% of the best plan) so the answer means something.
          </li>
          <li>
            <strong>Describe your space.</strong> Width, depth, height, and whether you can reach in from both long sides.
          </li>
          <li>
            <strong>Add constraints.</strong> Use “+ Constraint” to limit anything the solver would otherwise choose: floor space per cage type, the
            number of rabbits at once, tiers, does, weaning age, and so on. Each can be <em>exactly</em>, <em>at least</em> or <em>at most</em> a value,
            and you can also limit results: dressed meat, rabbits butchered, or rabbits on hand (peak). “Max rabbits” is different: it is a ceiling on
            the herd, so it only has “at most”. To ask for a bigger herd, use the result “Rabbits on hand (peak)” with “at least”. Anything you don't
            constrain is chosen for you.
          </li>
          <li>
            <strong>Read what each constraint costs.</strong> Under every constraint a line says what it costs you in the objective (for example “Costs
            35 kg/yr of meat”), or that it isn't limiting. It works that out by re-planning without it.
          </li>
          <li>
            <strong>Check the Code badge.</strong> The tick or cross at the top right opens the Code of Practice checks.
          </li>
        </ol>
        <h3>Floor space (quality of life)</h3>
        <p>
          Floor space is a multiplier on the Code minimum: “minimum” is the Code, “+50%” is one and a half times it. There are four cage types: the doe
          with her litter, the buck, growers 5–12 weeks and growers over 12 weeks. “All cages” sets all four; add a per-type constraint to override
          one. More room means fewer rabbits fit, so it costs meat.
        </p>
        <h3>Reading the results</h3>
        <ul>
          <li>Figures are averages over many randomised runs, after a start-up period. Hover a figure for the range that 80% of runs fell in.</li>
          <li>Pins (🔒) and limits (↕) in the plan show which values you set; the rest were chosen for you.</li>
          <li>
            “Butchered early (no room)” counts rabbits that had to go before the planned age because the cages were full. The solver only accepts plans
            where this stays at or under 3%, and warns if it can't.
          </li>
        </ul>
        <h3>How the model works</h3>
        <ul class="assume">
          <li>
            Day-by-day simulation across every CPU core. Does and bucks are bought in at the start and raised to breeding age; replacements are kept
            from your own litters (or bought in) and reared in time to take over when a breeder retires.
          </li>
          <li>
            Cages are different sizes: each doe has a cage sized for her and her litter to weaning, each buck a breeder-sized cage, and the rest of each
            row is cut into grower cages. Litters stay with the doe until weaning, then move into grower cages, packed to the Code's per-rabbit areas
            and split by sex once older than the split age.
          </li>
          <li>
            Two butchering rules: <em>at set age</em>, or <em>when room is needed</em> (oldest growers go whenever the cages fill or the headcount cap
            is hit, and all by the butchering age).
          </li>
          <li>
            A litter is weaned early if the doe is due to kindle again (nest box goes in 2 days before). Weaning at 5 weeks or earlier lets doe cages
            be the smaller 0.56 m² size; later weaning needs 0.74 m². The fastest re-mating allowed is 11 days after kindling, and does mated while
            still nursing conceive less often.
          </li>
          <li>With a headcount cap, a doe is only mated if there will be room for her litter; if the cap is still exceeded, the oldest growers are butchered early.</li>
          <li>
            Growth follows a typical NZ White curve scaled to the average dressed weight you set; dressing out rises from 50% at 8 weeks to 58% by 16
            weeks. Feed scales with metabolic weight (kg^0.75), plus extra for nursing does. These are assumptions, not Code requirements; change them
            under Assumptions.
          </li>
          <li>Not modelled: heat or season effects, inbreeding, falling litter size from intensive breeding, disease outbreaks.</li>
        </ul>
      </div>
    </dialog>
  );
}

function CodeBadge(props: { open: () => void }) {
  const bad = () => solution()?.checks.filter((c) => !c.ok).length ?? 0;
  return (
    <button
      id="code-badge"
      class={{ badge: true, ok: !!solution() && bad() === 0, bad: bad() > 0 }}
      type="button"
      data-tip="Code of Practice checks: click for details"
      onClick={props.open}
    >
      <Show when={solution()} fallback="Code of Practice">
        <span aria-hidden="true">{bad() === 0 ? "✓" : "✗"}</span> {bad() === 0 ? "Code of Practice" : `${bad()} Code issue${bad() === 1 ? "" : "s"}`}
      </Show>
    </button>
  );
}

export function App() {
  let codeDlg: HTMLDialogElement | undefined;
  let helpDlg: HTMLDialogElement | undefined;
  return (
    <>
      <Status />
      <header class="top">
        <div class="brand">
          <img src={logo} alt="" width="44" height="44" />
          <div>
            <h1>Warren</h1>
            <p class="tag">NZ White meat-rabbit planner: a herd that fits your space and the WA Code of Practice.</p>
          </div>
        </div>
        <div class="top-actions">
          <CodeBadge open={() => codeDlg?.showModal()} />
          <button class="help-btn" type="button" onClick={() => helpDlg?.showModal()}>
            Help
          </button>
        </div>
      </header>
      <Show
        when={meta()}
        fallback={
          <div class="wrap">
            <main id="results">
              <section class="card pending">
                <Show when={fatal()} fallback={<><div class="big-spinner" aria-hidden="true" /><p>Starting the planner…</p></>}>
                  <p>
                    <strong>The planner couldn't start.</strong>
                  </p>
                  <p class="small">{fatal()}</p>
                </Show>
              </section>
            </main>
          </div>
        }
      >
        <div id="app" class="wrap">
          <Sidebar />
          <main id="results" class={{ busy: busy() > 0 }} onScroll={hideTip}>
            <Show
              when={solution()}
              fallback={
                <section class="card pending">
                  <div class="big-spinner" aria-hidden="true" />
                  <p>Planning your herd…</p>
                  <p class="small">Running hundreds of randomised simulations and searching for the best plan.</p>
                </section>
              }
            >
              {(s) => <Results s={s()} />}
            </Show>
          </main>
        </div>
      </Show>
      <CodeDialog ref={(d) => (codeDlg = d)} />
      <HelpDialog ref={(d) => (helpDlg = d)} />
    </>
  );
}
