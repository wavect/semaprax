//! Canonical replay source for the exact Project-v8 linked HIR closure.
//!
//! Unlike the frozen v1-v7 function-only recipe, this projection retains the
//! stable-ID-authenticated authored record/variant declarations required by
//! the selected function closure. It never serializes an unrelated module.

use std::collections::BTreeMap;

use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::{
    DeclarationId, Place, PlaceProjection, ResolvedExpr, ResolvedExprKind, ResolvedMatchPattern,
    ResolvedRecordMatchFieldPattern, ResolvedStatement, ResolvedType, ResolvedTypeDeclaration,
    ResolvedTypeDeclarationKind,
};

use super::package_error;

const MAX_RECIPE_BYTES: usize = 1024 * 1024;
const MAX_FUNCTIONS: usize = 256;

struct Names {
    functions: BTreeMap<String, String>,
    types: BTreeMap<String, String>,
    cases: BTreeMap<String, String>,
    fields: BTreeMap<String, String>,
}

impl Names {
    fn derive(program: &crate::hir::ResolvedProgram) -> Result<Self, Diagnostic> {
        let mut functions = BTreeMap::new();
        for (index, function) in program.functions.iter().enumerate() {
            let name = if function.id == program.entrypoint {
                "main".to_owned()
            } else {
                format!("f{index}")
            };
            if functions
                .insert(function.id.as_str().to_owned(), name)
                .is_some()
            {
                return Err(package_error(
                    "owned-data semantic recipe duplicates a function identity",
                ));
            }
        }

        let mut authored = program
            .types
            .iter()
            .filter(|declaration| !crate::prelude::is_compiler_owned_id(declaration.id.as_str()))
            .collect::<Vec<_>>();
        authored.sort_by(|left, right| left.id.cmp(&right.id));
        let mut types = BTreeMap::new();
        let mut cases = BTreeMap::new();
        let mut fields = BTreeMap::new();
        for declaration in authored {
            if types
                .insert(declaration.id.as_str().to_owned(), declaration.name.clone())
                .is_some()
            {
                return Err(package_error(
                    "owned-data semantic recipe duplicates a type identity",
                ));
            }
            match &declaration.kind {
                ResolvedTypeDeclarationKind::Record {
                    fields: declaration_fields,
                } => {
                    for field in declaration_fields {
                        if fields
                            .insert(field.id.as_str().to_owned(), field.name.clone())
                            .is_some()
                        {
                            return Err(package_error(
                                "owned-data semantic recipe duplicates a field identity",
                            ));
                        }
                    }
                }
                ResolvedTypeDeclarationKind::Variant {
                    cases: declaration_cases,
                } => {
                    for case in declaration_cases {
                        if cases
                            .insert(case.id.as_str().to_owned(), case.name.clone())
                            .is_some()
                        {
                            return Err(package_error(
                                "owned-data semantic recipe duplicates a case identity",
                            ));
                        }
                        for field in &case.fields {
                            if fields
                                .insert(field.id.as_str().to_owned(), field.name.clone())
                                .is_some()
                            {
                                return Err(package_error(
                                    "owned-data semantic recipe duplicates a field identity",
                                ));
                            }
                        }
                    }
                }
                ResolvedTypeDeclarationKind::Class { .. }
                | ResolvedTypeDeclarationKind::Resource { .. } => {
                    return Err(package_error(
                        "owned-data semantic recipe admits only authored records and variants",
                    ));
                }
            }
        }
        Ok(Self {
            functions,
            types,
            cases,
            fields,
        })
    }

    fn type_name(&self, id: &DeclarationId) -> Result<&str, Diagnostic> {
        self.types
            .get(id.as_str())
            .map(String::as_str)
            .ok_or_else(|| package_error("owned-data semantic recipe type is unavailable"))
    }

    fn case_name(&self, id: &DeclarationId) -> Result<&str, Diagnostic> {
        match id.as_str() {
            crate::prelude::OPTION_NONE_ID => Ok("None"),
            crate::prelude::OPTION_SOME_ID => Ok("Some"),
            crate::prelude::RESULT_OK_ID => Ok("Ok"),
            crate::prelude::RESULT_ERR_ID => Ok("Err"),
            _ => self
                .cases
                .get(id.as_str())
                .map(String::as_str)
                .ok_or_else(|| package_error("owned-data semantic recipe case is unavailable")),
        }
    }

