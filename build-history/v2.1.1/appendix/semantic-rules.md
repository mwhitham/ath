# Semantic rules for the 2.1.0 record model

This list names every record rule that plain JSON Schema cannot fully express.
The native validator must run each rule. Part 1 covers the rules in the Zod
schemas. Part 2 covers the rules the database layer enforces outside Zod.

Part 1 rules carry IDs R1 to R56. The same IDs appear in the `rules` field of the
vector cases under `tests/vectors/model/`. The file
`tests/vectors/model-index.json` lists every case that triggers or satisfies
each rule. The generator asserts each listed result, so a rule cannot lose its
evidence without failing generation.

Case names below use the form `File#case`, where `File` is the vector file name
without `.json`. Paths are relative to the schema that owns the rule. When the
schema is nested, the parent path comes first. For example, R1 inside
`PointMeasurementData` reports `["input_timestamp", "timestamp_missing_reason"]`.

Regenerate the vectors with:

```sh
node --import tsx tools/reference/generate-model-vectors.ts
```

The vectors were observed with Zod 4.5.2 and ICU 78.2 (tz 2025c).

## How Zod reports issues

These behaviors apply to every rule below. The vectors show each one, and the
native validator must reproduce them to match the issue lists exactly.

1. **Issue order follows the schema, not the input.** Zod walks object keys in
   schema order. Each value's issues come before the issues of later keys. An
   object's `unrecognized_keys` issue comes after its own key issues. An object's
   refinements come last, in source order. In a current record, extension issues
   come before data issues, because `extensions` precedes `data` in the shape.
2. **Some issues stop refinements.** An object's refinements are skipped when any
   issue inside that object is `invalid_type`, `invalid_value`, `invalid_union`, or
   `invalid_key`. A missing required key is `invalid_type`. Refinements still run
   after `too_small`, `too_big`, `invalid_format`, `unrecognized_keys`, and `custom`.
   See `InputTimestampReceipt#order:*` and `PointMeasurementData#order:*`.
3. **Refinements see values that failed a format check.** The time-order rules
   (R10, R20, R21, R52, R54) compare values with JavaScript `Date.parse`. They run
   even when the string failed the timestamp format. `Date.parse` returns NaN for
   text it cannot read. NaN fails `>=` and passes `<`. So an unreadable sleep end
   triggers R20, while an unreadable period end passes R10. See
   `SleepSessionData#date_parse:*`, `ProviderReportingPeriod#date_parse:*`, and
   `MeasurementSeriesFile#date_parse:*`. The generator sets `TZ=UTC`, because
   `Date.parse` reads a time without a zone as local time.
4. **Integers must be safe integers.** `.int()` rejects a fraction with
   `invalid_type` "expected int", which stops refinements. It rejects a value above
   2^53 − 1 with `too_big` (origin `int`), which does not stop them. A value below
   −(2^53 − 1) gets `too_small` from the integer check, then any range issue. JSON
   text such as `1.0`, `1E0`, and `10e-1` is accepted because it parses to an
   integer. `-0` is accepted where the minimum is 0 and rejected by `.positive()`.
   See `RecordRevisionNumber`.
5. **Zod counts string length in code points.** `.min(1)` and `.max(4096)` count
   Unicode code points. A lone surrogate counts as one. The extension string rule
   (R35) counts UTF-16 units instead. See R35.
6. **Parsed output is rebuilt.** A strict object is written in schema key order. An
   optional key that is absent stays absent. A record-valued field (supplied
   fields, stage minutes, extensions) keeps JavaScript object order. Keys that are
   array indexes (canonical integers up to 4294967294) come first in numeric
   order. Other keys keep their input order. A duplicate key keeps its first
   position and its last value.
7. **`__proto__` keys are handled in two ways.** In a strict object, `__proto__` is
   an unrecognized key. In a record-valued field, it is silently dropped from the
   output, together with everything under it. See `SuppliedFields#keys:proto_*` and
   the R29 note.
8. **Some issue fields are nested.** `invalid_union` carries `errors`, one issue list
   per branch. A plain union such as `RecordRevision` lists every branch when the
   matching branch has a type error. When only one branch has non-stopping issues,
   Zod reports that branch's issues directly. See
   `RecordRevision#invalid:soft_issue_only` and `RecordRevision#invalid:missing_data`.
   A failed discriminator has `note`, `discriminator`, and `options`. `invalid_key`
   carries `issues`.

## Part 1: rules in the Zod schemas

### Input timestamp receipts

`InputTimestampReceipt` in `src/v1/schema.ts:161-233`. Every measured record and
athlete report carries at least one receipt.

**R1. A missing input timestamp needs a reason.** `src/v1/schema.ts:173-179`
When `timestamp_text` is absent, `timestamp_missing_reason` must be present.
Message `A missing input timestamp needs a reason`, path `["timestamp_missing_reason"]`.
Triggers: `InputTimestampReceipt#rule:empty_object`, `InputTimestampReceipt#rule:no_timestamp_no_reason`.
Satisfies: `InputTimestampReceipt#rule:all_satisfied_minimal`, `InputTimestampReceipt#rule:basis_without_timestamp`.

**R2. An available timestamp cannot also be marked missing.** `src/v1/schema.ts:180-186`
`timestamp_text` and `timestamp_missing_reason` cannot both be present.
Message `An available input timestamp cannot also be marked missing`, path `["timestamp_missing_reason"]`.
Triggers: `InputTimestampReceipt#rule:timestamp_and_reason`, `InputTimestampReceipt#order:empty_text_still_runs_refinements`.
Satisfies: `InputTimestampReceipt#rule:all_satisfied_maximal`.

**R3. An input timestamp needs its basis.** `src/v1/schema.ts:187-193`
When `timestamp_text` is present, `timestamp_basis` must be present. A basis
without text is allowed.
Message `An input timestamp needs its documented basis`, path `["timestamp_basis"]`.
Triggers: `InputTimestampReceipt#rule:timestamp_without_basis`.
Satisfies: `InputTimestampReceipt#rule:basis_without_timestamp`, `InputTimestampReceipt#rule:all_satisfied_maximal`.

**R4. A missing UTC offset needs a reason.** `src/v1/schema.ts:194-200`
When `utc_offset_text` is absent, `utc_offset_missing_reason` must be present.
Message `A missing input UTC offset needs a reason`, path `["utc_offset_missing_reason"]`.
Triggers: `InputTimestampReceipt#rule:no_offset_no_reason`, `InputTimestampReceipt#rule:empty_object`.
Satisfies: `InputTimestampReceipt#rule:all_satisfied_minimal`.

**R5. An available UTC offset cannot also be marked missing.** `src/v1/schema.ts:201-207`
`utc_offset_text` and `utc_offset_missing_reason` cannot both be present.
Message `An available input UTC offset cannot also be marked missing`, path `["utc_offset_missing_reason"]`.
Triggers: `InputTimestampReceipt#rule:offset_and_reason`.
Satisfies: `InputTimestampReceipt#rule:all_satisfied_maximal`.

**R6. A UTC offset needs its meaning.** `src/v1/schema.ts:208-214`
When `utc_offset_text` is present, `utc_offset_meaning` must be present.
Message `An input UTC offset needs its documented meaning`, path `["utc_offset_meaning"]`.
Triggers: `InputTimestampReceipt#rule:offset_without_meaning`.
Satisfies: `InputTimestampReceipt#rule:all_satisfied_maximal`.

