//! P2 experiment runner (`docs/experiments/p2-protocol.md` §8).
//!
//! ```text
//! cargo run --release --example p2 -- sanity
//! cargo run --release --example p2 -- dev [--smoke OUT_DIR]
//! cargo run --release --example p2 -- heldout
//! ```
//!
//! - `sanity`: runs A0 and RR on the G development block and checks §7.1's baseline sanity
//!   checks. Exits non-zero on failure.
//! - `dev`: the development phase (§8 step 4). Writes `docs/experiments/p2-dev-results.md` and
//!   `docs/experiments/p2-freeze.txt`, refusing to overwrite an existing freeze file. With
//!   `--smoke OUT_DIR`, runs a tiny labelled subset into OUT_DIR instead, to test the pipeline;
//!   smoke output is never a protocol result.
//! - `heldout`: the held-out phase (§8 step 7). Refuses to run until Family B is integrated.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use attn_workspace::experiment::phases::{self, Smoke};

const RESULTS: &str = "docs/experiments/p2-dev-results.md";
const FREEZE: &str = "docs/experiments/p2-freeze.txt";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["sanity"] => sanity(),
        ["dev"] => dev(None),
        ["dev", "--smoke", out] => dev(Some(PathBuf::from(out))),
        ["heldout"] => match phases::heldout_phase() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        },
        _ => {
            eprintln!("usage: p2 (sanity | dev [--smoke OUT_DIR] | heldout)");
            ExitCode::FAILURE
        }
    }
}

fn sanity() -> ExitCode {
    let s = phases::sanity();
    for (label, ok) in s.checks() {
        println!("{} {label}", if ok { "PASS" } else { "FAIL" });
    }
    println!(
        "A0 miss on C1–C3 = {:.4}; RR switch rate on C4 = {:.2} per 100; RR miss on C3 = {:.4}",
        s.a0_miss_c1_c3, s.rr_switch_c4, s.rr_miss_c3
    );
    if s.passed() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn dev(smoke_dir: Option<PathBuf>) -> ExitCode {
    let (results, freeze, smoke) = match &smoke_dir {
        Some(dir) => (
            dir.join("p2-dev-results.smoke.md"),
            dir.join("p2-freeze.smoke.txt"),
            Some(Smoke {
                instances_per_class: 2,
                vectors: 16,
            }),
        ),
        None => (PathBuf::from(RESULTS), PathBuf::from(FREEZE), None),
    };
    if smoke.is_none() && Path::new(FREEZE).exists() {
        eprintln!(
            "refusing to overwrite {FREEZE}: the development phase has already been frozen (§6.6)"
        );
        return ExitCode::FAILURE;
    }
    let start = Instant::now();
    let r = phases::dev_phase(smoke, |msg| {
        eprintln!("[{:>7.1}s] {msg}", start.elapsed().as_secs_f64())
    });
    if let Err(e) = std::fs::write(&results, phases::render_dev_report(&r))
        .and_then(|()| std::fs::write(&freeze, phases::render_freeze(&r)))
    {
        eprintln!("failed to write results: {e}");
        return ExitCode::FAILURE;
    }
    eprintln!(
        "wrote {} and {} in {:.1}s",
        results.display(),
        freeze.display(),
        start.elapsed().as_secs_f64()
    );
    ExitCode::SUCCESS
}
