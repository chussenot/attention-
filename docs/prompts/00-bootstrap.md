# Bootstrap prompt — attention workspace (Rust)

> Paste this whole file as the first message to Claude Code, at the root of
> `github.com/chussenot/attention-`. Its first task is to persist this context
> into the repo (`CLAUDE.md` + `docs/`), so later sessions do not depend on this prompt.

---

## 0. Mission

Build a small, deterministic, std-only Rust library that implements a **global-workspace
attention mechanism** (Global Workspace Theory / Conscious Turing Machine) and an
**evaluation harness** that measures whether it allocates attention better than a naïve
baseline — specifically whether it resists **fixation** and **premature narrowing**.

The question this project answers is: *"Does this allocate attention better?"* — measured
on scenarios. The question it does **not** answer: *"Is this conscious?"*. Do not drift there.

## 1. Context

- Long-term consumer: `jcs-core`, a Rust scaffold for a Joint Cognitive System grounded in
  Woods & Hollnagel, *Joint Cognitive Systems: Patterns in Cognitive Systems Engineering*
  (2006). Its sense-making functions (F6–F11) need an attention-allocation mechanism, and its
  anti-pattern checklist (premature narrowing, fixation) is the design guardrail here.
- **Do not depend on `jcs-core`.** Integration comes later through an adapter (phase P4),
  once this API is stable.
- Relevant Hollnagel concepts:
  - **ETTO** (Efficiency-Thoroughness Trade-Off): anti-fixation is paid for with instability;
    both must be measured.
  - **COCOM/ECOM** control modes: strategic / tactical / opportunistic / scrambled. A
    "scrambled" workspace = high switching, low dominance.
  - **FRAM**: later, a candidate method to analyse how variability propagates between functions.
- Woods' **garden path problem**: an early, salient interpretation captures attention, and a
  later, weaker but critical signal is missed. This is the reference failure scenario.

## 2. Scientific references

Write these to `docs/references.md`, with one line per entry saying why it matters here.
Entries marked ⚠ have an unverified arXiv ID: check each one and correct it, or mark it as
unverified. Never invent an ID.

**Core: the mechanism to implement**
- Blum & Blum, *A theory of consciousness from a theoretical computer science perspective:
  Insights from the Conscious Turing Machine*, arXiv:2107.13704 (PNAS 2022). This is the
  primary spec: processors, competition tree, global broadcast, links.
- Baars, *A Cognitive Theory of Consciousness* (1988), the origin of Global Workspace Theory.
- Dehaene & Changeux, Global Neuronal Workspace (ignition, limited capacity).
- Goyal et al., *Coordination Among Neural Modules Through a Shared Global Workspace*,
  arXiv:2103.01197. A limited-capacity shared workspace that specialists compete for.
- VanRullen & Kanai, *Deep learning and the Global Workspace Theory*, arXiv:2012.10390.
  An architectural roadmap.

**Attention as selection, and attention about attention**
- Bengio, *The Consciousness Prior*, arXiv:1709.08568. A sparse bottleneck over a large latent state.
- Goyal et al., *Recurrent Independent Mechanisms*, arXiv:1909.10893. Modules compete for input.
- Goyal & Bengio, *Inductive biases for deep learning of higher-level human cognition*,
  arXiv:2011.15091.
- Graziano, Attention Schema Theory: an agent models its own attention in order to control it.
  ⚠ Liu et al., *Attention Schema in Neural Agents*, arXiv:2305.17375. Also Wilterson &
  Graziano, PNAS 2021.
- Vaswani et al., *Attention Is All You Need*, arXiv:1706.03762. Context only:
  transformer attention is contextual weighting, not cognitive selection.

**Evaluation and framing**
- Butlin, Long et al., *Consciousness in Artificial Intelligence: Insights from the Science of
  Consciousness*, arXiv:2308.08708. Indicator properties derived from each theory; use it as
  the map of the field.
- Chalmers, *Could a Large Language Model be Conscious?*, arXiv:2303.07103.
- ⚠ Juliani et al., *On the link between conscious function and general intelligence in humans
  and machines*, arXiv:2204.05133.
