//! Parse a solver's raw `(get-model)` output into typed values, and reject
//! anything this module cannot interpret unambiguously.
//!
//! A `sat` verdict's model is untrusted input exactly like any other
//! external tool output: [`parse_model`] only ever recognizes the closed
//! `define-fun <name> () <Int|Bool> <literal>` shape this crate's own
//! [`super::translate`] declarations produce. Any other shape (an
//! uninterpreted function, an `as-array` term, a real/rational, a
//! multi-branch `ite` body the solver left unevaluated) is a parse
//! failure, never a best-effort guess; see
//! [`docs/SMT-DISCHARGE-V1.md`](../../../docs/SMT-DISCHARGE-V1.md) "Model
//! parsing and validation".

use std::collections::BTreeMap;

/// One concrete value the solver assigned to a declared constant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelValue {
    Int(i128),
    Bool(bool),
}

/// A fully parsed counterexample model: every `define-fun` the solver
/// reported, keyed by name. Free variables the solver left unconstrained
/// (no `define-fun` at all) are simply absent — callers must treat a
/// missing key as "the solver did not report this value", never as zero or
/// `false` by default; see [`super::replay`].
pub type Model = BTreeMap<String, ModelValue>;

#[derive(Clone, Debug, Eq, PartialEq)]
enum SExpr {
    Atom(String),
    List(Vec<SExpr>),
}

fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        match ch {
            '(' | ')' => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
                tokens.push(ch.to_string());
            }
            c if c.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// Parse exactly one complete s-expression from the front of `tokens`,
/// returning it and the remaining tokens. Rejects trailing-garbage-free
/// well-formedness only at the call site in [`parse_model`], which checks
/// every token was consumed.
fn parse_one(tokens: &[String]) -> Result<(SExpr, &[String]), String> {
    let (first, rest) = tokens
        .split_first()
        .ok_or_else(|| "unexpected end of model text".to_owned())?;
    if first == "(" {
        let mut items = Vec::new();
        let mut remaining = rest;
        loop {
            match remaining.first() {
                None => return Err("unterminated list in model text".to_owned()),
                Some(token) if token == ")" => {
                    remaining = &remaining[1..];
                    break;
                }
                _ => {
                    let (item, next) = parse_one(remaining)?;
                    items.push(item);
                    remaining = next;
                }
            }
        }
        Ok((SExpr::List(items), remaining))
    } else if first == ")" {
        Err("unexpected `)` in model text".to_owned())
    } else {
        Ok((SExpr::Atom(first.clone()), rest))
    }
}

fn atom(expr: &SExpr) -> Option<&str> {
    match expr {
        SExpr::Atom(text) => Some(text.as_str()),
        SExpr::List(_) => None,
    }
}

/// Evaluate a value sub-expression that must be exactly one closed
/// numeral or boolean literal, optionally wrapped in one `(- <numeral>)`
/// for a negative integer. Anything else (nested arithmetic, `ite`, an
/// uninterpreted function application) is rejected.
fn value_of(expr: &SExpr) -> Result<ModelValue, String> {
    match expr {
        SExpr::Atom(text) => {
            if text == "true" {
                Ok(ModelValue::Bool(true))
            } else if text == "false" {
                Ok(ModelValue::Bool(false))
            } else {
                text.parse::<i128>()
                    .map(ModelValue::Int)
                    .map_err(|_| format!("`{text}` is not a recognized literal"))
            }
        }
        SExpr::List(items) => {
            if let [SExpr::Atom(op), operand] = items.as_slice() {
                if op == "-" {
                    if let ModelValue::Int(v) = value_of(operand)? {
                        return Ok(ModelValue::Int(-v));
                    }
                }
            }
            Err("model value is not a closed numeral or boolean literal".to_owned())
        }
    }
}

/// Parse the body of a `(get-model)` response (the solver's `sat` output
/// with the leading `sat` line already stripped by [`super::solver`]).
pub fn parse_model(text: &str) -> Result<Model, String> {
    let tokens = tokenize(text);
    let (top, remaining) = parse_one(&tokens)?;
    if !remaining.is_empty() {
        return Err("trailing tokens after the model's outer list".to_owned());
    }
    let SExpr::List(entries) = top else {
        return Err("model text is not a single outer list".to_owned());
    };
    let mut model = BTreeMap::new();
    for entry in entries {
        let SExpr::List(fields) = &entry else {
            return Err("model entry is not a list".to_owned());
        };
        // `(define-fun <name> (<params>) <sort> <value>)`; this module
        // only ever declares nullary constants, so `<params>` must be `()`.
        let [head, name_expr, params_expr, _sort_expr, value_expr] = fields.as_slice() else {
            return Err("model entry is not a 5-element define-fun".to_owned());
        };
        if atom(head) != Some("define-fun") {
            return Err("model entry is not `define-fun`".to_owned());
        }
        let name = atom(name_expr)
            .ok_or_else(|| "define-fun name is not an atom".to_owned())?
            .to_owned();
        if !matches!(params_expr, SExpr::List(items) if items.is_empty()) {
            return Err(format!("`{name}` is not a nullary constant in the model"));
        }
        let value = value_of(value_expr)?;
        if model.insert(name.clone(), value).is_some() {
            return Err(format!("duplicate `{name}` in model"));
        }
    }
    Ok(model)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_simple_int_and_bool_model() {
        let text = "(\n  (define-fun a () Int\n    5)\n  (define-fun ok () Bool\n    false)\n)";
        let model = parse_model(text).expect("valid model");
        assert_eq!(model.get("a"), Some(&ModelValue::Int(5)));
        assert_eq!(model.get("ok"), Some(&ModelValue::Bool(false)));
    }

    #[test]
    fn parses_a_negative_int_literal() {
        let text = "(\n  (define-fun a () Int\n    (- 5))\n)";
        let model = parse_model(text).expect("valid model");
        assert_eq!(model.get("a"), Some(&ModelValue::Int(-5)));
    }

    #[test]
    fn rejects_an_uninterpreted_or_nonliteral_value() {
        let text = "(\n  (define-fun a () Int\n    (+ 1 2))\n)";
        assert!(parse_model(text).is_err());
    }

    #[test]
    fn rejects_a_non_nullary_function_entry() {
        let text = "(\n  (define-fun f ((x Int)) Int\n    x)\n)";
        assert!(parse_model(text).is_err());
    }

    #[test]
    fn rejects_trailing_garbage_after_the_outer_list() {
        let text = "() extra";
        assert!(parse_model(text).is_err());
    }

    #[test]
    fn rejects_a_duplicate_name() {
        let text = "(\n(define-fun a () Int 1)\n(define-fun a () Int 2)\n)";
        assert!(parse_model(text).is_err());
    }

    #[test]
    fn rejects_a_missing_outer_wrapper_and_an_unterminated_list() {
        // A bare `define-fun` with no enclosing `( ... )` list of entries.
        assert!(parse_model("(define-fun a () Int 1)").is_err());
        // Missing the final closing paren.
        assert!(parse_model("((define-fun a () Int 1)").is_err());
    }
}
