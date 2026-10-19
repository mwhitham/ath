# Provider capability and calculation coverage

One table for import fields and calculations. **Stage** names match
`build-history/plans/rust-adoption.md`.

Classification:

- **observation** — imported measured value with source
- **provider_result** — branded score or summary attributed to the vendor
- **reproducible** — ATH can recompute from documented inputs and a named method
- **ath_estimate** — transparent ATH method when vendor does not ship the value
- **unavailable** — required inputs or method missing; reason stated

| Provider capability | Input / export path | ATH representation | Method and reference | Required data | Current support | Stage |
|---|---|---|---|---|---|---|
| WHOOP recovery score | CSV `physiological_cycles` | `provider_score` | provider_result — store as reported | cycle row | partial import | Provider imports |
| WHOOP strain | CSV workouts / cycles | `provider_score` or session metrics | provider_result | workout/cycle | partial | Provider imports |
| WHOOP sleep performance | CSV `sleeps` | `provider_score` | provider_result | sleep row | partial | Provider imports |
| WHOOP journal entries | CSV | `athlete_report` or tagged notes | observation / report | text fields | partial | Provider imports |
| WHOOP HRV (RMSSD) | CSV cycles | `point_measurement` `hrv_rmssd` | observation | nightly value | yes | Existing calculations |
| WHOOP resting HR | CSV cycles | `point_measurement` `resting_heart_rate` | observation | nightly value | yes | Existing calculations |
| WHOOP workout HR samples | CSV | `measurement_sample` series | observation | time series | partial | Provider imports |
| Oura readiness score | export / API fields | `provider_score` | provider_result | readiness object | **gap** — investigation target | Provider imports |
| Oura sleep score | export | `provider_score` | provider_result | sleep summary | **gap** | Provider imports |
| Oura readiness contributors | export | extensions or structured fields | provider_result | contributor list | **gap** | Provider imports |
| Oura SpO2 / blood oxygen | export | `point_measurement` | observation | SpO2 readings | **gap** | Provider imports |
| Oura sleep stages | export | `sleep_session` stages | observation | stage durations | partial | Provider imports |
| Oura HRV (RMSSD) | export | `point_measurement` `hrv_rmssd` | observation | sleep summary | yes | Existing calculations |
| Oura temperature deviation | export | `point_measurement` | observation | temp signal | partial | Provider imports |
| Apple Health HRV (SDNN) | export.zip | `point_measurement` `hrv_sdnn` | observation | SDNN samples | yes | Existing calculations |
| Apple Health resting HR | export.zip | `point_measurement` | observation | daily RHR | yes | Provider imports |
| Apple Health workouts | export.zip | `workout_session` | observation | workout records | yes | Provider imports |
| Apple Health sleep analysis | export.zip | `sleep_session` | observation | segments | partial | Provider imports |
| Apple Health beat-to-beat for RMSSD | export.zip ECG / beat lists | series → RMSSD | reproducible — `hrv_rmssd` from intervals | beat timestamps | yes (`src/hrv.ts`) | Existing calculations |
| Summary stats (mean, min, max, count) | CLI `ath calculate summary` | calculation result | reproducible — `ath-runtime-v1` | point or series | yes | Existing calculations |
| Baseline (rolling window) | CLI baseline | calculation result | reproducible | history window | yes | Existing calculations |
| Trend (slope / direction) | CLI trend | calculation result | reproducible | time series | yes | Existing calculations |
| Calendar coverage | CLI coverage | calculation result | reproducible | dates in range | yes | Existing calculations |
| Compare sources | CLI compare-sources | calculation result | reproducible | two sources | yes | Existing calculations |
| Correlation | CLI correlation | calculation result | reproducible | two quantities | yes | Existing calculations |
| Workout drift | CLI workout-drift | calculation result | reproducible | workout samples | yes | Existing calculations |
| Provider period summary | CLI provider-period-summary | calculation result | reproducible | provider periods | yes | Existing calculations |
| HRV RMSSD from RR intervals | core library | `hrv_rmssd` | reproducible — standard RMSSD formula | beat intervals ms | yes | Existing calculations |
| HRV SDNN daily | measurements | `hrv_sdnn` | reproducible — SDNN definition | beat series | partial | Expanded calculations |
| HRV method-specific nightly summary | multi-sample nights | calculation result | reproducible — method registry | samples + method id | partial | Expanded calculations |
| Resting HR (defined methods) | HR time series | calculation result | reproducible — e.g. lowest 5-min mean | HR samples | unavailable — method not chosen | Expanded calculations |
| Heart-rate zones / time in zone | workouts + max HR | calculation result | reproducible — zone model documented | HR series, zone bounds | unavailable | Expanded calculations |
| Heart-rate recovery | post-workout HR | calculation result | reproducible — delta at 1/2 min | HR series | unavailable | Expanded calculations |
| Sleep duration / efficiency | sleep sessions | calculation result | reproducible | session times | partial from sessions | Expanded calculations |
| Sleep interruptions / timing / regularity | sleep sessions | calculation result | reproducible | session history | unavailable | Expanded calculations |
| Sleep stage summaries | staged sleep | calculation result | reproducible — sum supplied stages only | stage minutes | partial | Expanded calculations |
| Infer sleep stages from raw sensors | accelerometer / HR | — | unavailable — not inferring vendor stages | raw sensors | unavailable — by policy | — |
| Training load (HR-based) | workouts + HR | calculation result | ath_estimate — named ATH method TBD | HR zones, duration | unavailable | Expanded calculations |
| Training load (RPE-based) | athlete report + duration | calculation result | ath_estimate | RPE, duration | unavailable | Expanded calculations |
| Strength volume | workouts | calculation result | reproducible — sets × reps × load | structured workout | partial | Expanded calculations |
| Recovery / readiness (ATH generic) | HRV + sleep + RHR | calculation result | ath_estimate — explicit formula in registry | baselines per method | unavailable | Expanded calculations |
| WHOOP/Oura branded readiness as ATH score | — | — | unavailable — use provider_result or separate ATH method name | — | n/a | — |

## Calculation contract (prerequisite)

Every calculation result returned from `ath-core` must include:

- `method_id` and `method_version`
- parameters used (window, grouping, timezone, source filters)
- units and quantity
- time range and record revisions consumed
- exclusions and missing-data handling
- `unavailable` with reason when inputs are insufficient

Numerical tests use reference answers computed outside the implementation under test.
RMSSD and SDNN never share a baseline. Provider scores stay labeled as vendor results.

## Import completeness (measurable)

Imports must emit a **receipt**: imported, unchanged, corrected, skipped (with reason),
invalid, and unexpectedly missing fields. Silent loss of a supported mapped field is a
failure. Video findings (Oura readiness, sleep scores, contributors, SpO₂, invalid
records after reimport) are regression targets documented in the Rust plan until reproduced
with synthetic fixtures.
