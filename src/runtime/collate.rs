//! The order JavaScript's `a.localeCompare(b)` gives model IDs.
//!
//! Version 2.1.0 sorted the live model list with `localeCompare`, which uses
//! the Unicode collation order and not code-point order. For example, `:`
//! sorts before `.` and `/`, and punctuation sorts before digits. The model
//! list order is visible in `ath models` and decides the order of a
//! `backtest --all` report, so it is kept.
//!
//! Printable ASCII follows the root collation exactly, as measured from Node.
//! Letters compare without case first; a lowercase letter then sorts before
//! its uppercase form. Other characters sort after ASCII by code point. Model
//! catalogs use ASCII IDs.

use std::cmp::Ordering;

/// Printable ASCII in root collation order, lowercase before uppercase.
const ASCII_ORDER: &str =
    " _-,;:!?.'\"()[]{}@*/\\&#%`^+<=>|~$0123456789aAbBcCdDeEfFgGhHiIjJkKlLmMnNoOpPqQrRsStTuUvVwWxXyYzZ";

fn primary(character: char) -> u32 {
    let folded = character.to_ascii_lowercase();
    match ASCII_ORDER.find(folded) {
        Some(index) => index as u32,
        None => 0x100 + character as u32,
    }
}

fn tertiary(character: char) -> u8 {
    u8::from(character.is_ascii_uppercase())
}

/// Compare two strings as `a.localeCompare(b)` does for ASCII text.
pub(crate) fn locale_compare(a: &str, b: &str) -> Ordering {
    a.chars()
        .map(primary)
        .cmp(b.chars().map(primary))
        .then_with(|| a.chars().map(tertiary).cmp(b.chars().map(tertiary)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_node_for_model_like_ids() {
        // Node 25: ["a-b","ab","a_b","a.b","a:b","a/b","a1","aB","Ab","ab","AB"].sort(localeCompare)
        let mut ids = vec!["a-b", "ab", "a_b", "a.b", "a:b", "a/b", "a1", "aB", "Ab", "ab", "AB"];
        ids.sort_by(|a, b| locale_compare(a, b));
        assert_eq!(ids, ["a_b", "a-b", "a:b", "a.b", "a/b", "a1", "ab", "ab", "aB", "Ab", "AB"]);
    }

    #[test]
    fn colon_sorts_before_dot() {
        assert_eq!(locale_compare("qwen-2:free", "qwen-2.5"), Ordering::Less);
        assert_eq!(locale_compare("x", "x:free"), Ordering::Less);
    }
}
