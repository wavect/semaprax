//! Private drift guard captured at the checked source-to-HIR boundary. This
//! canonical binary projection excludes proof plans: those are replayed later.
//! Unsupported semantic nodes refuse, rather than being omitted from the hash.
use crate::hir::*;
use sha2::{Digest, Sha256};
const DOMAIN: &[u8] = b"semaprax.retained-primitive-body.v2\0";
struct Encoder<'a, const BYTE_ITERATOR: bool> {
    hash: Sha256,
    work: usize,
    debit: Option<&'a mut dyn FnMut(usize) -> Option<()>>,
}
impl<const BYTE_ITERATOR: bool> Encoder<'_, BYTE_ITERATOR> {
    fn bytes(&mut self, bytes: &[u8]) -> Option<()> {
        let units = bytes.len().checked_add(1)?;
        let remaining = self.work.checked_sub(units)?;
        if let Some(debit) = self.debit.as_deref_mut() {
            // The shared ledger accounts the actual length prefix and payload.
            // Keep the historical per-body local ceiling as an extra bound.
            debit(bytes.len().checked_add(std::mem::size_of::<u64>())?)?;
        }
        self.work = remaining;
        self.hash
            .update((u64::try_from(bytes.len()).ok()?).to_le_bytes());
        self.hash.update(bytes);
        Some(())
    }
    fn text(&mut self, text: &str) -> Option<()> {
        self.bytes(text.as_bytes())
    }
    fn number(&mut self, n: u64) -> Option<()> {
        self.bytes(&n.to_le_bytes())
    }
    fn span(&mut self, span: crate::ast::Span) -> Option<()> {
        self.number(u64::try_from(span.start).ok()?)?;
        self.number(u64::try_from(span.end).ok()?)?;
        self.number(u64::try_from(span.line).ok()?)?;
        self.number(u64::try_from(span.column).ok()?)
    }
    fn ty(&mut self, ty: &ResolvedType) -> Option<()> {
        if let ResolvedType::Nominal {
            declaration,
            arguments,
        } = ty
        {
            let vector = declaration.as_str() == crate::prelude::VEC_ID
                && matches!(arguments.as_slice(), [element] if admitted_vec_element(element));
            let byte_iterator = BYTE_ITERATOR
                && matches!(
                    declaration.as_str(),
                    crate::iterator_ops::ITER_ID | crate::iterator_ops::STEP_ID
                )
                && arguments.as_slice() == [ResolvedType::Bytes];
            if !arguments.is_empty()
                && !vector
                && !byte_iterator
                && (!crate::hir::borrowed_scalar::option_u8::selected_profile_ready()
                    || !crate::hir::borrowed_scalar::option_u8::is_type(ty))
            {
                return None;
            }
            self.text("nominal")?;
            self.text(declaration.as_str())?;
            self.number(arguments.len() as u64)?;
            for argument in arguments {
                self.ty(argument)?;
            }
            return Some(());
        }
        self.text(match ty {
            ResolvedType::I64 => "i64",
            ResolvedType::I32 => "i32",
            ResolvedType::Char => "char",
            ResolvedType::U8 => "u8",
            ResolvedType::Usize => "usize",
            ResolvedType::F32 => "f32",
            ResolvedType::F64 => "f64",
            ResolvedType::Bool => "bool",
            ResolvedType::String => "String",
            ResolvedType::Bytes => "Bytes",
            ResolvedType::Str => "str",
            ResolvedType::SliceU8 => "Slice",
            ResolvedType::Never => "Never",
            ResolvedType::ArrayU8(n) => {
                self.text("ArrayU8")?;
                return self.number(u64::from(*n));
            }
            _ => return None,
        })
    }
    fn ownership(&mut self, mode: OwnershipMode) -> Option<()> {
        self.text(match mode {
            OwnershipMode::Value => "value",
            OwnershipMode::Own => "own",
            OwnershipMode::Borrow => "borrow",
            OwnershipMode::Shared => "shared",
        })
    }
    fn binding(&mut self, binding: &ResolvedBinding) -> Option<()> {
        self.span(binding.span)?;
        self.text(binding.id.as_str())?;
        self.text(&binding.name)?;
        self.ty(&binding.ty)?;
        self.ownership(binding.ownership)
    }
    fn place(&mut self, place: &Place) -> Option<()> {
        self.text(place.root.as_str())?;
        self.number(place.projections.len() as u64)?;
        for projection in &place.projections {
            match projection {
                PlaceProjection::Field(field) => {
                    self.text("field")?;
                    self.text(field.as_str())?;
                }
                PlaceProjection::VariantField { case, field } => {
                    self.text("variant-field")?;
                    self.text(case.as_str())?;
                    self.text(field.as_str())?;
                }
            }
        }
        Some(())
    }
    fn owner(&mut self, owner: &FunctionExecutionId) -> Option<()> {
        match owner {
            FunctionExecutionId::Monomorphic(id) => {
                self.text("mono")?;
                self.text(id.as_str())
            }
            FunctionExecutionId::Generic(_) => None,
        }
    }
    fn pattern(&mut self, pattern: &ResolvedMatchPattern, depth: usize) -> Option<()> {
        if depth > 256 {
            return None;
        }
        match pattern {
            ResolvedMatchPattern::Wildcard => self.text("wildcard"),
            ResolvedMatchPattern::Binding(binding) => {
                self.text("binding")?;
                self.binding(binding)
            }
            ResolvedMatchPattern::Literal(value) => {
                self.text("literal")?;
                match value {
                    PatternValue::Int(n) => {
                        self.text("i64")?;
                        self.bytes(&n.to_le_bytes())
                    }
                    PatternValue::Int32(n) => {
                        self.text("i32")?;
                        self.bytes(&n.to_le_bytes())
                    }
                    PatternValue::Uint8(n) => {
                        self.text("u8")?;
                        self.bytes(&[*n])
                    }
                    PatternValue::Usize(n) => {
                        self.text("usize")?;
                        self.number(*n)
                    }
                    PatternValue::Char(n) => {
                        self.text("char")?;
                        self.bytes(&n.to_le_bytes())
                    }
                    PatternValue::Bool(n) => {
                        self.text("bool")?;
                        self.bytes(&[u8::from(*n)])
                    }
                }
            }
            ResolvedMatchPattern::Or(alternatives) => {
                self.text("or")?;
                self.number(alternatives.len() as u64)?;
                for p in alternatives {
                    self.pattern(p, depth + 1)?;
                }
                Some(())
            }
            ResolvedMatchPattern::Variant {
                variant,
                case,
                fields,
            } if crate::hir::borrowed_scalar::option_u8::selected_profile_ready()
                && crate::hir::borrowed_scalar::option_u8::pattern(pattern) =>
            {
                self.text("option-u8-pattern")?;
                self.text(variant.as_str())?;
                self.text(case.as_str())?;
                self.number(fields.len() as u64)?;
                for field in fields {
                    self.text(field.field.as_str())?;
                    self.binding(&field.binding)?;
                }
                Some(())
            }
            ResolvedMatchPattern::Variant {
                variant,
                case,
                fields,
            } if BYTE_ITERATOR
                && variant.as_str() == crate::iterator_ops::STEP_ID
                && match (case.as_str(), fields.as_slice()) {
                    (crate::iterator_ops::DONE_ID, []) => true,
                    (crate::iterator_ops::YIELD_ID, [item, rest]) => {
                        item.field.as_str() == crate::iterator_ops::ITEM_ID
                            && rest.field.as_str() == crate::iterator_ops::REST_ID
                    }
                    _ => false,
                } =>
            {
                self.text("byte-iterator-step-pattern")?;
                self.text(variant.as_str())?;
                self.text(case.as_str())?;
                self.number(fields.len() as u64)?;
                for field in fields {
                    self.text(field.field.as_str())?;
                    self.binding(&field.binding)?;
                }
                Some(())
            }
            ResolvedMatchPattern::Variant { .. } | ResolvedMatchPattern::Record { .. } => None,
        }
    }
    fn expression(&mut self, expression: &ResolvedExpr, depth: usize) -> Option<()> {
        if depth > 256 {
            return None;
        }
        self.span(expression.span)?;
        self.text(expression.id.as_str())?;
        self.ty(&expression.ty)?;
        self.ownership(expression.ownership)?;
        use ResolvedExprKind as E;
        match &expression.kind {
            E::Int(n) => {
                self.text("int")?;
                self.bytes(&n.to_le_bytes())
            }
            E::Int32(n) => {
                self.text("int32")?;
                self.bytes(&n.to_le_bytes())
            }
            E::Char(n) => {
                self.text("char")?;
                self.bytes(&n.to_le_bytes())
            }
            E::Uint8(n) => {
                self.text("u8")?;
                self.bytes(&[*n])
            }
            E::Usize(n) => {
                self.text("usize")?;
                self.number(*n)
            }
            E::Float32(n) => {
                self.text("f32bits")?;
                self.bytes(&n.to_le_bytes())
            }
            E::Float64(n) => {
                self.text("f64bits")?;
                self.number(*n)
            }
            E::Bool(n) => {
                self.text("bool")?;
                self.bytes(&[u8::from(*n)])
            }
            E::ArrayU8(bytes) => {
                self.text("array")?;
                self.bytes(bytes)
            }
            E::RepeatArrayU8 { value, count } => {
                self.text("repeat-array")?;
                self.bytes(&[*value])?;
                self.number(u64::from(*count))
            }
            E::String(text) => {
                self.text("string")?;
                self.text(text)
            }
            E::BorrowedUtf8Literal(text) => {
                if !text.is_backed() {
                    return None;
                }
                self.text("static-str")?;
                self.text(text.as_str())
            }
            E::Place(place) => {
                self.text("place")?;
                self.place(place)
            }
            E::BorrowPlace { operation, place } => {
                self.text("borrow-place")?;
                self.text(operation.as_str())?;
                self.place(place)
            }
            E::ByteRange {
                operation,
                source,
                start,
                end,
            } => {
                self.text("byte-range")?;
                self.text(operation.as_str())?;
                self.expression(source, depth + 1)?;
                self.expression(start, depth + 1)?;
                self.expression(end, depth + 1)
            }
            E::Call {
                callee,
                type_arguments,
                instance,
                args,
            } => {
                if instance.is_some() {
                    return None;
                }
                if !type_arguments.is_empty() {
                    let tag = if crate::vec_ops::by_id(callee.as_str()).is_some()
                        && matches!(type_arguments.as_slice(), [element] if admitted_vec_element(element))
                    {
                        "monomorphic-vec-type-argument"
                    } else if BYTE_ITERATOR
                        && crate::iterator_ops::by_id(callee.as_str()).is_some()
                        && type_arguments.as_slice() == [ResolvedType::Bytes]
                    {
                        "monomorphic-byte-iterator-type-argument"
                    } else {
                        return None;
                    };
                    self.text(tag)?;
                    self.ty(&type_arguments[0])?;
                }
                self.text("call")?;
                self.text(callee.as_str())?;
                self.number(args.len() as u64)?;
                for arg in args {
                    self.expression(arg, depth + 1)?;
                }
                Some(())
            }
            E::Unary { op, value } => {
                self.text("unary")?;
                self.text(match op {
                    crate::ast::UnaryOp::Neg => "neg",
                    crate::ast::UnaryOp::Not => "not",
                })?;
                self.expression(value, depth + 1)
            }
            E::Binary { op, left, right } => {
                self.text("binary")?;
                self.text(op.text())?;
                self.expression(left, depth + 1)?;
                self.expression(right, depth + 1)
            }
            E::VecConstructor {
                element,
                count,
                statements,
                tail,
            } => {
                self.text("vec-constructor")?;
                self.ty(element)?;
                self.number(u64::from(*count))?;
                self.number(statements.len() as u64)?;
                for statement in statements {
                    self.statement(statement, depth + 1)?;
                }
                self.expression(tail, depth + 1)
            }
            E::Block { statements, tail } => {
                self.text("block")?;
                self.number(statements.len() as u64)?;
                for statement in statements {
                    self.statement(statement, depth + 1)?;
                }
                self.expression(tail, depth + 1)
            }
            E::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.text("if")?;
                self.expression(condition, depth + 1)?;
                self.expression(then_branch, depth + 1)?;
                self.expression(else_branch, depth + 1)
            }
            E::Match {
                mode,
                scrutinee,
                arms,
            } => {
                self.text("match")?;
                self.text(match mode {
                    ResolvedMatchMode::Value => "value",
                    ResolvedMatchMode::Own => "own",
                    ResolvedMatchMode::Borrow => "borrow",
                })?;
                self.expression(scrutinee, depth + 1)?;
                self.number(arms.len() as u64)?;
                for arm in arms {
                    self.span(arm.span)?;
                    self.pattern(&arm.pattern, 0)?;
                    self.bytes(&[u8::from(arm.guard.is_some())])?;
                    if let Some(guard) = &arm.guard {
                        self.expression(guard, depth + 1)?;
                    }
                    self.expression(&arm.value, depth + 1)?;
                }
                Some(())
            }
            E::LexicalExit {
                kind,
                target,
                value,
            } => {
                self.text("exit")?;
                self.text(match kind {
                    crate::ast::LexicalExitKind::Break => "break",
                    crate::ast::LexicalExitKind::Continue => "continue",
                    crate::ast::LexicalExitKind::Return => "return",
                })?;
                match target {
                    ControlTarget::Function(owner) => {
                        self.text("function")?;
                        self.owner(owner)?;
                    }
                    ControlTarget::Loop { owner, site } => {
                        self.text("loop")?;
                        self.owner(owner)?;
                        self.text(site.as_str())?;
                    }
                }
                self.bytes(&[u8::from(value.is_some())])?;
                if let Some(value) = value {
                    self.expression(value, depth + 1)?;
                }
                Some(())
            }
            E::ConstructRecord { record, fields } => {
                self.text("construct-record")?;
                self.text(record.as_str())?;
                self.number(fields.len() as u64)?;
                for field in fields {
                    self.text(field.field.as_str())?;
                    self.expression(&field.value, depth + 1)?;
                }
                Some(())
            }
            E::VecFieldRead {
                origin,
                element,
                field,
                bytes,
                args,
            } => {
                if !admitted_vec_element(element) || args.len() != 2 {
                    return None;
                }
                self.text("vec-field-read")?;
                self.text(origin.as_str())?;
                self.ty(element)?;
                self.text(field.as_str())?;
                self.bytes(&[u8::from(*bytes)])?;
                self.number(args.len() as u64)?;
                for argument in args {
                    self.expression(argument, depth + 1)?;
                }
                Some(())
            }
            // Row Loan v1: every descriptor identity participates in drift.
            E::RowLoan { loan, args } => {
                if !admitted_vec_element(&loan.element) || args.len() != 2 {
                    return None;
                }
                self.text("row-loan")?;
                self.text(loan.version.as_str())?;
                self.ty(&loan.element)?;
                for identity in [
                    loan.traversal.as_str(),
                    loan.bound.as_str(),
                    loan.callee.as_str(),
                    loan.parameter.as_str(),
                ] {
                    self.text(identity)?;
                }
                self.number(args.len() as u64)?;
                for argument in args {
                    self.expression(argument, depth + 1)?;
                }
                Some(())
            }
            E::Project { base, field } => {
                self.text("project")?;
                self.text(field.as_str())?;
                self.expression(base, depth + 1)
            }
            E::ConstructVariant {
                variant,
                case,
                fields,
            } if BYTE_ITERATOR
                && variant.as_str() == crate::iterator_ops::STEP_ID
                && case.as_str() == crate::iterator_ops::DONE_ID
                && fields.is_empty()
                && crate::iterator_ops::element(&expression.ty) == Some(&ResolvedType::Bytes) =>
            {
                self.text("byte-iterator-done")?;
                self.text(variant.as_str())?;
                self.text(case.as_str())
            }
            E::Closure { .. }
            | E::FunctionReference { .. }
            | E::Invoke { .. }
            | E::LiteralFormat { .. }
            | E::NativeRustImportCall(_)
            | E::HostCommandCall(_)
            | E::ConstructVariant { .. }
            | E::Try { .. }
            | E::TryOption { .. }
            | E::UpdateRecord { .. }
            | E::Upcast { .. }
            | E::Yield { .. } => None,
        }
    }
    fn statement(&mut self, statement: &ResolvedStatement, depth: usize) -> Option<()> {
        if depth > 256 {
            return None;
        }
        self.span(match statement {
            ResolvedStatement::Let { span, .. }
            | ResolvedStatement::Assign { span, .. }
            | ResolvedStatement::While { span, .. }
            | ResolvedStatement::LexicalWhile { span, .. }
            | ResolvedStatement::CopyDiscard { span, .. }
            | ResolvedStatement::Unsafe { span, .. } => *span,
        })?;
        match statement {
            ResolvedStatement::Let {
                binding,
                mutable,
                value,
                ..
            } => {
                self.text("let")?;
                self.binding(binding)?;
                self.bytes(&[u8::from(*mutable)])?;
                self.expression(value, depth)
            }
            ResolvedStatement::Assign {
                binding,
                field,
                value,
                ..
            } => {
                self.text("assign")?;
                self.binding(binding)?;
                self.bytes(&[u8::from(field.is_some())])?;
                if let Some(field) = field {
                    self.text(field.as_str())?;
                }
                self.expression(value, depth)
            }
            ResolvedStatement::While {
                condition, body, ..
            } => {
                self.text("while")?;
                self.expression(condition, depth)?;
                self.expression(body, depth)
            }
            ResolvedStatement::LexicalWhile {
                condition,
                body,
                latch,
                ..
            } => {
                match latch {
                    LoopLatchKind::Condition => self.text("lexical-while-condition")?,
                    // A borrowed record traversal binds its generated step by
                    // identity; selected replay independently validates the latch.
                    LoopLatchKind::SourceForStep {
                        kind: SourceForKind::BorrowRecord,
                        step,
                    } => {
                        self.text("lexical-while-source-for-borrow-record")?;
                        self.text(step.as_str())?;
                    }
                    LoopLatchKind::OwnedIteratorStep {
                        next,
                        assignment,
                        step,
                        remainder,
                    } if BYTE_ITERATOR => {
                        self.text("lexical-while-byte-iterator")?;
                        for identity in [
                            next.as_str(),
                            assignment.as_str(),
                            step.as_str(),
                            remainder.as_str(),
                        ] {
                            self.text(identity)?;
                        }
                    }
                    _ => return None,
                }
                self.expression(condition, depth)?;
                self.expression(body, depth)
            }
            // The closed digest profile cannot authenticate this boundary.
            ResolvedStatement::CopyDiscard { .. } | ResolvedStatement::Unsafe { .. } => None,
        }
    }
}
/// Preserve successful byte/node debits even when a hostile body cannot encode.
pub(in crate::workspace_graph) fn typed_static_attempt_metered(
    function: &ResolvedFunction,
    source_revision: &[u8; 32],
    debit: Option<&mut dyn FnMut(usize) -> Option<()>>,
) -> (Option<[u8; 32]>, usize) {
    encode::<true>(function, source_revision, debit)
}
pub(in crate::workspace_graph) fn typed_attempt_metered(
    function: &ResolvedFunction,
    source_revision: &[u8; 32],
    debit: Option<&mut dyn FnMut(usize) -> Option<()>>,
) -> (Option<[u8; 32]>, usize) {
    encode::<false>(function, source_revision, debit)
}

