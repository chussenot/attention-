#!/usr/bin/env bash
# Assembles the Family B author bundle exactly as docs/experiments/family-b-author-brief.md
# specifies, then validates it standalone, outside the repository:
#   - the bundle (with a stand-in src/family_b.rs) passes fmt, clippy -D warnings and tests;
#   - a separate crate can use the API and metrics the brief promises;
#   - no selection-policy implementation or reference to an excluded module is present;
#   - neither the bundle nor the brief mentions protocol details (leakage scan).
#
# src/lib.rs and src/arbiter.rs are extracted from the brief's own code blocks, so the
# documented text is what gets compiled. Commissioner-side tooling: never part of the bundle.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
brief="$repo/docs/experiments/family-b-author-brief.md"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
bundle="$work/bundle"
verify="$work/verify"
mkdir -p "$bundle/src" "$verify/src" "$verify/tests"

# Prints the first ```rust block that follows the line exactly equal to $1.
extract_block() {
    awk -v marker="$1" '
        $0 == marker { found = 1; next }
        found && /^```rust$/ { inside = 1; next }
        inside && /^```$/ { exit }
        inside { print }
    ' "$brief"
}

echo "== assembling bundle"
cp "$repo/Cargo.toml" "$bundle/"
cp "$repo/src/workspace.rs" "$repo/src/metrics.rs" "$repo/src/rng.rs" "$bundle/src/"
extract_block '`src/lib.rs`:' > "$bundle/src/lib.rs"
extract_block '`src/arbiter.rs`:' > "$bundle/src/arbiter.rs"
for f in lib.rs arbiter.rs; do
    [ -s "$bundle/src/$f" ] || { echo "FAIL: could not extract src/$f from the brief"; exit 1; }
done

# Stand-in for the author's deliverable: the interface from brief section 4.1, trivially filled.
cat > "$bundle/src/family_b.rs" <<'EOF'
//! Stand-in for the author's deliverable, used only to validate the bundle.

use crate::metrics::{CriticalStream, GroundTruth};
use crate::rng::SplitMix64;
use crate::workspace::{Chunk, ChunkId, Processor, ProcessorId, Tick};

pub enum InstanceLabel {
    Critical,
    Stable,
}

pub struct FamilyBInstance {
    pub processors: Vec<Box<dyn Processor>>,
    pub truth: GroundTruth,
    pub horizon: u64,
    pub label: InstanceLabel,
}

struct Constant {
    id: ProcessorId,
    weight: f64,
}

impl Processor for Constant {
    fn id(&self) -> ProcessorId {
        self.id
    }

    fn propose(&mut self, t: Tick) -> Option<Chunk> {
        Some(Chunk {
            id: ChunkId((u64::from(self.id.0) << 32) | t),
            source: self.id,
            weight: self.weight,
            payload: format!("P{}@{t}", self.id.0),
        })
    }

    fn receive(&mut self, _winner: &Chunk, _t: Tick) {}
}

pub fn instance(seed: u64) -> FamilyBInstance {
    let mut rng = SplitMix64::new(seed);
    let weight = rng.next_f64();
    let critical = seed % 2 == 0;
    let truth = if critical {
        GroundTruth {
            streams: vec![CriticalStream {
                name: "s".to_string(),
                chunks: (10..100).map(|t| ChunkId((1u64 << 32) | t)).collect(),
            }],
        }
    } else {
        GroundTruth::default()
    };
    FamilyBInstance {
        processors: vec![
            Box::new(Constant {
                id: ProcessorId(0),
                weight,
            }),
            Box::new(Constant {
                id: ProcessorId(1),
                weight: weight / 2.0,
            }),
        ],
        truth,
        horizon: 100,
        label: if critical {
            InstanceLabel::Critical
        } else {
            InstanceLabel::Stable
        },
    }
}
EOF

echo "== bundle contents"
(cd "$bundle" && find . -type f | sort)

