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

/// One instance's value for an estimand, with how many configurations it used or skipped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InstanceValue {
    /// Mean over the used configurations; `None` if all were skipped or the instance's stratum
    /// does not feed this metric.
    pub value: Option<f64>,
    /// Whether the instance's stratum feeds this metric.
    pub relevant: bool,
    pub used: u32,
    /// Configurations skipped because some arm's value was undefined (Amendment 1, item 8).
    pub skipped: u32,
}

/// Per-instance values with skip counts: for each instance, the mean over `configs` of the
/// contrast, skipping configurations where any arm's value is undefined.
pub fn per_instance_detail(
    grid: &Grid,
    contrast: &Contrast,
    metric: Metric,
    configs: &[usize],
) -> Vec<InstanceValue> {
    (0..grid.instances.len())
        .map(|i| {
            let stratum = &grid.strata[grid.instances[i].stratum];
            if !metric.uses_stratum(stratum.critical) {
                return InstanceValue {
                    value: None,
                    relevant: false,
                    used: 0,
                    skipped: 0,
                };
            }
            let mut sum = 0.0;
            let mut used = 0u32;
            let mut skipped = 0u32;
            'configs: for &c in configs {
                let mut total = 0.0;
                for &(arm, coef) in &contrast.0 {
                    match value(grid, arm, c, i, metric) {
                        Some(v) => total += coef * v,
                        None => {
                            skipped += 1;
                            continue 'configs;
                        }
                    }
                }
                sum += total;
                used += 1;
            }
            InstanceValue {
                value: (used > 0).then(|| sum / f64::from(used)),
                relevant: true,
                used,
                skipped,
            }
        })
        .collect()
}

/// Per-instance values only. See [`per_instance_detail`].
pub fn per_instance(
    grid: &Grid,
    contrast: &Contrast,
    metric: Metric,
    configs: &[usize],
) -> Vec<Option<f64>> {
    per_instance_detail(grid, contrast, metric, configs)
        .iter()
        .map(|v| v.value)
        .collect()
}

/// Stratum-weighted mean (§5.1), with the number of strata that feed the metric but had no
/// defined value. Those strata are left out of the average (Amendment 1, item 8), which changes
/// the target population; callers must surface a non-zero count (Amendment 2).
pub fn weighted_detail(
    grid: &Grid,
    values: &[Option<f64>],
    metric: Metric,
    members: &[Vec<usize>],
) -> (Option<f64>, u32) {
    let mut stratum_means = Vec::new();
    let mut without_data = 0u32;
    for (s, list) in members.iter().enumerate() {
        if !metric.uses_stratum(grid.strata[s].critical) {
            continue;
        }
        let defined: Vec<f64> = list.iter().filter_map(|&i| values[i]).collect();
        if defined.is_empty() {
            without_data += 1;
        } else {
            stratum_means.push(defined.iter().sum::<f64>() / defined.len() as f64);
        }
    }
    let mean = (!stratum_means.is_empty())
        .then(|| stratum_means.iter().sum::<f64>() / stratum_means.len() as f64);
    (mean, without_data)
}

/// Stratum-weighted mean (§5.1). See [`weighted_detail`] for the strata it leaves out.
pub fn weighted(
    grid: &Grid,
    values: &[Option<f64>],
    metric: Metric,
    members: &[Vec<usize>],
) -> Option<f64> {
    weighted_detail(grid, values, metric, members).0
}

/// What an estimate covers (Amendment 2). Counts are over the strata that feed the metric.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Coverage {
    pub instances: u32,
    /// Instances with no defined value at all; left out of their stratum's mean.
    pub excluded_instances: u32,
    /// Instances where some configurations were used and others skipped. Undefined values in
    /// P2 depend only on the instance, not the arm, so this should be 0; a non-zero count means
    /// arm-dependent missingness.
    pub partially_defined_instances: u32,
    /// (configuration, instance) cells skipped.
    pub skipped_cells: u64,
    /// Strata with no defined value, left out of the point estimate's average.
    pub strata_without_data: u32,
    /// Bootstrap replicates in which some stratum had no defined value.
    pub replicates_dropping_stratum: u32,
    /// Bootstrap replicates with no defined value at all (left out of the interval).
    pub replicates_undefined: u32,
}