fn encode<const BYTE_ITERATOR: bool>(
    function: &ResolvedFunction,
    source_revision: &[u8; 32],
    debit: Option<&mut dyn FnMut(usize) -> Option<()>>,
) -> (Option<[u8; 32]>, usize) {
    let mut encoder = Encoder::<BYTE_ITERATOR> {
        hash: Sha256::new(),
        work: 1_048_576,
        debit,
    };
    let encoded = (|| {
        encoder.bytes(DOMAIN)?;
        encoder.bytes(source_revision)?;
        encoder.span(function.span)?;
        encoder.text(function.id.as_str())?;
        encoder.text(&function.name)?;
        encoder.ty(&function.return_type)?;
        encoder.text(function.result_id.as_str())?;
        encoder.number(function.params.len() as u64)?;
        for param in &function.params {
            encoder.span(param.span)?;
            encoder.text(param.id.as_str())?;
            encoder.text(&param.name)?;
            encoder.ty(&param.ty)?;
            encoder.ownership(param.ownership)?;
        }
        if !function.effects.is_empty() || function.yields.is_some() {
            return None;
        }
        encoder.text("requires")?;
        encoder.number(function.requires.len() as u64)?;
        for expression in &function.requires {
            encoder.expression(expression, 0)?;
        }
        encoder.text("body")?;
        encoder.expression(&function.body, 0)?;
        encoder.text("ensures")?;
        encoder.number(function.ensures.len() as u64)?;
        for expression in &function.ensures {
            encoder.expression(expression, 0)?;
        }
        Some(())
    })();
    let spent = 1_048_576 - encoder.work;
    let digest = encoded.map(|()| encoder.hash.finalize().into());
    (digest, spent)
}
pub(in crate::workspace_graph) fn source(path: &str, module: &str, source: &str) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"semaprax.retained-primitive-source.v1\0");
    for bytes in [path.as_bytes(), module.as_bytes(), source.as_bytes()] {
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    hash.finalize().into()
}

