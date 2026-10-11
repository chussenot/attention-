# Experiment ledger

Append-only. Record every run on a held-out set, every generator fix, every freeze commit,
and every held-out block used. See `p2-protocol.md` §8.

| Date | Event | Sets touched | Arms / configs | Commit / PR | Notes |
|---|---|---|---|---|---|
| 2026-10-11 | P1 baseline run | `distractor_flood` (held out) | A0 (`ArgmaxArbiter`) only | PR #1 | No parameters exist for A0; nothing was tuned. 152 switches, longest streak 5/200, critical stream missed. |
| 2026-10-11 | Exploratory habituation sweep (not committed) | `garden_path`, `stable` (development) | argmax + habituation, grid over k ∈ (0, 0.3], λ ∈ [0.5, 0.99] | none | About 29% of the grid passed both scenarios. Disclosed in `p2-protocol.md` §9. `distractor_flood` not touched. |
| 2026-10-11 | P2 protocol pre-registered (PR #3 merged) | none | none | `46068d1` | Pre-registration reference (`p2-protocol.md` §8 step 1) and the source commit for the Family B author bundle. Every later entry refers to this commit. |
| 2026-10-11 | P2 development grid (§8 step 4) | G development block (seeds 0–199), `garden_path`, `stable` (development only) | All 10 arms; parameterised arms at all 512 shared vectors | code at `dd4588e` (branch `claude/p2-dev-results`) | Sanity checks passed before the sweep. Run twice; the freeze file was byte-identical, and the second run differs from the first only in two report labels. No held-out data generated or touched. The freeze commit (step 5) is the merge of this results PR, and must wait until the `family-b` delivery hash is in this ledger. |
