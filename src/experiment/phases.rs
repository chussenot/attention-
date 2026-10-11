//! The P2 runner phases (`docs/experiments/p2-protocol.md` §8): baseline sanity checks, the
//! development phase and the (gated) held-out phase.

use std::fmt::Write as _;

use super::analysis::{
    self, Branch, Contrast, Coverage, DESCRIPTIVE_LEVEL, Estimate, MECHANISM_LEVEL, Mechanism,
    MechanismVerdict, Metric, StratumCoverage,
};
use super::arms::{Arm, ParamVector, VECTOR_SEED, shared_vectors};
use super::grid::{Grid, Instance, RunOutcome, SetSpec, Stratum, run_grid, run_one};
use crate::scenario::family_g::{self, DEV_SEEDS, GClass};
use crate::scenario::{Scenario, garden_path, stable};

/// Instance seeds of the fixed scenarios, for arm randomness (§7.1, §7.2).
pub const GARDEN_PATH_SEED: u64 = 1_000_001;
pub const STABLE_SEED: u64 = 1_000_002;
pub const DISTRACTOR_FLOOD_SEED: u64 = 1_000_003;

fn g_strata() -> Vec<Stratum> {
    GClass::ALL
        .iter()
        .map(|c| Stratum {
            label: c.label().to_string(),
            critical: c.is_critical(),
        })
        .collect()
}

/// The G development block (set id 0).
pub fn g_dev_set() -> SetSpec {
    SetSpec {
        name: "G development block (seeds 0–199)".to_string(),
        set_id: 0,
        strata: g_strata(),
        instances: DEV_SEEDS
            .map(|seed| Instance {
                seed,
                stratum: GClass::ALL
                    .iter()
                    .position(|&c| c == GClass::of_seed(seed))
                    .unwrap(),
            })
            .collect(),
        build: Box::new(|seed| family_g::instance(seed).0),
    }
}

// ---------------------------------------------------------------------------------------
// Baseline sanity checks (§7.1)
// ---------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sanity {
    /// A0's miss rate on C1–C3 (must be 1.0).
    pub a0_miss_c1_c3: f64,
    /// RR's switch rate on C4 (must be ≥ 90 per 100).
    pub rr_switch_c4: f64,
    /// RR's miss rate on C3 (must be > 0).
    pub rr_miss_c3: f64,
}

