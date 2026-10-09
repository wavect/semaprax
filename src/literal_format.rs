//! Closed, compile-time scanner for `string_format` literals.
//!
//! The retained HIR contains the decoded source literal, never these derived
//! pieces. Every consumer reparses it, so a cached recipe cannot become an
//! authority separate from the canonical source expression.

pub(crate) const NAME: &str = "string_format";
pub(crate) const ID: &str = "core.string.format.literal.v1";
pub(crate) const MAX_TEMPLATE_BYTES: usize = 65_536;
pub(crate) const MAX_FIELDS: usize = 32;

pub(crate) fn operation_id() -> &'static crate::hir::DeclarationId {
    static IDENTITY: std::sync::LazyLock<crate::hir::DeclarationId> =
        std::sync::LazyLock::new(|| crate::hir::DeclarationId::new(ID));
    &IDENTITY
}

pub(crate) fn resolved_params(args: &[crate::hir::ResolvedExpr]) -> Vec<crate::hir::ResolvedParam> {
    args.iter()
        .enumerate()
        .map(|(index, arg)| crate::hir::ResolvedParam {
            id: crate::hir::ValueId::intrinsic_parameter(ID, index),
            name: format!("value{index}"),
            ownership: if arg.ty == crate::hir::ResolvedType::String {
                crate::hir::OwnershipMode::Own
            } else {
                crate::hir::OwnershipMode::Value
            },
            ty: arg.ty.clone(),
            span: arg.span,
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScanError {
    TooLong,
    TooManyFields,
    InvalidBrace,
    Capacity,
}

impl ScanError {
    pub(crate) const fn message(self) -> &'static str {
        match self {
            Self::TooLong => "string_format literal exceeds 65536 decoded UTF-8 bytes",
            Self::TooManyFields => "string_format literal exceeds 32 fields",
            Self::InvalidBrace => "string_format literal permits only {}, {{, and }} braces",
            Self::Capacity => "string_format scanner exceeds its builder budget",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Piece {
    Literal(String),
    Field,
}

/// Parse the decoded AST literal. Braces are ASCII, so byte indexing never
/// splits a UTF-8 scalar. Adjacent text and escaped braces coalesce into one
/// literal piece, and field order is exactly source order.
pub(crate) fn scan(template: &str) -> Result<Vec<Piece>, ScanError> {
    if template.len() > MAX_TEMPLATE_BYTES {
        return Err(ScanError::TooLong);
    }
    let scratch_bound = template
        .len()
        .saturating_mul(2)
        .saturating_add((2 * MAX_FIELDS + 1) * std::mem::size_of::<Piece>());
    if !crate::bounded_output::reserve_active_required(scratch_bound) {
        return Err(ScanError::Capacity);
    }
    let mut pieces = Vec::new();
    let mut literal = String::new();
    let mut cursor = 0;
    let mut plain_start = 0;
    let bytes = template.as_bytes();
    let mut fields = 0;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'{' | b'}' => {
                literal.push_str(&template[plain_start..cursor]);
                let next = bytes.get(cursor + 1).copied();
                match (bytes[cursor], next) {
                    (b'{', Some(b'{')) => literal.push('{'),
                    (b'}', Some(b'}')) => literal.push('}'),
                    (b'{', Some(b'}')) => {
                        if fields == MAX_FIELDS {
                            return Err(ScanError::TooManyFields);
                        }
                        if !literal.is_empty() {
                            pieces.push(Piece::Literal(std::mem::take(&mut literal)));
                        }
                        pieces.push(Piece::Field);
                        fields += 1;
                    }
                    _ => return Err(ScanError::InvalidBrace),
                }
                cursor += 2;
                plain_start = cursor;
            }
            _ => cursor += 1,
        }
    }
    literal.push_str(&template[plain_start..]);
    if !literal.is_empty() || pieces.is_empty() {
        pieces.push(Piece::Literal(literal));
    }
    Ok(pieces)
}

pub(crate) fn field_count(pieces: &[Piece]) -> usize {
    pieces
        .iter()
        .filter(|piece| matches!(piece, Piece::Field))
        .count()
}

pub(crate) fn expression_uses(expression: &crate::hir::ResolvedExpr) -> bool {
    let mut pending = vec![expression];
    while let Some(next) = pending.pop() {
        if matches!(
            next.kind,
            crate::hir::ResolvedExprKind::LiteralFormat { .. }
        ) {
            return true;
        }
        crate::hir::push_resolved_expression_children_in_authored_order(next, &mut pending);
    }
    false
}

pub(crate) fn accepts_ast_type(ty: &crate::ast::Type) -> bool {
    matches!(
        ty,
        crate::ast::Type::I64
            | crate::ast::Type::U8
            | crate::ast::Type::Usize
            | crate::ast::Type::Bool
            | crate::ast::Type::String
    )
}

pub(crate) fn accepts_hir_type(ty: &crate::hir::ResolvedType) -> bool {
    matches!(
        ty,
        crate::hir::ResolvedType::I64
            | crate::hir::ResolvedType::U8
            | crate::hir::ResolvedType::Usize
            | crate::hir::ResolvedType::Bool
            | crate::hir::ResolvedType::String
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_scanner_preserves_utf8_nul_and_escape_order() {
        assert_eq!(
            scan("é\0{{{}}}").unwrap(),
            vec![
                Piece::Literal("é\0{".into()),
                Piece::Field,
                Piece::Literal("}".into())
            ]
        );
        assert_eq!(field_count(&scan(&"{}".repeat(32)).unwrap()), 32);
        assert_eq!(scan(&"{}".repeat(33)), Err(ScanError::TooManyFields));
        assert_eq!(scan(&"a".repeat(65_537)), Err(ScanError::TooLong));
        for invalid in ["{", "}", "{0}", "{name}", "{ }", "{{}", "{}}"] {
            assert_eq!(scan(invalid), Err(ScanError::InvalidBrace), "{invalid}");
        }
    }
}
