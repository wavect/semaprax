//! Fail-closed materialization coverage for the exact-source allocation
//! projection. Every reachable expression of every held function is either a
//! catalogued checked allocation site, a proven non-allocating or same-owner
//! forwarding form, a held callee whose own candidate covers its sites, or a
//! refusal of the whole projection. This walk is separate from the candidate
//! builder and from status discovery. Passing it lends no executor, allocator
//! or release lease; it only prevents a fatal allocation path from remaining
//! inside a checked closure.
#![cfg_attr(
    not(test),
    allow(dead_code, reason = "checked allocation projection is not activated")
)]
use super::*;
use ordinary_status::allocation::Selection;

/// Refuses uncatalogued materialization in requires, body and ensures of all
/// monomorphic functions and generic instances, in declaration order.
pub(in crate::cleanup_plan) fn validate(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    for function in program.functions.iter().chain(
        program
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        let selection = Selection::new(function)?;
        for root in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(function.ensures.iter())
        {
            walk(program, function, root, &selection)?;
        }
    }
    Ok(())
}

fn walk(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    root: &ResolvedExpr,
    selection: &Selection,
) -> Result<(), Diagnostic> {
    let mut stack = [None; 514];
    stack[0] = Some((root, 0usize));
    let mut len = 1usize;
    while len != 0 {
        len -= 1;
        let (expression, next) = stack[len].take().expect("coverage frame retained");
        if next == 0 {
            classify(program, function, expression, selection)?;
        }
        if let Some(child) = replay_expression_child(expression, next) {
            if len + 2 > stack.len() {
                return Err(replay_error(
                    function,
                    "checked allocation coverage depth exceeds 512",
                ));
            }
            stack[len] = Some((expression, next + 1));
            stack[len + 1] = Some((child, 0));
            len += 2;
        }
    }
    Ok(())
}

fn refuse(function: &ResolvedFunction, expression: &ResolvedExpr, site: &str) -> Diagnostic {
    replay_error(
        function,
        format_args!(
            "checked allocation projection has no failure inventory for {site} at `{}`",
            expression.id,
        ),
    )
}

fn owned_drop(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
) -> Result<bool, Diagnostic> {
    Ok(expression.ownership == OwnershipMode::Own
        && type_needs_drop(program, function, &expression.ty)?)
}

fn classify(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    selection: &Selection,
) -> Result<(), Diagnostic> {
    if selection.operation(expression).is_some() {
        return Ok(());
    }
    // The once-closure protocol constructs and invokes an environment whose
    // body lies outside the replay child domain.
    if crate::hir::closure::once::call(expression).is_some() {
        return Err(refuse(function, expression, "a once-closure environment"));
    }
    match &expression.kind {
        ResolvedExprKind::Closure { .. } => {
            Err(refuse(function, expression, "closure backing and capture clones"))
        }
        ResolvedExprKind::FunctionReference { .. } => {
            Err(refuse(function, expression, "function-value signature metadata"))
        }
        ResolvedExprKind::Invoke { .. } => {
            Err(refuse(function, expression, "a function-value invocation"))
        }
        ResolvedExprKind::Yield { .. } => {
            Err(refuse(function, expression, "a resumable request carrier"))
        }
        ResolvedExprKind::HostCommandCall(_)
        | ResolvedExprKind::NativeRustImportCall(_)
        | ResolvedExprKind::VecFieldRead { .. } => {
            if owned_drop(program, function, expression)? {
                Err(refuse(function, expression, "an owned host, native or field result"))
            } else {
                Ok(())
            }
        }
        ResolvedExprKind::Call { callee, instance, type_arguments, .. } => {
            call(program, function, expression, callee, instance.as_ref(), type_arguments)
        }
        // Catalogued above; reaching here would be an uncovered constructor.
        ResolvedExprKind::String(_) | ResolvedExprKind::LiteralFormat { .. } => {
            Err(refuse(function, expression, "a String constructor outside the closed catalogue"))
        }
        // Native aggregates can be inline, but the ordinary interpreter builds
        // BTreeMap fields and an Arc carrier. Until each engine has a checked
        // metadata receiver, in-place native layout cannot certify this site.
        ResolvedExprKind::ConstructRecord { .. }
        | ResolvedExprKind::ConstructVariant { .. }
        | ResolvedExprKind::UpdateRecord { .. } => {
            Err(refuse(function, expression, "aggregate carrier metadata"))
        }
        // Even an empty interpreter array owns an Arc carrier. Native inline
        // array storage cannot certify allocation behavior on the other engines.
        ResolvedExprKind::ArrayU8(_) | ResolvedExprKind::RepeatArrayU8 { .. } => {
            Err(refuse(function, expression, "array carrier metadata"))
        }
        // An owning String place outside the catalogue is a selected move or
        // condition read; any other owning place is a transfer, not a copy.
        ResolvedExprKind::Place(_)
        | ResolvedExprKind::ByteRange { .. }
        | ResolvedExprKind::Unary { .. }
        | ResolvedExprKind::Binary { .. }
        | ResolvedExprKind::Block { .. }
        // The constructor marker expands to its catalogued capacity and push
        // chain, walked as ordinary children; it adds no runtime operation.
        | ResolvedExprKind::VecConstructor { .. }
        | ResolvedExprKind::LexicalExit { .. }
        | ResolvedExprKind::If { .. }
        | ResolvedExprKind::Try { .. }
        | ResolvedExprKind::TryOption { .. }
        | ResolvedExprKind::Match { .. }
        | ResolvedExprKind::Project { .. }
        | ResolvedExprKind::Upcast { .. }
        | ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::BorrowedUtf8Literal(_)
        | ResolvedExprKind::BorrowPlace { .. }
        | ResolvedExprKind::RowLoan { .. } => Ok(()),
    }
}

