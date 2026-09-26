use jadpo_core::{
    accept_index_recommendation, analyze_project, checked_source_revision, create_project,
    derive_artifacts, derive_target, diff_schema_identities, discover_sources, format_source,
    index_recommendation_count, index_recommendations_json, initialize_schema_identities,
    register_schema_additions, rename_schema_identity, snapshot_schema_identities,
    validate_schema_decisions, validate_schema_identities, write_artifacts,
    write_schema_decision_template, write_schema_migration_plan, write_schema_migration_sql_review,
    AnalyzedProject,
};
use jadpo_diagnostics::{catalogue_definition, json_string, Diagnostic, DiagnosticFact};
use jadpo_semantic::checked_manifest_json;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Stdio};
use std::thread;
use std::time::{Duration, Instant};

mod lsp;

fn main() -> ExitCode {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    if arguments.first().map(String::as_str) == Some("check")
        && arguments
            .iter()
            .any(|argument| argument == "--diagnostic-format=json")
    {
        return run_json_check(&arguments);
    }

    match run(arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(diagnostic) => {
            print_human_diagnostic(&diagnostic);
            ExitCode::from(1)
        }
    }
}

fn run(arguments: Vec<String>) -> Result<(), Diagnostic> {
    let Some(command) = arguments.first().map(String::as_str) else {
        print_help();
        return Ok(());
    };

    if matches!(command, "help" | "--help" | "-h") {
        print_help();
        return Ok(());
    }

    if command == "schema" {
        return run_schema(&arguments);
    }

    if command == "lsp" {
        if arguments.len() != 1 {
            return Err(Diagnostic::error("CLI_LSP_ARGUMENTS"));
        }
        return lsp::run_stdio();
    }

    let project = arguments
        .get(1)
        .ok_or_else(|| Diagnostic::error("CLI_PROJECT_REQUIRED"))?;
    let project = Path::new(project);

    match command {
        "new" => {
            let files = create_project(project)?;
            println!(
                "created project {} with {} deterministic file(s)",
                project.display(),
                files.len()
            );
            Ok(())
        }
        "check" => {
            if arguments.len() != 2 {
                return Err(Diagnostic::error(
                    "CLI_CHECK_ARGUMENTS")
                .with_note("expected: jadpo check <project> [--diagnostic-format=json]"));
            }
            run_human_check(project)
        }
        "inspect" => {
            let analyzed = analyze_project(project)?;
            require_valid_frontend(&analyzed)?;
            validate_schema_identities(project, &analyzed)?;
            println!(
                "{}",
                checked_manifest_json(&analyzed.semantics, &analyzed.typing, &analyzed.failures)
            );
            Ok(())
        }
        "artifacts" => {
            let analyzed = analyze_project(project)?;
            require_valid_frontend(&analyzed)?;
            validate_schema_identities(project, &analyzed)?;
            let artifacts = derive_artifacts(project, &analyzed);
            let output = write_artifacts(project, &artifacts)?;
            println!(
                "generated {} deterministic artifact(s) beneath {}",
                artifacts.len(),
                output.display()
            );
            Ok(())
        }
        "build" => run_human_build(project),
        "incident" => {
            if arguments.len() != 3 {
                return Err(Diagnostic::error(
                    "CLI_INCIDENT_ARGUMENTS")
                .with_note("expected: jadpo incident <project> <event-json-file>"));
            }
            run_incident_enrichment(project, Path::new(&arguments[2]))
        }
        "test" => run_human_test(project),
        "fmt" => {
            let check = match arguments.get(2).map(String::as_str) {
                None => false,
                Some("--check") => true,
                Some(_) => {
                    return Err(Diagnostic::error(
                        "CLI_FMT_ARGUMENTS"));
                }
            };
            if arguments.len() > 3 {
                return Err(Diagnostic::error(
                    "CLI_FMT_ARGUMENTS"));
            }
            run_format(project, check)
        }
        "watch" => {
            if project == Path::new("--diagnostic-format=json") {
                return Err(Diagnostic::error(
                    "CLI_PROJECT_REQUIRED"));
            }
            let json = match arguments.get(2).map(String::as_str) {
                None => false,
                Some("--diagnostic-format=json") => true,
                Some(_) => {
                    return Err(Diagnostic::error(
                        "CLI_WATCH_ARGUMENTS")
                    .with_note("expected: jadpo watch <project> [--diagnostic-format=json]"));
                }
            };
            if arguments.len() > 3 {
                return Err(Diagnostic::error(
                    "CLI_WATCH_ARGUMENTS")
                .with_note("expected: jadpo watch <project> [--diagnostic-format=json]"));
            }
            run_watch(project, json)
        }
        "dev" => {
            if project == Path::new("--diagnostic-format=json") {
                return Err(Diagnostic::error(
                    "CLI_PROJECT_REQUIRED"));
            }
            let json = match arguments.get(2).map(String::as_str) {
                None => false,
                Some("--diagnostic-format=json") => true,
                Some(_) => {
                    return Err(Diagnostic::error(
                        "CLI_DEV_ARGUMENTS")
                    .with_note("expected: jadpo dev <project> [--diagnostic-format=json]"));
                }
            };
            if arguments.len() > 3 {
                return Err(Diagnostic::error(
                    "CLI_DEV_ARGUMENTS")
                .with_note("expected: jadpo dev <project> [--diagnostic-format=json]"));
            }
            run_dev(project, json)
        }
        _other => Err(Diagnostic::error(
            "CLI_UNKNOWN_COMMAND")
        .with_note(
            "expected one of: new, check, inspect, artifacts, build, test, fmt, watch, dev, schema, help",
        )),
    }
}

