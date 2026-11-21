---
name: baseline
description: Average and population standard deviation for one measurement, separately for each device. Use when asking for this summary across more than one record.
license: MIT
compatibility: Requires the ath CLI and an Athletic Standard 2.0.0 database. The calculation runs with ath apply baseline or ath calculate baseline.
metadata:
  version: "2.0.0"
---

# baseline

Average and population standard deviation for one measurement, separately for each device. One request names one measurement, so RMSSD and SDNN stay separate. Personal reports are left out. Devices are not added together.

```ath-calculation
name: baseline
needs: quantity
allows: source
allows: from
allows: to
allows: group
allows: timezone
per: source
use: mean
use: population_standard_deviation
method: population mean and population standard deviation per source
```
