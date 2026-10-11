//! Runs every scenario with every arbiter and prints a metrics table (Markdown).
//!
//! `cargo run --example run_scenarios`

use attn_workspace::metrics::{Metrics, MetricsConfig};
use attn_workspace::scenario::BUILDERS;
use attn_workspace::{Arbiter, ArgmaxArbiter};

type ArbiterFactory = fn() -> Box<dyn Arbiter>;

/// Arbiters under comparison. P2 and later arbiters are added here.
const ARBITERS: [(&str, ArbiterFactory); 1] = [("argmax", || Box::new(ArgmaxArbiter::new()))];

fn main() {
    let config = MetricsConfig::default();

    println!(
        "| scenario | arbiter | held out | switches | longest streak | critical latency | missed (deadline {}) | entropy mean (bits) | entropy min (bits) |",
        config.deadline
    );
    println!("|---|---|---|---:|---|---|---|---:|---:|");

    for build in BUILDERS {
        for (arbiter_name, make_arbiter) in ARBITERS {
            let run = build().run(make_arbiter());
            let m = run.metrics(&config);
            println!(
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                run.name,
                arbiter_name,
                if run.held_out { "yes" } else { "no" },
                m.switch_count,
                longest_streak(&m),
                latencies(&m),
                missed(&m),
                fmt_bits(m.score_entropy.mean),
                fmt_bits(m.score_entropy.min),
            );
        }
    }
}

fn longest_streak(m: &Metrics) -> String {
    match m.longest_streak() {
        Some((source, streak)) => format!("{streak}/{} ({source})", m.ticks),
        None => "–".to_string(),
    }
}

/// One entry per critical stream; "–" when the scenario has none.
fn latencies(m: &Metrics) -> String {
    if m.streams.is_empty() {
        return "–".to_string();
    }
    m.streams
        .iter()
        .map(|s| match s.switch_latency {
            Some(l) => format!("{l} ticks"),
            None => "never".to_string(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn missed(m: &Metrics) -> String {
    if m.streams.is_empty() {
        return "–".to_string();
    }
    m.streams
        .iter()
        .map(|s| if s.missed_critical { "yes" } else { "no" })
        .collect::<Vec<_>>()
        .join(", ")
}

fn fmt_bits(value: Option<f64>) -> String {
    value.map_or_else(|| "–".to_string(), |v| format!("{v:.3}"))
}
