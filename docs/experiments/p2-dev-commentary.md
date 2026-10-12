# P2 development results: commentary before the held-out phase

Hand-written. It interprets [`p2-dev-results.md`](p2-dev-results.md) and changes nothing in
it. The generated report stays byte-identical to what `p2 report-from-archive` rebuilds from
the raw archive. No hypothesis, estimand, threshold, parameter range or selection rule is
changed here. Every verdict stands as generated.

This note is committed before any held-out run, so that the expectations in §2 are on record
as predictions, not as explanations written after the fact.

## 1. H5 failed because of the mechanisms' features, not because G is unsolvable

**What the protocol establishes:**
- H1, H2 and H4 are supported, with large effects:
  - habituation −0.55;
  - novelty −0.39;
  - stochastic selection −0.34.
- No configuration of any parameterised arm meets both provisional targets (miss rate ≤ 5% and
  ≤ 2.5 switches per 100). H5 is not met for any arm: 0/512 configurations pass, for all seven
  arms.
- X\* = A2 was chosen by fallback rule 2. It still misses 54% of critical signals.

The evidence supports the mechanisms, not a usable policy.

**Distance from the target box.** This is an exploratory analysis of the archived development
outcomes, done before the freeze. It involved no new runs and feeds no frozen quantity. It
covers the 7 × 512 parameterised configurations. Rates are stratum-weighted (§5.1) with the
crate's `f64` arithmetic.

| Constraint | Best achievable | Where |
|---|---|---|
| Switch rate ≤ 2.5 | Miss rate 54.2% | A2. Four other A2 configurations tie on miss rate; the selection rule's switch-rate tie-break picks c = 389 = c\*. |
| Miss rate ≤ 5% | 34.4 switches per 100 | S1, c = 324 |

The second figure sits exactly on the 5% boundary. Its miss rate is 6/120 in exact arithmetic
and evaluates to `0.049999999999999996`. Several configurations are at that boundary:
- S1, c = 324: 34.4 switches per 100;
- S2, c = 318: 37.4;
- A3, c = 380: 37.5.

Excluding the boundary gives 38.3 (S2, c = 115). Whichever reading is used, the gap is about
14–15 times the switch target.

**Where the stability cost comes from.** Development data, A2:
- **C4 costs nothing.** A2's switch rate on C4 is 0.0 at all 512 configurations.
- **All of A2's stability cost is C5:** 4.60 per 100 at c\*.
- **A2 at c\* fails most on small onsets.** Miss rates by class:
  - C1 85% (the jump from `w_pre` is small);
  - C2 43%;
  - C3 35%.

**Why the pre-registered features fall short (interpretation).** Novelty rewards rises only,
and its bonus decays with ρ. In C5, every fluctuating source draws a fresh weight every tick, so
its random rises look exactly like onsets to novelty. The mechanism cannot tell noise from an
onset with the signal it uses. Habituation cannot either: it acts on win history, not on the
dynamics of a source's weight.

**G can be solved with weights and their history (interpretation, not yet tested).** Per the
generator spec (§7.1 and Amendment 1 item 5):
- **Noise has no persistence.** Each C5 fluctuating source draws a fresh value from
  U[0.1, 0.6] every tick, so its weight essentially never repeats on consecutive ticks.
- **Every critical stream persists.** At `T_on` it shifts level (C1) or appears (C2, C3). It
  then holds its weight for the rest of the run (C1, C2) or for m ≥ 3 ticks (C3).
- **Background and dominant sources** are constant everywhere except C5.

Consider a rule that gives one win to a source whose weight changed at t − 1 and held at t,
ignoring t = 0. It would fire only on critical sources: once, at `T_on + 1`, inside the
deadline and inside every C3 window. It would never fire on C4 or C5. That predicts:
- about 0% misses, at latency 1;
- 0 switches on stable instances;
- no false trigger from the C1 cases where the onset is a drop (`w_d − g < w_pre`, about 0.9%
  of draws), because the rule reacts to any change.

This uses only weights and their history, so the joint target can be met on G. An earlier
draft of this note conjectured the opposite, from the overlap between critical and stable weight
gaps. That conjecture is withdrawn: gap overlap only rules out arbiters that read weight
levels.

**A flaw in G itself.** Its noise is i.i.d. with no persistence, so the right detector separates
the two classes trivially. A real environment would not offer that. Results on G therefore say little
about how such a detector would do on realistic noise.

**Planned after the freeze (exploratory):**
- **Separability check.** A demonstration arbiter, run on the G development block only. It is
  built from the generator spec, with knowledge of G, so it is not a candidate policy. It must
  never be run on a held-out block as evidence.
- **Mechanism frontier.** The miss/switch frontier over the archived 7 × 512 configurations,
  reported as a separate result about the mechanisms.

## 2. H7 is expected to fail, decided by G held-out block 1 alone

**How X\* was chosen.** Nothing passed, so rule 2 picked the lowest miss rate among candidates
whose switch-rate **point estimate** is at most 2.5.

**What H7 requires.** On each held-out set, the **upper bound** of the 95% interval for the
switch-rate increase over A0 must be at most 2.5.