pub(in crate::workspace_graph) fn scratch_bytes() -> usize {
    // Fixed SHA state; no formatter or materialized source is created.
    // Expression recursion is independently bounded to 256 and constructs no heap.
    std::mem::size_of::<Encoder<'_, false>>() + std::mem::size_of::<Sha256>()
}

fn admitted_vec_element(ty: &ResolvedType) -> bool {
    crate::vec_ops::resolved_element_is_admitted(ty)
        || matches!(ty, ResolvedType::String | ResolvedType::Bytes)
        || matches!(ty, ResolvedType::Nominal { declaration, arguments } if arguments.is_empty() && declaration.as_str() != crate::prelude::VEC_ID)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = r#"module static.scratch;
@id("app.main") fn main()->i64 {
 let key=str"x";
 let a=vec_push<Bytes>(vec_with_capacity<Bytes>(2usize),bytes_zeroed(1usize));
 let values=vec_push<Bytes>(a,bytes_zeroed(2usize));
 let mut count=0;
 for own item in vec_into_iter<Bytes>(values) {
  let scratch=bytes_zeroed(3usize);
  count=count+2;
  if byte_len(bytes_as_slice(item))==1usize {continue;};
  if byte_len(bytes_as_slice(scratch))!=3usize {return 0;};
  0
 }
 return count + str_len_bytes(key);
}
"#;

    #[test]
    fn static_body_binds_each_owned_byte_iterator_latch_identity() {
        let ast = crate::check(SOURCE, "static-scratch.spx").unwrap();
        let resolved = crate::hir::resolve(&ast).unwrap();
        let function = resolved
            .functions
            .iter()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        let revision = source("static-scratch.spx", "static.scratch", SOURCE);
        assert!(typed_attempt_metered(function, &revision, None).0.is_none());
        let (digest, _) = typed_static_attempt_metered(function, &revision, None);
        assert!(digest.is_some());
        for changed_field in 0..4 {
            let mut changed = function.clone();
            let ResolvedExprKind::Block { statements, .. } = &mut changed.body.kind else {
                panic!("function body");
            };
            let latch = statements
                .iter_mut()
                .find_map(|statement| {
                    let ResolvedStatement::Let { value, .. } = statement else {
                        return None;
                    };
                    let ResolvedExprKind::Block { statements, .. } = &mut value.kind else {
                        return None;
                    };
                    match statements.last_mut() {
                        Some(ResolvedStatement::LexicalWhile { latch, .. }) => Some(latch),
                        _ => None,
                    }
                })
                .expect("lowered iterator latch");
            let LoopLatchKind::OwnedIteratorStep {
                next,
                assignment,
                step,
                remainder,
            } = latch
            else {
                panic!("owned iterator latch");
            };
            match changed_field {
                0 => *next = assignment.clone(),
                1 => *assignment = next.clone(),
                2 => *step = remainder.clone(),
                3 => *remainder = step.clone(),
                _ => unreachable!(),
            }
            let changed = typed_static_attempt_metered(&changed, &revision, None)
                .0
                .unwrap();
            assert_ne!(digest, Some(changed));
        }
    }

    #[test]
    fn static_body_iterator_encoding_preserves_exact_debits_and_earlier_bytes() {
        let ast = crate::check(SOURCE, "static-scratch.spx").unwrap();
        let resolved = crate::hir::resolve(&ast).unwrap();
        let function = resolved
            .functions
            .iter()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        let revision = source("static-scratch.spx", "static.scratch", SOURCE);
        let mut bytes = 0usize;
        let (digest, work) = typed_static_attempt_metered(
            function,
            &revision,
            Some(&mut |units| {
                bytes = bytes.checked_add(units)?;
                Some(())
            }),
        );
        assert!(digest.is_some());
        for (limit, expected) in [(bytes, digest), (bytes - 1, None)] {
            let mut left = limit;
            let (actual, spent) = typed_static_attempt_metered(
                function,
                &revision,
                Some(&mut |units| {
                    left = left.checked_sub(units)?;
                    Some(())
                }),
            );
            assert_eq!(actual, expected);
            if actual.is_some() {
                assert_eq!((left, spent), (0, work));
            } else {
                assert!(spent < work);
            }
        }

        let legacy = r#"module static.earlier; @id("app.main") fn main()->i64 { return str_len_bytes(str"x"); }"#;
        let ast = crate::check(legacy, "static-earlier.spx").unwrap();
        let resolved = crate::hir::resolve(&ast).unwrap();
        let function = resolved
            .functions
            .iter()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        let revision = source("static-earlier.spx", "static.earlier", legacy);
        let before = typed_attempt_metered(function, &revision, None);
        assert!(before.0.is_some());
        assert_eq!(
            typed_static_attempt_metered(function, &revision, None),
            before
        );
    }
}
