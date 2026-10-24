use serde::Serialize;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub(crate) struct Summary {
    pub(crate) count: usize,
    pub(crate) p50: Option<f64>,
    pub(crate) p99: Option<f64>,
    pub(crate) p999: Option<f64>,
    pub(crate) max: Option<f64>,
}

impl Summary {
    pub(crate) fn of(values: &[f64]) -> Summary {
        let mut sorted: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
        sorted.sort_by(f64::total_cmp);
        Summary {
            count: sorted.len(),
            p50: percentile(&sorted, 50.0),
            p99: percentile(&sorted, 99.0),
            p999: percentile(&sorted, 99.9),
            max: sorted.last().copied(),
        }
    }
}

// Nearest rank: the smallest value that at least p percent of the values are less than or
// equal to, so every reported percentile is a value that was measured.
pub(crate) fn percentile(sorted: &[f64], p: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    // 99.9% of 1000 is 999.0000000000001 in floating point; the rank is 999.
    let rank = (p / 100.0 * sorted.len() as f64 - 1e-9).ceil().max(1.0) as usize;
    sorted.get(rank.min(sorted.len()) - 1).copied()
}

pub(crate) fn difference(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    Some(a? - b?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_are_measured_values_by_nearest_rank() {
        let values: Vec<f64> = (1..=1000).map(f64::from).collect();
        let s = Summary::of(&values);
        assert_eq!(s.count, 1000);
        assert_eq!(s.p50, Some(500.0));
        assert_eq!(s.p99, Some(990.0));
        assert_eq!(s.p999, Some(999.0));
        assert_eq!(s.max, Some(1000.0));
        assert_eq!(Summary::of(&[7.0]).p999, Some(7.0));
        assert_eq!(Summary::of(&[]).p50, None);
        assert_eq!(Summary::of(&[f64::NAN, 2.0]).count, 1);
    }

    #[test]
    fn a_difference_needs_both_sides() {
        assert_eq!(difference(Some(3.0), Some(1.0)), Some(2.0));
        assert_eq!(difference(None, Some(1.0)), None);
    }
}
