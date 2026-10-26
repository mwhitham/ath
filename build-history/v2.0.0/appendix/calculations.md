# Calculation example

Heart-rate zones use a maximum of 200 and these samples:

| offset_ms | bpm | zone |
|---|---|---|
| 0 | 100 | below 60 percent |
| 1000 | 130 | 60 to 70 percent |
| 2000 | 160 | 80 to 90 percent |
| 3000 | 190 | 90 percent and above |

Each sample lasts until the next one. The last sample lasts 1000 milliseconds,
the same as the gap before it. The result is 1 second, 1 second, 0 seconds,
1 second, and 1 second. `cargo test` checks those numbers against
`skills/heart-rate-zones/SKILL.md`.

The other shipped files state their own steps. A missing input is named in
`missing` and the numeric result is empty.
