# Fitting extra work around class

Most athletes who use this skill train in group classes at a CrossFit gym. The
gym sets the class workouts. Extra work must fit around them, not compete with
them.

## What CrossFit says about extra work

- **Official.** Accessory work means "exercises uncommon in standard workouts
  performed before or after the main training session". It is "rarely essential
  for most CrossFit athletes" (O'Connor, 2025).
- **Official.** It is "best used as part of the post-workout period". "Focus on one
  well-executed accessory movement per session" (O'Connor, 2025).
- **Official.** Valid reasons: rehabilitation, imbalances, enjoyment, and weak
  points in experienced athletes, such as triceps work for handstand push-up
  lockout (O'Connor, 2025).
- **Official.** Hitting the same body region on back-to-back days raises overuse
  risk (O'Connor, 2025).
- **Official.** "If you increase the volume, your intensity will decrease in some of
  the pieces you're doing" (Fernandez, 2023).
- **Research.** No study measures how extra sessions interact with class programming.
  The rules below are **Practice** unless marked.

## Procedure

### Step 1. Get the class week

Ask the athlete:

1. Which days and times are your classes?
2. Where does the gym post the workouts, and how far ahead?
3. Does the class have a strength part and a conditioning part?
4. Will your class coach let you change a part of class?

If the gym posts the week ahead, read it before you plan. If it posts only the day
before, plan the extra work with rules ("if Monday's class has more than 30
pull-ups, then ...") instead of fixed content.

### Step 2. Tag each class

For each class, write one line with:

- modality: M (monostructural), G (gymnastics), W (weightlifting);
- main patterns: squat, hinge, press, pull, hanging core, upside-down pressing,
  jumping, running, rowing;
- load: heavy, moderate, or light;
- time domain of the conditioning part;
- the volume of any priority movement, such as total pull-ups.

### Step 3. Draw the pattern grid

Make a table of days against patterns. Mark each cell H (hard), M (moderate), or
blank. This shows where the athlete is already loaded.

| Pattern | Mon (class) | Tue | Wed (class) | Thu | Fri (class) | Sat | Sun |
|---|---|---|---|---|---|---|---|
| Squat | H (back squat 5×3) | | M (wall balls) | | | | |
| Hinge | | | | | H (deadlift 5×5) | | |
| Pull | H (45 pull-ups) | | | | H (50 pull-ups) | | |
| Hanging core | | | | | | | |
| Run or row | | | M (row) | | | | |

### Step 4. Place the extra work

Rules, in order:

1. **Keep at least one full rest day.**
2. **Do not load the same pattern hard on two days in a row.** Official basis:
   overuse risk from back-to-back days (O'Connor, 2025). Research basis: sets to
   failure slowed recovery for 24 to 48 hours in 10 trained men (Morán-Navarro et
   al., 2017).
3. **Skill practice is the exception.** Short sets, far from failure, while fresh,
   can be done more often. Keep it low in fatigue.
4. **Do not put hard extra work before class on the same day.** It blunts the class
   workout. Light skill practice in the warm-up is fine.
5. **After class, add at most one accessory movement**, short and well done
   (O'Connor, 2025).
6. **Prefer a separate short session on a non-class day** for a priority skill.

### Step 5. Name what to reduce or replace

When a class repeats the athlete's priority pattern at high volume, change
something. Say which session and why.

| Class demand | Change to the extra work |
|---|---|
| More than about 30 pull-ups, or any toes-to-bar | Next day: no strict pulling sets. Keep hollow holds and light knee raises. |
| Heavy deadlifts or cleans | Next day: no heavy hinge, no high-rep kettlebell swings. |
| Heavy squats | Next day: no heavy squats or long wall-ball sets. |
| Running intervals or a long run | Next day: no extra running. |
| Handstand push-ups or heavy pressing | Next day: no handstand or pressing skill. |
| A benchmark retest is the next day | That class: lighter loads, and skip the movement the retest uses. |

The thresholds are **Practice**. Adjust them to the athlete's record.

Sometimes the class part is what should change:

- When the athlete follows a separate strength cycle, they can do the class
  strength part lighter, or swap it for the cycle's work. Ask the class coach first.
- When a class workout uses the athlete's weak movement far beyond their set size,
  scale it with the rules in [scaling.md](scaling.md).
- When the athlete is preparing for a retest, they can skip one class or one part
  of it.

Respect the gym. The class coach sees the athlete move and you do not. Suggest
that the athlete shares the plan with their class coach.

### Step 6. Check the weekly total

- Compare planned minutes and sessions with the last 3 to 4 weeks.
- After the first week, compare session-effort load (see
  [weekly-review.md](weekly-review.md)).
- **Practice.** Extra work should be a small share of the week. If class workouts
  start to take much longer than intended, or effort ratings climb on class days,
  cut the extra work first.

## Saving class sessions

- If the athlete shares the class programming ahead, you may save each class as a
  `planned_session` with `block_type: class` and the scaling instructions in
  `instructions`.
- If not, save the rules in a `coaching_note` with `applies_from` and `applies_to`.
  The weekly review then compares completed class workouts against the note.

Shapes are in [records.md](records.md). A full example of both is in
[example-block.md](example-block.md).

## Good and bad answers

Bad: "Add 5×5 strict pull-ups every day to fix your pull-ups." It ignores the 45
pull-ups in Monday's class and loads the same pattern seven days in a row.

Good: "Monday's class had 45 pull-ups and Friday's has 50. So this week Tuesday has
no strict pulling sets: do hollow holds and knee raises instead. On Friday, do the
class pull-ups as ring rows, so Saturday's kipping practice starts fresh. In a week
when Monday has 30 pull-ups or fewer, Tuesday's strict sets go back in."
