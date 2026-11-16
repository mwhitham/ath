# Audit of the 2.1.1 skills

This audit led to the 2.1.2 plan. It covers the 24 skills shipped with 2.1.1: the
`athletic-standard` tool skill and 23 calculation skills. It used the 2.1.1
executable and invented records only.

## Verdict

The skills had a sound layout but were not ready to show as a polished set. Most
calculation skills were 18–29 lines: a one-line description and a calculation
block. Their length was only part of the problem. Several methods gave wrong or
impossible results, and some advertised commands did not run.

## Functional failures

Each case below was reproduced on invented records with `ath apply` or
`ath calculate`.

| Finding | What happened | 2.1.2 decision |
|---|---|---|
| SDNN change missed its history | `hrv-change` on an SDNN sample reported no earlier readings although two earlier samples existed. The comparison read only point measurements; Apple writes SDNN as samples. | D232, D233 |
| "Earlier" comparisons included later readings | RMSSD readings of 50, 60, and 100 on three days. `hrv-change` on the reading of 60 reported a change of −15, because the later 100 entered the "earlier" mean. | D232 |
| Heart-rate recovery could never succeed | It looked for a sample 60 seconds after the last sample. | D235 |
| Sleep regularity was wrong | Seven identical sleep windows scored 0.5. Nights were compared at the same clock instant, not 24 hours apart. | D236 |
| Strength volume mixed units | One set of 10 × 100 kg and one of 10 × 100 lb gave a volume of 2000, labelled kilograms. | D237 |
| Advertised commands failed | The range calculation skills said "runs with ath apply baseline". `ath apply baseline` returned "unknown calculation mean". | D243 |
| Effort load accepted any rating | `training-load-effort` multiplied the `rating` of any athlete report, including soreness or mood, by any duration. | D238 |
| Resting heart rate almost never succeeded | It needed a heart rate that did not change at all for 60 seconds. | D241 |
| Heart-rate load used one weighting for everyone | The TRIMP weighting was the male exponent without its coefficient. | D240 |
| Sleep stage totals included awake time | `total_minutes` added the `awake` stage to the sleep stages. | D242 |
| The HRV change result was misnamed | Its result said `same-quantity-change`, not `hrv-change`. | D230 |

The 2.1.1 rebuild notes already recorded the first two behaviours as knowingly
preserved, awaiting "a separate format decision"
([deliberate changes](../../v2.1.1/appendix/deliberate-changes.md)).

## Gaps in the skills themselves

- Descriptions were one generic sentence ("Use when asking for this summary across
  more than one record") and did not say when not to use the skill.
- No skill stated its minimum data, comparison window, units, exclusions, or how to
  read the result.
- No skill had a worked example that a test checked.
- The compatibility line named 2.0.0 although the program was 2.1.1.
- Coaching advice had no home. The tool skill forbids prescribing training, so a
  request for a HYROX plan had no guidance at all.

## Installation

All 24 skills installed into `.agents/skills` with byte-identical content. A
calculation skill that was already installed could never be updated, so a repaired
method would never reach an existing user (D231).

## Both HRV statistics

The record model already had `hrv_rmssd` and `hrv_sdnn`, and the range calculations
kept them apart. A separate SDNN baseline worked. The one-record change did not,
because it ignored measurement samples. Storing both statistics did not prove that
every skill supported both.