/// Closed library classification. Every match is exhaustive so a new
/// operation cannot silently inherit a non-allocating verdict.
fn call(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    callee: &DeclarationId,
    instance: Option<&FunctionInstanceId>,
    type_arguments: &[ResolvedType],
) -> Result<(), Diagnostic> {
    let reads = |site: &str| -> Result<(), Diagnostic> {
        if owned_drop(program, function, expression)? {
            Err(refuse(function, expression, site))
        } else {
            Ok(())
        }
    };
    if instance.is_none() {
        if crate::string_ops::take::operand(expression).is_some() {
            return Ok(());
        }
        if let Some(operation) = crate::string_ops::by_id(callee.as_str()) {
            use crate::string_ops::StringOp as S;
            return match operation {
                S::FileReadText => Err(refuse(function, expression, "a file text result")),
                // Catalogued by the selection above.
                S::Concat
                | S::FromChar
                | S::FromI64
                | S::FromUsize
                | S::Slice
                | S::Trim
                | S::MapNew
                | S::MapAdd
                | S::MapSet
                | S::MapKeyAt
                | S::FromStr
                | S::FromUtf8 => Err(refuse(
                    function,
                    expression,
                    "an uncatalogued String operation shape",
                )),
                S::Take
                | S::Len
                | S::IsEmpty
                | S::StartsWith
                | S::Contains
                | S::LenChars
                | S::Find
                | S::ToI64
                | S::ByteAt
                | S::Compare
                | S::MapGetOr
                | S::MapHas
                | S::MapLen
                | S::MapValueAt
                | S::MapRemove
                | S::F64FromI64
                | S::I64FromF64
                | S::UsizeFromI64
                | S::I64FromUsize
                | S::I64FromU8
                | S::I64FromI32
                | S::UsizeFromU8
                | S::U8FromI64
                | S::CharFromU8
                | S::CharFromI64 => Ok(()),
            };
        }
        if let Some(operation) = crate::byte_ops::by_id(callee.as_str()) {
            use crate::byte_ops::ByteOp as B;
            return match operation {
                B::Partition => Err(refuse(function, expression, "a byte partition carrier")),
                B::Copy | B::Zeroed | B::ZeroedBounded => Err(refuse(
                    function,
                    expression,
                    "an uncatalogued byte operation shape",
                )),
                B::Len
                | B::Get
                | B::Range
                | B::BytesAsSlice
                | B::ArrayAsSlice
                | B::StrAsBytes
                | B::StringAsStr
                | B::Set
                | B::Set5
                | B::Set1Or5
                | B::Set1Or6Or48 => Ok(()),
            };
        }
        if let Some(operation) = crate::vec_ops::by_id(callee.as_str()) {
            use crate::vec_ops::VecOp as V;
            return match operation {
                V::Sort | V::SortOwned | V::SortBy | V::UniqueBy => Err(refuse(
                    function,
                    expression,
                    "Vec sort or selection scratch",
                )),
                V::WithCapacity | V::ReserveExact | V::ReserveOwned | V::CloneAt => Err(refuse(
                    function,
                    expression,
                    "an uncatalogued Vec operation shape",
                )),
                V::Get => reads("an owned Vec element read"),
                V::Push | V::Len | V::Capacity | V::Set | V::Clear | V::Replace | V::StrAt => {
                    Ok(())
                }
            };
        }
        if let Some(operation) = crate::map_ops::by_id(callee.as_str()) {
            use crate::map_ops::MapOp as M;
            let key_clone = || -> Result<(), Diagnostic> {
                match type_arguments.first() {
                    Some(ResolvedType::String) => Err(refuse(
                        function,
                        expression,
                        "an uncatalogued String key shape",
                    )),
                    Some(key) if type_needs_drop(program, function, key)? => {
                        Err(refuse(function, expression, "a non-String owned key clone"))
                    }
                    _ => Ok(()),
                }
            };
            return match operation {
                M::New | M::SetNew => Err(refuse(
                    function,
                    expression,
                    "an uncatalogued collection backing shape",
                )),
                M::Add | M::Set | M::SetInsert => key_clone(),
                M::KeyAt | M::SetKeyAt => reads("an owned collection key clone"),
                M::GetOr | M::ValueAt => reads("an owned collection value clone"),
                M::Remove | M::Has | M::Len | M::SetRemove | M::SetHas | M::SetLen => Ok(()),
            };
        }
        if let Some(operation) = crate::box_ops::by_id(callee.as_str()) {
            use crate::box_ops::BoxOp as X;
            return match operation {
                X::New => Err(refuse(
                    function,
                    expression,
                    "an uncatalogued Box backing shape",
                )),
                X::Get | X::IntoInner => Ok(()),
            };
        }
        if let Some(operation) = crate::list_ops::by_id(callee.as_str()) {
            use crate::list_ops::ListOp as L;
            return match operation {
                L::Nil | L::Cons => Err(refuse(function, expression, "an opaque List carrier")),
                L::Uncons => Ok(()),
            };
        }
        if let Some(operation) = crate::iterator_ops::by_id(callee.as_str()) {
            use crate::iterator_ops::IteratorOp as I;
            return match operation {
                I::VecIntoIter => Err(refuse(function, expression, "an iterator carrier")),
                I::Next => Ok(()),
            };
        }
        if crate::str_ops::by_id(callee.as_str()).is_some()
            || super::super::deferred_commit::is_total_str_len_bytes(callee)
        {
            return Ok(());
        }
    }
    // A held callee is projected and replayed under the same selection; it
    // materializes nothing in the caller frame beyond its returned owner.
    if ReplayContext::resolve_call_target(program, callee, instance).is_some() {
        return Ok(());
    }
    Err(refuse(function, expression, "an uncatalogued call target"))
}
