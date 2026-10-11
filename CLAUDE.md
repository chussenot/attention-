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
   critical. Criticality lives in `metrics::GroundTruth`, keyed by `ChunkId`, never on
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
src/arbiter/argmax.rs      P1 baseline (P2 arm A0)
src/arbiter/score.rs       P2 arms A1–A3, S0–S3: habituation, novelty, deterministic or stochastic selection
src/arbiter/tournament.rs  stochastic tournament, and the deterministic one used as an implementation check
src/arbiter/round_robin.rs, uniform_random.rs   P2 baselines RR, RND
src/metrics.rs             GroundTruth + metrics over &[TickRecord], per-run switch rate
src/stats.rs               Wilson interval, stratified paired bootstrap, percentile rule
src/rng.rs                 SplitMix64, the only randomness source
src/scenario/              Scenario, ScriptedProcessor, fixed scenarios, family G generator
src/experiment/            P2: arm matrix, shared vectors and seeds, run grid, estimands, runner phases
tests/                     integration tests (development data only)
examples/run_scenarios.rs  metrics table per scenario × arbiter (P1)
examples/p2.rs             P2 runner: sanity | dev [--smoke DIR] | heldout (gated)
scripts/check-family-b-bundle.sh   Family B bundle check, pinned to the pre-registration commit
```

## Phase plan and acceptance criteria

- **P0 — Harness.** ✅ Core types, `Workspace::step`, `TickRecord`, metrics, all three
  scenarios. Done when metrics are unit-tested on hand-built record sequences, including an
  empty tick, all-equal scores, and a critical stream that never wins.
- **P1 — Baseline `ArgmaxArbiter`.** ✅ Winner = max weight, ties broken by ascending
  `ProcessorId`. Done when a test asserts that the baseline **misses** the critical signal in
  `garden_path` (if it does not, fix the scenario, not the baseline), and
  `examples/run_scenarios.rs` prints a metrics table per scenario and arbiter.
- **P2 — anti-fixation arbiters, run as a controlled experiment.** Defined entirely by the
  pre-registration [`docs/experiments/p2-protocol.md`](docs/experiments/p2-protocol.md),
  which is binding. It covers:
  - a factorial of habituation × novelty under deterministic and stochastic selection,
    against round-robin and random baselines;
  - mechanism effects estimated on one shared set of parameter vectors, kept separate from
    best-tuned policy comparisons;
  - a stratified, paired, run-level bootstrap;
  - a pass-fraction measure over a fixed parameter space;
  - parameters frozen before any held-out run;
  - inferential held-out sets that are used up (family G held-out blocks and a blind
    Family B), with `distractor_flood` as a non-inferential case study.

  The thresholds there are provisional targets, not validated acceptance criteria. No arm is
  called "CTM" unless stochastic selection beats its deterministic counterpart.
- **P3 — Attention schema.** A meta-processor that predicts the next winner from recent
  records and can bias selection. Compared against P2's hard-coded habituation.
- **P4 — `jcs-core` adapter.** **Blocked** on an open dependency (see `docs/design.md` §7):
  the sense-making function definitions and the per-function metric naming must first be
  imported verbatim, as `docs/jcs-contract.md`, from the external JCS functional architecture
  document. Lives outside this crate's core; this crate never depends on `jcs-core`.

## Verification (all must pass before reporting done)

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo run --example run_scenarios
cargo run --release --example p2 -- sanity   # P2 baseline sanity checks, development data only
scripts/check-family-b-bundle.sh   # Family B author bundle: standalone build + leakage scan
```

CI (`.github/workflows/ci.yml`) runs all of these on every pull request and on pushes to `main`.

## Do not

- Add dependencies (runtime or dev) without explicit approval.
- Use async, threads, wall-clock time, `HashMap`/`HashSet` iteration order in anything that
  affects output, or any randomness other than the seeded in-crate PRNG.
- Put criticality (or anything derived from ground truth) on `Chunk`, in payloads, or
  anywhere an `Arbiter` or `Processor` can read it.
- Tune any parameter against a scenario marked `held_out` (currently `distractor_flood`).
  Freeze parameters in a commit first, then run it and report the numbers unchanged.
- Change `docs/experiments/p2-protocol.md` except through a dated amendment section, run
  anything on a held-out set without a ledger entry (`docs/experiments/ledger.md`), or reuse
  a held-out block that has already been used.
- Give the Family B author anything beyond `docs/experiments/family-b-author-brief.md` and
  the code bundle it lists, start that author inside this repository, or open the
  `family-b` branch before the P2 freeze commit.
- Weaken the baseline or a scenario to make a new arbiter look better. If a scenario does not
  produce the failure it was built for, fix the scenario and say so.
- Compute metrics from arbiter or processor internals instead of `TickRecord`s.
- Make claims about consciousness, sentience or awareness in docs, code or comments. Theories
  of consciousness are cited only as sources of *mechanisms*.
- Depend on `jcs-core`.
- Never invent or infer JCS function numbering or names.
