//! Raw-data archive of the development phase: every run outcome as JSON Lines.
//!
//! The archive lets anyone recompute every development statistic without re-running the
//! simulation (`p2 report-from-archive`). Schema `attn-workspace/p2-dev-raw/v1`, one JSON
//! object per line:
//!
//! - line 1, `"record":"header"`: schema, source commit, generation command and the seed
//!   configuration (shared-vector seed, bootstrap seed base, run-seed formula, case-study
//!   instance seeds);
//! - `"record":"vector"`: one per shared vector, `c`, `k`, `lambda`, `eta`, `rho`, `tau`;
//! - `"record":"run"`: one per run, `set` (`g_dev`, `garden_path`, `stable`), `seed`
//!   (instance seed), `stratum` (label), `arm`, `c` (configuration; 0 for parameter-free
//!   arms), and the [`RunOutcome`] fields `missed`, `latency`, `switch_rate`, `switch_count`,
//!   `eligible`, `longest_streak` (`null` when undefined).
//!
//! Floats are written with Rust's shortest round-trip representation, so they parse back to
//! the identical `f64`. Compression (gzip) is applied outside the crate, which stays std-only.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::{BufRead, Write};

use super::arms::{Arm, N_VECTORS, ParamVector, VECTOR_SEED, shared_vectors};
use super::grid::{Grid, Instance, RunOutcome, Stratum};
use super::phases::{DevGrids, GARDEN_PATH_SEED, STABLE_SEED, g_dev_set, g_strata};
use crate::stats::BOOTSTRAP_SEED_BASE;

pub const SCHEMA: &str = "attn-workspace/p2-dev-raw/v1";

fn arm_by_label(label: &str) -> Option<Arm> {
    Arm::ALL.iter().copied().find(|a| a.label() == label)
}

fn opt<T: std::fmt::Display>(v: Option<T>) -> String {
    v.map_or("null".to_string(), |x| x.to_string())
}

fn opt_f64(v: Option<f64>) -> String {
    v.map_or("null".to_string(), |x| format!("{x:?}"))
}

/// Writes the archive. `source_commit` and `command` are recorded in the header.
pub fn write(
    grids: &DevGrids,
    source_commit: &str,
    command: &str,
    out: &mut impl Write,
) -> std::io::Result<()> {
    writeln!(
        out,
        "{{\"record\":\"header\",\"schema\":\"{SCHEMA}\",\"source_commit\":\"{source_commit}\",\
         \"command\":\"{command}\",\"vector_seed\":\"{VECTOR_SEED:#x}\",\"n_vectors\":{},\
         \"bootstrap_seed_base\":\"{BOOTSTRAP_SEED_BASE:#x}\",\
         \"run_seed\":\"mix(mix(mix(instance_seed) ^ arm_id) ^ c), mix(x) = SplitMix64::new(x).next_u64()\",\
         \"garden_path_seed\":{GARDEN_PATH_SEED},\"stable_seed\":{STABLE_SEED}}}",
        grids.vectors.len()
    )?;
    for (c, v) in grids.vectors.iter().enumerate() {
        writeln!(
            out,
            "{{\"record\":\"vector\",\"c\":{c},\"k\":{:?},\"lambda\":{:?},\"eta\":{:?},\"rho\":{:?},\"tau\":{:?}}}",
            v.k, v.lambda, v.eta, v.rho, v.tau
        )?;
    }
    for (set, grid) in [
        ("g_dev", &grids.g),
        ("garden_path", &grids.garden_path),
        ("stable", &grids.stable),
    ] {
        for (&arm, per_config) in &grid.runs {
            for (c, outcomes) in per_config.iter().enumerate() {
                for (i, o) in outcomes.iter().enumerate() {
                    let inst = grid.instances[i];
                    let mut line = String::new();
                    let _ = write!(
                        line,
                        "{{\"record\":\"run\",\"set\":\"{set}\",\"seed\":{},\"stratum\":\"{}\",\"arm\":\"{}\",\"c\":{c},\
                         \"missed\":{},\"latency\":{},\"switch_rate\":{},\"switch_count\":{},\"eligible\":{},\"longest_streak\":{}}}",
                        inst.seed,
                        grid.strata[inst.stratum].label,
                        arm.label(),
                        opt(o.missed),
                        opt(o.latency),
                        opt_f64(o.switch_rate),
                        o.switch_count,
                        o.eligible,
                        o.longest_streak
                    );
                    writeln!(out, "{line}")?;
                }
            }
        }
    }
    Ok(())
}

