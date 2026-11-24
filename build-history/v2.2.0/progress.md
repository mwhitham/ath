# v2.2.0 progress

This folder is the plan. The maintainer has not approved it. Nothing in the
program, the record, or the database has changed for 2.2.0.

| Step | What | Status |
|---|---|---|
| 0 | The plan: proposals D272–D289, and the Open Wearables study. The proposals were D198–D215 until 2.1.1 used D208–D226, then D227–D244 until 2.1.2 used D227–D253, then D254–D271 until 2.1.3 used D254–D271. Checked against 2.1.1 at `0de71bb` on 15 November 2026: a page is one import preview and one apply, and D282 uses the revision check. D284 through D286 stay. D287 and D288 answer the review at `e145fa6`. D289 withholds a credential-shaped value. The pass after the review at `d636d7a` amends D272, D275, D276, D281, D287, D288, and D289. The pass after the reviews at `3c3f660` amends D273, D275, D276, D287, and D288 and adds no decision number. The pass after the reviews at `57838e1` amends D276, D287, and D288 and adds no decision number. Three questions stay open for the maintainer, in section 10 of the specification: the Garmin detail default, the owner-correction scope, and storing vendor originals in their own files. Still not adopted. The new head needs a fresh review | written, not approved |
| 1 | Version 2.2.0 and migration step 8. Step 5 is the published 2.1.1 migration. Step 6 is the published 2.1.2 migration. Step 7 is the published 2.1.3 migration. No new column on an existing table. The root check gains `source_report` by the copy in D281. Summary and score values may be omitted in pairs (D272, D281) | not started |
| 2 | `ath pull openwearables` in the `athleticstandard` crate, with the rewritten mapping (D273, D275, D281) | not started |
| 3 | Key, server, and user id kept out of the record (D274) | not started |
| 4 | Fixture tests, with no live key in CI | not started |
| 5 | Generated schema, demo athlete, skill command reference, and the public-data list | not started |
| 6 | Laptop card and the proof in the build pull request | not started |

The build, after approval, is one pull request. It is not three.

The Windows failed-init cleanup, the size of `src/import/mapping.rs`, a
way to clear the Garmin key, and a name check outside import stay out of
that pull request (D276). `persist.rs` is already gone.
