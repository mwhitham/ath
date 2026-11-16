//! The order of `String.prototype.localeCompare` in the 2.1.0 program.
//!
//! Import previews list skip reasons in this order. Node compares with the
//! ICU root collation: letters ignore case first, punctuation sorts before
//! digits, and digits sort before letters. Case decides only between strings
//! that are otherwise equal, and lowercase comes first.
//!
//! The table below is exact for ASCII. It was read from Node 24 with the
//! `en-US` locale. A character outside ASCII sorts after every ASCII
//! character, by code point. That is not what ICU does with accented letters,
//! so a reason that differs from another only by such a letter may be listed
//! in another order. Control characters other than whitespace are ignored, as
//! ICU ignores them.

use std::cmp::Ordering;

/// Printable ASCII and whitespace in ICU root order, except letters.
const ORDER: &str = "\t\n\u{b}\u{c}\r _-,;:!?.'\"()[]{}@*/\\&#%`^+<=>|~$0123456789";

/// The primary weight of a character, or `None` when ICU ignores it.
fn primary(character: char) -> Option<u32> {
    if character.is_ascii_alphabetic() {
        let base = ORDER.chars().count() as u32;
        return Some(base + u32::from(character.to_ascii_lowercase() as u8 - b'a'));
    }
    if let Some(position) = ORDER.chars().position(|item| item == character) {
        return Some(position as u32);
    }
    let code = character as u32;
    if code < 0x20 || code == 0x7f {
        return None;
    }
    Some(0x1000 + code)
}

fn tertiary(character: char) -> u8 {
    u8::from(character.is_ascii_uppercase())
}

/// Compare two strings as `left.localeCompare(right)` does.
pub fn locale_compare(left: &str, right: &str) -> Ordering {
    let weights = |text: &str| -> Vec<(u32, u8)> {
        text.chars().filter_map(|character| primary(character).map(|weight| (weight, tertiary(character)))).collect()
    };
    let left = weights(left);
    let right = weights(right);
    let primary_order = left.iter().map(|(weight, _)| weight).cmp(right.iter().map(|(weight, _)| weight));
    primary_order.then_with(|| left.iter().map(|(_, case)| case).cmp(right.iter().map(|(_, case)| case)))
}

#[cfg(test)]
mod tests {
    use super::locale_compare;

    fn sorted(items: &[&str]) -> Vec<String> {
        let mut out: Vec<String> = items.iter().map(|item| item.to_string()).collect();
        out.sort_by(|left, right| locale_compare(left, right));
        out
    }

    #[test]
    fn letters_ignore_case_before_punctuation_and_digits() {
        assert_eq!(
            sorted(&["ab", "a b", "a-b", "a_b", "aB", "Ab", "AB", "a", "A", "a1", "a10", "a2"]),
            ["a", "A", "a b", "a_b", "a-b", "a1", "a10", "a2", "ab", "aB", "Ab", "AB"]
        );
    }

    #[test]
    fn control_characters_are_ignored() {
        assert_eq!(locale_compare("a\u{0}b", "ab"), std::cmp::Ordering::Equal);
        assert_eq!(locale_compare("a\tb", "ab"), std::cmp::Ordering::Less);
    }
}
