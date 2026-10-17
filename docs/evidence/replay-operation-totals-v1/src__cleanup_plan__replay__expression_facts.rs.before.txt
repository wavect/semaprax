//! Exact operation and child facts used by independent cleanup replay.
use super::*;
mod arena;
#[cfg(test)]
mod borrowed;
mod capacity;
pub(super) mod errors;
pub(super) mod receiving;
#[cfg(test)]
mod saved;
#[cfg(test)]
pub(super) use borrowed::BorrowedFacts;

/// Selected receivers retain the legacy tree or consume an independently built
/// metered borrowed table. Ordinary tree lookup keeps its original behavior.
pub(super) enum Entries<'a> {
    Tree(std::collections::btree_map::Iter<'a, ExpressionId, Option<CallFact<'a>>>),
    Arena(arena::Iter<'a, 'a>),
    #[cfg(test)]
    Borrowed(std::slice::Iter<'a, (ExpressionId, Option<CallFact<'a>>)>),
}
impl<'a> Iterator for Entries<'a> {
    type Item = (&'a ExpressionId, &'a Option<CallFact<'a>>);
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Tree(entries) => entries.next(),
            Self::Arena(entries) => entries.next(),
            #[cfg(test)]
            Self::Borrowed(entries) => entries.next().map(|(key, value)| (key, value)),
        }
    }
}
pub(super) trait Lookup {
    #[cfg(test)]
    fn is_metered(&self) -> bool {
        false
    }
    fn entries(&self) -> Entries<'_>;
    fn entries_paid(
        &self,
        _: &ResolvedFunction,
        _: &mut ReplayBudget,
    ) -> Result<Entries<'_>, Diagnostic> {
        Ok(self.entries())
    }
    fn fact<'a>(
        &'a self,
        function: &ResolvedFunction,
        key: &ExpressionId,
        budget: &mut ReplayBudget,
    ) -> Result<Option<&'a Option<CallFact<'a>>>, Diagnostic>;
    fn require(
        &self,
        function: &ResolvedFunction,
        key: &ExpressionId,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        if self.fact(function, key, budget)?.is_some() {
            Ok(())
        } else {
            Err(replay_error(
                function,
                format_args!("cleanup plan references unknown expression `{key}`"),
            ))
        }
    }
}
impl<'hir> Lookup for BTreeMap<ExpressionId, Option<CallFact<'hir>>> {
    fn entries(&self) -> Entries<'_> {
        Entries::Tree(self.iter())
    }
    fn fact<'a>(
        &'a self,
        _: &ResolvedFunction,
        key: &ExpressionId,
        _: &mut ReplayBudget,
    ) -> Result<Option<&'a Option<CallFact<'a>>>, Diagnostic> {
        Ok(self.get(key))
    }
}

pub(super) enum Facts<'hir> {
    Tree(BTreeMap<ExpressionId, Option<CallFact<'hir>>>),
    Arena(arena::Arena<'hir>),
}
impl Facts<'_> {
    pub(super) fn len(&self) -> usize {
        match self {
            Self::Tree(tree) => tree.len(),
            Self::Arena(arena) => arena.len(),
        }
    }
}
impl Lookup for Facts<'_> {
    fn entries(&self) -> Entries<'_> {
        match self {
            Self::Tree(tree) => Lookup::entries(tree),
            Self::Arena(arena) => Lookup::entries(arena),
        }
    }
    fn entries_paid(
        &self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Entries<'_>, Diagnostic> {
        match self {
            Self::Tree(tree) => tree.entries_paid(function, budget),
            Self::Arena(arena) => arena.entries_paid(function, budget),
        }
    }
    fn fact<'a>(
        &'a self,
        function: &ResolvedFunction,
        key: &ExpressionId,
        budget: &mut ReplayBudget,
    ) -> Result<Option<&'a Option<CallFact<'a>>>, Diagnostic> {
        match self {
            Self::Tree(tree) => tree.fact(function, key, budget),
            Self::Arena(arena) => arena.fact(function, key, budget),
        }
    }
}

type FactFrame<'a> = Option<(expression_children::ReplayChildCursor<'a>, bool)>;