impl Sanity {
    pub fn checks(&self) -> [(&'static str, bool); 3] {
        [
            ("A0 miss rate on C1–C3 is 100%", self.a0_miss_c1_c3 == 1.0),
            (
                "RR switch rate on C4 is at least 90 per 100",
                self.rr_switch_c4 >= 90.0,
            ),
            ("RR miss rate on C3 is above 0", self.rr_miss_c3 > 0.0),
        ]
    }

    pub fn passed(&self) -> bool {
        self.checks().iter().all(|(_, ok)| *ok)
    }
}

fn class_mean(grid: &Grid, arm: Arm, classes: &[GClass], metric: Metric) -> f64 {
    let values = analysis::per_instance(grid, &Contrast::arm(arm), metric, &[0]);
    let picked: Vec<f64> = grid
        .instances
        .iter()
        .enumerate()
        .filter(|(_, inst)| classes.contains(&GClass::of_seed(inst.seed)))
        .filter_map(|(i, _)| values[i])
        .collect();
    picked.iter().sum::<f64>() / picked.len() as f64
}

/// Runs A0 and RR on the G development block and evaluates the three sanity checks.
pub fn sanity() -> Sanity {
    let grid = run_grid(&g_dev_set(), &[Arm::A0, Arm::RoundRobin], &[], |_, _| {});
    sanity_from_grid(&grid)
}

fn sanity_from_grid(grid: &Grid) -> Sanity {
    use GClass::*;
    Sanity {
        a0_miss_c1_c3: class_mean(
            grid,
            Arm::A0,
            &[C1Onset, C2SilentOnset, C3Transient],
            Metric::Miss,
        ),
        rr_switch_c4: class_mean(grid, Arm::RoundRobin, &[C4Stable], Metric::SwitchRate),
        rr_miss_c3: class_mean(grid, Arm::RoundRobin, &[C3Transient], Metric::Miss),
    }
}

// ---------------------------------------------------------------------------------------
// Development phase (§8 step 4)
// ---------------------------------------------------------------------------------------

pub struct MechanismResult {
    pub mechanism: Mechanism,
    pub miss: Estimate,
    pub switch: Option<Estimate>,
    pub mc_error: Option<f64>,
    pub verdict: MechanismVerdict,
    /// (label, miss estimate, switch estimate), 95%.
    pub simple: Vec<(&'static str, Option<Estimate>, Option<Estimate>)>,
}

pub struct ArmPolicy {
    pub arm: Arm,
    /// `None` for parameter-free arms.
    pub c_star: Option<usize>,
    pub branch: Option<Branch>,
    pub passing: Option<(u64, (f64, f64), bool)>,
    pub miss: Option<Estimate>,
    pub switch: Option<Estimate>,
    pub miss_vs_a0: Option<Estimate>,
    pub switch_vs_a0: Option<Estimate>,
}

pub struct CaseStudy {
    pub name: &'static str,
    /// Per arm at its frozen configuration.
    pub frozen: Vec<(Arm, RunOutcome)>,
    /// Per parameterised arm: the fraction of the configurations (512 in a protocol run) that detect the critical
    /// stream (garden_path) or the mean switch rate (stable).
    pub across_v: Vec<(Arm, f64)>,
}

pub struct DevResults {
    /// Per-stratum missingness for each metric over every run (Amendment 2).
    pub coverage_tables: Vec<(Metric, Vec<StratumCoverage>)>,
    pub smoke: Option<Smoke>,
    pub vectors: Vec<ParamVector>,
    pub sanity: Sanity,
    pub mechanisms: Vec<MechanismResult>,
    pub floor_diagnostic: f64,
    pub h4_by_tercile: Vec<Option<Estimate>>,
    pub policies: Vec<ArmPolicy>,
    pub x_star: Arm,
    pub x_star_branch: Branch,
    pub case_studies: Vec<CaseStudy>,
}

impl DevResults {
    pub fn c_star(&self, arm: Arm) -> usize {
        self.policies
            .iter()
            .find(|p| p.arm == arm)
            .and_then(|p| p.c_star)
            .unwrap_or(0)
    }
}

fn fixed_case_study(
    name: &'static str,
    seed: u64,
    build: fn() -> Scenario,
    vectors: &[ParamVector],
    c_star: &dyn Fn(Arm) -> usize,
    detect: bool,
) -> CaseStudy {
    let frozen = Arm::ALL
        .iter()
        .map(|&arm| (arm, run_one(arm, vectors, c_star(arm), seed, build())))
        .collect();
    let across_v = Arm::PARAMETERISED
        .iter()
        .map(|&arm| {
            let outcomes: Vec<RunOutcome> = (0..vectors.len())
                .map(|c| run_one(arm, vectors, c, seed, build()))
                .collect();
            let value = if detect {
                outcomes.iter().filter(|o| o.missed == Some(false)).count() as f64
                    / outcomes.len() as f64
            } else {
                let rates: Vec<f64> = outcomes.iter().filter_map(|o| o.switch_rate).collect();
                rates.iter().sum::<f64>() / rates.len() as f64
            };
            (arm, value)
        })
        .collect();
    CaseStudy {
        name,
        frozen,
        across_v,
    }
}

/// Restricts the development phase to a small subset, to exercise the pipeline end to end.
/// A smoke run is never a protocol run: its output is labelled as such and must not be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Smoke {
    pub instances_per_class: usize,
    pub vectors: usize,
}

/// Runs the development phase: the full development grid, the sanity checks, every
/// development estimand, the selection rule and the fixed development case studies. With
/// `smoke`, runs on a subset instead and does not enforce the sanity checks.
///
/// # Panics
///
/// In a protocol run, if a baseline sanity check fails: the generator must be fixed first
/// (§7.1).
pub fn dev_phase(smoke: Option<Smoke>, mut log: impl FnMut(&str)) -> DevResults {
    let mut vectors = shared_vectors();
    let mut set = g_dev_set();
    if let Some(sm) = smoke {
        vectors.truncate(sm.vectors);
        let mut kept = vec![0usize; set.strata.len()];
        set.instances.retain(|inst| {
            kept[inst.stratum] += 1;
            kept[inst.stratum] <= sm.instances_per_class
        });
    }

    // §7.1: the sanity checks run before the sweep; a protocol run stops here if one fails.
    // A smoke run uses a subset where they need not hold, so it only reports them.
    if smoke.is_none() {
        log("running baseline sanity checks");
        let s = sanity();
        assert!(
            s.passed(),
            "baseline sanity check failed: {s:?}. Fix the generator (§7.1)."
        );
    }

    log("running the development grid");
    let grid = run_grid(&set, &Arm::ALL, &vectors, |done, total| {
        if done % 20 == 0 || done == total {
            log(&format!("  {done}/{total} instances"));
        }
    });
    // Recomputed from the grid for the report; in a protocol run it equals the check above.
    let sanity = sanity_from_grid(&grid);

    log("computing estimands");
    let resamples = analysis::resamples_for(&grid);
    let all = analysis::all_configs(vectors.len());
    let est = |contrast: &Contrast, metric, configs: &[usize], level| {
        analysis::estimate(&grid, &resamples, contrast, metric, configs, level)
    };

    let mechanisms = Mechanism::ALL
        .iter()
        .map(|&m| {
            let contrast = m.contrast();
            let miss =
                est(&contrast, Metric::Miss, &all, MECHANISM_LEVEL).expect("critical strata exist");
            MechanismResult {
                mechanism: m,
                verdict: analysis::mechanism_verdict(m, &miss),
                miss,
                switch: est(&contrast, Metric::SwitchRate, &all, DESCRIPTIVE_LEVEL),
                mc_error: analysis::monte_carlo_error(&grid, &contrast, Metric::Miss, &all),
                simple: m
                    .simple_effects()
                    .into_iter()
                    .map(|(label, c)| {
                        (
                            label,
                            est(&c, Metric::Miss, &all, DESCRIPTIVE_LEVEL),
                            est(&c, Metric::SwitchRate, &all, DESCRIPTIVE_LEVEL),
                        )
                    })
                    .collect(),
            }
        })
        .collect();

    let h4_by_tercile = (0..3)
        .map(|t| {
            let configs = analysis::tau_tercile(&vectors, t);
            est(
                &Mechanism::H4Stochastic.contrast(),
                Metric::Miss,
                &configs,
                DESCRIPTIVE_LEVEL,
            )
        })
        .collect();

    log("selecting best-tuned configurations");
    let mut policies = Vec::new();
    let mut best = Vec::new();
    for &arm in &Arm::ALL {
        let (c_star, branch, passing) = if arm.is_parameterised() {
            let candidates = analysis::arm_candidates(&grid, arm);
            let n_pass = candidates.iter().filter(|c| c.passes()).count() as u64;
            let (met, ci) = analysis::h5_met(n_pass, candidates.len() as u64);
            let (chosen, branch) = analysis::select(&candidates);
            best.push((arm, chosen, branch));
            (
                Some(chosen.index as usize),
                Some(branch),
                Some((n_pass, ci, met)),
            )
        } else {
            (None, None, None)
        };
        let configs = [c_star.unwrap_or(0)];
        let vs_a0 = |metric| {
            if arm == Arm::A0 {
                None
            } else {
                // A0 is parameter-free, so its value is reused at c*.
                est(
                    &Contrast::diff(arm, Arm::A0),
                    metric,
                    &configs,
                    DESCRIPTIVE_LEVEL,
                )
            }
        };
        policies.push(ArmPolicy {
            arm,
            c_star,
            branch,
            passing,
            miss: est(
                &Contrast::arm(arm),
                Metric::Miss,
                &configs,
                DESCRIPTIVE_LEVEL,
            ),
            switch: est(
                &Contrast::arm(arm),
                Metric::SwitchRate,
                &configs,
                DESCRIPTIVE_LEVEL,
            ),
            miss_vs_a0: vs_a0(Metric::Miss),
            switch_vs_a0: vs_a0(Metric::SwitchRate),
        });
    }

    // X*: the selection rule over the seven best-tuned configurations, index = arm_id (§6.5).
    let x_candidates: Vec<analysis::Candidate> = best
        .iter()
        .map(|(arm, c, _)| analysis::Candidate {
            index: arm.id(),
            ..*c
        })
        .collect();
    let (x, x_star_branch) = analysis::select(&x_candidates);
    let x_star = *Arm::ALL
        .iter()
        .find(|a| a.id() == x.index)
        .expect("valid arm id");

    log("running development case studies");
    let c_star_of = |arm: Arm| {
        policies
            .iter()
            .find(|p| p.arm == arm)
            .and_then(|p| p.c_star)
            .unwrap_or(0)
    };
    let case_studies = vec![
        fixed_case_study(
            "garden_path",
            GARDEN_PATH_SEED,
            garden_path::build,
            &vectors,
            &c_star_of,
            true,
        ),
        fixed_case_study(
            "stable",
            STABLE_SEED,
            stable::build,
            &vectors,
            &c_star_of,
            false,
        ),
    ];

    DevResults {
        smoke,
        floor_diagnostic: analysis::floor_diagnostic(&grid, &all),
        coverage_tables: vec![
            (
                Metric::Miss,
                analysis::stratum_coverage(&grid, Metric::Miss),
            ),
            (
                Metric::SwitchRate,
                analysis::stratum_coverage(&grid, Metric::SwitchRate),
            ),
        ],
        vectors,
        sanity,
        mechanisms,
        h4_by_tercile,
        policies,
        x_star,
        x_star_branch,
        case_studies,
    }
}

// ---------------------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------------------

/// Marks an estimate whose coverage is not trivially complete: `†` when instances or cells
/// were excluded but the population is intact, `⚠` when the population is incomplete
/// (Amendment 2). Details are listed in the report's coverage section.
fn coverage_mark(c: &Coverage) -> &'static str {
    if !c.complete() {
        " ⚠"
    } else if c.excluded_instances > 0 || c.skipped_cells > 0 {
        " †"
    } else {
        ""
    }
}

