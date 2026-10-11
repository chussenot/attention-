//! Uniform random baseline (arm RND, `docs/experiments/p2-protocol.md` §3.3).

use super::{Arbiter, Selection};
use crate::rng::SplitMix64;
use crate::workspace::{Chunk, Tick};

/// Each candidate is equally likely. One draw `floor(u · n)` per tick with at least one
/// candidate, none on empty ticks (Amendment 1, item 3). Scores are the raw weights, for the
/// record only (Amendment 1, item 2).
#[derive(Clone, Debug)]
pub struct UniformRandomArbiter {
    rng: SplitMix64,
}

impl UniformRandomArbiter {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: SplitMix64::new(seed),
        }
    }
}

impl Arbiter for UniformRandomArbiter {
    fn select(&mut self, candidates: &[Chunk], _t: Tick) -> Selection {
        let scores: Vec<f64> = candidates.iter().map(|c| c.weight).collect();
        let n = candidates.len();
        let winner = (n > 0).then(|| ((self.rng.next_f64() * n as f64) as usize).min(n - 1));
        let reason = match winner {
            Some(i) => format!("uniform random: {}", candidates[i].source),
            None => "uniform random: no candidates".to_string(),
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
    use crate::workspace::{ChunkId, ProcessorId};

    #[test]
    fn picks_every_candidate_and_is_reproducible() {
        let c: Vec<Chunk> = (0..4)
            .map(|i| Chunk {
                id: ChunkId(i),
                source: ProcessorId(i as u32),
                weight: 0.1,
                payload: String::new(),
            })
            .collect();
        let run = |seed| {
            let mut a = UniformRandomArbiter::new(seed);
            (0..400)
                .map(|t| a.select(&c, t).winner.unwrap())
                .collect::<Vec<_>>()
        };
        let winners = run(5);
        assert_eq!(winners, run(5));
        for i in 0..4 {
            assert!(winners.contains(&i));
        }
        assert_eq!(UniformRandomArbiter::new(1).select(&[], 0).winner, None);
    }
}
