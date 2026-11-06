# v2.2.0 progress

This folder is the plan. The maintainer has not approved it. Nothing in the
program, the record, or the database has changed for 2.2.0.

| Step | What | Status |
|---|---|---|
| 0 | The plan: spec, decisions D198–D202, and the Open Wearables study | written, not approved |
| 1 | Version 2.2.0, migration step 5, no new column (D198) | not started |
| 2 | `ath pull openwearables` in `ath-core`, using the shipped mapping (D199, D201) | not started |
| 3 | Key, server, and user id kept out of the record (D200) | not started |
| 4 | Fixture tests, with no live key in CI | not started |
| 5 | Generated schema, demo athlete, skill command reference, and the public-data list | not started |
| 6 | Laptop card and the proof in the build pull request | not started |

The build, after approval, is one pull request. It is not three.

The Windows failed-init cleanup, the size of `store.rs` and `mapping.rs`, and
the `persist.rs` field-name check stay out of that pull request (D202).
