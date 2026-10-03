//! Bounded, acyclic Semaprax helper calls in the experimental owner renderer.
//! Every helper runs its own checked cleanup CFG. Tokens cross a call only at
//! the caller's existing CallCommit and leave a helper only at CommitResult.
use super::*;

pub(super) fn parameters(function: &ResolvedFunction) -> String {
    function
        .params
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            let ty = if parameter.ownership == OwnershipMode::Own {
                "spx_owner"
            } else {
                "int64_t"
            };
            format!(", {ty} arg_{index}")
        })
        .collect()
}

pub(super) fn render(
    program: &ResolvedProgram,
    root: &ResolvedFunction,
    constructor: &DeclarationId,
    method: &DeclarationId,
    lifecycle: &DeclarationId,
    resource: &ResolvedType,
    container: Option<&ContainerLayout>,
) -> Result<String, Diagnostic> {
    let mut closure = Closure {
        program,
        constructor,
        method,
        resource,
        container,
        visiting: BTreeSet::new(),
        retained: BTreeSet::new(),
    };
    closure.visit(root)?;
    let helpers = closure
        .retained
        .iter()
        .filter(|id| *id != &root.id)
        .map(|id| {
            program
                .functions
                .iter()
                .find(|function| &function.id == id)
                .ok_or_else(|| sdk_error("opaque owner helper is absent"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let symbols = helpers
        .iter()
        .enumerate()
        .map(|(ordinal, function)| (function.id.clone(), format!("spx_owner_helper_{ordinal}")))
        .collect::<BTreeMap<_, _>>();
    let root_source = render_function(
        root,
        constructor,
        method,
        lifecycle,
        resource,
        &program.functions,
        &symbols,
        false,
        container,
    )?;
    // Existing scalar-root programs without helpers retain their exact bytes.
    if helpers.is_empty() {
        return Ok(root_source);
    }
    let mut out = String::from(PRELUDE);
    for function in &helpers {
        let result = if &function.return_type == resource {
            "spx_owner"
        } else {
            "int64_t"
        };
        writeln!(
            out,
            "int32_t {}(uint64_t context{}, {result} *result);",
            symbols[&function.id],
            parameters(function)
        )
        .unwrap();
    }
    for (ordinal, function) in helpers.iter().enumerate() {
        let source = render_function(
            function,
            constructor,
            method,
            lifecycle,
            resource,
            &program.functions,
            &symbols,
            true,
            container,
        )?;
        let body = source
            .strip_prefix(PRELUDE)
            .ok_or_else(|| sdk_error("opaque owner helper prelude is invalid"))?;
        // Only compiler-owned C identifiers are renamed. Source spellings and
        // canonical cleanup vectors never pass through this namespacing step.
        let body = body
            .replace("spx_owner_entry", &symbols[&function.id])
            .replace("spx_eval_", &format!("spx_helper_{ordinal}_eval_"))
            .replace("spx_frame", &format!("spx_helper_{ordinal}_frame"));
        out.push_str(&body);
    }
    out.push_str(
        root_source
            .strip_prefix(PRELUDE)
            .ok_or_else(|| sdk_error("opaque owner root prelude is invalid"))?,
    );
    Ok(out)
}

struct Closure<'a> {
    program: &'a ResolvedProgram,
    constructor: &'a DeclarationId,
    method: &'a DeclarationId,
    resource: &'a ResolvedType,
    container: Option<&'a ContainerLayout>,
    visiting: BTreeSet<DeclarationId>,
    retained: BTreeSet<DeclarationId>,
}
impl<'a> Closure<'a> {
    fn visit(&mut self, function: &'a ResolvedFunction) -> Result<(), Diagnostic> {
        if self.retained.contains(&function.id) {
            return Ok(());
        }
        if self.retained.len() + self.visiting.len() >= 32 {
            return Err(sdk_error("opaque owner helper limit"));
        }
        if !self.visiting.insert(function.id.clone()) {
            return Err(sdk_error("opaque owner recursive helper is unsupported"));
        }
        let symbols = BTreeMap::new();
        let mut emitter = Emitter {
            function,
            expressions: Vec::new(),
            bindings: BTreeMap::new(),
            constructor: self.constructor,
            method: self.method,
            functions: &self.program.functions,
            symbols: &symbols,
            callees: BTreeSet::new(),
            helper: true,
            owned_result: &function.return_type == self.resource,
            container: self.container,
        };
        emitter.collect(&function.body)?;
        for callee in emitter.callees {
            let target = self
                .program
                .functions
                .iter()
                .find(|function| function.id == callee)
                .ok_or_else(|| sdk_error("opaque owner helper is absent"))?;
            self.visit(target)?;
        }
        self.visiting.remove(&function.id);
        self.retained.insert(function.id.clone());
        Ok(())
    }
}