- Long, Sebo et al., *Taking AI Welfare Seriously*, arXiv:2411.00986.

**Explicitly out of scope, with reasons**
- IIT 4.0 (Albantakis et al., arXiv:2212.14787) and PyPhi: computing Φ is combinatorially
  intractable and has no operational value here.
- Transformer attention via `candle` / `burn`: well covered elsewhere, and does not serve this goal.
- Existing implementations to read, not to port: LIDA (Franklin, Java, GWT-based), pymdp
  (active inference, Python, ⚠ arXiv:2201.03904).

**Social and folk side (context only)**
- Colombatto & Fleming (2024), *Neuroscience of Consciousness*: lay attribution of
  consciousness to LLMs grows with usage.
- Herbert Simon (1971) on the attention economy; Yves Citton, *Pour une écologie de l'attention*.

## 3. Binding design decisions

1. **Benchmark before architecture.** Scenarios and metrics exist before any clever arbiter.
2. **Mechanism separated from policy.** The workspace only knows traits, and scoring is pluggable.
3. **Determinism.** Synchronous tick loop, no async, no threads, no wall-clock time. Ties are
   broken by ascending `ProcessorId`. If randomness is ever needed, use an explicit seeded
   PRNG written in-crate, with no external dependency.
4. **std-only.** No runtime dependencies. Do not add dev-dependencies without asking.
5. **No ground-truth leakage.** Arbiters must be structurally unable to see which chunks are
   "critical". Criticality lives in the scenario's ground truth, keyed by `ChunkId`, and never
   on `Chunk`.
6. **Observability by construction.** Every tick emits a `TickRecord` (candidates, scores,
   winner, reason). Metrics are computed from the records, never from internal state.
7. **Explicit over clever.** Prefer readable code, and document non-obvious choices inline.
8. **Naming.** Crate name is `attn-workspace`. The repo name ends with a stray hyphen
   (`attention-`): flag this to the user, but do not rename anything yourself.

## 4. Core API (target shape — refine if needed, and justify changes in `docs/design.md`)

```rust
pub type Tick = u64;
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)] pub struct ProcessorId(pub u32);
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)] pub struct ChunkId(pub u64);

#[derive(Clone, Debug)]
pub struct Chunk { pub id: ChunkId, pub source: ProcessorId, pub weight: f64, pub payload: String }

pub trait Processor {
    fn id(&self) -> ProcessorId;
    fn propose(&mut self, t: Tick) -> Option<Chunk>;
    fn receive(&mut self, winner: &Chunk, t: Tick); // global broadcast
}

pub trait Arbiter {
    /// Returns the index of the winner in `candidates`, plus a score per candidate, for the record.
    fn select(&mut self, candidates: &[Chunk], t: Tick) -> Selection;
}

pub struct Selection { pub winner: Option<usize>, pub scores: Vec<f64>, pub reason: String }

pub struct TickRecord { pub t: Tick, pub candidates: Vec<Chunk>, pub scores: Vec<f64>,
                        pub winner: Option<ChunkId>, pub winner_source: Option<ProcessorId>, pub reason: String }

pub struct Workspace { /* processors: Vec<Box<dyn Processor>>, arbiter: Box<dyn Arbiter> */ }
impl Workspace { pub fn step(&mut self) -> TickRecord { /* propose → select → broadcast */ } }
```

Candidates are collected in ascending `ProcessorId` order so that results are reproducible.

## 5. Metrics (exact definitions)

Metrics are computed from `&[TickRecord]` plus the scenario ground truth.

- `switch_latency`: for each critical chunk stream, the number of ticks between the first
  tick it is proposed and the first tick a critical chunk from that stream wins. `None` if it
  never wins within the horizon.
- `missed_critical`: true if a critical stream does not win within `deadline` ticks
  (default 20) of its first proposal.
