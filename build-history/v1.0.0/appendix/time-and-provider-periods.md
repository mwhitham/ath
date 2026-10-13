# Time and provider periods

Research checked on 2026-10-12 for D88.

## Question

Wearable data contains several different time facts:

- An instant on the UTC timeline.
- A start and end interval.
- A numeric offset printed beside a timestamp.
- A named timezone.
- A date assigned by a provider.
- A provider period that does not follow midnight.
- The time the database first learned a value.

The v1.0.0 model must keep those facts without turning one into another.

## Findings

### Apple Health and HealthKit

Apple defines Foundation `Date` as a point in time independent of a calendar or
timezone. HealthKit sample start and end values are `Date` values.

Sources:

- [Foundation Date](https://developer.apple.com/documentation/foundation/date)
- [HKSample startDate](https://developer.apple.com/documentation/healthkit/hksample/startdate)
- [HKSample endDate](https://developer.apple.com/documentation/healthkit/hksample/enddate)

Apple defines `HKMetadataKeyTimeZone` as:

> The user's time zone when the HealthKit object was created.

It accepts a named timezone. The metadata dictionary is optional. The wording does
not say the object was measured in that timezone or that the timezone applied at the
sample's start.

Source:
[HKMetadataKeyTimeZone](https://developer.apple.com/documentation/healthkit/hkmetadatakeytimezone).

Apple activity summaries use date components that identify the day perceived by the
user. Apple says that day may be longer or shorter than 24 hours after travel.

Source:
[predicateForActivitySummary(with:)](https://developer.apple.com/documentation/healthkit/hkquery/predicateforactivitysummary(with:)).

Apple documents that Health data can be exported as XML. It does not document the
timezone meaning of the offset printed in an XML timestamp.

Source:
[Share your health data](https://support.apple.com/guide/iphone/share-your-health-data-iph5ede58c3d/ios).

Independent repeated-export tests have observed the same instant rendered with
different offsets after the export timezone changed. Open Wearables also reports
that ordinary HealthKit samples usually lack timezone metadata. These are useful
observations, not Apple guarantees.

Sources:

- [Repeated export analysis](https://www.johngoldin.com/blog/apple-health-export/2020-02-apple-health-export1/index.en.html)
- [Open Wearables timezone findings](https://github.com/the-momentum/open-wearables/discussions/606)

Consequences:

- The XML timestamp can be parsed as an instant.
- Its exact source text and offset must be preserved.
- The offset must not be described as the recording location or recording timezone
  without route documentation that establishes that meaning.
- An Apple activity-summary date should be stored as Apple's date, not reconstructed
  from sample timestamps.

The v0.4.3 importer does not follow all of those rules:

- `src/import/apple.ts` calls the first ten timestamp characters the local day.
- It says Apple dates begin in the wearer's own timezone.
- It ignores string metadata such as `HKTimeZone`.
- It does not import `ActivitySummary` elements.

Those statements and omissions are implementation facts. They are not retained as
v1.0.0 format rules.

### WHOOP

WHOOP says:

> We always reference a member's activity in the context of a Physiological Cycle
> (known as Cycle for short) rather than calendar days.

A current cycle has a start and no end. A past cycle has both. The API defines
`timezone_offset` as the user's offset at the time the cycle was recorded. It does
not say that one offset applies to both interval endpoints.

Sources:

- [WHOOP Cycle](https://developer.whoop.com/docs/developing/user-data/cycle/)
- [WHOOP Sleep](https://developer.whoop.com/docs/developing/user-data/sleep)
- [WHOOP Workout](https://developer.whoop.com/docs/developing/user-data/workout)

WHOOP does not publish a complete contract for the dashboard CSV columns used by the
current importer. Real exports show local timestamp columns and a `Cycle timezone`
column. That observed layout must be preserved, but undocumented meaning must not be
added to it.

Consequences:

- Store the cycle ID and bounds when supplied.
- Keep a cycle distinct from a calendar day.
- Preserve the offset field with WHOOP's documented wording.
- Do not assume one cycle offset safely reconstructs both endpoints across travel or
  a daylight-saving change.

The v0.4.3 importer applies one cycle offset to both sleep and workout endpoints. It
also describes cycle end as wake. WHOOP's cycle documentation does not support that
description.

### Oura

Oura defines three day types:

- Activity Day: 04:00 to 04:00.
- Sleep Day: 18:00 to 18:00.
- Calendar Day: local midnight to midnight in the phone's timezone.

Source:
[Understanding the Different Types of Oura Days](https://partnersupport.ouraring.com/hc/en-us/articles/29160913203219-Understanding-the-Different-Types-of-Oura-Days-in-Oura-API-Data).

Oura API records carry a `day` assigned by Oura. Sleep records also carry localized
bedtime start and end values. The date and timestamps are separate source facts.

Source:
[Oura API v2 schema](https://cloud.ouraring.com/v2/static/json/openapi-1.41.json).

The importer maps a day by the route that supplied it:

- `daily_activity.day` and activity-score CSV columns use `oura_activity_day`.
  Oura's API schema describes daily activity as a 24-hour period starting at 04:00.
- `daily_sleep.day`, sleep records, and sleep-score CSV columns use
  `oura_sleep_day`.
- A route that expressly documents midnight-to-midnight use may use
  `oura_calendar_day`.
- `daily_readiness.day` and any other Oura date whose day type is not stated by the
  route use `oura_day_unspecified`. They are not guessed to be calendar days.

A combined Trends CSV can therefore produce several period records from one row.
Each metric uses its own category. The importer does not assign one day type to the
whole row.

Consequences:

- Preserve `day` and its Oura day kind.
- Preserve bedtime timestamps separately.
- Do not derive the Oura day from bedtime.
- Do not invent an instant when a downloaded row carries only a date.

The v0.4.3 importer reads `day` but may replace it with bedtime end. When no bedtime
exists, it creates `${day}T00:00:00Z` and describes that as local-day start. UTC
midnight is not an Oura Activity Day or Sleep Day boundary.

## What is documented and what is chosen

The provider sections above report source behavior. The following are Athletic
Standard design choices:

1. Preserve every source time, date, offset, timezone, and period field that the
   route exposes.
2. Normalize an absolute instant or interval when the source provides enough
   information.
3. Keep provider dates and periods separately from instants.
4. Do not create an instant for a date-only record.
5. Use an explicit IANA timezone for a midnight-to-midnight calendar calculation.
6. Allow the athlete profile timezone as the chosen default for that calculation.
7. Refuse a calendar calculation when no timezone was supplied or saved.
8. State the timezone, provider-period kind, exclusions, and unknown time facts in
   the calculation receipt.

The profile timezone is a preference. It is not historical travel evidence. A caller
may choose another timezone for a calculation without changing the stored source
facts.

## Record examples

### Oura activity after midnight

An activity at 02:00 on January 1 may belong to Oura Activity Day December 31.
Athletic Standard keeps both:

- Instant: January 1 at 02:00 with its source timestamp.
- Provider period: Oura Activity Day December 31.

A calendar calculation in a named timezone may place the instant on January 1. An
Oura activity-day calculation uses December 31.

### WHOOP strain

A strain score belongs to a WHOOP cycle with an ID and bounds. It is not relabelled
as a midnight-to-midnight total. A calendar workout query may still place the
workout interval inside a named timezone's dates.

### Apple export after a timezone change

The XML timestamp is retained and parsed as an instant. Its printed offset is also
retained. If the route does not establish that the offset describes the recording
location, Athletic Standard does not make that claim. A calculation can still
render the instant in the athlete's chosen timezone.

### Date-only provider score

If a source supplies only `2026-05-08`, the record keeps that date and provider
period kind. It does not receive `2026-05-08T00:00:00Z`.

## Required tests

- Apple timestamps keep their exact source text and parse to the same instant after
  an export timezone changes.
- Apple `HKTimeZone` metadata is retained with Apple's documented object-creation
  meaning.
- Apple activity-summary dates remain dates.
- WHOOP cycle IDs and bounds survive import and export.
- A WHOOP cycle is never returned as a calendar-day total.
- Oura Activity, Sleep, and Calendar Day labels remain distinct.
- Date-only Oura rows receive no invented timestamp.
- Calendar calculations name their IANA timezone.
- Calendar calculations refuse without an explicit or saved timezone.
- Travel and daylight-saving fixtures cover short days, long days, and intervals
  whose endpoints have different offsets.
