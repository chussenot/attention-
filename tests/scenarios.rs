//! Scenario definitions: weights match the spec, ground truth matches what is proposed,
//! nothing leaks criticality, and runs are reproducible.

use std::collections::HashSet;

use attn_workspace::metrics::DEFAULT_DEADLINE;
use attn_workspace::scenario::{self, ScenarioRun, distractor_flood, garden_path, stable};
use attn_workspace::{Arbiter, Chunk, ChunkId, ProcessorId, Selection, Tick, TickRecord};

/// Never selects anything, so scenario output is independent of any real policy.
struct NullArbiter;

impl Arbiter for NullArbiter {
    fn select(&mut self, candidates: &[Chunk], _t: Tick) -> Selection {
        Selection {
            winner: None,
            scores: vec![0.0; candidates.len()],
            reason: "null".to_string(),
        }
    }
}

fn run(scenario: scenario::Scenario) -> ScenarioRun {
    scenario.run(Box::new(NullArbiter))
}

fn weight_of(record: &TickRecord, source: ProcessorId) -> Option<f64> {
    record
        .candidates
        .iter()
        .find(|c| c.source == source)
        .map(|c| c.weight)
}

#[test]
fn garden_path_weights_follow_the_spec() {
    let run = run(garden_path::build());
    assert_eq!(run.records.len(), 200);
    for r in &run.records {
        assert_eq!(weight_of(r, garden_path::A), Some(0.9), "t={}", r.t);
        let expected_b = if r.t < 50 { 0.3 } else { 0.6 };
        assert_eq!(weight_of(r, garden_path::B), Some(expected_b), "t={}", r.t);
    }
}

#[test]
fn stable_weights_follow_the_spec_and_nothing_is_critical() {
    let run = run(stable::build());
    assert_eq!(run.records.len(), 200);
    assert!(run.truth.streams.is_empty());
    for r in &run.records {
        assert_eq!(weight_of(r, stable::DOMINANT), Some(0.9));
        assert_eq!(weight_of(r, stable::BACKGROUND_1), Some(0.2));
        assert_eq!(weight_of(r, stable::BACKGROUND_2), Some(0.25));
    }
}

#[test]
fn distractor_flood_weights_follow_the_spec() {
    let run = run(distractor_flood::build());
    assert_eq!(run.records.len(), 200);
    assert!(run.held_out);
    for r in &run.records {
        for &d in &distractor_flood::DISTRACTORS {
            let w = weight_of(r, d).expect("distractors propose every tick");
            assert!((0.3..0.9).contains(&w), "t={} {d} weight {w}", r.t);
        }
        let critical = weight_of(r, distractor_flood::CRITICAL);
        if r.t < 80 {
            assert_eq!(critical, None, "t={}", r.t);
        } else {
            assert_eq!(critical, Some(0.5), "t={}", r.t);
        }
    }
}

#[test]
fn distractor_weights_actually_fluctuate_and_differ_between_sources() {
    let run = run(distractor_flood::build());
    let series: Vec<Vec<f64>> = distractor_flood::DISTRACTORS
        .iter()
        .map(|&d| {
            run.records
                .iter()
                .map(|r| weight_of(r, d).unwrap())
                .collect()
        })
        .collect();
    for s in &series {
        let distinct: HashSet<u64> = s.iter().map(|w| w.to_bits()).collect();
        assert!(distinct.len() > 150, "weights barely vary");
    }
    assert_ne!(series[0], series[1]);
}

#[test]
fn ground_truth_is_exactly_the_critical_sources_chunks_from_onset() {
    let cases = [
        (garden_path::build(), garden_path::B, garden_path::ONSET),
        (
            distractor_flood::build(),
            distractor_flood::CRITICAL,
            distractor_flood::ONSET,
        ),
    ];
    for (scenario, source, onset) in cases {
        let run = run(scenario);
        assert_eq!(run.truth.streams.len(), 1, "{}", run.name);
        let proposed: HashSet<ChunkId> = run
            .records
            .iter()
            .filter(|r| r.t >= onset)
            .flat_map(|r| r.candidates.iter())
            .filter(|c| c.source == source)
            .map(|c| c.id)
            .collect();
        assert_eq!(run.truth.streams[0].chunks, proposed, "{}", run.name);
    }
}

