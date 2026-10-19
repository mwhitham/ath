# Provider capability and calculation coverage

One table for imports and calculations. **Step** names match
[`rust-adoption.md`](rust-adoption.md).

Classification:

- **observation** — measured value with source
- **provider_result** — vendor score or summary, attributed to the vendor
- **reproducible** — ATH recomputes with a named method
- **ath_estimate** — transparent ATH method when the vendor does not ship the value
- **unavailable** — missing inputs or method; reason stated

| Provider capability | Input / export path | ATH representation | Method and reference | Required data | Current support | Step |
|---|---|---|---|---|---|---|
| WHOOP recovery score | CSV `physiological_cycles` | `provider_score` | provider_result | cycle row | partial import | Provider imports |
| WHOOP strain | CSV workouts / cycles | `provider_score` or session metrics | provider_result | workout/cycle | partial | Provider imports |
| WHOOP sleep performance | CSV `sleeps` | `provider_score` | provider_result | sleep row | partial | Provider imports |
| WHOOP journal entries | CSV | `athlete_report` or notes | observation / report | text fields | partial | Provider imports |
| WHOOP HRV (RMSSD) | CSV cycles | `point_measurement` `hrv_rmssd` | observation | nightly value | yes | Provider imports |
| WHOOP resting HR | CSV cycles | `point_measurement` `resting_heart_rate` | observation | nightly value | yes | Provider imports |
| WHOOP workout HR samples | CSV / API | — | unavailable — CSV has avg/max HR only; API has no continuous HR series | continuous HR | not supported | — |
| Oura readiness score | CSV export | `provider_score` | provider_result | readiness row | mapped in `src/import/oura.ts`; owner-reported real-export failures not yet reproduced | Provider imports |
| Oura sleep score | CSV export | `provider_score` | provider_result | sleep summary | mapped; owner-reported real-export failures not yet reproduced | Provider imports |
| Oura readiness contributors (nine) | CSV export | `provider_score` | provider_result | contributor columns | mapped; owner-reported real-export failures not yet reproduced | Provider imports |
| Oura SpO2 | CSV export | `point_measurement` | observation | SpO2 columns | mapped; owner-reported real-export failures not yet reproduced | Provider imports |
| Oura sleep stages | CSV export | `sleep_session` | observation | stage durations | partial | Provider imports |
| Oura HRV (RMSSD) | CSV export | `point_measurement` `hrv_rmssd` | observation | sleep summary | yes | Provider imports |
| Oura temperature deviation | CSV export | `point_measurement` | observation | temp signal | partial | Provider imports |
| Apple Health HRV (SDNN) | export.zip | `point_measurement` `hrv_sdnn` | observation | SDNN samples | yes (import) | Provider imports |
| Apple Health resting HR | export.zip | `point_measurement` | observation | daily RHR | yes | Provider imports |
| Apple Health workouts | export.zip | `workout_session` | observation | workout records | yes | Provider imports |
| Apple Health sleep analysis | export.zip | `sleep_session` | observation | segments | partial | Provider imports |
| Apple beat lists → RMSSD | export.zip | `hrv_rmssd` via `src/hrv.ts` | reproducible | beat timestamps | yes | Existing calculations |
| Summary stats | CLI `ath calculate summary` | calculation result | reproducible — `ath-runtime-v1` | points or series | yes | Existing calculations |
| Baseline | CLI baseline | calculation result | mean and SD over the requested date range (not a rolling window) | range + quantity | yes | Existing calculations |
| Trend | CLI trend | calculation result | reproducible | time series | yes | Existing calculations |
| Calendar coverage | CLI coverage | calculation result | reproducible | dates in range | yes | Existing calculations |
| Compare sources | CLI compare-sources | calculation result | reproducible | two sources | yes | Existing calculations |
| Correlation | CLI correlation | calculation result | reproducible | two quantities | yes | Existing calculations |
| Workout drift | CLI workout-drift | calculation result | reproducible | workout samples | yes | Existing calculations |
| Provider period summary | CLI provider-period-summary | calculation result | reproducible | provider periods | yes | Existing calculations |
| HRV RMSSD from RR intervals | `src/hrv.ts` | `hrv_rmssd` | reproducible — RMSSD formula | beat intervals ms | yes | Existing calculations |
| HRV SDNN daily aggregate | — | — | unavailable — no SDNN calculation in codebase | beat series | unavailable | Expanded calculations |
| HRV method-specific nightly summary | multi-sample nights | calculation result | reproducible — method registry | samples + method id | partial | Expanded calculations |
| Resting HR (defined methods) | HR time series | calculation result | reproducible — method TBD | HR samples | unavailable | Expanded calculations |
| Heart-rate zones / time in zone | workouts + max HR | calculation result | reproducible — zones documented | HR series | unavailable | Expanded calculations |
| Heart-rate recovery | post-workout HR | calculation result | reproducible | HR series | unavailable | Expanded calculations |
| Sleep duration / efficiency | sleep sessions | calculation result | reproducible | session times | partial | Expanded calculations |
| Sleep interruptions / timing / regularity | sleep sessions | calculation result | reproducible | session history | unavailable | Expanded calculations |
| Sleep stage summaries | staged sleep | calculation result | sum supplied stages only | stage minutes | partial | Expanded calculations |
| Infer sleep stages from raw sensors | — | — | unavailable — policy | raw sensors | unavailable | — |
| Training load (HR-based) | workouts + HR | calculation result | ath_estimate — method TBD | HR, duration | unavailable | Expanded calculations |
| Training load (RPE-based) | athlete report + duration | calculation result | ath_estimate | RPE, duration | unavailable | Expanded calculations |
| Strength volume | workouts | — | unavailable — no implementation | structured workout | unavailable | Expanded calculations |
| Recovery / readiness (ATH generic) | HRV + sleep + RHR | calculation result | ath_estimate — method TBD | baselines | unavailable | Expanded calculations |
| Vendor readiness as ATH-branded score | — | — | unavailable — use provider_result or a distinct ATH method name | — | n/a | — |

## Calculation contract

Each `ath-core` result should include method id and version, parameters, units, time
range, input revisions, exclusions, missing-data handling, and a clear unavailable reason
when inputs are not enough.

Numerical tests use answers computed outside the code under test. RMSSD and SDNN never
share a baseline.

## Import completeness

Imports must emit a receipt (imported, unchanged, corrected, skipped, invalid, missing).
Silent loss of a mapped field is a failure.

Owner-reported import failures on real exports should become public synthetic regressions
when reproduced.