fn fmt_est(e: &Option<Estimate>) -> String {
    match e {
        Some(e) => format!(
            "{:+.4} [{:+.4}, {:+.4}]{}",
            e.point,
            e.lo,
            e.hi,
            coverage_mark(&e.coverage)
        ),
        None => "– (no defined value)".to_string(),
    }
}

fn fmt_rate(e: &Option<Estimate>) -> String {
    match e {
        Some(e) => format!(
            "{:.4} [{:.4}, {:.4}]{}",
            e.point,
            e.lo,
            e.hi,
            coverage_mark(&e.coverage)
        ),
        None => "– (no defined value)".to_string(),
    }
}

/// Every reported estimate with a label, for the coverage section.
fn all_estimates(r: &DevResults) -> Vec<(String, Option<Estimate>)> {
    let mut out = Vec::new();
    for m in &r.mechanisms {
        let h = m.mechanism.label();
        out.push((format!("{h} miss-rate estimand"), Some(m.miss)));
        out.push((format!("{h} switch-rate contrast"), m.switch));
        for (label, miss, switch) in &m.simple {
            out.push((format!("{h} {label}, miss"), *miss));
            out.push((format!("{h} {label}, switch"), *switch));
        }
    }
    for (t, e) in r.h4_by_tercile.iter().enumerate() {
        out.push((format!("H4 τ tercile {}", t + 1), *e));
    }
    for p in &r.policies {
        out.push((format!("{} miss rate at c*", p.arm), p.miss));
        out.push((format!("{} switch rate at c*", p.arm), p.switch));
        out.push((format!("{} miss − A0", p.arm), p.miss_vs_a0));
        out.push((format!("{} switch − A0", p.arm), p.switch_vs_a0));
    }
    out
}

