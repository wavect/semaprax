//! CPU reference executor tests.
//!
//! - [`differential`] runs every admitted kernel through both the CPU
//!   reference and the ordinary reference interpreter executing the same
//!   checked function as scalar code, and includes the negative-control
//!   mutant that proves the comparison is not vacuous;
//! - [`lifecycle`] covers transfers, bounds, misuse, cancellation, device
//!   loss, stale artifacts, sticky failure, and exactly-once cleanup order;
//! - [`admission`] covers every refusal reached from checked source.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::hir::ResolvedProgram;

use super::*;

mod admission;
mod differential;
mod lifecycle;

/// The shared kernel module. Every kernel is an ordinary checked function.
const KERNELS: &str = r#"
module test.compute_kernels;

@id("k.affine")
fn affine(x: i64, y: i64) -> i64
{
    let scaled = x * 3;
    if scaled > y { scaled - y } else { y - scaled + 1 }
}

@id("k.halve32")
fn halve32(x: i32) -> i32
{
    if x < 0i32 { 0i32 - x } else { x / 2i32 }
}

@id("k.byte_mix")
fn byte_mix(a: u8, b: u8) -> u8
{
    a * 2u8 + b / 7u8
}

@id("k.index_scale")
fn index_scale(n: usize) -> usize
{
    n * 4usize - 1usize
}

@id("k.flag")
fn flag(x: i64, keep: bool) -> bool
{
    (keep && x != 0) || (!keep && x == 0)
}

@id("k.ratio")
fn ratio(x: i64, y: i64) -> i64
{
    x / y
}

@id("k.sum")
fn sum(acc: i64, element: i64) -> i64
{
    acc + element
}

@id("k.count_small")
fn count_small(acc: usize, element: u8) -> usize
{
    if element < 10u8 { acc + 1usize } else { acc }
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;

fn resolve(source: &str) -> ResolvedProgram {
    let ast = crate::parse(source, "compute-kernels.spx").expect("fixture parses");
    crate::hir::resolve(&ast).expect("fixture resolves")
}

fn session() -> CpuReferenceSession {
    CpuReferenceSession::open(ComputeCapability::cpu_reference_all())
}

fn i64s(values: &[i64]) -> Vec<Scalar> {
    values.iter().copied().map(Scalar::I64).collect()
}

/// A fresh private source file for the file-based reference interpreter.
fn write_source(source: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "semaprax-cpu-reference-{}-{}.spx",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, source).expect("temporary kernel source is writable");
    path
}
