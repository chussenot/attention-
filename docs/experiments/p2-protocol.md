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

## 2. Hypotheses

Each hypothesis is tested with the paired, run-level procedure in §5.

| ID | Hypothesis | Comparison |
|---|---|---|
| H1 | Habituation reduces the critical miss rate. | A1 vs A0, and A3 vs A2 |
| H2 | Novelty reduces the critical miss rate. | A2 vs A0, and A3 vs A1 |
| H3 | The two terms are complementary: combined beats either alone. | A3 vs A1, and A3 vs A2 |
| H4 | Stochastic selection adds value over the same score selected deterministically. | S*n* vs A*n*, for n = 0..3 |
| H5 | The benefit is robust to parameters, not tied to one point. | Pass fraction over the declared space (§6) |
| H6 | The benefit generalises beyond the development data. | Same direction, with a CI excluding zero, on the held-out sets (§7.3) |

For each comparison the stability cost is reported next to the miss-rate effect.

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
  amount from every score changes nothing. As `τ → 0` it approaches the deterministic rule.
- **Ties:** equal scores give `P = 0.5`.
- **Draws:** one `u = rng.next_f64()` per contest, consumed round by round, left to right;
  *a* wins iff `u < P(a wins)`. Byes consume no draw.
- **RNG:** `SplitMix64` (`src/rng.rs`), seeded per run with
  `instance_seed ^ (0xA5A5_0000_0000_0000 | arm_id)`.

This rule is our own. Blum & Blum's Conscious Turing Machine (CTM) also uses a probabilistic
competition, but no claim of fidelity to it is made. No arm is called "CTM" unless H4 holds.

**Deterministic tournament (implementation check, not an arm):** the same tree, where the
higher score wins and ties go to the lower `ProcessorId`. A property test must show it picks
exactly the deterministic winner on every tick of every development run. If that code path
is kept, it stays as that test.

### 3.3 Arm matrix

| Arm | Selection | Habituation | Novelty | Parameters |
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
- **Uniform random:** each candidate is equally likely. Seeded like the stochastic arms.

`arm_id` is the arm's row index above, from A0 = 0 to RND = 9.

## 4. Metrics (frozen definitions)

**Unit of observation:** one run, meaning one scenario instance × one arm × one parameter
configuration. Ticks within a run are never treated as independent observations.

**Primary metrics**

| Metric | Definition | Where it is computed |
|---|---|---|
| **Critical miss rate** | Fraction of runs in which at least one critical stream has `missed_critical = true` (deadline 20 ticks, as in `docs/design.md` §4). Every instance in this protocol has exactly one critical stream, so this equals the per-stream rate. | Critical classes: C1–C3 and `garden_path` |
| **Switch rate** | Switches per 100 eligible transitions, pooled over runs: `100 · Σ switch_count / Σ eligible`. For one run, `eligible = (number of ticks with a winner) − 1`, since ticks without a winner are skipped exactly as `switch_count` skips them. A run with `eligible = 0` contributes nothing to either sum. Raw `switch_count` totals are reported alongside. | Stable classes: C4, C5 and `stable` |

**Secondary metrics** (descriptive; never used for acceptance):
- detection latency, for runs where the critical stream was detected;
- the switch rate on critical classes;
- `max_dominance_streak`;
- mean and minimum score entropy.

Anything else computed is labelled *exploratory*.

**Provisional targets.** These are an engineering screen, not validated thresholds, and do
not support a claim that an arm allocates attention well:
- critical miss rate ≤ 5%;
- switch rate on stable classes ≤ 2.5 per 100 eligible transitions;
- at least 20% of the declared parameter space passing both, measured as in §6.

## 5. Statistical procedure

- **Pairing.** Every arm and configuration runs on exactly the same instances, with the same
  seeds. All comparisons are paired by instance.
- **Miss rate, one arm.** Point estimate with a 95% Wilson score interval.
- **Paired differences** in miss rate and pooled switch rate:
  - percentile 95% CI from a paired bootstrap over instances;
  - 10,000 resamples drawn with `SplitMix64(0x5EED_0003)`;
  - the switch-rate difference is recomputed as a pooled ratio in each resample.
- **Effects of habituation and novelty.** Each effect is reported for both levels of the
  other factor (the pairs in §2), not just averaged.
- **No significance testing beyond these intervals.** All arms are reported, including arms
  that look bad.
- **Implementation.** The Wilson interval and the bootstrap are written in-crate (std-only)
  and unit-tested against known values before any arm runs.

## 6. Parameter space and selection rule

**Ranges.** These are fixed now and do not change after any run. They are sampled uniformly
on the ranges shown; that uniform measure is what "pass fraction" refers to.

| Parameter | Range | Used by |
|---|---|---|
| k | [0.005, 0.30] | A1, A3, S1, S3 |
| λ | [0.50, 0.99] | A1, A3, S1, S3 |
| η | [0, 2] | A2, A3, S2, S3 |
| ρ | [0, 0.95] | A2, A3, S2, S3 |
| τ | [0.01, 0.50] | S0–S3 |

**Sampling.**
- Each parameterised arm gets 512 configurations, drawn independently and uniformly over its
  ranges.
