//! Private executable evidence for resource-bearing aggregate cleanup.
//!
//! Public native resource admission remains closed by SPX-B104. This module is
//! test-only proof scaffolding: it authenticates cleanup orders from resolved
//! plans, lowers one frozen action program to C, and compares its effects with
//! the sibling Wasm test lowering of the same action program.

use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest as _, Sha256};

use crate::aggregate_layout::{AggregateLayout, AggregateTarget};
use crate::cleanup_plan::{
    CleanupPlace, CleanupTransition, ExitContinuation, StatusProducer, StorageId,
};
use crate::hir::{
    self, DeclarationId, ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedStatement,
};
use crate::parse;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

pub(crate) const RESOURCE_SOURCE: &str = r#"
module test.aggregate_resource_private;
@id("token.type") resource Token { @id("token.drop") drop trivial; }
@id("pair.type") record Pair {
    @id("pair.first") first: Token,
    @id("pair.second") second: Token,
}
@id("inner.type") record Inner {
    @id("inner.left") left: Token,
    @id("inner.right") right: Token,
}
@id("outer.type") record Outer {
    @id("outer.inner") inner: Inner,
    @id("outer.tail") tail: Token,
}
@id("token.identity") fn identity(value: own Token) -> Token { value }
@id("pair.update") fn update(pair: own Pair, second: own Token) -> Pair {
    pair with { second: second }
}
@id("pair.partial") fn partial(
    pair: own Pair,
    first: own Token,
    second: own Token
) -> Pair {
    pair with { second: second, first: identity(first) }
}
@id("outer.update") fn update_outer(outer: own Outer, inner: own Inner) -> Outer {
    outer with { inner: inner }
}
@id("pair.take") fn take(first: own Token, second: own Token) -> Token {
    let pair = Pair { second: second, first: first };
    pair.first
}
@id("app.main") fn main() -> i64 { 0 }
"#;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum HarnessStorage {
    PairParam,
    PairBase,
    PairReplacementParam,
    PairUpdate,
    PairBody,
    PairProvisional,
    PairCaller,
    OuterParam,
    OuterBase,
    OuterReplacementParam,
    OuterUpdate,
    OuterBody,
    OuterProvisional,
    OuterCaller,
    PartialParam,
    PartialBase,
    PartialFirstParam,
    PartialSecondParam,
    PartialUpdate,
    PartialCallArgument,
    PartialCalleeArgument,
    TakeFirstParam,
    TakeSecondParam,
    TakeConstruct,
    TakeLocal,
    TakeBody,
    TakeProvisional,
    TakeCaller,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum HarnessLeaf {
    First,
    Second,
    Left,
    Right,
    Tail,
    Scalar,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct HarnessSlot {
    pub(crate) storage: HarnessStorage,
    pub(crate) leaf: HarnessLeaf,
}

const fn slot(storage: HarnessStorage, leaf: HarnessLeaf) -> HarnessSlot {
    HarnessSlot { storage, leaf }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HarnessAction {
    Store(HarnessSlot, u32),
    Transfer(HarnessSlot, HarnessSlot),
    Finalize(HarnessSlot),
    PoisonPartialResult,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResourceHarnessScenario {
    pub(crate) actions: Vec<HarnessAction>,
    pub(crate) expected_trace: Vec<u32>,
}

impl ResourceHarnessScenario {
    pub(crate) fn validate(&self) -> Result<(), String> {
        let trace = execute_actions(&self.actions)?;
        let digest = action_digest(&self.actions);
        if digest != "26d09d418c249e5a94567758be17ace6b26b58964e16a2223d59ae8db9b442ea" {
            return Err(format!(
                "private aggregate action program differs from the frozen cleanup-plan projection: {digest}"
            ));
        }
        if trace != self.expected_trace {
            return Err(
                "private aggregate expected trace is not derived from its action program".into(),
            );
        }
        Ok(())
    }
}

fn action_digest(actions: &[HarnessAction]) -> String {
    let mut digest = Sha256::new();
    for action in actions {
        digest.update(format!("{action:?}\n"));
    }
    let mut output = String::with_capacity(64);
    for byte in digest.finalize() {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

pub(crate) fn resource_harness_scenario() -> ResourceHarnessScenario {
    let parsed = parse(RESOURCE_SOURCE, Path::new("aggregate-resource-private.spx")).unwrap();
    let program = hir::resolve(&parsed).unwrap();
    for target in [AggregateTarget::Native64, AggregateTarget::Wasm32] {
        let pair = AggregateLayout::for_record(&program, target, &DeclarationId::new("pair.type"))
            .unwrap();
        pair.validate(&program).unwrap();
        assert_eq!(
            pair.fields
                .iter()
                .map(|field| field.field.as_str())
                .collect::<Vec<_>>(),
            ["pair.first", "pair.second"]
        );
        let inner =
            AggregateLayout::for_record(&program, target, &DeclarationId::new("inner.type"))
                .unwrap();
        inner.validate(&program).unwrap();
        assert_eq!(
            inner
                .fields
                .iter()
                .map(|field| field.field.as_str())
                .collect::<Vec<_>>(),
            ["inner.left", "inner.right"]
        );
        let outer =
            AggregateLayout::for_record(&program, target, &DeclarationId::new("outer.type"))
                .unwrap();
        outer.validate(&program).unwrap();
        assert_eq!(
            outer
                .fields
                .iter()
                .map(|field| field.field.as_str())
                .collect::<Vec<_>>(),
            ["outer.inner", "outer.tail"]
        );
    }

    let pair_function = function(&program.functions, "pair.update");
    authenticate_update_transfers(pair_function, "pair.first", "pair.second");
    authenticate_result_chain(pair_function);
    let pair_cleanup = continuing_cleanup(pair_function);
    assert_eq!(pair_cleanup, ["pair.second"]);
    let outer_function = function(&program.functions, "outer.update");
    authenticate_update_transfers(outer_function, "outer.tail", "outer.inner");
    authenticate_result_chain(outer_function);
    let outer_cleanup = continuing_cleanup(outer_function);
    assert_eq!(
        outer_cleanup,
        ["outer.inner/inner.right", "outer.inner/inner.left"]
    );
    let partial_function = function(&program.functions, "pair.partial");
    authenticate_update_transfers(partial_function, "pair.first", "pair.second");
    authenticate_partial_call(partial_function);
    let partial_cleanup = exact_exit_cleanup(
        partial_function,
        &["pair.second", "pair.second", "pair.first"],
        ExpectedExit::PropagatedFailure("token.identity"),
    );
    let take_function = function(&program.functions, "pair.take");
    authenticate_take_transfers(take_function);
    let take_cleanup =
        exact_exit_cleanup(take_function, &["pair.second"], ExpectedExit::CommitResult);

    use HarnessAction::{Finalize, PoisonPartialResult, Store, Transfer};
    use HarnessLeaf::{First, Left, Right, Scalar, Second, Tail};
    use HarnessStorage::*;
    let s = slot;
    let mut actions = vec![
        Store(s(PairParam, First), 11),
        Store(s(PairParam, Second), 12),
        Store(s(PairReplacementParam, Scalar), 22),
        Transfer(s(PairParam, First), s(PairBase, First)),
        Transfer(s(PairParam, Second), s(PairBase, Second)),
        Transfer(s(PairReplacementParam, Scalar), s(PairUpdate, Second)),
        Transfer(s(PairBase, First), s(PairUpdate, First)),
    ];
    append_cleanup(
        &mut actions,
        &pair_cleanup,
        &[("pair.second", s(PairBase, Second))],
    );
    actions.extend([
        Transfer(s(PairUpdate, First), s(PairBody, First)),
        Transfer(s(PairUpdate, Second), s(PairBody, Second)),
        Transfer(s(PairBody, First), s(PairProvisional, First)),
        Transfer(s(PairBody, Second), s(PairProvisional, Second)),
        Transfer(s(PairProvisional, First), s(PairCaller, First)),
        Transfer(s(PairProvisional, Second), s(PairCaller, Second)),
        Finalize(s(PairCaller, Second)),
        Finalize(s(PairCaller, First)),
        Store(s(OuterParam, Left), 31),
        Store(s(OuterParam, Right), 32),
        Store(s(OuterParam, Tail), 33),
        Store(s(OuterReplacementParam, Left), 41),
        Store(s(OuterReplacementParam, Right), 42),
        Transfer(s(OuterParam, Left), s(OuterBase, Left)),
        Transfer(s(OuterParam, Right), s(OuterBase, Right)),
        Transfer(s(OuterParam, Tail), s(OuterBase, Tail)),
        Transfer(s(OuterReplacementParam, Left), s(OuterUpdate, Left)),
        Transfer(s(OuterReplacementParam, Right), s(OuterUpdate, Right)),
        Transfer(s(OuterBase, Tail), s(OuterUpdate, Tail)),
    ]);
    append_cleanup(
        &mut actions,
        &outer_cleanup,
        &[
            ("outer.inner/inner.right", s(OuterBase, Right)),
            ("outer.inner/inner.left", s(OuterBase, Left)),
        ],
    );
    actions.extend([
        Transfer(s(OuterUpdate, Left), s(OuterBody, Left)),
        Transfer(s(OuterUpdate, Right), s(OuterBody, Right)),
        Transfer(s(OuterUpdate, Tail), s(OuterBody, Tail)),
        Transfer(s(OuterBody, Left), s(OuterProvisional, Left)),
        Transfer(s(OuterBody, Right), s(OuterProvisional, Right)),
        Transfer(s(OuterBody, Tail), s(OuterProvisional, Tail)),
        Transfer(s(OuterProvisional, Left), s(OuterCaller, Left)),
        Transfer(s(OuterProvisional, Right), s(OuterCaller, Right)),
        Transfer(s(OuterProvisional, Tail), s(OuterCaller, Tail)),
        Finalize(s(OuterCaller, Tail)),
        Finalize(s(OuterCaller, Right)),
        Finalize(s(OuterCaller, Left)),
        Store(s(PartialParam, First), 51),
        Store(s(PartialParam, Second), 52),
        Store(s(PartialFirstParam, Scalar), 61),
        Store(s(PartialSecondParam, Scalar), 62),
        Transfer(s(PartialParam, First), s(PartialBase, First)),
        Transfer(s(PartialParam, Second), s(PartialBase, Second)),
        Transfer(s(PartialSecondParam, Scalar), s(PartialUpdate, Second)),
        Transfer(s(PartialFirstParam, Scalar), s(PartialCallArgument, Scalar)),
        Transfer(
            s(PartialCallArgument, Scalar),
            s(PartialCalleeArgument, Scalar),
        ),
        Finalize(s(PartialCalleeArgument, Scalar)),
        PoisonPartialResult,
    ]);
    append_cleanup(
        &mut actions,
        &partial_cleanup,
        &[
            ("pair.second", s(PartialUpdate, Second)),
            ("pair.second", s(PartialBase, Second)),
            ("pair.first", s(PartialBase, First)),
        ],
    );
    actions.extend([
        Store(s(TakeFirstParam, Scalar), 71),
        Store(s(TakeSecondParam, Scalar), 72),
        Transfer(s(TakeSecondParam, Scalar), s(TakeConstruct, Second)),
        Transfer(s(TakeFirstParam, Scalar), s(TakeConstruct, First)),
        Transfer(s(TakeConstruct, First), s(TakeLocal, First)),
        Transfer(s(TakeConstruct, Second), s(TakeLocal, Second)),
        Transfer(s(TakeLocal, First), s(TakeBody, Scalar)),
        Transfer(s(TakeBody, Scalar), s(TakeProvisional, Scalar)),
    ]);
    append_cleanup(
        &mut actions,
        &take_cleanup,
        &[("pair.second", s(TakeLocal, Second))],
    );
    actions.extend([
        Transfer(s(TakeProvisional, Scalar), s(TakeCaller, Scalar)),
        Finalize(s(TakeCaller, Scalar)),
    ]);

    let expected_trace = execute_actions(&actions).unwrap();
    let scenario = ResourceHarnessScenario {
        actions,
        expected_trace,
    };
    scenario.validate().unwrap();
    scenario
}

fn append_cleanup(
    actions: &mut Vec<HarnessAction>,
    actual: &[String],
    mapping: &[(&str, HarnessSlot)],
) {
    assert_eq!(actual.len(), mapping.len());
    for (path, (expected_path, slot)) in actual.iter().zip(mapping) {
        assert_eq!(path, expected_path);
        actions.push(HarnessAction::Finalize(*slot));
    }
}

fn function<'a>(functions: &'a [ResolvedFunction], id: &str) -> &'a ResolvedFunction {
    functions
        .iter()
        .find(|function| function.id.as_str() == id)
        .unwrap()
}

fn paths(exit: &crate::cleanup_plan::ExitTarget) -> Vec<String> {
    exit.finalize_in_order
        .iter()
        .map(|action| {
            action
                .source
                .projections
                .iter()
                .map(|item| item.as_str())
                .collect::<Vec<_>>()
                .join("/")
        })
        .collect()
}

fn exact_exit_cleanup(
    function: &ResolvedFunction,
    expected: &[&str],
    expected_exit: ExpectedExit<'_>,
) -> Vec<String> {
    let matches = function
        .cleanup_plan
        .exits
        .iter()
        .filter(|exit| {
            let continuation_matches = match (&expected_exit, &exit.continuation) {
                (ExpectedExit::CommitResult, ExitContinuation::CommitResult { .. }) => true,
                (
                    ExpectedExit::PropagatedFailure(expected_callee),
                    ExitContinuation::ReturnFailure { source },
                ) => function.cleanup_plan.status_sources.iter().any(|status| {
                    status.id == *source
                        && matches!(
                            &status.producer,
                            StatusProducer::PropagatedCall { callee }
                                if callee.as_str() == *expected_callee
                        )
                }),
                _ => false,
            };
            continuation_matches && paths(exit) == expected
        })
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "expected one exact cleanup exit");
    paths(matches[0])
}

enum ExpectedExit<'a> {
    CommitResult,
    PropagatedFailure(&'a str),
}

fn continuing_cleanup(function: &ResolvedFunction) -> Vec<String> {
    let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
        panic!("expected block")
    };
    let ResolvedExprKind::UpdateRecord { base, .. } = &tail.kind else {
        panic!("expected update")
    };
    let storage = StorageId::Temporary(base.id.clone());
    let matches = function
        .cleanup_plan
        .exits
        .iter()
        .filter(|exit| {
            matches!(exit.continuation, ExitContinuation::Continue(_))
                && exit
                    .finalize_in_order
                    .iter()
                    .any(|action| action.source.storage == storage)
        })
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "expected one update continuation cleanup");
    paths(matches[0])
}

