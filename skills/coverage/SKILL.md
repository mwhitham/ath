---
name: coverage
description: Calendar days that have a reading, and the days that do not. Use when asking for this summary across more than one record.
license: MIT
compatibility: Requires the ath CLI and an Athletic Standard 2.0.0 database. The calculation runs with ath apply coverage or ath calculate coverage.
metadata:
  version: "2.0.0"
---

# coverage

Calendar days that have a reading, and the days that do not. One request names one measurement, so RMSSD and SDNN stay separate. Personal reports are left out. Devices are not added together.

```ath-calculation
name: coverage
needs: quantity
needs: timezone
allows: source
allows: from
allows: to
use: missing_days
method: calendar dates containing at least one record, calculated separately per source
```
