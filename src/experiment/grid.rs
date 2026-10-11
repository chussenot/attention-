//! Running arms over scenario sets and storing per-run outcomes.
//!
//! A grid holds, for one set of instances, the outcome of every arm at every configuration on
//! every instance (`docs/experiments/p2-protocol.md` §6.2). Parameter-free arms have a single
//! configuration, reused for every *c* by the estimands.

use std::collections::BTreeMap;

use super::arms::{Arm, ParamVector};
use crate::metrics::{self, GroundTruth, MetricsConfig};
use crate::scenario::Scenario;
use crate::workspace::TickRecord;

/// What one run contributes to the analysis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RunOutcome {
    /// Critical miss, on critical instances only (§4).
    pub missed: Option<bool>,
    /// Detection latency, when detected (secondary).
    pub latency: Option<u64>,
    /// Per-run switch rate per 100 eligible transitions; `None` when there are none (§4).
    pub switch_rate: Option<f64>,
    pub switch_count: u64,
    pub eligible: u64,
    /// Longest dominance streak of any source (secondary).
    pub longest_streak: u64,
}

impl RunOutcome {
    pub fn from_records(records: &[TickRecord], truth: &GroundTruth) -> Self {
        let m = metrics::evaluate(records, truth, &MetricsConfig::default());
        let critical = !truth.streams.is_empty();
        RunOutcome {
            missed: critical.then(|| m.any_missed()),
            latency: m.streams.first().and_then(|s| s.switch_latency),
            switch_rate: metrics::run_switch_rate(records),
            switch_count: m.switch_count,
            eligible: metrics::eligible_transitions(records),
            longest_streak: m.longest_streak().map_or(0, |(_, s)| s),
        }
    }
}

/// A stratum of a set: G's classes, or Family B's labels (§5.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stratum {
    pub label: String,
    /// Critical strata feed miss-rate estimands; the others feed switch-rate estimands.
    pub critical: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Instance {
    /// Generator seed, also the instance seed for run seeds (§6.4).
    pub seed: u64,
    pub stratum: usize,
}

/// A set of instances to run: how to build each one, and how they are stratified.
pub struct SetSpec {
    pub name: String,
    /// Bootstrap set id (§5.2): 0 = G development, 1 = G held-out block 1, 2 = Family B.
    pub set_id: u64,
    pub strata: Vec<Stratum>,
    pub instances: Vec<Instance>,
    pub build: Box<dyn Fn(u64) -> Scenario>,
}

/// Outcomes for one set: `runs[arm][config][instance]`.
pub struct Grid {
    pub name: String,
    pub set_id: u64,
    pub strata: Vec<Stratum>,
    pub instances: Vec<Instance>,
    pub runs: BTreeMap<Arm, Vec<Vec<RunOutcome>>>,
}

impl Grid {
    pub fn members(&self) -> Vec<Vec<usize>> {
        let mut members = vec![Vec::new(); self.strata.len()];
        let mut order: Vec<usize> = (0..self.instances.len()).collect();
        order.sort_by_key(|&i| self.instances[i].seed);
        for i in order {
            members[self.instances[i].stratum].push(i);
        }
        members
    }

    /// Number of configurations stored for `arm` (1 for parameter-free arms).
    pub fn n_configs(&self, arm: Arm) -> usize {
        self.runs[&arm].len()
    }

    /// Outcome of `arm` at configuration `c` on instance `i`; parameter-free arms ignore `c`.
    pub fn outcome(&self, arm: Arm, c: usize, i: usize) -> &RunOutcome {
        let per_config = &self.runs[&arm];
        let c = if per_config.len() == 1 { 0 } else { c };
        &per_config[c][i]
    }
}

/// Runs one arm at configuration `c` on a freshly built scenario.
pub fn run_one(
    arm: Arm,
    vectors: &[ParamVector],
    c: usize,
    instance_seed: u64,
    scenario: Scenario,
) -> RunOutcome {
    let arbiter = arm.build(vectors, c, instance_seed);
    let run = scenario.run(arbiter);
    RunOutcome::from_records(&run.records, &run.truth)
}

/// Runs `arms` on every instance of `set`: parameterised arms at every vector, the others once.
/// `progress` is called after each instance with (done, total).
pub fn run_grid(
    set: &SetSpec,
    arms: &[Arm],
    vectors: &[ParamVector],
    mut progress: impl FnMut(usize, usize),
) -> Grid {
    let mut runs: BTreeMap<Arm, Vec<Vec<RunOutcome>>> = BTreeMap::new();
    for &arm in arms {
        let n = if arm.is_parameterised() {
            vectors.len()
        } else {
            1
        };
        runs.insert(arm, vec![Vec::with_capacity(set.instances.len()); n]);
    }
    for (done, inst) in set.instances.iter().enumerate() {
        for &arm in arms {
            let per_config = runs.get_mut(&arm).expect("arm inserted above");
            for (c, outcomes) in per_config.iter_mut().enumerate() {
                outcomes.push(run_one(arm, vectors, c, inst.seed, (set.build)(inst.seed)));
            }
        }
        progress(done + 1, set.instances.len());
    }
    Grid {
        name: set.name.clone(),
        set_id: set.set_id,
        strata: set.strata.clone(),
        instances: set.instances.clone(),
        runs,
    }
}
