//! `attn-workspace`: a deterministic, std-only global-workspace attention mechanism and an
//! evaluation harness that measures whether an arbiter resists fixation and premature
//! narrowing.
//!
//! The cycle is *propose → select → broadcast*: every [`Processor`] may propose one
//! [`Chunk`] per tick, an [`Arbiter`] selects at most one winner, and the winner is
//! broadcast to all processors. Each tick yields a [`TickRecord`]; metrics are computed from
//! records only.
//!
//! See `docs/design.md` for the rationale behind each design decision.

pub mod arbiter;
pub mod metrics;
pub mod rng;
pub mod scenario;
pub mod stats;
pub mod workspace;

pub use arbiter::{Arbiter, ArgmaxArbiter, Selection};
pub use workspace::{
    Chunk, ChunkId, Processor, ProcessorId, Tick, TickRecord, Workspace, WorkspaceError,
};
