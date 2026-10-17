//! Complete original Program destructor, independently compiled for controls.
use super::*;
pub(super) struct Original(pub(super) Program);
impl std::ops::Deref for Original {
    type Target = Program;
    fn deref(&self) -> &Program {
        &self.0
    }
}
impl std::ops::DerefMut for Original {
    fn deref_mut(&mut self) -> &mut Program {
        &mut self.0
    }
}
impl Drop for Original {
    fn drop(&mut self) {
        // Accepted source trees may reach the compiler's exact depth limit.
        // Take every recursive root before field drop glue runs, then replace
        // each node with a leaf before releasing its children from worklists.
        let mut functions = mem::take(&mut self.functions);
        let _agents = mem::take(&mut self.agents);
        let mut types = Vec::new();
        let mut expressions = Vec::new();
        let mut patterns = Vec::new();
        let mut record_patterns = Vec::new();

        for declaration in mem::take(&mut self.types) {
            if let Some(parent) = declaration.extends {
                types.push(parent);
            }
            if let Some(invariants) = declaration.invariants {
                expressions.extend(*invariants);
            }
            match declaration.kind {
                TypeDeclarationKind::Resource { .. } => {}
                TypeDeclarationKind::Record { fields } => {
                    types.extend(fields.into_iter().map(|field| field.ty));
                }
                TypeDeclarationKind::Variant { cases } => {
                    types.extend(
                        cases
                            .into_iter()
                            .flat_map(|case| case.fields)
                            .map(|field| field.ty),
                    );
                }
                TypeDeclarationKind::Class { fields, methods } => {
                    types.extend(fields.into_iter().map(|field| field.ty));
                    functions.extend(methods);
                }
            }
        }

        for interface in mem::take(&mut self.interfaces) {
            for import in interface.imports {
                types.extend(import.params.into_iter().map(|param| param.ty));
            }
        }
        for protocol in mem::take(&mut self.protocols) {
            for method in protocol.methods {
                types.extend(method.params.into_iter().map(|param| param.ty));
                types.push(method.return_type);
            }
        }
        for function in functions {
            types.extend(function.params.into_iter().map(|param| param.ty));
            types.push(function.return_type);
            if let Some(yields) = function.yields {
                types.push(yields.request_type);
                types.push(yields.response_type);
            }
            expressions.extend(function.requires);
            expressions.extend(function.ensures);
            expressions.push(function.body);
        }

        while let Some(mut expression) = expressions.pop() {
            match mem::replace(&mut expression.kind, ExprKind::Int(0)) {
                ExprKind::VecConstructor { element, value, .. } => {
                    types.push(element);
                    expressions.push(*value);
                }
                ExprKind::Closure {
                    params,
                    return_type,
                    body,
                    owning: _,
                    retained: _,
                    mutable: _,
                } => {
                    types.extend(params.into_iter().map(|param| param.ty));
                    types.push(return_type);
                    expressions.push(*body);
                }
                ExprKind::Call {
                    type_arguments,
                    args,
                    ..
                } => {
                    types.extend(type_arguments);
                    expressions.extend(args);
                }
                ExprKind::SuperMethod { args, .. } => expressions.extend(args),
                ExprKind::MethodCall {
                    receiver,
                    type_arguments,
                    args,
                    ..
                } => {
                    types.extend(type_arguments);
                    expressions.push(*receiver);
                    expressions.extend(args);
                }
                ExprKind::Unary { value, .. }
                | ExprKind::Try { operand: value }
                | ExprKind::Project { base: value, .. }
                | ExprKind::Yield { request: value } => expressions.push(*value),
                ExprKind::LexicalExit { value, .. } => expressions.extend(value.map(|v| *v)),
                ExprKind::Binary { left, right, .. } => {
                    expressions.push(*left);
                    expressions.push(*right);
                }
                ExprKind::Block { statements, tail } => {
                    expressions.push(*tail);
                    for statement in statements {
                        match statement {
                            Statement::Let {
                                declared, value, ..
                            } => {
                                types.extend(declared);
                                expressions.push(value);
                            }
                            Statement::Assign { value, .. } => expressions.push(value),
                            Statement::Unsafe { body, .. } => expressions.push(*body),
                            Statement::While {
                                condition, body, ..
                            } => {
                                expressions.push(*condition);
                                expressions.push(*body);
                            }
                            Statement::For { values, body, .. }
                            | Statement::ForBorrow { values, body, .. }
                            | Statement::ForOwn { values, body, .. } => {
                                expressions.push(*values);
                                expressions.push(*body);
                            }
                        }
                    }
                }
                ExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    expressions.push(*condition);
                    expressions.push(*then_branch);
                    expressions.push(*else_branch);
                }
                ExprKind::ConstructRecord {
                    type_arguments,
                    fields,
                    ..
                }
                | ExprKind::ConstructVariant {
                    type_arguments,
                    fields,
                    ..
                } => {
                    types.extend(type_arguments);
                    expressions.extend(fields.into_iter().map(|field| field.value));
                }
                ExprKind::Match {
                    scrutinee, arms, ..
                } => {
                    expressions.push(*scrutinee);
                    for arm in arms {
                        patterns.push(arm.pattern);
                        expressions.extend(arm.guard.map(|guard| *guard));
                        expressions.push(arm.value);
                    }
                }
                ExprKind::UpdateRecord { base, fields } => {
                    expressions.push(*base);
                    expressions.extend(fields.into_iter().map(|field| field.value));
                }
                ExprKind::Int(_)
                | ExprKind::Int32(_)
                | ExprKind::Char(_)
                | ExprKind::Uint8(_)
                | ExprKind::Usize(_)
                | ExprKind::ArrayU8(_)
                | ExprKind::RepeatArrayU8 { .. }
                | ExprKind::Float32(_)
                | ExprKind::Float64(_)
                | ExprKind::Bool(_)
                | ExprKind::String(_)
                | ExprKind::BorrowedUtf8Literal(_)
                | ExprKind::Var(_) => {}
            }
        }

