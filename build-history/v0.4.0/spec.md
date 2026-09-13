# Athletic Standard v0.4.0 — Build Specification

The backtest: finding out whether the predictions are any good without waiting six months.

**Status: draft.** The shape below is taken from [v0.1.0 §7](../v0.1.0/spec.md), which designed the backtest before anything it needed existed. The open questions at the end have to be settled before the build starts, and each answer becomes a decision in [decisions.md](decisions.md).

This plan completes v0.1.0's build step 6, the last one still open. [v0.3.0](../v0.3.0/spec.md) built the loop it measures: `ath predict --as-of` hides the future, `ath log` writes a prediction, `ath grade` scores it. Everything in the three earlier specs still stands.

## 0. Why this version exists

The loop works one prediction at a time. An athlete makes a claim, does the workout, and finds out. Learning whether the reasoning is any good that way takes as many months as there are attempts.

The file already holds the answer key. Every benchmark result in it is a workout whose outcome is known, and `ath predict --as-of <day>` can show an agent only what was knowable the day before. So the question "would this reasoning have been right?" can be asked of every result in the history, in one sitting.

One thing has to be kept apart. A prediction made in a backtest is not a prediction the athlete made before a real attempt, and the two must not share a track record. `ath predict` shows past predictions as evidence and `ath stats` counts them by author; a hundred replayed guesses mixed in with three live ones would make both meaningless. So a replayed prediction has to say what it is, and that is a format change. It is the reason this is 0.4.0 and not a tool release on 0.3.0.

## 1. What ships in this version

- `ath backtest`, which replays every benchmark result that has at least one earlier result on the same benchmark, and reports how far off the predictions were.
- A way for a prediction to say it was made in a backtest, so replayed predictions never appear as live history.
- The planted-contradiction eval from v0.1.0 §7: a summary that says sleep is fine while the raw rows show two bad nights, and a check that the prediction followed the rows.
- The backtest as a regression test: a change to the prompt, the evidence package, or the summaries must not worsen the demo athlete's score.

## 2. The backtest, as v0.1.0 §7 designed it

For each benchmark result in the file, in date order:

1. Skip it if no earlier result exists on the same benchmark. A first-ever result is an anchor, not a test case, because the prediction method starts from the previous result and adjusts. Guessing from general fitness is a different and weaker kind of estimate, and scoring it alongside anchored predictions would pollute the number.
2. Build the evidence with `ath predict <benchmark> --as-of <the day before>`.
3. Ask the agent to predict from that evidence alone.
4. Grade the prediction against the result already in the file, using the same arithmetic `ath grade` uses.

Then report:

```
replayed 9 benchmark results (2 skipped: first-ever result, nothing to anchor to)
median abs error: 4.2%   ·   mean: 5.1%
calibration: actual within stated range 7/9 (78%)
by history depth: 1 prior → 9.8% (n=3) · 2-3 priors → 4.0% (n=4) · 4+ → 2.1% (n=2)
```

Median and mean error say how far off the predictions were. Calibration says how often reality landed inside the stated range, which should roughly match the stated confidence. History depth is the cold-start answer, measured from the athlete's own data: how many results on a benchmark before the numbers are worth trusting.

Every number carries its coverage, the same as every other command (D47).

## 3. Open questions, to settle before building

These are the decisions the build turns on. Each one gets a D-number once answered.

1. **Where does the model come from?** `ath` has no model in it (D46 and everywhere). The backtest needs one to make the predictions. The choices are: the CLI calls a model directly, with a key in the environment, which is the one place the tool would ever do so; or the CLI prepares every evidence package and a harness outside the CLI runs the agent and hands the predictions back; or the agent drives the loop itself with the skill telling it how, and `ath backtest` only does the scoring. v0.1.0 §3 allowed a model call for the backtest alone and nowhere else.
2. **Where do replayed predictions live?** Written into the file with a marker, so `ath check` can see them and the format stays the single record; or kept out of the file entirely, in the backtest's own output, so the ledger only ever holds live claims.
3. **What does the marker look like?** One optional field on a prediction, such as the `as_of` day it was replayed for. Its presence says "backtest", and its value says which day the agent was pretending it was. Every reader of predictions then has to know to filter on it.
4. **What is the regression test's pass line?** The demo athlete's score must not get worse — but by how much, measured how, and against which stored number. A model's output varies between runs, and a test that fails on noise gets deleted.
5. **Does the planted-contradiction eval need a second fixture?** The demo athlete's data is generated to be plausible. The contradiction has to be planted on purpose, which is either a variant of the generator or a second small file.

## 4. Format changes

To be written once question 3 is answered. Whatever the marker is, it is one optional field, so a 0.3.0 file still loads (SPEC.md, Versioning).

## 5. Build order

To be written once the questions are answered. The shape will be: the format change and its validator rule first, then the scoring with a fixed set of predictions so it can be tested without a model, then whichever way the model is reached, then the evals, then the docs.
