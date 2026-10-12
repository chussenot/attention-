# P2 development results: commentary before the held-out phase

Hand-written. It interprets [`p2-dev-results.md`](p2-dev-results.md) and changes nothing in
it. The generated report stays byte-identical to what `p2 report-from-archive` rebuilds from
the raw archive. No hypothesis, estimand, threshold, parameter range or selection rule is
changed here. Every verdict stands as generated.

This note is committed before any held-out run, so that the expectations in §2 are on record
as predictions, not as explanations written after the fact.

## 1. Mechanisms work; no tuned policy meets the targets

- **Mechanism effects.** H1, H2 and H4 are supported, with large effects:
  - habituation −0.55;
  - novelty −0.39;
  - stochastic selection −0.34.
- **Policy targets.** No configuration of any parameterised arm meets both provisional
  targets (miss rate ≤ 5% and ≤ 2.5 switches per 100). H5 is not met for any arm: 0/512
  configurations pass, for all seven arms.
- **The selected policy.** X\* = A2 was chosen by fallback rule 2. It still misses 54% of
  critical signals.
- **Every arm trades misses for switches.** No arm lowers both:

  | Arm | Miss rate | Switches per 100 |
  |---|---:|---:|
  | RND | 7.5% | 84 |
  | RR | 7.5% | 100 |
  | A0 | 100% | 0 |

  A3 averaged over all 512 configurations misses 6.3%, but switches far more often than A2
  (+48 per 100).
- **What the evidence supports.** It supports the mechanisms, not a usable policy.

**Interpretation, not a protocol result.** Family G's critical weight gaps (0.15–0.45) overlap
its stable dominant-to-background gaps (0.25–0.85). An arbiter that sees only weights and
selection history may therefore be unable to meet both targets on G, however it is tuned. If
so, the H5 failure reflects the benchmark's difficulty under these targets as much as the
mechanisms. This has not been tested. A descriptive miss/switch frontier over the archived
7 × 512 configurations would test it. That analysis is exploratory and is deferred until after
the freeze.

## 2. H7 is expected to fail on the stability margin

- **How X\* was chosen.** Nothing passed, so rule 2 picked the lowest miss rate among
  candidates whose switch-rate **point estimate** is at most 2.5.
- **What H7 requires.** On each held-out set, the **upper bound** of the 95% interval for the
  switch-rate increase over A0 must be at most 2.5.
- **The development contrast already fails that test.** A0 switches 0 times, so the contrast
  equals A2's own switch rate. On development data that is 2.30 [1.21, 3.66]. The upper bound
  of 3.66 is already above 2.5.
- **The held-out sets will not narrow the interval.** G held-out block 1 has the same size as
  the development block (200 instances).
- **Selection pushes the other way.** Among 512 × 7 candidates, rule 2 favours ones whose
  switch rate fell under 2.5 partly by chance. Regression toward the mean should raise the
  held-out switch rate.

**Prediction recorded now:**
- H7 fails, at least on G held-out block 1.
- The miss-rate condition most likely holds: A2's development contrast is −0.46 [−0.54, −0.38].
- The stability condition most likely fails.

A failed H7 is a valid pre-registered result, and the final report states it as a failure.

**Cause.** This comes from the protocol's design: the configuration is selected on point
estimates but tested on an interval bound. It is recorded here as a lesson for the next
pre-registration, not fixed by amendment. Amending the rule after seeing development results
would make it outcome-driven.

## 3. H3 "redundant" was forced by the ceiling

A0 misses every critical instance. With A0 = 1, the interaction is

  I = A3 − A2 − A1 + 1.

The parameter-averaged miss rates are A1 = 0.245, A2 = 0.412 and A3 = 0.063. These are
derived from the report's simple effects. The constraint follows from them:

- Complementarity (I < 0) would need A3 < A1 + A2 − 1 = −0.343.
- A miss rate cannot be negative.
- So once the single-mechanism arms were this strong, **I ≥ 0.343 was guaranteed**.
- The observed I = 0.405 exceeds that floor by exactly A3's own miss rate, 0.063.

The floor diagnostic points to the same thing: A1 and A2 both detect the signal in 46% of
(configuration, instance) cells, and each such cell adds at least +1 to I.

**Reading.** H3's verdict is "redundant", as generated. In this design, though, the test could
not have found complementarity. The verdict tells us nothing about whether habituation and
novelty interact beyond what the ceiling forces. A future protocol should test interaction
where the baseline does not miss everything, or only on instances where both single
mechanisms miss.

## 4. Configuration-sampling error is comparable to, or larger than, the bootstrap error

The 98.75% intervals come from resampling instances only. The Monte-Carlo error from using 512
vectors (protocol §5.2) is reported separately and is **not** included. Comparing standard
errors on the same footing:

| Hypothesis | Bootstrap SE (from interval width) | Configuration MC SE | MC / bootstrap | Combined 98.75% interval (descriptive) |
|---|---:|---:|---:|---|
| H1 | 0.0048 | 0.0120 | 2.5 | [−0.584, −0.520] |
| H2 | 0.0086 | 0.0104 | 1.2 | [−0.419, −0.351] |
| H3 | 0.0211 | 0.0169 | 0.8 | [+0.338, +0.473] |
| H4 | 0.0063 | 0.0073 | 1.2 | [−0.363, −0.315] |

How the table was computed:
- Bootstrap SE = interval half-width / z, with z = 2.4977 (the normal quantile at 98.75%).
- The combined interval adds the two variances in quadrature. That is an approximation for a
  crossed instances × configurations design. A two-way bootstrap would be exact.

Reading:
- For H1, configuration sampling dominates: 2.5× the instance error.
- No verdict changes: every estimate is at least 15 combined standard errors from zero.
- The 98.75% intervals in the report should not be read as the full uncertainty of the
  parameter-averaged effects.

## 5. Lessons for the next pre-registration (not applied to P2)

- Select on the same quantity that is tested. For example, filter rule-2 candidates on the
  interval bound, or test H7 on point estimates.
- Test interactions away from a ceiling (§3).
- Put configuration sampling into the interval with a two-way bootstrap, or use enough
  vectors that its error is negligible.
- Check, before fixing joint targets, that they can be met on the benchmark at all (§1).
