//! P2 implementation checks on development data only (p2-protocol.md §3, §6.4, §7.1).
//! No test here generates a held-out instance or runs a P2 arm on a held-out scenario.

use attn_workspace::experiment::arms::{Arm, shared_vectors};
use attn_workspace::experiment::grid::run_one;
use attn_workspace::experiment::phases::{self, GARDEN_PATH_SEED, STABLE_SEED};
use attn_workspace::scenario::family_g::{self, DEV_SEEDS, GClass};
use attn_workspace::scenario::{garden_path, stable};

#[test]
fn baseline_sanity_checks_pass_on_the_development_block() {
    let s = phases::sanity();
    for (label, ok) in s.checks() {
        assert!(ok, "{label}: {s:?}");
    }
}

/// Every arm runs on a sample of development instances without tripping the deterministic
/// tournament check (§3.2), and every run is reproducible from its seeds (§6.4).
#[test]
fn every_arm_runs_and_reproduces_on_development_samples() {
    let vectors = shared_vectors();
    let seeds: Vec<u64> = DEV_SEEDS.take(10).collect();
    for &arm in &Arm::ALL {
        for c in [0, 1, 255, 511] {
            for &seed in &seeds {
                let a = run_one(arm, &vectors, c, seed, family_g::instance(seed).0);
                let b = run_one(arm, &vectors, c, seed, family_g::instance(seed).0);
                assert_eq!(a, b, "{arm} c={c} seed={seed}");
                assert_eq!(a.missed.is_some(), GClass::of_seed(seed).is_critical());
            }
        }
    }
}

/// Sanity of the mechanisms on the fixed development scenarios, independent of tuning:
/// habituation strong enough to overcome the 0.3 gap makes B win in garden_path, and
/// argmax-equivalent selection never switches on stable.
#[test]
fn mechanisms_behave_as_defined_on_fixed_development_scenarios() {
    let vectors = shared_vectors();
    // Find a vector whose habituation plateau k/(1−λ) exceeds the 0.3 garden_path gap; A1 must
    // then detect B. The plateau argument is in docs, not tuned here.
    let c = (0..vectors.len())
        .find(|&c| vectors[c].k / (1.0 - vectors[c].lambda) > 0.35)
        .expect("such a vector exists in V");
    let a1 = run_one(Arm::A1, &vectors, c, GARDEN_PATH_SEED, garden_path::build());
    assert_eq!(a1.missed, Some(false), "A1 at c={c} should detect B");

    let a0 = run_one(Arm::A0, &vectors, 0, GARDEN_PATH_SEED, garden_path::build());
    assert_eq!(a0.missed, Some(true), "argmax still misses (P1)");

    let a0_stable = run_one(Arm::A0, &vectors, 0, STABLE_SEED, stable::build());
    assert_eq!(a0_stable.switch_count, 0);
}
