# P2 protocol: pre-registration

**Status:** pre-registration. Merging this document freezes everything below. Nothing in it
may change after any P2 arbiter code runs, except through a new, dated amendment section at
the end that states what changed, why, and which results it affects. Amendments never
reopen a held-out set that has already been used (§8).

## 1. Question

Does a score-based anti-fixation policy allocate attention better than simple baselines
**because of its design**, or only because its parameters happen to fit the development
scenarios?

"Better" always means two things together: fewer missed critical signals **and** an
acceptable stability cost. Neither is reported without the other.

The protocol separates two kinds of question and never answers one with the other's
evidence:

- **Mechanism effects (H1–H4, and H6 for them).** What does adding habituation, adding
  novelty, or replacing deterministic selection with stochastic selection do, with every
  other parameter held constant? These are estimated on configurations that share their
  parameters (§6.2), averaged over the declared parameter measure. No configuration selection
  is involved.
- **Best-tuned policy performance (H7, and the policy table in §6.5).** How well does the
  configuration chosen on development data perform on unseen data? This involves selection
  and says nothing, on its own, about which mechanism caused the result.

### 1.1 Scope assumption

- **A1, one winner per tick.** All conclusions apply to single-winner selection only. Whether
  the intended consumer needs more than one winner per tick depends on the JCS function
  definitions, which are an open dependency (`docs/design.md` §7). This scope is reviewed when
  `docs/jcs-contract.md` is imported. Until then, no result here is claimed to transfer to a
  multi-winner workspace.

### 1.2 What this protocol does not establish

- **Thresholds are not validated.** The targets in §4 are provisional engineering screens.
  Validated thresholds need a downstream diagnosis task, which tells us how much delay,
  omission and repeated attention a decision can tolerate. That task comes after P2.
- **Decision quality is not measured.** A critical signal winning the workspace is a proxy
  for useful attention, not evidence of it.
- **No generalisation beyond the families sampled.** Confidence intervals describe sampling
  variability *within* a declared scenario family (§5.1). They do not show that a result
  holds for families that were never sampled. Family B (§7.2) is the only evidence about
  structure beyond family G, and it is a single family.

## 2. Hypotheses

Notation:
- *c* = 0…511 indexes the shared parameter vectors (§6.2). *i* indexes scenario instances.
- *X*(*c*, *i*) is arm *X*'s outcome at configuration *c* on instance *i*. For the miss rate
  this is the missed indicator, on critical instances; for the switch rate it is the per-run
  rate, on stable instances (§4).
- Parameter-free arms (A0, RR, RND) have one outcome per instance, which is reused for every
  *c*.
- ⟨·⟩ is the mean over the 512 configurations, followed by the class-weighted mean over
  instances (§5.1).

### 2.1 Mechanism hypotheses (development set, G development block)

| ID | Claim (miss rate) | Primary estimand | Simple effects (secondary) |
|---|---|---|---|
| H1 | Habituation reduces the critical miss rate. | Main effect Δ_H = ½⟨(A1 − A0) + (A3 − A2)⟩ | ⟨A1 − A0⟩ (novelty off), ⟨A3 − A2⟩ (novelty on) |
| H2 | Novelty reduces the critical miss rate. | Main effect Δ_N = ½⟨(A2 − A0) + (A3 − A1)⟩ | ⟨A2 − A0⟩ (habituation off), ⟨A3 − A1⟩ (habituation on) |
| H3 | Habituation and novelty are complementary: together they reduce misses by more than the sum of their separate reductions. | Interaction I = ⟨A3 − A2 − A1 + A0⟩ | ⟨A3 − A1⟩ and ⟨A3 − A2⟩ |
| H4 | Stochastic selection reduces the critical miss rate relative to deterministic selection of the same score with identical score parameters. | Δ_S = ¼ Σₙ ⟨Sn − An⟩, n = 0…3 | ⟨Sn − An⟩ for each n; Δ_S by τ tercile |

How each hypothesis is decided:
- **Support.** H1, H2 and H4 are supported if the upper bound of the primary estimand's
  simultaneous interval (§5.3) is below zero. H3 is supported, meaning the terms are
  complementary, if the upper bound of I's interval is below zero.
