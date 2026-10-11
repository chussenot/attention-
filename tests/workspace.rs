//! Workspace mechanics: candidate order, broadcast, tick advance, contract checks.

use std::cell::RefCell;
use std::rc::Rc;

use attn_workspace::{
    Arbiter, Chunk, ChunkId, Processor, ProcessorId, Selection, Tick, Workspace, WorkspaceError,
};

type Log = Rc<RefCell<Vec<String>>>;

/// Proposes a fixed weight while `active(t)` holds, and logs every broadcast it receives.
struct TestProcessor {
    id: ProcessorId,
    weight: f64,
    active: fn(Tick) -> bool,
    log: Log,
}

impl TestProcessor {
    fn boxed(id: u32, weight: f64, active: fn(Tick) -> bool, log: &Log) -> Box<dyn Processor> {
        Box::new(Self {
            id: ProcessorId(id),
            weight,
            active,
            log: Rc::clone(log),
        })
    }
}

impl Processor for TestProcessor {
    fn id(&self) -> ProcessorId {
        self.id
    }

    fn propose(&mut self, t: Tick) -> Option<Chunk> {
        (self.active)(t).then(|| Chunk {
            id: ChunkId(u64::from(self.id.0) * 1000 + t),
            source: self.id,
            weight: self.weight,
            payload: format!("{}@{t}", self.id),
        })
    }

    fn receive(&mut self, winner: &Chunk, t: Tick) {
        self.log
            .borrow_mut()
            .push(format!("{} got {} at {t}", self.id, winner.payload));
    }
}

/// Picks the first candidate and logs the candidate order it was shown.
struct FirstArbiter {
    log: Log,
}

impl Arbiter for FirstArbiter {
    fn select(&mut self, candidates: &[Chunk], t: Tick) -> Selection {
        let order: Vec<String> = candidates.iter().map(|c| c.source.to_string()).collect();
        self.log
            .borrow_mut()
            .push(format!("select t={t} [{}]", order.join(",")));
        Selection {
            winner: (!candidates.is_empty()).then_some(0),
            scores: candidates.iter().map(|c| c.weight).collect(),
            reason: "first".to_string(),
        }
    }
}

fn always(_: Tick) -> bool {
    true
}

fn never(_: Tick) -> bool {
    false
}

fn new_log() -> Log {
    Rc::new(RefCell::new(Vec::new()))
}

#[test]
fn candidates_are_ordered_by_processor_id_regardless_of_insertion_order() {
    let log = new_log();
    let processors = vec![
        TestProcessor::boxed(7, 0.1, always, &log),
        TestProcessor::boxed(2, 0.2, always, &log),
        TestProcessor::boxed(5, 0.3, always, &log),
    ];
    let mut ws = Workspace::new(processors, Box::new(FirstArbiter { log: log.clone() })).unwrap();

    let record = ws.step();

    let sources: Vec<u32> = record.candidates.iter().map(|c| c.source.0).collect();
    assert_eq!(sources, vec![2, 5, 7]);
    assert_eq!(record.winner_source, Some(ProcessorId(2)));
    assert_eq!(record.scores, vec![0.2, 0.3, 0.1]);
    assert_eq!(log.borrow()[0], "select t=0 [proc2,proc5,proc7]");
}

#[test]
fn winner_is_broadcast_to_every_processor_including_its_source() {
    let log = new_log();
    let processors = vec![
        TestProcessor::boxed(1, 0.5, always, &log),
        TestProcessor::boxed(2, 0.5, never, &log),
    ];
    let mut ws = Workspace::new(processors, Box::new(FirstArbiter { log: log.clone() })).unwrap();

    ws.step();

    let entries = log.borrow();
    assert!(entries.contains(&"proc1 got proc1@0 at 0".to_string()));
    assert!(entries.contains(&"proc2 got proc1@0 at 0".to_string()));
}

#[test]
fn empty_tick_still_calls_the_arbiter_and_broadcasts_nothing() {
    let log = new_log();
    let processors = vec![TestProcessor::boxed(1, 0.5, never, &log)];
    let mut ws = Workspace::new(processors, Box::new(FirstArbiter { log: log.clone() })).unwrap();

    let record = ws.step();

    assert!(record.candidates.is_empty());
    assert!(record.scores.is_empty());
    assert_eq!(record.winner, None);
    assert_eq!(record.winner_source, None);
    assert_eq!(*log.borrow(), vec!["select t=0 []".to_string()]);
}

#[test]
fn ticks_advance_by_one_per_step() {
    let log = new_log();
    let processors = vec![TestProcessor::boxed(1, 0.5, always, &log)];
    let mut ws = Workspace::new(processors, Box::new(FirstArbiter { log })).unwrap();

    assert_eq!(ws.now(), 0);
    let records = ws.run(3);
    let ticks: Vec<Tick> = records.iter().map(|r| r.t).collect();
    assert_eq!(ticks, vec![0, 1, 2]);
    assert_eq!(ws.now(), 3);
}

#[test]
fn duplicate_processor_ids_are_rejected() {
    let log = new_log();
    let processors = vec![
        TestProcessor::boxed(3, 0.5, always, &log),
        TestProcessor::boxed(3, 0.6, always, &log),
    ];
    let result = Workspace::new(processors, Box::new(FirstArbiter { log }));
    assert_eq!(
        result.err(),
        Some(WorkspaceError::DuplicateProcessorId(ProcessorId(3)))
    );
}

struct BadIndexArbiter;

impl Arbiter for BadIndexArbiter {
    fn select(&mut self, candidates: &[Chunk], _t: Tick) -> Selection {
        Selection {
            winner: Some(candidates.len()),
            scores: vec![0.0; candidates.len()],
            reason: String::new(),
        }
    }
}

#[test]
#[should_panic(expected = "out of 1 candidates")]
fn out_of_range_winner_index_panics() {
    let log = new_log();
    let processors = vec![TestProcessor::boxed(1, 0.5, always, &log)];
    let mut ws = Workspace::new(processors, Box::new(BadIndexArbiter)).unwrap();
    ws.step();
}

struct ImpostorProcessor;

impl Processor for ImpostorProcessor {
    fn id(&self) -> ProcessorId {
        ProcessorId(1)
    }

    fn propose(&mut self, t: Tick) -> Option<Chunk> {
        Some(Chunk {
            id: ChunkId(t),
            source: ProcessorId(9),
            weight: 1.0,
            payload: String::new(),
        })
    }

    fn receive(&mut self, _winner: &Chunk, _t: Tick) {}
}

#[test]
#[should_panic(expected = "proposed a chunk with source proc9")]
fn chunk_with_foreign_source_panics() {
    let mut ws = Workspace::new(
        vec![Box::new(ImpostorProcessor)],
        Box::new(FirstArbiter { log: new_log() }),
    )
    .unwrap();
    ws.step();
}