- `max_dominance_streak[source]`: the longest run of consecutive ticks won by a source.
- `switch_count`: the number of ticks where the winning source differs from the previous tick's.
- `score_entropy[t]`: Shannon entropy in bits of the candidate scores at tick t, after shifting
  them to be non-negative and normalising them to sum to 1. Report the mean and the minimum.
  If all scores are zero, use the uniform distribution.

## 6. Scenarios (`src/scenario/`)

A scenario is a set of scripted processors, a horizon, and a ground truth (the set of critical
`ChunkId`s or streams).

- `garden_path` (the reference failure case). Horizon 200 ticks.
  - Processor A emits weight 0.9 on every tick.
  - Processor B emits weight 0.3 for t < 50, then from t = 50 emits a **critical** chunk of weight 0.6 on every tick.
- `stable` (the oscillation guard). A single correct dominant source at 0.9, and two background
  sources at 0.2 and 0.25. There is no critical signal. A good arbiter keeps a low `switch_count`.
- `distractor_flood` (a held-out scenario for P2, see §7). Five sources with fluctuating
  weights from a deterministic in-crate sequence, plus one critical stream at 0.5 from t = 80.
  Define it now, but do not tune anything against it.

## 7. Phases and acceptance criteria

- **P0 — Harness.** Core types, `Workspace::step`, `TickRecord`, the metrics, and all three
  scenarios. Done when the metrics are unit-tested on hand-built record sequences, with
  edge cases: an empty tick, all-equal scores, a critical stream that never wins.
- **P1 — Baseline `ArgmaxArbiter`.** The winner is the max weight, ties broken by ascending
  `ProcessorId`. Done when a test **asserts that the baseline misses the critical signal in
  `garden_path`**. If it does not, the scenario is wrong; fix the scenario, not the baseline.
  Add an `examples/run_scenarios.rs` that prints a metrics table per scenario and arbiter.
- **P2 — `CtmArbiter`** (not in this session). It runs a binary tournament over candidates.
  Score = `weight + novelty_bonus − habituation[source]`. A source's habituation increases by
  `k` when it wins and decays by a factor `λ` every tick. The habituation term is our addition
  to the CTM, encoding the anti-fixation guardrail. Done when it beats P1 on `garden_path`
  while keeping `switch_count` on `stable` below a threshold. **Parameters are frozen before
  `distractor_flood` is run**, and the results on it are reported as-is (no overfitting).
- **P3 — Attention schema** (later). A meta-processor that predicts the next winner from the
  recent records and can bias selection. Compare it with P2's hard-coded habituation.
- **P4 — `jcs-core` adapter** (later). Map to sense-making functions F6–F11, and emit the
  metrics using the JCS per-function naming.

## 8. Your tasks in this session (P0 + P1 only)

1. Create `CLAUDE.md` containing: the mission (one paragraph), the binding decisions from §3,
   the phase plan with acceptance criteria from §7, the verification commands, and "do not"
   rules (no deps, no async, no ground-truth leakage, no tuning on held-out scenarios,
   no "consciousness" claims in docs or code comments).
2. Create `docs/design.md`, containing the rationale for each decision in §3 and the JCS link
   from §1, and `docs/references.md`, built from §2 after checking the ⚠ entries.
3. Save this prompt verbatim as `docs/prompts/00-bootstrap.md`.
4. Implement P0 and P1 with the following layout: `src/lib.rs`, `src/workspace.rs`,
   `src/arbiter/{mod.rs,argmax.rs}`, `src/metrics.rs`,
   `src/scenario/{mod.rs,garden_path.rs,stable.rs,distractor_flood.rs}`, `tests/`, `examples/`.
5. Make small, focused commits, in this order: docs → core types → metrics + tests →
   scenarios → baseline + failure test → example.
6. Finish by updating the README with a status line and an excerpt of the metrics table.

## 9. Verification (all must pass before you report done)

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo run --example run_scenarios
```

## 10. Report back

Your report must include: what was built; the metrics table; any deviation from §4 and why;
the ⚠ references you verified or corrected; open questions for P2 (initial values of `k`, `λ`
and `novelty_bonus`, and the `switch_count` threshold for `stable`); and the repo-name issue.
