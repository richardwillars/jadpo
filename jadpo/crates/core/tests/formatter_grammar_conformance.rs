use jadpo_core::format_source;
use jadpo_syntax::{lex, parse, TokenKind};
use std::fs;
use std::path::{Path, PathBuf};

fn source_files(directory: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            source_files(&path, files)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension == "jadpo")
        {
            files.push(path);
        }
    }
    Ok(())
}

fn token_meaning(path: &Path, source: &str) -> Vec<(TokenKind, String)> {
    let result = lex(path, source);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    result
        .tokens
        .iter()
        .filter(|token| !matches!(token.kind, TokenKind::Whitespace | TokenKind::Eof))
        .map(|token| (token.kind, token.text(source).to_owned()))
        .collect()
}

fn token_inventory(path: &Path, source: &str) -> Vec<(String, String)> {
    let mut inventory = token_meaning(path, source)
        .into_iter()
        .map(|(kind, text)| (format!("{kind:?}"), text))
        .collect::<Vec<_>>();
    inventory.sort();
    inventory
}

fn token_meaning_even_with_lexical_errors(path: &Path, source: &str) -> Vec<(TokenKind, String)> {
    lex(path, source)
        .tokens
        .into_iter()
        .filter(|token| !matches!(token.kind, TokenKind::Whitespace | TokenKind::Eof))
        .map(|token| (token.kind, token.text(source).to_owned()))
        .collect()
}

fn formatted_golden(repository: &Path, source: &Path) -> String {
    let fixture = source
        .file_name()
        .expect("fixture filename")
        .to_string_lossy();
    let path = repository
        .join("tests/formatter/golden")
        .join(format!("{fixture}.formatted"));
    fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "missing exact formatter expectation {}: {error}",
            path.display()
        )
    })
}

fn perturb_whitespace(path: &Path, source: &str) -> String {
    let result = lex(path, source);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let mut perturbed = String::new();
    for token in result.tokens {
        if token.kind != TokenKind::Whitespace {
            perturbed.push_str(token.text(source));
            continue;
        }

        for character in token.text(source).chars() {
            match character {
                '\r' => {}
                '\n' => perturbed.push_str(" \t\r\n\t  "),
                ' ' | '\t' => perturbed.push_str(" \t\t"),
                other => perturbed.push(other),
            }
        }
    }
    perturbed
}

fn remove_non_comment_line_breaks(path: &Path, source: &str) -> String {
    let result = lex(path, source);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let mut compact = String::new();
    let mut previous = None;
    for token in result.tokens {
        match token.kind {
            TokenKind::Whitespace => {
                let whitespace = token.text(source);
                if previous == Some(TokenKind::LineComment) {
                    // A line comment must keep its terminating line break or
                    // it would consume the next source token.
                    if whitespace.contains('\n') {
                        compact.push('\n');
                    }
                } else {
                    compact.push_str(&whitespace.replace("\r\n", " ").replace('\n', " "));
                }
            }
            TokenKind::Eof => {}
            kind => {
                compact.push_str(token.text(source));
                previous = Some(kind);
            }
        }
    }
    compact
}

fn add_excess_blank_lines(source: &str) -> String {
    let mut expanded = String::from("\n\n");
    for line in source.split_inclusive('\n') {
        expanded.push_str(line);
        if line
            .trim_matches(|character| matches!(character, ' ' | '\t' | '\r' | '\n'))
            .is_empty()
        {
            expanded.push('\n');
        }
    }
    expanded.push_str("\n\n");
    expanded
}

