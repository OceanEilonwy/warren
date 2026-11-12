//! Server-rendered HTML (maud). Every fragment is a pure function of its inputs.

use maud::{DOCTYPE, Markup, PreEscaped, html};

use super::charts::{ChartKind, ChartSeries, ChartSpec, chart, esc, fmt_num};
use super::controls::{
    ASSUMPTIONS, Ctl, Kind, OUT_UI, Op, Request, SPACE, VAR_UI, allowed_ops, bound_value, out_ui, short_label, short_out_label, var_ui,
};
use super::presets::PresetInfo;
use crate::engine::{Bound, CageKind, CageRole, MetricKey, Objective, Out, Solution, Subject, Var, code};

pub struct View<'a> {
    pub req: &'a Request,
    pub sol: &'a Solution,
    pub presets: &'a [PresetInfo],
    /// A message for the top of the sidebar (e.g. "Loaded preset …" and anything that was ignored).
    pub note: Option<&'a str>,
}

impl<'a> View<'a> {
    fn p(&self) -> &'a crate::engine::Params {
        &self.sol.params
    }
    fn summary(&self) -> &'a crate::engine::Summary {
        &self.sol.summary
    }
}

fn objective_label(o: Objective) -> &'static str {
    match o {
        Objective::MaxMeat => "Maximise meat",
        Objective::MaxQol => "Maximise floor space",
    }
}

fn objective_blurb(o: Objective) -> &'static str {
    match o {
        Objective::MaxMeat => "The most dressed meat your space can produce.",
        Objective::MaxQol => "The most room for the rabbits that still meets your result limits.",
    }
}

fn constraints_note(o: Objective) -> &'static str {
    match o {
        Objective::MaxMeat => "Limits on what the plan may do. Everything not listed is chosen to give you the most meat.",
        Objective::MaxQol => {
            "Limits on what the plan may do. Everything not listed is chosen to give the rabbits more room. The meat minimum is what stops floor space growing: raise it and the rabbits get less room."
        }
    }
}

// ---------------- page shell ----------------

/// Browser-side helpers. (1) Every slider has a number box that stays in step: dragging updates the box
/// instantly, typing moves the slider (and so re-plans). The slider is what the form submits. (2) Progress:
/// requests in flight are counted so the page can show a progress bar and a status pill, and cost lines
/// keep their last value (dimmed) while they are recomputed. (3) A failed request shows a banner with Retry.
const SCRIPT: &str = r#"
function trim1(v){return v.toFixed(1).replace(/\.0$/,'')}
function sync(sl,force){
  sl.style.setProperty('--fill',((sl.value-sl.min)/(sl.max-sl.min)*100)+'%');
  var nb=document.getElementById(sl.dataset.num);
  if(nb&&(force||document.activeElement!==nb))nb.value=(sl.value*(+sl.dataset.scale||1)).toFixed(+sl.dataset.dec||0);
  var cap=document.getElementById('cap-'+sl.id);if(cap)cap.textContent=' = '+trim1(sl.value/7)+' wk';
}
document.addEventListener('input',function(e){
  var t=e.target;
  if(t.type==='range')sync(t,false);
  else if(t.classList&&t.classList.contains('numbox')){
    var sl=document.getElementById(t.dataset.for),v=parseFloat(t.value);
    if(sl&&isFinite(v)){sl.value=v/(+sl.dataset.scale||1);sync(sl,false);sl.dispatchEvent(new Event('input',{bubbles:true}))}
  }
});
document.addEventListener('change',function(e){
  var t=e.target;
  if(t.classList&&t.classList.contains('numbox')){var sl=document.getElementById(t.dataset.for);if(sl)sync(sl,true)}
});
/* ---- select a number box's contents when it is entered ---- */
document.addEventListener('focusin',function(e){var t=e.target;if(t.classList&&t.classList.contains('numbox')){t.select();t._sel=true}});
document.addEventListener('mouseup',function(e){var t=e.target;if(t._sel&&t.classList&&t.classList.contains('numbox')){e.preventDefault();t._sel=false}});
/* ---- tooltips (any element with data-tip) ---- */
var tipEl=null,tipTimer=0,tipTarget=null,mx=0,my=0;
function tipNode(){if(!tipEl){tipEl=document.createElement('div');tipEl.id='tooltip';tipEl.setAttribute('role','tooltip');document.body.appendChild(tipEl)}return tipEl}
function placeTip(x,y){var el=tipNode(),w=el.offsetWidth,h=el.offsetHeight,vw=window.innerWidth,vh=window.innerHeight;
  var left=Math.min(Math.max(8,x+14),vw-w-8),top=y+18;if(top+h>vh-8)top=Math.max(8,y-h-12);el.style.left=left+'px';el.style.top=top+'px'}