fn run_schema(arguments: &[String]) -> Result<(), Diagnostic> {
    let subcommand = arguments.get(1).ok_or_else(|| {
        Diagnostic::error(
            "CLI_SCHEMA_COMMAND_REQUIRED")
        .with_note("expected: jadpo schema <init|check|add|rename|snapshot|diff|decision-template|decision-check|plan|sql|index-recommend|index-accept> <project> ...")
    })?;
    let project = arguments
        .get(2)
        .ok_or_else(|| Diagnostic::error("CLI_PROJECT_REQUIRED"))?;
    let project = Path::new(project);
    match subcommand.as_str() {
        "init" => {
            let analyzed = analyze_project(project)?;
            require_valid_frontend(&analyzed)?;
            let path = initialize_schema_identities(project, &analyzed)?;
            println!("initialized schema identities at {}", path.display());
            Ok(())
        }
        "check" => {
            let analyzed = analyze_project(project)?;
            require_valid_frontend(&analyzed)?;
            let path = validate_schema_identities(project, &analyzed)?.ok_or_else(|| {
                Diagnostic::error(
                    "MIG_IDENTITY_REGISTRY_MISSING")
                .with_note("run `jadpo schema init <project>` first")
            })?;
            println!("schema identities match checked source: {}", path.display());
            Ok(())
        }
        "add" => {
            let analyzed = analyze_project(project)?;
            require_valid_frontend(&analyzed)?;
            let (path, count) = register_schema_additions(project, &analyzed)?;
            println!(
                "registered {count} additive schema identity entry(s) in {}",
                path.display()
            );
            Ok(())
        }
        "snapshot" => {
            let output = arguments.get(3).ok_or_else(|| {
                Diagnostic::error(
                    "CLI_SCHEMA_SNAPSHOT_ARGUMENTS")
                .with_note("expected: jadpo schema snapshot <project> <output>")
            })?;
            let analyzed = analyze_project(project)?;
            require_valid_frontend(&analyzed)?;
            let path = snapshot_schema_identities(project, &analyzed, Path::new(output))?;
            println!(
                "wrote immutable schema identity snapshot to {}",
                path.display()
            );
            Ok(())
        }
        "diff" => {
            if arguments.get(3).map(String::as_str) != Some("--against") {
                return Err(Diagnostic::error(
                    "CLI_SCHEMA_DIFF_ARGUMENTS")
                .with_note("expected: jadpo schema diff <project> --against <snapshot>"));
            }
            let previous = arguments.get(4).ok_or_else(|| {
                Diagnostic::error(
                    "CLI_SCHEMA_DIFF_ARGUMENTS")
            })?;
            let analyzed = analyze_project(project)?;
            require_valid_frontend(&analyzed)?;
            print!(
                "{}",
                diff_schema_identities(project, &analyzed, Path::new(previous))?
            );
            Ok(())
        }
        "decision-template" => {
            if arguments.get(3).map(String::as_str) != Some("--against") {
                return Err(Diagnostic::error(
                    "CLI_SCHEMA_DECISION_ARGUMENTS"));
            }
            let previous = arguments.get(4).ok_or_else(|| {
                Diagnostic::error(
                    "CLI_SCHEMA_DECISION_ARGUMENTS")
            })?;
            let output = arguments.get(5).ok_or_else(|| {
                Diagnostic::error(
                    "CLI_SCHEMA_DECISION_ARGUMENTS")
            })?;
            let analyzed = analyze_project(project)?;
            require_valid_frontend(&analyzed)?;
            let (path, count) = write_schema_decision_template(
                project,
                &analyzed,
                Path::new(previous),
                Path::new(output),
            )?;
            println!(
                "wrote {count} unresolved schema decision(s) to {}",
                path.display()
            );
            Ok(())
        }
        "decision-check" => {
            if arguments.get(3).map(String::as_str) != Some("--against") {
                return Err(Diagnostic::error(
                    "CLI_SCHEMA_DECISION_ARGUMENTS"));
            }
            let previous = arguments.get(4).ok_or_else(|| {
                Diagnostic::error(
                    "CLI_SCHEMA_DECISION_ARGUMENTS")
            })?;
            let decisions = arguments.get(5).ok_or_else(|| {
                Diagnostic::error(
                    "CLI_SCHEMA_DECISION_ARGUMENTS")
            })?;
            let analyzed = analyze_project(project)?;
            require_valid_frontend(&analyzed)?;
            print!(
                "{}",
                validate_schema_decisions(
                    project,
                    &analyzed,
                    Path::new(previous),
                    Path::new(decisions),
                )?
            );
            Ok(())
        }
        "plan" => {
            if arguments.get(3).map(String::as_str) != Some("--against")
                || arguments.get(5).map(String::as_str) != Some("--decisions")
                || arguments.get(7).map(String::as_str) != Some("--adapter")
            {
                return Err(Diagnostic::error(
                    "CLI_SCHEMA_PLAN_ARGUMENTS"));
            }
            let previous = &arguments[4];
            let decisions = &arguments[6];
            let adapter = arguments.get(8).ok_or_else(|| {
                Diagnostic::error("CLI_SCHEMA_PLAN_ARGUMENTS")
            })?;
            let output = arguments.get(9).ok_or_else(|| {
                Diagnostic::error(
                    "CLI_SCHEMA_PLAN_ARGUMENTS")
            })?;
            let analyzed = analyze_project(project)?;
            require_valid_frontend(&analyzed)?;
            let (path, steps, irreversible) = write_schema_migration_plan(
                project,
                &analyzed,
                Path::new(previous),
                Path::new(decisions),
                adapter,
                Path::new(output),
            )?;
            println!(
                "wrote non-executable {adapter} migration plan with {steps} step(s), {irreversible} irreversible, to {}",
                path.display()
            );
            Ok(())
        }
        "sql" => {
            if arguments.get(3).map(String::as_str) != Some("--against")
                || arguments.get(5).map(String::as_str) != Some("--decisions")
                || arguments.get(7).map(String::as_str) != Some("--adapter")
            {
                return Err(Diagnostic::error(
                    "CLI_SCHEMA_SQL_ARGUMENTS"));
            }
            let previous = &arguments[4];
            let decisions = &arguments[6];
            let adapter = arguments.get(8).ok_or_else(|| {
                Diagnostic::error("CLI_SCHEMA_SQL_ARGUMENTS")
            })?;
            let output = arguments.get(9).ok_or_else(|| {
                Diagnostic::error(
                    "CLI_SCHEMA_SQL_ARGUMENTS")
            })?;
            let analyzed = analyze_project(project)?;
            require_valid_frontend(&analyzed)?;
            let (path, forward, rollback) = write_schema_migration_sql_review(
                project,
                &analyzed,
                Path::new(previous),
                Path::new(decisions),
                adapter,
                Path::new(output),
            )?;
            println!(
                "wrote non-executable {adapter} SQL review with {forward} forward and {rollback} rollback statement(s) to {}",
                path.display()
            );
            Ok(())
        }
        "index-recommend" => {
            let analyzed = analyze_project(project)?;
            require_valid_frontend(&analyzed)?;
            print!("{}", index_recommendations_json(&analyzed));
            Ok(())
        }
        "index-accept" => {
            let path = arguments.get(3).ok_or_else(|| {
                Diagnostic::error(
                    "CLI_SCHEMA_INDEX_ACCEPT_ARGUMENTS")
            })?;
            let analyzed = analyze_project(project)?;
            require_valid_frontend(&analyzed)?;
            let (source, registry) = accept_index_recommendation(project, &analyzed, path)?;
            println!(
                "accepted index recommendation `{path}` in {}; registered identity in {}",
                source.display(),
                registry.display()
            );
            Ok(())
        }
        "rename" => {
            let kind = arguments.get(3).ok_or_else(|| {
                Diagnostic::error(
                    "CLI_SCHEMA_RENAME_ARGUMENTS")
                .with_note("expected: jadpo schema rename <project> <entity|field> <old> <new>")
            })?;
            let old_path = arguments.get(4).ok_or_else(|| {
                Diagnostic::error(
                    "CLI_SCHEMA_RENAME_ARGUMENTS")
            })?;
            let new_path = arguments.get(5).ok_or_else(|| {
                Diagnostic::error(
                    "CLI_SCHEMA_RENAME_ARGUMENTS")
            })?;
            let analyzed = analyze_project(project)?;
            require_valid_frontend(&analyzed)?;
            let path = rename_schema_identity(project, &analyzed, kind, old_path, new_path)?;
            println!(
                "renamed schema {kind} identity `{old_path}` to `{new_path}` in {}",
                path.display()
            );
            Ok(())
        }
        _other => Err(Diagnostic::error(
            "CLI_UNKNOWN_SCHEMA_COMMAND")
        .with_note(
            "expected: jadpo schema <init|check|add|rename|snapshot|diff|decision-template|decision-check|plan|sql|index-recommend|index-accept> <project> ...",
        )),
    }
}

