//! Independent resolution of bounded inference from exact declaration types.
//! No source-verifier inference result is trusted or retained here.
use super::*;
use crate::ast::{BinaryOp, Expr, ExprKind, Type, UnaryOp};

const MAX_EVIDENCE_NODES: usize = 4_096;
const MAX_EVIDENCE_DEPTH: usize = 128;

impl Resolver<'_> {
    pub(super) fn infer_call_arguments(
        &self,
        caller: &FunctionExecutionId,
        target: &crate::ast::Function,
        args: &[Expr],
        bindings: &BTreeMap<String, Binding>,
    ) -> Option<Vec<Type>> {
        if !matches!(caller, FunctionExecutionId::Monomorphic(_))
            || target.type_parameters.is_empty()
            || args.len() != target.params.len()
        {
            return None;
        }
        let mut remaining = MAX_EVIDENCE_NODES;
        self.infer_call_arguments_with_budget(caller, target, args, bindings, &mut remaining, 0)
    }

    fn infer_call_arguments_with_budget(
        &self,
        caller: &FunctionExecutionId,
        target: &crate::ast::Function,
        args: &[Expr],
        bindings: &BTreeMap<String, Binding>,
        remaining: &mut usize,
        depth: usize,
    ) -> Option<Vec<Type>> {
        if target.type_parameters.is_empty() || args.len() != target.params.len() {
            return None;
        }
        let owner = DeclarationId::new(target.stable_id.clone());
        let mut inferred = vec![None; target.type_parameters.len()];
        for (formal, argument) in target.params.iter().zip(args) {
            let formal = self
                .resolve_function_type(target, &formal.ty, formal.span)
                .ok()?;
            let actual = self.evidence_type(caller, argument, bindings, remaining, depth)?;
            let mut pending = vec![(&formal, &actual)];
            while let Some((formal, actual)) = pending.pop() {
                if matches!(formal, ResolvedType::TypeParameter { owner: parameter_owner, .. } if *parameter_owner == owner)
                {
                    let ResolvedType::TypeParameter { index, .. } = formal else {
                        unreachable!("type-parameter match retained its shape");
                    };
                    let slot = inferred.get_mut(usize::try_from(*index).ok()?)?;
                    let ty = self.evidence_source_type(caller, actual)?;
                    if slot.as_ref().is_some_and(|old| old != &ty) {
                        return None;
                    }
                    *slot = Some(ty);
                } else if let (
                    ResolvedType::Nominal {
                        declaration: a,
                        arguments: aa,
                    },
                    ResolvedType::Nominal {
                        declaration: b,
                        arguments: ba,
                    },
                ) = (formal, actual)
                {
                    if a != b || aa.len() != ba.len() {
                        return None;
                    }
                    pending.extend(aa.iter().zip(ba));
                } else if formal != actual {
                    return None;
                }
            }
        }
        inferred.into_iter().collect()
    }

    /// Collect type facts only. This intentionally does not resolve an
    /// expression, visit its children through the normal resolver, or inspect
    /// ownership: the ordinary call path remains the only evaluation boundary.
    fn evidence_type(
        &self,
        caller: &FunctionExecutionId,
        expression: &Expr,
        bindings: &BTreeMap<String, Binding>,
        remaining: &mut usize,
        depth: usize,
    ) -> Option<ResolvedType> {
        if depth >= MAX_EVIDENCE_DEPTH {
            return None;
        }
        *remaining = remaining.checked_sub(1)?;
        match &expression.kind {
            ExprKind::Int(_) => Some(ResolvedType::I64),
            ExprKind::Int32(_) => Some(ResolvedType::I32),
            ExprKind::Uint8(_) => Some(ResolvedType::U8),
            ExprKind::Usize(_) => Some(ResolvedType::Usize),
            ExprKind::Char(_) => Some(ResolvedType::Char),
            ExprKind::Float32(_) => Some(ResolvedType::F32),
            ExprKind::Float64(_) => Some(ResolvedType::F64),
            ExprKind::Bool(_) => Some(ResolvedType::Bool),
            ExprKind::Var(name) => Some(bindings.get(name)?.ty.clone()),
            ExprKind::ConstructRecord {
                type_name,
                type_arguments,
                ..
            }
            | ExprKind::ConstructVariant {
                type_name,
                type_arguments,
                ..
            } => self
                .resolve_call_type_argument(
                    caller,
                    &Type::Named {
                        name: type_name.clone(),
                        arguments: type_arguments.clone(),
                    },
                    expression.span,
                )
                .ok(),
            ExprKind::Unary { op, value } => {
                let value = self.evidence_type(caller, value, bindings, remaining, depth + 1)?;
                match op {
                    UnaryOp::Neg if signed_numeric(&value) => Some(value),
                    UnaryOp::Not if value == ResolvedType::Bool => Some(ResolvedType::Bool),
                    _ => None,
                }
            }
            ExprKind::Binary { op, left, right } => {
                let left = self.evidence_type(caller, left, bindings, remaining, depth + 1)?;
                let right = self.evidence_type(caller, right, bindings, remaining, depth + 1)?;
                match op {
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div
                        if arithmetic(&left) && left == right =>
                    {
                        Some(left)
                    }
                    BinaryOp::Rem if left == ResolvedType::I64 && right == ResolvedType::I64 => {
                        Some(ResolvedType::I64)
                    }
                    BinaryOp::Eq | BinaryOp::Ne if left == right => Some(ResolvedType::Bool),
                    BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge
                        if left == right && ordered(&left) =>
                    {
                        Some(ResolvedType::Bool)
                    }
                    BinaryOp::And | BinaryOp::Or
                        if left == ResolvedType::Bool && right == ResolvedType::Bool =>
                    {
                        Some(ResolvedType::Bool)
                    }
                    _ => None,
                }
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                if self.evidence_type(caller, condition, bindings, remaining, depth + 1)?
                    != ResolvedType::Bool
                {
                    return None;
                }
                let then_branch =
                    self.evidence_type(caller, then_branch, bindings, remaining, depth + 1)?;
                (then_branch
                    == self.evidence_type(caller, else_branch, bindings, remaining, depth + 1)?)
                .then_some(then_branch)
            }
            ExprKind::Call { .. } => {
                self.evidence_call_result(caller, expression, bindings, remaining, depth + 1)
            }
            ExprKind::Block { statements, tail } if statements.is_empty() => {
                self.evidence_type(caller, tail, bindings, remaining, depth + 1)
            }
            _ => None,
        }
    }

    fn evidence_call_result(
        &self,
        caller: &FunctionExecutionId,
        expression: &Expr,
        bindings: &BTreeMap<String, Binding>,
        remaining: &mut usize,
        depth: usize,
    ) -> Option<ResolvedType> {
        let ExprKind::Call {
            name,
            type_arguments,
            args,
        } = &expression.kind
        else {
            return None;
        };
        let span = expression.span;
        let target = self
            .program
            .functions
            .iter()
            .find(|function| function.name == *name)?;
        if args.len() != target.params.len() {
            return None;
        }
        if target.type_parameters.is_empty() {
            return type_arguments.is_empty().then(|| {
                self.resolve_function_type(target, &target.return_type, span)
                    .ok()
            })?;
        }
        let inferred;
        let arguments = if type_arguments.is_empty() {
            inferred = self
                .infer_call_arguments_with_budget(caller, target, args, bindings, remaining, depth);
            inferred.as_deref()?
        } else {
            type_arguments
        };
        if arguments.len() != target.type_parameters.len() {
            return None;
        }
        let resolved = arguments
            .iter()
            .map(|argument| self.resolve_call_type_argument(caller, argument, span))
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        self.generic_function_arguments_are_admitted(caller, target, &resolved)
            .ok()?
            .then(|| {
                super::monomorphize::substitute_source_function_type(
                    target,
                    arguments,
                    &target.return_type,
                )
                .and_then(|result| self.resolve_call_type_argument(caller, &result, span).ok())
            })?
    }

    fn evidence_source_type(
        &self,
        caller: &FunctionExecutionId,
        ty: &ResolvedType,
    ) -> Option<Type> {
        match ty {
            ResolvedType::Unit => None,
            ResolvedType::I64 => Some(Type::I64),
            ResolvedType::I32 => Some(Type::I32),
            ResolvedType::U8 => Some(Type::U8),
            ResolvedType::Usize => Some(Type::Usize),
            ResolvedType::Char => Some(Type::Char),
            ResolvedType::F32 => Some(Type::F32),
            ResolvedType::F64 => Some(Type::F64),
            ResolvedType::Bool => Some(Type::Bool),
            ResolvedType::String => Some(Type::String),
            ResolvedType::Bytes => Some(Type::Bytes),
            ResolvedType::Str => Some(Type::Str),
            ResolvedType::SliceU8 => Some(Type::SliceU8),
            ResolvedType::ArrayU8(length) => Some(Type::ArrayU8(*length)),
            ResolvedType::TypeParameter { owner, index } => {
                let FunctionExecutionId::Monomorphic(caller) = caller else {
                    return None;
                };
                if owner != caller {
                    return None;
                }
                let function = self
                    .program
                    .functions
                    .iter()
                    .find(|function| function.stable_id == caller.as_str())?;
                Some(Type::Named {
                    name: function
                        .type_parameters
                        .get(usize::try_from(*index).ok()?)?
                        .name
                        .clone(),
                    arguments: Vec::new(),
                })
            }
            ResolvedType::Nominal {
                declaration,
                arguments,
            } => Some(Type::Named {
                name: self.evidence_nominal_name(declaration)?.to_owned(),
                arguments: arguments
                    .iter()
                    .map(|argument| self.evidence_source_type(caller, argument))
                    .collect::<Option<Vec<_>>>()?,
            }),
        }
    }

    fn evidence_nominal_name<'a>(&'a self, declaration: &DeclarationId) -> Option<&'a str> {
        match declaration.as_str() {
            crate::prelude::OPTION_ID => Some("Option"),
            crate::prelude::RESULT_ID => Some("Result"),
            crate::prelude::VEC_ID => Some("Vec"),
            crate::prelude::BOX_ID => Some("Box"),
            _ => self
                .program
                .types
                .iter()
                .find(|item| item.stable_id == declaration.as_str())
                .map(|item| item.name.as_str()),
        }
    }
}

