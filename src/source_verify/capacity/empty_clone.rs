//! A local resource fact, never a runtime bypass or retained certificate.
use super::{Expr, ExprKind, Function, Statement, Type, TypeTable};

fn walk<'a>(root: &'a Expr, mut visitor: impl FnMut(&'a Expr)) -> bool {
    const DEPTH: usize = 513;
    let mut stack = [None; DEPTH];
    if !crate::bounded_output::reserve_active_required(std::mem::size_of_val(&stack)) {
        return false;
    }
    stack[0] = Some((root, 0usize));
    let mut len = 1;
    let mut work = 65_536usize;
    while len != 0 {
        let (expr, next) = stack[len - 1].expect("occupied empty-source frame");
        if next == 0 {
            if work == 0 {
                return false;
            }
            work -= 1;
            visitor(expr);
        }
        if let Some(child) = expr.child(next) {
            if len == DEPTH {
                return false;
            }
            stack[len - 1] = Some((expr, next + 1));
            stack[len] = Some((child, 0));
            len += 1;
        } else {
            len -= 1;
        }
    }
    true
}
fn capacity(types: &TypeTable<'_>, element: &Type) -> Option<u64> {
    let Type::Named { name, .. } = element else {
        return None;
    };
    let crate::ast::TypeDeclarationKind::Record { fields } = &types.declaration(name)?.kind else {
        return None;
    };
    let owned = fields
        .iter()
        .filter(|field| matches!(field.ty, Type::Bytes | Type::String))
        .count();
    let scalar = fields.len().checked_sub(owned)?;
    if owned == 0 {
        return None;
    }
    let words = if scalar == 0 {
        crate::vec_ops::MAX_CAPACITY
    } else {
        crate::vec_ops::MAX_CAPACITY / scalar as u64
    };
    Some(crate::vec_ops::MAX_CAPACITY.min(words).min(
        crate::vec_ops::MAX_OWNED_PAYLOAD_BYTES
            / (owned as u64 * crate::vec_ops::OWNED_PAYLOAD_BYTES_PER_ELEMENT),
    ))
}
fn same_var(expr: &Expr, root: &str) -> bool {
    matches!(&expr.kind, ExprKind::Var(name) if name == root)
}
/// The selected clone must be the fallible row operand of a direct consuming
/// renewal. Ordinary checking still authenticates every call, type and move.
pub(super) fn refused(function: &Function, types: &TypeTable<'_>, target: &Expr) -> bool {
    let ExprKind::Call {
        name,
        type_arguments,
        args,
    } = &target.kind
    else {
        return false;
    };
    let [element] = type_arguments.as_slice() else {
        return false;
    };
    let [carrier, index] = args.as_slice() else {
        return false;
    };
    let ExprKind::Var(root) = &carrier.kind else {
        return false;
    };
    if name != crate::vec_ops::CLONE_AT_NAME
        || !matches!(index.kind, ExprKind::Usize(_))
        || !matches!(element, Type::Named { arguments, .. } if arguments.is_empty())
        || !super::super::declared_type::owned_leaf_collection::admitted(types, element)
        || function.params.iter().any(|p| p.name == *root)
    {
        return false;
    }
    let ExprKind::Block { statements, .. } = &function.body.kind else {
        return false;
    };
    let Some(declaration) = statements.iter().position(|statement| matches!(statement,
        Statement::Let { name, mutable: false, declared: None, value, .. }
        if name == root && matches!(&value.kind, ExprKind::Call { name, type_arguments, args }
            if name == crate::vec_ops::WITH_CAPACITY_NAME && type_arguments.as_slice() == std::slice::from_ref(element)
                && matches!(args.as_slice(), [arg] if matches!(arg.kind, ExprKind::Usize(value) if capacity(types, element).is_some_and(|bound| value <= bound)))))) else { return false; };
    let eligible = statements[declaration + 1..].iter().any(|statement| {
        let Statement::ForOwn { values, body, .. } = statement else { return false; };
        if !matches!(&values.kind, ExprKind::Call { name, type_arguments, args }
            if name == "vec_into_iter" && type_arguments.as_slice() == std::slice::from_ref(element) && args.len() == 1) { return false; }
        let ExprKind::Block { statements: inner, .. } = &body.kind else { return false; };
        inner.iter().any(|statement| matches!(statement,
            Statement::Assign { name: destination, field: None, value, .. }
            if statements[..declaration].iter().any(|s| matches!(s,
                Statement::Let { name, mutable: true, value, .. } if name == destination
                && matches!(&value.kind, ExprKind::Call { name, type_arguments, .. }
                    if name == crate::vec_ops::WITH_CAPACITY_NAME && type_arguments.as_slice() == std::slice::from_ref(element))))
            && matches!(&value.kind, ExprKind::Call { name, type_arguments, args }
                if name == crate::vec_ops::PUSH_NAME && type_arguments.as_slice() == std::slice::from_ref(element)
                    && matches!(args.as_slice(), [out, row] if same_var(out, destination) && std::ptr::eq(row, target)))))
    });
    if !eligible {
        return false;
    }
    let mut references = 0usize;
    let mut allowed = 0usize;
    let mut declarations = 0usize;
    let mut valid = true;
    let complete = walk(&function.body, |expr| {
        references += usize::from(same_var(expr, root));
        if let ExprKind::Closure { .. } = &expr.kind {
            // No capture authority is inferred from a read-only use.
            let mut captured = false;
            valid &= walk(expr, |child| captured |= same_var(child, root));
            valid &= !captured;
        }
        if let ExprKind::Block { statements, .. } = &expr.kind {
            for statement in statements {
                match statement {
                    Statement::Let { name, .. } if name == root => declarations += 1,
                    Statement::Assign { name, .. } if name == root => valid = false,
                    Statement::ForOwn { item, .. }
                    | Statement::For { item, .. }
                    | Statement::ForBorrow { item, .. }
                        if item == root =>
                    {
                        valid = false
                    }
                    _ => {}
                }
            }
        }
        if let ExprKind::Call {
            name,
            type_arguments,
            args,
        } = &expr.kind
        {
            if type_arguments.as_slice() == std::slice::from_ref(element)
                && ((name == crate::vec_ops::LEN_NAME && args.len() == 1)
                    || (name == crate::vec_ops::CLONE_AT_NAME
                        && args.len() == 2
                        && matches!(args[1].kind, ExprKind::Usize(_))))
                && args.first().is_some_and(|arg| same_var(arg, root))
            {
                allowed += 1;
            }
        }
    });
    complete && valid && declarations == 1 && references == allowed && references != 0
}

