//! Metrics computed from tick records plus scenario ground truth.
//!
//! Every function here is a pure function of `&[TickRecord]` (and, where needed,
//! [`GroundTruth`]). Nothing reads arbiter or processor internals. Exact definitions and the
//! handling of edge cases are documented in `docs/design.md` §4.

use std::collections::{BTreeMap, HashSet};

use crate::workspace::{ChunkId, ProcessorId, Tick, TickRecord};

/// Default number of ticks a critical stream may wait before it counts as missed.
pub const DEFAULT_DEADLINE: u64 = 20;

/// A named set of chunks that the scenario designer considers critical.
///
/// Typically one source's chunks from some onset tick on. Only metrics read this; it is
/// never handed to a workspace, processor or arbiter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CriticalStream {
    pub name: String,
    pub chunks: HashSet<ChunkId>,
}

impl CriticalStream {
    pub fn contains(&self, id: ChunkId) -> bool {
        self.chunks.contains(&id)
    }
}

/// What the scenario designer knows and the arbiter must not: which chunks are critical.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GroundTruth {
    pub streams: Vec<CriticalStream>,
}

/// Evaluation parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MetricsConfig {
    /// A critical stream that does not win within this many ticks of its first proposal is
    /// missed. Inclusive: winning exactly `deadline` ticks later is on time.
    pub deadline: u64,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            deadline: DEFAULT_DEADLINE,
        }
    }
}

/// How one critical stream fared.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamOutcome {
    pub name: String,
    /// First tick any chunk of the stream was a candidate. `None` means the scenario never
    /// proposed it, which is a scenario bug rather than an arbiter failure.
    pub first_proposed: Option<Tick>,
    /// First tick (at or after `first_proposed`) a chunk of the stream won.
    pub first_won: Option<Tick>,
    /// `first_won − first_proposed`; `None` if it never won.
    pub switch_latency: Option<u64>,
    /// Proposed, and did not win within the deadline.
    pub missed_critical: bool,
}

/// Summary of per-tick score entropy.
#[derive(Clone, Debug, PartialEq)]
pub struct EntropySummary {
    /// Entropy in bits per record; `None` for ticks where it is undefined (no candidates or a
    /// non-finite score).
    pub per_tick: Vec<Option<f64>>,
    /// Mean over defined ticks; `None` if no tick is defined.
    pub mean: Option<f64>,
    /// Minimum over defined ticks; `None` if no tick is defined.
    pub min: Option<f64>,
}

/// All metrics for one run.
#[derive(Clone, Debug, PartialEq)]
pub struct Metrics {
    pub ticks: usize,
    pub streams: Vec<StreamOutcome>,
    pub max_dominance_streak: BTreeMap<ProcessorId, u64>,
    pub switch_count: u64,
    pub score_entropy: EntropySummary,
}

impl Metrics {
    /// The longest streak of any source, with that source (lowest id on ties).
    pub fn longest_streak(&self) -> Option<(ProcessorId, u64)> {
        // BTreeMap iterates in ascending id order and we only replace on a strictly longer
        // streak, so ties resolve to the lowest id.
        let mut best: Option<(ProcessorId, u64)> = None;
        for (&source, &streak) in &self.max_dominance_streak {
            if best.is_none_or(|(_, b)| streak > b) {
                best = Some((source, streak));
            }
        }
        best
    }

    /// True if any critical stream was missed.
    pub fn any_missed(&self) -> bool {
        self.streams.iter().any(|s| s.missed_critical)
    }
}

/// Computes every metric for one run.
pub fn evaluate(records: &[TickRecord], truth: &GroundTruth, config: &MetricsConfig) -> Metrics {
    Metrics {
        ticks: records.len(),
        streams: truth
            .streams
            .iter()
            .map(|stream| stream_outcome(records, stream, config.deadline))
            .collect(),
        max_dominance_streak: max_dominance_streak(records),
        switch_count: switch_count(records),
        score_entropy: score_entropy(records),
    }
}

/// `switch_latency` and `missed_critical` for one critical stream.
pub fn stream_outcome(
    records: &[TickRecord],
    stream: &CriticalStream,
    deadline: u64,
) -> StreamOutcome {
    let first_proposed = records
        .iter()
        .find(|r| r.candidates.iter().any(|c| stream.contains(c.id)))
        .map(|r| r.t);

    let first_won = first_proposed.and_then(|onset| {
        records
            .iter()
            .filter(|r| r.t >= onset)
            .find(|r| r.winner.is_some_and(|id| stream.contains(id)))
            .map(|r| r.t)
    });

    let switch_latency = match (first_proposed, first_won) {
        (Some(onset), Some(won)) => Some(won - onset),
        _ => None,
    };

    // Never proposed → not missed (scenario bug, asserted against in scenario tests).
    // Proposed but never won → missed, even if the run ended before the deadline elapsed.
    let missed_critical = first_proposed.is_some() && switch_latency.is_none_or(|l| l > deadline);

    StreamOutcome {
        name: stream.name.clone(),
        first_proposed,
        first_won,
        switch_latency,
        missed_critical,
    }
}

