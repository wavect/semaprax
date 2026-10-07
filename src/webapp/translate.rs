//! Translation of the admitted pure expression subset into JavaScript that
//! keeps SEMAPRAX semantics: `i64` is a BigInt with checked arithmetic
//! through `rt`, `f64` is an IEEE double, `&&`/`||` stay lazy, and string
//! lengths count UTF-8 bytes. Anything else is `SPX-WA103`.

use std::collections::{BTreeMap, BTreeSet};

use super::Ty;
use crate::ast::{
    BinaryOp, Expr, ExprKind, Function, MatchPattern, ParamMode, PatternLiteral, Statement, Type,
    UnaryOp,
};
use crate::diagnostic::Diagnostic;

const SUBSET_HELP: &str = "webapp functions may use literals, their parameters, `let`, arithmetic, comparisons, `&&`/`||`/`!`, `if`/`else`, `match` on variants and scalars, payload-free variant cases, string_len, string_len_chars, string_is_empty, string_contains, string_starts_with, string_concat, string_from_i64, string_from_char, `string_as_str` with the `str_*` view functions, and calls to other such functions";

/// A JSON string literal, which is also a JavaScript string literal.
pub(super) fn js_string(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_owned())
}

fn outside(what: &str, expr: &Expr) -> Diagnostic {
    Diagnostic::error(
        "SPX-WA103",
        format!("{what} is outside the webapp expression subset"),
        expr.span,
    )
    .with_help(SUBSET_HELP)
}

struct Binding {
    name: String,
    js: String,
    ty: Ty,
    param: bool,
}

/// One name a convention function's parameter or account binding resolves
/// to: `r.title` for a row field, `u.id` for `me`, and so on.
pub(super) struct Bound {
    pub(super) name: String,
    pub(super) js: String,
    pub(super) ty: Ty,
    /// A row field, reported as a rule's field when read.
    pub(super) field: bool,
}

pub(super) struct Translator<'a> {
    source: &'a str,
    enums: &'a BTreeMap<String, Vec<String>>,
    functions: &'a BTreeMap<&'a str, &'a Function>,
    helpers: BTreeMap<String, String>,
    pending: BTreeSet<String>,
    scope: Vec<Binding>,
    used: BTreeSet<String>,
    fresh: usize,
}

impl<'a> Translator<'a> {
    pub(super) fn new(
        source: &'a str,
        enums: &'a BTreeMap<String, Vec<String>>,
        functions: &'a BTreeMap<&'a str, &'a Function>,
    ) -> Self {
        Translator {
            source,
            enums,
            functions,
            helpers: BTreeMap::new(),
            pending: BTreeSet::new(),
            scope: Vec::new(),
            used: BTreeSet::new(),
            fresh: 0,
        }
    }

    /// Every helper function reached from an entity function, by name.
    pub(super) fn helpers(&self) -> String {
        self.helpers.values().map(String::as_str).collect()
    }

    /// Whether a helper with this name was translated.
    pub(super) fn reached(&self, name: &str) -> bool {
        self.helpers.contains_key(name) || self.pending.contains(name)
    }

    pub(super) fn scalar(&self, ty: &Type) -> Option<Ty> {
        match ty {
            Type::I64 => Some(Ty::Int),
            Type::F64 => Some(Ty::Float),
            Type::Bool => Some(Ty::Bool),
            // A borrowed `str` view and an owned `string` share one runtime
            // representation, so helpers over views translate unchanged.
            Type::String | Type::Str => Some(Ty::Str),
            Type::Char => Some(Ty::Char),
            Type::Named { name, arguments }
                if arguments.is_empty() && self.enums.contains_key(name) =>
            {
                Some(Ty::Enum(name.clone()))
            }
            _ => None,
        }
    }

    fn enter(&mut self, bound: &[Bound]) {
        self.scope = bound
            .iter()
            .map(|bound| Binding {
                name: bound.name.clone(),
                js: bound.js.clone(),
                ty: bound.ty.clone(),
                param: bound.field,
            })
            .collect();
    }

    /// The translated body of a convention function (`requires` clauses
    /// become preconditions), and the row fields it reads.
    pub(super) fn body(
        &mut self,
        function: &'a Function,
        bound: &[Bound],
    ) -> Result<(String, Vec<String>), Vec<Diagnostic>> {
        self.enter(bound);
        self.used.clear();
        let body = self.function_body(function).map_err(|error| vec![error])?;
        let fields = bound
            .iter()
            .filter(|bound| bound.field && self.used.contains(&bound.name))
            .map(|bound| bound.name.clone())
            .collect();
        Ok((body, fields))
    }

