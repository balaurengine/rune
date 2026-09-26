//! Balaur fork: every instruction a thread executes, counted by the function
//! that holds it, for a host's profiler.
//!
//! Upstream counts nothing, and a host that times its own calls into a unit
//! sees neither the functions inside them nor a callback a native function
//! makes. Off, this costs the run loop one relaxed load; on, an index into a
//! table per unit.

use core::sync::atomic::{AtomicBool, Ordering};

use ::rust_alloc::sync::Arc;
use std::cell::RefCell;
use std::collections::HashMap;
use std::string::{String, ToString};
use std::vec::Vec;

use crate::runtime::{Unit, UnitStorage};

static ON: AtomicBool = AtomicBool::new(false);

/// One function's share of what ran since profiling was turned on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionCost {
    /// The function's item path, or `<unit>` where the unit carries no debug
    /// information, which is what an exported unit drops.
    pub path: String,
    /// Instructions executed inside the function, not counting its callees.
    pub instructions: u64,
    /// How many times it was entered.
    pub calls: u64,
}

/// Counts by instruction pointer, one table per unit, the unit held so its
/// address is not reused by a later one while its counts are kept.
#[derive(Default)]
struct Counts {
    units: Vec<(Arc<Unit>, Vec<u64>)>,
    /// The unit the last instruction came from: a run stays in one.
    last: usize,
}

std::thread_local! {
    static COUNTS: RefCell<Counts> = RefCell::new(Counts::default());
}

/// Start or stop counting. Turning it on clears what was counted before.
pub fn set_profiling(on: bool) {
    if on {
        COUNTS.with_borrow_mut(|counts| *counts = Counts::default());
    }
    ON.store(on, Ordering::Relaxed);
}

/// Whether instructions are being counted.
#[inline]
pub fn profiling() -> bool {
    ON.load(Ordering::Relaxed)
}

/// Count one instruction at `ip` in `unit`.
pub(crate) fn record(unit: &Arc<Unit>, ip: usize) {
    COUNTS.with_borrow_mut(|counts| {
        let at = match counts.units.get(counts.last) {
            Some((held, _)) if Arc::ptr_eq(held, unit) => counts.last,
            _ => {
                let found = counts
                    .units
                    .iter()
                    .position(|(held, _)| Arc::ptr_eq(held, unit));
                let at = found.unwrap_or_else(|| {
                    let size = unit.instructions().end();
                    counts.units.push((unit.clone(), std::vec![0; size]));
                    counts.units.len() - 1
                });
                counts.last = at;
                at
            }
        };
        let table = &mut counts.units[at].1;
        if ip >= table.len() {
            table.resize(ip + 1, 0);
        }
        table[ip] += 1;
    });
}

/// What each function cost since profiling was turned on, dearest first.
///
/// Leaves the counts where they are, so a reader every frame sees a running
/// total until it turns profiling on again.
pub fn snapshot() -> Vec<FunctionCost> {
    let mut by_path: HashMap<String, (u64, u64)> = HashMap::new();
    COUNTS.with_borrow(|counts| {
        for (unit, table) in &counts.units {
            let debug = unit.debug_info();
            let mut starts: Vec<usize> = debug
                .map(|debug| debug.functions_rev.keys().copied().collect())
                .unwrap_or_default();
            starts.sort_unstable();
            // By function start first, then one name per function: a table
            // holds thousands of counted instructions and a handful of starts.
            let mut by_start: HashMap<Option<usize>, (u64, u64)> = HashMap::new();
            for (ip, &count) in table.iter().enumerate() {
                if count == 0 {
                    continue;
                }
                let start = match starts.binary_search(&ip) {
                    Ok(at) => Some(starts[at]),
                    Err(0) => None,
                    Err(at) => Some(starts[at - 1]),
                };
                let slot = by_start.entry(start).or_default();
                slot.0 += count;
                if start == Some(ip) {
                    slot.1 += count;
                }
            }
            for (start, (instructions, calls)) in by_start {
                let path = start
                    .and_then(|start| debug?.function_at(start))
                    .map_or_else(|| "<unit>".to_string(), |(_, sig)| sig.path.to_string());
                let slot = by_path.entry(path).or_default();
                slot.0 += instructions;
                slot.1 += calls;
            }
        }
    });
    let mut out: Vec<FunctionCost> = by_path
        .into_iter()
        .map(|(path, (instructions, calls))| FunctionCost {
            path,
            instructions,
            calls,
        })
        .collect();
    out.sort_by(|a, b| {
        b.instructions
            .cmp(&a.instructions)
            .then_with(|| a.path.cmp(&b.path))
    });
    out
}