/// Longest run of adjacent records won by the same source, per source.
///
/// A record without a winner breaks every streak. Sources that never win are absent.
pub fn max_dominance_streak(records: &[TickRecord]) -> BTreeMap<ProcessorId, u64> {
    let mut best: BTreeMap<ProcessorId, u64> = BTreeMap::new();
    let mut current: Option<(ProcessorId, u64)> = None;

    for record in records {
        current = match (record.winner_source, current) {
            (Some(source), Some((prev, run))) if source == prev => Some((source, run + 1)),
            (Some(source), _) => Some((source, 1)),
            (None, _) => None,
        };
        if let Some((source, run)) = current {
            let entry = best.entry(source).or_insert(0);
            *entry = (*entry).max(run);
        }
    }
    best
}

/// Number of ticks whose winning source differs from that of the most recent tick that had a
/// winner. Ticks without a winner are skipped, not counted as switches.
pub fn switch_count(records: &[TickRecord]) -> u64 {
    let mut last: Option<ProcessorId> = None;
    let mut switches = 0;
    for source in records.iter().filter_map(|r| r.winner_source) {
        if last.is_some_and(|prev| prev != source) {
            switches += 1;
        }
        last = Some(source);
    }
    switches
}

/// A chunk id proposed more than once in a run, if any.
///
/// Ground truth and stream metrics identify chunks by `ChunkId`, so ids must be unique within
/// a run; a reused id could attribute a win to the wrong critical stream. Every chunk proposed
/// in the run counts, on every tick and from every processor.
pub fn duplicate_chunk_id(records: &[TickRecord]) -> Option<ChunkId> {
    // Sort and compare neighbours: cheaper than hashing every id, and deterministic. With
    // several duplicates, the smallest duplicated id is returned.
    let mut ids: Vec<u64> = records
        .iter()
        .flat_map(|r| r.candidates.iter())
        .map(|c| c.id.0)
        .collect();
    ids.sort_unstable();
    ids.windows(2).find(|w| w[0] == w[1]).map(|w| ChunkId(w[0]))
}

/// Number of winner-to-winner transitions that could have been switches: ticks with a winner,
/// minus one. Uses the same skipping convention as [`switch_count`].
pub fn eligible_transitions(records: &[TickRecord]) -> u64 {
    let winners = records.iter().filter(|r| r.winner_source.is_some()).count() as u64;
    winners.saturating_sub(1)
}

/// Switches per 100 eligible transitions for one run (`docs/experiments/p2-protocol.md` §4).
/// `None` when the run has no eligible transition; such runs are excluded, not counted as 0.
pub fn run_switch_rate(records: &[TickRecord]) -> Option<f64> {
    let eligible = eligible_transitions(records);
    (eligible > 0).then(|| 100.0 * switch_count(records) as f64 / eligible as f64)
}

/// Per-tick score entropy with its mean and minimum over defined ticks.
pub fn score_entropy(records: &[TickRecord]) -> EntropySummary {
    let per_tick: Vec<Option<f64>> = records.iter().map(|r| tick_entropy(&r.scores)).collect();
    let defined: Vec<f64> = per_tick.iter().flatten().copied().collect();

    let mean = (!defined.is_empty()).then(|| defined.iter().sum::<f64>() / defined.len() as f64);
    let min = defined.iter().copied().reduce(f64::min);

    EntropySummary {
        per_tick,
        mean,
        min,
    }
}

/// Shannon entropy in bits of one tick's scores, treated as a distribution.
///
/// Scores are shifted by `−min(0, min score)`, so non-negative scores are left unchanged
/// (always subtracting the minimum would zero out the weakest candidate), then normalised to
/// sum to 1. All-zero scores use the uniform distribution. Returns `None` for an empty slice
/// or if any score is non-finite.
pub fn tick_entropy(scores: &[f64]) -> Option<f64> {
    if scores.is_empty() || scores.iter().any(|s| !s.is_finite()) {
        return None;
    }

    // Divide by the largest magnitude first, so that neither the shift nor the sum can
    // overflow for large finite scores (e.g. [1e308, 1e308]). The distribution is invariant
    // under positive scaling, so this changes no result mathematically: scaled scores lie in
    // [−1, 1], shifted ones in [0, 2], and their sum is at most 2n.
    let scale = scores.iter().fold(0.0f64, |m, s| m.max(s.abs()));
    if scale == 0.0 {
        return Some((scores.len() as f64).log2());
    }
    let scaled: Vec<f64> = scores.iter().map(|s| s / scale).collect();

    let min = scaled.iter().copied().fold(f64::INFINITY, f64::min);
    let shift = if min < 0.0 { -min } else { 0.0 };
    let shifted: Vec<f64> = scaled.iter().map(|s| s + shift).collect();
    let total: f64 = shifted.iter().sum();

    if total == 0.0 {
        return Some((scores.len() as f64).log2());
    }

    let sum_p_log_p: f64 = shifted
        .iter()
        .map(|s| s / total)
        .filter(|&p| p > 0.0) // 0·log 0 = 0
        .map(|p| p * p.log2())
        .sum();
    // `0.0 - x` rather than `-x` so a single candidate yields +0.0, not -0.0.
    Some(0.0 - sum_p_log_p)
}