function showTip(t,x,y){var txt=t.dataset.tip;if(!txt)return;var el=tipNode();el.textContent=txt;el.style.display='block';placeTip(x,y);tipTarget=t;t.setAttribute('aria-describedby','tooltip')}
function hideTip(){clearTimeout(tipTimer);tipTimer=0;if(tipEl)tipEl.style.display='none';if(tipTarget){tipTarget.removeAttribute('aria-describedby');tipTarget=null}}
document.addEventListener('mouseover',function(e){
  var t=e.target.closest&&e.target.closest('[data-tip]');if(t===tipTarget)return;hideTip();
  if(!t||!t.dataset.tip)return;mx=e.clientX;my=e.clientY;
  tipTimer=setTimeout(function(){showTip(t,mx,my)},t.closest('.chart')?0:300);
});
document.addEventListener('mousemove',function(e){mx=e.clientX;my=e.clientY;if(tipTarget)placeTip(mx,my)});
document.addEventListener('mouseout',function(e){var t=e.target.closest&&e.target.closest('[data-tip]');if(!t)return;if(e.relatedTarget&&t.contains(e.relatedTarget))return;hideTip()});
document.addEventListener('focusin',function(e){var t=e.target.closest&&e.target.closest('[data-tip]');if(t&&t.dataset.tip){var r=t.getBoundingClientRect();showTip(t,r.left+r.width/2,r.bottom-14)}});
document.addEventListener('focusout',hideTip);
document.addEventListener('keydown',function(e){if(e.key==='Escape')hideTip()});
window.addEventListener('scroll',hideTip,true);
document.addEventListener('htmx:beforeSwap',hideTip);
/* ---- presets: export downloads the form as a text file; import posts a chosen file ---- */
document.addEventListener('click',function(e){
  if(e.target.closest('[data-export]')){var f=document.getElementById('form');if(f)window.location.href='/preset/export?'+new URLSearchParams(new FormData(f)).toString();return}
  if(e.target.closest('[data-import]')){var i=document.getElementById('preset-file');if(i)i.click();return}
});
document.addEventListener('change',function(e){
  var t=e.target;
  if(t.id==='preset-file'&&t.files&&t.files[0]){
    var rd=new FileReader();
    rd.onload=function(){htmx.ajax('POST','/preset/load',{target:'#app',swap:'outerHTML',values:{text:String(rd.result)}});t.value=''};
    rd.readAsText(t.files[0]);
  }
});
document.addEventListener('click',function(e){
  var t=e.target.closest('[data-dialog]');
  if(t){var d=document.getElementById(t.dataset.dialog);if(d&&d.showModal)d.showModal();return}
  if(e.target.closest('[data-retry]')&&window.lastFailed){
    var f=window.lastFailed;hideError();
    htmx.ajax('GET',f.path,{target:f.target,swap:f.swap,source:f.elt});
  }
});
/* ---- progress ---- */
var root=document.documentElement,mainReq=new Set(),costReq=new Set();
function paint(){root.classList.toggle('calculating',mainReq.size>0);root.classList.toggle('costing',costReq.size>0)}
function hideError(){var b=document.getElementById('status-error');if(b)b.hidden=true}
document.addEventListener('htmx:beforeRequest',function(e){
  var x=e.detail.xhr,p=(e.detail.pathInfo&&e.detail.pathInfo.requestPath)||'';
  var set=p.indexOf('/cost')===0?costReq:mainReq;
  set.add(x);x.addEventListener('loadend',function(){set.delete(x);paint()});paint();
});
document.addEventListener('htmx:afterRequest',function(e){if(e.detail.successful)hideError()});
/* keep a cost line's last value (dimmed) while the new one is worked out */
document.addEventListener('htmx:oobBeforeSwap',function(e){
  var t=e.detail.target,f=e.detail.fragment;
  var el=f&&(f.firstElementChild||f); /* htmx wraps the incoming element in a fragment */
  if(t&&el&&t.classList&&t.classList.contains('cost')&&el.classList){el.textContent=t.textContent;el.classList.add('stale')}
});
document.addEventListener('htmx:afterSwap',function(e){if(e.target&&e.target.classList&&e.target.classList.contains('cost'))e.target.classList.remove('stale')});
/* ---- errors ---- */
function fail(e){
  var d=e.detail,p=d.pathInfo||{};
  if((p.requestPath||'').indexOf('/cost')===0)return; /* a cost line can simply stay as it was */
  window.lastFailed={path:p.finalRequestPath||p.requestPath,target:d.target,elt:d.elt,swap:(d.elt&&d.elt.getAttribute&&d.elt.getAttribute('hx-swap'))||'innerHTML'};
  var b=document.getElementById('status-error');if(b)b.hidden=false;
}
['htmx:sendError','htmx:responseError','htmx:timeout'].forEach(function(n){document.addEventListener(n,fail)});
document.addEventListener('DOMContentLoaded',function(){if(window.htmx)htmx.config.timeout=120000});
"#;

/// The page before any calculation: the form (from the request alone) and a placeholder where the plan
/// will appear. The browser fetches the plan immediately, so the page is usable and shows progress at once.
pub fn page(req: &Request, presets: &[PresetInfo]) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en-AU" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "NZ White meat rabbit planner — WA Code of Practice" }
                link rel="stylesheet" href="/style.css";
                script { (PreEscaped(SCRIPT)) }
                script src="/htmx.min.js" defer {}
            }
            body {
                div #topbar aria-hidden="true" {}
                div #status role="status" aria-live="polite" {
                    span.spinner aria-hidden="true" {}
                    span.when-calc { "Re-planning your herd…" }
                    span.when-cost { "Working out what each constraint costs…" }
                }
                div #status-error role="alert" hidden {
                    span { "Couldn't reach the server, so what's on screen may be out of date." }
                    button type="button" data-retry { "Retry" }
                }
                header.top {
                    div { h1 { "NZ White meat rabbit planner" } p.tag { "Plan a small meat-rabbit herd that fits your space and the WA Code." } }
                    div.top-actions {
                        button #code-badge.badge type="button" data-dialog="code-dialog" data-tip="Code of Practice checks: click for details" { "Code of Practice" }
                        button.help-btn type="button" data-dialog="help-dialog" { "Help" }
                    }
                }
                div #app.wrap {
                    aside {
                        (preset_bar(presets, None))
                        (form(req, false))
                        div.actions style="margin-top:10px" {
                            button type="button" hx-get="/app?action=defaults" hx-target="#app" hx-swap="outerHTML" hx-indicator="#app" { "Restore starting values" }
                        }
                    }
                    main #results hx-get="/results" hx-include="#form" hx-trigger="load" hx-swap="innerHTML" hx-indicator="this" {
                        section.card.pending { div.big-spinner aria-hidden="true" {} p { "Planning your herd…" } p.small { "Running hundreds of randomised simulations and searching for the best plan." } }
                    }
                }
                dialog #code-dialog {}
                (help_dialog())
            }
        }
    }
}

/// Preset dropdown plus export / import. Lives outside the form so it never takes part in its requests.
fn preset_bar(presets: &[PresetInfo], note: Option<&str>) -> Markup {
    html! {
        div.presets {
            select #preset-select name="preset" aria-label="Load a preset" hx-get="/app?action=preset" hx-target="#app" hx-swap="outerHTML"
                hx-indicator="#app" hx-trigger="change" hx-sync="this:replace" {
                option value="" selected { "Load a preset…" }
                @for p in presets { option value=(p.id) title=(p.description) { (p.name) } }
            }
            div.preset-actions {
                button type="button" data-export data-tip="Download the current setup as a text file you can keep, share or edit" { "Export" }
                button type="button" data-import data-tip="Load a preset file saved with Export" { "Import…" }
                input #preset-file type="file" accept=".txt,text/plain" hidden;
            }
            @if let Some(n) = note { div.preset-note role="status" { @for (i, line) in n.lines().enumerate() { @if i > 0 { br; } (line) } } }
        }
    }
}

