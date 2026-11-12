use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

async fn get(uri: &str) -> (StatusCode, String) {
    let res = rabbitsim::web::router().oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap()).await.unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

/// Number in the stat with this label (read from its tooltip: `data-tip="Label: 337 kg/yr\n80% of runs…"`).
fn kpi_value(html: &str, label: &str) -> f64 {
    let needle = format!("data-tip=\"{label}: ");
    let i = html.find(&needle).unwrap_or_else(|| panic!("no stat {label}")) + needle.len();
    let num: String =
        html[i..].chars().take_while(|c| c.is_ascii_digit() || matches!(c, ',' | '.' | '$')).filter(|c| !matches!(c, ',' | '$')).collect();
    num.parse().unwrap()
}

/// Query string for a request: fast runs, default-style constraints, plus any extras.
fn q(extra: &str) -> String {
    format!("cons=1&runs=40&years=4&{extra}")
}

/// The shell (`/`) plus the plan the browser fetches straight after (`/results`), for assertions about both.
async fn full() -> String {
    let shell = get("/").await.1;
    let plan = get(&format!("/results?{}", q(DEFAULT_CONS))).await.1;
    format!("{shell}{plan}")
}

const DEFAULT_CONS: &str = "on_qol=1&op_qol=min&val_qol=1&on_max_headcount=1&op_max_headcount=max&val_max_headcount=0";

#[tokio::test]
async fn index_shows_objective_space_default_constraints_and_assumptions() {
    let (s, shell) = get("/").await;
    let h = format!("{shell}{}", get(&format!("/results?{}", q(DEFAULT_CONS))).await.1);
    assert_eq!(s, StatusCode::OK);
    assert!(h.contains("<!DOCTYPE html>") && h.contains("/htmx.min.js"));
    // objective
    for o in ["max_meat", "max_qol"] {
        assert!(!h.contains("min_cost"));
        assert!(h.contains(&format!("name=\"objective\" value=\"{o}\"")), "objective {o}");
    }
    // fixed space always shown
    for k in ["width_m", "depth_m", "height_m", "access_both_sides"] {
        assert!(h.contains(&format!("name=\"{k}\"")), "{k}");
    }
    // default constraints: one QoL and the herd cap, nothing else
    assert!(h.contains("name=\"on_qol\"") && h.contains("name=\"on_max_headcount\""));
    for k in ["does", "bucks", "tiers", "qol_doe", "wean_days", "meat"] {
        assert!(!h.contains(&format!("name=\"on_{k}\"")), "{k} should not be a default constraint");
    }
    // the menu offers the rest, grouped, and not what's already active
    assert!(h.contains("+ Constraint"));
    assert!(h.contains("optgroup label=\"Welfare\"") && h.contains("optgroup label=\"Results\""));
    assert!(h.contains("<option value=\"qol_doe\">") && h.contains("<option value=\"does\">") && h.contains("<option value=\"meat\">"));
    assert!(!h.contains("<option value=\"qol\">") && !h.contains("<option value=\"max_headcount\">"));
    // assumptions are pre-loaded but tucked away
    assert!(h.contains("Assumptions") && h.contains("name=\"pellet_price\"") && h.contains("name=\"litter_mean\""));
    // no simple/advanced split any more
    assert!(!h.contains(">Advanced<") && !h.contains("Find maximum"));
    // results: a plan and stats
    assert!(h.contains("class=\"big\"") && h.contains("dressed meat"));
    assert!(kpi_value(&h, "Dressed meat") > 185.0);
}

