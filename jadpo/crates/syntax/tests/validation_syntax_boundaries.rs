//! Independent contract cases: docs/grammar-v0.1.md §§2, 3, 4 and 15.
use jadpo_syntax::{lex, parse, Declaration, Expression, Statement, TokenKind};
use std::path::Path;

fn path() -> &'static Path {
    Path::new("unicode-and-boundaries.jadpo")
}

fn assert_valid_spans(source: &str) {
    let parsed = parse(path(), source);
    for token in &parsed.tokens {
        assert!(token.range.start <= token.range.end);
        assert!(
            source.get(token.range.start..token.range.end).is_some(),
            "{token:?} in {source:?}"
        );
    }
    for diagnostic in &parsed.diagnostics {
        if let Some(span) = &diagnostic.primary {
            assert_eq!(span.source, path().to_str().unwrap());
            assert!(
                source.get(span.start..span.end).is_some(),
                "{diagnostic:?} in {source:?}"
            );
        }
    }
    let eof = parsed.tokens.last().unwrap();
    assert_eq!(eof.kind, TokenKind::Eof);
    assert_eq!(
        (eof.range.start, eof.range.end),
        (source.len(), source.len())
    );
}

#[test]
fn unicode_strings_and_comments_have_lossless_byte_ranges() {
    let source = "// café 🦀\r\nfunction greeting() -> Text { return \"é中🦀\" }";
    let result = lex(path(), source);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(
        result
            .tokens
            .iter()
            .map(|token| token.text(source))
            .collect::<String>(),
        source
    );
    let literal = result
        .tokens
        .iter()
        .find(|token| token.kind == TokenKind::StringLiteral)
        .unwrap();
    assert_eq!(literal.text(source), "\"é中🦀\"");
    assert_eq!(literal.range.end - literal.range.start, 11);
    assert_valid_spans(source);
}

#[test]
fn invalid_unicode_escape_points_to_complete_escape_and_resumes() {
    let source = "// 🦀\n\"café\\中\" type After = Text {}";
    let result = lex(path(), source);
    assert_eq!(result.diagnostics.len(), 1);
    let diagnostic = &result.diagnostics[0];
    assert_eq!(diagnostic.code, "SYN_INVALID_ESCAPE");
    let span = diagnostic.primary.as_ref().unwrap();
    assert_eq!(&source[span.start..span.end], "\\中");
    assert!(result
        .tokens
        .iter()
        .any(|token| token.text(source) == "After"));
    assert_valid_spans(source);
}

#[test]
fn every_documented_escape_is_accepted_and_unknown_escapes_are_rejected() {
    for escape in ["\\\"", "\\\\", "\\n", "\\r", "\\t"] {
        let source = format!("\"left{escape}right\"");
        assert!(lex(path(), &source).diagnostics.is_empty(), "{source}");
    }
    for escape in ["\\0", "\\x", "\\u", "\\/", "\\é"] {
        let source = format!("\"left{escape}right\"");
        let result = lex(path(), &source);
        assert_eq!(
            result
                .diagnostics
                .iter()
                .map(|d| d.code)
                .collect::<Vec<_>>(),
            ["SYN_INVALID_ESCAPE"],
            "{source}"
        );
        assert_valid_spans(&source);
    }
}

#[test]
fn unterminated_strings_at_eof_and_line_end_are_lexical_errors() {
    for source in ["\"", "\"é🦀", "\"text\\", "\"text\n", "\"text\r\n"] {
        let result = lex(path(), source);
        assert_eq!(
            result
                .diagnostics
                .iter()
                .map(|d| d.code)
                .collect::<Vec<_>>(),
            ["SYN_UNTERMINATED_STRING"],
            "{source:?}"
        );
        assert_valid_spans(source);
    }
}

