//! Persistence contracts: docs/type-system.md §§3.6,4.2,6; persistence-v0.1
//! generated storage contract. Expected behaviour recorded before target inspection.
use jadpo_core::{analyze_sources, derive_target, GeneratedArtifact};
use jadpo_syntax::SourceFile;
use std::path::Path;

const NULLABLE_STORAGE: &str = r#"
type Template = Object { note: Text? }
type Linked = Object { note: Template.note }
entity Entry {
    id: Uuid
    label: Text
    note: Linked.note
    identity: id
    persistence { store: primary role: authority }
}
"#;
fn targets(source: &str) -> Vec<GeneratedArtifact> {
    let project = analyze_sources(vec![SourceFile::new(
        "persistence-contract.jadpo".into(),
        source.into(),
    )])
    .unwrap();
    let diagnostics = project
        .syntax
        .diagnostics()
        .chain(project.semantics.diagnostics.iter())
        .chain(project.typing.diagnostics.iter())
        .chain(project.failures.diagnostics.iter())
        .collect::<Vec<_>>();
    assert!(
        diagnostics.is_empty(),
        "contract source must check: {diagnostics:#?}"
    );
    derive_target(Path::new("."), &project).expect("supported storage contract should generate")
}
#[test]
fn inherited_nullability_is_reflected_in_both_sql_dialects() {
    let outputs = targets(NULLABLE_STORAGE);
    for dialect in ["sqlite", "postgres"] {
        let artifact = outputs
            .iter()
            .find(|a| a.relative_path == format!("sql/{dialect}/schema.sql"))
            .unwrap();
        let note = artifact
            .contents
            .lines()
            .find(|line| line.trim_start().starts_with("\"note\" "))
            .unwrap();
        assert!(
            !note.contains("NOT NULL"),
            "inherited nullable field was made required in {dialect}: {note}"
        );
        let label = artifact
            .contents
            .lines()
            .find(|line| line.trim_start().starts_with("\"label\" "))
            .unwrap();
        assert!(
            label.contains("NOT NULL"),
            "ordinary required field lost NOT NULL: {label}"
        );
    }
}
#[test]
fn persistence_manifest_records_effective_field_nullability() {
    let outputs = targets(NULLABLE_STORAGE);
    let manifest = &outputs
        .iter()
        .find(|a| a.relative_path == "persistence/entities.json")
        .unwrap()
        .contents;
    assert!(
        manifest.contains(r#""name":"note","type":"Linked.note","nullable":true"#),
        "manifest erased inherited nullability: {manifest}"
    );
    assert!(manifest.contains(r#""name":"label","type":"Text","nullable":false"#));
}

#[test]
fn inherited_nullable_identity_is_rejected_before_sql_generation() {
    let source = "type Template = Object { id: Uuid? }\ntype Linked = Object { id: Template.id }\nentity Entry { id: Linked.id identity: id persistence { store: primary role: authority } }";
    let project = analyze_sources(vec![SourceFile::new(
        "identity-contract.jadpo".into(),
        source.into(),
    )])
    .unwrap();
    let diagnostics = &project.semantics.diagnostics;
    assert_eq!(
        diagnostics.len(),
        1,
        "storage rejection must not cascade: {diagnostics:#?}"
    );
    assert_eq!(
        diagnostics
            .iter()
            .filter(|d| d.code == "DATA_IDENTITY_NULLABLE")
            .count(),
        1,
        "nullable identities must fail once: {diagnostics:#?}"
    );
    let diagnostic = diagnostics
        .iter()
        .find(|d| d.code == "DATA_IDENTITY_NULLABLE")
        .unwrap();
    let span = diagnostic.primary.as_ref().unwrap();
    assert_eq!(&source[span.start..span.end], "id: Linked.id");
}
#[test]
fn inherited_nullable_unique_field_cannot_be_a_foreign_key_target() {
    let source = r#"
type Template = Object { key: Uuid? }
entity Parent {
    id: Uuid
    external: Template.key
    identity: id
    persistence { store: primary role: authority unique: external }
}
entity Child {
    id: Uuid
    parent: Parent.external
    identity: id
    persistence { store: primary role: authority references parent: Parent.external on_delete: restrict }
}
"#;
    let project = analyze_sources(vec![SourceFile::new(
        "reference-contract.jadpo".into(),
        source.into(),
    )])
    .unwrap();
    let syntax = project.syntax.diagnostics().collect::<Vec<_>>();
    assert!(
        syntax.is_empty(),
        "relationship fixture must parse: {syntax:#?}"
    );
    let diagnostics = &project.semantics.diagnostics;
    assert_eq!(
        diagnostics.len(),
        1,
        "storage rejection must not cascade: {diagnostics:#?}"
    );
    assert_eq!(
        diagnostics
            .iter()
            .filter(|d| d.code == "DATA_RELATIONSHIP_TARGET_NOT_KEY")
            .count(),
        1,
        "nullable key target must fail once: {diagnostics:#?}"
    );
}

#[test]
fn checked_local_patch_binding_generates_storage_plans_without_panicking() {
    let source = r#"
entity Entry {
    id: Uuid
    note: Text?
    identity: id
    persistence { store: primary role: authority }
    action patch_entry(input: PatchRequest) fails EntryMissing, PatchEmpty, EntryConflict -> Entry {
        var changes: EntryPatch = input.changes
        return attempt update required Entry {
            where: id == input.id patch: changes empty: PatchEmpty missing: EntryMissing conflict: EntryConflict
        }
    }
}
type EntryPatch = Object { note: Entry.note optional }
type PatchRequest = Object { id: Entry.id changes: EntryPatch }
failure EntryMissing { kind: NotFound code: "entry_missing" }
failure PatchEmpty { kind: InvalidValue code: "patch_empty" }
failure EntryConflict { kind: Conflict code: "entry_conflict" }
"#;
    let outputs = targets(source);
    let persistence = &outputs
        .iter()
        .find(|a| a.relative_path == "target/persistence.ts")
        .unwrap()
        .contents;
    assert!(
        persistence.contains("CASE WHEN"),
        "local patch needs the same omission-aware SQL contract as parameter patches"
    );
}