impl Coverage {
    /// The estimate describes the declared target population: no stratum left out, in the point
    /// estimate or any replicate, and no arm-dependent missingness. Excluded instances alone do
    /// not make it incomplete (complete-case within a stratum), but they are always reported.
    pub fn complete(&self) -> bool {
        self.strata_without_data == 0
            && self.partially_defined_instances == 0
            && self.replicates_dropping_stratum == 0
            && self.replicates_undefined == 0
    }

    /// One-line summary for reports.
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        if self.excluded_instances > 0 {
            parts.push(format!(
                "{}/{} instances excluded",
                self.excluded_instances, self.instances
            ));
        }
        if self.skipped_cells > 0 {
            parts.push(format!("{} cells skipped", self.skipped_cells));
        }
        if self.partially_defined_instances > 0 {
            parts.push(format!(
                "{} partially defined instances (arm-dependent missingness)",
                self.partially_defined_instances
            ));
        }
        if self.strata_without_data > 0 {
            parts.push(format!(
                "{} strata without data (population changed)",
                self.strata_without_data
            ));
        }
        if self.replicates_dropping_stratum > 0 {
            parts.push(format!(
                "{} replicates dropped a stratum",
                self.replicates_dropping_stratum
            ));
        }
        if self.replicates_undefined > 0 {
            parts.push(format!(
                "{} replicates undefined",
                self.replicates_undefined
            ));
        }
        if parts.is_empty() {
            "complete".to_string()
        } else if self.complete() {
            format!("complete; {}", parts.join("; "))
        } else {
            format!("INCOMPLETE: {}", parts.join("; "))
        }
    }
}

/// Per-stratum missingness for one metric over every arm and configuration of a grid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StratumCoverage {
    pub label: String,
    pub instances: u32,
    /// Instances where at least one run has an undefined value.
    pub undefined_in_some_run: u32,
    /// Instances where every run has an undefined value.
    pub undefined_in_every_run: u32,
}

/// Missingness table for one metric (Amendment 2): one row per stratum that feeds it.
pub fn stratum_coverage(grid: &Grid, metric: Metric) -> Vec<StratumCoverage> {
    let members = grid.members();
    grid.strata
        .iter()
        .enumerate()
        .filter(|(_, st)| metric.uses_stratum(st.critical))
        .map(|(s, st)| {
            let mut some = 0u32;
            let mut every = 0u32;
            for &i in &members[s] {
                let mut defined = 0usize;
                let mut total = 0usize;
                for (&arm, per_config) in &grid.runs {
                    for c in 0..per_config.len() {
                        total += 1;
                        if value(grid, arm, c, i, metric).is_some() {
                            defined += 1;
                        }
                    }
                }
                if defined < total {
                    some += 1;
                }
                if defined == 0 {
                    every += 1;
                }
            }
            StratumCoverage {
                label: st.label.clone(),
                instances: members[s].len() as u32,
                undefined_in_some_run: some,
                undefined_in_every_run: every,
            }
        })
        .collect()
}

/// A point estimate with a percentile bootstrap interval.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Estimate {
    pub point: f64,
    pub lo: f64,
    pub hi: f64,
    pub level: Level,
    pub coverage: Coverage,
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