fn place(storage: StorageId, projections: &[&str]) -> CleanupPlace {
    CleanupPlace {
        storage,
        projections: projections
            .iter()
            .map(|projection| DeclarationId::new(*projection))
            .collect(),
    }
}

fn assert_transfer(
    function: &ResolvedFunction,
    at: &ResolvedExpr,
    source: CleanupPlace,
    destination: CleanupPlace,
) {
    let matches = function
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
        .filter(|transition| {
            matches!(
                transition,
                CleanupTransition::Transfer {
                    at: actual_at,
                    source: actual_source,
                    destination: actual_destination,
                } if actual_at == &at.id
                    && actual_source == &source
                    && actual_destination == &destination
            )
        })
        .count();
    assert_eq!(
        matches, 1,
        "expected one exact authenticated cleanup transfer at {:?} from {:?} to {:?}",
        at.id, source, destination
    );
}

fn update_parts(
    function: &ResolvedFunction,
) -> (
    &ResolvedExpr,
    &ResolvedExpr,
    &[crate::hir::ResolvedFieldInitializer],
) {
    let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
        panic!("expected block")
    };
    let ResolvedExprKind::UpdateRecord { base, fields, .. } = &tail.kind else {
        panic!("expected update")
    };
    (tail, base, fields)
}

fn authenticate_update_transfers(
    function: &ResolvedFunction,
    untouched_field: &str,
    replacement_field: &str,
) {
    let (update, base, fields) = update_parts(function);
    let replacement = fields
        .iter()
        .find(|field| field.field.as_str() == replacement_field)
        .unwrap();
    assert_transfer(
        function,
        base,
        place(StorageId::Value(function.params[0].id.clone()), &[]),
        place(StorageId::Temporary(base.id.clone()), &[]),
    );
    let ResolvedExprKind::Place(replacement_place) = &replacement.value.kind else {
        panic!("expected direct replacement place")
    };
    assert_transfer(
        function,
        &replacement.value,
        place(StorageId::Value(replacement_place.root.clone()), &[]),
        place(
            StorageId::Temporary(update.id.clone()),
            &[replacement_field],
        ),
    );
    if fields
        .iter()
        .all(|field| field.field.as_str() != untouched_field)
    {
        assert_transfer(
            function,
            update,
            place(StorageId::Temporary(base.id.clone()), &[untouched_field]),
            place(StorageId::Temporary(update.id.clone()), &[untouched_field]),
        );
    }
}

