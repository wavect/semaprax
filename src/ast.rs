use std::fmt;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

impl Span {
    pub fn merge(self, other: Span) -> Span {
        Span {
            start: self.start,
            end: other.end,
            line: self.line,
            column: self.column,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Type {
    I64,
    I32,
    Char,
    U8,
    /// A target-independent checked unsigned 64-bit semantic integer.
    Usize,
    /// One inline Copy byte array with an exact target-independent length.
    ArrayU8(u32),
    F32,
    F64,
    Bool,
    String,
    /// One uniquely owned immutable byte buffer.
    Bytes,
    /// A borrowed UTF-8 view. Source functions may receive it only through
    /// an explicit `borrow str` parameter; it has no literal or owned form.
    Str,
    /// A non-escaping immutable byte view rooted in one external invocation
    /// input. It is written exactly `Slice<u8>` and has no owned form.
    SliceU8,
    Named {
        name: String,
        arguments: Vec<Type>,
    },
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        enum Frame<'a> {
            Type(&'a Type),
            Arguments(&'a [Type], usize),
        }
        let mut frames = vec![Frame::Type(self)];
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Type(Type::I64) => f.write_str("i64")?,
                Frame::Type(Type::I32) => f.write_str("i32")?,
                Frame::Type(Type::Char) => f.write_str("char")?,
                Frame::Type(Type::U8) => f.write_str("u8")?,
                Frame::Type(Type::Usize) => f.write_str("usize")?,
                Frame::Type(Type::ArrayU8(length)) => write!(f, "[u8; {length}]")?,
                Frame::Type(Type::F32) => f.write_str("f32")?,
                Frame::Type(Type::F64) => f.write_str("f64")?,
                Frame::Type(Type::Bool) => f.write_str("bool")?,
                Frame::Type(Type::String) => f.write_str("string")?,
                Frame::Type(Type::Bytes) => f.write_str("Bytes")?,
                Frame::Type(Type::Str) => f.write_str("str")?,
                Frame::Type(Type::SliceU8) => f.write_str("Slice<u8>")?,
                Frame::Type(Type::Named { name, arguments }) => {
                    f.write_str(name)?;
                    if !arguments.is_empty() {
                        f.write_str("<")?;
                        frames.push(Frame::Arguments(arguments, 0));
                    }
                }
                Frame::Arguments(arguments, index) => {
                    if let Some(argument) = arguments.get(index) {
                        if index != 0 {
                            f.write_str(", ")?;
                        }
                        frames.push(Frame::Arguments(arguments, index + 1));
                        frames.push(Frame::Type(argument));
                    } else {
                        f.write_str(">")?;
                    }
                }
            }
        }
        Ok(())
    }
}

impl Type {
    pub fn is_named(&self) -> bool {
        matches!(self, Type::Named { .. })
    }