fn print_help() {
    println!("Jadpo — one language for the whole web");
    println!("https://jadpo.dev/");
    println!();
    println!("jadpo <new|check|inspect|artifacts|build|test|fmt|watch|dev> <project>");
    println!("jadpo incident <project> <event-json-file>");
    println!("jadpo lsp");
    println!("jadpo schema init <project>");
    println!("jadpo schema check <project>");
    println!("jadpo schema add <project>");
    println!("jadpo schema snapshot <project> <output>");
    println!("jadpo schema diff <project> --against <snapshot>");
    println!("jadpo schema decision-template <project> --against <snapshot> <output>");
    println!("jadpo schema decision-check <project> --against <snapshot> <artifact>");
    println!("jadpo schema plan <project> --against <snapshot> --decisions <artifact> --adapter <postgres|sqlite> <output>");
    println!("jadpo schema sql <project> --against <snapshot> --decisions <artifact> --adapter <postgres|sqlite> <output>");
    println!("jadpo schema index-recommend <project>");
    println!("jadpo schema index-accept <project> <Entity.field>");
    println!("jadpo schema rename <project> <entity|field> <old> <new>");
    println!("jadpo check <project> --diagnostic-format=json");
    println!("jadpo watch <project> [--diagnostic-format=json]");
    println!("jadpo dev <project> [--diagnostic-format=json]");
    println!();
    println!("new      create the deterministic static base scaffold");
    println!("check    validate syntax, names, types, failures, and effects");
    println!("         add --diagnostic-format=json for the versioned machine protocol");
    println!("inspect  emit the deterministic checked semantic manifest");
    println!("artifacts  write deterministic derived artifacts beneath build/");
    println!("build    generate the checked Bun target beneath build/");
    println!("test     build and execute authored test blocks with Bun");
    println!("fmt      deterministically format authored .jadpo files (`--check` is read-only)");
    println!("lsp      run the compiler-backed language server over standard input/output");
    println!("watch    rebuild atomically after coalesced authored-input changes");
    println!("dev      watch, run Bun, and restart after successful ready builds");
    println!("incident enrich one secret-safe runtime event from the local compiler graph");
    println!("schema init  create the checked-in persistent schema identity registry");
    println!("schema check  validate registry identity against checked source");
    println!("schema add  register additions only; reject removals and renames");
    println!("schema snapshot  write an immutable canonical checked-shape comparison input");
    println!("schema diff  emit a shape-aware change set and typed decision requirements");
    println!("schema decision-template  bind unresolved decisions to an exact change set");
    println!("schema decision-check  reject stale, incomplete, or invalid decisions");
    println!("schema plan  write an ordered non-executable adapter migration plan");
    println!("schema sql  compile supported changes into reviewed forward/rollback SQL");
    println!("schema index-recommend  report query-backed missing indexes without changing source");
    println!("schema index-accept  add one recommended index to source and schema identity");
    println!("schema rename  preserve identity across an explicit source rename");
}