fn authenticate_result_chain(function: &ResolvedFunction) {
    let (update, _, _) = update_parts(function);
    assert_transfer(
        function,
        &function.body,
        place(StorageId::Temporary(update.id.clone()), &[]),
        place(StorageId::Temporary(function.body.id.clone()), &[]),
    );
    assert_transfer(
        function,
        &function.body,
        place(StorageId::Temporary(function.body.id.clone()), &[]),
        place(StorageId::ProvisionalResult, &[]),
    );
}

fn authenticate_partial_call(function: &ResolvedFunction) {
    let (update, _, fields) = update_parts(function);
    let first = fields
        .iter()
        .find(|field| field.field.as_str() == "pair.first")
        .unwrap();
    let ResolvedExprKind::Call { args, .. } = &first.value.kind else {
        panic!("expected propagated replacement call")
    };
    let ResolvedExprKind::Place(argument_place) = &args[0].kind else {
        panic!("expected owned place call argument")
    };
    let call_argument = StorageId::CallArgument {
        call: first.value.id.clone(),
        parameter_index: 0,
        value_expression: args[0].id.clone(),
    };
    assert_transfer(
        function,
        &args[0],
        place(StorageId::Value(argument_place.root.clone()), &[]),
        place(call_argument.clone(), &[]),
    );
    let commits = function
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
        .filter(|transition| {
            matches!(
                transition,
                CleanupTransition::CallCommit { call, arguments }
                    if call == &first.value.id
                        && arguments.len() == 1
                        && arguments[0].parameter_index == 0
                        && arguments[0].source == place(call_argument.clone(), &[])
            )
        })
        .count();
    assert_eq!(commits, 1, "expected exact authenticated owned call commit");
    assert_eq!(update.ty, function.return_type);
}