#[test]
fn keywords_only_reserve_the_complete_identifier() {
    for keyword in [
        "type",
        "enum",
        "entity",
        "value",
        "input",
        "output",
        "failure",
        "function",
        "action",
        "fixture",
        "test",
        "route",
        "config",
        "module",
        "import",
        "persist",
        "code",
        "kind",
        "message",
        "public",
        "internal",
        "optional",
        "required",
        "many",
        "missing",
        "fails",
        "var",
        "mut",
        "return",
        "reject",
        "attempt",
        "call",
        "async",
        "await",
        "if",
        "else",
        "match",
        "success",
        "propagate",
        "assert",
        "advance",
        "and",
        "or",
        "not",
        "auth",
        "path",
        "explicitly",
        "run",
        "min",
        "max",
        "min_length",
        "max_length",
        "pattern",
        "format",
        "throw",
        "create",
        "query",
        "where",
        "order_by",
        "asc",
        "desc",
        "update",
        "set",
        "patch",
        "empty",
        "conflict",
        "constraint",
        "identity",
        "unique",
        "index",
        "references",
        "as",
        "on_delete",
        "restrict",
        "cascade",
        "set_null",
        "inverse",
        "via",
        "include",
        "into",
        "limit",
        "offset",
        "delete",
        "true",
        "false",
        "none",
        "GET",
        "POST",
        "PUT",
        "PATCH",
        "DELETE",
    ] {
        let result = lex(path(), keyword);
        assert!(result.diagnostics.is_empty(), "{keyword}");
        assert_eq!(result.tokens[0].text(keyword), keyword);
        assert_ne!(result.tokens[0].kind, TokenKind::Identifier, "{keyword}");
        for name in [
            format!("{keyword}_value"),
            format!("{keyword}2"),
            format!("_{keyword}"),
        ] {
            let result = lex(path(), &name);
            assert!(result.diagnostics.is_empty());
            assert_eq!(result.tokens.len(), 2);
            assert_eq!(result.tokens[0].kind, TokenKind::Identifier, "{name}");
            assert_eq!(result.tokens[0].text(&name), name);
        }
    }

    for case_variant in ["Type", "TYPE", "Function", "TRUE", "Get", "get", "Patch"] {
        let result = lex(path(), case_variant);
        assert!(result.diagnostics.is_empty(), "{case_variant}");
        assert_eq!(
            result.tokens[0].kind,
            TokenKind::Identifier,
            "keyword matching is exact and case-sensitive: {case_variant}"
        );
    }
}

#[test]
fn identifier_tokens_obey_the_documented_ascii_boundaries() {
    for name in [
        "_",
        "__",
        "_leading",
        "trailing_",
        "a__b",
        "A",
        "z",
        "name2",
    ] {
        let result = lex(path(), name);
        assert!(
            result.diagnostics.is_empty(),
            "{name}: {:?}",
            result.diagnostics
        );
        assert_eq!(result.tokens[0].kind, TokenKind::Identifier, "{name}");
        assert_eq!(result.tokens[0].text(name), name);
    }

    let digit_prefix = "9name";
    let result = lex(path(), digit_prefix);
    assert!(result.diagnostics.is_empty());
    assert_eq!(result.tokens[0].kind, TokenKind::IntegerLiteral);
    assert_eq!(result.tokens[0].text(digit_prefix), "9");
    assert_eq!(result.tokens[1].kind, TokenKind::Identifier);
    assert_eq!(result.tokens[1].text(digit_prefix), "name");

    let unicode_prefix = "éclair";
    let result = lex(path(), unicode_prefix);
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].code, "SYN_UNEXPECTED_CHARACTER");
    let span = result.diagnostics[0].primary.as_ref().unwrap();
    assert_eq!(&unicode_prefix[span.start..span.end], "é");
    assert!(result
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Identifier && token.text(unicode_prefix) == "clair"));
    assert_valid_spans(unicode_prefix);
}

#[test]
fn punctuation_uses_complete_operator_tokens() {
    let source = "== != <= >= -> => = < > - . , : ?";
    let tokens = lex(path(), source);
    assert!(tokens.diagnostics.is_empty());
    assert_eq!(
        tokens
            .tokens
            .iter()
            .filter(|t| !t.kind.is_trivia())
            .map(|t| t.kind)
            .collect::<Vec<_>>(),
        [
            TokenKind::EqualEqual,
            TokenKind::BangEqual,
            TokenKind::LessEqual,
            TokenKind::GreaterEqual,
            TokenKind::Arrow,
            TokenKind::FatArrow,
            TokenKind::Equal,
            TokenKind::LeftAngle,
            TokenKind::RightAngle,
            TokenKind::Minus,
            TokenKind::Dot,
            TokenKind::Comma,
            TokenKind::Colon,
            TokenKind::Question,
            TokenKind::Eof
        ]
    );
}

#[test]
fn route_paths_and_comment_markers_inside_strings_remain_literal() {
    let source = "route GET /v1/items/{item_id} { auth: none path: { item_id: Uuid } output: Text run: greeting() }\nfunction greeting() -> Text { return \"https://example.test//x\" }";
    let result = parse(path(), source);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(
        result
            .tokens
            .iter()
            .filter(|t| t.kind == TokenKind::RoutePath)
            .map(|t| t.text(source))
            .collect::<Vec<_>>(),
        ["/v1/items/{item_id}"]
    );
    assert!(!result
        .tokens
        .iter()
        .any(|t| t.kind == TokenKind::LineComment));
}

