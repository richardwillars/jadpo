use jadpo_core::{analyze_sources, format_source};
use jadpo_diagnostics::{DecisionOwner, RepairKind, TextEdit};
use jadpo_syntax::{lex, parse, Declaration, SourceFile, TokenKind};
use std::path::Path;

fn apply(source: &str, edits: &[TextEdit]) -> String {
    assert!(!edits.is_empty(), "repair must supply actual edits");
    let mut edits = edits.to_vec();
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.start));
    let mut result = source.to_owned();
    let mut previous = source.len() + 1;
    for edit in edits {
        assert_eq!(edit.source, "tooling.jadpo");
        assert!(edit.end <= previous, "edits must not overlap");
        assert!(
            source.get(edit.start..edit.end).is_some(),
            "edit must respect UTF-8 boundaries"
        );
        result.replace_range(edit.start..edit.end, &edit.replacement);
        previous = edit.start;
    }
    result
}

fn token_meaning(source: &str) -> Vec<(TokenKind, String)> {
    let result = lex(Path::new("tooling.jadpo"), source);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    result
        .tokens
        .iter()
        .filter(|t| t.kind != TokenKind::Whitespace && t.kind != TokenKind::Eof)
        .map(|t| {
            (
                t.kind,
                if t.kind == TokenKind::LineComment {
                    t.text(source).trim_end_matches('\r').to_owned()
                } else {
                    t.text(source).to_owned()
                },
            )
        })
        .collect()
}

#[test]
fn formatting_preserves_literal_comments_and_token_semantics_across_line_endings() {
    let cases = [
        "type Label = Text {\nmin_length: 1\n}\nfunction greeting() -> Label {\nvar literal = \"🦀 \\\" { // } é\"\nreturn Label(literal) // ] } {\n}\n",
        "type Count = Int {}\nfunction subtract(value: Count) -> Count\n{\nreturn Count(value-2)\n}\n",
        "module demo.api\npublic type Record = Object {\nvalue: Text\n}\n// final comment {",
    ];
    for original in cases {
        for source in [original.to_owned(), original.replace('\n', "\r\n")] {
            let formatted = format_source(&source);
            assert_eq!(format_source(&formatted), formatted);
            assert_eq!(token_meaning(&source), token_meaning(&formatted));
            assert!(parse(Path::new("tooling.jadpo"), &formatted)
                .diagnostics
                .is_empty());
        }
    }
}

#[test]
fn applying_multiple_constraint_repairs_after_unicode_preserves_values_and_rechecks_cleanly() {
    let source = "// é🦀\ntype Label = Text { min_length 2 max_length 8 }\n";
    let parsed = parse(Path::new("tooling.jadpo"), source);
    assert_eq!(parsed.diagnostics.len(), 2);
    let mut edits = Vec::new();
    for diagnostic in &parsed.diagnostics {
        assert_eq!(diagnostic.code, "SYN_CONSTRAINT_COLON_REQUIRED");
        assert_eq!(
            diagnostic.recommended_next_step.kind,
            RepairKind::AutomaticFix
        );
        edits.extend(diagnostic.recommended_next_step.edits.clone());
    }
    let repaired = apply(source, &edits);
    assert_eq!(
        repaired,
        "// é🦀\ntype Label = Text { min_length: 2 max_length: 8 }\n"
    );
    let checked = analyze_sources(vec![SourceFile::new("tooling.jadpo".into(), repaired)]).unwrap();
    assert!(checked
        .syntax
        .diagnostics()
        .chain(checked.semantics.diagnostics.iter())
        .chain(checked.typing.diagnostics.iter())
        .chain(checked.failures.diagnostics.iter())
        .next()
        .is_none());
}

#[test]
fn authentication_choices_remain_human_owned_and_their_edits_have_distinct_security_effects() {
    let source = "// 🦀\ntype Health = Object { ok: Bool }\nroute GET /health { auth: nonke output: Health action: { return Health { ok: true } } }";
    let parsed = parse(Path::new("tooling.jadpo"), source);
    assert_eq!(parsed.diagnostics.len(), 1, "{:?}", parsed.diagnostics);
    let diagnostic = &parsed.diagnostics[0];
    assert_eq!(diagnostic.code, "ROUTE_AUTH_VALUE_INVALID");
    assert_eq!(diagnostic.decision_owner, DecisionOwner::Human);
    assert_eq!(
        diagnostic.recommended_next_step.kind,
        RepairKind::HumanDecision
    );
    assert!(diagnostic.recommended_next_step.edits.is_empty());
    assert_eq!(diagnostic.alternatives.len(), 2);
    let mut public_choices = Vec::new();
    for choice in &diagnostic.alternatives {
        assert_eq!(choice.decision_owner, DecisionOwner::Human);
        assert!(!choice.preferred);
        let candidate = apply(source, &choice.edits);
        let checked = parse(Path::new("tooling.jadpo"), &candidate);
        assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
        let route = checked
            .file
            .declarations
            .iter()
            .find_map(|d| {
                if let Declaration::Route(route) = d {
                    Some(route)
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(route.path, "/health");
        assert!(route.inline_action.is_some());
        assert!(route.output.is_some());
        public_choices.push(route.public);
    }
    public_choices.sort();
    assert_eq!(public_choices, [false, true]);
}
