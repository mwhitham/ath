---
name: trend
description: Straight-line slope of one measurement against elapsed days, separately for each device. Use when asking for this summary across more than one record.
license: MIT
compatibility: Requires the ath CLI and an Athletic Standard 2.0.0 database. The calculation runs with ath apply trend or ath calculate trend.
metadata:
  version: "2.0.0"
---

# trend

Straight-line slope of one measurement against elapsed days, separately for each device. One request names one measurement, so RMSSD and SDNN stay separate. Personal reports are left out. Devices are not added together.

```ath-calculation
name: trend
needs: quantity
allows: source
allows: from
allows: to
per: source
use: slope_per_day
method: ordinary least-squares slope against elapsed days, calculated separately per source
```
