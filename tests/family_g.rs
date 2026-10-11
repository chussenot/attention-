//! Family G development block (p2-protocol.md §7.1). Only development seeds are generated
//! here: held-out instances may be generated only by the held-out phase (§7.2).

use std::collections::{BTreeMap, HashSet};

use attn_workspace::scenario::family_g::{self, DEV_SEEDS, GClass};
use attn_workspace::{Arbiter, Chunk, ChunkId, ProcessorId, Selection, Tick, TickRecord};

/// Never selects anything, so the records show exactly what the processors proposed.
struct NullArbiter;

impl Arbiter for NullArbiter {
    fn select(&mut self, candidates: &[Chunk], _t: Tick) -> Selection {
        Selection {
            winner: None,
            scores: vec![0.0; candidates.len()],
            reason: String::new(),
        }
    }
}

fn run(
    seed: u64,
) -> (
    Vec<TickRecord>,
    GClass,
    attn_workspace::metrics::GroundTruth,
    bool,
) {
    let (scenario, class) = family_g::instance(seed);
    let held_out = scenario.held_out;
    let run = scenario.run(Box::new(NullArbiter));
    (run.records, class, run.truth, held_out)
}

/// Weight series per source over the run (None when silent).
fn series(records: &[TickRecord]) -> BTreeMap<ProcessorId, Vec<Option<f64>>> {
    let mut out: BTreeMap<ProcessorId, Vec<Option<f64>>> = BTreeMap::new();
    for r in records {
        for c in &r.candidates {
            out.entry(c.source)
                .or_insert_with(|| vec![None; records.len()])[r.t as usize] = Some(c.weight);
        }
    }
    out
}

#[test]
fn forty_instances_per_class_and_none_held_out() {
    let mut counts: BTreeMap<GClass, usize> = BTreeMap::new();
    for seed in DEV_SEEDS {
        let (records, class, _, held_out) = run(seed);
        assert_eq!(records.len(), 200);
        assert!(!held_out, "seed {seed}");
        *counts.entry(class).or_default() += 1;
    }
    assert!(counts.values().all(|&n| n == 40), "{counts:?}");
}

#[test]
fn instances_are_reproducible() {
    for seed in [0, 1, 2, 3, 4, 77, 199] {
        assert_eq!(run(seed).0, run(seed).0, "seed {seed}");
    }
}

#[test]
fn weights_follow_the_class_definitions() {
    for seed in DEV_SEEDS {
        let (records, class, truth, _) = run(seed);
        let s = series(&records);
        let n = s.len();
        let dominant = s[&ProcessorId(0)][0].unwrap();
        assert!((0.75..=0.95).contains(&dominant), "seed {seed}");
        assert!(s[&ProcessorId(0)].iter().all(|w| *w == Some(dominant)));

        let (n_bg, critical) = match class {
            GClass::C1Onset | GClass::C2SilentOnset | GClass::C3Transient => (n - 2, true),
            GClass::C4Stable | GClass::C5NoisyStable => (n - 1, false),
        };
        let bg_range = match class {
            GClass::C1Onset | GClass::C2SilentOnset => 1..=6,
            GClass::C3Transient => 3..=10,
            GClass::C4Stable => 2..=12,
            GClass::C5NoisyStable => 2..=8,
        };
        assert!(
            bg_range.contains(&n_bg),
            "seed {seed}: {n_bg} background sources"
        );
        for id in 1..=n_bg as u32 {
            let w = &s[&ProcessorId(id)];
            assert!(w.iter().all(|x| x.is_some()), "background always proposes");
            let hi = if class == GClass::C5NoisyStable {
                0.6
            } else {
                0.5
            };
            assert!(
                w.iter().all(|x| (0.1..=hi).contains(&x.unwrap())),
                "seed {seed}"
            );
            if class == GClass::C5NoisyStable {
                let distinct: HashSet<u64> = w.iter().map(|x| x.unwrap().to_bits()).collect();
                assert!(distinct.len() > 150, "noisy source barely varies");
            } else {
                assert!(w.iter().all(|x| *x == w[0]), "constant background");
            }
        }

        assert_eq!(truth.streams.len(), usize::from(critical), "seed {seed}");
        if critical {
            let crit = ProcessorId(n_bg as u32 + 1);
            let w = &s[&crit];
            let onset = w
                .iter()
                .position(|x| x.is_some() && class != GClass::C1Onset)
                .unwrap_or(0);
            let crit_weights: Vec<f64> = records
                .iter()
                .flat_map(|r| r.candidates.iter())
                .filter(|c| truth.streams[0].contains(c.id))
                .map(|c| c.weight)
                .collect();
            assert!(!crit_weights.is_empty());
            assert!(crit_weights.iter().all(|&x| x == crit_weights[0]));
            let gap = dominant - crit_weights[0];
            assert!((0.15..=0.45).contains(&gap), "seed {seed}: gap {gap}");
            assert!(
                crit_weights[0] < dominant,
                "critical below dominant by construction"
            );
            match class {
                GClass::C1Onset => {
                    let pre = w[0].unwrap();
                    assert!((0.1..=0.4).contains(&pre), "seed {seed}");
                }
                GClass::C2SilentOnset => {
                    assert!((30..=150).contains(&onset));
                    assert!(w[onset..].iter().all(|x| x.is_some()));
                }
                GClass::C3Transient => {
                    assert!((30..=150).contains(&onset));
                    let len = w.iter().filter(|x| x.is_some()).count();
                    assert!((3..=12).contains(&len), "seed {seed}: window {len}");
                }
                _ => unreachable!(),
            }
        }
    }
}

#[test]
fn ground_truth_is_exactly_the_critical_chunks_proposed() {
    for seed in DEV_SEEDS.filter(|s| GClass::of_seed(*s).is_critical()) {
        let (records, class, truth, _) = run(seed);
        let crit = ProcessorId(series(&records).len() as u32 - 1);
        let first_critical_tick = records
            .iter()
            .find(|r| r.candidates.iter().any(|c| truth.streams[0].contains(c.id)))
            .map(|r| r.t)
            .unwrap();
        let proposed: HashSet<ChunkId> = records
            .iter()
            .filter(|r| r.t >= first_critical_tick)
            .flat_map(|r| r.candidates.iter())
            .filter(|c| c.source == crit)
            .map(|c| c.id)
            .collect();
        assert_eq!(truth.streams[0].chunks, proposed, "seed {seed}");
        // Room for the deadline (§7.1: T_on ≤ 150, horizon 200).
        assert!(first_critical_tick + 20 < 200, "seed {seed}");
        if class == GClass::C1Onset {
            // C1's critical source proposes before the onset too, but those chunks are not critical.
            assert!(first_critical_tick >= 30);
        }
    }
}

#[test]
fn payloads_are_neutral() {
    for seed in DEV_SEEDS {
        for c in run(seed).0.iter().flat_map(|r| r.candidates.iter()) {
            let p = c.payload.to_lowercase();
            assert!(!p.contains("crit") && !p.contains("dominant") && !p.contains("noise"));
        }
    }
}