fn authenticate_take_transfers(function: &ResolvedFunction) {
    let ResolvedExprKind::Block { statements, .. } = &function.body.kind else {
        panic!("expected take block")
    };
    let ResolvedStatement::Let { binding, value, .. } = &statements[0];
    let ResolvedExprKind::ConstructRecord { fields, .. } = &value.kind else {
        panic!("expected record construction")
    };
    for (parameter_index, field_name) in [(1_usize, "pair.second"), (0, "pair.first")] {
        let initializer = fields
            .iter()
            .find(|field| field.field.as_str() == field_name)
            .unwrap();
        assert_transfer(
            function,
            &initializer.value,
            place(
                StorageId::Value(function.params[parameter_index].id.clone()),
                &[],
            ),
            place(StorageId::Temporary(value.id.clone()), &[field_name]),
        );
    }
    assert_transfer(
        function,
        value,
        place(StorageId::Temporary(value.id.clone()), &[]),
        place(StorageId::Value(binding.id.clone()), &[]),
    );
    assert_transfer(
        function,
        &function.body,
        place(StorageId::Value(binding.id.clone()), &["pair.first"]),
        place(StorageId::Temporary(function.body.id.clone()), &[]),
    );
    assert_transfer(
        function,
        &function.body,
        place(StorageId::Temporary(function.body.id.clone()), &[]),
        place(StorageId::ProvisionalResult, &[]),
    );
}