- **Other outcomes for H3.** If I's lower bound is above zero, the terms are *redundant*:
  together they reduce misses by less than the sum. Otherwise H3 is *inconclusive*.
- **Combined arm beating both single arms is not enough.** A3 doing better than A1 and A2
  does not establish complementarity: it is consistent with purely additive effects (I = 0).
  Only I tests it.
- **Floor effect.** I can only be negative if the single-mechanism arms leave misses to
  remove. The floor diagnostic reports the fraction of (*c*, *i*) cells where A1 and A2
  *both* detect the critical stream. An inconclusive or redundant H3 is read against that
  diagnostic.
- **Stability cost.** For every estimand above, the same contrast is computed on the switch
  rate over stable instances and reported next to it with its interval. It does not decide
  support for H1–H4; it decides H7.

### 2.2 Robustness, generalisation and policy hypotheses

| ID | Claim | Data | Decision rule |
|---|---|---|---|
| H5 | An arm's good performance is not tied to one parameter point. | G development block | For each parameterised arm, the pass fraction over V (§6.3) with a 95% Wilson interval. H5 is met for arm *X* if the interval's lower bound is at least 0.20, the provisional target. |
| H6 | Each mechanism effect supported on development data generalises. | G held-out block 1 **and** Family B, analysed separately | Applies to each of H1–H4 that was supported on development data. Re-estimate the same estimand, with the same arms, all 512 shared vectors and the same interval level, on each held-out set. H6 is supported for that hypothesis if, on **both** sets, the interval excludes zero in the same direction as on development data. `distractor_flood` is **not** part of H6 (§7.3). |
| H7 | The development-selected policy beats argmax on unseen data at an acceptable stability cost. | G held-out block 1 **and** Family B, analysed separately | X\* is the development-selected policy (§6.5). On each set, estimate X\*(*c*\*) − A0 for the miss rate and the switch rate, with 95% paired intervals (§5.3). H7 is supported if, on **both** sets, the miss-rate interval's upper bound is below zero **and** the switch-rate increase interval's upper bound is at most 2.5 per 100. That margin is the provisional target, used here as a non-inferiority margin. |

## 3. Arms

### 3.1 Score

For candidate *i*, from source *s(i)*, at tick *t*:

```
score_i = w_i + N · η · nov[s(i)] − Hb · h[s(i)]
```

- `Hb` switches habituation on or off and `N` switches novelty on or off. Both are 0 or 1,
  fixed per arm.
- `w_i` is the candidate's weight.
- `h` (habituation) and `nov` (novelty) are per-source state defined below.

**Habituation** is selection-history state.
- Each source starts at `h = 0`.
- After selection, every tick, including ticks with no candidates:
  1. `h[s] ← λ · h[s]` for every source seen so far;
  2. then `h[winner] ← h[winner] + k`, if there was a winner.
- Decay comes first, so a winner's increment is not decayed in the same tick.
- Parameters: `k`, `λ`.

**Novelty** is signal state, reflecting how the evidence from a source changes.
- `prev[s]` is the weight source *s* proposed at tick *t − 1*, or 0 if it proposed nothing then.
- Raw change: `δ[s] = max(0, w_s(t) − prev[s])`. It counts **positive increases only**: a
  dropping signal earns no bonus.
- A source appearing after being silent counts as an increase from 0. This follows the
  abrupt-onset principle: a new signal is the textbook attention-capturing event. See §9 for
  why this is disclosed.
- Duration: `nov[s] ← max(δ[s], ρ · nov[s])` each tick the source proposes. When it does not
  propose, `nov[s] ← ρ · nov[s]`.
- Parameters: `η` (bonus scale), `ρ` (persistence; 0 means a one-tick bonus).
- **Known risk, pre-registered:** a source whose weight fluctuates earns positive changes
  often. Class C5 (§7.1) exists to measure this cost. Novelty is not modified to hide it.

No second recency mechanism is added in P2. Habituation is the only selection-history term.

### 3.2 Selection rules

**Deterministic:** the highest score wins; ties go to the lowest `ProcessorId`.