pub fn app(v: &View) -> Markup {
    html! {
        div #app.wrap {
            aside {
                (preset_bar(v.presets, v.note))
                (form(v.req, true))
                div.actions style="margin-top:10px" {
                    button type="button" hx-get="/app?action=defaults" hx-target="#app" hx-swap="outerHTML" hx-indicator="#app" { "Restore starting values" }
                }
            }
            main #results hx-indicator="this" {
                (results(v))
            }
        }
    }
}

/// Response for whole-app swaps: the app plus the (out-of-band) Code badge and dialog that live outside it.
pub fn app_swap(v: &View) -> Markup {
    html! { (app(v)) (code_badge(v, true)) (code_dialog(v, true)) }
}

fn form(req: &Request, armed: bool) -> Markup {
    let addable_vars: Vec<_> = VAR_UI.iter().filter(|u| !req.has(u.var.key())).collect();
    let addable_outs: Vec<_> = OUT_UI.iter().filter(|u| !req.has(u.out.key())).collect();
    let var_groups = ["Welfare", "Cage design", "Herd", "Timing"];
    html! {
        form #form hx-get="/results" hx-target="#results" hx-swap="innerHTML" hx-sync="this:replace" hx-indicator="#results"
            hx-trigger="input[target.type=='range'] changed delay:250ms, change[target.type!='range' && target.type!='number' && target.name!='add' && target.name!='objective']"
            onsubmit="return false" {
            input type="hidden" name="cons" value="1";
            details open { summary { "Objective" }
                div.objs role="radiogroup" aria-label="Objective" {
                    @for o in Objective::ALL {
                        label.obj {
                            input type="radio" name="objective" value=(o.key()) checked[o == req.objective]
                                hx-get="/app?action=objective" hx-include="#form" hx-target="#app" hx-swap="outerHTML" hx-indicator="#app" hx-trigger="change" hx-sync="this:replace";
                            span.obj-body { strong { (objective_label(o)) } small { (objective_blurb(o)) } }
                        }
                    }
                }
            }
            details open { summary { (SPACE.title) }
                @for c in SPACE.ctls { (control_row(req, c)) }
            }
            details open { summary { "Constraints" }
                p.note { (constraints_note(req.objective)) }
                @for (var, b) in &req.vars { (var_row(armed, *var, *b)) }
                @for (out, b) in &req.outs { (out_row(armed, *out, *b)) }
                @if req.vars.is_empty() && req.outs.is_empty() { p.note { "No constraints: everything is free." } }
                select.addcon name="add" aria-label="Add a constraint"
                    hx-get="/app?action=add" hx-include="#form" hx-target="#app" hx-swap="outerHTML" hx-indicator="#app" hx-trigger="change" hx-sync="this:replace" {
                    option value="" selected { "+ Constraint" }
                    @for g in var_groups {
                        @if addable_vars.iter().any(|u| u.group == g) {
                            optgroup label=(g) { @for u in addable_vars.iter().filter(|u| u.group == g) { option value=(u.var.key()) { (short_label(u.var)) } } }
                        }
                    }
                    @if !addable_outs.is_empty() {
                        optgroup label="Results" { @for u in &addable_outs { option value=(u.out.key()) { (short_out_label(u.out)) } } }
                    }
                }
            }
            details { summary { "Assumptions" }
                p.note { "Fixed inputs with typical NZ White and WA values pre-loaded. Change them if yours differ." }
                @for g in ASSUMPTIONS.iter() {
                    details open[g.open] { summary { (g.title) }
                        @if !g.note.is_empty() { p.note { (g.note) } }
                        @for c in g.ctls { (control_row(req, c)) }
                    }
                }
            }
        }
    }
}

/// Digits needed to show `step`.
fn decimals(step: f64) -> usize {
    let mut d = 0;
    while d < 4 && ((step * 10f64.powi(d as i32)).round() - step * 10f64.powi(d as i32)).abs() > 1e-6 {
        d += 1;
    }
    d
}

/// Slider plus its number box: the slider carries what the browser needs to keep the two in step.
#[allow(clippy::too_many_arguments)]
fn slider_with_entry(
    id: &str,
    name: &str,
    min: f64,
    max: f64,
    step: f64,
    value: f64,
    pre: &str,
    suf: &str,
    scale: f64,
    weeks: bool,
) -> (Markup, Markup) {
    let dec = decimals(step * scale);
    let fmt = |x: f64| format!("{:.*}", dec, x * scale);
    let num_id = format!("num-{id}");
    let slider = html! {
        input type="range" id=(id) name=(name) min=(min) max=(max) step=(step) value=(value)
            data-num=(num_id) data-scale=(scale) data-dec=(dec) style={"--fill:" ((value - min) / (max - min) * 100.0) "%"};
    };
    let entry = html! {
        span.numwrap {
            @if !pre.is_empty() { span.pre { (pre) } }
            input.numbox type="number" id=(num_id) data-for=(id) min=(fmt(min)) max=(fmt(max)) step=(format!("{:.*}", dec, step * scale)) value=(fmt(value)) aria-label="Type a value"
                style={"--n:" (fmt(min).len().max(fmt(max).len()).max(2))};
            @if !suf.is_empty() { span.unit { (suf) } }
            @if weeks { span.unit id={"cap-" (id)} { " = " (trim1_w(value / 7.0)) " wk" } }
        }
    };
    (slider, entry)
}

fn trim1_w(v: f64) -> String {
    let s = format!("{v:.1}");
    s.strip_suffix(".0").map(str::to_string).unwrap_or(s)
}

/// A fixed input. Sliders show their value; dropdowns and choices already show theirs, so they don't repeat it.
fn control_row(req: &Request, c: &Ctl) -> Markup {
    let cur = (c.get)(&req.params);
    html! {
        div.ctl data-key=(c.key) {
            @match c.kind {
                Kind::Range { min, max, step } => {
                    @let (pre, suf) = c.fmt.affixes();
                    @let (slider, entry) = slider_with_entry(&format!("in-{}", c.key), c.key, min, max, step, cur, pre, suf, c.fmt.scale(), matches!(c.fmt, super::controls::Fmt::WeeksDays));
                    div.ctl-top {
                        label for={"in-" (c.key)} { (c.label) }
                        (entry)
                    }
                    (slider)
                    @if !c.help.is_empty() { p.help { (c.help) } }
                }
                Kind::Bool => {
                    div.ctl-top {
                        label for={"in-" (c.key)} data-tip=(c.help) { (c.label) }
                        select id={"in-" (c.key)} name=(c.key) {
                            option value="true" selected[cur > 0.5] { "yes" }
                            option value="false" selected[cur <= 0.5] { "no" }
                        }
                    }
                }
                Kind::Choice(cs) => {
                    div.ctl-top { label data-tip=(c.help) { (c.label) } }
                    div.seg role="radiogroup" {
                        @for (val, label) in cs.iter() {
                            label { input type="radio" name=(c.key) value=(val) checked[(val - cur).abs() < 1e-9]; span { (label) } }
                        }
                    }
                }
            }
        }
    }
}

