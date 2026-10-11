//! Scenarios: scripted processors, a horizon, and ground truth.
//!
//! A scenario is consumed by [`Scenario::run`], which gives the processors and the arbiter
//! to a [`Workspace`] and keeps the [`GroundTruth`] aside for evaluation. The workspace,
//! processors and arbiter never see it.

pub mod distractor_flood;
pub mod garden_path;
pub mod stable;

use std::collections::HashSet;
use std::ops::Range;

use crate::arbiter::Arbiter;
use crate::metrics::{self, GroundTruth, Metrics, MetricsConfig};
use crate::workspace::{Chunk, ChunkId, Processor, ProcessorId, Tick, TickRecord, Workspace};

pub use crate::metrics::CriticalStream;

/// A self-contained experiment. Build a fresh one per run: processors are stateful.
pub struct Scenario {
    pub name: &'static str,
    pub description: &'static str,
    pub horizon: u64,
    /// Held-out scenarios exist to check generalisation. Never tune parameters against them;
    /// freeze parameters first, then run and report the results unchanged.
    pub held_out: bool,
    pub processors: Vec<Box<dyn Processor>>,
    pub truth: GroundTruth,
}

impl Scenario {
    /// Runs the scenario for its full horizon with the given arbiter.
    ///
    /// # Panics
    ///
    /// If the scenario defines two processors with the same id (a scenario bug).
    pub fn run(self, arbiter: Box<dyn Arbiter>) -> ScenarioRun {
        let mut workspace = Workspace::new(self.processors, arbiter)
            .unwrap_or_else(|e| panic!("scenario {}: {e}", self.name));
        ScenarioRun {
            name: self.name,
            held_out: self.held_out,
            records: workspace.run(self.horizon),
            truth: self.truth,
        }
    }
}

/// The records of one scenario run, together with the ground truth needed to evaluate them.
pub struct ScenarioRun {
    pub name: &'static str,
    pub held_out: bool,
    pub records: Vec<TickRecord>,
    pub truth: GroundTruth,
}

impl ScenarioRun {
    pub fn metrics(&self, config: &MetricsConfig) -> Metrics {
        metrics::evaluate(&self.records, &self.truth, config)
    }
}

/// Constructors for every scenario, in a stable order. Includes held-out ones; check
/// [`Scenario::held_out`]. A scenario is consumed by a run, so call a builder per run.
pub const BUILDERS: [fn() -> Scenario; 3] =
    [garden_path::build, stable::build, distractor_flood::build];

/// One fresh instance of every scenario, in [`BUILDERS`] order.
pub fn all() -> Vec<Scenario> {
    BUILDERS.iter().map(|build| build()).collect()
}

/// Chunk id used by every scripted processor: source in the high 32 bits, tick in the low
/// 32. This lets a scenario enumerate its critical chunk ids up front. It reveals only the
/// source and tick, both already visible on the chunk.
pub fn chunk_id(source: ProcessorId, t: Tick) -> ChunkId {
    debug_assert!(
        t <= u64::from(u32::MAX),
        "tick {t} does not fit the chunk id encoding"
    );
    ChunkId((u64::from(source.0) << 32) | t)
}

/// The chunk ids a scripted `source` emits over `ticks`.
pub fn stream_chunks(source: ProcessorId, ticks: Range<Tick>) -> HashSet<ChunkId> {
    ticks.map(|t| chunk_id(source, t)).collect()
}

/// A processor driven by a script mapping each tick to an optional weight.
///
/// Payloads are neutral (`"<label>@<tick>"`) so they carry no hint of criticality. Broadcasts
/// are ignored: scripted processors do not adapt.
pub struct ScriptedProcessor {
    id: ProcessorId,
    label: &'static str,
    script: Box<dyn FnMut(Tick) -> Option<f64>>,
}

impl ScriptedProcessor {
    pub fn new(
        id: ProcessorId,
        label: &'static str,
        script: impl FnMut(Tick) -> Option<f64> + 'static,
    ) -> Self {
        Self {
            id,
            label,
            script: Box::new(script),
        }
    }

    pub fn boxed(
        id: ProcessorId,
        label: &'static str,
        script: impl FnMut(Tick) -> Option<f64> + 'static,
    ) -> Box<dyn Processor> {
        Box::new(Self::new(id, label, script))
    }
}

impl Processor for ScriptedProcessor {
    fn id(&self) -> ProcessorId {
        self.id
    }

    fn propose(&mut self, t: Tick) -> Option<Chunk> {
        (self.script)(t).map(|weight| Chunk {
            id: chunk_id(self.id, t),
            source: self.id,
            weight,
            payload: format!("{}@{t}", self.label),
        })
    }

    fn receive(&mut self, _winner: &Chunk, _t: Tick) {}
}