**Stochastic tournament:**
- **Leaves:** the candidates, in ascending `ProcessorId` order.
- **Rounds:** in each round, adjacent entries are paired left to right. If a round has an odd
  number of entries, the last one gets a bye and advances without a contest.
- **Contest:** between *a* and *b*, `P(a wins) = 1 / (1 + exp(−(score_a − score_b) / τ))`.
  Temperature `τ > 0` is a parameter. This rule is shift-invariant, so subtracting the same
  amount from every score changes nothing. As `τ → 0`, a contest between **unequal** scores
  approaches the deterministic outcome.
- **Ties:** equal scores give `P = 0.5` for any `τ`. This differs from the deterministic rule,
  which gives ties to the lower `ProcessorId`, so the stochastic rule does not converge to the
  deterministic one when scores are tied.
- **Draws:** one `u = rng.next_f64()` per contest, consumed round by round, left to right;
  *a* wins iff `u < P(a wins)`. Byes consume no draw.
- **RNG:** `SplitMix64` (`src/rng.rs`), with one stream per run, seeded as defined in §6.4.

This rule is our own. Blum & Blum's Conscious Turing Machine (CTM) also uses a probabilistic
competition, but no claim of fidelity to it is made. No arm is called "CTM" unless H4 holds.

**Deterministic tournament (implementation check, not an arm):** the same tree, where the
higher score wins and ties go to the lower `ProcessorId`. A property test must show it picks
exactly the deterministic winner on every tick of every development run. If that code path
is kept, it stays as that test.

### 3.3 Arm matrix

| Arm | Selection | Habituation | Novelty | Components of the shared vector *v_c* used (§6.2) |
|---|---|---|---|---|
| A0 | deterministic | off | off | none (this is `ArgmaxArbiter`) |
| A1 | deterministic | on | off | k, λ |
| A2 | deterministic | off | on | η, ρ |
| A3 | deterministic | on | on | k, λ, η, ρ |
| S0 | stochastic | off | off | τ |
| S1 | stochastic | on | off | k, λ, τ |
| S2 | stochastic | off | on | η, ρ, τ |
| S3 | stochastic | on | on | k, λ, η, ρ, τ |
| RR | round-robin | — | — | none |
| RND | uniform random | — | — | none |

- **Round-robin:** the winner is the proposing source with the smallest id greater than the
  previous winner's id; if there is none, the smallest proposing id. The first tick picks
  the smallest id.
- **Uniform random:** each candidate is equally likely. Seeded as in §6.4.
- **Arm ids.** `arm_id` is the arm's row index above, from A0 = 0 to RND = 9.
- **Parameterised arms** are A1–A3 and S0–S3.

## 4. Metrics (frozen definitions)

**Unit of observation:** one run, meaning one scenario instance × one arm × one parameter
configuration. Ticks within a run are never treated as independent observations.

**Primary metrics, per run**

| Metric | Per-run value | Computed on |
|---|---|---|
| **Critical miss** | 1 if at least one critical stream has `missed_critical = true` (deadline 20 ticks, as in `docs/design.md` §4), otherwise 0. Every critical instance in this protocol has exactly one critical stream. | Critical instances |
| **Switch rate** | `100 · switch_count / eligible`, where `eligible = (number of ticks with a winner) − 1`. Ticks without a winner are skipped exactly as `switch_count` skips them. Runs with `eligible = 0` are excluded, and how many were excluded is reported. | Stable instances |

How these are aggregated over instances is defined in §5.1. A mean of per-run rates is used,
not a pooled ratio, so that every estimand is linear in per-run values. That linearity allows
the stratified paired bootstrap (§5.2) and averaging over configurations. In G's stable
classes every source proposes on every tick, so `eligible = 199` in every run, and the mean
of per-run rates equals the pooled ratio there. Raw `switch_count` totals are reported
alongside.

**Secondary metrics** (descriptive; never used to decide a hypothesis):
- detection latency, for runs where the critical stream was detected;
- the switch rate on critical instances;
- `max_dominance_streak`;
- mean and minimum score entropy.

Anything else computed is labelled *exploratory*.

