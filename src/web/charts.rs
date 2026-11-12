//! Hand-rolled server-side SVG charts (hover a week for its numbers via native tooltips).

use maud::{Markup, PreEscaped};

pub fn fmt_num(v: f64, dp: usize) -> String {
    let s = format!("{:.*}", dp, v.abs());
    let (int, frac) = s.split_once('.').map_or((s.as_str(), None), |(a, b)| (a, Some(b)));
    let mut out = String::new();
    for (i, ch) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if let Some(f) = frac {
        out.push('.');
        out.push_str(f);
    }
    if v < 0.0 && s.chars().any(|c| c.is_ascii_digit() && c != '0') { format!("-{out}") } else { out }
}

fn nice_step(raw: f64) -> f64 {
    if raw <= 0.0 {
        return 1.0;
    }
    let p = 10f64.powf(raw.log10().floor());
    let f = raw / p;
    (if f <= 1.0 {
        1.0
    } else if f <= 2.0 {
        2.0
    } else if f <= 2.5 {
        2.5
    } else if f <= 5.0 {
        5.0
    } else {
        10.0
    }) * p
}

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChartKind {
    Line,
    Bar,
}

pub struct ChartSeries<'a> {
    pub name: &'a str,
    /// CSS class suffix: 1, 2 or 3 pick the colour.
    pub colour: u8,
    pub mean: &'a [f64],
    pub band: Option<(&'a [f64], &'a [f64])>,
}

pub struct ChartSpec<'a> {
    pub series: Vec<ChartSeries<'a>>,
    pub kind: ChartKind,
    pub dp: usize,
    pub warm_weeks: usize,
    pub capacity: Option<(f64, String)>,
}