**R7. An offset meaning needs offset text.** `src/v1/schema.ts:215-221`
When `utc_offset_text` is absent, `utc_offset_meaning` must be absent.
Message `UTC offset meaning requires UTC offset text`, path `["utc_offset_meaning"]`.
Triggers: `InputTimestampReceipt#rule:meaning_without_offset`.
Satisfies: `InputTimestampReceipt#rule:all_satisfied_minimal`.

**R8. A source timezone and its meaning come together.** `src/v1/schema.ts:222-232`
`source_time_zone` and `source_time_zone_meaning` are both present or both absent.
The zone text is not checked against IANA names.
Message `A source timezone and its documented meaning must be provided together`, path `["source_time_zone_meaning"]`.
Triggers: `InputTimestampReceipt#rule:time_zone_without_meaning`, `InputTimestampReceipt#rule:time_zone_meaning_without_zone`.
Satisfies: `InputTimestampReceipt#rule:time_zone_not_checked_as_iana`, `InputTimestampReceipt#rule:all_satisfied_maximal`.

### Provider reporting periods

`ProviderReportingPeriod` in `src/v1/schema.ts:252-307`.

**R9. A reporting period needs an anchor.** `src/v1/schema.ts:266-275`
At least one of `provider_period_id`, `provider_date`, or `start_at` must be
present. `end_at` alone is not an anchor. A present but malformed `start_at` still
counts.
Message `A provider reporting period needs an ID, date, or start instant`, path `[]`.
Triggers: `ProviderReportingPeriod#rule:no_identifier`, `ProviderReportingPeriod#rule:end_only_is_not_an_identifier`.
Satisfies: `ProviderReportingPeriod#rule:id_only`, `ProviderReportingPeriod#rule:date_only`, `ProviderReportingPeriod#rule:start_only`, `ProviderReportingPeriod#rule:invalid_start_counts_as_present`.

**R10. A reporting period cannot end before it starts.** `src/v1/schema.ts:276-286`
When both are present, `Date.parse(end_at) < Date.parse(start_at)` is an error.
Equal instants are allowed. An unreadable end passes (NaN comparison).
Message `Provider reporting period end cannot precede its start`, path `["end_at"]`.
Triggers: `ProviderReportingPeriod#rule:end_before_start`, `ProviderReportingPeriod#rule:end_one_ms_before_start`, `ProviderReportingPeriod#date_parse:date_only_end_before_start`.
Satisfies: `ProviderReportingPeriod#rule:end_equals_start`, `ProviderReportingPeriod#rule:end_without_start`, `ProviderReportingPeriod#date_parse:unparseable_end_passes`.

**R11. A provider UTC offset needs its meaning.** `src/v1/schema.ts:287-296`
When `timezone_offset_text` is present, `timezone_offset_meaning` must be present.
A meaning without text is allowed. Unlike R4, no text and no reason is allowed.
Message `A provider UTC offset needs its documented meaning`, path `["timezone_offset_meaning"]`.
Triggers: `ProviderReportingPeriod#rule:offset_without_meaning`.
Satisfies: `ProviderReportingPeriod#rule:meaning_without_offset`, `ProviderReportingPeriod#rule:offset_neither_text_nor_reason`.

**R12. An available provider offset cannot also be marked missing.** `src/v1/schema.ts:297-306`
`timezone_offset_text` and `timezone_offset_missing_reason` cannot both be present.
Message `An available provider UTC offset cannot also be marked missing`, path `["timezone_offset_missing_reason"]`.
Triggers: `ProviderReportingPeriod#rule:offset_and_missing_reason`.
Satisfies: `ProviderReportingPeriod#rule:offset_neither_text_nor_reason`.

### Source values

`validateSourceValue` in `src/v1/schema.ts:318-341`. It runs for `SourceValue`
(`:343-347`) and `SourceReportedSummary` (`:349-356`).

**R13. Give the input value or one reason.** `src/v1/schema.ts:327-333`
Exactly one of `input_value` and `input_value_missing_reason` is present.
Message `Provide the input value or one missing reason`, path `["input_value_missing_reason"]`.
Triggers: `SourceValue#rule:input_value_and_reason`, `SourceValue#rule:input_value_neither`, `SourceReportedSummary#rule:both_neither`.
Satisfies: `SourceValue#rule:mixed_value_present_unit_missing`, `SourceReportedSummary#rule:satisfied`.

**R14. Give the input unit or one reason.** `src/v1/schema.ts:334-340`
Exactly one of `input_unit` and `input_unit_missing_reason` is present.
Message `Provide the input unit or one missing reason`, path `["input_unit_missing_reason"]`.
Triggers: `SourceValue#rule:input_unit_and_reason`, `SourceValue#rule:input_unit_neither`, `SourceValue#order:empty_input_unit_still_runs_refinements`.
Satisfies: `SourceValue#rule:mixed_value_present_unit_missing`, `SourceReportedSummary#rule:satisfied`.

### Athlete profile

**R15. The default timezone must be a zone the runtime accepts.** `src/v1/schema.ts:415-422`, `:432`
`default_time_zone` passes when `new Intl.DateTimeFormat("en-US", { timeZone })`
does not throw. This depends on the ICU build. With ICU 78.2, names match without
regard to case, aliases pass, and UTC offsets such as `+05:00`, `+0530`, and `+05`
pass. `Z`, `Factory`, `Etc/GMT+13`, `+24:00`, and padded names fail. The output
keeps the input spelling. An empty string reports both `too_small` and R15.
Message `IANA timezone`, path `["default_time_zone"]`.
Triggers: `AthleteProfileData#time_zone:Not/AZone`, `AthleteProfileData#time_zone:Etc/Unknown`.
Satisfies: `AthleteProfileData#time_zone:UTC`, `AthleteProfileData#time_zone:America/Chicago`.
Every other `AthleteProfileData#time_zone:*` case records the observed verdict without a fixed expectation.

### Units and positive values

**R16. A point measurement uses its quantity's unit.** `src/v1/schema.ts:461-480`, `:542-549`
`value.standard_unit` must equal the unit in `POINT_MEASUREMENT_UNITS` for
`quantity`. The match is exact and case-sensitive.
Message `<quantity> uses <unit>`, path `["value", "standard_unit"]`.
Triggers: `PointMeasurementData#rule:unit:<quantity>:wrong` for every quantity, `PointMeasurementData#rule:unit:celsius_sign_character`, `PointMeasurementData#rule:unit:trailing_space`.
Satisfies: `PointMeasurementData#rule:unit:<quantity>:matches` for every quantity.

| quantity | unit | quantity | unit |
| --- | --- | --- | --- |
| hrv_rmssd | ms | body_temperature | °C (U+00B0 U+0043) |
| hrv_sdnn | ms | skin_temperature | °C |
| resting_heart_rate | bpm | wrist_temperature_sleeping | °C |
| body_weight | kg | temperature_deviation | °C |
| respiratory_rate | brpm | height | cm |
| vo2_max | ml/kg/min | lean_body_mass | kg |
| hr_recovery | bpm | body_fat_percentage | % |
| walking_heart_rate | bpm | blood_pressure_systolic | mmHg |
| oxygen_saturation | % | blood_pressure_diastolic | mmHg |

**R17. A point measurement value is positive.** `src/v1/schema.ts:550-556`
`value.standard_value` must be greater than 0, except for `temperature_deviation`.
`-0` fails.
Message `<quantity> must be positive`, path `["value", "standard_value"]`.
Triggers: `PointMeasurementData#rule:positive:zero`, `PointMeasurementData#rule:positive:negative_zero`, `PointMeasurementData#rule:unit_and_positive_together`.
Satisfies: `PointMeasurementData#rule:positive:smallest_positive`, `PointMeasurementData#rule:positive:temperature_deviation_negative`.