    fn field_name(&self, id: &DeclarationId) -> Result<&str, Diagnostic> {
        match id.as_str() {
            crate::prelude::OPTION_SOME_VALUE_ID | crate::prelude::RESULT_OK_VALUE_ID => {
                Ok("value")
            }
            crate::prelude::RESULT_ERR_ERROR_ID => Ok("error"),
            _ => self
                .fields
                .get(id.as_str())
                .map(String::as_str)
                .ok_or_else(|| package_error("owned-data semantic recipe field is unavailable")),
        }
    }
}

pub(super) fn render(program: &crate::hir::ResolvedProgram) -> Result<String, Diagnostic> {
    if program.functions.is_empty() || program.functions.len() > MAX_FUNCTIONS {
        return Err(package_error(
            "owned-data semantic recipe function inventory is unbounded",
        ));
    }
    if !program.permits.is_empty()
        || !program.interfaces.is_empty()
        || !program.function_templates.is_empty()
        || !program.function_instances.is_empty()
    {
        return Err(package_error(
            "owned-data semantic recipe contains unsupported authority or callable metadata",
        ));
    }
    let names = Names::derive(program)?;
    let mut output = String::from("module semaprax_npm_recipe;\n\n");
    render_types(program, &names, &mut output)?;
    for function in &program.functions {
        if !function.effects.is_empty()
            || !function.requires.is_empty()
            || !function.ensures.is_empty()
        {
            return Err(package_error(
                "owned-data semantic recipe does not admit effects or contracts",
            ));
        }
        let mut values = BTreeMap::<String, String>::new();
        let parameters = function
            .params
            .iter()
            .map(|parameter| {
                if values
                    .insert(parameter.id.as_str().to_owned(), parameter.name.clone())
                    .is_some()
                {
                    return Err(package_error(
                        "owned-data semantic recipe duplicates a parameter identity",
                    ));
                }
                let mode = match parameter.ownership {
                    crate::hir::OwnershipMode::Borrow => "borrow ",
                    crate::hir::OwnershipMode::Value => "",
                    crate::hir::OwnershipMode::Own => "own ",
                    crate::hir::OwnershipMode::Shared => {
                        return Err(package_error(
                            "owned-data semantic recipe parameter ownership is unsupported",
                        ));
                    }
                };
                Ok(format!(
                    "{}: {mode}{}",
                    parameter.name,
                    recipe_type(&parameter.ty, &names, None)?
                ))
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?
            .join(", ");
        let name = names
            .functions
            .get(function.id.as_str())
            .ok_or_else(|| package_error("owned-data semantic recipe function is unavailable"))?;
        output.push_str(&format!(
            "@id({})\nfn {name}({parameters}) -> {}\n",
            quote_json(function.id.as_str()),
            recipe_type(&function.return_type, &names, None)?,
        ));
        let mut local_index = 0_usize;
        output.push_str(&render_expr(
            &function.body,
            &names,
            &mut values,
            &mut local_index,
        )?);
        output.push_str("\n\n");
        ensure_bound(&output)?;
    }
    Ok(output)
}

/// Independently parse, resolve, and re-render one v8 recipe, then require its
/// stable declaration inventory and canonical semantic projection to equal
/// the exact linked HIR that produced it. Function and local display names and
/// source spans are intentionally not compared. Aggregate display names are
/// retained because the Project-v9 descriptor exposes them as source-consumer
/// facts.
pub(super) fn replay_against(
    linked: &crate::hir::ResolvedProgram,
    recipe: &str,
) -> Result<crate::hir::ResolvedProgram, Diagnostic> {
    if render(linked)? != recipe {
        return Err(package_error(
            "owned-data semantic recipe disagrees with linked HIR",
        ));
    }
    let replayed = replay(recipe)?;
    if linked.entrypoint != replayed.entrypoint
        || declaration_inventory(linked) != declaration_inventory(&replayed)
        || function_inventory(linked) != function_inventory(&replayed)
    {
        return Err(package_error(
            "owned-data semantic recipe replay changes the linked HIR inventory",
        ));
    }
    Ok(replayed)
}

pub(super) fn replay(recipe: &str) -> Result<crate::hir::ResolvedProgram, Diagnostic> {
    if recipe.is_empty() || recipe.len() > MAX_RECIPE_BYTES {
        return Err(package_error(
            "owned-data semantic recipe is outside its byte bound",
        ));
    }
    let ast = crate::parse(
        recipe,
        std::path::Path::new("semaprax-owned-data-recipe.spx"),
    )
    .map_err(|_| package_error("owned-data semantic recipe does not parse"))?;
    let replayed = crate::hir::resolve(&ast)
        .map_err(|_| package_error("owned-data semantic recipe does not resolve"))?;
    if render(&replayed)? != recipe {
        return Err(package_error("owned-data semantic recipe is not canonical"));
    }
    Ok(replayed)
}

fn declaration_inventory(
    program: &crate::hir::ResolvedProgram,
) -> Vec<(
    String,
    crate::hir::DeclarationKind,
    crate::hir::IdentityOrigin,
    Option<String>,
)> {
    let mut inventory = program
        .declarations
        .workspace_declarations()
        .into_iter()
        .map(|declaration| {
            (
                declaration.id.as_str().to_owned(),
                declaration.kind,
                declaration.identity_origin,
                declaration.owner.map(|owner| owner.as_str().to_owned()),
            )
        })
        .collect::<Vec<_>>();
    inventory.sort_by(|left, right| left.0.cmp(&right.0));
    inventory
}

fn function_inventory(program: &crate::hir::ResolvedProgram) -> Vec<String> {
    let mut inventory = program
        .functions
        .iter()
        .map(|function| function.id.as_str().to_owned())
        .collect::<Vec<_>>();
    inventory.sort();
    inventory
}

fn render_types(
    program: &crate::hir::ResolvedProgram,
    names: &Names,
    output: &mut String,
) -> Result<(), Diagnostic> {
    let mut authored = program
        .types
        .iter()
        .filter(|declaration| !crate::prelude::is_compiler_owned_id(declaration.id.as_str()))
        .collect::<Vec<_>>();
    authored.sort_by(|left, right| left.id.cmp(&right.id));
    for declaration in authored {
        render_type(declaration, names, output)?;
        ensure_bound(output)?;
    }
    Ok(())
}

fn render_type(
    declaration: &ResolvedTypeDeclaration,
    names: &Names,
    output: &mut String,
) -> Result<(), Diagnostic> {
    let type_name = names.type_name(&declaration.id)?;
    let parameters = if declaration.type_parameters.is_empty() {
        String::new()
    } else {
        format!(
            "<{}>",
            declaration
                .type_parameters
                .iter()
                .map(|parameter| format!("P{}", parameter.index))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    match &declaration.kind {
        ResolvedTypeDeclarationKind::Record { fields } => {
            output.push_str(&format!(
                "@id({})\nrecord {type_name}{parameters} {{\n",
                quote_json(declaration.id.as_str())
            ));
            for field in fields {
                output.push_str(&format!(
                    "    @id({})\n    {}: {},\n",
                    quote_json(field.id.as_str()),
                    names.field_name(&field.id)?,
                    recipe_type(&field.ty, names, Some(&declaration.id))?,
                ));
            }
            output.push_str("}\n\n");
        }
        ResolvedTypeDeclarationKind::Variant { cases } => {
            output.push_str(&format!(
                "@id({})\nvariant {type_name}{parameters} {{\n",
                quote_json(declaration.id.as_str())
            ));
            for case in cases {
                output.push_str(&format!(
                    "    @id({})\n    {}",
                    quote_json(case.id.as_str()),
                    names.case_name(&case.id)?,
                ));
                if case.fields.is_empty() {
                    output.push_str(",\n");
                } else {
                    output.push_str(" {\n");
                    for field in &case.fields {
                        output.push_str(&format!(
                            "        @id({})\n        {}: {},\n",
                            quote_json(field.id.as_str()),
                            names.field_name(&field.id)?,
                            recipe_type(&field.ty, names, Some(&declaration.id))?,
                        ));
                    }
                    output.push_str("    },\n");
                }
            }
            output.push_str("}\n\n");
        }
        ResolvedTypeDeclarationKind::Class { .. }
        | ResolvedTypeDeclarationKind::Resource { .. } => {
            return Err(package_error(
                "owned-data semantic recipe admits only authored records and variants",
            ));
        }
    }
    Ok(())
}

fn recipe_type(
    ty: &ResolvedType,
    names: &Names,
    parameter_owner: Option<&DeclarationId>,
) -> Result<String, Diagnostic> {
    match ty {
        ResolvedType::I64 => Ok("i64".to_owned()),
        ResolvedType::I32 => Ok("i32".to_owned()),
        ResolvedType::Char => Ok("char".to_owned()),
        ResolvedType::U8 => Ok("u8".to_owned()),
        ResolvedType::Usize => Ok("usize".to_owned()),
        ResolvedType::ArrayU8(length) => Ok(format!("[u8; {length}]")),
        ResolvedType::F32 => Ok("f32".to_owned()),
        ResolvedType::F64 => Ok("f64".to_owned()),
        ResolvedType::Bool => Ok("bool".to_owned()),
        ResolvedType::String => Ok("string".to_owned()),
        ResolvedType::Bytes => Ok("Bytes".to_owned()),
        ResolvedType::Str => Ok("str".to_owned()),
        ResolvedType::SliceU8 => Ok("Slice<u8>".to_owned()),
        ResolvedType::Nominal {
            declaration,
            arguments,
        } => {
            let name = match declaration.as_str() {
                crate::prelude::OPTION_ID => "Option".to_owned(),
                crate::prelude::RESULT_ID => "Result".to_owned(),
                _ => names.type_name(declaration)?.to_owned(),
            };
            if arguments.is_empty() {
                Ok(name)
            } else {
                Ok(format!(
                    "{name}<{}>",
                    arguments
                        .iter()
                        .map(|argument| recipe_type(argument, names, parameter_owner))
                        .collect::<Result<Vec<_>, _>>()?
                        .join(", ")
                ))
            }
        }
        ResolvedType::TypeParameter { owner, index }
            if parameter_owner.is_some_and(|expected| expected == owner) =>
        {
            Ok(format!("P{index}"))
        }
        ResolvedType::Unit | ResolvedType::TypeParameter { .. } => Err(package_error(
            "owned-data semantic recipe type is unsupported",
        )),
    }
}

fn render_expr(
    expression: &ResolvedExpr,
    names: &Names,
    values: &mut BTreeMap<String, String>,
    local_index: &mut usize,
) -> Result<String, Diagnostic> {
    match &expression.kind {
        ResolvedExprKind::Int(value) => Ok(value.to_string()),
        ResolvedExprKind::Int32(value) => Ok(format!("{value}i32")),
        ResolvedExprKind::Char(value) => render_char(*value),
        ResolvedExprKind::Uint8(value) => Ok(format!("{value}u8")),
        ResolvedExprKind::Usize(value) => Ok(format!("{value}usize")),
        ResolvedExprKind::ArrayU8(items) => Ok(format!(
            "[{}]",
            items
                .iter()
                .map(|value| format!("{value}u8"))
                .collect::<Vec<_>>()
                .join(", ")
        )),
        ResolvedExprKind::RepeatArrayU8 { value, count } => Ok(format!("[{value}u8; {count}]")),
        ResolvedExprKind::Float32(bits) => render_float(f64::from(f32::from_bits(*bits)), "f32"),
        ResolvedExprKind::Float64(bits) => render_float(f64::from_bits(*bits), "f64"),
        ResolvedExprKind::Bool(value) => Ok(value.to_string()),
        ResolvedExprKind::String(value) => Ok(crate::format::canonical_string(value)),
        ResolvedExprKind::Place(place) => render_place(place, names, values),
        ResolvedExprKind::BorrowPlace { operation, place } => {
            let operation = crate::byte_ops::by_id(operation.as_str()).ok_or_else(|| {
                package_error("owned-data semantic recipe borrow operation is unavailable")
            })?;
            Ok(format!(
                "{}({})",
                operation.name(),
                render_place(place, names, values)?
            ))
        }
        ResolvedExprKind::ByteRange {
            operation,
            source,
            start,
            end,
        } if operation.as_str() == crate::byte_ops::RANGE_ID => Ok(format!(
            "{}({}, {}, {})",
            crate::byte_ops::RANGE_NAME,
            render_expr(source, names, values, local_index)?,
            render_expr(start, names, values, local_index)?,
            render_expr(end, names, values, local_index)?,
        )),
        ResolvedExprKind::Call {
            callee,
            type_arguments,
            instance,
            args,
        } if type_arguments.is_empty() && instance.is_none() => {
            let callee = crate::string_ops::by_id(callee.as_str())
                .map(|operation| operation.name().to_owned())
                .or_else(|| crate::str_ops::by_id(callee.as_str()).map(|op| op.name().to_owned()))
                .or_else(|| crate::byte_ops::by_id(callee.as_str()).map(|op| op.name().to_owned()))
                .or_else(|| {
                    crate::host_io_ops::by_id(callee.as_str()).map(|op| op.name().to_owned())
                })
                .or_else(|| names.functions.get(callee.as_str()).cloned())
                .ok_or_else(|| package_error("owned-data semantic recipe callee is unavailable"))?;
            let args = args
                .iter()
                .map(|argument| render_expr(argument, names, values, local_index))
                .collect::<Result<Vec<_>, _>>()?
                .join(", ");
            Ok(format!("{callee}({args})"))
        }
        ResolvedExprKind::NativeRustImportCall(_) | ResolvedExprKind::HostCommandCall(_) => Err(
            package_error("owned-data semantic recipe contains an imported operation"),
        ),
        ResolvedExprKind::Unary { op, value } => Ok(format!(
            "({}{})",
            match op {
                crate::ast::UnaryOp::Neg => "-",
                crate::ast::UnaryOp::Not => "!",
            },
            render_expr(value, names, values, local_index)?
        )),
        ResolvedExprKind::Binary { op, left, right } => Ok(format!(
            "({} {} {})",
            render_expr(left, names, values, local_index)?,
            op.text(),
            render_expr(right, names, values, local_index)?,
        )),
        ResolvedExprKind::Block { statements, tail } => {
            render_block(statements, tail, names, values, local_index)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => Ok(format!(
            "if {} {} else {}",
            render_expr(condition, names, values, local_index)?,
            render_expr(then_branch, names, values, local_index)?,
            render_expr(else_branch, names, values, local_index)?,
        )),
        ResolvedExprKind::ConstructRecord { fields, .. } => Ok(format!(
            "{} {{ {} }}",
            recipe_type(&expression.ty, names, None)?,
            render_fields(fields, names, values, local_index)?
        )),
        ResolvedExprKind::ConstructVariant { case, fields, .. } => Ok(format!(
            "{}::{} {{ {} }}",
            recipe_type(&expression.ty, names, None)?,
            names.case_name(case)?,
            render_fields(fields, names, values, local_index)?
        )),
        ResolvedExprKind::Match {
            mode,
            scrutinee,
            arms,
        } => {
            let mode = match mode {
                crate::hir::ResolvedMatchMode::Value => "",
                crate::hir::ResolvedMatchMode::Own => "own ",
                crate::hir::ResolvedMatchMode::Borrow => "borrow ",
            };
            let mut rendered = format!(
                "match {mode}{} {{ ",
                render_expr(scrutinee, names, values, local_index)?
            );
            for arm in arms {
                let mut arm_values = values.clone();
                let pattern = render_pattern(&arm.pattern, names, &mut arm_values, local_index)?;
                let guard = arm.guard.as_ref().map_or_else(
                    || Ok(String::new()),
                    |guard| {
                        Ok(format!(
                            " if {}",
                            render_expr(guard, names, &mut arm_values, local_index)?
                        ))
                    },
                )?;
                rendered.push_str(&format!(
                    "{pattern}{guard} => {}, ",
                    render_expr(&arm.value, names, &mut arm_values, local_index)?
                ));
            }
            rendered.push('}');
            Ok(rendered)
        }
        ResolvedExprKind::Try { operand, .. } | ResolvedExprKind::TryOption { operand, .. } => Ok(
            format!("({})?", render_expr(operand, names, values, local_index)?),
        ),
        ResolvedExprKind::UpdateRecord { base, fields, .. } => Ok(format!(
            "{} with {{ {} }}",
            render_expr(base, names, values, local_index)?,
            render_fields(fields, names, values, local_index)?
        )),
        ResolvedExprKind::Project { base, field } => Ok(format!(
            "{}.{}",
            render_expr(base, names, values, local_index)?,
            names.field_name(field)?
        )),
        ResolvedExprKind::Upcast { .. }
        | ResolvedExprKind::ByteRange { .. }
        | ResolvedExprKind::Call { .. } => Err(package_error(
            "owned-data semantic recipe expression is unsupported",
        )),
    }
}

fn render_fields(
    fields: &[crate::hir::ResolvedFieldInitializer],
    names: &Names,
    values: &mut BTreeMap<String, String>,
    local_index: &mut usize,
) -> Result<String, Diagnostic> {
    fields
        .iter()
        .map(|field| {
            Ok(format!(
                "{}: {}",
                names.field_name(&field.field)?,
                render_expr(&field.value, names, values, local_index)?
            ))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()
        .map(|fields| fields.join(", "))
}

fn render_block(
    statements: &[ResolvedStatement],
    tail: &ResolvedExpr,
    names: &Names,
    values: &mut BTreeMap<String, String>,
    local_index: &mut usize,
) -> Result<String, Diagnostic> {
    let mut rendered = String::from("{ ");
    for statement in statements {
        match statement {
            ResolvedStatement::Let {
                binding,
                mutable,
                value,
                ..
            } => {
                let value = render_expr(value, names, values, local_index)?;
                let name = next_local(values, local_index);
                values.insert(binding.id.as_str().to_owned(), name.clone());
                rendered.push_str(&format!(
                    "let {}{name} = {value}; ",
                    if *mutable { "mut " } else { "" }
                ));
            }
            ResolvedStatement::Assign {
                binding,
                field,
                value,
                ..
            } => {
                let value = render_expr(value, names, values, local_index)?;
                let target = values.get(binding.id.as_str()).ok_or_else(|| {
                    package_error("owned-data semantic recipe assignment target is unavailable")
                })?;
                if let Some(field) = field {
                    rendered.push_str(&format!(
                        "{target}.{} = {value}; ",
                        names.field_name(field)?
                    ));
                } else {
                    rendered.push_str(&format!("{target} = {value}; "));
                }
            }
            ResolvedStatement::Unsafe { .. } => {
                return Err(package_error(
                    "owned-data semantic recipe unsafe boundary is unsupported",
                ));
            }
            ResolvedStatement::While {
                condition, body, ..
            } => rendered.push_str(&format!(
                "while {} {} ",
                render_expr(condition, names, values, local_index)?,
                render_expr(body, names, values, local_index)?
            )),
        }
    }
    rendered.push_str(&render_expr(tail, names, values, local_index)?);
    rendered.push_str(" }");
    Ok(rendered)
}

fn render_pattern(
    pattern: &ResolvedMatchPattern,
    names: &Names,
    values: &mut BTreeMap<String, String>,
    local_index: &mut usize,
) -> Result<String, Diagnostic> {
    match pattern {
        ResolvedMatchPattern::Variant {
            variant,
            case,
            fields,
        } => {
            let type_name = match variant.as_str() {
                crate::prelude::OPTION_ID => "Option",
                crate::prelude::RESULT_ID => "Result",
                _ => names.type_name(variant)?,
            };
            let fields = fields
                .iter()
                .map(|field| {
                    let name = next_local(values, local_index);
                    values.insert(field.binding.id.as_str().to_owned(), name.clone());
                    Ok(format!("{}: {name}", names.field_name(&field.field)?))
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?
                .join(", ");
            Ok(format!(
                "{type_name}::{} {{ {fields} }}",
                names.case_name(case)?
            ))
        }
        ResolvedMatchPattern::Record { record, fields, .. } => Ok(format!(
            "{} {{ {} }}",
            names.type_name(record)?,
            render_record_pattern_fields(fields, names, values, local_index)?
        )),
        ResolvedMatchPattern::Wildcard => Ok("_".to_owned()),
        ResolvedMatchPattern::Literal(value) => render_pattern_value(*value),
        ResolvedMatchPattern::Or(alternatives) => alternatives
            .iter()
            .map(|pattern| render_pattern(pattern, names, values, local_index))
            .collect::<Result<Vec<_>, _>>()
            .map(|items| items.join(" | ")),
        ResolvedMatchPattern::Binding(binding) => {
            let name = next_local(values, local_index);
            values.insert(binding.id.as_str().to_owned(), name.clone());
            Ok(name)
        }
    }
}

fn render_record_pattern_fields(
    fields: &[crate::hir::ResolvedRecordMatchPatternField],
    names: &Names,
    values: &mut BTreeMap<String, String>,
    local_index: &mut usize,
) -> Result<String, Diagnostic> {
    fields
        .iter()
        .map(|field| {
            let pattern = match &field.pattern {
                ResolvedRecordMatchFieldPattern::Binding(binding) => {
                    let name = next_local(values, local_index);
                    values.insert(binding.id.as_str().to_owned(), name.clone());
                    name
                }
                ResolvedRecordMatchFieldPattern::Wildcard => "_".to_owned(),
                ResolvedRecordMatchFieldPattern::Record { record, fields, .. } => format!(
                    "{} {{ {} }}",
                    names.type_name(record)?,
                    render_record_pattern_fields(fields, names, values, local_index)?
                ),
            };
            Ok(format!("{}: {pattern}", names.field_name(&field.field)?))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()
        .map(|fields| fields.join(", "))
}

fn render_place(
    place: &Place,
    names: &Names,
    values: &BTreeMap<String, String>,
) -> Result<String, Diagnostic> {
    let mut rendered = values
        .get(place.root.as_str())
        .cloned()
        .ok_or_else(|| package_error("owned-data semantic recipe place is unavailable"))?;
    for projection in &place.projections {
        let field = match projection {
            PlaceProjection::Field(field) | PlaceProjection::VariantField { field, .. } => field,
        };
        rendered.push('.');
        rendered.push_str(names.field_name(field)?);
    }
    Ok(rendered)
}

fn next_local(values: &BTreeMap<String, String>, local_index: &mut usize) -> String {
    loop {
        let candidate = format!("v{}", *local_index);
        *local_index += 1;
        if values.values().all(|value| value != &candidate) {
            return candidate;
        }
    }
}

fn render_pattern_value(value: crate::hir::PatternValue) -> Result<String, Diagnostic> {
    match value {
        crate::hir::PatternValue::Int(value) => Ok(value.to_string()),
        crate::hir::PatternValue::Int32(value) => Ok(format!("{value}i32")),
        crate::hir::PatternValue::Uint8(value) => Ok(format!("{value}u8")),
        crate::hir::PatternValue::Usize(value) => Ok(format!("{value}usize")),
        crate::hir::PatternValue::Char(value) => render_char(value),
        crate::hir::PatternValue::Bool(value) => Ok(value.to_string()),
    }
}

fn render_char(value: u32) -> Result<String, Diagnostic> {
    let value = char::from_u32(value)
        .ok_or_else(|| package_error("owned-data semantic recipe char is invalid"))?;
    Ok(match value {
        '\n' => "'\\n'".to_owned(),
        '\r' => "'\\r'".to_owned(),
        '\t' => "'\\t'".to_owned(),
        '\\' => "'\\\\'".to_owned(),
        '\'' => "'\\\''".to_owned(),
        value if !value.is_control() => format!("'{value}'"),
        _ => {
            return Err(package_error(
                "owned-data semantic recipe control char is unsupported",
            ))
        }
    })
}

fn render_float(value: f64, suffix: &str) -> Result<String, Diagnostic> {
    if !value.is_finite() {
        return Err(package_error(
            "owned-data semantic recipe non-finite float is unsupported",
        ));
    }
    let mut value = value.to_string();
    if !value
        .chars()
        .any(|character| matches!(character, '.' | 'e' | 'E'))
    {
        value.push_str(".0");
    }
    Ok(format!("{value}{suffix}"))
}

fn ensure_bound(output: &str) -> Result<(), Diagnostic> {
    if output.len() > MAX_RECIPE_BYTES {
        Err(package_error(
            "owned-data semantic recipe exceeds its byte limit",
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unrelated_aggregate_declarations_cannot_enter_the_exact_recipe() {
        let source = r#"
module recipe.aggregate;
@id("recipe.kept") record Kept { @id("recipe.kept.value") value: i64, }
@id("recipe.unrelated") record Unrelated { @id("recipe.unrelated.value") value: bool, }
@id("recipe.read") fn read(value: Kept) -> i64 { value.value }
@id("recipe.main") fn main() -> i64 { read(Kept { value: 7 }) }
"#;
        let ast = crate::parse(source, std::path::Path::new("recipe-aggregate.spx")).unwrap();
        let mut program = crate::hir::resolve(&ast).unwrap();
        program
            .types
            .retain(|declaration| declaration.id.as_str() != "recipe.unrelated");
        let recipe = render(&program).unwrap();
        assert!(recipe.contains("@id(\"recipe.kept\")"));
        assert!(recipe.contains("@id(\"recipe.kept.value\")"));
        assert!(!recipe.contains("recipe.unrelated"));
        let replayed = crate::hir::resolve(
            &crate::parse(&recipe, std::path::Path::new("recipe-replay.spx")).unwrap(),
        )
        .unwrap();
        assert_eq!(render(&replayed).unwrap(), recipe);
    }

    #[test]
    fn aggregate_member_identity_drift_changes_the_canonical_recipe() {
        let source = r#"
module recipe.variant;
@id("recipe.choice") variant Choice {
    @id("recipe.choice.ready") Ready { @id("recipe.choice.ready.value") value: i64, },
    @id("recipe.choice.empty") Empty,
}
@id("recipe.main") fn main() -> i64 {
    match Choice::Ready { value: 9 } {
        Choice::Ready { value } => value,
        Choice::Empty {} => 0,
    }
}
"#;
        let program = crate::hir::resolve(
            &crate::parse(source, std::path::Path::new("recipe-variant.spx")).unwrap(),
        )
        .unwrap();
        let recipe = render(&program).unwrap();
        let drifted = recipe.replace("recipe.choice.ready.value", "recipe.choice.ready.other");
        assert_ne!(drifted, recipe);
        let replayed = crate::hir::resolve(
            &crate::parse(&drifted, std::path::Path::new("recipe-drift.spx")).unwrap(),
        )
        .unwrap();
        assert_ne!(render(&replayed).unwrap(), recipe);
        assert!(replay_against(&program, &drifted).is_err());
    }

    #[test]
    fn replay_retains_authenticated_aggregate_display_names() {
        let source = r#"
module recipe.names;
@id("recipe.frame") record FrameInfo {
    @id("recipe.frame.payload") payload: Bytes,
    @id("recipe.frame.kind") kind: i64,
}
@id("recipe.main") fn main() -> i64 { 0 }
"#;
        let program = crate::hir::resolve(
            &crate::parse(source, std::path::Path::new("recipe-names.spx")).unwrap(),
        )
        .unwrap();
        let recipe = render(&program).unwrap();
        assert!(recipe.contains("record FrameInfo"));
        assert!(recipe.contains("payload: Bytes"));
        let replayed = replay_against(&program, &recipe).unwrap();
        let declaration = replayed
            .types
            .iter()
            .find(|declaration| declaration.id.as_str() == "recipe.frame")
            .unwrap();
        assert_eq!(declaration.name, "FrameInfo");
        let ResolvedTypeDeclarationKind::Record { fields } = &declaration.kind else {
            panic!("fixture must remain a record")
        };
        assert_eq!(fields[0].name, "payload");
        assert_eq!(fields[1].name, "kind");
    }
}