#[allow(clippy::needless_range_loop)] // band edges walk two series by index
pub fn chart(spec: &ChartSpec) -> Markup {
    let n = spec.series.first().map_or(0, |s| s.mean.len());
    if n == 0 {
        return PreEscaped(String::new());
    }
    let (w, h) = (640.0f64, 220.0f64);
    let (ml, mr, mt, mb) = (44.0, 12.0, 12.0, 28.0);
    let mut ymax = 0.0f64;
    for s in &spec.series {
        for i in 0..n {
            ymax = ymax.max(s.band.map_or(s.mean[i], |b| b.1[i]));
        }
    }
    if let Some((c, _)) = &spec.capacity {
        ymax = ymax.max(*c);
    }
    let step = nice_step(ymax / 4.0);
    ymax = step.max((ymax / step).ceil() * step);
    let x = |i: f64| ml + (w - ml - mr) * (i + 0.5) / n as f64;
    let y = |v: f64| mt + (h - mt - mb) * (1.0 - v / ymax);
    let mut g = String::new();
    let warm_w = x(spec.warm_weeks as f64) - x(0.0);
    g += &format!(
        r#"<rect x="{ml}" y="{mt}" width="{warm_w:.1}" height="{}" class="warm"/><text x="{}" y="{}" class="ax">start-up</text>"#,
        h - mt - mb,
        ml + 4.0,
        mt + 12.0
    );
    let mut v = 0.0;
    while v <= ymax + 1e-9 {
        g += &format!(
            r#"<line x1="{ml}" x2="{}" y1="{:.1}" y2="{:.1}" class="grid"/><text x="{}" y="{:.1}" text-anchor="end" class="ax">{}</text>"#,
            w - mr,
            y(v),
            y(v),
            ml - 6.0,
            y(v) + 4.0,
            fmt_num(v, 0)
        );
        v += step;
    }
    let years = (n as f64 / 52.14).floor() as usize;
    for yr in 0..=years {
        let i = (((yr as f64) * 52.14).round() as usize).min(n - 1);
        g += &format!(
            r#"<text x="{:.1}" y="{}" text-anchor="middle" class="ax">{}</text>"#,
            x(i as f64),
            h - 8.0,
            if yr == 0 { "week 0".into() } else { format!("yr {yr}") }
        );
    }
    if let Some((c, label)) = &spec.capacity {
        g += &format!(
            r#"<line x1="{ml}" x2="{}" y1="{:.1}" y2="{:.1}" class="cap"/><text x="{}" y="{:.1}" text-anchor="end" class="ax">{}</text>"#,
            w - mr,
            y(*c),
            y(*c),
            w - mr,
            y(*c) - 4.0,
            esc(label)
        );
    }
    for s in &spec.series {
        match spec.kind {
            ChartKind::Bar => {
                let bw = ((w - ml - mr) / n as f64 - 1.0).max(1.0);
                for i in 0..n {
                    g += &format!(
                        r#"<rect x="{:.2}" y="{:.2}" width="{bw:.2}" height="{:.2}" class="bar" rx="{:.1}"/>"#,
                        x(i as f64) - bw / 2.0,
                        y(s.mean[i]),
                        (y(0.0) - y(s.mean[i])).max(0.0),
                        (bw / 2.0).min(2.0)
                    );
                }
            }
            ChartKind::Line => {
                if let Some((lo, hi)) = s.band {
                    let mut d = format!("M{:.1},{:.1}", x(0.0), y(hi[0]));
                    for i in 1..n {
                        d += &format!("L{:.1},{:.1}", x(i as f64), y(hi[i]))
                    }
                    for i in (0..n).rev() {
                        d += &format!("L{:.1},{:.1}", x(i as f64), y(lo[i]))
                    }
                    g += &format!(r#"<path d="{d}Z" class="band{}"/>"#, s.colour);
                }
                let mut d = String::new();
                for i in 0..n {
                    d += &format!("{}{:.1},{:.1}", if i == 0 { "M" } else { "L" }, x(i as f64), y(s.mean[i]))
                }
                g += &format!(r#"<path d="{d}" fill="none" stroke-width="2" stroke-linejoin="round" class="line{}"/>"#, s.colour);
            }
        }
    }
    // hover targets: one column per week with a native tooltip
    let cw = (w - ml - mr) / n as f64;
    for i in 0..n {
        let mut tip = format!("Week {}", i + 1);
        for s in &spec.series {
            tip += &format!("\n{}: {}", s.name, fmt_num(s.mean[i], spec.dp));
            if let Some((lo, hi)) = s.band {
                tip += &format!(" ({}–{})", fmt_num(lo[i], spec.dp), fmt_num(hi[i], spec.dp));
            }
        }
        g += &format!(
            r#"<rect x="{:.2}" y="{mt}" width="{cw:.2}" height="{}" class="hit" data-tip="{}"></rect>"#,
            x(i as f64) - cw / 2.0,
            h - mt - mb,
            esc(&tip)
        );
    }
    PreEscaped(format!(r#"<svg viewBox="0 0 {w} {h}" role="img">{g}</svg>"#))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_formatting() {
        assert_eq!(fmt_num(1234567.891, 1), "1,234,567.9");
        assert_eq!(fmt_num(12.0, 0), "12");
        assert_eq!(fmt_num(-1234.6, 0), "-1,235");
        assert_eq!(fmt_num(-0.0001, 1), "0.0");
    }

    #[test]
    fn nice_steps() {
        assert_eq!(nice_step(0.0), 1.0);
        assert_eq!(nice_step(3.0), 5.0);
        assert_eq!(nice_step(70.0), 100.0);
    }

    #[test]
    fn chart_renders_every_week_and_escapes() {
        let mean = vec![1.0, 2.0, 3.0];
        let s = chart(&ChartSpec {
            series: vec![ChartSeries { name: "a<b", colour: 1, mean: &mean, band: None }],
            kind: ChartKind::Bar,
            dp: 1,
            warm_weeks: 1,
            capacity: None,
        })
        .into_string();
        assert_eq!(s.matches(r#"class="hit""#).count(), 3);
        assert!(s.contains("a&lt;b"));
        assert!(
            chart(&ChartSpec {
                series: vec![ChartSeries { name: "x", colour: 1, mean: &[], band: None }],
                kind: ChartKind::Line,
                dp: 0,
                warm_weeks: 0,
                capacity: None
            })
            .into_string()
            .is_empty()
        );
    }
}
