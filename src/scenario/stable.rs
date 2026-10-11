//! `stable`: the oscillation guard.
//!
//! One correct dominant source and two weak background sources, no critical signal. An
//! arbiter that switches away from the dominant source here is paying for anti-fixation with
//! pointless instability (the ETTO cost), and should keep `switch_count` low.

use super::{Scenario, ScriptedProcessor};
use crate::metrics::GroundTruth;
use crate::workspace::ProcessorId;

pub const NAME: &str = "stable";
pub const HORIZON: u64 = 200;

pub const DOMINANT: ProcessorId = ProcessorId(0);
pub const DOMINANT_WEIGHT: f64 = 0.9;
pub const BACKGROUND_1: ProcessorId = ProcessorId(1);
pub const BACKGROUND_1_WEIGHT: f64 = 0.2;
pub const BACKGROUND_2: ProcessorId = ProcessorId(2);
pub const BACKGROUND_2_WEIGHT: f64 = 0.25;

pub fn build() -> Scenario {
    let processors = vec![
        ScriptedProcessor::boxed(DOMINANT, "A", |_| Some(DOMINANT_WEIGHT)),
        ScriptedProcessor::boxed(BACKGROUND_1, "B", |_| Some(BACKGROUND_1_WEIGHT)),
        ScriptedProcessor::boxed(BACKGROUND_2, "C", |_| Some(BACKGROUND_2_WEIGHT)),
    ];
    Scenario {
        name: NAME,
        description: "dominant A (0.9) with background B (0.2) and C (0.25); nothing critical",
        horizon: HORIZON,
        held_out: false,
        processors,
        truth: GroundTruth::default(),
    }
}
