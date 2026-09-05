use super::*;

const PATH: &str = "lex.spx";

fn tokens(source: &str) -> Vec<Token> {
    lex(source, PATH).unwrap_or_else(|diagnostic| panic!("`{source}` must lex: {diagnostic}"))
}

fn kinds(source: &str) -> Vec<TokenKind> {
    tokens(source).into_iter().map(|token| token.kind).collect()
}

/// The single non-`Eof` token of a source that holds exactly one.
fn only(source: &str) -> Token {
    let mut produced = tokens(source);
    assert_eq!(
        produced.len(),
        2,
        "`{source}` must lex to one token plus Eof"
    );
    produced.remove(0)
}

fn rejection(source: &str) -> Diagnostic {
    lex(source, PATH).expect_err(&format!("`{source}` must be rejected"))
}

fn code(source: &str) -> &'static str {
    rejection(source).code
}

fn comments(source: &str) -> Comments {
    lex_with_comments(source, PATH)
        .unwrap_or_else(|diagnostic| panic!("`{source}` must lex: {diagnostic}"))
        .1
}

#[test]
fn an_empty_source_yields_one_eof_token_at_the_origin() {
    let produced = tokens("");
    assert_eq!(produced.len(), 1);
    assert_eq!(produced[0].kind, TokenKind::Eof);
    assert_eq!(
        produced[0].span,
        Span {
            start: 0,
            end: 0,
            line: 1,
            column: 1
        }
    );
    assert_eq!(comments(""), Comments::default());
}

#[test]
fn spans_count_bytes_while_columns_count_characters() {
    // Two two-byte characters inside the string put the byte offset (7) and
    // the column (6) of the following token permanently out of step.
    let produced = tokens("\"éé\" y");
    assert_eq!(produced[0].kind, TokenKind::String("éé".to_owned()));
    assert_eq!(
        produced[0].span,
        Span {
            start: 0,
            end: 6,
            line: 1,
            column: 1
        }
    );
    assert_eq!(
        produced[1].span,
        Span {
            start: 7,
            end: 8,
            line: 1,
            column: 6
        },
        "the identifier starts at byte 7 and column 6"
    );
    assert_eq!(
        produced[2].span,
        Span {
            start: 8,
            end: 8,
            line: 1,
            column: 7
        }
    );
}

#[test]
fn a_newline_inside_a_string_moves_later_tokens_onto_the_next_line() {
    // A literal newline is admitted inside a string. The token keeps the line
    // it started on; everything after it is on the line the string ended on.
    let produced = tokens("\"a\nb\" c");
    assert_eq!(produced[0].kind, TokenKind::String("a\nb".to_owned()));
    assert_eq!(
        produced[0].span,
        Span {
            start: 0,
            end: 5,
            line: 1,
            column: 1
        }
    );
    assert_eq!(
        produced[1].span,
        Span {
            start: 6,
            end: 7,
            line: 2,
            column: 4
        }
    );
}

#[test]
fn the_eof_token_sits_after_trailing_trivia() {
    let produced = tokens("a\n\n// tail");
    let end = produced.last().expect("Eof is always produced");
    assert_eq!(end.kind, TokenKind::Eof);
    assert_eq!(
        end.span,
        Span {
            start: 10,
            end: 10,
            line: 3,
            column: 8
        },
        "trivia advances the position the Eof token reports"
    );
}

