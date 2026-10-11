//! P1 acceptance: the argmax baseline's selection rule, and its failure on `garden_path`.

use attn_workspace::metrics::MetricsConfig;
use attn_workspace::scenario::{garden_path, stable};
use attn_workspace::{Arbiter, ArgmaxArbiter, Chunk, ChunkId, ProcessorId};

fn chunk(source: u32, weight: f64) -> Chunk {
    Chunk {
        id: ChunkId(u64::from(source)),
        source: ProcessorId(source),
        weight,
        payload: String::new(),
    }
}

/// P1 acceptance criterion. If this ever fails, the scenario is wrong: fix the scenario,
/// never the baseline.
#[test]
fn baseline_misses_the_critical_signal_in_garden_path() {
    let run = garden_path::build().run(Box::new(ArgmaxArbiter::new()));
    let metrics = run.metrics(&MetricsConfig::default());

    let outcome = &metrics.streams[0];
    assert_eq!(outcome.first_proposed, Some(garden_path::ONSET));
    assert_eq!(outcome.first_won, None);
    assert_eq!(outcome.switch_latency, None);
    assert!(outcome.missed_critical);

    // Full fixation on the salient source.
    assert_eq!(metrics.switch_count, 0);
    assert_eq!(metrics.longest_streak(), Some((garden_path::A, 200)));
}

#[test]
fn baseline_never_switches_on_stable() {
    let run = stable::build().run(Box::new(ArgmaxArbiter::new()));
    let metrics = run.metrics(&MetricsConfig::default());
    assert_eq!(metrics.switch_count, 0);
    assert_eq!(metrics.longest_streak(), Some((stable::DOMINANT, 200)));
}

#[test]
fn picks_the_highest_weight_and_reports_weights_as_scores() {
    let candidates = [chunk(0, 0.2), chunk(1, 0.7), chunk(2, 0.5)];
    let selection = ArgmaxArbiter::new().select(&candidates, 0);
    assert_eq!(selection.winner, Some(1));
    assert_eq!(selection.scores, vec![0.2, 0.7, 0.5]);
    assert!(selection.reason.contains("P1"), "{}", selection.reason);
}

#[test]
fn ties_go_to_the_lowest_processor_id_even_if_candidates_are_unordered() {
    let candidates = [chunk(4, 0.5), chunk(2, 0.5), chunk(3, 0.1)];
    let selection = ArgmaxArbiter::new().select(&candidates, 0);
    assert_eq!(selection.winner, Some(1));
}

#[test]
fn no_candidates_means_no_winner() {
    let selection = ArgmaxArbiter::new().select(&[], 0);
    assert_eq!(selection.winner, None);
    assert!(selection.scores.is_empty());
}

#[test]
fn non_finite_weights_are_never_selected() {
    let candidates = [chunk(0, f64::NAN), chunk(1, 0.1), chunk(2, f64::INFINITY)];
    let selection = ArgmaxArbiter::new().select(&candidates, 0);
    assert_eq!(selection.winner, Some(1));

    let only_nan = [chunk(0, f64::NAN)];
    assert_eq!(ArgmaxArbiter::new().select(&only_nan, 0).winner, None);
}