const DIAGNOSTIC_SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CheckSummary {
    source_files: usize,
    declarations: usize,
    semantic_nodes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CheckReport {
    passed: bool,
    diagnostics: Vec<Diagnostic>,
    summary: Option<CheckSummary>,
}

struct CheckedProject {
    analyzed: AnalyzedProject,
    diagnostics: Vec<Diagnostic>,
    summary: CheckSummary,
}

#[derive(Debug)]
struct BuildSuccess {
    diagnostics: Vec<Diagnostic>,
    summary: CheckSummary,
    artifact_count: usize,
    output: PathBuf,
}

fn run_json_check(arguments: &[String]) -> ExitCode {
    let project = match arguments {
        [command, project, format]
            if command == "check" && format == "--diagnostic-format=json" =>
        {
            Path::new(project)
        }
        [command, format] if command == "check" && format == "--diagnostic-format=json" => {
            let report = failed_report(Diagnostic::error("CLI_PROJECT_REQUIRED"));
            println!("{}", check_report_json(Path::new(""), &report));
            return ExitCode::from(1);
        }
        _ => {
            let report = failed_report(
                Diagnostic::error("CLI_CHECK_ARGUMENTS")
                    .with_note("expected: jadpo check <project> --diagnostic-format=json"),
            );
            println!("{}", check_report_json(Path::new(""), &report));
            return ExitCode::from(1);
        }
    };

    let report = check_project(project);
    println!("{}", check_report_json(project, &report));
    if report.passed {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn run_human_check(project: &Path) -> Result<(), Diagnostic> {
    let mut report = check_project(project);
    if !report.passed {
        let terminal = report
            .diagnostics
            .pop()
            .expect("a failed check report must contain a diagnostic");
        for diagnostic in report.diagnostics {
            print_human_diagnostic(&diagnostic);
        }
        return Err(terminal);
    }

    for diagnostic in &report.diagnostics {
        print_human_diagnostic(diagnostic);
    }
    let summary = report
        .summary
        .expect("a successful check report must contain a summary");
    println!(
        "semantic check passed: {} source file(s), {} declaration(s), {} semantic node(s)",
        summary.source_files, summary.declarations, summary.semantic_nodes
    );
    Ok(())
}

fn check_project(project: &Path) -> CheckReport {
    match checked_project(project) {
        Ok(checked) => CheckReport {
            passed: true,
            diagnostics: checked.diagnostics,
            summary: Some(checked.summary),
        },
        Err(report) => report,
    }
}

fn checked_project(project: &Path) -> Result<CheckedProject, CheckReport> {
    let analyzed = match analyze_project(project) {
        Ok(analyzed) => analyzed,
        Err(diagnostic) => return Err(failed_report(diagnostic)),
    };
    let summary = CheckSummary {
        source_files: analyzed.syntax.sources.len(),
        declarations: analyzed.syntax.declaration_count(),
        semantic_nodes: analyzed.semantics.nodes.len(),
    };
    let source_revision = checked_source_revision(project, &analyzed);

    if let Some(mut diagnostics) = frontend_failure_diagnostics(&analyzed) {
        for diagnostic in &mut diagnostics {
            diagnostic.source_revision.clone_from(&source_revision);
        }
        return Err(CheckReport {
            passed: false,
            diagnostics,
            summary: Some(summary),
        });
    }
    if let Err(mut diagnostic) = validate_schema_identities(project, &analyzed) {
        diagnostic.source_revision.clone_from(&source_revision);
        return Err(CheckReport {
            passed: false,
            diagnostics: vec![diagnostic],
            summary: Some(summary),
        });
    }

    let mut diagnostics = Vec::new();
    let index_recommendations = index_recommendation_count(&analyzed);
    if index_recommendations > 0 {
        diagnostics.push(
            Diagnostic::warning("INDEX_RECOMMENDATION_AVAILABLE").with_note(format!(
                "run `jadpo schema index-recommend {}` for evidence",
                project.display()
            )),
        );
    }
    for diagnostic in &mut diagnostics {
        diagnostic.source_revision.clone_from(&source_revision);
    }

    Ok(CheckedProject {
        analyzed,
        diagnostics,
        summary,
    })
}

fn run_incident_enrichment(project_path: &Path, event_path: &Path) -> Result<(), Diagnostic> {
    let packet = incident_packet(project_path, event_path)?;
    println!(
        "{}",
        serde_json::to_string(&packet).expect("incident packet is serializable")
    );
    Ok(())
}

fn incident_packet(
    project_path: &Path,
    event_path: &Path,
) -> Result<serde_json::Value, Diagnostic> {
    let analyzed = analyze_project(project_path)?;
    require_valid_frontend(&analyzed)?;
    let event_text = fs::read_to_string(event_path)
        .map_err(|_error| Diagnostic::error("CLI_INCIDENT_READ_FAILED"))?;
    let event: serde_json::Value = serde_json::from_str(&event_text)
        .map_err(|_error| Diagnostic::error("CLI_INCIDENT_INVALID"))?;
    if event.get("kind").and_then(serde_json::Value::as_str) != Some("operational_log_event") {
        return Err(Diagnostic::error("CLI_INCIDENT_INVALID"));
    }
    let revision = checked_source_revision(project_path, &analyzed);
    let project_root = if project_path.is_dir() {
        project_path
    } else {
        project_path.parent().unwrap_or_else(|| Path::new("."))
    };
    let manifest_path = project_root.join("build/app.meta.json");
    let manifest_text = fs::read_to_string(manifest_path).map_err(|_error| {
        Diagnostic::error("CLI_INCIDENT_MANIFEST_MISSING")
            .with_note("run `jadpo build` for the exact source revision before enrichment")
    })?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_text)
        .map_err(|_error| Diagnostic::error("CLI_INCIDENT_MANIFEST_INVALID"))?;
    if manifest
        .get("source_revision")
        .and_then(serde_json::Value::as_str)
        != Some(revision.as_str())
    {
        return Err(Diagnostic::error("CLI_INCIDENT_MANIFEST_STALE"));
    }
    let event_revision = event
        .get("sourceRevision")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    if event_revision != revision {
        return Err(Diagnostic::error("CLI_INCIDENT_REVISION_MISMATCH")
            .with_fact(DiagnosticFact::EventRevision(event_revision.to_owned()))
            .with_fact(DiagnosticFact::LocalRevision(revision)));
    }
    let operation = bounded_identifier(&event, "semanticOperationId")?;
    let classification = event
        .get("classification")
        .and_then(serde_json::Value::as_str)
        .filter(|value| matches!(*value, "RUNTIME_UNHANDLED_FAULT" | "RUNTIME_STARTUP_FAILED"))
        .ok_or_else(|| Diagnostic::error("CLI_INCIDENT_INVALID"))?;
    let request_id = event
        .get("requestId")
        .and_then(serde_json::Value::as_str)
        .filter(|value| *value == "startup" || valid_generated_request_id(value))
        .ok_or_else(|| Diagnostic::error("CLI_INCIDENT_INVALID"))?;
    let operation_entry = manifest
        .get("operations")
        .and_then(serde_json::Value::as_array)
        .and_then(|operations| {
            operations.iter().find(|candidate| {
                candidate.get("id").and_then(serde_json::Value::as_str) == Some(operation)
            })
        });
    let (source, range) = if let Some(entry) = operation_entry {
        let source = entry
            .get("source")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("<unknown>")
            .to_owned();
        let start = entry
            .pointer("/range/start")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as usize;
        let end = entry
            .pointer("/range/end")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(start as u64) as usize;
        (source, jadpo_syntax::TextRange::new(start, end))
    } else if operation == "runtime:start" {
        ("<runtime>".to_owned(), jadpo_syntax::TextRange::new(0, 0))
    } else {
        return Err(Diagnostic::error("CLI_INCIDENT_OPERATION_UNKNOWN"));
    };
    let definition = catalogue_definition(classification);
    let packet = serde_json::json!({
        "schemaVersion": 1,
        "kind": "agent_incident_packet",
        "sourceRevision": revision,
        "semanticOperationId": operation,
        "occurrence": {
            "summary": "A secret-safe operational event was reported for this semantic operation.",
            "requestId": request_id,
            "classification": classification
        },
        "location": {
            "source": source,
            "range": { "start": range.start, "end": range.end }
        },
        "ruleId": definition.rule_id,
        "context": {
            "operation": operation,
            "classification": classification
        },
        "impact": {
            "affected": [operation],
            "summary": "The named semantic operation failed; customer values and provider payloads were intentionally omitted."
        },
        "recommendedNextStep": {
            "kind": definition.repair_kind.as_str(),
            "title": definition.recommended_title,
            "decisionOwner": definition.decision_owner.as_str()
        },
        "alternatives": [],
        "helpId": definition.help_id
    });
    Ok(packet)
}

fn bounded_identifier<'a>(event: &'a serde_json::Value, key: &str) -> Result<&'a str, Diagnostic> {
    let value = event
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 256
                && value.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric()
                        || matches!(byte, b'_' | b'-' | b':' | b'/' | b'{' | b'}' | b'.')
                })
        })
        .ok_or_else(|| Diagnostic::error("CLI_INCIDENT_INVALID"))?;
    Ok(value)
}

fn valid_generated_request_id(value: &str) -> bool {
    let Some(uuid) = value.strip_prefix("req_") else {
        return false;
    };
    uuid.len() == 36
        && uuid.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

fn run_human_build(project: &Path) -> Result<(), Diagnostic> {
    match build_project(project) {
        Ok(success) => {
            for diagnostic in &success.diagnostics {
                print_human_diagnostic(diagnostic);
            }
            println!(
                "built {} derived and target file(s) beneath {}",
                success.artifact_count,
                success.output.display()
            );
            Ok(())
        }
        Err(mut report) => {
            let terminal = report
                .diagnostics
                .pop()
                .expect("a failed build must contain a diagnostic");
            for diagnostic in report.diagnostics {
                print_human_diagnostic(&diagnostic);
            }
            Err(terminal)
        }
    }
}

fn run_human_test(project: &Path) -> Result<(), Diagnostic> {
    let success = match build_project(project) {
        Ok(success) => success,
        Err(mut report) => {
            let terminal = report
                .diagnostics
                .pop()
                .expect("a failed test build must contain a diagnostic");
            for diagnostic in report.diagnostics {
                print_human_diagnostic(&diagnostic);
            }
            return Err(terminal);
        }
    };
    for diagnostic in &success.diagnostics {
        print_human_diagnostic(diagnostic);
    }
    let entrypoint = success.output.join("target/tests.ts");
    if !entrypoint.is_file() {
        return Err(Diagnostic::error("TEST_NO_TESTS"));
    }
    let status = Command::new("bun")
        .arg("--no-install")
        .arg(&entrypoint)
        .status()
        .map_err(|_error| Diagnostic::error("TEST_RUNTIME_START_FAILED"))?;
    if status.success() {
        Ok(())
    } else {
        Err(Diagnostic::error("TEST_FAILED"))
    }
}

fn run_format(project: &Path, check: bool) -> Result<(), Diagnostic> {
    let sources = discover_sources(project)?;
    let mut changed = Vec::new();
    for source in sources {
        let formatted = format_source(&source.text);
        if formatted != source.text {
            changed.push(source.path.clone());
            if !check {
                fs::write(&source.path, formatted)
                    .map_err(|_error| Diagnostic::error("FMT_WRITE_FAILED"))?;
            }
        }
    }
    if check && !changed.is_empty() {
        return Err(Diagnostic::error("FMT_CHANGES_REQUIRED").with_note(
            changed
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", "),
        ));
    }
    println!(
        "{} {} source file(s)",
        if check { "checked" } else { "formatted" },
        changed.len()
    );
    Ok(())
}

