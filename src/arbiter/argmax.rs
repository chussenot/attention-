//! P1 baseline: pick the most salient candidate.

use super::{Arbiter, Selection};
use crate::workspace::{Chunk, ProcessorId, Tick};

/// Selects the candidate with the highest weight; ties go to the lowest `ProcessorId`.
///
/// Stateless and history-blind by design: it is the naïve salience-maximiser that the
/// garden path scenario is built to defeat. Scores are the raw weights. Non-finite weights
/// (NaN, ±∞) are never selected but are still reported as scores.
#[derive(Clone, Copy, Debug, Default)]
pub struct ArgmaxArbiter;

impl ArgmaxArbiter {
    pub fn new() -> Self {
        Self
    }
}

impl Arbiter for ArgmaxArbiter {
    fn select(&mut self, candidates: &[Chunk], _t: Tick) -> Selection {
        let scores: Vec<f64> = candidates.iter().map(|c| c.weight).collect();

        // Tie-break on the source id explicitly rather than relying on candidate order, so
        // the rule holds even if a caller passes candidates in a different order.
        let mut best: Option<(usize, f64, ProcessorId)> = None;
        for (index, chunk) in candidates.iter().enumerate() {
            if !chunk.weight.is_finite() {
                continue;
            }
            let better = match best {
                None => true,
                Some((_, weight, source)) => {
                    chunk.weight > weight || (chunk.weight == weight && chunk.source < source)
                }
            };
            if better {
                best = Some((index, chunk.weight, chunk.source));
            }
        }

        let reason = match best {
            Some((_, weight, source)) => format!("argmax: {source} weight={weight:.3}"),
            None if candidates.is_empty() => "argmax: no candidates".to_string(),
            None => "argmax: no finite weight".to_string(),
        };
        Selection {
            winner: best.map(|(index, _, _)| index),
            scores,
            reason,
        }
    }
}