**R18. A measurement sample uses its quantity's unit.** `src/v1/schema.ts:488-513`, `:571-578`
`value.standard_unit` must equal the unit in `SERIES_QUANTITY_UNITS` for `quantity`.
Samples have no positive-value rule (`MeasurementSampleData#no_rule:negative_value`).
Message `<quantity> uses <unit>`, path `["value", "standard_unit"]`.
Triggers: `MeasurementSampleData#rule:unit:<quantity>:wrong` for every quantity.
Satisfies: `MeasurementSampleData#rule:unit:<quantity>:matches` for every quantity.

| quantity | unit | quantity | unit |
| --- | --- | --- | --- |
| heart_rate | bpm | running_vertical_oscillation | cm |
| hrv_beats | ms | running_ground_contact_time | ms |
| ecg_beats | ms | physical_effort | MET |
| hrv_sdnn | ms | basal_energy | kcal |
| respiratory_rate | brpm | exercise_time | min |
| oxygen_saturation | % | flights_climbed | count |
| steps | count | walking_speed | m/s |
| active_energy | kcal | walking_step_length | m |
| distance_walking_running | m | walking_asymmetry_percentage | % |
| distance_cycling | m | time_in_daylight | min |
| distance_swimming | m | running_speed | m/s |
| running_power | W | running_stride_length | m |

**R19. Only a reconstructed timestamp names its base and method.** `src/v1/schema.ts:579-592`
Let `reconstructed` mean `input_timestamp.timestamp_basis` is
`reconstructed_from_series_start`. Let `claimed` mean both `timestamp_base_text`
and `timestamp_reconstruction_method` are present. They must be equal. One field
alone on a non-reconstructed sample is therefore allowed.
Message `A reconstructed timestamp needs its base and method, and other timestamps cannot claim them`, path `["timestamp_reconstruction_method"]`.
Triggers: `MeasurementSampleData#rule:reconstructed:missing_base`, `MeasurementSampleData#rule:reconstructed:missing_method`, `MeasurementSampleData#rule:not_reconstructed:both_claimed`.
Satisfies: `MeasurementSampleData#rule:reconstructed:base_and_method`, `MeasurementSampleData#rule:not_reconstructed:base_only_allowed`, `MeasurementSampleData#rule:not_reconstructed:neither`.

### Session order