fn build_project(project: &Path) -> Result<BuildSuccess, CheckReport> {
    let checked = checked_project(project)?;
    let mut outputs = derive_artifacts(project, &checked.analyzed);
    let target = derive_target(project, &checked.analyzed).map_err(|diagnostic| CheckReport {
        passed: false,
        diagnostics: vec![diagnostic],
        summary: Some(checked.summary),
    })?;
    outputs.extend(target);
    let artifact_count = outputs.len();
    let output = write_artifacts(project, &outputs).map_err(|diagnostic| CheckReport {
        passed: false,
        diagnostics: vec![diagnostic],
        summary: Some(checked.summary),
    })?;
    Ok(BuildSuccess {
        diagnostics: checked.diagnostics,
        summary: checked.summary,
        artifact_count,
        output,
    })
}

type InputSnapshot = BTreeMap<PathBuf, u64>;

#[allow(clippy::too_many_arguments)]
fn emit_lifecycle_event(
    json: bool,
    command: &str,
    project: &Path,
    sequence: u64,
    revision: u64,
    event: &str,
    status: &str,
    stale: bool,
    summary: Option<CheckSummary>,
    diagnostics: &[Diagnostic],
    artifact_count: Option<usize>,
    output: Option<&Path>,
) -> Result<(), Diagnostic> {
    if json {
        println!(
            "{}",
            lifecycle_event_json(
                command,
                project,
                sequence,
                revision,
                event,
                status,
                stale,
                summary,
                diagnostics,
                artifact_count,
                output,
            )
        );
    } else {
        println!("{command}[{event}]: revision {revision}, status {status}, stale {stale}");
        for diagnostic in diagnostics {
            print_human_diagnostic(diagnostic);
        }
        if let (Some(count), Some(output)) = (artifact_count, output) {
            println!(
                "{command}[built]: revision {revision}, {count} artifact(s) beneath {}",
                output.display()
            );
        }
    }
    io::stdout()
        .flush()
        .map_err(|_error| Diagnostic::error("CLI_WATCH_OUTPUT_FAILED"))
}

#[allow(clippy::too_many_arguments)]
fn lifecycle_event_json(
    command: &str,
    project: &Path,
    sequence: u64,
    revision: u64,
    event: &str,
    status: &str,
    stale: bool,
    summary: Option<CheckSummary>,
    diagnostics: &[Diagnostic],
    artifact_count: Option<usize>,
    output: Option<&Path>,
) -> String {
    let diagnostics = diagnostics_json(diagnostics);
    let summary = summary_json(summary);
    let artifact_count =
        artifact_count.map_or_else(|| "null".to_owned(), |count| count.to_string());
    let output = output.map_or_else(
        || "null".to_owned(),
        |path| json_string(&path.to_string_lossy()),
    );
    format!(
        "{{\"schemaVersion\":{DIAGNOSTIC_SCHEMA_VERSION},\"kind\":\"lifecycle_event\",\"command\":{},\"sequence\":{sequence},\"revision\":{revision},\"event\":{},\"status\":{},\"project\":{},\"stale\":{stale},\"summary\":{summary},\"artifactCount\":{artifact_count},\"output\":{output},\"diagnostics\":{diagnostics}}}",
        json_string(command),
        json_string(event),
        json_string(status),
        json_string(&project.to_string_lossy()),
    )
}

fn run_watch(project: &Path, json: bool) -> Result<(), Diagnostic> {
    let mut sequence = 0_u64;
    let mut revision = 0_u64;
    let mut has_successful_build = false;
    let mut snapshot = watch_input_snapshot(project).unwrap_or_default();

    loop {
        revision += 1;
        sequence += 1;
        emit_lifecycle_event(
            json,
            "watch",
            project,
            sequence,
            revision,
            "checking",
            "running",
            has_successful_build,
            None,
            &[],
            None,
            None,
        )?;

        match build_project(project) {
            Ok(success) => {
                has_successful_build = true;
                sequence += 1;
                emit_lifecycle_event(
                    json,
                    "watch",
                    project,
                    sequence,
                    revision,
                    "build_succeeded",
                    "succeeded",
                    false,
                    Some(success.summary),
                    &success.diagnostics,
                    Some(success.artifact_count),
                    Some(&success.output),
                )?;
            }
            Err(report) => {
                sequence += 1;
                emit_lifecycle_event(
                    json,
                    "watch",
                    project,
                    sequence,
                    revision,
                    "build_failed",
                    "failed",
                    has_successful_build,
                    report.summary,
                    &report.diagnostics,
                    None,
                    None,
                )?;
            }
        }

        snapshot =
            wait_for_settled_change(project, &snapshot, "watch", json, &mut sequence, revision)?;
    }
}

fn run_dev(project: &Path, json: bool) -> Result<(), Diagnostic> {
    let port = dev_port()?;
    let mut sequence = 0_u64;
    let mut revision = 0_u64;
    let mut runtime: Option<Child> = None;
    let mut has_ready_runtime = false;
    let mut snapshot = watch_input_snapshot(project).unwrap_or_default();

    loop {
        revision += 1;
        sequence += 1;
        emit_lifecycle_event(
            json,
            "dev",
            project,
            sequence,
            revision,
            "checking",
            "running",
            has_ready_runtime,
            None,
            &[],
            None,
            None,
        )?;

        match build_project(project) {
            Err(report) => {
                sequence += 1;
                emit_lifecycle_event(
                    json,
                    "dev",
                    project,
                    sequence,
                    revision,
                    "build_failed",
                    "failed",
                    has_ready_runtime,
                    report.summary,
                    &report.diagnostics,
                    None,
                    None,
                )?;
            }
            Ok(success) => {
                sequence += 1;
                emit_lifecycle_event(
                    json,
                    "dev",
                    project,
                    sequence,
                    revision,
                    "build_succeeded",
                    "succeeded",
                    has_ready_runtime,
                    Some(success.summary),
                    &success.diagnostics,
                    Some(success.artifact_count),
                    Some(&success.output),
                )?;

                if let Some(mut child) = runtime.take() {
                    sequence += 1;
                    emit_lifecycle_event(
                        json,
                        "dev",
                        project,
                        sequence,
                        revision,
                        "runtime_restarting",
                        "running",
                        true,
                        Some(success.summary),
                        &[],
                        None,
                        Some(&success.output),
                    )?;
                    stop_runtime(&mut child);
                    has_ready_runtime = false;
                }

                sequence += 1;
                emit_lifecycle_event(
                    json,
                    "dev",
                    project,
                    sequence,
                    revision,
                    "runtime_starting",
                    "running",
                    false,
                    Some(success.summary),
                    &[],
                    None,
                    Some(&success.output),
                )?;

                match start_bun_runtime(project, port, json) {
                    Ok(mut child) => match wait_until_ready(&mut child, port) {
                        Ok(()) => {
                            has_ready_runtime = true;
                            sequence += 1;
                            emit_lifecycle_event(
                                json,
                                "dev",
                                project,
                                sequence,
                                revision,
                                "runtime_ready",
                                "ready",
                                false,
                                Some(success.summary),
                                &[],
                                None,
                                Some(&success.output),
                            )?;
                            runtime = Some(child);
                        }
                        Err(diagnostic) => {
                            stop_runtime(&mut child);
                            sequence += 1;
                            emit_lifecycle_event(
                                json,
                                "dev",
                                project,
                                sequence,
                                revision,
                                "runtime_failed",
                                "failed",
                                false,
                                Some(success.summary),
                                &[diagnostic],
                                None,
                                Some(&success.output),
                            )?;
                        }
                    },
                    Err(diagnostic) => {
                        sequence += 1;
                        emit_lifecycle_event(
                            json,
                            "dev",
                            project,
                            sequence,
                            revision,
                            "runtime_failed",
                            "failed",
                            false,
                            Some(success.summary),
                            &[diagnostic],
                            None,
                            Some(&success.output),
                        )?;
                    }
                }
            }
        }

        snapshot = wait_for_dev_change(
            project,
            &snapshot,
            json,
            &mut sequence,
            revision,
            &mut runtime,
            &mut has_ready_runtime,
        )?;
    }
}