pub(super) fn expression_facts<'hir>(
    function: &'hir ResolvedFunction,
) -> Result<BTreeMap<ExpressionId, Option<CallFact<'hir>>>, Diagnostic> {
    let mut facts = BTreeMap::new();
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        collect_expression_facts::<errors::Legacy>(function, expression, &mut facts, None)?;
    }
    Ok(facts)
}

/// The selected entry retains this independently authenticated map across census
/// and structural replay. All entries borrow payloads; the existing clone reservations remain conservative.
pub(super) fn expression_facts_with_reservation<'hir>(
    function: &'hir ResolvedFunction,
    reservation: &mut super::physical_storage::PhysicalReservation,
) -> Result<BTreeMap<ExpressionId, Option<CallFact<'hir>>>, Diagnostic> {
    with_policy::<errors::Legacy>(function, reservation)
}

fn with_policy<'hir, P: errors::Policy>(
    function: &'hir ResolvedFunction,
    reservation: &mut super::physical_storage::PhysicalReservation,
) -> Result<BTreeMap<ExpressionId, Option<CallFact<'hir>>>, P::Error> {
    with_capacity::<P>(function, capacity::Payer::Selected(reservation))
}
fn with_capacity<'hir, P: errors::Policy>(
    function: &'hir ResolvedFunction,
    payer: capacity::Payer<'_>,
) -> Result<BTreeMap<ExpressionId, Option<CallFact<'hir>>>, P::Error> {
    let mut capacity = capacity::FactCapacity::with_payer(function, payer)
        .map_err(|failure| P::capacity(function, failure))?;
    let mut facts = BTreeMap::new();
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        collect_expression_facts::<P>(function, expression, &mut facts, Some(&mut capacity))?;
    }
    Ok(facts)
}

/// Ordinary callers retain the complete legacy verdict and error text. Only
/// acquisition refusal consumes the actual builder's already-paid diagnostic.
pub(super) fn with_budget<'hir>(
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Facts<'hir>, Diagnostic> {
    let builder_payer = match budget.payer {
        AllocationPayer::ActiveBuilder => Some(capacity::Payer::ActiveBuilder),
        #[cfg(test)]
        AllocationPayer::Oracle(lease) => Some(capacity::Payer::Oracle(lease)),
        _ => None,
    };
    if let Some(builder_payer) = builder_payer {
        budget.prepare_builder(function)?;
        let result = arena::build::<errors::Selected>(function, builder_payer, budget);
        return match result {
            Err(errors::EngineError::Fact(receipt)) if receipt.physical() => {
                Err(budget.budget_refusal(function, receipt.detail(), ""))
            }
            Err(errors::EngineError::Fact(receipt)) => Err(receipt.legacy(function)),
            Err(errors::EngineError::Owned(error)) => Err(error),
            Err(errors::EngineError::Structural(receipt)) => Err(receipt.legacy(function)),
            Err(errors::EngineError::Failed) => {
                Err(budget.budget_refusal(function, "expression facts already failed", ""))
            }
            Ok(facts) => Ok(Facts::Arena(facts)),
        };
    }
    let mut physical = budget.physical.take();
    let result = match physical.as_deref_mut() {
        Some(physical) => {
            arena::build::<errors::Legacy>(function, capacity::Payer::Selected(physical), budget)
                .map(Facts::Arena)
        }
        None => expression_facts(function).map(Facts::Tree),
    };
    budget.physical = physical;
    if budget.refusal_spent {
        if let Some(physical) = budget.physical.as_deref_mut() {
            physical.reject();
        }
    }
    result
}

/// Only this shared producer's errors are inline. Other replay children keep
/// their owned Diagnostic contract until their independent migration.
pub(super) fn paid_arena<'hir>(
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Facts<'hir>, errors::EngineError> {
    let mut physical = budget.physical.take();
    let result = match physical.as_deref_mut() {
        Some(physical) => {
            arena::build::<errors::Selected>(function, capacity::Payer::Selected(physical), budget)
                .map(Facts::Arena)
        }
        None => paid(function, budget).map(Facts::Tree),
    };
    budget.physical = physical;
    if budget.refusal_spent {
        if let Some(physical) = budget.physical.as_deref_mut() {
            physical.reject();
        }
    }
    match result {
        Err(errors::EngineError::Fact(receipt)) if receipt.physical() => {
            // End the physical sub-borrow before consuming the original
            // budget's prepaid text. Never format or discard its first receipt.
            Err(errors::EngineError::Owned(budget.budget_refusal(
                function,
                receipt.detail(),
                "",
            )))
        }
        other => other,
    }
}

