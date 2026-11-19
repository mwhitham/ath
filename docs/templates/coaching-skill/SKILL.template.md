---
name: <sport>
description: <What the skill does for this sport.> Use when <the athlete asks for a plan, an adaptation, a result analysis, race or competition preparation, or a review for this sport; name concrete triggers>. Not for <near misses, and which skill to use instead>.
license: MIT
compatibility: Requires the ath CLI <version> or later (npm package athleticstandard) and an Athletic Standard database. Install with ath skill install <sport>.
metadata:
  version: "<version>"
  kind: coaching
---

<!--
Copy this file to skills/<sport>/SKILL.md and rename nothing else. The file
name SKILL.template.md keeps this template out of skill discovery. The
standard is docs/coaching-skill-standard.md; the crossfit and hyrox skills are
complete examples.
-->

# <Sport> coaching

<One paragraph: what this skill helps an athlete do, and the boundary: advice is
saved as authored records; not medical advice.>

## When to use it

- <Trigger.>

## When not to use it

- <Near miss, and the skill to use instead.>

## Safety

- Pain, injury, illness, and medical conditions go to a qualified clinician.
- <Sport-specific red flags.>

## Before the first ath command

This skill runs `ath`, the Athletic Standard program, version <version> or later.
Check it once with `ath --version`.

- If `ath` is not found, it is not installed. It comes from the npm package
  `athleticstandard`. The npm package named `ath` is a different program. Install it
  with `npm install -g athleticstandard`, which needs Node.js 22 or later. Other ways
  to install it are at https://www.npmjs.com/package/athleticstandard.
- If the version is older than <version>, update it with `ath --update`.
- Install or update only after the person agrees. Then run `ath --version` again. If
  it is still older than <version>, say so and stop.

If the answer needs the athlete's records or a calculation and `ath` cannot run, stop.
Tell the person what is missing and how to fix it. Do not read the database another
way or do the calculation yourself: the calculations and their checks run only in
`ath`. For any other message from `ath`, follow what it says, and ask before anything
that creates or changes a file.

## Workflow

1. Assess: read `ath stats --json` and the records in [references/assessment.md](references/assessment.md).
2. Plan: [references/planning.md](references/planning.md).
3. Adapt: [references/adapting.md](references/adapting.md).
4. Analyse results: [references/analysis.md](references/analysis.md).
5. Compete: [references/competition.md](references/competition.md), naming the season's rules.
6. Review a week: [references/review.md](references/review.md).

Label every claim: official guidance, research finding, coaching practice, or a
hypothesis for this athlete. Keep what the records show apart from what you infer.

## Saving work

Show the preview (`ath log --json`) and wait for approval. Templates are in
[references/records.md](references/records.md), as `json ath-record-example` blocks.

## References

- [references/sources.md](references/sources.md): every source, with URLs.