- Draws come from `SplitMix64(0x5EED_0002 + arm_id)`, one `next_f64()` per parameter the arm
  uses, in the table's row order, mapped linearly onto that parameter's range.
- Using random samples rather than a grid makes the pass fraction an estimate of the volume
  of the passing region under this measure. It does not depend on grid density.
- The pass fraction is reported with a Wilson interval.
- Sensitivity surfaces are published: pairwise marginal plots or tables of pass and fail over
  the samples.

**Pass, for one configuration.** On the development set (§7.1), both of the following hold:
- critical miss rate ≤ 5%;
- pooled stable switch rate ≤ 2.5 per 100.

**Selection rule** (applied mechanically after the development sweep):
1. Among the passing configurations, pick the one with the lowest development miss rate.
2. Break ties by the lower switch rate, then by the lower sample index.
3. If an arm has no passing configuration, use the configuration with the lowest miss rate
   among those meeting the switch-rate target. If none meets the target, use the one with
   the lowest switch rate. Report that the arm did not pass.

The chosen configuration for every arm is committed as the **freeze commit** before any
held-out evaluation.

## 7. Scenarios

All instances have horizon 200 and deadline 20. Payloads stay neutral, and criticality lives
only in `GroundTruth`, as in `CLAUDE.md`.

### 7.1 Development set

**Fixed scenarios:** `garden_path` and `stable`. Their instance seeds for arm randomness are
`1_000_001` and `1_000_002`.

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
assigned in this order: dominant, background sources, critical.

**Baseline sanity checks.** These run on the development set before the sweep. If any check
fails, the **generator** is fixed, never a baseline, and the fix is recorded in the ledger.
1. A0's miss rate on C1–C3 is 100%.
2. RR's switch rate on C4 is at least 90 per 100.
3. RR's miss rate on C3 is above 0.

### 7.2 Held-out sets

Held-out sets are used up: each one counts as unseen for one evaluation phase only (§8).

1. **`distractor_flood`:** the existing frozen held-out scenario, unchanged. Its arm seed is
   `1_000_003`.
2. **Family G, held-out block 1:** seeds 10,000–10,199. Block *b* is seeds
   `10_000·b … 10_000·b + 199`, and each block is used once.
3. **Blind family B:** a generator written by an author who has not seen any of the following:
   - arbiter code;
   - development results;
   - parameter ranges or chosen values;
   - this protocol's §3, §6 and §7.1;
   - failed experiments.

   The author receives only:
   - the `Processor`, `Chunk` and `Scenario` API (`src/workspace.rs`, `src/scenario/mod.rs`);
   - the metric definitions and deadline (§4 of this document and `docs/design.md` §4);
   - the failure classes to probe: fixation, premature narrowing, garden path, transient
     critical signals and distractors;
   - the leakage rules (`CLAUDE.md`).

   Family B is evaluated on its seeds 0–199. Its generator, distributions and ground-truth
   rules are committed, and the commit hash is recorded in the ledger, **before** the
   held-out phase runs.

   **Limitation:** if the author is another Claude session, its priors are correlated with
   the developer's. A human author gives stronger independence.

Held-out instances are generated only when the held-out phase runs. Nobody looks at them
before then.

### 7.3 Generalisation criterion (H6)

An effect generalises if, on each held-out set, its paired 95% CI excludes zero in the same
direction as on the development set. This is reported separately for G-block-1, Family B and
`distractor_flood`. `distractor_flood` is a single instance, so it only gets a per-arm
outcome table, with no interval.

## 8. Procedure

1. **Pre-register.** Merge this document. Its merge commit is the pre-registration reference.
2. **Implement.**
   - The arms;
   - the switch-rate metric (§4);
   - family G;
   - the Wilson interval and bootstrap;
   - the deterministic-tournament property test;
   - a runner: an `examples/` binary with `dev` and `heldout` phases.

   Then run the baseline sanity checks (§7.1).
3. **Blind authoring.** Family B is commissioned in parallel and frozen before step 5.
4. **Development sweep.**
   - Every arm × configuration on the development set;
   - apply the selection rule (§6);
   - publish the full sensitivity results;
   - commit the frozen configurations (the freeze commit).
5. **Held-out phase,** run once, as one batch:
   - all arms with the frozen configurations;
   - on G-block-1, Family B and `distractor_flood`;
   - results committed unchanged.
6. **Report.** `docs/experiments/p2-results.md`:
   - every arm, every set;
   - effects with confidence intervals;
   - pass fractions;
   - failures stated as failures.

**Ledger.** `docs/experiments/ledger.md` records every run of anything on a held-out set,
every generator fix, the freeze commit and every block used.

**Failure handling.** A held-out failure is evidence. It leads to a revised hypothesis, a
new amendment, and a new held-out block. The used block is never re-run against re-tuned
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
  against `distractor_flood`. Even so, `distractor_flood` is a weaker held-out test of
  novelty than Family B, and its results are read with that in mind.
- **`distractor_flood` has already been run once, with A0 only** (P1, PR #1). No parameter
  was tuned on it.
- **Single-winner scope:** see A1 (§1.1).