/// A JSON scalar in the archive's flat objects.
#[derive(Clone, Debug, PartialEq)]
enum Value {
    Str(String),
    Num(String),
    Bool(bool),
    Null,
}

/// Parses one flat JSON object (string keys; string, number, boolean or null values).
fn parse_object(line: &str) -> Result<BTreeMap<String, Value>, String> {
    let s = line.trim();
    let inner = s
        .strip_prefix('{')
        .and_then(|r| r.strip_suffix('}'))
        .ok_or_else(|| format!("not a JSON object: {s}"))?;
    let chars: Vec<char> = inner.chars().collect();
    let mut i = 0;
    let mut map = BTreeMap::new();
    let read_string = |i: &mut usize| -> Result<String, String> {
        if chars.get(*i) != Some(&'"') {
            return Err(format!("expected '\"' at {i} in {s}", i = *i));
        }
        *i += 1;
        let mut out = String::new();
        while let Some(&ch) = chars.get(*i) {
            *i += 1;
            match ch {
                '"' => return Ok(out),
                '\\' => {
                    let esc = chars.get(*i).copied().ok_or("dangling escape")?;
                    *i += 1;
                    out.push(match esc {
                        '"' => '"',
                        '\\' => '\\',
                        'n' => '\n',
                        other => return Err(format!("unsupported escape \\{other}")),
                    });
                }
                other => out.push(other),
            }
        }
        Err("unterminated string".to_string())
    };
    while i < chars.len() {
        let key = read_string(&mut i)?;
        if chars.get(i) != Some(&':') {
            return Err(format!("expected ':' after key {key}"));
        }
        i += 1;
        let value = match chars.get(i) {
            Some('"') => Value::Str(read_string(&mut i)?),
            Some(_) => {
                let start = i;
                while i < chars.len() && chars[i] != ',' {
                    i += 1;
                }
                let raw: String = chars[start..i].iter().collect();
                match raw.as_str() {
                    "true" => Value::Bool(true),
                    "false" => Value::Bool(false),
                    "null" => Value::Null,
                    _ => Value::Num(raw),
                }
            }
            None => return Err(format!("missing value for {key}")),
        };
        if map.insert(key.clone(), value).is_some() {
            return Err(format!("duplicate key {key}"));
        }
        if chars.get(i) == Some(&',') {
            i += 1;
        }
    }
    Ok(map)
}

fn get<'a>(m: &'a BTreeMap<String, Value>, key: &str) -> Result<&'a Value, String> {
    m.get(key).ok_or_else(|| format!("missing field {key}"))
}

fn as_str<'a>(m: &'a BTreeMap<String, Value>, key: &str) -> Result<&'a str, String> {
    match get(m, key)? {
        Value::Str(s) => Ok(s),
        other => Err(format!("{key}: expected string, got {other:?}")),
    }
}

fn as_u64(m: &BTreeMap<String, Value>, key: &str) -> Result<u64, String> {
    match get(m, key)? {
        Value::Num(n) => n.parse().map_err(|e| format!("{key}: {e}")),
        other => Err(format!("{key}: expected integer, got {other:?}")),
    }
}

fn as_f64(m: &BTreeMap<String, Value>, key: &str) -> Result<f64, String> {
    match get(m, key)? {
        Value::Num(n) => n.parse().map_err(|e| format!("{key}: {e}")),
        other => Err(format!("{key}: expected number, got {other:?}")),
    }
}

fn opt_of<T>(
    m: &BTreeMap<String, Value>,
    key: &str,
    f: impl Fn(&Value) -> Result<T, String>,
) -> Result<Option<T>, String> {
    match get(m, key)? {
        Value::Null => Ok(None),
        v => f(v).map(Some),
    }
}

