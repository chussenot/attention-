# Design

This document records *why* `attn-workspace` is built the way it is. The binding rules are
summarised in [`CLAUDE.md`](../CLAUDE.md); this is the rationale behind them, plus the
interpretations and API refinements made while implementing P0 and P1.

## 1. Purpose and the JCS link

The crate answers one question: **does a given arbiter allocate attention better than a naïve
baseline, on scenarios built to expose known failure modes?** It does not ask whether anything
is conscious. Theories of consciousness (GWT, GNW, the Conscious Turing Machine) are used here
only as sources of an *architecture*: many specialist processors, one limited-capacity stage,
competition for that stage, and global broadcast of the winner.

The long-term consumer is `jcs-core`, a scaffold for a Joint Cognitive System after Woods &
Hollnagel (2006). Its sense-making functions (F6–F11) need a mechanism to decide what gets
attended to next. The JCS literature also supplies the failure modes this crate must measure:

| JCS concept | How it shows up here |
|---|---|
| **Fixation / premature narrowing** | One source monopolises the workspace: long `max_dominance_streak`, high `switch_latency`, `missed_critical = true`. |
| **Garden path problem** (Woods) | The `garden_path` scenario: an early, salient source captures attention and a later, weaker but critical signal is missed. |
| **ETTO** (efficiency–thoroughness trade-off) | Anti-fixation costs stability. `switch_count` (on `stable`) is always reported next to `switch_latency` (on `garden_path`); an arbiter that improves one by wrecking the other has not improved. |
| **COCOM/ECOM control modes** | A *scrambled* workspace has high `switch_count`, short streaks and high score entropy; a *strategic* one has long but interruptible streaks. The metrics are chosen so these modes are distinguishable. |
| **FRAM** | Later: a method to analyse how variability in one function (a processor's weights) propagates through the workspace to others. |

`jcs-core` is **not** a dependency. Integration happens in P4 through an adapter, once this API
has been stable for a while, so that the evaluation harness can evolve without dragging a
larger system along.

## 2. Rationale for the binding decisions

### 2.1 Benchmark before architecture

The interesting part of the project (a CTM-style arbiter with habituation, later an attention
schema) has free parameters. Without scenarios and metrics fixed *beforehand*, any such arbiter
can be made to look good by choosing what to measure after the fact. So P0 builds the
harness, P1 establishes a baseline and proves the reference scenario actually produces the
failure, and only then does P2 introduce a clever arbiter.

### 2.2 Mechanism separated from policy

`Workspace` implements only the cycle — *propose → select → broadcast* — and knows processors
and arbiters only through the `Processor` and `Arbiter` traits. Every policy question (what
counts as salient, how to penalise a dominant source, whether to model one's own attention)
lives in an `Arbiter` implementation. This keeps the comparison fair: two arbiters run on
identical mechanics, so any metric difference is attributable to policy.

### 2.3 Determinism

Experiments must be exactly reproducible, so that a metric change is always caused by a code
change and never by scheduling noise. Concretely:

- One synchronous tick loop; no async, no threads, no wall-clock time. Time is the `Tick`
  counter.
- Candidates are collected in ascending `ProcessorId` order (processors are sorted once, at
  construction), so the arbiter always sees the same slice for the same state.
- Ties are broken by ascending `ProcessorId`.
- The only randomness source is `rng::SplitMix64`, seeded explicitly. `HashSet` is used only
  for membership tests (ground truth lookups), never iterated in a way that affects output;
  per-source maps are `BTreeMap`.

### 2.4 std-only

The crate is small and is meant to be embedded in `jcs-core`. Zero dependencies means no
supply-chain surface, no version drift, fast builds, and nothing that could silently
introduce non-determinism (e.g. a randomly seeded hasher in a map that gets iterated). The
PRNG is ~15 lines; a crate would buy nothing.

### 2.5 No ground-truth leakage

If an arbiter could see which chunks are critical, it could trivially "solve" the garden path.
The guarantee is structural rather than a convention:

- `Chunk` has no criticality field, and none can be added without changing its type.
- Criticality lives in `scenario::GroundTruth`, a set of `CriticalStream`s keyed by `ChunkId`,
  which is consumed only by `metrics::evaluate`.
- `Scenario::run` hands processors and the arbiter to the `Workspace` and keeps the ground
  truth aside; the workspace never holds it.
- Scenario payloads are neutral labels (`"B@57"`). A test asserts no payload mentions
  criticality.

What an arbiter *may* legitimately use is everything a real system would observe: weights,
sources, payloads, and its own history (including the broadcast winners). For example, in
`garden_path` source B's weight jumps from 0.3 to 0.6 at t = 50; detecting that change is a
fair novelty signal, not leakage.

### 2.6 Observability by construction

Every `step` returns a `TickRecord`: candidates, per-candidate scores, the winner, its source,
and a human-readable reason from the arbiter. Metrics are pure functions of
`&[TickRecord]` plus ground truth. Consequences:

- Arbiters with completely different internals are measured identically.
- A suspicious metric can always be explained by reading the records for the ticks involved.
- The record is the natural export surface for P4 (mapping to JCS functions) and for any
  later tracing integration.

### 2.7 Explicit over clever

The crate is a measuring instrument; its credibility depends on readers being able to check
it. Metric definitions are written as plain loops, edge cases are handled by explicit
branches with comments, and contract violations panic with a message rather than being
silently patched.

### 2.8 Naming

The crate is `attn-workspace`. The repository is named `attention-` with a trailing hyphen,
which looks accidental. It is flagged to the owner and deliberately left alone: renaming a
repository breaks links and remotes, so it is the owner's call.

## 3. Refinements to the target API (brief §4)

The brief's API was a target shape. Implementation kept it, with these additions; none
changes the meaning of an existing item.

| Change | Why |
|---|---|
| `Chunk`, `Selection`, `TickRecord` also derive `PartialEq` (and `Clone`, `Debug` where missing). | Determinism tests compare full record sequences from two runs. |
| `Workspace::new(processors, arbiter) -> Result<Workspace, WorkspaceError>`. Processors are sorted by `ProcessorId` once; duplicates are rejected. | "Ascending `ProcessorId` order" must be guaranteed, not assumed. A duplicate id would make tie-breaking and per-source metrics ambiguous. |
| `Workspace::run(ticks)` and `Workspace::now()`. | Convenience; `run` is just repeated `step`. |
| `step` calls the arbiter **even when there are no candidates**. | Stateful arbiters (P2 habituation decays every tick) must observe the passage of time. The `ArgmaxArbiter` returns no winner in that case. |
| `step` panics if the arbiter returns an out-of-range winner index or a `scores` vector whose length differs from the candidate count, or if a processor proposes a chunk whose `source` is not its own id. | These are programming errors in an implementation of a trait, comparable to an out-of-bounds index. Silently fixing them would corrupt records and therefore metrics. |
| New module `src/rng.rs` (`SplitMix64`). | `distractor_flood` needs a deterministic pseudo-random sequence; decision §2.3 requires an in-crate seeded PRNG. Placed at crate level so P2/P3 can reuse it. |
| `scenario::{Scenario, ScenarioRun, GroundTruth, CriticalStream, ScriptedProcessor}`; `Scenario` has a `held_out` flag. | Makes the scenario contract concrete and puts the "do not tune on this" rule in the data, so reports can show it. |
| Chunk ids in scenarios are `chunk_id(source, t) = (source << 32) | t`. | Lets a scenario enumerate its critical `ChunkId`s up front, so ground truth is a plain set. The encoding reveals source and tick, both already visible on the chunk. |

## 4. Metric interpretations

The brief (§5) defines the metrics; implementation had to settle edge cases. All are
computed in `src/metrics.rs` and unit-tested in `tests/metrics.rs`.

- **Ticks are tick values, not slice positions.** Latencies are differences of `TickRecord.t`.
- **`switch_latency`** for a stream = `first_won − first_proposed`, where `first_proposed` is
  the first tick any of the stream's chunks appears among the candidates and `first_won` is
  the first tick one of them is the winner. `None` if it never wins. Latency 0 means it won on
  the very tick it first appeared.
- **`missed_critical`** = the stream was proposed and did not win within `deadline` ticks
  (inclusive: latency == deadline is *not* missed). A stream that never wins before the end
  of the records counts as missed even if the deadline would have extended past the horizon,
  so scenarios leave at least `deadline` ticks after the critical onset (`garden_path`: 150,
  `distractor_flood`: 120). A stream that is never proposed is reported with
  `first_proposed = None` and `missed = false`: that is a scenario bug, not an arbiter
  failure, and the scenario tests assert it cannot happen.
- **`max_dominance_streak`** is per source, over adjacent records (records are assumed
  contiguous, as `Workspace::run` produces them). A tick without a winner breaks every streak.
  Sources that never win are absent from the map.
- **`switch_count`** counts ticks whose winning source differs from the source of the **most
  recent tick that had a winner**. Ticks without a winner are skipped rather than counted as
  switches: `A, –, A` is 0 switches and `A, –, B` is 1. Rationale: an empty tick means nothing
  competed, not that attention moved; counting it twice would inflate instability in
  scenarios with silent periods.
- **`score_entropy`** per tick: scores are shifted by `−min(0, min score)` (so already
  non-negative scores are left unchanged — shifting by the minimum unconditionally would
  always zero out the lowest candidate and distort the distribution), normalised to sum 1, and
  the Shannon entropy is taken in bits (0·log 0 = 0). All-zero scores use the uniform
  distribution (entropy = log₂ n). A tick with no candidates, or with any non-finite score, has
  no entropy (`None`) and is excluded from the mean and the minimum. One candidate gives 0.

## 5. Scenarios

| Scenario | Horizon | Processors | Critical stream | Role |
|---|---|---|---|---|
| `garden_path` | 200 | A (id 0): 0.9 every tick. B (id 1): 0.3 for t < 50, then 0.6 from t = 50. | B's chunks for t ≥ 50 | Reference failure: salience captures attention, critical signal is weaker. |
| `stable` | 200 | Dominant (id 0) 0.9; background (id 1) 0.2, (id 2) 0.25. | none | Oscillation guard: a good arbiter keeps `switch_count` low. |
| `distractor_flood` | 200 | Five distractors (ids 0–4), weight drawn uniformly from [0.3, 0.9) every tick by `SplitMix64` with a fixed per-source seed; critical source (id 5) at 0.5 from t = 80, silent before. | id 5's chunks for t ≥ 80 | **Held out** for P2. Distractor mean (0.6) exceeds the critical weight, so salience alone almost never favours it. Defined before any P2 work; not tuned. |

## 6. Baseline (P1)

`ArgmaxArbiter` scores each candidate by its weight and picks the maximum, ties to the lowest
`ProcessorId`. Non-finite weights are never selected. On `garden_path` it locks onto A for all
200 ticks and never selects B's critical chunks: `missed_critical = true`. A test asserts this;
if it ever stops being true, the scenario (not the baseline) is wrong.
