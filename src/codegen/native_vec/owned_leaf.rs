//! Private compact owning rows. Descriptor identity is emitted from checked HIR.
use crate::hir::{ResolvedProgram, ResolvedType};

pub(in crate::codegen) struct Layout {
    pub(in crate::codegen) symbol: String,
    pub(in crate::codegen) kinds: Vec<u32>,
    pub(in crate::codegen) offsets: Vec<u32>,
    pub(in crate::codegen) stride: u32,
    capacity: u64,
    pub(in crate::codegen) tag: u32,
    identity: String,
}

pub(in crate::codegen) fn program_uses_field_reads(program: &ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
        .any(|function| {
            std::iter::once(&function.body)
                .chain(&function.requires)
                .chain(&function.ensures)
                .any(crate::vec_field::expression_uses)
        })
}

pub(in crate::codegen) fn layout(
    program: &ResolvedProgram,
    element: &ResolvedType,
) -> Option<Layout> {
    let admitted = crate::hir::owned_leaf_collection::layout(&program.declarations, element)?;
    let legacy = crate::hir::owned_record_collection::is_admitted_owned_record_collection_element(
        &program.declarations,
        element,
    );
    let symbol = if *element == ResolvedType::String {
        "spx_leaf_string_v1".to_owned()
    } else {
        format!(
            "{}_leaf_v1",
            super::super::native_emit::c_record_symbol(element)
        )
    };
    let types: Vec<_> = admitted.fields.map_or_else(
        || vec![ResolvedType::String],
        |f| f.iter().map(|f| f.ty.clone()).collect(),
    );
    let mut offsets = Vec::with_capacity(types.len());
    let mut kinds = Vec::with_capacity(types.len());
    let mut stride = 0;
    let mut legacy_owner = 0;
    for ty in types {
        let kind = match ty {
            ResolvedType::I64 => 1,
            ResolvedType::I32 => 2,
            ResolvedType::U8 => 3,
            ResolvedType::Usize => 4,
            ResolvedType::Char => 5,
            ResolvedType::F32 => 6,
            ResolvedType::F64 => 7,
            ResolvedType::Bool => 8,
            ResolvedType::String => 9,
            ResolvedType::Bytes => 10,
            _ => return None,
        };
        kinds.push(kind);
        offsets.push(if legacy {
            if kind == 10 {
                let offset = legacy_owner * 16;
                legacy_owner += 1;
                offset
            } else {
                32
            }
        } else {
            stride
        });
        stride += if kind == 10 { 16 } else { 8 };
    }
    let mut identity = element.identity_key();
    if let Some(fields) = admitted.fields {
        for f in fields {
            identity.push('|');
            identity.push_str(f.id.as_str());
            identity.push(':');
            identity.push_str(&f.ty.identity_key());
        }
    }
    Some(Layout {
        symbol,
        kinds,
        offsets,
        stride,
        capacity: admitted.capacity(),
        tag: if legacy { 10 } else { 12 },
        identity,
    })
}

pub(super) fn emit_runtime(output: &mut impl super::super::COutput, program: &ResolvedProgram) {
    let field_reads = program_uses_field_reads(program);
    if field_reads {
        // Additive programs bind tag-10 descriptors at construction. Keep the
        // legacy runtime text and context layout exact for all older programs.
        let runtime = include_str!("owned_leaf.c");
        let anchor = "    return e;\n}";
        assert_eq!(runtime.matches(anchor).count(), 1);
        output.push_str(&runtime.replacen(anchor,
            "    if (d->tag == 10 && e->owned_leaf_layout != d)\n        spx_runtime_invariant_failure(\"legacy owned record descriptor mismatch\");\n    return e;\n}", 1));
        output.push_str(include_str!("legacy_field.c"));
    } else {
        output.push_str(include_str!("owned_leaf.c"));
    }
    let mut layouts = std::collections::BTreeMap::new();
    for f in program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
    {
        for root in std::iter::once(&f.body)
            .chain(f.requires.iter())
            .chain(f.ensures.iter())
        {
            let mut pending = vec![root];
            while let Some(expression) = pending.pop() {
                if let crate::hir::ResolvedExprKind::VecFieldRead { element, .. } = &expression.kind
                {
                    if let Some(l) = layout(program, element) {
                        layouts.insert(l.symbol.clone(), l);
                    }
                }
                if field_reads {
                    if let crate::hir::ResolvedExprKind::Closure { body, .. } = &expression.kind {
                        pending.push(body);
                    }
                }
                crate::hir::push_resolved_expression_children_in_authored_order(
                    expression,
                    &mut pending,
                );
            }
            crate::hir::visit_resolved_calls(root, &mut |_, _, arguments| {
                for element in arguments {
                    if let Some(l) = layout(program, element) {
                        layouts.insert(l.symbol.clone(), l);
                    }
                }
            });
        }
    }
    for l in layouts.into_values() {
        let kinds = l
            .kinds
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let offsets = l
            .offsets
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let identity = super::super::native_emit::c_string(&l.identity);
        output.push_str(&format!("static __attribute__((unused)) const spx_leaf_layout_v1 {} = {{ .count = {}, .stride = {}, .capacity = {}, .tag = {}, .kinds = {{ {kinds} }}, .offsets = {{ {offsets} }}, .identity = \"{identity}\" }};\n", l.symbol, l.kinds.len(), l.stride, l.capacity, l.tag));
    }
}