/// A constraint's cost line. `armed` chips fetch their text as soon as they appear; the page shell's
/// chips wait (the plan response replaces them with armed ones), so the shell triggers no extra solves.
fn cost_chip(key: &str, oob: bool, armed: bool) -> Markup {
    html! {
        p.cost id={"cost-" (key)} hx-get={"/cost?drop=" (key)} hx-include="#form" hx-trigger=[armed.then_some("load")] hx-target="this" hx-swap="innerHTML" hx-indicator="this" hx-sync="this:replace"
            hx-swap-oob=[oob.then_some("true")] { @if armed { "working out what this costs…" } @else { "waiting for the plan…" } }
    }
}

fn op_select(key: &str, cur: Op, allowed: &[Op]) -> Markup {
    html! {
        @if allowed.len() == 1 {
            input type="hidden" name={"op_" (key)} value=(allowed[0].key());
            span.op-fixed { (allowed[0].label()) }
        } @else {
            select.op name={"op_" (key)} aria-label="Constraint type" {
                @for op in allowed { option value=(op.key()) selected[*op == cur] { (op.label()) } }
            }
        }
    }
}

fn remove_button(key: &str) -> Markup {
    html! {
        button.x type="button" data-tip="Remove this constraint" aria-label="Remove this constraint"
            hx-get={"/app?action=remove&key=" (key)} hx-include="#form" hx-target="#app" hx-swap="outerHTML" hx-indicator="#app" hx-sync="this:replace" { "×" }
    }
}

/// Compact: name and × on one line, then operator, slider and value on one line.
#[allow(clippy::too_many_arguments)]
fn cons_row(
    armed: bool,
    key: &str,
    label: &str,
    help: &str,
    op: Op,
    allowed: &[Op],
    slider: Markup,
    value: Markup,
    body_override: Option<Markup>,
) -> Markup {
    html! {
        div.ctl.cons data-key=(key) {
            input type="hidden" name={"on_" (key)} value="1";
            div.cons-top { label.cons-label data-tip=(help) for={"in-val_" (key)} { (label) } (remove_button(key)) }
            @if let Some(b) = body_override { (b) } @else {
                div.cons-ctl { (op_select(key, op, allowed)) (slider) (value) }
            }
            (cost_chip(key, false, armed))
        }
    }
}

fn var_row(armed: bool, var: Var, b: Bound) -> Markup {
    let u = var_ui(var);
    let key = var.key();
    let cur = bound_value(&b);
    match u.kind {
        Kind::Choice(cs) => {
            let radios = html! {
                div.seg role="radiogroup" {
                    @for (val, label) in cs.iter() {
                        label { input type="radio" name={"val_" (key)} value=(val) checked[(val - cur).abs() < 1e-9]; span { (label) } }
                    }
                }
            };
            cons_row(armed, key, short_label(var), u.help, Op::Exactly, &[Op::Exactly], html! {}, html! {}, Some(radios))
        }
        _ => {
            let (min, max, step) = u.range().unwrap_or((0.0, 1.0, 0.01));
            let (pre, suf) = u.fmt.affixes();
            let (slider, entry) = slider_with_entry(
                &format!("in-val_{key}"),
                &format!("val_{key}"),
                min,
                max,
                step,
                cur,
                pre,
                suf,
                u.fmt.scale(),
                matches!(u.fmt, super::controls::Fmt::WeeksDays),
            );
            cons_row(armed, key, short_label(var), u.help, Op::of(&b), allowed_ops(var), slider, entry, None)
        }
    }
}

fn out_row(armed: bool, out: Out, b: Bound) -> Markup {
    let u = out_ui(out);
    let key = out.key();
    let cur = bound_value(&b);
    let (slider, entry) =
        slider_with_entry(&format!("in-val_{key}"), &format!("val_{key}"), u.min, u.max, u.step, cur, "", u.unit, 1.0, false);
    cons_row(
        armed,
        key,
        &format!("Result: {}", short_out_label(out)),
        "",
        Op::of(&b),
        &[Op::AtLeast, Op::Exactly, Op::AtMost],
        slider,
        entry,
        None,
    )
}

/// What a constraint costs, in the objective's own terms.
pub fn cost_text(objective: Objective, full: &Solution, without: &Solution) -> String {
    if !full.feasible() && without.feasible() {
        return "Conflicts with your other constraints — removing it would make the plan work.".into();
    }
    let gain = objective.improvement(full.objective_value, without.objective_value);
    if gain < objective.epsilon() {
        return "Not limiting — the plan would be the same without it.".into();
    }
    match objective {
        Objective::MaxMeat => {
            let pct = if full.objective_value > 0.0 { gain / full.objective_value * 100.0 } else { 0.0 };
            format!("Costs {} kg/yr of meat ({}%)", fmt_num(gain, 0), fmt_num(pct, 0))
        }
        Objective::MaxQol => format!("Costs {:.0}% of extra floor space", gain * 100.0),
    }
}

// ---------------- results ----------------

struct Stat {
    key: MetricKey,
    icon: &'static str,
    label: &'static str,
    unit: &'static str,
    dp: usize,
    scale: f64,
    money: bool,
}

const fn stat(key: MetricKey, icon: &'static str, label: &'static str, unit: &'static str, dp: usize, scale: f64, money: bool) -> Stat {
    Stat { key, icon, label, unit, dp, scale, money }
}

