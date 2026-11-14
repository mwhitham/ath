# v2.2.0 progress

This folder is the plan. The maintainer has not approved it. Nothing in the
program, the record, or the database has changed for 2.2.0.

| Step | What | Status |
|---|---|---|
| 0 | The plan: proposals D198–D215, and the Open Wearables study. D210 through D212 stay. D214 corrects the live-read procedure. D215 is the privacy choice for vendor objects. Still not adopted. The new head needs a fresh four-role review | written, not approved |
| 1 | Version 2.2.0 and migration step 5. No new column on an existing table. The root check gains `source_report` by the copy in D207. Summary and score values may be omitted in pairs (D198, D207) | not started |
| 2 | `ath pull openwearables` in `ath-core`, with the rewritten mapping (D199, D201, D207) | not started |
| 3 | Key, server, and user id kept out of the record (D200) | not started |
| 4 | Fixture tests, with no live key in CI | not started |
| 5 | Generated schema, demo athlete, skill command reference, and the public-data list | not started |
| 6 | Laptop card and the proof in the build pull request | not started |

The build, after approval, is one pull request. It is not three.

The Windows failed-init cleanup, the size of `store.rs` and `mapping.rs`, and
the `persist.rs` field-name check stay out of that pull request (D202).
