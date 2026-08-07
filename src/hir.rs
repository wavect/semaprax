//! Resolved high-level intermediate representation.
//!
//! The parsed AST keeps the names humans wrote. HIR replaces every nominal,
//! callable, and value reference with a deterministic identity. Backends should
//! consume this layer as the language grows rather than repeating name lookup.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fmt::Write as _;

use crate::ast::{BinaryOp, Expr, ExprKind, ParamMode, Program, Span, Statement, Type, UnaryOp};
use crate::diagnostic::Diagnostic;
use crate::verify;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DeclarationId(String);

impl DeclarationId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DeclarationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ValueId(String);

impl ValueId {
    fn parameter(function: &DeclarationId, index: usize) -> Self {
        Self(scoped_identity(function, "value:param", &index.to_string()))
    }

    fn local(function: &DeclarationId, path: &str) -> Self {
        Self(scoped_identity(function, "value:local", path))
    }

    fn result(function: &DeclarationId) -> Self {
        Self(scoped_identity(function, "value:result", ""))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ValueId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExpressionId(String);

impl ExpressionId {
    fn new(function: &DeclarationId, path: &str) -> Self {
        Self(scoped_identity(function, "expression", path))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn scoped_identity(owner: &DeclarationId, kind: &str, path: &str) -> String {
    format!(
        "declaration:{}:{}:{kind}:{}:{path}",
        owner.as_str().len(),
        owner,
        path.len()
    )
}

impl fmt::Display for ExpressionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclarationKind {
    Resource,
    Function,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Declaration {
    pub id: DeclarationId,
    pub name: String,
    pub kind: DeclarationKind,
}

/// A deterministic, display-name-to-identity index.
///
/// Types and values occupy distinct namespaces so future record/variant type
/// declarations can coexist with functions without ambiguous lookup.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeclarationIndex {
    declarations: BTreeMap<DeclarationId, Declaration>,
    types_by_name: BTreeMap<String, DeclarationId>,
    functions_by_name: BTreeMap<String, DeclarationId>,
}

impl DeclarationIndex {
    pub fn declaration(&self, id: &DeclarationId) -> Option<&Declaration> {
        self.declarations.get(id)
    }

    pub fn type_id(&self, name: &str) -> Option<&DeclarationId> {
        self.types_by_name.get(name)
    }

    pub fn function_id(&self, name: &str) -> Option<&DeclarationId> {
        self.functions_by_name.get(name)
    }

    pub fn declarations(&self) -> impl ExactSizeIterator<Item = &Declaration> {
        self.declarations.values()
    }

    /// Computes the recursive semantic facts shared by ownership and backends.
    ///
    /// `None` is reserved for unresolved type parameters and future malformed
    /// HIR. Every type produced for today's verified language has facts.
    pub fn type_facts(&self, ty: &ResolvedType) -> Option<TypeFacts> {
        match ty {
            ResolvedType::I64 => Some(TypeFacts {
                copy: true,
                contains_resource: false,
                sized: true,
                needs_drop: false,
                layout_key: "scalar:i64".to_owned(),
            }),
            ResolvedType::Bool => Some(TypeFacts {
                copy: true,
                contains_resource: false,
                sized: true,
                needs_drop: false,
                layout_key: "scalar:bool".to_owned(),
            }),
            ResolvedType::TypeParameter { .. } => None,
            ResolvedType::Nominal { declaration, .. } => {
                let declaration = self.declaration(declaration)?;
                match declaration.kind {
                    DeclarationKind::Resource => Some(TypeFacts {
                        copy: false,
                        contains_resource: true,
                        sized: true,
                        needs_drop: true,
                        layout_key: format!("resource:{}", ty.identity_key()),
                    }),
                    DeclarationKind::Function => None,
                }
            }
        }
    }

    fn from_verified(program: &Program) -> Self {
        let mut index = Self::default();
        for resource in &program.resources {
            index.insert(
                resource.name.clone(),
                DeclarationId::new(resource.stable_id.clone()),
                DeclarationKind::Resource,
            );
        }
        for function in &program.functions {
            index.insert(
                function.name.clone(),
                DeclarationId::new(function.stable_id.clone()),
                DeclarationKind::Function,
            );
        }
        index
    }

    fn insert(&mut self, name: String, id: DeclarationId, kind: DeclarationKind) {
        let namespace = match kind {
            DeclarationKind::Resource => &mut self.types_by_name,
            DeclarationKind::Function => &mut self.functions_by_name,
        };
        namespace.insert(name.clone(), id.clone());
        self.declarations
            .insert(id.clone(), Declaration { id, name, kind });
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResolvedType {
    I64,
    Bool,
    TypeParameter {
        owner: DeclarationId,
        index: u32,
    },
    Nominal {
        declaration: DeclarationId,
        arguments: Vec<ResolvedType>,
    },
}

impl ResolvedType {
    pub fn nominal_id(&self) -> Option<&DeclarationId> {
        match self {
            Self::Nominal { declaration, .. } => Some(declaration),
            Self::I64 | Self::Bool | Self::TypeParameter { .. } => None,
        }
    }

    /// A name-independent key suitable as an input to future layout hashing.
    pub fn identity_key(&self) -> String {
        match self {
            Self::I64 => "i64".to_owned(),
            Self::Bool => "bool".to_owned(),
            Self::TypeParameter { owner, index } => {
                format!("parameter:{}:{}:{index}", owner.as_str().len(), owner)
            }
            Self::Nominal {
                declaration,
                arguments,
            } => {
                let argument_count = arguments.len();
                let encoded_arguments =
                    arguments
                        .iter()
                        .fold(String::new(), |mut output, argument| {
                            let key = argument.identity_key();
                            write!(output, "{}:{key}", key.len())
                                .expect("writing to a string cannot fail");
                            output
                        });
                format!(
                    "nominal:{}:{}:{}:{}",
                    declaration.as_str().len(),
                    declaration,
                    argument_count,
                    encoded_arguments
                )
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeFacts {
    pub copy: bool,
    pub contains_resource: bool,
    pub sized: bool,
    pub needs_drop: bool,
    pub layout_key: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnershipMode {
    Value,
    Own,
    Borrow,
    Shared,
}

impl From<ParamMode> for OwnershipMode {
    fn from(mode: ParamMode) -> Self {
        match mode {
            ParamMode::Value => Self::Value,
            ParamMode::Own => Self::Own,
            ParamMode::Borrow => Self::Borrow,
            ParamMode::Shared => Self::Shared,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedProgram {
    pub module: String,
    pub permits: Vec<String>,
    pub entrypoint: DeclarationId,
    pub declarations: DeclarationIndex,
    pub types: Vec<ResolvedTypeDeclaration>,
    pub functions: Vec<ResolvedFunction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedTypeDeclaration {
    pub id: DeclarationId,
    pub name: String,
    pub kind: ResolvedTypeDeclarationKind,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedTypeDeclarationKind {
    Resource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFunction {
    pub id: DeclarationId,
    pub name: String,
    pub params: Vec<ResolvedParam>,
    pub result_id: ValueId,
    pub return_type: ResolvedType,
    pub effects: Vec<String>,
    pub requires: Vec<ResolvedExpr>,
    pub ensures: Vec<ResolvedExpr>,
    pub body: ResolvedExpr,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedParam {
    pub id: ValueId,
    pub name: String,
    pub ownership: OwnershipMode,
    pub ty: ResolvedType,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedBinding {
    pub id: ValueId,
    pub name: String,
    pub ownership: OwnershipMode,
    pub ty: ResolvedType,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedExpr {
    pub id: ExpressionId,
    pub ty: ResolvedType,
    pub ownership: OwnershipMode,
    pub kind: ResolvedExprKind,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedExprKind {
    Int(i64),
    Bool(bool),
    Place(Place),
    Call {
        callee: DeclarationId,
        args: Vec<ResolvedExpr>,
    },
    Unary {
        op: UnaryOp,
        value: Box<ResolvedExpr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<ResolvedExpr>,
        right: Box<ResolvedExpr>,
    },
    Block {
        statements: Vec<ResolvedStatement>,
        tail: Box<ResolvedExpr>,
    },
    If {
        condition: Box<ResolvedExpr>,
        then_branch: Box<ResolvedExpr>,
        else_branch: Box<ResolvedExpr>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedStatement {
    Let {
        binding: ResolvedBinding,
        value: ResolvedExpr,
        span: Span,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Place {
    pub root: ValueId,
    pub projections: Vec<PlaceProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlaceProjection {
    Field(DeclarationId),
    VariantField {
        case: DeclarationId,
        field: DeclarationId,
    },
}

#[derive(Clone)]
struct Binding {
    id: ValueId,
    ty: ResolvedType,
    ownership: OwnershipMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Availability {
    Available,
    Moved,
    MaybeMoved,
}

impl Availability {
    fn join(self, other: Self) -> Self {
        if self == other {
            self
        } else {
            Self::MaybeMoved
        }
    }
}

#[derive(Clone)]
struct ValidationBinding {
    ty: ResolvedType,
    ownership: OwnershipMode,
    availability: Availability,
}

/// Verify and resolve a parsed program into deterministic HIR.
///
/// Verification errors are returned unchanged. This makes the HIR boundary
/// fail closed: no backend can accidentally resolve and execute an invalid AST.
pub fn resolve(program: &Program) -> Result<ResolvedProgram, Vec<Diagnostic>> {
    let diagnostics = verify::verify(program);
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity.is_error())
    {
        return Err(diagnostics);
    }

    let declarations = DeclarationIndex::from_verified(program);
    Resolver {
        program,
        declarations,
    }
    .resolve()
    .map_err(|diagnostic| vec![diagnostic])
}

/// Validate an identity-resolved program before a semantic consumer uses it.
///
/// Resolved HIR is intentionally public for agent and compiler integrations,
/// so callers can inspect or transform HIR produced by [`resolve`]. Every
/// backend calls this function and therefore fails closed when a transformation
/// breaks identities, lexical scope, or current type rules. A versioned wire
/// schema for constructing HIR outside the compiler is future work.
pub fn validate(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    HirValidator::new(program)?.validate()
}

struct HirValidator<'a> {
    program: &'a ResolvedProgram,
    functions: BTreeMap<DeclarationId, &'a ResolvedFunction>,
    expression_ids: BTreeSet<ExpressionId>,
    value_ids: BTreeSet<ValueId>,
}

impl<'a> HirValidator<'a> {
    fn new(program: &'a ResolvedProgram) -> Result<Self, Diagnostic> {
        let mut functions = BTreeMap::new();
        for function in &program.functions {
            if functions.insert(function.id.clone(), function).is_some() {
                return Err(hir_error(format!(
                    "duplicate resolved function identity `{}`",
                    function.id
                )));
            }
            if program
                .declarations
                .declaration(&function.id)
                .is_none_or(|declaration| declaration.kind != DeclarationKind::Function)
            {
                return Err(hir_error(format!(
                    "resolved function `{}` is absent from the declaration index",
                    function.id
                )));
            }
        }
        Ok(Self {
            program,
            functions,
            expression_ids: BTreeSet::new(),
            value_ids: BTreeSet::new(),
        })
    }

    fn validate(mut self) -> Result<(), Diagnostic> {
        let entrypoint = self
            .functions
            .get(&self.program.entrypoint)
            .ok_or_else(|| hir_error("resolved entry point is not indexed"))?;
        if !entrypoint.params.is_empty() || entrypoint.return_type != ResolvedType::I64 {
            return Err(hir_error(
                "resolved entry point must have type `fn main() -> i64`",
            ));
        }

        let type_ids = self
            .program
            .types
            .iter()
            .map(|declaration| declaration.id.clone())
            .collect::<BTreeSet<_>>();
        if type_ids.len() != self.program.types.len() {
            return Err(hir_error("duplicate resolved type declaration identity"));
        }
        for declaration in &self.program.types {
            if self
                .program
                .declarations
                .declaration(&declaration.id)
                .is_none_or(|item| item.kind != DeclarationKind::Resource)
            {
                return Err(hir_error(format!(
                    "resolved type `{}` is absent from the declaration index",
                    declaration.id
                )));
            }
        }
        for declaration in self.program.declarations.declarations() {
            match declaration.kind {
                DeclarationKind::Resource if !type_ids.contains(&declaration.id) => {
                    return Err(hir_error(format!(
                        "resource `{}` has no resolved type declaration",
                        declaration.id
                    )));
                }
                DeclarationKind::Function if !self.functions.contains_key(&declaration.id) => {
                    return Err(hir_error(format!(
                        "function `{}` has no resolved function body",
                        declaration.id
                    )));
                }
                DeclarationKind::Resource | DeclarationKind::Function => {}
            }
        }

        for function in &self.program.functions {
            self.validate_function(function)?;
        }
        Ok(())
    }

    fn validate_function(&mut self, function: &ResolvedFunction) -> Result<(), Diagnostic> {
        self.validate_type(&function.return_type)?;
        let permits = self
            .program
            .permits
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        for effect in &function.effects {
            if !permits.contains(effect.as_str()) {
                return Err(hir_error(format!(
                    "function `{}` declares effect `{effect}` which the module does not permit",
                    function.id
                )));
            }
        }
        let declared_effects = function.effects.iter().cloned().collect::<BTreeSet<_>>();
        let mut scope = BTreeMap::new();
        for (index, param) in function.params.iter().enumerate() {
            let expected = ValueId::parameter(&function.id, index);
            if param.id != expected {
                return Err(hir_error(format!(
                    "parameter {} of `{}` has a non-canonical identity",
                    index, function.id
                )));
            }
            self.insert_value(&param.id)?;
            self.validate_type(&param.ty)?;
            self.validate_declared_ownership(&param.ty, param.ownership)?;
            scope.insert(
                param.id.clone(),
                ValidationBinding {
                    ty: param.ty.clone(),
                    ownership: param.ownership,
                    availability: Availability::Available,
                },
            );
        }
        if function.result_id != ValueId::result(&function.id) {
            return Err(hir_error(format!(
                "function `{}` has a non-canonical result identity",
                function.id
            )));
        }
        self.insert_value(&function.result_id)?;

        for (index, contract) in function.requires.iter().enumerate() {
            let mut contract_scope = scope.clone();
            self.validate_expr(
                &function.id,
                contract,
                &mut contract_scope,
                &format!("requires.{index}"),
                false,
                None,
            )?;
            self.require_type(&contract.ty, &ResolvedType::Bool, "precondition")?;
        }
        self.validate_expr(
            &function.id,
            &function.body,
            &mut scope,
            "body",
            true,
            Some(&declared_effects),
        )?;
        self.require_type(&function.body.ty, &function.return_type, "function body")?;
        let returned = self.expected_ownership(&function.return_type, OwnershipMode::Own)?;
        if function.body.ownership != returned {
            return Err(hir_error(format!(
                "function `{}` body has invalid return ownership",
                function.id
            )));
        }

        let mut ensures_scope = scope;
        ensures_scope.insert(
            function.result_id.clone(),
            ValidationBinding {
                ty: function.return_type.clone(),
                ownership: returned,
                availability: Availability::Available,
            },
        );
        for (index, contract) in function.ensures.iter().enumerate() {
            let mut contract_scope = ensures_scope.clone();
            self.validate_expr(
                &function.id,
                contract,
                &mut contract_scope,
                &format!("ensures.{index}"),
                false,
                None,
            )?;
            self.require_type(&contract.ty, &ResolvedType::Bool, "postcondition")?;
        }
        Ok(())
    }

    fn validate_expr(
        &mut self,
        function: &DeclarationId,
        expression: &ResolvedExpr,
        scope: &mut BTreeMap<ValueId, ValidationBinding>,
        path: &str,
        allow_moves: bool,
        allowed_effects: Option<&BTreeSet<String>>,
    ) -> Result<(), Diagnostic> {
        if expression.id != ExpressionId::new(function, path) {
            return Err(hir_error(format!(
                "expression `{}` has a non-canonical identity",
                expression.id
            )));
        }
        if !self.expression_ids.insert(expression.id.clone()) {
            return Err(hir_error(format!(
                "duplicate resolved expression identity `{}`",
                expression.id
            )));
        }
        self.validate_type(&expression.ty)?;

        let (ty, ownership) = match &expression.kind {
            ResolvedExprKind::Int(_) => (ResolvedType::I64, OwnershipMode::Value),
            ResolvedExprKind::Bool(_) => (ResolvedType::Bool, OwnershipMode::Value),
            ResolvedExprKind::Place(place) => {
                if !place.projections.is_empty() {
                    return Err(hir_error(
                        "aggregate place projections are not valid in the current HIR",
                    ));
                }
                let binding = scope.get(&place.root).ok_or_else(|| {
                    hir_error(format!("resolved value `{}` is out of scope", place.root))
                })?;
                match binding.availability {
                    Availability::Available => {}
                    Availability::Moved => {
                        return Err(hir_error(format!(
                            "resolved value `{}` is used after it was moved",
                            place.root
                        )));
                    }
                    Availability::MaybeMoved => {
                        return Err(hir_error(format!(
                            "resolved value `{}` may have been moved",
                            place.root
                        )));
                    }
                }
                (binding.ty.clone(), binding.ownership)
            }
            ResolvedExprKind::Call { callee, args } => {
                let target = self.functions.get(callee).copied().ok_or_else(|| {
                    hir_error(format!("resolved callee `{callee}` is not indexed"))
                })?;
                if args.len() != target.params.len() {
                    return Err(hir_error(format!(
                        "call to `{callee}` has {} arguments but expects {}",
                        args.len(),
                        target.params.len()
                    )));
                }
                let params = target.params.clone();
                let return_type = target.return_type.clone();
                let target_effects = target.effects.clone();
                match allowed_effects {
                    Some(allowed) => {
                        for effect in &target_effects {
                            if !allowed.contains(effect) {
                                return Err(hir_error(format!(
                                    "call to `{callee}` requires undeclared effect `{effect}`"
                                )));
                            }
                        }
                    }
                    None if !target_effects.is_empty() => {
                        return Err(hir_error(format!(
                            "contract calls effectful function `{callee}`"
                        )));
                    }
                    None => {}
                }
                for (index, (argument, param)) in args.iter().zip(&params).enumerate() {
                    self.validate_expr(
                        function,
                        argument,
                        scope,
                        &format!("{path}.arg.{index}"),
                        allow_moves,
                        allowed_effects,
                    )?;
                    self.require_type(&argument.ty, &param.ty, "call argument")?;
                    self.validate_argument_ownership(argument.ownership, param)?;
                    if self.argument_transfers(param)? {
                        if !allow_moves {
                            return Err(hir_error(format!(
                                "contract cannot transfer ownership to `{callee}`"
                            )));
                        }
                        self.mark_value_sources_moved(argument, scope)?;
                    }
                }
                let ownership = self.expected_ownership(&return_type, OwnershipMode::Own)?;
                (return_type, ownership)
            }
            ResolvedExprKind::Unary { op, value } => {
                self.validate_expr(
                    function,
                    value,
                    scope,
                    &format!("{path}.value"),
                    allow_moves,
                    allowed_effects,
                )?;
                let expected = match op {
                    UnaryOp::Neg => ResolvedType::I64,
                    UnaryOp::Not => ResolvedType::Bool,
                };
                self.require_type(&value.ty, &expected, "unary operand")?;
                (expected, OwnershipMode::Value)
            }
            ResolvedExprKind::Binary { op, left, right } => {
                self.validate_expr(
                    function,
                    left,
                    scope,
                    &format!("{path}.left"),
                    allow_moves,
                    allowed_effects,
                )?;
                if matches!(op, BinaryOp::And | BinaryOp::Or) {
                    let baseline_ids = scope.keys().cloned().collect::<Vec<_>>();
                    let mut conditional_scope = scope.clone();
                    self.validate_expr(
                        function,
                        right,
                        &mut conditional_scope,
                        &format!("{path}.right"),
                        allow_moves,
                        allowed_effects,
                    )?;
                    Self::join_conditional(scope, &conditional_scope, &baseline_ids);
                } else {
                    self.validate_expr(
                        function,
                        right,
                        scope,
                        &format!("{path}.right"),
                        allow_moves,
                        allowed_effects,
                    )?;
                }
                let output = match op {
                    BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::Div
                    | BinaryOp::Rem => {
                        self.require_type(&left.ty, &ResolvedType::I64, "binary operand")?;
                        self.require_type(&right.ty, &ResolvedType::I64, "binary operand")?;
                        ResolvedType::I64
                    }
                    BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                        self.require_type(&left.ty, &ResolvedType::I64, "comparison operand")?;
                        self.require_type(&right.ty, &ResolvedType::I64, "comparison operand")?;
                        ResolvedType::Bool
                    }
                    BinaryOp::And | BinaryOp::Or => {
                        self.require_type(&left.ty, &ResolvedType::Bool, "boolean operand")?;
                        self.require_type(&right.ty, &ResolvedType::Bool, "boolean operand")?;
                        ResolvedType::Bool
                    }
                    BinaryOp::Eq | BinaryOp::Ne => {
                        self.require_type(&left.ty, &right.ty, "equality operands")?;
                        ResolvedType::Bool
                    }
                };
                (output, OwnershipMode::Value)
            }
            ResolvedExprKind::Block { statements, tail } => {
                let mut block_scope = scope.clone();
                for (index, statement) in statements.iter().enumerate() {
                    match statement {
                        ResolvedStatement::Let { binding, value, .. } => {
                            let statement_path = format!("{path}.s{index}");
                            self.validate_expr(
                                function,
                                value,
                                &mut block_scope,
                                &format!("{statement_path}.value"),
                                allow_moves,
                                allowed_effects,
                            )?;
                            if binding.id != ValueId::local(function, &statement_path) {
                                return Err(hir_error(format!(
                                    "local `{}` has a non-canonical identity",
                                    binding.id
                                )));
                            }
                            self.insert_value(&binding.id)?;
                            self.require_type(&binding.ty, &value.ty, "local binding")?;
                            if binding.ownership != value.ownership {
                                return Err(hir_error(format!(
                                    "local `{}` has inconsistent ownership",
                                    binding.id
                                )));
                            }
                            self.validate_declared_ownership(&binding.ty, binding.ownership)?;
                            if self.is_owned_resource(&binding.ty, binding.ownership)? {
                                if !allow_moves {
                                    return Err(hir_error(
                                        "contract cannot transfer ownership into a local binding",
                                    ));
                                }
                                self.mark_value_sources_moved(value, &mut block_scope)?;
                            }
                            block_scope.insert(
                                binding.id.clone(),
                                ValidationBinding {
                                    ty: binding.ty.clone(),
                                    ownership: binding.ownership,
                                    availability: Availability::Available,
                                },
                            );
                        }
                    }
                }
                self.validate_expr(
                    function,
                    tail,
                    &mut block_scope,
                    &format!("{path}.tail"),
                    allow_moves,
                    allowed_effects,
                )?;
                let outer_ids = scope.keys().cloned().collect::<Vec<_>>();
                Self::merge_availability(scope, &block_scope, &outer_ids);
                (tail.ty.clone(), tail.ownership)
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.validate_expr(
                    function,
                    condition,
                    scope,
                    &format!("{path}.condition"),
                    allow_moves,
                    allowed_effects,
                )?;
                self.require_type(&condition.ty, &ResolvedType::Bool, "if condition")?;
                let outer_ids = scope.keys().cloned().collect::<Vec<_>>();
                let mut then_scope = scope.clone();
                let mut else_scope = scope.clone();
                self.validate_expr(
                    function,
                    then_branch,
                    &mut then_scope,
                    &format!("{path}.then"),
                    allow_moves,
                    allowed_effects,
                )?;
                self.validate_expr(
                    function,
                    else_branch,
                    &mut else_scope,
                    &format!("{path}.else"),
                    allow_moves,
                    allowed_effects,
                )?;
                Self::join_branches(scope, &then_scope, &else_scope, &outer_ids);
                self.require_type(&then_branch.ty, &else_branch.ty, "if branches")?;
                if then_branch.ownership != else_branch.ownership {
                    return Err(hir_error("if branches have inconsistent ownership"));
                }
                (then_branch.ty.clone(), then_branch.ownership)
            }
        };

        self.require_type(&expression.ty, &ty, "expression")?;
        if expression.ownership != ownership {
            return Err(hir_error(format!(
                "expression `{}` has inconsistent ownership",
                expression.id
            )));
        }
        Ok(())
    }

    fn argument_transfers(&self, param: &ResolvedParam) -> Result<bool, Diagnostic> {
        self.is_owned_resource(&param.ty, param.ownership)
    }

    fn is_owned_resource(
        &self,
        ty: &ResolvedType,
        ownership: OwnershipMode,
    ) -> Result<bool, Diagnostic> {
        self.program
            .declarations
            .type_facts(ty)
            .map(|facts| !facts.copy && ownership == OwnershipMode::Own)
            .ok_or_else(|| {
                hir_error(format!(
                    "type `{}` has no semantic facts",
                    ty.identity_key()
                ))
            })
    }

    fn mark_value_sources_moved(
        &self,
        expression: &ResolvedExpr,
        scope: &mut BTreeMap<ValueId, ValidationBinding>,
    ) -> Result<(), Diagnostic> {
        match &expression.kind {
            ResolvedExprKind::Place(place) => {
                let Some(binding) = scope.get(&place.root) else {
                    // A block result may be backed by a local whose lexical
                    // scope ended after the expression was validated. Its
                    // transfer cannot affect any still-visible root.
                    return Ok(());
                };
                let should_move = self.is_owned_resource(&binding.ty, binding.ownership)?
                    && binding.availability == Availability::Available;
                if should_move {
                    let binding = scope.get_mut(&place.root).ok_or_else(|| {
                        hir_error(format!(
                            "resolved value `{}` disappeared during ownership validation",
                            place.root
                        ))
                    })?;
                    binding.availability = Availability::Moved;
                }
            }
            ResolvedExprKind::Block { tail, .. } => {
                self.mark_value_sources_moved(tail, scope)?;
            }
            ResolvedExprKind::If {
                then_branch,
                else_branch,
                ..
            } => {
                let ids = scope.keys().cloned().collect::<Vec<_>>();
                let mut then_scope = scope.clone();
                let mut else_scope = scope.clone();
                self.mark_value_sources_moved(then_branch, &mut then_scope)?;
                self.mark_value_sources_moved(else_branch, &mut else_scope)?;
                Self::join_branches(scope, &then_scope, &else_scope, &ids);
            }
            ResolvedExprKind::Int(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::Call { .. }
            | ResolvedExprKind::Unary { .. }
            | ResolvedExprKind::Binary { .. } => {}
        }
        Ok(())
    }

    fn merge_availability(
        target: &mut BTreeMap<ValueId, ValidationBinding>,
        source: &BTreeMap<ValueId, ValidationBinding>,
        ids: &[ValueId],
    ) {
        for id in ids {
            if let (Some(target), Some(source)) = (target.get_mut(id), source.get(id)) {
                target.availability = source.availability;
            }
        }
    }

    fn join_conditional(
        baseline: &mut BTreeMap<ValueId, ValidationBinding>,
        conditional: &BTreeMap<ValueId, ValidationBinding>,
        ids: &[ValueId],
    ) {
        for id in ids {
            if let (Some(baseline), Some(conditional)) = (baseline.get_mut(id), conditional.get(id))
            {
                baseline.availability = baseline.availability.join(conditional.availability);
            }
        }
    }

    fn join_branches(
        target: &mut BTreeMap<ValueId, ValidationBinding>,
        then_scope: &BTreeMap<ValueId, ValidationBinding>,
        else_scope: &BTreeMap<ValueId, ValidationBinding>,
        ids: &[ValueId],
    ) {
        for id in ids {
            if let (Some(target), Some(then_value), Some(else_value)) =
                (target.get_mut(id), then_scope.get(id), else_scope.get(id))
            {
                target.availability = then_value.availability.join(else_value.availability);
            }
        }
    }

    fn validate_type(&self, ty: &ResolvedType) -> Result<(), Diagnostic> {
        match ty {
            ResolvedType::I64 | ResolvedType::Bool => Ok(()),
            ResolvedType::TypeParameter { .. } => Err(hir_error(
                "uninstantiated type parameters are not valid in executable HIR",
            )),
            ResolvedType::Nominal {
                declaration,
                arguments,
            } => {
                if self
                    .program
                    .declarations
                    .declaration(declaration)
                    .is_none_or(|item| item.kind != DeclarationKind::Resource)
                {
                    return Err(hir_error(format!(
                        "nominal type `{declaration}` is not a resolved resource declaration"
                    )));
                }
                if !arguments.is_empty() {
                    return Err(hir_error(format!(
                        "resource type `{declaration}` does not declare type parameters"
                    )));
                }
                for argument in arguments {
                    self.validate_type(argument)?;
                }
                self.program.declarations.type_facts(ty).ok_or_else(|| {
                    hir_error(format!(
                        "type `{}` has no semantic facts",
                        ty.identity_key()
                    ))
                })?;
                Ok(())
            }
        }
    }

    fn validate_declared_ownership(
        &self,
        ty: &ResolvedType,
        ownership: OwnershipMode,
    ) -> Result<(), Diagnostic> {
        let facts = self.program.declarations.type_facts(ty).ok_or_else(|| {
            hir_error(format!(
                "type `{}` has no semantic facts",
                ty.identity_key()
            ))
        })?;
        if (facts.copy && ownership != OwnershipMode::Value)
            || (!facts.copy && ownership == OwnershipMode::Value)
        {
            return Err(hir_error(format!(
                "type `{}` has an invalid ownership mode",
                ty.identity_key()
            )));
        }
        Ok(())
    }

    fn validate_argument_ownership(
        &self,
        actual: OwnershipMode,
        param: &ResolvedParam,
    ) -> Result<(), Diagnostic> {
        let facts = self
            .program
            .declarations
            .type_facts(&param.ty)
            .ok_or_else(|| {
                hir_error(format!(
                    "type `{}` has no semantic facts",
                    param.ty.identity_key()
                ))
            })?;
        let valid = if facts.copy {
            actual == OwnershipMode::Value && param.ownership == OwnershipMode::Value
        } else {
            match param.ownership {
                OwnershipMode::Own => actual == OwnershipMode::Own,
                OwnershipMode::Borrow => true,
                OwnershipMode::Shared => actual == OwnershipMode::Shared,
                OwnershipMode::Value => false,
            }
        };
        if valid {
            Ok(())
        } else {
            Err(hir_error(format!(
                "argument ownership is incompatible with parameter `{}`",
                param.id
            )))
        }
    }

    fn expected_ownership(
        &self,
        ty: &ResolvedType,
        non_copy: OwnershipMode,
    ) -> Result<OwnershipMode, Diagnostic> {
        self.program
            .declarations
            .type_facts(ty)
            .map(|facts| {
                if facts.copy {
                    OwnershipMode::Value
                } else {
                    non_copy
                }
            })
            .ok_or_else(|| {
                hir_error(format!(
                    "type `{}` has no semantic facts",
                    ty.identity_key()
                ))
            })
    }

    fn require_type(
        &self,
        actual: &ResolvedType,
        expected: &ResolvedType,
        context: &str,
    ) -> Result<(), Diagnostic> {
        if actual == expected {
            Ok(())
        } else {
            Err(hir_error(format!(
                "{context} has inconsistent resolved types"
            )))
        }
    }

    fn insert_value(&mut self, id: &ValueId) -> Result<(), Diagnostic> {
        if self.value_ids.insert(id.clone()) {
            Ok(())
        } else {
            Err(hir_error(format!(
                "duplicate resolved value identity `{id}`"
            )))
        }
    }
}

fn hir_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-H006", message)
}

struct Resolver<'a> {
    program: &'a Program,
    declarations: DeclarationIndex,
}

impl Resolver<'_> {
    fn resolve(self) -> Result<ResolvedProgram, Diagnostic> {
        let entrypoint = self
            .program
            .functions
            .iter()
            .find(|function| function.name == "main")
            .map(|function| DeclarationId::new(function.stable_id.clone()))
            .ok_or_else(|| {
                self.error(
                    "SPX-H005",
                    "verified program has no resolved entry point",
                    Span::default(),
                )
            })?;
        let types = self
            .program
            .resources
            .iter()
            .map(|resource| ResolvedTypeDeclaration {
                id: DeclarationId::new(resource.stable_id.clone()),
                name: resource.name.clone(),
                kind: ResolvedTypeDeclarationKind::Resource,
                span: resource.span,
            })
            .collect();
        let functions = self
            .program
            .functions
            .iter()
            .map(|function| self.resolve_function(function))
            .collect::<Result<_, _>>()?;
        Ok(ResolvedProgram {
            module: self.program.module.clone(),
            permits: self.program.permits.clone(),
            entrypoint,
            declarations: self.declarations,
            types,
            functions,
        })
    }

    fn resolve_function(
        &self,
        function: &crate::ast::Function,
    ) -> Result<ResolvedFunction, Diagnostic> {
        let function_id = DeclarationId::new(function.stable_id.clone());
        let mut bindings = BTreeMap::new();
        let params = function
            .params
            .iter()
            .enumerate()
            .map(|(index, param)| {
                let ty = self.resolve_type(&param.ty, param.span)?;
                let id = ValueId::parameter(&function_id, index);
                let ownership = param.mode.into();
                bindings.insert(
                    param.name.clone(),
                    Binding {
                        id: id.clone(),
                        ty: ty.clone(),
                        ownership,
                    },
                );
                Ok(ResolvedParam {
                    id,
                    name: param.name.clone(),
                    ownership,
                    ty,
                    span: param.span,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let return_type = self.resolve_type(&function.return_type, function.span)?;
        let result_id = ValueId::result(&function_id);

        let requires = function
            .requires
            .iter()
            .enumerate()
            .map(|(index, expression)| {
                self.resolve_expr(
                    &function_id,
                    expression,
                    &bindings,
                    &format!("requires.{index}"),
                )
            })
            .collect::<Result<_, _>>()?;
        let body = self.resolve_expr(&function_id, &function.body, &bindings, "body")?;

        let mut ensures_bindings = bindings;
        ensures_bindings.insert(
            "result".to_owned(),
            Binding {
                id: result_id.clone(),
                ty: return_type.clone(),
                ownership: self.expression_ownership(
                    &return_type,
                    OwnershipMode::Own,
                    function.span,
                )?,
            },
        );
        let ensures = function
            .ensures
            .iter()
            .enumerate()
            .map(|(index, expression)| {
                self.resolve_expr(
                    &function_id,
                    expression,
                    &ensures_bindings,
                    &format!("ensures.{index}"),
                )
            })
            .collect::<Result<_, _>>()?;

        Ok(ResolvedFunction {
            id: function_id,
            name: function.name.clone(),
            params,
            result_id,
            return_type,
            effects: function.effects.clone(),
            requires,
            ensures,
            body,
            span: function.span,
        })
    }

    fn resolve_type(&self, ty: &Type, span: Span) -> Result<ResolvedType, Diagnostic> {
        match ty {
            Type::I64 => Ok(ResolvedType::I64),
            Type::Bool => Ok(ResolvedType::Bool),
            Type::Resource(name) => self
                .declarations
                .type_id(name)
                .cloned()
                .map(|declaration| ResolvedType::Nominal {
                    declaration,
                    arguments: Vec::new(),
                })
                .ok_or_else(|| self.error("SPX-H001", format!("unresolved type `{name}`"), span)),
        }
    }

    fn resolve_expr(
        &self,
        function: &DeclarationId,
        expr: &Expr,
        bindings: &BTreeMap<String, Binding>,
        path: &str,
    ) -> Result<ResolvedExpr, Diagnostic> {
        let id = ExpressionId::new(function, path);
        let (kind, ty, ownership) = match &expr.kind {
            ExprKind::Int(value) => (
                ResolvedExprKind::Int(*value),
                ResolvedType::I64,
                OwnershipMode::Value,
            ),
            ExprKind::Bool(value) => (
                ResolvedExprKind::Bool(*value),
                ResolvedType::Bool,
                OwnershipMode::Value,
            ),
            ExprKind::Var(name) => {
                let binding = bindings.get(name).ok_or_else(|| {
                    self.error("SPX-H002", format!("unresolved value `{name}`"), expr.span)
                })?;
                (
                    ResolvedExprKind::Place(Place {
                        root: binding.id.clone(),
                        projections: Vec::new(),
                    }),
                    binding.ty.clone(),
                    binding.ownership,
                )
            }
            ExprKind::Call { name, args } => {
                let callee = self
                    .declarations
                    .function_id(name)
                    .cloned()
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H003",
                            format!("unresolved function `{name}`"),
                            expr.span,
                        )
                    })?;
                let target = self
                    .program
                    .functions
                    .iter()
                    .find(|function| function.stable_id == callee.as_str())
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H003",
                            format!("function identity `{callee}` has no declaration"),
                            expr.span,
                        )
                    })?;
                let args = args
                    .iter()
                    .enumerate()
                    .map(|(index, argument)| {
                        self.resolve_expr(
                            function,
                            argument,
                            bindings,
                            &format!("{path}.arg.{index}"),
                        )
                    })
                    .collect::<Result<_, _>>()?;
                let ty = self.resolve_type(&target.return_type, target.span)?;
                let ownership = self.expression_ownership(&ty, OwnershipMode::Own, target.span)?;
                (ResolvedExprKind::Call { callee, args }, ty, ownership)
            }
            ExprKind::Unary { op, value } => {
                let value =
                    self.resolve_expr(function, value, bindings, &format!("{path}.value"))?;
                let ty = match op {
                    UnaryOp::Neg => ResolvedType::I64,
                    UnaryOp::Not => ResolvedType::Bool,
                };
                (
                    ResolvedExprKind::Unary {
                        op: *op,
                        value: Box::new(value),
                    },
                    ty,
                    OwnershipMode::Value,
                )
            }
            ExprKind::Binary { op, left, right } => {
                let left = self.resolve_expr(function, left, bindings, &format!("{path}.left"))?;
                let right =
                    self.resolve_expr(function, right, bindings, &format!("{path}.right"))?;
                let ty = match op {
                    BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::Div
                    | BinaryOp::Rem => ResolvedType::I64,
                    BinaryOp::Eq
                    | BinaryOp::Ne
                    | BinaryOp::Lt
                    | BinaryOp::Le
                    | BinaryOp::Gt
                    | BinaryOp::Ge
                    | BinaryOp::And
                    | BinaryOp::Or => ResolvedType::Bool,
                };
                (
                    ResolvedExprKind::Binary {
                        op: *op,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                    ty,
                    OwnershipMode::Value,
                )
            }
            ExprKind::Block { statements, tail } => {
                let mut scope = bindings.clone();
                let mut resolved_statements = Vec::with_capacity(statements.len());
                for (index, statement) in statements.iter().enumerate() {
                    let statement_path = format!("{path}.s{index}");
                    match statement {
                        Statement::Let {
                            name,
                            name_span,
                            value,
                            span,
                        } => {
                            let value = self.resolve_expr(
                                function,
                                value,
                                &scope,
                                &format!("{statement_path}.value"),
                            )?;
                            let binding = ResolvedBinding {
                                id: ValueId::local(function, &statement_path),
                                name: name.clone(),
                                ownership: value.ownership,
                                ty: value.ty.clone(),
                                span: *name_span,
                            };
                            scope.insert(
                                name.clone(),
                                Binding {
                                    id: binding.id.clone(),
                                    ty: binding.ty.clone(),
                                    ownership: binding.ownership,
                                },
                            );
                            resolved_statements.push(ResolvedStatement::Let {
                                binding,
                                value,
                                span: *span,
                            });
                        }
                    }
                }
                let tail = self.resolve_expr(function, tail, &scope, &format!("{path}.tail"))?;
                let ty = tail.ty.clone();
                let ownership = tail.ownership;
                (
                    ResolvedExprKind::Block {
                        statements: resolved_statements,
                        tail: Box::new(tail),
                    },
                    ty,
                    ownership,
                )
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition =
                    self.resolve_expr(function, condition, bindings, &format!("{path}.condition"))?;
                let then_branch =
                    self.resolve_expr(function, then_branch, bindings, &format!("{path}.then"))?;
                let else_branch =
                    self.resolve_expr(function, else_branch, bindings, &format!("{path}.else"))?;
                let ty = then_branch.ty.clone();
                let ownership = then_branch.ownership;
                (
                    ResolvedExprKind::If {
                        condition: Box::new(condition),
                        then_branch: Box::new(then_branch),
                        else_branch: Box::new(else_branch),
                    },
                    ty,
                    ownership,
                )
            }
        };
        Ok(ResolvedExpr {
            id,
            ty,
            ownership,
            kind,
            span: expr.span,
        })
    }

    fn error(&self, code: &'static str, message: impl Into<String>, span: Span) -> Diagnostic {
        Diagnostic::error(code, message, span).at_path(&self.program.path)
    }

    fn expression_ownership(
        &self,
        ty: &ResolvedType,
        non_copy_mode: OwnershipMode,
        span: Span,
    ) -> Result<OwnershipMode, Diagnostic> {
        self.declarations
            .type_facts(ty)
            .map(|facts| {
                if facts.copy {
                    OwnershipMode::Value
                } else {
                    non_copy_mode
                }
            })
            .ok_or_else(|| {
                self.error(
                    "SPX-H004",
                    format!(
                        "semantic facts are unavailable for type `{}`",
                        ty.identity_key()
                    ),
                    span,
                )
            })
    }
}