const THEMES: [(&str, [Stat; 4]); 3] = [
    (
        "Meat",
        [
            stat(MetricKey::MeatKgPerYr, "🥩", "Dressed meat", "kg/yr", 0, 1.0, false),
            stat(MetricKey::FryersPerYr, "🍖", "Rabbits butchered", "/yr", 0, 1.0, false),
            stat(MetricKey::ButcherAgeWk, "📅", "Age at butchering", "wk", 1, 1.0, false),
            stat(MetricKey::ForcedPerYr, "⚠️", "Butchered early (no room)", "/yr", 1, 1.0, false),
        ],
    ),
    (
        "Food",
        [
            stat(MetricKey::PelletKgPerYr, "🌾", "Pellets", "kg/yr", 0, 1.0, false),
            stat(MetricKey::HayKgPerYr, "🌿", "Hay", "kg/yr", 0, 1.0, false),
            stat(MetricKey::FeedCostPerYr, "💵", "Feed cost", "/yr", 0, 1.0, true),
            stat(MetricKey::CostPerKg, "⚖️", "Cost per kg of meat", "/kg", 2, 1.0, true),
        ],
    ),
    (
        "Herd",
        [
            stat(MetricKey::PeakHead, "🐇", "Peak rabbits on hand", "", 0, 1.0, false),
            stat(MetricKey::CageUse, "🏠", "Cages in use", "%", 0, 100.0, false),
            stat(MetricKey::PeakHead, "", "", "", 0, 1.0, false),
            stat(MetricKey::PeakHead, "", "", "", 0, 1.0, false),
        ],
    ),
];

fn stats_card(v: &View) -> Markup {
    let s = v.summary();
    html! {
        section.card.stats {
            @for (title, items) in THEMES.iter() {
                div.theme {
                    h3 { (title) }
                    div.theme-items {
                    @for it in items.iter().filter(|i| !i.label.is_empty()) {
                        @let st = s.metric(it.key);
                        @let cash = if it.money { "$" } else { "" };
                        @let tip = format!("{}: {}{} {}\n80% of runs fall between {}{} and {}{}", it.label, cash, fmt_num(st.mean * it.scale, it.dp), it.unit,
                            cash, fmt_num(st.p10 * it.scale, it.dp), cash, fmt_num(st.p90 * it.scale, it.dp));
                        div.stat data-tip=(tip) {
                            span.ico aria-hidden="true" { (it.icon) }
                            span.sv { (cash) (fmt_num(st.mean * it.scale, it.dp)) span.su { (it.unit) } }
                            span.sl { (it.label) }
                        }
                    }
                    }
                }
            }
        }
    }
}

fn role_label(r: CageRole) -> &'static str {
    match r {
        CageRole::DoeLitter => "Doe + litter",
        CageRole::Doe => "Doe",
        CageRole::Buck => "Buck",
        CageRole::Replacement => "Replacement",
        CageRole::Growers => "Growers",
        CageRole::Empty => "Empty",
    }
}

fn role_var(r: CageRole) -> &'static str {
    match r {
        CageRole::DoeLitter => "--s1",
        CageRole::Buck => "--s2",
        CageRole::Doe => "--s3",
        CageRole::Replacement => "--s4",
        CageRole::Growers => "--s5",
        CageRole::Empty => "--empty",
    }
}

