//! Private trusted host for the standalone profile; no earlier runtime is reused or changed.

use crate::diagnostic::quote_json;

#[cfg(test)]
mod tests;

pub(super) fn render(
    descriptor: &str,
    wasm_sha256: &str,
    wasm_byte_length: usize,
    integer_conversions: bool,
) -> String {
    let runtime = crate::bounded_output::budgeted_format(format_args!(
        "// semaprax.wasm-internal-strings.runtime.v1\nconst DESCRIPTOR={descriptor};\nconst EXPECTED_SHA256={};\nconst EXPECTED_BYTES={wasm_byte_length};\n{}\n{}\n{}",
        quote_json(wasm_sha256),
        include_str!("runtime/input.js"),
        include_str!("runtime/arena.js"),
        include_str!("runtime/facade.js"),
    ));
    if !integer_conversions {
        return runtime;
    }
    const STATUS_GUARD: &str = "status<0||status>11||memory.buffer";
    const CAPACITY_CASE: &str =
        "else if(status===11)result=Object.freeze({kind:\"capacity\",cause});";
    assert_eq!(runtime.matches(STATUS_GUARD).count(), 1);
    assert_eq!(runtime.matches(CAPACITY_CASE).count(), 1);
    runtime
        .replacen(
            STATUS_GUARD,
            "status<0||(status>11&&status!==21)||memory.buffer",
            1,
        )
        .replacen(
            CAPACITY_CASE,
            "else if(status===11)result=Object.freeze({kind:\"capacity\",cause});\n      else if(status===21)result=Object.freeze({kind:\"failure\",domain:\"semaprax.convert.v1\",code:1});",
            1,
        )
}

/// Render the additive profile without changing any byte of the frozen host.
pub(super) fn render_toolkit(
    descriptor: &str,
    wasm_sha256: &str,
    wasm_byte_length: usize,
    program: &crate::hir::ResolvedProgram,
    closure: &std::collections::BTreeSet<crate::hir::DeclarationId>,
) -> String {
    let mut selected = program.clone();
    selected
        .functions
        .retain(|function| closure.contains(&function.id));
    selected.function_instances.clear();
    // Import selection is bound to the chosen executable closure. Unused
    // type declarations cannot add a host arena or widen its import inventory.
    selected.types.clear();
    let uses_byte_get = crate::wasm::aggregate::text_toolkit::uses_byte_get(&selected);
    let collections = crate::wasm::aggregate::map_collections::uses(&selected);
    let names = ["from_i64", "from_usize", "compare"]
        .into_iter()
        .chain(
            crate::wasm::aggregate::text_toolkit::selected(&selected)
                .into_iter()
                .map(crate::wasm::aggregate::text_toolkit::import_name),
        )
        .chain(
            collections
                .then_some(["spx_collection_checked_v2", "spx_collection_drop_v2"])
                .into_iter()
                .flatten(),
        )
        .chain(uses_byte_get.then_some("spx_bytes_get"))
        .map(quote_json)
        .collect::<Vec<_>>()
        .join(",");
    let mut input = include_str!("runtime/input.js").replace(
        "\"contains\",\"drop\"]);",
        &format!("\"contains\",\"drop\",{names}]);"),
    );
    input = input
        .replace(
            "const IMPORT_NAMES=Object.freeze(",
            "const ENV_IMPORT_NAMES=Object.freeze([\"spx_bytes_get\",\"spx_collection_checked_v2\",\"spx_collection_drop_v2\"]);\nconst IMPORT_NAMES=Object.freeze(",
        )
        .replace(
            "item.module!==\"semaprax.internal-strings.v1\"",
            "item.module!==(ENV_IMPORT_NAMES.includes(item.name)?\"env\":\"semaprax.internal-strings.v1\")",
        );
    let mut arena = include_str!("runtime/arena.js")
        .replace("function createArena(fail,isPoisoned){", "function createArena(fail,isPoisoned,options){")
        .replace("  const imports=Object.create(null);", "  const toolkit=createToolkitOperations({authenticate,mint,checkedMemory,fail,options});\n  Object.assign(operations,toolkit.operations);\n  const imports=Object.create(null);")
        .replace("cumulative=0;cause=null;active=true", "cumulative=0;cause=null;active=true;toolkit.begin()");
    let mut facade = include_str!("runtime/facade.js")
        .replace("instantiate(input)", "instantiate(input,options={})")
        .replace("createArena(fail,()=>poisoned)", "createArena(fail,()=>poisoned,options)")
        .replace("status<0||status>11", "!(status>=0&&status<=11||status>=21&&status<=25||status>=65&&status<=71)")
        .replace("domain:status<=8?\"semaprax.arithmetic.v1\":\"semaprax.contract.v1\",code:status<=8?status:status-8", "domain:status<=8?\"semaprax.arithmetic.v1\":status<=10?\"semaprax.contract.v1\":status<=22?\"semaprax.convert.v1\":status<=25?\"semaprax.text.v1\":\"semaprax.filesystem.v1\",code:status<=8?status:status<=10?status-8:status<=22?status-20:status<=25?status-22:status-64");
    if collections {
        arena = arena.replace("  const imports=Object.create(null);", "  function refuseCapacity(reason){requireActive();if(cause!==null)fail();cause=reason;return 11}\n  const collections=createCollectionOperations({authenticate,mint,checkedMemory,requireActive,refuseCapacity,fail,options});\n  Object.assign(operations,collections.operations);\n  const imports=Object.create(null);")
            .replace("active=true;toolkit.begin()", "active=true;toolkit.begin();collections.begin()")
            .replace("active=false;return cause", "collections.settle();active=false;return cause");
        facade = facade.replace("arena.imports}", "arena.imports,env:arena.imports}")
            .replace("status>=21&&status<=25", "status>=21&&status<=33")
            .replace("status<=25?\"semaprax.text.v1\":\"semaprax.filesystem.v1\"", "status<=25?\"semaprax.text.v1\":status<=29?\"semaprax.map.v1\":status<=33?\"semaprax.map.v2\":\"semaprax.filesystem.v1\"")
            .replace("status<=25?status-22:status-64", "status<=25?status-22:status<=29?status-25:status<=33?status-29:status-64");
    }
    facade = facade.replace(
        "Object.freeze({\"semaprax.internal-strings.v1\":arena.imports})",
        "Object.freeze({\"semaprax.internal-strings.v1\":arena.imports,env:arena.imports})",
    );
    let operations = if collections {
        format!(
            "{}\n{}",
            include_str!("runtime/toolkit.js"),
            include_str!("runtime/collections.js")
        )
    } else {
        include_str!("runtime/toolkit.js").to_owned()
    };
    crate::bounded_output::budgeted_format(format_args!(
        "// semaprax.wasm-text-toolkit.runtime.v1\nconst DESCRIPTOR={descriptor};\nconst EXPECTED_SHA256={};\nconst EXPECTED_BYTES={wasm_byte_length};\n{input}\n{operations}\n{arena}\n{facade}",
        quote_json(wasm_sha256)
    ))
}
