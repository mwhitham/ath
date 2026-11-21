# Calculation methods: primary sources and exact definitions

> Compiled on 2026-11-15 by a research agent for the 2.1.2 calculation methods (D232–D244). "I" in the text means
> that compiler. It is research input, not a decision; the decisions are in
> [../decisions.md](../decisions.md). Sources were read on that date.

Research appendix for the training and recovery calculations. Compiled 2026-11-15.

## How to read this file

Each topic has five parts: sources, definition, minimum data, limits, and one sentence on consumer wearable data.

Each claim is marked by how it was checked:

- **Verified** means I read the primary text, the official abstract, or the official developer page.
- **Secondary** means I read a later peer-reviewed paper that quotes or applies the source. I could not open the original.
- **Not verified** means I found no reliable copy. Treat these claims as open questions.

Quotations are exact. Formulas use plain text. "ms" means milliseconds. "bpm" means beats per minute. "ln" means natural logarithm.

---

## 1. HRV statistics: RMSSD and SDNN

### Sources

- Task Force of the European Society of Cardiology and the North American Society of Pacing and Electrophysiology (1996). *Heart rate variability: standards of measurement, physiological interpretation, and clinical use.* Circulation 93(5):1043–1065. doi:[10.1161/01.CIR.93.5.1043](https://doi.org/10.1161/01.CIR.93.5.1043). The same text appeared in Eur Heart J 17(3):354–381. This is an expert standard. It has no study population. **Verified**: I read the full European Heart Journal text.
- Shaffer F, Ginsberg JP (2017). *An overview of heart rate variability metrics and norms.* Front Public Health 5:258. doi:[10.3389/fpubh.2017.00258](https://doi.org/10.3389/fpubh.2017.00258). Narrative review. **Verified** (PMC full text).
- Munoz ML, et al. (2015). *Validity of (ultra-)short recordings for heart rate variability measurements.* PLoS One 10(9):e0138921. doi:[10.1371/journal.pone.0138921](https://doi.org/10.1371/journal.pone.0138921). 3,387 adults. **Verified** (abstract).
- Esco MR, Flatt AA (2014). *Ultra-short-term heart rate variability indexes at rest and post-exercise in athletes.* J Sports Sci Med 13(3):535–541. 23 collegiate athletes. **Verified** (abstract).

### Definitions

An NN interval is the time between two normal heartbeats. "Normal" means beats that start in the sinus node. Artifacts and ectopic beats are removed.

Task Force Table 1, exact wording:

- **SDNN** (ms): "Standard deviation of all NN intervals."
- **RMSSD** (ms): "The square root of the mean of the sum of the squares of differences between adjacent NN intervals."

The same rules as formulas, for N normal intervals NN_1 to NN_N:

```
SDNN  = sqrt( sum_{i=1..N} (NN_i - mean(NN))^2 / (N - 1) )
RMSSD = sqrt( sum_{i=1..N-1} (NN_{i+1} - NN_i)^2 / (N - 1) )
lnRMSSD = ln(RMSSD in ms)
```

The Task Force does not say whether SDNN divides by N or N − 1. With hundreds of beats the difference is very small. RMSSD averages the N − 1 successive differences. Athlete studies take the natural log of RMSSD because RMSSD is skewed (see topic 3).

What each one reflects:

- Task Force: SDNN "reflects all the cyclic components responsible for variability in the period of recording."
- Task Force: RMSSD, NN50 and pNN50 "estimate high frequency variations in heart rate."
- Shaffer and Ginsberg: RMSSD "is the primary time-domain measure used to estimate the vagally mediated changes reflected in HRV." They also write: "Both SNS and PNS activity contribute to SDNN." (SNS is the sympathetic nervous system. PNS is the parasympathetic, or vagal, system.)

### Why SDNN and RMSSD are not interchangeable

They measure different things. SDNN is the spread of all intervals around their mean. RMSSD is the size of the change from one beat to the next. A slow rise or fall in heart rate across the window raises SDNN. It barely changes RMSSD.

SDNN also depends on how long the recording is. The Task Force says:

- "the total variance of HRV increases with the length of analysed recording."
- "SDNN is not a well defined statistical quantity because of its dependence on the length of recording period."
- "it is inappropriate to compare SDNN measures obtained from recordings of different durations."
- "It is inappropriate to compare time–domain measures, especially those expressing overall HRV, obtained from recordings of different durations."

Shaffer and Ginsberg say the same: "24 h, short-term, and ultra-short-term values are not interchangeable." Their norms show the size of the gap. For 24-hour SDNN, values below 50 ms are classed unhealthy and above 100 ms healthy. For short recordings of about 5 minutes, a pooled set of healthy adults (Nunan and colleagues, cited by Shaffer and Ginsberg) had SDNN 50 (SD 16) ms and RMSSD 42 (SD 15) ms.

### Minimum data and recording conditions

- Standardize the length. Task Force: "short-term 5-min recordings and nominal 24h long-term recordings seem to be appropriate options."
- For frequency analysis, the Task Force says about 1 minute is needed for the high-frequency band and about 2 minutes for the low-frequency band. It prefers 5 minutes of a stable recording.
- Short windows: Munoz and colleagues found a single 10-second ECG gave a valid RMSSD (r = 0.853–0.862 against 4–5 minutes). SDNN agreed less well at every length. Both reached near-perfect agreement at 120 seconds. Esco and Flatt found 60-second lnRMSSD agreed well with 5-minute lnRMSSD in athletes. Agreement fell at 30 and 10 seconds.
- Use only normal beats. Edit or remove ectopic beats and artifacts first.

### Known limits

- Short-term SDNN is a different quantity from 24-hour SDNN. The 24-hour norms do not apply to short windows.
- RMSSD squares each difference, so a few bad beats can inflate it.
- Both statistics change with posture, breathing, and mean heart rate.

### Consumer wearable data

Store the statistic name and the window length with every HRV value, and never put SDNN and RMSSD values, or values from different window lengths, into one baseline.

---

## 2. Wearable HRV: what each vendor states

### Apple (HealthKit)

- `heartRateVariabilitySDNN`. Apple's developer page: "A quantity sample type that measures the standard deviation of heartbeat intervals." It continues: "HealthKit uses SDNN heart rate variability, which uses the standard deviation of the inter-beat (RR) intervals between normal heartbeats (typically measured in milliseconds). The system automatically records samples on Apple Watch." **Verified** ([developer page](https://developer.apple.com/documentation/healthkit/hkquantitytypeidentifier/heartratevariabilitysdnn)).
- When it is sampled. Apple's support page *Your heart rate* (HT204666) says: "HRV is sampled while you're still, so the number of measurements you see will vary according to your activity level." **Verified**. The Apple pages I read do not give the window length or the schedule.
- New in 2026. The same support page describes "Recovery HRV" on Apple Watch Series 12 and Apple Watch Ultra 4. It says Recovery HRV "measures how time varies between heartbeats, and can be an indicator of stress and recovery." Apple's newsroom says Overnight Vitals now include Recovery HRV compared with a personal baseline. HealthKit also lists a new identifier, `heartRateVariabilityRMSSD`, introduced in iOS and watchOS 27.0. Its developer page is a bare declaration with no description. **Verified** that the identifier exists. **Not verified**: that Recovery HRV is RMSSD, or that Apple Watch writes samples to the new identifier. Apple has not said either in the pages I read.
- **Not verified**: the common claim that a Breathe or Mindfulness session triggers an SDNN reading. It is not on the Apple pages I opened.

### WHOOP (developer API)

- WHOOP's OpenAPI specification, `RecoveryScore`:
  - `hrv_rmssd_milli`: "The user's Heart Rate Variability measured using Root Mean Square of Successive Differences (RMSSD), in milliseconds."
  - `resting_heart_rate`: "The user's resting heart rate."
  - **Verified** ([OpenAPI spec](https://api.prod.whoop.com/developer/doc/openapi.json)).
- When it is measured. The API does not say. Two peer-reviewed papers do:
  - Bellenger CR, et al. (2021). *Wrist-based photoplethysmography assessment of heart rate and heart rate variability: validation of WHOOP.* Sensors 21(10):3571. doi:[10.3390/s21103571](https://doi.org/10.3390/s21103571). They write: "WHOOP quantify HR and HRV in the final SWS episode of each sleep opportunity." (SWS is slow-wave, or deep, sleep.) Population: 6 healthy young adults (3 men, 3 women, mean age about 23), 15 nights, against ECG and polysomnography. **Verified** (PMC full text).
  - Dial MB, et al. (2025), below, quote WHOOP's wording: a "dynamic average during sleep… weighted towards your last slow wave sleep stage each night." **Secondary**. WHOOP's own help pages blocked my requests.

### Oura

- Oura support article 360025441974 says Oura "measures HRV only during sleep." It takes "five-minute samples" through the night. Average HRV is "the mean of all five-minute samples taken while you sleep." Max HRV is "your highest HRV value recorded during the night." **Verified**.
- Statistic. Oura's blog (2021) says: "rMSSD is the most widely used metric for HRV estimation, including within the Oura app." **Verified**. The support article I read does not name the statistic.

### Garmin

- Garmin owner's manual, HRV status: "Your watch analyzes your wrist heart rate readings while you are sleeping to determine your heart rate variability (HRV)." "The watch requires three weeks of consistent sleep data to display your heart rate variability status." Status compares "Your seven-day average HRV" with "your baseline range." **Verified** ([manual page](https://www8.garmin.com/manuals/webhelp/GUID-3D8FCD7B-7C56-43A9-8665-5CDE4FFD7DF3/EN-US/GUID-9282196F-D969-404D-B678-F48A13D8D0CB.html)).
- A Garmin blog says compatible devices "continuously analyse HRV during sleep" and show "your overnight average." **Verified**.
- Statistic. The Garmin consumer pages I read do not name the statistic. Garmin's Health white paper on beat-to-beat intervals (November 2023) says: "A very commonly used aggregate statistical measurement of HRV is RMSSD. A five-minute RMMSD measurement is typically computed." (The typo is in the original.) It also notes that RMSSD "is very susceptible to outliers due to the squaring operation." Dial and colleagues (2025) state that all five devices they tested report HRV as RMSSD, and that Garmin HRV is measured in 5-minute windows during sleep and averaged. **Secondary**. Treat "Garmin overnight HRV is RMSSD" as likely, not confirmed by Garmin.

### Cross-device validation

Dial MB, Hollander ME, Vatne EA, Emerson AM, Edwards NA, Hagen JA (2025). *Validation of nocturnal resting heart rate and heart rate variability in consumer wearables.* Physiol Rep 13(16):e70527. doi:[10.14814/phy2.70527](https://doi.org/10.14814/phy2.70527). **Verified** (PMC full text).

- Population: 13 healthy adults (7 men, 6 women, age 33.2 ± 8.6 years), 536 nights at home.
- Reference: Polar H10 chest strap ECG processed in Kubios. Values averaged over the whole night.
- RMSSD mean absolute percentage error: Oura Gen 4 5.96%, Oura Gen 3 7.15%, WHOOP 4.0 8.17%, Garmin Fenix 6 10.52%, Polar Grit X Pro 16.32%.

### Known limits

- Each vendor uses a different window. Oura uses the mean of 5-minute samples over the night. WHOOP weights the last deep sleep. Garmin averages the sleep period. Apple uses SDNN at times when the wearer is still, with an unpublished window.
- So values are not interchangeable across vendors, even when two vendors both use RMSSD.
- Vendors change algorithms without notice.

### Consumer wearable data

Store the vendor, the statistic, and the measurement window with each value, and compare a person only with their own earlier values from the same source.

---

## 3. HRV monitoring in athletes

### Sources

- Plews DJ, Laursen PB, Kilding AE, Buchheit M (2012). *Heart rate variability in elite triathletes, is variation in variability the key to effective training? A case comparison.* Eur J Appl Physiol 112(11):3729–3741. doi:[10.1007/s00421-012-2354-4](https://doi.org/10.1007/s00421-012-2354-4). Population: 2 elite triathletes (one man aged 22, one woman aged 20), training 23 ± 2 h per week, followed for 77 days. **Verified**: abstract, plus methods read in the author's open doctoral thesis (Plews 2014, Auckland University of Technology, [hdl:10292/7122](https://hdl.handle.net/10292/7122)), whose chapter 3 is this paper. The thesis copy may differ slightly from the journal version.
- Plews DJ, Laursen PB, Kilding AE, Buchheit M (2013). *Evaluating training adaptation with heart-rate measures: a methodological comparison.* Int J Sports Physiol Perform 8(6):688–691. doi:[10.1123/ijspp.8.6.688](https://doi.org/10.1123/ijspp.8.6.688). Population: 10 runners who responded to a 9-week training program. **Verified** (abstract).
- Plews DJ, Laursen PB, Stanley J, Kilding AE, Buchheit M (2013). *Training adaptation and heart rate variability in elite endurance athletes: opening the door to effective monitoring.* Sports Med 43(9):773–781. doi:[10.1007/s40279-013-0071-8](https://doi.org/10.1007/s40279-013-0071-8). Review with case data from Olympic and world-champion athletes. **Verified** (abstract).
- Plews DJ, Laursen PB, Le Meur Y, Hausswirth C, Kilding AE, Buchheit M (2014). *Monitoring training with heart rate-variability: how much compliance is needed for valid assessment?* Int J Sports Physiol Perform 9(5):783–790. doi:[10.1123/ijspp.2013-0455](https://doi.org/10.1123/ijspp.2013-0455). Population: trained triathletes through normal training, functional overreaching, and taper. **Verified** (abstract).
- Buchheit M (2014). *Monitoring training status with HR measures: do all roads lead to Rome?* Front Physiol 5:73. doi:[10.3389/fphys.2014.00073](https://doi.org/10.3389/fphys.2014.00073). Review. **Verified** (PMC full text).
- Kiviniemi AM, Hautala AJ, Kinnunen H, Tulppo MP (2007). *Endurance training guided individually by daily heart rate variability measurements.* Eur J Appl Physiol 101(6):743–751. doi:[10.1007/s00421-007-0552-2](https://doi.org/10.1007/s00421-007-0552-2). Population: 26 healthy, moderately fit men over 4 weeks. **Verified** (abstract only).
- Added because it states its rule exactly: Vesterinen V, et al. (2016). *Individual endurance training prescription with heart rate variability.* Med Sci Sports Exerc 48(7):1347–1354. doi:[10.1249/MSS.0000000000000910](https://doi.org/10.1249/MSS.0000000000000910). Population: 40 recreational endurance runners (20 women, 20 men). **Verified** (open accepted manuscript).

### Definitions, baselines, and thresholds

| Source | Measure | Recording | Baseline | Averaging | Threshold or rule |
|---|---|---|---|---|---|
| Plews 2012 | Ln rMSSD | On waking. 6-minute supine recording; last 5 minutes analysed. Polar RS800cx chest strap. | First 2 weeks | 7-day rolling mean. Also a 7-day rolling CV of Ln rMSSD. | Smallest worthwhile change (SWC) from the individual coefficient of variation (CV) of Ln rMSSD over the first 2 weeks. "a change of more or less than 0.5 of an athletes' CV is worthwhile" (citing Hopkins 2009). |
| Plews 2013 (IJSPP) | Ln rMSSD and resting HR | Daily | — | Single day vs 1-week mean | Weekly mean tracked fitness far better. For maximal aerobic speed: r = .72 (week) vs r = −.06 (single day). |
| Plews 2014 | Ln rMSSD | Daily | — | Mean of 1 to 7 randomly chosen days per week | "Practitioners using HRV to monitor training adaptation should use a minimum of 3 (randomly selected) valid data points per week." |
| Plews thesis 2014 (summary chapter) | Ln rMSSD | On waking | 2 weeks during normal training | 7-day rolling mean | Early warning: "two consecutively averaged values below the SWC, alongside a continually reducing rolling average." |
| Buchheit 2014 | Ln rMSSD and others | 5–10 minutes on waking; supine is common | — | Weekly mean | Typical error of resting vagal HRV about 12% CV for Ln rMSSD. SWC about +3%. Day-to-day CV of Ln rMSSD 10–20%. Record "at least 3–4 times a week." |
| Kiviniemi 2007 | High-frequency power of R-R intervals | Every morning | 10 days | Single day | Low intensity or rest if HRV fell "below reference value [10-day mean-SD] or decreasing trend for 2 days." Otherwise high intensity. |
| Vesterinen 2016 | RMSSD (not log) | Every morning after waking, 4 minutes supine, chest belt | 4 weeks | 7-day rolling mean | SWC = "mean ± 0.5 * SD". Outside the SWC: low intensity or rest. |

Notes on the table:

- The Plews 2012 text says the SWC "was calculated from the individual coefficient of variation (CV) in Ln rMSSD." The pages I read do not show the band arithmetic. A literal reading is: band = baseline mean × (1 ± 0.5 × CV). Treat that as an interpretation.
- Buchheit 2014 explains why averaging helps: noise "decreases by a factor of 1 / √n (i.e., using 4 tests halves the noise)."
- Buchheit 2014 lists Kiviniemi 2007 among studies that recorded sitting. I could not confirm the posture in the original.

### Minimum data and recording conditions

- At least 3 valid days per week (Plews 2014). Buchheit suggests 3–4.
- A baseline from a normal training period: 2 weeks (Plews), 10 days (Kiviniemi), or 4 weeks (Vesterinen).
- Same time, same posture, same place. All of these studies used morning recordings with a chest strap.

### Known limits

- The samples are small: 2 athletes, 10 runners, 26 men.
- The rules were built from morning supine chest-strap recordings, not from overnight wrist or ring data.
- "Saturation": in athletes with a very low resting heart rate, RMSSD can fall even when vagal activity is high (Plews 2013 review; Buchheit 2014). A drop is not always fatigue.
- Context changes the meaning. Plews 2013 reports that in elite athletes both rises and falls in HRV have gone with poor adaptation.
- Buchheit notes that night recordings change with sleep stages and fragmentation, which can mislead.

### Consumer wearable data

Nightly RMSSD can use the same log scale, 7-day rolling mean, and personal band built from at least 2 weeks of normal training, but the published thresholds came from morning chest-strap recordings, so treat them as a starting point rather than a validated rule for overnight wrist or ring data.

---

## 4. Resting heart rate from wearables

### Vendor definitions

- **Apple** `restingHeartRate`. Developer page: "It is an estimation of the user's lowest heart rate during periods of rest." "the system estimates the resting heart rate by analyzing sedentary heart rate samples throughout the day." "Because the resting heart rate estimates become more accurate as the day progresses, the system may delete earlier samples and replace them with better estimates. Apple Watch replaces only the samples written by the watch for the current or previous day." Units are count per time. **Verified** ([developer page](https://developer.apple.com/documentation/healthkit/hkquantitytypeidentifier/restingheartrate)).
- **Oura** support article 360025588793. Average RHR is the average "captured during the night." Lowest RHR is "the lowest RHR value captured during the night." "Your RHR values are captured every 10 minutes throughout the night." **Verified**. Dial and colleagues (2025) describe Oura heart rate as averaged into 10-minute segments. **Secondary**.
- **Garmin**. Resting heart rate is "the lowest 30 min average in a 24 h period." This is Garmin's definition as quoted by Dial and colleagues (2025). **Secondary**. I could not open Garmin's own help page. Dial and colleagues left Garmin out of their resting heart rate analysis because the 30-minute window has no time stamp.
- **WHOOP**. A "dynamic average during sleep… weighted towards your last slow wave sleep stage each night" (WHOOP wording quoted by Dial and colleagues). **Secondary**.
- **Fitbit**, from Quer G, Gouda P, Galarnyk M, Topol EJ, Steinhubl SR (2020). *Inter- and intraindividual variability in daily resting heart rate…* PLoS One 15(2):e0227709. doi:[10.1371/journal.pone.0227709](https://doi.org/10.1371/journal.pone.0227709). Daily resting heart rate was "calculated by a proprietary formula from Fitbit" and "primarily calculated during periods of sleep." The authors describe it as representative of a supine value "immediately after waking." **Verified**.
  - Population: 92,457 US adults. Only days with at least 20 hours of wear counted. People needed at least 2 such days per week for at least 35 weeks.
  - Findings: mean daily resting heart rate 65 bpm (range 40–109). 20% of people had at least one week where it moved by 10 bpm or more.

### Research definitions

- Morning supine: Plews 2012 computed resting heart rate from the mean R-R interval of the same 5-minute morning window: `RHR = 60 / (mean R-R in seconds)`. **Verified** (thesis chapter 3).
- Weekly means work better than single days. Plews 2013 (IJSPP) found weekly resting heart rate correlated with performance change (r = −.62 and .73), while single-day values did not (r = .35 and −.21). **Verified** (abstract).
- Buchheit 2014 lists the typical error of resting heart rate as about 10% (CV) and its SWC as about −2%. **Verified** (Table 1).
- Accuracy against ECG at night (Dial 2025): mean absolute percentage error Oura Gen 3 1.67%, Oura Gen 4 1.94%, Polar Grit X Pro 2.71%, WHOOP 4.0 3.00%. **Verified**.

### "Lowest 5-minute average"

I found no primary research source that defines resting heart rate as the lowest 5-minute average. The nearest published definitions are vendor rules: lowest 10-minute value (Oura) and lowest 30-minute average in 24 hours (Garmin). **Not verified**: any validation of a lowest-5-minute rule.

### Known limits

- A minimum is pulled down by noise. The more windows a night has, the lower the expected minimum. So "lowest" depends on night length and window size. (This is a statistical point, not a finding from a source.)
- "Lowest during sleep," "average during sleep," and "estimated while still in the daytime" are different quantities. They are not interchangeable.
- Illness, alcohol, heat, and late meals all raise night heart rate.

### Consumer wearable data

Keep "lowest overnight," "average overnight," and "daytime resting estimate" as separate kinds of value with their window length, and trend each one only against itself.

---

## 5. Heart-rate recovery

### Sources

- Cole CR, Blackstone EH, Pashkow FJ, Snader CE, Lauer MS (1999). *Heart-rate recovery immediately after exercise as a predictor of mortality.* N Engl J Med 341(18):1351–1357. doi:[10.1056/NEJM199910283411804](https://doi.org/10.1056/NEJM199910283411804). **Verified** (abstract; the full text was blocked).
  - Population: 2,428 consecutive adults (age 57 ± 12, 63% men) referred for a symptom-limited exercise test with thallium imaging. No heart failure, no prior revascularization, no pacemaker. Followed 6 years. 213 deaths.
  - Definition: "the decrease in the heart rate from peak exercise to one minute after the cessation of exercise."
  - Cutoff: abnormal is "a reduction of 12 beats per minute or less." 639 patients (26%) were abnormal. Adjusted relative risk of death 2.0.
- Active versus passive recovery:
  - Watanabe J, Thamilarasan M, Blackstone EH, Thomas JD, Lauer MS (2001). *Heart rate recovery immediately after treadmill exercise and left ventricular systolic dysfunction as predictors of mortality.* Circulation 104(16):1911–1916. **Verified** (abstract). They state: "In prior studies, recovery heart rates were measured while patients were exercising lightly, that is, during a cool-down period." Their patients lay in the left lateral position with no cool-down. Abnormal was ≤18 bpm at 1 minute. Population: 5,438 patients referred for exercise echocardiography.
  - Nishime EO, Cole CR, Blackstone EH, Pashkow FJ, Lauer MS (2000). *Heart rate recovery and treadmill exercise score as predictors of mortality in patients referred for exercise ECG.* JAMA 284(11):1392–1398. doi:[10.1001/jama.284.11.1392](https://doi.org/10.1001/jama.284.11.1392). Same research group. "Following peak exercise, patients walked for a 2-minute cool-down period at 1.5 mph at a 2.5% grade." Population: 9,454 patients. **Verified** (journal page).
  - **Not verified**: that Cole 1999 used the same 1.5 mph, 2.5% cool-down. Watanabe confirms it was an active cool-down. I could not read Cole's methods.
- Other time points:
  - Cole CR, Foody JM, Blackstone EH, Lauer MS (2000). Ann Intern Med 132(7):552–555. Submaximal test. Recovery at 2 minutes; abnormal ≤42 bpm. Population: 5,234 adults without heart disease. **Verified** (abstract).
  - Shetler K, et al. (2001). *Heart rate recovery: validation and methodologic issues.* J Am Coll Cardiol 38(7):1980–1987. 2,193 male veterans. Recovery at 2 minutes predicted death best; a fall of less than 22 bpm had hazard ratio 2.6. **Verified** (abstract).
- Athletes:
  - Daanen HAM, Lamberts RP, Kallen VL, Jin A, Van Meeteren NLU (2012). *A systematic review on heart-rate recovery to monitor changes in training status in athletes.* Int J Sports Physiol Perform 7(3):251–260. doi:[10.1123/ijspp.7.3.251](https://doi.org/10.1123/ijspp.7.3.251). 5 cross-sectional and 8 longitudinal studies. Recovery was related to training status. Confounders: "age, ambient temperature, and the intensity and duration of the exercise period preceding HRR." They call for "better standardization." **Verified** (abstract).
  - Buchheit 2014: post-exercise recovery is driven mainly by the metaboreflex. In practice it tracks fitness, because it depends on the relative intensity of the effort before it. SWC about +7%. **Verified**.
- Apple HealthKit `heartRateRecoveryOneMinute`: "A quantity sample that records the reduction in heart rate from the peak exercise rate to the rate one minute after exercising ended." "These samples always record a positive value." iOS 16 and watchOS 9 or later. The page does not say how peak or end of exercise is found. **Verified**.

### Definition

```
HRR60 = HR at peak exercise (or at the moment exercise stops) - HR 60 s later   [bpm]
```

### Minimum data and recording conditions

- A known end of exercise.
- A standard effort before it: maximal, or a fixed submaximal intensity.
- A fixed recovery state: walking, standing, sitting, or lying.
- A fixed time point: 60 or 120 seconds.
- Similar temperature.

### Known limits

- Cutoffs only hold for the protocol that made them: ≤12 bpm with active walking, ≤18 bpm lying down, <22 bpm at 2 minutes, ≤42 bpm at 2 minutes after a submaximal test.
- The mortality cutoffs come from clinical patients. They say nothing about training status in athletes.
- In everyday workouts the end of exercise is often unclear. Optical sensors lag fast changes.

### Consumer wearable data

Compute recovery only when the end of a hard effort is clearly marked, record whether recovery was active or still, and never apply clinical treadmill cutoffs to everyday workouts.

---

## 6. Heart-rate zones and maximum heart rate

### Zone bands

- **ACSM 2011** (Garber CE, et al. *Quantity and quality of exercise…* Med Sci Sports Exerc 43(7):1334–1359. doi:[10.1249/MSS.0b013e318213fefb](https://doi.org/10.1249/MSS.0b013e318213fefb)). Percent of maximum heart rate: very light <57, light 57–63, moderate 64–76, vigorous 77–95, near-maximal to maximal ≥96. **Secondary**: I saw these values in later tables that cite Garber 2011. I could not open the original table.
- **ACSM 1998** (Pollock ML, et al. Med Sci Sports Exerc 30(6):975–991). Very light <35, light 35–54, moderate 55–69, hard 70–89, very hard ≥90, maximal 100. **Verified** (reprinted Table 1, "courtesy of Haskell and Pollock").
- So "ACSM zones" is not one fixed set. Record which edition.
- **Edwards**: 50–60%, 60–70%, 70–80%, 80–90%, 90–100% of maximum, weighted 1 to 5. Haddad 2017 writes the bands as ">50–60% x1; >60–70% x2; >70–80% x3; >80–90% x4; >90–100% x5," so each lower bound is exclusive. Original: Edwards S. *The Heart Rate Monitor Book*, 1993 (some papers cite 1992). **Secondary** (Haddad 2017; Crawford 2018). I could not see the book.

### Why maximum heart rate should be measured

- Tanaka H, Monahan KD, Seals DR (2001). *Age-predicted maximal heart rate revisited.* J Am Coll Cardiol 37(1):153–156. doi:[10.1016/S0735-1097(00)01054-8](https://doi.org/10.1016/S0735-1097(00)01054-8). **Verified** (abstract).
  - Meta-analysis: 351 studies, 492 groups, 18,712 people. HRmax = 208 − 0.7 × age (r = −0.90).
  - Lab check: 514 healthy adults. HRmax = 209 − 0.7 × age.
  - No difference by sex or activity level. The authors say 220 − age "underestimates HRmax in older adults."
  - The meta-analysis used group means. So it shows the average trend, not the error for one person.
- Nes BM, Janszky I, Wisløff U, Støylen A, Karlsen T (2013). *Age-predicted maximal heart rate in healthy subjects: the HUNT fitness study.* Scand J Med Sci Sports 23(6):697–704. doi:[10.1111/j.1600-0838.2012.01445.x](https://doi.org/10.1111/j.1600-0838.2012.01445.x). 3,320 healthy men and women with a verified maximal test. HRmax = 211 − 0.64 × age, standard error of estimate 10.8 bpm. "a standard error of 10.8 beats/min must be taken into account." **Verified** (abstract).
- Gellish RL, et al. (2007). Med Sci Sports Exerc 39(5):822–829. 132 people, 908 tests over 25 years. HRmax = 207 − 0.7 × age. **Verified** (abstract).

Worked example (my arithmetic). Say the true maximum is 190 bpm and the formula gives 179 bpm, one standard error low. The formula's 80% line is 143 bpm. That is only 75% of the true maximum. The athlete trains a full zone lower than planned.

### Known limits

- Zone bands are conventions. They are not physiological thresholds.
- Optical sensors can record false peaks. A measured maximum needs artifact checks.

### Consumer wearable data

Use the highest reliable heart rate from a maximal test or race, store where the maximum came from, and use an age formula only as a labeled fallback.

---

## 7. Banister TRIMP

### Formula as commonly cited

```
x     = (HR_ex - HR_rest) / (HR_max - HR_rest)     # "delta HR ratio", 0 to 1
TRIMP = D × x × a × e^(b × x)                       # D = duration in minutes

men:   a = 0.64, b = 1.92
women: a = 0.86, b = 1.67
```

The weighting `a × e^(b × x)` is described as coming from the rise of blood lactate with heart rate. Haddad 2017 describes it as "a pre-evaluated coefficient obtained with the relationship between HR and blood lactate during an incremental exercise."

### Sources

- Morton RH, Fitz-Clarke JR, Banister EW (1990). *Modeling human performance in running.* J Appl Physiol 69(3):1171–1177. doi:[10.1152/jappl.1990.69.3.1171](https://doi.org/10.1152/jappl.1990.69.3.1171). The abstract describes the fitness-fatigue model, tested on "athletes and nonathletes." **Verified** (abstract). The full text is closed access (Unpaywall: no open copy). I could not read it.
- Banister EW (1991). *Modeling elite athletic performance.* In: MacDougall JD, Wenger HA, Green HJ, eds. *Physiological Testing of the High-Performance Athlete*, 2nd ed. Human Kinetics, pp. 403–424. **Not accessed**.
- Later peer-reviewed papers print the formula with 0.64:
  - Crawford 2018 (topic 8): "Training Load = D × HRr × 0.64e^(1.92 × HRr)," with HRr from average session heart rate and individual resting heart rate. It prints only the male constants, though half its participants were women.
  - Uchida MC, et al. (2014). *Does the timing of measurement alter session-RPE in boxers?* J Sports Sci Med 13(1):59–65. It prints the same male formula and credits Banister 1975.
  - **Secondary**.

### Is the 0.64 / 0.86 multiplier part of the published formula?

**Not verified.** Every secondary source I found includes it and credits Morton 1990 or Banister 1991. I could not read either original. Some software has left it out and later added it back.

What the choice changes. The multiplier is a fixed number for each sex. Leaving it out makes TRIMP 1/0.64 = 1.5625 times larger for men and 1/0.86 = 1.163 times larger for women. Within one person, trends and ratios do not change. Absolute values do change, and so do comparisons between men and women.

### Edwards TRIMP

```
Edwards load = sum over zones k = 1..5 of (minutes in zone k × k)
```

Zones are the Edwards bands in topic 6. Time below 50% of maximum adds nothing. Crawford 2018 used this rule. **Secondary**.

### Minimum data

- A measured resting heart rate and a measured maximum heart rate.
- The sex-specific constants.
- Duration in minutes.

### Known limits

- The exponential weight makes errors in maximum heart rate matter more at high intensity.
- Session mean or minute by minute. Banister-style papers use average session heart rate. Because the weight curves upward, adding minute-by-minute TRIMP gives a larger number than using the session mean when intensity varies. (This is my computation note, not from a source.)
- Heart rate methods fit steady endurance work poorly for heavy lifting and short intervals. Uchida 2014 notes TRIMP is "limited in monitoring very high-intensity exercises."
- The constants come from a lactate curve. The original group is not described in the parts I could read.

### Consumer wearable data

Compute TRIMP only when the record has a measured resting heart rate, a measured maximum, and the sex-specific constants, and store which constants and which heart-rate summary (session mean or per sample) were used.

---

## 8. Session RPE training load

### Sources

- Foster C, Florhaug JA, Franklin J, Gottschall L, Hrovatin LA, Parker S, Doleshal P, Dodge C (2001). *A new approach to monitoring exercise training.* J Strength Cond Res 15(1):109–115. PMID 11708692. Two parts with different subjects: steady and interval cycling, and basketball practice. Session RPE scores matched a heart-rate method closely, though they ran higher. **Verified** (abstract). I could not open the full text.
- Haddad M, Stylianides G, Djaoui L, Dellal A, Chamari K (2017). *Session-RPE method for training load monitoring: validity, ecological usefulness, and influencing factors.* Front Neurosci 11:612. doi:[10.3389/fnins.2017.00612](https://doi.org/10.3389/fnins.2017.00612). Review. 950 studies cited Foster 2001. 36 tested validity and reliability with the modified CR-10. **Verified** (PMC full text).
- Crawford DA, Drake NB, Carper MJ, DeBlauw J, Heinrich KM (2018). *Validity, reliability, and application of the session-RPE method for quantifying training loads during high intensity functional training.* Sports 6(3):84. doi:[10.3390/sports6030084](https://doi.org/10.3390/sports6030084). **Verified** (PMC full text).
- Uchida 2014 (topic 7) for rating timing. **Verified** (PMC full text).

### Definition (Foster method)

```
session load (arbitrary units, AU) = session RPE (modified CR-10, 0–10) × session duration (min)
```

- Haddad: "The session-duration refers to the length of session expressed in minutes." Worked example: "for a session of 87 min, with RPE of 4 (quite hard)… 87 × 4 will provide a TL of 348 A.U."
- The question the athlete answers: "How was your workout?" The single number "refers to the mean intensity of the entire exercise session."
- Modified CR-10 (Haddad Table 1, after Foster 2001): 0 Rest; 1 Very, Very Easy; 2 Easy; 3 Moderate; 4 Somewhat Hard; 5 Hard; 6 no label; 7 Very Hard; 8 no label; 9 no label; 10 Maximal.
- Timing. Uchida 2014: "According to the methods of Foster et al., 2001, 30 minutes" after the session. They tested 8 boxers and found 10-minute and 30-minute ratings did not differ. **Secondary** for Foster's 30 minutes.

### Borg 6–20 versus CR-10

- They are different scales. The 6–20 scale runs from 6 to 20. The modified CR-10 runs from 0 to 10.
- A load built on one scale is not on the same scale as a load built on the other. You cannot convert one to the other by a fixed factor.

### What Crawford 2018 used

- Population: 25 healthy, recreationally active adults (13 men, age 22.6 ± 3.5; 12 women, age 21.0 ± 1.5) with no prior high-intensity functional training. 6 weeks, 30 planned sessions.
- Scale: "the original Borg's Rate of Perceived Exertion scale (i.e., 6–20)." Not CR-10.
- Timing: "approximately five minutes following the completion of each DW." (DW is the programmed daily workout.)
- Duration: the daily workout in minutes. Each full session was about 1 hour, with a warm-up and a mobility cool-down. Only the workout part was counted.
- Load: "the product of the DW duration in minutes by the perceived training intensity measured by Borg's rating of perceived exertion scale."
- Criterion methods: Edwards load and Banister TRIMP (male constants printed).
- Result: session RPE load was valid against the heart-rate methods. Reliability of matching rating to percent of maximum heart rate was poor (ICC = 0.583).

### Known limits

- The scale, the delay, and the duration rule all change the number.
- Some studies count the whole session. Others count only the work part.
- The rating covers the whole session, including rest.

### Consumer wearable data

Store the scale (CR-10 or 6–20), the duration rule (whole session or work part), and the delay after the session, and only add or compare loads that share all three.

---

## 9. Sleep

### Standard terms

- **Time in bed (TIB)** or **total recording time (TRT)**. Shrivastava 2014: total recording time is "the total amount of time during which the patient is in bed with recording equipment activated." Menghini 2021 sets TIB from lights-off to lights-on.
- **Total sleep time (TST)**. Menghini: "Number of minutes classified as sleep between lights-off and lights-on." Computed as sleep epochs × epoch length (s) / 60.
- **Sleep efficiency (SE)**. Menghini: "Percentage of total sleep time over TIB," that is `SE = TST / TIB × 100`.
- **Sleep onset latency (SOL)**. Menghini: wake minutes "before the first epoch classified as sleep."
- **Wake after sleep onset (WASO)**. Menghini: "Number of minutes classified as wake after the first epoch classified as sleep," inside lights-off to lights-on. Shrivastava: "periods of wakefulness occurring after defined sleep onset."
- **Sleep period** (sleep onset to final awakening). Reed and Sacco 2016 argue the SE denominator should be the whole sleep episode, from the first attempt to sleep to the final awakening. In their worked diary example, SE is 55% with TIB and 64% with the sleep episode.
- **Epoch**. The AASM standard scores sleep studies in 30-second epochs (cited in Menghini).

Sources:

- Menghini L, Cellini N, Goldstone A, Baker FC, de Zambotti M (2021). *A standardized framework for testing the performance of sleep-tracking technology: step-by-step guidelines and open-source code.* Sleep 44(2):zsaa170. doi:[10.1093/sleep/zsaa170](https://doi.org/10.1093/sleep/zsaa170). **Verified**.
- Shrivastava D, Jung S, Saadat M, Sirohi R, Crewson K (2014). *How to interpret the results of a sleep study.* J Community Hosp Intern Med Perspect 4(5):24983. doi:[10.3402/jchimp.v4.24983](https://doi.org/10.3402/jchimp.v4.24983). **Verified**.
- Reed DL, Sacco WP (2016). *Measuring sleep efficiency: what should the denominator be?* J Clin Sleep Med 12(2):263–266. doi:[10.5664/jcsm.5498](https://doi.org/10.5664/jcsm.5498). **Verified**. They found only 6 of 18 sleep studies in one meta-analysis stated SE as TST/TIB.
- The AASM scoring manual itself (Iber C, et al., 2007, and later editions). **Not accessed**. Its exact reporting formulas are not verified here.

Note: under Menghini's rules, WASO includes wake after the final awakening until lights-on. A sleep-period definition leaves that time out. State which one you use.

### Sleep Regularity Index (SRI)

Source: Phillips AJK, Clerx WM, O'Brien CS, Sano A, Barger LK, Picard RW, Lockley SW, Klerman EB, Czeisler CA (2017). *Irregular sleep/wake patterns are associated with poorer academic performance and delayed circadian and sleep/wake timing.* Sci Rep 7:3216. doi:[10.1038/s41598-017-03171-4](https://doi.org/10.1038/s41598-017-03171-4). **Verified** (PMC full text).

- Population: 61 Harvard undergraduates (32 men, 29 women, age 18–24). Online sleep diaries for about 30 days (26–36).
- Definition: "SRI was computed as the likelihood that any two time-points (minute-by-minute) 24 h apart were the same sleep/wake state, across all days. The value could theoretically range 0 to 1, and was rescaled (y = 200 (x − 1/2)) to give a range of −100 to 100."
- Meaning: someone who sleeps and wakes at the same times every day scores 100. Someone who sleeps at random scores 0. Values below 0 are possible but very unlikely.
- Missing data: "Only pairs of non-missing time-points were used to compute SRI." The final analysis used "the longest interval of a whole number of weeks with no missing data to avoid day-of-week bias." Of the 59 people in that analysis, 2 had one week, 7 had two, 7 had three, and 43 had four.

Exact formula, as written by Lunsford-Avery 2018 and Fischer 2021:

```
SRI = -100 + 200 / (M × (N - 1)) × sum_{j=1..M} sum_{i=1..N-1} delta(s[i,j], s[i+1,j])

M = number of epochs per day
N = number of days
s[i,j] = sleep or wake state on day i in epoch j
delta = 1 if the two states are equal, else 0
```

Epoch length: 1 minute in Phillips (diary data). 30 seconds in Lunsford-Avery (actigraphy). Fischer uses 30-second epochs as the example.

Later work:

- Lunsford-Avery JR, Engelhard MM, Navar AM, Kollins SH (2018). *Validation of the Sleep Regularity Index in older adults and associations with cardiometabolic risk.* Sci Rep 8:14158. doi:[10.1038/s41598-018-32402-5](https://doi.org/10.1038/s41598-018-32402-5). Population: 1,978 adults from MESA (age 68.7 ± 9.2). Wrist actigraphy for "seven consecutive days," 30-second epochs, scored by hand with a diary. SRI mean 71.6 ± 14.5 (range 6.2–97.0). **Verified**. The paper has two errata (2020, 2021). I did not review them.
- Fischer D, Klerman EB, Phillips AJK (2021). *Measuring sleep regularity: theoretical properties and practical usage of existing metrics.* Sleep 44(10):zsab103. doi:[10.1093/sleep/zsab103](https://doi.org/10.1093/sleep/zsab103). Simulation study of 2 to 28 days, with one day in seven missing. **Verified** (PMC full text). Findings:
  - "SRI displayed the least dependence on study length of all metrics."
  - The standard deviation of sleep timing and interdaily stability "required over a week of sleep–wake data for unbiased estimates." With 7 days or fewer they overstate regularity.
  - SRI confidence intervals were wider than those of interdaily stability: 39% wider for 7 days or fewer, 20% for 8–14 days, 14% for more than 14 days.
  - SRI "tends to be uncorrelated with total sleep time."
  - The paper gives tables for 2, 7, 14, 21, and 28 days. It does not name one minimum.
- Fischer D, McHill AW, Sano A, Picard RW, Barger LK, Czeisler CA, Klerman EB, Phillips AJK (2020). *Irregular sleep and event schedules are associated with poorer self-reported well-being in US college students.* Sleep 43(6):zsz300. doi:[10.1093/sleep/zsz300](https://doi.org/10.1093/sleep/zsz300). 223 students, about 30 days. Used composite phase deviation. Irregular sleep predicted lower average well-being. Sleep duration predicted day-to-day well-being. **Verified** (abstract).

### Why nightly totals cannot reproduce SRI

- SRI needs the sleep or wake state at each clock time on consecutive days. A nightly total keeps only how much sleep there was, not when.
- Two weeks with the same nightly totals can have an SRI near 100 or far lower, depending on timing.
- Bedtime and wake time alone also miss naps and night wakings. SRI counts both. Phillips: the SRI "does not require designation of a main daily sleep episode."

### Sleep midpoint and its variability

- Midpoint: halfway between sleep onset and final wake of the main sleep.
- Average midpoints with circular statistics, because clock time wraps at midnight. Phillips used vector averages. Lunsford-Avery give a circular mean of all sleep epochs (their equation 3).
- Variability: the within-person standard deviation of daily midpoint. Fischer 2021: it needs more than a week for an unbiased value. It tracks timing only. It misses changes in duration and broken sleep.
- Social jetlag: average midpoint on free days minus average midpoint on workdays (Fischer 2021). It measures weekly, not daily, change.

### Consumer wearable data

Compute SRI only from epoch-level sleep and wake data, or at least sleep start and end times, on consecutive days, use only pairs of non-missing epochs, report the number of valid day pairs, and never derive it from nightly totals.

---

## 10. Strength training volume

### Sources

- McBride JM, McCaulley GO, Cormie P, Nuzzo JL, Cavill MJ, Triplett NT (2009). *Comparison of methods to quantify volume during resistance exercise.* J Strength Cond Res 23(1):106–110. doi:[10.1519/JSC.0b013e31818efdfe](https://doi.org/10.1519/JSC.0b013e31818efdfe). 10 university students experienced in lifting. **Verified** (abstract).
- Scott BR, Duthie GM, Thornton HR, Dascombe BJ (2016). *Training monitoring for resistance exercise: theory and applications.* Sports Med 46(5):687–698. doi:[10.1007/s40279-015-0454-0](https://doi.org/10.1007/s40279-015-0454-0). Review. **Verified** (abstract only).
- Schoenfeld BJ, Ogborn D, Krieger JW (2017). *Dose-response relationship between weekly resistance training volume and increases in muscle mass.* J Sports Sci 35(11):1073–1082. doi:[10.1080/02640414.2016.1210197](https://doi.org/10.1080/02640414.2016.1210197). 15 studies, 34 groups. Uses weekly sets per muscle as the volume measure. **Verified** (abstract).
- NIST Special Publication 811, *Guide for the Use of the International System of Units*, Appendix B and its footnotes. **Verified** ([footnotes](https://www.nist.gov/pml/special-publication-811/nist-guide-si-footnotes)).

### Definition

```
volume load (kg) = sum over sets of (repetitions × external load in kg)
                 = sets × reps × load, when every set is the same
```

McBride defines volume load as "repetitions [no.] x external load [kg]." They compared it with three other methods: a version that adds body mass minus shank mass, time under tension, and mechanical work (force × displacement). The methods gave "substantially different values" for protocols of different intensity. Jump squats at 0% of one-repetition maximum have zero volume load by the external-load definition.

### Per-implement loads

I found no published standard for dumbbells, kettlebells, or other paired implements. A clear rule is to store the load of one implement and the number of implements moved in each repetition. For example, two 20 kg dumbbells lifted together is 40 kg per repetition. (This is a recording convention, not a sourced rule.)

### Pounds to kilograms

```
1 lb (avoirdupois) = 0.45359237 kg exactly
```

NIST SP 811 footnote: "The exact conversion factor is 4.535 923 7 E-01. All units in Secs. B.8 and B.9 that contain the pound refer to the avoirdupois pound." The main NIST table prints a rounded 4.535 924 E−01. Use the exact footnote value. The value dates from the international yard and pound agreement, in force in the United States from 1 July 1959. I could not read the 1959 Federal Register notice text (it is a scanned image).

### Known limits

- Volume load ignores how close the load is to maximum, how close the set is to failure, range of motion, bar speed, and rest between sets. Scott 2016 says percent of one-repetition maximum "does not account for the effects of inter-set recovery periods, repetition velocity, or the number of repetitions performed in each set."
- Volume load is not comparable across exercises. A kilogram squatted is not a kilogram curled.
- It leaves out body mass in bodyweight movements.
- Scott 2016: there is "not yet a consensus" on how to combine resistance training measures.

### Consumer wearable data

Compute volume load per exercise from logged sets, store the original unit and the number of implements, convert pounds with 0.45359237 exactly, and do not add volume load across different exercises into one stress number.

---

## 11. Cardiac drift and aerobic decoupling

### Sources

- Coyle EF, González-Alonso J (2001). *Cardiovascular drift during prolonged exercise: new perspectives.* Exerc Sport Sci Rev 29(2):88–92. doi:[10.1097/00003677-200104000-00009](https://doi.org/10.1097/00003677-200104000-00009). Review. Their thesis: drift, "characterized by a progressive decline in stroke volume after 10-20 min of exercise, is primarily due to increased heart rate rather than a progressive increase in cutaneous blood flow as body temperature rises." **Verified** (abstract).
- Wingo JE, Ganio MS, Cureton KJ (2012). *Cardiovascular drift during heat stress: implications for exercise prescription.* Exerc Sport Sci Rev 40(2):88–94. doi:[10.1097/JES.0b013e31824c43af](https://doi.org/10.1097/JES.0b013e31824c43af). Drift is "the progressive increase in heart rate and decrease in stroke volume that begins after approximately 10 min of prolonged moderate-intensity exercise." It goes with a lower maximal oxygen uptake, especially in heat. So "the increased heart rate reflects an increased relative metabolic intensity." **Verified** (abstract).
- González-Alonso J, Mora-Rodríguez R, Below PR, Coyle EF (1997). J Appl Physiol 82(4):1229–1236. doi:[10.1152/jappl.1997.82.4.1229](https://doi.org/10.1152/jappl.1997.82.4.1229). 15 endurance-trained cyclists. Dehydration of 4% body weight on top of high body temperature cut stroke volume by 26 ml per beat and cardiac output by 13%. **Verified** (abstract).
- Coaching method: Friel J. *Aerobic endurance and decoupling.* TrainingPeaks article (updated 2026). It compares "the first half of a steady effort to the second half." "< 5% decoupling: Strong aerobic endurance at that intensity." Suggested tests: 2–4 hours at aerobic threshold on the bike, 1–2 hours running, with warm-up and cool-down left out. Heat and dehydration are named as causes of high decoupling. **Verified** ([article](https://www.trainingpeaks.com/learn/articles/aerobic-endurance-and-decoupling/)). The article does not give the arithmetic or say whether power is average or normalized.
- Smyth B, Maunder E, Meyler S, Hunter B, Muniz-Pumares D (2022). *Decoupling of internal and external workload during a marathon: an analysis of durability in 82,303 recreational runners.* Sports Med 52(9):2283–2295. doi:[10.1007/s40279-022-01680-5](https://doi.org/10.1007/s40279-022-01680-5). **Verified** (abstract).
  - Population: 82,303 marathon runners (13,125 women).
  - Internal load: percent of maximum heart rate. External load: speed relative to estimated critical speed.
  - Decoupling: the internal-to-external ratio in the 35–40 km segment relative to the 5–10 km segment. Low below 1.1, moderate 1.1 to below 1.2, high 1.2 or more. Onset is where decoupling first exceeds 1.025.
  - Mean decoupling 1.16 ± 0.22, first seen at 25.2 ± 9.9 km.

### Definitions

```
efficiency factor (EF) = output / heart rate          # output = speed, pace converted to speed, or power
decoupling (%)         = (EF_first_half - EF_second_half) / EF_first_half × 100
HR drift               = change in heart rate over time at about constant output
```

The decoupling formula is Friel's, as reported in coaching sources. **Not verified** in a Friel primary text.

### Why heart-rate drift alone is not efficiency

- Drift has several causes: high body temperature, dehydration, and a falling stroke volume (Coyle; González-Alonso). These are not changes in fitness.
- Wingo: when drift happens, the same heart rate means a higher relative intensity. So heart rate stops tracking the work done.
- Mechanical efficiency is work done divided by energy used. Heart rate does not measure energy used.
- Decoupling relates output to heart rate. It still changes with heat, hydration, terrain, wind, fueling, and pacing.

### Minimum data

- A steady segment long enough to split in two.
- Known output (speed or power).
- A note on heat, terrain, and fueling.

### Consumer wearable data

Report heart-rate drift and output-to-heart-rate decoupling as separate numbers, require a steady segment with known output and conditions, and do not label either one "efficiency."

---

## 12. Correlation of daily time series

### Sources

- Dean RT, Dunsmuir WTM (2016). *Dangers and uses of cross-correlation in analyzing time series in perception, performance, movement, and neuroscience: the importance of constructing transfer function autoregressive models.* Behav Res Methods 48(2):783–802. doi:[10.3758/s13428-015-0611-2](https://doi.org/10.3758/s13428-015-0611-2). Simulation and worked examples. **Verified** (abstract).
  - "Most such series are individually autocorrelated: they do not comprise independent values."
  - "Such cross-correlations can indicate spurious relationships, because of autocorrelation."
  - "even a genuine cross-correlation function does not answer key motivating questions, such as whether there are likely causal relationships between the series."
  - Remedies they list: check stationarity, prewhiten, then fit transfer-function models. "causal intervention experiments will likely also be needed."
- Bretherton CS, Widmann M, Dymnikov VP, Wallace JM, Bladé I (1999). *The effective number of spatial degrees of freedom of a time-varying field.* J Climate 12(7):1990–2009. **Verified** (full text). For two series that each follow a first-order autoregressive pattern, with lag-1 autocorrelations r_X and r_Y, their equation 31 gives the effective sample size:

  ```
  T* = T × (1 - r_X × r_Y) / (1 + r_X × r_Y)
  ```

  They state this case "was derived even earlier by Bartlett (1935)." Their general form (equation 30) is `T* = T / sum_{tau=-(T-1)..T-1} (1 - |tau|/T) × rho_X(tau) × rho_Y(tau)`.
- Bakdash JZ, Marusich LR (2017). *Repeated measures correlation.* Front Psychol 8:456. doi:[10.3389/fpsyg.2017.00456](https://doi.org/10.3389/fpsyg.2017.00456). Simple correlation on non-independent data "may produce biased, specious results." **Verified** (abstract).
- Schönbrodt FD, Perugini M (2013). *At what sample size do correlations stabilize?* J Res Pers 47(5):609–612. doi:[10.1016/j.jrp.2013.05.009](https://doi.org/10.1016/j.jrp.2013.05.009). Simulations with independent observations. For typical cases, sample size "should approach 250" for a stable estimate. **Secondary**.

### Worked numbers (my computation)

95% confidence intervals for Pearson r using the Fisher z method. This assumes independent days.

| Days | CI if r = 0.3 | CI if r = 0.5 | Smallest r with p < 0.05 |
|---|---|---|---|
| 7 | −0.59 to 0.86 | −0.41 to 0.91 | 0.75 |
| 14 | −0.27 to 0.72 | −0.04 to 0.81 | 0.53 |
| 28 | −0.08 to 0.61 | 0.16 to 0.74 | 0.37 |
| 60 | 0.05 to 0.51 | 0.28 to 0.67 | 0.25 |
| 90 | 0.10 to 0.48 | 0.33 to 0.64 | 0.21 |

Autocorrelation shrinks the real day count. Using equation 31, with both series at lag-1 autocorrelation 0.5, the factor is 0.6. So 28 days act like about 17 independent days. At 0.7 the factor is 0.34, so 28 days act like about 10.

### Minimum days

I found no published minimum for correlating one person's daily wearable series. The table shows that two to four weeks give very wide intervals. Autocorrelation makes them wider still.

### Correlation and causation

- Training load can drive both HRV and sleep. That alone makes them correlate.
- The direction can run either way.
- Shared trends, such as both rising over a training block, create correlation without any link. Remove trends or use day-to-day changes first.
- Dean and Dunsmuir: only intervention experiments settle cause.

### Consumer wearable data

Report a daily correlation only with its day count, a confidence interval, and a note on autocorrelation (or after removing trends), and word it as an association, never a cause.

---

## 13. Coverage and missing data

### Sources

- Sterne JAC, White IR, Carlin JB, Spratt M, Royston P, Kenward MG, Wood AM, Carpenter JR (2009). *Multiple imputation for missing data in epidemiological and clinical research: potential and pitfalls.* BMJ 338:b2393. doi:[10.1136/bmj.b2393](https://doi.org/10.1136/bmj.b2393). **Verified** (PMC full text). Box 1 definitions:
  - Missing completely at random: "There are no systematic differences between the missing values and the observed values."
  - Missing at random: "Any systematic difference between the missing values and the observed values can be explained by differences in observed data."
  - Missing not at random: "Even after the observed data are taken into account, systematic differences remain between the missing values and the observed values."
- Baroudi L, Zernicke RF, Tewari M, Carlozzi NE, Choi SW, Cain SM (2025). *Using wear time for the analysis of consumer-grade wearables' data: case study using Fitbit data.* JMIR mHealth uHealth 13:e46149. doi:[10.2196/46149](https://doi.org/10.2196/46149). **Verified** (PMC full text).
  - Population: three Fitbit datasets, six groups (caregivers in three disease settings, university students, children with cancer and their caregivers).
  - Average daily wear ranged from about 9.8 to about 20.8 hours. Wear fell over time in every group.
  - Changing the valid-day rule moved group step counts by up to almost 2,000 steps in the low-wear group, and by under 15 steps in high-wear groups.
  - In four of six groups, about 15% of people changed by more than 1,000 steps.
  - "Average daily heart rate appeared to be robust to changes in wear time."
  - Dropping the device over time "can potentially create a bias in results over time."
  - They advise a full analysis of wear time and to "systematically document" it.
- How the sources above handled gaps:
  - Plews 2014: at least 3 valid days per week for HRV.
  - Quer 2020: only days with 20 or more hours of wear.
  - Phillips 2017: SRI only from pairs where both epochs are present, over whole weeks.
  - Fischer 2021: tested regularity metrics with one day in seven missing.

### Known limits

- Wearable gaps are rarely random. A person may skip the device on nights of illness, travel, or drinking. Those are the nights that would change the average. That is "missing not at random." It cannot be fixed by imputation that relies only on the data you have.
- A 7-day average from 3 days is noisier than one from 7 days, even though both are called a "7-day average."
- Gaps break day-pair measures such as SRI and day-to-day differences more than they break simple means.

### Consumer wearable data

Report the number of valid days and the rule that defines a valid day beside every summary, refuse to compute a summary when coverage falls below its stated minimum, and never fill gaps silently.

---

## Claims I could not verify

1. Whether the 0.64 (men) and 0.86 (women) multipliers appear in Morton 1990 or Banister 1991. Both originals were closed.
2. Whether Apple's Recovery HRV is RMSSD, and whether Apple Watch writes `heartRateVariabilityRMSSD` samples.
3. Apple's HRV window length and schedule, and the Breathe or Mindfulness trigger.
4. That Garmin overnight HRV is RMSSD. Only secondary sources say so.
5. Garmin's "lowest 30-minute average" resting heart rate, from Garmin's own page. Seen only as quoted by Dial 2025.
6. WHOOP's weighting toward the last deep sleep, from WHOOP's own page. Seen only as quoted by Dial 2025 and described by Bellenger 2021.
7. The cool-down protocol in Cole 1999 itself (1.5 mph, 2.5% grade is from the same group's 2000 JAMA paper).
8. The ACSM 2011 percent-of-maximum bands from the original Garber table.
9. The Edwards bands and weights from the original book.
10. Foster 2001's 30-minute rating delay, from Foster's own text.
11. The exact band arithmetic for Plews' "0.5 × CV" rule.
12. Kiviniemi 2007 recording posture.
13. AASM scoring manual formulas (TRT, WASO, SE), from the manual itself.
14. Friel's decoupling arithmetic, and whether TrainingPeaks uses average or normalized power.
15. Any research source for a "lowest 5-minute average" resting heart rate. None found.

---

## Reference list

1. Task Force of the European Society of Cardiology and the North American Society of Pacing and Electrophysiology. Heart rate variability: standards of measurement, physiological interpretation, and clinical use. Circulation. 1996;93(5):1043–1065. doi:10.1161/01.CIR.93.5.1043. Also Eur Heart J. 1996;17(3):354–381.
2. Shaffer F, Ginsberg JP. An overview of heart rate variability metrics and norms. Front Public Health. 2017;5:258. doi:10.3389/fpubh.2017.00258.
3. Munoz ML, van Roon A, Riese H, et al. Validity of (ultra-)short recordings for heart rate variability measurements. PLoS One. 2015;10(9):e0138921. doi:10.1371/journal.pone.0138921.
4. Esco MR, Flatt AA. Ultra-short-term heart rate variability indexes at rest and post-exercise in athletes: evaluating the agreement with accepted recommendations. J Sports Sci Med. 2014;13(3):535–541.
5. Apple Inc. HealthKit documentation: heartRateVariabilitySDNN; heartRateVariabilityRMSSD; restingHeartRate; heartRateRecoveryOneMinute. https://developer.apple.com/documentation/healthkit (accessed 2026-11-15).
6. Apple Inc. Your heart rate. What it means, and where on Apple Watch you'll find it. Support article HT204666. https://support.apple.com/en-us/HT204666 (accessed 2026-11-15).
7. WHOOP Inc. WHOOP Developer API, OpenAPI specification (RecoveryScore, SleepScore). https://api.prod.whoop.com/developer/doc/openapi.json (accessed 2026-11-15).
8. Oura Health. Heart Rate Variability. Support article 360025441974. https://support.ouraring.com/hc/en-us/articles/360025441974 (accessed 2026-11-15).
9. Oura Health. Resting Heart Rate. Support article 360025588793. https://support.ouraring.com/hc/en-us/articles/360025588793 (accessed 2026-11-15).
10. Oura Health. West Virginia University validates Oura's HRV against popular commercial tools. Oura blog, 2021. https://ouraring.com/blog/west-virginia-university-validates-oura/
11. Garmin Ltd. Owner's manual: Heart Rate Variability Status. https://www8.garmin.com/manuals/webhelp/GUID-3D8FCD7B-7C56-43A9-8665-5CDE4FFD7DF3/EN-US/GUID-9282196F-D969-404D-B678-F48A13D8D0CB.html (accessed 2026-11-15).
12. Garmin Health. Garmin Enhanced BBI: an example night. White paper, November 2023. https://www8.garmin.com/garminhealth/news/Garmin-Enhanced-BBI_Final.pdf
13. Bellenger CR, Miller DJ, Halson SL, Roach GD, Sargent C. Wrist-based photoplethysmography assessment of heart rate and heart rate variability: validation of WHOOP. Sensors. 2021;21(10):3571. doi:10.3390/s21103571.
14. Dial MB, Hollander ME, Vatne EA, Emerson AM, Edwards NA, Hagen JA. Validation of nocturnal resting heart rate and heart rate variability in consumer wearables. Physiol Rep. 2025;13(16):e70527. doi:10.14814/phy2.70527.
15. Plews DJ, Laursen PB, Kilding AE, Buchheit M. Heart rate variability in elite triathletes, is variation in variability the key to effective training? A case comparison. Eur J Appl Physiol. 2012;112(11):3729–3741. doi:10.1007/s00421-012-2354-4.
16. Plews DJ, Laursen PB, Kilding AE, Buchheit M. Evaluating training adaptation with heart-rate measures: a methodological comparison. Int J Sports Physiol Perform. 2013;8(6):688–691. doi:10.1123/ijspp.8.6.688.
17. Plews DJ, Laursen PB, Stanley J, Kilding AE, Buchheit M. Training adaptation and heart rate variability in elite endurance athletes: opening the door to effective monitoring. Sports Med. 2013;43(9):773–781. doi:10.1007/s40279-013-0071-8.
18. Plews DJ, Laursen PB, Le Meur Y, Hausswirth C, Kilding AE, Buchheit M. Monitoring training with heart rate-variability: how much compliance is needed for valid assessment? Int J Sports Physiol Perform. 2014;9(5):783–790. doi:10.1123/ijspp.2013-0455.
19. Plews DJ. The practical application of heart rate variability: monitoring training adaptation in world class athletes [doctoral thesis]. Auckland University of Technology; 2014. hdl:10292/7122.
20. Buchheit M. Monitoring training status with HR measures: do all roads lead to Rome? Front Physiol. 2014;5:73. doi:10.3389/fphys.2014.00073.
21. Kiviniemi AM, Hautala AJ, Kinnunen H, Tulppo MP. Endurance training guided individually by daily heart rate variability measurements. Eur J Appl Physiol. 2007;101(6):743–751. doi:10.1007/s00421-007-0552-2.
22. Vesterinen V, Nummela A, Heikura I, Laine T, Hynynen E, Botella J, Häkkinen K. Individual endurance training prescription with heart rate variability. Med Sci Sports Exerc. 2016;48(7):1347–1354. doi:10.1249/MSS.0000000000000910.
23. Quer G, Gouda P, Galarnyk M, Topol EJ, Steinhubl SR. Inter- and intraindividual variability in daily resting heart rate and its associations with age, sex, sleep, BMI, and time of year: retrospective, longitudinal cohort study of 92,457 adults. PLoS One. 2020;15(2):e0227709. doi:10.1371/journal.pone.0227709.
24. Cole CR, Blackstone EH, Pashkow FJ, Snader CE, Lauer MS. Heart-rate recovery immediately after exercise as a predictor of mortality. N Engl J Med. 1999;341(18):1351–1357. doi:10.1056/NEJM199910283411804.
25. Cole CR, Foody JM, Blackstone EH, Lauer MS. Heart rate recovery after submaximal exercise testing as a predictor of mortality in a cardiovascularly healthy cohort. Ann Intern Med. 2000;132(7):552–555. doi:10.7326/0003-4819-132-7-200004040-00007.
26. Nishime EO, Cole CR, Blackstone EH, Pashkow FJ, Lauer MS. Heart rate recovery and treadmill exercise score as predictors of mortality in patients referred for exercise ECG. JAMA. 2000;284(11):1392–1398. doi:10.1001/jama.284.11.1392.
27. Watanabe J, Thamilarasan M, Blackstone EH, Thomas JD, Lauer MS. Heart rate recovery immediately after treadmill exercise and left ventricular systolic dysfunction as predictors of mortality: the case of stress echocardiography. Circulation. 2001;104(16):1911–1916. doi:10.1161/circ.104.16.1911.
28. Shetler K, Marcus R, Froelicher VF, et al. Heart rate recovery: validation and methodologic issues. J Am Coll Cardiol. 2001;38(7):1980–1987. doi:10.1016/S0735-1097(01)01652-7.
29. Daanen HAM, Lamberts RP, Kallen VL, Jin A, Van Meeteren NLU. A systematic review on heart-rate recovery to monitor changes in training status in athletes. Int J Sports Physiol Perform. 2012;7(3):251–260. doi:10.1123/ijspp.7.3.251.
30. Garber CE, Blissmer B, Deschenes MR, et al. Quantity and quality of exercise for developing and maintaining cardiorespiratory, musculoskeletal, and neuromotor fitness in apparently healthy adults: guidance for prescribing exercise. Med Sci Sports Exerc. 2011;43(7):1334–1359. doi:10.1249/MSS.0b013e318213fefb.
31. Pollock ML, Gaesser GA, Butcher JD, et al. ACSM position stand: the recommended quantity and quality of exercise for developing and maintaining cardiorespiratory and muscular fitness, and flexibility in healthy adults. Med Sci Sports Exerc. 1998;30(6):975–991. doi:10.1097/00005768-199806000-00032.
32. Edwards S. The Heart Rate Monitor Book. 1993 (also cited as 1992). Not accessed.
33. Tanaka H, Monahan KD, Seals DR. Age-predicted maximal heart rate revisited. J Am Coll Cardiol. 2001;37(1):153–156. doi:10.1016/S0735-1097(00)01054-8.
34. Nes BM, Janszky I, Wisløff U, Støylen A, Karlsen T. Age-predicted maximal heart rate in healthy subjects: the HUNT fitness study. Scand J Med Sci Sports. 2013;23(6):697–704. doi:10.1111/j.1600-0838.2012.01445.x.
35. Gellish RL, Goslin BR, Olson RE, McDonald A, Russi GD, Moudgil VK. Longitudinal modeling of the relationship between age and maximal heart rate. Med Sci Sports Exerc. 2007;39(5):822–829. doi:10.1097/mss.0b013e31803349c6.
36. Morton RH, Fitz-Clarke JR, Banister EW. Modeling human performance in running. J Appl Physiol. 1990;69(3):1171–1177. doi:10.1152/jappl.1990.69.3.1171.
37. Banister EW. Modeling elite athletic performance. In: MacDougall JD, Wenger HA, Green HJ, eds. Physiological Testing of the High-Performance Athlete. 2nd ed. Champaign, IL: Human Kinetics; 1991:403–424. Not accessed.
38. Uchida MC, Teixeira LFM, Godoi VJ, Marchetti PH, Conte M, Coutts AJ, Bacurau RFP. Does the timing of measurement alter session-RPE in boxers? J Sports Sci Med. 2014;13(1):59–65.
39. Foster C, Florhaug JA, Franklin J, et al. A new approach to monitoring exercise training. J Strength Cond Res. 2001;15(1):109–115.
40. Haddad M, Stylianides G, Djaoui L, Dellal A, Chamari K. Session-RPE method for training load monitoring: validity, ecological usefulness, and influencing factors. Front Neurosci. 2017;11:612. doi:10.3389/fnins.2017.00612.
41. Crawford DA, Drake NB, Carper MJ, DeBlauw J, Heinrich KM. Validity, reliability, and application of the session-RPE method for quantifying training loads during high intensity functional training. Sports. 2018;6(3):84. doi:10.3390/sports6030084.
42. Menghini L, Cellini N, Goldstone A, Baker FC, de Zambotti M. A standardized framework for testing the performance of sleep-tracking technology: step-by-step guidelines and open-source code. Sleep. 2021;44(2):zsaa170. doi:10.1093/sleep/zsaa170.
43. Shrivastava D, Jung S, Saadat M, Sirohi R, Crewson K. How to interpret the results of a sleep study. J Community Hosp Intern Med Perspect. 2014;4(5):24983. doi:10.3402/jchimp.v4.24983.
44. Reed DL, Sacco WP. Measuring sleep efficiency: what should the denominator be? J Clin Sleep Med. 2016;12(2):263–266. doi:10.5664/jcsm.5498.
45. Iber C, Ancoli-Israel S, Chesson A, Quan SF. The AASM Manual for the Scoring of Sleep and Associated Events: Rules, Terminology and Technical Specifications. Westchester, IL: American Academy of Sleep Medicine; 2007 (and later editions). Not accessed.
46. Phillips AJK, Clerx WM, O'Brien CS, et al. Irregular sleep/wake patterns are associated with poorer academic performance and delayed circadian and sleep/wake timing. Sci Rep. 2017;7:3216. doi:10.1038/s41598-017-03171-4.
47. Lunsford-Avery JR, Engelhard MM, Navar AM, Kollins SH. Validation of the Sleep Regularity Index in older adults and associations with cardiometabolic risk. Sci Rep. 2018;8:14158. doi:10.1038/s41598-018-32402-5. Errata: Sci Rep. 2020;10:2993; Sci Rep. 2021;11:24398.
48. Fischer D, Klerman EB, Phillips AJK. Measuring sleep regularity: theoretical properties and practical usage of existing metrics. Sleep. 2021;44(10):zsab103. doi:10.1093/sleep/zsab103.
49. Fischer D, McHill AW, Sano A, et al. Irregular sleep and event schedules are associated with poorer self-reported well-being in US college students. Sleep. 2020;43(6):zsz300. doi:10.1093/sleep/zsz300.
50. McBride JM, McCaulley GO, Cormie P, Nuzzo JL, Cavill MJ, Triplett NT. Comparison of methods to quantify volume during resistance exercise. J Strength Cond Res. 2009;23(1):106–110. doi:10.1519/JSC.0b013e31818efdfe.
51. Scott BR, Duthie GM, Thornton HR, Dascombe BJ. Training monitoring for resistance exercise: theory and applications. Sports Med. 2016;46(5):687–698. doi:10.1007/s40279-015-0454-0.
52. Schoenfeld BJ, Ogborn D, Krieger JW. Dose-response relationship between weekly resistance training volume and increases in muscle mass: a systematic review and meta-analysis. J Sports Sci. 2017;35(11):1073–1082. doi:10.1080/02640414.2016.1210197.
53. Thompson A, Taylor BN. Guide for the Use of the International System of Units (SI). NIST Special Publication 811, Appendix B and footnotes. https://www.nist.gov/pml/special-publication-811 (accessed 2026-11-15).
54. National Bureau of Standards. Refinement of values for the yard and the pound. Federal Register, 1959. Not read (scanned image).
55. Coyle EF, González-Alonso J. Cardiovascular drift during prolonged exercise: new perspectives. Exerc Sport Sci Rev. 2001;29(2):88–92. doi:10.1097/00003677-200104000-00009.
56. Wingo JE, Ganio MS, Cureton KJ. Cardiovascular drift during heat stress: implications for exercise prescription. Exerc Sport Sci Rev. 2012;40(2):88–94. doi:10.1097/JES.0b013e31824c43af.
57. González-Alonso J, Mora-Rodríguez R, Below PR, Coyle EF. Dehydration markedly impairs cardiovascular function in hyperthermic endurance athletes during exercise. J Appl Physiol. 1997;82(4):1229–1236. doi:10.1152/jappl.1997.82.4.1229.
58. Friel J. Aerobic endurance and decoupling. TrainingPeaks, updated 2026. https://www.trainingpeaks.com/learn/articles/aerobic-endurance-and-decoupling/
59. Smyth B, Maunder E, Meyler S, Hunter B, Muniz-Pumares D. Decoupling of internal and external workload during a marathon: an analysis of durability in 82,303 recreational runners. Sports Med. 2022;52(9):2283–2295. doi:10.1007/s40279-022-01680-5.
60. Dean RT, Dunsmuir WTM. Dangers and uses of cross-correlation in analyzing time series in perception, performance, movement, and neuroscience: the importance of constructing transfer function autoregressive models. Behav Res Methods. 2016;48(2):783–802. doi:10.3758/s13428-015-0611-2.
61. Bretherton CS, Widmann M, Dymnikov VP, Wallace JM, Bladé I. The effective number of spatial degrees of freedom of a time-varying field. J Climate. 1999;12(7):1990–2009.
62. Bartlett MS. Some aspects of the time-correlation problem in regard to tests of significance. J R Stat Soc. 1935;98(3):536–543. Not accessed; cited through reference 61.
63. Bakdash JZ, Marusich LR. Repeated measures correlation. Front Psychol. 2017;8:456. doi:10.3389/fpsyg.2017.00456.
64. Schönbrodt FD, Perugini M. At what sample size do correlations stabilize? J Res Pers. 2013;47(5):609–612. doi:10.1016/j.jrp.2013.05.009.
65. Sterne JAC, White IR, Carlin JB, et al. Multiple imputation for missing data in epidemiological and clinical research: potential and pitfalls. BMJ. 2009;338:b2393. doi:10.1136/bmj.b2393.
66. Baroudi L, Zernicke RF, Tewari M, Carlozzi NE, Choi SW, Cain SM. Using wear time for the analysis of consumer-grade wearables' data: case study using Fitbit data. JMIR Mhealth Uhealth. 2025;13:e46149. doi:10.2196/46149.
