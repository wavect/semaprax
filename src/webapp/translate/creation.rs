//! Creation previews interpret the already-admitted policy AST with unknown
//! row inputs. They grant no write authority and never invent a default row.
use super::{binary_values, builtin, js_string, Binding, Bound, Translator, Ty};
use crate::ast::{
    BinaryOp, Expr, ExprKind, Function, MatchPattern, PatternLiteral, Statement, UnaryOp,
};
use std::collections::BTreeSet;

// Unknown values and definite evaluation failures are distinct: NOT must not
// turn a trap into permission. Thunks preserve known short-circuit decisions.
const SUPPORT: &str = r#"const U=Symbol('unknown row'), F=Symbol('policy failure');
const safe=f=>{try{const v=f();return v===undefined?U:v;}catch{return F;}};
const apply=(f,...v)=>v.includes(F)?F:v.includes(U)?U:safe(()=>f(...v));
const not=v=>v===F?F:v===U?U:v===true?false:v===false?true:F;
const and=(v,f)=>v===F?F:v===false?false:v===true?f():v===U?(()=>{const w=f();return w===false?false:w===F?F:U;})():F;
const or=(v,f)=>v===F?F:v===true?true:v===false?f():v===U?(()=>{const w=f();return w===true?true:U;})():F;
const merge=(a,b)=>a===b?a:(a===F&&b===false)||(b===F&&a===false)?false:U;
const choose=(v,a,b)=>v===F?F:v===true?a():v===false?b():v===U?merge(a(),b()):F;
const gate=(v,f)=>v===F||v===false?F:v===true?f():v===U?(()=>{const w=f();return w===false||w===F?w:U;})():F;
"#;

impl<'a> Translator<'a> {
    pub(in crate::webapp) fn creation(&self, function: &'a Function, bound: &[Bound]) -> String {
        let scope = bound
            .iter()
            .map(|b| Binding {
                name: b.name.clone(),
                js: if b.field {
                    "U".to_owned()
                } else {
                    format!("safe(()=>{})", b.js)
                },
                ty: b.ty.clone(),
                param: false,
            })
            .collect();
        let mut projection = Projection {
            translator: self,
            scope,
            fresh: 0,
            pending: BTreeSet::new(),
            remaining: 4096,
        };
        let body = projection.function(function);
        format!("(u) => {{ {SUPPORT}const value={body}; return value===F?false:typeof value==='boolean'?value:null; }}")
    }
}

struct Projection<'t, 'a> {
    translator: &'t Translator<'a>,
    scope: Vec<Binding>,
    fresh: usize,
    pending: BTreeSet<String>,
    remaining: usize,
}

