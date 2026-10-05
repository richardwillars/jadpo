//! Deterministic small malformed-input campaign, isolated behind a deadline.
//! This checks termination, determinism and UTF-8 ranges, not complete parsing.
use jadpo_syntax::{lex, parse, TokenKind};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn check(source: &str) {
    let parsed = parse(Path::new("generated.jadpo"), source);
    assert_eq!(
        parse(Path::new("generated.jadpo"), source),
        parsed,
        "parsing must be deterministic"
    );
    for token in &parsed.tokens {
        assert!(
            source.get(token.range.start..token.range.end).is_some(),
            "invalid token range {}..{} of {} bytes in {source:?}",
            token.range.start,
            token.range.end,
            source.len()
        );
    }
    for diagnostic in &parsed.diagnostics {
        let span = diagnostic
            .primary
            .as_ref()
            .expect("syntax errors identify source");
        assert_eq!(span.source, "generated.jadpo");
        assert!(
            source.get(span.start..span.end).is_some(),
            "invalid diagnostic range {}..{} of {} bytes in {source:?}",
            span.start,
            span.end,
            source.len()
        );
    }
    let eof = parsed.tokens.last().unwrap();
    assert_eq!(eof.kind, TokenKind::Eof);
    assert_eq!(
        (eof.range.start, eof.range.end),
        (source.len(), source.len())
    );
}

fn corpus() {
    let atoms = [
        "type",
        "function",
        "module",
        "import",
        "public",
        "entity",
        "failure",
        "route",
        "return",
        "match",
        "success",
        "propagate",
        "auth",
        "none",
        "Name",
        "value",
        "-1",
        "{",
        "}",
        "(",
        ")",
        "[",
        "]",
        ":",
        ".",
        "?",
        "=>",
        "->",
        "=",
        "\"",
        "\\",
        "é",
        "🦀",
        "// comment\n",
        "\r\n",
        "\0",
    ];
    let mut seed: u32 = 0x5a17_d09f;
    for case in 0..384 {
        let mut source = String::new();
        for _ in 0..(8 + case % 41) {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            source.push_str(atoms[seed as usize % atoms.len()]);
            source.push(' ');
        }
        check(&source);
    }
    let source = "module demo.api\npublic type Item = Object { name: Text }\nfunction echo(input: Item) -> Item { var text = \"é🦀\" return input }";
    let tokens = lex(Path::new("generated.jadpo"), source).tokens;
    for token in tokens
        .iter()
        .filter(|t| !t.kind.is_trivia() && t.kind != TokenKind::Eof)
    {
        for replacement in ["", "}", "\"", "🦀"] {
            let mut mutation = source.to_owned();
            mutation.replace_range(token.range.start..token.range.end, replacement);
            check(&mutation);
        }
    }

    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let mut mutations = 0usize;
    for (corpus, expect_valid, minimum_files) in [("pass", true, 50), ("fail", false, 100)] {
        let fixtures = repository.join("tests/compile").join(corpus);
        let mut sources = Vec::new();
        collect_sources(&fixtures, &mut sources);
        sources.sort();
        assert!(
            sources.len() >= minimum_files,
            "compile-{corpus} syntax corpus unexpectedly shrank"
        );

        for path in sources {
            let source = fs::read_to_string(&path).unwrap();
            let parsed = parse(&path, &source);
            if expect_valid {
                assert!(
                    parsed.diagnostics.is_empty(),
                    "accepted fixture {} stopped parsing: {:?}",
                    path.display(),
                    parsed.diagnostics
                );
            }
            let significant = lex(&path, &source)
                .tokens
                .into_iter()
                .filter(|token| !token.kind.is_trivia() && token.kind != TokenKind::Eof)
                .collect::<Vec<_>>();
            if significant.is_empty() {
                continue;
            }

            // Sample across every fixture rather than spending the deadline
            // on large files. Each selected token is deleted and replaced by
            // both a structural delimiter and a multibyte unexpected character.
            let stride = significant.len().div_ceil(12).max(1);
            for index in (0..significant.len()).step_by(stride).take(12) {
                let token = &significant[index];
                for replacement in ["", "}", "🦀"] {
                    let mut mutation = source.clone();
                    mutation.replace_range(token.range.start..token.range.end, replacement);
                    check(&mutation);
                    mutations += 1;
                }
            }
        }
    }
    assert!(
        mutations >= 3_000,
        "compile corpus mutation campaign was too small: {mutations}"
    );
}

fn collect_sources(directory: &Path, sources: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_sources(&path, sources);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "jadpo")
        {
            sources.push(path);
        }
    }
}

#[test]
fn deterministic_malformed_corpus_terminates_with_valid_ranges() {
    const MARKER: &str = "JADPO_BOUNDED_PARSER_CHILD";
    if std::env::var_os(MARKER).is_some() {
        corpus();
        return;
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "deterministic_malformed_corpus_terminates_with_valid_ranges",
            "--nocapture",
        ])
        .env(MARKER, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if child.try_wait().unwrap().is_some() {
            let output = child.wait_with_output().unwrap();
            assert!(
                output.status.success(),
                "bounded parser child failed:\n{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            break;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let output = child.wait_with_output().unwrap();
            panic!(
                "bounded parser campaign exceeded 10 seconds:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
