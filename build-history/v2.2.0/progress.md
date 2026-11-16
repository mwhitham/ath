# v2.2.0 progress

This folder is the plan. The maintainer has not approved it. Nothing in the
program, the record, or the database has changed for 2.2.0.

| Step | What | Status |
|---|---|---|
| 0 | The plan: proposals D227–D244, and the Open Wearables study. D239 through D241 stay. D243 corrects the live-read procedure. D244 is the privacy choice for vendor objects. Still not adopted. The new head needs a fresh four-role review | written, not approved |
| 1 | Version 2.2.0 and migration step 6. Step 5 is the published 2.1.1 migration. No new column on an existing table. The root check gains `source_report` by the copy in D236. Summary and score values may be omitted in pairs (D227, D236) | not started |
| 2 | `ath pull openwearables` in the `athleticstandard` crate, with the rewritten mapping (D228, D230, D236) | not started |
| 3 | Key, server, and user id kept out of the record (D229) | not started |
| 4 | Fixture tests, with no live key in CI | not started |
| 5 | Generated schema, demo athlete, skill command reference, and the public-data list | not started |
| 6 | Laptop card and the proof in the build pull request | not started |

The build, after approval, is one pull request. It is not three.

The Windows failed-init cleanup, the size of `src/import/mapping.rs`, and a
name check outside import stay out of that pull request (D231). `persist.rs`
is already gone.