/// Partially filled outcomes while reading: `[arm][configuration][instance]`.
type PartialRuns = BTreeMap<Arm, Vec<Vec<Option<RunOutcome>>>>;

struct SetLayout {
    strata: Vec<Stratum>,
    instances: Vec<Instance>,
}

fn layouts() -> BTreeMap<&'static str, SetLayout> {
    let g = g_dev_set();
    let mut m = BTreeMap::new();
    m.insert(
        "g_dev",
        SetLayout {
            strata: g_strata(),
            instances: g.instances,
        },
    );
    for (name, seed, critical) in [
        ("garden_path", GARDEN_PATH_SEED, true),
        ("stable", STABLE_SEED, false),
    ] {
        m.insert(
            name,
            SetLayout {
                strata: vec![Stratum {
                    label: name.to_string(),
                    critical,
                }],
                instances: vec![Instance { seed, stratum: 0 }],
            },
        );
    }
    m
}

/// Reads and validates an archive, rebuilding the grids exactly. Fails on any schema
/// mismatch, unknown or duplicated run, missing run, or vector that differs from the shared
/// vectors.
pub fn read(input: impl BufRead) -> Result<DevGrids, String> {
    let layouts = layouts();
    let mut lines = input.lines().enumerate();

    let (_, header) = lines.next().ok_or("empty archive")?;
    let header = parse_object(&header.map_err(|e| e.to_string())?)?;
    if as_str(&header, "record")? != "header" || as_str(&header, "schema")? != SCHEMA {
        return Err(format!("first line must be a {SCHEMA} header"));
    }
    if as_str(&header, "vector_seed")? != format!("{VECTOR_SEED:#x}")
        || as_u64(&header, "n_vectors")? != N_VECTORS as u64
    {
        return Err("header seed configuration does not match this code".to_string());
    }

    let expected_vectors = shared_vectors();
    let mut vectors: Vec<Option<ParamVector>> = vec![None; N_VECTORS];
    // runs[set][arm][c][instance index]
    let mut runs: BTreeMap<String, PartialRuns> = BTreeMap::new();
    for (name, layout) in &layouts {
        let per_arm = Arm::ALL
            .iter()
            .map(|&arm| {
                let n = if arm.is_parameterised() { N_VECTORS } else { 1 };
                (arm, vec![vec![None; layout.instances.len()]; n])
            })
            .collect();
        runs.insert(name.to_string(), per_arm);
    }

    for (n, line) in lines {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let ctx = |e: String| format!("line {}: {e}", n + 1);
        let m = parse_object(&line).map_err(ctx)?;
        match as_str(&m, "record").map_err(ctx)? {
            "vector" => {
                let c = as_u64(&m, "c").map_err(ctx)? as usize;
                let v = ParamVector {
                    k: as_f64(&m, "k").map_err(ctx)?,
                    lambda: as_f64(&m, "lambda").map_err(ctx)?,
                    eta: as_f64(&m, "eta").map_err(ctx)?,
                    rho: as_f64(&m, "rho").map_err(ctx)?,
                    tau: as_f64(&m, "tau").map_err(ctx)?,
                };
                if expected_vectors.get(c) != Some(&v) {
                    return Err(ctx(format!("vector {c} differs from the shared vectors")));
                }
                if vectors[c].replace(v).is_some() {
                    return Err(ctx(format!("vector {c} appears twice")));
                }
            }
            "run" => {
                let set = as_str(&m, "set").map_err(ctx)?;
                let layout = layouts
                    .get(set)
                    .ok_or_else(|| ctx(format!("unknown set {set}")))?;
                let seed = as_u64(&m, "seed").map_err(ctx)?;
                let i = layout
                    .instances
                    .iter()
                    .position(|inst| inst.seed == seed)
                    .ok_or_else(|| ctx(format!("unknown instance {seed} in {set}")))?;
                let label = as_str(&m, "stratum").map_err(ctx)?;
                if layout.strata[layout.instances[i].stratum].label != label {
                    return Err(ctx(format!(
                        "instance {seed} has stratum {label}, expected another"
                    )));
                }
                let arm_label = as_str(&m, "arm").map_err(ctx)?;
                let arm = arm_by_label(arm_label)
                    .ok_or_else(|| ctx(format!("unknown arm {arm_label}")))?;
                let c = as_u64(&m, "c").map_err(ctx)? as usize;
                let outcome = RunOutcome {
                    missed: opt_of(&m, "missed", |v| match v {
                        Value::Bool(b) => Ok(*b),
                        other => Err(format!("missed: {other:?}")),
                    })
                    .map_err(ctx)?,
                    latency: opt_of(&m, "latency", |v| match v {
                        Value::Num(x) => x.parse().map_err(|e| format!("latency: {e}")),
                        other => Err(format!("latency: {other:?}")),
                    })
                    .map_err(ctx)?,
                    switch_rate: opt_of(&m, "switch_rate", |v| match v {
                        Value::Num(x) => x.parse().map_err(|e| format!("switch_rate: {e}")),
                        other => Err(format!("switch_rate: {other:?}")),
                    })
                    .map_err(ctx)?,
                    switch_count: as_u64(&m, "switch_count").map_err(ctx)?,
                    eligible: as_u64(&m, "eligible").map_err(ctx)?,
                    longest_streak: as_u64(&m, "longest_streak").map_err(ctx)?,
                };
                let slot = runs
                    .get_mut(set)
                    .and_then(|a| a.get_mut(&arm))
                    .and_then(|per_c| per_c.get_mut(c))
                    .map(|per_i| &mut per_i[i])
                    .ok_or_else(|| ctx(format!("configuration {c} out of range for {arm}")))?;
                if slot.replace(outcome).is_some() {
                    return Err(ctx(format!("duplicate run {set}/{seed}/{arm}/{c}")));
                }
            }
            other => return Err(ctx(format!("unexpected record {other}"))),
        }
    }

    let vectors: Vec<ParamVector> = vectors
        .into_iter()
        .enumerate()
        .map(|(c, v)| v.ok_or_else(|| format!("vector {c} missing")))
        .collect::<Result<_, _>>()?;

    let mut grids: BTreeMap<String, Grid> = BTreeMap::new();
    for (name, layout) in layouts {
        let mut complete = BTreeMap::new();
        for (arm, per_c) in runs.remove(name).expect("inserted above") {
            let mut rows = Vec::with_capacity(per_c.len());
            for (c, per_i) in per_c.into_iter().enumerate() {
                let row: Vec<RunOutcome> = per_i
                    .into_iter()
                    .enumerate()
                    .map(|(i, o)| {
                        o.ok_or_else(|| {
                            format!("missing run {name}/{}/{arm}/{c}", layout.instances[i].seed)
                        })
                    })
                    .collect::<Result<_, _>>()?;
                rows.push(row);
            }
            complete.insert(arm, rows);
        }
        grids.insert(
            name.to_string(),
            Grid {
                name: name.to_string(),
                set_id: if name == "g_dev" { 0 } else { 100 },
                strata: layout.strata,
                instances: layout.instances,
                runs: complete,
            },
        );
    }
    Ok(DevGrids {
        vectors,
        g: grids.remove("g_dev").expect("present"),
        garden_path: grids.remove("garden_path").expect("present"),
        stable: grids.remove("stable").expect("present"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_flat_objects() {
        let m = parse_object(r#"{"a":"x\"y","b":1.5,"c":null,"d":true,"e":-3}"#).unwrap();
        assert_eq!(m["a"], Value::Str("x\"y".into()));
        assert_eq!(m["b"], Value::Num("1.5".into()));
        assert_eq!(m["c"], Value::Null);
        assert_eq!(m["d"], Value::Bool(true));
        assert_eq!(m["e"], Value::Num("-3".into()));
        assert!(parse_object(r#"{"a":1,"a":2}"#).is_err());
        assert!(parse_object("not json").is_err());
    }

    #[test]
    fn floats_round_trip_exactly() {
        for x in [0.1, 2.248743718592965, 1e-300, 100.0, f64::MIN_POSITIVE] {
            let s = format!("{x:?}");
            assert_eq!(s.parse::<f64>().unwrap(), x);
        }
    }
}
