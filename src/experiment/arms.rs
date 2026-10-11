//! The P2 arm matrix (`docs/experiments/p2-protocol.md` §3.3), the shared parameter vectors
//! (§6.2) and per-run seeds (§6.4).

use std::fmt;

use crate::arbiter::{
    Arbiter, ArgmaxArbiter, Habituation, Novelty, RoundRobinArbiter, Rule, ScoreArbiter,
    ScoreConfig, UniformRandomArbiter,
};
use crate::rng::SplitMix64;

/// Number of shared parameter vectors.
pub const N_VECTORS: usize = 512;

/// Seed of the shared parameter vectors (§6.2).
pub const VECTOR_SEED: u64 = 0x5EED_0002;

/// One shared parameter vector `v_c = (k, λ, η, ρ, τ)`. Each arm uses only its own components.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParamVector {
    pub k: f64,
    pub lambda: f64,
    pub eta: f64,
    pub rho: f64,
    pub tau: f64,
}

/// Declared ranges (§6.1), in draw order.
pub const K_RANGE: (f64, f64) = (0.005, 0.30);
pub const LAMBDA_RANGE: (f64, f64) = (0.50, 0.99);
pub const ETA_RANGE: (f64, f64) = (0.0, 2.0);
pub const RHO_RANGE: (f64, f64) = (0.0, 0.95);
pub const TAU_RANGE: (f64, f64) = (0.01, 0.50);

fn map(u: f64, (lo, hi): (f64, f64)) -> f64 {
    lo + (hi - lo) * u
}

/// The shared set V: `N_VECTORS` vectors from `SplitMix64(VECTOR_SEED)`, five draws each in the
/// order k, λ, η, ρ, τ, every component drawn for every vector (§6.2).
pub fn shared_vectors() -> Vec<ParamVector> {
    let mut rng = SplitMix64::new(VECTOR_SEED);
    (0..N_VECTORS)
        .map(|_| ParamVector {
            k: map(rng.next_f64(), K_RANGE),
            lambda: map(rng.next_f64(), LAMBDA_RANGE),
            eta: map(rng.next_f64(), ETA_RANGE),
            rho: map(rng.next_f64(), RHO_RANGE),
            tau: map(rng.next_f64(), TAU_RANGE),
        })
        .collect()
}

/// `mix(x) = SplitMix64::new(x).next_u64()` (§6.4).
pub fn mix(x: u64) -> u64 {
    SplitMix64::new(x).next_u64()
}

/// `run_seed = mix(mix(mix(instance_seed) ^ arm_id) ^ c)` (§6.4). `c` is the configuration
/// index for S0–S3 and 0 for RND.
pub fn run_seed(instance_seed: u64, arm: Arm, c: u64) -> u64 {
    mix(mix(mix(instance_seed) ^ arm.id()) ^ c)
}

/// The ten arms, in the protocol's row order; `id()` is the row index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Arm {
    A0,
    A1,
    A2,
    A3,
    S0,
    S1,
    S2,
    S3,
    RoundRobin,
    UniformRandom,
}

impl Arm {
    pub const ALL: [Arm; 10] = [
        Arm::A0,
        Arm::A1,
        Arm::A2,
        Arm::A3,
        Arm::S0,
        Arm::S1,
        Arm::S2,
        Arm::S3,
        Arm::RoundRobin,
        Arm::UniformRandom,
    ];

    /// Arms with parameters: A1–A3 and S0–S3.
    pub const PARAMETERISED: [Arm; 7] = [
        Arm::A1,
        Arm::A2,
        Arm::A3,
        Arm::S0,
        Arm::S1,
        Arm::S2,
        Arm::S3,
    ];

    pub fn id(self) -> u64 {
        self as u64
    }

