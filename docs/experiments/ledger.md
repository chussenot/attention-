# Experiment ledger

Append-only. Record every run on a held-out set, every generator fix, every freeze commit,
and every held-out block used. See `p2-protocol.md` §8.

| Date | Event | Sets touched | Arms / configs | Commit / PR | Notes |
|---|---|---|---|---|---|
| 2026-10-11 | P1 baseline run | `distractor_flood` (held out) | A0 (`ArgmaxArbiter`) only | PR #1 | No parameters exist for A0; nothing was tuned. 152 switches, longest streak 5/200, critical stream missed. |
| 2026-10-11 | Exploratory habituation sweep (not committed) | `garden_path`, `stable` (development) | argmax + habituation, grid over k ∈ (0, 0.3], λ ∈ [0.5, 0.99] | none | About 29% of the grid passed both scenarios. Disclosed in `p2-protocol.md` §9. `distractor_flood` not touched. |
