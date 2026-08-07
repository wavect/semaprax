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
    Bool,
    Named(String),
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::I64 => write!(f, "i64"),
            Type::Bool => write!(f, "bool"),
            Type::Named(name) => write!(f, "{name}"),
        }
    }
}

impl Type {
    pub fn is_named(&self) -> bool {
        matches!(self, Type::Named(_))
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
    pub permits: Vec<String>,
    pub types: Vec<TypeDeclaration>,
    pub functions: Vec<Function>,
}

#[derive(Clone, Debug)]
pub struct TypeDeclaration {
    pub stable_id: String,
    pub explicit_id: bool,
    pub name: String,
    pub name_span: Span,
    pub kind: TypeDeclarationKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum TypeDeclarationKind {
    Resource,
    Record { fields: Vec<FieldDeclaration> },
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
    Bool(bool),
    Var(String),
    Call {
        name: String,
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
        fields: Vec<FieldInitializer>,
    },
    Project {
        base: Box<Expr>,
        field: String,
        field_span: Span,
    },
}

#[derive(Clone, Debug)]
pub struct FieldInitializer {
    pub name: String,
    pub name_span: Span,
    pub value: Expr,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum Statement {
    Let {
        name: String,
        name_span: Span,
        value: Expr,
        span: Span,
    },
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
    pub fn visit_calls(&self, visit: &mut impl FnMut(&str, Span)) {
        match &self.kind {
            ExprKind::Call { name, args } => {
                visit(name, self.span);
                for arg in args {
                    arg.visit_calls(visit);
                }
            }
            ExprKind::Unary { value, .. } => value.visit_calls(visit),
            ExprKind::Binary { left, right, .. } => {
                left.visit_calls(visit);
                right.visit_calls(visit);
            }
            ExprKind::Block { statements, tail } => {
                for statement in statements {
                    match statement {
                        Statement::Let { value, .. } => value.visit_calls(visit),
                    }
                }
                tail.visit_calls(visit);
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                condition.visit_calls(visit);
                then_branch.visit_calls(visit);
                else_branch.visit_calls(visit);
            }
            ExprKind::ConstructRecord { fields, .. } => {
                for field in fields {
                    field.value.visit_calls(visit);
                }
            }
            ExprKind::Project { base, .. } => base.visit_calls(visit),
            ExprKind::Int(_) | ExprKind::Bool(_) | ExprKind::Var(_) => {}
        }
    }
}
