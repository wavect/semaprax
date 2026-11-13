#![allow(
    clippy::duplicate_mod,
    reason = "The complete fixture intentionally compiles independently against current and predecessor owner types."
)]
//! Test-scoped observation only. No proof debits, diagnostics or verdicts change.
use super::*;
use std::cell::Cell;

#[derive(Clone, Copy, Default)]
struct Stats {
    nodes: usize,
    edges: usize,
    visits: [usize; 3],
    frames: [usize; 4],
    hits: usize,
    misses: usize,
    pairs: usize,
    dags: usize,
}
thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
    static STATS: Cell<Stats> = Cell::new(Stats::default());
}
fn update(action: impl FnOnce(&mut Stats)) {
    if ACTIVE.with(Cell::get) {
        STATS.with(|cell| {
            let mut stats = cell.get();
            action(&mut stats);
            cell.set(stats);
        });
    }
}
pub(super) fn begin(function: &ResolvedFunction, budget: &ReplayBudget) {
    if ACTIVE.with(Cell::get) {
        STATS.with(|cell| cell.set(Stats::default()));
        phase(function, budget, "begin", None);
    }
}
pub(super) fn phase(
    function: &ResolvedFunction,
    budget: &ReplayBudget,
    phase: &str,
    paths: Option<&[SkeletonPath]>,
) {
    if !ACTIVE.with(Cell::get) {
        return;
    }
    let stats = STATS.with(Cell::get);
    let mut terminals = [0usize; 3];
    if let Some(paths) = paths {
        for path in paths {
            terminals[match path.terminal {
                SkeletonTerminal::Success => 0,
                SkeletonTerminal::Failure => 1,
                SkeletonTerminal::LoopLatch => 2,
            }] += 1;
        }
    }
    eprintln!("factored-observer {} phase={} free={} held={} spent={} paths={:?} nodes={} edges={} visits(seq,cat,alt)={:?} frames(visit,seq,cat,alt)={:?} hits={} misses={} pairs={} distinct_dags={}",function.id,phase,budget.remaining,budget.skeleton_remaining,budget.skeleton_spent,terminals,stats.nodes,stats.edges,stats.visits,stats.frames,stats.hits,stats.misses,stats.pairs,stats.dags);
}
pub(super) fn node() {
    update(|stats| stats.nodes += 1);
}
pub(super) fn edge() {
    update(|stats| stats.edges += 1);
}
pub(super) fn visit(dag: &ObservationDag, hit: bool) {
    update(|stats| {
        if hit {
            stats.hits += 1;
        } else {
            stats.misses += 1;
        }
        stats.visits[match dag.kind {
            ObservationDagKind::Seq(..) => 0,
            ObservationDagKind::Cat(..) => 1,
            ObservationDagKind::Alt(..) => 2,
        }] += 1;
    });
}
pub(super) fn memo(key: (usize, usize), memo: &BTreeMap<(usize, usize), (usize, usize)>) {
    update(|stats| {
        if !memo.contains_key(&key) {
            stats.pairs += 1;
            if memo
                .range((key.0, 0)..=(key.0, usize::MAX))
                .next()
                .is_none()
            {
                stats.dags += 1;
            }
        }
    });
}
struct Scope(bool);
impl Scope {
    fn new() -> Self {
        Self(ACTIVE.with(|active| active.replace(true)))
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        ACTIVE.with(|active| active.set(self.0));
    }
}

#[test]
fn original_shiftsim_factored_work_phase_witness() {
    let _scope = Scope::new();
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let example = repository.join("examples/shiftsim-typed-record-successor");
    let root = std::env::temp_dir().join(format!(
        "semaprax-original-shiftsim-phase-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join("fixtures")).unwrap();
    std::fs::copy(example.join("semaprax.toml"), root.join("semaprax.toml")).unwrap();
    for directory in ["src", "fixtures"] {
        for entry in std::fs::read_dir(example.join(directory)).unwrap() {
            let entry = entry.unwrap();
            assert!(entry.file_type().unwrap().is_file());
            std::fs::copy(entry.path(), root.join(directory).join(entry.file_name())).unwrap();
        }
    }
    let controls = repository.join("tests/project/standard_library/application_json/owned");
    for (target, original) in [
        (
            "model.spx",
            "string_slot_adoption/shiftsim_model_wrapper.txt",
        ),
        (
            "order.spx",
            "string_slot_adoption/shiftsim_order_wrapper.txt",
        ),
        ("schedule.spx", "shiftsim_schedule_unfactored.txt"),
    ] {
        std::fs::copy(controls.join(original), root.join("src").join(target)).unwrap();
    }
    let original = std::fs::read(controls.join("shiftsim_schedule_unfactored.txt")).unwrap();
    let verdict =
        crate::project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
            crate::hir::validate(snapshot.public_api_program()).map_err(|error| vec![error])
        });
    assert_eq!(
        std::fs::read(root.join("src/schedule.spx")).unwrap(),
        original
    );
    std::fs::remove_dir_all(root).unwrap();
    verdict.expect("unchanged original ShiftSim must pass exact factored replay");
}

pub(super) fn frame(frame: &BuildFrame<'_>) {
    update(|stats| {
        stats.frames[match frame {
            BuildFrame::Visit(..) => 0,
            BuildFrame::Seq(..) => 1,
            BuildFrame::Cat(..) => 2,
            BuildFrame::Alt(..) => 3,
        }] += 1
    });
}
