//! Tournament selection over scored candidates (`docs/experiments/p2-protocol.md` §3.2).
//!
//! Leaves are the entries in ascending `ProcessorId` order. In each round adjacent entries are
//! paired left to right; with an odd count the last entry gets a bye and advances without a
//! contest. Rounds repeat until one entry remains.

use crate::rng::SplitMix64;
use crate::workspace::ProcessorId;

/// A tournament entrant: the candidate's index in the arbiter's candidate slice, its source
/// and its score.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entry {
    pub index: usize,
    pub source: ProcessorId,
    pub score: f64,
}

/// Probability that `a` beats `b`: `1 / (1 + exp(−(score_a − score_b) / τ))`. Equal scores
/// give exactly 0.5. Written to avoid overflow for large score gaps.
pub fn win_probability(a: f64, b: f64, tau: f64) -> f64 {
    if a == b {
        return 0.5;
    }
    let d = (a - b) / tau;
    if d >= 0.0 {
        1.0 / (1.0 + (-d).exp())
    } else {
        let e = d.exp();
        e / (1.0 + e)
    }
}

/// The stochastic tournament. One `u = rng.next_f64()` is consumed per contest, round by round
/// and left to right; `a` wins iff `u < P(a wins)`. Byes consume no draw. Returns the winning
/// entry's `index`, or `None` for no entries.
///
/// `entries` must already be in ascending `ProcessorId` order.
pub fn stochastic(entries: &[Entry], tau: f64, rng: &mut SplitMix64) -> Option<usize> {
    run(entries, |a, b| {
        let u = rng.next_f64();
        if u < win_probability(a.score, b.score, tau) {
            a
        } else {
            b
        }
    })
}

/// The deterministic tournament: the higher score wins, ties go to the lower `ProcessorId`.
/// Used only as an implementation check against argmax (§3.2).
pub fn deterministic(entries: &[Entry]) -> Option<usize> {
    run(entries, |a, b| {
        if a.score > b.score || (a.score == b.score && a.source < b.source) {
            a
        } else {
            b
        }
    })
}

fn run(entries: &[Entry], mut contest: impl FnMut(Entry, Entry) -> Entry) -> Option<usize> {
    debug_assert!(
        entries.windows(2).all(|w| w[0].source < w[1].source),
        "tournament entries must be in ascending ProcessorId order"
    );
    let mut round: Vec<Entry> = entries.to_vec();
    while round.len() > 1 {
        let mut next = Vec::with_capacity(round.len().div_ceil(2));
        let mut pairs = round.chunks_exact(2);
        for pair in &mut pairs {
            next.push(contest(pair[0], pair[1]));
        }
        // An odd entry left over gets a bye.
        next.extend_from_slice(pairs.remainder());
        round = next;
    }
    round.first().map(|e| e.index)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(scores: &[f64]) -> Vec<Entry> {
        scores
            .iter()
            .enumerate()
            .map(|(i, &score)| Entry {
                index: i,
                source: ProcessorId(i as u32),
                score,
            })
            .collect()
    }

    /// A generator that counts how many draws were consumed.
    fn draws_used(scores: &[f64], seed: u64) -> u64 {
        let mut rng = SplitMix64::new(seed);
        stochastic(&entries(scores), 0.1, &mut rng);
        let mut probe = SplitMix64::new(seed);
        let mut n = 0;
        while probe != rng {
            probe.next_u64();
            n += 1;
        }
        n
    }

    #[test]
    fn ties_are_a_fair_coin_for_any_tau() {
        assert_eq!(win_probability(0.3, 0.3, 1e-9), 0.5);
        assert_eq!(win_probability(0.3, 0.3, 10.0), 0.5);
    }

    #[test]
    fn unequal_scores_approach_deterministic_as_tau_shrinks() {
        assert!(win_probability(0.6, 0.5, 1e-3) > 1.0 - 1e-12);
        assert!(win_probability(0.5, 0.6, 1e-3) < 1e-12);
        assert!((win_probability(0.6, 0.5, 0.1) - 1.0 / (1.0 + (-1.0f64).exp())).abs() < 1e-15);
    }

    #[test]
    fn win_probability_does_not_overflow() {
        assert_eq!(win_probability(1e6, -1e6, 1e-3), 1.0);
        assert_eq!(win_probability(-1e6, 1e6, 1e-3), 0.0);
    }

    #[test]
    fn one_draw_per_contest_and_none_for_byes() {
        // n entries always need n − 1 contests, byes included.
        assert_eq!(draws_used(&[], 1), 0);
        assert_eq!(draws_used(&[0.5], 1), 0);
        assert_eq!(draws_used(&[0.5, 0.4], 1), 1);
        assert_eq!(draws_used(&[0.5, 0.4, 0.3], 1), 2);
        assert_eq!(draws_used(&[0.5, 0.4, 0.3, 0.2, 0.1], 1), 4);
    }

    #[test]
    fn bracket_pairs_adjacent_entries_and_gives_the_last_a_bye() {
        // [a, b, c]: round 1 is a vs b with c on a bye; the final is that winner vs c.
        assert_eq!(deterministic(&entries(&[0.2, 0.9, 0.1])), Some(1));
        // A tie in the final goes to the lower id.
        assert_eq!(deterministic(&entries(&[0.5, 0.1, 0.5])), Some(0));
    }

    #[test]
    fn deterministic_tournament_equals_argmax_on_random_sets() {
        let mut rng = SplitMix64::new(42);
        for _ in 0..10_000 {
            let n = 1 + (rng.next_f64() * 13.0) as usize;
            // Coarse scores so ties are common.
            let scores: Vec<f64> = (0..n)
                .map(|_| (rng.next_f64() * 5.0).floor() / 5.0)
                .collect();
            let es = entries(&scores);
            let argmax = es
                .iter()
                .fold(None::<Entry>, |best, &e| match best {
                    Some(b) if b.score > e.score || (b.score == e.score && b.source < e.source) => {
                        Some(b)
                    }
                    _ => Some(e),
                })
                .map(|e| e.index);
            assert_eq!(deterministic(&es), argmax, "scores {scores:?}");
        }
    }

    #[test]
    fn stochastic_tournament_is_reproducible() {
        let es = entries(&[0.5, 0.45, 0.4, 0.35, 0.3]);
        let run = |seed| {
            let mut rng = SplitMix64::new(seed);
            (0..100)
                .map(|_| stochastic(&es, 0.2, &mut rng))
                .collect::<Vec<_>>()
        };
        assert_eq!(run(3), run(3));
    }
}
