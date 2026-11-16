//! Deterministic prediction grade arithmetic.
//!
//! A grade compares a prediction revision with a result revision in the
//! benchmark's own score unit. The rounding matches the published grades:
//! signed error to two decimals and percentage error to one decimal, with
//! halves rounded toward positive infinity.

use super::common::Score;
use super::data::PredictionData;

/// `Math.round`: the nearest integer, with halves rounded up.
pub fn js_round(value: f64) -> f64 {
    // `f64::round` rounds negative halves away from zero: -2.5 becomes -3.
    // JavaScript rounds them up to -2.
    if value.fract() == -0.5 {
        value.ceil()
    } else {
        value.round()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradeArithmetic {
    pub predicted_value: f64,
    pub actual_value: f64,
    pub signed_error: f64,
    pub absolute_error_percentage: f64,
    pub result_in_predicted_range: bool,
}

fn native(score: &Score, score_type: &str) -> Result<f64, String> {
    score.value_for(score_type).ok_or_else(|| format!("Score does not contain the benchmark's {score_type} value"))
}

/// Compute a grade from a prediction and the actual score.
pub fn grade_arithmetic(
    prediction: &PredictionData,
    actual: &Score,
    score_type: &str,
) -> Result<GradeArithmetic, String> {
    let predicted_value = native(&prediction.predicted_score, score_type)?;
    let actual_value = native(actual, score_type)?;
    let signed_error = js_round((predicted_value - actual_value) * 100.0) / 100.0;
    let absolute_error_percentage = js_round(signed_error.abs() / actual_value * 1_000.0) / 10.0;
    let result_in_predicted_range = match &prediction.predicted_range {
        Some(range) => {
            actual_value >= native(&range.low, score_type)? && actual_value <= native(&range.high, score_type)?
        }
        None => false,
    };
    Ok(GradeArithmetic {
        predicted_value,
        actual_value,
        signed_error: if signed_error == 0.0 { 0.0 } else { signed_error },
        absolute_error_percentage,
        result_in_predicted_range,
    })
}

#[cfg(test)]
mod tests {
    use super::js_round;

    #[test]
    fn rounding_matches_javascript() {
        assert_eq!(js_round(2.5), 3.0);
        assert_eq!(js_round(-2.5), -2.0);
        assert_eq!(js_round(-0.5), 0.0);
        assert_eq!(js_round(-1.5), -1.0);
        assert_eq!(js_round(1.4999999999999998), 1.0);
        assert_eq!(js_round(0.49999999999999994), 0.0);
        assert_eq!(js_round(-3.7), -4.0);
        assert_eq!(js_round(12345.5), 12346.0);
    }
}