fn arithmetic(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::I64
            | ResolvedType::I32
            | ResolvedType::U8
            | ResolvedType::Usize
            | ResolvedType::F32
            | ResolvedType::F64
    )
}

fn signed_numeric(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::I64 | ResolvedType::I32 | ResolvedType::F32 | ResolvedType::F64
    )
}

fn ordered(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::I64
            | ResolvedType::I32
            | ResolvedType::Char
            | ResolvedType::U8
            | ResolvedType::Usize
            | ResolvedType::F32
            | ResolvedType::F64
    )
}

impl Resolver<'_> {
    /// Prepare one ordinary call without evaluating or resolving its arguments.
    /// Both resolver engines use this boundary and then visit arguments once.
    pub(super) fn ordinary_call_signature(
        &self,
        caller: &FunctionExecutionId,
        target: &crate::ast::Function,
        authored_arguments: &[Type],
        args: &[Expr],
        bindings: &BTreeMap<String, Binding>,
        span: Span,
    ) -> Result<(Vec<ResolvedType>, Option<FunctionInstanceId>, Type), Diagnostic> {
        let inferred = if authored_arguments.is_empty() && !target.type_parameters.is_empty() {
            self.infer_call_arguments(caller, target, args, bindings)
        } else {
            None
        };
        let arguments = inferred.as_deref().unwrap_or(authored_arguments);
        let resolved = arguments
            .iter()
            .map(|argument| self.resolve_call_type_argument(caller, argument, span))
            .collect::<Result<Vec<_>, _>>()?;
        let template = DeclarationId::new(target.stable_id.clone());
        if target.type_parameters.is_empty() {
            if !resolved.is_empty() {
                return Err(self.error(
                    "SPX-H006",
                    format!("monomorphic function `{template}` has type arguments"),
                    span,
                ));
            }
            return Ok((resolved, None, target.return_type.clone()));
        }
        if resolved.len() != target.type_parameters.len()
            || !self.generic_function_arguments_are_admitted(caller, target, &resolved)?
        {
            return Err(self.error(
                "SPX-H006",
                format!("generic function `{template}` has invalid type arguments"),
                span,
            ));
        }
        let instance = FunctionInstanceId::derive(&template, &resolved);
        let return_type = super::monomorphize::substitute_source_function_type(
            target,
            arguments,
            &target.return_type,
        )
        .ok_or_else(|| {
            self.error(
                "SPX-H006",
                format!("generic function `{template}` return substitution failed"),
                span,
            )
        })?;
        Ok((resolved, Some(instance), return_type))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolver(program: &crate::ast::Program) -> Resolver<'_> {
        Resolver {
            program,
            declarations: DeclarationIndex::from_verified(program).expect("declaration index"),
            reuse: None,
            function_work: super::super::FunctionResolutionWork::default(),
        }
    }

    fn program() -> crate::ast::Program {
        crate::parse(
            r#"
module test.hir_inference_bounds;
@id("app.id") fn id<T>(value: T) -> T { value }
@id("app.outer") fn outer<T>(value: T) -> T { value }
@id("app.main") fn main() -> i64 { 0 }
"#,
            "hir-inference-bounds.spx",
        )
        .expect("parser accepts direct hostile-HIR fixture")
    }

    fn unary_chain(count: usize) -> Expr {
        let mut expression = Expr {
            kind: ExprKind::Int(1),
            span: Span::default(),
        };
        for _ in 0..count {
            expression = Expr {
                kind: ExprKind::Unary {
                    op: UnaryOp::Neg,
                    value: Box::new(expression),
                },
                span: Span::default(),
            };
        }
        expression
    }

    fn omitted_id(value: Expr) -> Expr {
        Expr {
            kind: ExprKind::Call {
                name: "id".to_owned(),
                type_arguments: Vec::new(),
                args: vec![value],
            },
            span: Span::default(),
        }
    }

    #[test]
    fn direct_hir_inference_bounds_depth_and_total_evidence_without_source_verification() {
        let program = program();
        let resolver = resolver(&program);
        let caller = FunctionExecutionId::Monomorphic(DeclarationId::new("app.main"));
        let target = program
            .functions
            .iter()
            .find(|function| function.stable_id == "app.id")
            .expect("fixture generic function");
        let bindings = BTreeMap::new();

        // Depth is zero-based: 127 unary nodes plus the literal has exactly
        // 128 evidence nodes; the next unary would visit depth 128.
        assert_eq!(
            resolver.infer_call_arguments(&caller, target, &[unary_chain(127)], &bindings),
            Some(vec![Type::I64])
        );
        assert!(resolver
            .infer_call_arguments(&caller, target, &[unary_chain(128)], &bindings)
            .is_none());

        let mut wide = target.clone();
        wide.params = vec![target.params[0].clone(); MAX_EVIDENCE_NODES];
        let within = vec![
            Expr {
                kind: ExprKind::Int(1),
                span: Span::default(),
            };
            MAX_EVIDENCE_NODES
        ];
        assert_eq!(
            resolver.infer_call_arguments(&caller, &wide, &within, &bindings),
            Some(vec![Type::I64])
        );
        wide.params.push(target.params[0].clone());
        let mut beyond = within;
        beyond.push(Expr {
            kind: ExprKind::Int(1),
            span: Span::default(),
        });
        assert!(resolver
            .infer_call_arguments(&caller, &wide, &beyond, &bindings)
            .is_none());

        let literal = || Expr {
            kind: ExprKind::Int(1),
            span: Span::default(),
        };
        let mut nested = literal();
        for _ in 0..127 {
            nested = omitted_id(nested);
        }
        assert_eq!(
            resolver.infer_call_arguments(&caller, target, &[nested], &bindings),
            Some(vec![Type::I64])
        );
        let mut too_deep = literal();
        for _ in 0..128 {
            too_deep = omitted_id(too_deep);
        }
        assert!(resolver
            .infer_call_arguments(&caller, target, &[too_deep], &bindings)
            .is_none());

        // 2,047 nested calls consume two evidence nodes each; two literal
        // siblings make the exact shared 4,096-node limit.
        let mut nested_siblings = (0..2_047)
            .map(|_| omitted_id(literal()))
            .collect::<Vec<_>>();
        nested_siblings.extend([literal(), literal()]);
        let mut wide_nested = target.clone();
        wide_nested.params = vec![target.params[0].clone(); nested_siblings.len()];
        assert_eq!(
            resolver.infer_call_arguments(&caller, &wide_nested, &nested_siblings, &bindings),
            Some(vec![Type::I64])
        );
        wide_nested.params.push(target.params[0].clone());
        nested_siblings.push(literal());
        assert!(resolver
            .infer_call_arguments(&caller, &wide_nested, &nested_siblings, &bindings)
            .is_none());
    }

    #[test]
    fn direct_hir_inference_retains_a_generic_caller_symbol_through_nested_omission() {
        let program = program();
        let resolver = resolver(&program);
        let caller = FunctionExecutionId::Monomorphic(DeclarationId::new("app.outer"));
        let target = program
            .functions
            .iter()
            .find(|function| function.stable_id == "app.id")
            .expect("fixture generic function");
        let nested = Expr {
            kind: ExprKind::Call {
                name: "id".to_owned(),
                type_arguments: Vec::new(),
                args: vec![Expr {
                    kind: ExprKind::Var("value".to_owned()),
                    span: Span::default(),
                }],
            },
            span: Span::default(),
        };
        let mut bindings = BTreeMap::new();
        bindings.insert(
            "value".to_owned(),
            Binding {
                id: ValueId::local(&caller, "test.value"),
                ty: ResolvedType::TypeParameter {
                    owner: DeclarationId::new("app.outer"),
                    index: 0,
                },
                ownership: OwnershipMode::Value,
                mutable: false,
            },
        );
        assert_eq!(
            resolver.infer_call_arguments(&caller, target, &[nested], &bindings),
            Some(vec![Type::Named {
                name: "T".to_owned(),
                arguments: Vec::new(),
            }])
        );
        bindings.get_mut("value").expect("fixture binding").ty = ResolvedType::TypeParameter {
            owner: DeclarationId::new("app.id"),
            index: 0,
        };
        assert!(resolver
            .infer_call_arguments(
                &caller,
                target,
                &[Expr {
                    kind: ExprKind::Var("value".to_owned()),
                    span: Span::default(),
                }],
                &bindings,
            )
            .is_none());
    }
}
