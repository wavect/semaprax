// Deterministic, bounded canonical input parsing, hashing, and rendering.
// This module has no filesystem, process, platform, settlement, or publication authority.
use super::*;

pub(super) fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

pub(super) fn raw_digest(bytes: &[u8]) -> String {
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(bytes))
    )
}

pub(super) fn identifier_gate(value: &str) -> Result<(), Diagnostic> {
    if value.is_empty() || value.len() > MAX_IDENTIFIER_BYTES || value.contains('\0') {
        Err(b109("max_identifier_bytes", MAX_IDENTIFIER_BYTES))
    } else {
        Ok(())
    }
}

pub(super) fn identifier_audit(program: &Program, spec: &Spec) -> Result<(), Diagnostic> {
    identifier_gate(&program.module)?;
    if spec.capabilities.len() > MAX_EFFECTS {
        return Err(b109("max_effects", MAX_EFFECTS));
    }
    for value in spec
        .exports
        .iter()
        .chain(&spec.imports)
        .chain(&spec.capabilities)
    {
        identifier_gate(value)?;
    }
    Ok(())
}

pub(super) fn full_hash(value: &str) -> String {
    format!(
        "{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(value.as_bytes()))
    )
}

pub(super) fn frame(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(bytes);
}

fn framed_digest<'a>(domain: &[u8], fields: impl IntoIterator<Item = &'a [u8]>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    for field in fields {
        frame(&mut hasher, field);
    }
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

fn sorted_unique(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

pub(super) fn json_depth(bytes: &[u8]) -> Result<usize, Diagnostic> {
    let mut depth = 0_usize;
    let mut maximum = 0_usize;
    let mut quoted = false;
    let mut escaped = false;
    for byte in bytes {
        if quoted {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                quoted = false;
            }
            continue;
        }
        match *byte {
            b'"' => quoted = true,
            b'{' | b'[' => {
                depth = depth.checked_add(1).ok_or_else(b106)?;
                maximum = maximum.max(depth);
            }
            b'}' | b']' => depth = depth.checked_sub(1).ok_or_else(b106)?,
            _ => {}
        }
    }
    if quoted || depth != 0 {
        return Err(b106());
    }
    Ok(maximum)
}

fn maximum_spec_strings() -> Result<usize, Diagnostic> {
    MAX_EXPORTS
        .checked_add(MAX_IMPORTS)
        .and_then(|count| count.checked_add(MAX_EFFECTS))
        .and_then(|count| count.checked_add(NONCLAIMS.len()))
        .and_then(|count| count.checked_add(64))
        .ok_or_else(b106)
}

pub(super) struct CountingSink {
    pub(super) bytes: usize,
    pub(super) maximum: usize,
    pub(super) overflowed: bool,
}

impl std::fmt::Write for CountingSink {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        let Some(bytes) = self.bytes.checked_add(value.len()) else {
            self.overflowed = true;
            return Ok(());
        };
        if bytes > self.maximum {
            self.overflowed = true;
        } else {
            self.bytes = bytes;
        }
        Ok(())
    }
}

pub(super) fn count_exact_artifact<F>(
    field: &'static str,
    maximum: usize,
    render: &mut F,
) -> Result<usize, Diagnostic>
where
    F: FnMut(&mut dyn std::fmt::Write) -> Result<(), Diagnostic>,
{
    let mut counter = CountingSink {
        bytes: 0,
        maximum,
        overflowed: false,
    };
    render(&mut counter)?;
    if counter.overflowed {
        return Err(b109(field, maximum));
    }
    Ok(counter.bytes)
}

pub(super) fn render_counted_artifact<F>(
    field: &'static str,
    maximum: usize,
    exact_bytes: usize,
    render: &mut F,
) -> Result<String, Diagnostic>
where
    F: FnMut(&mut dyn std::fmt::Write) -> Result<(), Diagnostic>,
{
    #[cfg(test)]
    EXACT_ARTIFACT_OUTPUT_ALLOCATION_COUNT.with(|count| count.set(count.get() + 1));
    let mut output = String::with_capacity(exact_bytes);
    let initial_capacity = output.capacity();
    if initial_capacity != exact_bytes {
        return Err(b109(field, maximum));
    }
    render(&mut output)?;
    if output.len() != exact_bytes || output.capacity() != initial_capacity {
        return Err(b109(field, maximum));
    }
    Ok(output)
}

pub(super) fn render_exact_artifact<F>(
    field: &'static str,
    maximum: usize,
    mut render: F,
) -> Result<String, Diagnostic>
where
    F: FnMut(&mut dyn std::fmt::Write) -> Result<(), Diagnostic>,
{
    let exact_bytes = count_exact_artifact(field, maximum, &mut render)?;
    render_counted_artifact(field, maximum, exact_bytes, &mut render)
}

