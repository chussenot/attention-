//! Estimands, the selection rule and decision rules of `docs/experiments/p2-protocol.md`
//! §2, §5 and §6.3.
//!
//! Every estimand is a linear contrast of arms, evaluated per instance as the mean over the
//! chosen configurations of the per-run values, then aggregated with the stratum weighting of
//! §5.1. Intervals come from stratified resamples generated once per set and reused for every
//! estimand, so all comparisons stay paired.

use super::arms::{Arm, ParamVector, TAU_RANGE};
use super::grid::Grid;
use crate::stats::{self, LEVEL_95, LEVEL_98_75, Level, Z95};

/// Which per-run value an estimand uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Metric {
    /// Critical miss (0/1), over critical strata.
    Miss,
    /// Per-run switch rate per 100 eligible transitions, over stable strata.
    SwitchRate,
}

impl Metric {
    fn uses_stratum(self, critical: bool) -> bool {
        match self {
            Metric::Miss => critical,
            Metric::SwitchRate => !critical,
        }
    }
}

/// A linear combination of arms, e.g. `A3 − A2 − A1 + A0`.
#[derive(Clone, Debug, PartialEq)]
pub struct Contrast(pub Vec<(Arm, f64)>);

impl Contrast {
    pub fn arm(arm: Arm) -> Self {
        Contrast(vec![(arm, 1.0)])
    }

    pub fn diff(a: Arm, b: Arm) -> Self {
        Contrast(vec![(a, 1.0), (b, -1.0)])
    }
}

fn value(grid: &Grid, arm: Arm, c: usize, i: usize, metric: Metric) -> Option<f64> {
    let o = grid.outcome(arm, c, i);
    match metric {
        Metric::Miss => o.missed.map(|m| if m { 1.0 } else { 0.0 }),
        Metric::SwitchRate => o.switch_rate,
    }
}

/// Per-instance values: for each instance, the mean over `configs` of the contrast, skipping
/// configurations where any arm's value is undefined. `None` if every configuration is skipped
/// or the instance's stratum does not feed this metric.
pub fn per_instance(
    grid: &Grid,
    contrast: &Contrast,
    metric: Metric,
    configs: &[usize],
) -> Vec<Option<f64>> {
    (0..grid.instances.len())
        .map(|i| {
            let stratum = &grid.strata[grid.instances[i].stratum];
            if !metric.uses_stratum(stratum.critical) {
                return None;
            }
            let mut sum = 0.0;
            let mut n = 0usize;
            'configs: for &c in configs {
                let mut total = 0.0;
                for &(arm, coef) in &contrast.0 {
                    match value(grid, arm, c, i, metric) {
                        Some(v) => total += coef * v,
                        None => continue 'configs,
                    }
                }
                sum += total;
                n += 1;
            }
            (n > 0).then(|| sum / n as f64)
        })
        .collect()
}

/// Stratum-weighted mean (§5.1): the mean of defined values within each stratum that feeds the
/// metric, then the unweighted mean over those strata. `members` lists instance indices per
/// stratum (original or resampled). Sums run in the given order.
pub fn weighted(
    grid: &Grid,
    values: &[Option<f64>],
    metric: Metric,
    members: &[Vec<usize>],
) -> Option<f64> {
    let mut stratum_means = Vec::new();
    for (s, list) in members.iter().enumerate() {
        if !metric.uses_stratum(grid.strata[s].critical) {
            continue;
        }
        let defined: Vec<f64> = list.iter().filter_map(|&i| values[i]).collect();
        if !defined.is_empty() {
            stratum_means.push(defined.iter().sum::<f64>() / defined.len() as f64);
        }
    }
    (!stratum_means.is_empty())
        .then(|| stratum_means.iter().sum::<f64>() / stratum_means.len() as f64)
}

/// A point estimate with a percentile bootstrap interval.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Estimate {
    pub point: f64,
    pub lo: f64,
    pub hi: f64,
    pub level: Level,
}

/// Stratified resamples for a grid (§5.2): `RESAMPLES` draws from
/// `SplitMix64(0x5EED_0003 + set_id)`.
pub fn resamples_for(grid: &Grid) -> Vec<Vec<Vec<usize>>> {
    stats::stratified_resamples(
        &grid.members(),
        stats::RESAMPLES,
        stats::BOOTSTRAP_SEED_BASE + grid.set_id,
    )
}

