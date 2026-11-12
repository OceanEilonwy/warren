// Tooltips for any element with `data-tip`: shown after a short pause (at once over charts), follow the
// mouse, kept on screen, shown on keyboard focus, hidden on Escape / scroll.
let tipEl: HTMLDivElement | null = null;
let timer = 0;
let target: HTMLElement | null = null;
let mx = 0;
let my = 0;

function node(): HTMLDivElement {
  if (!tipEl) {
    tipEl = document.createElement("div");
    tipEl.id = "tooltip";
    tipEl.setAttribute("role", "tooltip");
    document.body.appendChild(tipEl);
  }
  return tipEl;
}

function place(x: number, y: number) {
  const el = node();
  const w = el.offsetWidth;
  const h = el.offsetHeight;
  const left = Math.min(Math.max(8, x + 14), window.innerWidth - w - 8);
  let top = y + 18;
  if (top + h > window.innerHeight - 8) top = Math.max(8, y - h - 12);
  el.style.left = `${left}px`;
  el.style.top = `${top}px`;
}

function show(t: HTMLElement, x: number, y: number) {
  const txt = t.dataset.tip;
  if (!txt) return;
  const el = node();
  el.textContent = txt;
  el.style.display = "block";
  place(x, y);
  target = t;
  t.setAttribute("aria-describedby", "tooltip");
}

export function hideTip() {
  clearTimeout(timer);
  timer = 0;
  if (tipEl) tipEl.style.display = "none";
  if (target) {
    target.removeAttribute("aria-describedby");
    target = null;
  }
}

const tipOf = (e: Event) => (e.target as Element | null)?.closest?.<HTMLElement>("[data-tip]") ?? null;

export function installTooltips() {
  document.addEventListener("mouseover", (e) => {
    const t = tipOf(e);
    if (t === target) return;
    hideTip();
    if (!t?.dataset.tip) return;
    mx = e.clientX;
    my = e.clientY;
    timer = window.setTimeout(() => show(t, mx, my), t.closest(".chart") ? 0 : 300);
  });
  document.addEventListener("mousemove", (e) => {
    mx = e.clientX;
    my = e.clientY;
    if (target) place(mx, my);
  });
  document.addEventListener("mouseout", (e) => {
    const t = tipOf(e);
    if (!t) return;
    if (e.relatedTarget instanceof Node && t.contains(e.relatedTarget)) return;
    hideTip();
  });
  document.addEventListener("focusin", (e) => {
    const t = tipOf(e);
    if (t?.dataset.tip) {
      const r = t.getBoundingClientRect();
      show(t, r.left + r.width / 2, r.bottom - 14);
    }
  });
  document.addEventListener("focusout", hideTip);
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") hideTip();
  });
  window.addEventListener("scroll", hideTip, true);
}
