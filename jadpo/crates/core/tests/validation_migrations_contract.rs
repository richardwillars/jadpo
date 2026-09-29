//! docs/migration-identity-v0.1.md §§2–6: explicit rename continuity,
//! immutable review artifacts and exact decision bindings. No execution-safety
//! claim is inferred from a generated plan.
use jadpo_core::{
    analyze_project, diff_schema_identities, initialize_schema_identities,
    register_schema_additions, rename_schema_identity, snapshot_schema_identities,
    validate_schema_decisions, validate_schema_identities, write_artifacts,
    write_schema_decision_template, write_schema_migration_plan, write_schema_migration_sql_review,
    AnalyzedProject, GeneratedArtifact,
};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "jadpo-migration-validation-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("fresh test directory");
        Self(path)
    }
    fn source(&self, text: &str) -> AnalyzedProject {
        fs::write(self.0.join("app.jadpo"), text).unwrap();
        let project = analyze_project(&self.0).unwrap();
        let diagnostics: Vec<_> = project
            .syntax
            .diagnostics()
            .chain(project.semantics.diagnostics.iter())
            .chain(project.typing.diagnostics.iter())
            .chain(project.failures.diagnostics.iter())
            .collect();
        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        project
    }
    fn registry(&self) -> String {
        fs::read_to_string(self.0.join("schema.identities.json")).unwrap()
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
const ORIGINAL: &str = "entity Item { id: Text identity label: Text index }";
fn values(source: &str, key: &str) -> Vec<String> {
    // These generated fixtures contain simple ASCII names. Read the named
    // string members independently of the compiler's private registry parser.
    let needle = format!("\"{key}\":\"");
    let mut values: Vec<_> = source
        .split(&needle)
        .skip(1)
        .map(|tail| tail.split('"').next().unwrap().to_owned())
        .collect();
    values.sort();
    values
}
fn addition() -> (Project, AnalyzedProject, PathBuf, PathBuf) {
    let project = Project::new();
    let baseline = project.source(ORIGINAL);
    initialize_schema_identities(&project.0, &baseline).unwrap();
    let snapshot = project.0.join("baseline.json");
    snapshot_schema_identities(&project.0, &baseline, &snapshot).unwrap();
    let added = project.source("entity Item { id: Text identity label: Text index title: Text }");
    register_schema_additions(&project.0, &added).unwrap();
    let decisions = project.0.join("decisions.json");
    let (_, count) =
        write_schema_decision_template(&project.0, &added, &snapshot, &decisions).unwrap();
    assert_eq!(count, 1);
    (project, added, snapshot, decisions)
}
fn resolve(decisions: &PathBuf, expression: &str) {
    let source = fs::read_to_string(decisions).unwrap();
    assert!(source.contains("\"strategy\":null,\"evidence\":[]"));
    fs::write(decisions, source.replace("\"strategy\":null,\"evidence\":[]", &format!("\"strategy\":\"backfill\",\"evidence\":[{{\"kind\":\"typed_expression\",\"value\":\"{expression}\"}}]"))).unwrap();
}

#[test]
fn registry_and_snapshot_recreation_cannot_overwrite_existing_evidence() {
    let project = Project::new();
    let analyzed = project.source(ORIGINAL);
    initialize_schema_identities(&project.0, &analyzed).unwrap();
    let original = project.registry();
    assert_eq!(
        initialize_schema_identities(&project.0, &analyzed)
            .unwrap_err()
            .code,
        "MIG_IDENTITY_REGISTRY_EXISTS"
    );
    assert_eq!(project.registry(), original);
    let snapshot = project.0.join("baseline.json");
    snapshot_schema_identities(&project.0, &analyzed, &snapshot).unwrap();
    let bytes = fs::read(&snapshot).unwrap();
    assert_eq!(
        snapshot_schema_identities(&project.0, &analyzed, &snapshot)
            .unwrap_err()
            .code,
        "MIG_IDENTITY_SNAPSHOT_EXISTS"
    );
    assert_eq!(fs::read(snapshot).unwrap(), bytes);
}

#[test]
fn unmatched_source_rename_cannot_be_laundered_through_schema_add() {
    let project = Project::new();
    let original = project.source(ORIGINAL);
    initialize_schema_identities(&project.0, &original).unwrap();
    let before = project.registry();
    let renamed = project.source(&ORIGINAL.replace("Item", "Renamed"));
    assert_eq!(
        validate_schema_identities(&project.0, &renamed)
            .unwrap_err()
            .code,
        "MIG_IDENTITY_REGISTRY_DRIFT"
    );
    assert_eq!(
        register_schema_additions(&project.0, &renamed)
            .unwrap_err()
            .code,
        "MIG_IDENTITY_ADDITIONS_HAVE_REMOVAL"
    );
    assert_eq!(project.registry(), before);
}

#[test]
fn successive_entity_and_field_renames_preserve_ids_owners_and_physical_names() {
    let project = Project::new();
    let baseline = project.source(ORIGINAL);
    initialize_schema_identities(&project.0, &baseline).unwrap();
    let before = project.registry();
    let snapshot = project.0.join("baseline.json");
    snapshot_schema_identities(&project.0, &baseline, &snapshot).unwrap();
    let renamed = project.source(&ORIGINAL.replace("Item", "Task"));
    rename_schema_identity(&project.0, &renamed, "entity", "Item", "Task").unwrap();
    let field_renamed = project.source("entity Task { id: Text identity title: Text index }");
    rename_schema_identity(
        &project.0,
        &field_renamed,
        "field",
        "Task.label",
        "Task.title",
    )
    .unwrap();
    validate_schema_identities(&project.0, &field_renamed).unwrap();
    let after = project.registry();
    for key in ["id", "owner_id", "physical_name"] {
        assert_eq!(values(&after, key), values(&before, key), "{key}");
    }
    assert!(after.contains("Task.title"));
    assert!(after.contains("\"previous_paths\":[\"Item.label\",\"Task.label\"]"));
    let diff = diff_schema_identities(&project.0, &field_renamed, &snapshot).unwrap();
    assert!(diff.contains("\"change\":\"logical_rename\""));
    assert!(diff.contains("identity_preserved"));
    assert!(!diff.contains("\"change\":\"removed\""));
    assert!(!diff.contains("\"change\":\"added\""));
}

#[test]
fn unresolved_existing_data_decision_produces_neither_plan_nor_sql() {
    let (project, added, snapshot, decisions) = addition();
    let plan = project.0.join("plan.json");
    let sql = project.0.join("review.json");
    assert_eq!(
        write_schema_migration_plan(&project.0, &added, &snapshot, &decisions, "sqlite", &plan)
            .unwrap_err()
            .code,
        "MIG_DECISION_UNRESOLVED"
    );
    assert_eq!(
        write_schema_migration_sql_review(
            &project.0, &added, &snapshot, &decisions, "sqlite", &sql
        )
        .unwrap_err()
        .code,
        "MIG_DECISION_UNRESOLVED"
    );
    assert!(!plan.exists() && !sql.exists());
}

#[test]
fn reviewed_decision_is_invalidated_by_a_later_shape_change() {
    let (project, added, snapshot, decisions) = addition();
    resolve(&decisions, "literal(reviewed)");
    validate_schema_decisions(&project.0, &added, &snapshot, &decisions).unwrap();
    let changed = project.source("entity Item { id: Text identity label: Text index title: Int }");
    let output = project.0.join("stale-review.json");
    assert_eq!(
        validate_schema_decisions(&project.0, &changed, &snapshot, &decisions)
            .unwrap_err()
            .code,
        "MIG_DECISION_CHANGE_SET_STALE"
    );
    assert_eq!(
        write_schema_migration_sql_review(
            &project.0, &changed, &snapshot, &decisions, "sqlite", &output
        )
        .unwrap_err()
        .code,
        "MIG_DECISION_CHANGE_SET_STALE"
    );
    assert!(!output.exists());
}

#[test]
fn complete_decision_does_not_make_unsupported_backfill_executable() {
    let (project, added, snapshot, decisions) = addition();
    resolve(&decisions, "clock.now");
    validate_schema_decisions(&project.0, &added, &snapshot, &decisions).unwrap();
    let plan = project.0.join("plan.json");
    write_schema_migration_plan(&project.0, &added, &snapshot, &decisions, "sqlite", &plan)
        .unwrap();
    let planned = fs::read_to_string(plan).unwrap();
    assert!(planned.contains("\"executable\":false"));
    assert!(planned.contains("\"sql_generated\":false"));
    let output = project.0.join("review.json");
    assert_eq!(
        write_schema_migration_sql_review(
            &project.0, &added, &snapshot, &decisions, "sqlite", &output
        )
        .unwrap_err()
        .code,
        "MIG_SQL_EXPRESSION_UNSUPPORTED"
    );
    assert!(!output.exists());
}

#[test]
fn reviewed_sql_cannot_overwrite_a_previous_review() {
    let (project, added, snapshot, decisions) = addition();
    resolve(&decisions, "literal(reviewed)");
    let output = project.0.join("review.json");
    write_schema_migration_sql_review(&project.0, &added, &snapshot, &decisions, "sqlite", &output)
        .unwrap();
    let before = fs::read(&output).unwrap();
    assert_eq!(
        write_schema_migration_sql_review(
            &project.0, &added, &snapshot, &decisions, "sqlite", &output
        )
        .unwrap_err()
        .code,
        "MIG_SQL_REVIEW_EXISTS"
    );
    assert_eq!(fs::read(output).unwrap(), before);
}

#[test]
fn failed_build_staging_preserves_registry_snapshot_and_previous_complete_tree() {
    let project = Project::new();
    let analyzed = project.source(ORIGINAL);
    initialize_schema_identities(&project.0, &analyzed).unwrap();
    let snapshot = project.0.join("baseline.json");
    snapshot_schema_identities(&project.0, &analyzed, &snapshot).unwrap();
    let registry_before = project.registry();
    let snapshot_before = fs::read(&snapshot).unwrap();
    write_artifacts(
        &project.0,
        &[
            GeneratedArtifact {
                relative_path: "target/app.ts",
                contents: "complete-old-app".into(),
            },
            GeneratedArtifact {
                relative_path: "metadata/a.json",
                contents: "complete-old-metadata".into(),
            },
        ],
    )
    .unwrap();
    let failure = write_artifacts(
        &project.0,
        &[
            GeneratedArtifact {
                relative_path: "target/app.ts",
                contents: "partial-new-app".into(),
            },
            GeneratedArtifact {
                relative_path: "collision/child.txt",
                contents: "new-child".into(),
            },
            GeneratedArtifact {
                relative_path: "collision",
                contents: "cannot-replace-directory".into(),
            },
        ],
    )
    .unwrap_err();
    assert_eq!(failure.code, "JADPO_ARTIFACT_WRITE_FAILED");
    assert_eq!(
        fs::read_to_string(project.0.join("build/target/app.ts")).unwrap(),
        "complete-old-app"
    );
    assert_eq!(
        fs::read_to_string(project.0.join("build/metadata/a.json")).unwrap(),
        "complete-old-metadata"
    );
    assert_eq!(project.registry(), registry_before);
    assert_eq!(fs::read(snapshot).unwrap(), snapshot_before);
    assert!(!project.0.join("build/collision").exists());
    assert!(fs::read_dir(&project.0).unwrap().all(|entry| !entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".jadpo-build-")));
    write_artifacts(
        &project.0,
        &[GeneratedArtifact {
            relative_path: "target/app.ts",
            contents: "complete-new-app".into(),
        }],
    )
    .unwrap();
    assert!(!project.0.join("build/metadata/a.json").exists());
    assert_eq!(
        fs::read_to_string(project.0.join("build/target/app.ts")).unwrap(),
        "complete-new-app"
    );
    assert_eq!(project.registry(), registry_before);
}