echo "== bundle: cargo fmt --check"
(cd "$bundle" && cargo fmt --check)
echo "== bundle: cargo clippy --all-targets -- -D warnings"
(cd "$bundle" && cargo clippy --quiet --all-targets -- -D warnings)
echo "== bundle: cargo test"
(cd "$bundle" && cargo test --quiet)

echo "== API verification crate"
cat > "$verify/Cargo.toml" <<EOF
[package]
name = "bundle-verify"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
attn-workspace = { path = "$bundle" }
EOF
: > "$verify/src/lib.rs"
cat > "$verify/tests/api.rs" <<'EOF'
use attn_workspace::arbiter::{Arbiter, Selection};
use attn_workspace::family_b::{InstanceLabel, instance};
use attn_workspace::metrics::{self, CriticalStream, DEFAULT_DEADLINE, GroundTruth, MetricsConfig};
use attn_workspace::rng::SplitMix64;
use attn_workspace::workspace::{Chunk, ChunkId, ProcessorId, Tick, TickRecord, Workspace};

/// Selects nothing: only shows the interface can drive a Workspace.
struct NoSelection;

impl Arbiter for NoSelection {
    fn select(&mut self, candidates: &[Chunk], _t: Tick) -> Selection {
        Selection {
            winner: None,
            scores: vec![0.0; candidates.len()],
            reason: String::new(),
        }
    }
}

#[test]
fn api_and_metrics_are_available() {
    assert_eq!(DEFAULT_DEADLINE, 20);
    assert_eq!(SplitMix64::new(0).next_u64(), 0xE220_A839_7B1D_CDAF);

    let inst = instance(0);
    assert!(matches!(inst.label, InstanceLabel::Critical));
    let mut ws = Workspace::new(inst.processors, Box::new(NoSelection)).unwrap();
    let records: Vec<TickRecord> = ws.run(inst.horizon);
    let m = metrics::evaluate(&records, &inst.truth, &MetricsConfig::default());
    assert_eq!(m.streams.len(), 1);
    assert_eq!(m.streams[0].first_proposed, Some(10));
    assert!(m.streams[0].missed_critical);

    let _ = CriticalStream {
        name: String::new(),
        chunks: Default::default(),
    };
    let _ = (GroundTruth::default(), ChunkId(0), ProcessorId(0));
}
EOF
(cd "$verify" && cargo test --quiet)

echo "== no selection-policy implementation"
[ ! -e "$bundle/src/arbiter" ] || { echo "FAIL: src/arbiter/ present"; exit 1; }
if grep -nE '^\s*impl\b' "$bundle/src/arbiter.rs"; then
    echo "FAIL: impl block in src/arbiter.rs"; exit 1
fi
if grep -nE '^\s*fn ' "$bundle/src/arbiter.rs" | grep -vE ';\s*$'; then
    echo "FAIL: function body in src/arbiter.rs"; exit 1
fi
if grep -nE 'crate::scenario|arbiter::argmax|ArgmaxArbiter|mod argmax' "$bundle"/src/*.rs; then
    echo "FAIL: reference to an excluded module"; exit 1
fi

echo "== leakage scan (bundle sources and brief)"
leak='habituation|novelty|tournament|temperature|\bCTM\b|conscious|held.?out|distractor_flood|garden_path|\bP2\b|\b512\b|\barms?\b|\bA[0-3]\b|\bS[0-3]\b|λ|η|ρ|τ|bootstrap|freeze|selection rule|pass fraction|family G|\bC[1-5]\b|argmax|p2-protocol'
if grep -niE "$leak" "$bundle/Cargo.toml" "$bundle/src/workspace.rs" "$bundle/src/metrics.rs" \
    "$bundle/src/rng.rs" "$bundle/src/arbiter.rs" "$bundle/src/lib.rs" "$brief"; then
    echo "FAIL: leakage scan matched"; exit 1
fi

echo "PASS: Family B bundle builds standalone, exposes the promised API, and leaks nothing scanned"