    /// The `requires` clauses of `<entity>_valid` as rule objects.
    pub(super) fn rules(
        &mut self,
        function: &'a Function,
        params: &[Bound],
    ) -> Result<Vec<String>, Vec<Diagnostic>> {
        let mut rules = Vec::new();
        let mut errors = Vec::new();
        for clause in &function.requires {
            self.enter(params);
            self.used.clear();
            match self.expr(clause) {
                Ok((js, Ty::Bool)) => {
                    let text = self
                        .source
                        .get(clause.span.start..clause.span.end)
                        .unwrap_or("");
                    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
                    let fields: Vec<String> = params
                        .iter()
                        .filter(|bound| bound.field && self.used.contains(&bound.name))
                        .map(|bound| js_string(&bound.name))
                        .collect();
                    rules.push(format!(
                        "{{ text: {}, fields: [{}], test: (r) => {js} }}",
                        js_string(&text),
                        fields.join(", ")
                    ));
                }
                Ok(_) => errors.push(outside("a non-bool rule", clause)),
                Err(error) => errors.push(error),
            }
        }
        if errors.is_empty() {
            Ok(rules)
        } else {
            Err(errors)
        }
    }

    /// One computed field object for `<entity>_<name>`.
    pub(super) fn computed(
        &mut self,
        function: &'a Function,
        name: &str,
        params: &[Bound],
    ) -> Result<String, Vec<Diagnostic>> {
        let Some(result) = self.scalar(&function.return_type) else {
            return Err(vec![Diagnostic::error(
                "SPX-WA102",
                format!("computed field `{name}` must return a webapp scalar"),
                function.name_span,
            )
            .with_help(super::FIELD_TYPE_HELP)]);
        };
        self.enter(params);
        let body = self.function_body(function).map_err(|error| vec![error])?;
        let mut out = format!(
            "{{ name: {}, type: \"{}\"",
            js_string(name),
            result.js_name()
        );
        if let Ty::Enum(enumeration) = &result {
            out.push_str(", enum: ");
            out.push_str(&js_string(enumeration));
        }
        out.push_str(", value: (r) => ");
        out.push_str(&body);
        out.push_str(" }");
        Ok(out)
    }

    /// `(rt.precondition(..), .., body)` with `requires` checked first.
    fn function_body(&mut self, function: &'a Function) -> Result<String, Diagnostic> {
        let mut parts = Vec::new();
        for clause in &function.requires {
            let (js, _) = self.expr(clause)?;
            parts.push(format!("rt.precondition({js})"));
        }
        let (body, ty) = self.expr(&function.body)?;
        if function.ensures.is_empty() {
            parts.push(body);
            return Ok(if parts.len() == 1 {
                parts.remove(0)
            } else {
                format!("({})", parts.join(", "))
            });
        }
        let result = self.fresh("result");
        let depth = self.scope.len();
        self.scope.push(Binding {
            name: "result".to_owned(),
            js: result.clone(),
            ty,
            param: false,
        });
        let mut checks = Vec::new();
        for clause in &function.ensures {
            let (js, _) = self.expr(clause)?;
            checks.push(format!("rt.postcondition({js});"));
        }
        self.scope.truncate(depth);
        let pre = parts
            .into_iter()
            .map(|part| format!("{part}; "))
            .collect::<String>();
        Ok(format!(
            "(() => {{ {pre}const {result} = {body}; {} return {result}; }})()",
            checks.join(" ")
        ))
    }

    fn helper(&mut self, function: &'a Function, call: &Expr) -> Result<Ty, Diagnostic> {
        let result = self
            .scalar(&function.return_type)
            .ok_or_else(|| outside("a call returning a non-webapp type", call))?;
        if self.helpers.contains_key(&function.name) || !self.pending.insert(function.name.clone())
        {
            return Ok(result);
        }
        if !function.effects.is_empty() || !function.type_parameters.is_empty() {
            return Err(outside("a call to an effectful or generic function", call));
        }
        let mut scope = Vec::new();
        for param in &function.params {
            let ty = self
                .scalar(&param.ty)
                .filter(|_| {
                    param.mode == ParamMode::Value
                        || (param.mode == ParamMode::Borrow && param.ty == Type::Str)
                })
                .ok_or_else(|| outside("a call to a function with a non-scalar parameter", call))?;
            scope.push(Binding {
                name: param.name.clone(),
                js: format!("p_{}", param.name),
                ty,
                param: false,
            });
        }
        let names: Vec<String> = scope.iter().map(|binding| binding.js.clone()).collect();
        let saved = std::mem::replace(&mut self.scope, scope);
        let saved_used = std::mem::take(&mut self.used);
        let body = self.function_body(function);
        self.scope = saved;
        self.used = saved_used;
        let body = body?;
        self.helpers.insert(
            function.name.clone(),
            format!(
                "function f_{}({}) {{ return {body}; }}\n",
                function.name,
                names.join(", ")
            ),
        );
        Ok(result)
    }

