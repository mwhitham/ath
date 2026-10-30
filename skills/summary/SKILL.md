---
name: summary
description: Count, minimum, maximum, average, and sum for one measurement, separately for each device. Use when asking for this summary across more than one record.
license: MIT
compatibility: Requires the ath CLI and an Athletic Standard 2.0.0 database. The calculation runs with ath apply summary or ath calculate summary.
metadata:
  version: "2.0.0"
---

# summary

Count, minimum, maximum, average, and sum for one measurement, separately for each device. One request names one measurement, so RMSSD and SDNN stay separate. Personal reports are left out. Devices are not added together.

```ath-calculation
name: summary
needs: quantity
allows: source
allows: from
allows: to
allows: group
allows: timezone
per: source
use: count
use: minimum
use: maximum
use: mean
use: sum
method: count, minimum, maximum, mean, and sum per source
```