fn layout_card(v: &View) -> Markup {
    let l = &v.summary().layout;
    let p = v.p();
    let snap = (v.summary().snapshot.len() == l.n_cages()).then_some(&v.summary().snapshot);
    let (tw, gap) = (220.0, 28.0);
    let scale = (tw / p.width_m).min(150.0 / p.depth_m);
    let (pw, pd) = (p.width_m * scale, p.depth_m * scale);
    let w = l.tiers.max(1) as f64 * (pw + gap) - gap + 20.0;
    let h = pd + 56.0;
    let mut svg =
        format!(r#"<svg viewBox="0 0 {w:.0} {h:.0}" width="{w:.0}" height="{h:.0}" role="img" aria-label="Plan view of each cage tier">"#);
    for t in 0..l.tiers {
        let (x0, y0) = (10.0 + t as f64 * (pw + gap), 22.0);
        let suffix = if t == 0 {
            " (bottom)"
        } else if t == l.tiers - 1 {
            " (top)"
        } else {
            ""
        };
        svg += &format!(
            r#"<text x="{x0:.1}" y="14" class="t-lab">Tier {}{suffix}</text><rect x="{x0:.1}" y="{y0}" width="{pw:.1}" height="{pd:.1}" class="space"/>"#,
            t + 1
        );
        let ch = l.row_depth * scale;
        for (ci, cg) in l.cages.iter().enumerate().filter(|(_, c)| c.tier == t) {
            let cw = cg.w * scale;
            let x = x0 + cg.x * scale;
            let y = if l.rows == 2 && cg.row == 1 { y0 } else { y0 + pd - ch };
            let (role, count) = snap.map_or((CageRole::Empty, 0), |s| (s[ci].role, s[ci].count));
            let label = match role {
                CageRole::Growers => format!("Growers ×{count}"),
                CageRole::DoeLitter => format!("Doe + {}", count - 1),
                r => role_label(r).to_string(),
            };
            let kind = match cg.kind {
                CageKind::Doe => "doe cage",
                CageKind::Buck => "buck cage",
                CageKind::Grow => "grower cage",
            };
            svg += &format!(
                r#"<g data-tip="{} — {kind}, {:.2} × {:.2} m = {:.2} m²"><rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="4" class="cage" style="fill:var({})"/>"#,
                esc(&label),
                cg.w,
                cg.d,
                cg.area,
                x + 1.0,
                y + 1.0,
                (cw - 2.0).max(0.0),
                (ch - 2.0).max(0.0),
                role_var(role)
            );
            if cw > 46.0 {
                svg += &format!(
                    r#"<text x="{:.1}" y="{:.1}" class="c-lab" text-anchor="middle">{}</text><text x="{:.1}" y="{:.1}" class="t-dim" text-anchor="middle">{:.2} m²</text>"#,
                    x + cw / 2.0,
                    y + ch / 2.0 + 1.0,
                    esc(&label),
                    x + cw / 2.0,
                    y + ch / 2.0 + 12.0,
                    cg.area
                );
            }
            svg += "</g>";
        }
        svg += &format!(
            r#"<text x="{:.1}" y="{:.1}" class="t-dim" text-anchor="middle">{}</text>"#,
            x0 + pw / 2.0,
            y0 + pd + 16.0,
            if l.rows == 2 { "access front &amp; back" } else { "access front ▾" }
        );
    }
    svg += "</svg>";
    let area_range = |kind: CageKind| -> String {
        let a: Vec<f64> = l.cages.iter().filter(|c| c.kind == kind).map(|c| c.area).collect();
        let (lo, hi) = a.iter().fold((f64::MAX, 0.0f64), |(x, y), &v| (x.min(v), y.max(v)));
        if a.is_empty() {
            "–".into()
        } else if (hi - lo) < 0.005 {
            format!("{lo:.2} m²")
        } else {
            format!("{lo:.2}–{hi:.2} m²")
        }
    };
    html! {
        section.card {
            h2 { "Cage layout" }
            div.layout-body {
                div.layout-plan {
                    div.layout-svg { @if l.tiers > 0 { (PreEscaped(svg)) } @else { p { "No cages fit." } } }
                    div.legend {
                        @for r in [CageRole::DoeLitter, CageRole::Buck, CageRole::Doe, CageRole::Replacement, CageRole::Growers, CageRole::Empty] {
                            span.lg { i style={"background:var(" (role_var(r)) ")"} {} (role_label(r)) }
                        }
                    }
                }
                div.layout-tables {
                    table.facts-table {
                        thead { tr { th { "Cages" } th.num { "No." } th.num { "Each" } } }
                        tbody {
                            tr { td { "Doe + litter" } td.num { (l.n_doe) } td.num { (area_range(CageKind::Doe)) } }
                            tr { td { "Buck" } td.num { (l.n_buck) } td.num { (area_range(CageKind::Buck)) } }
                            tr { td { "Grower" } td.num { (l.n_grow) } td.num { (area_range(CageKind::Grow)) } }
                        }
                        tfoot { tr { td { "Total" } td.num { (l.n_cages()) } td.num {} } }
                    }
                    table.facts-table {
                        thead { tr { th colspan="2" { "Space" } } }
                        tbody {
                            tr { td { "Tiers" } td.num { (l.tiers) " of " (l.max_tiers) " that fit in " (format!("{:.2}", p.height_m)) " m" } }
                            tr { td { "Rows" } td.num { (l.rows) @if l.rows == 2 { " (front and back)" } } }
                            tr { td { "Cage depth" } td.num { (format!("{:.2} m", l.row_depth)) } }
                            tr { td { "Cage height" } td.num { (p.cage_height_cm) " cm" } }
                            @if l.unused_depth > 0.01 { tr { td { "Out of reach" } td.num { (format!("{:.2} m of depth, unused", l.unused_depth)) } } }
                        }
                    }
                }
            }
        }
    }
}

// ---------------- Code of Practice dialog ----------------

fn trim1(v: f64) -> String {
    let s = format!("{v:.1}");
    s.strip_suffix(".0").map(str::to_string).unwrap_or(s)
}

/// Every check as (passed, text).
fn code_checks(v: &View) -> Vec<(bool, String)> {
    let l = &v.summary().layout;
    let p = v.p();
    let s = v.summary();
    let w = if s.runs > 0 { s.eff_wean } else { p.wean_days.min(56) };
    let doe_min = (if w > 35 { code::DOE_LITTER_8WK } else { code::DOE_LITTER_5WK }) * p.qol_doe;
    let forced = s.metric(MetricKey::ForcedPerYr).mean;
    let share = s.forced_share();
    let mut c = vec![
        (
            l.n_doe > 0 && l.doe_area + 1e-9 >= doe_min,
            format!("Doe + litter cages are {:.2} m² (Code: {doe_min:.2} m² for litters to {} wk)", l.doe_area, trim1(w as f64 / 7.0)),
        ),
        (
            l.n_buck > 0 && l.buck_area + 1e-9 >= code::BREEDER * p.qol_buck,
            format!("Buck cages are {:.2} m² (adult breeder minimum {:.2} m²)", l.buck_area, code::BREEDER * p.qol_buck),
        ),
        (
            p.cage_height_cm >= code::MIN_HEIGHT_CM,
            format!("Cage height {} cm (minimum 45 cm, and enough to sit up with ears erect)", p.cage_height_cm),
        ),
        (
            true,
            format!(
                "Growers get {:.2} m² each (5–12 wk) and {:.2} m² each (12 wk+), checked every simulated day",
                code::GROWER_5_TO_12WK * p.qol_young,
                code::OVER_12WK_GROUP * p.qol_old
            ),
        ),
        (true, "Replacement bucks housed alone from 10 weeks; replacement does from 12 weeks".to_string()),
    ];
    if s.runs > 0 && s.eff_does < p.does as usize {
        c.push((false, format!("Only {} of {} doe cages fit in this space", s.eff_does, p.does)));
    }
    if forced > 0.5 {
        if share > 0.03 {
            c.push((
                false,
                format!(
                    "{:.0}% of rabbits ({forced:.0}/yr) have to be butchered before the planned age because the cages are full",
                    share * 100.0
                ),
            ));
        } else {
            c.push((true, format!("Occasionally (about {forced:.0}/yr) a rabbit is butchered a little early because the cages are full")));
        }
    }
    for i in &l.issues {
        c.push((false, i.clone()));
    }
    c
}

fn code_badge(v: &View, oob: bool) -> Markup {
    let bad = code_checks(v).iter().filter(|(ok, _)| !ok).count();
    html! {
        button #code-badge class={"badge " (if bad == 0 { "ok" } else { "bad" })} type="button" data-dialog="code-dialog"
            data-tip="Code of Practice checks: click for details" hx-swap-oob=[oob.then_some("true")] {
            span aria-hidden="true" { (if bad == 0 { "✓" } else { "✗" }) }
            " " (if bad == 0 { "Code of Practice".to_string() } else { format!("{bad} Code issue{}", if bad == 1 { "" } else { "s" }) })
        }
    }
}

fn code_dialog(v: &View, oob: bool) -> Markup {
    html! {
        dialog #code-dialog hx-swap-oob=[oob.then_some("true")] {
            div.dlg-head { h2 { "Code of Practice checks" } form method="dialog" { button { "Close" } } }
            ul.checks {
                @for (ok, text) in code_checks(v) {
                    li class=(if ok { "ok" } else { "bad" }) { span aria-hidden="true" { (if ok { "✓" } else { "!" }) } " " span.sr { (if ok { "OK:" } else { "Problem:" }) } (text) }
                }
            }
            p.small {
                strong { "Not modelled, still required by the Code:" }
                " mesh openings ≤ 19 × 19 mm (13 × 13 mm for kittens), wire ≥ 2.5 mm; a solid resting board ≥ 0.1 m² covering at most ⅓ of the floor; nest box in at least 2 days before kindling; daily inspection (leave litters under a week old undisturbed); 10–25 °C optimum; does taken to the buck for mating; humane slaughter by a competent person."
            }
        }
    }
}