fn render_coverage(s: &mut String, r: &DevResults) {
    let _ = writeln!(
        s,
        "## Coverage and exclusions (Amendment 1 item 8, Amendment 2)
"
    );
    let _ = writeln!(
        s,
        "Undefined values are excluded, never imputed. Rows below show, per stratum, how many \
         instances had an undefined value in at least one run, or in every run.\n"
    );
    for (metric, table) in &r.coverage_tables {
        let name = match metric {
            Metric::Miss => "Critical miss (critical strata)",
            Metric::SwitchRate => "Switch rate (stable strata)",
        };
        let _ = writeln!(s, "**{name}**\n");
        let _ = writeln!(
            s,
            "| Stratum | Instances | Undefined in some run | Undefined in every run |"
        );
        let _ = writeln!(s, "|---|---:|---:|---:|");
        for row in table {
            let _ = writeln!(
                s,
                "| {} | {} | {} | {} |",
                row.label, row.instances, row.undefined_in_some_run, row.undefined_in_every_run
            );
        }
        let _ = writeln!(s);
    }
    let flagged: Vec<(String, String)> = all_estimates(r)
        .into_iter()
        .filter_map(|(label, e)| match e {
            None => Some((label, "no defined value".to_string())),
            Some(e) if e.coverage.describe() != "complete" => Some((label, e.coverage.describe())),
            Some(_) => None,
        })
        .collect();
    if flagged.is_empty() {
        let _ = writeln!(
            s,
            "Every reported estimate covers its full declared population: no excluded instance, \
             no skipped cell, no stratum without data, no affected bootstrap replicate.\n"
        );
    } else {
        let _ = writeln!(
            s,
            "Estimates with exclusions (†) or an incomplete population (⚠, no verdict):\n"
        );
        for (label, desc) in flagged {
            let _ = writeln!(s, "- {label}: {desc}");
        }
        let _ = writeln!(s);
    }
}