pub(super) fn canonical_format_scratch_capacity(
    program: &Program,
) -> Result<crate::private_format::PrivateScratchCapacity, Diagnostic> {
    let mut expression_stack = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let expressions = program.functions.iter().flat_map(|function| {
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
    });
    let expression_depth = scan_ast_capacity(expressions, program, false, &mut expression_stack)?
        .max_depth
        .max(1);
    let mut type_depth = 1usize;
    for expression in program.functions.iter().flat_map(|function| {
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
    }) {
        type_depth = type_depth.max(ast_expression_type_depth(expression)?);
    }
    for function in &program.functions {
        type_depth = type_depth.max(ast_type_depth(&function.return_type)?);
        for parameter in &function.params {
            type_depth = type_depth.max(ast_type_depth(&parameter.ty)?);
        }
    }
    for interface in &program.interfaces {
        for import in &interface.imports {
            for parameter in &import.params {
                type_depth = type_depth.max(ast_type_depth(&parameter.ty)?);
            }
        }
    }
    for declaration in &program.types {
        match &declaration.kind {
            crate::ast::TypeDeclarationKind::Resource { .. } => {}
            crate::ast::TypeDeclarationKind::Record { fields }
            | crate::ast::TypeDeclarationKind::Class { fields, .. } => {
                for field in fields {
                    type_depth = type_depth.max(ast_type_depth(&field.ty)?);
                }
            }
            crate::ast::TypeDeclarationKind::Variant { cases } => {
                for case in cases {
                    for field in &case.fields {
                        type_depth = type_depth.max(ast_type_depth(&field.ty)?);
                    }
                }
            }
        }
    }
    let mut pattern_depth = 1usize;
    for function in &program.functions {
        let roots = function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures);
        for expression in roots {
            pattern_depth = pattern_depth.max(ast_pattern_depth(expression)?);
        }
    }
    crate::private_format::private_scratch_capacity(expression_depth, type_depth, pattern_depth)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
}

fn ast_expression_type_depth(root: &crate::ast::Expr) -> Result<usize, Diagnostic> {
    let mut expressions = [None; MAX_FORMAT_NESTING];
    expressions[0] = Some((root, 0usize));
    let mut len = 1usize;
    let mut maximum = 1usize;
    while len != 0 {
        len -= 1;
        let (expression, next) = expressions[len]
            .take()
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        if next == 0 {
            match &expression.kind {
                crate::ast::ExprKind::Call { type_arguments, .. } => {
                    for ty in type_arguments {
                        maximum = maximum.max(ast_type_depth(ty)?);
                    }
                }
                crate::ast::ExprKind::ConstructRecord { type_arguments, .. }
                | crate::ast::ExprKind::ConstructVariant { type_arguments, .. } => {
                    for ty in type_arguments {
                        maximum = maximum.max(ast_type_depth(ty)?);
                    }
                }
                _ => {}
            }
        }
        let mut child_cursor = next;
        if let Some((_, child)) = ast_child(expression, &mut child_cursor) {
            if len + 2 > expressions.len() {
                return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
            }
            expressions[len] = Some((expression, child_cursor));
            expressions[len + 1] = Some((child, 0));
            len += 2;
        }
    }
    Ok(maximum)
}

fn ast_type_depth(root: &crate::ast::Type) -> Result<usize, Diagnostic> {
    let mut stack: [Option<(&crate::ast::Type, usize, usize)>; MAX_FORMAT_NESTING] =
        [None; MAX_FORMAT_NESTING];
    stack[0] = Some((root, 1, 0));
    let mut len = 1usize;
    let mut maximum = 1usize;
    while len != 0 {
        len -= 1;
        let (ty, depth, next_child) = stack[len]
            .take()
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        maximum = maximum.max(depth);
        if let crate::ast::Type::Named { arguments, .. } = ty {
            if let Some(argument) = arguments.get(next_child) {
                if len + 2 > stack.len() {
                    return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
                }
                stack[len] = Some((ty, depth, next_child + 1));
                stack[len + 1] = Some((argument, depth + 1, 0));
                len += 2;
            }
        }
    }
    Ok(maximum)
}

