//! Seedable typed grammar for the commonly admitted scalar subset.
//!
//! A seed names exactly one module. The generator holds a structured program —
//! not a string — so the shrinker can remove a statement, an operand, a
//! contract clause, or a whole helper and still render a module the compiler
//! admits. Rendering is a pure function of the structure, so a seed, a shrink
//! step, and a replay on another machine all produce the same bytes.
//!
//! The admitted surface is deliberately narrow and effect-free: `i64`, `i32`,
//! `u8`, and `bool`; `let`, `let mut`, assignment, shadowing, `if`/`else`,
//! bounded `while`, `requires`/`ensures`, lazy `&&`/`||`, and calls into
//! strictly lower-indexed helpers. No module ever emits `permit`, `uses`,
//! `unsafe`, `extern`, a record, a string, or a path literal, so no generated
//! file can reach the filesystem, the network, a process, or a signing key.

use std::fmt::Write as _;

/// The scalar types this tranche generates. Observable case functions return
/// only `I64` or `Bool`, which is exactly the profile the native probe and the
/// Core-Wasm scalar export lane both admit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Type {
    I64,
    I32,
    U8,
    Bool,
}

impl Type {
    pub(crate) fn keyword(self) -> &'static str {
        match self {
            Self::I64 => "i64",
            Self::I32 => "i32",
            Self::U8 => "u8",
            Self::Bool => "bool",
        }
    }

    fn suffix(self) -> &'static str {
        match self {
            Self::I64 | Self::Bool => "",
            Self::I32 => "i32",
            Self::U8 => "u8",
        }
    }

    /// The neutral literal the shrinker collapses an expression to.
    pub(crate) fn zero(self) -> Expr {
        match self {
            Self::Bool => Expr::BoolLiteral(false),
            other => Expr::IntLiteral(other, 0),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    And,
    Or,
}

impl BinaryOp {
    fn spelling(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Rem => "%",
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
            Self::Eq => "==",
            Self::Ne => "!=",
            Self::And => "&&",
            Self::Or => "||",
        }
    }

    /// `&&` and `||` evaluate their right operand only when the left one
    /// demands it. The differential lanes must agree on that, so the checker
    /// needs to know which generated operators are lazy.
    pub(crate) fn is_lazy(self) -> bool {
        matches!(self, Self::And | Self::Or)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Expr {
    IntLiteral(Type, i64),
    BoolLiteral(bool),
    Variable(Type, String),
    Not(Box<Expr>),
    Binary {
        result: Type,
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Call {
        result: Type,
        callee: usize,
        arguments: Vec<Expr>,
    },
    Conditional {
        result: Type,
        condition: Box<Expr>,
        consequent: Box<Expr>,
        alternative: Box<Expr>,
    },
}

impl Expr {
    pub(crate) fn result_type(&self) -> Type {
        match self {
            Self::IntLiteral(scalar, _) => *scalar,
            Self::BoolLiteral(_) | Self::Not(_) => Type::Bool,
            Self::Variable(scalar, _) => *scalar,
            Self::Binary { result, .. }
            | Self::Call { result, .. }
            | Self::Conditional { result, .. } => *result,
        }
    }

    fn is_leaf(&self) -> bool {
        matches!(
            self,
            Self::IntLiteral(..) | Self::BoolLiteral(_) | Self::Variable(..)
        )
    }

    /// Immediate operands, in the fixed order every traversal uses.
    pub(crate) fn children(&self) -> Vec<&Expr> {
        match self {
            Self::IntLiteral(..) | Self::BoolLiteral(_) | Self::Variable(..) => Vec::new(),
            Self::Not(inner) => vec![inner.as_ref()],
            Self::Binary { left, right, .. } => vec![left.as_ref(), right.as_ref()],
            Self::Call { arguments, .. } => arguments.iter().collect(),
            Self::Conditional {
                condition,
                consequent,
                alternative,
                ..
            } => vec![
                condition.as_ref(),
                consequent.as_ref(),
                alternative.as_ref(),
            ],
        }
    }

    /// Operands that carry this expression's own type. The shrinker replaces a
    /// compound expression with one of these, which keeps the module typed
    /// while removing a whole subtree.
    fn same_typed_children(&self) -> Vec<Expr> {
        let scalar = self.result_type();
        self.children()
            .into_iter()
            .filter(|child| child.result_type() == scalar)
            .cloned()
            .collect()
    }

    /// Every strictly smaller replacement for this expression, in the order the
    /// shrinker should try them: collapse to a neutral literal first, then to
    /// each same-typed operand.
    pub(crate) fn simplifications(&self) -> Vec<Expr> {
        if self.is_leaf() {
            return Vec::new();
        }
        let mut candidates = vec![self.result_type().zero()];
        candidates.extend(self.same_typed_children());
        candidates.retain(|candidate| candidate != self);
        candidates
    }

    /// Immediate children with a setter, so a shrink pass can rewrite one
    /// operand in place without a visitor for every variant.
    pub(crate) fn children_mut(&mut self) -> Vec<&mut Expr> {
        match self {
            Self::IntLiteral(..) | Self::BoolLiteral(_) | Self::Variable(..) => Vec::new(),
            Self::Not(inner) => vec![inner.as_mut()],
            Self::Binary { left, right, .. } => vec![left.as_mut(), right.as_mut()],
            Self::Call { arguments, .. } => arguments.iter_mut().collect(),
            Self::Conditional {
                condition,
                consequent,
                alternative,
                ..
            } => vec![
                condition.as_mut(),
                consequent.as_mut(),
                alternative.as_mut(),
            ],
        }
    }

    pub(crate) fn calls(&self) -> Vec<usize> {
        let mut found = Vec::new();
        self.collect_calls(&mut found);
        found
    }

    fn collect_calls(&self, found: &mut Vec<usize>) {
        if let Self::Call { callee, .. } = self {
            found.push(*callee);
        }
        match self {
            Self::IntLiteral(..) | Self::BoolLiteral(_) | Self::Variable(..) => {}
            Self::Not(inner) => inner.collect_calls(found),
            Self::Binary { left, right, .. } => {
                left.collect_calls(found);
                right.collect_calls(found);
            }
            Self::Call { arguments, .. } => {
                for argument in arguments {
                    argument.collect_calls(found);
                }
            }
            Self::Conditional {
                condition,
                consequent,
                alternative,
                ..
            } => {
                condition.collect_calls(found);
                consequent.collect_calls(found);
                alternative.collect_calls(found);
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Statement {
    Let {
        name: String,
        mutable: bool,
        value: Expr,
    },
    Assign {
        name: String,
        value: Expr,
    },
    /// A counted loop. `counter` starts at `bound` and the rendered body
    /// decrements it once per iteration, so every generated loop terminates and
    /// the trip count is a structural constant the scaling fixture can vary.
    While {
        counter: String,
        bound: i64,
        accumulator: String,
        step: Expr,
    },
}

impl Statement {
    pub(crate) fn expressions_mut(&mut self) -> Vec<&mut Expr> {
        match self {
            Self::Let { value, .. } | Self::Assign { value, .. } => vec![value],
            Self::While { step, .. } => vec![step],
        }
    }

    pub(crate) fn expressions(&self) -> Vec<&Expr> {
        match self {
            Self::Let { value, .. } | Self::Assign { value, .. } => vec![value],
            Self::While { step, .. } => vec![step],
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Function {
    pub(crate) index: usize,
    pub(crate) stable_id: String,
    pub(crate) name: String,
    pub(crate) parameters: Vec<(String, Type)>,
    pub(crate) result: Type,
    pub(crate) requires: Option<Expr>,
    pub(crate) ensures: Option<Expr>,
    pub(crate) body: Vec<Statement>,
    pub(crate) tail: Expr,
}

impl Function {
    pub(crate) fn calls(&self) -> Vec<usize> {
        let mut found = self.tail.calls();
        for statement in &self.body {
            for expression in statement.expressions() {
                found.extend(expression.calls());
            }
        }
        found
    }
}

/// One generated module: helper functions, observable zero-argument case
/// functions, and the `main` the toolchain requires.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Module {
    pub(crate) seed: u64,
    pub(crate) module_name: String,
    pub(crate) helpers: Vec<Function>,
    pub(crate) cases: Vec<Function>,
}

impl Module {
    pub(crate) fn render(&self) -> String {
        let mut text = format!("module {};\n", self.module_name);
        for helper in &self.helpers {
            text.push('\n');
            render_function(&mut text, helper);
        }
        for case in &self.cases {
            text.push('\n');
            render_function(&mut text, case);
        }
        text.push('\n');
        text.push_str("@id(\"app.main\")\n");
        text.push_str("fn main() -> i64\n{\n");
        for case in &self.cases {
            // `main` names every case so no observable export is unreachable
            // from the entry point on any backend. The bindings are unused on
            // purpose; the case results are read through the export lanes.
            let _ = writeln!(text, "    let observed{} = {}();", case.index, case.name);
        }
        text.push_str("    0\n");
        text.push_str("}\n");
        text
    }
}

fn render_function(text: &mut String, function: &Function) {
    let _ = writeln!(text, "@id(\"{}\")", function.stable_id);
    let parameters = function
        .parameters
        .iter()
        .map(|(name, scalar)| format!("{name}: {}", scalar.keyword()))
        .collect::<Vec<_>>()
        .join(", ");
    let _ = writeln!(
        text,
        "fn {}({parameters}) -> {}",
        function.name,
        function.result.keyword()
    );
    if let Some(clause) = &function.requires {
        let _ = writeln!(text, "    requires {}", render_expr(clause));
    }
    if let Some(clause) = &function.ensures {
        let _ = writeln!(text, "    ensures {}", render_expr(clause));
    }
    text.push_str("{\n");
    for statement in &function.body {
        render_statement(text, statement);
    }
    let _ = writeln!(text, "    {}", render_expr(&function.tail));
    text.push_str("}\n");
}

fn render_statement(text: &mut String, statement: &Statement) {
    match statement {
        Statement::Let {
            name,
            mutable,
            value,
        } => {
            let marker = if *mutable { "mut " } else { "" };
            let _ = writeln!(text, "    let {marker}{name} = {};", render_expr(value));
        }
        Statement::Assign { name, value } => {
            let _ = writeln!(text, "    {name} = {};", render_expr(value));
        }
        Statement::While {
            counter,
            bound,
            accumulator,
            step,
        } => {
            let _ = writeln!(text, "    let mut {counter} = {bound};");
            let _ = writeln!(text, "    while {counter} > 0 {{");
            let _ = writeln!(text, "        {accumulator} = {};", render_expr(step));
            let _ = writeln!(text, "        {counter} = {counter} - 1;");
            let _ = writeln!(text, "        {counter} > 0");
            text.push_str("    }\n");
        }
    }
}

pub(crate) fn render_expr(expr: &Expr) -> String {
    match expr {
        Expr::IntLiteral(scalar, value) => format!("{value}{}", scalar.suffix()),
        Expr::BoolLiteral(value) => value.to_string(),
        Expr::Variable(_, name) => name.clone(),
        Expr::Not(inner) => format!("!({})", render_expr(inner)),
        Expr::Binary {
            op, left, right, ..
        } => format!(
            "({} {} {})",
            render_expr(left),
            op.spelling(),
            render_expr(right)
        ),
        Expr::Call {
            callee, arguments, ..
        } => {
            let rendered = arguments
                .iter()
                .map(render_expr)
                .collect::<Vec<_>>()
                .join(", ");
            format!("helper{callee}({rendered})")
        }
        Expr::Conditional {
            condition,
            consequent,
            alternative,
            ..
        } => format!(
            "if {} {{ {} }} else {{ {} }}",
            render_expr(condition),
            render_expr(consequent),
            render_expr(alternative)
        ),
    }
}

/// SplitMix64. Chosen because it is four lines, needs no dependency, and gives
/// the same stream on every platform and every Rust version, so a seed recorded
/// in a discrepancy report keeps reproducing.
#[derive(Clone, Debug)]
pub(crate) struct Rng {
    state: u64,
}

impl Rng {
    pub(crate) fn new(seed: u64) -> Self {
        Self {
            state: seed.wrapping_mul(0x9e37_79b9_7f4a_7c15).wrapping_add(1),
        }
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn below(&mut self, bound: usize) -> usize {
        assert!(bound > 0, "a choice needs at least one alternative");
        (self.next_u64() % bound as u64) as usize
    }

    fn chance(&mut self, numerator: u64, denominator: u64) -> bool {
        self.next_u64() % denominator < numerator
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len())]
    }
}

/// Hard structural bounds on one generated module, so a campaign can never
/// wander into a program whose compile or evaluation cost is unbounded.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Shape {
    pub(crate) helpers: usize,
    pub(crate) cases: usize,
    pub(crate) expression_depth: usize,
    pub(crate) statements: usize,
    pub(crate) loop_bound: i64,
}

impl Default for Shape {
    fn default() -> Self {
        Self {
            helpers: 3,
            cases: 5,
            expression_depth: 4,
            statements: 3,
            loop_bound: 3,
        }
    }
}

struct Scope {
    bindings: Vec<(String, Type, bool)>,
}

impl Scope {
    fn readable(&self, scalar: Type) -> Vec<String> {
        self.bindings
            .iter()
            .filter(|(_, binding_type, _)| *binding_type == scalar)
            .map(|(name, _, _)| name.clone())
            .collect()
    }

    fn mutable(&self, scalar: Type) -> Vec<String> {
        self.bindings
            .iter()
            .filter(|(_, binding_type, mutable)| *binding_type == scalar && *mutable)
            .map(|(name, _, _)| name.clone())
            .collect()
    }
}

pub(crate) struct Generator {
    rng: Rng,
    shape: Shape,
    seed: u64,
}

impl Generator {
    pub(crate) fn new(seed: u64, shape: Shape) -> Self {
        Self {
            rng: Rng::new(seed),
            shape,
            seed,
        }
    }

    pub(crate) fn module(&mut self) -> Module {
        let mut helpers: Vec<Function> = Vec::new();
        for index in 0..self.shape.helpers {
            let helper = self.helper(index, &helpers);
            helpers.push(helper);
        }
        let mut cases = Vec::new();
        for index in 0..self.shape.cases {
            cases.push(self.case(index, &helpers));
        }
        Module {
            seed: self.seed,
            module_name: format!("test.differential.s{:016x}", self.seed),
            helpers,
            cases,
        }
    }

    fn helper(&mut self, index: usize, earlier: &[Function]) -> Function {
        let arity = 1 + self.rng.below(2);
        let parameters = (0..arity)
            .map(|position| {
                let scalar =
                    self.rng
                        .pick(&[Type::I64, Type::I64, Type::I32, Type::U8, Type::Bool]);
                (format!("p{position}"), scalar)
            })
            .collect::<Vec<_>>();
        let result = self
            .rng
            .pick(&[Type::I64, Type::I64, Type::I32, Type::Bool]);
        let parameter_scope = Scope {
            bindings: parameters
                .iter()
                .map(|(name, scalar)| (name.clone(), *scalar, false))
                .collect(),
        };
        let requires = self.precondition(&parameter_scope);
        let ensures = self.postcondition(result);
        let mut scope = Scope {
            bindings: parameter_scope.bindings.clone(),
        };
        let body = self.block(&mut scope, index, earlier);
        let tail = self.expr(result, self.shape.expression_depth, &scope, index, earlier);
        Function {
            index,
            stable_id: format!("differential.helper{index}"),
            name: format!("helper{index}"),
            parameters,
            result,
            requires,
            ensures,
            body,
            tail,
        }
    }

    fn case(&mut self, index: usize, helpers: &[Function]) -> Function {
        let result = if self.rng.chance(1, 5) {
            Type::Bool
        } else {
            Type::I64
        };
        let mut scope = Scope {
            bindings: Vec::new(),
        };
        let callable = helpers.len();
        let ensures = self.postcondition(result);
        let body = self.block(&mut scope, callable, helpers);
        let tail = self.expr(
            result,
            self.shape.expression_depth,
            &scope,
            callable,
            helpers,
        );
        Function {
            index,
            stable_id: format!("differential.case{index}"),
            name: format!("case{index}"),
            parameters: Vec::new(),
            result,
            requires: None,
            ensures,
            body,
            tail,
        }
    }

    /// A precondition is one comparison over parameters or a bare `bool`
    /// parameter. Contracts stay call-free so a `requires` clause can never
    /// itself select an unrelated failure.
    fn precondition(&mut self, scope: &Scope) -> Option<Expr> {
        if !self.rng.chance(1, 3) {
            return None;
        }
        let booleans = scope.readable(Type::Bool);
        if !booleans.is_empty() && self.rng.chance(1, 2) {
            let index = self.rng.below(booleans.len());
            return Some(Expr::Variable(Type::Bool, booleans[index].clone()));
        }
        for scalar in [Type::I64, Type::I32, Type::U8] {
            let names = scope.readable(scalar);
            if names.is_empty() {
                continue;
            }
            let index = self.rng.below(names.len());
            let op = self
                .rng
                .pick(&[BinaryOp::Ge, BinaryOp::Le, BinaryOp::Ne, BinaryOp::Gt]);
            let bound = match scalar {
                Type::U8 => self.rng.pick(&[0_i64, 1, 128]),
                _ => self.rng.pick(&[-1_000_000_i64, 0, 1_000_000]),
            };
            return Some(Expr::Binary {
                result: Type::Bool,
                op,
                left: Box::new(Expr::Variable(scalar, names[index].clone())),
                right: Box::new(Expr::IntLiteral(scalar, bound)),
            });
        }
        // Every parameter list is non-empty, so this is only reached when the
        // one parameter is `bool` and the coin above chose the numeric arm.
        Some(Expr::BoolLiteral(true))
    }

    /// A postcondition names `result`. Sometimes it holds and sometimes it does
    /// not, so the postcondition-failure status is generated rather than only
    /// the precondition one.
    fn postcondition(&mut self, result: Type) -> Option<Expr> {
        if !self.rng.chance(1, 4) {
            return None;
        }
        let clause = Expr::Variable(result, "result".to_owned());
        Some(match result {
            Type::Bool => {
                if self.rng.chance(1, 2) {
                    clause
                } else {
                    Expr::Not(Box::new(clause))
                }
            }
            Type::U8 => Expr::Binary {
                result: Type::Bool,
                op: BinaryOp::Ge,
                left: Box::new(clause),
                right: Box::new(Expr::IntLiteral(Type::U8, self.rng.pick(&[0_i64, 128]))),
            },
            scalar => {
                let bound = self.rng.pick(&[-1_000_000_i64, 0, 1_000_000]);
                Expr::Binary {
                    result: Type::Bool,
                    op: BinaryOp::Ge,
                    left: Box::new(clause),
                    right: Box::new(Expr::IntLiteral(scalar, bound)),
                }
            }
        })
    }

    fn block(
        &mut self,
        scope: &mut Scope,
        callable: usize,
        helpers: &[Function],
    ) -> Vec<Statement> {
        let count = self.rng.below(self.shape.statements + 1);
        let mut statements = Vec::new();
        for position in 0..count {
            let Some(statement) = self.statement(position, scope, callable, helpers) else {
                continue;
            };
            if let Statement::Let {
                name,
                mutable,
                value,
            } = &statement
            {
                // Shadowing is deliberate: the name may already be bound, and
                // the later binding must win in every projection.
                scope
                    .bindings
                    .push((name.clone(), value.result_type(), *mutable));
            }
            statements.push(statement);
        }
        statements
    }

    fn statement(
        &mut self,
        position: usize,
        scope: &Scope,
        callable: usize,
        helpers: &[Function],
    ) -> Option<Statement> {
        let depth = self.shape.expression_depth.saturating_sub(1);
        match self.rng.below(6) {
            0..=2 => {
                let scalar = self
                    .rng
                    .pick(&[Type::I64, Type::I64, Type::I32, Type::Bool]);
                // SEMAPRAX does not admit shadowing: rebinding a live name is
                // `SPX-T209`. Every binding therefore gets a fresh name, and
                // the rebinding half of the issue's "shadowing/mutation" is
                // covered by `let mut` plus assignment below. The negative case
                // is pinned by its own hostile fixture.
                let name = format!("v{position}");
                let value = self.expr(scalar, depth, scope, callable, helpers);
                Some(Statement::Let {
                    name,
                    mutable: self.rng.chance(1, 2),
                    value,
                })
            }
            3..=4 => {
                let scalar = self.rng.pick(&[Type::I64, Type::I32, Type::Bool]);
                let targets = scope.mutable(scalar);
                if targets.is_empty() {
                    return None;
                }
                let index = self.rng.below(targets.len());
                let name = targets[index].clone();
                let value = self.expr(scalar, depth, scope, callable, helpers);
                Some(Statement::Assign { name, value })
            }
            _ => {
                let targets = scope.mutable(Type::I64);
                if targets.is_empty() {
                    return None;
                }
                let index = self.rng.below(targets.len());
                let accumulator = targets[index].clone();
                let increment = 1 + self.rng.below(3) as i64;
                Some(Statement::While {
                    counter: format!("n{position}"),
                    bound: self.shape.loop_bound,
                    step: Expr::Binary {
                        result: Type::I64,
                        op: BinaryOp::Add,
                        left: Box::new(Expr::Variable(Type::I64, accumulator.clone())),
                        right: Box::new(Expr::IntLiteral(Type::I64, increment)),
                    },
                    accumulator,
                })
            }
        }
    }

    fn expr(
        &mut self,
        scalar: Type,
        depth: usize,
        scope: &Scope,
        callable: usize,
        helpers: &[Function],
    ) -> Expr {
        if depth == 0 {
            return self.leaf(scalar, scope);
        }
        let arms = if scalar == Type::Bool { 7 } else { 5 };
        match self.rng.below(arms) {
            0 => self.leaf(scalar, scope),
            1 => {
                let candidates = helpers
                    .iter()
                    .filter(|helper| helper.index < callable && helper.result == scalar)
                    .map(|helper| (helper.index, helper.parameters.clone()))
                    .collect::<Vec<_>>();
                if candidates.is_empty() {
                    return self.operation(scalar, depth, scope, callable, helpers);
                }
                let selected = self.rng.below(candidates.len());
                let (callee, signature) = candidates[selected].clone();
                let arguments = signature
                    .iter()
                    .map(|(_, parameter_type)| {
                        self.expr(*parameter_type, depth - 1, scope, callable, helpers)
                    })
                    .collect();
                Expr::Call {
                    result: scalar,
                    callee,
                    arguments,
                }
            }
            2 => {
                let condition = self.expr(Type::Bool, depth - 1, scope, callable, helpers);
                let consequent = self.expr(scalar, depth - 1, scope, callable, helpers);
                let alternative = self.expr(scalar, depth - 1, scope, callable, helpers);
                Expr::Conditional {
                    result: scalar,
                    condition: Box::new(condition),
                    consequent: Box::new(consequent),
                    alternative: Box::new(alternative),
                }
            }
            5 => {
                // Lazy operands. A right side that would fail on its own proves
                // the short circuit rather than the arithmetic.
                let op = if self.rng.chance(1, 2) {
                    BinaryOp::And
                } else {
                    BinaryOp::Or
                };
                let left = self.expr(Type::Bool, depth - 1, scope, callable, helpers);
                let right = self.expr(Type::Bool, depth - 1, scope, callable, helpers);
                Expr::Binary {
                    result: Type::Bool,
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                }
            }
            6 => Expr::Not(Box::new(self.expr(
                Type::Bool,
                depth - 1,
                scope,
                callable,
                helpers,
            ))),
            _ => self.operation(scalar, depth, scope, callable, helpers),
        }
    }

    fn operation(
        &mut self,
        scalar: Type,
        depth: usize,
        scope: &Scope,
        callable: usize,
        helpers: &[Function],
    ) -> Expr {
        if scalar == Type::Bool {
            let operand = self.rng.pick(&[Type::I64, Type::I64, Type::I32, Type::U8]);
            let op = self.rng.pick(&[
                BinaryOp::Lt,
                BinaryOp::Le,
                BinaryOp::Gt,
                BinaryOp::Ge,
                BinaryOp::Eq,
                BinaryOp::Ne,
            ]);
            let left = self.expr(operand, depth - 1, scope, callable, helpers);
            let right = self.expr(operand, depth - 1, scope, callable, helpers);
            return Expr::Binary {
                result: Type::Bool,
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        // `%` is admitted on `i64` only; on `i32`, `u8`, and the floats it is
        // `SPX-T208`, so the operator table is per type rather than global.
        let op = if scalar == Type::I64 {
            self.rng.pick(&[
                BinaryOp::Add,
                BinaryOp::Add,
                BinaryOp::Sub,
                BinaryOp::Mul,
                BinaryOp::Div,
                BinaryOp::Rem,
            ])
        } else {
            self.rng.pick(&[
                BinaryOp::Add,
                BinaryOp::Add,
                BinaryOp::Sub,
                BinaryOp::Mul,
                BinaryOp::Div,
            ])
        };
        let left = self.expr(scalar, depth - 1, scope, callable, helpers);
        let right = if matches!(op, BinaryOp::Div | BinaryOp::Rem) {
            // One divisor in eight is the exact literal zero. Checked division
            // failure is an admitted outcome every backend must select
            // identically, not something to generate around.
            let magnitude = if self.rng.chance(1, 8) {
                0
            } else {
                1 + self.rng.below(7) as i64
            };
            Expr::IntLiteral(scalar, magnitude)
        } else {
            self.expr(scalar, depth - 1, scope, callable, helpers)
        };
        Expr::Binary {
            result: scalar,
            op,
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    fn leaf(&mut self, scalar: Type, scope: &Scope) -> Expr {
        let names = scope.readable(scalar);
        if !names.is_empty() && self.rng.chance(1, 2) {
            let index = self.rng.below(names.len());
            return Expr::Variable(scalar, names[index].clone());
        }
        match scalar {
            Type::Bool => Expr::BoolLiteral(self.rng.chance(1, 2)),
            Type::U8 => Expr::IntLiteral(Type::U8, self.rng.pick(&[0_i64, 1, 2, 7, 128, 255])),
            // Extremes stay in the table on purpose: checked overflow is a
            // first-class outcome the lanes must select identically. They are
            // outnumbered so that ordinary values dominate.
            Type::I32 => Expr::IntLiteral(
                Type::I32,
                self.rng
                    .pick(&[0_i64, 1, 2, 7, -3, 65_536, 2_147_483_647, -2_147_483_647]),
            ),
            Type::I64 => Expr::IntLiteral(
                Type::I64,
                self.rng.pick(&[
                    0_i64,
                    1,
                    2,
                    7,
                    -3,
                    42,
                    4_611_686_018_427_387_904,
                    9_223_372_036_854_775_807,
                ]),
            ),
        }
    }
}

/// What one module actually exercises. The differential tests assert on this so
/// a refactor cannot quietly stop generating the constructs the tranche claims
/// to cover.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Coverage {
    pub(crate) lazy_operators: usize,
    pub(crate) helper_calls: usize,
    pub(crate) conditionals: usize,
    pub(crate) nested_operands: usize,
    pub(crate) bounded_loops: usize,
    pub(crate) contracts: usize,
    pub(crate) shadowed_bindings: usize,
    pub(crate) mutations: usize,
    pub(crate) zero_divisors: usize,
}

impl Coverage {
    pub(crate) fn accumulate(&mut self, other: &Self) {
        self.lazy_operators += other.lazy_operators;
        self.helper_calls += other.helper_calls;
        self.conditionals += other.conditionals;
        self.nested_operands += other.nested_operands;
        self.bounded_loops += other.bounded_loops;
        self.contracts += other.contracts;
        self.shadowed_bindings += other.shadowed_bindings;
        self.mutations += other.mutations;
        self.zero_divisors += other.zero_divisors;
    }
}

pub(crate) fn coverage(module: &Module) -> Coverage {
    let mut found = Coverage::default();
    for function in module.helpers.iter().chain(module.cases.iter()) {
        found.contracts += usize::from(function.requires.is_some());
        found.contracts += usize::from(function.ensures.is_some());
        let mut bound: Vec<&str> = function
            .parameters
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        for statement in &function.body {
            match statement {
                Statement::Let { name, .. } => {
                    if bound.contains(&name.as_str()) {
                        // Never reached by construction; counted so the
                        // coverage test can pin that it stays zero.
                        found.shadowed_bindings += 1;
                    }
                    bound.push(name.as_str());
                }
                Statement::Assign { .. } => found.mutations += 1,
                Statement::While { .. } => {
                    found.bounded_loops += 1;
                    found.mutations += 1;
                }
            }
            for expression in statement.expressions() {
                walk_coverage(expression, &mut found);
            }
        }
        for clause in [function.requires.as_ref(), function.ensures.as_ref()]
            .into_iter()
            .flatten()
        {
            walk_coverage(clause, &mut found);
        }
        walk_coverage(&function.tail, &mut found);
    }
    found
}

fn walk_coverage(expr: &Expr, found: &mut Coverage) {
    match expr {
        Expr::Binary {
            op, left, right, ..
        } => {
            if op.is_lazy() {
                found.lazy_operators += 1;
            }
            if !left.is_leaf() || !right.is_leaf() {
                found.nested_operands += 1;
            }
            if matches!(op, BinaryOp::Div | BinaryOp::Rem)
                && matches!(right.as_ref(), Expr::IntLiteral(_, 0))
            {
                found.zero_divisors += 1;
            }
        }
        Expr::Call { .. } => found.helper_calls += 1,
        Expr::Conditional { .. } => found.conditionals += 1,
        Expr::IntLiteral(..) | Expr::BoolLiteral(_) | Expr::Variable(..) | Expr::Not(_) => {}
    }
    for child in expr.children() {
        walk_coverage(child, found);
    }
}
