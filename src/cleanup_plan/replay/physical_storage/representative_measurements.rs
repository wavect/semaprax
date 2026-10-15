//! Paired constructor microphase on one authenticated, unchanged HIR snapshot.
//! This is not full replay, execution of the application, or an agent benchmark.
use super::{replay_reserve_vec, PhysicalRefusal, PhysicalReservation};
use crate::hir::lexical_exit::{child, ChildCursor};
use crate::hir::{ResolvedExpr, ResolvedExprKind as K, ResolvedProgram};
use crate::test_allocation_observer::{self, Metrics};
use std::time::Instant;

mod applications;
const BACKING: usize = 67_108_864;
const WORK: usize = 32_000_000;
const DEPTH: usize = crate::cache_codec::MAX_DEPTH + 1;
const SAMPLES: usize = 7;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Strategy {
    IndexedExact,
    CursorExact,
    IndexedGeometric,
    CursorGeometric,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    Work,
    Depth,
    Backing(PhysicalRefusal),
}
#[derive(Default)]
struct Work {
    remaining: usize,
    spent: usize,
}
impl Work {
    fn debit(&mut self, count: usize) -> Result<(), Refusal> {
        self.remaining = self.remaining.checked_sub(count).ok_or(Refusal::Work)?;
        self.spent = self.spent.checked_add(count).ok_or(Refusal::Work)?;
        Ok(())
    }
}
// Saved indexed prefix algorithm: count each actually examined statement/arm,
// not a width multiplier substituted for the original operations.
fn indexed<'a>(
    e: &'a ResolvedExpr,
    index: usize,
    work: &mut Work,
) -> Result<Option<&'a ResolvedExpr>, Refusal> {
    work.debit(1)?;
    match &e.kind {
        K::Block { statements, tail }
        | K::VecConstructor {
            statements, tail, ..
        } => {
            let mut offset = 0;
            for statement in statements {
                work.debit(1)?;
                let count = statement.child_count();
                if index < offset + count {
                    return Ok(statement.child(index - offset));
                }
                offset += count;
            }
            Ok((index == offset).then_some(tail.as_ref()))
        }
        K::Match {
            scrutinee, arms, ..
        } => {
            if index == 0 {
                return Ok(Some(scrutinee));
            }
            let mut offset = 1;
            for arm in arms {
                work.debit(1)?;
                if let Some(guard) = &arm.guard {
                    if index == offset {
                        return Ok(Some(guard));
                    }
                    offset += 1;
                }
                if index == offset {
                    return Ok(Some(&arm.value));
                }
                offset += 1;
            }
            Ok(None)
        }
        _ => Ok(child(e, index)),
    }
}
#[derive(Clone, Copy)]
struct Frame<'a> {
    expression: &'a ResolvedExpr,
    cursor: ChildCursor<'a>,
    index: usize,
    entered: bool,
}
fn push<'a>(
    values: &mut Vec<&'a ResolvedExpr>,
    value: &'a ResolvedExpr,
    strategy: Strategy,
    physical: &mut PhysicalReservation,
) -> Result<(), Refusal> {
    match strategy {
        Strategy::IndexedGeometric | Strategy::CursorGeometric => {
            replay_reserve_vec(physical, values, 1)
        }
        Strategy::IndexedExact | Strategy::CursorExact => physical.reserve(0).and_then(|()| {
            let required = values
                .len()
                .checked_add(1)
                .ok_or(PhysicalRefusal::Overflow)?;
            if required > values.capacity() {
                physical.reserve_array::<&ResolvedExpr>(required)?;
                values
                    .try_reserve_exact(required - values.len())
                    .map_err(|_| PhysicalRefusal::LocalLimit)?;
                if values.capacity() != required {
                    return Err(PhysicalRefusal::LocalLimit);
                }
            }
            Ok(())
        }),
    }
    .map_err(Refusal::Backing)?;
    values.push(value);
    Ok(())
}
struct Outcome<'a> {
    values: Vec<&'a ResolvedExpr>,
    result: Result<(), Refusal>,
    work: usize,
    charged: usize,
}
fn inventory(
    program: &ResolvedProgram,
    strategy: Strategy,
    work_limit: usize,
    backing_limit: usize,
) -> Outcome<'_> {
    let mut physical = PhysicalReservation::new(backing_limit);
    let mut work = Work {
        remaining: work_limit,
        spent: 0,
    };
    let mut values = Vec::new();
    let result = (|| {
        // Same fixed frames for both algorithms. Charge their actual carrier
        // before constructing them; observer highwater excludes stack bytes.
        physical
            .reserve_array::<Option<Frame<'_>>>(DEPTH)
            .map_err(Refusal::Backing)?;
        let mut frames = [None; DEPTH];
        for function in program.functions.iter().chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        ) {
            for root in function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
            {
                frames[0] = Some(Frame {
                    expression: root,
                    cursor: ChildCursor::new(root),
                    index: 0,
                    entered: false,
                });
                let mut depth = 1;
                while depth != 0 {
                    let frame = frames[depth - 1].as_mut().unwrap();
                    if !frame.entered {
                        work.debit(1)?;
                        push(&mut values, frame.expression, strategy, &mut physical)?;
                        frame.entered = true;
                        if matches!(strategy, Strategy::CursorExact | Strategy::CursorGeometric) {
                            // A single real statement census, as in selected
                            // walkers, plus constant cursor advance charges.
                            if let K::Block { statements, .. }
                            | K::VecConstructor { statements, .. } = &frame.expression.kind
                            {
                                for _ in statements {
                                    work.debit(1)?;
                                }
                            }
                        }
                    }
                    let next = match strategy {
                        Strategy::IndexedExact | Strategy::IndexedGeometric => {
                            indexed(frame.expression, frame.index, &mut work)?
                        }
                        Strategy::CursorExact | Strategy::CursorGeometric => {
                            work.debit(1)?;
                            frame.cursor.next()
                        }
                    };
                    if let Some(expression) = next {
                        frame.index += 1;
                        if depth == DEPTH {
                            return Err(Refusal::Depth);
                        }
                        frames[depth] = Some(Frame {
                            expression,
                            cursor: ChildCursor::new(expression),
                            index: 0,
                            entered: false,
                        });
                        depth += 1;
                    } else {
                        frames[depth - 1] = None;
                        depth -= 1;
                    }
                }
            }
        }
        Ok(())
    })();
    Outcome {
        values,
        result,
        work: work.spent,
        charged: physical.charged(),
    }
}
#[derive(Clone, Copy, Debug)]
struct Sample {
    nanos: u64,
    metrics: Metrics,
    work: usize,
    charged: usize,
    count: usize,
    result: Result<(), Refusal>,
}
fn sample(program: &ResolvedProgram, strategy: Strategy) -> Sample {
    let start = Instant::now();
    let timed = inventory(program, strategy, WORK, BACKING);
    let expected = (timed.work, timed.charged, timed.values.len(), timed.result);
    drop(timed);
    let nanos = u64::try_from(start.elapsed().as_nanos()).unwrap();
    let ((work, charged, count, result), metrics) = test_allocation_observer::measure(|| {
        let outcome = inventory(program, strategy, WORK, BACKING);
        (
            outcome.work,
            outcome.charged,
            outcome.values.len(),
            outcome.result,
        )
        // Outcome drops here, so all observed phase backing is released.
    });
    assert_eq!((work, charged, count, result), expected);
    Sample {
        nanos,
        metrics,
        work,
        charged,
        count,
        result,
    }
}
fn compare(program: &ResolvedProgram) {
    // Equality is outside timing/allocation observation; no equality-map
    // allocation is credited to either measured phase.
    let old = inventory(program, Strategy::IndexedExact, WORK, BACKING);
    for strategy in [
        Strategy::CursorExact,
        Strategy::IndexedGeometric,
        Strategy::CursorGeometric,
    ] {
        let candidate = inventory(program, strategy, WORK, BACKING);
        for (a, b) in old.values.iter().zip(&candidate.values) {
            assert!(std::ptr::eq(*a, *b));
        }
        if old.result.is_ok() && candidate.result.is_ok() {
            assert_eq!(old.values.len(), candidate.values.len());
        }
    }
}
fn report(name: &str, source_digest: &str, program: &ResolvedProgram) {
    compare(program);
    let strategies = [
        Strategy::IndexedExact,
        Strategy::CursorExact,
        Strategy::IndexedGeometric,
        Strategy::CursorGeometric,
    ];
    let mut observations = [[None; SAMPLES]; 4];
    // Rotate the first strategy; the timed pass is uninstrumented. Its exact
    // counters/verdict must equal the separately allocator-observed pass.
    #[allow(
        clippy::needless_range_loop,
        reason = "Preserve the rotated sample order across the four strategy columns."
    )]
    for sample_index in 0..SAMPLES {
        for offset in 0..4 {
            let index = (sample_index + offset) % 4;
            observations[index][sample_index] = Some(sample(program, strategies[index]));
        }
    }
    for (strategy, observations) in strategies.into_iter().zip(observations) {
        let samples = observations.map(Option::unwrap);
        let mut times = samples.map(|s| s.nanos);
        times.sort_unstable();
        let first = samples[0];
        for current in samples {
            assert_eq!(
                (
                    current.work,
                    current.charged,
                    current.count,
                    current.result,
                    current.metrics
                ),
                (
                    first.work,
                    first.charged,
                    first.count,
                    first.result,
                    first.metrics
                )
            );
            assert!(!current.metrics.invalid);
            assert_eq!(current.metrics.live, 0);
            assert_eq!(
                current.metrics.allocated + DEPTH * std::mem::size_of::<Option<Frame<'_>>>(),
                current.charged
            );
        }
        eprintln!(
            "{}",
            serde_json::json!({
                "schema":"semaprax.replay-constructor-measurement.v1", "application":name,
                "effective_source_sha256":source_digest, "strategy":format!("{strategy:?}"),
                "scope":"borrowed-authored-inventory-construction; not full replay or application runtime",
                "admission":format!("{:?}",first.result), "completed_nodes":first.count,
                "constructor_work":first.work, "cumulative_reserved_bytes":first.charged,
                "requested_heap_bytes":first.metrics.allocated, "heap_highwater_bytes":first.metrics.highwater,
                "heap_live_after_drop":first.metrics.live, "wall_ns_samples":samples.map(|s| s.nanos),
                "wall_ns_min_median_max":[times[0],times[SAMPLES/2],times[SAMPLES-1]],
                "work_limit":WORK,"backing_limit":BACKING,
                "measurement_context":"wall pass uses ordinary allocator; separate thread-bound replacement-overlap Layout pass; fixed observer storage excluded"
            })
        );
    }
}
#[test]
fn independent_order_and_exact_one_short_constructor_work_and_backing() {
    let source = "module measurement; @id(\"probe\") fn probe(n:i64)->i64 requires n>=0 ensures result>=0 {let mut a=0;while a<n {a=a+1;0} if a>0 {a}else{0}} @id(\"main\") fn main()->i64 {probe(4)}";
    let program = crate::hir::resolve(&crate::check(source, "measurement.spx").unwrap()).unwrap();
    compare(&program);
    for strategy in [
        Strategy::IndexedExact,
        Strategy::CursorExact,
        Strategy::IndexedGeometric,
        Strategy::CursorGeometric,
    ] {
        let baseline = inventory(&program, strategy, WORK, BACKING);
        assert!(baseline.result.is_ok());
        let exact = inventory(&program, strategy, baseline.work, baseline.charged);
        assert_eq!(exact.result, Ok(()));
        assert_eq!(exact.values.len(), baseline.values.len());
        assert_eq!(
            inventory(&program, strategy, baseline.work - 1, baseline.charged).result,
            Err(Refusal::Work)
        );
        assert!(matches!(
            inventory(&program, strategy, baseline.work, baseline.charged - 1).result,
            Err(Refusal::Backing(PhysicalRefusal::LocalLimit))
        ));
    }
}
#[test]
#[ignore = "focused opt-in measurement owner; run explicitly with --ignored --nocapture"]
fn unchanged_applications_report_paired_constructor_work_backing_and_wall_samples() {
    applications::run(report);
}