    fn lookup(&mut self, name: &str) -> Option<(String, Ty)> {
        let binding = self
            .scope
            .iter()
            .rev()
            .find(|binding| binding.name == name)?;
        if binding.param {
            self.used.insert(name.to_owned());
        }
        Some((binding.js.clone(), binding.ty.clone()))
    }

    fn fresh(&mut self, prefix: &str) -> String {
        self.fresh += 1;
        format!("{prefix}{}", self.fresh)
    }

    fn expr(&mut self, expr: &'a Expr) -> Result<(String, Ty), Diagnostic> {
        Ok(match &expr.kind {
            ExprKind::Int(value) => (format!("{value}n"), Ty::Int),
            ExprKind::Float64(bits) => {
                let value = f64::from_bits(*bits);
                if !value.is_finite() {
                    return Err(outside("a non-finite float literal", expr));
                }
                (format!("{value:?}"), Ty::Float)
            }
            ExprKind::Bool(value) => (value.to_string(), Ty::Bool),
            ExprKind::String(text) => (js_string(text), Ty::Str),
            ExprKind::Char(code) => {
                let ch = char::from_u32(*code).ok_or_else(|| outside("an invalid char", expr))?;
                (js_string(&ch.to_string()), Ty::Char)
            }
            ExprKind::Var(name) => self
                .lookup(name)
                .ok_or_else(|| outside(&format!("`{name}`"), expr))?,
            ExprKind::Unary { op, value } => {
                let (js, ty) = self.expr(value)?;
                match (op, &ty) {
                    (UnaryOp::Not, Ty::Bool) => (format!("(!{js})"), Ty::Bool),
                    (UnaryOp::Neg, Ty::Int) => (format!("rt.neg({js})"), Ty::Int),
                    (UnaryOp::Neg, Ty::Float) => (format!("(-{js})"), Ty::Float),
                    _ => return Err(outside("this unary operator", expr)),
                }
            }
            ExprKind::Binary { op, left, right } => self.binary(*op, left, right, expr)?,
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let (condition, _) = self.expr(condition)?;
                let (then_js, ty) = self.expr(then_branch)?;
                let (else_js, _) = self.expr(else_branch)?;
                (format!("({condition} ? {then_js} : {else_js})"), ty)
            }
            ExprKind::Block { statements, tail } if statements.is_empty() => self.expr(tail)?,
            ExprKind::Block { statements, tail } => {
                let depth = self.scope.len();
                let mut out = String::from("(() => { ");
                for statement in statements {
                    let Statement::Let { name, value, .. } = statement else {
                        self.scope.truncate(depth);
                        return Err(outside("a statement other than `let`", expr));
                    };
                    let (js, ty) = self.expr(value)?;
                    let local = self.fresh("v");
                    out.push_str(&format!("const {local} = {js}; "));
                    self.scope.push(Binding {
                        name: name.clone(),
                        js: local,
                        ty,
                        param: false,
                    });
                }
                let tail = self.expr(tail);
                self.scope.truncate(depth);
                let (js, ty) = tail?;
                out.push_str(&format!("return {js}; }})()"));
                (out, ty)
            }
            ExprKind::Match {
                scrutinee, arms, ..
            } => {
                let (value, scrutinee_ty) = self.expr(scrutinee)?;
                let local = self.fresh("m");
                let mut branches = Vec::new();
                let mut result = None;
                for arm in arms {
                    let depth = self.scope.len();
                    let condition = self.pattern(&arm.pattern, &local, &scrutinee_ty, expr)?;
                    let condition = match &arm.guard {
                        Some(guard) => format!("({condition} && {})", self.expr(guard)?.0),
                        None => condition,
                    };
                    let (js, ty) = self.expr(&arm.value)?;
                    self.scope.truncate(depth);
                    result.get_or_insert(ty);
                    branches.push((condition, js));
                }
                let mut out = String::from("rt.unreachable()");
                for (condition, js) in branches.into_iter().rev() {
                    out = format!("{condition} ? {js} : {out}");
                }
                let ty = result.ok_or_else(|| outside("an empty match", expr))?;
                (format!("(({local}) => {out})({value})"), ty)
            }
            ExprKind::ConstructVariant {
                type_name,
                case_name,
                fields,
                ..
            } if fields.is_empty() && self.enums.contains_key(type_name) => {
                (js_string(case_name), Ty::Enum(type_name.clone()))
            }
            ExprKind::Call {
                name,
                type_arguments,
                args,
            } if type_arguments.is_empty() => self.call(name, args, expr)?,
            _ => return Err(outside("this expression", expr)),
        })
    }

    fn pattern(
        &mut self,
        pattern: &MatchPattern,
        local: &str,
        ty: &Ty,
        expr: &Expr,
    ) -> Result<String, Diagnostic> {
        Ok(match pattern {
            MatchPattern::Wildcard { .. } => "true".to_owned(),
            MatchPattern::Variant {
                case_name, fields, ..
            } if fields.is_empty() => format!("{local} === {}", js_string(case_name)),
            MatchPattern::Literal { value, .. } => match value {
                PatternLiteral::Int(value) => format!("{local} === {value}n"),
                PatternLiteral::Bool(value) => format!("{local} === {value}"),
                PatternLiteral::Char(code) => {
                    let ch =
                        char::from_u32(*code).ok_or_else(|| outside("an invalid char", expr))?;
                    format!("{local} === {}", js_string(&ch.to_string()))
                }
                _ => return Err(outside("this literal pattern", expr)),
            },
            MatchPattern::Or { alternatives, .. } => {
                let mut parts = Vec::new();
                for alternative in alternatives {
                    parts.push(self.pattern(alternative, local, ty, expr)?);
                }
                format!("({})", parts.join(" || "))
            }
            MatchPattern::Binding { name, .. } => {
                self.scope.push(Binding {
                    name: name.clone(),
                    js: local.to_owned(),
                    ty: ty.clone(),
                    param: false,
                });
                "true".to_owned()
            }
            _ => return Err(outside("this match pattern", expr)),
        })
    }

    fn binary(
        &mut self,
        op: BinaryOp,
        left: &'a Expr,
        right: &'a Expr,
        expr: &Expr,
    ) -> Result<(String, Ty), Diagnostic> {
        let (l, ty) = self.expr(left)?;
        let (r, _) = self.expr(right)?;
        let checked = |name: &str| (format!("rt.{name}({l}, {r})"), Ty::Int);
        Ok(match (op, &ty) {
            (BinaryOp::Add, Ty::Int) => checked("add"),
            (BinaryOp::Sub, Ty::Int) => checked("sub"),
            (BinaryOp::Mul, Ty::Int) => checked("mul"),
            (BinaryOp::Div, Ty::Int) => checked("div"),
            (BinaryOp::Rem, Ty::Int) => checked("rem"),
            (BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div, Ty::Float) => {
                (format!("({l} {} {r})", op.text()), Ty::Float)
            }
            (BinaryOp::Eq, _) => (format!("({l} === {r})"), Ty::Bool),
            (BinaryOp::Ne, _) => (format!("({l} !== {r})"), Ty::Bool),
            (BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge, Ty::Int | Ty::Float) => {
                (format!("({l} {} {r})", op.text()), Ty::Bool)
            }
            (BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge, Ty::Str) => (
                format!("(rt.compareStrings({l}, {r}) {} 0)", op.text()),
                Ty::Bool,
            ),
            (BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge, Ty::Char) => (
                format!("({l}.codePointAt(0) {} {r}.codePointAt(0))", op.text()),
                Ty::Bool,
            ),
            (BinaryOp::And | BinaryOp::Or, Ty::Bool) => {
                (format!("({l} {} {r})", op.text()), Ty::Bool)
            }
            _ => return Err(outside(&format!("operator `{}` here", op.text()), expr)),
        })
    }

    fn call(
        &mut self,
        name: &str,
        args: &'a [Expr],
        expr: &Expr,
    ) -> Result<(String, Ty), Diagnostic> {
        let builtin = match name {
            "string_len" => Some(("rt.len", Ty::Int)),
            "string_len_chars" => Some(("rt.lenChars", Ty::Int)),
            "string_is_empty" => Some(("rt.isEmpty", Ty::Bool)),
            "string_contains" => Some(("rt.contains", Ty::Bool)),
            "string_starts_with" => Some(("rt.startsWith", Ty::Bool)),
            "string_concat" => Some(("rt.concat", Ty::Str)),
            "string_from_i64" => Some(("rt.fromI64", Ty::Str)),
            "string_from_char" | "string_as_str" => Some(("", Ty::Str)),
            "str_len_bytes" => Some(("rt.len", Ty::Int)),
            "str_is_empty" => Some(("rt.isEmpty", Ty::Bool)),
            "str_contains" => Some(("rt.contains", Ty::Bool)),
            "str_starts_with" => Some(("rt.startsWith", Ty::Bool)),
            _ => None,
        };
        let mut translated = Vec::new();
        for arg in args {
            translated.push(self.expr(arg)?.0);
        }
        let args = translated.join(", ");
        if let Some((target, ty)) = builtin {
            return Ok((format!("{target}({args})"), ty));
        }
        let function = *self
            .functions
            .get(name)
            .ok_or_else(|| outside(&format!("a call to `{name}`"), expr))?;
        let ty = self.helper(function, expr)?;
        Ok((format!("f_{name}({args})"), ty))
    }
}
