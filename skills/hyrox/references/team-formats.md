# Doubles and Relay

Read this file when the athlete races Doubles or Relay, or asks how to share work
with partners. Every rule here is from the 2026/27 rulebooks. Loads are in
[rules-2026-27.md](rules-2026-27.md#divisions-and-loads).

## Doubles

### Rules (Doubles 26/27)

- A team is two racers. Both must start together; starting with one is not allowed
  (p. 22).
- Divisions: Doubles Women, Doubles Women Pro, Doubles Men, Doubles Men Pro, Doubles
  Mixed (one woman and one man) (pp. 8, 22).
- **Both athletes run every run together.** "Both Doubles Team Members [DTMs] must
  run together for the entire 1,000m between each workout station. If one team
  member runs ahead, a 1-minute penalty will apply" (p. 25). More than 3 such
  penalties make the team Out Of Competition, with no ranking (p. 25). No maximum gap
  in metres is set.
- Both enter each station together. Work starts only when both are there. Both leave
  together after all the work is done (p. 25).
- Station work may be shared. The book's "You Go I Go" example has one partner row a
  self-chosen distance, "E.G. 250 METERS", then switch (p. 24). No minimum or maximum
  share is set, and equal shares are not required.
- One partner works while the other rests. The book does not say this in one
  sentence, but its example and its repeated "working DTM" and "resting DTM" wording
  show it (pp. 24, 27–37). DTM means Doubles Team Member.
- The resting partner stays on their feet. Kneeling, sitting, lying, or any knee or
  body part on the floor is not allowed. They stay in the workout zone without
  getting in others' way (p. 24).
- The finish time is taken when both cross the line; the later chip read counts
  (p. 10). The team's age group is the partners' average age on race day (p. 10).

### Resting-partner and change-over rules by station (Doubles 26/27)

| Station | Rule |
|---|---|
| SkiErg (p. 27) | Resting partner stays in the marked area. Handles may not be passed between partners. Only the working partner may change the damper. |
| Sled Push (p. 28) | Resting partner walks immediately behind the working partner, not in a neighbouring lane. |
| Sled Pull (p. 29) | Resting partner stays behind and may not help with the rope. Spoken and physical guidance is allowed if they stay off the box lines and out of the box. |
| Burpee Broad Jumps (p. 31) | The first partner starts with a burpee. The resting partner walks behind. At a change-over, the new partner places their hands where the last partner's feet landed. |
| Rowing (p. 32) | Resting partner stays in the marked area behind. No passing the handle or setting the damper or straps for each other. |
| Farmers Carry (p. 33) | Kettlebells may be passed sideways or backwards, never forwards. Resting partner walks behind. |
| Sandbag Lunges (p. 35) | The sandbag may be passed sideways or backwards, never forwards. Resting partner walks behind. |
| Wall Balls (p. 36) | Resting partner stays in the marked area under the rig. Hand the ball over or let it drop. A "flying transition" (one throws, the other catches and keeps going) is a no-rep. |

Rulebook gap: the SkiErg and rowing rules say the athlete may not leave the machine
before the full distance, yet the "You Go I Go" example switches partners mid-row
(Doubles 26/27 pp. 24, 27, 32). Tell the team to confirm change-over practice with
the judges before the race.

### Coaching Doubles

**Sharing stations does not halve the race.** Both athletes run the full 8 km, at the
pace of the slower runner on the day (Rule: Doubles 26/27 p. 25). Only station work is
shared, and change-overs cost time. Hypothesis: running fitness matters about as much
as in Singles. No study of Doubles physiology exists to test this.

Practice for Doubles (no study of Doubles physiology exists):

1. **Plan the split for each station before race day.** Base it on each partner's
   fresh and fatigued station times. Shorter turns (for example 250 m on the
   machines, 10–25 wall balls) keep both partners working near a hard but repeatable
   effort.
2. **Rehearse change-overs.** Practise passing the kettlebells and sandbag sideways or
   backwards, the burpee hand placement, and the wall-ball hand-off.
3. **Agree the run pace.** The slower runner sets it. Agree a signal for "slow down".
   A partner who runs ahead costs 1 minute.
4. **Train each partner for every station.** Plan for one partner to take more of a
   station if the other struggles on the day.
5. **Train together** at least every two weeks in the build and race-specific phases.

Each partner trains as for Singles running volume. Station training can be shorter
and more intense than in Singles, because turns are shorter.

### Logging a Doubles result

- Use `"format": "doubles"`, the division, and the category (`women`, `men`, or
  `mixed`) in the extension, with `"rulebook": "HYROX Rulebook Doubles 26/27"`.
- Use a Doubles benchmark definition, for example `hyrox-doubles-open-mixed`.
- The splits are team splits. A station split includes both partners' turns and the
  change-overs. Never compare a Doubles station split with a Singles split.
- Who did what in each station is the athlete's account of the race, so log it as an
  athlete report with `"report_type": "note"`, not as a coaching note. The result
  itself cannot show it.

## Relay

### Rules (Relay 26/27)

- A team is four racers: 4 women, 4 men, or 2 women and 2 men (p. 10). Divisions:
  Women, Men, Mixed. There is no Pro Relay (p. 8).
- "Each RM has to run 2 x 1 km and perform 2 workout stations." (pp. 8, 10). RM means
  Relay Member.
- "Which run/workstations each RM completes is entirely up to the team." (p. 10) A
  member may do two legs back to back, or one leg, then another later.
- Only one member is active at a time (p. 12). Each leg is a run followed by its
  station (p. 13).
- **Exchange.** "When RM 1 finishes their run and workout station, they enter the TZ.
  They then high-five RM 2 who is waiting for them. Once the transition occurs, RM2
  starts their run and RM 1 promptly exits the TZ" (p. 13). TZ means Transition
  Zone. The next member may enter only shortly before the exchange; no waiting around
  in the zone.
- A member doing two legs back to back must "run through the TZ after each workout to
  ensure the timing chip is 'seen'"; otherwise "an automatic penalty" applies, size
  not stated (p. 13).
- **Finish.** After the final member leaves the TZ for run 8, the other three go to
  the wall-ball station by the spectator paths and wait at the "Relay Entry Point"
  until the final member has started wall balls. After the 100 wall balls, "all four
  teammates must run together to the Finisher Stage" (p. 25).
- Loads: Women and Men columns only (p. 8). The book does not say which loads a Mixed
  team uses. Tell the team to ask the organiser.
- Age groups: under 40 and 40+, by the team's average age (p. 9).
- Not stated: the Transition Zone's location, the penalty size, and which chip read
  sets the team time (pp. 13, 25).

### Coaching Relay

Practice (no study of Relay racing exists):

1. **Assign legs by strength.** Put the strongest sled athletes on legs 2 and 3, the
   best wall-ball athlete on leg 8, and so on, using fresh station tests.
2. **Back-to-back or split.** Back-to-back legs give one long effort; split legs give
   rest between. Choose from each member's recovery and the team's plan.
3. **Pace each leg harder than Singles.** Each member does only about 2 km of running
   and two stations, so legs are shorter and faster.
4. **Rehearse the exchange.** Practise arriving at the zone on time, the high-five,
   and running through the zone after a back-to-back leg.
5. **Warm up between legs.** A member waiting long between legs needs a short
   re-warm-up before the next one.

Training for a Relay member: 1 km repeats at a hard pace with full stations at race
load, plus the specific stations they will race.

### Logging a Relay result

- Use `"format": "relay"` and `"rulebook": "HYROX Rulebook Relay 26/27"` in the
  extension, and a Relay benchmark definition.
- The splits are the team's. Log all segments so `hyrox-race-splits` can check the
  total. Log which runs and stations were the athlete's own as an athlete report with
  `"report_type": "note"`, because it is the athlete's account. Review only those
  segments as the athlete's performance.