**Provisional targets.** These are an engineering screen, not validated thresholds, and do
not support a claim that an arm allocates attention well:
- critical miss rate ≤ 5%;
- switch rate on stable instances ≤ 2.5 per 100 eligible transitions;
- at least 20% of the declared parameter space passing both (H5, §6.3).

## 5. Statistical procedure

### 5.1 Target populations and weighting

| Set | Population the estimates describe | Critical-metric weighting | Stability-metric weighting |
|---|---|---|---|
| G development block, G held-out block 1 | Instances drawn from family G's declared generator (§7.1), with classes weighted equally | C1, C2, C3: ⅓ each | C4, C5: ½ each |
| Family B | Instances drawn from Family B's delivered generator | All instances labelled `Critical`, equally weighted | All instances labelled `Stable`, equally weighted |

- **Within a class or label group,** every instance has equal weight.
- **Parameter-averaged estimands** (H1–H4, H6) first average each instance's per-run values
  over the 512 shared vectors, then apply the weighting above.
- **Fixed scenarios** (`garden_path`, `stable`, `distractor_flood`) are single instances.
  They are reported separately as per-instance case studies (§7.3) and never pooled into the
  family estimates above.

### 5.2 Stratified paired bootstrap

- **Strata:** G uses its classes C1…C5; Family B uses its two labels, `Critical` then
  `Stable`.
- **Resampling:** each resample draws, within every stratum, as many instances as the stratum
  holds, with replacement. Index = `floor(u · n_s)` with `u = rng.next_f64()`. Strata are
  processed in the order above, and instances within a stratum are listed in ascending seed
  order.
- **Pairing is preserved.** A resampled instance brings every arm and configuration run on
  it, so every contrast is recomputed on the same resampled instances.
- **Resamples:** 10,000 per set, from `SplitMix64(0x5EED_0003 + set_id)`, with `set_id` 0 =
  G development block, 1 = G held-out block 1, 2 = Family B. The same resamples serve every
  estimand within a set.
- **Interval:** a percentile interval at level 1 − α. Sort the 10,000 replicate values; the
  lower bound is the value at index `floor(10000 · α/2)` and the upper bound the value at
  index `ceil(10000 · (1 − α/2)) − 1`, counting from 0.
- **Configuration sampling is not resampled.** For parameter-averaged estimands, the
  Monte-Carlo error from using 512 vectors instead of the full uniform measure is reported
  separately: the standard deviation across *c* of the per-configuration estimate, divided
  by √512.

### 5.3 Interval levels and multiplicity

- **H1–H4 support (development) and H6 (held-out):** α = 0.0125 (98.75% intervals). This is a
  Bonferroni correction across the four primary mechanism estimands. Within H6 the two
  held-out sets must *both* agree; that conjunction needs no further adjustment.
- **H7:** α = 0.05 on each held-out set. It is a single pre-declared contrast, and both sets
  must agree.
- **Everything else:** 95% intervals, labelled descriptive. This covers simple effects,
  per-class and per-*n* breakdowns, stability contrasts and per-arm rates.
- **Pass fractions (H5):** 95% Wilson intervals over the 512 configurations, which are
  independent draws.
- No other significance tests are run. All arms are reported, including arms that look bad.

### 5.4 Implementation

The Wilson interval, the stratified bootstrap and the percentile rule are written in-crate
(std-only) and unit-tested against known values before any arm runs.

## 6. Parameter space, selection and seeds

### 6.1 Ranges

These are fixed now and do not change after any run. They are sampled uniformly on the ranges
shown; that uniform measure is what "pass fraction" and "parameter-averaged" refer to.

| Parameter | Range | Used by |
|---|---|---|
| k | [0.005, 0.30] | A1, A3, S1, S3 |
| λ | [0.50, 0.99] | A1, A3, S1, S3 |
| η | [0, 2] | A2, A3, S2, S3 |
| ρ | [0, 0.95] | A2, A3, S2, S3 |
| τ | [0.01, 0.50] | S0–S3 |

### 6.2 One shared set of parameter vectors

- **V** = {*v_c* : *c* = 0…511}, drawn **once** from `SplitMix64(0x5EED_0002)`.
- Each *v_c* = (k, λ, η, ρ, τ) takes exactly five `next_f64()` draws in that order, each
  mapped to `lo + (hi − lo) · u`. All five components are drawn for every *c*, whichever arms
  use them.