**Why it fails on G:**
- **The contrast equals A2's own switch rate.** On G, A0 never switches.
- **The development value already fails.** It is 2.30 [1.21, 3.66], and 3.66 is above 2.5.
- **The interval won't narrow on the held-out block.** G held-out block 1 has the same size and
  generator as the development block, with different seeds (10,000–10,199). Its interval width
  is roughly equal in expectation, not fixed.
- **Passing would need a halving.** The held-out point estimate would have to fall to about
  1.1, half its development value. That would require C5 to behave differently, and C5 comes
  from the same generator.
- **Selection pushes the other way.** Among 512 × 7 candidates, rule 2 favours ones whose
  switch rate fell under 2.5 partly by chance. Regression toward the mean should raise the
  held-out switch rate.

**Family B is open on both conditions.** The equality "contrast = A2's switch rate" holds only
where A0 never switches. That is true on G and unknown on Family B. Whether A0 misses every
critical instance there is also unknown.

**Prediction recorded now:**
- H7 not supported, decided by G held-out block 1 alone.
- The miss-rate condition is near-certain on G: A2's development contrast is −0.46
  [−0.54, −0.38].
- Both conditions are open on Family B, and Family B cannot rescue H7.

A failed H7 is a valid pre-registered result, and the final report states it as a failure.

**Cause and fix, for the next pre-registration.** The protocol selects on point estimates and
tests on an interval bound. The fix is to select on the same quantity that is tested:
- require the development upper bound to be ≤ 2.5; or
- select against a stricter margin than the one H7 tests.

This is not fixed here by amendment. Changing the rule after seeing development results would
make it outcome-driven.

## 3. H3 "redundant": forced by effect size, and true on the difference scale only

**The constraint.** A0 misses every critical instance. With A0 = 1, the interaction is

  I = A3 − A2 − A1 + 1.

The parameter-averaged miss rates are A1 = 0.2452, A2 = 0.4123 and A3 = 0.0626. These are
derived from the report's simple effects. Then:
- Complementarity (I < 0) would need A3 < A1 + A2 − 1 = −0.343. That is impossible.
- So once the single-mechanism results were in, I ≥ 0.343 was guaranteed.
- The observed I = 0.405 exceeds that floor by exactly A3's own miss rate.

**What forced it was effect size, not the design.** With A0 = 1, I < 0 means
A1 + A2 > 1 + A3. That can happen when both mechanisms are individually weak: for example,
A1 = A2 = 0.8 and A3 = 0.3 give I = −0.3. Here the single mechanisms removed 75 + 59 =
134 points of miss rate, more than the 100 available, so a negative interaction was impossible.

The floor diagnostic points the same way: A1 and A2 both detect the signal in 46% of
(configuration, instance) cells, and each such cell adds at least +1 to I.

**The verdict depends on the scale.** The pre-registered estimand compares differences in miss
rates. On that scale the result is redundant, and that verdict stands. On a ratio scale it reads
differently:
- A3·A0 / (A1·A2) = 0.0626 / 0.1011 = 0.62.
- Combined misses are 38% lower than multiplying the two separate reductions would predict.
- That reads as synergy.

The ratio is computed from parameter-averaged means, has no interval, and is exploratory.

**Reading.** "Redundant" is a statement on the difference scale only. These data do not show
that habituation and novelty do overlapping work.

## 4. Configuration-sampling error is comparable to, or larger than, the bootstrap error

The 98.75% intervals come from resampling instances only. The Monte-Carlo error from using 512
vectors (protocol §5.2) is reported separately and is **not** included. Comparing standard
errors on the same footing:

| Hypothesis | Bootstrap SE (from interval width) | Configuration MC SE | MC / bootstrap | Combined 98.75% interval (descriptive) | Distance from zero (combined SEs) |
|---|---:|---:|---:|---|---:|
| H1 | 0.0048 | 0.0120 | 2.5 | [−0.584, −0.520] | 42.8 |
| H2 | 0.0086 | 0.0104 | 1.2 | [−0.419, −0.351] | 28.5 |
| H3 | 0.0211 | 0.0169 | 0.8 | [+0.338, +0.473] | 15.0 |
| H4 | 0.0063 | 0.0073 | 1.2 | [−0.363, −0.315] | 35.3 |

How the table was computed:
- Bootstrap SE = interval half-width / z, with z = 2.4977 (the normal quantile at 98.75%).
- The combined interval adds the two variances.
- That sum is conservative. Both components include the configuration × instance interaction,
  so adding them counts it twice and slightly overstates the combined error.
- A two-way bootstrap, resampling instances and configurations together, would be exact.

Reading:
- For H1, configuration sampling dominates: 2.5× the instance error.
- No verdict changes: every estimate is at least 15 combined standard errors from zero.
- The 98.75% intervals in the report should not be read as the full uncertainty of the
  parameter-averaged effects.

## 5. Lessons for the next pre-registration (not applied to P2)

- **Select on the quantity that is tested** (§2): filter candidates on the development upper
  bound, or select against a stricter margin than the test uses.
- **Interactions** (§3):
  - declare the scale the interaction is tested on, and justify it;
  - test where the baseline does not miss everything, or only where both single mechanisms miss.
- **Configuration sampling** (§4): put it into the interval with a two-way bootstrap, or use
  enough vectors that its error is negligible.
- **Benchmarks** (§1): give noise persistence, so that a benchmark is not trivially separable
  by a detector of source dynamics. Check, before fixing joint targets, what a feature set can
  and cannot separate.
