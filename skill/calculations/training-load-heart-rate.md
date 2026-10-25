# Training load from heart rate

For each sample, reserve is (heart rate - resting heart rate) / (maximum -
resting heart rate). Add duration in minutes times reserve times e to the
power of factor times reserve. Harder minutes count more. If resting heart
rate, maximum heart rate, or samples are missing, the result names the
missing one.

```ath-calculation
name: training-load-heart-rate
sample_field: heart_rate_samples
rest_field: resting_heart_rate_bpm
maximum_field: maximum_heart_rate_bpm
factor: 1.92
```
