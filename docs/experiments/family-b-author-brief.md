# Family B: scenario author brief

This is the complete brief for the author of Family B, a held-out scenario family. The
author receives **this file and the code bundle in §6, and nothing else.**

> **For the commissioner, not part of the author's task.** Give the author only this file and
> the bundle. Do not give them the repository, its URL, `CLAUDE.md`, anything under `docs/`,
> any results, or any description of the attention-allocation policies under test. If the
> author is a Claude session, start it in a fresh, empty workspace with no repository
> attached and no network access. A session started inside this repository automatically
> loads `CLAUDE.md` and can read `docs/`, which would break the barrier. Record in the
> ledger who or what the author was. Another Claude session has priors correlated with the
> developer's, so it gives weaker independence than a human author. Never describe the
> family as fully independent.

---

## 1. What you are building

You are writing a **generator of test scenarios** for a system that allocates attention:
- on every tick, a set of *processors* each propose at most one *chunk* (a weight plus a
  payload);
- a *selection policy* picks at most one chunk as the winner.

Your scenarios will be used to evaluate several selection policies that you will not be
shown. Do not try to guess or target any particular policy. Write scenarios that a good
attention-allocation policy should handle, and that expose the failure modes listed in §3.

## 2. How a scenario is scored

These definitions are fixed. Design your ground truth so that they are meaningful.

- A scenario instance has a **horizon** (a number of ticks) and a set of processors.
- **Critical stream:** a named set of chunk ids that matter. The policies cannot see which
  chunks are critical; that information exists only in your `GroundTruth`.
- **Missed critical stream:** the stream was proposed (some chunk of it was a candidate at
  some tick *t0*) and none of its chunks won within **20 ticks** of *t0*, inclusive. Winning
  exactly 20 ticks later is on time. A stream that never wins before the end counts as
  missed.
- **Detection latency:** the first tick a chunk of the stream won, minus *t0*.
- **Switch:** a tick whose winner comes from a different processor than the most recent tick
  that had a winner. Ticks without a winner are skipped, not counted as switches. On
  instances without a critical stream, frequent switching is undesirable.

## 3. Failure modes to probe

Design instances that probe these, in whatever proportions and forms you judge useful:
- **Fixation:** one salient source keeps winning while something else needs attention.
- **Premature narrowing:** settling early on one source and failing to revise when the
  situation changes.
- **Garden path:** an early, salient source captures attention, and a later, weaker but
  critical signal is missed.
- **Transient critical signals:** a critical signal is only available for a short window.
- **Distractors:** many salient but irrelevant sources compete with the critical one.
- **Stability:** with nothing critical going on, a good policy should not switch pointlessly.

## 4. Requirements

Your generator must satisfy all of these. They are checked structurally, without running any
policy.

1. **Deliverable.** A single Rust file `src/family_b.rs` exposing exactly:

   ```rust
   use crate::metrics::GroundTruth;
   use crate::workspace::Processor;

   pub enum InstanceLabel { Critical, Stable }

   pub struct FamilyBInstance {
       pub processors: Vec<Box<dyn Processor>>,
       pub truth: GroundTruth,
       pub horizon: u64,
       pub label: InstanceLabel,
   }

   pub fn instance(seed: u64) -> FamilyBInstance { /* ... */ }
   ```

   Also write a short design note, `family_b.md`. Describe your instance types, their
   parameter distributions, how ground truth is defined, and which failure modes from §3
   each type targets. **Write it before you finish the code, and do not change it after
   delivery.**
2. **Determinism.** `instance(seed)` is a pure function of `seed`. The only randomness allowed
   is `crate::rng::SplitMix64`, seeded from `seed`. No clocks, threads, environment
   variables, files, or `HashMap`/`HashSet` iteration order affecting output.
3. **Scripted processors.** Each processor's proposals depend only on the seed and the tick.
   `receive` must do nothing that changes later proposals.
4. **Well-formed chunks.**
   - Processor ids are unique within an instance.
   - Every chunk's `source` equals its processor's id.
   - Chunk ids are unique within an instance.
   - Weights are finite.
5. **No leakage.** Payloads are neutral labels, such as `"A@57"`. They must not hint at
   criticality, role or importance, and no field other than `GroundTruth` may encode which
   chunks are critical.
6. **Labels and ground truth.**
   - A `Critical` instance has exactly one critical stream in `truth`, and that stream must
     be proposed at some tick *t0* with `t0 + 20 < horizon`.
   - A `Stable` instance has no critical streams.
   - Among seeds 0–199, at least 80 instances must be `Critical` and at least 80 `Stable`.
7. **Size limits.** `100 ≤ horizon ≤ 1000`, and at most 32 processors per instance.
8. **Do not run any selection policy**, including the one in `src/arbiter/argmax.rs`, against
   your scenarios. Do not look for, ask for, or infer information about the policies under
   test, their parameters, or any results. Do not access any repository other than the
   bundle you were given.

## 5. Delivery

Hand `src/family_b.rs` and `family_b.md` to the commissioner. Once delivered, they are
frozen. If a structural check fails, you will be told only which requirement in §4 failed,
and you may fix exactly that.

## 6. Code bundle

The bundle is the following files, taken unchanged from the commit at which the P2 protocol
was merged (`docs/experiments/p2-protocol.md`, §8, step 1):

| File | What it gives you |
|---|---|
| `Cargo.toml` | The crate manifest. The crate is std-only: do not add dependencies. |
| `src/workspace.rs` | `Tick`, `ProcessorId`, `ChunkId`, `Chunk`, the `Processor` trait, `TickRecord`, `Workspace`. |
| `src/metrics.rs` | `GroundTruth`, `CriticalStream`, and the metric implementations matching §2. |
| `src/rng.rs` | `SplitMix64`, the only allowed source of randomness. |
| `src/arbiter/mod.rs`, `src/arbiter/argmax.rs` | Needed only so the crate compiles. Do not run them (§4.8). |

It also includes `src/lib.rs` with exactly this content, replacing the repository's own
`lib.rs`:

```rust
pub mod arbiter;
pub mod family_b;
pub mod metrics;
pub mod rng;
pub mod workspace;
```

Nothing else from the repository is part of the bundle.
