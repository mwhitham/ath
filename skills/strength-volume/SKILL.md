---
name: strength-volume
description: Adds up volume load, repetitions times load in kilograms, for every strength set on one workout, converting pounds, counting per-implement loads such as two dumbbells, and splitting by movement. Use when someone asks how much total weight they lifted, the volume of a strength session, or volume per lift. Needs sets with reps, weight, and weight_unit. Not for heart-rate or effort load (use training-load-heart-rate or training-load-effort).
license: MIT
compatibility: Requires the ath CLI 2.1.3 or later (npm package athleticstandard) and an Athletic Standard database. Runs with ath apply strength-volume --record <workout-session-id>.
metadata:
  version: "2.1.3"
  kind: calculation
---

# Strength volume

Volume load: for every set, repetitions × load, added together, in kilograms.

## Before the first ath command

This skill runs `ath`, the Athletic Standard program, version 2.1.3 or later.
Check it once with `ath --version`.

- If `ath` is not found, it is not installed. It comes from the npm package
  `athleticstandard`. The npm package named `ath` is a different program. Install it
  with `npm install -g athleticstandard`, which needs Node.js 22 or later. Other ways
  to install it are at https://www.npmjs.com/package/athleticstandard.
- If the version is older than 2.1.3, update it with `ath --update`.
- Install or update only after the person agrees. Then run `ath --version` again. If
  it is still older than 2.1.3, say so and stop.

If the answer needs the athlete's records or a calculation and `ath` cannot run, stop.
Tell the person what is missing and how to fix it. Do not read the database another
way or do the calculation yourself: the calculations and their checks run only in
`ath`. For any other message from `ath`, follow what it says, and ask before anything
that creates or changes a file.

## Run it

```sh
ath apply strength-volume --record <workout-session-id> --json
```

## Log sets so they can be counted

Each set needs `reps`, `weight`, and `weight_unit`. Since 2.1.2 a set can also say
what it was and how the load was carried (D239):

```json
{ "reps": 10, "weight": 22.5, "weight_unit": "kg", "movement": "dumbbell bench press",
  "load_basis": "per_implement", "implement_count": 2 }
```

- `load_basis: total` means `weight` is the whole load, such as a loaded barbell.
- `load_basis: per_implement` means `weight` is each implement's load, and
  `implement_count` says how many were used.

## Method

Operation `volume_load` (D237).

1. A unit of `kg` (or `kgs`, `kilogram`) counts as kilograms. `lb` (or `lbs`,
   `pound`) is multiplied by 0.45359237 exactly. Any other unit stops the
   calculation and names the set.
2. A `per_implement` load is multiplied by `implement_count`.
3. A set without `load_basis` is counted as written, and
   `unstated_load_basis_sets` says how many there were.
4. Each set adds `reps × kilograms × implements`. The total is rounded to 0.001.
5. When any set names a movement, `by_movement` gives the volume per movement.

## Reading the result

| Field | Meaning |
|---|---|
| `volume`, `weight_unit` | Total volume load in kilograms. |
| `sets`, `reps` | How many sets and repetitions. |
| `by_movement` | Volume, sets, and reps per movement, when named. |
| `unstated_load_basis_sets` | Sets whose load basis is unknown. |

How to describe it:

- Volume load tracks how much work a strength session or week contained. It does
  not say how hard the work was: 10 easy reps and 10 reps at a limit can have the
  same volume. Pair it with effort or the share of sets near a limit.
- Compare a movement with itself over weeks. Volume across different movements
  (squats and curls) adds unlike things.
- Bodyweight movements have no `weight` and are not in the total.

## Limits

- If an older record gives each dumbbell's load with no `load_basis`, the volume
  is half what was lifted. `unstated_load_basis_sets` warns of this.
- A workout logged only as text (`text_result`) has no sets and no volume.

## Worked example

Three sets of 5 back squats at 100 kg (1 500 kg), three sets of 10 dumbbell bench
press with two 22.5 kg dumbbells (1 350 kg), and one set of 5 deadlifts at 225 lb
(510.291 kg): 3 360.291 kg in total.

```json ath-example
{
  "records": [
    { "id": "w", "record_type": "workout_session", "start": "2026-10-01T17:00:00.000Z", "end": "2026-10-01T18:00:00.000Z",
      "sets": [
        { "reps": 5, "weight": 100, "weight_unit": "kg", "movement": "back squat", "load_basis": "total" },
        { "reps": 5, "weight": 100, "weight_unit": "kg", "movement": "back squat", "load_basis": "total" },
        { "reps": 5, "weight": 100, "weight_unit": "kg", "movement": "back squat", "load_basis": "total" },
        { "reps": 10, "weight": 22.5, "weight_unit": "kg", "movement": "dumbbell bench press", "load_basis": "per_implement", "implement_count": 2 },
        { "reps": 10, "weight": 22.5, "weight_unit": "kg", "movement": "dumbbell bench press", "load_basis": "per_implement", "implement_count": 2 },
        { "reps": 10, "weight": 22.5, "weight_unit": "kg", "movement": "dumbbell bench press", "load_basis": "per_implement", "implement_count": 2 },
        { "reps": 5, "weight": 225, "weight_unit": "lb", "movement": "deadlift", "load_basis": "total" }] }
  ],
  "apply": "w",
  "result": {
    "volume": 3360.291, "weight_unit": "kg", "sets": 7, "reps": 50,
    "by_movement": [
      { "movement": "back squat", "volume": 1500, "sets": 3, "reps": 15 },
      { "movement": "dumbbell bench press", "volume": 1350, "sets": 3, "reps": 30 },
      { "movement": "deadlift", "volume": 510.291, "sets": 1, "reps": 5 }],
    "unstated_load_basis_sets": 0
  }
}
```

## Related

`training-load-effort` for how hard the session felt.

```ath-calculation
name: strength-volume
use: volume_load
field: sets
```