fn verdict_label(m: Mechanism, v: MechanismVerdict) -> &'static str {
    match (m, v) {
        (Mechanism::H3Interaction, MechanismVerdict::Supported) => "supported (complementary)",
        (_, MechanismVerdict::Supported) => "supported",
        (_, MechanismVerdict::Redundant) => "redundant",
        (Mechanism::H3Interaction, MechanismVerdict::NotSupported) => "inconclusive",
        (_, MechanismVerdict::NotSupported) => "not supported",
        (_, MechanismVerdict::Undecidable) => "undecidable (incomplete population, Amendment 2)",
    }
}

fn branch_label(b: Branch) -> &'static str {
    match b {
        Branch::Passing => "passes (rule 1)",
        Branch::SwitchTargetOnly => "does not pass (rule 2)",
        Branch::Neither => "does not pass (rule 3)",
    }
}

/// The development results report (Markdown).
pub fn render_dev_report(r: &DevResults) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "# P2 development results\n");
    if let Some(sm) = r.smoke {
        let _ = writeln!(
            s,
            "> **SMOKE RUN, NOT A PROTOCOL RUN.** {} instances per class, {} vectors. Do not use these numbers.\n",
            sm.instances_per_class, sm.vectors
        );
    }
    let _ = writeln!(
        s,
        "Generated by `cargo run --release --example p2 -- dev`, following `p2-protocol.md`. \
         Development data only: G development block (seeds 0–199), `garden_path` and `stable`.\n"
    );

    let _ = writeln!(s, "## Baseline sanity checks (§7.1)\n");
    for (label, ok) in r.sanity.checks() {
        let _ = writeln!(s, "- {} {label}", if ok { "✅" } else { "❌" });
    }
    let _ = writeln!(
        s,
        "\nValues: A0 miss on C1–C3 = {:.4}; RR switch rate on C4 = {:.2} per 100; RR miss on C3 = {:.4}.\n",
        r.sanity.a0_miss_c1_c3, r.sanity.rr_switch_c4, r.sanity.rr_miss_c3
    );

    render_coverage(&mut s, r);

    let _ = writeln!(s, "## Mechanism hypotheses H1–H4 (§2.1)\n");
    let _ = writeln!(
        s,
        "Primary estimands: miss-rate contrasts averaged over the shared vectors (512 in a protocol run), 98.75% intervals. \
         Stability contrasts on the same arms: 95%, descriptive.\n"
    );
    let _ = writeln!(
        s,
        "| Hypothesis | Miss-rate estimand [98.75%] | MC error | Switch-rate contrast [95%] | Verdict |"
    );
    let _ = writeln!(s, "|---|---|---:|---|---|");
    for m in &r.mechanisms {
        let _ = writeln!(
            s,
            "| {} | {} | {} | {} | {} |",
            m.mechanism.label(),
            fmt_est(&Some(m.miss)),
            m.mc_error.map_or("–".into(), |e| format!("{e:.5}")),
            fmt_est(&m.switch),
            verdict_label(m.mechanism, m.verdict)
        );
    }
    let _ = writeln!(
        s,
        "\nH3 floor diagnostic: A1 and A2 both detect in {:.2}% of (configuration, critical instance) cells.\n",
        100.0 * r.floor_diagnostic
    );

    let _ = writeln!(s, "### Simple effects (95%, descriptive)\n");
    let _ = writeln!(s, "| Hypothesis | Contrast | Miss rate | Switch rate |");
    let _ = writeln!(s, "|---|---|---|---|");
    for m in &r.mechanisms {
        for (label, miss, switch) in &m.simple {
            let _ = writeln!(
                s,
                "| {} | {label} | {} | {} |",
                m.mechanism.label(),
                fmt_est(miss),
                fmt_est(switch)
            );
        }
    }
    let _ = writeln!(
        s,
        "\n### H4 by τ tercile (equal-width thirds of [0.01, 0.50]; 95%, descriptive)\n"
    );
    for (t, e) in r.h4_by_tercile.iter().enumerate() {
        let _ = writeln!(s, "- tercile {}: {}", t + 1, fmt_est(e));
    }

    let _ = writeln!(
        s,
        "\n## Robustness H5 and best-tuned configurations (§2.2, §6.3, §6.5)\n"
    );
    let _ = writeln!(
        s,
        "Rates at the best-tuned configuration, class-weighted, with 95% intervals. \
         This is the independently tuned policy comparison: it is not evidence for any mechanism hypothesis.\n"
    );
    let _ = writeln!(
        s,
        "| Arm | c* | Selection | Pass fraction [Wilson 95%] | H5 | Miss rate | Switch rate | Miss − A0 | Switch − A0 |"
    );
    let _ = writeln!(s, "|---|---:|---|---|---|---|---|---|---|");
    for p in &r.policies {
        let (pass, h5) = match p.passing {
            Some((n, (lo, hi), met)) => (
                format!(
                    "{n}/{} = {:.3} [{lo:.3}, {hi:.3}]",
                    r.vectors.len(),
                    n as f64 / r.vectors.len() as f64
                ),
                if met { "met" } else { "not met" }.to_string(),
            ),
            None => ("–".into(), "–".into()),
        };
        let _ = writeln!(
            s,
            "| {} | {} | {} | {pass} | {h5} | {} | {} | {} | {} |",
            p.arm,
            p.c_star.map_or("–".into(), |c| c.to_string()),
            p.branch.map_or("–", branch_label),
            fmt_rate(&p.miss),
            fmt_rate(&p.switch),
            fmt_est(&p.miss_vs_a0),
            fmt_est(&p.switch_vs_a0),
        );
    }
    let _ = writeln!(
        s,
        "\n**Selected policy X\\* = {}** ({}).\n",
        r.x_star,
        branch_label(r.x_star_branch)
    );

    let _ = writeln!(s, "## Development case studies (non-inferential, §7.1)\n");
    for cs in &r.case_studies {
        let _ = writeln!(s, "### `{}`\n", cs.name);
        let _ = writeln!(
            s,
            "| Arm (at c*) | Detected | Latency | Switches | Longest streak |"
        );
        let _ = writeln!(s, "|---|---|---|---:|---:|");
        for (arm, o) in &cs.frozen {
            let _ = writeln!(
                s,
                "| {arm} | {} | {} | {} | {} |",
                o.missed.map_or("–", |m| if m { "no" } else { "yes" }),
                o.latency.map_or("–".into(), |l| l.to_string()),
                o.switch_count,
                o.longest_streak
            );
        }
        let what = if cs.name == "garden_path" {
            "fraction of the configurations that detect"
        } else {
            "mean switch rate per 100 over the configurations"
        };
        let _ = writeln!(s, "\nAcross V ({what}):");
        for (arm, v) in &cs.across_v {
            let _ = writeln!(s, "- {arm}: {v:.4}");
        }
        let _ = writeln!(s);
    }
    s
}

