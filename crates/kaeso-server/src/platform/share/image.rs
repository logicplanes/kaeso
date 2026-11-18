use time::Date;

use super::data::SharedRow;

const WIDTH: f32 = 1200.0;
const HEIGHT: f32 = 630.0;
// Kaeso's cover colours: the paper, the three oranges and reds, and the ink of the wordmark.
const PAPER: &str = "#f1ece2";
const BARS: [&str; 3] = ["#fb8b24", "#e36414", "#9a031e"];
const INK: &str = "#141414";
const WORDMARK: [&str; 5] = [
    "m72 71.5h34.5v76.1l86-76.1h47.1l-89.6 81.9 90.5 91.8h-46.3l-70.8-70.3-16.9 15.7v54.6h-34.5z",
    "m330.5 71.5h31.6l80.2 173.7h-38.1l-20.5-43.2h-79.6l-19.5 43.2h-34.5zm14.6 42.6-26.7 59.2h53.6z",
    "m457.7 71.5h136.4v30.6h-101.4v37.8h87.4v31h-87.4v43.1h102.3v31.2h-137.3z",
    "m748.1 79.3v31.8c-20.5-7.1-41.8-11-65.4-11-21.7 0-34.7 6.2-34.7 19.8 0 11.5 9.8 16.8 26.4 18.9l21.3 1.7c40.7 2.5 63.3 18.2 63.3 52.2 0 36.1-26.1 54.3-74.8 54.3-24 0-46.4-3.6-66.4-11.7v-32.3c21.3 8.5 40.9 12.9 65.4 12.9 25.9 0 39.9-6.7 39.9-20.3 0-13.3-10.4-19.1-30.2-20.6l-21.4-1.4c-40.2-2.6-60.5-19.1-60.5-50.2 0-33.7 26.8-54.1 69.8-54.1 27.1 0 48.4 3 67.3 10z",
    "m862.8 69.7c52.3 0 91.1 37.1 91.1 88.8 0 52.4-37.2 88.3-90.9 88.3-53.9 0-92-36.4-92-88 0-51.1 39.4-89.1 91.8-89.1zm0 33.3c-32.7 0-55.3 23.4-55.3 55.8 0 32.5 22.5 55 55.5 55 32.9 0 55-22.8 55-55s-22.7-55.8-55.2-55.8z",
];

/// The metric of each day of the period, days without usage as 0.
pub(super) fn series(days: &[SharedRow], (from, to): (Date, Date), metric: &str) -> Vec<f64> {
    let mut out = Vec::new();
    let mut day = from;
    while day <= to {
        let key = day.to_string();
        let value = days
            .iter()
            .find(|r| r.key.as_deref() == Some(key.as_str()))
            .map_or(0.0, |r| of(r, metric));
        out.push(value);
        match day.next_day() {
            Some(next) => day = next,
            None => break,
        }
    }
    out
}

#[expect(clippy::cast_precision_loss, reason = "a chart's bar heights")]
fn of(r: &SharedRow, metric: &str) -> f64 {
    let input = r.input_tokens + r.cache_read_tokens + r.cache_write_tokens;
    match metric {
        "tokens" => (input + r.output_tokens) as f64,
        "cost" => r
            .cost
            .as_deref()
            .and_then(|c| c.parse().ok())
            .unwrap_or(0.0),
        "errors" => r.errors as f64,
        "tools" => r.tool_calls as f64,
        "cache" if input > 0 => r.cache_read_tokens as f64 / input as f64,
        "cache" => 0.0,
        _ => r.requests as f64,
    }
}

fn svg(values: &[f64]) -> String {
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {WIDTH} {HEIGHT}\"><rect width=\"{WIDTH}\" height=\"{HEIGHT}\" fill=\"{PAPER}\"/>"
    );
    let highest = values.iter().copied().fold(0.0_f64, f64::max);
    if highest > 0.0 {
        let (left, right, top, bottom) = (80.0_f32, 1120.0_f32, 80.0_f32, 440.0_f32);
        #[expect(clippy::cast_precision_loss, reason = "at most 400 bars")]
        let slot = (right - left) / values.len() as f32;
        let gap = (slot * 0.2).min(8.0);
        for (i, v) in values.iter().enumerate() {
            #[expect(clippy::cast_possible_truncation, reason = "a bar's height in pixels")]
            let h = ((v / highest) as f32 * (bottom - top)).max(if *v > 0.0 { 4.0 } else { 0.0 });
            #[expect(clippy::cast_precision_loss, reason = "at most 400 bars")]
            let x = left + slot * i as f32 + gap / 2.0;
            let colour = BARS
                .get((i * BARS.len()) / values.len().max(1))
                .unwrap_or(&BARS[0]);
            out.push_str(&format!(
                "<rect x=\"{x:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{h:.1}\" fill=\"{colour}\"/>",
                bottom - h,
                (slot - gap).max(1.0)
            ));
        }
    } else {
        // Without numbers, the cover's three squares.
        for (i, (size, colour)) in [(300.0_f32, BARS[0]), (240.0, BARS[2]), (180.0, BARS[1])]
            .into_iter()
            .enumerate()
        {
            #[expect(clippy::cast_precision_loss, reason = "three squares")]
            let shift = 30.0 * i as f32;
            out.push_str(&format!(
                "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{size}\" height=\"{size}\" fill=\"{colour}\"/>",
                750.0 + shift,
                90.0 + shift * 1.5
            ));
        }
    }
    out.push_str(&format!(
        "<g fill=\"{INK}\" transform=\"translate(80 520) scale(0.24) translate(-39.05 -39.67)\">"
    ));
    for d in WORDMARK {
        out.push_str(&format!("<path d=\"{d}\"/>"));
    }
    out.push_str("</g></svg>");
    out
}

/// The preview as a PNG.
pub(super) fn render(values: &[f64]) -> Result<Vec<u8>, String> {
    let tree = resvg::usvg::Tree::from_str(&svg(values), &resvg::usvg::Options::default())
        .map_err(|e| e.to_string())?;
    let size = tree.size().to_int_size();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height())
        .ok_or_else(|| "the preview has no size".to_owned())?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::default(),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use time::macros::date;

    use super::*;

    fn day(key: &str, requests: u64) -> SharedRow {
        SharedRow {
            key: Some(key.into()),
            requests,
            input_tokens: 30,
            cache_read_tokens: 10,
            output_tokens: 5,
            cost: Some("0.5".into()),
            ..SharedRow::default()
        }
    }

    #[test]
    fn every_day_of_the_period_gets_a_value() {
        let days = [day("2026-11-02", 4), day("2026-11-04", 6)];
        let window = (date!(2026 - 11 - 01), date!(2026 - 11 - 04));
        assert_eq!(series(&days, window, "requests"), [0.0, 4.0, 0.0, 6.0]);
        assert_eq!(series(&days, window, "tokens"), [0.0, 45.0, 0.0, 45.0]);
        assert_eq!(series(&days, window, "cost"), [0.0, 0.5, 0.0, 0.5]);
        assert_eq!(series(&days, window, "cache"), [0.0, 0.25, 0.0, 0.25]);
    }

    #[test]
    fn the_preview_is_a_png_of_1200_by_630() {
        for values in [vec![1.0, 0.0, 3.0], Vec::new()] {
            let png = render(&values).unwrap();
            assert_eq!(&png[1..4], b"PNG");
            let width = u32::from_be_bytes(png[16..20].try_into().unwrap());
            let height = u32::from_be_bytes(png[20..24].try_into().unwrap());
            assert_eq!((width, height), (1200, 630));
        }
    }
}