pub(super) fn clone_capacity_flow(
    context: &super::SourceCapacityContext<'_>,
    expression: &Expr,
    path: &str,
) -> Option<crate::byte_data_capacity::CapacityFlow> {
    if matches!(&expression.kind, ExprKind::Call { name, .. } if name == crate::vec_ops::CLONE_AT_NAME)
        && context
            .ordinary
            .values()
            .any(|function| refused(function.payload(), context.types, expression))
    {
        Some(crate::byte_data_capacity::CapacityFlow::Empty)
    } else {
        super::super::declared_type::owned_leaf_collection::clone_capacity_flow(
            context.types,
            expression,
            path,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &str = r#"module owned_renewal;
@id("row") record Row {@id("row.n") n:i64,@id("row.text") text:string,@id("row.bytes") bytes:Bytes,}
@id("main") fn main()->i64 {
 let mut words=vec_with_capacity<Row>(2usize);
 let later=vec_with_capacity<Row>(1usize);
 let first=vec_push<Row>(vec_with_capacity<Row>(2usize),Row{n:7,text:"ab",bytes:bytes_zeroed(1usize)});
 let inputs=vec_push<Row>(first,Row{n:7,text:"ab",bytes:bytes_zeroed(1usize)});
 for own item in vec_into_iter<Row>(inputs) {words=vec_push<Row>(words,item);0}
 if vec_len<Row>(words)==2usize && vec_len<Row>(later)==0usize {29}else{0}
}
"#;
    fn selected(function: &Function) -> &Expr {
        let mut selected = None;
        assert!(walk(&function.body, |expr| {
            if matches!(&expr.kind, ExprKind::Call { name, .. } if name == crate::vec_ops::CLONE_AT_NAME)
            {
                selected = Some(expr);
            }
        }));
        selected.expect("owning clone operand")
    }
    #[test]
    fn immutable_empty_refusal_is_source_derived_and_mutation_alias_index_and_nonempty_stay_closed()
    {
        let source = SOURCE.replace(
            "words=vec_push<Row>(words,item)",
            "words=vec_push<Row>(words,vec_clone_at<Row>(later,0usize))",
        );
        let parsed = crate::parse(&source, "empty-refusal.spx").unwrap();
        let types = TypeTable::new(&parsed);
        let function = &parsed.functions[0];
        assert!(refused(function, &types, selected(function)));
        assert!(crate::check(&source, "empty-refusal.spx").is_ok());
        let alternate = source
            .replace(
                "let later=vec_with_capacity<Row>(1usize)",
                "let later=vec_with_capacity<Row>(3usize)",
            )
            .replace("later,0usize", "later,7usize");
        let alternate = crate::check(&alternate, "alternate-empty-refusal.spx").unwrap();
        assert!(refused(
            &alternate.functions[0],
            &TypeTable::new(&alternate),
            selected(&alternate.functions[0])
        ));
        for changed in [
            source.replace("let later=", "let mut later="),
            source.replace("later,0usize", "later,0usize+1usize"),
            source.replace("let later=vec_with_capacity<Row>(1usize);", "let later=vec_push<Row>(vec_with_capacity<Row>(1usize),Row{n:8,text: \"x\",bytes:bytes_zeroed(1usize)});"),
            source.replace("let later=vec_with_capacity<Row>(1usize);", "let later=vec_with_capacity<Row>(1usize);let alias=later;"),
            source.replace("words=vec_push<Row>(words,vec_clone_at<Row>(later,0usize))", "later=vec_with_capacity<Row>(1usize);words=vec_push<Row>(words,vec_clone_at<Row>(later,0usize))"),
            source.replace("vec_clone_at<Row>(later,0usize)", "vec_clone_at<Row>(words,0usize)"),
        ] {
            let parsed = crate::parse(&changed, "empty-refusal-hostile.spx").unwrap();
            let function = &parsed.functions[0];
            assert!(!refused(function, &TypeTable::new(&parsed), selected(function)), "{changed}");
        }
    }
}
