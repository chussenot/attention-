//! Statistics for the P2 protocol (`docs/experiments/p2-protocol.md` §5): Wilson intervals,
//! the stratified paired bootstrap and the percentile interval rule.
//!
//! Everything is deterministic: the bootstrap draws come from an explicitly seeded
//! [`SplitMix64`], and interval indices use integer arithmetic so no floating-point rounding
//! can move a bound.

use crate::rng::SplitMix64;

/// Two-sided 95% normal quantile, used for Wilson intervals.
pub const Z95: f64 = 1.959963984540054;

/// Number of bootstrap resamples per set (§5.2).
pub const RESAMPLES: usize = 10_000;

/// Seed base for bootstrap resampling; the set id is added (§5.2).
pub const BOOTSTRAP_SEED_BASE: u64 = 0x5EED_0003;

/// A two-sided interval level, written as α in basis points (parts per 10,000) so that the
/// percentile indices are computed exactly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Level {
    pub alpha_bp: u64,
}

/// 95% intervals (α = 0.05).
pub const LEVEL_95: Level = Level { alpha_bp: 500 };

/// 98.75% intervals (α = 0.0125): Bonferroni across the four primary mechanism estimands
/// (§5.3).
pub const LEVEL_98_75: Level = Level { alpha_bp: 125 };

/// Wilson score interval for `successes` out of `n`, with normal quantile `z`.
///
/// # Panics
///
/// If `n == 0` or `successes > n`.
pub fn wilson(successes: u64, n: u64, z: f64) -> (f64, f64) {
    assert!(n > 0, "Wilson interval needs n > 0");
    assert!(successes <= n, "successes {successes} exceed n {n}");
    let n_f = n as f64;
    let p = successes as f64 / n_f;
    let z2 = z * z;
    let denom = 1.0 + z2 / n_f;
    let centre = (p + z2 / (2.0 * n_f)) / denom;
    let half = z * (p * (1.0 - p) / n_f + z2 / (4.0 * n_f * n_f)).sqrt() / denom;
    ((centre - half).max(0.0), (centre + half).min(1.0))
}

/// Percentile interval over bootstrap replicates (§5.2). Replicates are sorted with
/// `f64::total_cmp`; with N replicates, the lower bound is element `floor(N·α/2)` and the
/// upper bound element `ceil(N·(1 − α/2)) − 1`, both 0-based.
///
/// # Panics
///
/// If `replicates` is empty.
pub fn percentile_interval(replicates: &[f64], level: Level) -> (f64, f64) {
    assert!(!replicates.is_empty(), "no bootstrap replicates");
    let mut sorted = replicates.to_vec();
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len() as u64;
    let (lo, hi) = percentile_indices(n, level);
    (sorted[lo], sorted[hi])
}

/// The 0-based indices used by [`percentile_interval`] for `n` replicates.
pub fn percentile_indices(n: u64, level: Level) -> (usize, usize) {
    // α/2 = alpha_bp / 20_000.
    let lo = n * level.alpha_bp / 20_000;
    let upper_numerator = n * (20_000 - level.alpha_bp);
    let hi = upper_numerator.div_ceil(20_000) - 1;
    (lo as usize, hi as usize)
}

/// One stratified resample: for each stratum, as many draws (with replacement) as it has
/// members, each draw `floor(u · n_s)` with `u = rng.next_f64()`. Strata are processed in the
/// given order. The result holds, per stratum, the drawn member values (instance indices).
pub fn stratified_resample(strata: &[Vec<usize>], rng: &mut SplitMix64) -> Vec<Vec<usize>> {
    strata
        .iter()
        .map(|members| {
            let n = members.len();
            (0..n)
                .map(|_| {
                    let draw = (rng.next_f64() * n as f64) as usize;
                    members[draw.min(n - 1)]
                })
                .collect()
        })
        .collect()
}

/// `count` stratified resamples drawn from `SplitMix64(seed)`. Generating them once per set
/// and reusing them for every estimand keeps all comparisons paired (§5.2).
pub fn stratified_resamples(
    strata: &[Vec<usize>],
    count: usize,
    seed: u64,
) -> Vec<Vec<Vec<usize>>> {
    let mut rng = SplitMix64::new(seed);
    (0..count)
        .map(|_| stratified_resample(strata, &mut rng))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-12;

    fn close(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < EPS && (a.1 - b.1).abs() < EPS
    }

    #[test]
    fn wilson_matches_reference_values() {
        // Reference values computed independently from the closed-form Wilson formula.
        assert!(close(wilson(0, 10, Z95), (0.0, 0.2775327998628892)));
        assert!(close(
            wilson(5, 10, Z95),
            (0.236593090512564, 0.7634069094874361)
        ));
        assert!(close(
            wilson(10, 10, Z95),
            (0.7224672001371107, 0.9999999999999999)
        ));
        assert!(close(
            wilson(1, 512, Z95),
            (0.00034485776154337486, 0.010979278212421853)
        ));
        assert!(close(
            wilson(103, 512, Z95),
            (0.1687317782031719, 0.23806270338120725)
        ));
    }

    #[test]
    fn percentile_indices_follow_the_protocol_rule() {
        assert_eq!(percentile_indices(10_000, LEVEL_95), (250, 9749));
        assert_eq!(percentile_indices(10_000, LEVEL_98_75), (62, 9937));
    }

    #[test]
    fn percentile_interval_sorts_and_picks_indices() {
        let replicates: Vec<f64> = (0..10_000).rev().map(f64::from).collect();
        assert_eq!(percentile_interval(&replicates, LEVEL_95), (250.0, 9749.0));
    }

    #[test]
    fn resamples_keep_stratum_sizes_and_membership() {
        let strata = vec![vec![0, 1, 2], vec![10, 11]];
        for resample in stratified_resamples(&strata, 50, 1) {
            assert_eq!(resample[0].len(), 3);
            assert_eq!(resample[1].len(), 2);
            assert!(resample[0].iter().all(|i| strata[0].contains(i)));
            assert!(resample[1].iter().all(|i| strata[1].contains(i)));
        }
    }

    #[test]
    fn resamples_are_reproducible() {
        let strata = vec![vec![0, 1, 2, 3], vec![4, 5]];
        assert_eq!(
            stratified_resamples(&strata, 20, 7),
            stratified_resamples(&strata, 20, 7)
        );
        assert_ne!(
            stratified_resamples(&strata, 20, 7),
            stratified_resamples(&strata, 20, 8)
        );
    }
}
