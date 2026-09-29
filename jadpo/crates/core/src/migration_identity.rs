use crate::AnalyzedProject;
use jadpo_diagnostics::Diagnostic;
use jadpo_syntax::{
    ConstraintKind, Declaration, FieldDeclaration, PersistenceModifier, RecordKind,
    ReferenceDeleteAction, TypeReference,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct RegistryEntry {
    id: String,
    kind: String,
    path: String,
    physical_name: String,
    owner_id: Option<String>,
    previous_paths: Vec<String>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SchemaSnapshotEntry {
    id: String,
    kind: String,
    path: String,
    physical_name: String,
    owner_id: Option<String>,
    shape: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SchemaChange {
    identity: String,
    change: &'static str,
    disposition: &'static str,
    before: Option<SchemaSnapshotEntry>,
    after: Option<SchemaSnapshotEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecisionEvidence {
    kind: String,
    value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SchemaDecision {
    identity: String,
    change: String,
    strategy: Option<String>,
    evidence: Vec<DecisionEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SchemaDecisionArtifact {
    change_set: String,
    decisions: Vec<SchemaDecision>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MigrationAdapter {
    Postgres,
    Sqlite,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MigrationPlanStep {
    identity: String,
    change: &'static str,
    operation: &'static str,
    decision_strategy: Option<String>,
    evidence: Vec<DecisionEvidence>,
    rollback_kind: &'static str,
    rollback_operation: &'static str,
    irreversible: bool,
}

type DecisionStrategy = (&'static str, Vec<&'static str>);
type DecisionStrategies = (&'static str, Vec<DecisionStrategy>);

pub fn initialize_schema_identities(
    project: &Path,
    analyzed: &AnalyzedProject,
) -> Result<PathBuf, Diagnostic> {
    let project_root = if project.is_dir() {
        project
    } else {
        project.parent().unwrap_or_else(|| Path::new("."))
    };
    let registry_path = project_root.join("schema.identities.json");
    if registry_path.exists() {
        return Err(Diagnostic::error("MIG_IDENTITY_REGISTRY_EXISTS")
            .with_note("schema init never overwrites or regenerates persistent identities"));
    }

    let contents = registry_json(&derive_registry_entries(analyzed));
    fs::write(&registry_path, contents)
        .map_err(|_error| Diagnostic::error("MIG_IDENTITY_REGISTRY_WRITE_FAILED"))?;
    Ok(registry_path)
}

pub fn validate_schema_identities(
    project: &Path,
    analyzed: &AnalyzedProject,
) -> Result<Option<PathBuf>, Diagnostic> {
    let path = schema_registry_path(project);
    if !path.exists() {
        return Ok(None);
    }
    let source = read_registry_source(&path)?;
    let entries = parse_registry(&source)?;
    validate_registry_entries(&entries, &derive_registry_entries(analyzed))?;
    if registry_json(&entries) != source {
        return Err(Diagnostic::error("MIG_IDENTITY_REGISTRY_NOT_CANONICAL")
            .with_note("registry files are compiler-owned; use schema commands to modify them"));
    }
    Ok(Some(path))
}

pub fn rename_schema_identity(
    project: &Path,
    analyzed: &AnalyzedProject,
    kind: &str,
    old_path: &str,
    new_path: &str,
) -> Result<PathBuf, Diagnostic> {
    if !matches!(kind, "entity" | "field") {
        return Err(Diagnostic::error("MIG_IDENTITY_RENAME_KIND"));
    }
    let path = schema_registry_path(project);
    if !path.exists() {
        return Err(Diagnostic::error("MIG_IDENTITY_REGISTRY_MISSING")
            .with_note("run `jadpo schema init <project>` before tracking renames"));
    }
    let source = read_registry_source(&path)?;
    let mut entries = parse_registry(&source)?;
    validate_registry_structure(&entries)?;

    if !entries
        .iter()
        .any(|entry| entry.kind == kind && entry.path == old_path)
    {
        return Err(Diagnostic::error("MIG_IDENTITY_RENAME_SOURCE_UNKNOWN"));
    }
    let live = derive_registry_entries(analyzed);
    if !live
        .iter()
        .any(|entry| entry.kind == kind && entry.path == new_path)
    {
        return Err(Diagnostic::error("MIG_IDENTITY_RENAME_TARGET_UNKNOWN")
            .with_note("rename the source declaration before updating its registry identity"));
    }
    if entries
        .iter()
        .any(|entry| entry.kind == kind && entry.path == new_path)
    {
        return Err(Diagnostic::error("MIG_IDENTITY_RENAME_TARGET_EXISTS"));
    }

    for entry in &mut entries {
        let replacement = if kind == "entity" {
            entry
                .path
                .strip_prefix(old_path)
                .filter(|suffix| suffix.is_empty() || suffix.starts_with('.'))
                .map(|suffix| format!("{new_path}{suffix}"))
        } else if entry.path == old_path {
            Some(new_path.to_owned())
        } else {
            entry
                .path
                .strip_prefix(old_path)
                .filter(|suffix| suffix.starts_with('#'))
                .map(|suffix| format!("{new_path}{suffix}"))
        };
        if let Some(replacement) = replacement {
            entry.previous_paths.push(entry.path.clone());
            entry.path = replacement;
        }
    }
    entries.sort_by(|left, right| left.id.cmp(&right.id));
    validate_registry_entries(&entries, &live)?;
    fs::write(&path, registry_json(&entries))
        .map_err(|_error| Diagnostic::error("MIG_IDENTITY_REGISTRY_WRITE_FAILED"))?;
    Ok(path)
}

pub fn register_schema_additions(
    project: &Path,
    analyzed: &AnalyzedProject,
) -> Result<(PathBuf, usize), Diagnostic> {
    let path = schema_registry_path(project);
    if !path.exists() {
        return Err(Diagnostic::error("MIG_IDENTITY_REGISTRY_MISSING")
            .with_note("run `jadpo schema init <project>` before registering additions"));
    }
    let source = read_registry_source(&path)?;
    let mut entries = parse_registry(&source)?;
    validate_registry_structure(&entries)?;
    let live = derive_registry_entries(analyzed);
    let live_keys = live
        .iter()
        .map(|entry| (entry.kind.clone(), entry.path.clone()))
        .collect::<BTreeSet<_>>();
    if let Some(_stale) = entries
        .iter()
        .find(|entry| !live_keys.contains(&(entry.kind.clone(), entry.path.clone())))
    {
        return Err(Diagnostic::error("MIG_IDENTITY_ADDITIONS_HAVE_REMOVAL")
            .with_note("resolve renames or lifecycle removals before registering additions"));
    }

    let mut registered_keys = entries
        .iter()
        .map(|entry| (entry.kind.clone(), entry.path.clone()))
        .collect::<BTreeSet<_>>();
    let mut added = 0;
    for entry in live {
        if registered_keys.insert((entry.kind.clone(), entry.path.clone())) {
            entries.push(entry);
            added += 1;
        }
    }

    let entity_ids = entries
        .iter()
        .filter(|entry| entry.kind == "entity")
        .map(|entry| (entry.path.clone(), entry.id.clone()))
        .collect::<BTreeMap<_, _>>();
    for entry in entries.iter_mut().filter(|entry| entry.kind != "entity") {
        let entity_path = entry.path.split('.').next().unwrap_or_default();
        entry.owner_id = entity_ids.get(entity_path).cloned();
    }
    entries.sort_by(|left, right| left.id.cmp(&right.id));
    validate_registry_entries(&entries, &derive_registry_entries(analyzed))?;
    fs::write(&path, registry_json(&entries))
        .map_err(|_error| Diagnostic::error("MIG_IDENTITY_REGISTRY_WRITE_FAILED"))?;
    Ok((path, added))
}

pub fn snapshot_schema_identities(
    project: &Path,
    analyzed: &AnalyzedProject,
    output: &Path,
) -> Result<PathBuf, Diagnostic> {
    let current = validate_schema_identities(project, analyzed)?.ok_or_else(|| {
        Diagnostic::error("MIG_IDENTITY_REGISTRY_MISSING")
            .with_note("run `jadpo schema init <project>` before taking a snapshot")
    })?;
    if output.exists() {
        return Err(Diagnostic::error("MIG_IDENTITY_SNAPSHOT_EXISTS")
            .with_note("schema snapshots are immutable comparison inputs"));
    }
    let registry = parse_registry(&read_registry_source(&current)?)?;
    let snapshot = derive_schema_snapshot_entries(analyzed, &registry)?;
    fs::write(output, schema_snapshot_json(&snapshot))
        .map_err(|_error| Diagnostic::error("MIG_IDENTITY_SNAPSHOT_WRITE_FAILED"))?;
    Ok(output.to_owned())
}

pub fn diff_schema_identities(
    project: &Path,
    analyzed: &AnalyzedProject,
    previous: &Path,
) -> Result<String, Diagnostic> {
    Ok(schema_change_set_json(&checked_schema_changes(
        project, analyzed, previous,
    )?))
}

pub fn write_schema_decision_template(
    project: &Path,
    analyzed: &AnalyzedProject,
    previous: &Path,
    output: &Path,
) -> Result<(PathBuf, usize), Diagnostic> {
    if output.exists() {
        return Err(Diagnostic::error("MIG_DECISION_ARTIFACT_EXISTS")
            .with_note("decision templates never overwrite authored decisions"));
    }
    let changes = checked_schema_changes(project, analyzed, previous)?;
    let change_set = schema_change_set_json(&changes);
    let decisions = required_decision_templates(&changes);
    fs::write(
        output,
        schema_decision_artifact_json(&change_set, &decisions),
    )
    .map_err(|_error| Diagnostic::error("MIG_DECISION_ARTIFACT_WRITE_FAILED"))?;
    Ok((output.to_owned(), decisions.len()))
}

pub fn validate_schema_decisions(
    project: &Path,
    analyzed: &AnalyzedProject,
    previous: &Path,
    decisions_path: &Path,
) -> Result<String, Diagnostic> {
    let (_, artifact) = checked_schema_decisions(project, analyzed, previous, decisions_path)?;
    Ok(format!(
        "{{\"schema_version\":1,\"kind\":\"schema_decision_validation\",\"change_set_bound\":true,\"complete\":true,\"migration_plan\":false,\"decisions\":{}}}\n",
        artifact.decisions.len()
    ))
}

fn checked_schema_decisions(
    project: &Path,
    analyzed: &AnalyzedProject,
    previous: &Path,
    decisions_path: &Path,
) -> Result<(Vec<SchemaChange>, SchemaDecisionArtifact), Diagnostic> {
    let changes = checked_schema_changes(project, analyzed, previous)?;
    let expected_change_set = schema_change_set_json(&changes);
    let artifact_source = fs::read_to_string(decisions_path)
        .map_err(|_error| Diagnostic::error("MIG_DECISION_ARTIFACT_READ_FAILED"))?;
    let artifact = parse_schema_decision_artifact(&artifact_source)?;
    if artifact.change_set != expected_change_set {
        return Err(Diagnostic::error("MIG_DECISION_CHANGE_SET_STALE")
            .with_note("regenerate the decision template and review every changed requirement"));
    }

    let required = changes
        .iter()
        .filter(|change| decision_strategies(change.change, change.disposition).is_some())
        .map(|change| ((change.identity.as_str(), change.change), change))
        .collect::<BTreeMap<_, _>>();
    let mut supplied = BTreeSet::new();
    for decision in &artifact.decisions {
        let key = (decision.identity.as_str(), decision.change.as_str());
        if !supplied.insert(key) {
            return Err(Diagnostic::error("MIG_DECISION_DUPLICATE"));
        }
        let Some(change) = required.get(&key).copied() else {
            return Err(Diagnostic::error("MIG_DECISION_UNEXPECTED"));
        };
        validate_schema_decision(change, decision)?;
    }
    if let Some(((_identity, _change), _)) =
        required.iter().find(|(key, _)| !supplied.contains(*key))
    {
        return Err(Diagnostic::error("MIG_DECISION_MISSING"));
    }
    Ok((changes, artifact))
}

pub fn write_schema_migration_plan(
    project: &Path,
    analyzed: &AnalyzedProject,
    previous: &Path,
    decisions_path: &Path,
    adapter: &str,
    output: &Path,
) -> Result<(PathBuf, usize, usize), Diagnostic> {
    if output.exists() {
        return Err(Diagnostic::error("MIG_PLAN_EXISTS")
            .with_note("migration plans are immutable review artifacts"));
    }
    let adapter = MigrationAdapter::parse(adapter)?;
    let (changes, artifact) =
        checked_schema_decisions(project, analyzed, previous, decisions_path)?;
    let steps = migration_plan_steps(&changes, &artifact.decisions, adapter)?;
    let irreversible = steps.iter().filter(|step| step.irreversible).count();
    let plan = schema_migration_plan_json(adapter, &schema_change_set_json(&changes), &steps);
    fs::write(output, plan).map_err(|_error| Diagnostic::error("MIG_PLAN_WRITE_FAILED"))?;
    Ok((output.to_owned(), steps.len(), irreversible))
}

pub fn write_schema_migration_sql_review(
    project: &Path,
    analyzed: &AnalyzedProject,
    previous: &Path,
    decisions_path: &Path,
    adapter: &str,
    output: &Path,
) -> Result<(PathBuf, usize, usize), Diagnostic> {
    if output.exists() {
        return Err(Diagnostic::error("MIG_SQL_REVIEW_EXISTS")
            .with_note("migration SQL reviews are immutable artifacts"));
    }
    let adapter = MigrationAdapter::parse(adapter)?;
    let (changes, artifact) =
        checked_schema_decisions(project, analyzed, previous, decisions_path)?;
    let registry_path = schema_registry_path(project);
    let registry = parse_registry(&read_registry_source(&registry_path)?)?;
    let (forward, rollback) =
        migration_sql(analyzed, &changes, &artifact.decisions, &registry, adapter)?;
    let contents = schema_migration_sql_review_json(
        adapter,
        &schema_change_set_json(&changes),
        &forward,
        &rollback,
    );
    fs::write(output, contents)
        .map_err(|_error| Diagnostic::error("MIG_SQL_REVIEW_WRITE_FAILED"))?;
    Ok((output.to_owned(), forward.len(), rollback.len()))
}

impl MigrationAdapter {
    fn parse(value: &str) -> Result<Self, Diagnostic> {
        match value {
            "postgres" => Ok(Self::Postgres),
            "sqlite" => Ok(Self::Sqlite),
            _ => Err(Diagnostic::error("MIG_PLAN_ADAPTER_INVALID")
                .with_note("expected `postgres` or `sqlite`")),
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
            Self::Sqlite => "sqlite",
        }
    }
}

fn checked_schema_changes(
    project: &Path,
    analyzed: &AnalyzedProject,
    previous: &Path,
) -> Result<Vec<SchemaChange>, Diagnostic> {
    let current_path = validate_schema_identities(project, analyzed)?.ok_or_else(|| {
        Diagnostic::error("MIG_IDENTITY_REGISTRY_MISSING")
            .with_note("run `jadpo schema init <project>` before comparing identities")
    })?;
    if !previous.exists() {
        return Err(Diagnostic::error("MIG_IDENTITY_SNAPSHOT_MISSING"));
    }
    let current_registry = parse_registry(&read_registry_source(&current_path)?)?;
    let current = derive_schema_snapshot_entries(analyzed, &current_registry)?;
    let previous_source = read_registry_source(previous)?;
    let previous_entries = parse_schema_snapshot(&previous_source)?;
    if schema_snapshot_json(&previous_entries) != previous_source {
        return Err(Diagnostic::error("MIG_IDENTITY_SNAPSHOT_NOT_CANONICAL")
            .with_note("create snapshots with `jadpo schema snapshot`"));
    }
    Ok(schema_changes(&previous_entries, &current))
}

fn schema_changes(
    previous: &[SchemaSnapshotEntry],
    current: &[SchemaSnapshotEntry],
) -> Vec<SchemaChange> {
    let previous = previous
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let current = current
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let identities = previous
        .keys()
        .chain(current.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    let mut changes = Vec::new();
    for identity in identities {
        let before = previous.get(identity).copied();
        let after = current.get(identity).copied();
        let (change, disposition) = match (before, after) {
            (None, Some(after)) if after.kind == "field" && field_shape_nullable(&after.shape) => {
                ("added_nullable_field", "requires_schema_plan")
            }
            (None, Some(after)) if after.kind == "field" => {
                ("added_required_field", "requires_existing_data_decision")
            }
            (None, Some(_)) => ("added", "requires_shape_analysis"),
            (Some(_), None) => ("removed", "requires_lifecycle_decision"),
            (Some(before), Some(after))
                if before.kind != after.kind || before.owner_id != after.owner_id =>
            {
                (
                    "incompatible_identity_change",
                    "requires_lifecycle_decision",
                )
            }
            (Some(before), Some(after))
                if before.shape != after.shape
                    && reference_delete_action(&before.shape)
                        != reference_delete_action(&after.shape) =>
            {
                (
                    "relationship_lifecycle_changed",
                    "requires_lifecycle_decision",
                )
            }
            (Some(before), Some(after))
                if after.kind == "field"
                    && field_shape_is_nullability_widening(&before.shape, &after.shape) =>
            {
                ("field_nullability_widened", "requires_schema_plan")
            }
            (Some(before), Some(after))
                if after.kind == "field"
                    && field_shape_is_nullability_narrowing(&before.shape, &after.shape) =>
            {
                (
                    "field_nullability_narrowed",
                    "requires_existing_data_decision",
                )
            }
            (Some(before), Some(after)) if before.shape != after.shape => {
                if after.kind == "field" {
                    ("field_shape_changed", "requires_existing_data_decision")
                } else {
                    ("schema_shape_changed", "requires_schema_plan")
                }
            }
            (Some(before), Some(after))
                if before.path != after.path && before.physical_name != after.physical_name =>
            {
                ("logical_and_physical_rename", "requires_schema_plan")
            }
            (Some(before), Some(after)) if before.path != after.path => {
                ("logical_rename", "identity_preserved")
            }
            (Some(before), Some(after)) if before.physical_name != after.physical_name => {
                ("physical_rename", "requires_schema_plan")
            }
            (Some(_), Some(_)) => continue,
            (None, None) => unreachable!(),
        };
        changes.push(SchemaChange {
            identity: identity.to_owned(),
            change,
            disposition,
            before: before.cloned(),
            after: after.cloned(),
        });
    }
    changes
}

fn schema_change_set_json(changes: &[SchemaChange]) -> String {
    let changes = changes
        .iter()
        .map(|change| format!(
            "{{\"identity\":{},\"change\":{},\"disposition\":{},\"decision_requirement\":{},\"before\":{},\"after\":{}}}",
            json_string(&change.identity),
            json_string(change.change),
            json_string(change.disposition),
            decision_requirement_json(change.change, change.disposition),
            change.before.as_ref().map_or_else(|| "null".to_owned(), snapshot_entry_state_json),
            change.after.as_ref().map_or_else(|| "null".to_owned(), snapshot_entry_state_json)
        ))
        .collect::<Vec<_>>();
    format!(
        "{{\"schema_version\":1,\"kind\":\"schema_change_set\",\"migration_plan\":false,\"changes\":[{}]}}\n",
        changes.join(",")
    )
}

fn snapshot_entry_state_json(entry: &SchemaSnapshotEntry) -> String {
    format!(
        "{{\"kind\":{},\"path\":{},\"physical_name\":{},\"owner_id\":{},\"shape\":{}}}",
        json_string(&entry.kind),
        json_string(&entry.path),
        json_string(&entry.physical_name),
        entry
            .owner_id
            .as_ref()
            .map_or_else(|| "null".to_owned(), |owner| json_string(owner)),
        json_string(&entry.shape)
    )
}

fn field_shape_nullable(shape: &str) -> bool {
    shape.contains("\"nullable\":true")
}

fn field_shape_is_nullability_widening(before: &str, after: &str) -> bool {
    if field_shape_nullable(before) || !field_shape_nullable(after) {
        return false;
    }
    let Some(before_type) = field_shape_type(before) else {
        return false;
    };
    let Some(after_type) = field_shape_type(after) else {
        return false;
    };
    if after_type.strip_suffix('?') != Some(before_type) {
        return false;
    }
    after
        .replacen(
            &format!("\"type\":\"{after_type}\""),
            &format!("\"type\":\"{before_type}\""),
            1,
        )
        .replacen("\"nullable\":true", "\"nullable\":false", 1)
        == before
}

fn field_shape_is_nullability_narrowing(before: &str, after: &str) -> bool {
    field_shape_is_nullability_widening(after, before)
}

fn reference_delete_action(shape: &str) -> Option<&str> {
    let value = shape.split_once("\"on_delete\":\"")?.1;
    value.split_once('"').map(|(action, _)| action)
}

fn decision_requirement_json(change: &str, disposition: &str) -> String {
    let Some((kind, strategies)) = decision_strategies(change, disposition) else {
        return "null".to_owned();
    };
    let strategies = strategies
        .iter()
        .map(|(name, required)| {
            format!(
                "{{\"name\":{},\"requires\":[{}]}}",
                json_string(name),
                required
                    .iter()
                    .map(|item| json_string(item))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"kind\":{},\"status\":\"unresolved\",\"strategies\":[{strategies}]}}",
        json_string(kind)
    )
}

fn decision_strategies(change: &str, disposition: &str) -> Option<DecisionStrategies> {
    match disposition {
        "requires_existing_data_decision" if change == "added_required_field" => Some((
            "existing_data",
            vec![
                ("reject_if_rows", vec!["row_check"]),
                ("backfill", vec!["typed_expression"]),
                ("staged_transition", vec!["ordered_steps"]),
            ],
        )),
        "requires_existing_data_decision" if change == "field_nullability_narrowed" => Some((
            "existing_data",
            vec![
                ("validate_existing", vec!["typed_predicate"]),
                ("staged_transition", vec!["ordered_steps"]),
            ],
        )),
        "requires_existing_data_decision" => Some((
            "existing_data",
            vec![
                ("validate_existing", vec!["typed_predicate"]),
                ("transform", vec!["typed_expression"]),
                ("staged_transition", vec!["ordered_steps"]),
            ],
        )),
        "requires_lifecycle_decision" => Some((
            "lifecycle",
            vec![
                ("reject", vec![]),
                ("retain", vec!["retention_contract"]),
                ("anonymise", vec!["typed_expression"]),
                ("destructive_remove", vec!["approval_reference"]),
            ],
        )),
        _ => None,
    }
}

fn required_decision_templates(changes: &[SchemaChange]) -> Vec<SchemaDecision> {
    changes
        .iter()
        .filter(|change| decision_strategies(change.change, change.disposition).is_some())
        .map(|change| SchemaDecision {
            identity: change.identity.clone(),
            change: change.change.to_owned(),
            strategy: None,
            evidence: Vec::new(),
        })
        .collect()
}

fn schema_decision_artifact_json(change_set: &str, decisions: &[SchemaDecision]) -> String {
    let decisions = decisions
        .iter()
        .map(|decision| {
            let evidence = decision
                .evidence
                .iter()
                .map(|item| {
                    format!(
                        "{{\"kind\":{},\"value\":{}}}",
                        json_string(&item.kind),
                        json_string(&item.value)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"identity\":{},\"change\":{},\"strategy\":{},\"evidence\":[{evidence}]}}",
                json_string(&decision.identity),
                json_string(&decision.change),
                decision
                    .strategy
                    .as_ref()
                    .map_or_else(|| "null".to_owned(), |strategy| json_string(strategy))
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema_version\":1,\"kind\":\"schema_decisions\",\"change_set\":{},\"decisions\":[{decisions}]}}\n",
        json_string(change_set)
    )
}

fn validate_schema_decision(
    change: &SchemaChange,
    decision: &SchemaDecision,
) -> Result<(), Diagnostic> {
    let strategy = decision
        .strategy
        .as_deref()
        .ok_or_else(|| Diagnostic::error("MIG_DECISION_UNRESOLVED"))?;
    let (_, strategies) = decision_strategies(change.change, change.disposition)
        .expect("validation only visits changes requiring a decision");
    let required = strategies
        .iter()
        .find(|(name, _)| *name == strategy)
        .map(|(_, required)| required)
        .ok_or_else(|| Diagnostic::error("MIG_DECISION_STRATEGY_INVALID"))?;
    let mut supplied = BTreeMap::new();
    for evidence in &decision.evidence {
        if evidence.value.trim().is_empty() {
            return Err(Diagnostic::error("MIG_DECISION_EVIDENCE_EMPTY"));
        }
        if supplied.insert(evidence.kind.as_str(), evidence).is_some() {
            return Err(Diagnostic::error("MIG_DECISION_EVIDENCE_DUPLICATE"));
        }
    }
    if let Some(_kind) = required.iter().find(|kind| !supplied.contains_key(**kind)) {
        return Err(Diagnostic::error("MIG_DECISION_EVIDENCE_MISSING"));
    }
    if let Some(_kind) = supplied.keys().find(|kind| !required.contains(kind)) {
        return Err(Diagnostic::error("MIG_DECISION_EVIDENCE_UNEXPECTED"));
    }
    Ok(())
}

fn migration_plan_steps(
    changes: &[SchemaChange],
    decisions: &[SchemaDecision],
    adapter: MigrationAdapter,
) -> Result<Vec<MigrationPlanStep>, Diagnostic> {
    let decisions = decisions
        .iter()
        .map(|decision| {
            (
                (decision.identity.as_str(), decision.change.as_str()),
                decision,
            )
        })
        .collect::<BTreeMap<_, _>>();
    changes
        .iter()
        .map(|change| {
            let decision = decisions
                .get(&(change.identity.as_str(), change.change))
                .copied();
            if decision.and_then(|item| item.strategy.as_deref()) == Some("reject") {
                return Err(Diagnostic::error("MIG_PLAN_DECISION_REJECTS_CHANGE"));
            }
            let strategy = decision.and_then(|item| item.strategy.as_deref());
            let operation = migration_operation(change, strategy, adapter);
            let irreversible = matches!(
                strategy,
                Some("anonymise" | "destructive_remove" | "transform")
            );
            let (rollback_kind, rollback_operation) = if irreversible {
                ("unavailable", "manual_restore_from_verified_backup")
            } else if change.change == "logical_rename" {
                ("automatic", "restore_previous_registry_path")
            } else if matches!(strategy, Some("retain")) {
                ("automatic", "restore_source_declaration")
            } else {
                (
                    "conditional",
                    "reverse_before_dependent_writes_or_restore_backup",
                )
            };
            Ok(MigrationPlanStep {
                identity: change.identity.clone(),
                change: change.change,
                operation,
                decision_strategy: strategy.map(str::to_owned),
                evidence: decision.map_or_else(Vec::new, |item| item.evidence.clone()),
                rollback_kind,
                rollback_operation,
                irreversible,
            })
        })
        .collect()
}

fn migration_operation(
    change: &SchemaChange,
    strategy: Option<&str>,
    adapter: MigrationAdapter,
) -> &'static str {
    match (change.change, strategy, adapter) {
        ("logical_rename", _, _) => "metadata_only_rename",
        ("physical_rename" | "logical_and_physical_rename", _, _) => "rename_physical_object",
        ("field_nullability_widened", _, MigrationAdapter::Postgres) => "drop_not_null_constraint",
        ("field_nullability_widened", _, MigrationAdapter::Sqlite) => {
            "rebuild_table_nullable_widening"
        }
        ("field_nullability_narrowed", Some("validate_existing"), MigrationAdapter::Postgres) => {
            "validate_then_set_not_null"
        }
        ("field_nullability_narrowed", Some("validate_existing"), MigrationAdapter::Sqlite) => {
            "validate_then_rebuild_table_required"
        }
        ("field_nullability_narrowed", Some("staged_transition"), _) => {
            "staged_nullability_narrowing"
        }
        ("added_nullable_field", _, _) => "add_nullable_column",
        ("added_required_field", Some("backfill"), MigrationAdapter::Postgres) => {
            "add_nullable_backfill_enforce_required"
        }
        ("added_required_field", Some("backfill"), MigrationAdapter::Sqlite) => {
            "rebuild_table_with_backfill"
        }
        ("added_required_field", Some("reject_if_rows"), _) => {
            "assert_empty_then_add_required_column"
        }
        ("added_required_field", Some("staged_transition"), _) => "staged_required_field",
        ("field_shape_changed", Some("validate_existing"), MigrationAdapter::Postgres) => {
            "validate_then_alter_column"
        }
        ("field_shape_changed", Some("validate_existing"), MigrationAdapter::Sqlite) => {
            "validate_then_rebuild_table"
        }
        ("field_shape_changed", Some("transform"), MigrationAdapter::Postgres) => {
            "transform_then_alter_column"
        }
        ("field_shape_changed", Some("transform"), MigrationAdapter::Sqlite) => {
            "transform_during_table_rebuild"
        }
        ("field_shape_changed", Some("staged_transition"), _) => "staged_shape_transition",
        ("relationship_lifecycle_changed", _, MigrationAdapter::Postgres) => {
            "replace_foreign_key_constraint"
        }
        ("relationship_lifecycle_changed", _, MigrationAdapter::Sqlite) => {
            "rebuild_table_foreign_key"
        }
        ("removed" | "incompatible_identity_change", Some("retain"), _) => "retain_physical_object",
        ("removed" | "incompatible_identity_change", Some("anonymise"), _) => {
            "anonymise_then_remove_object"
        }
        ("removed" | "incompatible_identity_change", Some("destructive_remove"), _) => {
            "drop_physical_object"
        }
        ("added", _, _) => match change.after.as_ref().map(|entry| entry.kind.as_str()) {
            Some("entity") => "create_table",
            Some("index") => "create_index",
            Some("primary_key" | "unique_constraint") => "create_constraint",
            _ => "create_schema_object",
        },
        ("schema_shape_changed", _, MigrationAdapter::Postgres) => "alter_schema_object",
        ("schema_shape_changed", _, MigrationAdapter::Sqlite) => "rebuild_schema_object",
        _ => "reviewed_adapter_operation",
    }
}

fn schema_migration_plan_json(
    adapter: MigrationAdapter,
    change_set: &str,
    steps: &[MigrationPlanStep],
) -> String {
    let steps_json = steps
        .iter()
        .enumerate()
        .map(|(index, step)| {
            let evidence = step
                .evidence
                .iter()
                .map(|item| {
                    format!(
                        "{{\"kind\":{},\"value\":{}}}",
                        json_string(&item.kind),
                        json_string(&item.value)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"order\":{},\"identity\":{},\"change\":{},\"operation\":{},\"decision_strategy\":{},\"evidence\":[{evidence}],\"rollback\":{{\"kind\":{},\"operation\":{}}},\"irreversible\":{}}}",
                index + 1,
                json_string(&step.identity),
                json_string(step.change),
                json_string(step.operation),
                step.decision_strategy.as_ref().map_or_else(
                    || "null".to_owned(),
                    |strategy| json_string(strategy)
                ),
                json_string(step.rollback_kind),
                json_string(step.rollback_operation),
                step.irreversible
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let irreversible_steps = steps
        .iter()
        .enumerate()
        .filter(|(_, step)| step.irreversible)
        .map(|(index, _)| (index + 1).to_string())
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema_version\":1,\"kind\":\"schema_migration_plan\",\"migration_plan\":true,\"adapter\":{},\"executable\":false,\"sql_generated\":false,\"change_set\":{},\"steps\":[{steps_json}],\"irreversible_steps\":[{irreversible_steps}]}}\n",
        json_string(adapter.name()),
        json_string(change_set)
    )
}

fn migration_sql(
    analyzed: &AnalyzedProject,
    changes: &[SchemaChange],
    decisions: &[SchemaDecision],
    registry: &[RegistryEntry],
    adapter: MigrationAdapter,
) -> Result<(Vec<String>, Vec<String>), Diagnostic> {
    let decisions = decisions
        .iter()
        .map(|decision| {
            (
                (decision.identity.as_str(), decision.change.as_str()),
                decision,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let entity_tables = registry
        .iter()
        .filter(|entry| entry.kind == "entity")
        .map(|entry| (entry.id.as_str(), entry.physical_name.as_str()))
        .collect::<BTreeMap<_, _>>();
    if adapter == MigrationAdapter::Sqlite {
        let physical_changes = changes
            .iter()
            .filter(|change| !migration_change_is_metadata_only(change))
            .collect::<Vec<_>>();
        if physical_changes
            .iter()
            .any(|change| change.change == "added_required_field")
            && physical_changes.iter().all(|change| {
                matches!(
                    change.change,
                    "added_nullable_field" | "added_required_field"
                )
            })
        {
            let mut changes_by_owner = BTreeMap::<&str, Vec<&SchemaChange>>::new();
            for change in physical_changes {
                let field = change
                    .after
                    .as_ref()
                    .expect("field additions have after state");
                let owner = field
                    .owner_id
                    .as_deref()
                    .ok_or_else(|| Diagnostic::error("MIG_SQL_FIELD_OWNER_MISSING"))?;
                changes_by_owner.entry(owner).or_default().push(change);
            }

            let mut units = Vec::new();
            for (owner, owner_changes) in changes_by_owner {
                if owner_changes
                    .iter()
                    .any(|change| change.change == "added_required_field")
                {
                    units.push(sqlite_added_fields_rebuild(
                        analyzed,
                        &owner_changes,
                        &decisions,
                        registry,
                    )?);
                } else {
                    let table = entity_tables
                        .get(owner)
                        .copied()
                        .ok_or_else(|| Diagnostic::error("MIG_SQL_FIELD_OWNER_MISSING"))?;
                    let mut forward = Vec::new();
                    let mut rollback = Vec::new();
                    for change in owner_changes {
                        let field = change
                            .after
                            .as_ref()
                            .expect("field additions have after state");
                        let field_type = field_shape_type(&field.shape)
                            .ok_or_else(|| Diagnostic::error("MIG_SQL_FIELD_SHAPE_INVALID"))?;
                        let sql_type = migration_sql_type(field_type, adapter)?;
                        let table = sql_identifier_for_migration(table);
                        let column = sql_identifier_for_migration(&field.physical_name);
                        forward.push(format!(
                            "ALTER TABLE {table} ADD COLUMN {column} {sql_type};"
                        ));
                        rollback.push(format!("ALTER TABLE {table} DROP COLUMN {column};"));
                    }
                    rollback.reverse();
                    units.push((forward, rollback));
                }
            }

            let forward = units
                .iter()
                .flat_map(|(statements, _)| statements.iter().cloned())
                .collect();
            let rollback = units
                .iter()
                .rev()
                .flat_map(|(_, statements)| statements.iter().cloned())
                .collect();
            return Ok((forward, rollback));
        }
        if !physical_changes.is_empty()
            && physical_changes
                .iter()
                .all(|change| change.change == "field_nullability_widened")
        {
            let mut changes_by_owner = BTreeMap::<&str, Vec<&SchemaChange>>::new();
            for change in physical_changes {
                let field = change
                    .after
                    .as_ref()
                    .expect("field shape changes have after state");
                let owner = field
                    .owner_id
                    .as_deref()
                    .ok_or_else(|| Diagnostic::error("MIG_SQL_FIELD_OWNER_MISSING"))?;
                changes_by_owner.entry(owner).or_default().push(change);
            }
            let units = changes_by_owner
                .into_values()
                .map(|owner_changes| {
                    sqlite_nullability_rebuild(analyzed, &owner_changes, registry, false)
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            let forward = units
                .iter()
                .flat_map(|(statements, _)| statements.iter().cloned())
                .collect();
            let rollback = units
                .iter()
                .rev()
                .flat_map(|(_, statements)| statements.iter().cloned())
                .collect();
            return Ok((forward, rollback));
        }
        if !physical_changes.is_empty()
            && physical_changes
                .iter()
                .all(|change| change.change == "field_nullability_narrowed")
        {
            let mut changes_by_owner = BTreeMap::<&str, Vec<&SchemaChange>>::new();
            for change in physical_changes {
                let decision = decisions
                    .get(&(change.identity.as_str(), change.change))
                    .copied()
                    .expect("nullability narrowing has a validated decision");
                validate_nullability_narrowing_decision(change, decision)?;
                let field = change
                    .after
                    .as_ref()
                    .expect("field shape changes have after state");
                let owner = field
                    .owner_id
                    .as_deref()
                    .ok_or_else(|| Diagnostic::error("MIG_SQL_FIELD_OWNER_MISSING"))?;
                changes_by_owner.entry(owner).or_default().push(change);
            }
            let units = changes_by_owner
                .into_values()
                .map(|owner_changes| {
                    sqlite_nullability_rebuild(analyzed, &owner_changes, registry, true)
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            let forward = units
                .iter()
                .flat_map(|(statements, _)| statements.iter().cloned())
                .collect();
            let rollback = units
                .iter()
                .rev()
                .flat_map(|(_, statements)| statements.iter().cloned())
                .collect();
            return Ok((forward, rollback));
        }
    }
    let mut forward = Vec::new();
    let mut rollback = Vec::new();
    for change in changes {
        if migration_change_is_metadata_only(change) {
            continue;
        }
        if change.change == "field_nullability_widened" {
            if adapter == MigrationAdapter::Sqlite {
                return Err(Diagnostic::error("MIG_SQL_SQLITE_REBUILD_REQUIRED"));
            }
            let field = change
                .after
                .as_ref()
                .expect("field shape changes have after state");
            let owner = field
                .owner_id
                .as_deref()
                .ok_or_else(|| Diagnostic::error("MIG_SQL_FIELD_OWNER_MISSING"))?;
            let table = entity_tables
                .get(owner)
                .copied()
                .ok_or_else(|| Diagnostic::error("MIG_SQL_FIELD_OWNER_MISSING"))?;
            let table = sql_identifier_for_migration(table);
            let column = sql_identifier_for_migration(&field.physical_name);
            forward.push(format!(
                "ALTER TABLE {table} ALTER COLUMN {column} DROP NOT NULL;"
            ));
            rollback.push(format!(
                "ALTER TABLE {table} ALTER COLUMN {column} SET NOT NULL;"
            ));
            continue;
        }
        if change.change == "field_nullability_narrowed" {
            if adapter == MigrationAdapter::Sqlite {
                return Err(Diagnostic::error("MIG_SQL_SQLITE_REBUILD_REQUIRED"));
            }
            let decision = decisions
                .get(&(change.identity.as_str(), change.change))
                .copied()
                .expect("nullability narrowing has a validated decision");
            validate_nullability_narrowing_decision(change, decision)?;
            let field = change
                .after
                .as_ref()
                .expect("field shape changes have after state");
            let owner = field
                .owner_id
                .as_deref()
                .ok_or_else(|| Diagnostic::error("MIG_SQL_FIELD_OWNER_MISSING"))?;
            let table = entity_tables
                .get(owner)
                .copied()
                .ok_or_else(|| Diagnostic::error("MIG_SQL_FIELD_OWNER_MISSING"))?;
            let table = sql_identifier_for_migration(table);
            let column = sql_identifier_for_migration(&field.physical_name);
            forward.push(format!(
                "SELECT COUNT(*) AS \"migration_null_violation_count\" FROM {table} WHERE {column} IS NULL;"
            ));
            forward.push(format!(
                "ALTER TABLE {table} ALTER COLUMN {column} SET NOT NULL;"
            ));
            rollback.push(format!(
                "ALTER TABLE {table} ALTER COLUMN {column} DROP NOT NULL;"
            ));
            continue;
        }
        if !matches!(
            change.change,
            "added_nullable_field" | "added_required_field"
        ) {
            return Err(Diagnostic::error("MIG_SQL_CHANGE_UNSUPPORTED")
                .with_note("the non-executable adapter plan remains available for review"));
        }
        let field = change
            .after
            .as_ref()
            .expect("field additions have after state");
        let owner = field
            .owner_id
            .as_deref()
            .ok_or_else(|| Diagnostic::error("MIG_SQL_FIELD_OWNER_MISSING"))?;
        let table = entity_tables
            .get(owner)
            .copied()
            .ok_or_else(|| Diagnostic::error("MIG_SQL_FIELD_OWNER_MISSING"))?;
        let field_type = field_shape_type(&field.shape)
            .ok_or_else(|| Diagnostic::error("MIG_SQL_FIELD_SHAPE_INVALID"))?;
        let sql_type = migration_sql_type(field_type, adapter)?;
        let table = sql_identifier_for_migration(table);
        let column = sql_identifier_for_migration(&field.physical_name);
        forward.push(format!(
            "ALTER TABLE {table} ADD COLUMN {column} {sql_type};"
        ));
        rollback.push(format!("ALTER TABLE {table} DROP COLUMN {column};"));

        if change.change == "added_required_field" {
            if adapter == MigrationAdapter::Sqlite {
                return Err(Diagnostic::error("MIG_SQL_SQLITE_REBUILD_REQUIRED"));
            }
            let decision = decisions
                .get(&(change.identity.as_str(), change.change))
                .copied()
                .expect("required additions have validated decisions");
            if decision.strategy.as_deref() != Some("backfill") {
                return Err(Diagnostic::error("MIG_SQL_STRATEGY_UNSUPPORTED"));
            }
            let expression = decision
                .evidence
                .iter()
                .find(|evidence| evidence.kind == "typed_expression")
                .map(|evidence| evidence.value.as_str())
                .expect("validated backfill decisions have typed expression evidence");
            let literal = compile_migration_literal(expression, field_type, adapter)?;
            forward.push(format!(
                "UPDATE {table} SET {column} = {literal} WHERE {column} IS NULL;"
            ));
            forward.push(format!(
                "ALTER TABLE {table} ALTER COLUMN {column} SET NOT NULL;"
            ));
        }
    }
    rollback.reverse();
    Ok((forward, rollback))
}

fn migration_change_is_metadata_only(change: &SchemaChange) -> bool {
    change.change == "logical_rename"
        || (change.change == "schema_shape_changed"
            && change
                .before
                .as_ref()
                .zip(change.after.as_ref())
                .is_some_and(|(before, after)| {
                    before.path != after.path
                        && before.physical_name == after.physical_name
                        && before.owner_id == after.owner_id
                }))
}

fn validate_nullability_narrowing_decision(
    _change: &SchemaChange,
    decision: &SchemaDecision,
) -> Result<(), Diagnostic> {
    if decision.strategy.as_deref() != Some("validate_existing") {
        return Err(Diagnostic::error("MIG_SQL_STRATEGY_UNSUPPORTED"));
    }
    let predicate = decision
        .evidence
        .iter()
        .find(|evidence| evidence.kind == "typed_predicate")
        .map(|evidence| evidence.value.as_str())
        .expect("validated narrowing decisions have typed predicate evidence");
    if predicate != "not_null" {
        return Err(Diagnostic::error("MIG_SQL_PREDICATE_UNSUPPORTED")
            .with_note("use the compiler-owned `not_null` predicate for this field"));
    }
    Ok(())
}

fn sqlite_added_fields_rebuild(
    analyzed: &AnalyzedProject,
    changes: &[&SchemaChange],
    decisions: &BTreeMap<(&str, &str), &SchemaDecision>,
    registry: &[RegistryEntry],
) -> Result<(Vec<String>, Vec<String>), Diagnostic> {
    let first_added = changes
        .first()
        .expect("SQLite rebuild groups contain at least one field addition")
        .after
        .as_ref()
        .expect("field additions have after state");
    let owner_id = first_added
        .owner_id
        .as_deref()
        .ok_or_else(|| Diagnostic::error("MIG_SQL_FIELD_OWNER_MISSING"))?;
    let entity_entry = registry
        .iter()
        .find(|entry| entry.id == owner_id && entry.kind == "entity")
        .ok_or_else(|| Diagnostic::error("MIG_SQL_FIELD_OWNER_MISSING"))?;
    if !entity_entry.previous_paths.is_empty() {
        return Err(Diagnostic::error(
            "MIG_SQL_SQLITE_RENAMED_TABLE_UNSUPPORTED",
        ));
    }
    let record = analyzed
        .syntax
        .sources
        .iter()
        .flat_map(|source| &source.file.declarations)
        .find_map(|declaration| match declaration {
            Declaration::Record(record)
                if record.kind == RecordKind::Entity && record.name.text == entity_entry.path =>
            {
                Some(record)
            }
            _ => None,
        })
        .ok_or_else(|| Diagnostic::error("MIG_SQL_SQLITE_ENTITY_MISSING"))?;
    let mut added_names = BTreeSet::new();
    let mut added_values = BTreeMap::new();
    for change in changes {
        let added = change
            .after
            .as_ref()
            .expect("field additions have after state");
        if added.owner_id.as_deref() != Some(owner_id) {
            return Err(Diagnostic::error("MIG_SQL_SQLITE_REBUILD_UNSUPPORTED"));
        }
        let added_name = added
            .path
            .rsplit_once('.')
            .map(|(_, field)| field)
            .unwrap_or_default();
        let added_field = record
            .fields
            .iter()
            .find(|field| field.name.text == added_name)
            .ok_or_else(|| Diagnostic::error("MIG_SQL_SQLITE_FIELD_MISSING"))?;
        if added_field.persistence.iter().any(|modifier| {
            matches!(
                modifier,
                PersistenceModifier::Identity
                    | PersistenceModifier::Unique
                    | PersistenceModifier::Index
            )
        }) {
            return Err(Diagnostic::error("MIG_SQL_SQLITE_REBUILD_UNSUPPORTED"));
        }
        let value = if change.change == "added_required_field" {
            let decision = decisions
                .get(&(change.identity.as_str(), change.change))
                .copied()
                .expect("required additions have validated decisions");
            if decision.strategy.as_deref() != Some("backfill") {
                return Err(Diagnostic::error("MIG_SQL_STRATEGY_UNSUPPORTED"));
            }
            let expression = decision
                .evidence
                .iter()
                .find(|evidence| evidence.kind == "typed_expression")
                .map(|evidence| evidence.value.as_str())
                .expect("validated backfill decisions have typed expression evidence");
            let added_type = field_shape_type(&added.shape)
                .ok_or_else(|| Diagnostic::error("MIG_SQL_FIELD_SHAPE_INVALID"))?;
            compile_migration_literal(expression, added_type, MigrationAdapter::Sqlite)?
        } else {
            "NULL".to_owned()
        };
        added_names.insert(added_name);
        added_values.insert(added_name, value);
    }
    let table = sql_identifier_for_migration(&entity_entry.physical_name);
    let forward_temp_name = format!("__migration_new_{}", entity_entry.physical_name);
    let rollback_temp_name = format!("__migration_rollback_{}", entity_entry.physical_name);
    let forward_temp = sql_identifier_for_migration(&forward_temp_name);
    let rollback_temp = sql_identifier_for_migration(&rollback_temp_name);
    let no_exclusions = BTreeSet::new();
    let no_required_overrides = BTreeSet::new();
    let no_nullable_overrides = BTreeSet::new();
    let current_body = sqlite_rebuild_table_body(
        analyzed,
        record,
        registry,
        &no_exclusions,
        &no_required_overrides,
        &no_nullable_overrides,
    )?;
    let previous_body = sqlite_rebuild_table_body(
        analyzed,
        record,
        registry,
        &added_names,
        &no_required_overrides,
        &no_nullable_overrides,
    )?;
    let current_columns = sqlite_rebuild_columns(record, registry, &no_exclusions)?;
    let previous_columns = sqlite_rebuild_columns(record, registry, &added_names)?;
    let select_values = record
        .fields
        .iter()
        .map(|field| {
            if let Some(value) = added_values.get(field.name.text.as_str()) {
                Ok(value.clone())
            } else {
                sqlite_field_physical_name(record, field, registry)
                    .map(sql_identifier_for_migration)
            }
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?
        .join(", ");
    let indexes = sqlite_rebuild_index_sql(record, registry, &added_names)?;

    let mut forward = vec![
        "PRAGMA foreign_keys = OFF;".to_owned(),
        "BEGIN IMMEDIATE;".to_owned(),
        format!("CREATE TABLE {forward_temp} ({current_body});"),
        format!(
            "INSERT INTO {forward_temp} ({current_columns}) SELECT {select_values} FROM {table};"
        ),
        format!("DROP TABLE {table};"),
        format!("ALTER TABLE {forward_temp} RENAME TO {table};"),
    ];
    forward.extend(indexes.iter().cloned());
    forward.extend([
        "COMMIT;".to_owned(),
        "PRAGMA foreign_keys = ON;".to_owned(),
        "PRAGMA foreign_key_check;".to_owned(),
    ]);

    let mut rollback = vec![
        "PRAGMA foreign_keys = OFF;".to_owned(),
        "BEGIN IMMEDIATE;".to_owned(),
        format!("CREATE TABLE {rollback_temp} ({previous_body});"),
        format!(
            "INSERT INTO {rollback_temp} ({previous_columns}) SELECT {previous_columns} FROM {table};"
        ),
        format!("DROP TABLE {table};"),
        format!("ALTER TABLE {rollback_temp} RENAME TO {table};"),
    ];
    rollback.extend(indexes);
    rollback.extend([
        "COMMIT;".to_owned(),
        "PRAGMA foreign_keys = ON;".to_owned(),
        "PRAGMA foreign_key_check;".to_owned(),
    ]);
    Ok((forward, rollback))
}

fn sqlite_nullability_rebuild(
    analyzed: &AnalyzedProject,
    changes: &[&SchemaChange],
    registry: &[RegistryEntry],
    narrowing: bool,
) -> Result<(Vec<String>, Vec<String>), Diagnostic> {
    let first = changes
        .first()
        .expect("SQLite rebuild groups contain at least one nullability change");
    let first_field = first
        .after
        .as_ref()
        .expect("field shape changes have after state");
    let owner_id = first_field
        .owner_id
        .as_deref()
        .ok_or_else(|| Diagnostic::error("MIG_SQL_FIELD_OWNER_MISSING"))?;
    let entity_entry = registry
        .iter()
        .find(|entry| entry.id == owner_id && entry.kind == "entity")
        .ok_or_else(|| Diagnostic::error("MIG_SQL_FIELD_OWNER_MISSING"))?;
    if !entity_entry.previous_paths.is_empty() {
        return Err(Diagnostic::error(
            "MIG_SQL_SQLITE_RENAMED_TABLE_UNSUPPORTED",
        ));
    }
    let record = analyzed
        .syntax
        .sources
        .iter()
        .flat_map(|source| &source.file.declarations)
        .find_map(|declaration| match declaration {
            Declaration::Record(record)
                if record.kind == RecordKind::Entity && record.name.text == entity_entry.path =>
            {
                Some(record)
            }
            _ => None,
        })
        .ok_or_else(|| Diagnostic::error("MIG_SQL_SQLITE_ENTITY_MISSING"))?;
    let mut changed_names = BTreeSet::new();
    for change in changes {
        let before = change
            .before
            .as_ref()
            .expect("field shape changes have before state");
        let after = change
            .after
            .as_ref()
            .expect("field shape changes have after state");
        let compatible = if narrowing {
            field_shape_is_nullability_narrowing(&before.shape, &after.shape)
        } else {
            field_shape_is_nullability_widening(&before.shape, &after.shape)
        };
        if after.owner_id.as_deref() != Some(owner_id) || !compatible {
            return Err(Diagnostic::error("MIG_SQL_SQLITE_REBUILD_UNSUPPORTED"));
        }
        let field_name = after
            .path
            .rsplit_once('.')
            .map(|(_, field)| field)
            .unwrap_or_default();
        if !record
            .fields
            .iter()
            .any(|field| field.name.text == field_name)
        {
            return Err(Diagnostic::error("MIG_SQL_SQLITE_FIELD_MISSING"));
        }
        changed_names.insert(field_name);
    }

    let no_exclusions = BTreeSet::new();
    let no_required_overrides = BTreeSet::new();
    let no_nullable_overrides = BTreeSet::new();
    let current_body = sqlite_rebuild_table_body(
        analyzed,
        record,
        registry,
        &no_exclusions,
        &no_required_overrides,
        &no_nullable_overrides,
    )?;
    let previous_required = if narrowing {
        &no_required_overrides
    } else {
        &changed_names
    };
    let previous_nullable = if narrowing {
        &changed_names
    } else {
        &no_nullable_overrides
    };
    let previous_body = sqlite_rebuild_table_body(
        analyzed,
        record,
        registry,
        &no_exclusions,
        previous_required,
        previous_nullable,
    )?;
    let columns = sqlite_rebuild_columns(record, registry, &no_exclusions)?;
    let indexes = sqlite_rebuild_index_sql(record, registry, &no_exclusions)?;
    let table = sql_identifier_for_migration(&entity_entry.physical_name);
    let direction = if narrowing { "required" } else { "nullable" };
    let forward_temp = sql_identifier_for_migration(&format!(
        "__migration_{direction}_{}",
        entity_entry.physical_name
    ));
    let rollback_temp = sql_identifier_for_migration(&format!(
        "__migration_rollback_{}",
        entity_entry.physical_name
    ));

    let mut forward = vec![
        "PRAGMA foreign_keys = OFF;".to_owned(),
        "BEGIN IMMEDIATE;".to_owned(),
        format!("CREATE TABLE {forward_temp} ({current_body});"),
        format!("INSERT INTO {forward_temp} ({columns}) SELECT {columns} FROM {table};"),
        format!("DROP TABLE {table};"),
        format!("ALTER TABLE {forward_temp} RENAME TO {table};"),
    ];
    forward.extend(indexes.iter().cloned());
    forward.extend([
        "COMMIT;".to_owned(),
        "PRAGMA foreign_keys = ON;".to_owned(),
        "PRAGMA foreign_key_check;".to_owned(),
    ]);

    let mut rollback = vec![
        "PRAGMA foreign_keys = OFF;".to_owned(),
        "BEGIN IMMEDIATE;".to_owned(),
        format!("CREATE TABLE {rollback_temp} ({previous_body});"),
        format!("INSERT INTO {rollback_temp} ({columns}) SELECT {columns} FROM {table};"),
        format!("DROP TABLE {table};"),
        format!("ALTER TABLE {rollback_temp} RENAME TO {table};"),
    ];
    rollback.extend(indexes);
    rollback.extend([
        "COMMIT;".to_owned(),
        "PRAGMA foreign_keys = ON;".to_owned(),
        "PRAGMA foreign_key_check;".to_owned(),
    ]);
    Ok((forward, rollback))
}

fn sqlite_rebuild_table_body(
    analyzed: &AnalyzedProject,
    record: &jadpo_syntax::RecordDeclaration,
    registry: &[RegistryEntry],
    excluded_fields: &BTreeSet<&str>,
    required_fields: &BTreeSet<&str>,
    nullable_fields: &BTreeSet<&str>,
) -> Result<String, Diagnostic> {
    let mut columns = Vec::new();
    let mut constraints = Vec::new();
    for field in &record.fields {
        if excluded_fields.contains(field.name.text.as_str()) {
            continue;
        }
        let physical = sqlite_field_physical_name(record, field, registry)?;
        let sql_type = sqlite_rebuild_field_type(analyzed, field)?;
        let nullable = if nullable_fields.contains(field.name.text.as_str())
            || (field.field_type.nullable && !required_fields.contains(field.name.text.as_str()))
        {
            ""
        } else {
            " NOT NULL"
        };
        columns.push(format!(
            "{} {sql_type}{nullable}",
            sql_identifier_for_migration(physical)
        ));
        for (modifier, suffix) in [
            (PersistenceModifier::Identity, "primary_key"),
            (PersistenceModifier::Unique, "unique"),
        ] {
            if field.persistence.contains(&modifier) {
                let path = format!("{}.{}#{suffix}", record.name.text, field.name.text);
                let constraint = registry
                    .iter()
                    .find(|entry| entry.path == path)
                    .ok_or_else(|| Diagnostic::error("MIG_SQL_SQLITE_CONSTRAINT_MISSING"))?;
                let kind = if modifier == PersistenceModifier::Identity {
                    "PRIMARY KEY"
                } else {
                    "UNIQUE"
                };
                constraints.push(format!(
                    "CONSTRAINT {} {kind} ({})",
                    sql_identifier_for_migration(&constraint.physical_name),
                    sql_identifier_for_migration(physical)
                ));
            }
        }
        if let Some(reference) = &field.reference {
            let target_entity = reference
                .target
                .path
                .first()
                .map(|name| name.text.as_str())
                .unwrap_or_default();
            let target_field = reference
                .target
                .path
                .get(1)
                .map(|name| name.text.as_str())
                .unwrap_or_default();
            let target_table = registry
                .iter()
                .find(|entry| entry.kind == "entity" && entry.path == target_entity)
                .ok_or_else(|| Diagnostic::error("MIG_SQL_SQLITE_REFERENCE_MISSING"))?;
            let target_path = format!("{target_entity}.{target_field}");
            let target_column = registry
                .iter()
                .find(|entry| entry.kind == "field" && entry.path == target_path)
                .ok_or_else(|| Diagnostic::error("MIG_SQL_SQLITE_REFERENCE_MISSING"))?;
            let owner_table = registry
                .iter()
                .find(|entry| entry.kind == "entity" && entry.path == record.name.text)
                .expect("checked registry contains owning entity");
            let delete_action = match reference.on_delete {
                ReferenceDeleteAction::Restrict => "RESTRICT",
                ReferenceDeleteAction::Cascade => "CASCADE",
                ReferenceDeleteAction::SetNull => "SET NULL",
            };
            constraints.push(format!(
                "CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {} ({}) ON DELETE {delete_action}",
                sql_identifier_for_migration(&format!(
                    "{}_{}_fk",
                    owner_table.physical_name, physical
                )),
                sql_identifier_for_migration(physical),
                sql_identifier_for_migration(&target_table.physical_name),
                sql_identifier_for_migration(&target_column.physical_name)
            ));
        }
    }
    for constraint in &record.persistence_constraints {
        if constraint
            .fields
            .iter()
            .any(|field| excluded_fields.contains(field.text.as_str()))
        {
            return Err(Diagnostic::error("MIG_SQL_SQLITE_REBUILD_UNSUPPORTED"));
        }
        let path = format!("{}.{}", record.name.text, constraint.name.text);
        let identity = registry
            .iter()
            .find(|entry| entry.kind == "unique_constraint" && entry.path == path)
            .ok_or_else(|| Diagnostic::error("MIG_SQL_SQLITE_CONSTRAINT_MISSING"))?;
        let fields = constraint
            .fields
            .iter()
            .map(|name| {
                let field = record
                    .fields
                    .iter()
                    .find(|field| field.name.text == name.text)
                    .expect("checked compound constraint field exists");
                sqlite_field_physical_name(record, field, registry)
                    .map(sql_identifier_for_migration)
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?
            .join(", ");
        constraints.push(format!(
            "CONSTRAINT {} UNIQUE ({fields})",
            sql_identifier_for_migration(&identity.physical_name)
        ));
    }
    columns.extend(constraints);
    Ok(columns.join(", "))
}

fn sqlite_rebuild_field_type(
    analyzed: &AnalyzedProject,
    field: &FieldDeclaration,
) -> Result<&'static str, Diagnostic> {
    let raw = type_name(&field.field_type);
    if let Ok(sql_type) = migration_sql_type(&raw, MigrationAdapter::Sqlite) {
        return Ok(sql_type);
    }
    if field.field_type.path.len() == 2 {
        let entity = &field.field_type.path[0].text;
        let target = &field.field_type.path[1].text;
        let target_field = analyzed
            .syntax
            .sources
            .iter()
            .flat_map(|source| &source.file.declarations)
            .find_map(|declaration| match declaration {
                Declaration::Record(record)
                    if record.kind == RecordKind::Entity && record.name.text == *entity =>
                {
                    record
                        .fields
                        .iter()
                        .find(|field| field.name.text == *target)
                }
                _ => None,
            })
            .ok_or_else(|| Diagnostic::error("MIG_SQL_SQLITE_REFERENCE_MISSING"))?;
        return migration_sql_type(
            &type_name(&target_field.field_type),
            MigrationAdapter::Sqlite,
        );
    }
    Err(Diagnostic::error("MIG_SQL_TYPE_UNSUPPORTED"))
}

fn sqlite_rebuild_columns(
    record: &jadpo_syntax::RecordDeclaration,
    registry: &[RegistryEntry],
    excluded_fields: &BTreeSet<&str>,
) -> Result<String, Diagnostic> {
    record
        .fields
        .iter()
        .filter(|field| !excluded_fields.contains(field.name.text.as_str()))
        .map(|field| {
            sqlite_field_physical_name(record, field, registry).map(sql_identifier_for_migration)
        })
        .collect::<Result<Vec<_>, Diagnostic>>()
        .map(|columns| columns.join(", "))
}

fn sqlite_field_physical_name<'a>(
    record: &jadpo_syntax::RecordDeclaration,
    field: &FieldDeclaration,
    registry: &'a [RegistryEntry],
) -> Result<&'a str, Diagnostic> {
    let path = format!("{}.{}", record.name.text, field.name.text);
    registry
        .iter()
        .find(|entry| entry.kind == "field" && entry.path == path)
        .map(|entry| entry.physical_name.as_str())
        .ok_or_else(|| Diagnostic::error("MIG_SQL_SQLITE_FIELD_IDENTITY_MISSING"))
}

fn sqlite_rebuild_index_sql(
    record: &jadpo_syntax::RecordDeclaration,
    registry: &[RegistryEntry],
    excluded_fields: &BTreeSet<&str>,
) -> Result<Vec<String>, Diagnostic> {
    let table = registry
        .iter()
        .find(|entry| entry.kind == "entity" && entry.path == record.name.text)
        .map(|entry| sql_identifier_for_migration(&entry.physical_name))
        .ok_or_else(|| Diagnostic::error("MIG_SQL_SQLITE_ENTITY_MISSING"))?;
    record
        .fields
        .iter()
        .filter(|field| {
            !excluded_fields.contains(field.name.text.as_str())
                && (field.persistence.contains(&PersistenceModifier::Index)
                    || field.reference.is_some())
        })
        .map(|field| {
            let path = format!("{}.{}#index", record.name.text, field.name.text);
            let index = registry
                .iter()
                .find(|entry| entry.kind == "index" && entry.path == path)
                .ok_or_else(|| Diagnostic::error("MIG_SQL_SQLITE_INDEX_MISSING"))?;
            let column = sqlite_field_physical_name(record, field, registry)?;
            Ok(format!(
                "CREATE INDEX {} ON {table} ({});",
                sql_identifier_for_migration(&index.physical_name),
                sql_identifier_for_migration(column)
            ))
        })
        .collect()
}

fn field_shape_type(shape: &str) -> Option<&str> {
    let value = shape.strip_prefix("{\"type\":\"")?;
    value.split_once('"').map(|(field_type, _)| field_type)
}

fn migration_sql_type(
    field_type: &str,
    adapter: MigrationAdapter,
) -> Result<&'static str, Diagnostic> {
    match field_type.trim_end_matches('?') {
        "Bool" => Ok(match adapter {
            MigrationAdapter::Postgres => "BOOLEAN",
            MigrationAdapter::Sqlite => "INTEGER",
        }),
        "Int" => Ok("BIGINT"),
        "Decimal" => Ok(match adapter {
            MigrationAdapter::Postgres => "NUMERIC",
            MigrationAdapter::Sqlite => "REAL",
        }),
        "Text" | "Uuid" => Ok("TEXT"),
        "Instant" => Ok(match adapter {
            MigrationAdapter::Postgres => "TIMESTAMPTZ(3)",
            MigrationAdapter::Sqlite => "BIGINT",
        }),
        "CalendarDate" => Ok(match adapter {
            MigrationAdapter::Postgres => "DATE",
            MigrationAdapter::Sqlite => "TEXT",
        }),
        "Duration" => Ok("BIGINT"),
        _other => Err(Diagnostic::error("MIG_SQL_TYPE_UNSUPPORTED")),
    }
}

fn compile_migration_literal(
    expression: &str,
    field_type: &str,
    adapter: MigrationAdapter,
) -> Result<String, Diagnostic> {
    let value = expression
        .strip_prefix("literal(")
        .and_then(|value| value.strip_suffix(')'))
        .ok_or_else(|| Diagnostic::error("MIG_SQL_EXPRESSION_UNSUPPORTED"))?;
    match field_type.trim_end_matches('?') {
        "Text" => Ok(sql_string_literal(value)),
        "Uuid" if valid_uuid(value) => Ok(sql_string_literal(value)),
        "Uuid" => Err(Diagnostic::error("MIG_SQL_LITERAL_INVALID")),
        "Instant" if adapter == MigrationAdapter::Postgres && valid_migration_instant(value) => {
            Ok(sql_string_literal(value))
        }
        "Instant" => Err(Diagnostic::error("MIG_SQL_LITERAL_INVALID")),
        "CalendarDate" if valid_migration_calendar_date(value) => Ok(sql_string_literal(value)),
        "CalendarDate" => Err(Diagnostic::error("MIG_SQL_LITERAL_INVALID")),
        "Int" if value.parse::<i64>().is_ok() => Ok(value.to_owned()),
        "Decimal" if valid_decimal(value) => Ok(value.to_owned()),
        "Bool" if value == "true" => Ok(match adapter {
            MigrationAdapter::Postgres => "TRUE".to_owned(),
            MigrationAdapter::Sqlite => "1".to_owned(),
        }),
        "Bool" if value == "false" => Ok(match adapter {
            MigrationAdapter::Postgres => "FALSE".to_owned(),
            MigrationAdapter::Sqlite => "0".to_owned(),
        }),
        _kind => Err(Diagnostic::error("MIG_SQL_LITERAL_INVALID")),
    }
}

fn sql_identifier_for_migration(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn sql_string_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn valid_uuid(value: &str) -> bool {
    value.len() == 36
        && value
            .chars()
            .enumerate()
            .all(|(index, character)| match index {
                8 | 13 | 18 | 23 => character == '-',
                _ => character.is_ascii_hexdigit(),
            })
}

fn valid_decimal(value: &str) -> bool {
    let value = value.strip_prefix('-').unwrap_or(value);
    let mut parts = value.split('.');
    let whole = parts.next().unwrap_or_default();
    let fraction = parts.next();
    !whole.is_empty()
        && whole.chars().all(|character| character.is_ascii_digit())
        && fraction.map_or(true, |part| {
            !part.is_empty() && part.chars().all(|character| character.is_ascii_digit())
        })
        && parts.next().is_none()
}

fn valid_migration_calendar_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let number = |range: std::ops::Range<usize>| value.get(range)?.parse::<u32>().ok();
    let (Some(year), Some(month), Some(day)) = (number(0..4), number(5..7), number(8..10)) else {
        return false;
    };
    if year == 0 || !(1..=12).contains(&month) {
        return false;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let maximum = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    (1..=maximum).contains(&day)
}

fn valid_migration_instant(value: &str) -> bool {
    let Some((date, time_and_offset)) = value.split_once('T') else {
        return false;
    };
    if !valid_migration_calendar_date(date) {
        return false;
    }
    let (clock, offset) = if let Some(clock) = time_and_offset.strip_suffix('Z') {
        (clock, "Z")
    } else if time_and_offset.len() >= 6 {
        time_and_offset.split_at(time_and_offset.len() - 6)
    } else {
        return false;
    };
    let (whole_clock, fraction) = clock
        .split_once('.')
        .map_or((clock, None), |(whole, fraction)| (whole, Some(fraction)));
    if fraction.is_some_and(|fraction| {
        fraction.is_empty()
            || fraction.len() > 3
            || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    }) {
        return false;
    }
    let clock_fields = whole_clock.split(':').collect::<Vec<_>>();
    if clock_fields.len() != 3
        || clock_fields
            .iter()
            .any(|field| field.len() != 2 || !field.bytes().all(|byte| byte.is_ascii_digit()))
        || clock_fields[0].parse::<u8>().map_or(true, |hour| hour > 23)
        || clock_fields[1]
            .parse::<u8>()
            .map_or(true, |minute| minute > 59)
        || clock_fields[2]
            .parse::<u8>()
            .map_or(true, |second| second > 59)
    {
        return false;
    }
    if offset == "Z" {
        return true;
    }
    let bytes = offset.as_bytes();
    bytes.len() == 6
        && matches!(bytes[0], b'+' | b'-')
        && bytes[3] == b':'
        && bytes[1..3].iter().all(u8::is_ascii_digit)
        && bytes[4..6].iter().all(u8::is_ascii_digit)
        && offset[1..3].parse::<u8>().is_ok_and(|hour| hour <= 23)
        && offset[4..6].parse::<u8>().is_ok_and(|minute| minute <= 59)
}

fn schema_migration_sql_review_json(
    adapter: MigrationAdapter,
    change_set: &str,
    forward: &[String],
    rollback: &[String],
) -> String {
    let forward = forward
        .iter()
        .map(|statement| json_string(statement))
        .collect::<Vec<_>>()
        .join(",");
    let rollback = rollback
        .iter()
        .map(|statement| json_string(statement))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema_version\":1,\"kind\":\"schema_migration_sql_review\",\"adapter\":{},\"executable\":false,\"review_required\":true,\"change_set\":{},\"forward_sql\":[{forward}],\"rollback_sql\":[{rollback}]}}\n",
        json_string(adapter.name()),
        json_string(change_set)
    )
}

fn schema_registry_path(project: &Path) -> PathBuf {
    let project_root = if project.is_dir() {
        project
    } else {
        project.parent().unwrap_or_else(|| Path::new("."))
    };
    project_root.join("schema.identities.json")
}

fn read_registry_source(path: &Path) -> Result<String, Diagnostic> {
    fs::read_to_string(path)
        .map_err(|_error| Diagnostic::error("MIG_IDENTITY_REGISTRY_READ_FAILED"))
}

fn validate_registry_entries(
    entries: &[RegistryEntry],
    live: &[RegistryEntry],
) -> Result<(), Diagnostic> {
    validate_registry_structure(entries)?;
    let registered = entries
        .iter()
        .map(|entry| ((entry.kind.as_str(), entry.path.as_str()), entry))
        .collect::<BTreeMap<_, _>>();
    let live_keys = live
        .iter()
        .map(|entry| (entry.kind.as_str(), entry.path.as_str()))
        .collect::<BTreeSet<_>>();

    if let Some(_entry) = live
        .iter()
        .find(|entry| !registered.contains_key(&(entry.kind.as_str(), entry.path.as_str())))
    {
        return Err(Diagnostic::error("MIG_IDENTITY_REGISTRY_DRIFT")
            .with_note("additions and renames require an explicit schema registry operation"));
    }
    if let Some(_entry) = entries
        .iter()
        .find(|entry| !live_keys.contains(&(entry.kind.as_str(), entry.path.as_str())))
    {
        return Err(Diagnostic::error("MIG_IDENTITY_REGISTRY_DRIFT")
            .with_note("removals and renames require an explicit schema registry operation"));
    }

    let entity_ids = entries
        .iter()
        .filter(|entry| entry.kind == "entity")
        .map(|entry| (entry.path.as_str(), entry.id.as_str()))
        .collect::<BTreeMap<_, _>>();
    for entry in entries.iter().filter(|entry| entry.kind != "entity") {
        let entity_path = entry.path.split('.').next().unwrap_or_default();
        let expected_owner = entity_ids.get(entity_path).copied();
        if entry.owner_id.as_deref() != expected_owner {
            return Err(Diagnostic::error("MIG_IDENTITY_REGISTRY_OWNER"));
        }
    }
    Ok(())
}

fn validate_registry_structure(entries: &[RegistryEntry]) -> Result<(), Diagnostic> {
    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for entry in entries {
        if !ids.insert(entry.id.as_str()) {
            return Err(Diagnostic::error("MIG_IDENTITY_DUPLICATE_ID"));
        }
        if !paths.insert((entry.kind.as_str(), entry.path.as_str())) {
            return Err(Diagnostic::error("MIG_IDENTITY_DUPLICATE_PATH"));
        }
        if entry.physical_name.is_empty() {
            return Err(Diagnostic::error("MIG_IDENTITY_PHYSICAL_NAME"));
        }
    }
    Ok(())
}

fn derive_registry_entries(analyzed: &AnalyzedProject) -> Vec<RegistryEntry> {
    let mut entries = Vec::new();
    for source in &analyzed.syntax.sources {
        for declaration in &source.file.declarations {
            let Declaration::Record(record) = declaration else {
                continue;
            };
            if !record.is_persistent_entity() {
                continue;
            }

            let entity_name = record.name.text.as_str();
            let entity_id = identity("entity", entity_name);
            let table_name = snake_case(entity_name);
            entries.push(RegistryEntry {
                id: entity_id.clone(),
                kind: "entity".to_owned(),
                path: entity_name.to_owned(),
                physical_name: table_name.clone(),
                owner_id: None,
                previous_paths: Vec::new(),
            });

            for field in &record.fields {
                let field_path = format!("{entity_name}.{}", field.name.text);
                entries.push(RegistryEntry {
                    id: identity("field", &field_path),
                    kind: "field".to_owned(),
                    path: field_path.clone(),
                    physical_name: field.name.text.clone(),
                    owner_id: Some(entity_id.clone()),
                    previous_paths: Vec::new(),
                });
                if field.persistence.contains(&PersistenceModifier::Identity) {
                    entries.push(RegistryEntry {
                        id: identity("constraint", &format!("{field_path}#primary_key")),
                        kind: "primary_key".to_owned(),
                        path: format!("{field_path}#primary_key"),
                        physical_name: format!("{table_name}_pkey"),
                        owner_id: Some(entity_id.clone()),
                        previous_paths: Vec::new(),
                    });
                }
                if field.persistence.contains(&PersistenceModifier::Unique) {
                    entries.push(RegistryEntry {
                        id: identity("constraint", &format!("{field_path}#unique")),
                        kind: "unique_constraint".to_owned(),
                        path: format!("{field_path}#unique"),
                        physical_name: format!("{table_name}_{}_unique", field.name.text),
                        owner_id: Some(entity_id.clone()),
                        previous_paths: Vec::new(),
                    });
                }
                if field.persistence.contains(&PersistenceModifier::Index)
                    || field.reference.is_some()
                {
                    entries.push(RegistryEntry {
                        id: identity("index", &field_path),
                        kind: "index".to_owned(),
                        path: format!("{field_path}#index"),
                        physical_name: format!("{table_name}_{}_idx", field.name.text),
                        owner_id: Some(entity_id.clone()),
                        previous_paths: Vec::new(),
                    });
                }
            }

            for constraint in &record.persistence_constraints {
                let constraint_path = format!("{entity_name}.{}", constraint.name.text);
                entries.push(RegistryEntry {
                    id: identity("constraint", &constraint_path),
                    kind: "unique_constraint".to_owned(),
                    path: constraint_path,
                    physical_name: format!("{table_name}_{}_unique", constraint.name.text),
                    owner_id: Some(entity_id.clone()),
                    previous_paths: Vec::new(),
                });
            }
        }
    }
    entries.sort_by(|left, right| left.id.cmp(&right.id));
    entries
}

fn registry_json(entries: &[RegistryEntry]) -> String {
    let entries = entries
        .iter()
        .map(|entry| {
            format!(
                "{{\"id\":{},\"kind\":{},\"path\":{},\"physical_name\":{},\"owner_id\":{},\"previous_paths\":[{}]}}",
                json_string(&entry.id),
                json_string(&entry.kind),
                json_string(&entry.path),
                json_string(&entry.physical_name),
                entry
                    .owner_id
                    .as_ref()
                    .map_or_else(|| "null".to_owned(), |owner| json_string(owner)),
                entry
                    .previous_paths
                    .iter()
                    .map(|path| json_string(path))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"schema_version\":1,\"entries\":[{entries}]}}\n")
}

fn derive_schema_snapshot_entries(
    analyzed: &AnalyzedProject,
    registry: &[RegistryEntry],
) -> Result<Vec<SchemaSnapshotEntry>, Diagnostic> {
    let mut shapes = BTreeMap::new();
    for source in &analyzed.syntax.sources {
        for declaration in &source.file.declarations {
            let Declaration::Record(record) = declaration else {
                continue;
            };
            if !record.is_persistent_entity() {
                continue;
            }
            let entity = record.name.text.as_str();
            shapes.insert(("entity".to_owned(), entity.to_owned()), "{}".to_owned());
            for field in &record.fields {
                let field_path = format!("{entity}.{}", field.name.text);
                shapes.insert(("field".to_owned(), field_path.clone()), field_shape(field));
                if field.persistence.contains(&PersistenceModifier::Identity) {
                    shapes.insert(
                        (
                            "primary_key".to_owned(),
                            format!("{field_path}#primary_key"),
                        ),
                        format!("{{\"field\":{}}}", json_string(&field.name.text)),
                    );
                }
                if field.persistence.contains(&PersistenceModifier::Unique) {
                    shapes.insert(
                        (
                            "unique_constraint".to_owned(),
                            format!("{field_path}#unique"),
                        ),
                        format!("{{\"fields\":[{}]}}", json_string(&field.name.text)),
                    );
                }
                if field.persistence.contains(&PersistenceModifier::Index)
                    || field.reference.is_some()
                {
                    shapes.insert(
                        ("index".to_owned(), format!("{field_path}#index")),
                        format!("{{\"fields\":[{}]}}", json_string(&field.name.text)),
                    );
                }
            }
            for constraint in &record.persistence_constraints {
                let fields = constraint
                    .fields
                    .iter()
                    .map(|field| json_string(&field.text))
                    .collect::<Vec<_>>()
                    .join(",");
                shapes.insert(
                    (
                        "unique_constraint".to_owned(),
                        format!("{entity}.{}", constraint.name.text),
                    ),
                    format!("{{\"fields\":[{fields}]}}"),
                );
            }
        }
    }

    let mut entries = registry
        .iter()
        .map(|entry| {
            let shape = shapes
                .get(&(entry.kind.clone(), entry.path.clone()))
                .cloned()
                .ok_or_else(|| Diagnostic::error("MIG_IDENTITY_SNAPSHOT_SHAPE_MISSING"))?;
            Ok(SchemaSnapshotEntry {
                id: entry.id.clone(),
                kind: entry.kind.clone(),
                path: entry.path.clone(),
                physical_name: entry.physical_name.clone(),
                owner_id: entry.owner_id.clone(),
                shape,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    entries.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(entries)
}

fn field_shape(field: &FieldDeclaration) -> String {
    let constraints = field
        .constraints
        .iter()
        .map(|constraint| {
            format!(
                "{{\"kind\":{},\"value\":{}}}",
                json_string(constraint_name(constraint.kind)),
                json_string(&constraint.value.text)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let reference = field.reference.as_ref().map_or_else(
        || "null".to_owned(),
        |reference| {
            format!(
                "{{\"target\":{},\"relationship\":{},\"on_delete\":{}}}",
                json_string(&type_name(&reference.target)),
                reference
                    .relationship
                    .as_ref()
                    .map_or_else(|| "null".to_owned(), |name| json_string(&name.text)),
                json_string(reference_delete_name(reference.on_delete))
            )
        },
    );
    format!(
        "{{\"type\":{},\"nullable\":{},\"identity\":{},\"unique\":{},\"indexed\":{},\"constraints\":[{constraints}],\"reference\":{reference}}}",
        json_string(&type_name(&field.field_type)),
        field.field_type.nullable,
        field.persistence.contains(&PersistenceModifier::Identity),
        field.persistence.contains(&PersistenceModifier::Unique),
        field.persistence.contains(&PersistenceModifier::Index) || field.reference.is_some()
    )
}

fn schema_snapshot_json(entries: &[SchemaSnapshotEntry]) -> String {
    let entries = entries
        .iter()
        .map(|entry| {
            format!(
                "{{\"id\":{},\"kind\":{},\"path\":{},\"physical_name\":{},\"owner_id\":{},\"shape\":{}}}",
                json_string(&entry.id),
                json_string(&entry.kind),
                json_string(&entry.path),
                json_string(&entry.physical_name),
                entry
                    .owner_id
                    .as_ref()
                    .map_or_else(|| "null".to_owned(), |owner| json_string(owner)),
                json_string(&entry.shape)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"schema_version\":1,\"kind\":\"schema_snapshot\",\"entries\":[{entries}]}}\n")
}

fn parse_schema_snapshot(source: &str) -> Result<Vec<SchemaSnapshotEntry>, Diagnostic> {
    let result = (|| {
        let mut parser = RegistryParser::new(source);
        parser.expect('{')?;
        parser.expect_key("schema_version")?;
        if parser.parse_u32()? != 1 {
            return parser.fail("unsupported schema snapshot version");
        }
        parser.expect(',')?;
        parser.expect_key("kind")?;
        if parser.parse_string()? != "schema_snapshot" {
            return parser.fail("expected `schema_snapshot` kind");
        }
        parser.expect(',')?;
        parser.expect_key("entries")?;
        parser.expect('[')?;
        let mut entries = Vec::new();
        parser.skip_whitespace();
        if !parser.at(']') {
            loop {
                parser.expect('{')?;
                parser.expect_key("id")?;
                let id = parser.parse_string()?;
                parser.expect(',')?;
                parser.expect_key("kind")?;
                let kind = parser.parse_string()?;
                parser.expect(',')?;
                parser.expect_key("path")?;
                let path = parser.parse_string()?;
                parser.expect(',')?;
                parser.expect_key("physical_name")?;
                let physical_name = parser.parse_string()?;
                parser.expect(',')?;
                parser.expect_key("owner_id")?;
                let owner_id = parser.parse_nullable_string()?;
                parser.expect(',')?;
                parser.expect_key("shape")?;
                let shape = parser.parse_string()?;
                parser.expect('}')?;
                entries.push(SchemaSnapshotEntry {
                    id,
                    kind,
                    path,
                    physical_name,
                    owner_id,
                    shape,
                });
                parser.skip_whitespace();
                if parser.at(',') {
                    parser.position += 1;
                } else {
                    break;
                }
            }
        }
        parser.expect(']')?;
        parser.expect('}')?;
        parser.skip_whitespace();
        if parser.position != parser.characters.len() {
            return parser.fail("unexpected content after schema snapshot");
        }
        let mut ids = BTreeSet::new();
        for entry in &entries {
            if !ids.insert(entry.id.as_str()) {
                return parser.fail("schema snapshot repeats an identity");
            }
        }
        Ok(entries)
    })();
    result.map_err(|_message| {
        Diagnostic::error("MIG_IDENTITY_SNAPSHOT_INVALID")
            .with_note("create snapshots with `jadpo schema snapshot`")
    })
}

fn parse_schema_decision_artifact(source: &str) -> Result<SchemaDecisionArtifact, Diagnostic> {
    let result = (|| {
        let mut parser = RegistryParser::new(source);
        parser.expect('{')?;
        parser.expect_key("schema_version")?;
        if parser.parse_u32()? != 1 {
            return parser.fail("unsupported schema decision artifact version");
        }
        parser.expect(',')?;
        parser.expect_key("kind")?;
        if parser.parse_string()? != "schema_decisions" {
            return parser.fail("expected `schema_decisions` kind");
        }
        parser.expect(',')?;
        parser.expect_key("change_set")?;
        let change_set = parser.parse_string()?;
        parser.expect(',')?;
        parser.expect_key("decisions")?;
        parser.expect('[')?;
        let mut decisions = Vec::new();
        parser.skip_whitespace();
        if !parser.at(']') {
            loop {
                parser.expect('{')?;
                parser.expect_key("identity")?;
                let identity = parser.parse_string()?;
                parser.expect(',')?;
                parser.expect_key("change")?;
                let change = parser.parse_string()?;
                parser.expect(',')?;
                parser.expect_key("strategy")?;
                let strategy = parser.parse_nullable_string()?;
                parser.expect(',')?;
                parser.expect_key("evidence")?;
                parser.expect('[')?;
                let mut evidence = Vec::new();
                parser.skip_whitespace();
                if !parser.at(']') {
                    loop {
                        parser.expect('{')?;
                        parser.expect_key("kind")?;
                        let kind = parser.parse_string()?;
                        parser.expect(',')?;
                        parser.expect_key("value")?;
                        let value = parser.parse_string()?;
                        parser.expect('}')?;
                        evidence.push(DecisionEvidence { kind, value });
                        parser.skip_whitespace();
                        if parser.at(',') {
                            parser.position += 1;
                        } else {
                            break;
                        }
                    }
                }
                parser.expect(']')?;
                parser.expect('}')?;
                decisions.push(SchemaDecision {
                    identity,
                    change,
                    strategy,
                    evidence,
                });
                parser.skip_whitespace();
                if parser.at(',') {
                    parser.position += 1;
                } else {
                    break;
                }
            }
        }
        parser.expect(']')?;
        parser.expect('}')?;
        parser.skip_whitespace();
        if parser.position != parser.characters.len() {
            return parser.fail("unexpected content after schema decision artifact");
        }
        Ok(SchemaDecisionArtifact {
            change_set,
            decisions,
        })
    })();
    result.map_err(|_message| {
        Diagnostic::error("MIG_DECISION_ARTIFACT_INVALID")
            .with_note("start from `jadpo schema decision-template`")
    })
}

fn parse_registry(source: &str) -> Result<Vec<RegistryEntry>, Diagnostic> {
    RegistryParser::new(source).parse().map_err(|_message| {
        Diagnostic::error("MIG_IDENTITY_REGISTRY_INVALID")
            .with_note("revert manual edits and use jadpo schema commands")
    })
}

struct RegistryParser {
    characters: Vec<char>,
    position: usize,
}

impl RegistryParser {
    fn new(source: &str) -> Self {
        Self {
            characters: source.chars().collect(),
            position: 0,
        }
    }

    fn parse(mut self) -> Result<Vec<RegistryEntry>, String> {
        self.expect('{')?;
        self.expect_key("schema_version")?;
        if self.parse_u32()? != 1 {
            return self.fail("unsupported schema identity registry version");
        }
        self.expect(',')?;
        self.expect_key("entries")?;
        self.expect('[')?;
        let mut entries = Vec::new();
        self.skip_whitespace();
        if !self.at(']') {
            loop {
                entries.push(self.parse_entry()?);
                self.skip_whitespace();
                if self.at(',') {
                    self.position += 1;
                } else {
                    break;
                }
            }
        }
        self.expect(']')?;
        self.expect('}')?;
        self.skip_whitespace();
        if self.position != self.characters.len() {
            return self.fail("unexpected content after registry object");
        }
        Ok(entries)
    }

    fn parse_entry(&mut self) -> Result<RegistryEntry, String> {
        self.expect('{')?;
        self.expect_key("id")?;
        let id = self.parse_string()?;
        self.expect(',')?;
        self.expect_key("kind")?;
        let kind = self.parse_string()?;
        self.expect(',')?;
        self.expect_key("path")?;
        let path = self.parse_string()?;
        self.expect(',')?;
        self.expect_key("physical_name")?;
        let physical_name = self.parse_string()?;
        self.expect(',')?;
        self.expect_key("owner_id")?;
        let owner_id = self.parse_nullable_string()?;
        self.expect(',')?;
        self.expect_key("previous_paths")?;
        let previous_paths = self.parse_string_array()?;
        self.expect('}')?;
        Ok(RegistryEntry {
            id,
            kind,
            path,
            physical_name,
            owner_id,
            previous_paths,
        })
    }

    fn expect_key(&mut self, expected: &str) -> Result<(), String> {
        let actual = self.parse_string()?;
        if actual != expected {
            return self.fail(&format!(
                "expected registry property `{expected}`, found `{actual}`"
            ));
        }
        self.expect(':')
    }

    fn parse_nullable_string(&mut self) -> Result<Option<String>, String> {
        self.skip_whitespace();
        if self.remaining_starts_with("null") {
            self.position += 4;
            Ok(None)
        } else {
            self.parse_string().map(Some)
        }
    }

    fn parse_string_array(&mut self) -> Result<Vec<String>, String> {
        self.expect('[')?;
        let mut values = Vec::new();
        self.skip_whitespace();
        if !self.at(']') {
            loop {
                values.push(self.parse_string()?);
                self.skip_whitespace();
                if self.at(',') {
                    self.position += 1;
                } else {
                    break;
                }
            }
        }
        self.expect(']')?;
        Ok(values)
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.expect('"')?;
        let mut value = String::new();
        while self.position < self.characters.len() {
            let character = self.characters[self.position];
            self.position += 1;
            match character {
                '"' => return Ok(value),
                '\\' => {
                    let escaped = self
                        .characters
                        .get(self.position)
                        .copied()
                        .ok_or_else(|| self.error("unterminated string escape"))?;
                    self.position += 1;
                    match escaped {
                        '"' | '\\' | '/' => value.push(escaped),
                        'b' => value.push('\u{0008}'),
                        'f' => value.push('\u{000c}'),
                        'n' => value.push('\n'),
                        'r' => value.push('\r'),
                        't' => value.push('\t'),
                        'u' => value.push(self.parse_unicode_escape()?),
                        _ => return self.fail("invalid string escape in registry"),
                    }
                }
                character if character.is_control() => {
                    return self.fail("unescaped control character in registry string");
                }
                character => value.push(character),
            }
        }
        self.fail("unterminated registry string")
    }

    fn parse_unicode_escape(&mut self) -> Result<char, String> {
        let mut value = 0_u32;
        for _ in 0..4 {
            let digit = self
                .characters
                .get(self.position)
                .and_then(|character| character.to_digit(16))
                .ok_or_else(|| self.error("invalid unicode escape in registry string"))?;
            self.position += 1;
            value = value * 16 + digit;
        }
        char::from_u32(value).ok_or_else(|| self.error("invalid unicode scalar in registry string"))
    }

    fn parse_u32(&mut self) -> Result<u32, String> {
        self.skip_whitespace();
        let start = self.position;
        while self
            .characters
            .get(self.position)
            .is_some_and(char::is_ascii_digit)
        {
            self.position += 1;
        }
        if start == self.position {
            return self.fail("expected registry version number");
        }
        self.characters[start..self.position]
            .iter()
            .collect::<String>()
            .parse()
            .map_err(|_| self.error("invalid registry version number"))
    }

    fn expect(&mut self, expected: char) -> Result<(), String> {
        self.skip_whitespace();
        if self.at(expected) {
            self.position += 1;
            Ok(())
        } else {
            self.fail(&format!(
                "expected `{expected}` in schema identity registry"
            ))
        }
    }

    fn at(&self, expected: char) -> bool {
        self.characters.get(self.position).copied() == Some(expected)
    }

    fn remaining_starts_with(&self, expected: &str) -> bool {
        self.characters[self.position..]
            .iter()
            .take(expected.len())
            .copied()
            .eq(expected.chars())
    }

    fn skip_whitespace(&mut self) {
        while self
            .characters
            .get(self.position)
            .is_some_and(|character| character.is_whitespace())
        {
            self.position += 1;
        }
    }

    fn error(&self, message: &str) -> String {
        format!("{message} at character {}", self.position)
    }

    fn fail<T>(&self, message: &str) -> Result<T, String> {
        Err(self.error(message))
    }
}

fn identity(kind: &str, initial_path: &str) -> String {
    format!("schema-v1/{kind}/{initial_path}")
}

fn type_name(reference: &TypeReference) -> String {
    let mut name = reference
        .path
        .iter()
        .map(|part| part.text.as_str())
        .collect::<Vec<_>>()
        .join(".");
    if !reference.arguments.is_empty() {
        name.push('<');
        name.push_str(
            &reference
                .arguments
                .iter()
                .map(type_name)
                .collect::<Vec<_>>()
                .join(","),
        );
        name.push('>');
    }
    if reference.nullable {
        name.push('?');
    }
    name
}

const fn constraint_name(kind: ConstraintKind) -> &'static str {
    match kind {
        ConstraintKind::Min => "min",
        ConstraintKind::Max => "max",
        ConstraintKind::MinLength => "min_length",
        ConstraintKind::MaxLength => "max_length",
        ConstraintKind::Pattern => "pattern",
        ConstraintKind::Format => "format",
    }
}

const fn reference_delete_name(action: ReferenceDeleteAction) -> &'static str {
    match action {
        ReferenceDeleteAction::Restrict => "restrict",
        ReferenceDeleteAction::Cascade => "cascade",
        ReferenceDeleteAction::SetNull => "set_null",
    }
}

fn snake_case(value: &str) -> String {
    let mut result = String::new();
    for (index, character) in value.chars().enumerate() {
        if character.is_ascii_uppercase() {
            if index > 0 {
                result.push('_');
            }
            result.push(character.to_ascii_lowercase());
        } else {
            result.push(character);
        }
    }
    result
}

fn json_string(value: &str) -> String {
    let mut output = String::from("\"");
    for character in value.chars() {
        match character {
            '\"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                output.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => output.push(character),
        }
    }
    output.push('\"');
    output
}

#[cfg(test)]
mod tests {
    use super::{
        diff_schema_identities, initialize_schema_identities, register_schema_additions,
        rename_schema_identity, snapshot_schema_identities, validate_schema_decisions,
        validate_schema_identities, write_schema_decision_template, write_schema_migration_plan,
        write_schema_migration_sql_review,
    };
    use crate::analyze_project;
    use std::fs;

    #[test]
    fn initializes_a_deterministic_non_overwriting_registry() {
        let root =
            std::env::temp_dir().join(format!("jadpo-schema-identities-{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).expect("stale test directory should be removable");
        }
        fs::create_dir_all(&root).expect("test project should be created");
        fs::write(
            root.join("app.jadpo"),
            "entity User { id: Uuid identity email: Text unique }\nentity Todo { id: Uuid identity owner_id: User.id references User.id as owner on_delete cascade }\n",
        )
        .expect("test source should be written");
        let analyzed = analyze_project(&root).expect("test project should analyze");
        assert!(analyzed.semantics.diagnostics.is_empty());

        let path = initialize_schema_identities(&root, &analyzed)
            .expect("registry should initialize once");
        let contents = fs::read_to_string(path).expect("registry should be readable");
        assert!(contents.contains("\"id\":\"schema-v1/entity/Todo\""));
        assert!(contents.contains("\"id\":\"schema-v1/field/Todo.owner_id\""));
        assert!(contents.contains("\"physical_name\":\"todo_owner_id_idx\""));
        let snapshot = root.join("initial.schema.identities.json");
        snapshot_schema_identities(&root, &analyzed, &snapshot)
            .expect("initial snapshot should be written once");
        assert_eq!(
            snapshot_schema_identities(&root, &analyzed, &snapshot)
                .expect_err("snapshot overwrite must fail")
                .code,
            "MIG_IDENTITY_SNAPSHOT_EXISTS"
        );
        assert_eq!(
            initialize_schema_identities(&root, &analyzed)
                .expect_err("registry overwrite must fail")
                .code,
            "MIG_IDENTITY_REGISTRY_EXISTS"
        );

        fs::write(
            root.join("app.jadpo"),
            "entity User { id: Uuid identity email: Text unique }\nentity Task { id: Uuid identity owner_id: User.id references User.id as owner on_delete cascade }\n",
        )
        .expect("entity rename should be written to source");
        let renamed = analyze_project(&root).expect("renamed project should analyze");
        assert_eq!(
            validate_schema_identities(&root, &renamed)
                .expect_err("manual rename must produce drift")
                .code,
            "MIG_IDENTITY_REGISTRY_DRIFT"
        );
        rename_schema_identity(&root, &renamed, "entity", "Todo", "Task")
            .expect("explicit entity rename should preserve identity");
        validate_schema_identities(&root, &renamed)
            .expect("renamed entity registry should validate");
        let contents = fs::read_to_string(root.join("schema.identities.json"))
            .expect("renamed registry should be readable");
        assert!(contents.contains("\"id\":\"schema-v1/entity/Todo\""));
        assert!(contents.contains("\"path\":\"Task\""));
        assert!(contents.contains("\"physical_name\":\"todo\""));
        assert!(contents.contains("\"previous_paths\":[\"Todo\"]"));

        fs::write(
            root.join("app.jadpo"),
            "entity User { id: Uuid identity email: Text unique }\nentity Task { id: Uuid identity assignee_id: User.id references User.id as owner on_delete cascade }\n",
        )
        .expect("field rename should be written to source");
        let renamed = analyze_project(&root).expect("field-renamed project should analyze");
        rename_schema_identity(
            &root,
            &renamed,
            "field",
            "Task.owner_id",
            "Task.assignee_id",
        )
        .expect("explicit field rename should preserve identity");
        validate_schema_identities(&root, &renamed)
            .expect("renamed field registry should validate");
        let contents = fs::read_to_string(root.join("schema.identities.json"))
            .expect("field-renamed registry should be readable");
        assert!(contents.contains("\"id\":\"schema-v1/field/Todo.owner_id\""));
        assert!(contents.contains("\"path\":\"Task.assignee_id\""));
        assert!(contents.contains("\"physical_name\":\"owner_id\""));
        assert!(contents.contains("\"previous_paths\":[\"Todo.owner_id\",\"Task.owner_id\"]"));

        fs::write(
            root.join("app.jadpo"),
            "entity User { id: Uuid identity email: Text unique }\nentity Task { id: Uuid identity assignee_id: User.id references User.id as owner on_delete cascade title: Text }\n",
        )
        .expect("additive field should be written to source");
        let added = analyze_project(&root).expect("additive project should analyze");
        assert_eq!(
            validate_schema_identities(&root, &added)
                .expect_err("unregistered addition must produce drift")
                .code,
            "MIG_IDENTITY_REGISTRY_DRIFT"
        );
        let (_, count) = register_schema_additions(&root, &added)
            .expect("additive identity should register explicitly");
        assert_eq!(count, 1);
        validate_schema_identities(&root, &added)
            .expect("registry with explicit addition should validate");
        let change_set = diff_schema_identities(&root, &added, &snapshot)
            .expect("identity change set should compare with the initial snapshot");
        assert!(change_set.contains("\"migration_plan\":false"));
        assert!(change_set
            .contains("\"identity\":\"schema-v1/entity/Todo\",\"change\":\"logical_rename\""));
        assert!(change_set.contains(
            "\"identity\":\"schema-v1/field/Task.title\",\"change\":\"added_required_field\""
        ));
        assert!(change_set.contains(
            "\"decision_requirement\":{\"kind\":\"existing_data\",\"status\":\"unresolved\""
        ));
        assert!(change_set.contains("\"name\":\"backfill\",\"requires\":[\"typed_expression\"]"));
        assert!(!change_set.contains("\"change\":\"physical_rename\""));

        let decisions = root.join("schema.decisions.json");
        let (_, decision_count) =
            write_schema_decision_template(&root, &added, &snapshot, &decisions)
                .expect("decision template should bind to the exact change set");
        assert_eq!(decision_count, 1);
        assert_eq!(
            write_schema_decision_template(&root, &added, &snapshot, &decisions)
                .expect_err("decision templates must not overwrite authored work")
                .code,
            "MIG_DECISION_ARTIFACT_EXISTS"
        );
        assert_eq!(
            validate_schema_decisions(&root, &added, &snapshot, &decisions)
                .expect_err("an unresolved template must not validate")
                .code,
            "MIG_DECISION_UNRESOLVED"
        );
        let resolved = root.join("schema.decisions.resolved.json");
        let resolved_source = fs::read_to_string(&decisions)
            .expect("decision template should be readable")
            .replace(
                "\"strategy\":null,\"evidence\":[]",
                "\"strategy\":\"backfill\",\"evidence\":[{\"kind\":\"typed_expression\",\"value\":\"literal(untitled)\"}]",
            );
        fs::write(&resolved, resolved_source).expect("resolved decisions should be written");
        let validation = validate_schema_decisions(&root, &added, &snapshot, &resolved)
            .expect("complete bound decisions should validate");
        assert!(validation.contains("\"change_set_bound\":true"));
        assert!(validation.contains("\"complete\":true"));
        assert!(validation.contains("\"migration_plan\":false"));
        let postgres_plan = root.join("migration.postgres.plan.json");
        let (_, steps, irreversible) = write_schema_migration_plan(
            &root,
            &added,
            &snapshot,
            &resolved,
            "postgres",
            &postgres_plan,
        )
        .expect("validated decisions should produce a review plan");
        assert!(steps > 1);
        assert_eq!(irreversible, 0);
        let plan = fs::read_to_string(&postgres_plan).expect("plan should be readable");
        assert!(plan.contains("\"migration_plan\":true"));
        assert!(plan.contains("\"adapter\":\"postgres\""));
        assert!(plan.contains("\"executable\":false,\"sql_generated\":false"));
        assert!(plan.contains("\"operation\":\"add_nullable_backfill_enforce_required\""));
        assert!(plan.contains("\"rollback\":{\"kind\":\"conditional\""));
        let postgres_sql = root.join("migration.postgres.sql-review.json");
        let (_, forward, rollback) = write_schema_migration_sql_review(
            &root,
            &added,
            &snapshot,
            &resolved,
            "postgres",
            &postgres_sql,
        )
        .expect("supported additive migration should produce reviewed SQL");
        assert_eq!(forward, 3);
        assert_eq!(rollback, 1);
        let sql = fs::read_to_string(postgres_sql).expect("SQL review should be readable");
        assert!(sql.contains("\"executable\":false,\"review_required\":true"));
        assert!(sql.contains("ALTER TABLE \\\"todo\\\" ADD COLUMN \\\"title\\\" TEXT;"));
        assert!(sql.contains(
            "UPDATE \\\"todo\\\" SET \\\"title\\\" = 'untitled' WHERE \\\"title\\\" IS NULL;"
        ));
        assert!(sql.contains("ALTER TABLE \\\"todo\\\" ALTER COLUMN \\\"title\\\" SET NOT NULL;"));
        assert!(sql.contains("ALTER TABLE \\\"todo\\\" DROP COLUMN \\\"title\\\";"));

        let before_shape_change = root.join("before-shape-change.json");
        snapshot_schema_identities(&root, &added, &before_shape_change)
            .expect("shape baseline should be written");
        fs::write(
            root.join("app.jadpo"),
            "entity User { id: Uuid identity email: Text unique }\nentity Task { id: Uuid identity assignee_id: User.id references User.id as owner on_delete cascade title: Text? }\n",
        )
        .expect("nullable shape change should be written to source");
        let shape_changed = analyze_project(&root).expect("shape-changed project should analyze");
        validate_schema_identities(&root, &shape_changed)
            .expect("shape changes preserve registry identity agreement");
        let change_set = diff_schema_identities(&root, &shape_changed, &before_shape_change)
            .expect("shape diff should compare checked snapshots");
        assert!(change_set.contains(
            "\"identity\":\"schema-v1/field/Task.title\",\"change\":\"field_nullability_widened\""
        ));
        assert!(change_set.contains("\"disposition\":\"requires_schema_plan\""));
        assert_eq!(
            validate_schema_decisions(&root, &shape_changed, &snapshot, &resolved)
                .expect_err("decisions bound to an older current shape must be stale")
                .code,
            "MIG_DECISION_CHANGE_SET_STALE"
        );

        let before_lifecycle_change = root.join("before-lifecycle-change.json");
        snapshot_schema_identities(&root, &shape_changed, &before_lifecycle_change)
            .expect("lifecycle baseline should be written");
        fs::write(
            root.join("app.jadpo"),
            "entity User { id: Uuid identity email: Text unique }\nentity Task { id: Uuid identity assignee_id: User.id references User.id as owner on_delete restrict title: Text? }\n",
        )
        .expect("relationship lifecycle change should be written to source");
        let lifecycle_changed =
            analyze_project(&root).expect("lifecycle-changed project should analyze");
        validate_schema_identities(&root, &lifecycle_changed)
            .expect("lifecycle changes preserve registry identity agreement");
        let change_set =
            diff_schema_identities(&root, &lifecycle_changed, &before_lifecycle_change)
                .expect("lifecycle diff should compare checked snapshots");
        assert!(change_set.contains("\"change\":\"relationship_lifecycle_changed\""));
        assert!(change_set.contains("\"disposition\":\"requires_lifecycle_decision\""));
        assert!(change_set.contains(
            "\"decision_requirement\":{\"kind\":\"lifecycle\",\"status\":\"unresolved\""
        ));
        assert!(change_set
            .contains("\"name\":\"destructive_remove\",\"requires\":[\"approval_reference\"]"));
        let lifecycle_decisions = root.join("lifecycle.decisions.json");
        write_schema_decision_template(
            &root,
            &lifecycle_changed,
            &before_lifecycle_change,
            &lifecycle_decisions,
        )
        .expect("lifecycle decisions should have a template");
        let incomplete_source = fs::read_to_string(&lifecycle_decisions)
            .expect("lifecycle template should be readable")
            .replace(
                "\"strategy\":null,\"evidence\":[]",
                "\"strategy\":\"destructive_remove\",\"evidence\":[]",
            );
        let incomplete = root.join("lifecycle.decisions.incomplete.json");
        fs::write(&incomplete, incomplete_source)
            .expect("incomplete lifecycle decision should be written");
        assert_eq!(
            validate_schema_decisions(
                &root,
                &lifecycle_changed,
                &before_lifecycle_change,
                &incomplete,
            )
            .expect_err("destructive removal without approval evidence must fail")
            .code,
            "MIG_DECISION_EVIDENCE_MISSING"
        );
        let approved_source = fs::read_to_string(&lifecycle_decisions)
            .expect("lifecycle template should be readable")
            .replace(
                "\"strategy\":null,\"evidence\":[]",
                "\"strategy\":\"destructive_remove\",\"evidence\":[{\"kind\":\"approval_reference\",\"value\":\"review-42\"}]",
            );
        let approved = root.join("lifecycle.decisions.approved.json");
        fs::write(&approved, approved_source)
            .expect("approved lifecycle decision should be written");
        let sqlite_plan = root.join("migration.sqlite.plan.json");
        let (_, _, irreversible) = write_schema_migration_plan(
            &root,
            &lifecycle_changed,
            &before_lifecycle_change,
            &approved,
            "sqlite",
            &sqlite_plan,
        )
        .expect("approved lifecycle decision should produce a review plan");
        assert_eq!(irreversible, 1);
        let plan = fs::read_to_string(sqlite_plan).expect("SQLite plan should be readable");
        assert!(plan.contains("\"operation\":\"rebuild_table_foreign_key\""));
        assert!(plan.contains("\"irreversible\":true"));
        assert!(plan.contains("\"irreversible_steps\":[1]"));

        fs::write(
            root.join("app.jadpo"),
            "entity User { id: Uuid identity email: Text unique }\nentity Task { id: Uuid identity assignee_id: User.id references User.id as owner on_delete cascade }\n",
        )
        .expect("field removal should be written to source");
        let removed = analyze_project(&root).expect("removal project should analyze");
        assert_eq!(
            register_schema_additions(&root, &removed)
                .expect_err("addition command must not absorb a removal")
                .code,
            "MIG_IDENTITY_ADDITIONS_HAVE_REMOVAL"
        );
        fs::remove_dir_all(root).expect("test project should be removable");
    }

    #[test]
    fn emits_checked_sqlite_rebuild_for_one_required_field() {
        let root =
            std::env::temp_dir().join(format!("jadpo-sqlite-migration-{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).expect("stale test directory should be removable");
        }
        fs::create_dir_all(&root).expect("test project should be created");
        fs::write(
            root.join("app.jadpo"),
            "entity Parent {\n    id: Uuid identity\n}\nentity Item {\n    id: Uuid identity\n    parent_id: Parent.id references Parent.id on_delete cascade\n    label: Text index\n    constraint parent_label: unique(parent_id, label)\n}\n",
        )
        .expect("baseline source should be written");
        let baseline = analyze_project(&root).expect("baseline should analyze");
        initialize_schema_identities(&root, &baseline).expect("registry should initialize");
        let snapshot = root.join("baseline.schema.json");
        snapshot_schema_identities(&root, &baseline, &snapshot)
            .expect("baseline snapshot should be written");

        fs::write(
            root.join("app.jadpo"),
            "entity Parent {\n    id: Uuid identity\n}\nentity Item {\n    id: Uuid identity\n    parent_id: Parent.id references Parent.id on_delete cascade\n    label: Text index\n    title: Text\n    constraint parent_label: unique(parent_id, label)\n}\n",
        )
        .expect("required field should be added");
        let added = analyze_project(&root).expect("addition should analyze");
        register_schema_additions(&root, &added).expect("field identity should register");
        let decisions = root.join("schema.decisions.json");
        write_schema_decision_template(&root, &added, &snapshot, &decisions)
            .expect("decision template should be written");
        let resolved = root.join("schema.decisions.resolved.json");
        let resolved_source = fs::read_to_string(&decisions)
            .expect("decision template should be readable")
            .replace(
                "\"strategy\":null,\"evidence\":[]",
                "\"strategy\":\"backfill\",\"evidence\":[{\"kind\":\"typed_expression\",\"value\":\"literal(untitled)\"}]",
            );
        fs::write(&resolved, resolved_source).expect("resolved decision should be written");

        let output = root.join("migration.sqlite.sql-review.json");
        let (_, forward, rollback) = write_schema_migration_sql_review(
            &root, &added, &snapshot, &resolved, "sqlite", &output,
        )
        .expect("simple required addition should produce SQLite rebuild SQL");
        assert_eq!(forward, 11);
        assert_eq!(rollback, 11);
        let sql = fs::read_to_string(output).expect("SQL review should be readable");
        assert!(sql.contains("CREATE TABLE \\\"__migration_new_item\\\""));
        assert!(sql.contains(
            "INSERT INTO \\\"__migration_new_item\\\" (\\\"id\\\", \\\"parent_id\\\", \\\"label\\\", \\\"title\\\") SELECT \\\"id\\\", \\\"parent_id\\\", \\\"label\\\", 'untitled' FROM \\\"item\\\";"
        ));
        assert!(sql.contains("ALTER TABLE \\\"__migration_new_item\\\" RENAME TO \\\"item\\\";"));
        assert!(sql.contains("CONSTRAINT \\\"item_parent_id_fk\\\" FOREIGN KEY (\\\"parent_id\\\") REFERENCES \\\"parent\\\" (\\\"id\\\") ON DELETE CASCADE"));
        assert!(sql.contains(
            "CONSTRAINT \\\"item_parent_label_unique\\\" UNIQUE (\\\"parent_id\\\", \\\"label\\\")"
        ));
        assert!(sql.contains(
            "CREATE INDEX \\\"item_parent_id_idx\\\" ON \\\"item\\\" (\\\"parent_id\\\");"
        ));
        assert!(
            sql.contains("CREATE INDEX \\\"item_label_idx\\\" ON \\\"item\\\" (\\\"label\\\");")
        );
        assert!(sql.contains("CREATE TABLE \\\"__migration_rollback_item\\\""));
        assert!(sql.contains("PRAGMA foreign_key_check;"));
        fs::remove_dir_all(root).expect("test project should be removable");
    }

    #[test]
    fn coalesces_compatible_sqlite_field_additions_into_one_rebuild() {
        let root = std::env::temp_dir().join(format!(
            "jadpo-sqlite-multi-migration-{}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).expect("stale test directory should be removable");
        }
        fs::create_dir_all(&root).expect("test project should be created");
        fs::write(
            root.join("app.jadpo"),
            "entity Item {\n    id: Uuid identity\n}\n",
        )
        .expect("baseline source should be written");
        let baseline = analyze_project(&root).expect("baseline should analyze");
        initialize_schema_identities(&root, &baseline).expect("registry should initialize");
        let snapshot = root.join("baseline.schema.json");
        snapshot_schema_identities(&root, &baseline, &snapshot)
            .expect("baseline snapshot should be written");

        fs::write(
            root.join("app.jadpo"),
            "entity Item {\n    id: Uuid identity\n    title: Text\n    detail: Text?\n    category: Text\n}\n",
        )
        .expect("field additions should be written");
        let added = analyze_project(&root).expect("additions should analyze");
        register_schema_additions(&root, &added).expect("field identities should register");
        let decisions = root.join("schema.decisions.json");
        write_schema_decision_template(&root, &added, &snapshot, &decisions)
            .expect("decision template should be written");
        let resolved = root.join("schema.decisions.resolved.json");
        let resolved_source = fs::read_to_string(&decisions)
            .expect("decision template should be readable")
            .replace(
                "\"strategy\":null,\"evidence\":[]",
                "\"strategy\":\"backfill\",\"evidence\":[{\"kind\":\"typed_expression\",\"value\":\"literal(seed)\"}]",
            );
        fs::write(&resolved, resolved_source).expect("resolved decision should be written");

        let output = root.join("migration.sqlite.sql-review.json");
        let (_, forward, rollback) = write_schema_migration_sql_review(
            &root, &added, &snapshot, &resolved, "sqlite", &output,
        )
        .expect("compatible additions should produce one SQLite rebuild");
        assert_eq!(forward, 9);
        assert_eq!(rollback, 9);
        let sql = fs::read_to_string(output).expect("SQL review should be readable");
        assert_eq!(
            sql.matches("CREATE TABLE \\\"__migration_new_item\\\"")
                .count(),
            1
        );
        assert!(sql.contains(
            "INSERT INTO \\\"__migration_new_item\\\" (\\\"id\\\", \\\"title\\\", \\\"detail\\\", \\\"category\\\") SELECT \\\"id\\\", 'seed', NULL, 'seed' FROM \\\"item\\\";"
        ));
        assert!(sql.contains(
            "INSERT INTO \\\"__migration_rollback_item\\\" (\\\"id\\\") SELECT \\\"id\\\" FROM \\\"item\\\";"
        ));
        fs::remove_dir_all(root).expect("test project should be removable");
    }

    #[test]
    fn emits_reversible_sql_for_required_to_nullable_widening() {
        let root =
            std::env::temp_dir().join(format!("jadpo-nullability-widening-{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).expect("stale test directory should be removable");
        }
        fs::create_dir_all(&root).expect("test project should be created");
        fs::write(
            root.join("app.jadpo"),
            "entity Item {\n    id: Uuid identity\n    title: Text\n}\n",
        )
        .expect("baseline source should be written");
        let baseline = analyze_project(&root).expect("baseline should analyze");
        initialize_schema_identities(&root, &baseline).expect("registry should initialize");
        let snapshot = root.join("baseline.schema.json");
        snapshot_schema_identities(&root, &baseline, &snapshot)
            .expect("baseline snapshot should be written");

        fs::write(
            root.join("app.jadpo"),
            "entity Item {\n    id: Uuid identity\n    title: Text?\n}\n",
        )
        .expect("nullable widening should be written");
        let widened = analyze_project(&root).expect("widened project should analyze");
        validate_schema_identities(&root, &widened)
            .expect("nullability widening should preserve identities");
        let change_set = diff_schema_identities(&root, &widened, &snapshot)
            .expect("nullability widening should produce a change set");
        assert!(change_set.contains("\"change\":\"field_nullability_widened\""));
        assert!(change_set.contains("\"disposition\":\"requires_schema_plan\""));
        assert!(change_set.contains("\"decision_requirement\":null"));

        let decisions = root.join("schema.decisions.json");
        let (_, decision_count) =
            write_schema_decision_template(&root, &widened, &snapshot, &decisions)
                .expect("safe widening should produce an empty decision artifact");
        assert_eq!(decision_count, 0);

        let postgres_output = root.join("migration.postgres.sql-review.json");
        let (_, forward, rollback) = write_schema_migration_sql_review(
            &root,
            &widened,
            &snapshot,
            &decisions,
            "postgres",
            &postgres_output,
        )
        .expect("PostgreSQL nullability widening should produce reviewed SQL");
        assert_eq!((forward, rollback), (1, 1));
        let postgres =
            fs::read_to_string(postgres_output).expect("PostgreSQL review should be readable");
        assert!(
            postgres.contains("ALTER TABLE \\\"item\\\" ALTER COLUMN \\\"title\\\" DROP NOT NULL;")
        );
        assert!(
            postgres.contains("ALTER TABLE \\\"item\\\" ALTER COLUMN \\\"title\\\" SET NOT NULL;")
        );

        let sqlite_output = root.join("migration.sqlite.sql-review.json");
        let (_, forward, rollback) = write_schema_migration_sql_review(
            &root,
            &widened,
            &snapshot,
            &decisions,
            "sqlite",
            &sqlite_output,
        )
        .expect("SQLite nullability widening should produce reviewed rebuild SQL");
        assert_eq!((forward, rollback), (9, 9));
        let sqlite = fs::read_to_string(sqlite_output).expect("SQLite review should be readable");
        assert!(sqlite.contains(
            "CREATE TABLE \\\"__migration_nullable_item\\\" (\\\"id\\\" TEXT NOT NULL, \\\"title\\\" TEXT, CONSTRAINT"
        ));
        assert!(sqlite.contains(
            "CREATE TABLE \\\"__migration_rollback_item\\\" (\\\"id\\\" TEXT NOT NULL, \\\"title\\\" TEXT NOT NULL, CONSTRAINT"
        ));
        fs::remove_dir_all(root).expect("test project should be removable");
    }

    #[test]
    fn requires_checked_not_null_evidence_for_nullability_narrowing() {
        let root = std::env::temp_dir().join(format!(
            "jadpo-nullability-narrowing-{}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).expect("stale test directory should be removable");
        }
        fs::create_dir_all(&root).expect("test project should be created");
        fs::write(
            root.join("app.jadpo"),
            "entity Item {\n    id: Uuid identity\n    title: Text?\n}\n",
        )
        .expect("baseline source should be written");
        let baseline = analyze_project(&root).expect("baseline should analyze");
        initialize_schema_identities(&root, &baseline).expect("registry should initialize");
        let snapshot = root.join("baseline.schema.json");
        snapshot_schema_identities(&root, &baseline, &snapshot)
            .expect("baseline snapshot should be written");

        fs::write(
            root.join("app.jadpo"),
            "entity Item {\n    id: Uuid identity\n    title: Text\n}\n",
        )
        .expect("required narrowing should be written");
        let narrowed = analyze_project(&root).expect("narrowed project should analyze");
        validate_schema_identities(&root, &narrowed)
            .expect("nullability narrowing should preserve identities");
        let change_set = diff_schema_identities(&root, &narrowed, &snapshot)
            .expect("nullability narrowing should produce a change set");
        assert!(change_set.contains("\"change\":\"field_nullability_narrowed\""));
        assert!(change_set.contains("\"disposition\":\"requires_existing_data_decision\""));
        assert!(change_set
            .contains("\"name\":\"validate_existing\",\"requires\":[\"typed_predicate\"]"));

        let decisions = root.join("schema.decisions.json");
        let (_, decision_count) =
            write_schema_decision_template(&root, &narrowed, &snapshot, &decisions)
                .expect("narrowing should produce one decision");
        assert_eq!(decision_count, 1);
        let template = fs::read_to_string(&decisions).expect("template should be readable");
        let invalid = root.join("schema.decisions.invalid.json");
        fs::write(
            &invalid,
            template.replace(
                "\"strategy\":null,\"evidence\":[]",
                "\"strategy\":\"validate_existing\",\"evidence\":[{\"kind\":\"typed_predicate\",\"value\":\"trusted_rows\"}]",
            ),
        )
        .expect("invalid decision should be written");
        assert_eq!(
            write_schema_migration_sql_review(
                &root,
                &narrowed,
                &snapshot,
                &invalid,
                "postgres",
                &root.join("migration.invalid.sql-review.json"),
            )
            .expect_err("unrecognized evidence must not produce SQL")
            .code,
            "MIG_SQL_PREDICATE_UNSUPPORTED"
        );

        let resolved = root.join("schema.decisions.resolved.json");
        fs::write(
            &resolved,
            template.replace(
                "\"strategy\":null,\"evidence\":[]",
                "\"strategy\":\"validate_existing\",\"evidence\":[{\"kind\":\"typed_predicate\",\"value\":\"not_null\"}]",
            ),
        )
        .expect("resolved decision should be written");

        let postgres_output = root.join("migration.postgres.sql-review.json");
        let (_, forward, rollback) = write_schema_migration_sql_review(
            &root,
            &narrowed,
            &snapshot,
            &resolved,
            "postgres",
            &postgres_output,
        )
        .expect("PostgreSQL narrowing should produce reviewed SQL");
        assert_eq!((forward, rollback), (2, 1));
        let postgres =
            fs::read_to_string(postgres_output).expect("PostgreSQL review should be readable");
        assert!(postgres.contains(
            "SELECT COUNT(*) AS \\\"migration_null_violation_count\\\" FROM \\\"item\\\" WHERE \\\"title\\\" IS NULL;"
        ));
        assert!(
            postgres.contains("ALTER TABLE \\\"item\\\" ALTER COLUMN \\\"title\\\" SET NOT NULL;")
        );
        assert!(
            postgres.contains("ALTER TABLE \\\"item\\\" ALTER COLUMN \\\"title\\\" DROP NOT NULL;")
        );

        let sqlite_output = root.join("migration.sqlite.sql-review.json");
        let (_, forward, rollback) = write_schema_migration_sql_review(
            &root,
            &narrowed,
            &snapshot,
            &resolved,
            "sqlite",
            &sqlite_output,
        )
        .expect("SQLite narrowing should produce reviewed rebuild SQL");
        assert_eq!((forward, rollback), (9, 9));
        let sqlite = fs::read_to_string(sqlite_output).expect("SQLite review should be readable");
        assert!(sqlite.contains(
            "CREATE TABLE \\\"__migration_required_item\\\" (\\\"id\\\" TEXT NOT NULL, \\\"title\\\" TEXT NOT NULL, CONSTRAINT"
        ));
        assert!(sqlite.contains(
            "CREATE TABLE \\\"__migration_rollback_item\\\" (\\\"id\\\" TEXT NOT NULL, \\\"title\\\" TEXT, CONSTRAINT"
        ));
        fs::remove_dir_all(root).expect("test project should be removable");
    }
}
