//! Text rules 2.1.0 inherited from JavaScript.
//!
//! The old program trimmed text with `String.prototype.trim`, read numbers
//! with `Number()`, and measured help columns in UTF-16 code units. Readers and
//! the command line use these helpers so native results and refusals match.

/// JavaScript's `WhiteSpace` and `LineTerminator` characters: what `\s`
/// matches and what `trim()` removes.
pub fn is_js_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'
    )
}

/// `String.prototype.trim`.
pub fn js_trim(text: &str) -> &str {
    text.trim_matches(is_js_whitespace)
}

/// The length JavaScript reports for a string: UTF-16 code units.
pub fn js_length(text: &str) -> usize {
    crate::json::utf16_len(text)
}

/// `Number(text)` for a string: decimal, `Infinity`, or a `0x`, `0o`, or `0b`
/// integer, with surrounding whitespace ignored. Empty text is 0. Anything
/// else is NaN.
pub fn js_number(text: &str) -> f64 {
    let text = js_trim(text);
    if text.is_empty() {
        return 0.0;
    }
    for (prefix, radix) in [("0x", 16), ("0X", 16), ("0o", 8), ("0O", 8), ("0b", 2), ("0B", 2)] {
        if let Some(digits) = text.strip_prefix(prefix) {
            if digits.is_empty() {
                return f64::NAN;
            }
            let mut value = 0.0_f64;
            for c in digits.chars() {
                match c.to_digit(radix) {
                    Some(digit) => value = value * f64::from(radix) + f64::from(digit),
                    None => return f64::NAN,
                }
            }
            return value;
        }
    }
    let (sign, unsigned) = match text.as_bytes()[0] {
        b'+' => (1.0, &text[1..]),
        b'-' => (-1.0, &text[1..]),
        _ => (1.0, text),
    };
    if unsigned == "Infinity" {
        return sign * f64::INFINITY;
    }
    if !is_decimal_literal(unsigned) {
        return f64::NAN;
    }
    unsigned.parse::<f64>().map(|value| sign * value).unwrap_or(f64::NAN)
}

/// `StrUnsignedDecimalLiteral`: digits with an optional fraction and exponent,
/// with at least one digit before or after the point.
fn is_decimal_literal(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut index = 0;
    let digits = |index: &mut usize| {
        let start = *index;
        while *index < bytes.len() && bytes[*index].is_ascii_digit() {
            *index += 1;
        }
        *index - start
    };
    let whole = digits(&mut index);
    let mut fraction = 0;
    if index < bytes.len() && bytes[index] == b'.' {
        index += 1;
        fraction = digits(&mut index);
    }
    if whole == 0 && fraction == 0 {
        return false;
    }
    if index < bytes.len() && (bytes[index] == b'e' || bytes[index] == b'E') {
        index += 1;
        if index < bytes.len() && (bytes[index] == b'+' || bytes[index] == b'-') {
            index += 1;
        }
        if digits(&mut index) == 0 {
            return false;
        }
    }
    index == bytes.len()
}

/// `Number.prototype.toFixed(digits)` for a finite, non-negative value below
/// 1e21: the exact decimal value rounded half up.
pub fn js_to_fixed(value: f64, digits: usize) -> String {
    // Rust prints the exact decimal expansion of a double when asked for
    // enough digits, so rounding that text half up matches JavaScript.
    let exact = format!("{value:.1100}");
    let (whole, fraction) = exact.split_once('.').unwrap_or((exact.as_str(), ""));
    let mut kept: Vec<u8> = whole.bytes().chain(fraction.bytes().take(digits)).collect();
    let round_up = fraction.as_bytes().get(digits).is_some_and(|digit| *digit >= b'5');
    if round_up {
        let mut index = kept.len();
        loop {
            if index == 0 {
                kept.insert(0, b'1');
                break;
            }
            index -= 1;
            if kept[index] == b'9' {
                kept[index] = b'0';
            } else {
                kept[index] += 1;
                break;
            }
        }
    }
    let split = kept.len() - digits;
    let mut out = String::from_utf8(kept[..split].to_vec()).expect("digits are ASCII");
    if digits > 0 {
        out.push('.');
        out.push_str(std::str::from_utf8(&kept[split..]).expect("digits are ASCII"));
    }
    out
}

/// `Number.prototype.toLocaleString("en-US")` for a whole number: digits
/// grouped by commas.
pub fn grouped_count(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, c) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// `text.padEnd(width)` measured in UTF-16 code units.
pub fn pad_end(text: &str, width: usize) -> String {
    let length = js_length(text);
    let mut out = text.to_string();
    for _ in length..width {
        out.push(' ');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_follows_javascript() {
        assert_eq!(js_number("12"), 12.0);
        assert_eq!(js_number(" 7\n"), 7.0);
        assert_eq!(js_number(""), 0.0);
        assert_eq!(js_number("1.5"), 1.5);
        assert_eq!(js_number("5."), 5.0);
        assert_eq!(js_number(".5"), 0.5);
        assert_eq!(js_number("1e3"), 1000.0);
        assert_eq!(js_number("-2"), -2.0);
        assert_eq!(js_number("+2"), 2.0);
        assert_eq!(js_number("0x10"), 16.0);
        assert_eq!(js_number("0b101"), 5.0);
        assert_eq!(js_number("Infinity"), f64::INFINITY);
        for text in ["abc", "1,000", "0x", "-0x10", "1e", ".", "inf", "nan", "1_000", "12abc", "e5"] {
            assert!(js_number(text).is_nan(), "{text}");
        }
    }

    #[test]
    fn trim_uses_javascript_whitespace() {
        assert_eq!(js_trim("\u{feff} yes \u{a0}"), "yes");
        assert_eq!(js_trim("\u{85}y"), "\u{85}y");
    }

    #[test]
    fn to_fixed_rounds_ties_up() {
        assert_eq!(js_to_fixed(1.25, 1), "1.3");
        assert_eq!(js_to_fixed(0.05, 1), "0.1");
        assert_eq!(js_to_fixed(9.96, 1), "10.0");
        assert_eq!(js_to_fixed(2.5, 0), "3");
        assert_eq!(js_to_fixed(12.0, 0), "12");
    }

    #[test]
    fn counts_are_grouped() {
        assert_eq!(grouped_count(0), "0");
        assert_eq!(grouped_count(999), "999");
        assert_eq!(grouped_count(1000), "1,000");
        assert_eq!(grouped_count(1234567), "1,234,567");
    }
}