fn execute_actions(actions: &[HarnessAction]) -> Result<Vec<u32>, String> {
    let mut values = std::collections::HashMap::<HarnessSlot, Option<u32>>::new();
    let mut trace = Vec::new();
    for action in actions {
        match *action {
            HarnessAction::Store(slot, value) => {
                if values.insert(slot, Some(value)).flatten().is_some() {
                    return Err("private aggregate store overwrote a live owner".into());
                }
            }
            HarnessAction::Transfer(source, destination) => {
                let value = values
                    .insert(source, None)
                    .flatten()
                    .ok_or_else(|| "private aggregate transfer read a dead source".to_string())?;
                if values.insert(destination, Some(value)).flatten().is_some() {
                    return Err("private aggregate transfer overwrote a live destination".into());
                }
            }
            HarnessAction::Finalize(slot) => {
                let value = values
                    .insert(slot, None)
                    .flatten()
                    .ok_or_else(|| "private aggregate finalizer read a dead source".to_string())?;
                trace.push(value);
            }
            HarnessAction::PoisonPartialResult => {}
        }
    }
    if values.values().any(Option::is_some) {
        return Err("private aggregate action program ended with live owners".into());
    }
    Ok(trace)
}

pub(crate) fn wasm_address(slot: HarnessSlot) -> i32 {
    let field = match slot.leaf {
        HarnessLeaf::First | HarnessLeaf::Left | HarnessLeaf::Scalar => 0,
        HarnessLeaf::Second | HarnessLeaf::Right => 4,
        HarnessLeaf::Tail => 8,
    };
    i32::from(slot.storage as u8) * 32 + field
}