    /// Canonical ownership predicate. `Bytes` transfers uniquely without
    /// being misclassified as a user resource.
    pub fn is_uniquely_owned(&self) -> bool {
        matches!(self, Type::String | Type::Bytes)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ParamMode {
    #[default]
    Value,
    Own,
    Borrow,
    Shared,
}

impl ParamMode {
    pub fn text(self) -> &'static str {
        match self {
            ParamMode::Value => "value",
            ParamMode::Own => "own",
            ParamMode::Borrow => "borrow",
            ParamMode::Shared => "shared",
        }
    }

    pub fn source_prefix(self) -> &'static str {
        match self {
            ParamMode::Value => "",
            ParamMode::Own => "own ",
            ParamMode::Borrow => "borrow ",
            ParamMode::Shared => "shared ",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Program {
    pub path: String,
    pub module: String,
    pub module_uses: Vec<ModuleUse>,
    pub permits: Vec<String>,
    pub types: Vec<TypeDeclaration>,
    pub interfaces: Vec<InterfaceDeclaration>,
    /// Protocol Projection v1: method-set declarations over the eventual
    /// class receiver. Read-only in this tranche; records stand in as the
    /// future conformance carriers and no backend consumes protocols yet.
    pub protocols: Vec<ProtocolDeclaration>,
    pub functions: Vec<Function>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ModuleUseKind {
    Function,
    Type,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleUse {
    pub kind: ModuleUseKind,
    pub persistent_id: String,
    pub target_module: String,
    pub alias: String,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct TypeDeclaration {
    pub stable_id: String,
    pub explicit_id: bool,
    pub name: String,
    pub name_span: Span,
    pub type_parameters: Vec<TypeParameterDeclaration>,
    pub kind: TypeDeclarationKind,
    /// Class Inheritance v1: the single named parent of a `class C : P` block.
    /// `None` for every other declaration and for parentless classes.
    pub extends: Option<Type>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeParameterDeclaration {
    pub name: String,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum TypeDeclarationKind {
    Resource {
        lifecycles: Vec<ResourceLifecycleDeclaration>,
    },
    Record {
        fields: Vec<FieldDeclaration>,
    },
    Variant {
        cases: Vec<VariantCaseDeclaration>,
    },
    Class {
        fields: Vec<FieldDeclaration>,
        methods: Vec<Function>,
    },
}

#[derive(Clone, Debug)]
pub struct VariantCaseDeclaration {
    pub stable_id: String,
    pub explicit_id: bool,
    pub name: String,
    pub name_span: Span,
    pub fields: Vec<FieldDeclaration>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct ResourceLifecycleDeclaration {
    pub stable_id: Option<String>,
    pub kind: ResourceLifecycleKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ResourceLifecycleKind {
    Trivial,
    Imported { import_key: String },
}

#[derive(Clone, Debug)]
pub struct InterfaceDeclaration {
    pub stable_id: String,
    pub explicit_id: bool,
    pub name: String,
    pub name_span: Span,
    pub permits: Vec<String>,
    pub imports: Vec<ImportDeclaration>,
    pub span: Span,
}

/// Protocol Projection v1: one `protocol` declaration — a named method set
/// whose signatures are checked to resolve. Distinct from the host-import
/// `interface` concept; protocols carry no imports, effects, or permits.
#[derive(Clone, Debug)]
pub struct ProtocolDeclaration {
    pub stable_id: String,
    pub explicit_id: bool,
    pub name: String,
    pub name_span: Span,
    pub methods: Vec<ProtocolMethod>,
    pub span: Span,
}

/// One body-less method signature inside a protocol. The first parameter is
/// the receiver and must be typed `Self` or the protocol's own name; the
/// canonical projection keeps it verbatim.
#[derive(Clone, Debug)]
pub struct ProtocolMethod {
    pub stable_id: String,
    pub explicit_id: bool,
    pub name: String,
    pub name_span: Span,
    pub params: Vec<Param>,
    pub return_type: Type,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct ImportDeclaration {
    pub stable_id: String,
    pub explicit_id: bool,
    pub name: String,
    pub name_span: Span,
    pub native_rust: bool,
    pub params: Vec<Param>,
    pub result: ImportResult,
    pub effects: Vec<String>,
    pub failure: ImportFailure,
    pub consumes: String,
    pub consumes_span: Span,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportResult {
    Unit,
    I64,
    Bool,
}

impl fmt::Display for ImportResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unit => "unit",
            Self::I64 => "i64",
            Self::Bool => "bool",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImportFailure {
    Infallible,
    Status { domain_id: String },
}

#[derive(Clone, Debug)]
pub struct FieldDeclaration {
    pub stable_id: String,
    pub explicit_id: bool,
    pub name: String,
    pub name_span: Span,
    pub ty: Type,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Function {
    pub stable_id: String,
    pub explicit_id: bool,
    pub name: String,
    pub name_span: Span,
    pub type_parameters: Vec<TypeParameterDeclaration>,
    pub params: Vec<Param>,
    pub return_type: Type,
    pub effects: Vec<String>,
    pub requires: Vec<Expr>,
    pub ensures: Vec<Expr>,
    pub body: Expr,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Param {
    pub name: String,
    pub mode: ParamMode,
    pub ty: Type,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    Int(i64),
    /// An `i32` literal stored as its exact value.
    Int32(i32),
    /// A `char` literal stored as its exact Unicode scalar value.
    Char(u32),
    /// A `u8` literal stored as its exact value.
    Uint8(u8),
    /// A `usize` literal stored as its target-independent unsigned value.
    Usize(u64),
    /// An explicit byte inventory. Its length is its exact array type.
    ArrayU8(Vec<u8>),
    /// A canonical repeated-byte fixed-array literal.
    RepeatArrayU8 {
        value: u8,
        count: u32,
    },
    /// An `f32` literal stored as its exact IEEE-754 bit pattern.
    Float32(u32),
    /// An `f64` literal stored as its exact IEEE-754 bit pattern.
    Float64(u64),
    Bool(bool),
    String(String),
    Var(String),
    Call {
        name: String,
        type_arguments: Vec<Type>,
        args: Vec<Expr>,
    },
    /// Method call `receiver.method(args)` lowered to static call of class method.
    MethodCall {
        receiver: Box<Expr>,
        method: String,
        method_span: Span,
        type_arguments: Vec<Type>,
        args: Vec<Expr>,
    },
    /// Class Inheritance v1: `super.method(args)` inside a class-method
    /// override. Resolves statically against the parent chain, skipping the
    /// enclosing class, with the enclosing method's receiver as `self`.
    SuperMethod {
        method: String,
        method_span: Span,
        args: Vec<Expr>,
    },
    Unary {
        op: UnaryOp,
        value: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Block {
        statements: Vec<Statement>,
        tail: Box<Expr>,
    },
    If {
        condition: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
    },
    ConstructRecord {
        type_name: String,
        type_span: Span,
        type_arguments: Vec<Type>,
        fields: Vec<FieldInitializer>,
    },
    ConstructVariant {
        type_name: String,
        type_span: Span,
        type_arguments: Vec<Type>,
        case_name: String,
        case_span: Span,
        fields: Vec<FieldInitializer>,
    },
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    Try {
        operand: Box<Expr>,
    },
    UpdateRecord {
        base: Box<Expr>,
        fields: Vec<FieldInitializer>,
    },
    Project {
        base: Box<Expr>,
        field: String,
        field_span: Span,
    },
}

#[derive(Clone, Debug)]
pub struct MatchArm {
    pub pattern: MatchPattern,
    /// Refutable Match v1: `pattern if guard => value`. The guard is an
    /// ordinary bool expression evaluated after the pattern matches and at
    /// most once; a failing guard falls through to the following arms.
    pub guard: Option<Box<Expr>>,
    pub value: Expr,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum MatchPattern {
    Variant {
        type_name: String,
        type_span: Span,
        case_name: String,
        case_span: Span,
        fields: Vec<MatchPatternField>,
        span: Span,
    },
    Record {
        type_name: String,
        type_span: Span,
        fields: Vec<RecordMatchPatternField>,
        span: Span,
    },
    Wildcard {
        span: Span,
    },
    /// Refutable Match v1: one exact scalar literal (`-3`, `7i32`, `9u8`,
    /// `'x'`, `true`). Floats and strings never parse as patterns.
    Literal {
        value: PatternLiteral,
        span: Span,
    },
    /// Refutable Match v1: `a | b` over literal alternatives of one type.
    /// The parser accepts only literal atoms here; deeper restrictions are
    /// enforced by the resolvers with stable diagnostics.
    Or {
        alternatives: Vec<MatchPattern>,
        span: Span,
    },
    /// Refutable Match v1: irrefutable whole-scrutinee binding (`n => ...`).
    Binding {
        name: String,
        span: Span,
    },
}

/// The exact scalar value of a literal pattern. Suffixed typing rules are
/// identical to expression literals because the lexer produces the same
/// tokens; sign folding for negative integers happens in the parser.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatternLiteral {
    Int(i64),
    Int32(i32),
    Uint8(u8),
    Usize(u64),
    Char(u32),
    Bool(bool),
}

impl PatternLiteral {
    /// The scrutinee scalar type this literal compares against exactly.
    pub fn type_text(&self) -> &'static str {
        match self {
            Self::Int(_) => "i64",
            Self::Int32(_) => "i32",
            Self::Uint8(_) => "u8",
            Self::Usize(_) => "usize",
            Self::Char(_) => "char",
            Self::Bool(_) => "bool",
        }
    }
}

impl MatchPattern {
    pub fn span(&self) -> Span {
        match self {
            Self::Variant { span, .. }
            | Self::Record { span, .. }
            | Self::Wildcard { span }
            | Self::Literal { span, .. }
            | Self::Or { span, .. }
            | Self::Binding { span, .. } => *span,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RecordMatchPatternField {
    pub name: String,
    pub name_span: Span,
    pub pattern: RecordMatchFieldPattern,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum RecordMatchFieldPattern {
    Binding {
        name: String,
        span: Span,
    },
    Wildcard {
        span: Span,
    },
    Record {
        type_name: String,
        type_span: Span,
        fields: Vec<RecordMatchPatternField>,
        span: Span,
    },
}

impl RecordMatchFieldPattern {
    pub fn span(&self) -> Span {
        match self {
            Self::Binding { span, .. } | Self::Wildcard { span } | Self::Record { span, .. } => {
                *span
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct MatchPatternField {
    pub name: String,
    pub name_span: Span,
    pub binding: String,
    pub binding_span: Span,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct FieldInitializer {
    pub name: String,
    pub name_span: Span,
    pub value: Expr,
    pub span: Span,
}

/// Field Mutation v1: the single-level field of a `<binding>.<field> = ...`
/// assignment target. Nested place chains (`a.b.c = ...`) never parse.
#[derive(Clone, Debug)]
pub struct FieldTarget {
    pub name: String,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum Statement {
    Let {
        name: String,
        name_span: Span,
        /// Explicit Mutation v1: `true` when the source declared `let mut`.
        mutable: bool,
        /// Class Inheritance v1: optional declared type annotation
        /// `let name: T = value;`. `None` when the source omitted `: T`.
        declared: Option<Type>,
        value: Expr,
        span: Span,
    },
    /// Explicit Mutation v1: `<binding> = <expr>;` over a simple local
    /// binding. Statement-only; there is no assignment expression.
    /// Field Mutation v1 extends the target with one direct scalar field:
    /// `<binding>.<field> = <expr>;`.
    Assign {
        name: String,
        name_span: Span,
        field: Option<FieldTarget>,
        value: Expr,
        span: Span,
    },
    /// Unsafe Boundary Mechanics v1: `@audit("...") unsafe { ... }`. The body
    /// is an ordinary safe block expression; no raw pointers or memory
    /// operations exist inside. The audit summary is recorded verbatim.
    Unsafe {
        audit: String,
        audit_span: Span,
        body: Box<Expr>,
        span: Span,
    },
    /// Bounded While-Loops v1: `while <condition> { <body> }`. The condition
    /// must be exactly `bool` and the body is an ordinary block whose value is
    /// discarded; the statement itself produces no value.
    While {
        condition: Box<Expr>,
        body: Box<Expr>,
        span: Span,
    },
}

impl Statement {
    /// The statement's evaluated expression: the initializer of a `let`, the
    /// assigned value of an assignment, or the ordinary block body of an
    /// unsafe boundary statement. While statements carry two evaluated
    /// expressions and must be traversed with [`Statement::child`] instead.
    pub fn value(&self) -> &Expr {
        match self {
            Self::Let { value, .. } | Self::Assign { value, .. } => value,
            Self::Unsafe { body, .. } => body,
            Self::While { .. } => panic!("while statements expose condition and body children"),
        }
    }

    /// Mutable access to the statement's evaluated expression.
    pub fn value_mut(&mut self) -> &mut Expr {
        match self {
            Self::Let { value, .. } | Self::Assign { value, .. } => value,
            Self::Unsafe { body, .. } => body,
            Self::While { .. } => panic!("while statements expose condition and body children"),
        }
    }

    /// The statement's source-level binding name. Only `let` and assignment
    /// statements bind or target a name.
    pub fn name(&self) -> &str {
        match self {
            Self::Let { name, .. } | Self::Assign { name, .. } => name,
            Self::Unsafe { .. } | Self::While { .. } => {
                panic!("only let and assignment statements declare a binding")
            }
        }
    }

    /// The verbatim audit summary of an unsafe boundary statement.
    pub fn audit(&self) -> Option<&str> {
        match self {
            Self::Unsafe { audit, .. } => Some(audit),
            _ => None,
        }
    }

    /// `true` for assignment statements.
    pub fn is_assign(&self) -> bool {
        matches!(self, Self::Assign { .. })
    }

    /// Number of directly nested evaluated expressions. `let`, assignment,
    /// and unsafe statements contribute one; while statements contribute its
    /// condition then its body, in evaluation order.
    pub fn child_count(&self) -> usize {
        match self {
            Self::Let { .. } | Self::Assign { .. } | Self::Unsafe { .. } => 1,
            Self::While { .. } => 2,
        }
    }

    /// One directly nested evaluated expression in left-to-right order.
    pub fn child(&self, index: usize) -> Option<&Expr> {
        match self {
            Self::Let { value, .. } | Self::Assign { value, .. } => (index == 0).then_some(value),
            Self::Unsafe { body, .. } => (index == 0).then_some(body.as_ref()),
            Self::While {
                condition, body, ..
            } => [condition.as_ref(), body.as_ref()].get(index).copied(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

impl BinaryOp {
    pub fn precedence(self) -> u8 {
        match self {
            BinaryOp::Or => 1,
            BinaryOp::And => 2,
            BinaryOp::Eq | BinaryOp::Ne => 3,
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => 4,
            BinaryOp::Add | BinaryOp::Sub => 5,
            BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => 6,
        }
    }

    pub fn text(self) -> &'static str {
        match self {
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Rem => "%",
            BinaryOp::Eq => "==",
            BinaryOp::Ne => "!=",
            BinaryOp::Lt => "<",
            BinaryOp::Le => "<=",
            BinaryOp::Gt => ">",
            BinaryOp::Ge => ">=",
            BinaryOp::And => "&&",
            BinaryOp::Or => "||",
        }
    }
}

impl Expr {
    fn visit_call_nodes(&self, mut visit: impl FnMut(&Expr)) {
        const FIXED_DEPTH: usize = 513;
        let mut stack = [None; FIXED_DEPTH];
        stack[0] = Some((self, 0usize));
        let mut len = 1usize;
        let mut overflow = Vec::new();
        loop {
            let state = if let Some(state) = overflow.pop() {
                state
            } else if len != 0 {
                len -= 1;
                stack[len].take().expect("call visitor frame retained")
            } else {
                break;
            };
            let (expression, next_child) = state;
            if next_child == 0 {
                visit(expression);
            }
            if let Some(child) = expression.child(next_child) {
                let parent = (expression, next_child + 1);
                if overflow.is_empty() && len + 2 <= stack.len() {
                    stack[len] = Some(parent);
                    stack[len + 1] = Some((child, 0));
                    len += 2;
                } else {
                    if overflow.is_empty() {
                        overflow.extend(stack[..len].iter_mut().filter_map(Option::take));
                        len = 0;
                    }
                    overflow.push(parent);
                    overflow.push((child, 0));
                }
            }
        }
    }

    fn child(&self, index: usize) -> Option<&Expr> {
        match &self.kind {
            ExprKind::Call { args, .. } => args.get(index),
            ExprKind::MethodCall { receiver, args, .. } => (index == 0)
                .then_some(receiver.as_ref())
                .or_else(|| args.get(index - 1)),
            ExprKind::SuperMethod { args, .. } => args.get(index),
            ExprKind::Unary { value, .. }
            | ExprKind::Try { operand: value }
            | ExprKind::Project { base: value, .. } => (index == 0).then_some(value),
            ExprKind::Binary { left, right, .. } => {
                [left.as_ref(), right.as_ref()].get(index).copied()
            }
            ExprKind::Block { statements, tail } => {
                let mut offset = 0;
                for statement in statements {
                    let count = statement.child_count();
                    if index < offset + count {
                        return statement.child(index - offset);
                    }
                    offset += count;
                }
                (index == offset).then_some(tail)
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => [
                condition.as_ref(),
                then_branch.as_ref(),
                else_branch.as_ref(),
            ]
            .get(index)
            .copied(),
            ExprKind::ConstructRecord { fields, .. }
            | ExprKind::ConstructVariant { fields, .. } => {
                fields.get(index).map(|field| &field.value)
            }
            ExprKind::Match { scrutinee, arms } => (index == 0)
                .then_some(scrutinee.as_ref())
                .or_else(|| arms.get(index - 1).map(|arm| &arm.value)),
            ExprKind::UpdateRecord { base, fields } => (index == 0)
                .then_some(base.as_ref())
                .or_else(|| fields.get(index - 1).map(|field| &field.value)),
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
            | ExprKind::Var(_) => None,
        }
    }

    pub fn visit_calls(&self, visit: &mut impl FnMut(&str, Span)) {
        self.visit_call_nodes(|expression| {
            if let ExprKind::Call { name, .. } = &expression.kind {
                visit(name, expression.span);
            }
        });
    }

    pub fn visit_call_instances(&self, visit: &mut impl FnMut(&str, &[Type], Span)) {
        self.visit_call_nodes(|expression| {
            if let ExprKind::Call {
                name,
                type_arguments,
                ..
            } = &expression.kind
            {
                visit(name, type_arguments, expression.span);
            }
        });
    }
}

#[cfg(test)]
mod call_visitor_tests {
    use super::*;

    fn call(name: &str, marker: usize) -> Expr {
        Expr {
            span: Span {
                start: marker,
                end: marker + 1,
                line: 1,
                column: marker + 1,
            },
            kind: ExprKind::Call {
                name: name.to_owned(),
                type_arguments: vec![Type::Named {
                    name: format!("T{marker}"),
                    arguments: Vec::new(),
                }],
                args: Vec::new(),
            },
        }
    }

    #[test]
    fn iterative_call_visitors_preserve_preorder_and_authored_child_order() {
        let span = Span::default();
        let expression = Expr {
            span,
            kind: ExprKind::Call {
                name: "outer".to_owned(),
                type_arguments: vec![Type::Named {
                    name: "T0".to_owned(),
                    arguments: Vec::new(),
                }],
                args: vec![
                    Expr {
                        span,
                        kind: ExprKind::Block {
                            statements: vec![Statement::Let {
                                name: "value".to_owned(),
                                name_span: span,
                                mutable: false,
                                declared: None,
                                value: call("first", 1),
                                span,
                            }],
                            tail: Box::new(Expr {
                                span,
                                kind: ExprKind::If {
                                    condition: Box::new(call("second", 2)),
                                    then_branch: Box::new(call("third", 3)),
                                    else_branch: Box::new(call("fourth", 4)),
                                },
                            }),
                        },
                    },
                    Expr {
                        span,
                        kind: ExprKind::ConstructRecord {
                            type_name: "Pair".to_owned(),
                            type_span: span,
                            type_arguments: Vec::new(),
                            fields: vec![
                                FieldInitializer {
                                    name: "left".to_owned(),
                                    name_span: span,
                                    value: call("fifth", 5),
                                    span,
                                },
                                FieldInitializer {
                                    name: "right".to_owned(),
                                    name_span: span,
                                    value: call("sixth", 6),
                                    span,
                                },
                            ],
                        },
                    },
                    Expr {
                        span,
                        kind: ExprKind::Match {
                            scrutinee: Box::new(call("seventh", 7)),
                            arms: vec![
                                MatchArm {
                                    pattern: MatchPattern::Wildcard { span },
                                    guard: None,
                                    value: call("eighth", 8),
                                    span,
                                },
                                MatchArm {
                                    pattern: MatchPattern::Wildcard { span },
                                    guard: None,
                                    value: call("ninth", 9),
                                    span,
                                },
                            ],
                        },
                    },
                    Expr {
                        span,
                        kind: ExprKind::UpdateRecord {
                            base: Box::new(call("tenth", 10)),
                            fields: vec![
                                FieldInitializer {
                                    name: "left".to_owned(),
                                    name_span: span,
                                    value: call("eleventh", 11),
                                    span,
                                },
                                FieldInitializer {
                                    name: "right".to_owned(),
                                    name_span: span,
                                    value: call("twelfth", 12),
                                    span,
                                },
                            ],
                        },
                    },
                    Expr {
                        span,
                        kind: ExprKind::Try {
                            operand: Box::new(call("thirteenth", 13)),
                        },
                    },
                    Expr {
                        span,
                        kind: ExprKind::Project {
                            base: Box::new(call("fourteenth", 14)),
                            field: "value".to_owned(),
                            field_span: span,
                        },
                    },
                    Expr {
                        span,
                        kind: ExprKind::ConstructVariant {
                            type_name: "Choice".to_owned(),
                            type_span: span,
                            type_arguments: Vec::new(),
                            case_name: "Value".to_owned(),
                            case_span: span,
                            fields: vec![FieldInitializer {
                                name: "value".to_owned(),
                                name_span: span,
                                value: call("fifteenth", 15),
                                span,
                            }],
                        },
                    },
                    Expr {
                        span,
                        kind: ExprKind::Binary {
                            op: BinaryOp::Add,
                            left: Box::new(call("sixteenth", 16)),
                            right: Box::new(call("seventeenth", 17)),
                        },
                    },
                ],
            },
        };

        let expected_names = [
            "outer",
            "first",
            "second",
            "third",
            "fourth",
            "fifth",
            "sixth",
            "seventh",
            "eighth",
            "ninth",
            "tenth",
            "eleventh",
            "twelfth",
            "thirteenth",
            "fourteenth",
            "fifteenth",
            "sixteenth",
            "seventeenth",
        ];
        let mut calls = Vec::new();
        expression.visit_calls(&mut |name, span| calls.push((name.to_owned(), span.start)));
        assert_eq!(
            calls
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            expected_names
        );
        assert_eq!(
            calls.iter().map(|(_, marker)| *marker).collect::<Vec<_>>(),
            (0..=17).collect::<Vec<_>>()
        );

        let mut instances = Vec::new();
        expression.visit_call_instances(&mut |name, arguments, span| {
            instances.push((name.to_owned(), arguments[0].to_string(), span.start));
        });
        assert_eq!(
            instances
                .iter()
                .map(|(name, _, _)| name.as_str())
                .collect::<Vec<_>>(),
            expected_names
        );
        assert_eq!(
            instances
                .iter()
                .map(|(_, ty, _)| ty.as_str())
                .collect::<Vec<_>>(),
            (0..=17)
                .map(|marker| format!("T{marker}"))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            instances
                .iter()
                .map(|(_, _, marker)| *marker)
                .collect::<Vec<_>>(),
            (0..=17).collect::<Vec<_>>()
        );
    }
}