#[test]
fn every_critical_stream_is_proposed_with_room_for_the_deadline() {
    for scenario in scenario::all() {
        let horizon = scenario.horizon;
        let run = run(scenario);
        for stream in &run.truth.streams {
            let first = run
                .records
                .iter()
                .find(|r| r.candidates.iter().any(|c| stream.contains(c.id)))
                .map(|r| r.t);
            let first =
                first.unwrap_or_else(|| panic!("{}: {} never proposed", run.name, stream.name));
            assert!(
                first + DEFAULT_DEADLINE < horizon,
                "{}: {} starts too close to the horizon",
                run.name,
                stream.name
            );
        }
    }
}

#[test]
fn payloads_do_not_leak_criticality() {
    for scenario in scenario::all() {
        let run = run(scenario);
        for c in run.records.iter().flat_map(|r| r.candidates.iter()) {
            let payload = c.payload.to_lowercase();
            assert!(
                !payload.contains("crit") && !payload.contains("signal"),
                "{}: payload {:?} hints at criticality",
                run.name,
                c.payload
            );
        }
    }
}

#[test]
fn scenarios_are_reproducible() {
    for (a, b) in scenario::all().into_iter().zip(scenario::all()) {
        let (ra, rb) = (run(a), run(b));
        assert_eq!(ra.records, rb.records, "{} is not deterministic", ra.name);
    }
}

#[test]
fn only_distractor_flood_is_held_out() {
    let held_out: Vec<&str> = scenario::all()
        .iter()
        .filter(|s| s.held_out)
        .map(|s| s.name)
        .collect();
    assert_eq!(held_out, vec!["distractor_flood"]);
}

#[test]
#[should_panic(expected = "proposed more than once")]
fn scenario_run_rejects_reused_chunk_ids() {
    use attn_workspace::Processor;
    use attn_workspace::metrics::GroundTruth;
    use attn_workspace::scenario::Scenario;

    /// Proposes the same chunk id on every tick: a contract violation.
    struct Reuser;
    impl Processor for Reuser {
        fn id(&self) -> ProcessorId {
            ProcessorId(0)
        }
        fn propose(&mut self, _t: Tick) -> Option<Chunk> {
            Some(Chunk {
                id: ChunkId(7),
                source: ProcessorId(0),
                weight: 0.5,
                payload: "A".to_string(),
            })
        }
        fn receive(&mut self, _winner: &Chunk, _t: Tick) {}
    }

    let scenario = Scenario {
        name: "reuser",
        description: "reuses a chunk id",
        horizon: 3,
        held_out: false,
        processors: vec![Box::new(Reuser)],
        truth: GroundTruth::default(),
    };
    scenario.run(Box::new(NullArbiter));
}

#[test]
fn every_scenario_and_development_instance_has_unique_chunk_ids() {
    use attn_workspace::metrics::duplicate_chunk_id;
    use attn_workspace::scenario::family_g::{self, DEV_SEEDS};
    // Scenario::run already panics on a duplicate; checking explicitly documents the property.
    for s in scenario::all() {
        let run = run(s);
        assert_eq!(duplicate_chunk_id(&run.records), None, "{}", run.name);
    }
    for seed in DEV_SEEDS {
        let run = run(family_g::instance(seed).0);
        assert_eq!(
            duplicate_chunk_id(&run.records),
            None,
            "family G seed {seed}"
        );
    }
}

#[test]
#[should_panic(expected = "does not fit the chunk id encoding")]
fn chunk_id_rejects_ticks_beyond_32_bits() {
    let _ = scenario::chunk_id(ProcessorId(0), 1u64 << 32);
}

#[test]
fn chunk_id_accepts_the_largest_encodable_tick() {
    let id = scenario::chunk_id(ProcessorId(1), u64::from(u32::MAX));
    assert_eq!(id, ChunkId((1u64 << 32) | u64::from(u32::MAX)));
}