/// Estimate of `contrast` on `metric`, averaged over `configs`, with an interval at `level`.
pub fn estimate(
    grid: &Grid,
    resamples: &[Vec<Vec<usize>>],
    contrast: &Contrast,
    metric: Metric,
    configs: &[usize],
    level: Level,
) -> Option<Estimate> {
    let values = per_instance(grid, contrast, metric, configs);
    let point = weighted(grid, &values, metric, &grid.members())?;
    let replicates: Vec<f64> = resamples
        .iter()
        .filter_map(|r| weighted(grid, &values, metric, r))
        .collect();
    if replicates.is_empty() {
        return None;
    }
    let (lo, hi) = stats::percentile_interval(&replicates, level);
    Some(Estimate {
        point,
        lo,
        hi,
        level,
    })
}

/// Monte-Carlo error from using the configurations in `configs` instead of the full measure:
/// the standard deviation across configurations of the per-configuration estimate, divided by
/// √(number of configurations) (§5.2).
pub fn monte_carlo_error(
    grid: &Grid,
    contrast: &Contrast,
    metric: Metric,
    configs: &[usize],
) -> Option<f64> {
    let members = grid.members();
    let per_config: Vec<f64> = configs
        .iter()
        .filter_map(|&c| {
            weighted(
                grid,
                &per_instance(grid, contrast, metric, &[c]),
                metric,
                &members,
            )
        })
        .collect();
    let n = per_config.len();
    if n < 2 {
        return None;
    }
    let mean = per_config.iter().sum::<f64>() / n as f64;
    let var = per_config.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1) as f64;
    Some(var.sqrt() / (n as f64).sqrt())
}

/// Floor diagnostic for H3 (§2.1): the fraction of (configuration, critical instance) cells
/// where A1 and A2 both detect the critical stream.
pub fn floor_diagnostic(grid: &Grid, configs: &[usize]) -> f64 {
    let mut both = 0u64;
    let mut cells = 0u64;
    for i in 0..grid.instances.len() {
        if !grid.strata[grid.instances[i].stratum].critical {
            continue;
        }
        for &c in configs {
            let a1 = grid.outcome(Arm::A1, c, i).missed;
            let a2 = grid.outcome(Arm::A2, c, i).missed;
            if let (Some(m1), Some(m2)) = (a1, a2) {
                cells += 1;
                if !m1 && !m2 {
                    both += 1;
                }
            }
        }
    }
    if cells == 0 {
        0.0
    } else {
        both as f64 / cells as f64
    }
}

/// Configuration indices `0..n` (all 512 in a protocol run).
pub fn all_configs(n: usize) -> Vec<usize> {
    (0..n).collect()
}

/// Configurations whose τ falls in tercile `t` (0, 1, 2) of the declared τ range, using
/// equal-width thirds of the range (Amendment 1, item 6).
pub fn tau_tercile(vectors: &[ParamVector], t: usize) -> Vec<usize> {
    let (lo, hi) = TAU_RANGE;
    let width = (hi - lo) / 3.0;
    vectors
        .iter()
        .enumerate()
        .filter(|(_, v)| {
            let bin = (((v.tau - lo) / width) as usize).min(2);
            bin == t
        })
        .map(|(c, _)| c)
        .collect()
}

// ---------------------------------------------------------------------------------------
// Hypotheses
// ---------------------------------------------------------------------------------------

/// A primary mechanism estimand (§2.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Mechanism {
    H1Habituation,
    H2Novelty,
    H3Interaction,
    H4Stochastic,
}

impl Mechanism {
    pub const ALL: [Mechanism; 4] = [
        Mechanism::H1Habituation,
        Mechanism::H2Novelty,
        Mechanism::H3Interaction,
        Mechanism::H4Stochastic,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Mechanism::H1Habituation => "H1",
            Mechanism::H2Novelty => "H2",
            Mechanism::H3Interaction => "H3",
            Mechanism::H4Stochastic => "H4",
        }
    }

    /// The primary contrast.
    pub fn contrast(self) -> Contrast {
        use Arm::*;
        match self {
            // ½[(A1 − A0) + (A3 − A2)]
            Mechanism::H1Habituation => {
                Contrast(vec![(A1, 0.5), (A0, -0.5), (A3, 0.5), (A2, -0.5)])
            }
            // ½[(A2 − A0) + (A3 − A1)]
            Mechanism::H2Novelty => Contrast(vec![(A2, 0.5), (A0, -0.5), (A3, 0.5), (A1, -0.5)]),
            // A3 − A2 − A1 + A0
            Mechanism::H3Interaction => {
                Contrast(vec![(A3, 1.0), (A2, -1.0), (A1, -1.0), (A0, 1.0)])
            }
            // ¼ Σₙ (Sn − An)
            Mechanism::H4Stochastic => Contrast(vec![
                (S0, 0.25),
                (A0, -0.25),
                (S1, 0.25),
                (A1, -0.25),
                (S2, 0.25),
                (A2, -0.25),
                (S3, 0.25),
                (A3, -0.25),
            ]),
        }
    }

    /// Secondary simple effects, with labels.
    pub fn simple_effects(self) -> Vec<(&'static str, Contrast)> {
        use Arm::*;
        match self {
            Mechanism::H1Habituation => vec![
                ("A1 − A0 (novelty off)", Contrast::diff(A1, A0)),
                ("A3 − A2 (novelty on)", Contrast::diff(A3, A2)),
            ],
            Mechanism::H2Novelty => vec![
                ("A2 − A0 (habituation off)", Contrast::diff(A2, A0)),
                ("A3 − A1 (habituation on)", Contrast::diff(A3, A1)),
            ],
            Mechanism::H3Interaction => vec![
                ("A3 − A1", Contrast::diff(A3, A1)),
                ("A3 − A2", Contrast::diff(A3, A2)),
            ],
            Mechanism::H4Stochastic => vec![
                ("S0 − A0", Contrast::diff(S0, A0)),
                ("S1 − A1", Contrast::diff(S1, A1)),
                ("S2 − A2", Contrast::diff(S2, A2)),
                ("S3 − A3", Contrast::diff(S3, A3)),
            ],
        }
    }
}

