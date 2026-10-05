//! Fixed-seed parser/type/failure campaign over compile-corpus mutations.
//! Failures report a delta-reduced source that must be retained as a fixture.
use jadpo_semantic::{build_semantic_graph, check_failures, check_types};
use jadpo_syntax::{lex, parse, ParsedSyntax, TokenKind};
use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const INITIAL_SEED: u32 = 0x7a4d_93e1;
const CAMPAIGN_CASES: usize = 2_048;
const ATOMS: &[&str] = &[
    "type",
    "function",
    "entity",
    "failure",
    "fails",
    "module",
    "import",
    "public",
    "return",
    "reject",
    "match",
    "attempt",
    "success",
    "propagate",
    "Text",
    "Int",
    "true",
    "false",
    "none",
    "-1",
    "\"text\"",
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
    "é",
    "🦀",
    "// fuzz\n",
];

#[derive(Debug)]
struct FuzzFailure {
    kind: &'static str,
    detail: String,
}

fn fail(kind: &'static str, detail: impl Into<String>) -> FuzzFailure {
    FuzzFailure {
        kind,
        detail: detail.into(),
    }
}

fn caught_check(source: &str) -> Result<(), FuzzFailure> {
    match catch_unwind(AssertUnwindSafe(|| check_once(source))) {
        Ok(result) => result,
        Err(payload) => {
            let detail = payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| {
                    payload
                        .downcast_ref::<&str>()
                        .map(|message| (*message).to_owned())
                })
                .unwrap_or_else(|| "non-string panic payload".to_owned());
            Err(fail("panic", detail))
        }
    }
}

fn check_once(source: &str) -> Result<(), FuzzFailure> {
    let path = Path::new("fuzz-generated.jadpo");
    let parsed = parse(path, source);
    let parsed_again = parse(path, source);
    if parsed != parsed_again {
        return Err(fail(
            "parser_nondeterminism",
            "same source parsed differently",
        ));
    }
    for token in &parsed.tokens {
        validate_range(source, "token", token.range.start, token.range.end)?;
    }
    check_parsed_diagnostics(source, &parsed, "parse")?;

    let files = [parsed.clone()];
    let graph = build_semantic_graph(&files);
    let graph_again = build_semantic_graph(&files);
    if graph != graph_again {
        return Err(fail(
            "graph_nondeterminism",
            "semantic graph changed between runs",
        ));
    }
    check_diagnostic_spans(source, &graph.diagnostics, "semantic")?;

    let types = check_types(&files, &graph);
    let types_again = check_types(&files, &graph);
    if types != types_again {
        return Err(fail(
            "type_nondeterminism",
            "type-check result changed between runs",
        ));
    }
    check_diagnostic_spans(source, &types.diagnostics, "type")?;

    let failures = check_failures(&files, &graph);
    let failures_again = check_failures(&files, &graph);
    if failures != failures_again {
        return Err(fail(
            "failure_nondeterminism",
            "failure analysis changed between runs",
        ));
    }
    check_diagnostic_spans(source, &failures.diagnostics, "failure")?;
    Ok(())
}

fn check_parsed_diagnostics(
    source: &str,
    parsed: &ParsedSyntax,
    phase: &str,
) -> Result<(), FuzzFailure> {
    check_diagnostic_spans(source, &parsed.diagnostics, phase)
}

fn check_diagnostic_spans(
    source: &str,
    diagnostics: &[jadpo_diagnostics::Diagnostic],
    phase: &str,
) -> Result<(), FuzzFailure> {
    for diagnostic in diagnostics {
        let Some(span) = &diagnostic.primary else {
            continue;
        };
        if span.source == "fuzz-generated.jadpo" {
            validate_range(source, phase, span.start, span.end)?;
        }
    }
    Ok(())
}

fn validate_range(source: &str, phase: &str, start: usize, end: usize) -> Result<(), FuzzFailure> {
    if start > end || source.get(start..end).is_none() {
        return Err(fail(
            "invalid_span",
            format!(
                "{phase} range {start}..{end} is invalid for {} bytes",
                source.len()
            ),
        ));
    }
    Ok(())
}

fn next_random(state: &mut u32) -> usize {
    *state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    *state as usize
}