#[test]
fn compile_pass_grammar_corpus_survives_formatter_and_whitespace_variations() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let corpus = repository.join("tests/compile/pass");
    let mut files = Vec::new();
    source_files(&corpus, &mut files).unwrap();
    files.sort();
    assert!(files.len() >= 50, "grammar corpus unexpectedly shrank");

    for path in files {
        let source = fs::read_to_string(&path).unwrap();
        let expected = formatted_golden(&repository, &path);
        let parsed = parse(&path, &source);
        assert!(
            parsed.diagnostics.is_empty(),
            "{} did not parse before formatting: {:?}",
            path.display(),
            parsed.diagnostics
        );

        let formatted = format_source(&source);
        assert_eq!(
            formatted,
            expected,
            "unexpected exact formatted output for {}",
            path.display()
        );
        assert_eq!(
            token_inventory(&path, &source),
            token_inventory(&path, &formatted),
            "formatter changed tokens in {}",
            path.display()
        );
        assert!(
            parse(&path, &formatted).diagnostics.is_empty(),
            "formatted {} no longer parses",
            path.display()
        );
        assert_eq!(
            format_source(&formatted),
            formatted,
            "formatter is not idempotent for {}",
            path.display()
        );

        let perturbed = perturb_whitespace(&path, &source);
        assert_eq!(
            format_source(&perturbed),
            formatted,
            "whitespace variation changed canonical output for {}",
            path.display()
        );
    }
}

#[test]
fn compile_pass_grammar_corpus_survives_added_and_removed_line_breaks() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let corpus = repository.join("tests/compile/pass");
    let mut files = Vec::new();
    source_files(&corpus, &mut files).unwrap();
    files.sort();

    for path in files {
        let source = fs::read_to_string(&path).unwrap();
        let expected = formatted_golden(&repository, &path);
        let original_tokens = token_meaning(&path, &source);
        for (variation, changed) in [(
            "removed line breaks",
            remove_non_comment_line_breaks(&path, &source),
        )] {
            assert_eq!(
                original_tokens,
                token_meaning(&path, &changed),
                "{variation} changed tokens in {}",
                path.display()
            );
            assert!(
                parse(&path, &changed).diagnostics.is_empty(),
                "{variation} made {} unparsable",
                path.display()
            );

            let formatted = format_source(&changed);
            assert_eq!(
                token_inventory(&path, &source),
                token_inventory(&path, &formatted),
                "formatting {variation} changed tokens in {}",
                path.display()
            );
            assert!(
                parse(&path, &formatted).diagnostics.is_empty(),
                "formatting {variation} made {} unparsable",
                path.display()
            );
            assert_eq!(
                format_source(&formatted),
                formatted,
                "formatting {variation} was not idempotent for {}",
                path.display()
            );
        }

        let with_excess_blank_lines = add_excess_blank_lines(&source);
        assert_eq!(
            token_inventory(&path, &source),
            token_inventory(&path, &with_excess_blank_lines),
            "extra blank lines changed tokens in {}",
            path.display()
        );
        assert!(
            parse(&path, &with_excess_blank_lines)
                .diagnostics
                .is_empty(),
            "extra blank lines made {} unparsable",
            path.display()
        );
        assert_eq!(
            format_source(&with_excess_blank_lines),
            expected,
            "extra blank lines changed canonical output for {}",
            path.display()
        );
    }
}

#[test]
fn compile_fail_fixture_tokens_and_diagnostics_survive_formatting() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let corpus = repository.join("tests/compile/fail");
    let mut files = Vec::new();
    source_files(&corpus, &mut files).unwrap();
    files.sort();
    assert!(
        files.len() >= 100,
        "negative syntax corpus unexpectedly shrank"
    );

    for path in files {
        let source = fs::read_to_string(&path).unwrap();
        let before = parse(&path, &source);
        let formatted = format_source(&source);
        let after = parse(&path, &formatted);

        assert_eq!(
            {
                let mut tokens = token_meaning_even_with_lexical_errors(&path, &source);
                tokens.sort_by_key(|(kind, text)| (format!("{kind:?}"), text.clone()));
                tokens
            },
            {
                let mut tokens = token_meaning_even_with_lexical_errors(&path, &formatted);
                tokens.sort_by_key(|(kind, text)| (format!("{kind:?}"), text.clone()));
                tokens
            },
            "formatter changed tokens in {}",
            path.display()
        );
        assert_eq!(
            before
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code)
                .collect::<Vec<_>>(),
            after
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code)
                .collect::<Vec<_>>(),
            "formatter changed syntax diagnostics in {}",
            path.display()
        );
        assert_eq!(
            format_source(&formatted),
            formatted,
            "formatter is not idempotent for {}",
            path.display()
        );
    }
}
