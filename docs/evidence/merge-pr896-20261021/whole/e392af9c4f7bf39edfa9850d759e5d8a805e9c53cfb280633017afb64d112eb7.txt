//! Complete overlay walk; only default and directory refusal custody changes.
#[macro_use]
#[path = "alias/owned_growth.rs"]
mod owned_growth;
#[cfg(test)]
#[path = "alias/before_owned_directory.rs"]
mod before_owned_directory;
use super::alias::owned_growth_tests;

use super::super::{
    alias_clone, budgeted_format, eligible, graph_error, import_stub, iterative, refusal,
    type_rewrite, AuthoredDeclaration, LayoutRegistry,
};
use super::typed_cost;
use crate::ast::program_source::generated_roots::GeneratedRoots;
use crate::ast::{Expr, ExprKind, Function, ModuleUseKind, Program, Type, TypeDeclaration};
use crate::workspace_graph::constructor_failure::{receiving::Receiver, typed::Failure};
use std::collections::BTreeMap;
use typed_cost::{pay_default_teardown, pay_signature, pay_teardown_minimum, pay_type};

pub(in crate::workspace_graph) fn construct_transient<'p>(
    program: &'p Program,
    authored: &'p BTreeMap<&'p str, AuthoredDeclaration<'p>>,
    programs: &'p [Program],
    registry: &LayoutRegistry<'_>,
    core: Option<&mut import_stub::core_proofs::CoreProviderProofs<'p>>,
    owner: &mut Receiver,
) -> Result<crate::ast::program_source::function_alias::TransientFunctions<'p>, Failure> {
    use crate::ast::program_source::function_alias::{
        FunctionDirectory, FunctionEntry, FunctionView, TransientFunctions,
    };
    owner.entry::<TransientFunctions<'p>>()?;
    owner.reserve(
        std::mem::size_of::<FunctionDirectory<'p>>()
            + std::mem::size_of::<Vec<FunctionEntry<'p>>>(),
    )?;
    owner.reserve(0)?;
    if !eligible(program, authored, programs) {
        return Err(Failure::one(graph_error(
            "SPX-G173",
            "imported borrowed projection requires the owned route",
        )));
    }
    // Direct callers receive the same preselection as the actual core route:
    // invariant transforms get the complete owning imported-function producer.
    owner.reserve(
        std::mem::size_of::<bool>() + std::mem::size_of::<std::slice::Iter<'_, Program>>(),
    )?;
    owner.reserve(
        std::mem::size_of::<std::slice::Iter<'_, TypeDeclaration>>()
            + std::mem::size_of::<Option<&TypeDeclaration>>()
            + std::mem::size_of::<&[Expr]>()
            + std::mem::size_of::<bool>(),
    )?;
    let mut owning_invariants = program
        .types
        .iter()
        .any(|declaration| !declaration.invariants().is_empty());
    for candidate in programs {
        owner.reserve(
            std::mem::size_of::<std::slice::Iter<'_, TypeDeclaration>>()
                + std::mem::size_of::<Option<&TypeDeclaration>>()
                + std::mem::size_of::<&[Expr]>()
                + std::mem::size_of::<bool>(),
        )?;
        if candidate
            .types
            .iter()
            .any(|declaration| !declaration.invariants().is_empty())
        {
            owning_invariants = true;
            break;
        }
    }
    let layout = registry.get(program, authored, programs)?;
    let explicit = program
        .module_uses
        .iter()
        .filter(|import| import.kind == ModuleUseKind::Type)
        .count();
    let type_count = explicit
        .checked_add(layout.entries.len())
        .ok_or_else(refusal)?;
    let recursive = iterative(program, authored);
    if recursive {
        pay_teardown_minimum(owner)?;
    }
    owner.reserve(std::mem::size_of::<GeneratedRoots>())?;
    let roots = if recursive {
        GeneratedRoots::iterative(directory::<TypeDeclaration>(type_count, owner)?, Vec::new())
    } else {
        GeneratedRoots::plain(directory::<TypeDeclaration>(type_count, owner)?, Vec::new())
    };
    // A single original roots owner covers both types and owned functions
    // before any recursive payload is initialized, including refusal paths.
    owner.reserve(std::mem::size_of::<FunctionDirectory<'p>>())?;
    let mut directory_owner = FunctionDirectory {
        roots,
        entries: Vec::new(),
    };
    let types = &mut directory_owner.roots.types;
    for import in program
        .module_uses
        .iter()
        .filter(|import| import.kind == ModuleUseKind::Type)
    {
        let target = &authored[import.persistent_id.as_str()];
        let declaration = target.ty.expect("validated type target");
        pay_type(declaration, owner)?;
        if recursive {
            typed_cost::teardown_type(declaration, owner)?;
        }
        types.push(alias_clone::type_payload(declaration));
        let copy = types.last_mut().expect("inserted type");
        copy.name = owner.text(&import.alias)?;
        owner.reserve(0)?;
        super::super::super::super::generic_type_import::receiving::rewrite_declaration_with_layout(
            copy,
            target.module,
            program,
            programs,
            Some(&layout),
            owner,
        )?;
    }
    // Preserve the independent closure's exact order after explicit aliases.
    for entry in &layout.entries {
        pay_type(entry.declaration, owner)?;
        if recursive {
            typed_cost::teardown_type(entry.declaration, owner)?;
        }
        types.push(alias_clone::type_payload(entry.declaration));
        let copy = types.last_mut().expect("inserted private type");
        copy.name = owner.text(&entry.alias)?;
        owner.reserve(0)?;
        super::super::super::super::generic_type_import::receiving::rewrite_declaration_with_layout(
            copy,
            entry.module,
            program,
            programs,
            Some(&layout),
            owner,
        )?;
    }
    let all_count = program
        .types
        .len()
        .checked_add(types.len())
        .ok_or_else(refusal)?;
    let mut type_index = directory::<(&str, &TypeDeclaration)>(all_count, owner)?;
    type_index.extend(
        program
            .types
            .iter()
            .chain(types.iter())
            .map(|declaration| (declaration.name.as_str(), declaration)),
    );
    type_index.sort_unstable_by(|a, b| a.0.cmp(b.0));
    if type_index.windows(2).any(|items| items[0].0 == items[1].0) {
        return Err(Failure::one(graph_error(
            "SPX-G173",
            "synthetic workspace type-name index is not unique",
        )));
    }
    let needs_main = !program
        .functions
        .iter()
        .any(|function| function.name == "main")
        && !program
            .module_uses
            .iter()
            .any(|import| import.kind == ModuleUseKind::Function && import.alias == "main");
    let imports = program
        .module_uses
        .iter()
        .filter(|import| import.kind == ModuleUseKind::Function)
        .count();
    // GeneratedRoots already holds the empty owned directory. Borrowed entries
    // acquire no Function backing; owning fallback acquires the original full bound at its real frontier.
    directory_owner.entries = directory::<FunctionEntry<'p>>(
        imports
            .checked_add(usize::from(needs_main))
            .ok_or_else(refusal)?,
        owner,
    )?;
    owner.reserve(
        std::mem::size_of::<&mut Vec<Function>>()
            + std::mem::size_of::<&mut Vec<FunctionEntry<'p>>>(),
    )?;
    let functions = &mut directory_owner.roots.functions;
    let entries = &mut directory_owner.entries;
    let mut byte_proofs =
        import_stub::core_proofs::Phase::new(authored, programs, core).map_err(Failure::one)?;
    for import in program
        .module_uses
        .iter()
        .filter(|import| import.kind == ModuleUseKind::Function)
    {
        let target = &authored[import.persistent_id.as_str()];
        let original = target.function.expect("validated function target");
        let retain_body = programs
            .iter()
            .find(|provider| provider.module == target.module)
            .is_some_and(|provider| {
                crate::vec_ops::source_wrapper(provider, original).is_some()
                    || crate::box_ops::source_wrapper(provider, original).is_some()
            });
        owner.reserve(
            2 * std::mem::size_of::<bool>()
                + 3 * std::mem::size_of::<bool>()
                + 2 * std::mem::size_of::<&Vec<crate::ast::SessionProtocolDeclaration>>()
                + 2 * std::mem::size_of::<&Vec<crate::ast::AgentDeclaration>>()
                + 2 * std::mem::size_of::<&Vec<crate::ast::ProtocolDeclaration>>(),
        )?;
        // Raw byte-helper proof memo/cursors have ordinary payload subjects.
        // A Bytes-returning alias must retain the complete owning producer.
        if retain_body && !owning_invariants && original.return_type != Type::Bytes {
            // The entire real view, its Some/receiving carriers and forwarded
            // arguments are paid before evaluating the checked constructor.
            owner.reserve(
                2 * std::mem::size_of::<Option<FunctionView<'p>>>()
                    + 2 * std::mem::size_of::<FunctionView<'p>>()
                    + 2 * std::mem::size_of::<FunctionEntry<'p>>()
                    + 2 * std::mem::size_of::<&Program>()
                    + std::mem::size_of::<&Function>()
                    + std::mem::size_of::<&str>()
                    + std::mem::size_of::<std::slice::Iter<'_, Program>>()
                    + programs.len()
                        * (std::mem::size_of::<Option<&Program>>()
                            + 2 * std::mem::size_of::<&Program>()
                            + std::mem::size_of::<bool>()),
            )?;
            let provider = programs
                .iter()
                .find(|provider| provider.module == target.module)
                .expect("retained classifier found actual provider");
            owner.reserve(
                std::mem::size_of::<std::slice::Iter<'_, Function>>()
                    + 2 * std::mem::size_of::<bool>()
                    + provider.functions.len()
                        * (std::mem::size_of::<Option<&Function>>()
                            + std::mem::size_of::<&Function>()
                            + 2 * std::mem::size_of::<*const Function>()
                            + std::mem::size_of::<bool>()),
            )?;
            if let Some(view) = FunctionView::checked(provider, original, &import.alias) {
                entries.push(FunctionEntry::Borrowed(view));
                #[cfg(test)]
                crate::workspace_graph::constructor_failure::typed::before_function_alias_core::borrowed_entry();
                continue;
            }
        }
        // Ordinary scalar signatures keep provider-owned metadata and the exact
        // finite default. Every argument/formal/output, check, iterator and
        // reached bounded scan role is admitted before constructing the view.
        if !retain_body
            && !owning_invariants
            && program.session_protocols.is_empty()
            && program.agents.is_empty()
            && program.protocols.is_empty()
        {
            use crate::ast::program_source::function_alias::scalar_signature::ScalarSignature;
            use std::mem::size_of;
            owner.reserve(
                4 * size_of::<&Program>()
                    + 4 * size_of::<&Function>()
                    + 4 * size_of::<&str>()
                    + 2 * size_of::<&String>()
                    + size_of::<ScalarSignature<'p>>()
                    // The former outer Signature binding is consumed as its
                    // exact view field; same lifetime and publication boundary.
                    + size_of::<FunctionView<'p>>()
                    + 2 * size_of::<Option<ScalarSignature<'p>>>()
                    + size_of::<FunctionView<'p>>()
                    + size_of::<FunctionEntry<'p>>()
                    + size_of::<std::slice::Iter<'_, Program>>()
                    + size_of::<std::slice::Iter<'_, crate::ast::Param>>()
                    + size_of::<&Vec<crate::ast::TypeParameterDeclaration>>()
                    + size_of::<&Vec<crate::ast::Param>>()
                    + 10 * size_of::<bool>()
                    + 2 * size_of::<&Option<crate::ast::YieldsClause>>()
                    + 2 * size_of::<&Option<Box<crate::ast::SessionProtocolFollowsClause>>>()
                    + original.params.len()
                        * (size_of::<Option<&crate::ast::Param>>()
                            + size_of::<&crate::ast::Param>()
                            + 2 * size_of::<bool>())
                    + size_of::<Option<&crate::ast::Param>>(),
            )?;
            let mut providers = programs.iter();
            let provider = loop {
                owner.reserve(
                    2 * size_of::<&mut std::slice::Iter<'_, Program>>()
                        + 2 * size_of::<Option<&Program>>()
                        + 2 * size_of::<&Program>()
                        + 2 * size_of::<&String>()
                        + 2 * size_of::<&str>()
                        + size_of::<bool>(),
                )?;
                match providers.next() {
                    Some(candidate) if candidate.module == target.module => break candidate,
                    Some(_) => {}
                    None => unreachable!("validated function target has its provider"),
                }
            };
            owner.reserve(
                size_of::<std::slice::Iter<'_, Function>>()
                    + 2 * size_of::<&Vec<Function>>()
                    + provider.functions.len()
                        * (size_of::<Option<&Function>>()
                            + 2 * size_of::<&Function>()
                            + 2 * size_of::<*const Function>()
                            + 2 * size_of::<bool>())
                    + size_of::<Option<&Function>>(),
            )?;
            if let Some(ScalarSignature { view }) =
                ScalarSignature::checked_before_string(provider, original, &import.alias)
            {
                // Keep original proof-current checks and their ordering even
                // though primitive defaults cannot consume either permission.
                let _owned =
                    super::super::super::super::owned_function_import::admitted_with_layout(
                        program, target, authored, programs, &layout,
                    );
                let _bytes = byte_proofs
                    .get()
                    .map_err(Failure::one)?
                    .authorized(original, target.module)
                    .map_err(Failure::one)?;
                entries.push(FunctionEntry::Borrowed(view));
                continue;
            }
        }
        if retain_body {
            typed_cost::teardown_function(original, true, owner)?;
        } else {
            pay_signature(original, owner)?;
            if recursive {
                typed_cost::teardown_function(original, false, owner)?;
            }
        }
        owned_function_directory!(functions, entries, owner);
        let function = if retain_body {
            alias_clone::retained_function(original, &import.alias)
        } else {
            alias_clone::signature_function(original, &import.alias)
        };
        owner.reserve(std::mem::size_of::<FunctionEntry<'p>>() + std::mem::size_of::<usize>())?;
        entries.push(FunctionEntry::Owned(functions.len()));
        functions.push(function);
        #[cfg(test)]
        owned_growth_tests::observe_owned_push();
        owner.reserve(0)?;
        if retain_body {
            continue;
        }
        let function = functions.last_mut().expect("inserted signature");
        for param in &mut function.params {
            type_rewrite::receiving::rewrite_with_layout(
                &mut param.ty,
                target.module,
                program,
                programs,
                Some(&layout),
                owner,
            )?;
        }
        type_rewrite::receiving::rewrite_with_layout(
            &mut function.return_type,
            target.module,
            program,
            programs,
            Some(&layout),
            owner,
        )?;
        if recursive {
            pay_default_teardown(&function.return_type, &type_index, owner)?;
        }
        function.body = import_stub::typed::default_expr(
            function,
            &type_index,
            super::super::super::super::owned_function_import::admitted_with_layout(
                program, target, authored, programs, &layout,
            ),
            byte_proofs
                .get()
                .map_err(Failure::one)?
                .authorized(original, target.module)
                .map_err(Failure::one)?,
            owner,
        )?;
    }
    // Same old rule also recognizes an imported alias named main.
    if needs_main && !functions.iter().any(|function| function.name == "main") {
        owned_function_directory!(functions, entries, owner);
        owner.reserve(std::mem::size_of::<Expr>())?;
        let stable_id =
            budgeted_format(format_args!("workspace.synthetic.main.{}", program.module));
        owner.reserve(0)?;
        let name = owner.text("main")?;
        owner.reserve(std::mem::size_of::<FunctionEntry<'p>>() + std::mem::size_of::<usize>())?;
        entries.push(FunctionEntry::Owned(functions.len()));
        functions.push(Function {
            stable_id,
            explicit_id: true,
            name,
            name_span: crate::ast::Span::default(),
            type_parameters: Vec::new(),
            params: Vec::new(),
            return_type: Type::I64,
            effects: Vec::new(),
            yields: None,
            follows: None,
            requires: Vec::new(),
            ensures: Vec::new(),
            body: Expr {
                kind: ExprKind::Int(0),
                span: crate::ast::Span::default(),
            },
            span: crate::ast::Span::default(),
        });
        owner.reserve(0)?;
    }
    drop(type_index);
    // Infallible bounded string helpers record refusal and return empty text.
    // A generated view must never publish that partial construction as success.
    owner.reserve(0)?;
    program
        .functions
        .len()
        .checked_add(entries.len())
        .ok_or_else(refusal)?;
    program
        .types
        .len()
        .checked_add(directory_owner.roots.types.len())
        .ok_or_else(refusal)?;
    owner.reserve(
        2 * std::mem::size_of::<TransientFunctions<'p>>()
            + std::mem::size_of::<FunctionDirectory<'p>>()
            + std::mem::size_of::<&Program>()
            + std::mem::size_of::<crate::ast::program_source::OverlayIdentity>(),
    )?;
    let result = TransientFunctions::new(program, directory_owner);
    byte_proofs.complete().map_err(Failure::one)?;
    Ok(result)
}

/// Same exact-count directory as the independent old helper. The initialized
/// return, Vec header, layout and actual backing are paid by the original
/// receiver before allocation; a failed reservation propagates only Copy Setup.
fn directory<T>(count: usize, owner: &mut Receiver) -> Result<Vec<T>, Failure> {
    use std::mem::size_of;
    owner.entry::<Vec<T>>()?;
    owner.reserve(
        size_of::<Vec<T>>()
            + size_of::<usize>()
            + size_of::<std::alloc::Layout>()
            + size_of::<Result<std::alloc::Layout, std::alloc::LayoutError>>(),
    )?;
    let layout = match std::alloc::Layout::array::<T>(count) {
        Ok(layout) => layout,
        Err(_) => return Err(owner.physical().into()),
    };
    owner.reserve(layout.size())?;
    let values = Vec::with_capacity(count);
    if values.capacity() != count {
        return Err(owner.physical().into());
    }
    Ok(values)
}

#[cfg(test)]
#[path = "alias/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "alias/rename_tests.rs"]
mod rename_tests;
