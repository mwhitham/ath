//! Unit conversion for imported measurements.
//!
//! Every converter returns `None` for a unit it does not recognize. The reader
//! then counts a skip that names the unit. A refused row is reported and can be
//! fixed. A silently wrong conversion would look like real data and enter a
//! baseline.
//!
//! These are the converters that ran in 2.1.0 (`crates/ath-core/src/apple.rs`,
//! described by `src/import/units.ts`).

const POUND_KG: f64 = 0.453_592_37;
const STONE_KG: f64 = 6.350_293_18;

/// Lowercase the unit, drop whitespace, and write `·` as `*`.
pub fn normalize_unit(unit: &str) -> String {
    let mut out = String::new();
    for character in unit.trim().chars() {
        if character.is_whitespace() {
            continue;
        }
        for lower in character.to_lowercase() {
            out.push(if lower == '·' { '*' } else { lower });
        }
    }
    out
}

/// A conversion into one standard unit.
pub type Converter = fn(f64, &str) -> Option<f64>;

/// Any unit, for quantities with one sensible unit.
pub fn passthrough(value: f64, _unit: &str) -> Option<f64> {
    Some(value)
}

pub fn per_minute(value: f64, unit: &str) -> Option<f64> {
    matches!(normalize_unit(unit).as_str(), "count/min" | "count/minute" | "bpm" | "").then_some(value)
}

pub fn count(value: f64, unit: &str) -> Option<f64> {
    matches!(normalize_unit(unit).as_str(), "count" | "").then_some(value)
}

pub fn kilograms(value: f64, unit: &str) -> Option<f64> {
    match normalize_unit(unit).as_str() {
        "kg" => Some(value),
        "g" => Some(value / 1000.0),
        "lb" => Some(value * POUND_KG),
        "st" => Some(value * STONE_KG),
        _ => None,
    }
}

pub fn centimeters(value: f64, unit: &str) -> Option<f64> {
    match normalize_unit(unit).as_str() {
        "cm" => Some(value),
        "m" => Some(value * 100.0),
        "mm" => Some(value / 10.0),
        "in" => Some(value * 2.54),
        "ft" => Some(value * 30.48),
        _ => None,
    }
}

pub fn metres(value: f64, unit: &str) -> Option<f64> {
    match normalize_unit(unit).as_str() {
        "m" => Some(value),
        "km" => Some(value * 1000.0),
        "cm" => Some(value / 100.0),
        "mi" => Some(value * 1609.344),
        "yd" => Some(value * 0.9144),
        "ft" => Some(value * 0.3048),
        "in" => Some(value * 0.0254),
        _ => None,
    }
}

pub fn metres_per_second(value: f64, unit: &str) -> Option<f64> {
    match normalize_unit(unit).as_str() {
        "m/s" => Some(value),
        "km/hr" | "km/h" => Some(value / 3.6),
        "mi/hr" | "mi/h" => Some(value * 0.44704),
        "ft/s" => Some(value * 0.3048),
        _ => None,
    }
}

pub fn milliseconds(value: f64, unit: &str) -> Option<f64> {
    match normalize_unit(unit).as_str() {
        "ms" => Some(value),
        "s" | "sec" => Some(value * 1000.0),
        _ => None,
    }
}

pub fn minutes(value: f64, unit: &str) -> Option<f64> {
    match normalize_unit(unit).as_str() {
        "min" => Some(value),
        "s" | "sec" => Some(value / 60.0),
        "hr" | "h" => Some(value * 60.0),
        _ => None,
    }
}

/// Apple writes dietary Calories as `Cal`, which are kilocalories.
pub fn kilocalories(value: f64, unit: &str) -> Option<f64> {
    match normalize_unit(unit).as_str() {
        "kcal" | "cal" => Some(value),
        "kj" => Some(value / 4.184),
        _ => None,
    }
}

pub fn celsius(value: f64, unit: &str) -> Option<f64> {
    match normalize_unit(unit).as_str() {
        "degc" | "°c" | "c" => Some(value),
        "degf" | "°f" | "f" => Some(((value - 32.0) * 5.0) / 9.0),
        _ => None,
    }
}

pub fn watts(value: f64, unit: &str) -> Option<f64> {
    match normalize_unit(unit).as_str() {
        "w" => Some(value),
        "kw" => Some(value * 1000.0),
        _ => None,
    }
}

pub fn millimetres_mercury(value: f64, unit: &str) -> Option<f64> {
    (normalize_unit(unit) == "mmhg").then_some(value)
}

/// Physical effort is METs, which Apple writes as `kcal/hr·kg`.
pub fn mets(value: f64, unit: &str) -> Option<f64> {
    matches!(normalize_unit(unit).as_str(), "kcal/hr*kg" | "met" | "mets").then_some(value)
}

/// Percentages arrive as a fraction or already scaled. Values at or below 1
/// are read as fractions: Apple stores blood oxygen as 0.97.
pub fn percent(value: f64, unit: &str) -> Option<f64> {
    matches!(normalize_unit(unit).as_str(), "%" | "percent" | "").then_some(if value <= 1.0 {
        value * 100.0
    } else {
        value
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_known_units_and_refuses_others() {
        assert_eq!(kilograms(10.0, " LB "), Some(10.0 * POUND_KG));
        assert_eq!(mets(3.0, "kcal/hr·kg"), Some(3.0));
        assert_eq!(celsius(212.0, "degF"), Some(100.0));
        assert_eq!(percent(0.97, "%"), Some(97.0));
        assert_eq!(percent(97.0, ""), Some(97.0));
        assert_eq!(metres(1.0, "furlong"), None);
        assert_eq!(per_minute(60.0, "count/s"), None);
    }
}