#[test]
fn every_supported_string_escape_decodes() {
    let source = r#""\n\r\t\"\'\\\u{41}\u{1F600}""#;
    assert_eq!(
        only(source).kind,
        TokenKind::String("\n\r\t\"'\\A\u{1F600}".to_owned())
    );
    // One to six hexadecimal digits, upper or lower case, including the
    // null scalar.
    assert_eq!(
        only(r#""\u{0}\u{aB}\u{10FFFF}""#).kind,
        TokenKind::String("\0\u{ab}\u{10FFFF}".to_owned())
    );
}

#[test]
fn rejected_string_escapes_report_spx_p005() {
    for source in [
        r#""\0""#, // `\0` is a char escape only, never a string escape
        r#""\b""#,
        r#""\x41""#,
        r#""\ ""#,
        r#""\uZ""#,         // `\u` not followed by a brace
        r#""\u{}""#,        // no digits
        r#""\u{ZZ}""#,      // no digits before the closing brace
        r#""\u{41""#,       // unterminated brace
        r#""\u{1234567}""#, // seven digits
        r#""\u{D800}""#,    // a surrogate is not a scalar value
        r#""\u{110000}""#,  // above the Unicode range
    ] {
        assert_eq!(code(source), "SPX-P005", "{source}");
    }
}

#[test]
fn unterminated_strings_report_spx_p004() {
    for source in ["\"", "\"abc", "\"abc\\", "\"abc\\\""] {
        let diagnostic = rejection(source);
        assert_eq!(diagnostic.code, "SPX-P004", "{source}");
        assert_eq!(
            diagnostic.span.expect("lexer rejections are located").end,
            source.len(),
            "{source}: the span runs to end of input"
        );
    }
}

#[test]
fn char_literals_decode_one_scalar_value() {
    for (source, scalar) in [
        ("'a'", 'a'),
        ("'😀'", '\u{1F600}'),
        (r"'\n'", '\n'),
        (r"'\r'", '\r'),
        (r"'\t'", '\t'),
        (r"'\0'", '\0'),
        (r"'\''", '\''),
        (r"'\\'", '\\'),
        (r"'\u{41}'", 'A'),
        (r"'\u{10FFFF}'", '\u{10FFFF}'),
    ] {
        assert_eq!(
            only(source).kind,
            TokenKind::Char(scalar as u32),
            "{source}"
        );
    }
    assert_eq!(
        only("'😀'").span,
        Span {
            start: 0,
            end: 6,
            line: 1,
            column: 1
        },
        "the span covers the four-byte scalar and both quotes"
    );
}

#[test]
fn malformed_char_literals_report_their_stable_codes() {
    // Nothing left to close the literal.
    for source in ["'", "'a", r"'\", r"'\u{41}"] {
        assert_eq!(code(source), "SPX-P006", "{source}");
    }
    // Closed, but not exactly one scalar value.
    for source in ["''", "'ab'", "'a'x", "'a'0"] {
        assert_eq!(code(source), "SPX-P008", "{source}");
    }
    // A bad escape inside an otherwise well-formed literal.
    for source in [
        r"'\q'",
        r#"'\"'"#, // `\"` is a string escape only, never a char escape
        r"'\u{}'",
        r"'\u{D800}'",
        r"'\u{1234567}'",
        r"'\uZ'",
    ] {
        assert_eq!(code(source), "SPX-P007", "{source}");
    }
}

#[test]
fn integer_literals_cover_the_i64_range_and_reject_overflow() {
    assert_eq!(only("9223372036854775807").kind, TokenKind::Int(i64::MAX));
    assert_eq!(
        only("007").kind,
        TokenKind::Int(7),
        "leading zeros are kept"
    );
    // Signed Minimum Literals v1: exactly `i64::MAX + 1` survives
    // tokenization as its own magnitude token, so a directly applied unary
    // `-` in the grammar can spell `i64::MIN`. Tokenization stays
    // context-free: the magnitude never consults the preceding token, and the
    // parser rejects the magnitude wherever no `-` claims it.
    assert_eq!(only("9223372036854775808").kind, TokenKind::IntMinMagnitude);
    assert_eq!(
        only("0009223372036854775808").kind,
        TokenKind::IntMinMagnitude,
        "the rule is the numeric magnitude, not its spelling"
    );
    assert_eq!(code("9223372036854775809"), "SPX-P003");
    assert_eq!(
        kinds("-9223372036854775807"),
        vec![TokenKind::Minus, TokenKind::Int(i64::MAX), TokenKind::Eof]
    );
    assert_eq!(
        kinds("-9223372036854775808"),
        vec![TokenKind::Minus, TokenKind::IntMinMagnitude, TokenKind::Eof]
    );
    // Subtraction is untouched: the magnitude token is produced identically
    // whether or not a `-` precedes it, so no expression retokenizes.
    assert_eq!(
        kinds("1 - 9223372036854775808"),
        vec![
            TokenKind::Int(1),
            TokenKind::Minus,
            TokenKind::IntMinMagnitude,
            TokenKind::Eof
        ]
    );
    assert_eq!(code("-9223372036854775809"), "SPX-P003");
}

#[test]
fn suffixed_integer_literals_carry_their_declared_width() {
    assert_eq!(only("2147483647i32").kind, TokenKind::Int32(i32::MAX));
    assert_eq!(only("255u8").kind, TokenKind::Uint8(u8::MAX));
    assert_eq!(only("0u8").kind, TokenKind::Uint8(0));
    assert_eq!(
        only("18446744073709551615usize").kind,
        TokenKind::Usize(u64::MAX)
    );
    assert_eq!(
        only("1usize").span,
        Span {
            start: 0,
            end: 6,
            line: 1,
            column: 1
        },
        "the span covers the suffix"
    );
    // Out of range for the declared width, including a magnitude that does
    // not even fit the i64 the `u8` path parses through first.
    assert_eq!(
        only("2147483648i32").kind,
        TokenKind::Int32MinMagnitude,
        "Signed Minimum Literals v1 keeps exactly i32::MAX + 1 for the parser"
    );
    assert_eq!(code("2147483649i32"), "SPX-P003");
    assert_eq!(code("9223372036854775808i32"), "SPX-P003");
    assert_eq!(code("256u8"), "SPX-P003");
    assert_eq!(code("99999999999999999999u8"), "SPX-P003");
    assert_eq!(code("18446744073709551616usize"), "SPX-T260");
}

#[test]
fn glued_and_unknown_integer_suffixes_report_their_stable_codes() {
    for source in ["1i64", "1u16", "1abc", "1_x"] {
        let diagnostic = rejection(source);
        assert_eq!(diagnostic.code, "SPX-P003", "{source}");
        assert!(diagnostic.message.contains("suffix"), "{source}");
    }
    // A well-formed suffix with more identifier glued to it.
    assert_eq!(code("1i32x"), "SPX-P003");
    assert_eq!(code("1u8x"), "SPX-P003");
    assert_eq!(
        code("1usizex"),
        "SPX-T260",
        "the usize path keeps its own typed code"
    );
}

#[test]
fn float_literals_record_their_declared_precision() {
    assert_eq!(
        only("1.5").kind,
        TokenKind::Float(FloatLiteral {
            value: 1.5,
            wide: true
        }),
        "an unsuffixed literal is f64"
    );
    assert_eq!(
        only("1.5f64").kind,
        TokenKind::Float(FloatLiteral {
            value: 1.5,
            wide: true
        }),
        "an explicit `f64` suffix is admitted and is not part of the number"
    );
    assert_eq!(
        only("0.1f64").kind,
        TokenKind::Float(FloatLiteral {
            value: 0.1f64,
            wide: true
        }),
        "an `f64` suffix agrees digit for digit with the unsuffixed literal"
    );
    assert_eq!(
        only("1.0e40f64").kind,
        TokenKind::Float(FloatLiteral {
            value: 1.0e40f64,
            wide: true
        }),
        "an exponent and an `f64` suffix compose"
    );
    let TokenKind::Float(narrow) = only("0.1f32").kind else {
        panic!("`0.1f32` must lex as a float");
    };
    assert!(!narrow.wide, "an `f32` suffix clears the wide flag");
    assert_eq!(
        narrow.value,
        f64::from(0.1f32),
        "an f32 literal rounds once, from the decimal digits"
    );
    assert_ne!(
        narrow.value, 0.1f64,
        "rounding through f64 first would lose the single-rounding guarantee"
    );
    // Exponents, with and without an explicit sign.
    for (source, value) in [("1.5e3", 1500.0), ("1.5e+3", 1500.0), ("1.5e-3", 0.0015)] {
        assert_eq!(
            only(source).kind,
            TokenKind::Float(FloatLiteral { value, wide: true }),
            "{source}"
        );
    }
}

#[test]
fn floats_require_a_fraction_and_a_complete_exponent() {
    // An exponent alone never promotes an integer literal to a float; `e10`
    // is read as a glued suffix.
    assert_eq!(code("1e10"), "SPX-P003");
    for source in ["1.5e", "1.5e+", "1.5e-", "1.5ex"] {
        assert_eq!(code(source), "SPX-P003", "{source}");
    }
    // Suffixes other than `f32`/`f64`, and identifier glued to a good one.
    for source in [
        "1.5x",
        "1.5f16",
        "1.5f32f32",
        "1.5f32_",
        "1.5f64f64",
        "1.5f64_",
    ] {
        assert_eq!(code(source), "SPX-P003", "{source}");
    }
}

#[test]
fn float_literals_that_do_not_round_to_a_finite_value_are_rejected() {
    assert_eq!(code("1.0e400"), "SPX-P003");
    assert_eq!(
        code("1.0e40f32"),
        "SPX-P003",
        "the f32 overflow is caught in the declared precision"
    );
}

#[test]
fn a_dot_without_a_fraction_stays_an_integer_followed_by_a_dot() {
    assert_eq!(
        kinds("1.x"),
        vec![
            TokenKind::Int(1),
            TokenKind::Dot,
            TokenKind::Ident("x".to_owned()),
            TokenKind::Eof
        ]
    );
}

#[test]
fn the_full_operator_inventory_lexes() {
    assert_eq!(
        kinds("@ ( ) { } [ ] : :: . , ; -> + - * / % ? ! = => == != < <= > >= && || |"),
        vec![
            TokenKind::At,
            TokenKind::LParen,
            TokenKind::RParen,
            TokenKind::LBrace,
            TokenKind::RBrace,
            TokenKind::LBracket,
            TokenKind::RBracket,
            TokenKind::Colon,
            TokenKind::ColonColon,
            TokenKind::Dot,
            TokenKind::Comma,
            TokenKind::Semicolon,
            TokenKind::Arrow,
            TokenKind::Plus,
            TokenKind::Minus,
            TokenKind::Star,
            TokenKind::Slash,
            TokenKind::Percent,
            TokenKind::Question,
            TokenKind::Bang,
            TokenKind::Eq,
            TokenKind::FatArrow,
            TokenKind::EqEq,
            TokenKind::BangEq,
            TokenKind::Lt,
            TokenKind::Le,
            TokenKind::Gt,
            TokenKind::Ge,
            TokenKind::AndAnd,
            TokenKind::OrOr,
            TokenKind::Pipe,
            TokenKind::Eof
        ]
    );
}

#[test]
fn glued_operators_take_the_longest_match_first() {
    assert_eq!(
        kinds("||| ::: === ->- <=<"),
        vec![
            TokenKind::OrOr,
            TokenKind::Pipe,
            TokenKind::ColonColon,
            TokenKind::Colon,
            TokenKind::EqEq,
            TokenKind::Eq,
            TokenKind::Arrow,
            TokenKind::Minus,
            TokenKind::Le,
            TokenKind::Lt,
            TokenKind::Eof
        ]
    );
}

#[test]
fn a_lone_ampersand_reports_spx_p002_and_names_the_pair() {
    let diagnostic = rejection("a & b");
    assert_eq!(diagnostic.code, "SPX-P002");
    assert_eq!(
        diagnostic.span,
        Some(Span {
            start: 2,
            end: 3,
            line: 1,
            column: 3
        })
    );
    assert_eq!(diagnostic.path.as_deref(), Some(PATH));
}

#[test]
fn characters_outside_the_grammar_report_spx_p001_over_their_whole_scalar() {
    let diagnostic = rejection("é");
    assert_eq!(diagnostic.code, "SPX-P001");
    assert_eq!(
        diagnostic.span,
        Some(Span {
            start: 0,
            end: 2,
            line: 1,
            column: 1
        }),
        "the span covers the two-byte scalar, not one byte of it"
    );
    // Non-ASCII letters are never identifier characters, and a zero-width
    // no-break space is not whitespace, so neither can hide in source.
    assert_eq!(code("ä"), "SPX-P001");
    assert_eq!(code("a\u{feff}"), "SPX-P001");
    assert_eq!(code("#"), "SPX-P001");
}

#[test]
fn a_leading_byte_order_mark_is_rejected_before_lexing() {
    let diagnostic = rejection("\u{feff}module a;");
    assert_eq!(diagnostic.code, "SPX-P001");
    assert_eq!(
        diagnostic.span,
        Some(Span {
            start: 0,
            end: 3,
            line: 1,
            column: 1
        })
    );
    assert_eq!(diagnostic.path.as_deref(), Some(PATH));
    assert!(
        diagnostic.help.is_some(),
        "the mark rejection carries a fix"
    );
}

#[test]
fn own_line_and_trailing_comments_are_distinguished() {
    let captured = comments("a // trailing\nb\n// own");
    assert_eq!(
        captured.items,
        vec![
            Comment {
                text: " trailing".to_owned(),
                offset: 2,
                line: 1,
                own_line: false,
                previous_token_line: 1,
            },
            Comment {
                text: " own".to_owned(),
                offset: 16,
                line: 3,
                own_line: true,
                previous_token_line: 2,
            }
        ]
    );
}

#[test]
fn previous_token_line_records_where_the_previous_token_ended() {
    // The string ends on line 2, so the comment on line 3 sticks to it.
    let captured = comments("\"a\nb\"\n// c");
    assert_eq!(captured.items.len(), 1);
    assert_eq!(captured.items[0].line, 3);
    assert_eq!(captured.items[0].previous_token_line, 2);
    // Before the first token there is no previous line at all.
    assert_eq!(comments("// c\na").items[0].previous_token_line, 0);
}

#[test]
fn a_final_comment_without_a_trailing_newline_is_captured() {
    let captured = comments("a\n// end");
    assert_eq!(
        captured.items,
        vec![Comment {
            text: " end".to_owned(),
            offset: 2,
            line: 2,
            own_line: true,
            previous_token_line: 1,
        }]
    );
}

#[test]
fn first_token_offset_separates_the_file_header_from_the_module() {
    assert_eq!(comments("// header\na").first_token_offset, 10);
    assert_eq!(
        comments("").first_token_offset,
        0,
        "an empty file has no header"
    );
    let only_comments = comments("// only");
    assert_eq!(only_comments.items.len(), 1);
    assert_eq!(
        only_comments.first_token_offset, 7,
        "with no token at all every comment stays in the header"
    );
}

#[test]
fn comment_text_keeps_interior_bytes_and_drops_trailing_whitespace() {
    let captured = comments("// hi\t \r\nx");
    assert_eq!(
        captured.items[0].text, " hi",
        "a carriage return and padding are trimmed, the leading space is not"
    );
    // Interior text is verbatim, so `//x` and `// x` stay distinct bytes.
    assert_eq!(comments("//x").items[0].text, "x");
    assert_eq!(comments("//").items[0].text, "");
}
