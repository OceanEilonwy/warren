// "The workings": the Code checks, the plan in full, your assumptions, examples to start from, and how it works.
import { For, Show } from "solid-js";
import { exportPreset, importPreset, loadPreset, meta, note, presets, restoreDefaults, solution } from "../state";
import { ControlRow } from "./Controls";
import { Charts, LayoutCard, PlanCard, StatsCard } from "./Results";

const CODE_URL =
  "https://www.dpird.wa.gov.au/siteassets/documents/agriculture/livestock/animal-welfare-code-of-practice/rabbits-code-of-practice-for-keeping-rabbits-in-wa.pdf";

function Checks() {
  return (
    <section class="card" id="checks">
      <h2>Is it legal, and kind?</h2>
      <p class="muted">Checked against the WA Code of Practice for keeping rabbits, every simulated day.</p>
      <ul class="checks">
        <For each={solution()?.checks ?? []}>
          {(c) => (
            <li class={c.ok ? "ok" : "bad"}>
              <span class="mark" aria-hidden="true">
                {c.ok ? "✓" : "!"}
              </span>
              <span class="sr">{c.ok ? "Meets the Code:" : "Problem:"}</span>
              <span>{c.text}</span>
            </li>
          )}
        </For>
      </ul>
      <h3>Still on your list</h3>
      <p>
        Mesh no bigger than 19 × 19 mm (13 × 13 mm for kittens) and wire at least 2.5 mm; a solid resting board of at least 0.1 m², covering no more
        than a third of the floor; the nest box in at least 2 days before kindling; a look every day (leave litters under a week old undisturbed);
        10–25 °C; humane slaughter by a competent person. <a href={CODE_URL}>Read the Code (PDF)</a>
      </p>
    </section>
  );
}

function Examples() {
  let file!: HTMLInputElement;
  return (
    <section class="card">
      <h2>Start from an example</h2>
      <div class="examples">
        <For each={presets()}>
          {(p) => (
            <button type="button" class="example" onClick={() => void loadPreset(p.id)}>
              <strong>{p.name}</strong>
              <span class="muted">{p.description}</span>
            </button>
          )}
        </For>
      </div>
      <Show when={note()}>
        {(n) => (
          <p class="note" role="status">
            <For each={n().split("\n")}>{(line, i) => <>{i() > 0 && <br />}{line}</>}</For>
          </p>
        )}
      </Show>
      <div class="row-btns">
        <button type="button" class="soft" onClick={() => file.click()}>
          Open a saved plan
        </button>
        <button type="button" class="primary" onClick={() => void exportPreset()}>
          Save this plan
        </button>
        <button type="button" class="soft" onClick={restoreDefaults}>
          Start again
        </button>
      </div>
      <input
        ref={file}
        type="file"
        accept=".txt,text/plain"
        hidden
        onChange={async (e) => {
          const f = e.currentTarget.files?.[0];
          e.currentTarget.value = "";
          if (f) await importPreset(await f.text());
        }}
      />
      <p class="hint">Plans save as a small text file you can keep, share or edit.</p>
    </section>
  );
}

function Assumptions() {
  const m = meta()!;
  return (
    <section class="card">
      <h2>About your rabbits</h2>
      <p class="muted">Typical NZ White and WA values to start with. Change any that don't match yours.</p>
      <For each={m.assumptions}>
        {(g) => (
          <details class="group-box" open={g.open}>
            <summary>{g.title}</summary>
            <Show when={g.note}>
              <p class="hint">{g.note}</p>
            </Show>
            <div class="fields">
              <For each={g.ctls}>{(c) => <ControlRow c={c} />}</For>
            </div>
          </details>
        )}
      </For>
    </section>
  );
}

function HowItWorks() {
  return (
    <section class="card how">
      <h2>How Warren works</h2>
      <p>
        It simulates a small breeding herd of NZ White meat rabbits day by day, hundreds of times over with randomised litters, losses and
        conceptions, in the space you describe and under the <a href={CODE_URL}>WA Code of Practice</a> floor areas. Then it searches for the best
        plan: how many does and bucks, how many tiers and what cage sizes, and when to wean, re-mate and butcher.
      </p>
      <ul>
        <li>Figures are averages over many runs after a start-up year. Hover a figure to see the range 80% of runs fell in.</li>
        <li>
          Each doe has a cage big enough for her and her litter to weaning, plus room for the nest box; each buck has a breeder-sized cage; the rest
          of each row becomes grower cages, packed to the Code's per-rabbit areas.
        </li>
        <li>
          Youngsters go either at a set age or whenever the cages fill (oldest first). Warren only accepts plans where no more than 3% have to go
          early for lack of room.
        </li>
        <li>
          Weaning at 5 weeks or earlier lets doe cages be the smaller 0.56 m² size; later weaning needs 0.74 m². Does mated while still nursing
          conceive less often.
        </li>
        <li>
          Growth follows a typical NZ White curve scaled to the dressed weight you set. Feed scales with body weight, plus extra for nursing does.
        </li>
        <li>Not modelled: heat or seasons, inbreeding, smaller litters from intensive breeding, disease.</li>
      </ul>
    </section>
  );
}

export function Workings() {
  return (
    <div class="workings">
      <h1>The workings</h1>
      <div class="w-top">
        <Checks />
        <Examples />
      </div>
      <Show when={solution()}>
        {(s) => (
          <>
            <h2 class="group">Your plan in full</h2>
            <PlanCard s={s()} />
            <StatsCard s={s()} />
            <LayoutCard s={s()} />
            <Charts s={s()} />
          </>
        )}
      </Show>
      <Assumptions />
      <HowItWorks />
    </div>
  );
}