#[allow(clippy::too_many_arguments)]
fn wait_for_dev_change(
    project: &Path,
    baseline: &InputSnapshot,
    json: bool,
    sequence: &mut u64,
    revision: u64,
    runtime: &mut Option<Child>,
    has_ready_runtime: &mut bool,
) -> Result<InputSnapshot, Diagnostic> {
    loop {
        let exited = if let Some(child) = runtime.as_mut() {
            match child.try_wait() {
                Ok(Some(_status)) => Some(Diagnostic::error("CLI_DEV_RUNTIME_EXITED")),
                Ok(None) => None,
                Err(_error) => Some(Diagnostic::error("CLI_DEV_RUNTIME_STATUS_FAILED")),
            }
        } else {
            None
        };
        if let Some(diagnostic) = exited {
            if let Some(mut child) = runtime.take() {
                stop_runtime(&mut child);
            }
            *has_ready_runtime = false;
            *sequence += 1;
            emit_lifecycle_event(
                json,
                "dev",
                project,
                *sequence,
                revision,
                "runtime_failed",
                "failed",
                false,
                None,
                &[diagnostic],
                None,
                None,
            )?;
        }

        thread::sleep(Duration::from_millis(100));
        let mut candidate = match watch_input_snapshot(project) {
            Ok(snapshot) => snapshot,
            Err(diagnostic) => {
                *sequence += 1;
                emit_lifecycle_event(
                    json,
                    "dev",
                    project,
                    *sequence,
                    revision,
                    "watch_failed",
                    "failed",
                    *has_ready_runtime,
                    None,
                    &[diagnostic],
                    None,
                    None,
                )?;
                continue;
            }
        };
        if &candidate == baseline {
            continue;
        }
        loop {
            thread::sleep(Duration::from_millis(75));
            let next = match watch_input_snapshot(project) {
                Ok(snapshot) => snapshot,
                Err(_) => continue,
            };
            if next == candidate {
                return Ok(candidate);
            }
            candidate = next;
        }
    }
}

fn dev_port() -> Result<u16, Diagnostic> {
    let value = env::var("PORT").unwrap_or_else(|_| "3000".to_owned());
    parse_dev_port(&value)
}

fn parse_dev_port(value: &str) -> Result<u16, Diagnostic> {
    value
        .parse::<u16>()
        .ok()
        .filter(|port| *port > 0)
        .ok_or_else(|| Diagnostic::error("CLI_DEV_PORT_INVALID"))
}

fn start_bun_runtime(project: &Path, port: u16, json: bool) -> Result<Child, Diagnostic> {
    let project_root = if project.is_dir() {
        project
    } else {
        project.parent().unwrap_or_else(|| Path::new("."))
    };
    let target = project_root.join("build/target/app.ts");
    let target = target
        .canonicalize()
        .map_err(|_error| Diagnostic::error("CLI_DEV_TARGET_MISSING"))?;
    let mut command = Command::new("bun");
    command
        .arg("--no-install")
        .arg(&target)
        .current_dir(project_root)
        .env("PORT", port.to_string())
        .stdin(Stdio::null())
        .stderr(Stdio::inherit());
    if json {
        command.stdout(Stdio::piped());
    } else {
        command.stdout(Stdio::inherit());
    }
    let mut child = command
        .spawn()
        .map_err(|_error| Diagnostic::error("CLI_DEV_BUN_START_FAILED"))?;
    if json {
        if let Some(mut output) = child.stdout.take() {
            thread::spawn(move || {
                let _ = io::copy(&mut output, &mut io::stderr());
            });
        }
    }
    Ok(child)
}

fn wait_until_ready(child: &mut Child, port: u16) -> Result<(), Diagnostic> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(_status) = child
            .try_wait()
            .map_err(|_error| Diagnostic::error("CLI_DEV_RUNTIME_STATUS_FAILED"))?
        {
            return Err(Diagnostic::error("CLI_DEV_RUNTIME_EXITED"));
        }
        if healthcheck_ready(port) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(Diagnostic::error("CLI_DEV_READINESS_TIMEOUT"));
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn healthcheck_ready(port: u16) -> bool {
    let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) else {
        return false;
    };
    let timeout = Some(Duration::from_millis(250));
    let _ = stream.set_read_timeout(timeout);
    let _ = stream.set_write_timeout(timeout);
    if stream
        .write_all(b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .is_err()
    {
        return false;
    }

    let mut response = [0_u8; 64];
    let Ok(length) = stream.read(&mut response) else {
        return false;
    };
    response[..length].starts_with(b"HTTP/1.1 200")
        || response[..length].starts_with(b"HTTP/1.0 200")
}

