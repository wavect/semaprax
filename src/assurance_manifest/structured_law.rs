//! LAW-07 bounded immutable aggregate projection into the shared scalar VC.
//!
//! This is a source-independent draft. A caller must first obtain a checked
//! Program; no proof status may be promoted from this lowering alone.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{
    BinaryOp, Expr, ExprKind, Function, MatchMode, MatchPattern, Param, ParamMode, Program, Span,
    Statement, Type, TypeDeclaration, TypeDeclarationKind,
};

const MAX_DEPTH: usize = 3;
const MAX_FIELDS: usize = 8;
const MAX_CASES: usize = 8;
const MAX_LEAVES: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Refusal {
    MissingType(String),
    UnsupportedType(String),
    GenericType(String),
    RecursiveType(String),
    Capacity,
    Impure,
    Ownership,
    UnsupportedExpression(&'static str),
    UnknownBinding(String),
    WrongField(String),
    MatchCoverage,
    MatchPattern,
    TypeMismatch,
}

impl Refusal {
    pub fn code(&self) -> &'static str {
        match self {
            Self::MissingType(_) => "missing_type",
            Self::UnsupportedType(_) => "unsupported_type",
            Self::GenericType(_) => "generic_type",
            Self::RecursiveType(_) => "recursive_type",
            Self::Capacity => "capacity",
            Self::Impure => "impure",
            Self::Ownership => "ownership",
            Self::UnsupportedExpression(_) => "unsupported_expression",
            Self::UnknownBinding(_) => "unknown_binding",
            Self::WrongField(_) => "wrong_field",
            Self::MatchCoverage => "match_coverage",
            Self::MatchPattern => "match_pattern",
            Self::TypeMismatch => "type_mismatch",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Leaf {
    pub parameter: String,
    pub declaration_id: Option<String>,
    pub field_path: Vec<String>,
    pub ty: Type,
}

#[derive(Clone, Debug)]
pub struct Lowered {
    pub scalar: Function,
    pub leaves: Vec<Leaf>,
    pub declaration_ids: Vec<String>,
    pub source_ensures_index: Option<usize>,
    pub profile: &'static str,
}

pub const PROFILE: &str = "semaprax-structured-law-v1: checked immutable records depth<=3 fields<=8, closed variants cases<=8, scalar leaves<=32, explicit complete value match, scalar queries with structured result substitution";

#[derive(Clone)]
enum Value {
    Scalar(Expr, Type),
    Record {
        declaration_id: String,
        fields: BTreeMap<String, Value>,
    },
    Variant {
        declaration_id: String,
        tag: Expr,
        cases: BTreeMap<String, BTreeMap<String, Value>>,
    },
}

struct Lowerer<'a> {
    types: BTreeMap<&'a str, &'a TypeDeclaration>,
    names: BTreeSet<String>,
    params: Vec<Param>,
    leaves: Vec<Leaf>,
    declarations: BTreeSet<String>,
    scope: Vec<(String, Value)>,
    next_name: usize,
    tag_requires: Vec<Expr>,
    collect_touches: bool,
    guards: Vec<Expr>,
    touches: Vec<(Vec<Expr>, Expr, Type)>,
}

fn scalar(ty: &Type) -> bool {
    matches!(
        ty,
        Type::I64 | Type::I32 | Type::U8 | Type::Usize | Type::Bool
    )
}

fn expression(kind: ExprKind, span: Span) -> Expr {
    Expr { kind, span }
}

fn scalar_value(value: Value) -> Result<(Expr, Type), Refusal> {
    match value {
        Value::Scalar(expr, ty) => Ok((expr, ty)),
        Value::Record { .. } | Value::Variant { .. } => Err(Refusal::TypeMismatch),
    }
}

impl<'a> Lowerer<'a> {
    fn fresh(&mut self) -> String {
        loop {
            let name = format!("spx_law07_v{}", self.next_name);
            self.next_name += 1;
            if self.names.insert(name.clone()) {
                return name;
            }
        }
    }

    fn declaration(&self, name: &str) -> Result<&'a TypeDeclaration, Refusal> {
        self.types
            .get(name)
            .copied()
            .ok_or_else(|| Refusal::MissingType(name.to_owned()))
    }

    fn touch(&mut self, value: &Expr, ty: &Type) {
        if self.collect_touches {
            self.touches
                .push((self.guards.clone(), value.clone(), ty.clone()));
        }
    }

    fn validate_shape(
        &self,
        ty: &Type,
        active: &mut Vec<String>,
        leaves: &mut usize,
    ) -> Result<(), Refusal> {
        if scalar(ty) {
            *leaves += 1;
            return if *leaves <= MAX_LEAVES {
                Ok(())
            } else {
                Err(Refusal::Capacity)
            };
        }
        let Type::Named { name, arguments } = ty else {
            return Err(Refusal::UnsupportedType(ty.to_string()));
        };
        if !arguments.is_empty() {
            return Err(Refusal::GenericType(name.clone()));
        }
        if active.len() >= MAX_DEPTH {
            return Err(Refusal::Capacity);
        }
        let declaration = self.declaration(name)?;
        if !declaration.type_parameters.is_empty() {
            return Err(Refusal::GenericType(name.clone()));
        }
        if active.contains(&declaration.stable_id) {
            return Err(Refusal::RecursiveType(declaration.stable_id.clone()));
        }
        active.push(declaration.stable_id.clone());
        match &declaration.kind {
            TypeDeclarationKind::Record { fields } => {
                if fields.is_empty() || fields.len() > MAX_FIELDS {
                    return Err(Refusal::Capacity);
                }
                for field in fields {
                    self.validate_shape(&field.ty, active, leaves)?;
                }
            }
            TypeDeclarationKind::Variant { cases } => {
                if cases.is_empty() || cases.len() > MAX_CASES {
                    return Err(Refusal::Capacity);
                }
                *leaves += 1; // closed case tag
                for case in cases {
                    if case.fields.len() > MAX_FIELDS {
                        return Err(Refusal::Capacity);
                    }
                    for field in &case.fields {
                        self.validate_shape(&field.ty, active, leaves)?;
                    }
                }
            }
            _ => return Err(Refusal::UnsupportedType(name.clone())),
        }
        active.pop();
        if *leaves > MAX_LEAVES {
            Err(Refusal::Capacity)
        } else {
            Ok(())
        }
    }

    fn leaf(
        &mut self,
        ty: &Type,
        owner: Option<&str>,
        path: &[String],
        span: Span,
    ) -> Result<Value, Refusal> {
        if self.leaves.len() >= MAX_LEAVES {
            return Err(Refusal::Capacity);
        }
        let name = self.fresh();
        self.params.push(Param {
            name: name.clone(),
            mode: ParamMode::Value,
            ty: ty.clone(),
            span,
        });
        self.leaves.push(Leaf {
            parameter: name.clone(),
            declaration_id: owner.map(str::to_owned),
            field_path: path.to_vec(),
            ty: ty.clone(),
        });
        Ok(Value::Scalar(
            expression(ExprKind::Var(name), span),
            ty.clone(),
        ))
    }

    fn flatten(
        &mut self,
        ty: &Type,
        owner: Option<&str>,
        path: &[String],
        active: &mut Vec<String>,
        span: Span,
    ) -> Result<Value, Refusal> {
        if scalar(ty) {
            return self.leaf(ty, owner, path, span);
        }
        let Type::Named { name, arguments } = ty else {
            return Err(Refusal::UnsupportedType(ty.to_string()));
        };
        if !arguments.is_empty() {
            return Err(Refusal::GenericType(name.clone()));
        }
        if active.len() >= MAX_DEPTH {
            return Err(Refusal::Capacity);
        }
        let declaration = self.declaration(name)?;
        if !declaration.type_parameters.is_empty() {
            return Err(Refusal::GenericType(name.clone()));
        }
        if active.contains(&declaration.stable_id) {
            return Err(Refusal::RecursiveType(declaration.stable_id.clone()));
        }
        let id = declaration.stable_id.clone();
        active.push(id.clone());
        self.declarations.insert(id.clone());
        let result = match &declaration.kind {
            TypeDeclarationKind::Record { fields } => {
                if fields.is_empty() || fields.len() > MAX_FIELDS {
                    return Err(Refusal::Capacity);
                }
                let mut values = BTreeMap::new();
                for field in fields {
                    let mut field_path = path.to_vec();
                    field_path.push(field.stable_id.clone());
                    let value = self.flatten(&field.ty, Some(&id), &field_path, active, span)?;
                    values.insert(field.stable_id.clone(), value);
                }
                Value::Record {
                    declaration_id: id,
                    fields: values,
                }
            }
            TypeDeclarationKind::Variant { cases } => {
                if cases.is_empty() || cases.len() > MAX_CASES {
                    return Err(Refusal::Capacity);
                }
                let tag = scalar_value(self.leaf(&Type::U8, Some(&id), path, span)?)?.0;
                let mut payloads = BTreeMap::new();
                for case in cases {
                    if case.fields.len() > MAX_FIELDS {
                        return Err(Refusal::Capacity);
                    }
                    self.declarations.insert(case.stable_id.clone());
                    let mut fields = BTreeMap::new();
                    for field in &case.fields {
                        let mut field_path = path.to_vec();
                        field_path.push(case.stable_id.clone());
                        field_path.push(field.stable_id.clone());
                        let value =
                            self.flatten(&field.ty, Some(&id), &field_path, active, span)?;
                        fields.insert(field.stable_id.clone(), value);
                    }
                    payloads.insert(case.stable_id.clone(), fields);
                }
                let bound = expression(ExprKind::Uint8(cases.len() as u8), span);
                self.tag_requires.push(expression(
                    ExprKind::Binary {
                        op: BinaryOp::Lt,
                        left: Box::new(tag.clone()),
                        right: Box::new(bound),
                    },
                    span,
                ));
                Value::Variant {
                    declaration_id: id,
                    tag,
                    cases: payloads,
                }
            }
            _ => return Err(Refusal::UnsupportedType(name.clone())),
        };
        active.pop();
        Ok(result)
    }

    fn lookup(&self, name: &str) -> Result<Value, Refusal> {
        self.scope
            .iter()
            .rev()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
            .ok_or_else(|| Refusal::UnknownBinding(name.to_owned()))
    }

    fn field(&mut self, base: Value, name: &str) -> Result<Value, Refusal> {
        let Value::Record {
            declaration_id,
            fields,
        } = base
        else {
            return Err(Refusal::TypeMismatch);
        };
        let declaration = self
            .types
            .values()
            .copied()
            .find(|decl| decl.stable_id == declaration_id)
            .ok_or_else(|| Refusal::MissingType(declaration_id.clone()))?;
        let TypeDeclarationKind::Record { fields: schema } = &declaration.kind else {
            return Err(Refusal::TypeMismatch);
        };
        let field = schema
            .iter()
            .find(|field| field.name == name)
            .ok_or_else(|| Refusal::WrongField(name.to_owned()))?;
        self.declarations.insert(field.stable_id.clone());
        fields
            .get(&field.stable_id)
            .cloned()
            .ok_or_else(|| Refusal::WrongField(name.to_owned()))
    }

    fn matches_type(&self, value: &Value, ty: &Type) -> bool {
        match (value, ty) {
            (Value::Scalar(_, found), expected) => found == expected,
            (
                Value::Record { declaration_id, .. } | Value::Variant { declaration_id, .. },
                Type::Named { name, arguments },
            ) => {
                arguments.is_empty()
                    && self
                        .types
                        .get(name.as_str())
                        .is_some_and(|decl| decl.stable_id == *declaration_id)
            }
            _ => false,
        }
    }

    fn construct_record(
        &mut self,
        name: &str,
        args: &[Type],
        fields: &[crate::ast::FieldInitializer],
    ) -> Result<Value, Refusal> {
        if !args.is_empty() {
            return Err(Refusal::GenericType(name.to_owned()));
        }
        let declaration = self.declaration(name)?;
        let TypeDeclarationKind::Record { fields: schema } = &declaration.kind else {
            return Err(Refusal::UnsupportedType(name.to_owned()));
        };
        if schema.len() != fields.len() || schema.len() > MAX_FIELDS {
            return Err(Refusal::WrongField(name.to_owned()));
        }
        let mut values = BTreeMap::new();
        for initializer in fields {
            let field = schema
                .iter()
                .find(|field| field.name == initializer.name)
                .ok_or_else(|| Refusal::WrongField(initializer.name.clone()))?;
            if values.contains_key(&field.stable_id) {
                return Err(Refusal::WrongField(initializer.name.clone()));
            }
            let value = self.lower(&initializer.value)?;
            if !self.matches_type(&value, &field.ty) {
                return Err(Refusal::TypeMismatch);
            }
            self.declarations.insert(field.stable_id.clone());
            values.insert(field.stable_id.clone(), value);
        }
        self.declarations.insert(declaration.stable_id.clone());
        Ok(Value::Record {
            declaration_id: declaration.stable_id.clone(),
            fields: values,
        })
    }

    fn construct_variant(
        &mut self,
        name: &str,
        args: &[Type],
        case_name: &str,
        fields: &[crate::ast::FieldInitializer],
        span: Span,
    ) -> Result<Value, Refusal> {
        if !args.is_empty() {
            return Err(Refusal::GenericType(name.to_owned()));
        }
        let declaration = self.declaration(name)?;
        let TypeDeclarationKind::Variant { cases } = &declaration.kind else {
            return Err(Refusal::UnsupportedType(name.to_owned()));
        };
        if cases.len() > MAX_CASES {
            return Err(Refusal::Capacity);
        }
        let Some((index, case)) = cases
            .iter()
            .enumerate()
            .find(|(_, case)| case.name == case_name)
        else {
            return Err(Refusal::MatchPattern);
        };
        if case.fields.len() != fields.len() {
            return Err(Refusal::WrongField(case_name.to_owned()));
        }
        let mut values = BTreeMap::new();
        for initializer in fields {
            let field = case
                .fields
                .iter()
                .find(|field| field.name == initializer.name)
                .ok_or_else(|| Refusal::WrongField(initializer.name.clone()))?;
            if values.contains_key(&field.stable_id) {
                return Err(Refusal::WrongField(initializer.name.clone()));
            }
            let value = self.lower(&initializer.value)?;
            if !self.matches_type(&value, &field.ty) {
                return Err(Refusal::TypeMismatch);
            }
            self.declarations.insert(field.stable_id.clone());
            values.insert(field.stable_id.clone(), value);
        }
        let mut payloads = BTreeMap::new();
        payloads.insert(case.stable_id.clone(), values);
        self.declarations.insert(declaration.stable_id.clone());
        self.declarations.insert(case.stable_id.clone());
        Ok(Value::Variant {
            declaration_id: declaration.stable_id.clone(),
            tag: expression(ExprKind::Uint8(index as u8), span),
            cases: payloads,
        })
    }

    fn lower(&mut self, expr: &Expr) -> Result<Value, Refusal> {
        let span = expr.span;
        match &expr.kind {
            ExprKind::Int(_)
            | ExprKind::Int32(_)
            | ExprKind::Uint8(_)
            | ExprKind::Usize(_)
            | ExprKind::Bool(_) => {
                let ty = match &expr.kind {
                    ExprKind::Int(_) => Type::I64,
                    ExprKind::Int32(_) => Type::I32,
                    ExprKind::Uint8(_) => Type::U8,
                    ExprKind::Usize(_) => Type::Usize,
                    ExprKind::Bool(_) => Type::Bool,
                    _ => unreachable!(),
                };
                Ok(Value::Scalar(expr.clone(), ty))
            }
            ExprKind::Var(name) => self.lookup(name),
            ExprKind::Project { base, field, .. } => {
                let value = self.lower(base)?;
                self.field(value, field)
            }
            ExprKind::Unary { op, value } => {
                let (value, ty) = scalar_value(self.lower(value)?)?;
                let ty = if matches!(op, crate::ast::UnaryOp::Not) {
                    Type::Bool
                } else {
                    ty
                };
                let lowered = expression(
                    ExprKind::Unary {
                        op: *op,
                        value: Box::new(value),
                    },
                    span,
                );
                if matches!(op, crate::ast::UnaryOp::Neg) {
                    self.touch(&lowered, &ty);
                }
                Ok(Value::Scalar(lowered, ty))
            }
            ExprKind::Binary { op, left, right } => {
                if matches!(op, BinaryOp::Div | BinaryOp::Rem) {
                    return Err(Refusal::UnsupportedExpression("division or remainder"));
                }
                let (left, left_ty) = scalar_value(self.lower(left)?)?;
                if matches!(op, BinaryOp::And | BinaryOp::Or) {
                    let guard = if *op == BinaryOp::And {
                        left.clone()
                    } else {
                        expression(
                            ExprKind::Unary {
                                op: crate::ast::UnaryOp::Not,
                                value: Box::new(left.clone()),
                            },
                            span,
                        )
                    };
                    self.guards.push(guard);
                }
                let (right, right_ty) = scalar_value(self.lower(right)?)?;
                if matches!(op, BinaryOp::And | BinaryOp::Or) {
                    self.guards.pop();
                }
                if left_ty != right_ty {
                    return Err(Refusal::TypeMismatch);
                }
                let ty = if matches!(op, BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul) {
                    left_ty
                } else {
                    Type::Bool
                };
                let value = expression(
                    ExprKind::Binary {
                        op: *op,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                    span,
                );
                if matches!(op, BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul) {
                    self.touch(&value, &ty);
                }
                Ok(Value::Scalar(value, ty))
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let (condition, condition_ty) = scalar_value(self.lower(condition)?)?;
                if condition_ty != Type::Bool {
                    return Err(Refusal::TypeMismatch);
                }
                self.guards.push(condition.clone());
                let then_value = self.lower(then_branch)?;
                self.guards.pop();
                self.guards.push(expression(
                    ExprKind::Unary {
                        op: crate::ast::UnaryOp::Not,
                        value: Box::new(condition.clone()),
                    },
                    span,
                ));
                let else_value = self.lower(else_branch)?;
                self.guards.pop();
                let (then_branch, then_ty) = scalar_value(then_value)?;
                let (else_branch, else_ty) = scalar_value(else_value)?;
                if then_ty != else_ty {
                    return Err(Refusal::TypeMismatch);
                }
                Ok(Value::Scalar(
                    expression(
                        ExprKind::If {
                            condition: Box::new(condition),
                            then_branch: Box::new(then_branch),
                            else_branch: Box::new(else_branch),
                        },
                        span,
                    ),
                    then_ty,
                ))
            }
            ExprKind::Block { statements, tail } => {
                let depth = self.scope.len();
                for statement in statements {
                    let Statement::Let {
                        name,
                        mutable: false,
                        declared,
                        value,
                        ..
                    } = statement
                    else {
                        return Err(Refusal::UnsupportedExpression(
                            "mutable or non-let statement",
                        ));
                    };
                    let value = self.lower(value)?;
                    if declared
                        .as_ref()
                        .is_some_and(|ty| !self.matches_type(&value, ty))
                    {
                        return Err(Refusal::TypeMismatch);
                    }
                    self.scope.push((name.clone(), value));
                }
                let result = self.lower(tail);
                self.scope.truncate(depth);
                result
            }
            ExprKind::Match {
                mode: MatchMode::Value,
                scrutinee,
                arms,
            } => self.lower_match(scrutinee, arms, span),
            ExprKind::Match { .. } => Err(Refusal::Ownership),
            ExprKind::ConstructRecord {
                type_name,
                type_arguments,
                fields,
                ..
            } => self.construct_record(type_name, type_arguments, fields),
            ExprKind::ConstructVariant {
                type_name,
                type_arguments,
                case_name,
                fields,
                ..
            } => self.construct_variant(type_name, type_arguments, case_name, fields, span),
            _ => Err(Refusal::UnsupportedExpression(
                "outside finite aggregate profile",
            )),
        }
    }

    fn lower_match(
        &mut self,
        scrutinee: &Expr,
        arms: &[crate::ast::MatchArm],
        span: Span,
    ) -> Result<Value, Refusal> {
        let Value::Variant {
            declaration_id,
            tag,
            cases: payloads,
        } = self.lower(scrutinee)?
        else {
            return Err(Refusal::TypeMismatch);
        };
        let declaration = self
            .types
            .values()
            .copied()
            .find(|decl| decl.stable_id == declaration_id)
            .ok_or_else(|| Refusal::MissingType(declaration_id.clone()))?;
        let TypeDeclarationKind::Variant { cases } = &declaration.kind else {
            return Err(Refusal::TypeMismatch);
        };
        if arms.len() != cases.len() {
            return Err(Refusal::MatchCoverage);
        }
        let mut seen = BTreeSet::new();
        let mut lowered = Vec::new();
        for arm in arms {
            if arm.guard.is_some() {
                return Err(Refusal::MatchPattern);
            }
            let MatchPattern::Variant {
                type_name,
                case_name,
                fields,
                ..
            } = &arm.pattern
            else {
                return Err(Refusal::MatchPattern);
            };
            if type_name != &declaration.name {
                return Err(Refusal::MatchPattern);
            }
            let Some((case_index, case)) = cases
                .iter()
                .enumerate()
                .find(|(_, case)| case.name == *case_name)
            else {
                return Err(Refusal::MatchPattern);
            };
            if !seen.insert(case.stable_id.clone()) {
                return Err(Refusal::MatchCoverage);
            }
            let payload = payloads.get(&case.stable_id);
            if payload.is_none() && payloads.len() != 1 {
                return Err(Refusal::MatchPattern);
            }
            let depth = self.scope.len();
            let mut bound_fields = BTreeSet::new();
            for binding in fields {
                let Some(field) = case.fields.iter().find(|field| field.name == binding.name)
                else {
                    return Err(Refusal::MatchPattern);
                };
                if !bound_fields.insert(field.stable_id.clone()) {
                    return Err(Refusal::MatchPattern);
                }
                let Some(payload) = payload else {
                    continue;
                };
                let value = payload
                    .get(&field.stable_id)
                    .ok_or(Refusal::MatchPattern)?
                    .clone();
                self.declarations.insert(field.stable_id.clone());
                self.scope.push((binding.binding.clone(), value));
            }
            let value = if payload.is_some() {
                let guard = expression(
                    ExprKind::Binary {
                        op: BinaryOp::Eq,
                        left: Box::new(tag.clone()),
                        right: Box::new(expression(ExprKind::Uint8(case_index as u8), span)),
                    },
                    span,
                );
                self.guards.push(guard);
                let value = scalar_value(self.lower(&arm.value)?)?;
                self.guards.pop();
                Some(value)
            } else {
                None
            };
            self.scope.truncate(depth);
            self.declarations.insert(case.stable_id.clone());
            if let Some(value) = value {
                lowered.push((case_index, value));
            }
        }
        if seen.len() != cases.len() {
            return Err(Refusal::MatchCoverage);
        }
        let (_, (mut value, ty)) = lowered.pop().ok_or(Refusal::MatchCoverage)?;
        for (index, (arm_value, arm_ty)) in lowered.into_iter().rev() {
            if arm_ty != ty {
                return Err(Refusal::TypeMismatch);
            }
            let test = expression(
                ExprKind::Binary {
                    op: BinaryOp::Eq,
                    left: Box::new(tag.clone()),
                    right: Box::new(expression(ExprKind::Uint8(index as u8), span)),
                },
                span,
            );
            value = expression(
                ExprKind::If {
                    condition: Box::new(test),
                    then_branch: Box::new(arm_value),
                    else_branch: Box::new(value),
                },
                span,
            );
        }
        Ok(Value::Scalar(value, ty))
    }
}

/// Lower one checked same-module scalar-result law to a scalar subject.
pub fn lower(program: &Program, function: &Function) -> Result<Lowered, Refusal> {
    if !function.type_parameters.is_empty()
        || !function.effects.is_empty()
        || function.yields.is_some()
    {
        return Err(Refusal::Impure);
    }
    if !scalar(&function.return_type) {
        return Err(Refusal::UnsupportedType(function.return_type.to_string()));
    }
    let types = program
        .types
        .iter()
        .map(|decl| (decl.name.as_str(), decl))
        .collect();
    let mut lowerer = Lowerer {
        types,
        names: function
            .params
            .iter()
            .map(|param| param.name.clone())
            .collect(),
        params: Vec::new(),
        leaves: Vec::new(),
        declarations: BTreeSet::new(),
        scope: Vec::new(),
        next_name: 0,
        tag_requires: Vec::new(),
        collect_touches: false,
        guards: Vec::new(),
        touches: Vec::new(),
    };
    for param in &function.params {
        if param.mode != ParamMode::Value {
            return Err(Refusal::Ownership);
        }
        let value = lowerer.flatten(&param.ty, None, &[], &mut Vec::new(), param.span)?;
        lowerer.scope.push((param.name.clone(), value));
    }
    let mut requires = std::mem::take(&mut lowerer.tag_requires);
    for clause in &function.requires {
        let (expr, ty) = scalar_value(lowerer.lower(clause)?)?;
        if ty != Type::Bool {
            return Err(Refusal::TypeMismatch);
        }
        requires.push(expr);
    }
    lowerer.collect_touches = true;
    let (mut body, body_ty) = scalar_value(lowerer.lower(&function.body)?)?;
    lowerer.collect_touches = false;
    if body_ty != function.return_type {
        return Err(Refusal::TypeMismatch);
    }
    let mut touch_statements = Vec::new();
    for (guards, value, ty) in std::mem::take(&mut lowerer.touches) {
        let name = lowerer.fresh();
        touch_statements.push(Statement::Let {
            name,
            name_span: value.span,
            mutable: false,
            declared: Some(ty.clone()),
            value: guarded_touch_value(guards, value, &ty)?,
            span: function.body.span,
        });
    }
    if !touch_statements.is_empty() {
        body = expression(
            ExprKind::Block {
                statements: touch_statements,
                tail: Box::new(body),
            },
            function.body.span,
        );
    }
    lowerer.scope.push((
        "result".to_owned(),
        Value::Scalar(
            expression(ExprKind::Var("result".to_owned()), function.body.span),
            body_ty,
        ),
    ));
    let mut ensures = Vec::new();
    for clause in &function.ensures {
        let (expr, ty) = scalar_value(lowerer.lower(clause)?)?;
        if ty != Type::Bool {
            return Err(Refusal::TypeMismatch);
        }
        ensures.push(expr);
    }
    let mut scalar_function = function.clone();
    scalar_function.params = lowerer.params;
    scalar_function.requires = requires;
    scalar_function.ensures = ensures;
    scalar_function.body = body;
    Ok(Lowered {
        scalar: scalar_function,
        leaves: lowerer.leaves,
        declaration_ids: lowerer.declarations.into_iter().collect(),
        source_ensures_index: None,
        profile: PROFILE,
    })
}

fn zero(ty: &Type, span: Span) -> Result<Expr, Refusal> {
    let kind = match ty {
        Type::I64 => ExprKind::Int(0),
        Type::I32 => ExprKind::Int32(0),
        Type::U8 => ExprKind::Uint8(0),
        Type::Usize => ExprKind::Usize(0),
        _ => return Err(Refusal::TypeMismatch),
    };
    Ok(expression(kind, span))
}

fn guarded_touch_value(guards: Vec<Expr>, mut value: Expr, ty: &Type) -> Result<Expr, Refusal> {
    let span = value.span;
    for guard in guards.into_iter().rev() {
        value = expression(
            ExprKind::If {
                condition: Box::new(guard),
                then_branch: Box::new(value),
                else_branch: Box::new(zero(ty, span)?),
            },
            span,
        );
    }
    Ok(value)
}

fn touch_predicate(guards: Vec<Expr>, value: Expr) -> Expr {
    let span = value.span;
    let mut checked = expression(
        ExprKind::Binary {
            op: BinaryOp::Eq,
            left: Box::new(value.clone()),
            right: Box::new(value),
        },
        span,
    );
    for guard in guards.into_iter().rev() {
        checked = expression(
            ExprKind::If {
                condition: Box::new(guard),
                then_branch: Box::new(checked),
                else_branch: Box::new(expression(ExprKind::Bool(true), span)),
            },
            span,
        );
    }
    checked
}

/// Build a proof query for one aggregate-result postcondition. The query's
/// body evaluates every arithmetic operation from the original body, in
/// source order and under its exact path, before checking the selected law.
/// All source clauses are lowered first, so one unsupported clause refuses
/// the whole function rather than lending partial proved status.
pub fn lower_aggregate_clause(
    program: &Program,
    function: &Function,
    ensures_index: usize,
) -> Result<Lowered, Refusal> {
    if !function.type_parameters.is_empty()
        || !function.effects.is_empty()
        || function.yields.is_some()
    {
        return Err(Refusal::Impure);
    }
    if scalar(&function.return_type) || ensures_index >= function.ensures.len() {
        return Err(Refusal::UnsupportedType(function.return_type.to_string()));
    }
    let types = program
        .types
        .iter()
        .map(|decl| (decl.name.as_str(), decl))
        .collect();
    let mut lowerer = Lowerer {
        types,
        names: function
            .params
            .iter()
            .map(|param| param.name.clone())
            .collect(),
        params: Vec::new(),
        leaves: Vec::new(),
        declarations: BTreeSet::new(),
        scope: Vec::new(),
        next_name: 0,
        tag_requires: Vec::new(),
        collect_touches: false,
        guards: Vec::new(),
        touches: Vec::new(),
    };
    let mut result_leaves = 0;
    lowerer.validate_shape(&function.return_type, &mut Vec::new(), &mut result_leaves)?;
    for param in &function.params {
        if param.mode != ParamMode::Value {
            return Err(Refusal::Ownership);
        }
        let value = lowerer.flatten(&param.ty, None, &[], &mut Vec::new(), param.span)?;
        lowerer.scope.push((param.name.clone(), value));
    }
    let mut requires = std::mem::take(&mut lowerer.tag_requires);
    for clause in &function.requires {
        let (expr, ty) = scalar_value(lowerer.lower(clause)?)?;
        if ty != Type::Bool {
            return Err(Refusal::TypeMismatch);
        }
        requires.push(expr);
    }
    lowerer.collect_touches = true;
    let result_value = lowerer.lower(&function.body)?;
    lowerer.collect_touches = false;
    if !lowerer.matches_type(&result_value, &function.return_type) {
        return Err(Refusal::TypeMismatch);
    }
    lowerer.scope.push(("result".to_owned(), result_value));
    let mut clauses = Vec::new();
    for clause in &function.ensures {
        let (expr, ty) = scalar_value(lowerer.lower(clause)?)?;
        if ty != Type::Bool {
            return Err(Refusal::TypeMismatch);
        }
        clauses.push(expr);
    }
    let mut body = clauses.swap_remove(ensures_index);
    for (guards, value, _) in lowerer.touches.into_iter().rev() {
        let checked = touch_predicate(guards, value);
        body = expression(
            ExprKind::Binary {
                op: BinaryOp::And,
                left: Box::new(checked),
                right: Box::new(body),
            },
            function.body.span,
        );
    }
    let mut scalar_function = function.clone();
    scalar_function.params = lowerer.params;
    scalar_function.requires = requires;
    scalar_function.return_type = Type::Bool;
    scalar_function.body = body;
    scalar_function.ensures = vec![expression(
        ExprKind::Var("result".to_owned()),
        function.ensures[ensures_index].span,
    )];
    Ok(Lowered {
        scalar: scalar_function,
        leaves: lowerer.leaves,
        declaration_ids: lowerer.declarations.into_iter().collect(),
        source_ensures_index: Some(ensures_index),
        profile: PROFILE,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assurance_manifest::smt_discharge::{
        discharge_postcondition, provision_from_env, DischargeOutcome, RunLimits,
    };

    const SOURCE: &str = r#"
module law07.accounting;

@id("law07.account")
record Account {
    @id("law07.account.balance") balance: i64,
}

@id("law07.accounts")
record Accounts {
    @id("law07.accounts.debit") debit: Account,
    @id("law07.accounts.credit") credit: Account,
}

@id("law07.outcome")
variant Outcome {
    @id("law07.outcome.success") Success {
        @id("law07.outcome.success.credited") credited: i64,
    },
    @id("law07.outcome.failure") Failure {
        @id("law07.outcome.failure.code") code: i64,
    },
}

@id("law07.sum")
fn sum(input: Accounts) -> i64
    requires input.debit.balance >= 0
    requires input.debit.balance <= 1000
    requires input.credit.balance >= 0
    requires input.credit.balance <= 1000
    ensures result == input.debit.balance + input.credit.balance
{
    input.debit.balance + input.credit.balance
}

@id("law07.transfer")
fn transfer(before: Accounts, amount: i64) -> Accounts
    requires before.debit.balance >= 0
    requires before.debit.balance <= 1000
    requires before.credit.balance >= 0
    requires before.credit.balance <= 1000
    requires amount >= 0
    requires amount <= before.debit.balance
    ensures result.debit.balance == before.debit.balance - amount
    ensures result.credit.balance == before.credit.balance + amount
    ensures result.debit.balance + result.credit.balance == before.debit.balance + before.credit.balance
{
    Accounts {
        debit: Account { balance: before.debit.balance - amount },
        credit: Account { balance: before.credit.balance + amount },
    }
}

@id("law07.failure")
fn failure(code: i64) -> Outcome
    requires code >= 0
    requires code <= 10
    ensures match result {
        Outcome::Success { credited } => false,
        Outcome::Failure { code: observed } => observed == code,
    }
{
    Outcome::Failure { code: code }
}

@id("law07.outcome-zero")
fn outcome_zero(outcome: Outcome) -> i64
    ensures result == 0
{
    match outcome {
        Outcome::Success { credited } => credited - credited,
        Outcome::Failure { code } => code - code,
    }
}

@id("law07.main")
fn main() -> i64
{
    0
}
"#;

    fn checked() -> Program {
        crate::check(SOURCE, "law07_accounting.spx").expect("checked aggregate law source")
    }

    fn selected<'a>(program: &'a Program, id: &str) -> &'a Function {
        program
            .functions
            .iter()
            .find(|function| function.stable_id == id)
            .unwrap()
    }

    #[test]
    fn nested_record_projection_retains_distinct_persistent_field_paths() {
        let program = checked();
        let lowered = lower(&program, selected(&program, "law07.sum")).unwrap();
        assert_eq!(lowered.leaves.len(), 2);
        assert_ne!(lowered.leaves[0].field_path, lowered.leaves[1].field_path);
        assert!(lowered.leaves.iter().all(|leaf| leaf
            .field_path
            .last()
            .is_some_and(|id| id == "law07.account.balance")));
        assert!(lowered
            .declaration_ids
            .contains(&"law07.accounts.credit".to_owned()));
        crate::assurance_manifest::law_vc::build(&lowered.scalar).expect("shared typed VC");
    }

    #[test]
    fn aggregate_result_and_failure_variant_translate_whole_selected_clauses() {
        let program = checked();
        let transfer = selected(&program, "law07.transfer");
        for index in 0..transfer.ensures.len() {
            let lowered = lower_aggregate_clause(&program, transfer, index).unwrap();
            crate::assurance_manifest::smt_discharge::translate_function(&lowered.scalar)
                .expect("complete scalarized clause");
        }
        let failure = selected(&program, "law07.failure");
        let lowered = lower_aggregate_clause(&program, failure, 0).unwrap();
        assert!(lowered
            .declaration_ids
            .contains(&"law07.outcome.failure".to_owned()));
        crate::assurance_manifest::smt_discharge::translate_function(&lowered.scalar)
            .expect("complete constructed-variant match");
        let outcome = lower(&program, selected(&program, "law07.outcome-zero")).unwrap();
        crate::assurance_manifest::smt_discharge::translate_function(&outcome.scalar)
            .expect("complete parameter-variant match");
    }

    #[test]
    #[ignore = "requires explicitly provisioned installed Z3"]
    fn real_z3_two_account_transfer_and_failure_variant() {
        let solver = provision_from_env().expect("explicit Z3");
        let program = checked();
        let transfer = selected(&program, "law07.transfer");
        for index in 0..transfer.ensures.len() {
            let lowered = lower_aggregate_clause(&program, transfer, index).unwrap();
            assert!(matches!(
                discharge_postcondition(&lowered.scalar, 0, Some(&solver), &RunLimits::default()),
                DischargeOutcome::Proved { .. }
            ));
        }
        let failure = selected(&program, "law07.failure");
        let lowered = lower_aggregate_clause(&program, failure, 0).unwrap();
        assert!(matches!(
            discharge_postcondition(&lowered.scalar, 0, Some(&solver), &RunLimits::default()),
            DischargeOutcome::Proved { .. }
        ));
        let outcome = lower(&program, selected(&program, "law07.outcome-zero")).unwrap();
        assert!(matches!(
            discharge_postcondition(&outcome.scalar, 0, Some(&solver), &RunLimits::default()),
            DischargeOutcome::Proved { .. }
        ));
    }

    #[test]
    #[ignore = "requires explicitly provisioned installed Z3"]
    fn real_z3_seeded_transfer_and_failure_mutants_refute() {
        let solver = provision_from_env().expect("explicit Z3");
        for (label, original, replacement, target, clause) in [
            (
                "duplicate debit",
                "debit: Account { balance: before.debit.balance - amount },",
                "debit: Account { balance: before.debit.balance - amount - amount },",
                "law07.transfer",
                0,
            ),
            (
                "wrong credit",
                "credit: Account { balance: before.credit.balance + amount },",
                "credit: Account { balance: before.credit.balance - amount },",
                "law07.transfer",
                1,
            ),
            (
                "non-conservation",
                "credit: Account { balance: before.credit.balance + amount },",
                "credit: Account { balance: before.credit.balance + amount + 1 },",
                "law07.transfer",
                2,
            ),
            (
                "overflow",
                "credit: Account { balance: before.credit.balance + amount },",
                "credit: Account { balance: before.credit.balance + 9223372036854775807 },",
                "law07.transfer",
                1,
            ),
            (
                "wrong failure variant",
                "Outcome::Failure { code: code }\n}",
                "Outcome::Success { credited: code }\n}",
                "law07.failure",
                0,
            ),
        ] {
            let source = SOURCE.replacen(original, replacement, 1);
            assert_ne!(source, SOURCE, "{label}: seeded source mutation missing");
            let program = crate::check(&source, "law07_mutant.spx").unwrap();
            let function = selected(&program, target);
            let lowered = lower_aggregate_clause(&program, function, clause).unwrap();
            let result =
                discharge_postcondition(&lowered.scalar, 0, Some(&solver), &RunLimits::default());
            assert!(
                matches!(result, DischargeOutcome::Refuted { .. }),
                "{label}: {result:?}"
            );
        }
    }

    #[test]
    #[ignore = "requires explicitly provisioned pinned Lean 4.34.0"]
    fn real_lean_record_and_variant_match_prove_and_seeded_false_theorem_refuses() {
        use crate::proof_export::{kernel_report, lean};
        use std::process::Command;
        let tool = std::env::var_os("SEMAPRAX_LAW_LEAN").expect("explicit pinned Lean binary");
        let program = checked();
        let lowered = lower(&program, selected(&program, "law07.sum")).unwrap();
        let variant = lower(&program, selected(&program, "law07.outcome-zero")).unwrap();
        let mut synthetic = program.clone();
        synthetic.types.clear();
        synthetic.functions = vec![lowered.scalar.clone(), variant.scalar];
        let proof = lean::export_structured_module(&synthetic, "law07-record-projection-v1");
        assert!(proof.unsupported.is_empty(), "{:?}", proof.unsupported);
        let root = std::env::temp_dir().join(format!(
            "semaprax-law07-lean-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("AccountProof.lean");
        std::fs::write(&file, &proof.lean_source).unwrap();
        let run = Command::new(&tool)
            .arg(&file)
            .output()
            .expect("run pinned Lean");
        let transcript = format!(
            "{}{}",
            String::from_utf8_lossy(&run.stdout),
            String::from_utf8_lossy(&run.stderr)
        );
        assert!(run.status.success(), "{transcript}");
        assert!(
            matches!(
                kernel_report::parse(
                    &proof.theorem_names(),
                    kernel_report::PINNED_TOOLCHAIN,
                    &transcript
                ),
                kernel_report::KernelVerdict::Checked { .. }
            ),
            "{transcript}"
        );

        let mut wrong = lowered.scalar;
        let mut altered = wrong.ensures[0].clone();
        let ExprKind::Binary { right, .. } = &mut altered.kind else {
            panic!("equality")
        };
        *right = Box::new(expression(ExprKind::Int(0), altered.span));
        wrong.ensures[0] = altered;
        synthetic.functions = vec![wrong];
        let violation = lean::export_structured_module(&synthetic, "law07-record-projection-v1");
        std::fs::write(&file, &violation.lean_source).unwrap();
        let rejected = Command::new(&tool)
            .arg(&file)
            .output()
            .expect("run pinned Lean negative");
        let negative = format!(
            "{}{}",
            String::from_utf8_lossy(&rejected.stdout),
            String::from_utf8_lossy(&rejected.stderr)
        );
        assert!(
            !rejected.status.success()
                || !matches!(
                    kernel_report::parse(
                        &violation.theorem_names(),
                        kernel_report::PINNED_TOOLCHAIN,
                        &negative
                    ),
                    kernel_report::KernelVerdict::Checked { .. }
                ),
            "wrong structured-state law unexpectedly checked"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
