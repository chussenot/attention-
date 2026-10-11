//! The workspace cycle: *propose → select → broadcast*.
//!
//! The workspace is pure mechanism. It knows processors and arbiters only through the
//! [`Processor`] and [`Arbiter`] traits; every policy decision lives in an arbiter.

use std::error::Error;
use std::fmt;

use crate::arbiter::Arbiter;

/// Discrete time. One tick is one full propose → select → broadcast cycle.
pub type Tick = u64;

/// Identifies a processor. Its ordering is the tie-breaking order of the whole crate.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ProcessorId(pub u32);

impl fmt::Display for ProcessorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "proc{}", self.0)
    }
}

/// Identifies a chunk. Ground truth (which chunks are critical) is keyed by this id and
/// kept outside the workspace.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ChunkId(pub u64);

/// A unit of content competing for the workspace.
///
/// Deliberately carries no notion of criticality: an arbiter must not be able to learn from
/// a chunk whether it matters to the scenario's ground truth.
#[derive(Clone, Debug, PartialEq)]
pub struct Chunk {
    pub id: ChunkId,
    pub source: ProcessorId,
    /// Salience as claimed by the source. Arbiters may interpret it however they like.
    pub weight: f64,
    pub payload: String,
}

/// A specialist that may propose one chunk per tick and receives every broadcast winner.
pub trait Processor {
    fn id(&self) -> ProcessorId;

    /// Proposes at most one chunk for tick `t`. The chunk's `source` must equal `self.id()`.
    fn propose(&mut self, t: Tick) -> Option<Chunk>;

    /// Global broadcast: called on every processor (including the winner's source) with the
    /// chunk that won tick `t`. Not called on ticks without a winner.
    fn receive(&mut self, winner: &Chunk, t: Tick);
}

/// Everything that happened in one tick. Metrics are computed from these records only.
#[derive(Clone, Debug, PartialEq)]
pub struct TickRecord {
    pub t: Tick,
    /// Candidates in ascending `ProcessorId` order, exactly as the arbiter saw them.
    pub candidates: Vec<Chunk>,
    /// One score per candidate, as returned by the arbiter.
    pub scores: Vec<f64>,
    pub winner: Option<ChunkId>,
    pub winner_source: Option<ProcessorId>,
    pub reason: String,
}

/// Errors detected when assembling a workspace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkspaceError {
    /// Two processors report the same id, which would make tie-breaking and per-source
    /// metrics ambiguous.
    DuplicateProcessorId(ProcessorId),
}

impl fmt::Display for WorkspaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorkspaceError::DuplicateProcessorId(id) => {
                write!(f, "duplicate processor id {id}")
            }
        }
    }
}

impl Error for WorkspaceError {}

/// A limited-capacity stage (one winner per tick) that processors compete for.
pub struct Workspace {
    /// Sorted by ascending `ProcessorId`; this order defines candidate order.
    processors: Vec<Box<dyn Processor>>,
    arbiter: Box<dyn Arbiter>,
    now: Tick,
}

impl Workspace {
    /// Builds a workspace starting at tick 0.
    ///
    /// Processors are sorted by id here, once, so that every tick collects candidates in the
    /// same order regardless of how the caller listed them.
    pub fn new(
        mut processors: Vec<Box<dyn Processor>>,
        arbiter: Box<dyn Arbiter>,
    ) -> Result<Self, WorkspaceError> {
        processors.sort_by_key(|p| p.id());
        for pair in processors.windows(2) {
            if pair[0].id() == pair[1].id() {
                return Err(WorkspaceError::DuplicateProcessorId(pair[0].id()));
            }
        }
        Ok(Self {
            processors,
            arbiter,
            now: 0,
        })
    }

    /// The tick the next call to [`step`](Self::step) will execute.
    pub fn now(&self) -> Tick {
        self.now
    }

    /// Runs one cycle: collect proposals, let the arbiter select, broadcast the winner.
    ///
    /// The arbiter is called even when no processor proposes anything, so that stateful
    /// arbiters (e.g. with decaying habituation) observe the passage of time.
    ///
    /// # Panics
    ///
    /// On contract violations by a trait implementation, which would otherwise corrupt the
    /// record: a processor proposing a chunk whose `source` is not its own id, or an arbiter
    /// returning a winner index out of range or a `scores` vector of the wrong length.
    pub fn step(&mut self) -> TickRecord {
        let t = self.now;

        let mut candidates = Vec::new();
        for processor in &mut self.processors {
            if let Some(chunk) = processor.propose(t) {
                assert_eq!(
                    chunk.source,
                    processor.id(),
                    "processor {} proposed a chunk with source {} at t={t}",
                    processor.id(),
                    chunk.source,
                );
                candidates.push(chunk);
            }
        }

        let selection = self.arbiter.select(&candidates, t);
        assert_eq!(
            selection.scores.len(),
            candidates.len(),
            "arbiter returned {} scores for {} candidates at t={t}",
            selection.scores.len(),
            candidates.len(),
        );
        if let Some(index) = selection.winner {
            assert!(
                index < candidates.len(),
                "arbiter selected index {index} out of {} candidates at t={t}",
                candidates.len(),
            );
        }

        let winner = selection.winner.map(|index| &candidates[index]);
        if let Some(chunk) = winner {
            for processor in &mut self.processors {
                processor.receive(chunk, t);
            }
        }

        let record = TickRecord {
            t,
            winner: winner.map(|c| c.id),
            winner_source: winner.map(|c| c.source),
            candidates,
            scores: selection.scores,
            reason: selection.reason,
        };
        self.now += 1;
        record
    }

    /// Runs `ticks` cycles and returns their records in order.
    pub fn run(&mut self, ticks: u64) -> Vec<TickRecord> {
        (0..ticks).map(|_| self.step()).collect()
    }
}