#[test]
fn legal_trivia_does_not_change_significant_tokens_or_declaration_count() {
    let source = "module demo.api\npublic type Greeting = Text {}\nfunction greeting() -> Text { return \"hello\" }";
    let base = parse(path(), source);
    assert!(base.diagnostics.is_empty());
    let significant = base
        .tokens
        .iter()
        .filter(|t| !t.kind.is_trivia() && t.kind != TokenKind::Eof)
        .map(|t| (t.kind, t.text(source)))
        .collect::<Vec<_>>();
    for separator in [" ", "\t", "\r\n", " // comment é🦀\n"] {
        let mutated = significant
            .iter()
            .map(|(_, text)| *text)
            .collect::<Vec<_>>()
            .join(separator);
        let parsed = parse(path(), &mutated);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        assert_eq!(parsed.file.declarations.len(), base.file.declarations.len());
        assert_eq!(
            parsed
                .tokens
                .iter()
                .filter(|t| !t.kind.is_trivia() && t.kind != TokenKind::Eof)
                .map(|t| (t.kind, t.text(&mutated)))
                .collect::<Vec<_>>(),
            significant
        );
    }
}

#[test]
fn all_utf8_truncations_of_representative_sources_terminate_with_valid_spans() {
    // A deterministic truncation property, not a claim of fuzz-campaign coverage.
    for source in [
        "// é🦀\nmodule demo.api\nimport demo.domain { Greeting }\npublic type Answer = Object { text: Greeting }",
        "function greeting(input: Text) -> Text { return \"hello é🦀\\n\" }",
        "failure Missing { kind: NotFound message: \"absent\" public: { item_id: Uuid } }",
        "route GET /health { auth: none output: Text run: greeting() }",
    ] {
        for end in (0..=source.len()).filter(|end| source.is_char_boundary(*end)) {
            assert_valid_spans(&source[..end]);
        }
    }
}

#[test]
fn incomplete_declarations_and_delimiters_cannot_be_silently_accepted() {
    for source in [
        "type",
        "type Name =",
        "type Name = {",
        "type Name = { value:",
        "function f(",
        "function f() ->",
        "function f() -> Text { return",
        "module",
        "module demo.",
        "module demo\nimport",
        "module demo\nimport demo.other {",
        "module demo\nimport demo.other { Name",
        "route GET /health {",
    ] {
        let parsed = parse(path(), source);
        assert!(
            !parsed.diagnostics.is_empty(),
            "accepted incomplete source: {source}"
        );
        assert_valid_spans(source);
    }
}

#[test]
fn malformed_top_level_token_recovers_to_following_declaration() {
    let source = "; type Recovered = Text {}";
    let parsed = parse(path(), source);
    assert!(!parsed.diagnostics.is_empty());
    assert_eq!(
        parsed.file.declarations.len(),
        1,
        "{:?}",
        parsed.diagnostics
    );
    assert_valid_spans(source);
}

#[test]
fn module_header_import_order_and_nonempty_import_set_are_enforced() {
    for source in [
        "import demo.domain { Name }\ntype Local = Text {}",
        "module demo\nmodule other\ntype Local = Text {}",
        "module demo\ntype Local = Text {}\nimport other { Name }",
        "module demo\nimport other {}\ntype Local = Text {}",
    ] {
        let parsed = parse(path(), source);
        assert!(
            !parsed.diagnostics.is_empty(),
            "accepted invalid module syntax: {source}"
        );
    }
}

#[test]
fn subtraction_does_not_require_semantic_whitespace() {
    // §2 makes whitespace non-semantic; §12 specifies numeric subtraction.
    for expression in ["3 - 2", "3-2", "3 -2", "3- 2", "3 - -2", "3--2"] {
        let source = format!("function difference() -> Int {{ return {expression} }}");
        let parsed = parse(path(), &source);
        assert!(
            parsed.diagnostics.is_empty(),
            "{expression}: {:?}",
            parsed.diagnostics
        );
        let Declaration::Callable(callable) = &parsed.file.declarations[0] else {
            panic!("expected callable")
        };
        assert_eq!(callable.body.statements.len(), 1);
        let Statement::Return(result) = &callable.body.statements[0] else {
            panic!("expected return")
        };
        let expected = if expression == "3 - -2" || expression == "3--2" {
            "(Subtract 3 (Negate 2))"
        } else {
            "(Subtract 3 2)"
        };
        assert_eq!(expression_shape(&result.value), expected, "{expression}");
    }
}

#[test]
fn subtraction_after_names_members_calls_and_grouped_operands_keeps_the_operator() {
    for operand in [
        "count",
        "input",
        "output",
        "value",
        "config",
        "path",
        "item.min",
        "item.max",
        "read_count()",
        "(3)",
    ] {
        for separator in ["", " ", " // 🦀\n"] {
            let expression = format!("{operand}{separator}-2");
            let source = format!("function difference() -> Int {{ return {expression} }}");
            let parsed = parse(path(), &source);
            assert!(
                parsed.diagnostics.is_empty(),
                "{expression}: {:?}",
                parsed.diagnostics
            );
            let Declaration::Callable(callable) = &parsed.file.declarations[0] else {
                panic!("expected callable")
            };
            let Statement::Return(result) = &callable.body.statements[0] else {
                panic!("expected return")
            };
            let Expression::Binary(binary) = &result.value else {
                panic!("missing subtraction: {expression}")
            };
            assert_eq!(binary.operator, jadpo_syntax::BinaryOperator::Subtract);
            assert_eq!(expression_shape(&binary.right), "2");
        }
    }
}