**R20. A sleep session cannot end before it starts.** `src/v1/schema.ts:608-611`
The rule passes only when `Date.parse(end_at) >= Date.parse(start_at)`. Equal
instants pass. If either side is unreadable, the rule fails (see "How Zod reports
issues", item 3).
Message `Sleep session end cannot precede its start`, path `["end_at"]`.
Triggers: `SleepSessionData#rule:end_before_start`, `SleepSessionData#date_parse:unparseable_end`, `SleepSessionData#date_parse:offset_end_compared_as_instant`.
Satisfies: `SleepSessionData#rule:end_equals_start`, `SleepSessionData#date_parse:bad_format_later_end`, `SleepSessionData#date_parse:wrong_type_end_skips_refinement`.

**R21. A workout session cannot end before it starts.** `src/v1/schema.ts:639-642`
Same comparison as R20.
Message `Workout session end cannot precede its start`, path `["end_at"]`.
Triggers: `WorkoutSessionData#rule:end_before_start`, `WorkoutSessionData#date_parse:unparseable_end`.
Satisfies: `WorkoutSessionData#rule:end_equals_start`, `WorkoutSessionData#rule:end_after_start`.

### Scores and anchors

**R22. A score has at least one value.** `src/v1/schema.ts:650-654`
At least one of `duration_s`, `reps`, or `weight_kg` is present. A present but
invalid value counts as present. The rule applies wherever `Score` appears.
Message `A score needs duration_s, reps, or weight_kg`, path `[]`.
Triggers: `Score#rule:empty`, `Score#order:unknown_key_still_runs_refinement`, `BenchmarkResultData#rule:empty_score`, `PredictionData#rule:empty_range_scores`, `GoalData#rule:empty_target_score`.
Satisfies: `Score#rule:reps`, `Score#rule:duration`, `Score#rule:weight`, `Score#order:zero_reps_counts_as_present`.

**R23. A provider summary needs an instant or a period.** `src/v1/schema.ts:681-690`
`recorded_at` is present, or `provider_reporting_periods` has at least one item.
Message `A provider summary needs an instant or a provider reporting period`, path `["provider_reporting_periods"]`.
Triggers: `ProviderSummaryData#rule:no_instant_no_period`, `ProviderSummaryData#rule:empty_period_list`.
Satisfies: `ProviderSummaryData#rule:period_without_instant`, `ProviderSummaryData#rule:instant_without_period`, `ProviderSummaryData#rule:invalid_period_counts_as_present`.

**R24. A provider score needs an instant or a period.** `src/v1/schema.ts:701-710`
Same rule as R23 for provider scores.
Message `A provider score needs an instant or a provider reporting period`, path `["provider_reporting_periods"]`.
Triggers: `ProviderScoreData#rule:no_instant_no_period`, `ProviderScoreData#rule:empty_period_list`.
Satisfies: `ProviderScoreData#rule:period_without_instant`, `ProviderScoreData#rule:instant_without_period`.

### Authored records

**R25. A rating needs its scale.** `src/v1/schema.ts:734-737`
When `rating` is present, `rating_scale` must be present. A scale without a rating
is allowed. An empty scale counts as present (it fails `too_small` instead).
Message `A rating needs its scale`, path `["rating_scale"]`.
Triggers: `AthleteReportData#rule:rating_without_scale`.
Satisfies: `AthleteReportData#rule:rating_with_scale`, `AthleteReportData#rule:scale_without_rating`, `AthleteReportData#order:empty_scale_still_runs_refinement`.

**R26. An analysis without causes is marked unexplained.** `src/v1/schema.ts:829-832`
`unexplained` is true, or `causes` has at least one item.
Message `An analysis without causes must mark the error unexplained`, path `["unexplained"]`.
Triggers: `PredictionErrorAnalysisData#rule:no_causes_not_unexplained`.
Satisfies: `PredictionErrorAnalysisData#rule:no_causes_unexplained`, `PredictionErrorAnalysisData#rule:causes_and_unexplained`.

**R27. A planned session needs a date or a start instant.** `src/v1/schema.ts:864-870`
At least one of `scheduled_date` and `scheduled_start_at` is present.
Message `A planned session needs a date or start instant`, path `["scheduled_date"]`.
Triggers: `PlannedSessionData#rule:no_date_no_start`.
Satisfies: `PlannedSessionData#rule:start_only`, `PlannedSessionData#rule:date_only`.

### Revisions and the two-tier wall in extensions

**R28. A revision names the revision before it.** `src/v1/schema.ts:1051-1060`
Revision 1 has `previous_revision` null. Revision n > 1 has `previous_revision`
n − 1. It applies to every `*Revision` schema.
Message `previous_revision must be null for revision 1 or the prior revision number`, path `["previous_revision"]`.
Triggers: `<Type>Revision#rule:revision_1_previous_1`, `#rule:revision_2_previous_null`, `#rule:revision_2_previous_2`, `#rule:revision_3_previous_1`, and `RecordRevision#rule:previous_revision_chain`.
Satisfies: `<Type>Revision#rule:revision_1_previous_null`, `#rule:revision_2_previous_1`, `#order:bad_previous_type_skips_chain_rule`.

**R29. Athlete-reported extensions cannot claim a source.** `src/v1/schema.ts:923-933`, `:1061-1071`; `src/v1/extensions.ts:44-51`, `:74-97`, `:212-227`
For `athlete_report`, `nutrition_entry`, `attachment`, and `athlete_profile`, no
object key named `source_id` may appear anywhere inside `extensions`. That includes
an extension ID that is exactly `source_id`. Only keys are checked, not string
values. Measured and authored types may carry `source_id` in extensions. Only the
first match is reported. It applies to `*Record` and `*Revision` variants.
Message `extensions cannot contain source_id`, path `["extensions"]`.
Triggers: `<AthleteReported>Record#two_tier:extension_nested_source_id`, `#two_tier:extension_deep_source_id_in_array`, `#two_tier:extension_source_id_as_extension_key`, `CurrentRecord#invalid:athlete_extension_source_id`.
Satisfies: every type's `#two_tier:extension_source_id_as_value_only`, every measured or authored type's `#two_tier:extension_nested_source_id`.
Known gap: the rule reads the parsed value. A `source_id` under a `__proto__` key is
dropped during parsing, so the record is accepted with that content removed
(`<Type>Record#two_tier:extension_source_id_hidden_under_proto`). The native port
should reject `__proto__` keys instead of copying this.

### Extension limits

`RecordExtensions` in `src/v1/extensions.ts:150-208`. Paths for R34 to R37 come from
splitting a dotted path string on `.`, `[`, and `]`. As a result, the extension ID
`org.example/vectors` becomes `["org", "example/vectors"]`, a key such as `x.y` becomes
`["x", "y"]`, and array indexes are strings (`"0"`). Zod's own structural issues for
the same value use the unsplit ID and numeric indexes.
See `RecordExtensions#path:dot_in_extension_id_splits_path`.

**R30. Each extension key is a namespaced extension ID.** `src/v1/extensions.ts:34-40`, `:165-174`
A key matches `^[a-z][a-z0-9]*(?:\.[a-z][a-z0-9]*)+\/[a-z][a-z0-9][a-z0-9._-]*$` and
has at most 200 characters. The name after the slash cannot contain another slash.
The `ath.fit/demo/widget` example in the source comment fails this pattern. One
issue is reported per bad key, in key order.
Message `extension key must be a namespaced extension id`, path `[<key>]`.
Triggers: `RecordExtensions#key:no_dot`, `RecordExtensions#key:second_slash`, `RecordExtensions#key:length_201`, `RecordExtensions#key:two_bad_keys_in_order`.
Satisfies: `RecordExtensions#key:valid`, `RecordExtensions#key:length_200`.

**R31. At most 32 extension keys.** `src/v1/extensions.ts:175-181`
More than 32 keys is an error. After this issue, Zod skips R32 to R37 for the
whole object. R30 issues are still reported first.
Message `extensions cannot contain more than 32 keys`, path `[]`.
Triggers: `RecordExtensions#count:33_keys`, `RecordExtensions#count:33_keys_with_bad_key`.
Satisfies: `RecordExtensions#count:32_keys`.

**R32. Encoded extensions are at most 65,536 bytes.** `src/v1/extensions.ts:182-188`
The UTF-8 length of `JSON.stringify(extensions)` must be at most 65,536. The count
uses JavaScript escaping. `"`, `\`, and `\b \f \n \r \t` take 2 bytes. Other
control characters below U+0020 become `\u00XX` (6 bytes). Lone surrogates become
`\uXXXX` (6 bytes). DEL and U+2028 stay raw (1 and 3 bytes).
Message `extensions exceed 65536 encoded bytes`, path `[]`.
Triggers: `RecordExtensions#bytes:total_65537`, `RecordExtensions#bytes:total_over_with_two_byte_chars`.
Satisfies: `RecordExtensions#bytes:total_65536`.

**R33. One encoded extension value is at most 16,384 bytes.** `src/v1/extensions.ts:189-197`
The UTF-8 length of `JSON.stringify(value)` for each extension must be at most
16,384. The escaping rules of R32 apply.
Message `extension value exceeds 16384 encoded bytes`, path `[<extension id>]` (not split).
Triggers: `RecordExtensions#bytes:value_16385`, `RecordExtensions#bytes:value_over_by_escaped_control_characters`, `RecordExtensions#bytes:value_over_by_escaped_lone_surrogates`.
Satisfies: `RecordExtensions#bytes:value_16384`, `RecordExtensions#bytes:value_with_raw_line_separators`.

**R34. Extension values nest at most 8 levels.** `src/v1/extensions.ts:99-108`
The extension value is depth 1. Each array element or object member is one level
deeper. Any value at depth 9 is an error, including an empty array, an empty
object, or null. The walk stops at the first value that is too deep, so a deeper
chain reports once.
Message `extension value exceeds depth 8`, path split as described above.
Triggers: `RecordExtensions#depth:9_arrays_scalar_at_depth_9`, `RecordExtensions#depth:9_empty_arrays`, `RecordExtensions#depth:9_empty_objects`, `RecordExtensions#depth:10_reports_once`.
Satisfies: `RecordExtensions#depth:8_arrays_scalar_at_depth_8`, `RecordExtensions#depth:8_empty_arrays`, `RecordExtensions#depth:8_empty_objects`.

**R35. Extension strings are at most 4,096 UTF-16 units.** `src/v1/extensions.ts:112-119`, `:152`
Two checks apply. The custom rule counts UTF-16 units (JavaScript `length`). Zod's
`z.string().max(4096)` counts code points and adds a `too_big` issue first. A string
of 4,097 units but at most 4,096 code points gets only the custom issue.
Message `extension string exceeds 4096 characters`, path split as described above.
Triggers: `RecordExtensions#string:4097_ascii` (both issues), `RecordExtensions#string:4097_utf16_units_with_emoji` (custom only), `RecordExtensions#string:2049_emoji_is_4098_units_2049_code_points` (custom only).
Satisfies: `RecordExtensions#string:4096_ascii`, `RecordExtensions#string:2048_emoji_is_4096_units`, `RecordExtensions#string:4096_two_byte_chars`.

**R36. Extension arrays have at most 256 items.** `src/v1/extensions.ts:124-130`, `:156`
Zod's structural `.max(256)` adds a `too_big` issue first, then the custom issue.
Message `extension array exceeds 256 elements`, path split as described above.
Triggers: `RecordExtensions#array:257`, `RecordExtensions#array:257_nested`.
Satisfies: `RecordExtensions#array:256`.

**R37. Extension objects have at most 64 keys.** `src/v1/extensions.ts:136-143`
Only the custom rule checks this. The top-level extension object is limited by R31
instead.
Message `extension object exceeds 64 keys`, path split as described above.
Triggers: `RecordExtensions#object:65_keys`, `RecordExtensions#object:65_keys_nested`.
Satisfies: `RecordExtensions#object:64_keys`, `RecordExtensions#count:33_keys_skips_later_checks` (skipped by R31).

### Complete record history

`validateCompleteRecordHistory` in `src/v1/schema.ts:1249-1398`. It runs inside
`AthleticStandardPackage` and `MeasurementSeriesFile`. Issues are reported in this
order: R38, then R39 to R41 per revision, then R42 and R43 per deletion, then
R44 to R49 per current record in first-seen order, then R50 and R51 per import key.
See `AthleticStandardPackage#history:every_issue_in_order`.

**R38. A current record appears once.** `src/v1/schema.ts:1258-1265`
A repeated `record_id` in `records` is an error. Later checks use the first copy.
Message `A package cannot contain the same current record twice`, path `["records", i, "record_id"]`.
Triggers: `AthleticStandardPackage#history:duplicate_current_record`, `AthleticStandardPackage#order:soft_error_still_runs_history`, `MeasurementSeriesFile#history:duplicate_current_record`.
Satisfies: `AthleticStandardPackage#history:all_satisfied`, `AthleticStandardPackage#order:structural_error_skips_history`.

**R39. A revision appears once.** `src/v1/schema.ts:1272-1279`
A repeated (`record_id`, `revision`) pair is an error.
Message `A package cannot contain the same record revision twice`, path `["record_revisions", i]`.
Triggers: `AthleticStandardPackage#history:duplicate_revision`.
Satisfies: `AthleticStandardPackage#history:all_satisfied`.

**R40. A revision has its current record.** `src/v1/schema.ts:1281-1287`
Every revision's `record_id` must match a current record.
Message `A retained revision needs its current record`, path `["record_revisions", i, "record_id"]`.
Triggers: `AthleticStandardPackage#history:revision_without_current_record`, `MeasurementSeriesFile#history:revision_without_record`.
Satisfies: `AthleticStandardPackage#history:all_satisfied`.

**R41. A revision keeps its record type.** `src/v1/schema.ts:1288-1294`
A revision's `record_type` must equal its current record's type.
Message `A revision must keep its record type`, path `["record_revisions", i, "record_type"]`.
Triggers: `AthleticStandardPackage#history:revision_changes_record_type`.
Satisfies: `AthleticStandardPackage#history:all_satisfied`.

**R42. A deletion has its current record.** `src/v1/schema.ts:1302-1309`
Message `A deletion record needs its retained current record`, path `["record_deletions", i, "record_id"]`.
Triggers: `AthleticStandardPackage#history:deletion_without_current_record`.
Satisfies: `AthleticStandardPackage#history:all_satisfied`.

**R43. A record has at most one deletion.** `src/v1/schema.ts:1310-1316`
Message `A record can have only one current deletion record`, path `["record_deletions", i, "record_id"]`.
Triggers: `AthleticStandardPackage#history:two_deletions_for_one_record`.
Satisfies: `AthleticStandardPackage#history:all_satisfied`.

**R44. Every revision through the current one is present.** `src/v1/schema.ts:1324-1330`
The number of revisions for a record must equal `current_revision`.
Message `A complete package needs every revision through current_revision`, path `["records", i, "current_revision"]`.
Triggers: `AthleticStandardPackage#history:missing_revision`, `AthleticStandardPackage#history:no_revisions_at_all`.
Satisfies: `AthleticStandardPackage#history:all_satisfied`, `AthleticStandardPackage#history:revisions_listed_out_of_order`.

**R45. Revision numbers run 1, 2, 3 without gaps.** `src/v1/schema.ts:1331-1340`
After sorting a record's revisions by number, the k-th must be k. One issue is
reported per position that differs. Array order does not matter.
Message `Record revision numbers must be complete and consecutive`, path `["records", i, "current_revision"]`.
Triggers: `AthleticStandardPackage#history:revision_gap`.
Satisfies: `AthleticStandardPackage#history:revisions_listed_out_of_order`.

**R46. The current record equals its latest revision.** `src/v1/schema.ts:1341-1352`
The highest revision number must equal `current_revision`. Its `data` must produce
the same `JSON.stringify` text as the current record's `data`. The comparison uses
the parsed output. Schema keys are therefore reordered, but keys inside
record-valued fields keep their order. So `{"a":1,"b":2}` and `{"b":2,"a":1}` in
supplied fields differ. `extensions` is not compared.
Message `The current record must equal its current retained revision`, path `["records", i, "data"]`.
Triggers: `AthleticStandardPackage#history:current_data_differs`, `AthleticStandardPackage#history:current_data_key_order_differs`.
Satisfies: `AthleticStandardPackage#history:shape_key_order_normalized`, `AthleticStandardPackage#history:integer_like_key_order_normalized`, `AthleticStandardPackage#history:number_spelling_normalized`, `AthleticStandardPackage#history:extensions_not_compared`.

**R47. A deleted record has its deletion.** `src/v1/schema.ts:1355-1361`
Message `A deleted current record needs its deletion record`, path `["records", i, "is_deleted"]`.
Triggers: `AthleticStandardPackage#history:deleted_without_deletion`.
Satisfies: `AthleticStandardPackage#history:all_satisfied`.

**R48. A deletion requires `is_deleted`.** `src/v1/schema.ts:1362-1368`
Message `A current deletion record requires is_deleted`, path `["records", i, "is_deleted"]`.
Triggers: `AthleticStandardPackage#history:deletion_without_is_deleted`.
Satisfies: `AthleticStandardPackage#history:all_satisfied`.

**R49. A deletion names the current revision.** `src/v1/schema.ts:1369-1375`
`record_revision` on the deletion must equal `current_revision`. When a record has
two deletions, the last one is compared.
Message `A deletion record must name the current retained revision`, path `["records", i, "current_revision"]`.
Triggers: `AthleticStandardPackage#history:deletion_names_old_revision`.
Satisfies: `AthleticStandardPackage#history:all_satisfied`.

**R50. An import key has its current record.** `src/v1/schema.ts:1380-1386`
Message `A provider import key needs its current record`, path `["record_import_keys", i, "record_id"]`.
Triggers: `AthleticStandardPackage#history:import_key_without_record`.
Satisfies: `AthleticStandardPackage#history:import_key_repeated_for_same_record`.

**R51. One import key identifies one record.** `src/v1/schema.ts:1387-1395`
Two import keys with the same `match_key_method` and `match_key` must name the same
record. A repeat for the same record is allowed. The pair is joined with U+0000
before comparison, so method `a\0b` with key `c` collides with method `a` and key
`b\0c`. The native port should compare the pair, not the joined text.
Message `One provider import key cannot identify two records`, path `["record_import_keys", i, "match_key"]`.
Triggers: `AthleticStandardPackage#history:import_key_two_records`, `AthleticStandardPackage#history:import_key_nul_separator_collision`, `MeasurementSeriesFile#history:import_key_two_records`.
Satisfies: `AthleticStandardPackage#history:import_key_repeated_for_same_record`, `AthleticStandardPackage#history:import_key_same_text_other_method`.

### Series files and packages

**R52. A series file cannot end before it starts.** `src/v1/schema.ts:1420-1426`
`Date.parse(end_at) < Date.parse(start_at)` is an error. An unreadable end passes.
Message `Series file end cannot precede its start`, path `["end_at"]`.
Triggers: `MeasurementSeriesFile#rule:end_before_start`.
Satisfies: `MeasurementSeriesFile#rule:end_equals_start`, `MeasurementSeriesFile#date_parse:unparseable_end_passes`.

**R53. Every sample matches the series source and quantity.** `src/v1/schema.ts:1428-1434`
Each current record's `data.source_id` and `data.quantity` equal the file's.
Revisions are not checked.
Message `Every sample must match the series source and quantity`, path `["records", i]`.
Triggers: `MeasurementSeriesFile#rule:sample_source_mismatch`, `MeasurementSeriesFile#rule:sample_quantity_mismatch`.
Satisfies: `MeasurementSeriesFile#rule:sample_matches`.

**R54. Every current sample falls inside the series bounds.** `src/v1/schema.ts:1435-1442`
`recorded_at` must be at or after `start_at` and at or before `end_at`. Deleted
samples are included. Older revisions are not checked. An unreadable sample time
passes.
Message `Every current sample must fall inside the series time bounds`, path `["records", i, "data", "recorded_at"]`.
Triggers: `MeasurementSeriesFile#rule:sample_before_start`, `MeasurementSeriesFile#rule:sample_after_end`, `MeasurementSeriesFile#rule:deleted_sample_outside_bounds`.
Satisfies: `MeasurementSeriesFile#rule:sample_at_end`, `MeasurementSeriesFile#rule:older_revision_outside_bounds_allowed`.

**R55. A package references each series file once.** `src/v1/schema.ts:1467-1477`
Two references with the same `file` text are an error. The comparison is exact, so
paths that differ only in letter case pass.
Message `A package cannot reference the same measurement-series file twice`, path `["measurement_series_files", i, "file"]`.
Triggers: `AthleticStandardPackage#package:duplicate_series_file`.
Satisfies: `AthleticStandardPackage#package:two_series_files`, `AthleticStandardPackage#package:series_file_path_case_differs`.

**R56. An erased record is not a current record.** `src/v1/schema.ts:1479-1488`
No erasure marker may name a `record_id` in `records`. Repeated markers for one ID
are allowed (`AthleticStandardPackage#package:duplicate_erasure_markers_allowed`).
Message `An erased record cannot remain in current package records`, path `["record_erasure_markers", i, "record_id"]`.
Triggers: `AthleticStandardPackage#package:erasure_marker_matches_current_record`.
Satisfies: `AthleticStandardPackage#package:erasure_marker_for_absent_record`.

### Rules that are absent

The vectors also show checks that do not exist. The port must not add them while
matching 2.1.0 behavior.

- A source device's coverage dates, a training plan's dates, and a review period may run backwards (`*#no_rule:end_before_start`).
- A workout segment's end offset may precede its start offset (`WorkoutSessionSegment#no_rule:end_offset_before_start_offset`). `ath check` reports it later (S43).
- A predicted range may have `low` above `high` (`PredictionData#no_rule:range_low_above_high`).
- `asleep_seconds` may exceed `in_bed_seconds` (`SleepSessionData#no_rule:asleep_exceeds_in_bed`).
- Route points may fall outside the workout (`WorkoutSessionData#no_rule:route_point_outside_session`).
- A series file reference's times and counts are not checked by Zod (`MeasurementSeriesFileReference#no_rule:*`). Package import checks the counts (S27).
- `extensionValueDepth` in `src/v1/extensions.ts:57-72` is never called.

## Part 2: rules enforced outside Zod

These rules run in the database layer. They are listed so the native store keeps
them. Each line gives the location, the rule, the exact message, and when it runs.
Most write-time checks already run in Rust: `writeRecords`, `writeRecordChanges`,
`correctRecord`, `deleteRecord`, and `importProviderRecords` send actions to
`crates/ath-core/src/persist.rs` through `runAthCore` (`src/db/records.ts:81-95`).

### Writes, corrections, and deletions

- **S1.** `src/db/records.ts:97-103`. Every stored value must serialize to JSON. "Record value cannot be serialized as JSON". Write, package import, repair.
- **S2.** `src/db/records.ts:935-937`. A write needs at least one record. "writeRecords needs at least one record". Write.
- **S3.** `src/db/records.ts:955-963`. A new record starts at revision 1, not deleted. `became_known_at` defaults to `created_at`; the basis defaults to `local_write`. No message. Write.
- **S4.** `src/db/records.ts:940-946`, `:993-999`; `persist.rs:90`, `:93`. An expected current revision must exist, match, and not be deleted. "Expected record does not exist: {record_id}" / "Record {record_id} changed after preview; preview it again". Write.
- **S5.** `src/db/records.ts:988-990`. A change set needs at least one change. "writeRecordChanges needs at least one change". Grade, attachment correction.
- **S6.** `src/db/records.ts:1008-1015`, `:1022`. Records created through `writeRecordChanges` drop their extensions. No message. Grade.
- **S7.** `src/db/records.ts:1053-1058`, `:1131-1140`; `persist.rs:146-158`. A correction target must exist and be at the expected revision. "Record not found: ${recordId}" / "Record ${recordId} changed after preview; preview it again" / "Record ${recordId} is at revision ${currentRevision}, not expected revision ${expected}". Write.
- **S8.** `src/db/records.ts:1141-1143`; `persist.rs:159-161`. A deleted record cannot be corrected. "Deleted record cannot be corrected: ${recordId}". Write.
- **S9.** `src/db/records.ts:1059-1061`, `:1144-1148`; `persist.rs:163-165`. A new revision's `became_known_at` cannot precede the current revision's. "Revision knowledge time cannot precede revision ${currentRevision}". Write.
- **S10.** `src/db/records.ts:1062-1069`, `:1149-1157`. A correction copies `record_type` and `created_at` from the stored row, so they cannot change. Extensions carry over when omitted. No message. Write.
- **S11.** `persist.rs:210-221` (called from `src/db/records.ts:1178-1200`). A deletion needs an existing, undeleted record at the expected revision, and `deleted_at` cannot precede the current knowledge time. "Record not found: {record_id}" / "Record {record_id} is at revision {current}, not expected revision {expected}" / "Record is already deleted: {record_id}" / "Deletion time cannot precede the current revision knowledge time". Delete.
- **S12.** `persist.rs:69-76`. The database revision must equal the preview's. "Database changed after provider import preview; preview it again". Provider import.
- **S13.** `persist.rs:233-258`. An import key that already names another record fails; the same record is a no-op. "Provider import key changed after preview". Provider import.

### Relationships between records

`validateTypedRelationships` in `src/db/records.ts:818-867` runs on package restore
and merge (`src/db/import-package.ts:1303-1305`, `:1537-1542`) and on `ath check` for
undeleted links and grades only (`src/db/check.ts:512-532`). `persist.rs:568-648` runs
the same checks on writes.

- **S14.** `src/db/records.ts:750-753`; `persist.rs:681`. A referenced (record, revision) must exist. "Referenced record revision does not exist: ${recordId}@${revision}".
- **S15.** `src/db/records.ts:754-758`; `persist.rs:684`. The referenced revision must have the expected type. "Referenced ${recordId}@${revision} is ${type}, not ${expectedRecordType}".
- **S16.** `src/db/records.ts:808-810`; `persist.rs:666`. A link's prediction and benchmark result use the same benchmark definition. "Prediction and benchmark result use different benchmark definitions".
- **S17.** `src/db/records.ts:826-834`. A grade's link must be an existing `prediction_result_link` revision; its prediction and result are then checked by S14 to S16.
- **S18.** `src/db/records.ts:835-837`; `persist.rs:608-610`. A grade's `predicted_score` equals the linked prediction revision's, by `JSON.stringify` (key order matters). "Saved grade predicted score does not match the linked prediction revision".
- **S19.** `src/db/records.ts:838-840`; `persist.rs:611-613`. A grade's `actual_score` equals the linked result revision's `score`. "Saved grade actual score does not match the linked result revision".
- **S20.** `src/db/records.ts:842-849`; `persist.rs:626-628`. The benchmark definition must have a current typed row. Its revision and deletion state are not checked. "Linked benchmark definition does not exist".
- **S21.** `src/db/records.ts:776-778`; `persist.rs:689-700`. A score holds the field for the score type: time uses `duration_s`, reps uses `reps`, others use `weight_kg`. "Score does not contain the benchmark's ${scoreType} value".
- **S22.** `src/db/records.ts:850-866`; `persist.rs:629-646`. Grade arithmetic: `signed_error = round((predicted − actual) × 100) / 100`; `absolute_error_percentage = round(|signed| / actual × 1000) / 10`; `result_in_predicted_range` is true only with a range and `low ≤ actual ≤ high`. "Saved grade arithmetic does not match its linked revisions".

### Logging

- **S23.** `src/db/log-entry.ts:71-85`. Plain-text measurements and results use the earliest undeleted `manual` source. "manual source is missing; run ath check".
- **S24.** `src/db/log-entry.ts:298-302`. Bare "HRV" is refused. "HRV is ambiguous. Say RMSSD or SDNN so the two statistics never share a baseline."
- **S25.** `src/db/log-entry.ts:316-320`. A plain-text unit must equal the standard unit; nothing is converted. "${quantity} must be logged in ${expectedUnit}; '${inputUnit}' is not converted by plain-text log".
- **S26.** `src/db/log-entry.ts:499-530`. A structured log is a non-empty array of objects with only `record_type`, `record_id`, `created_at`, `current_revision`, `is_deleted`, `data`, and `extensions`. It cannot create links or grades, a revision other than 1, or a deleted record. Messages: "structured log array cannot be empty", "structured log item ${n} must be an object", "structured log item ${n} has unknown field[s]: ...", "structured log cannot create ${recordType}; use ath grade so the tool validates and writes it", "structured log item ${n} can only create revision 1", "structured log item ${n} cannot create a deleted record".
- Other log rules: an empty text ("nothing to log", `:295-297`), an unusable benchmark name (`:114-123`), a score without its type's field (`:140-142`), a missing definition during automatic grading (`:204-207`), invalid JSON input (`:586-590`). `ath log` sets `field_basis.value` to `entered_by_hand` on measured types (`:485-491`). Unknown benchmark keys create a custom time-scored definition (`:361-383`).

### Package import, restore, and merge

`AthleticStandardPackage.safeParse` runs first (`src/db/import-package.ts:322`), so
R38 to R56 apply. The following checks are in addition.

- **S27.** `src/db/import-package.ts:405-426`. A series file's SHA-256 must equal its reference, and its `source_id`, `quantity`, and counts must equal the reference. "series hash does not match package reference: ${file}" / "series counts or grouping do not match: ${file}". The reference's `start_at` and `end_at` are not compared with the file.
- **S28.** `src/db/import-package.ts:427-445`. Within one source and quantity, sorted files cannot overlap (string comparison), and a sample at a shared boundary cannot appear in both files. "measurement-series time ranges overlap: ${file}" / "package repeats record ID across series files: ${record_id}".
- **S29.** `src/db/import-package.ts:297-321`. 0.x packages are refused, and the version must be `2.0.0` or `2.1.0`. Zod alone accepts `1.0.0` (`AthleticStandardPackage#version:1.0.0_accepted`). "Athletic Standard ${version} is a prerelease JSON format. v1 does not import 0.x files." / "...athleticstandard_version: ${version} is not supported by 2.1.0".
- **S30.** `src/db/import-package.ts:203-268`. JSON nesting is at most 128 levels, a document is at most 512 MiB and a regular file, and referenced files stay inside the package directory. "package JSON exceeds nesting limit ${n}: ${path}" / "package path is not a regular file: ${path}" / "package JSON exceeds ${n} bytes: ${path}" / "package file escapes its directory: ${file}".
- **S31.** `src/db/import-package.ts:330-332`, `:354-356`, `:460-477`. Limits: 10,000 series files, 10,002 manifest files, 100,000,000 record and history objects.
- **S32.** `src/db/import-package.ts:334-370`. The manifest is optional. When present, its database ID and revision must match the package, and every listed file must match its size and hash. "export manifest does not match the package database" / "package file does not match its manifest: ${file}".
- **S33.** `src/db/import-package.ts:479-506`. Every undeleted attachment needs its file, a matching media type, size, and hash, and a manifest entry when a manifest exists. "attachment does not match record ${record_id}" / "attachment is missing from the manifest: ${path}".
- **S34.** `src/db/import-package.ts:1000-1009`. A series batch holds only `measurement_sample` records. "measurement-series file contains a non-sample record".
- **S35.** `src/db/import-package.ts:758-768`. Restore happens only into an empty database at revision 0. Otherwise the database IDs must match. Conflict "destination database ${id} differs from package ${id}; live databases are not merged".
- **S36.** `src/db/import-package.ts:709-741`. In a merge, local revisions must be an exact prefix of the incoming ones (`JSON.stringify`, key order matters). Conflicts "record revision history differs" / "current record does not match its imported revision history".
- **S37.** `src/db/import-package.ts:1220-1268`, `:1328-1340`, `:1451-1457`. Every conflict has exactly one matching resolution, and the destination and package must be unchanged since preview. "conflict has more than one resolution: ${id}" / "import has unresolved conflicts or unsupported records" / "resolution does not match an import conflict: ${id}" / "destination database changed after preview" / "package conflict changed after preview" / "package records changed after preview".
- **S38.** `src/db/import-package.ts:1490-1506`. A resolution cannot change record type or deleted state, and cannot predate the current revision. "conflict cannot change record type or deleted state: ${id}" / "conflict resolution time precedes the current revision: ${id}".
- **S39.** `src/db/import-package.ts:1429-1449`, `:1543-1560`. A merged import key cannot name a different record, and an existing erasure marker must keep its time and reason. "provider import key changed after preview" / "erasure marker changed after preview".
- **S40.** `src/db/import-package.ts:1191-1196`. An existing local attachment file must have the same hash. "local attachment conflicts: ${id}".
- **S41.** `src/db/conflicts.ts:152-165`. A saved conflict set must still match the package and database. "the package or database changed after the conflict preview; run ath import again".

### `ath check` and typed rows

All findings are reported, not thrown.

- **S42.** `src/db/check.ts:137-192`. Foreign keys are on, metadata and `user_version` agree with schema version 4, `application_id` is `0x41544831`, and `integrity_check` and `foreign_key_check` are clean.
- **S43.** `src/db/check.ts:267-295`. A workout segment cannot end before it starts or fall outside the session length. "segment ${n} ends before it starts" / "segment ${n} falls outside the workout session".
- **S44.** `src/db/check.ts:344-384`. Stored revisions run 1..n, chain correctly, keep the record type, and end at `current_revision`. "revision chain breaks at stored revision ${revision}; expected ${expected} after ${previous}".
- **S45.** `src/db/check.ts:405-422`. `is_deleted` is 1 exactly when a deletion row exists. "records.is_deleted does not match record_deletions".
- **S46.** `src/db/check.ts:443-456`. An erased ID has no `records` row. "erased record still has a current record row".
- **S47.** `src/db/check.ts:194-203`. Two-tier wall by table shape: `athlete_reports`, `nutrition_entries`, and `reported_nutrition_items` have no `source_id` column. "${table} must not have a source_id column".
- **S48.** `src/db/check.ts:458-510`; `src/db/typed-denormalized.ts:54-411`. Typed rows and their child rows match the current revision, and typed `data_json` is byte-equal to the revision's `record_json`. "typed-table denormalized columns do not match the current revision" / "${recordType} is missing its ${table} row" / "${table}.data_json does not match the current revision".
- **S49.** `src/db/check.ts:264-266`, `:70-119`; `src/db/attachments.ts:10-33`. Undeleted attachment files exist and match size, hash, and media type. PNG, JPEG, WebP, and PDF are checked by magic bytes. Other `image/*` types are refused. "attachment bytes do not match supported media type ${media_type}: ${record_id}".
- `ath check` does not verify deletion revisions, import keys, or knowledge-time order across stored revisions.

### SQL constraints and triggers

`src/db/migrations/001-initial.ts`, `003-version-2.ts`, and
`enforcement-triggers.ts`. Foreign keys are deferred to commit
(`src/db/transaction.ts:19-23`).

- **S50.** `001-initial.ts:27-59`. `record_type` is one of the 24 types, `current_revision ≥ 1`, `is_deleted` is 0 or 1, and the current revision must exist (deferred foreign key).
- **S51.** `001-initial.ts:64-92`. Revisions: `revision ≥ 1`, valid JSON, extensions are null or a JSON object with `length(extensions_json) ≤ 65536` (characters, not bytes), the basis is one of four values, the type matches the parent, the previous revision exists, and revision 1 has null previous while n has n − 1.
- **S52.** `001-initial.ts:96-121`. One deletion per record, naming an existing revision (not necessarily the current one). One erasure marker per record with a known reason. One record per (method, key) import key.
- **S53.** `001-initial.ts:142-143`, `:164-165`, `:471`. One `athlete_profile` row, one source per (writer, route, sensor type), and one benchmark definition per key. Deleted rows count.
- **S54.** `001-initial.ts:171-379` (source columns). Two-tier wall: every measured table has `source_id NOT NULL` referencing `sources`. Whether the source is deleted is not checked.
- **S55.** `001-initial.ts:271`, `:292`. Sleep and workout sessions need `end_at >= start_at` by text comparison.
- **S56.** `001-initial.ts:323-706`. Foreign keys from reference fields to their typed tables and to `record_revisions` for evidence (benchmark definition, workout session, prediction, result, link, grade, training plan, nutrition entry, attachment links, calculation inputs, error causes, authored evidence). Nutrition interpretation `confidence` is in [0, 1]. An attachment can be linked to one owner once.
- **S57.** `enforcement-triggers.ts:8-32`. A revision's type matches its parent. "record_revisions.record_type must match records.record_type".
- **S58.** `enforcement-triggers.ts:34-44`. An updated `current_revision` must exist. "records.current_revision must reference an existing revision".
- **S59.** `enforcement-triggers.ts:46-62`. `athlete_report` and `nutrition_entry` data has no top-level `source_id`. "athlete-reported record data cannot contain source_id".
- **S60.** `enforcement-triggers.ts:64-90`. The four athlete-reported types have no `source_id` key at any depth of `extensions_json` (`json_tree`). "athlete-reported record extensions cannot contain source_id". This matches R29, except that it reads stored text, so content Zod dropped under `__proto__` never reaches it.
- **S61.** `enforcement-triggers.ts:92-106`. An updated `is_deleted` matches the deletion rows. "records.is_deleted must match record_deletions".
- **S62.** `enforcement-triggers.ts:108-126`. An erased ID cannot be inserted or updated in `records`. "erased records cannot have a current records row".
- **S63.** `enforcement-triggers.ts:128-184`. Typed-row types for four tables, `source_id` on point measurements, and existing revisions for a new link. "<table>.record_type must be <type>" / "measured records require source_id" / "prediction_result_links must reference existing revisions".

### Erasure, provider import, repair, and opening

- **S64.** `src/db/erase.ts:47-108`. Erasure cascades through every record that depends on the root by reference, including deleted ones.
- **S65.** `src/db/erase.ts:138-158`, `:254-278`. The expected revision is a positive whole number, the root exists at that revision, and the database and dependency set are unchanged since preview. "Expected revision must be a positive whole number" / "Record was already erased: ${id}" / "Record not found: ${id}" / "Record ${id} is at revision ${current}, not expected revision ${expected}" / "Database changed after the erasure preview; preview it again" / "Record changed after the erasure preview; preview it again" / "Erasure dependencies changed after preview; preview it again".
- **S66.** `src/db/provider-import.ts:81-87`, `:381-387`, `:446-448`. A provider timestamp is an absolute instant, a date-only record has a reporting period, and a summary has an anchor. "Provider timestamp is not an absolute instant: ${timestamp}" / "A date-only provider record needs a reporting period" / "A provider summary needs a source time, date, or record ID".
- **S67.** `src/db/provider-import.ts:1066-1127`. Import keys are unique within an export, point to an undeleted record of the same type, and a changed duplicate group is a conflict. Conflicts "provider export repeats one import key" / "provider export gives one import key two different records" / "provider import key points to a normally deleted record" / "provider import key points to ${type}, not ${record_type}" / "an indistinguishable duplicate group changed; the importer will not guess which row was corrected".
- **S68.** `src/db/repair.ts:133-142`, `:241-247`. Without `--from-revision`, repair fixes only key-order differences between typed rows and revisions.
- **S69.** `src/db/migrate.ts:68-101`, `:426-508`; `src/db/client.ts:63-74`. Opening needs one metadata row with marker `ATH1`, version 1.0.0, 2.0.0, or 2.1.0, a matching `user_version`, schema version at most 4, and foreign keys on.
- **S70.** `src/db/attachment-ingest.ts:76-98`, `:136-140`. A logged attachment is a regular file of at most 25 MiB whose magic bytes show PNG, JPEG, WebP, or PDF. Exactly one athlete report or nutrition entry in the batch owns it.

### Rust-only rules in `crates/ath-core`

These run for CLI commands that call the native core directly
(`crates/ath-core/src/store.rs:128-130`, `:255-257`, `:381-383`).

- **S71.** `store.rs:828-899` (`validate_workout`). A workout cannot end before it starts, needs a known `received_at_basis`, a `source_id`, and `source_reported_summaries`, a non-empty name, a positive maximum heart rate, route coordinates in range, and positive heart-rate samples with non-negative offsets. Messages include "workout end is before the start", "route point ${n} latitude is outside -90 to 90", and "heart rate sample ${n} is not a positive number".
- **S72.** `store.rs:901-926` (`check_supplied_fields`). Supplied fields, including route-point supplied fields, are an object with no empty name and no `__proto__` key. "${label}: a supplied field has an empty name, and the format needs a name for every field" / "${label}: a supplied field is named __proto__, which JavaScript readers drop".

## Gaps found while building the vectors

1. **The `__proto__` key hides content.** Zod drops a `__proto__` key and its content
   from record-valued fields. That hides a `source_id` from R29 and silently
   changes supplied fields. S72 already refuses such names in the native core. The
   native validator should refuse them everywhere.
2. **Time-order rules read malformed values.** R20 and R21 fail on unreadable text,
   while R10, R52, and R54 pass it. The port must either copy `Date.parse` or
   record a decision to report only the format issue.
3. **Extension issue paths are split on dots.** R34 to R37 paths do not match the
   key structure. The port must copy the split to match the issue lists.
4. **The import-key separator can collide.** R51 joins method and key with U+0000.
5. **Two length measures apply to extension strings.** Zod counts code points; R35
   counts UTF-16 units. SQLite `length()` in S51 counts characters, while R32
   counts UTF-8 bytes.
6. **History equality depends on key order.** R46, S18, and S36 compare
   `JSON.stringify` text. Key order inside supplied fields therefore matters.
7. **Zod accepts version 1.0.0 packages.** Only S29 refuses them on import.
8. **A merge of a deleted record may fail.** The database survey found that a
   merge updates `is_deleted` (`src/db/import-package.ts:1398-1409`) before it
   inserts the deletion row (`:1414-1428`). The S61 trigger would then abort. No
   test covers this, and it was not reproduced here.
