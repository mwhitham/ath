# Coaching skill standard

A coaching skill gives training advice for one sport. It must meet this standard
before it ships in `skills/`. The `crossfit` and `hyrox` skills are the reference
examples. A starting file is in
[`docs/templates/coaching-skill/SKILL.template.md`](templates/coaching-skill/SKILL.template.md).

## 1. Role and boundary

- The skill advises; the record and the tool skill do not (D228). Its advice is
  saved as authored records (`training_plan`, `planned_session`, `coaching_note`,
  `saved_training_review`, `goal`) with agent authorship and the exact evidence
  revisions it used.
- It never writes advice as a measurement or an athlete report, and never gives an
  athlete report a source.
- It is not medical advice. Pain, injury, illness, pregnancy and postpartum, eating
  concerns, and any medical condition go to a qualified clinician. The skill may
  adapt training around a clinician's guidance; it never diagnoses or treats.
- It never assigns an adaptive or para classification or eligibility. It uses only a
  classification the organiser confirmed.

## 2. Files

- `SKILL.md`: frontmatter with `name`, a `description` that says when to use and
  when not to use the skill (300–700 characters), `license: MIT`, a
  `compatibility` line naming the Athletic Standard version, and
  `metadata.version` and `metadata.kind: coaching`. The body states the purpose,
  the workflow, safety, how to save work, and which reference to open for what.
- `references/`: the depth, one topic per file, each linked from `SKILL.md` with
  when to read it. Every relative link must resolve.
- No `ath-calculation` block. A calculation the sport needs is its own calculation
  skill with a worked example (see `hyrox-race-splits`).

## 3. Content

The skill supports a complete cycle, each as an explicit procedure:

1. Assessment: goals, experience, current training, equipment, schedule, limits,
   and the records to read first.
2. Planning: coherent weeks and blocks, with the purpose of each session.
3. Adapting: how to change a session and why, with a route back.
4. Analysis: how to compare results only when they are comparable.
5. Competition: the named season's rules, divisions, and scoring, with the
   rulebook cited.
6. Review: planned against completed work, separate signals from the calculation
   skills, a few justified changes, saved as records.

Every claim is labelled by its basis: official guidance, research finding (with
its sample and limits), coaching practice, or the coach's hypothesis for this
athlete. The skill keeps what the records show apart from what it infers. It asks
for missing essentials before writing a plan.

## 4. Records

- Every record template is a fenced `json ath-record-example` block holding a JSON
  array of `{ "record_type", "data" }` objects (with optional `record_id` and
  `extensions`). `tests/skill_examples.rs` validates each against the record model.
- Sport detail goes in a namespaced extension on a `benchmark_result`
  (`org.athleticstandard/<sport>-<name>`), not in new fields (D118). Document its
  shape in `references/records.md`.
- Show the preview and wait for approval before every write.

## 5. Writing

Plain English: short sentences, one idea each, the point before the reason. Write
the rule, then the reason (D24). No slogans, no marketing, no real people's names.
Cite sources by author and year or document name, with URLs in a sources file.

## 6. Evidence that it works

A pull request that adds a coaching skill includes:

| Evidence | Requirement |
|---|---|
| Tests | `cargo test` passes, including `skill_examples.rs`. |
| Agent behaviour | At least six fresh-session cases: a full plan, an adaptation, an analysis, a review, a request outside the skill's scope, and a safety referral. Record the commands the agent ran, the records it proposed, and the claims it made, as in the [2.1.2 evaluation](../build-history/v2.1.2/appendix/agent-evaluation.md). |
| Expert review | Review by at least one independent, qualified coach in the sport, with their findings and how each was resolved. An AI review does not count. |
| Decisions | A D-number in the current version's `decisions.md` for any new extension or rule. |
