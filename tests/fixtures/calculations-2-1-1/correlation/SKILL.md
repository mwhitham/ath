---
name: correlation
description: How two measurements move together, using the average of each calendar day. Use when asking for this summary across more than one record.
license: MIT
compatibility: Requires the ath CLI and an Athletic Standard 2.0.0 database. The calculation runs with ath apply correlation or ath calculate correlation.
metadata:
  version: "2.0.0"
---

# correlation

How two measurements move together, using the average of each calendar day. One request names one measurement, so RMSSD and SDNN stay separate. Personal reports are left out. Devices are not added together.

```ath-calculation
name: correlation
needs: quantity
needs: other_quantity
needs: timezone
allows: source
allows: compare_source
allows: from
allows: to
use: correlation
method: Pearson correlation of same-calendar-day means
```
