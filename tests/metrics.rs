//! Metrics on hand-built record sequences, including edge cases.

use std::collections::{BTreeMap, HashSet};

use attn_workspace::metrics::{
    self, CriticalStream, GroundTruth, MetricsConfig, max_dominance_streak, score_entropy,
    stream_outcome, switch_count, tick_entropy,
};
use attn_workspace::{Chunk, ChunkId, ProcessorId, Tick, TickRecord};

const EPS: f64 = 1e-12;

fn chunk(source: u32, id: u64, weight: f64) -> Chunk {
    Chunk {
        id: ChunkId(id),
        source: ProcessorId(source),
        weight,
        payload: String::new(),
    }
}

/// A record whose scores equal the candidates' weights and whose winner is `winner`
/// (an index into `candidates`).
fn record(t: Tick, candidates: Vec<Chunk>, winner: Option<usize>) -> TickRecord {
    let winning = winner.map(|i| candidates[i].clone());
    TickRecord {
        t,
        scores: candidates.iter().map(|c| c.weight).collect(),
        candidates,
        winner: winning.as_ref().map(|c| c.id),
        winner_source: winning.as_ref().map(|c| c.source),
        reason: String::new(),
    }
}

fn empty(t: Tick) -> TickRecord {
    record(t, vec![], None)
}

/// Records where `winners[t]` is the winning source at tick t (`None` = empty tick).
/// Every source in the sequence is a candidate on its winning tick only.
fn won_by(winners: &[Option<u32>]) -> Vec<TickRecord> {
    winners
        .iter()
        .enumerate()
        .map(|(t, w)| match w {
            Some(s) => record(t as Tick, vec![chunk(*s, t as u64, 1.0)], Some(0)),
            None => empty(t as Tick),
        })
        .collect()
}

fn stream(name: &str, ids: &[u64]) -> CriticalStream {
    CriticalStream {
        name: name.to_string(),
        chunks: ids.iter().copied().map(ChunkId).collect::<HashSet<_>>(),
    }
}

// --- switch_latency / missed_critical -------------------------------------------------

/// Source 1 is dominant; source 2 proposes critical chunk ids 100+t from `onset`, and wins
/// at tick `win_at` (if any).
fn critical_sequence(len: u64, onset: u64, win_at: Option<u64>) -> Vec<TickRecord> {
    (0..len)
        .map(|t| {
            let mut candidates = vec![chunk(1, t, 0.9)];
            if t >= onset {
                candidates.push(chunk(2, 100 + t, 0.6));
            }
            let winner = if Some(t) == win_at { 1 } else { 0 };
            record(t, candidates, Some(winner))
        })
        .collect()
}

fn critical_ids(onset: u64, len: u64) -> Vec<u64> {
    (onset..len).map(|t| 100 + t).collect()
}

#[test]
fn latency_is_ticks_from_first_proposal_to_first_win() {
    let records = critical_sequence(30, 5, Some(12));
    let outcome = stream_outcome(&records, &stream("crit", &critical_ids(5, 30)), 20);
    assert_eq!(outcome.first_proposed, Some(5));
    assert_eq!(outcome.first_won, Some(12));
    assert_eq!(outcome.switch_latency, Some(7));
    assert!(!outcome.missed_critical);
}

#[test]
fn winning_on_the_first_proposed_tick_is_latency_zero() {
    let records = critical_sequence(10, 3, Some(3));
    let outcome = stream_outcome(&records, &stream("crit", &critical_ids(3, 10)), 20);
    assert_eq!(outcome.switch_latency, Some(0));
    assert!(!outcome.missed_critical);
}

#[test]
fn critical_stream_that_never_wins_is_missed_with_no_latency() {
    let records = critical_sequence(50, 5, None);
    let outcome = stream_outcome(&records, &stream("crit", &critical_ids(5, 50)), 20);
    assert_eq!(outcome.first_proposed, Some(5));
    assert_eq!(outcome.first_won, None);
    assert_eq!(outcome.switch_latency, None);
    assert!(outcome.missed_critical);
}

#[test]
fn deadline_is_inclusive() {
    let ids = critical_ids(0, 40);
    let on_time = stream_outcome(&critical_sequence(40, 0, Some(20)), &stream("c", &ids), 20);
    assert_eq!(on_time.switch_latency, Some(20));
    assert!(!on_time.missed_critical);

    let late = stream_outcome(&critical_sequence(40, 0, Some(21)), &stream("c", &ids), 20);
    assert_eq!(late.switch_latency, Some(21));
    assert!(late.missed_critical);
}

#[test]
fn never_won_counts_as_missed_even_if_run_ends_before_deadline() {
    let records = critical_sequence(10, 5, None);
    let outcome = stream_outcome(&records, &stream("crit", &critical_ids(5, 10)), 20);
    assert!(outcome.missed_critical);
}