fn ast_pattern_depth(root: &crate::ast::Expr) -> Result<usize, Diagnostic> {
    let mut expressions = [None; MAX_FORMAT_NESTING];
    expressions[0] = Some((root, 0usize));
    let mut expression_len = 1usize;
    let mut maximum = 1usize;
    while expression_len != 0 {
        expression_len -= 1;
        let (expression, next) = expressions[expression_len]
            .take()
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        if next == 0 {
            if let crate::ast::ExprKind::Match { arms, .. } = &expression.kind {
                for arm in arms {
                    maximum = maximum.max(match_pattern_depth(&arm.pattern)?);
                }
            }
        }
        let mut child_cursor = next;
        if let Some((_, child)) = ast_child(expression, &mut child_cursor) {
            if expression_len + 2 > expressions.len() {
                return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
            }
            expressions[expression_len] = Some((expression, child_cursor));
            expressions[expression_len + 1] = Some((child, 0));
            expression_len += 2;
        }
    }
    Ok(maximum)
}

fn match_pattern_depth(pattern: &crate::ast::MatchPattern) -> Result<usize, Diagnostic> {
    let crate::ast::MatchPattern::Record { fields, .. } = pattern else {
        return Ok(1);
    };
    let mut stack: [Option<(&[crate::ast::RecordMatchPatternField], usize, usize)>;
        MAX_FORMAT_NESTING] = [None; MAX_FORMAT_NESTING];
    stack[0] = Some((fields, 1, 0));
    let mut len = 1usize;
    let mut maximum = 1usize;
    while len != 0 {
        len -= 1;
        let (fields, depth, next_child) = stack[len]
            .take()
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        maximum = maximum.max(depth);
        if let Some(field) = fields.get(next_child) {
            if let crate::ast::RecordMatchFieldPattern::Record { fields: nested, .. } =
                &field.pattern
            {
                if len + 2 > stack.len() {
                    return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
                }
                stack[len] = Some((fields, depth, next_child + 1));
                stack[len + 1] = Some((nested, depth + 1, 0));
                len += 2;
            } else if next_child + 1 < fields.len() {
                stack[len] = Some((fields, depth, next_child + 1));
                len += 1;
            }
        }
    }
    Ok(maximum)
}

pub(super) fn canonical_source_bounded(program: &Program) -> Result<String, Diagnostic> {
    let scratch_bytes = canonical_format_scratch_capacity(program)?;
    let scratch_budget = reserve_temporary_exact(scratch_bytes.bytes())?;
    // Pass one establishes the exact final capacity while its frame scratch is
    // already authorized. Pass two holds the same scratch and exact String.
    let mut counter = CountingSink {
        bytes: 0,
        maximum: MAX_SOURCE_BYTES,
        overflowed: false,
    };
    note_canonical_format_pass();
    crate::private_format::write_canonical_with_scratch(program, &mut counter, scratch_bytes);
    if counter.overflowed {
        return Err(b109("max_source_bytes", MAX_SOURCE_BYTES));
    }
    let budget = reserve_temporary_exact(counter.bytes)?;
    let mut source = String::with_capacity(counter.bytes);
    note_canonical_format_pass();
    crate::private_format::write_canonical_with_scratch(program, &mut source, scratch_bytes);
    if source.len() != counter.bytes || source.capacity() != counter.bytes {
        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    budget.retain(source.capacity())?;
    drop(scratch_budget);
    Ok(source)
}

/// A single-pass parser for the exact canonical Spec shape.  It admits every
/// container, member, scalar, and array element before allocating the decoded
/// value, so hostile generic JSON never reaches a serde DOM.
struct SpecCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> SpecCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn expect(&mut self, expected: &[u8]) -> Result<(), Diagnostic> {
        let end = self.offset.checked_add(expected.len()).ok_or_else(b106)?;
        if self.bytes.get(self.offset..end) != Some(expected) {
            return Err(b106());
        }
        self.offset = end;
        Ok(())
    }

    fn string(&mut self) -> Result<String, Diagnostic> {
        let start = self.offset;
        self.expect(b"\"")?;
        let mut escaped = false;
        loop {
            let byte = *self.bytes.get(self.offset).ok_or_else(b106)?;
            self.offset = self.offset.checked_add(1).ok_or_else(b106)?;
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                break;
            } else if byte < 0x20 {
                return Err(b106());
            }
        }
        let value: String =
            serde_json::from_slice(&self.bytes[start..self.offset]).map_err(|_| b106())?;
        if value.contains('\0') {
            return Err(b106());
        }
        Ok(value)
    }

    fn string_array(
        &mut self,
        maximum: usize,
        field: &'static str,
    ) -> Result<Vec<String>, Diagnostic> {
        self.expect(b"[")?;
        let mut values = Vec::new();
        if self.bytes.get(self.offset) == Some(&b']') {
            self.offset += 1;
            return Ok(values);
        }
        loop {
            if values.len() == maximum {
                return Err(b109(field, maximum));
            }
            values.push(self.string()?);
            match self.bytes.get(self.offset) {
                Some(b',') => self.offset += 1,
                Some(b']') => {
                    self.offset += 1;
                    return Ok(values);
                }
                _ => return Err(b106()),
            }
        }
    }

    fn finish(self) -> Result<(), Diagnostic> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(b106())
        }
    }
}