#[test]
fn wide_same_source_separates_cursor_work_from_growth_backing() {
    let mut source = String::from("module measurement.wide; @id(\"main\") fn main()->i64 { ");
    for index in 0..64 {
        source.push_str(&format!("let value{index}={index};"));
    }
    source.push_str("value63}");
    let program =
        crate::hir::resolve(&crate::check(&source, "measurement-wide.spx").unwrap()).unwrap();
    let baseline = inventory(&program, Strategy::IndexedExact, WORK, BACKING);
    let cursor = inventory(&program, Strategy::CursorExact, WORK, BACKING);
    let growth = inventory(&program, Strategy::IndexedGeometric, WORK, BACKING);
    let both = inventory(&program, Strategy::CursorGeometric, WORK, BACKING);
    for candidate in [&cursor, &growth, &both] {
        assert_eq!(candidate.result, Ok(()));
        assert_eq!(candidate.values.len(), baseline.values.len());
        for (a, b) in candidate.values.iter().zip(&baseline.values) {
            assert!(std::ptr::eq(*a, *b));
        }
    }
    assert_eq!(baseline.charged, cursor.charged);
    assert_eq!(growth.charged, both.charged);
    assert_eq!(baseline.work, growth.work);
    assert_eq!(cursor.work, both.work);
    assert!(cursor.work < baseline.work);
    assert!(growth.charged < baseline.charged);
    for strategy in [
        Strategy::IndexedExact,
        Strategy::CursorExact,
        Strategy::IndexedGeometric,
        Strategy::CursorGeometric,
    ] {
        let observed = sample(&program, strategy);
        assert!(!observed.metrics.invalid);
        assert_eq!(observed.metrics.live, 0);
        assert!(observed.metrics.highwater <= observed.charged);
    }
}
