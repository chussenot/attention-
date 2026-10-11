//! The P2 score arbiter (`docs/experiments/p2-protocol.md` §3.1–3.2), covering arms A1–A3
//! and S0–S3:
//!
//! ```text
//! score_i = w_i + N · η · nov[s(i)] − Hb · h[s(i)]
//! ```
//!
//! with habituation (`Hb`) and novelty (`N`) switched on or off per arm, selected either
//! deterministically or by the stochastic tournament.

use std::collections::BTreeMap;

use super::tournament::{self, Entry};
use super::{Arbiter, Selection};
use crate::rng::SplitMix64;
use crate::workspace::{Chunk, ProcessorId, Tick};

/// Habituation: selection-history state. After every selection, every known source's `h`
/// decays by `lambda`, then the winner's `h` grows by `k`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Habituation {
    pub k: f64,
    pub lambda: f64,
}

/// Novelty: signal state. `eta` scales the bonus and `rho` is its persistence.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Novelty {
    pub eta: f64,
    pub rho: f64,
}

/// How the winner is chosen from the scores.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Rule {
    /// The highest score wins; ties go to the lowest `ProcessorId`.
    Deterministic,
    /// The stochastic tournament at temperature `tau`, drawing from `SplitMix64(seed)`.
    Stochastic { tau: f64, seed: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScoreConfig {
    pub habituation: Option<Habituation>,
    pub novelty: Option<Novelty>,
    pub rule: Rule,
}

/// Arbiter for arms A1–A3 and S0–S3.
#[derive(Debug)]
pub struct ScoreArbiter {
    config: ScoreConfig,
    /// Habituation per source seen so far.
    h: BTreeMap<ProcessorId, f64>,
    /// Novelty bonus per source seen so far.
    nov: BTreeMap<ProcessorId, f64>,
    /// Weights proposed on the previous tick; absent means the source proposed nothing.
    prev: BTreeMap<ProcessorId, f64>,
    rng: Option<SplitMix64>,
}

impl ScoreArbiter {
    pub fn new(config: ScoreConfig) -> Self {
        let rng = match config.rule {
            Rule::Deterministic => None,
            Rule::Stochastic { seed, .. } => Some(SplitMix64::new(seed)),
        };
        Self {
            config,
            h: BTreeMap::new(),
            nov: BTreeMap::new(),
            prev: BTreeMap::new(),
            rng,
        }
    }

    /// Updates novelty from this tick's proposals, before scoring (Amendment 1, item 1):
    /// proposers get `nov ← max(δ, ρ·nov)` with `δ = max(0, w − prev)`, where `prev` is
    /// last tick's weight or 0 if the source proposed nothing then; known sources that do not
    /// propose get `nov ← ρ·nov`.
    fn update_novelty(&mut self, novelty: Novelty, candidates: &[Chunk]) {
        let proposing: BTreeMap<ProcessorId, f64> =
            candidates.iter().map(|c| (c.source, c.weight)).collect();
        for (source, bonus) in self.nov.iter_mut() {
            if !proposing.contains_key(source) {
                *bonus *= novelty.rho;
            }
        }
        for (&source, &weight) in &proposing {
            let previous = self.prev.get(&source).copied().unwrap_or(0.0);
            let delta = (weight - previous).max(0.0);
            let carried = novelty.rho * self.nov.get(&source).copied().unwrap_or(0.0);
            self.nov.insert(source, delta.max(carried));
        }
        self.prev = proposing;
    }

    fn score(&self, chunk: &Chunk) -> f64 {
        let mut score = chunk.weight;
        if let Some(novelty) = self.config.novelty {
            score += novelty.eta * self.nov.get(&chunk.source).copied().unwrap_or(0.0);
        }
        if self.config.habituation.is_some() {
            score -= self.h.get(&chunk.source).copied().unwrap_or(0.0);
        }
        score
    }

    /// After selection, every tick: decay, then add `k` to the winner (§3.1).
    fn update_habituation(&mut self, habituation: Habituation, winner: Option<ProcessorId>) {
        for value in self.h.values_mut() {
            *value *= habituation.lambda;
        }
        if let Some(source) = winner {
            *self.h.entry(source).or_insert(0.0) += habituation.k;
        }
    }
}

/// Argmax over finite scores, ties to the lowest `ProcessorId`. Returns the entry's index.
fn argmax(entries: &[Entry]) -> Option<usize> {
    let mut best: Option<Entry> = None;
    for &e in entries {
        let better = match best {
            None => true,
            Some(b) => e.score > b.score || (e.score == b.score && e.source < b.source),
        };
        if better {
            best = Some(e);
        }
    }
    best.map(|e| e.index)
}

impl Arbiter for ScoreArbiter {
    fn select(&mut self, candidates: &[Chunk], _t: Tick) -> Selection {
        if let Some(novelty) = self.config.novelty {
            self.update_novelty(novelty, candidates);
        }

        let scores: Vec<f64> = candidates.iter().map(|c| self.score(c)).collect();

        // Entrants: finite scores only, in ascending ProcessorId order (Amendment 1, item 4).
        let mut entries: Vec<Entry> = candidates
            .iter()
            .zip(&scores)
            .enumerate()
            .filter(|(_, (_, s))| s.is_finite())
            .map(|(index, (chunk, &score))| Entry {
                index,
                source: chunk.source,
                score,
            })
            .collect();
        entries.sort_by_key(|e| e.source);

        let winner = match self.config.rule {
            Rule::Deterministic => {
                let chosen = argmax(&entries);
                // Implementation check required by §3.2: on every tick of every deterministic
                // run, the deterministic tournament must pick exactly the argmax winner.
                assert_eq!(
                    tournament::deterministic(&entries),
                    chosen,
                    "deterministic tournament disagrees with argmax"
                );
                chosen
            }
            Rule::Stochastic { tau, .. } => {
                let rng = self.rng.as_mut().expect("stochastic rule has an RNG");
                tournament::stochastic(&entries, tau, rng)
            }
        };

        if let Some(habituation) = self.config.habituation {
            self.update_habituation(habituation, winner.map(|i| candidates[i].source));
        }

        let reason = match winner {
            Some(i) => format!("score: {} {:.3}", candidates[i].source, scores[i]),
            None => "score: no candidates".to_string(),
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

    fn chunk(source: u32, weight: f64) -> Chunk {
        Chunk {
            id: ChunkId(u64::from(source)),
            source: ProcessorId(source),
            weight,
            payload: String::new(),
        }
    }

    fn deterministic(habituation: Option<Habituation>, novelty: Option<Novelty>) -> ScoreArbiter {
        ScoreArbiter::new(ScoreConfig {
            habituation,
            novelty,
            rule: Rule::Deterministic,
        })
    }

    #[test]
    fn habituation_decays_then_adds_to_the_winner() {
        let mut a = deterministic(
            Some(Habituation {
                k: 0.1,
                lambda: 0.5,
            }),
            None,
        );
        let cands = [chunk(0, 0.9), chunk(1, 0.3)];
        // t0: h = 0 for everyone; A wins; then h_A = 0·0.5 + 0.1 = 0.1.
        assert_eq!(a.select(&cands, 0).winner, Some(0));
        assert_eq!(a.h[&ProcessorId(0)], 0.1);
        // t1: A scores 0.9 − 0.1 = 0.8; wins; h_A = 0.1·0.5 + 0.1 = 0.15.
        let s = a.select(&cands, 1);
        assert_eq!(s.scores, vec![0.9 - 0.1, 0.3]);
        assert!((a.h[&ProcessorId(0)] - 0.15).abs() < 1e-15);
        // Empty tick: decay only.
        a.select(&[], 2);
        assert!((a.h[&ProcessorId(0)] - 0.075).abs() < 1e-15);
    }

    #[test]
    fn novelty_counts_positive_increases_and_onsets_only() {
        let mut a = deterministic(None, Some(Novelty { eta: 1.0, rho: 0.5 }));
        // t0: both appear from silence → δ = w.
        let s = a.select(&[chunk(0, 0.9), chunk(1, 0.3)], 0);
        assert_eq!(s.scores, vec![0.9 + 0.9, 0.3 + 0.3]);
        // t1: no change → δ = 0; bonus carried at ρ.
        let s = a.select(&[chunk(0, 0.9), chunk(1, 0.3)], 1);
        assert_eq!(s.scores, vec![0.9 + 0.45, 0.3 + 0.15]);
        // t2: B rises to 0.6 → δ = 0.3 beats the carried 0.075; A drops → no bonus from δ.
        let s = a.select(&[chunk(0, 0.5), chunk(1, 0.6)], 2);
        assert!((s.scores[0] - (0.5 + 0.225)).abs() < 1e-15);
        assert!((s.scores[1] - (0.6 + 0.3)).abs() < 1e-12);
        // t3: B silent → its bonus decays; t4: B reappears → counts as an onset from 0.
        a.select(&[chunk(0, 0.5)], 3);
        let s = a.select(&[chunk(0, 0.5), chunk(1, 0.2)], 4);
        assert!((s.scores[1] - (0.2 + 0.2)).abs() < 1e-12);
    }

    #[test]
    fn no_terms_reduces_to_argmax_with_lowest_id_ties() {
        let mut a = deterministic(None, None);
        assert_eq!(a.select(&[chunk(3, 0.5), chunk(4, 0.5)], 0).winner, Some(0));
        assert_eq!(a.select(&[chunk(3, 0.2), chunk(4, 0.5)], 1).winner, Some(1));
        assert_eq!(a.select(&[], 2).winner, None);
    }

    #[test]
    fn non_finite_scores_are_never_selected() {
        let mut a = deterministic(None, None);
        assert_eq!(
            a.select(&[chunk(0, f64::NAN), chunk(1, 0.1)], 0).winner,
            Some(1)
        );
    }

    #[test]
    fn stochastic_rule_is_seeded_and_reproducible() {
        let config = ScoreConfig {
            habituation: Some(Habituation {
                k: 0.05,
                lambda: 0.9,
            }),
            novelty: None,
            rule: Rule::Stochastic { tau: 0.2, seed: 99 },
        };
        let run = || {
            let mut a = ScoreArbiter::new(config);
            (0..200)
                .map(|t| {
                    a.select(&[chunk(0, 0.9), chunk(1, 0.6), chunk(2, 0.5)], t)
                        .winner
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(run(), run());
    }
}
