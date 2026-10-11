//! `distractor_flood`: a **held-out** scenario for P2.
//!
//! Five distractors with fluctuating weights, plus one critical stream that appears at
//! t = 80 with a constant, moderate weight. The distractors' mean weight (0.6) exceeds the
//! critical weight (0.5), so raw salience almost never favours the critical stream; the
//! maximum of five draws exceeds 0.5 with probability 1 − (0.2/0.6)⁵ ≈ 99.6% per tick.
//!
//! Defined before any P2 work and not tuned against any arbiter. Do not change its
//! parameters to make an arbiter look better; freeze arbiter parameters first, then run this
//! and report the numbers as they come.

use super::{Scenario, ScriptedProcessor, stream_chunks};
use crate::metrics::{CriticalStream, GroundTruth};
use crate::rng::SplitMix64;
use crate::workspace::{ProcessorId, Tick};

pub const NAME: &str = "distractor_flood";
pub const HORIZON: u64 = 200;

pub const DISTRACTORS: [ProcessorId; 5] = [
    ProcessorId(0),
    ProcessorId(1),
    ProcessorId(2),
    ProcessorId(3),
    ProcessorId(4),
];
/// Distractor weights are uniform in `[DISTRACTOR_MIN, DISTRACTOR_MIN + DISTRACTOR_SPAN)`.
pub const DISTRACTOR_MIN: f64 = 0.3;
pub const DISTRACTOR_SPAN: f64 = 0.6;
/// Distractor `i` is seeded with `SEED_BASE + i`.
pub const SEED_BASE: u64 = 0xD157_0000;

pub const CRITICAL: ProcessorId = ProcessorId(5);
pub const CRITICAL_WEIGHT: f64 = 0.5;
pub const ONSET: Tick = 80;

const LABELS: [&str; 5] = ["A", "B", "C", "D", "E"];

pub fn build() -> Scenario {
    let mut processors = Vec::new();
    for (i, (&id, &label)) in DISTRACTORS.iter().zip(LABELS.iter()).enumerate() {
        let mut rng = SplitMix64::new(SEED_BASE + i as u64);
        // `propose` is called exactly once per tick, so one draw per call is one draw per
        // tick and the sequence is reproducible.
        processors.push(ScriptedProcessor::boxed(id, label, move |_| {
            Some(DISTRACTOR_MIN + DISTRACTOR_SPAN * rng.next_f64())
        }));
    }
    processors.push(ScriptedProcessor::boxed(CRITICAL, "F", |t| {
        (t >= ONSET).then_some(CRITICAL_WEIGHT)
    }));

    let truth = GroundTruth {
        streams: vec![CriticalStream {
            name: "F from t=80".to_string(),
            chunks: stream_chunks(CRITICAL, ONSET..HORIZON),
        }],
    };
    Scenario {
        name: NAME,
        description: "five distractors uniform in [0.3, 0.9); F silent, then critical at 0.5 from t=80",
        horizon: HORIZON,
        held_out: true,
        processors,
        truth,
    }
}