#[test]
fn signed_literals_remain_intact_at_expression_starts_and_in_constraints() {
    for prefix in [
        "",
        "return ",
        "min: ",
        "max: ",
        "var count = ",
        "read_count(",
        "left + ",
        "left * ",
        "left - ",
        "left, ",
    ] {
        for literal in ["-1", "-1.25", "-9223372036854775808"] {
            let source = format!("{prefix}{literal}");
            let result = lex(path(), &source);
            assert!(result.diagnostics.is_empty());
            let number = result
                .tokens
                .iter()
                .rev()
                .find(|t| t.kind != TokenKind::Eof && !t.kind.is_trivia())
                .unwrap();
            assert_eq!(number.text(&source), literal, "{source}");
            assert_eq!(
                number.kind,
                if literal.contains('.') {
                    TokenKind::DecimalLiteral
                } else {
                    TokenKind::IntegerLiteral
                }
            );
        }
    }
    for source in [
        "type SignedCount = Int { min: -10 max: -1 }",
        "type SignedAmount = Decimal { min: -1.25 max: -0.25 }",
        "function minimum() -> Int { return -9223372036854775808 }",
    ] {
        let parsed = parse(path(), source);
        assert!(
            parsed.diagnostics.is_empty(),
            "{source}: {:?}",
            parsed.diagnostics
        );
    }
}

fn expression_shape(expression: &Expression) -> String {
    match expression {
        Expression::Binary(binary) => format!(
            "({:?} {} {})",
            binary.operator,
            expression_shape(&binary.left),
            expression_shape(&binary.right)
        ),
        Expression::Unary(unary) => {
            format!("({:?} {})", unary.operator, expression_shape(&unary.value))
        }
        Expression::Grouped(group) => expression_shape(&group.value),
        Expression::Literal(literal) if literal.text.starts_with('-') => {
            format!("(Negate {})", &literal.text[1..])
        }
        Expression::Literal(literal) => literal.text.clone(),
        other => panic!("unexpected expression in arithmetic case: {other:?}"),
    }
}

#[test]
fn arithmetic_and_boolean_precedence_match_the_contract_and_parentheses_override_it() {
    // Explicit expected trees are independent of the parser's precedence table.
    for (source_expression, expected) in [
        ("1 + 2 * 3", "(Add 1 (Multiply 2 3))"),
        ("(1 + 2) * 3", "(Multiply (Add 1 2) 3)"),
        ("8 - 3 - 1", "(Subtract (Subtract 8 3) 1)"),
        ("8 / 2 * 3 % 2", "(Remainder (Multiply (Divide 8 2) 3) 2)"),
        (
            "1 + 2 < 4 == true and not false or false",
            "(Or (And (Equal (Less (Add 1 2) 4) true) (Not false)) false)",
        ),
    ] {
        let source = format!("function evaluate() -> Int {{ return {source_expression} }}");
        let parsed = parse(path(), &source);
        assert!(
            parsed.diagnostics.is_empty(),
            "{source_expression}: {:?}",
            parsed.diagnostics
        );
        let Declaration::Callable(callable) = &parsed.file.declarations[0] else {
            panic!("expected callable")
        };
        assert_eq!(callable.body.statements.len(), 1);
        let Statement::Return(result) = &callable.body.statements[0] else {
            panic!("expected return")
        };
        assert_eq!(
            expression_shape(&result.value),
            expected,
            "{source_expression}"
        );
    }
}

#[test]
fn contextual_names_and_case_are_not_prematurely_rejected_by_the_parser() {
    for source in [
        "type Names = Object { input: Text output: Text value: Text }",
        "function echo_value(input: Text) -> Text { var output = input var value = output return value }",
        "type wrong_case = Text {}\nfunction WrongCase() -> Text { return \"ok\" }",
    ] {
        let parsed = parse(path(), source);
        assert!(parsed.diagnostics.is_empty(), "{source}: {:?}", parsed.diagnostics);
    }
}

#[test]
fn module_public_visibility_cannot_be_applied_to_routes() {
    let source =
        "module demo.api\npublic route GET /health { auth: none output: Text run: greeting() }";
    let parsed = parse(path(), source);
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|d| d.code == "SYN_ROUTE_EXPORT_INVALID"),
        "{:?}",
        parsed.diagnostics
    );
}