fn c_slot(slot: HarnessSlot) -> String {
    use HarnessStorage::*;
    let storage = match slot.storage {
        PairParam => "pair_param",
        PairBase => "pair_base",
        PairReplacementParam => "pair_replacement_param",
        PairUpdate => "pair_update",
        PairBody => "pair_body",
        PairProvisional => "pair_provisional",
        PairCaller => "pair_caller",
        OuterParam => "outer_param",
        OuterBase => "outer_base",
        OuterReplacementParam => "outer_replacement_param",
        OuterUpdate => "outer_update",
        OuterBody => "outer_body",
        OuterProvisional => "outer_provisional",
        OuterCaller => "outer_caller",
        PartialParam => "partial_param",
        PartialBase => "partial_base",
        PartialFirstParam => "partial_first_param",
        PartialSecondParam => "partial_second_param",
        PartialUpdate => "partial_update",
        PartialCallArgument => "partial_call_argument",
        PartialCalleeArgument => "partial_callee_argument",
        TakeFirstParam => "take_first_param",
        TakeSecondParam => "take_second_param",
        TakeConstruct => "take_construct",
        TakeLocal => "take_local",
        TakeBody => "take_body",
        TakeProvisional => "take_provisional",
        TakeCaller => "take_caller",
    };
    match slot.leaf {
        HarnessLeaf::First => format!("{storage}.first"),
        HarnessLeaf::Second => format!("{storage}.second"),
        HarnessLeaf::Left if slot.storage == OuterReplacementParam => format!("{storage}.left"),
        HarnessLeaf::Right if slot.storage == OuterReplacementParam => format!("{storage}.right"),
        HarnessLeaf::Left => format!("{storage}.inner.left"),
        HarnessLeaf::Right => format!("{storage}.inner.right"),
        HarnessLeaf::Tail => format!("{storage}.tail"),
        HarnessLeaf::Scalar => storage.into(),
    }
}

fn emit_c_harness(scenario: &ResourceHarnessScenario) -> String {
    scenario.validate().unwrap();
    let expected = scenario
        .expected_trace
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let mut c = r#"#include <stddef.h>
#include <stdint.h>
#include <string.h>
typedef uint64_t spx_handle;
struct Pair { spx_handle first; spx_handle second; };
struct Inner { spx_handle left; spx_handle right; };
struct Outer { struct Inner inner; spx_handle tail; };
_Static_assert(sizeof(struct Pair) == 16, "pair size");
_Static_assert(_Alignof(struct Pair) == 8, "pair align");
_Static_assert(offsetof(struct Pair, second) == 8, "pair second offset");
_Static_assert(sizeof(struct Outer) == 24, "outer size");
_Static_assert(offsetof(struct Outer, tail) == 16, "outer tail offset");
static uint8_t live[128]; static uint32_t live_count;
static uint32_t trace[32]; static size_t trace_length;
static void adopt(spx_handle value) { if (value == 0 || value >= 128 || live[value]) __builtin_trap(); live[value] = 1; live_count += 1; }
static void finalize(spx_handle *slot) { spx_handle value = *slot; if (value == 0 || value >= 128 || !live[value]) __builtin_trap(); live[value] = 0; live_count -= 1; trace[trace_length++] = (uint32_t)value; *slot = 0; }
int main(void) {
struct Pair pair_param = {0}, pair_base = {0}, pair_update = {0}, pair_body = {0}, pair_provisional = {0}, pair_caller = {0};
struct Outer outer_param = {0}, outer_base = {0}, outer_update = {0}, outer_body = {0}, outer_provisional = {0}, outer_caller = {0};
struct Inner outer_replacement_param = {0};
struct Pair partial_param = {0}, partial_base = {0}, partial_update = {0}, partial_result;
struct Pair take_construct = {0}, take_local = {0};
spx_handle pair_replacement_param = 0, partial_first_param = 0, partial_second_param = 0;
spx_handle partial_call_argument = 0, partial_callee_argument = 0;
spx_handle take_first_param = 0, take_second_param = 0, take_body = 0, take_provisional = 0, take_caller = 0;
"#
    .to_string();
    for action in &scenario.actions {
        match *action {
            HarnessAction::Store(slot, value) => writeln!(
                c,
                "adopt(UINT64_C({value})); {} = UINT64_C({value});",
                c_slot(slot)
            )
            .unwrap(),
            HarnessAction::Transfer(source, destination) => writeln!(
                c,
                "{} = {}; {} = 0;",
                c_slot(destination),
                c_slot(source),
                c_slot(source)
            )
            .unwrap(),
            HarnessAction::Finalize(slot) => writeln!(c, "finalize(&{});", c_slot(slot)).unwrap(),
            HarnessAction::PoisonPartialResult => {
                writeln!(c, "memset(&partial_result, 0xa5, sizeof(partial_result));").unwrap()
            }
        }
    }
    write!(c, r#"static const uint32_t expected[] = {{{expected}}};
if (live_count != 0 || trace_length != sizeof(expected) / sizeof(expected[0])) return 2;
for (size_t i = 0; i < trace_length; i += 1) if (trace[i] != expected[i]) return 3;
for (size_t i = 0; i < sizeof(partial_result); i += 1) if (((const uint8_t *)&partial_result)[i] != UINT8_C(0xa5)) return 4;
return 0;
}}
"#).unwrap();
    c
}