#[tokio::test]
async fn static_assets_are_served() {
    let (s, js) = get("/htmx.min.js").await;
    assert_eq!(s, StatusCode::OK);
    assert!(js.contains("htmx"));
    assert!(get("/style.css").await.1.contains("--accent"));
    assert_eq!(get("/nope").await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn results_fragment_has_plan_kpis_charts_and_oob_updates() {
    let (s, h) = get(&format!("/results?{}", q(DEFAULT_CONS))).await;
    assert_eq!(s, StatusCode::OK);
    assert!(!h.contains("<html"), "fragment, not a page");
    for needle in ["Plan", "Dressed meat", "Rabbits on hand", "<svg", "id=\"cost-qol\"", "id=\"cost-max_headcount\""] {
        assert!(h.contains(needle), "missing {needle}");
    }
    assert!(h.contains("hx-swap-oob=\"true\""));
    assert!(h.contains("hx-get=\"/cost?drop=qol\""));
}

#[tokio::test]
async fn qol_constraint_changes_the_plan() {
    let lo = kpi_value(&get(&format!("/results?{}", q(DEFAULT_CONS))).await.1, "Dressed meat");
    let hi = kpi_value(
        &get(&format!("/results?{}", q("on_qol=1&op_qol=min&val_qol=1.8&on_max_headcount=1&op_max_headcount=max&val_max_headcount=0")))
            .await
            .1,
        "Dressed meat",
    );
    assert!(hi < lo * 0.8, "{hi} vs {lo}");
}

#[tokio::test]
async fn pinned_and_limited_inputs_show_in_the_plan() {
    let h = get(&format!(
        "/results?{}",
        q(&format!("{DEFAULT_CONS}&on_does=1&op_does=eq&val_does=3&on_wean_days=1&op_wean_days=min&val_wean_days=42"))
    ))
    .await
    .1;
    assert!(h.contains("🔒"), "pinned mark");
    assert!(h.contains("↕"), "limit mark");
    assert!(h.contains("<span class=\"pv\">3</span>"), "does pinned at 3");
}

#[tokio::test]
async fn output_constraint_is_met_and_unmet_ones_are_flagged() {
    let ok = get(&format!("/results?{}", q(&format!("{DEFAULT_CONS}&on_meat=1&op_meat=min&val_meat=200")))).await.1;
    assert!(!ok.contains("can&#39;t all be met") && !ok.contains("can't all be met"));
    assert!(kpi_value(&ok, "Dressed meat") >= 195.0);
    let bad = get(&format!("/results?{}", q(&format!("{DEFAULT_CONS}&on_meat=1&op_meat=min&val_meat=1000")))).await.1;
    assert!(bad.contains("These constraints can"), "violation banner");
    assert!(bad.contains("Dressed meat: wanted at least 1000 kg/yr"));
}

#[tokio::test]
async fn max_floor_space_objective_trades_against_a_meat_limit() {
    let hint = get(&format!("/results?{}", q(&format!("{DEFAULT_CONS}&objective=max_qol")))).await.1;
    assert!(hint.contains("only what fits"));
    let a = get(&format!("/results?{}", q(&format!("{DEFAULT_CONS}&objective=max_qol&on_meat=1&op_meat=min&val_meat=150")))).await.1;
    let b = get(&format!("/results?{}", q(&format!("{DEFAULT_CONS}&objective=max_qol&on_meat=1&op_meat=min&val_meat=280")))).await.1;
    assert!(a.contains("floor space") && !a.contains("only what fits"));
    // more meat demanded -> less floor space
    let pct = |h: &str| -> f64 {
        let i = h.find("class=\"big\">").expect("headline") + "class=\"big\">".len();
        let rest = &h[i..];
        if rest.starts_with("Code minimum") { 0.0 } else { rest.trim_start_matches('+').split('%').next().unwrap().parse().unwrap() }
    };
    assert!(pct(&a) > pct(&b) + 5.0, "{} vs {}", pct(&a), pct(&b));
}

#[tokio::test]
async fn add_constraint_anchors_on_the_current_plan() {
    let (s, h) = get(&format!("/app?action=add&add=does&{}", q(DEFAULT_CONS))).await;
    assert_eq!(s, StatusCode::OK);
    assert!(h.starts_with("<div class=\"wrap\" id=\"app\""));
    assert!(h.contains("name=\"on_does\"") && h.contains("name=\"val_does\""));
    assert!(!h.contains("<option value=\"does\">"), "already active: not offered again");
    // exactly the optimum, so the dressed meat is unchanged
    let before = kpi_value(&get(&format!("/results?{}", q(DEFAULT_CONS))).await.1, "Dressed meat");
    assert!((kpi_value(&h, "Dressed meat") - before).abs() < before * 0.02);
    // result constraints
    let h = get(&format!("/app?action=add&add=meat&{}", q(DEFAULT_CONS))).await.1;
    assert!(h.contains("name=\"on_meat\"") && h.contains("at least"));
    // empty / unknown selections change nothing
    let h = get(&format!("/app?action=add&add=&{}", q(DEFAULT_CONS))).await.1;
    assert!(!h.contains("name=\"on_does\""));
    let h = get(&format!("/app?action=add&add=bogus&{}", q(DEFAULT_CONS))).await.1;
    assert!(h.contains("name=\"on_qol\""));
}

#[tokio::test]
async fn remove_constraint_and_restore_defaults() {
    let (_, h) = get(&format!("/app?action=remove&key=qol&{}", q(DEFAULT_CONS))).await;
    assert!(!h.contains("name=\"on_qol\"") && h.contains("name=\"on_max_headcount\""));
    assert!(h.contains("<option value=\"qol\">"), "removed constraint can be added again");
    let (_, h) = get(&format!("/app?action=remove&key=qol&{}", q("on_max_headcount=1&op_max_headcount=max&val_max_headcount=0"))).await;
    assert!(!h.contains("name=\"on_qol\""));
    // removing everything leaves a valid empty list, not the defaults
    let (_, h) = get("/app?action=remove&key=qol&cons=1&runs=40&years=4&on_qol=1&op_qol=min&val_qol=1").await;
    assert!(h.contains("No constraints: everything is free"));
    let (_, h) = get(&format!("/app?action=defaults&{}", q("on_does=1&op_does=eq&val_does=2&pellet_price=3"))).await;
    assert!(h.contains("name=\"on_qol\"") && !h.contains("name=\"on_does\""));
    assert!(h.contains("value=\"1.5\""), "pellet price back to default");
}

#[tokio::test]
async fn cost_chip_reports_what_a_constraint_costs() {
    let binding = get(&format!("/cost?drop=does&{}", q(&format!("{DEFAULT_CONS}&on_does=1&op_does=eq&val_does=2")))).await.1;
    assert!(binding.contains("Costs") && binding.contains("kg/yr"), "{binding}");
    let slack = get(&format!("/cost?drop=does&{}", q(&format!("{DEFAULT_CONS}&on_does=1&op_does=max&val_does=24")))).await.1;
    assert!(slack.contains("Not limiting"), "{slack}");
    let qol_obj = get(&format!(
        "/cost?drop=does&{}",
        q(&format!("{DEFAULT_CONS}&objective=max_qol&on_meat=1&op_meat=min&val_meat=150&on_does=1&op_does=eq&val_does=1"))
    ))
    .await
    .1;
    assert!(qol_obj.contains("floor space") || qol_obj.contains("Conflicts") || qol_obj.contains("Not limiting"), "{qol_obj}");
    let conflict = get(&format!("/cost?drop=meat&{}", q(&format!("{DEFAULT_CONS}&on_meat=1&op_meat=min&val_meat=3000")))).await.1;
    assert!(conflict.contains("Conflicts"), "{conflict}");
    assert_eq!(get("/cost?cons=1").await.0, StatusCode::OK);
}

#[tokio::test]
async fn without_the_cons_marker_the_defaults_apply() {
    let h = get("/results?runs=40&years=4").await.1;
    assert!(h.contains("id=\"cost-qol\"") && h.contains("id=\"cost-max_headcount\""));
}

#[tokio::test]
async fn fixed_inputs_flow_through() {
    let a = kpi_value(&get(&format!("/results?{}", q(DEFAULT_CONS))).await.1, "Dressed meat");
    let b = kpi_value(&get(&format!("/results?{}", q(&format!("{DEFAULT_CONS}&width_m=4")))).await.1, "Dressed meat");
    assert!(b > a * 1.5, "{b} vs {a}");
    let cheap = kpi_value(&get(&format!("/results?{}", q(DEFAULT_CONS))).await.1, "Feed cost");
    let dear = kpi_value(&get(&format!("/results?{}", q(&format!("{DEFAULT_CONS}&pellet_price=3&hay_price=1.6")))).await.1, "Feed cost");
    assert!(dear > cheap * 1.7, "{dear} vs {cheap}");
}

#[tokio::test]
async fn garbage_input_never_500s() {
    for uri in [
        "/results?cons=1&on_does=1&val_does=abc&op_does=zzz&objective=bogus&width_m=NaN",
        "/results?cons=1&on_does=1&val_does=99999&op_does=min&runs=0",
        "/results?cons=1&on_meat=1&val_meat=-5&op_meat=max",
        "/results?cons=1&width_m=0.5&depth_m=0.4",
        "/results?cons=1&height_m=0.4",
        "/results?cons=1&runs=999999999&years=-4&on_does=1&op_does=eq&val_does=100&runs=30",
        "/app?action=wat",
        "/app?action=remove",
        "/app?action=add",
        "/cost?drop=nonsense",
    ] {
        assert_eq!(get(uri).await.0, StatusCode::OK, "{uri}");
    }
}

#[tokio::test]
async fn infeasible_space_shows_a_message() {
    let h = get(&format!("/results?{}", q(&format!("{DEFAULT_CONS}&width_m=0.5&depth_m=0.4")))).await.1;
    assert!(h.contains("No production possible"));
    assert!(!h.contains("Rabbits on hand"));
}

#[tokio::test]
async fn html_is_escaped() {
    let h = get(&format!("/results?{}", q("objective=%3Cscript%3Ealert(1)%3C/script%3E&add=%3Cscript%3E"))).await.1;
    assert!(!h.contains("<script>alert"));
    let h = get("/app?action=add&add=%3Cscript%3Ealert(1)%3C/script%3E&cons=1&runs=30").await.1;
    assert!(!h.contains("<script>alert"));
}

#[tokio::test]
async fn cost_chips_target_themselves_not_the_results_panel() {
    // chips live inside the form, which targets #results: they must override it
    let h = full().await;
    let i = h.find("id=\"cost-qol\"").expect("chip");
    let tag = &h[h.rfind("<p").unwrap_or(0).min(i)..i + 400];
    let chip_start = h[..i].rfind("<p").unwrap();
    let chip = &h[chip_start..i + 400];
    assert!(chip.contains("hx-target=\"this\""), "{tag}");
}

#[tokio::test]
async fn stats_are_grouped_by_theme_with_icons_and_no_run_ranges_on_screen() {
    let h = full().await;
    for theme in ["<h3>Meat</h3>", "<h3>Food</h3>", "<h3>Herd</h3>"] {
        assert!(h.contains(theme), "{theme}");
    }
    for icon in ["🥩", "🌾", "🌿", "🐇"] {
        assert!(h.contains(icon), "{icon}");
    }
    assert!(h.contains("Pellets") && h.contains("Hay"));
    // the 80% range lives only in the hover text, not as visible copy
    assert!(!h.contains("class=\"kpi-range\""));
    assert!(h.contains("80% of runs fall between"));
}

#[tokio::test]
async fn code_checks_are_a_badge_and_a_dialog_not_a_card() {
    let h = full().await;
    assert!(h.contains("id=\"code-badge\"") && h.contains("data-dialog=\"code-dialog\""));
    assert!(h.contains("<dialog id=\"code-dialog\"") && h.contains("Code of Practice checks"));
    assert!(h.contains("badge ok"), "default plan complies");
    assert!(!h.contains("<h2>Code of Practice checks</h2></section>"));
    // a failing check flips the badge and is listed in the dialog
    let bad =
        get(&format!("/results?{}", q(&format!("{DEFAULT_CONS}&cage_height_cm=45&width_m=0.9&does=1&on_does=1&op_does=eq&val_does=5"))))
            .await
            .1;
    assert!(bad.contains("badge bad") && bad.contains("Code issue"), "badge should show issues");
    assert!(bad.contains("id=\"code-badge\"") && bad.contains("hx-swap-oob=\"true\""), "refreshed out of band");
    assert!(bad.contains("<dialog id=\"code-dialog\""));
}

#[tokio::test]
async fn help_dialog_replaces_the_header_paragraph() {
    let h = get("/").await.1;
    assert!(h.contains("data-dialog=\"help-dialog\"") && h.contains("id=\"help-dialog\""));
    for needle in ["What this does", "Using it", "Floor space (quality of life)", "How the model works", "Reading the results"] {
        assert!(h.contains(needle), "{needle}");
    }
    assert!(!h.contains("Simulates a breeding herd in your space under the"), "long header paragraph is gone");
    assert!(!h.contains("How the model works</h2>"), "no longer a card at the bottom");
}

#[tokio::test]
async fn every_slider_has_a_number_box_that_the_browser_keeps_in_step() {
    let h = get("/").await.1;
    // fixed input: box, linked both ways, in the units the label uses
    assert!(h.contains("id=\"num-in-width_m\"") && h.contains("data-for=\"in-width_m\"") && h.contains("data-num=\"num-in-width_m\""));
    assert!(h.contains("<span class=\"unit\">m</span>"));
    // constraint sliders too
    assert!(h.contains("id=\"num-in-val_qol\"") && h.contains("data-for=\"in-val_qol\""));
    assert!(h.contains("<span class=\"pre\">×</span>"), "floor space is typed as a multiple of the Code minimum");
    // result constraint carries its unit
    let a = get(&format!("/app?action=add&add=meat&{}", q(DEFAULT_CONS))).await.1;
    assert!(a.contains("id=\"num-in-val_meat\"") && a.contains("<span class=\"unit\">kg/yr</span>"));
    // the box has no name: only the slider is submitted
    assert!(!h.contains("class=\"numbox\" type=\"number\" id=\"num-in-width_m\" name"));
    for needle in ["data-scale=", "data-dec="] {
        assert!(h.contains(needle), "{needle}");
    }
    // the script wires both directions and snaps on change
    assert!(h.contains("function sync(") && h.contains("classList.contains('numbox')") && h.contains("dispatchEvent(new Event('input'"));
    // typing into a box must not fire its own request on top of the slider's
    assert!(h.contains("target.type!='number'"));
}

#[tokio::test]
async fn percent_inputs_are_typed_as_percentages_and_week_inputs_show_weeks() {
    let h = get("/").await.1;
    // kit losses are stored as a fraction (0.15) but typed as 15 %
    let i = h.find("id=\"num-in-pre_wean_mortality\"").expect("kit loss box");
    let tag = &h[i..i + 260];
    assert!(tag.contains("value=\"15\"") && tag.contains("max=\"40\""), "{tag}");
    assert!(h.contains("data-scale=\"100\""));
    // weaning age in days with a weeks readout that the script keeps up to date
    let w = get(&format!("/app?action=add&add=wean_days&{}", q(DEFAULT_CONS))).await.1;
    assert!(w.contains("id=\"cap-in-val_wean_days\"") && w.contains(" wk"));
    // typed values go through the same parser as slider values: out of range is clamped
    let big = kpi_value(&get(&format!("/results?{}", q(&format!("{DEFAULT_CONS}&width_m=999")))).await.1, "Dressed meat");
    let six = kpi_value(&get(&format!("/results?{}", q(&format!("{DEFAULT_CONS}&width_m=6")))).await.1, "Dressed meat");
    assert_eq!(big, six);
}

#[tokio::test]
async fn constraints_are_compact_one_line_controls() {
    let h = get("/").await.1;
    assert!(h.contains("class=\"cons-ctl\""), "operator, slider and value share a line");
    assert!(h.contains("class=\"cons-label\""));
    assert!(h.contains("All cages: floor space"), "short names in the list");
    assert!(!h.contains("All cages — floor space vs code minimum</label>"));
}

#[tokio::test]
async fn switching_objective_suggests_a_meat_minimum() {
    let base = DEFAULT_CONS;
    // to max floor space: a meat floor (three quarters of the best plan) is added and shown
    let h = get(&format!("/app?action=objective&{}", q(&format!("{base}&objective=max_qol")))).await.1;
    assert!(h.starts_with("<div class=\"wrap\" id=\"app\""));
    assert!(h.contains("name=\"on_meat\""), "meat minimum added");
    assert!(h.contains("checked") && h.contains("value=\"max_qol\" checked"));
    assert!(!h.contains("only what fits"), "no dead-end hint");
    assert!(h.contains("id=\"code-badge\""), "badge refreshed with the swap");
    let floor = {
        let i = h.find("name=\"val_meat\"").unwrap();
        let rest = &h[i..];
        let j = rest.find("value=\"").unwrap() + 7;
        rest[j..].split('"').next().unwrap().parse::<f64>().unwrap()
    };
    let best = kpi_value(&get(&format!("/results?{}", q(base))).await.1, "Dressed meat");
    assert!((floor - best * 0.75).abs() <= best * 0.04 + 5.0, "floor {floor} vs 75% of {best}");
    // an existing meat limit is respected
    let h = get(&format!("/app?action=objective&{}", q(&format!("{base}&objective=max_qol&on_meat=1&op_meat=min&val_meat=123")))).await.1;
    assert!(h.contains("value=\"123\""));
    // back to most meat: nothing is added
    let h = get(&format!("/app?action=objective&{}", q(&format!("{base}&objective=max_meat")))).await.1;
    assert!(!h.contains("name=\"on_meat\""));
}

#[tokio::test]
async fn objective_radios_swap_the_whole_app_but_do_not_trigger_the_form() {
    let h = get("/").await.1;
    assert!(h.contains("hx-get=\"/app?action=objective\""));
    assert!(h.contains("target.name!='objective'"));
}

#[tokio::test]
async fn results_have_gaps_between_cards_via_layout_not_margins() {
    let css = get("/style.css").await.1;
    assert!(css.contains("main#results { display: flex; flex-direction: column; gap: 16px;"));
}

#[tokio::test]
async fn whole_app_actions_have_their_own_sync_scope() {
    // they live inside the form, which uses hx-sync="this:replace"; inheriting it would let a pending
    // slider update cancel an objective switch, a constraint add, or a removal
    let h = get("/").await.1;
    for needle in ["hx-get=\"/app?action=objective\"", "hx-get=\"/app?action=add\"", "hx-get=\"/app?action=remove&amp;key=qol\""] {
        let i = h.find(needle).unwrap_or_else(|| panic!("missing {needle}"));
        let end = h[i..].find('>').unwrap() + i;
        assert!(h[i..end].contains("hx-sync=\"this:replace\""), "{needle}");
    }
}

#[tokio::test]
async fn cost_is_an_assumption_and_a_statistic_not_an_objective_or_constraint() {
    let h = full().await;
    assert!(!h.contains("Minimise cost"));
    assert!(!h.contains("<option value=\"cost_per_kg\">") && !h.contains("<option value=\"feed_cost\">"));
    // prices are assumptions...
    assert!(h.contains("name=\"pellet_price\"") && h.contains("name=\"hay_price\""));
    // ...and the resulting costs are still reported
    assert!(h.contains("Feed cost") && h.contains("Cost per kg of meat"));
    // results the user can still limit
    for o in ["meat", "fryers", "peak_head"] {
        assert!(h.contains(&format!("<option value=\"{o}\">")), "{o}");
    }
}

#[tokio::test]
async fn dropdown_and_choice_inputs_do_not_repeat_their_value_as_text() {
    let h = get("/").await.1;
    // the reach dropdown sits on the label's line with no number box or value text
    assert!(h.contains("id=\"in-access_both_sides\""));
    assert!(!h.contains("id=\"num-in-access_both_sides\""));
    // sliders have a number box
    assert!(h.contains("id=\"num-in-width_m\""));
    // choice inputs under Assumptions too
    assert!(!h.contains("id=\"num-in-replacement\""));
}

#[tokio::test]
async fn cage_layout_has_a_table_beside_the_plan_and_no_snapshot_note() {
    let h = full().await;
    assert!(!h.contains("Cage contents are a snapshot"));
    assert!(h.contains("class=\"layout-body\"") && h.contains("class=\"layout-tables\""));
    // cage table
    for needle in [
        "<th>Cages</th>",
        "Doe + litter</td>",
        "Buck</td>",
        "Grower</td>",
        "Total</td>",
        "<th class=\"num\">No.</th>",
        "<th class=\"num\">Each</th>",
    ] {
        assert!(h.contains(needle), "{needle}");
    }
    // space table next to the tiers
    for needle in ["<th colspan=\"2\">Space</th>", "Tiers</td>", "that fit in 1.35 m", "Cage depth</td>", "Cage height</td>"] {
        assert!(h.contains(needle), "{needle}");
    }
    // the old run-on sentence is gone
    assert!(!h.contains("cages: ") || !h.contains("m²), 1 buck"));
}

#[tokio::test]
async fn stat_themes_are_horizontal_rows() {
    let h = full().await;
    assert_eq!(h.matches("class=\"theme-items\"").count(), 3);
    let css = get("/style.css").await.1;
    assert!(css.contains(".theme { display: flex;") && css.contains(".theme-items { display: flex; flex-wrap: wrap;"));
    // order is kept: Meat, Food, Herd
    let stats = &h[h.find("class=\"card stats\"").expect("stats card")..];
    let (m, f, hd) = (stats.find("<h3>Meat</h3>").unwrap(), stats.find("<h3>Food</h3>").unwrap(), stats.find("<h3>Herd</h3>").unwrap());
    assert!(m < f && f < hd);
}

#[tokio::test]
async fn tiers_stays_a_constraint_not_a_space_input() {
    let h = get("/").await.1;
    assert!(!h.contains("name=\"tiers\""), "no fixed tiers input");
    assert!(h.contains("<option value=\"tiers\">"), "offered in the constraint menu");
    let one = get(&format!("/results?{}", q(&format!("{DEFAULT_CONS}&on_tiers=1&op_tiers=eq&val_tiers=1")))).await.1;
    let two = get(&format!("/results?{}", q(DEFAULT_CONS))).await.1;
    assert!(kpi_value(&one, "Dressed meat") < kpi_value(&two, "Dressed meat") * 0.7, "a single tier holds far fewer rabbits");
    assert!(one.contains("1 of 2 that fit"));
}

#[tokio::test]
async fn dressed_weight_is_a_biology_assumption_that_changes_the_results() {
    let h = get("/").await.1;
    let bio = &h[h.find("<summary>Biology</summary>").expect("Biology group")..];
    let weight = bio.find("name=\"dressed_weight_kg\"").expect("adult weight input under Biology");
    assert!(weight < bio.find("name=\"litter_mean\"").unwrap(), "first in the group");
    assert!(h.contains("Average dressed weight") && h.contains("id=\"num-in-dressed_weight_kg\""));
    assert!(!h.contains("name=\"on_dressed_weight_kg\""), "an assumption, not a constraint");
    let light = kpi_value(&get(&format!("/results?{}", q(&format!("{DEFAULT_CONS}&dressed_weight_kg=1.2")))).await.1, "Dressed meat");
    let heavy = kpi_value(&get(&format!("/results?{}", q(&format!("{DEFAULT_CONS}&dressed_weight_kg=1.8")))).await.1, "Dressed meat");
    assert!(heavy > light * 1.25, "{heavy} vs {light}");
}

#[tokio::test]
async fn max_rabbits_is_a_ceiling_only_and_peak_rabbits_is_the_herd_minimum() {
    let h = get("/").await.1;
    // the cap row has a fixed "at most", no operator dropdown
    let i = h.find("data-key=\"max_headcount\"").expect("cap row");
    let row = &h[i..i + 900];
    assert!(row.contains("<span class=\"op-fixed\">at most</span>") && !row.contains("name=\"op_max_headcount\" aria-label"), "{row}");
    assert!(row.contains("type=\"hidden\" name=\"op_max_headcount\" value=\"max\""));
    // an old link asking for "at least" is treated as "at most"
    let cap = get(&format!("/results?{}", q("on_max_headcount=1&op_max_headcount=min&val_max_headcount=40"))).await.1;
    assert!(cap.contains("id=\"cost-max_headcount\""));
    // the herd-size result is renamed and is offered with all three operators
    assert!(h.contains("<option value=\"peak_head\">Rabbits on hand (peak)</option>"));
    let a = get(&format!("/app?action=add&add=peak_head&{}", q(DEFAULT_CONS))).await.1;
    assert!(a.contains("Result: Rabbits on hand (peak)") && a.contains("<option value=\"min\" selected>at least</option>"));
    assert!(a.contains("<option value=\"eq\">exactly</option>") && a.contains("<option value=\"max\">at most</option>"));
}

#[tokio::test]
async fn max_floor_space_hint_only_shows_when_nothing_can_limit_room() {
    let none = get(&format!("/results?{}", q(&format!("{DEFAULT_CONS}&objective=max_qol")))).await.1;
    assert!(none.contains("only what fits"));
    // a minimum herd limits room
    let herd =
        get(&format!("/results?{}", q(&format!("{DEFAULT_CONS}&objective=max_qol&on_peak_head=1&op_peak_head=min&val_peak_head=50"))))
            .await
            .1;
    assert!(!herd.contains("only what fits"));
    // a lever limit (at least 4 does) does too
    let does = get(&format!("/results?{}", q(&format!("{DEFAULT_CONS}&objective=max_qol&on_does=1&op_does=min&val_does=4")))).await.1;
    assert!(!does.contains("only what fits"));
    // but a cap on the herd, or a QoL floor, does not
    let cap = get(&format!(
        "/results?{}",
        q("objective=max_qol&on_qol=1&op_qol=min&val_qol=1.2&on_max_headcount=1&op_max_headcount=max&val_max_headcount=60")
    ))
    .await
    .1;
    assert!(cap.contains("only what fits"));
}

#[tokio::test]
async fn switching_to_max_floor_space_suggests_a_meat_and_a_herd_minimum() {
    let h = get(&format!("/app?action=objective&{}", q(&format!("{DEFAULT_CONS}&objective=max_qol")))).await.1;
    assert!(h.contains("name=\"on_meat\"") && h.contains("name=\"on_peak_head\""));
    assert!(!h.contains("only what fits"));
    // both are "at least"
    assert!(h.matches("<option value=\"min\" selected>at least</option>").count() >= 3, "qol, meat and peak rabbits");
    // existing limits are respected: only the missing one is added
    let h = get(&format!(
        "/app?action=objective&{}",
        q(&format!("{DEFAULT_CONS}&objective=max_qol&on_peak_head=1&op_peak_head=max&val_peak_head=77"))
    ))
    .await
    .1;
    assert!(h.contains("name=\"on_meat\"") && h.contains("value=\"77\""));
    // going back to most meat adds nothing
    let h = get(&format!("/app?action=objective&{}", q(&format!("{DEFAULT_CONS}&objective=max_meat")))).await.1;
    assert!(!h.contains("name=\"on_meat\"") && !h.contains("name=\"on_peak_head\""));
}

#[tokio::test]
async fn litters_per_doe_is_not_a_headline_stat() {
    let h = full().await;
    assert!(!h.contains("Litters per doe") && !h.contains("🍼"));
    assert!(h.contains("Cages in use") && h.contains("Peak rabbits on hand"));
}

#[tokio::test]
async fn the_page_shell_is_instant_and_loads_the_plan_straight_after() {
    let t = std::time::Instant::now();
    let h = get("/").await.1;
    assert!(t.elapsed() < std::time::Duration::from_millis(300), "shell took {:?}: it must not wait for a calculation", t.elapsed());
    // a placeholder, not a plan
    assert!(h.contains("Planning your herd…") && h.contains("class=\"card pending\""));
    assert!(!h.contains("class=\"big\"") && !h.contains("data-tip=\"Dressed meat:"), "no results in the shell");
    // the browser fetches the plan on load, sending the form
    let i = h.find("<main").expect("main");
    let tag = &h[i..h[i..].find('>').unwrap() + i];
    assert!(tag.contains("hx-get=\"/results\"") && tag.contains("hx-include=\"#form\"") && tag.contains("hx-trigger=\"load\""), "{tag}");
    // the form is fully rendered from the request alone
    assert!(h.contains("name=\"width_m\"") && h.contains("name=\"on_qol\""));
    // cost chips don't fire on the shell (that would solve the same problem several times at once)
    assert_eq!(h.matches("hx-trigger=\"load\"").count(), 1, "only the plan loads on page load");
    assert!(h.contains("waiting for the plan…"));
}

#[tokio::test]
async fn cost_chips_arm_themselves_once_the_plan_is_back() {
    let frag = get(&format!("/results?{}", q(DEFAULT_CONS))).await.1;
    for key in ["qol", "max_headcount"] {
        let i = frag.find(&format!("id=\"cost-{key}\"")).expect("chip");
        let start = frag[..i].rfind("<p").unwrap();
        let end = frag[i..].find('>').unwrap() + i;
        let tag = &frag[start..end];
        assert!(tag.contains("hx-trigger=\"load\"") && tag.contains("hx-swap-oob=\"true\"") && tag.contains("hx-target=\"this\""), "{tag}");
    }
    let app = get(&format!("/app?action=remove&key=nothing&{}", q(DEFAULT_CONS))).await.1;
    assert!(app.contains("working out what this costs…"), "whole-app swaps arm chips too");
}

#[tokio::test]
async fn page_has_progress_and_error_plumbing() {
    let h = get("/").await.1;
    // top progress bar, status pill with both messages, hidden error banner with Retry
    for needle in [
        "id=\"topbar\"",
        "id=\"status\"",
        "Re-planning your herd…",
        "Working out what each constraint costs…",
        "id=\"status-error\" role=\"alert\" hidden",
        "data-retry",
    ] {
        assert!(h.contains(needle), "{needle}");
    }
    // the script counts requests in flight (main vs cost), keeps stale chips, and surfaces failures
    for needle in [
        "htmx:beforeRequest",
        "loadend",
        "classList.toggle('calculating'",
        "classList.toggle('costing'",
        "htmx:oobBeforeSwap",
        "firstElementChild",
        "htmx:sendError",
        "htmx:responseError",
        "lastFailed",
        "htmx.ajax('GET'",
        "htmx.config.timeout",
    ] {
        assert!(h.contains(needle), "{needle}");
    }
    let css = get("/style.css").await.1;
    for needle in ["#topbar", "html.calculating #status", "html.costing:not(.calculating)", ".cost.stale", "#status-error", ".pending"] {
        assert!(css.contains(needle), "{needle}");
    }
}

// ---------------------------------------------------------------- tooltips, inputs, sidebar

#[tokio::test]
async fn native_titles_are_replaced_by_js_tooltips() {
    let h = full().await;
    // content that used to be native `title`s now uses data-tip
    for needle in [
        "data-tip=\"Dressed meat: ",
        "data-tip=\"Remove this constraint\"",
        "data-tip=\"Code of Practice checks",
        "class=\"hit\" data-tip=\"Week 1",
        "<g data-tip=\"",
    ] {
        assert!(h.contains(needle), "{needle}");
    }
    let body = &h[h.find("</head>").unwrap()..];
    for old in ["<title>", "div class=\"stat\" title=", "title=\"Remove this constraint\"", "span class=\"pin\" title="] {
        assert!(!body.contains(old), "native tooltip left behind: {old}");
    }
    // the tooltip machinery: delegated hover/focus, keyboard dismiss, hidden on swaps and scroll
    for needle in [
        "id='tooltip'",
        "document.addEventListener('mouseover'",
        "document.addEventListener('focusin'",
        "key==='Escape'",
        "htmx:beforeSwap",
        "addEventListener('scroll',hideTip",
    ] {
        assert!(h.contains(needle), "{needle}");
    }
    let css = get("/style.css").await.1;
    assert!(css.contains("#tooltip {") && css.contains("white-space: pre-line"));
}

#[tokio::test]
async fn number_boxes_select_on_entry_and_are_sized_for_their_values() {
    let h = get("/").await.1;
    assert!(h.contains("t.select()") && h.contains("e.preventDefault();t._sel=false"), "select-all on focus survives the mouse-up");
    // widest expected value sets the width: peak rabbits to 300, runs to 5000, width to 6.00
    let n = |id: &str| -> usize {
        let i = h.find(&format!("id=\"{id}\"")).unwrap_or_else(|| panic!("no {id}"));
        let tag = &h[i..i + 400];
        let j = tag.find("--n:").unwrap() + 4;
        tag[j..].split(|c: char| !c.is_ascii_digit()).next().unwrap().parse().unwrap()
    };
    assert_eq!(n("num-in-width_m"), 4, "0.50 to 6.00");
    assert_eq!(n("num-in-runs"), 4, "10 to 5000");
    assert_eq!(n("num-in-val_qol"), 4, "1.00 to 3.00");
    let a = get(&format!("/app?action=add&add=peak_head&{}", q(DEFAULT_CONS))).await.1;
    assert_eq!(n_from(&a, "num-in-val_peak_head"), 3, "0 to 300: three figures");
    let css = get("/style.css").await.1;
    assert!(css.contains("field-sizing: content") && css.contains("width: calc(var(--n, 3) * 1ch"));
}
fn n_from(h: &str, id: &str) -> usize {
    let i = h.find(&format!("id=\"{id}\"")).unwrap();
    let tag = &h[i..i + 400];
    let j = tag.find("--n:").unwrap() + 4;
    tag[j..].split(|c: char| !c.is_ascii_digit()).next().unwrap().parse().unwrap()
}

#[tokio::test]
async fn sidebar_section_headers_have_padding_and_their_own_chevron() {
    let css = get("/style.css").await.1;
    assert!(css.contains("aside { padding: 14px 18px; }"));
    assert!(css.contains("aside summary { list-style: none;") && css.contains("aside summary::before { content: \"▸\""));
    assert!(css.contains("aside summary::-webkit-details-marker { display: none; }"));
}

// ---------------------------------------------------------------- presets

async fn post_form(uri: &str, text: &str) -> (StatusCode, String) {
    let body = serde_urlencoded::to_string([("text", text)]).unwrap();
    let res = rabbitsim::web::router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    (status, String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap())
}

#[tokio::test]
async fn preset_dropdown_lists_the_files_on_disk() {
    let h = get("/").await.1;
    assert!(h.contains("id=\"preset-select\"") && h.contains("Load a preset…"));
    for name in [
        "Backyard starter",
        "Comfortable cages (+50%)",
        "Welfare first (150 kg/yr)",
        "Single-tier wall run",
        "Large shed (herd capped at 120)",
    ] {
        assert!(h.contains(name), "{name}");
    }
    assert!(h.contains("<option value=\"backyard-starter\"") && h.contains("hx-get=\"/app?action=preset\""));
    // export / import controls
    for needle in ["data-export", "data-import", "id=\"preset-file\"", "type=\"file\"", "/preset/export?", "/preset/load"] {
        assert!(h.contains(needle), "{needle}");
    }
    // the preset bar is outside the form, so it never joins its requests
    let form_start = h.find("<form id=\"form\"").unwrap();
    assert!(h.find("id=\"preset-select\"").unwrap() < form_start);
}

#[tokio::test]
async fn presets_are_read_from_disk_on_every_request() {
    let dir = std::env::temp_dir().join(format!("rabbit-web-presets-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("first.txt"), "name = First one\nwidth_m = 3\n").unwrap();
    let get_from = |uri: String| {
        let dir = dir.clone();
        async move {
            let res = rabbitsim::web::router_with(dir).oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap()).await.unwrap();
            String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap()
        }
    };
    let h = get_from("/".into()).await;
    assert!(h.contains("First one") && !h.contains("Backyard starter"), "only this directory's files");
    std::fs::write(dir.join("second.txt"), "name = Second one\n").unwrap();
    assert!(get_from("/".into()).await.contains("Second one"), "a new file appears without restarting");
    let loaded = get_from(format!("/app?action=preset&preset=first&{}", q(DEFAULT_CONS))).await;
    assert!(
        loaded.contains("Loaded “First one”.")
            && loaded.contains("id=\"in-width_m\" name=\"width_m\" min=\"0.5\" max=\"6\" step=\"0.05\" value=\"3\"")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn choosing_a_preset_replaces_the_whole_setup() {
    let h = get(&format!("/app?action=preset&preset=welfare-first&{}", q("on_does=1&op_does=eq&val_does=2&pellet_price=3"))).await.1;
    assert!(h.starts_with("<div class=\"wrap\" id=\"app\""));
    assert!(h.contains("Loaded “Welfare first (150 kg/yr)”."));
    assert!(h.contains("value=\"max_qol\" checked"), "objective from the file");
    assert!(h.contains("name=\"on_meat\"") && h.contains("name=\"on_peak_head\""), "constraints from the file");
    assert!(!h.contains("name=\"on_does\""), "what was there before is gone");
    assert!(h.contains("id=\"preset-select\""), "the preset bar survives the swap");
    // a wall run with one tier reaches only from one side
    let w = get("/app?action=preset&preset=single-tier-wall-run").await.1;
    assert!(w.contains("No preset called"), "ids are file names, not display names");
    let w = get("/app?action=preset&preset=single-tier-shelf").await.1;
    assert!(w.contains("name=\"on_tiers\"") && w.contains("<option value=\"false\" selected>no</option>"));
    assert!(w.contains("1 of 1 that fit"), "{}", &w[w.find("Tiers</td>").unwrap_or(0)..][..120.min(w.len())]);
}

#[tokio::test]
async fn preset_ids_cannot_escape_the_directory() {
    for bad in ["../Cargo", "..%2FCargo", "%2e%2e%2fCargo", "/etc/passwd", "a/b", "backyard-starter.txt", "x".repeat(200).as_str()] {
        let (s, h) = get(&format!("/app?action=preset&preset={bad}&{}", q(DEFAULT_CONS))).await;
        assert_eq!(s, StatusCode::OK, "{bad}");
        assert!(h.contains("not a valid preset name") || h.contains("No preset called"), "{bad}");
        assert!(!h.contains("[package]"), "file contents must never be echoed");
    }
    // choosing the placeholder changes nothing
    let h = get(&format!("/app?action=preset&preset=&{}", q(DEFAULT_CONS))).await.1;
    assert!(!h.contains("Loaded"));
}

#[tokio::test]
async fn export_downloads_a_text_file_of_the_current_setup() {
    let res = rabbitsim::web::router()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/preset/export?{}",
                    q(&format!(
                        "{DEFAULT_CONS}&objective=max_qol&width_m=3.5&on_does=1&op_does=min&val_does=4&on_meat=1&op_meat=min&val_meat=180"
                    ))
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let h = res.headers();
    assert!(h["content-type"].to_str().unwrap().starts_with("text/plain"));
    assert!(h["content-disposition"].to_str().unwrap().starts_with("attachment; filename=\"rabbit-plan"));
    let body = String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    for line in [
        "objective = max_qol",
        "width_m = 3.5",
        "constraint.does = min 4",
        "constraint.meat = min 180",
        "constraint.qol = min 1",
        "pellet_price = 1.5",
    ] {
        assert!(body.lines().any(|l| l == line), "missing line {line:?} in:\n{body}");
    }
    assert!(body.lines().all(|l| l.is_empty() || l.starts_with('#') || l.contains('=')), "plain key = value text");
}

#[tokio::test]
async fn an_exported_file_imports_back_to_the_same_setup() {
    let uri = format!(
        "/preset/export?{}",
        q(&format!(
            "{DEFAULT_CONS}&objective=max_qol&width_m=3.5&pellet_price=2.1&on_does=1&op_does=min&val_does=4&on_meat=1&op_meat=min&val_meat=180"
        ))
    );
    let exported = get(&uri).await.1;
    let (s, h) = post_form("/preset/load", &exported).await;
    assert_eq!(s, StatusCode::OK);
    assert!(h.starts_with("<div class=\"wrap\" id=\"app\"") && h.contains("Loaded “My plan”."));
    assert!(!h.contains("Ignored:"), "nothing in our own export may be ignored");
    assert!(h.contains("value=\"max_qol\" checked"));
    assert!(
        h.contains("name=\"on_does\"")
            && h.contains("name=\"on_meat\"")
            && h.contains("name=\"val_meat\" min=\"0\" max=\"1000\" step=\"5\" value=\"180\"")
    );
    assert!(h.contains("id=\"in-pellet_price\" name=\"pellet_price\" min=\"0.3\" max=\"4\" step=\"0.05\" value=\"2.1\""));
    assert!(h.contains("value=\"3.5\""));
}

#[tokio::test]
async fn importing_bad_files_explains_what_was_ignored() {
    let (s, h) =
        post_form("/preset/load", "width_m = 3\nbogus = 1\nobjective = fastest\nthis line is junk\nconstraint.does = sideways 3\n").await;
    assert_eq!(s, StatusCode::OK);
    assert!(h.contains("Loaded “imported preset”."));
    for needle in ["Ignored: line 2: unknown setting “bogus”", "unknown objective", "expected “key = value”", "unknown operator"] {
        assert!(h.contains(needle), "{needle}");
    }
    assert!(h.contains("value=\"3\""), "the good line was still applied");
    // empty / whitespace / oversized
    assert!(post_form("/preset/load", "   \n").await.1.contains("That file was empty"));
    let big = format!("width_m = 4\n{}", "# x\n".repeat(40_000));
    let (s, h) = post_form("/preset/load", &big).await;
    assert_eq!(s, StatusCode::OK);
    assert!(h.contains("larger than 64 KB"));
    // not even form data
    let res =
        rabbitsim::web::router().oneshot(Request::builder().method("POST").uri("/preset/load").body(Body::empty()).unwrap()).await.unwrap();
    assert!(res.status().is_client_error(), "a body-less POST is rejected cleanly, not a 500");
}

#[tokio::test]
async fn every_shipped_preset_loads_cleanly_and_has_a_workable_plan() {
    use rabbitsim::engine::{OptConfig, solve};
    use rabbitsim::web::presets;
    let dir = rabbitsim::web::default_presets_dir();
    let all = presets::list(&dir);
    assert!(all.len() >= 5, "{all:?}");
    for info in all {
        let p = presets::load(&dir, &info.id).unwrap();
        assert!(p.warnings.is_empty(), "{}: {:?}", info.id, p.warnings);
        assert!(!info.name.is_empty() && !info.description.is_empty(), "{} needs a name and description", info.id);
        let mut problem = p.request.problem();
        problem.base.runs = 24;
        problem.base.years = 3;
        let sol = solve(&problem, OptConfig { eval_runs: 16, eval_years: 3, rounds: 2 });
        assert!(sol.feasible(), "{}: no workable plan: {:?}", info.id, sol.violations);
    }
}
