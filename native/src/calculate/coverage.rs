//! What a number rests on, in one shape every command uses (D47).
//!
//! A coverage record gives the observation count, the first and last day,
//! the days present out of the days in the window, the one source, and the
//! rule in words. Readings from different sources are never pooled (D31), so
//! one coverage record always names one source.

use std::cmp::Ordering;

use serde::Serialize;

use crate::model::common::js_date_parse;
use crate::model::grade::js_round;

/// The days between two calendar dates, counting both ends. Never below 1.
pub fn days_between(from: &str, to: &str) -> i64 {
    let day = |text: &str| js_date_parse(&format!("{text}T00:00:00Z"));
    match (day(from), day(to)) {
        (Some(start), Some(end)) => (js_round((end - start) as f64 / 86_400_000.0) as i64 + 1).max(1),
        // `Math.max(1, NaN)` is NaN in JavaScript; an unreadable date has no span.
        _ => 1,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Coverage {
    /// Observations the number rests on.
    pub n: usize,
    /// First day covered.
    pub from: String,
    /// Last day covered.
    pub to: String,
    /// Days with at least one observation.
    pub days_present: usize,
    /// Days in the window.
    pub days_expected: i64,
    /// The one source the observations came from.
    pub source: String,
    /// How the number was calculated, in words.
    pub rule: String,
}

/// Coverage for timestamped observations from one source. The ends of the
/// window are found by instant, not by comparing day text, because an offset
/// can name an earlier local day than a UTC time that happened before it.
pub fn coverage_of(observations: &[&str], source: &str, rule: &str) -> Option<Coverage> {
    let at = |text: &str| js_date_parse(text).map_or(f64::NAN, |millis| millis as f64);
    let mut earliest = *observations.first()?;
    let mut latest = earliest;
    for observation in &observations[1..] {
        // `reduce((a, b) => at(a) <= at(b) ? a : b)` keeps `b` when either is unreadable.
        if !matches!(at(earliest).partial_cmp(&at(observation)), Some(Ordering::Less | Ordering::Equal)) {
            earliest = observation;
        }
        if !matches!(at(latest).partial_cmp(&at(observation)), Some(Ordering::Greater | Ordering::Equal)) {
            latest = observation;
        }
    }
    let day = |text: &str| text.chars().take(10).collect::<String>();
    let from = day(earliest);
    let to = day(latest);
    let mut present: Vec<String> = observations.iter().map(|text| day(text)).collect();
    present.sort();
    present.dedup();
    Some(Coverage {
        n: observations.len(),
        days_expected: days_between(&from, &to),
        from,
        to,
        days_present: present.len(),
        source: source.to_string(),
        rule: rule.to_string(),
    })
}

/// Coverage on one line. The gap is days present out of days in the window.
pub fn render_coverage(coverage: &Coverage) -> String {
    let window = if coverage.from == coverage.to {
        coverage.from.clone()
    } else {
        format!("{} → {}", coverage.from, coverage.to)
    };
    let gap = if coverage.days_present as i64 == coverage.days_expected {
        String::new()
    } else {
        format!(", {}/{} days", coverage.days_present, coverage.days_expected)
    };
    format!("n={}, {window}{gap}, {} — {}", coverage.n, coverage.source, coverage.rule)
}

/// The rules the tools apply, written once so the same words appear everywhere.
pub mod rules {
    pub const PER_SOURCE: &str = "one source only, never pooled across devices";
    pub const SLEEP_DURATION: &str = "actual sleep, excluding time awake — not time in bed";
    pub const TIME_IN_BED: &str = "time in bed, including time awake";

    pub fn baseline(record_type: &str, window_days: u32) -> String {
        format!("mean of {record_type} over the trailing {window_days} days, from this source alone")
    }

    pub fn daily_summary(quantity: &str) -> String {
        format!("every {quantity} sample recorded that day, from one source")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_observations_the_window_and_days_present() {
        let coverage = coverage_of(
            &["2026-08-01T06:00:00Z", "2026-08-01T22:00:00Z", "2026-08-05T06:00:00Z"],
            "whoop-1",
            rules::PER_SOURCE,
        )
        .unwrap();
        assert_eq!((coverage.n, coverage.from.as_str(), coverage.to.as_str()), (3, "2026-08-01", "2026-08-05"));
        assert_eq!((coverage.days_present, coverage.days_expected), (2, 5));
        assert_eq!(coverage.source, "whoop-1");
    }

    #[test]
    fn finds_the_window_ends_by_instant() {
        let coverage =
            coverage_of(&["2026-08-31T01:00:00Z", "2026-08-30T20:00:00-07:00"], "whoop-1", rules::PER_SOURCE).unwrap();
        assert_eq!(coverage.to, "2026-08-30");
    }

    #[test]
    fn has_nothing_to_say_about_nothing() {
        assert!(coverage_of(&[], "whoop-1", rules::PER_SOURCE).is_none());
    }

    #[test]
    fn counts_both_ends_of_a_window() {
        assert_eq!(days_between("2026-08-01", "2026-08-01"), 1);
        assert_eq!(days_between("2026-08-01", "2026-08-03"), 3);
    }

    #[test]
    fn names_the_rule_and_hides_the_gap_only_when_there_is_none() {
        let full =
            coverage_of(&["2026-08-01T06:00:00Z", "2026-08-02T06:00:00Z"], "whoop-1", rules::SLEEP_DURATION).unwrap();
        assert_eq!(
            render_coverage(&full),
            "n=2, 2026-08-01 → 2026-08-02, whoop-1 — actual sleep, excluding time awake — not time in bed"
        );
        let gappy =
            coverage_of(&["2026-08-01T06:00:00Z", "2026-08-05T06:00:00Z"], "whoop-1", rules::PER_SOURCE).unwrap();
        assert!(render_coverage(&gappy).contains("2/5 days"));
    }
}