#[test]
fn stream_never_proposed_is_not_missed() {
    let records = critical_sequence(10, 99, None);
    let outcome = stream_outcome(&records, &stream("ghost", &[12_345]), 20);
    assert_eq!(outcome.first_proposed, None);
    assert_eq!(outcome.switch_latency, None);
    assert!(!outcome.missed_critical);
}

#[test]
fn latency_uses_tick_values_not_slice_positions() {
    // Records starting at t = 100, as if taken from the middle of a run.
    let records: Vec<TickRecord> = critical_sequence(10, 2, Some(6))
        .into_iter()
        .map(|mut r| {
            r.t += 100;
            r
        })
        .collect();
    let outcome = stream_outcome(&records, &stream("crit", &critical_ids(2, 10)), 20);
    assert_eq!(outcome.first_proposed, Some(102));
    assert_eq!(outcome.first_won, Some(106));
    assert_eq!(outcome.switch_latency, Some(4));
}

// --- max_dominance_streak ---------------------------------------------------------------

#[test]
fn dominance_streak_per_source() {
    let records = won_by(&[
        Some(1),
        Some(1),
        Some(2),
        Some(1),
        Some(1),
        Some(1),
        Some(2),
    ]);
    let streaks = max_dominance_streak(&records);
    assert_eq!(
        streaks,
        BTreeMap::from([(ProcessorId(1), 3), (ProcessorId(2), 1)])
    );
}

#[test]
fn empty_tick_breaks_a_streak() {
    let records = won_by(&[Some(1), Some(1), None, Some(1), Some(1)]);
    assert_eq!(
        max_dominance_streak(&records),
        BTreeMap::from([(ProcessorId(1), 2)])
    );
}

#[test]
fn dominance_streak_on_no_records_is_empty() {
    assert!(max_dominance_streak(&[]).is_empty());
    assert!(max_dominance_streak(&won_by(&[None, None])).is_empty());
}

// --- switch_count -----------------------------------------------------------------------

#[test]
fn switch_count_counts_changes_of_winning_source() {
    let records = won_by(&[Some(1), Some(1), Some(2), Some(2), Some(1), Some(3)]);
    assert_eq!(switch_count(&records), 3);
}

#[test]
fn switch_count_skips_ticks_without_a_winner() {
    assert_eq!(switch_count(&won_by(&[Some(1), None, Some(1)])), 0);
    assert_eq!(switch_count(&won_by(&[Some(1), None, Some(2)])), 1);
    assert_eq!(switch_count(&won_by(&[None, Some(1), None])), 0);
}

#[test]
fn switch_count_on_no_records_is_zero() {
    assert_eq!(switch_count(&[]), 0);
}

// --- score_entropy ----------------------------------------------------------------------

#[test]
fn entropy_of_all_equal_scores_is_log2_n() {
    let h = tick_entropy(&[0.4, 0.4, 0.4, 0.4]).unwrap();
    assert!((h - 2.0).abs() < EPS, "got {h}");
}

#[test]
fn entropy_of_all_zero_scores_is_uniform() {
    let h = tick_entropy(&[0.0, 0.0]).unwrap();
    assert!((h - 1.0).abs() < EPS, "got {h}");
}

#[test]
fn entropy_of_single_candidate_is_zero() {
    let h = tick_entropy(&[0.7]).unwrap();
    assert_eq!(h, 0.0);
    assert!(h.is_sign_positive());
}

#[test]
fn entropy_of_non_negative_scores_is_not_shifted() {
    // [0.9, 0.3] → p = [0.75, 0.25]
    let expected = -(0.75f64 * 0.75f64.log2() + 0.25 * 0.25f64.log2());
    let h = tick_entropy(&[0.9, 0.3]).unwrap();
    assert!((h - expected).abs() < EPS, "got {h}, expected {expected}");
}

#[test]
fn negative_scores_are_shifted_to_non_negative() {
    // [-1, 1] → shift by 1 → [0, 2] → p = [0, 1] → 0 bits.
    assert_eq!(tick_entropy(&[-1.0, 1.0]), Some(0.0));
    // [-1, -1] → [0, 0] → uniform → 1 bit.
    let h = tick_entropy(&[-1.0, -1.0]).unwrap();
    assert!((h - 1.0).abs() < EPS);
}

#[test]
fn entropy_undefined_for_empty_tick_or_non_finite_score() {
    assert_eq!(tick_entropy(&[]), None);
    assert_eq!(tick_entropy(&[0.5, f64::NAN]), None);
    assert_eq!(tick_entropy(&[0.5, f64::INFINITY]), None);
}

