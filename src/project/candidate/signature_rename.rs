//! Scope-preserving substitution over the canonical, previously admitted AST.
use super::*;
use crate::diagnostic::Diagnostic;

type Scope = BTreeMap<String, Option<String>>;

pub(super) fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G259", message)]
}

pub(super) fn apply(
    function: &mut Function,
    original: &[Param],
    renames: &BTreeMap<String, String>,
    destinations: &BTreeSet<String>,
    occupied: &mut BTreeSet<String>,
) -> Result<()> {
    let scope = original
        .iter()
        .map(|param| (param.name.clone(), renames.get(&param.name).cloned()))
        .collect::<Scope>();
    let mut pass = Rename {
        destinations,
        occupied,
        nodes: 0,
        generated: 0,
    };
    for expression in &mut function.requires {
        pass.expression(expression, &scope, 0)?;
    }
    let mut ensures_scope = scope.clone();
    ensures_scope.insert("result".into(), Some("result".into()));
    for expression in &mut function.ensures {
        pass.expression(expression, &ensures_scope, 0)?;
    }
    pass.expression(&mut function.body, &scope, 0)
}

struct Rename<'a> {
    destinations: &'a BTreeSet<String>,
    occupied: &'a mut BTreeSet<String>,
    nodes: usize,
    generated: usize,
}

impl Rename<'_> {
    fn budget(&mut self, depth: usize) -> Result<()> {
        self.nodes += 1;
        if depth > MAX_WALK_DEPTH || self.nodes > MAX_WALK_NODES {
            return Err(vec![Diagnostic::io(
                "SPX-G261",
                "parameter substitution exceeds its structural bound",
            )]);
        }
        Ok(())
    }

    fn reference(name: &mut String, scope: &Scope) -> Result<()> {
        match scope.get(name) {
            Some(Some(replacement)) => *name = replacement.clone(),
            Some(None) => {
                return Err(invalid(
                    "removed parameter remains referenced during parameter renaming",
                ))
            }
            None => {}
        }
        Ok(())
    }

    fn binding(&mut self, name: &mut String, scope: &mut Scope) -> Result<()> {
        let original = name.clone();
        if self.destinations.contains(name) {
            loop {
                let candidate = format!("spx_sig_bind_{}", self.generated);
                self.generated += 1;
                if self.generated > MAX_WALK_NODES {
                    return Err(vec![Diagnostic::io(
                        "SPX-G261",
                        "parameter substitution name allocation exceeds its bound",
                    )]);
                }
                if self.occupied.insert(candidate.clone()) {
                    *name = candidate;
                    break;
                }
            }
        }
        scope.insert(original, Some(name.clone()));
        Ok(())
    }

    fn record_pattern(
        &mut self,
        pattern: &mut RecordMatchFieldPattern,
        scope: &mut Scope,
        depth: usize,
    ) -> Result<()> {
        self.budget(depth)?;
        match pattern {
            RecordMatchFieldPattern::Binding { name, .. } => self.binding(name, scope)?,
            RecordMatchFieldPattern::Record { fields, .. } => {
                for field in fields {
                    self.record_pattern(&mut field.pattern, scope, depth + 1)?;
                }
            }
            RecordMatchFieldPattern::Wildcard { .. } => {}
        }
        Ok(())
    }

    fn pattern(
        &mut self,
        pattern: &mut MatchPattern,
        scope: &mut Scope,
        depth: usize,
    ) -> Result<()> {
        self.budget(depth)?;
        match pattern {
            MatchPattern::Binding { name, .. } => self.binding(name, scope)?,
            MatchPattern::Variant { fields, .. } => {
                for field in fields {
                    self.binding(&mut field.binding, scope)?;
                }
            }
            MatchPattern::Record { fields, .. } => {
                for field in fields {
                    self.record_pattern(&mut field.pattern, scope, depth + 1)?;
                }
            }
            MatchPattern::Or { alternatives, .. } => {
                // The admitted language restricts or-patterns to literal atoms.
                // Reject any future binding-bearing extension until its shared
                // branch-binding identity contract is implemented here.
                for alternative in alternatives {
                    if !matches!(alternative, MatchPattern::Literal { .. }) {
                        return Err(invalid(
                            "parameter substitution requires literal-only or-patterns",
                        ));
                    }
                    self.budget(depth + 1)?;
                }
            }
            MatchPattern::Wildcard { .. } | MatchPattern::Literal { .. } => {}
        }
        Ok(())
    }

    fn expression(&mut self, expression: &mut Expr, scope: &Scope, depth: usize) -> Result<()> {
        self.budget(depth)?;
        let next = depth + 1;
        match &mut expression.kind {
            ExprKind::Var(name) => Self::reference(name, scope)?,
            ExprKind::Block { statements, tail } => {
                let mut local = scope.clone();
                for statement in statements {
                    match statement {
                        Statement::Let { name, value, .. } => {
                            // Initializers resolve before this new binding exists.
                            self.expression(value, &local, next)?;
                            self.binding(name, &mut local)?;
                        }
                        Statement::Assign { name, value, .. } => {
                            Self::reference(name, &local)?;
                            self.expression(value, &local, next)?;
                        }
                        Statement::Unsafe { body, .. } => self.expression(body, &local, next)?,
                        Statement::While {
                            condition, body, ..
                        } => {
                            self.expression(condition, &local, next)?;
                            self.expression(body, &local, next)?;
                        }
                    }
                }
                self.expression(tail, &local, next)?;
            }
            ExprKind::Match {
                scrutinee, arms, ..
            } => {
                self.expression(scrutinee, scope, next)?;
                for arm in arms {
                    let mut local = scope.clone();
                    self.pattern(&mut arm.pattern, &mut local, next)?;
                    if let Some(guard) = &mut arm.guard {
                        self.expression(guard, &local, next)?;
                    }
                    self.expression(&mut arm.value, &local, next)?;
                }
            }
            ExprKind::Call { args, .. } | ExprKind::SuperMethod { args, .. } => {
                for arg in args {
                    self.expression(arg, scope, next)?;
                }
            }
            ExprKind::MethodCall { receiver, args, .. } => {
                self.expression(receiver, scope, next)?;
                for arg in args {
                    self.expression(arg, scope, next)?;
                }
            }
            ExprKind::Unary { value, .. }
            | ExprKind::Try { operand: value }
            | ExprKind::Project { base: value, .. } => self.expression(value, scope, next)?,
            ExprKind::Binary { left, right, .. } => {
                self.expression(left, scope, next)?;
                self.expression(right, scope, next)?;
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.expression(condition, scope, next)?;
                self.expression(then_branch, scope, next)?;
                self.expression(else_branch, scope, next)?;
            }
            ExprKind::ConstructRecord { fields, .. }
            | ExprKind::ConstructVariant { fields, .. } => {
                for field in fields {
                    self.expression(&mut field.value, scope, next)?;
                }
            }
            ExprKind::UpdateRecord { base, fields } => {
                self.expression(base, scope, next)?;
                for field in fields {
                    self.expression(&mut field.value, scope, next)?;
                }
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
            | ExprKind::String(_) => {}
        }
        Ok(())
    }
}