#[test]
fn private_native_resource_records_execute_plan_derived_cleanup_at_o0_o2() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let scenario = resource_harness_scenario();
    assert_eq!(
        scenario.expected_trace,
        [12, 22, 11, 32, 31, 33, 42, 41, 61, 62, 52, 51, 72, 71]
    );
    let source = emit_c_harness(&scenario);
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    for optimization in ["-O0", "-O2"] {
        let stem = format!(
            "semaprax-native-resource-{}-{id}-{}",
            std::process::id(),
            &optimization[1..]
        );
        let c_path = std::env::temp_dir().join(format!("{stem}.c"));
        let executable =
            std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&c_path, &source).unwrap();
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                "-pedantic-errors",
                "-Wall",
                "-Wextra",
                "-Werror",
                optimization,
            ])
            .arg(&c_path)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "private aggregate C compile failed: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let output = Command::new(&executable).output().unwrap();
        let _ = std::fs::remove_file(&c_path);
        let _ = std::fs::remove_file(&executable);
        assert!(
            output.status.success(),
            "private aggregate C runtime failed at {optimization}"
        );
    }
}

#[test]
fn hostile_private_action_mutations_are_rejected() {
    let scenario = resource_harness_scenario();

    // These two transfers are independent, so the liveness state machine
    // accepts the reorder while the authenticated action digest rejects it.
    let mut reordered = scenario.clone();
    reordered.actions.swap(5, 6);
    assert!(execute_actions(&reordered.actions).is_ok());
    assert!(reordered.validate().is_err());

    let mut deleted = scenario.clone();
    deleted.actions.remove(5);
    assert!(execute_actions(&deleted.actions).is_err());
    assert!(deleted.validate().is_err());

    let mut dead_source = scenario.clone();
    let HarnessAction::Transfer(source, destination) = dead_source.actions[5] else {
        panic!("frozen action 5 must be a transfer")
    };
    dead_source.actions[5] = HarnessAction::Transfer(destination, source);
    assert!(execute_actions(&dead_source.actions).is_err());
    assert!(dead_source.validate().is_err());

    let mut live_destination = scenario.clone();
    let HarnessAction::Transfer(_, occupied) = live_destination.actions[3] else {
        panic!("frozen action 3 must be a transfer")
    };
    live_destination.actions[5] = HarnessAction::Transfer(source, occupied);
    assert!(execute_actions(&live_destination.actions).is_err());
    assert!(live_destination.validate().is_err());

    let mut dependency_reorder = scenario;
    dependency_reorder.actions.swap(0, 3);
    assert!(execute_actions(&dependency_reorder.actions).is_err());
    assert!(dependency_reorder.validate().is_err());
}