- **Configuration *c* of a parameterised arm** is *v_c* restricted to the components the arm
  uses (§3.3); the rest are ignored.
- **What is held constant in each comparison:**
  - A1(*c*) and A3(*c*) share (k, λ);
  - A2(*c*) and A3(*c*) share (η, ρ);
  - S*n*(*c*) and A*n*(*c*) share every score parameter, and differ only in selection, at
    temperature τ from *v_c*.
- **Runs:** every parameterised arm runs at every *c* on every instance of a set, and
  parameter-free arms run once per instance. On each set, every configuration of every arm
  sees identical instances and seeds.
- **Grid density:** random draws rather than a grid make the pass fraction an estimate of the
  passing region's volume under the uniform measure, independent of grid density.
- **Sensitivity:** surfaces are published as pairwise marginal tables of pass and fail over V,
  per arm.

### 6.3 Pass and the selection rule

**Pass, for one configuration of one arm.** On the G development block, with the weighting
in §5.1, both of the following hold:
- critical miss rate ≤ 5%;
- stable switch rate ≤ 2.5 per 100.

`garden_path` and `stable` outcomes are reported for every configuration but do not affect
whether it passes (§5.1).

**Selection rule.** It is applied mechanically to a set of indexed candidates, and every
branch ends in a total order:
1. If any candidate passes: among the passing ones, the lowest miss rate, then the lowest
   switch rate, then the lowest index.
2. Otherwise, if any candidate meets the switch-rate target: among those, the lowest miss
   rate, then the lowest switch rate, then the lowest index.
3. Otherwise: the lowest switch rate, then the lowest miss rate, then the lowest index. The
   result is reported as not passing.

Rule 1 is the primary objective: the lowest development miss rate among passing
configurations. Rules 2 and 3 apply only when nothing passes. Rule 2 keeps the same ordering
restricted to the stability target; rule 3 ranks by stability first, because no candidate
meets it.

**Comparison of values.** Rates are computed in `f64`, summing in ascending seed order within
each stratum and then over strata in the order of §5.2, and compared with `f64::total_cmp`.
An exact tie falls through to the next key, and the index is always a final, unique key.

### 6.4 RNG streams

Every run of a randomised arm (S0–S3 and RND) gets its own reproducible stream:

```
mix(x)   = SplitMix64::new(x).next_u64()
run_seed = mix(mix(mix(instance_seed) ^ arm_id) ^ c)
```

- `c` is the configuration index (0…511) for S0–S3, and 0 for RND.
- Because `arm_id` and `c` both enter the seed, no two runs on the same instance share a
  stream.
- Deterministic arms use no randomness.
- The same seeds are used whenever a configuration is re-run (§7.3), so frozen
  configurations reproduce their grid results exactly.

### 6.5 Best-tuned configurations and the selected policy X\*

These are used only for H7 and for the descriptive policy table, never for H1–H6.
- **Best-tuned configuration of each parameterised arm.** Apply the selection rule (§6.3) to
  the arm's 512 configurations (index = *c*). The result is *c*\*_X.
- **Selected policy X\*.** Apply the same rule to the seven best-tuned configurations, with
  index = `arm_id`. A0, RR and RND are baselines and are not eligible. If X\* does not pass
  on development data (rule 2 or 3), H7 is still evaluated on the held-out sets, and the
  report states that X\* did not pass.
- **Policy table.** Every arm at its best-tuned configuration, plus A0, RR and RND, on every
  set: miss rate and switch rate with 95% intervals, and the contrast with A0. It is
  labelled "independently tuned policy comparison" and is not evidence for any mechanism
  hypothesis.

### 6.6 Freeze commit

Before any held-out evaluation, a single commit records:
- V, as the seed and the first and last vectors, as a check;
- every development-grid result needed to reproduce the analysis;
- *c*\*_X for each parameterised arm, and X\*;
- the development estimates for H1–H5.

## 7. Scenarios

Family G instances have horizon 200 and deadline 20. Payloads stay neutral, and criticality
lives only in `GroundTruth`, as in `CLAUDE.md`.