/// Interval level for primary mechanism estimands and H6 (§5.3).
pub const MECHANISM_LEVEL: Level = LEVEL_98_75;
/// Interval level for H7, secondary effects and per-arm rates (§5.3).
pub const DESCRIPTIVE_LEVEL: Level = LEVEL_95;

/// Verdict on a mechanism hypothesis from its primary miss-rate interval (§2.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MechanismVerdict {
    /// Upper bound below zero. For H3 this means complementary.
    Supported,
    /// H3 only: lower bound above zero.
    Redundant,
    /// Neither.
    NotSupported,
}

pub fn mechanism_verdict(m: Mechanism, e: &Estimate) -> MechanismVerdict {
    if e.hi < 0.0 {
        MechanismVerdict::Supported
    } else if m == Mechanism::H3Interaction && e.lo > 0.0 {
        MechanismVerdict::Redundant
    } else {
        MechanismVerdict::NotSupported
    }
}

/// H6 for one mechanism (§2.2): applicable only if supported on development data; supported
/// if every held-out interval excludes zero in the same (negative) direction.
pub fn h6_supported(dev: MechanismVerdict, heldout: &[Estimate]) -> Option<bool> {
    (dev == MechanismVerdict::Supported)
        .then(|| !heldout.is_empty() && heldout.iter().all(|e| e.hi < 0.0))
}

/// H7 on one held-out set (§2.2): miss-rate interval below zero and switch-rate increase upper
/// bound at most the provisional margin.
pub fn h7_supported_on(miss: &Estimate, switch: &Estimate) -> bool {
    miss.hi < 0.0 && switch.hi <= SWITCH_TARGET
}

/// H5 for one arm (§2.2): pass-fraction Wilson 95% lower bound at least the target.
pub fn h5_met(passing: u64, total: u64) -> (bool, (f64, f64)) {
    let ci = stats::wilson(passing, total, Z95);
    (ci.0 >= PASS_FRACTION_TARGET, ci)
}

// ---------------------------------------------------------------------------------------
// Pass and selection (§6.3)
// ---------------------------------------------------------------------------------------

/// Provisional targets (§4).
pub const MISS_TARGET: f64 = 0.05;
pub const SWITCH_TARGET: f64 = 2.5;
pub const PASS_FRACTION_TARGET: f64 = 0.20;

/// One candidate for the selection rule.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candidate {
    pub index: u64,
    pub miss: f64,
    pub switch: f64,
}

impl Candidate {
    pub fn passes(&self) -> bool {
        self.miss <= MISS_TARGET && self.switch <= SWITCH_TARGET
    }

    fn meets_switch_target(&self) -> bool {
        self.switch <= SWITCH_TARGET
    }
}

/// Which branch of the selection rule chose the result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Branch {
    /// Rule 1: something passed.
    Passing,
    /// Rule 2: nothing passed, something met the switch-rate target.
    SwitchTargetOnly,
    /// Rule 3: nothing met the switch-rate target. Reported as not passing.
    Neither,
}

