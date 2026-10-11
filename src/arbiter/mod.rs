//! Arbiters: the pluggable policy that decides which candidate wins the workspace.

mod argmax;
mod round_robin;
pub mod score;
pub mod tournament;
mod uniform_random;

pub use argmax::ArgmaxArbiter;
pub use round_robin::RoundRobinArbiter;
pub use score::{Habituation, Novelty, Rule, ScoreArbiter, ScoreConfig};
pub use uniform_random::UniformRandomArbiter;

use crate::workspace::{Chunk, Tick};

/// Selects at most one winner among the candidates of a tick.
///
/// Arbiters see only what a real system would observe: the candidate chunks (weights,
/// sources, payloads) and the tick. They never see scenario ground truth.
pub trait Arbiter {
    /// Returns the index of the winner in `candidates`, plus a score per candidate, for the
    /// record.
    ///
    /// Called on every tick, including ticks with no candidates. `candidates` is in
    /// ascending `ProcessorId` order. The returned `scores` must have exactly one entry per
    /// candidate, and `winner` must be a valid index or `None`.
    fn select(&mut self, candidates: &[Chunk], t: Tick) -> Selection;
}

/// The outcome of one arbitration.
#[derive(Clone, Debug, PartialEq)]
pub struct Selection {
    pub winner: Option<usize>,
    /// One score per candidate, in candidate order. Used for observability (records,
    /// score entropy), not re-interpreted by the workspace.
    pub scores: Vec<f64>,
    /// Human-readable explanation of the decision, kept in the tick record.
    pub reason: String,
}