    pub fn label(self) -> &'static str {
        match self {
            Arm::A0 => "A0",
            Arm::A1 => "A1",
            Arm::A2 => "A2",
            Arm::A3 => "A3",
            Arm::S0 => "S0",
            Arm::S1 => "S1",
            Arm::S2 => "S2",
            Arm::S3 => "S3",
            Arm::RoundRobin => "RR",
            Arm::UniformRandom => "RND",
        }
    }

    pub fn is_parameterised(self) -> bool {
        Arm::PARAMETERISED.contains(&self)
    }

    fn habituation(self) -> bool {
        matches!(self, Arm::A1 | Arm::A3 | Arm::S1 | Arm::S3)
    }

    fn novelty(self) -> bool {
        matches!(self, Arm::A2 | Arm::A3 | Arm::S2 | Arm::S3)
    }

    fn stochastic(self) -> bool {
        matches!(self, Arm::S0 | Arm::S1 | Arm::S2 | Arm::S3)
    }

    /// Builds the arbiter for this arm at configuration `c` (ignored by parameter-free arms)
    /// on an instance with `instance_seed`.
    pub fn build(self, vectors: &[ParamVector], c: usize, instance_seed: u64) -> Box<dyn Arbiter> {
        match self {
            Arm::A0 => Box::new(ArgmaxArbiter::new()),
            Arm::RoundRobin => Box::new(RoundRobinArbiter::new()),
            Arm::UniformRandom => {
                Box::new(UniformRandomArbiter::new(run_seed(instance_seed, self, 0)))
            }
            _ => {
                let v = vectors[c];
                let rule = if self.stochastic() {
                    Rule::Stochastic {
                        tau: v.tau,
                        seed: run_seed(instance_seed, self, c as u64),
                    }
                } else {
                    Rule::Deterministic
                };
                Box::new(ScoreArbiter::new(ScoreConfig {
                    habituation: self.habituation().then_some(Habituation {
                        k: v.k,
                        lambda: v.lambda,
                    }),
                    novelty: self.novelty().then_some(Novelty {
                        eta: v.eta,
                        rho: v.rho,
                    }),
                    rule,
                }))
            }
        }
    }
}

impl fmt::Display for Arm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn arm_ids_follow_the_protocol_rows() {
        let ids: Vec<u64> = Arm::ALL.iter().map(|a| a.id()).collect();
        assert_eq!(ids, (0..10).collect::<Vec<_>>());
        assert_eq!(Arm::RoundRobin.id(), 8);
        assert_eq!(Arm::UniformRandom.id(), 9);
    }

    #[test]
    fn shared_vectors_are_reproducible_and_in_range() {
        let v = shared_vectors();
        assert_eq!(v.len(), 512);
        assert_eq!(v, shared_vectors());
        for p in &v {
            assert!((K_RANGE.0..=K_RANGE.1).contains(&p.k));
            assert!((LAMBDA_RANGE.0..=LAMBDA_RANGE.1).contains(&p.lambda));
            assert!((ETA_RANGE.0..=ETA_RANGE.1).contains(&p.eta));
            assert!((RHO_RANGE.0..=RHO_RANGE.1).contains(&p.rho));
            assert!((TAU_RANGE.0..=TAU_RANGE.1).contains(&p.tau));
        }
        // First vector, from the first five draws of SplitMix64(0x5EED_0002).
        let mut rng = SplitMix64::new(VECTOR_SEED);
        let u: Vec<f64> = (0..5).map(|_| rng.next_f64()).collect();
        assert_eq!(v[0].k, K_RANGE.0 + (K_RANGE.1 - K_RANGE.0) * u[0]);
        assert_eq!(v[0].tau, TAU_RANGE.0 + (TAU_RANGE.1 - TAU_RANGE.0) * u[4]);
    }

    #[test]
    fn run_seeds_differ_across_arms_configs_and_instances() {
        let mut seen = HashSet::new();
        for instance in [0, 1, 1_000_001] {
            for arm in [Arm::S0, Arm::S1, Arm::S2, Arm::S3, Arm::UniformRandom] {
                for c in 0..512 {
                    assert!(seen.insert(run_seed(instance, arm, c)));
                }
            }
        }
    }
}
