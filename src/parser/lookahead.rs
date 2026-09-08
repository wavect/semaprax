//! Bounded explicit-generic lookahead shared by expression parsing.
use super::*;
impl Parser {
    pub(super) fn looks_like_generic_variant_qualifier(&self) -> bool {
        self.looks_like_generic_qualifier(TokenKind::ColonColon)
    }

    pub(super) fn looks_like_generic_record_qualifier(&self) -> bool {
        self.looks_like_generic_qualifier(TokenKind::LBrace)
    }

    pub(super) fn looks_like_generic_function_call(&self) -> bool {
        self.looks_like_generic_qualifier(TokenKind::LParen)
    }

    fn looks_like_generic_qualifier(&self, terminator: TokenKind) -> bool {
        if self.tokens.get(self.cursor).map(|token| &token.kind) != Some(&TokenKind::Lt) {
            return false;
        }
        let malformed_qualifier = self.looks_like_malformed_generic_qualifier(&terminator);
        let mut cursor = self.cursor + 1;
        if self.tokens.get(cursor).map(|token| &token.kind) == Some(&TokenKind::Gt) {
            return malformed_qualifier;
        }
        loop {
            let Some(next) = self.generic_type_end(cursor) else {
                return malformed_qualifier;
            };
            cursor = next;
            match self.tokens.get(cursor).map(|token| &token.kind) {
                Some(TokenKind::Comma) => cursor += 1,
                Some(TokenKind::Gt) => {
                    return self
                        .tokens
                        .get(cursor + 1)
                        .is_some_and(|next| next.kind == terminator);
                }
                _ => return malformed_qualifier,
            }
        }
    }

    fn looks_like_malformed_generic_qualifier(&self, terminator: &TokenKind) -> bool {
        let mut depth = 0_usize;
        for (offset, token) in self.tokens[self.cursor..].iter().enumerate() {
            match token.kind {
                TokenKind::Lt => depth += 1,
                TokenKind::Gt => {
                    let Some(next_depth) = depth.checked_sub(1) else {
                        return false;
                    };
                    depth = next_depth;
                    if depth == 0 {
                        return self
                            .tokens
                            .get(self.cursor + offset + 1)
                            .is_some_and(|next| &next.kind == terminator);
                    }
                }
                TokenKind::Ident(_) | TokenKind::Dot | TokenKind::Comma => {}
                _ => return false,
            }
        }
        false
    }

    fn generic_type_end(&self, mut cursor: usize) -> Option<usize> {
        if !matches!(self.tokens.get(cursor)?.kind, TokenKind::Ident(_)) {
            return None;
        }
        cursor += 1;
        while self.tokens.get(cursor).map(|token| &token.kind) == Some(&TokenKind::Dot) {
            cursor += 1;
            if !matches!(self.tokens.get(cursor)?.kind, TokenKind::Ident(_)) {
                return None;
            }
            cursor += 1;
        }
        if self.tokens.get(cursor).map(|token| &token.kind) != Some(&TokenKind::Lt) {
            return Some(cursor);
        }
        cursor += 1;
        if self.tokens.get(cursor).map(|token| &token.kind) == Some(&TokenKind::Gt) {
            return None;
        }
        loop {
            cursor = self.generic_type_end(cursor)?;
            match self.tokens.get(cursor).map(|token| &token.kind) {
                Some(TokenKind::Comma) => cursor += 1,
                Some(TokenKind::Gt) => return Some(cursor + 1),
                _ => return None,
            }
        }
    }
}