fn stop_runtime(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn wait_for_settled_change(
    project: &Path,
    baseline: &InputSnapshot,
    command: &str,
    json: bool,
    sequence: &mut u64,
    revision: u64,
) -> Result<InputSnapshot, Diagnostic> {
    loop {
        thread::sleep(Duration::from_millis(100));
        let mut candidate = match watch_input_snapshot(project) {
            Ok(snapshot) => snapshot,
            Err(diagnostic) => {
                *sequence += 1;
                emit_lifecycle_event(
                    json,
                    command,
                    project,
                    *sequence,
                    revision,
                    "watch_failed",
                    "failed",
                    true,
                    None,
                    &[diagnostic],
                    None,
                    None,
                )?;
                continue;
            }
        };
        if &candidate == baseline {
            continue;
        }

        loop {
            thread::sleep(Duration::from_millis(75));
            let next = match watch_input_snapshot(project) {
                Ok(snapshot) => snapshot,
                Err(_) => continue,
            };
            if next == candidate {
                return Ok(candidate);
            }
            candidate = next;
        }
    }
}

fn watch_input_snapshot(project: &Path) -> Result<InputSnapshot, Diagnostic> {
    let mut paths = Vec::new();
    if project.is_file() {
        paths.push(project.to_owned());
        if let Some(parent) = project.parent() {
            let registry = parent.join("schema.identities.json");
            if registry.is_file() {
                paths.push(registry);
            }
        }
    } else if project.is_dir() {
        collect_watch_inputs(project, &mut paths)?;
    }
    paths.sort();
    paths.dedup();

    let mut snapshot = BTreeMap::new();
    for path in paths {
        let bytes =
            fs::read(&path).map_err(|_error| Diagnostic::error("CLI_WATCH_INPUT_READ_FAILED"))?;
        snapshot.insert(path, stable_bytes_hash(&bytes));
    }
    Ok(snapshot)
}

fn collect_watch_inputs(path: &Path, output: &mut Vec<PathBuf>) -> Result<(), Diagnostic> {
    let entries =
        fs::read_dir(path).map_err(|_error| Diagnostic::error("CLI_WATCH_INPUT_READ_FAILED"))?;
    for entry in entries {
        let entry = entry.map_err(|_error| Diagnostic::error("CLI_WATCH_INPUT_READ_FAILED"))?;
        let entry_path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if entry_path.is_dir() {
            if name == "build" || name.starts_with(".jadpo-build-") {
                continue;
            }
            collect_watch_inputs(&entry_path, output)?;
        } else if entry_path
            .extension()
            .and_then(|extension| extension.to_str())
            == Some("jadpo")
            || name == "schema.identities.json"
        {
            output.push(entry_path);
        }
    }
    Ok(())
}

fn stable_bytes_hash(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn frontend_failure_diagnostics(project: &AnalyzedProject) -> Option<Vec<Diagnostic>> {
    let stages = [
        project.syntax.diagnostics().cloned().collect::<Vec<_>>(),
        project.semantics.diagnostics.clone(),
        project.typing.diagnostics.clone(),
        project.failures.diagnostics.clone(),
    ];

    for diagnostics in stages {
        if diagnostics.is_empty() {
            continue;
        }
        return Some(diagnostics);
    }
    None
}

fn failed_report(diagnostic: Diagnostic) -> CheckReport {
    CheckReport {
        passed: false,
        diagnostics: vec![diagnostic],
        summary: None,
    }
}

fn check_report_json(project: &Path, report: &CheckReport) -> String {
    let status = if report.passed { "passed" } else { "failed" };
    let summary = summary_json(report.summary);
    let diagnostics = diagnostics_json(&report.diagnostics);
    format!(
        "{{\"schemaVersion\":{DIAGNOSTIC_SCHEMA_VERSION},\"kind\":\"diagnostic_report\",\"command\":\"check\",\"status\":\"{status}\",\"project\":{},\"summary\":{summary},\"diagnostics\":{diagnostics}}}",
        json_string(&project.to_string_lossy()),
    )
}

fn summary_json(summary: Option<CheckSummary>) -> String {
    summary.map_or_else(
        || "null".to_owned(),
        |summary| {
            format!(
                "{{\"source_files\":{},\"declarations\":{},\"semantic_nodes\":{}}}",
                summary.source_files, summary.declarations, summary.semantic_nodes
            )
        },
    )
}

fn diagnostics_json(diagnostics: &[Diagnostic]) -> String {
    let diagnostics = diagnostics
        .iter()
        .map(Diagnostic::to_json)
        .collect::<Vec<_>>()
        .join(",");
    format!("[{diagnostics}]")
}

fn print_human_diagnostic(diagnostic: &Diagnostic) {
    eprint!("{}", human_diagnostic(diagnostic));
}

fn human_diagnostic(diagnostic: &Diagnostic) -> String {
    let mut output = format!("{}: {}\n", diagnostic.severity, diagnostic.message);

    if let Some(primary) = &diagnostic.primary {
        match fs::read_to_string(&primary.source)
            .ok()
            .and_then(|source| source_excerpt(&source, primary.start, primary.end))
        {
            Some(excerpt) => {
                output.push_str(&format!(
                    " --> {}:{}:{}\n",
                    primary.source, excerpt.line, excerpt.column
                ));
                let gutter = excerpt.line.to_string().len();
                output.push_str(&format!("{} |\n", " ".repeat(gutter)));
                output.push_str(&format!("{} | {}\n", excerpt.line, excerpt.text));
                output.push_str(&format!(
                    "{} | {}{}\n",
                    " ".repeat(gutter),
                    " ".repeat(excerpt.column.saturating_sub(1)),
                    "^".repeat(excerpt.width)
                ));
            }
            None => output.push_str(&format!(
                "  at {}:{}..{}\n",
                primary.source, primary.start, primary.end
            )),
        }
    }

    output.push_str(&format!("  reason: {}\n", diagnostic.reason));
    output.push_str(&format!(
        "  next: {} ({}, owner: {})\n",
        diagnostic.recommended_next_step.title,
        diagnostic.recommended_next_step.kind.as_str(),
        diagnostic.decision_owner.as_str()
    ));
    output.push_str(&format!(
        "  rule: {} (legacy: {})\n  help: {}\n",
        diagnostic.rule_id, diagnostic.code, diagnostic.help_id
    ));

    for note in &diagnostic.notes {
        output.push_str(&format!("  note: {note}\n"));
    }
    output
}

struct SourceExcerpt {
    line: usize,
    column: usize,
    width: usize,
    text: String,
}

fn source_excerpt(source: &str, start: usize, end: usize) -> Option<SourceExcerpt> {
    if start > source.len() || !source.is_char_boundary(start) {
        return None;
    }
    let end = end.min(source.len());
    if !source.is_char_boundary(end) {
        return None;
    }
    let line_start = source[..start].rfind('\n').map_or(0, |offset| offset + 1);
    let line_end = source[start..]
        .find('\n')
        .map_or(source.len(), |offset| start + offset);
    let marker_end = end
        .max(start + usize::from(start < source.len()))
        .min(line_end);
    let width = if marker_end > start {
        source[start..marker_end].chars().count().max(1)
    } else {
        1
    };

    Some(SourceExcerpt {
        line: source[..line_start]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count()
            + 1,
        column: source[line_start..start].chars().count() + 1,
        width,
        text: source[line_start..line_end]
            .strip_suffix('\r')
            .unwrap_or(&source[line_start..line_end])
            .to_owned(),
    })
}

fn require_valid_frontend(project: &AnalyzedProject) -> Result<(), Diagnostic> {
    if let Some(mut diagnostics) = frontend_failure_diagnostics(project) {
        let terminal = diagnostics
            .pop()
            .expect("a frontend failure must contain a summary diagnostic");
        for diagnostic in diagnostics {
            print_human_diagnostic(&diagnostic);
        }
        return Err(terminal);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        build_project, check_project, check_report_json, human_diagnostic, incident_packet,
        lifecycle_event_json, parse_dev_port, watch_input_snapshot, CheckReport, CheckSummary,
    };
    use jadpo_diagnostics::{Diagnostic, SourceSpan};
    use std::fs;
    use std::path::Path;

    fn repository_root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("CLI crate should be inside the repository")
    }

    #[test]
    fn emits_a_stable_versioned_check_envelope() {
        let report = CheckReport {
            passed: true,
            diagnostics: vec![
                Diagnostic::warning("TEST_WARNING").with_note("use the evidence command")
            ],
            summary: Some(CheckSummary {
                source_files: 2,
                declarations: 3,
                semantic_nodes: 5,
            }),
        };

        let json = check_report_json(Path::new("example"), &report);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid report JSON");
        assert_eq!(parsed["schemaVersion"], 2);
        assert_eq!(parsed["diagnostics"][0]["ruleId"], "test.warning");
        assert_eq!(parsed["diagnostics"][0]["summary"], "Warning");
        assert_eq!(
            parsed["diagnostics"][0]["recommendedNextStep"]["kind"],
            "guided_choice"
        );
    }

    #[test]
    fn json_check_report_preserves_precise_frontend_diagnostics() {
        let fixture = repository_root().join("tests/compile/fail/52_immutable_reassignment.jadpo");
        let report = check_project(&fixture);

        assert!(!report.passed);
        assert!(report.summary.is_some());
        assert_eq!(
            report
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code)
                .collect::<Vec<_>>(),
            vec![
                "TYPE_ASSIGN_IMMUTABLE",
                "TYPE_ASSIGN_IMMUTABLE",
                "TYPE_MISMATCH"
            ]
        );
        let json = check_report_json(&fixture, &report);
        assert!(json.contains("\"status\":\"failed\""));
        assert!(json.contains("\"location\":{\"source\":"));
        assert!(json.contains("\"range\":{\"start\":132,\"end\":154}"));
    }

    #[test]
    fn empty_project_fails_check() {
        let root =
            std::env::temp_dir().join(format!("jadpo-cli-empty-project-{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).expect("stale fixture should be removable");
        }
        fs::create_dir_all(&root).expect("fixture should be created");
        fs::write(root.join("app.jadpo"), "").expect("empty source should be written");

        let report = check_project(&root);

        assert!(!report.passed);
        assert_eq!(report.diagnostics[0].code, "JADPO_EMPTY_PROJECT");
        fs::remove_dir_all(root).expect("fixture should be removable");
    }

    #[test]
    fn human_diagnostics_show_source_line_column_and_caret() {
        let root =
            std::env::temp_dir().join(format!("jadpo-cli-human-diagnostic-{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).expect("stale fixture should be removable");
        }
        fs::create_dir_all(&root).expect("fixture should be created");
        let source = root.join("app.jadpo");
        fs::write(&source, "type User = {\n    id: UUID\n}\n").expect("source should be written");
        let mut diagnostic =
            Diagnostic::error("TEST_SYNTAX").with_note("use an entity declaration");
        diagnostic.primary = Some(SourceSpan {
            source: source.to_string_lossy().into_owned(),
            start: 12,
            end: 13,
        });

        let rendered = human_diagnostic(&diagnostic);

        assert!(rendered.contains(":1:13\n"));
        assert!(rendered.contains("1 | type User = {"));
        assert!(rendered.contains("  |             ^"));
        assert!(rendered.contains("note: use an entity declaration"));
        fs::remove_dir_all(root).expect("fixture should be removable");
    }

    #[test]
    fn runnable_build_includes_the_compiler_health_route() {
        let root = std::env::temp_dir().join(format!("jadpo-cli-no-routes-{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).expect("stale fixture should be removable");
        }
        fs::create_dir_all(&root).expect("fixture should be created");
        fs::write(root.join("app.jadpo"), "type Name = Text {}\n")
            .expect("source should be written");

        let success = build_project(&root).expect("type-only project should build");
        let target = fs::read_to_string(success.output.join("target/app.ts"))
            .expect("generated target should be readable");

        assert!(target.contains("url.pathname === \"/health\""));
        assert!(target.contains("{ ready: true }"));
        fs::remove_dir_all(root).expect("fixture should be removable");
    }

    #[test]
    fn enriches_safe_runtime_ids_locally_without_copying_event_payloads() {
        let root = std::env::temp_dir().join(format!(
            "jadpo-cli-incident-enrichment-{}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).expect("stale fixture should be removable");
        }
        fs::create_dir_all(&root).expect("fixture should be created");
        fs::write(root.join("app.jadpo"), "type Name = Text {}\n")
            .expect("source should be written");
        build_project(&root).expect("project should build a matching manifest");
        let manifest: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(root.join("build/app.meta.json"))
                .expect("manifest should be readable"),
        )
        .expect("manifest should be JSON");
        let revision = manifest["source_revision"]
            .as_str()
            .expect("manifest should carry source revision");
        let canary = "customer-secret-canary";
        let event_path = root.join("event.json");
        fs::write(
            &event_path,
            serde_json::to_vec(&serde_json::json!({
                "schemaVersion": 1,
                "kind": "operational_log_event",
                "eventName": "operation.failed",
                "classification": "RUNTIME_STARTUP_FAILED",
                "requestId": "startup",
                "traceId": null,
                "semanticOperationId": "runtime:start",
                "sourceRevision": revision,
                "attributes": { "unsafe": canary },
                "rawError": canary
            }))
            .expect("event should serialize"),
        )
        .expect("event should be written");

        let packet = incident_packet(&root, &event_path).expect("event should enrich locally");
        let encoded = serde_json::to_string(&packet).expect("packet should serialize");
        assert!(!encoded.contains(canary));
        assert_eq!(packet["kind"], "agent_incident_packet");
        assert_eq!(packet["semanticOperationId"], "runtime:start");
        assert_eq!(packet["sourceRevision"], revision);
        assert_eq!(packet["location"]["source"], "<runtime>");
        fs::remove_dir_all(root).expect("fixture should be removable");
    }

    #[test]
    fn emits_versioned_watch_lifecycle_events() {
        let diagnostic = Diagnostic::warning("TEST_WARNING");
        let json = lifecycle_event_json(
            "watch",
            Path::new("example"),
            2,
            1,
            "build_succeeded",
            "succeeded",
            false,
            Some(CheckSummary {
                source_files: 1,
                declarations: 2,
                semantic_nodes: 3,
            }),
            &[diagnostic],
            Some(8),
            Some(Path::new("example/build")),
        );

        assert!(json.starts_with(
            "{\"schemaVersion\":2,\"kind\":\"lifecycle_event\",\"command\":\"watch\""
        ));
        assert!(json.contains("\"sequence\":2,\"revision\":1"));
        assert!(json.contains("\"event\":\"build_succeeded\""));
        assert!(json.contains("\"artifactCount\":8"));
        assert!(json.contains("\"output\":\"example/build\""));
        assert!(json.contains("\"ruleId\":\"test.warning\""));
    }

    #[test]
    fn watch_snapshot_tracks_checked_inputs_and_ignores_generated_output() {
        let root = std::env::temp_dir().join(format!("jadpo-cli-watch-{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).expect("stale watch fixture should be removable");
        }
        fs::create_dir_all(root.join("build/target"))
            .expect("watch fixture directories should be created");
        fs::write(root.join("app.jadpo"), "type Name = Text {}\n")
            .expect("source should be written");
        fs::write(root.join("schema.identities.json"), "{}\n").expect("registry should be written");
        fs::write(root.join("build/target/generated.jadpo"), "ignored\n")
            .expect("generated source should be written");

        let first = watch_input_snapshot(&root).expect("snapshot should succeed");
        assert_eq!(first.len(), 2);
        fs::write(root.join("build/target/generated.jadpo"), "still ignored\n")
            .expect("generated source should change");
        assert_eq!(
            watch_input_snapshot(&root).expect("second snapshot should succeed"),
            first
        );
        fs::write(
            root.join("app.jadpo"),
            "type Name = Text { max_length 10 }\n",
        )
        .expect("source should change");
        assert_ne!(
            watch_input_snapshot(&root).expect("changed snapshot should succeed"),
            first
        );

        fs::remove_dir_all(root).expect("watch fixture should be removable");
    }

    #[test]
    fn validates_the_dev_server_port() {
        assert_eq!(parse_dev_port("3000").expect("port should parse"), 3000);
        assert_eq!(
            parse_dev_port("0").expect_err("zero should fail").code,
            "CLI_DEV_PORT_INVALID"
        );
        assert_eq!(
            parse_dev_port("not-a-port")
                .expect_err("text should fail")
                .code,
            "CLI_DEV_PORT_INVALID"
        );
    }
}
