//! `garden_path`: the reference failure case (Woods' garden path problem).
//!
//! An early, salient source captures attention; a later, weaker but critical signal is
//! missed. A pure salience-maximiser must fail here; if it does not, this scenario is wrong.

use super::{Scenario, ScriptedProcessor, stream_chunks};
use crate::metrics::{CriticalStream, GroundTruth};
use crate::workspace::{ProcessorId, Tick};

pub const NAME: &str = "garden_path";
pub const HORIZON: u64 = 200;

/// The salient source: weight 0.9 on every tick.
pub const A: ProcessorId = ProcessorId(0);
pub const A_WEIGHT: f64 = 0.9;

/// The weak source whose chunks become critical from `ONSET` on.
pub const B: ProcessorId = ProcessorId(1);
pub const B_WEIGHT_BEFORE: f64 = 0.3;
pub const B_WEIGHT_CRITICAL: f64 = 0.6;
pub const ONSET: Tick = 50;

pub fn build() -> Scenario {
    let processors = vec![
        ScriptedProcessor::boxed(A, "A", |_| Some(A_WEIGHT)),
        ScriptedProcessor::boxed(B, "B", |t| {
            Some(if t < ONSET {
                B_WEIGHT_BEFORE
            } else {
                B_WEIGHT_CRITICAL
            })
        }),
    ];
    let truth = GroundTruth {
        streams: vec![CriticalStream {
            name: "B from t=50".to_string(),
            chunks: stream_chunks(B, ONSET..HORIZON),
        }],
    };
    Scenario {
        name: NAME,
        description: "salient A (0.9) every tick; B 0.3 until t=50, then critical at 0.6",
        horizon: HORIZON,
        held_out: false,
        processors,
        truth,
    }
}