impl<'a> Projection<'_, 'a> {
    fn fresh(&mut self) -> String {
        self.fresh += 1;
        format!("c{}", self.fresh)
    }

    fn function(&mut self, function: &'a Function) -> String {
        if !self.pending.insert(function.name.clone()) {
            return "U".to_owned();
        }
        let pre: Vec<_> = function.requires.iter().map(|c| self.expr(c).0).collect();
        let (body, ty) = self.expr(&function.body);
        let result = self.fresh();
        let depth = self.scope.len();
        self.scope.push(Binding {
            name: "result".to_owned(),
            js: result.clone(),
            ty,
            param: false,
        });
        let checks: Vec<_> = function
            .ensures
            .iter()
            .map(|clause| self.expr(clause).0)
            .collect();
        let mut post = result.clone();
        for check in checks.into_iter().rev() {
            post = format!("gate({check},()=>{post})");
        }
        self.scope.truncate(depth);
        let mut out =
            format!("(()=>{{const {result}={body};if({result}===F)return F;return {post};}})()");
        for clause in pre.into_iter().rev() {
            out = format!("gate({clause},()=>{out})");
        }
        self.pending.remove(&function.name);
        out
    }

    fn expr(&mut self, expr: &'a Expr) -> (String, Ty) {
        // Inlining a repeated helper graph must not expand exponentially.
        // Exhaustion loses precision, never source admission or authority.
        if self.remaining == 0 {
            return ("U".to_owned(), Ty::Bool);
        }
        self.remaining -= 1;
        match &expr.kind {
            ExprKind::Int(v) => (format!("{v}n"), Ty::Int),
            ExprKind::Float64(bits) => (format!("{:?}", f64::from_bits(*bits)), Ty::Float),
            ExprKind::Bool(v) => (v.to_string(), Ty::Bool),
            ExprKind::String(v) => (js_string(v), Ty::Str),
            ExprKind::Char(v) => (
                char::from_u32(*v)
                    .map(|v| js_string(&v.to_string()))
                    .unwrap_or_else(|| "U".to_owned()),
                Ty::Char,
            ),
            ExprKind::Var(name) => self
                .scope
                .iter()
                .rev()
                .find(|b| b.name == *name)
                .map(|b| (b.js.clone(), b.ty.clone()))
                .unwrap_or_else(|| ("U".to_owned(), Ty::Bool)),
            ExprKind::Unary { op, value } => {
                let (v, ty) = self.expr(value);
                let js = match (op, &ty) {
                    (UnaryOp::Not, Ty::Bool) => format!("not({v})"),
                    (UnaryOp::Neg, Ty::Int) => format!("apply(x=>rt.neg(x),{v})"),
                    (UnaryOp::Neg, Ty::Float) => format!("apply(x=>-x,{v})"),
                    _ => "U".to_owned(),
                };
                (js, ty)
            }
            ExprKind::Binary { op, left, right } => {
                let (l, ty) = self.expr(left);
                let (r, _) = self.expr(right);
                if *op == BinaryOp::And {
                    return (format!("and({l},()=>{r})"), Ty::Bool);
                }
                if *op == BinaryOp::Or {
                    return (format!("or({l},()=>{r})"), Ty::Bool);
                }
                match binary_values(*op, "x".to_owned(), "y".to_owned(), &ty, expr) {
                    Ok((js, result)) => (format!("apply((x,y)=>{js},{l},{r})"), result),
                    Err(_) => ("U".to_owned(), ty),
                }
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let c = self.expr(condition).0;
                let (a, ty) = self.expr(then_branch);
                let b = self.expr(else_branch).0;
                (format!("choose({c},()=>{a},()=>{b})"), ty)
            }
            ExprKind::Block { statements, tail } => {
                let depth = self.scope.len();
                let mut out = "(()=>{".to_owned();
                for statement in statements {
                    let Statement::Let { name, value, .. } = statement else {
                        self.scope.truncate(depth);
                        return ("U".to_owned(), Ty::Bool);
                    };
                    let (value, ty) = self.expr(value);
                    let local = self.fresh();
                    out.push_str(&format!("const {local}={value};if({local}===F)return F;"));
                    self.scope.push(Binding {
                        name: name.clone(),
                        js: local,
                        ty,
                        param: false,
                    });
                }
                let (tail, ty) = self.expr(tail);
                self.scope.truncate(depth);
                out.push_str(&format!("return {tail};}})()"));
                (out, ty)
            }
            ExprKind::Match {
                scrutinee, arms, ..
            } => {
                let (value, ty) = self.expr(scrutinee);
                let local = self.fresh();
                let mut branches = Vec::new();
                let mut result = Ty::Bool;
                for arm in arms {
                    let depth = self.scope.len();
                    let mut condition = self.pattern(&arm.pattern, &local, &ty);
                    if let Some(guard) = &arm.guard {
                        condition = format!("and({condition},()=>{})", self.expr(guard).0);
                    }
                    let (value, result_ty) = self.expr(&arm.value);
                    result = result_ty;
                    self.scope.truncate(depth);
                    branches.push((condition, value));
                }
                let mut out = "F".to_owned();
                for (condition, value) in branches.into_iter().rev() {
                    out = format!("choose({condition},()=>{value},()=>{out})");
                }
                (format!("(({local})=>{local}===F?F:{out})({value})"), result)
            }
            ExprKind::ConstructVariant {
                type_name,
                case_name,
                fields,
                ..
            } if fields.is_empty() => (js_string(case_name), Ty::Enum(type_name.clone())),
            ExprKind::Call {
                name,
                type_arguments,
                args,
            } if type_arguments.is_empty() => self.call(name, args),
            // The ordinary translator has already validated the source subset.
            // This preview never converts an unsupported shape into refusal.
            _ => ("U".to_owned(), Ty::Bool),
        }
    }

    fn call(&mut self, name: &str, args: &'a [Expr]) -> (String, Ty) {
        let values: Vec<_> = args.iter().map(|arg| self.expr(arg)).collect();
        if let Some((target, ty)) = builtin(name) {
            let locals: Vec<_> = (0..values.len()).map(|i| format!("a{i}")).collect();
            let actual: Vec<_> = values.iter().map(|(v, _)| v.as_str()).collect();
            return (
                format!(
                    "apply(({})=>{target}({}),{})",
                    locals.join(","),
                    locals.join(","),
                    actual.join(",")
                ),
                ty,
            );
        }
        let Some(function) = self.translator.functions.get(name).copied() else {
            return ("U".to_owned(), Ty::Bool);
        };
        let ty = self
            .translator
            .scalar(&function.return_type)
            .unwrap_or(Ty::Bool);
        if values.len() != function.params.len() {
            return ("U".to_owned(), ty);
        }
        let mut out = "(()=>{".to_owned();
        let mut scope = Vec::new();
        for (param, (value, ty)) in function.params.iter().zip(values) {
            let local = self.fresh();
            out.push_str(&format!("const {local}={value};if({local}===F)return F;"));
            scope.push(Binding {
                name: param.name.clone(),
                js: local,
                ty,
                param: false,
            });
        }
        let saved = std::mem::replace(&mut self.scope, scope);
        let body = self.function(function);
        self.scope = saved;
        out.push_str(&format!("return {body};}})()"));
        (out, ty)
    }

    fn pattern(&mut self, pattern: &MatchPattern, value: &str, ty: &Ty) -> String {
        let literal = match pattern {
            MatchPattern::Wildcard { .. } => return "true".to_owned(),
            MatchPattern::Binding { name, .. } => {
                self.scope.push(Binding {
                    name: name.clone(),
                    js: value.to_owned(),
                    ty: ty.clone(),
                    param: false,
                });
                return "true".to_owned();
            }
            MatchPattern::Variant {
                case_name, fields, ..
            } if fields.is_empty() => js_string(case_name),
            MatchPattern::Literal { value, .. } => match value {
                PatternLiteral::Int(v) => format!("{v}n"),
                PatternLiteral::Bool(v) => v.to_string(),
                PatternLiteral::Char(v) => char::from_u32(*v)
                    .map(|v| js_string(&v.to_string()))
                    .unwrap_or_else(|| "U".to_owned()),
                _ => return "U".to_owned(),
            },
            MatchPattern::Or { alternatives, .. } => {
                let mut out = "false".to_owned();
                for alternative in alternatives.iter().rev() {
                    let p = self.pattern(alternative, value, ty);
                    out = format!("or({p},()=>{out})");
                }
                return out;
            }
            _ => return "U".to_owned(),
        };
        format!("apply(x=>x==={literal},{value})")
    }
}