### 7.1 Development set

**Generated family G, development block:**
- seeds 0–199;
- the class is `seed mod 5`, giving 40 instances per class;
- all instance parameters come from `SplitMix64(seed)` in the order listed for each class;
- each fluctuating source gets its own `SplitMix64`, seeded from the next instance draw;
- `U[a, b]` means uniform real and `U{a..b}` means uniform integer, both inclusive.

| Class | What it tests | Processors (constant weights unless stated) | Critical stream |
|---|---|---|---|
| C1 *onset* | Garden path, generalised: an existing source changes. | `n_bg ~ U{1..6}` background sources at `U[0.1, 0.5]` each. One dominant source at `w_d ~ U[0.75, 0.95]`. One critical source at `w_pre ~ U[0.1, 0.4]` from t = 0, jumping at `T_on ~ U{30..150}` to `w_d − g`, with `g ~ U[0.15, 0.45]`. | Critical source's chunks for `t ≥ T_on` |
| C2 *silent onset* | A new source appears. | As C1, but the critical source is silent before `T_on`. | Same |
| C3 *transient* | Responsiveness under a short window. | `n_bg ~ U{3..10}` background sources at `U[0.1, 0.5]`; dominant at `w_d ~ U[0.75, 0.95]`; critical silent except for `m ~ U{3..12}` ticks starting at `T_on ~ U{30..150}`, at weight `w_d − g`, with `g ~ U[0.15, 0.45]`. | Critical chunks during its window |
| C4 *stable* | Stability with many sources. | Dominant at `U[0.75, 0.95]`; `n_bg ~ U{2..12}` background sources at `U[0.1, 0.5]`. | None |
| C5 *noisy stable* | Novelty's sensitivity to noise. | Dominant at `U[0.75, 0.95]`; `n_bg ~ U{2..8}` background sources, each drawing a fresh weight from `U[0.1, 0.6]` every tick. | None |

By construction, the critical weight is always below the dominant weight. Processor ids are
assigned in this order: dominant, background sources, critical. C1–C3 are the critical
instances and C4–C5 the stable ones.

**Fixed development case studies:** `garden_path` and `stable`, with instance seeds
`1_000_001` and `1_000_002` for arm randomness. Every arm and configuration runs on them, and
results are reported per instance (§5.1).

**Baseline sanity checks.** These run on the G development block before the sweep. If any
check fails, the **generator** is fixed, never a baseline, and the fix is recorded in the
ledger.
1. A0's miss rate on C1–C3 is 100%.
2. RR's switch rate on C4 is at least 90 per 100.
3. RR's miss rate on C3 is above 0.

### 7.2 Held-out sets

Held-out sets are used up: each one counts as unseen for one evaluation phase only (§8).

1. **Family G, held-out block 1:** seeds 10,000–10,199, generated by the same generator as
   §7.1. Block *b* is seeds `10_000·b … 10_000·b + 199`, and each block is used once.
2. **Blind Family B:** a generator written under the information barrier defined in
   [`family-b-author-brief.md`](family-b-author-brief.md). That brief is the **only**
   document the author receives, together with the code bundle it lists.
   - **Contents.** Family B is evaluated on its seeds 0–199. Each instance is labelled
     `Critical` (exactly one critical stream) or `Stable` (none), with at least 80 of each.
   - **What the author never sees:**
     - the arm matrix, score or selection rules;
     - parameter ranges or values;
     - this protocol;
     - `CLAUDE.md`, `docs/` and the rest of the repository;
     - development results.
   - **Delivery and sealing.** The commissioner commits the delivered files unchanged to a
     branch named `family-b` and records that commit hash in the ledger **before the freeze
     commit**. The developer does not open the branch until after the freeze commit. The
     hash proves Family B was not changed after development results existed. The developer
     not reading it early is a procedural commitment, recorded in the ledger, not a technical
     guarantee.
   - **Validation, after the freeze commit.** Before any arbiter runs on Family B, it is
     checked structurally against the brief's requirements, with no arbiter involved. If it
     fails, it goes back to the author, with only the failing structural check named, and
     the fix is recorded in the ledger.
   - **Limitation.** If the author is another Claude session, its priors are correlated with
     the developer's, which gives weaker independence than a human author. Family B is never
     described as fully independent.