fn mutate(source: &str, state: &mut u32) -> String {
    let mut result = source.to_owned();
    let edit_count = 1 + next_random(state) % 4;
    for _ in 0..edit_count {
        let tokens = lex(Path::new("fuzz-generated.jadpo"), &result).tokens;
        let significant = tokens
            .iter()
            .filter(|token| !token.kind.is_trivia() && token.kind != TokenKind::Eof)
            .collect::<Vec<_>>();
        let atom = ATOMS[next_random(state) % ATOMS.len()];
        if significant.is_empty() {
            result.insert_str(0, atom);
            continue;
        }

        let token = significant[next_random(state) % significant.len()];
        match next_random(state) % 4 {
            0 => result.replace_range(token.range.start..token.range.end, atom),
            1 => result.replace_range(token.range.start..token.range.end, ""),
            2 => result.insert_str(token.range.start, atom),
            _ => result.insert_str(token.range.end, atom),
        }
    }
    result
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

fn load_seed_corpus() -> Vec<String> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let mut paths = Vec::new();
    for group in ["pass", "fail"] {
        collect_sources(&repository.join("tests/compile").join(group), &mut paths);
    }
    paths.sort();
    assert!(
        paths.len() >= 150,
        "compile fuzz seed corpus unexpectedly shrank"
    );
    paths
        .into_iter()
        .map(|path| fs::read_to_string(path).unwrap())
        .collect()
}

fn minimize(source: &str, expected_kind: &str) -> String {
    minimize_with(
        source,
        |candidate| matches!(caught_check(candidate), Err(issue) if issue.kind == expected_kind),
    )
}

fn minimize_with(source: &str, mut still_fails: impl FnMut(&str) -> bool) -> String {
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut current = source.to_owned();
    let mut granularity = 2usize;
    let mut attempts = 0usize;

    while Instant::now() < deadline && attempts < 512 {
        let boundaries = current
            .char_indices()
            .map(|(offset, _)| offset)
            .chain(std::iter::once(current.len()))
            .collect::<Vec<_>>();
        let character_count = boundaries.len().saturating_sub(1);
        if character_count < 2 {
            break;
        }
        let chunk_size = character_count.div_ceil(granularity.max(2));
        let mut reduced = false;
        for start_character in (0..character_count).step_by(chunk_size) {
            let end_character = (start_character + chunk_size).min(character_count);
            let mut candidate = current.clone();
            candidate.replace_range(boundaries[start_character]..boundaries[end_character], "");
            attempts += 1;
            if still_fails(&candidate) {
                current = candidate;
                granularity = 2;
                reduced = true;
                break;
            }
            if attempts >= 512 || Instant::now() >= deadline {
                break;
            }
        }
        if !reduced {
            if granularity >= character_count {
                break;
            }
            granularity = (granularity * 2).min(character_count);
        }
    }
    current
}

#[test]
fn delta_reducer_preserves_the_failure_while_shrinking() {
    let minimized = minimize_with("discard these words around trigger", |candidate| {
        candidate.contains("trigger")
    });
    assert_eq!(minimized, "trigger");
}

fn campaign() -> Result<(), FuzzFailure> {
    let corpus = load_seed_corpus();
    let mut state = INITIAL_SEED;
    for case in 0..CAMPAIGN_CASES {
        let source_index = next_random(&mut state) % corpus.len();
        let generated = mutate(&corpus[source_index], &mut state);
        if let Err(issue) = caught_check(&generated) {
            let minimized = minimize(&generated, issue.kind);
            return Err(fail(
                issue.kind,
                format!(
                    "seed=0x{INITIAL_SEED:08x}, case={case}, source_index={source_index}: {}\nminimized reproducer:\n{minimized}",
                    issue.detail
                ),
            ));
        }
    }
    Ok(())
}

#[test]
fn seeded_compile_corpus_mutations_analyze_deterministically() {
    const CHILD_MARKER: &str = "JADPO_SEMANTIC_FUZZ_CHILD";
    if std::env::var_os(CHILD_MARKER).is_some() {
        std::panic::set_hook(Box::new(|_| {}));
        if let Err(issue) = campaign() {
            eprintln!("{}: {}", issue.kind, issue.detail);
            std::process::exit(1);
        }
        return;
    }

    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "seeded_compile_corpus_mutations_analyze_deterministically",
            "--nocapture",
        ])
        .env(CHILD_MARKER, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        if child.try_wait().unwrap().is_some() {
            let output = child.wait_with_output().unwrap();
            assert!(
                output.status.success(),
                "semantic fuzz child failed:\n{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            break;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let output = child.wait_with_output().unwrap();
            panic!(
                "semantic fuzz campaign exceeded 90 seconds:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