pub(super) fn paid<'hir>(
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<BTreeMap<ExpressionId, Option<CallFact<'hir>>>, errors::EngineError> {
    let result = match budget.physical.as_deref_mut() {
        Some(physical) => with_policy::<errors::Selected>(function, physical),
        None => expression_facts(function).map_err(errors::EngineError::Owned),
    };
    match result {
        Err(errors::EngineError::Fact(receipt)) if receipt.physical() => {
            // End the physical sub-borrow before consuming the original
            // budget's prepaid text. Never format or discard its first receipt.
            Err(errors::EngineError::Owned(budget.budget_refusal(
                function,
                receipt.detail(),
                "",
            )))
        }
        other => other,
    }
}

fn collect_expression_facts<'a, P: errors::Policy>(
    function: &'a ResolvedFunction,
    expression: &'a ResolvedExpr,
    facts: &mut BTreeMap<ExpressionId, Option<CallFact<'a>>>,
    mut capacity: Option<&mut capacity::FactCapacity<'a, '_>>,
) -> Result<(), P::Error> {
    // One borrowed continuation per ancestor keeps the original pre-order walk
    // and avoids rescanning wide Block/Match prefixes for each child.
    let mut stack = [None; 514];
    stack[0] = Some((
        expression_children::ReplayChildCursor::new(expression),
        false,
    ));
    let mut len = 1usize;
    while len != 0 {
        len -= 1;
        let (mut cursor, started) = stack[len].take().expect("expression-fact frame retained");
        let current = cursor.expression();
        if !started {
            if let Some(capacity) = capacity.as_deref_mut() {
                capacity
                    .expression(current)
                    .map_err(|failure| P::capacity(function, failure))?;
            }
            let fact = match &current.kind {
                _ if crate::hir::closure::once::call(current).is_some() => {
                    let (callee, args) = if capacity.is_some() {
                        crate::hir::closure::once::call(current).expect("same closed affine call")
                    } else {
                        let (callee, args, _) = crate::hir::function_value::cleanup_call(current)?;
                        (callee, args)
                    };
                    Some(CallFact {
                        callee: std::borrow::Cow::Borrowed(callee),
                        instance: None,
                        arguments: call_payload::Arguments::Slice(args),
                        type_arguments: call_payload::Types::Slice(&[]),
                    })
                }
                ResolvedExprKind::Invoke { callable, args } => Some(CallFact {
                    callee: std::borrow::Cow::Borrowed(&crate::hir::function_value::INVOKE_ID),
                    instance: None,
                    arguments: call_payload::Arguments::Slice(args),
                    type_arguments: call_payload::Types::One(&callable.ty),
                }),
                ResolvedExprKind::VecFieldRead { args, .. } => Some(CallFact {
                    callee: std::borrow::Cow::Borrowed(crate::vec_field::operation_id()),
                    instance: None,
                    arguments: call_payload::Arguments::Slice(args),
                    type_arguments: call_payload::Types::Slice(&[]),
                }),
                ResolvedExprKind::LiteralFormat { args, .. } => Some(CallFact {
                    callee: std::borrow::Cow::Borrowed(crate::literal_format::operation_id()),
                    instance: None,
                    arguments: call_payload::Arguments::Slice(args),
                    type_arguments: call_payload::Types::Slice(&[]),
                }),
                ResolvedExprKind::Call {
                    callee,
                    instance,
                    args,
                    type_arguments,
                } => Some(CallFact {
                    callee: std::borrow::Cow::Borrowed(callee),
                    instance: instance.as_ref(),
                    arguments: call_payload::Arguments::Slice(args),
                    type_arguments: call_payload::Types::Slice(type_arguments),
                }),
                ResolvedExprKind::NativeRustImportCall(call)
                    if crate::cleanup_plan::native_rust::owns(current) =>
                {
                    Some(CallFact {
                        callee: std::borrow::Cow::Borrowed(&call.import),
                        instance: None,
                        arguments: call_payload::Arguments::Slice(&call.args),
                        type_arguments: call_payload::Types::Slice(&[]),
                    })
                }
                ResolvedExprKind::HostCommandCall(call) => Some(CallFact {
                    callee: std::borrow::Cow::Owned(DeclarationId::compiler(
                        crate::hir::CompilerDeclaration::Command(call.operation),
                    )),
                    instance: None,
                    arguments: call_payload::Arguments::Slice(&call.args),
                    type_arguments: call_payload::Types::Slice(&[]),
                }),
                ResolvedExprKind::ByteRange {
                    operation,
                    source,
                    start,
                    end,
                } => Some(CallFact {
                    callee: std::borrow::Cow::Borrowed(operation),
                    instance: None,
                    arguments: call_payload::Arguments::Range([source, start, end]),
                    type_arguments: call_payload::Types::Slice(&[]),
                }),
                _ => None,
            };
            if facts.insert(current.id.clone(), fact).is_some() {
                return Err(P::duplicate(function, &current.id));
            }
        }
        if let Some(child) = cursor.next() {
            if len + 2 > stack.len() {
                return Err(P::capacity(
                    function,
                    capacity::Failure {
                        detail: "HIR expression fact traversal exceeds the admitted depth",
                        physical: false,
                    },
                ));
            }
            stack[len] = Some((cursor, true));
            stack[len + 1] = Some((expression_children::ReplayChildCursor::new(child), false));
            len += 2;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    mod arena;
    mod errors;
    mod payloads;
    use crate::cleanup_plan::replay::physical_storage::PhysicalReservation;

    fn program() -> ResolvedProgram {
        let source = crate::check(
            r#"module replay.fact_owner;
@id("fact.read") fn read(input:borrow Bytes)->usize {
 let view=bytes_as_slice(input); let range=byte_range(view,0usize,1usize); byte_len(range)
}
@id("fact.main") fn main()->i64 {
 let mut data=bytes_zeroed(1usize); let mut at=0;
 while at<2 {data=bytes_set(data,0usize,65u8);at=at+1;if at==1 {continue;};0}
 if read(data)==1usize {42}else{0}
}"#,
            "replay-fact-owner.spx",
        )
        .unwrap();
        crate::hir::resolve(&source).unwrap()
    }
    fn same(
        left: &BTreeMap<ExpressionId, Option<CallFact<'_>>>,
        right: &BTreeMap<ExpressionId, Option<CallFact<'_>>>,
    ) {
        assert_eq!(left.len(), right.len());
        for (key, left) in left {
            let right = right.get(key).expect("same complete identity inventory");
            match (left, right) {
                (Some(left), Some(right)) => {
                    assert_eq!(left.callee, right.callee);
                    assert_eq!(left.instance, right.instance);
                    assert_eq!(left.arguments, right.arguments);
                    assert_eq!(left.type_arguments, right.type_arguments);
                }
                (None, None) => {}
                _ => panic!("call-fact kind differs"),
            }
        }
    }
    #[test]
    fn selected_fact_owner_matches_legacy_exact_and_one_short() {
        let program = program();
        for function in &program.functions {
            let legacy = expression_facts(function).unwrap();
            let (selected, refused, bytes) =
                crate::bounded_output::with_limit_usage(usize::MAX, || {
                    expression_facts_with_reservation(
                        function,
                        &mut PhysicalReservation::new(usize::MAX),
                    )
                    .unwrap()
                });
            assert!(!refused);
            same(&legacy, &selected);
            assert!(bytes > 0);
            let (exact, refused, used) = crate::bounded_output::with_limit_usage(bytes, || {
                expression_facts_with_reservation(function, &mut PhysicalReservation::new(bytes))
                    .unwrap()
            });
            assert!(!refused);
            assert_eq!(used, bytes);
            same(&legacy, &exact);
            let (short, refused, used) = crate::bounded_output::with_limit_usage(bytes - 1, || {
                let mut physical = PhysicalReservation::new(usize::MAX);
                let result = expression_facts_with_reservation(function, &mut physical);
                assert!(physical.reserve(0).is_err());
                result
            });
            assert!(refused);
            assert!(used < bytes);
            assert_eq!(short.err().unwrap().code, "SPX-H006");
        }
    }
    #[test]
    fn deep_type_clone_charges_new_backing_not_borrowed_input_spare() {
        let program = program();
        let original = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "fact.read")
            .unwrap();
        fn measured(
            function: &ResolvedFunction,
        ) -> (BTreeMap<ExpressionId, Option<CallFact<'_>>>, bool, usize) {
            crate::bounded_output::with_limit_usage(usize::MAX, || {
                expression_facts_with_reservation(
                    function,
                    &mut PhysicalReservation::new(usize::MAX),
                )
                .unwrap()
            })
        }
        let (_, refused, before) = measured(original);
        assert!(!refused);
        let mut function = original.clone();
        let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
            panic!("block fixture")
        };
        let ResolvedExprKind::Call { type_arguments, .. } = &mut tail.kind else {
            panic!("call fixture")
        };
        // Fact construction is not HIR admission. This shape exercises exact
        // deep Clone storage independently of any forged type's authority.
        let mut arguments = Vec::with_capacity(31);
        arguments.push(ResolvedType::U8);
        let mut types = Vec::with_capacity(64);
        types.push(ResolvedType::Nominal {
            declaration: DeclarationId::new("test.clone.type"),
            arguments,
        });
        *type_arguments = types;
        let tail_id = tail.id.clone();
        let (selected, refused, after) = measured(&function);
        assert!(!refused);
        assert_eq!(
            after - before,
            2 * std::mem::size_of::<ResolvedType>() + "test.clone.type".len()
        );
        same(&expression_facts(&function).unwrap(), &selected);
        let fact = selected.get(&tail_id).unwrap().as_ref().unwrap();
        let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
            panic!("block")
        };
        let ResolvedExprKind::Call { type_arguments, .. } = &tail.kind else {
            panic!("call")
        };
        assert!(std::ptr::eq(
            fact.type_arguments.as_slice(),
            type_arguments.as_slice()
        ));
        assert_eq!(type_arguments.capacity(), 64);
        let old = errors::old::expression_facts(&function).unwrap();
        let old = old.get(&tail_id).unwrap().as_ref().unwrap();
        assert_eq!(old.type_arguments.capacity(), 1);
        let ResolvedType::Nominal {
            arguments: old_arguments,
            ..
        } = &old.type_arguments[0]
        else {
            panic!("nominal")
        };
        assert_eq!(old_arguments.capacity(), 1);
        let ResolvedType::Nominal { arguments, .. } = &fact.type_arguments[0] else {
            panic!("nominal fixture")
        };
        assert_eq!(arguments.capacity(), 31);
    }

    #[test]
    fn selected_fact_owner_preserves_duplicate_identity_refusal() {
        let program = program();
        let mut function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "fact.read")
            .unwrap()
            .clone();
        let root = function.body.id.clone();
        let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
            panic!("block fixture")
        };
        tail.id = root;
        let legacy = expression_facts(&function).err().unwrap();
        let selected =
            expression_facts_with_reservation(&function, &mut PhysicalReservation::new(usize::MAX))
                .err()
                .unwrap();
        assert_eq!(legacy.code, "SPX-H006");
        assert_eq!(selected.code, legacy.code);
        assert_eq!(selected.message, legacy.message);
    }
    #[test]
    fn census_constructs_once_at_original_stage_and_retains_authenticated_owner() {
        let program = program();
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "fact.main")
            .unwrap();
        let mut retained = None;
        let mut calls = 0;
        super::super::lexical_census::validate_size_budget_with_expression_count(
            &program,
            function,
            || {
                calls += 1;
                let facts = expression_facts_with_reservation(
                    function,
                    &mut PhysicalReservation::new(usize::MAX),
                )?;
                let count = facts.len();
                retained = Some(facts);
                Ok(count)
            },
        )
        .unwrap();
        assert_eq!(calls, 1);
        same(
            &expression_facts(function).unwrap(),
            retained.as_ref().unwrap(),
        );
        let mut invalid = function.clone();
        invalid.cleanup_plan.blocks.clear();
        let mut called = false;
        let error = super::super::lexical_census::validate_size_budget_with_expression_count(
            &program,
            &invalid,
            || {
                called = true;
                Ok(0)
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "SPX-H006");
        assert!(!called, "invalid CFG must refuse before fact construction");
    }
}

#[cfg(test)]
mod receiving_before;

#[cfg(test)]
use crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V20;
#[cfg(test)]
pub(super) mod ordinary_before;
