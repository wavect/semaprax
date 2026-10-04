//! A closed translation of checked immutable-list source. Every admitted body
//! expression is emitted; source mutations never inherit a canned definition.
use super::refused;
use crate::ast::{
    BinaryOp, Expr, ExprKind, Function, MatchMode, MatchPattern, ParamMode, Program, Type,
};
use crate::diagnostic::Diagnostic;

fn list(ty: &Type) -> bool {
    matches!(ty, Type::Named { name, arguments } if name == "List" && arguments.as_slice() == [Type::I64])
}
fn variable(expr: &Expr, name: &str) -> bool {
    matches!(&expr.kind, ExprKind::Var(found) if found == name)
}
fn signature(function: &Function, insert: bool) -> bool {
    let wanted: &[(&str, bool)] = if insert {
        &[("value", false), ("input", true)]
    } else {
        &[("input", true)]
    };
    function.explicit_id
        && function.type_parameters.is_empty()
        && function.effects.is_empty()
        && function.yields.is_none()
        && function.follows.is_none()
        && function.requires.is_empty()
        && function.ensures.is_empty()
        && function.params.len() == wanted.len()
        && list(&function.return_type)
        && function
            .params
            .iter()
            .zip(wanted)
            .all(|(p, (name, is_list))| {
                p.name == *name
                    && p.mode == ParamMode::Value
                    && if *is_list {
                        list(&p.ty)
                    } else {
                        p.ty == Type::I64
                    }
            })
}

struct Translator {
    insert: bool,
    work: usize,
}
impl Translator {
    fn expr(
        &mut self,
        expression: &Expr,
        cons_scope: bool,
        depth: usize,
    ) -> Result<String, Diagnostic> {
        self.work += 1;
        if self.work > 128 || depth > 24 {
            return Err(refused("source expression budget exceeded"));
        }
        let nested = depth + 1;
        match &expression.kind {
            ExprKind::Block { statements, tail } if statements.is_empty() => {
                self.expr(tail, cons_scope, nested)
            }
            ExprKind::Var(name)
                if name == "input"
                    || self.insert && name == "value"
                    || cons_scope && matches!(name.as_str(), "head" | "tail") =>
            {
                Ok(name.clone())
            }
            ExprKind::Binary {
                op: BinaryOp::Le,
                left,
                right,
            } => Ok(format!(
                "({} ≤ {})",
                self.expr(left, cons_scope, nested)?,
                self.expr(right, cons_scope, nested)?
            )),
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => Ok(format!(
                "(if {} then {} else {})",
                self.expr(condition, cons_scope, nested)?,
                self.expr(then_branch, cons_scope, nested)?,
                self.expr(else_branch, cons_scope, nested)?
            )),
            ExprKind::Call {
                name,
                type_arguments,
                args,
            } if type_arguments.is_empty() => match (name.as_str(), args.as_slice()) {
                ("list_nil", []) => Ok("[]".into()),
                ("list_cons", [head, tail]) => Ok(format!(
                    "({} :: {})",
                    self.expr(head, cons_scope, nested)?,
                    self.expr(tail, cons_scope, nested)?
                )),
                ("insert", [value, input])
                    if !self.insert || cons_scope && variable(input, "tail") =>
                {
                    Ok(format!(
                        "(insert {} {})",
                        self.expr(value, cons_scope, nested)?,
                        self.expr(input, cons_scope, nested)?
                    ))
                }
                ("sort", [input]) if !self.insert && cons_scope && variable(input, "tail") => {
                    Ok("(sort tail)".into())
                }
                _ => Err(refused(
                    "source call or nondecreasing recursion is outside collection profile",
                )),
            },
            ExprKind::Match {
                mode: MatchMode::Value,
                scrutinee,
                arms,
            } if !cons_scope && arms.len() == 2 && arms.iter().all(|arm| arm.guard.is_none()) => {
                if !matches!(&scrutinee.kind, ExprKind::Call { name, type_arguments, args } if name == "list_uncons" && type_arguments.is_empty() && args.len() == 1 && variable(&args[0], "input"))
                {
                    return Err(refused(
                        "collection match must destructure its exact List input",
                    ));
                }
                let nil = matches!(&arms[0].pattern, MatchPattern::Variant { type_name, case_name, fields, .. } if type_name == "ListStep" && case_name == "Nil" && fields.is_empty());
                let cons = matches!(&arms[1].pattern, MatchPattern::Variant { type_name, case_name, fields, .. } if type_name == "ListStep" && case_name == "Cons" && fields.len() == 2 && fields[0].name == "head" && fields[0].binding == "head" && fields[1].name == "tail" && fields[1].binding == "tail");
                if !nil || !cons {
                    return Err(refused(
                        "collection match needs exact Nil/Cons head/tail binders",
                    ));
                }
                Ok(format!(
                    "(match input with | [] => {} | head :: tail => {})",
                    self.expr(&arms[0].value, false, nested)?,
                    self.expr(&arms[1].value, true, nested)?
                ))
            }
            _ => Err(refused(
                "source expression is outside the immutable comparison/list profile",
            )),
        }
    }
}

pub(super) fn definitions(program: &Program) -> Result<String, Diagnostic> {
    if !program.module_uses.is_empty()
        || program.functions.iter().any(|function| {
            matches!(
                function.name.as_str(),
                "list_nil" | "list_cons" | "list_uncons"
            )
        })
    {
        return Err(refused(
            "imported or shadowed list operations are outside the closed collection profile",
        ));
    }
    if let Some(error) = crate::verify::verify(program).into_iter().next() {
        return Err(error);
    }
    let hir = crate::hir::resolve(program).map_err(|errors| {
        errors
            .into_iter()
            .next()
            .unwrap_or_else(|| refused("HIR resolution failed"))
    })?;
    crate::hir::validate(&hir).map_err(|_| refused("HIR replay failed"))?;
    let mut output = String::from("import Std\nnamespace SemapraxLaw15Collection\n");
    for (name, stable_id, insert) in [
        ("insert", "law15.collection.insert", true),
        ("sort", "law15.collection.sort", false),
    ] {
        let function = program
            .functions
            .iter()
            .find(|f| f.stable_id == stable_id)
            .ok_or_else(|| refused("selected collection declaration is absent"))?;
        if function.name != name || !signature(function, insert) {
            return Err(refused(
                "selected declaration identity or pure List signature differs",
            ));
        }
        let resolved = hir
            .functions
            .iter()
            .find(|candidate| candidate.id.as_str() == stable_id)
            .ok_or_else(|| refused("selected collection HIR declaration is absent"))?;
        if !crate::list_ops::is_list(&resolved.return_type) {
            return Err(refused(
                "source result does not resolve to the compiler-owned immutable List",
            ));
        }
        let body = Translator { insert, work: 0 }.expr(&function.body, false, 0)?;
        let params = if insert {
            "(value : Int) (input : List Int)"
        } else {
            "(input : List Int)"
        };
        output.push_str(&format!("def {name} {params} : List Int :=\n  {body}\n\n"));
    }
    Ok(output)
}