3. **`distractor_flood`:** the existing held-out scenario, unchanged, with instance seed
   `1_000_003`. It is a single instance whose structure the designer already knows (§9), so
   it is a case study only (§7.3).

Held-out instances are generated only when the held-out phase runs. Nobody looks at them
before then.

### 7.3 Inferential and case-study evaluation

- **Inferential (H6, H7):** G held-out block 1 and Family B only, using the estimands,
  weighting, bootstrap and decision rules of §2.2 and §5. The full grid (every parameterised
  arm at all 512 vectors, plus the parameter-free arms) runs on both sets. The H7 and
  policy-table values are read from that grid at the frozen *c*\*_X; nothing is re-selected.
- **Case study, non-inferential:** `distractor_flood` runs A0, RR, RND and every arm at its
  frozen *c*\*_X. The report is a per-arm outcome table: detected or not, latency, switch
  count, longest streak. It has no interval and is not evidence for or against any
  hypothesis. `garden_path` and `stable` are reported the same way from the development
  grid.

## 8. Procedure

1. **Pre-register.** Merge this document. Its merge commit is the pre-registration reference,
   and also the commit from which the Family B code bundle is taken
   (`family-b-author-brief.md`).
2. **Commission Family B.** Hand the author only the brief and its bundle, in an environment
   that cannot read this repository (see the brief). This runs in parallel with steps 3–4.
   The `family-b` branch hash must be in the ledger before step 5.
3. **Implement.**
   - The arms;
   - the per-run switch rate (§4);
   - family G;
   - the Wilson interval, the stratified bootstrap and the percentile rule;
   - the deterministic-tournament property test;
   - a runner: an `examples/` binary with `dev` and `heldout` phases.

   Then run the baseline sanity checks (§7.1).
4. **Development grid.**
   - Every arm at every configuration on the G development block, `garden_path` and
     `stable`;
   - estimate H1–H5;
   - apply §6.5;
   - publish the sensitivity results.
5. **Freeze commit** (§6.6).
6. **Integrate Family B.** Open the `family-b` branch, then validate it structurally
   (§7.2).
7. **Held-out phase,** run once, as one batch:
   - the full grid on G held-out block 1 and Family B;
   - the case study on `distractor_flood`;
   - results committed unchanged, with a ledger entry marking the blocks as used.
8. **Report.** `docs/experiments/p2-results.md`:
   - every arm, every set;
   - every estimand with its interval;
   - verdicts for H1–H7 under the rules above, failures stated as failures;
   - pass fractions;
   - the policy table and case studies, labelled as such.

**Ledger.** `docs/experiments/ledger.md` is append-only. It records:
- every run of anything on a held-out set;
- every generator fix;
- the `family-b` delivery hash;
- the freeze commit;
- every block used.

**Failure handling.** A held-out failure is evidence. It leads to a revised hypothesis, a
new amendment, and a new held-out block. A used block is never re-run against re-tuned
parameters as if it were still unseen.

## 9. Disclosures (prior knowledge that could bias this protocol)

- **Exploration already done on the development scenarios.** Before this protocol was
  written, an exploratory, uncommitted habituation sweep was run on `garden_path` and
  `stable` only. It found that about 29% of a grid over k ∈ (0, 0.3] × λ ∈ [0.5, 0.99]
  passed both scenarios. That result shaped §1 and the decision to add family G. It informs
  priors; it is not a result of this protocol.
- **The designer knows `distractor_flood`'s structure.** Its critical stream appears from
  silence at t = 80. Counting a source's appearance as novelty (§3.1) helps any scenario
  with that structure. That definition was chosen on the abrupt-onset principle, not tuned
  against `distractor_flood`. This is one reason `distractor_flood` is a case study and not
  inferential evidence (§7.3).
- **`distractor_flood` has already been run once, with A0 only** (P1, PR #1). No parameter
  was tuned on it.
- **The developer will see Family B's generator source after the freeze commit,** when
  integrating it (§8, step 6). Nothing is tuned after that point.
- **Single-winner scope:** see A1 (§1.1).