/// Estimate of `contrast` on `metric`, averaged over `configs`, with an interval at `level`
/// and its coverage (Amendment 2). `None` only when no instance has a defined value.
pub fn estimate(
    grid: &Grid,
    resamples: &[Vec<Vec<usize>>],
    contrast: &Contrast,
    metric: Metric,
    configs: &[usize],
    level: Level,
) -> Option<Estimate> {
    let detail = per_instance_detail(grid, contrast, metric, configs);
    let values: Vec<Option<f64>> = detail.iter().map(|v| v.value).collect();
    let (point, strata_without_data) = weighted_detail(grid, &values, metric, &grid.members());
    let point = point?;

    let mut coverage = Coverage {
        strata_without_data,
        ..Coverage::default()
    };
    for v in detail.iter().filter(|v| v.relevant) {
        coverage.instances += 1;
        coverage.skipped_cells += u64::from(v.skipped);
        if v.value.is_none() {
            coverage.excluded_instances += 1;
        } else if v.skipped > 0 {
            coverage.partially_defined_instances += 1;
        }
    }

    let mut replicates = Vec::with_capacity(resamples.len());
    for r in resamples {
        match weighted_detail(grid, &values, metric, r) {
            (Some(x), dropped) => {
                if dropped > 0 {
                    coverage.replicates_dropping_stratum += 1;
                }
                replicates.push(x);
            }
            (None, _) => coverage.replicates_undefined += 1,
        }
    }
    if replicates.is_empty() {
        return None;
    }
    let (lo, hi) = stats::percentile_interval(&replicates, level);
    Some(Estimate {
        point,
        lo,
        hi,
        level,
        coverage,
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
    /// The estimate does not describe the declared target population (Amendment 2): no verdict.
    Undecidable,
    /// Upper bound below zero. For H3 this means complementary.
    Supported,
    /// H3 only: lower bound above zero.
    Redundant,
    /// Neither.
    NotSupported,
}

pub fn mechanism_verdict(m: Mechanism, e: &Estimate) -> MechanismVerdict {
    if !e.coverage.complete() {
        MechanismVerdict::Undecidable
    } else if e.hi < 0.0 {
        MechanismVerdict::Supported
    } else if m == Mechanism::H3Interaction && e.lo > 0.0 {
        MechanismVerdict::Redundant
    } else {
        MechanismVerdict::NotSupported
    }
}

/// H6 outcome for one mechanism.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum H6Verdict {
    /// The mechanism was not supported on development data.
    NotApplicable,
    Supported,
    NotSupported,
    /// A held-out estimate is missing or does not describe its declared population
    /// (Amendment 2).
    Undecidable,
}

/// H6 for one mechanism (§2.2): applicable only if supported on development data; supported
/// if every held-out interval excludes zero in the same (negative) direction.
pub fn h6_verdict(dev: MechanismVerdict, heldout: &[Option<Estimate>]) -> H6Verdict {
    if dev != MechanismVerdict::Supported {
        return H6Verdict::NotApplicable;
    }
    if heldout.is_empty()
        || heldout
            .iter()
            .any(|e| e.is_none_or(|e| !e.coverage.complete()))
    {
        return H6Verdict::Undecidable;
    }
    if heldout.iter().flatten().all(|e| e.hi < 0.0) {
        H6Verdict::Supported
    } else {
        H6Verdict::NotSupported
    }
}

/// H7 on one held-out set (§2.2): miss-rate interval below zero and switch-rate increase upper
/// bound at most the provisional margin. `None` (undecidable) if either estimate is missing or
/// does not describe its declared population (Amendment 2).
pub fn h7_on(miss: Option<&Estimate>, switch: Option<&Estimate>) -> Option<bool> {
    let (miss, switch) = (miss?, switch?);
    if !miss.coverage.complete() || !switch.coverage.complete() {
        return None;
    }
    Some(miss.hi < 0.0 && switch.hi <= SWITCH_TARGET)
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
            // A rate over an incomplete population (a stratum without data) is treated as
            // undefined, i.e. +∞ (Amendment 1, item 8; Amendment 2).
            let rate = |metric| {
                let values = per_instance(grid, &Contrast::arm(arm), metric, &[c]);
                match weighted_detail(grid, &values, metric, &members) {
                    (Some(x), 0) => x,
                    _ => f64::INFINITY,
                }
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
            coverage: Coverage::default(),
        };
        let incomplete = |lo, hi| Estimate {
            coverage: Coverage {
                strata_without_data: 1,
                ..Coverage::default()
            },
            ..e(lo, hi)
        };
        use MechanismVerdict::*;
        assert_eq!(
            mechanism_verdict(Mechanism::H1Habituation, &e(-0.3, -0.1)),
            Supported
        );
        assert_eq!(
            mechanism_verdict(Mechanism::H1Habituation, &e(-0.3, 0.1)),
            NotSupported
        );
        assert_eq!(
            mechanism_verdict(Mechanism::H1Habituation, &e(0.1, 0.3)),
            NotSupported
        );
        assert_eq!(
            mechanism_verdict(Mechanism::H3Interaction, &e(0.1, 0.3)),
            Redundant
        );
        // An incomplete population never yields a verdict, however clear the interval.
        assert_eq!(
            mechanism_verdict(Mechanism::H1Habituation, &incomplete(-0.3, -0.1)),
            Undecidable
        );

        let ok = Some(e(-1.0, -0.5));
        assert_eq!(h6_verdict(NotSupported, &[ok]), H6Verdict::NotApplicable);
        assert_eq!(
            h6_verdict(Supported, &[ok, Some(e(-1.0, 0.1))]),
            H6Verdict::NotSupported
        );
        assert_eq!(
            h6_verdict(Supported, &[ok, Some(e(-1.0, -0.1))]),
            H6Verdict::Supported
        );
        assert_eq!(h6_verdict(Supported, &[ok, None]), H6Verdict::Undecidable);
        assert_eq!(
            h6_verdict(Supported, &[ok, Some(incomplete(-1.0, -0.1))]),
            H6Verdict::Undecidable
        );

        assert_eq!(h7_on(Some(&e(-0.4, -0.1)), Some(&e(0.0, 2.5))), Some(true));
        assert_eq!(h7_on(Some(&e(-0.4, -0.1)), Some(&e(0.0, 2.6))), Some(false));
        assert_eq!(
            h7_on(Some(&incomplete(-0.4, -0.1)), Some(&e(0.0, 2.5))),
            None
        );
        assert_eq!(h7_on(None, Some(&e(0.0, 2.5))), None);
        assert!(!h5_met(103, 512).0 && h5_met(140, 512).0);
    }

    /// A grid with one critical stratum and two stable strata, where one stable stratum has no
    /// defined switch rate at all and one instance of the other is undefined.
    fn grid_with_missing_values() -> Grid {
        let strata = vec![
            Stratum {
                label: "X".into(),
                critical: true,
            },
            Stratum {
                label: "Y".into(),
                critical: false,
            },
            Stratum {
                label: "Z".into(),
                critical: false,
            },
        ];
        let instances: Vec<Instance> = [(0, 0), (1, 0), (2, 1), (3, 1), (4, 2)]
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
        let stab = |rate: Option<f64>| RunOutcome {
            missed: None,
            latency: None,
            switch_rate: rate,
            switch_count: 0,
            eligible: u64::from(rate.is_some()) * 199,
            longest_streak: 0,
        };
        let row = || {
            vec![
                crit(true),
                crit(false),
                stab(Some(4.0)),
                stab(None),
                stab(None),
            ]
        };
        let mut runs = BTreeMap::new();
        runs.insert(Arm::A0, vec![row()]);
        runs.insert(Arm::A1, vec![row(), row()]);
        Grid {
            name: "missing".into(),
            set_id: 0,
            strata,
            instances,
            runs,
        }
    }

    #[test]
    fn coverage_exposes_excluded_instances_and_dropped_strata() {
        let g = grid_with_missing_values();
        let r = resamples_for(&g);
        let e = estimate(
            &g,
            &r,
            &Contrast::arm(Arm::A1),
            Metric::SwitchRate,
            &[0, 1],
            LEVEL_95,
        )
        .unwrap();
        // Stratum Z has no defined value: the point is Y's mean only, and that is visible.
        assert_eq!(e.point, 4.0);
        assert_eq!(e.coverage.instances, 3);
        assert_eq!(e.coverage.excluded_instances, 2);
        assert_eq!(e.coverage.strata_without_data, 1);
        assert_eq!(e.coverage.skipped_cells, 4);
        assert!(!e.coverage.complete());
        assert!(e.coverage.describe().starts_with("INCOMPLETE"));

        let table = stratum_coverage(&g, Metric::SwitchRate);
        assert_eq!(
            table,
            vec![
                StratumCoverage {
                    label: "Y".into(),
                    instances: 2,
                    undefined_in_some_run: 1,
                    undefined_in_every_run: 1
                },
                StratumCoverage {
                    label: "Z".into(),
                    instances: 1,
                    undefined_in_some_run: 1,
                    undefined_in_every_run: 1
                },
            ]
        );
        // A rate over an incomplete population is treated as undefined for selection.
        assert!(
            arm_candidates(&g, Arm::A1)
                .iter()
                .all(|c| c.switch == f64::INFINITY)
        );
        // The critical side is unaffected and complete.
        let m = estimate(
            &g,
            &r,
            &Contrast::arm(Arm::A1),
            Metric::Miss,
            &[0, 1],
            LEVEL_95,
        )
        .unwrap();
        assert!(m.coverage.complete(), "{}", m.coverage.describe());
    }

    #[test]
    fn arm_dependent_missingness_is_flagged() {
        let mut g = grid_with_missing_values();
        // Make A1's config 1 undefined on instance 2 only: that instance becomes partially
        // defined (config 0 used, config 1 skipped) for any contrast involving A1.
        g.runs.get_mut(&Arm::A1).unwrap()[1][2].switch_rate = None;
        let r = resamples_for(&g);
        let e = estimate(
            &g,
            &r,
            &Contrast::arm(Arm::A1),
            Metric::SwitchRate,
            &[0, 1],
            LEVEL_95,
        )
        .unwrap();
        assert_eq!(e.coverage.partially_defined_instances, 1);
        assert!(!e.coverage.complete());
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