#[test]
fn entropy_summary_excludes_undefined_ticks() {
    let records = vec![
        record(0, vec![chunk(1, 0, 0.5), chunk(2, 1, 0.5)], Some(0)), // 1 bit
        empty(1),                                                     // undefined
        record(2, vec![chunk(1, 2, 0.9)], Some(0)),                   // 0 bits
    ];
    let summary = score_entropy(&records);
    assert_eq!(summary.per_tick.len(), 3);
    assert_eq!(summary.per_tick[1], None);
    assert!((summary.mean.unwrap() - 0.5).abs() < EPS);
    assert_eq!(summary.min, Some(0.0));
}

#[test]
fn entropy_summary_of_only_empty_ticks_is_none() {
    let summary = score_entropy(&[empty(0), empty(1)]);
    assert_eq!(summary.mean, None);
    assert_eq!(summary.min, None);
}

// --- evaluate ---------------------------------------------------------------------------

#[test]
fn evaluate_combines_all_metrics() {
    let records = critical_sequence(40, 10, None);
    let truth = GroundTruth {
        streams: vec![stream("crit", &critical_ids(10, 40))],
    };
    let m = metrics::evaluate(&records, &truth, &MetricsConfig::default());

    assert_eq!(m.ticks, 40);
    assert_eq!(m.switch_count, 0);
    assert_eq!(m.longest_streak(), Some((ProcessorId(1), 40)));
    assert!(m.any_missed());
    assert_eq!(m.streams[0].switch_latency, None);
    assert_eq!(m.score_entropy.min, Some(0.0)); // single-candidate ticks before onset
}

#[test]
fn evaluate_on_empty_input() {
    let m = metrics::evaluate(&[], &GroundTruth::default(), &MetricsConfig::default());
    assert_eq!(m.ticks, 0);
    assert!(m.streams.is_empty());
    assert_eq!(m.longest_streak(), None);
    assert_eq!(m.switch_count, 0);
    assert_eq!(m.score_entropy.mean, None);
    assert!(!m.any_missed());
}

// --- large finite scores and chunk-id uniqueness ---------------------------------------------

#[test]
fn entropy_is_correct_for_large_finite_scores() {
    // Equal scores: one bit, however large (the unscaled sum overflowed to infinity).
    let h = tick_entropy(&[1e308, 1e308]).unwrap();
    assert!((h - 1.0).abs() < EPS, "got {h}");
    let h = tick_entropy(&[f64::MAX; 4]).unwrap();
    assert!((h - 2.0).abs() < EPS, "got {h}");
    // [1e308, 5e307] → p = [2/3, 1/3].
    let expected = -(2.0f64 / 3.0 * (2.0f64 / 3.0).log2() + 1.0 / 3.0 * (1.0f64 / 3.0).log2());
    let h = tick_entropy(&[1e308, 5e307]).unwrap();
    assert!((h - expected).abs() < EPS, "got {h}, expected {expected}");
    // Shifting extreme negative scores must not overflow either: [−MAX, MAX] → [0, 2] → 0 bits.
    assert_eq!(tick_entropy(&[-f64::MAX, f64::MAX]), Some(0.0));
    let h = tick_entropy(&[-f64::MAX, -f64::MAX]).unwrap();
    assert!((h - 1.0).abs() < EPS);
}

#[test]
fn entropy_is_unchanged_by_scaling() {
    for scores in [vec![0.9, 0.3], vec![0.2, 0.25, 0.9], vec![-0.4, 0.1, 0.7]] {
        let base = tick_entropy(&scores).unwrap();
        for k in [1e-300, 1e-6, 1e6, 1e300] {
            let scaled: Vec<f64> = scores.iter().map(|s| s * k).collect();
            let h = tick_entropy(&scaled).unwrap();
            assert!((h - base).abs() < 1e-9, "{scores:?} × {k}: {h} vs {base}");
        }
    }
}

#[test]
fn duplicate_chunk_ids_are_detected() {
    let unique = vec![
        record(0, vec![chunk(1, 10, 0.9), chunk(2, 20, 0.3)], Some(0)),
        record(1, vec![chunk(1, 11, 0.9), chunk(2, 21, 0.3)], Some(0)),
    ];
    assert_eq!(metrics::duplicate_chunk_id(&unique), None);

    // Reused across ticks by the same source.
    let across_ticks = vec![
        record(0, vec![chunk(1, 10, 0.9)], Some(0)),
        record(1, vec![chunk(1, 10, 0.9)], Some(0)),
    ];
    assert_eq!(
        metrics::duplicate_chunk_id(&across_ticks),
        Some(ChunkId(10))
    );

    // Reused by two sources on the same tick.
    let across_sources = vec![record(0, vec![chunk(1, 7, 0.9), chunk(2, 7, 0.3)], Some(0))];
    assert_eq!(
        metrics::duplicate_chunk_id(&across_sources),
        Some(ChunkId(7))
    );
}