/// The selection rule (§6.3). Every branch ends in a total order, with the index last.
///
/// # Panics
///
/// If `candidates` is empty.
pub fn select(candidates: &[Candidate]) -> (Candidate, Branch) {
    assert!(
        !candidates.is_empty(),
        "selection needs at least one candidate"
    );
    let miss_first = |a: &&Candidate, b: &&Candidate| {
        a.miss
            .total_cmp(&b.miss)
            .then(a.switch.total_cmp(&b.switch))
            .then(a.index.cmp(&b.index))
    };
    let switch_first = |a: &&Candidate, b: &&Candidate| {
        a.switch
            .total_cmp(&b.switch)
            .then(a.miss.total_cmp(&b.miss))
            .then(a.index.cmp(&b.index))
    };
    if let Some(best) = candidates.iter().filter(|c| c.passes()).min_by(miss_first) {
        return (*best, Branch::Passing);
    }
    if let Some(best) = candidates
        .iter()
        .filter(|c| c.meets_switch_target())
        .min_by(miss_first)
    {
        return (*best, Branch::SwitchTargetOnly);
    }
    (
        *candidates.iter().min_by(switch_first).expect("non-empty"),
        Branch::Neither,
    )
}

/// Per-configuration development rates of `arm` (class-weighted, §5.1), as selection
/// candidates. Undefined rates count as +∞ (worst).
pub fn arm_candidates(grid: &Grid, arm: Arm) -> Vec<Candidate> {
    let members = grid.members();
    (0..grid.n_configs(arm))
        .map(|c| {
            let rate = |metric| {
                weighted(
                    grid,
                    &per_instance(grid, &Contrast::arm(arm), metric, &[c]),
                    metric,
                    &members,
                )
                .unwrap_or(f64::INFINITY)
            };
            Candidate {
                index: c as u64,
                miss: rate(Metric::Miss),
                switch: rate(Metric::SwitchRate),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::experiment::grid::{Instance, RunOutcome, Stratum};
    use std::collections::BTreeMap;

    fn cand(index: u64, miss: f64, switch: f64) -> Candidate {
        Candidate {
            index,
            miss,
            switch,
        }
    }

    #[test]
    fn selection_rule_one_picks_lowest_miss_among_passing() {
        let c = [
            cand(0, 0.04, 2.0),
            cand(1, 0.01, 2.4),
            cand(2, 0.0, 9.0),
            cand(3, 0.01, 1.0),
        ];
        assert_eq!(select(&c), (cand(3, 0.01, 1.0), Branch::Passing));
    }

    #[test]
    fn selection_rule_ties_fall_through_to_index() {
        let c = [cand(5, 0.01, 1.0), cand(2, 0.01, 1.0)];
        assert_eq!(select(&c).0.index, 2);
    }

    #[test]
    fn selection_rule_two_when_nothing_passes() {
        let c = [cand(0, 0.30, 2.0), cand(1, 0.10, 2.5), cand(2, 0.0, 3.0)];
        assert_eq!(select(&c), (cand(1, 0.10, 2.5), Branch::SwitchTargetOnly));
    }

    #[test]
    fn selection_rule_three_ranks_by_switch_then_miss_then_index() {
        let c = [
            cand(0, 0.0, 9.0),
            cand(1, 0.5, 3.0),
            cand(2, 0.2, 3.0),
            cand(3, 0.2, 3.0),
        ];
        assert_eq!(select(&c), (cand(2, 0.2, 3.0), Branch::Neither));
    }

    #[test]
    fn verdicts_follow_the_decision_rules() {
        let e = |lo, hi| Estimate {
            point: (lo + hi) / 2.0,
            lo,
            hi,
            level: MECHANISM_LEVEL,
        };
        assert_eq!(
            mechanism_verdict(Mechanism::H1Habituation, &e(-0.3, -0.1)),
            MechanismVerdict::Supported
        );
        assert_eq!(
            mechanism_verdict(Mechanism::H1Habituation, &e(-0.3, 0.1)),
            MechanismVerdict::NotSupported
        );
        assert_eq!(
            mechanism_verdict(Mechanism::H1Habituation, &e(0.1, 0.3)),
            MechanismVerdict::NotSupported
        );
        assert_eq!(
            mechanism_verdict(Mechanism::H3Interaction, &e(0.1, 0.3)),
            MechanismVerdict::Redundant
        );
        assert_eq!(
            h6_supported(MechanismVerdict::NotSupported, &[e(-1.0, -0.5)]),
            None
        );
        assert_eq!(
            h6_supported(MechanismVerdict::Supported, &[e(-1.0, -0.5), e(-1.0, 0.1)]),
            Some(false)
        );
        assert_eq!(
            h6_supported(MechanismVerdict::Supported, &[e(-1.0, -0.5), e(-1.0, -0.1)]),
            Some(true)
        );
        assert!(h7_supported_on(&e(-0.4, -0.1), &e(0.0, 2.5)));
        assert!(!h7_supported_on(&e(-0.4, -0.1), &e(0.0, 2.6)));
        assert!(!h5_met(103, 512).0 && h5_met(140, 512).0);
    }

    /// Two critical strata and one stable stratum; three arms with hand-set outcomes.
    fn synthetic_grid() -> Grid {
        let strata = vec![
            Stratum {
                label: "X".into(),
                critical: true,
            },
            Stratum {
                label: "Y".into(),
                critical: true,
            },
            Stratum {
                label: "Z".into(),
                critical: false,
            },
        ];
        // X: instances 0, 1; Y: instance 2; Z: instances 3, 4.
        let instances: Vec<Instance> = [(0, 0), (1, 0), (2, 1), (3, 2), (4, 2)]
            .iter()
            .map(|&(seed, stratum)| Instance { seed, stratum })
            .collect();
        let crit = |missed| RunOutcome {
            missed: Some(missed),
            latency: None,
            switch_rate: None,
            switch_count: 0,
            eligible: 0,
            longest_streak: 0,
        };
        let stab = |rate| RunOutcome {
            missed: None,
            latency: None,
            switch_rate: Some(rate),
            switch_count: 0,
            eligible: 199,
            longest_streak: 0,
        };
        let mut runs = BTreeMap::new();
        // A0: misses everything; switch rate 0.
        runs.insert(
            Arm::A0,
            vec![vec![
                crit(true),
                crit(true),
                crit(true),
                stab(0.0),
                stab(0.0),
            ]],
        );
        // A1 (two configs): config 0 detects in X only; config 1 detects everywhere.
        runs.insert(
            Arm::A1,
            vec![
                vec![crit(false), crit(false), crit(true), stab(2.0), stab(4.0)],
                vec![
                    crit(false),
                    crit(false),
                    crit(false),
                    stab(10.0),
                    stab(10.0),
                ],
            ],
        );
        Grid {
            name: "synthetic".into(),
            set_id: 0,
            strata,
            instances,
            runs,
        }
    }

    #[test]
    fn weighting_gives_each_stratum_equal_weight() {
        let g = synthetic_grid();
        // A1 at config 0: X miss 0/2, Y miss 1/1 → (0 + 1)/2 = 0.5 (not pooled 1/3).
        let v = per_instance(&g, &Contrast::arm(Arm::A1), Metric::Miss, &[0]);
        assert_eq!(weighted(&g, &v, Metric::Miss, &g.members()), Some(0.5));
        // Stable: mean of 2 and 4.
        let v = per_instance(&g, &Contrast::arm(Arm::A1), Metric::SwitchRate, &[0]);
        assert_eq!(
            weighted(&g, &v, Metric::SwitchRate, &g.members()),
            Some(3.0)
        );
    }

    #[test]
    fn contrasts_average_over_configs_and_reuse_parameter_free_arms() {
        let g = synthetic_grid();
        // A1 − A0 over configs {0, 1}: X: (−1 + −1)/2 = −1 for both instances; Y: (0 + −1)/2.
        let v = per_instance(&g, &Contrast::diff(Arm::A1, Arm::A0), Metric::Miss, &[0, 1]);
        assert_eq!(v[..3], [Some(-1.0), Some(-1.0), Some(-0.5)]);
        assert_eq!(weighted(&g, &v, Metric::Miss, &g.members()), Some(-0.75));
        assert_eq!(v[3], None, "stable instances do not feed miss estimands");
    }

    #[test]
    fn estimate_interval_brackets_the_point_and_is_reproducible() {
        let g = synthetic_grid();
        let r = resamples_for(&g);
        let e = estimate(
            &g,
            &r,
            &Contrast::diff(Arm::A1, Arm::A0),
            Metric::Miss,
            &[0, 1],
            LEVEL_95,
        )
        .unwrap();
        assert_eq!(e.point, -0.75);
        assert!(e.lo <= e.point && e.point <= e.hi);
        assert_eq!(
            Some(e),
            estimate(
                &g,
                &r,
                &Contrast::diff(Arm::A1, Arm::A0),
                Metric::Miss,
                &[0, 1],
                LEVEL_95
            )
        );
    }

    #[test]
    fn candidates_use_weighted_rates() {
        let g = synthetic_grid();
        let c = arm_candidates(&g, Arm::A1);
        assert_eq!(c[0], cand(0, 0.5, 3.0));
        assert_eq!(c[1], cand(1, 0.0, 10.0));
        assert_eq!(select(&c), (cand(0, 0.5, 3.0), Branch::Neither));
    }
}
