//! DATA-007 project roles: reject misplaced declarations at their own range;
//! keep file-wide entity-count constraints attached to the complete file.
use jadpo_core::analyze_sources;
use jadpo_diagnostics::Diagnostic;
use jadpo_syntax::SourceFile;

const SUPPORT: &str = "entity Anchor { id: Uuid identity: id }\ntype Label = Text {}\n";
const FAILURE: &str = "failure Missing { kind: NotFound code: \"missing\" }";
const ACTION: &str = "action execute() -> Label { return Label(\"ok\") }";
const FUNCTION: &str = "function describe() -> Label { return Label(\"ok\") }";
const ROUTE: &str =
    "route GET /health { auth: none output: Label action: { return Label(\"ok\") } }";
fn diagnostics(files: &[(&str, &str)]) -> Vec<Diagnostic> {
    let project = analyze_sources(
        std::iter::once(SourceFile::new("app.jadpo".into(), SUPPORT.into()))
            .chain(
                files
                    .iter()
                    .map(|(path, source)| SourceFile::new((*path).into(), (*source).into())),
            )
            .collect(),
    )
    .unwrap();
    project
        .syntax
        .diagnostics()
        .chain(project.semantics.diagnostics.iter())
        .chain(project.typing.diagnostics.iter())
        .chain(project.failures.diagnostics.iter())
        .cloned()
        .collect()
}
fn reject_at(path: &str, source: &str, excerpt: &str) {
    let found = diagnostics(&[(path, source)]);
    assert_eq!(
        found.len(),
        1,
        "only the project role should fail: {found:#?}"
    );
    let diagnostic = &found[0];
    assert_eq!(diagnostic.code, "DATA_PROJECT_ROLE_INVALID");
    let span = diagnostic.primary.as_ref().unwrap();
    assert_eq!(span.source, path);
    assert_eq!(&source[span.start..span.end], excerpt);
    assert!(!diagnostic.recommended_next_step.title.is_empty());
}

#[test]
fn misplaced_failure_after_valid_values_highlights_the_failure() {
    let source = format!("type Name = Text {{}}\ntype Amount = Int {{}}\n{FAILURE}");
    reject_at("values/domain.jadpo", &source, FAILURE);
    assert!(diagnostics(&[
        (
            "values/domain.jadpo",
            "type Name = Text {}\ntype Amount = Int {}"
        ),
        ("failures.jadpo", FAILURE)
    ])
    .is_empty());
}
#[test]
fn routes_file_highlights_wrong_callable_after_valid_route() {
    let source = format!("{ROUTE}\n{ACTION}");
    reject_at("routes/health.jadpo", &source, ACTION);
    assert!(diagnostics(&[
        ("routes/health.jadpo", ROUTE),
        ("workflows/execute.jadpo", ACTION)
    ])
    .is_empty());
}
#[test]
fn workflows_file_highlights_wrong_callable_kind_after_valid_action() {
    let source = format!("{ACTION}\n{FUNCTION}");
    reject_at("workflows/execute.jadpo", &source, FUNCTION);
    assert!(diagnostics(&[
        ("workflows/execute.jadpo", ACTION),
        ("helpers.jadpo", FUNCTION)
    ])
    .is_empty());
}
#[test]
fn queries_file_highlights_misplaced_action_after_valid_type() {
    let source = format!("type Name = Text {{}}\n{ACTION}");
    reject_at("queries/names.jadpo", &source, ACTION);
    assert!(diagnostics(&[
        ("queries/names.jadpo", "type Name = Text {}"),
        ("workflows/execute.jadpo", ACTION)
    ])
    .is_empty());
}
#[test]
fn entity_file_highlights_unowned_callable_when_entity_count_is_valid() {
    let source = format!("entity Item {{ id: Uuid identity: id }}\n{ACTION}");
    reject_at("entities/item.jadpo", &source, ACTION);
    assert!(diagnostics(&[
        (
            "entities/item.jadpo",
            "entity Item { id: Uuid identity: id }"
        ),
        ("workflows/execute.jadpo", ACTION)
    ])
    .is_empty());
}
#[test]
fn entity_count_violation_retains_the_whole_file_range() {
    for source in [
        "type Name = Text {}",
        "entity First { id: Uuid identity: id }\nentity Second { id: Uuid identity: id }",
    ] {
        reject_at("entities/item.jadpo", source, source);
    }
}
#[test]
fn multiple_wrong_kinds_keep_one_diagnostic_at_the_first_offender() {
    let source = format!("type Name = Text {{}}\n{FAILURE}\n{ACTION}");
    reject_at("values/domain.jadpo", &source, FAILURE);
}
