//! Round-robin baseline (arm RR, `docs/experiments/p2-protocol.md` §3.3).

use super::{Arbiter, Selection};
use crate::workspace::{Chunk, ProcessorId, Tick};

/// The winner is the proposing source with the smallest id greater than the previous
/// winner's; if there is none, the smallest proposing id. "Previous winner" is the most recent
/// tick that had one. Scores are the raw weights, for the record only (Amendment 1, item 2).
#[derive(Clone, Debug, Default)]
pub struct RoundRobinArbiter {
    last: Option<ProcessorId>,
}

impl RoundRobinArbiter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Arbiter for RoundRobinArbiter {
    fn select(&mut self, candidates: &[Chunk], _t: Tick) -> Selection {
        let scores: Vec<f64> = candidates.iter().map(|c| c.weight).collect();
        let smallest = |filter: &dyn Fn(ProcessorId) -> bool| {
            candidates
                .iter()
                .enumerate()
                .filter(|(_, c)| filter(c.source))
                .min_by_key(|(_, c)| c.source)
                .map(|(i, _)| i)
        };
        let winner = match self.last {
            Some(last) => smallest(&|s| s > last).or_else(|| smallest(&|_| true)),
            None => smallest(&|_| true),
        };
        if let Some(i) = winner {
            self.last = Some(candidates[i].source);
        }
        let reason = match winner {
            Some(i) => format!("round-robin: {}", candidates[i].source),
            None => "round-robin: no candidates".to_string(),
        };
        Selection {
            winner,
            scores,
            reason,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::ChunkId;

    fn chunks(ids: &[u32]) -> Vec<Chunk> {
        ids.iter()
            .map(|&id| Chunk {
                id: ChunkId(u64::from(id)),
                source: ProcessorId(id),
                weight: 0.5,
                payload: String::new(),
            })
            .collect()
    }

    #[test]
    fn cycles_through_proposing_sources_in_id_order() {
        let mut rr = RoundRobinArbiter::new();
        let c = chunks(&[0, 2, 5]);
        let winners: Vec<u32> = (0..5)
            .map(|t| c[rr.select(&c, t).winner.unwrap()].source.0)
            .collect();
        assert_eq!(winners, vec![0, 2, 5, 0, 2]);
    }

    #[test]
    fn skips_absent_sources_and_empty_ticks() {
        let mut rr = RoundRobinArbiter::new();
        let all = chunks(&[0, 1, 2]);
        assert_eq!(rr.select(&all, 0).winner, Some(0)); // proc0
        assert_eq!(rr.select(&[], 1).winner, None); // empty tick keeps last = proc0
        let only_0_2 = chunks(&[0, 2]);
        assert_eq!(
            only_0_2[rr.select(&only_0_2, 2).winner.unwrap()].source,
            ProcessorId(2)
        );
    }
}