/// The freeze file (§6.6): plain `key=value` lines.
pub fn render_freeze(r: &DevResults) -> String {
    let mut s = String::new();
    let v = |p: &ParamVector| {
        format!(
            "{:?},{:?},{:?},{:?},{:?}",
            p.k, p.lambda, p.eta, p.rho, p.tau
        )
    };
    let _ = writeln!(
        s,
        "# P2 freeze file (p2-protocol.md §6.6). Generated by `cargo run --release --example p2 -- dev`."
    );
    if r.smoke.is_some() {
        let _ = writeln!(s, "# SMOKE RUN, NOT A PROTOCOL RUN. Do not commit or use.");
    }
    let _ = writeln!(s, "vectors.seed={VECTOR_SEED:#x}");
    let _ = writeln!(s, "vectors.count={}", r.vectors.len());
    let _ = writeln!(s, "vectors.first={}", v(&r.vectors[0]));
    let _ = writeln!(
        s,
        "vectors.last={}",
        v(r.vectors.last().expect("non-empty"))
    );
    for p in &r.policies {
        if let Some(c) = p.c_star {
            let _ = writeln!(s, "c_star.{}={c}", p.arm);
        }
    }
    let _ = writeln!(s, "x_star={}", r.x_star);
    for m in &r.mechanisms {
        let _ = writeln!(s, "dev_verdict.{}={:?}", m.mechanism.label(), m.verdict);
    }
    s
}

// ---------------------------------------------------------------------------------------
// Held-out phase (§8 step 7)
// ---------------------------------------------------------------------------------------

/// The held-out phase may run only after the freeze commit and after Family B has been
/// integrated and validated (§8 steps 5–6). Family B is not integrated yet, so this refuses
/// to run and generates no held-out instance.
pub fn heldout_phase() -> Result<(), String> {
    Err(
        "held-out phase refused: Family B is not integrated (p2-protocol.md §8, step 6). \
         It must not run before the freeze commit and the Family B integration."
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heldout_phase_is_gated() {
        assert!(heldout_phase().is_err());
    }

    #[test]
    fn dev_set_has_forty_instances_per_class() {
        let set = g_dev_set();
        assert_eq!(set.instances.len(), 200);
        for s in 0..5 {
            assert_eq!(set.instances.iter().filter(|i| i.stratum == s).count(), 40);
        }
    }
}