pub(super) fn parse_spec(program: &Program, bytes: &[u8]) -> Result<Spec, Diagnostic> {
    let source = canonical_source_bounded(program)?;
    parse_spec_with_source(program, bytes, &source)
}

pub(super) fn canonical_sha256_text(value: &str) -> bool {
    value.len() == SHA256_TEXT_BYTES
        && value.starts_with("sha256:")
        && value["sha256:".len()..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn project_subject_string(
    object: &serde_json::Map<String, Value>,
    name: &str,
) -> Result<String, Diagnostic> {
    object
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && !value.contains('\0'))
        .map(str::to_owned)
        .ok_or_else(b106)
}

fn project_subject_usize(
    object: &serde_json::Map<String, Value>,
    name: &str,
) -> Result<usize, Diagnostic> {
    object
        .get(name)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(b106)
}

fn project_subject_owned_capacity(subject: &ProjectSubject) -> Result<usize, Diagnostic> {
    let strings = [
        &subject.name,
        &subject.manifest_digest,
        &subject.manifest_canonical,
        &subject.project_revision,
        &subject.workspace_revision,
        &subject.project_graph_digest,
        &subject.entry_module,
    ];
    let mut bytes = std::mem::size_of::<ProjectSubject>();
    for value in strings {
        bytes = bytes
            .checked_add(value.capacity())
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    bytes = bytes
        .checked_add(
            subject
                .sources
                .capacity()
                .checked_mul(std::mem::size_of::<ProjectSubjectSource>())
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .and_then(|bytes| {
            bytes.checked_add(
                subject
                    .exports
                    .capacity()
                    .checked_mul(std::mem::size_of::<ProjectSubjectExport>())?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    for source in &subject.sources {
        for value in [
            &source.path,
            &source.source_graph_schema,
            &source.source_revision,
            &source.source_digest,
        ] {
            bytes = bytes
                .checked_add(value.capacity())
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
    }
    for export in &subject.exports {
        for value in [&export.stable_id, &export.module, &export.path] {
            bytes = bytes
                .checked_add(value.capacity())
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
    }
    Ok(bytes)
}

pub(super) fn parse_project_subject(
    bytes: &[u8],
) -> Result<(ProjectSubject, TemporaryBudget), Diagnostic> {
    if bytes.len() > MAX_SPEC_BYTES {
        return Err(b109("max_spec_bytes", MAX_SPEC_BYTES));
    }
    if !bytes.ends_with(b"\n") || bytes.ends_with(b"\n\n") || bytes.contains(&b'\r') {
        return Err(b106());
    }
    if json_depth(bytes)? > MAX_JSON_DEPTH {
        return Err(b109("max_json_depth", MAX_JSON_DEPTH));
    }
    let parser_capacity = bytes
        .len()
        .checked_mul(6)
        .and_then(|value| value.checked_add(65_536))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let mut parser_budget = reserve_temporary_exact(parser_capacity)?;
    let value: Value = serde_json::from_slice(bytes).map_err(|_| b106())?;
    let object = value.as_object().ok_or_else(b106)?;
    if object.len() != 12
        || object.get("schema").and_then(Value::as_str) != Some(PROJECT_SUBJECT_SCHEMA)
        || object.get("project_schema").and_then(Value::as_str) != Some(PROJECT_SCHEMA)
        || object
            .get("imports")
            .and_then(Value::as_array)
            .map(Vec::len)
            != Some(0)
        || object
            .get("capabilities")
            .and_then(Value::as_array)
            .map(Vec::len)
            != Some(0)
    {
        return Err(b106());
    }
    let name = project_subject_string(object, "name")?;
    let project_revision = project_subject_string(object, "project_revision")?;
    let workspace_revision = project_subject_string(object, "workspace_revision")?;
    let entry_module = project_subject_string(object, "entry_module")?;
    let manifest = object
        .get("manifest")
        .and_then(Value::as_object)
        .filter(|manifest| manifest.len() == 3)
        .ok_or_else(b106)?;
    let manifest_bytes = project_subject_usize(manifest, "bytes")?;
    let manifest_digest = project_subject_string(manifest, "digest")?;
    let manifest_canonical = project_subject_string(manifest, "canonical")?;
    if manifest.len() != 3
        || manifest_canonical.len() != manifest_bytes
        || raw_digest(manifest_canonical.as_bytes()) != manifest_digest
    {
        return Err(b106());
    }
    let graph = object
        .get("project_graph")
        .and_then(Value::as_object)
        .filter(|graph| graph.len() == 2)
        .ok_or_else(b106)?;
    if graph.get("schema").and_then(Value::as_str) != Some(PROJECT_GRAPH_SCHEMA) {
        return Err(b106());
    }
    let project_graph_digest = project_subject_string(graph, "digest")?;
    if ![
        manifest_digest.as_str(),
        project_revision.as_str(),
        workspace_revision.as_str(),
        project_graph_digest.as_str(),
    ]
    .into_iter()
    .all(canonical_sha256_text)
    {
        return Err(b106());
    }
    let source_values = object
        .get("sources")
        .and_then(Value::as_array)
        .filter(|sources| (1..=16).contains(&sources.len()))
        .ok_or_else(b106)?;
    let mut sources = Vec::with_capacity(source_values.len());
    let mut total_source_bytes = 0usize;
    for value in source_values {
        let source = value
            .as_object()
            .filter(|source| source.len() == 5)
            .ok_or_else(b106)?;
        let source = ProjectSubjectSource {
            path: project_subject_string(source, "path")?,
            source_graph_schema: project_subject_string(source, "source_graph_schema")?,
            source_revision: project_subject_string(source, "source_revision")?,
            source_digest: project_subject_string(source, "source_digest")?,
            bytes: project_subject_usize(source, "bytes")?,
        };
        if source.path.len() > 4096
            || source.source_graph_schema.len() > MAX_IDENTIFIER_BYTES
            || !canonical_sha256_text(&source.source_revision)
            || !canonical_sha256_text(&source.source_digest)
            || source.bytes > MAX_SOURCE_BYTES
        {
            return Err(b106());
        }
        total_source_bytes = total_source_bytes
            .checked_add(source.bytes)
            .ok_or_else(|| b109("max_source_bytes", MAX_SOURCE_BYTES))?;
        sources.push(source);
    }
    if total_source_bytes > MAX_SOURCE_BYTES {
        return Err(b109("max_source_bytes", MAX_SOURCE_BYTES));
    }
    if !sources.windows(2).all(|pair| pair[0].path < pair[1].path) {
        return Err(b106());
    }
    let export_values = object
        .get("exports")
        .and_then(Value::as_array)
        .filter(|exports| !exports.is_empty() && exports.len() <= MAX_EXPORTS)
        .ok_or_else(b106)?;
    let mut exports = Vec::with_capacity(export_values.len());
    for value in export_values {
        let export = value
            .as_object()
            .filter(|export| export.len() == 3)
            .ok_or_else(b106)?;
        let export = ProjectSubjectExport {
            stable_id: project_subject_string(export, "stable_id")?,
            module: project_subject_string(export, "module")?,
            path: project_subject_string(export, "path")?,
        };
        identifier_gate(&export.stable_id)?;
        identifier_gate(&export.module)?;
        if export.path.len() > 4096 || !sources.iter().any(|source| source.path == export.path) {
            return Err(b106());
        }
        exports.push(export);
    }
    if !exports
        .windows(2)
        .all(|pair| pair[0].stable_id < pair[1].stable_id)
    {
        return Err(b106());
    }
    let subject = ProjectSubject {
        name,
        manifest_bytes,
        manifest_digest,
        manifest_canonical,
        project_revision,
        workspace_revision,
        project_graph_digest,
        entry_module,
        sources,
        exports,
    };
    let canonical = render_project_subject(&subject);
    if canonical.as_bytes() != bytes {
        return Err(b106());
    }
    let retained = project_subject_owned_capacity(&subject)?;
    drop((canonical, value));
    parser_budget.shrink_held(retained)?;
    Ok((subject, parser_budget))
}

pub(super) fn render_project_subject(subject: &ProjectSubject) -> String {
    let mut count = CountingSink {
        bytes: 0,
        maximum: MAX_SPEC_BYTES,
        overflowed: false,
    };
    if write_project_subject(subject, &mut count).is_err() || count.overflowed {
        return String::new();
    }
    let mut output = String::with_capacity(count.bytes);
    if write_project_subject(subject, &mut output).is_err() || output.capacity() != count.bytes {
        return String::new();
    }
    output
}

fn write_project_subject(
    subject: &ProjectSubject,
    output: &mut impl std::fmt::Write,
) -> std::fmt::Result {
    output.write_str("{\"schema\":")?;
    write_json_string(output, PROJECT_SUBJECT_SCHEMA)?;
    output.write_str(",\"project_schema\":")?;
    write_json_string(output, PROJECT_SCHEMA)?;
    output.write_str(",\"name\":")?;
    write_json_string(output, &subject.name)?;
    output.write_str(",\"manifest\":{\"bytes\":")?;
    write_usize_decimal(output, subject.manifest_bytes)?;
    output.write_str(",\"digest\":")?;
    write_json_string(output, &subject.manifest_digest)?;
    output.write_str(",\"canonical\":")?;
    write_json_string(output, &subject.manifest_canonical)?;
    output.write_str("},\"project_revision\":")?;
    write_json_string(output, &subject.project_revision)?;
    output.write_str(",\"workspace_revision\":")?;
    write_json_string(output, &subject.workspace_revision)?;
    output.write_str(",\"project_graph\":{\"schema\":")?;
    write_json_string(output, PROJECT_GRAPH_SCHEMA)?;
    output.write_str(",\"digest\":")?;
    write_json_string(output, &subject.project_graph_digest)?;
    output.write_str("},\"entry_module\":")?;
    write_json_string(output, &subject.entry_module)?;
    output.write_str(",\"sources\":[")?;
    for (index, source) in subject.sources.iter().enumerate() {
        if index != 0 {
            output.write_char(',')?;
        }
        output.write_str("{\"path\":")?;
        write_json_string(output, &source.path)?;
        output.write_str(",\"source_graph_schema\":")?;
        write_json_string(output, &source.source_graph_schema)?;
        output.write_str(",\"source_revision\":")?;
        write_json_string(output, &source.source_revision)?;
        output.write_str(",\"source_digest\":")?;
        write_json_string(output, &source.source_digest)?;
        output.write_str(",\"bytes\":")?;
        write_usize_decimal(output, source.bytes)?;
        output.write_char('}')?;
    }
    output.write_str("],\"exports\":[")?;
    for (index, export) in subject.exports.iter().enumerate() {
        if index != 0 {
            output.write_char(',')?;
        }
        output.write_str("{\"stable_id\":")?;
        write_json_string(output, &export.stable_id)?;
        output.write_str(",\"module\":")?;
        write_json_string(output, &export.module)?;
        output.write_str(",\"path\":")?;
        write_json_string(output, &export.path)?;
        output.write_char('}')?;
    }
    output.write_str("],\"imports\":[],\"capabilities\":[]}\n")
}

pub(super) fn parse_spec_with_source(
    program: &Program,
    bytes: &[u8],
    source: &str,
) -> Result<Spec, Diagnostic> {
    let (spec, authority) = parse_spec_with_source_authority(program, bytes, source)?;
    let retained = authority.maximum();
    authority.retain(retained)?;
    Ok(spec)
}

pub(super) fn parse_spec_with_source_authority(
    program: &Program,
    bytes: &[u8],
    source: &str,
) -> Result<(Spec, TemporaryBudget), Diagnostic> {
    if bytes.len() > MAX_SPEC_BYTES {
        return Err(b109("max_spec_bytes", MAX_SPEC_BYTES));
    }
    if json_depth(bytes)? > MAX_JSON_DEPTH {
        return Err(b109("max_json_depth", MAX_JSON_DEPTH));
    }
    let container_overhead = maximum_spec_strings()?
        .checked_mul(256)
        .and_then(|bytes| bytes.checked_add(65_536))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // The exact-shape cursor can own at most one decoded copy of every input
    // string plus the three bounded vectors.  Reserve that complete capacity
    // before decoding the first string.
    let spec_upper = bytes
        .len()
        .checked_add(container_overhead)
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Spec>()))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let spec_budget = reserve_temporary_exact(spec_upper)?;
    let mut cursor = SpecCursor::new(bytes);
    cursor.expect(b"{\"schema\":")?;
    if cursor.string()? != SPEC_SCHEMA {
        return Err(b106());
    }
    cursor.expect(b",\"module\":")?;
    let module = cursor.string()?;
    cursor.expect(b",\"source_revision\":")?;
    let source_revision = cursor.string()?;
    cursor.expect(b",\"target\":{\"triple\":")?;
    let triple = cursor.string()?;
    cursor.expect(b",\"pointer_width\":")?;
    let pointer_width = if cursor.bytes[cursor.offset..].starts_with(b"64") {
        cursor.offset += 2;
        64
    } else {
        return Err(b106());
    };
    cursor.expect(b",\"endian\":")?;
    let endian = cursor.string()?;
    cursor.expect(b",\"panic_strategy\":")?;
    let panic_strategy = cursor.string()?;
    cursor.expect(b",\"thread_policy\":")?;
    let thread_policy = cursor.string()?;
    cursor.expect(b"},\"exports\":")?;
    let exports = cursor.string_array(MAX_EXPORTS, "max_exports")?;
    cursor.expect(b",\"imports\":")?;
    let imports = cursor.string_array(MAX_IMPORTS, "max_imports")?;
    cursor.expect(b",\"capabilities\":")?;
    let capabilities = cursor.string_array(MAX_EFFECTS, "max_effects")?;
    cursor.expect(b",\"limits\":")?;
    cursor.expect(limits_json().as_bytes())?;
    cursor.expect(b",\"nonclaims\":[")?;
    for (index, expected) in NONCLAIMS.iter().enumerate() {
        if index != 0 {
            cursor.expect(b",")?;
        }
        if cursor.string()? != *expected {
            return Err(b106());
        }
    }
    cursor.expect(b"]}\n")?;
    cursor.finish()?;
    let target = Target {
        triple,
        pointer_width,
        endian,
        panic_strategy,
        thread_policy,
    };
    let spec = Spec {
        module,
        source_revision: Some(source_revision),
        target,
        exports,
        imports,
        capabilities,
    };
    if spec.exports.is_empty()
        || !sorted_unique(&spec.exports)
        || !sorted_unique(&spec.imports)
        || !sorted_unique(&spec.capabilities)
    {
        return Err(b106());
    }
    let canonical_budget = reserve_temporary_exact(MAX_SPEC_BYTES)?;
    let canonical = render_spec(&spec);
    canonical_budget.check(canonical.capacity())?;
    if canonical.as_bytes() != bytes {
        return Err(b106());
    }
    drop(canonical);
    drop(canonical_budget);
    let spec_owned = checked_spec_owned_capacity(&spec)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    for (actual, field, maximum) in [
        (spec.exports.len(), "max_exports", MAX_EXPORTS),
        (spec.imports.len(), "max_imports", MAX_IMPORTS),
        (spec.capabilities.len(), "max_effects", MAX_EFFECTS),
    ] {
        if actual > maximum {
            return Err(b109(field, maximum));
        }
    }
    identifier_gate(&spec.module)?;
    for value in spec
        .exports
        .iter()
        .chain(&spec.imports)
        .chain(&spec.capabilities)
    {
        identifier_gate(value)?;
    }
    if current_target().as_ref() != Some(&spec.target) {
        return Err(b107("target profile mismatch"));
    }
    if source.len() > MAX_SOURCE_BYTES {
        return Err(b109("max_source_bytes", MAX_SOURCE_BYTES));
    }
    if spec.module != program.module
        || spec.source_revision() != Some(domain_digest(SOURCE_DOMAIN, source.as_bytes()).as_str())
    {
        return Err(b107("selected identity missing"));
    }
    // Keep the complete decode reservation live through target construction,
    // source-digest materialization, and validation; only then transfer the
    // exact retained Spec capacity into the invocation-wide ledger.
    let mut spec_budget = spec_budget;
    spec_budget.shrink_held(spec_owned)?;
    Ok((spec, spec_budget))
}

pub(super) fn limits_json() -> String {
    format!(
        "{{\"max_exports\":{MAX_EXPORTS},\"max_imports\":{MAX_IMPORTS},\"max_parameters\":{MAX_PARAMETERS},\"max_closure_functions\":{MAX_CLOSURE_FUNCTIONS},\"max_status_domains\":{MAX_STATUS_DOMAINS},\"max_effects\":{MAX_EFFECTS},\"max_identifier_bytes\":{MAX_IDENTIFIER_BYTES},\"max_source_bytes\":{MAX_SOURCE_BYTES},\"max_spec_bytes\":{MAX_SPEC_BYTES},\"max_descriptor_bytes\":{MAX_DESCRIPTOR_BYTES},\"max_generated_c_bytes\":{MAX_GENERATED_C_BYTES},\"max_generated_header_bytes\":{MAX_GENERATED_HEADER_BYTES},\"max_generated_rust_bytes\":{MAX_GENERATED_RUST_BYTES},\"max_manifest_bytes\":{MAX_MANIFEST_BYTES},\"max_builder_bytes\":{MAX_BUILDER_BYTES},\"max_json_depth\":{MAX_JSON_DEPTH},\"max_semantic_expression_depth\":{MAX_SEMANTIC_EXPRESSION_DEPTH},\"max_call_depth\":{MAX_CALL_DEPTH},\"max_calls_per_bridge\":{MAX_CALLS_PER_BRIDGE},\"max_unexpected_inventory_entries\":0}}"
    )
}

pub(super) fn render_string_array(values: &[String]) -> String {
    let mut output = String::new();
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        write_json_string(&mut output, value).expect("writing JSON cannot fail");
    }
    output
}

pub(super) fn nonclaims_json() -> String {
    let mut output = String::new();
    for (index, value) in NONCLAIMS.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        write_json_string(&mut output, value).expect("writing JSON cannot fail");
    }
    output
}

pub(super) fn write_limits_json(output: &mut impl std::fmt::Write) -> std::fmt::Result {
    output.write_char('{')?;
    for (index, (name, value)) in LIMIT_ROWS.iter().enumerate() {
        if index != 0 {
            output.write_char(',')?;
        }
        write_json_string(output, name)?;
        output.write_char(':')?;
        write_usize_decimal(output, *value)?;
    }
    output.write_char('}')
}

pub(super) fn target_json(target: &Target) -> String {
    format!(
        "{{\"triple\":{},\"pointer_width\":{},\"endian\":{},\"panic_strategy\":{},\"thread_policy\":{}}}",
        quote_json(&target.triple),
        target.pointer_width,
        quote_json(&target.endian),
        quote_json(&target.panic_strategy),
        quote_json(&target.thread_policy)
    )
}

pub(super) fn write_json_string(
    output: &mut impl std::fmt::Write,
    value: &str,
) -> std::fmt::Result {
    output.write_char('"')?;
    for character in value.chars() {
        match character {
            '"' => output.write_str("\\\"")?,
            '\\' => output.write_str("\\\\")?,
            '\u{08}' => output.write_str("\\b")?,
            '\u{0c}' => output.write_str("\\f")?,
            '\n' => output.write_str("\\n")?,
            '\r' => output.write_str("\\r")?,
            '\t' => output.write_str("\\t")?,
            character if character <= '\u{1f}' => {
                write!(output, "\\u{:04x}", u32::from(character))?
            }
            character => output.write_char(character)?,
        }
    }
    output.write_char('"')
}

fn write_spec_string_array(
    output: &mut impl std::fmt::Write,
    values: &[String],
) -> std::fmt::Result {
    output.write_char('[')?;
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            output.write_char(',')?;
        }
        write_json_string(output, value)?;
    }
    output.write_char(']')
}

fn write_spec(spec: &Spec, output: &mut impl std::fmt::Write) -> std::fmt::Result {
    output.write_str("{\"schema\":")?;
    write_json_string(output, SPEC_SCHEMA)?;
    output.write_str(",\"module\":")?;
    write_json_string(output, &spec.module)?;
    output.write_str(",\"source_revision\":")?;
    write_json_string(
        output,
        spec.source_revision()
            .expect("canonical source Spec has a source revision"),
    )?;
    output.write_str(",\"target\":{\"triple\":")?;
    write_json_string(output, &spec.target.triple)?;
    write!(output, ",\"pointer_width\":{}", spec.target.pointer_width)?;
    output.write_str(",\"endian\":")?;
    write_json_string(output, &spec.target.endian)?;
    output.write_str(",\"panic_strategy\":")?;
    write_json_string(output, &spec.target.panic_strategy)?;
    output.write_str(",\"thread_policy\":")?;
    write_json_string(output, &spec.target.thread_policy)?;
    output.write_str("},\"exports\":")?;
    write_spec_string_array(output, &spec.exports)?;
    output.write_str(",\"imports\":")?;
    write_spec_string_array(output, &spec.imports)?;
    output.write_str(",\"capabilities\":")?;
    write_spec_string_array(output, &spec.capabilities)?;
    output.write_str(",\"limits\":")?;
    write_limits_json(output)?;
    output.write_str(",\"nonclaims\":[")?;
    for (index, value) in NONCLAIMS.iter().enumerate() {
        if index != 0 {
            output.write_char(',')?;
        }
        write_json_string(output, value)?;
    }
    output.write_str("]}\n")
}

pub(super) fn render_spec(spec: &Spec) -> String {
    let mut counter = CountingSink {
        bytes: 0,
        maximum: MAX_SPEC_BYTES,
        overflowed: false,
    };
    write_spec(spec, &mut counter).expect("counting Spec output cannot fail");
    if counter.overflowed {
        return String::new();
    }
    let mut output = String::with_capacity(counter.bytes);
    write_spec(spec, &mut output).expect("writing Spec output cannot fail");
    output
}