        while let Some(mut pattern) = patterns.pop() {
            match mem::replace(
                &mut pattern,
                MatchPattern::Wildcard {
                    span: Span::default(),
                },
            ) {
                MatchPattern::Record { fields, .. } => record_patterns.extend(fields),
                MatchPattern::Or { alternatives, .. } => patterns.extend(alternatives),
                MatchPattern::Variant { .. }
                | MatchPattern::Wildcard { .. }
                | MatchPattern::Literal { .. }
                | MatchPattern::Binding { .. } => {}
            }
        }
        while let Some(mut field) = record_patterns.pop() {
            match mem::replace(
                &mut field.pattern,
                RecordMatchFieldPattern::Wildcard {
                    span: Span::default(),
                },
            ) {
                RecordMatchFieldPattern::Record { fields, .. } => {
                    record_patterns.extend(fields);
                }
                RecordMatchFieldPattern::Binding { .. }
                | RecordMatchFieldPattern::Wildcard { .. } => {}
            }
        }

        while let Some(mut ty) = types.pop() {
            match mem::replace(&mut ty, Type::I64) {
                Type::Never => {}
                Type::Named { arguments, .. } => types.extend(arguments),
                Type::Function { parameters, result } => {
                    types.extend(parameters);
                    types.push(*result);
                }
                Type::OnceFunction
                | Type::OnceFunctionI64
                | Type::OnceFunctionI64Pair
                | Type::MutFunctionI64
                | Type::I64
                | Type::I32
                | Type::Char
                | Type::U8
                | Type::Usize
                | Type::ArrayU8(_)
                | Type::F32
                | Type::F64
                | Type::Bool
                | Type::String
                | Type::Bytes
                | Type::Str
                | Type::SliceU8
                | Type::StringMap => {}
            }
        }
    }
}

// This lexical adapter keeps the entire original deep-root test body unchanged.
fn drop(program: Program) {
    std::mem::drop(Original(program));
}
#[path = "drop_original_tests.rs"]
#[allow(
    clippy::duplicate_mod,
    reason = "Run the same controls against independent receivers."
)]
mod tests;
