---
name: compare-sources
description: Subtract the average of one device from the average of another. Records are not paired by time. Use when asking for this summary across more than one record.
license: MIT
compatibility: Requires the ath CLI and an Athletic Standard 2.0.0 database. The calculation runs with ath apply compare-sources or ath calculate compare-sources.
metadata:
  version: "2.0.0"
---

# compare-sources

Subtract the average of one device from the average of another. Records are not paired by time. One request names one measurement, so RMSSD and SDNN stay separate. Personal reports are left out. Devices are not added together.

```ath-calculation
name: compare-sources
needs: quantity
needs: source
needs: compare_source
allows: from
allows: to
use: mean
use: subtract
method: separate source summaries and difference between their means
```
