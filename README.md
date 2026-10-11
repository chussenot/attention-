# attn-workspace

A small, deterministic, std-only Rust library implementing a **global-workspace attention
mechanism** (after Global Workspace Theory and the Conscious Turing Machine), plus an
**evaluation harness** that measures whether an arbiter allocates attention better than a
naïve baseline — in particular, whether it resists **fixation** and **premature narrowing**
(Woods & Hollnagel).

It answers *"does this allocate attention better, on these scenarios?"*, and nothing about
consciousness.

**Status:** P0 (harness) and P1 (argmax baseline) done. P2 (`CtmArbiter` with habituation)
is next. See [`CLAUDE.md`](CLAUDE.md) for the phase plan.

## How it works

Each tick: every `Processor` may propose one `Chunk` → an `Arbiter` picks at most one winner
→ the winner is broadcast to all processors. Every tick produces a `TickRecord`, and all
metrics are computed from records plus the scenario's ground truth, which arbiters never see.

| Metric | Meaning |
|---|---|
| `switch_latency` | Ticks from a critical stream's first proposal to its first win. |
| `missed_critical` | The critical stream did not win within the deadline (20 ticks). |
| `max_dominance_streak` | Longest run of consecutive wins per source (fixation). |
| `switch_count` | Changes of winning source (instability — the ETTO cost). |
| `score_entropy` | Entropy of the arbiter's scores per tick, mean and min. |

## Baseline results

`cargo run --example run_scenarios`:

| scenario | arbiter | held out | switches | longest streak | critical latency | missed (deadline 20) | entropy mean (bits) | entropy min (bits) |
|---|---|---|---:|---|---|---|---:|---:|
| garden_path | argmax | no | 0 | 200/200 (proc0) | never | yes | 0.931 | 0.811 |
| stable | argmax | no | 0 | 200/200 (proc0) | – | – | 1.249 | 1.249 |
| distractor_flood | argmax | yes | 152 | 5/200 (proc0) | never | yes | 2.433 | 2.203 |

The baseline fixates on the salient source in `garden_path` and never attends to the
critical signal — the failure the scenario was built to expose. `distractor_flood` is held
out: no parameter may be tuned against it.

## Development

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo run --example run_scenarios
```

Design rationale: [`docs/design.md`](docs/design.md) · References:
[`docs/references.md`](docs/references.md).
