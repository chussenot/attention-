# CLAUDE.md — `attn-workspace`

## Mission

`attn-workspace` is a small, deterministic, std-only Rust library that implements a
global-workspace attention mechanism (after Global Workspace Theory and the Conscious Turing
Machine of Blum & Blum) together with an evaluation harness. The harness measures whether an
arbiter allocates attention better than a naïve baseline, and specifically whether it resists
**fixation** and **premature narrowing** (Woods & Hollnagel). The question this project
answers is *"does this allocate attention better, on these scenarios?"*. It does not ask, and
must never claim to answer, whether anything is conscious.

Background and rationale: [`docs/design.md`](docs/design.md). Literature:
[`docs/references.md`](docs/references.md). Original brief:
[`docs/prompts/00-bootstrap.md`](docs/prompts/00-bootstrap.md).

## Binding design decisions

1. **Benchmark before architecture.** Scenarios and metrics exist before any clever arbiter.
2. **Mechanism separated from policy.** The workspace only knows the `Processor` and
   `Arbiter` traits; scoring is pluggable.
3. **Determinism.** Synchronous tick loop, no async, no threads, no wall-clock time. Ties are
   broken by ascending `ProcessorId`. Randomness, if needed, comes only from the seeded
   in-crate PRNG (`src/rng.rs`).
4. **std-only.** No runtime dependencies. No dev-dependencies without asking the user first.
5. **No ground-truth leakage.** Arbiters must be structurally unable to see which chunks are
   critical. Criticality lives in `scenario::GroundTruth`, keyed by `ChunkId`, never on
   `Chunk`. Payloads are neutral labels.
6. **Observability by construction.** Every tick emits a `TickRecord` (candidates, scores,
   winner, reason). Metrics are computed from records only, never from internal state.
7. **Explicit over clever.** Readable code; non-obvious choices are documented inline.
8. **Naming.** The crate is `attn-workspace`. The repo name (`attention-`) has a stray
   trailing hyphen; this is known and left to the owner. Do not rename anything.

## Layout

```
src/lib.rs                 re-exports
src/workspace.rs           ids, Chunk, Processor, TickRecord, Workspace (propose → select → broadcast)
src/arbiter/mod.rs         Arbiter trait, Selection
src/arbiter/argmax.rs      P1 baseline
src/metrics.rs             metrics over &[TickRecord] + GroundTruth
src/rng.rs                 SplitMix64, the only randomness source
src/scenario/              Scenario, GroundTruth, ScriptedProcessor, the three scenarios
tests/                     integration tests (workspace, metrics, scenarios, baseline)
examples/run_scenarios.rs  metrics table per scenario × arbiter
```

## Phase plan and acceptance criteria

- **P0 — Harness.** Core types, `Workspace::step`, `TickRecord`, metrics, all three
  scenarios. Done when metrics are unit-tested on hand-built record sequences, including an
  empty tick, all-equal scores, and a critical stream that never wins.
- **P1 — Baseline `ArgmaxArbiter`.** Winner = max weight, ties broken by ascending
  `ProcessorId`. Done when a test asserts that the baseline **misses** the critical signal in
  `garden_path` (if it does not, fix the scenario, not the baseline), and
  `examples/run_scenarios.rs` prints a metrics table per scenario and arbiter.
- **P2 — `CtmArbiter`.** Binary tournament over candidates. Score =
  `weight + novelty_bonus − habituation[source]`; a source's habituation increases by `k` when
  it wins and decays by a factor `λ` every tick (our addition to the CTM, encoding the
  anti-fixation guardrail). Done when it beats P1 on `garden_path` while keeping
  `switch_count` on `stable` below a threshold. **Parameters are frozen and committed before
  `distractor_flood` is run**; its results are reported as-is.
- **P3 — Attention schema.** A meta-processor that predicts the next winner from recent
  records and can bias selection. Compared against P2's hard-coded habituation.
- **P4 — `jcs-core` adapter.** Maps to sense-making functions F6–F11 and emits metrics using
  JCS per-function naming. Lives outside this crate's core; this crate never depends on
  `jcs-core`.

## Verification (all must pass before reporting done)

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo run --example run_scenarios
```

## Do not

- Add dependencies (runtime or dev) without explicit approval.
- Use async, threads, wall-clock time, `HashMap`/`HashSet` iteration order in anything that
  affects output, or any randomness other than the seeded in-crate PRNG.
- Put criticality (or anything derived from ground truth) on `Chunk`, in payloads, or
  anywhere an `Arbiter` or `Processor` can read it.
- Tune any parameter against a scenario marked `held_out` (currently `distractor_flood`).
  Freeze parameters in a commit first, then run it and report the numbers unchanged.
- Weaken the baseline or a scenario to make a new arbiter look better. If a scenario does not
  produce the failure it was built for, fix the scenario and say so.
- Compute metrics from arbiter or processor internals instead of `TickRecord`s.
- Make claims about consciousness, sentience or awareness in docs, code or comments. Theories
  of consciousness are cited only as sources of *mechanisms*.
- Depend on `jcs-core`.