// ---------------- plan ----------------

fn plan_card(v: &View) -> Markup {
    let p = v.p();
    let l = &v.summary().layout;
    let mean_qol = Var::QOL_TYPES.iter().map(|x| x.get(p)).sum::<f64>() / 4.0;
    let mark = |var: Var| -> Markup {
        let own = v.req.vars.iter().find(|(x, _)| *x == var).map(|(_, b)| b);
        let inherited = var.is_qol() && var != Var::Qol && own.is_none() && v.req.vars.iter().any(|(x, _)| *x == Var::Qol);
        html! {
            @if own.is_some_and(|b| b.exact().is_some()) { span.pin data-tip="Pinned by you" { "🔒" } }
            @else if own.is_some() || inherited { span.lim data-tip="Within a limit you set" { "↕" } }
        }
    };
    let row = |label: &str, var: Option<Var>, text: String| {
        html! {
            div.pr { span.pl { (label) @if let Some(vr) = var { (mark(vr)) } } span.pv { (text) } }
        }
    };
    let t = |var: Var| var_ui(var).value_text(var.get(p));
    let (big, small) = match v.req.objective {
        Objective::MaxMeat => (
            format!("{} kg/yr", fmt_num(v.sol.objective_value, 0)),
            "dressed meat — the most this space gives within your constraints".to_string(),
        ),
        Objective::MaxQol => {
            (fmt_qol_short(mean_qol), "floor space vs the Code minimum — the most that still meets your result limits".to_string())
        }
    };
    html! {
        section.card.plan {
            div.plan-head {
                div { div.big { (big) } div.small { (small) } }
                @if v.req.objective == Objective::MaxQol && !v.req.limits_floor_space() {
                    p.warn { "Nothing you've set limits floor space, only what fits. Add a result constraint (dressed meat, or rabbits on hand) from “+ Constraint” to trade room against it." }
                }
            }
            div.plan-cols {
                div.pc { h3 { "Herd" }
                    (row("Breeding does", Some(Var::Does), t(Var::Does)))
                    (row("Bucks", Some(Var::Bucks), t(Var::Bucks)))
                    @if p.max_headcount > 0 { (row("Max rabbits", Some(Var::MaxHeadcount), p.max_headcount.to_string())) }
                }
                div.pc { h3 { "Cages" }
                    (row("Tiers", Some(Var::Tiers), format!("{}", l.tiers)))
                    (row("Doe / buck / grower", None, format!("{} / {} / {}", l.n_doe, l.n_buck, l.n_grow)))
                    (row("Grower width", Some(Var::GrowerCageW), t(Var::GrowerCageW)))
                }
                div.pc { h3 { "Timing" }
                    (row("Wean at", Some(Var::WeanDays), t(Var::WeanDays)))
                    (row("Re-mate after", Some(Var::RebreedDays), t(Var::RebreedDays)))
                    (row("Butcher", Some(Var::ButcherRule), match p.butcher_rule { crate::engine::ButcherRule::Age => format!("at {}", trim1(p.butcher_days as f64 / 7.0) + " wk"), crate::engine::ButcherRule::Room => format!("when room is needed, by {} wk", trim1(p.butcher_days as f64 / 7.0)) }))
                }
                div.pc { h3 { "Floor space" }
                    (row("Doe + litter", Some(Var::QolDoe), t(Var::QolDoe)))
                    (row("Buck", Some(Var::QolBuck), t(Var::QolBuck)))
                    (row("Growers 5–12 wk", Some(Var::QolYoung), t(Var::QolYoung)))
                    (row("Growers 12 wk+", Some(Var::QolOld), t(Var::QolOld)))
                }
            }
        }
    }
}

fn fmt_qol_short(v: f64) -> String {
    if v <= 1.0 + 1e-9 { "Code minimum".to_string() } else { format!("+{:.0}% floor space", (v - 1.0) * 100.0) }
}

fn violation_note(v: &View) -> Markup {
    let share = v.summary().forced_share();
    html! {
        @if !v.sol.violations.is_empty() {
            div.warn {
                strong { "! These constraints can't all be met" } " (closest plan shown):"
                ul { @for x in &v.sol.violations {
                    li {
                        @match x.subject {
                            Subject::Var(var) => { (var_ui(var).label) ": wanted " (bound_text(&x.bound, |n| var_ui(var).value_text(n))) ", got " (var_ui(var).value_text(x.actual)) }
                            Subject::Out(out) => { (out_ui(out).label) ": wanted " (bound_text(&x.bound, |n| out_ui(out).value_text(n))) ", got " (out_ui(out).value_text(x.actual)) }
                        }
                    }
                } }
            }
        }
        @if share > 0.03 {
            div.warn { strong { "! This plan doesn't fit:" } " " (format!("{:.0}", share * 100.0)) "% of rabbits are butchered early because the cages are full. The figures assume you do that; consider the \"when room is needed\" rule, fewer does or a younger butchering age." }
        }
    }
}

fn bound_text(b: &Bound, fmt: impl Fn(f64) -> String) -> String {
    match (b.exact(), b.min, b.max) {
        (Some(x), _, _) => format!("exactly {}", fmt(x)),
        (None, Some(m), None) => format!("at least {}", fmt(m)),
        (None, None, Some(m)) => format!("at most {}", fmt(m)),
        (None, Some(a), Some(c)) => format!("{}–{}", fmt(a), fmt(c)),
        _ => "any".into(),
    }
}

fn charts_block(v: &View) -> Markup {
    let s = v.summary();
    let p = v.p();
    let warm_weeks = ((p.warmup_days.min(p.years * 365 - 1)) / 7) as usize;
    html! {
        div.charts {
            section.card {
                h2 { "Rabbits on hand" }
                p.sub { "Average across runs; shaded band = 80% of runs. Annual figures exclude the shaded start-up period." }
                div.ch-legend { i style="background:var(--s1)" {} "All rabbits (incl. unweaned kits)" i style="background:var(--s2)" {} "Growers" }
                div.chart { (chart(&ChartSpec { kind: ChartKind::Line, dp: 0, warm_weeks, capacity: None, series: vec![
                    ChartSeries { name: "All rabbits", colour: 1, mean: &s.head.mean, band: Some((&s.head.p10, &s.head.p90)) },
                    ChartSeries { name: "Growers", colour: 2, mean: &s.growers.mean, band: None },
                ] })) }
            }
            div.grid2 {
                section.card {
                    h2 { "Dressed meat per week (kg)" } p.sub { "Average across runs." }
                    div.chart { (chart(&ChartSpec { kind: ChartKind::Bar, dp: 1, warm_weeks, capacity: None, series: vec![
                        ChartSeries { name: "Dressed meat (kg)", colour: 1, mean: &s.meat.mean, band: None } ] })) }
                }
                section.card {
                    h2 { "Cages in use" } p.sub { "Average across runs; band = 80% of runs." }
                    div.chart { (chart(&ChartSpec { kind: ChartKind::Line, dp: 1, warm_weeks,
                        capacity: Some((s.layout.n_cages() as f64, format!("{} cages available", s.layout.n_cages()))), series: vec![
                        ChartSeries { name: "Cages in use", colour: 3, mean: &s.cages.mean, band: Some((&s.cages.p10, &s.cages.p90)) } ] })) }
                }
            }
        }
    }
}

/// Everything inside `#results`.
pub fn results(v: &View) -> Markup {
    let l = &v.summary().layout;
    let empty = !l.feasible || v.summary().eff_does == 0;
    html! {
        div #lever-diff { (violation_note(v)) }
        @if empty {
            div #infeasible-msg {
                strong { "No production possible with this cage layout." } " "
                @for i in &l.issues { (i) " " }
            }
        }
        @if !empty {
            (plan_card(v))
            (stats_card(v))
        }
        (layout_card(v))
        @if !empty { (charts_block(v)) }
    }
}

/// `#results` content plus out-of-band refreshes of every cost chip and the Code badge.
pub fn results_fragment(v: &View) -> Markup {
    html! {
        (results(v))
        (code_badge(v, true))
        (code_dialog(v, true))
        @for (var, _) in &v.req.vars { (cost_chip(var.key(), true, true)) }
        @for (out, _) in &v.req.outs { (cost_chip(out.key(), true, true)) }
    }
}

fn help_dialog() -> Markup {
    html! {
        dialog #help-dialog.wide {
            div.dlg-head { h2 { "Help" } form method="dialog" { button { "Close" } } }
            div.help-body {
                h3 { "What this does" }
                p { "It simulates a small breeding herd of NZ White meat rabbits day by day, hundreds of times with randomised litters, losses and conceptions, in the space you describe and under the "
                    a href="https://www.dpird.wa.gov.au/siteassets/documents/agriculture/livestock/animal-welfare-code-of-practice/rabbits-code-of-practice-for-keeping-rabbits-in-wa.pdf" { "WA Code of Practice for keeping rabbits" }
                    " (Section 3 floor areas). Then it works out the best plan: how many does and bucks, how many tiers and what cage sizes, and when to wean, re-mate and butcher." }
                h3 { "Using it" }
                ol {
                    li { strong { "Choose an objective." } " Most meat, or the most floor space for the rabbits. Maximising floor space starts with a minimum for meat and for rabbits on hand (75% of the best plan) so the answer means something." }
                    li { strong { "Describe your space." } " Width, depth, height, and whether you can reach in from both long sides." }
                    li { strong { "Add constraints." } " Use “+ Constraint” to limit anything the solver would otherwise choose: floor space per cage type, the number of rabbits at once, tiers, does, weaning age, and so on. Each can be " em { "exactly" } ", " em { "at least" } " or " em { "at most" } " a value, and you can also limit results: dressed meat, rabbits butchered, or rabbits on hand (peak). “Max rabbits” is different: it is a ceiling on the herd, so it only has “at most”. To ask for a bigger herd, use the result “Rabbits on hand (peak)” with “at least”. Anything you don't constrain is chosen for you." }
                    li { strong { "Read what each constraint costs." } " Under every constraint a line says what it costs you in the objective (for example “Costs 35 kg/yr of meat”), or that it isn't limiting. It works that out by re-planning without it." }
                    li { strong { "Check the Code badge." } " The tick or cross at the top right opens the Code of Practice checks." }
                }
                h3 { "Floor space (quality of life)" }
                p { "Floor space is a multiplier on the Code minimum: “minimum” is the Code, “+50%” is one and a half times it. There are four cage types: the doe with her litter, the buck, growers 5–12 weeks and growers over 12 weeks. “All cages” sets all four; add a per-type constraint to override one. More room means fewer rabbits fit, so it costs meat." }
                h3 { "Reading the results" }
                ul {
                    li { "Figures are averages over many randomised runs, after a start-up period. Hover a figure for the range that 80% of runs fell in." }
                    li { "Pins (🔒) and limits (↕) in the plan show which values you set; the rest were chosen for you." }
                    li { "“Butchered early (no room)” counts rabbits that had to go before the planned age because the cages were full. The solver only accepts plans where this stays at or under 3%, and warns if it can't." }
                }
                (how_it_works())
            }
        }
    }
}

fn how_it_works() -> Markup {
    html! {
        h3 { "How the model works" }
        ul.assume {
            li { "Day-by-day simulation across every CPU core. Does and bucks are bought in at the start and raised to breeding age; replacements are kept from your own litters (or bought in)." }
            li { "Cages are different sizes: each doe has a cage sized for her and her litter to weaning, each buck a breeder-sized cage, and the rest of each row is cut into grower cages. Litters stay with the doe until weaning, then move into grower cages, packed to the Code's per-rabbit areas and split by sex once older than the split age." }
            li { "Two butchering rules: " em { "at set age" } ", or " em { "when room is needed" } " (oldest growers go whenever the cages fill or the headcount cap is hit, and all by the butchering age)." }
            li { "A litter is weaned early if the doe is due to kindle again (nest box goes in 2 days before). Weaning at 5 weeks or earlier lets doe cages be the smaller 0.56 m² size; later weaning needs 0.74 m². The fastest re-mating allowed is 11 days after kindling." }
            li { "With a headcount cap, a doe is only mated if there will be room for her litter; if the cap is still exceeded, the oldest growers are butchered early." }
            li { "Growth: about 1.7 kg at 8 weeks, 2.6 kg at 12 weeks, 3.3 kg at 16 weeks; dressing out 50% at 8 weeks rising to 58% by 16 weeks. Feed scales with metabolic weight (kg^0.75), plus extra for nursing does. These are typical NZ White figures, not Code requirements; change them under Assumptions." }
            li { "Not modelled: heat or season effects, inbreeding, falling litter size from intensive breeding, disease outbreaks." }
        }
    }
}
